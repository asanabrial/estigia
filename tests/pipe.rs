// The same exception the library makes for its own tests, restated because an
// integration test is a separate crate and the crate-level allow does not reach
// it: an assertion that panics is the assertion working, and threading `?`
// through a test buys nothing and costs the reader.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The shared fixture the pipe suite's modules reach through `use super::*`,
//! and the module declarations that bring each module's tests into the `pipe`
//! test target.
//!
//! The suite began as one file whose first test was a process-level one: the
//! defect was a process-level one, because `println!` panics when the pipe
//! closes and no unit test that captures output can see it. `estigia status |
//! head` is the most ordinary thing a person does with a command that prints a
//! list, and it ended in a backtrace. Running every verb as a process then
//! found four defects the whole suite had not, in one afternoon, so the whole
//! verb surface is exercised that way.
//!
//! The tests now live in the modules beside this file, one per block, because a
//! single 15,542-line file is more than a reviewer can hold. What stays here is
//! what more than one module needs: the `fake_process`-backed rig, the tracker
//! and git helpers, and the constants the moved tests share.

#[path = "pipe/cli_process.rs"]
mod cli_process;
#[path = "pipe/config.rs"]
mod config;
#[path = "pipe/filing.rs"]
mod filing;
#[path = "support/fixture_flags.rs"]
mod fixture_flags;
#[path = "pipe/gate_hook.rs"]
mod gate_hook;
#[path = "pipe/part_04.rs"]
mod part_04;
#[path = "pipe/part_05.rs"]
mod part_05;
#[path = "pipe/part_06.rs"]
mod part_06;
#[path = "pipe/part_07.rs"]
mod part_07;
#[path = "pipe/part_08.rs"]
mod part_08;
#[path = "pipe/part_09.rs"]
mod part_09;
#[path = "pipe/part_10.rs"]
mod part_10;
#[path = "pipe/release.rs"]
mod release;
#[path = "pipe/review_family.rs"]
mod review_family;
#[cfg(unix)]
#[path = "pipe/review_fold.rs"]
mod review_fold;
#[path = "pipe/review_job.rs"]
mod review_job;
#[path = "pipe/sources.rs"]
mod sources;
#[path = "pipe/status_doctor.rs"]
mod status_doctor;
#[path = "pipe/sync.rs"]
mod sync;

use std::io::Read;
use std::process::{Command, Stdio};

/// The binary under test, as Cargo built it.
fn estigia() -> &'static str {
    env!("CARGO_BIN_EXE_estigia")
}

fn conforming_body() -> String {
    include_str!("../skill/assets/analyst-issue-template.md")
        .split("```markdown\n")
        .nth(1)
        .and_then(|rest| rest.split("\n```").next())
        .expect("the template carries a fenced markdown block")
        .lines()
        .filter(|line| !line.starts_with("<!-- issue-flow"))
        .collect::<Vec<_>>()
        .join("\n")
        .replacen("## Problem\n", "## Problem\nA probe.\n", 1)
}

/// Every spawn of the binary in this file starts here, so a `CLAUDE_CONFIG_DIR`,
/// `DSH_HOME` or `PI_CODING_AGENT_DIR` exported by the developer's shell never
/// reaches a child: with any of them inherited, `setup` resolves that adapter's files under the
/// developer's real relocated root and writes there.
fn estigia_command() -> Command {
    let mut command = Command::new(estigia());
    command.env_remove("CLAUDE_CONFIG_DIR");
    command.env_remove("DSH_HOME");
    command.env_remove("PI_CODING_AGENT_DIR");
    command.env_remove("HERDR_PANE_ID");
    command
}

/// Guards `std::env::vars()` inside [`run_with_path_locked`] against the one
/// test in this binary that mutates the process environment, without which
/// `an_operators_gh_repo_never_reaches_a_spawned_gh`'s `set_var`/`remove_var`
/// around `GH_REPO` race every other concurrently-scheduled `run`/`run_in`
/// call's read of the same table — the exact premise `src/test_env.rs`'s
/// `ENVIRONMENT` mutex exists to rule out for `cargo test --lib`, and cannot
/// reach this binary since it is `pub(crate)` there.
///
/// [`run_with_path`] takes the read side for the duration of its call into
/// [`run_with_path_locked`] — any number of them run concurrently, matching
/// this suite's existing parallelism.
/// `an_operators_gh_repo_never_reaches_a_spawned_gh` takes the write side
/// instead, across `set_var`, the spawn and the restore, and calls
/// `run_with_path_locked` directly rather than through `run_with_path`,
/// because a read-side attempt on the same thread that already holds the
/// write side is not something `std::sync::RwLock` promises to resolve.
static ENV_LOCK: std::sync::RwLock<()> = std::sync::RwLock::new(());

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
/// reads does not put the trap back. `run_in`, which this calls through, reaches
/// `run_with_path_locked`, which clears three more — `GH_REPO`, `GH_HOST`,
/// `GH_TOKEN` — for a different reason spelled where that clearing happens;
/// `CLAUDE_CONFIG_DIR` is cleared one level further down, by `estigia_command`,
/// for every spawn in this file and not only the ones that come this way.
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
    run_with_path(home, here, arguments, stdin, None, &[])
}

/// The same, with the search path replaced.
///
/// One `doctor` row is about a program on the machine — `gh` — and its broken
/// state is *this machine has no authenticated GitHub CLI*. It was the last row
/// measured as a builder and unmeasured as report, and the reason given was
/// that forcing it takes a path with no `gh` on it. This is that path.
///
/// `extra_env` is applied last, after the `ESTIGIA_*` loop, the
/// `GH_REPO`/`GH_HOST`/`GH_TOKEN` clearing and the four home defaults in
/// `run_with_path_locked` — so a caller naming one of those three GH_* names
/// overrides the clearing rather than testing it, and a caller naming `HOME` or
/// `XDG_CONFIG_HOME` poses a machine rather than being overridden by the
/// fixture's own; both are deliberate. What
/// `an_operators_gh_repo_never_reaches_a_spawned_gh` actually depends on is the
/// other half of that ordering, the `ESTIGIA_*` loop: it hands the child
/// `ESTIGIA_FAKE_ENV_LOG`, a name that loop would
/// otherwise strip along with every other inherited `ESTIGIA_*` variable
/// before this function's own caller ever gets a say.
fn run_with_path(
    home: &std::path::Path,
    here: &std::path::Path,
    arguments: &[&str],
    stdin: &str,
    search_path: Option<&std::path::Path>,
    extra_env: &[(&str, &str)],
) -> (String, String, bool) {
    let _guard = ENV_LOCK.read().expect("the env lock is not poisoned");
    run_with_path_locked(home, here, arguments, stdin, search_path, extra_env)
}

/// The body of [`run_with_path`], reading `std::env::vars()` without taking
/// `ENV_LOCK` itself — see that lock's own doc for why
/// `an_operators_gh_repo_never_reaches_a_spawned_gh` calls this directly,
/// under its own already-held write lock, instead of going through
/// `run_with_path`.
fn run_with_path_locked(
    home: &std::path::Path,
    here: &std::path::Path,
    arguments: &[&str],
    stdin: &str,
    search_path: Option<&std::path::Path>,
    extra_env: &[(&str, &str)],
) -> (String, String, bool) {
    run_with_bytes_locked(
        home,
        here,
        arguments,
        stdin.as_bytes(),
        false,
        search_path,
        extra_env,
    )
}

fn run_with_bytes_locked(
    home: &std::path::Path,
    here: &std::path::Path,
    arguments: &[&str],
    stdin: &[u8],
    child_may_stop_reading: bool,
    search_path: Option<&std::path::Path>,
    extra_env: &[(&str, &str)],
) -> (String, String, bool) {
    let inherited: Vec<String> = std::env::vars()
        .map(|(name, _)| name)
        .filter(|name| name.starts_with("ESTIGIA_"))
        .collect();
    let mut command = estigia_command();
    for name in &inherited {
        command.env_remove(name);
    }
    // Debug-only overrides the binary honours only under `cfg(debug_assertions)`:
    // skip the production readback sleep and the full-disk syncs so this suite
    // does not pay them. Set after the `ESTIGIA_*` strip above so the clear does
    // not remove them. A `--release` child ignores both.
    command
        .env("ESTIGIA_TEST_READBACK_NO_WAIT", "1")
        .env("ESTIGIA_UNSAFE_SKIP_FSYNC", "1");
    // This spawns the real binary and inherits the operator's environment —
    // unlike a `tracker_command` fixture, which puts a scripted `gh` first on
    // `PATH` and answers from `ESTIGIA_FAKE_ANSWERS`, ignoring these two
    // entirely. `GH_REPO` is precisely the variable this crate's own transport
    // sets to steer `gh`'s repository resolution when a checkout's remote is
    // not the one to ask (`harness::mod::gate`, `holder_standing`) — so an
    // operator who has it exported for their own work hands every `gh` this
    // binary spawns a repository that is not the fixture's, and reconciliation
    // (`guard::adjudicate_action`, when two or more pointers cover one
    // checkout) is what first made a several-holder decision consult the
    // tracker at all, where none of these tests intend one. Checked: without
    // this, `a_per_call_working_directory_selects_the_holder_that_owns_it`
    // asks a real `gh` about two issues in whatever repository
    // `GH_REPO` names, and its answer would decide what the test asserts.
    // `GH_HOST` and `GH_TOKEN` are the same class of steering, cleared for the
    // same reason before either is ever read.
    for name in ["GH_REPO", "GH_HOST", "GH_TOKEN"] {
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
    command
        .args(effective)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home.join("AppData").join("Roaming"))
        .env("XDG_CONFIG_HOME", home.join(".config"));
    // Last, after the home defaults above: a caller naming `HOME` or
    // `XDG_CONFIG_HOME` is posing a machine, not being overridden by the
    // fixture's own.
    for (name, value) in extra_env {
        command.env(name, value);
    }
    let mut child = command
        .current_dir(here)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    use std::io::Write;
    let written = child.stdin.take().expect("stdin is piped").write_all(stdin);
    match written {
        Err(error) if child_may_stop_reading && error.kind() == std::io::ErrorKind::BrokenPipe => {}
        other => other.expect("the payload is written"),
    }
    let output = child.wait_with_output().expect("the process ends");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.success(),
    )
}

/// The same, with standard input as raw bytes and the child allowed to stop
/// reading before all of them are written.
fn run_in_with_bytes(
    home: &std::path::Path,
    here: &std::path::Path,
    arguments: &[&str],
    stdin: &[u8],
) -> (String, String, bool) {
    let _guard = ENV_LOCK.read().expect("the env lock is not poisoned");
    run_with_bytes_locked(home, here, arguments, stdin, true, None, &[])
}

/// Identity for every commit a fixture git call may create (rebase, merge,
/// cherry-pick), so a host without a global identity behaves like one with it.
const FIXTURE_GIT_IDENTITY: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "nobody"),
    ("GIT_AUTHOR_EMAIL", "nobody@example.invalid"),
    ("GIT_COMMITTER_NAME", "nobody"),
    ("GIT_COMMITTER_EMAIL", "nobody@example.invalid"),
];

/// Runs `git -C <at> <arguments>` for a fixture, and panics naming the
/// argument list, the exit status and stderr on a nonzero exit, or the spawn
/// error on a failure to run at all — the two ways `.output().is_ok_and(...)`
/// collapsed into one indistinguishable `false`.
fn checked_git(at: &std::path::Path, arguments: &[&str]) -> bool {
    match Command::new("git")
        .arg("-C")
        .arg(at)
        .envs(FIXTURE_GIT_IDENTITY)
        .args(arguments)
        .output()
    {
        Ok(output) if output.status.success() => true,
        Ok(output) => panic!(
            "git {arguments:?} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) => panic!("git {arguments:?} did not run: {error}"),
    }
}

/// `checked_git`'s sibling for a caller that needs the command's stdout
/// rather than a pass/fail: `bool` has nowhere to carry the output, so a
/// capture that also needs the exit status checked cannot go through it.
fn checked_git_stdout(at: &std::path::Path, arguments: &[&str]) -> String {
    match Command::new("git")
        .arg("-C")
        .arg(at)
        .envs(FIXTURE_GIT_IDENTITY)
        .args(arguments)
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        Ok(output) => panic!(
            "git {arguments:?} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) => panic!("git {arguments:?} did not run: {error}"),
    }
}

/// Whether `git worktree list` registers `checkout`, whatever spelling the
/// platform gives the path: git prints the long name with `/` where the fixture
/// holds the 8.3 name with `\` on Windows, and a removed checkout can no longer
/// be resolved, so its parent directory and its own name are compared.
fn worktree_registered(repo: &std::path::Path, checkout: &std::path::Path) -> bool {
    let listing = checked_git_stdout(repo, &["worktree", "list", "--porcelain"]);
    listing
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(std::path::Path::new)
        .any(|listed| {
            listed.file_name() == checkout.file_name()
                && match (listed.parent(), checkout.parent()) {
                    (Some(listed), Some(checkout)) => {
                        estigia::paths::same_directory(listed, checkout)
                    }
                    _ => false,
                }
        })
}

/// Commits one documentation-only change on top of the checkout's tip and
/// answers `(base, head)`: the smallest change the review tier reads as `low`,
/// so a computed verdict over it licenses no lens.
fn low_delivery(at: &std::path::Path) -> (String, String) {
    let base = checked_git_stdout(at, &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    std::fs::write(at.join("notes.md"), "A line of prose, reworded.\n").expect("a file to commit");
    checked_git(at, &["add", "notes.md"]);
    checked_git(
        at,
        &[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            "notes",
        ],
    );
    let head = checked_git_stdout(at, &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    (base, head)
}

/// The bytes a plain `git diff` produces for `base..head` under the same
/// pinned flags `pinned_diff_args` uses, with the comparison fenced from every
/// configuration and attribute source outside the repository. The ambient side
/// of a cross against the isolated payload must be as machine-independent as
/// the isolated side, or a developer's own `core.attributesFile` or `diff.*`
/// setting decides the comparison rather than the code under test.
/// Computed here rather than read from the function under test, so a defect in
/// `review_payload` cannot hide behind a self-comparison.
fn fenced_ambient_diff(at: &std::path::Path, base: &str, head: &str) -> Vec<u8> {
    Command::new("git")
        .arg("-C")
        .arg(at)
        .envs(estigia::transport::fenced_git_env())
        .env_remove("GIT_DIFF_OPTS")
        .args([
            "-c",
            "diff.mnemonicPrefix=false",
            "-c",
            "diff.noprefix=false",
            "-c",
            "diff.srcPrefix=a/",
            "-c",
            "diff.dstPrefix=b/",
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--unified=3",
            base,
            head,
        ])
        .output()
        .expect("git diff answers")
        .stdout
}

/// The **value** column of one `config list` row, cut at its `accepts:` column.
///
/// The line is not the row. Every row ends with `accepts:` and the full list of
/// what it accepts, so a needle matched anywhere on the line is answered by the
/// help text whatever the value column holds — `Change size` accepts a sentence
/// naming `800`, and a bare `contains("800")` is satisfied by it.
fn listed_value(out: &str, label: &str) -> String {
    let row = out
        .lines()
        .find(|line| line.starts_with(label))
        .unwrap_or_else(|| panic!("no {label:?} row in:\n{out}"));
    let (value, _) = row
        .split_once("accepts:")
        .unwrap_or_else(|| panic!("no accepts column to cut at: {row}"));
    value.trim_start_matches(label).trim().to_owned()
}

/// One agent's model routing, as `config list` reports it.
fn models_of(home: &std::path::Path, agent: &str) -> String {
    let (out, _, ok) = run(home, &["config", "list", "--agent", agent], "");
    assert!(ok, "`config list --agent {agent}` failed");
    listed_value(&out, "Model routing")
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

/// [`run`], with a stand-down declared first.
fn run_with_stand_down(home: &std::path::Path, payload: &str) -> (String, String, bool) {
    let (_, stderr, ok) = run(
        home,
        &[
            "stand-down",
            "--reason",
            "the definition is not mine to fix",
        ],
        "",
    );
    assert!(ok, "stand-down failed: {stderr}");
    run(home, &["hook", "pre-tool-use"], payload)
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
/// that one asserted: with two faults posed, a check that stopped naming a
/// command drops the count under two.
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
    // used to run `cargo test --release --target <triple>`, which puts the example under
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
    let flags = fixture_flags::fixture_build_flags(&built);
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
    let fake_gh = bin.path().join(if cfg!(windows) { "gh.exe" } else { "gh" });
    #[cfg(unix)]
    std::os::unix::fs::symlink(&built, &fake_gh).expect("the fake gh is linked onto the path");
    #[cfg(not(unix))]
    std::fs::copy(&built, &fake_gh).expect("the fake gh is copied onto the path");

    let repo = tempfile::tempdir().expect("a directory for the checkout");
    let git = |arguments: &[&str]| checked_git(repo.path(), arguments);
    let _ = git(&["init", "--quiet"])
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

/// Advances `repo` by one commit changing a single line of `path`, and names
/// both ends — real object ids `record_review_finding`'s own `git diff` can
/// answer for, unlike the arbitrary hex a fixture that never carried a
/// location was free to invent.
///
/// `path:1` is the location every caller of this helper checks a finding
/// against: the diff between the two ids this returns changes exactly that
/// line and nothing else.
fn commit_a_changed_line(repo: &std::path::Path, path: &str) -> (String, String) {
    let git = |arguments: &[&str]| {
        checked_git(repo, arguments);
    };
    let oid = || {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git names the checkout's head");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    std::fs::write(repo.join(path), "before\n").expect("the base revision of the file");
    git(&["add", "--", path]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base",
    ]);
    let base = oid();
    std::fs::write(repo.join(path), "after\n").expect("the head revision of the file");
    git(&["add", "--", path]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "head",
    ]);
    let head = oid();
    (base, head)
}

/// One `ESTIGIA_FAKE_ANSWERS` entry answering `gh issue view` for the run
/// holding this issue, in the state its pointer says.
///
/// The `op-id` is thirty-two hex characters because `is_operation_id` demands
/// exactly that, and a marker that fails it is not authoritative — an earlier
/// version of this fixture used `op1` and the transport answered *"current live
/// holder is none"*, which looks exactly like a broken gate.
fn issue_view_entry(state: &str) -> serde_json::Value {
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
    serde_json::json!({
        "matches": "issue view",
        "stdout": issue.to_string(),
        "status": 0,
    })
}

/// An issue the run holds, in the state its pointer says.
fn issue_answer(state: &str) -> String {
    serde_json::to_string(&serde_json::json!([issue_view_entry(state)]))
        .expect("the script serialises")
}

/// One `ESTIGIA_FAKE_ANSWERS` entry answering `gh pr view <pr>` with the PR
/// sitting at `head_ref_oid`.
///
/// Ordered ahead of any `issue view` entry it is combined with: `matches` is a
/// substring test, and `pr view {pr}`/`issue view` never overlap, but a widened
/// matcher added later should still see this one first.
fn pr_view_entry(pr: u64, head_ref_oid: &str) -> serde_json::Value {
    let view = serde_json::json!({
        "number": pr,
        "url": format!("https://example.invalid/pr/{pr}"),
        "headRefOid": head_ref_oid,
        "baseRefOid": "b".repeat(40),
        "state": "OPEN",
        "isDraft": false,
    });
    serde_json::json!({
        "matches": format!("pr view {pr}"),
        "stdout": view.to_string(),
        "status": 0,
    })
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
    run_with_tracker_and_env(home, here, bin, answers, arguments, stdin, &[])
}

/// [`run_with_tracker`] with environment set *after* [`tracker_command`]'s
/// `GIT_*` strip, so a test can put back a variable the strip removed. The
/// host injects `GIT_EDITOR` into every shell, and that strip is what kept this
/// suite green while the exemption it covers was inert on that host.
fn run_with_tracker_and_env(
    home: &std::path::Path,
    here: &std::path::Path,
    bin: &std::path::Path,
    answers: &str,
    arguments: &[&str],
    stdin: &str,
    extra_env: &[(&str, &str)],
) -> (String, String, bool) {
    let mut command = tracker_command(home, here, bin, answers);
    for (name, value) in extra_env {
        command.env(name, value);
    }
    let mut child = command
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

/// Forgets what the local store learned about every issue, for a test that poses
/// a second world for an issue the first one already touched.
///
/// GitHub changing on its own is the one thing the store cannot see: it decides
/// from what it imported and from what its own operations wrote, so a fixture
/// that rewrites the issue between two invocations is posing a world the store
/// would learn about only by being asked again from scratch.
///
/// The gate's own record — the decisions and roles the store holds — is kept: it
/// was a separate file before the ledger moved here, and a test posing a second
/// world for an issue is not posing one for what the gate already wrote down.
fn forget_store(home: &std::path::Path) {
    let path = home.join(".estigia").join("state.db");
    if path.exists() {
        let records = kept_records(&path);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(home.join(".estigia").join(format!("state.db{suffix}")));
        }
        if !records.is_empty()
            && estigia::transport::store::Store::open(&path).is_ok()
            && let Ok(connection) = rusqlite::Connection::open(&path)
        {
            for (table, at, body) in records {
                let _ = connection.execute(
                    &format!("INSERT INTO {table} (at, body) VALUES (?1, ?2)"),
                    rusqlite::params![at, body],
                );
            }
        }
    }
}

/// The decisions and roles a store holds, read out so a fixture can rebuild the
/// store around them.
fn kept_records(path: &std::path::Path) -> Vec<(String, Option<i64>, String)> {
    let mut kept: Vec<(String, Option<i64>, String)> = Vec::new();
    if let Ok(connection) = rusqlite::Connection::open(path) {
        for table in ["decisions", "roles"] {
            if let Ok(mut statement) = connection.prepare(&format!("SELECT at, body FROM {table}"))
                && let Ok(rows) = statement.query_map([], |row| {
                    Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, String>(1)?))
                })
            {
                for row in rows.flatten() {
                    kept.push((table.to_owned(), row.0, row.1));
                }
            }
        }
    }
    kept
}

/// Every decision the fixture's store holds, one JSON object per line, so a
/// test reads it the way it read `decisions.jsonl` before.
fn decisions(home: &std::path::Path) -> String {
    let path = home.join(".estigia").join("state.db");
    if path.exists() {
        read_decisions(&path)
    } else {
        String::new()
    }
}

fn read_decisions(path: &std::path::Path) -> String {
    rusqlite::Connection::open(path)
        .ok()
        .and_then(|connection| {
            let mut statement = connection
                .prepare("SELECT body FROM decisions ORDER BY rowid")
                .ok()?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .ok()?;
            Some(rows.filter_map(Result::ok).collect::<Vec<_>>().join("\n"))
        })
        .unwrap_or_default()
}

/// The entries a run of the binary wrote to the fixture's store itself, oldest
/// first: what it recorded locally, as opposed to what it imported from GitHub.
fn local_entries(home: &std::path::Path) -> Vec<String> {
    let path = home.join(".estigia").join("state.db");
    rusqlite::Connection::open(path)
        .ok()
        .and_then(|connection| {
            let mut statement = connection
                .prepare("SELECT body FROM entries WHERE source = 'local' ORDER BY seq")
                .ok()?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .ok()?;
            Some(rows.filter_map(Result::ok).collect())
        })
        .unwrap_or_default()
}

/// How many calls in a stand-in `gh` log write a comment to GitHub, by the CLI
/// or by the REST API.
fn comment_writes(calls: &str) -> usize {
    calls
        .lines()
        .filter(|line| {
            line.starts_with("issue comment")
                || (line.starts_with("api ") && line.contains("/comments") && line.contains("POST"))
        })
        .count()
}

/// Forgets every decision the fixture's store holds, keeping its claims and
/// timelines: for a test that poses two worlds for the gate's own record.
fn clear_decisions(home: &std::path::Path) {
    let path = home.join(".estigia").join("state.db");
    if let Ok(connection) = rusqlite::Connection::open(&path) {
        let _ = connection.execute_batch("DELETE FROM decisions");
    }
}

/// What `gh api .../issues/12/comments` lists for comments written in the shape
/// `gh issue view` gives them, by the viewer, unedited.
fn rest_listing(comments: &[serde_json::Value]) -> serde_json::Value {
    let listed: Vec<serde_json::Value> = comments
        .iter()
        .map(|comment| {
            serde_json::json!({
                "id": 1,
                "node_id": comment["id"],
                "user": { "login": "fixture" },
                "created_at": comment["createdAt"],
                "updated_at": comment["createdAt"],
                "body": comment["body"],
            })
        })
        .collect();
    serde_json::json!({
        "matches": "issues/12/comments",
        "stdout": serde_json::Value::Array(listed).to_string(),
        "status": 0,
    })
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

    let mut command = estigia_command();
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("GIT_"))
        {
            command.env_remove(name);
        }
    }
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home.join("AppData").join("Roaming"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("PATH", path)
        .env("ESTIGIA_FAKE_ANSWERS", answers)
        .env("ESTIGIA_TEST_READBACK_NO_WAIT", "1")
        .env("ESTIGIA_UNSAFE_SKIP_FSYNC", "1")
        .current_dir(here);
    command
}

/// Names `o/r` as `repo`'s origin, so the store key and the queue read agree on
/// the repository without a configured one.
fn name_origin_o_r(repo: &std::path::Path) {
    name_origin(repo, "git@github.com:o/r.git");
}

/// Names `url` as `repo`'s origin.
fn name_origin(repo: &std::path::Path, url: &str) {
    assert!(
        checked_git(repo, &["remote", "add", "origin", url]),
        "the fixture origin is configured"
    );
}

/// A checkout of `name` carrying `window` as its own row, recorded under `home`.
///
/// The window resolves a write aimed at `name` to this checkout's rows, so a
/// cross-repository filing is judged by the destination's own schedule.
fn destination_checkout(home: &std::path::Path, name: &str, window: &str) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("a destination checkout");
    assert!(
        checked_git(directory.path(), &["init", "--quiet", "-b", "main"]),
        "the destination is a checkout"
    );
    name_origin(directory.path(), &format!("git@github.com:{name}.git"));
    let file = estigia::skill::repository_config_path(directory.path());
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("the rows directory");
    std::fs::write(
        &file,
        format!("| Setting | Value |\n|---|---|\n| Git operation window | {window} |\n"),
    )
    .expect("the rows");
    estigia::skill::remember_repository(home, directory.path());
    directory
}

/// Seeds the fixture's local store with one issue per `(number, state)`, so the
/// queue read that decides from the store sees them without a GitHub read.
fn seed_store(home: &std::path::Path, rows: &[(u64, &str)]) {
    seed_store_with(
        home,
        &rows
            .iter()
            .map(|(number, state)| (*number, *state, Vec::new()))
            .collect::<Vec<_>>(),
    );
}

/// The same, with a timeline per issue.
fn seed_store_with(home: &std::path::Path, rows: &[(u64, &str, Vec<serde_json::Value>)]) {
    let path = home.join(".estigia").join("state.db");
    let mut store = estigia::transport::store::Store::open(&path).expect("the fixture store opens");
    let repo = estigia::transport::store::Repo {
        host: "github.com".to_owned(),
        owner: "o".to_owned(),
        name: "r".to_owned(),
    };
    for (number, state, comments) in rows {
        store
            .import(
                &repo,
                *number,
                &serde_json::json!({
                    "body": "Body.",
                    "labels": [{ "name": format!("status:{state}") }],
                    "comments": comments,
                }),
                "2026-01-01T00:00Z",
            )
            .expect("the fixture issue imports");
    }
}

/// The entries this run recorded locally for `issue` in the fixture's store,
/// oldest first, with their prose.
///
/// The review family posts nothing, so the prose a blind reviewer reads is the
/// store's entry and never a comment body in the call log.
fn recorded_entries(home: &std::path::Path, issue: u64) -> Vec<String> {
    let path = home.join(".estigia").join("state.db");
    let connection = rusqlite::Connection::open(path).expect("the fixture store opens");
    let mut statement = connection
        .prepare("SELECT body FROM entries WHERE source = 'local' AND issue = ?1 ORDER BY seq")
        .expect("the receipts query prepares");
    statement
        .query_map([i64::try_from(issue).expect("an issue number")], |row| {
            row.get::<_, String>(0)
        })
        .expect("the receipts query runs")
        .filter_map(Result::ok)
        .collect()
}

/// The newest entry this run recorded for `issue`, read back from the store: the
/// receipt a publication just recorded, or the verdict a later call did.
fn staged_receipt(home: &std::path::Path, issue: u64) -> String {
    recorded_entries(home, issue)
        .pop()
        .unwrap_or_else(|| panic!("no review transaction was recorded for issue {issue}"))
}

/// Whether any entry for `issue` was recorded; the refusal path this backs must
/// tolerate zero.
fn any_receipt_staged(home: &std::path::Path, issue: u64) -> bool {
    !recorded_entries(home, issue).is_empty()
}

/// An issue the timeline carries no acquisition event for, whatever a local
/// pointer says: open, and never claimed by anybody the transport can see.
///
/// `unassign` reads `assignees,labels,comments`, not `state,labels,comments` —
/// [`issue_answer`] is the wrong fixture for this by one field, and `state`
/// costs nothing extra to carry since a reader that does not ask for it does
/// not mind it being there.
fn issue_answer_nobody_claimed() -> String {
    let issue = serde_json::json!({
        "state": "OPEN",
        "assignees": [],
        "labels": [{"name": "status:in-progress"}],
        "comments": [],
    });
    serde_json::to_string(&serde_json::json!([{
        "matches": "issue view",
        "stdout": issue.to_string(),
        "status": 0,
    }]))
    .expect("the script serialises")
}

/// One open pull request, as `gh pr list` answers it.
/// The pull request `published` finds on its second list, once created.
const LISTED_SEVEN: &str = "[{\"number\":7,\"url\":\"https://github.com/o/r/pull/7\",\"headRefOid\":\"0000000000000000000000000000000000000000\",\"baseRefOid\":\"0000000000000000000000000000000000000000\",\"isDraft\":true}]";

const LISTED_PR: &str =
    "[{\"number\":99,\"url\":\"u\",\"headRefOid\":\"x\",\"baseRefOid\":\"y\",\"isDraft\":true}]";

/// A `Vec` under `dropped`, not a string — every reason established, not
/// only the last one written.
fn first_dropped(value: &serde_json::Value) -> Option<&str> {
    value
        .as_array()
        .and_then(|reasons| reasons.first())
        .and_then(|reason| reason.as_str())
}

/// The two `next` sentences `publish_review` answers with, pinned whole.
///
/// They are contract text: a caller reads one of them and acts. Deciding by
/// substring whether a sentence offers a route to the payload text was tried
/// twice and rejected twice — a guard spelled around the wording it policed,
/// then a token blocklist that `publication` and `--inline-payload` both walk
/// past. A pin cannot be walked past. Editing either string reddens this test,
/// which is the point: the judgement belongs at the edit.
const INLINE_PAYLOAD_NEXT: &str = "transition --to review, hand each judge the `payload` from \
     this answer rather than a checkout to search, record each lens report and the computed \
     verdict, then release_ci with this exact receipt";

const OMITTED_PAYLOAD_NEXT: &str = "ask for start_review on the publication, or transition \
     --to review, launch_review_input and collect_review_input for each slot review_status \
     offers, record_computed_verdict, then release_ci with this exact receipt";

fn correction_budget_sha(at: &std::path::Path, what: &str) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(at)
        .args(["rev-parse", what])
        .output()
        .expect("git answers");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn correction_budget_ordinary_answers(pr: &serde_json::Value) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 }),
        serde_json::json!({ "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 }),
        serde_json::json!({
            "matches": "json body",
            "stdout": serde_json::json!({"body": "names nothing"}).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "repo view",
            "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}",
            "status": 0,
        }),
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
    ]
}

/// Runs `publish_review` once and answers whether it refused, the answer
/// text, and the fake tracker's call log.
#[expect(
    clippy::too_many_arguments,
    reason = "one call site per fixture, and every argument is a distinct fixture input rather \
              than a group that groups naturally into a struct"
)]
fn correction_budget_publish(
    home: &std::path::Path,
    repo: &std::path::Path,
    bin: &std::path::Path,
    trace: &std::path::Path,
    run_id: &str,
    issue: u64,
    branch: &str,
    timeline: &serde_json::Value,
    pr: &serde_json::Value,
    label: &str,
) -> (bool, String, std::path::PathBuf) {
    publication_attempt(
        home,
        repo,
        bin,
        trace,
        run_id,
        issue,
        "publish_review",
        branch,
        timeline,
        pr,
        label,
        None,
        None,
    )
}

/// Runs one publication tool — `publish_review` or `republish_review` — once
/// and answers whether it refused, the answer text, and the fake tracker's
/// call log.
#[expect(
    clippy::too_many_arguments,
    reason = "every argument is a distinct fixture input rather than a group that groups \
              naturally into a struct"
)]
fn publication_attempt(
    home: &std::path::Path,
    repo: &std::path::Path,
    bin: &std::path::Path,
    trace: &std::path::Path,
    run_id: &str,
    issue: u64,
    tool: &str,
    branch: &str,
    timeline: &serde_json::Value,
    pr: &serde_json::Value,
    label: &str,
    topology: Option<&str>,
    topology_reason: Option<&str>,
) -> (bool, String, std::path::PathBuf) {
    let body = trace.join(format!("{label}-body.md"));
    std::fs::write(&body, format!("Refs #{issue}.\n")).expect("the pull request body");

    let mut script = vec![serde_json::json!({
        "matches": "issue view", "stdout": timeline.to_string(), "status": 0,
    })];
    script.extend(correction_budget_ordinary_answers(pr));
    let answers = serde_json::to_string(&script).expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(issue));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let mut arguments = serde_json::Map::new();
    arguments.insert("issue".to_owned(), serde_json::json!(issue));
    arguments.insert("run_id".to_owned(), serde_json::json!(run_id));
    arguments.insert("branch".to_owned(), serde_json::json!(branch));
    arguments.insert("base".to_owned(), serde_json::json!("main"));
    arguments.insert("pr_title".to_owned(), serde_json::json!("Something"));
    arguments.insert(
        "pr_body_file".to_owned(),
        serde_json::json!(body.display().to_string()),
    );
    arguments.insert(
        "worktree".to_owned(),
        serde_json::json!(repo.display().to_string()),
    );
    if let Some(topology) = topology {
        arguments.insert("topology".to_owned(), serde_json::json!(topology));
    }
    if let Some(reason) = topology_reason {
        arguments.insert("topology_reason".to_owned(), serde_json::json!(reason));
    }
    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments }
    })
    .to_string();

    let log = trace.join(format!("{label}.log"));
    let temp = trace.join(format!("{label}-temp"));
    std::fs::create_dir_all(&temp).expect("a temporary directory for the child");
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LOG", &log)
        .env("TMPDIR", &temp)
        .env("TMP", &temp)
        .env("TEMP", &temp)
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
    (response["result"]["isError"] == true, text, temp)
}
