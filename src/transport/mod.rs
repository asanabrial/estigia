//! The Rust half of the tracker transport, built against the Python as an oracle.
//!
//! `skill/scripts/github.py` is 4.343 lines whose correctness took twenty rounds
//! of review and mutation testing. It is not rewritten in one move and it is not
//! rewritten from its documentation: each piece was built beside the original
//! and held to the same answer over the same input by `tests/differential.rs`.
//!
//! That is history now, and the tense matters. The original is deleted, the
//! corpus recorded off it is deleted, and nothing crosses this code against a
//! second implementation any more. What holds it is the unit tests written
//! beside each fix. The README's *what this instrument does not measure* carries
//! what that cost.
//!
//! # Why this file comes before any command
//!
//! The transport has **one process boundary**. A single `subprocess.run` in
//! 4.343 lines, inside its own `run()`, and every one of the 49 call sites goes
//! through it — nineteen spelling `gh`, thirty spelling `git`. That single door
//! is what makes the port tractable:
//!
//! - Reproducible input for the *Python* side stops being an eighteen-command
//!   problem and becomes one: put a scripted `gh` and `git` ahead of the real
//!   ones on `PATH`.
//! - Reproducible input for the *Rust* side is the same fixture, as long as this
//!   side keeps the boundary equally narrow. So it does: nothing here calls a
//!   process except [`run`] and its two crate-private siblings, `run_with_env`
//!   (environment variables layered over the child's inherited ones — `run`
//!   is that call with none) and `run_with_isolated_env` (the inherited
//!   environment replaced first, for the one caller that must not read a
//!   caller's own `HOME`-derived Git configuration).
//!
//! # The one distinction this module exists to preserve
//!
//! `writes` changes nothing about how the command runs and everything about what
//! its failure means. A failed read learned nothing and may be retried. A failed
//! write **may already have happened**, so the caller has to go and look before
//! deciding. Reporting a failed write under the read contract tells a caller to
//! retry blindly, which is how one command posts two claim comments.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub mod board;
pub mod branch;
pub mod changelog;
pub mod claim;
pub mod closing;
pub mod commands;
pub mod dispatch;
pub mod draft;
pub mod generated;
pub(crate) mod handoff;
pub mod judges;
pub mod linear;
pub mod manifest;
pub mod markers;
pub(crate) mod outbox;
pub mod ownership;
pub mod stages;
pub mod store;
pub mod target;
pub mod template;
pub mod tier;
pub(crate) mod window;
pub mod worktree;

#[cfg(test)]
mod tests;

/// Why a call to the world did not produce an answer.
///
/// The three shapes the transport distinguishes, and they are not
/// interchangeable: each one maps to a different exit code and a different
/// instruction to the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Nothing was learned. Not a stand-down and never clearance.
    Read(String),
    /// A read the tracker refused because the API budget is spent. Still
    /// nothing learned, and never clearance, but not an outage: retrying spends
    /// the budget the caller is waiting on.
    RateLimited(String),
    /// Something may already have happened. Re-read before deciding.
    Write(String),
    /// The wait expired on a call that mutates, so the fate of the write is
    /// unknown — which is not the same as "it did not happen".
    Timeout(String),
    /// A check said stop. Not a malfunction — an answer, and the payload is the
    /// answer. Carries its own reason, because the caller acts on the reason.
    Stop(serde_json::Value),
    /// The operator's configuration is wrong, or an argument was malformed.
    ///
    /// Ordered after `Stop` in the original's handlers and kept separate here for
    /// the same reason: `ConfigDefect` *is* a `Stop`, so a broader match first
    /// would swallow it and hand back the code that means "authority changed".
    ConfigDefect(serde_json::Value),
}

impl Failure {
    /// The transport's exit code for this failure.
    ///
    /// Mirrors the Python's `main`, and the values are the contract
    /// `harness::tracker::translate` already reads on the other side.
    pub fn code(&self) -> i32 {
        match self {
            // Two reasons are argument defects rather than the tracker changing
            // its mind, and they exit `2` so a caller does not read a malformed
            // request as "somebody took your claim".
            Self::Stop(payload) => {
                let reason = payload.get("reason").and_then(serde_json::Value::as_str);
                match reason {
                    Some("invalid-operation-id" | "invalid-horizon") => 2,
                    _ => 1,
                }
            }
            Self::ConfigDefect(_) => 2,
            Self::Read(_) | Self::RateLimited(_) => 3,
            Self::Write(_) | Self::Timeout(_) => 5,
        }
    }

    /// What went wrong, as one line.
    ///
    /// For the callers that report a failure rather than raising it — the board
    /// mirror is the whole of that population, and it reports because raising
    /// would kill an authoritative write.
    pub fn detail(&self) -> String {
        match self {
            Self::Read(detail)
            | Self::RateLimited(detail)
            | Self::Write(detail)
            | Self::Timeout(detail) => detail.clone(),
            Self::Stop(payload) | Self::ConfigDefect(payload) => payload
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("stopped")
                .to_owned(),
        }
    }

    /// The envelope the transport prints for this failure.
    pub fn envelope(&self) -> serde_json::Value {
        // A stop carries its own payload: the check that raised it knows what to
        // say, and rewriting it here would replace an answer with a paraphrase.
        if let Self::Stop(payload) | Self::ConfigDefect(payload) = self {
            return payload.clone();
        }
        let (reason, detail, action) = match self {
            Self::Read(detail) => (
                "read-failed",
                detail,
                "fail closed: write nothing, retry the read. Do not treat this as a stop or a pass.",
            ),
            Self::RateLimited(detail) => (
                "rate-limited",
                detail,
                "fail closed: write nothing and wait for the limit to reset. Do not retry \
                 meanwhile — a retry spends the budget being waited for.",
            ),
            Self::Timeout(detail) => (
                "ambiguous-write",
                detail,
                "the command may have completed remotely after the wait expired — RE-READ the \
                 branch, link and refs before retrying. Do not assume it did nothing.",
            ),
            Self::Write(detail) => (
                "write-failed",
                detail,
                "the write may have landed before it failed — RE-READ to establish what actually \
                 happened, then decide. Do not retry blindly.",
            ),
            // Answered above, and spelled out rather than wildcarded so a
            // seventh shape breaks the build here instead of printing the wrong
            // envelope.
            Self::Stop(_) | Self::ConfigDefect(_) => unreachable!("answered above"),
        };
        serde_json::json!({
            "ok": false,
            "reason": reason,
            "detail": detail,
            "action": action,
        })
    }
}

/// What a finished process said.
#[derive(Debug, Clone)]
pub struct Output {
    /// The exit status, or `-1` when a signal took the process.
    pub status: i32,
    /// Standard output, decoded with replacement.
    pub stdout: String,
    /// Standard error, decoded with replacement.
    pub stderr: String,
    /// Whether decoding standard output replaced anything.
    ///
    /// Replacement is right for a message — a damaged line is still worth
    /// reporting — and wrong for a **path**, which is bytes on Unix and comes
    /// back from `git` unchanged. A caller that reports paths has to be able to
    /// tell the two apart, because a replaced byte is a path that is not the
    /// path, announced as if it were: the fault this crate refuses by name
    /// everywhere it can read a value back.
    pub stdout_replaced: bool,
}

/// How one call to the world is made.
///
/// A struct rather than seven positional arguments, because `run(args, cwd,
/// true, false, None)` is a line nobody can read and `writes` is the one flag
/// that must never be set by accident.
#[derive(Debug, Clone, Copy, Default)]
pub struct How {
    /// A non-zero status is a failure. The Python's `check`.
    pub check: bool,
    /// This command **mutates something**. Decides which failure is raised, and
    /// nothing else.
    pub writes: bool,
    /// Give up after this long. Only ever set on a call that mutates, which is
    /// why expiry is reported as an ambiguous write rather than as nothing.
    pub timeout: Option<Duration>,
}

impl How {
    /// A read that must succeed.
    pub fn read() -> Self {
        Self {
            check: true,
            ..Self::default()
        }
    }

    /// A write that must succeed.
    pub fn write() -> Self {
        Self {
            check: true,
            writes: true,
            ..Self::default()
        }
    }

    /// A write that must succeed and gives up after `limit`, which reports it as
    /// an ambiguous write.
    pub fn write_within(limit: Duration) -> Self {
        Self {
            timeout: Some(limit),
            ..Self::write()
        }
    }

    /// A call whose non-zero status is an answer rather than a failure.
    pub fn tolerated() -> Self {
        Self::default()
    }
}

#[cfg(test)]
thread_local! {
    static PROCESS_OVERRIDE: std::cell::RefCell<Option<ProcessOverride>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct ProcessOverride {
    directory: PathBuf,
    envs: Vec<(String, OsString)>,
}

#[cfg(test)]
struct ProcessOverrideScope {
    previous: Option<ProcessOverride>,
}

#[cfg(test)]
impl Drop for ProcessOverrideScope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        PROCESS_OVERRIDE.with(|slot| *slot.borrow_mut() = previous);
    }
}

/// Installs a fake-CLI directory and the environment its programs answer from
/// on the calling thread for the whole of `body`, and takes it away again on
/// either exit.
///
/// [`run`] resolves a program named in `directory` to the copy there and sets
/// `envs` on that child alone, so the process environment is not touched and two
/// fixtures run concurrently without seeing each other. A body that spawns its
/// own threads is not covered, because a spawned thread does not inherit this:
/// see [`crate::test_env::with_scripted_gh_across_threads`].
#[cfg(test)]
pub(crate) fn with_process_override<T>(
    directory: &Path,
    envs: &[(String, OsString)],
    body: impl FnOnce() -> T,
) -> T {
    let previous = PROCESS_OVERRIDE.with(|slot| {
        slot.replace(Some(ProcessOverride {
            directory: directory.to_path_buf(),
            envs: envs.to_vec(),
        }))
    });
    let _scope = ProcessOverrideScope { previous };
    body()
}

#[cfg(test)]
fn override_program(program: &str) -> Option<PathBuf> {
    PROCESS_OVERRIDE.with(|slot| {
        let slot = slot.borrow();
        let installed = slot.as_ref()?;
        let candidate = installed.directory.join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = installed.directory.join(format!("{program}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    })
}

#[cfg(test)]
pub(crate) fn overridden_program(program: &str) -> Option<(PathBuf, Vec<(String, OsString)>)> {
    let resolved = override_program(program)?;
    let envs = PROCESS_OVERRIDE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|installed| installed.envs.clone())
            .unwrap_or_default()
    });
    Some((resolved, envs))
}

#[cfg(test)]
const REVIEWER_LAUNCH_VARIABLES: [&str; 7] = [
    "ESTIGIA_FAKE_ARGV_LOG",
    "ESTIGIA_FAKE_BIG_STDOUT",
    "ESTIGIA_FAKE_DELAY_MS",
    "ESTIGIA_FAKE_ENV_NAMES",
    "ESTIGIA_FAKE_EXIT_WITHOUT_READING",
    "ESTIGIA_FAKE_STDIN_LOG",
    "ESTIGIA_FAKE_WRITE_ARG_FILE",
];

#[cfg(test)]
fn apply_process_override(command: &mut Command) {
    PROCESS_OVERRIDE.with(|slot| {
        if let Some(installed) = slot.borrow().as_ref() {
            for name in REVIEWER_LAUNCH_VARIABLES {
                command.env_remove(name);
            }
            for (name, value) in &installed.envs {
                command.env(name, value);
            }
        }
    });
}

fn command_for(program: &str) -> Command {
    #[cfg(test)]
    if let Some(resolved) = override_program(program) {
        return Command::new(resolved);
    }
    Command::new(program)
}

/// Runs one command with an argument list and no shell.
///
/// No shell means no PowerShell backtick expansion, no word splitting and no
/// quoting rules — the corruption `bindings/github.md` documents cannot occur
/// through this path. It is also the only place in the crate's transport that
/// starts a process, so a test that replaces `gh` and `git` on `PATH` replaces
/// the whole world this code can see.
pub fn run(args: &[&str], cwd: Option<&Path>, how: How) -> Result<Output, Failure> {
    run_with_env(args, cwd, how, &[])
}

/// [`run`], with additional environment variables applied over the child's
/// inherited ones. `run` is this with an empty `envs`; both spawn through the
/// same `Command`, so the single process boundary the module doc names holds
/// for either entry point.
pub(crate) fn run_with_env(
    args: &[&str],
    cwd: Option<&Path>,
    how: How,
    envs: &[(&str, &str)],
) -> Result<Output, Failure> {
    run_with_env_and_mode(args, cwd, how, envs, EnvMode::Layer)
}

/// [`run_with_env`], with the child's environment **replaced** rather than
/// layered onto: only [`ISOLATED_ENV_ESSENTIAL`], `LC_ALL=C`, and `envs`
/// reach the child. Ports `sanitizedGitEnvironmentForRun(..., isolateConfig:
/// true)` (`snapshot.go:1874-1922`). `docs/honesty.md` has why layering left
/// a caller's own default global `.gitattributes` reachable.
pub(crate) fn run_with_isolated_env(
    args: &[&str],
    cwd: Option<&Path>,
    how: How,
    envs: &[(&str, &str)],
) -> Result<Output, Failure> {
    run_with_env_and_mode(args, cwd, how, envs, EnvMode::Replace)
}

/// Whether [`run_with_env_and_mode`] layers `envs` onto the child's inherited
/// environment or replaces the inherited environment first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvMode {
    Layer,
    Replace,
}

/// The variables kept when [`EnvMode::Replace`] clears everything else:
/// `sanitizedGitEnvironmentForRun`'s own `processEssential` set
/// (`snapshot.go:1898-1901`), the minimum a spawned `git` needs to resolve
/// its own binary, temp directory and Windows runtime without inheriting
/// anything a caller's profile or locale set.
const ISOLATED_ENV_ESSENTIAL: &[&str] = &[
    "COMSPEC",
    "PATH",
    "PATHEXT",
    "SYSTEMDRIVE",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "TMPDIR",
    "WINDIR",
];

/// The [`ISOLATED_ENV_ESSENTIAL`] subset of `ambient`, name and value both
/// kept as given. Pure and process-free so `EnvMode::Replace`'s filter is
/// unit-testable without mutating this test binary's own environment, which
/// every concurrently running test shares.
fn essential_env(ambient: impl IntoIterator<Item = (String, String)>) -> Vec<(String, String)> {
    ambient
        .into_iter()
        .filter(|(name, _)| is_essential_env(name))
        .collect()
}

/// Whether `name` is one of [`ISOLATED_ENV_ESSENTIAL`], matched the way Windows
/// spells environment names: without regard to ASCII case.
pub(crate) fn is_essential_env(name: &str) -> bool {
    ISOLATED_ENV_ESSENTIAL
        .iter()
        .any(|essential| name.eq_ignore_ascii_case(essential))
}

fn run_with_env_and_mode(
    args: &[&str],
    cwd: Option<&Path>,
    how: How,
    envs: &[(&str, &str)],
    mode: EnvMode,
) -> Result<Output, Failure> {
    let Some((program, rest)) = args.split_first() else {
        return Err(Failure::Read("no command was given".to_owned()));
    };
    if let Some(refusal) = window::tripwire(args, how) {
        return Err(refusal);
    }
    store::note(args);

    let mut command = command_for(program);
    command.args(rest);
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }
    match mode {
        EnvMode::Layer => {
            // `builtin_diff` applies `GIT_DIFF_OPTS` after the command line, so no
            // flag on `pinned_diff_args` can defeat it once a child inherits it — see
            // `docs/honesty.md`. Removed unconditionally rather than only ahead of
            // `git diff`: no command this crate runs wants a user's ambient diff
            // options, and a non-`diff` command does not read the variable at all.
            command.env_remove("GIT_DIFF_OPTS");
        }
        EnvMode::Replace => {
            command.env_clear();
            for (name, value) in essential_env(std::env::vars()) {
                command.env(name, value);
            }
            command.env("LC_ALL", "C");
        }
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    #[cfg(test)]
    apply_process_override(&mut command);

    // `Command::output` waits without a deadline. A timeout is only ever set on
    // a call that mutates, and the honest answer when one expires is that the
    // write's fate is unknown — so an unsupported wait must not silently become
    // an unbounded one.
    if how.timeout.is_some() {
        return spawn_with_deadline(command, args, cwd, how);
    }

    match command.output() {
        Ok(output) => finish(args, how, output),
        Err(error) => Err(Failure::Read(spawn_error(OsStr::new(program), cwd, &error))),
    }
}

/// Builds a `git diff` invocation with its rendering pinned against the local
/// install's own configuration: an unset `core.quotePath` C-quotes a non-ASCII
/// path, `diff.noprefix` or `diff.mnemonicPrefix` change or drop the `a/`/`b/`
/// prefix, `diff.srcPrefix`/`diff.dstPrefix` replace it with something other
/// than `a/`/`b/` independently of those two, and
/// `diff.external`/`GIT_EXTERNAL_DIFF` hand the text to a program other than
/// `git` — any of which breaks a caller that parses the rendered text rather
/// than git's structured output. `rest` is appended after the pinned flags,
/// ahead of any paths or pathspec. `docs/honesty.md` has the git version this
/// was measured against and why `-c diff.srcPrefix=a/ -c diff.dstPrefix=b/` is
/// used here instead of `--default-prefix`.
pub(crate) fn pinned_diff_args<'a>(rest: &[&'a str]) -> Vec<&'a str> {
    let mut args: Vec<&'a str> = vec![
        "git",
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
    ];
    args.extend_from_slice(rest);
    args
}

/// Torn down on drop; best effort, the same standing `Reservation` in
/// `branch.rs` gives its own lock file.
pub(crate) struct IsolatedGitDir {
    dir: PathBuf,
}

impl Drop for IsolatedGitDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The environment that fences a `git` process from every configuration and
/// attribute source outside the repository it runs in: the system and global
/// config layers are replaced with the platform's null device, the system
/// attribute file is ignored, and `core.attributesFile` is pinned to the null
/// device so the default global attributes file — resolved from `HOME` and
/// `XDG_CONFIG_HOME`, which this does not clear — cannot reach the diff.
///
/// `isolated_diff_env` supplies these for the payload side of a rebuild, whose
/// environment is replaced outright. A test that compares that payload against
/// a diff it builds itself must supply them for the comparison too, or the
/// comparison is decided by the machine running the suite rather than by the
/// code under test — the equality such a test asserts is only meaningful over
/// a diff no ambient source moved.
pub fn fenced_git_env() -> Vec<(&'static str, &'static str)> {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    vec![
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_SYSTEM", null),
        ("GIT_CONFIG_GLOBAL", null),
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", "core.attributesFile"),
        ("GIT_CONFIG_VALUE_0", null),
        ("GIT_ATTR_NOSYSTEM", "1"),
    ]
}

/// Builds a fresh, empty, bare `GIT_DIR` and the environment that points a
/// diff at it instead of the caller's own — shared by every caller that must
/// read two trees immune to a caller's own attribute and config sources,
/// reached through [`run_with_isolated_env`] rather than [`run_with_env`]'s
/// layered one. `docs/honesty.md` has what each of the two closes and what
/// neither does.
///
/// Placed inside the worktree's own private admin directory — `--git-dir`,
/// not `--git-common-dir` — the same reliable-writable-location reasoning
/// `frozen_candidate_context.go:329-332` gives for doing the same there.
/// Objects are shared from `--git-common-dir` because that is where every
/// worktree's objects actually live.
pub(crate) fn isolated_diff_env(
    at: &Path,
) -> Result<(IsolatedGitDir, Vec<(String, String)>), Failure> {
    let git_dir = run(
        &["git", "rev-parse", "--path-format=absolute", "--git-dir"],
        Some(at),
        How::read(),
    )?
    .stdout
    .trim()
    .to_owned();
    let common_dir = run(
        &[
            "git",
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ],
        Some(at),
        How::read(),
    )?
    .stdout
    .trim()
    .to_owned();
    if git_dir.is_empty() || common_dir.is_empty() {
        return Err(Failure::Read(format!(
            "git did not report an admin directory for {}",
            at.display()
        )));
    }
    let object_format = run(
        &["git", "rev-parse", "--show-object-format"],
        Some(at),
        How::read(),
    )?
    .stdout
    .trim()
    .to_owned();

    let unique = format!(
        ".estigia-isolated-diff-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default(),
    );
    let dir = Path::new(&git_dir).join(unique);
    // `paths::replace_atomically` rather than a direct write: it is the one
    // writer `tests/guards.rs`'s `nothing_writes_a_file_another_program_reads_by_truncating_it`
    // holds every shipping write to, so a scratch file only this call ever
    // reads is held to the same rule as one somebody else reads live.
    let write = |path: &Path, content: &str| -> Result<(), Failure> {
        crate::paths::replace_atomically(path, content)
            .map_err(|error| Failure::Read(format!("could not write {}: {error}", path.display())))
    };
    std::fs::create_dir(&dir)
        .map_err(|error| Failure::Read(format!("could not create {}: {error}", dir.display())))?;
    let guard = IsolatedGitDir { dir: dir.clone() };
    for name in ["objects", "refs"] {
        let sub = dir.join(name);
        std::fs::create_dir(&sub).map_err(|error| {
            Failure::Read(format!("could not create {}: {error}", sub.display()))
        })?;
    }
    write(&dir.join("HEAD"), "ref: refs/heads/isolated-diff\n")?;
    let (format_version, extensions) = if object_format == "sha256" {
        ("1", "[extensions]\n\tobjectformat = sha256\n")
    } else {
        ("0", "")
    };
    write(
        &dir.join("config"),
        &format!(
            "[core]\n\trepositoryformatversion = {format_version}\n\tbare = true\n{extensions}"
        ),
    )?;
    let mut envs = vec![
        ("GIT_DIR".to_owned(), dir.display().to_string()),
        (
            "GIT_OBJECT_DIRECTORY".to_owned(),
            Path::new(&common_dir).join("objects").display().to_string(),
        ),
    ];
    envs.extend(
        fenced_git_env()
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned())),
    );
    envs.push(("LANG".to_owned(), "C".to_owned()));
    Ok((guard, envs))
}

/// Runs an ordinary command without forcing filesystem paths through UTF-8.
///
/// This narrow door is for local filesystem commands with no deadline. Timed
/// tracker calls retain [`run`]'s established behavior; widening that boundary
/// would mix path preservation with unrelated process-lifetime semantics.
pub fn run_os(args: &[OsString], cwd: Option<&Path>, how: How) -> Result<Output, Failure> {
    assert!(how.timeout.is_none(), "run_os does not implement deadlines");
    let Some((program, rest)) = args.split_first() else {
        return Err(Failure::Read("no command was given".to_owned()));
    };
    let mut command = Command::new(program);
    command.args(rest);
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }
    match command.output() {
        Ok(output) => finish_os(args, how, output),
        Err(error) => Err(Failure::Read(spawn_error(program, cwd, &error))),
    }
}

/// Describes why a process did not start without blaming every failure on PATH.
fn spawn_error(program: &OsStr, cwd: Option<&Path>, error: &std::io::Error) -> String {
    let program = program.to_string_lossy();
    if let Some(directory) = cwd
        && !directory.is_dir()
    {
        return format!(
            "{program} could not start in {}: {error}",
            directory.display()
        );
    }
    if error.kind() == std::io::ErrorKind::NotFound {
        return format!("{program} not found on PATH: {error}");
    }
    format!("{program} could not start: {error}")
}

/// How long the pipes get to reach end of file once the child has exited.
const PIPE_GRACE: Duration = Duration::from_secs(2);

fn drain_into(
    mut pipe: impl std::io::Read + Send + 'static,
    kept: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    done: std::sync::mpsc::Sender<()>,
) {
    std::thread::spawn(move || {
        let mut chunk = [0_u8; 8192];
        while let Ok(read) = pipe.read(&mut chunk) {
            if read == 0 {
                break;
            }
            if let Ok(mut kept) = kept.lock() {
                kept.extend_from_slice(&chunk[..read]);
            }
        }
        let _ = done.send(());
    });
}

/// The waiting half of [`run`], kept separate so the ordinary path stays short.
fn spawn_with_deadline(
    mut command: Command,
    args: &[&str],
    cwd: Option<&Path>,
    how: How,
) -> Result<Output, Failure> {
    use crate::reviewer::process_tree;
    use std::process::Stdio;
    use std::sync::{Arc, Mutex, mpsc};

    let deadline = how.timeout.unwrap_or_default();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let prepared = process_tree::prepare(&mut command)
        .map_err(|error| Failure::Read(format!("{} could not be contained: {error}", args[0])))?;
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return Err(Failure::Read(spawn_error(args[0].as_ref(), cwd, &error))),
    };
    let tree = match prepared.attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Failure::Read(format!(
                "{} could not be contained: {error}",
                args[0]
            )));
        }
    };

    let stdout = Arc::new(Mutex::new(Vec::new()));
    let stderr = Arc::new(Mutex::new(Vec::new()));
    let (done, finished) = mpsc::channel();
    let mut readers = 0;
    if let Some(pipe) = child.stdout.take() {
        drain_into(pipe, stdout.clone(), done.clone());
        readers += 1;
    }
    if let Some(pipe) = child.stderr.take() {
        drain_into(pipe, stderr.clone(), done);
        readers += 1;
    }

    let start = std::time::Instant::now();
    loop {
        match process_tree::exited(&mut child) {
            Ok(true) => {
                let limit = std::time::Instant::now() + PIPE_GRACE;
                for _ in 0..readers {
                    let wait = limit.saturating_duration_since(std::time::Instant::now());
                    if finished.recv_timeout(wait).is_err() {
                        let _ = tree.terminate();
                        break;
                    }
                }
                let status = match child.wait() {
                    Ok(status) => status,
                    Err(error) => {
                        return Err(Failure::Read(format!(
                            "{} could not be waited on: {error}",
                            args[0]
                        )));
                    }
                };
                let take = |kept: &Mutex<Vec<u8>>| {
                    kept.lock()
                        .map(|mut kept| std::mem::take(&mut *kept))
                        .unwrap_or_default()
                };
                return finish(
                    args,
                    how,
                    std::process::Output {
                        status,
                        stdout: take(&stdout),
                        stderr: take(&stderr),
                    },
                );
            }
            Ok(false) if start.elapsed() >= deadline => {
                // Killed, and still reported as ambiguous: the remote may have
                // completed the mutation before the local wait gave up.
                let _ = tree.terminate();
                let _ = child.kill();
                let _ = child.wait();
                return Err(Failure::Timeout(format!(
                    "{} did not finish within {}s",
                    args.iter().take(3).copied().collect::<Vec<_>>().join(" "),
                    deadline.as_secs_f64()
                )));
            }
            Ok(false) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => {
                let _ = tree.terminate();
                let _ = child.kill();
                let _ = child.wait();
                return Err(Failure::Read(format!(
                    "{} could not be waited on: {error}",
                    args[0]
                )));
            }
        }
    }
}

/// Turns a finished process into an answer or the right kind of failure.
fn finish(args: &[&str], how: How, output: std::process::Output) -> Result<Output, Failure> {
    // `errors="replace"`, the same as the Python: a byte that is not UTF-8 is a
    // damaged message, and a damaged message is still worth reporting. Refusing
    // to decode it would turn a readable failure into an unreadable one.
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stdout_replaced = stdout.len() != output.stdout.len();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output.status.code().unwrap_or(-1);

    if how.check && status != 0 {
        let detail = format!(
            "{} failed ({status}): {}",
            args.iter().take(3).copied().collect::<Vec<_>>().join(" "),
            stderr.trim()
        );
        return Err(if how.writes {
            Failure::Write(detail)
        } else if let Some(kind) = rate_limit_kind(&stderr) {
            Failure::RateLimited(rate_limited_detail(kind, &stderr, &detail))
        } else {
            Failure::Read(detail)
        });
    }

    Ok(Output {
        status,
        stdout,
        stderr,
        stdout_replaced,
    })
}

/// Which GitHub rate limit `gh` named on stderr, if it named one.
fn rate_limit_kind(stderr: &str) -> Option<&'static str> {
    let lowered = stderr.to_ascii_lowercase();
    if lowered.contains("secondary rate limit") {
        Some("secondary")
    } else if lowered.contains("rate limit exceeded")
        || lowered.contains("rate limit already exceeded")
    {
        Some("primary")
    } else {
        None
    }
}

/// The first `YYYY-MM-DDTHH:MM:SSZ` instant in `text`, if `gh` printed one.
fn instant_in(text: &str) -> Option<&str> {
    const SHAPE: &[u8] = b"dddd-dd-ddTdd:dd:ddZ";
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(SHAPE.len() - 1)).find_map(|start| {
        let window = &bytes[start..start + SHAPE.len()];
        let fits = window.iter().zip(SHAPE).all(|(byte, shape)| match shape {
            b'd' => byte.is_ascii_digit(),
            other => byte == other,
        });
        fits.then(|| &text[start..start + SHAPE.len()])
    })
}

fn rate_limited_detail(kind: &str, stderr: &str, failure: &str) -> String {
    let reset = match instant_in(stderr) {
        Some(instant) => format!("resets at {instant}"),
        None => "reset time unknown".to_owned(),
    };
    format!("GitHub {kind} rate limit hit, {reset}: {failure}")
}

fn finish_os(args: &[OsString], how: How, output: std::process::Output) -> Result<Output, Failure> {
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>();
    let borrowed = rendered.iter().map(AsRef::as_ref).collect::<Vec<_>>();
    finish(&borrowed, how, output)
}

/// Reads JSON from `gh`, the way every reading call site does.
pub fn gh_json(args: &[&str], cwd: Option<&Path>) -> Result<Option<serde_json::Value>, Failure> {
    let mut full = vec!["gh"];
    full.extend_from_slice(args);
    let output = run(&full, cwd, How::read())?;
    let text = output.stdout.trim();
    if text.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(text).map(Some).map_err(|error| {
        Failure::Read(format!(
            "gh returned unparseable JSON for {}: {error}",
            args.iter().take(2).copied().collect::<Vec<_>>().join(" ")
        ))
    })
}

/// Whether one operator-table row was written by somebody or rendered from
/// [`crate::config::Config::default`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Somebody wrote the row.
    Written,
    /// The row was rendered from [`crate::config::Config::default`].
    Defaulted,
}

/// Everything a command needs that it cannot work out for itself.
#[derive(Debug, Clone)]
pub struct Context {
    /// The skill directory — the parent of `scripts/`, and where the contract
    /// and the operator's override live.
    pub skill_dir: PathBuf,
    /// The repository the command runs against.
    pub repo_dir: PathBuf,
    /// The operator table, already resolved.
    pub config: Vec<(String, String, Source)>,
    /// The repository `gh` is told to work against, when the operator named one.
    ///
    /// Only when they did. Setting it to the checkout's own repository would be
    /// Estigia asserting a fact it read from git as though somebody had chosen
    /// it — the reason the spawned transport set `GH_REPO` conditionally, kept
    /// here now that the calls are made in this process.
    pub repo: Option<String>,
}

/// The `nodes` and `pageInfo` of a GraphQL connection, or a read that failed.
///
/// **One reader.** Two callers took the pair apart themselves, with the reason
/// written down at one of them and applied silently at the other: *an absent
/// `nodes` is not "no results" — it is a response nobody can read, and the
/// empty list it would flatten to is the answer that grants clearance*. That
/// sentence is the whole rule, and a copy of it is a place somebody can later
/// make lenient without meeting the sentence.
///
/// `what` names the read for the refusal, because *which* listing came back
/// unreadable is what tells the caller where to look.
pub(super) fn connection_page<'a>(
    connection: &'a serde_json::Value,
    what: &str,
) -> Result<(&'a Vec<serde_json::Value>, &'a serde_json::Value), Failure> {
    let (Some(nodes), Some(page_info)) = (
        connection
            .get("nodes")
            .and_then(serde_json::Value::as_array),
        connection.get("pageInfo").filter(|value| value.is_object()),
    ) else {
        return Err(Failure::Read(format!(
            "{what} omitted nodes or page metadata"
        )));
    };
    Ok((nodes, page_info))
}

impl Context {
    /// The context a live call runs in: the operator's table, read from the
    /// contract beside the skill.
    ///
    /// Built here rather than at each caller, because building it wrong is
    /// invisible. Both live callers passed `config: Vec::new()` when the
    /// operations moved into this process, and an empty table is not an empty
    /// answer — `Context::get` prefix-matches a list, so every row resolved to
    /// nothing and the worktree template row turned every isolated checkout
    /// automatic. Measured against the operator's own table, which reads the
    /// rows they wrote.
    ///
    /// Quietly, which is the part that costs. This module's own header says what
    /// is not allowed is *not trying*.
    ///
    /// A contract that cannot be read refuses nothing: the caller is the gate,
    /// and a gate that cannot start because a file is unreadable is a gate
    /// somebody switches off. What it leaves absent is only the *unmodelled*
    /// rows — a label this crate has no `Setting` for. Every *modelled* row
    /// still arrives, because `installed_config_in_keeping_what_parses` falls
    /// back to `Config::default()` and the merge below folds in that default's
    /// `render_rows()` unconditionally. [`Context::get`] answers that effective table,
    /// so a rendered default reads the same as a written answer there;
    /// [`Context::get_written`] answers only rows somebody wrote, so a caller that must
    /// tell the two apart reads that instead.
    pub fn live(skill_dir: PathBuf, repo_dir: PathBuf, repo: Option<String>) -> Self {
        // The contract's own rows, as written. Rows this crate does not model
        // are kept: `get` matches on a prefix over whatever is in the table, and
        // dropping a row nobody here has a `Setting` for would decide, on the
        // operator's behalf, that it means nothing.
        let contract_rows = std::fs::read_to_string(skill_dir.join(crate::skill::CONTRACT))
            .map(|text| crate::config::table_rows(&text))
            .unwrap_or_default();

        let mut written = std::collections::HashSet::new();
        for (label, value) in &contract_rows {
            if written_row(label, value) {
                written.insert(label.to_lowercase());
            }
        }
        if let Ok(Some((_, text))) = crate::skill::local_overrides(&skill_dir) {
            for (label, value) in crate::config::table_rows(&text) {
                if written_row(&label, &value) {
                    written.insert(label.to_lowercase());
                }
            }
        }
        if let Ok(settings) = crate::skill::repository_owned_settings(&repo_dir) {
            for setting in settings {
                written.insert(setting.label().to_lowercase());
            }
        }

        let mut config: Vec<(String, String, Source)> = contract_rows
            .into_iter()
            .map(|(label, value)| {
                let source = if written.contains(&label.to_lowercase()) {
                    Source::Written
                } else {
                    Source::Defaulted
                };
                (label, value, source)
            })
            .collect();

        // And then the two documents that override it, both the operator's own:
        // `estigia.local.md` beside the contract, and the one inside a
        // repository that answers for itself.
        //
        // This read the contract and stopped. `estigia config list` layers both
        // and reports the result, so the operator was told a worktree location
        // the transport read as unset — automatic for everybody who configured
        // it in their own file, which is the same outcome, and the same silence,
        // as the round that gave this function its first line. That fix carried
        // the contract; the level below it was taken as covered.
        //
        // Asked of `installed_config_in` rather than layered here: it owns
        // which document overrides which **and** which rows a repository may
        // speak for, and a second copy of that written for this caller is the
        // shape this crate keeps paying for. Tolerant, because the caller is the
        // gate: a table with one bad row must leave the rest standing, and a
        // gate that will not start because a file will not parse is a gate
        // somebody turns off.
        let (layered, _) =
            crate::skill::installed_config_in_keeping_what_parses(&skill_dir, &repo_dir);
        for (label, value) in crate::config::table_rows(&layered.render_rows()) {
            // In place, never appended. `get` answers with the **first** row
            // whose label the question is a prefix of, so an override pushed
            // onto the end would be a row nothing ever reads.
            let source = if written.contains(&label.to_lowercase()) {
                Source::Written
            } else {
                Source::Defaulted
            };
            match config
                .iter_mut()
                .find(|(known, _, _)| known.eq_ignore_ascii_case(&label))
            {
                Some(row) => {
                    row.1 = value;
                    row.2 = source;
                }
                None => config.push((label, value, source)),
            }
        }

        Self {
            skill_dir,
            repo_dir,
            config,
            repo,
        }
    }

    /// The whole configuration in force here, the repository's own rows
    /// included, which [`Self::config`] does not carry: a repository-scoped row
    /// is never rendered into a contract table.
    pub fn layered(&self) -> crate::config::Config {
        crate::skill::installed_config_in_keeping_what_parses(&self.skill_dir, &self.repo_dir).0
    }

    /// One configured value, by the label the table uses.
    ///
    /// A **prefix** match on the lowercased label, first row wins — which is the
    /// Python's `cfg`, surprise included. `cfg(config, "delivery")` matches both
    /// `delivery authorisation` and `delivery route`, and answers with whichever
    /// the table lists first. Copied rather than corrected: the two sides have to
    /// resolve a question the same way while both exist, and tightening it here
    /// would make Estigia read a row the transport does not.
    pub fn get(&self, label: &str) -> Option<&str> {
        let wanted = label.to_lowercase();
        self.config
            .iter()
            .find(|(key, _, _)| key.to_lowercase().starts_with(&wanted))
            .map(|(_, value, _)| value.as_str())
    }

    /// One configured value, answered only when somebody wrote the row.
    ///
    /// Same prefix match as [`Context::get`]; `None` when the effective value was
    /// rendered from [`crate::config::Config::default`].
    pub fn get_written(&self, label: &str) -> Option<&str> {
        let wanted = label.to_lowercase();
        self.config
            .iter()
            .find(|(key, _, _)| key.to_lowercase().starts_with(&wanted))
            .filter(|(_, _, source)| *source == Source::Written)
            .map(|(_, value, _)| value.as_str())
    }
}

/// Whether one raw table row counts as written.
///
/// An unmodelled row is the operator's own note, kept verbatim. A modelled
/// row counts when its value parses — a row the tolerant reader drops keeps
/// the default, and answering that default as written would repeat that mistake.
fn written_row(label: &str, value: &str) -> bool {
    match crate::config::Setting::from_label(label) {
        None => true,
        Some(setting) => setting
            .apply(&mut crate::config::Config::default(), value)
            .is_ok(),
    }
}
