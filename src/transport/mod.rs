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
//!   process except [`run`].
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
pub mod manifest;
pub mod markers;
pub mod ownership;
pub mod target;
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
            Self::Read(_) => 3,
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
            Self::Read(detail) | Self::Write(detail) | Self::Timeout(detail) => detail.clone(),
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

    /// A call whose non-zero status is an answer rather than a failure.
    pub fn tolerated() -> Self {
        Self::default()
    }
}

/// Runs one command with an argument list and no shell.
///
/// No shell means no PowerShell backtick expansion, no word splitting and no
/// quoting rules — the corruption `bindings/github.md` documents cannot occur
/// through this path. It is also the only place in the crate's transport that
/// starts a process, so a test that replaces `gh` and `git` on `PATH` replaces
/// the whole world this code can see.
pub fn run(args: &[&str], cwd: Option<&Path>, how: How) -> Result<Output, Failure> {
    let Some((program, rest)) = args.split_first() else {
        return Err(Failure::Read("no command was given".to_owned()));
    };

    let mut command = Command::new(program);
    command.args(rest);
    if let Some(directory) = cwd {
        command.current_dir(directory);
    }

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

/// The waiting half of [`run`], kept separate so the ordinary path stays short.
fn spawn_with_deadline(
    mut command: Command,
    args: &[&str],
    cwd: Option<&Path>,
    how: How,
) -> Result<Output, Failure> {
    use std::io::Read;

    let deadline = how.timeout.unwrap_or_default();
    let mut child = match command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return Err(Failure::Read(spawn_error(args[0].as_ref(), cwd, &error))),
    };

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut handle) = child.stdout.take() {
                    let _ = handle.read_to_string(&mut stdout);
                }
                if let Some(mut handle) = child.stderr.take() {
                    let _ = handle.read_to_string(&mut stderr);
                }
                return finish(
                    args,
                    how,
                    std::process::Output {
                        status,
                        stdout: stdout.into_bytes(),
                        stderr: stderr.into_bytes(),
                    },
                );
            }
            Ok(None) if start.elapsed() >= deadline => {
                // Killed, and still reported as ambiguous: the remote may have
                // completed the mutation before the local wait gave up.
                let _ = child.kill();
                return Err(Failure::Timeout(format!(
                    "{} did not finish within {}s",
                    args.iter().take(3).copied().collect::<Vec<_>>().join(" "),
                    deadline.as_secs_f64()
                )));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => {
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

/// Everything a command needs that it cannot work out for itself.
#[derive(Debug, Clone)]
pub struct Context {
    /// The skill directory — the parent of `scripts/`, and where the contract
    /// and the operator's override live.
    pub skill_dir: PathBuf,
    /// The repository the command runs against.
    pub repo_dir: PathBuf,
    /// The operator table, already resolved.
    pub config: Vec<(String, String)>,
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
    /// nothing and `Board::parse("")` turned the **board mirror off for every
    /// run on the machine**. Measured: `enabled: false` with the empty table and
    /// `true` with the operator's own.
    ///
    /// Quietly, which is the part that costs. This module's own header says what
    /// is not allowed is *not trying* — a skipped attempt is how a board was
    /// once found five states behind.
    ///
    /// A contract that cannot be read leaves the table empty rather than
    /// refusing: the caller is the gate, and a gate that cannot start because a
    /// file is unreadable is a gate somebody switches off. What depends on a row
    /// answers `Skipped` and says so.
    pub fn live(skill_dir: PathBuf, repo_dir: PathBuf, repo: Option<String>) -> Self {
        // The contract's own rows, as written. Rows this crate does not model
        // are kept: `get` matches on a prefix over whatever is in the table, and
        // dropping a row nobody here has a `Setting` for would decide, on the
        // operator's behalf, that it means nothing.
        let mut config = std::fs::read_to_string(skill_dir.join(crate::skill::CONTRACT))
            .map(|text| crate::config::table_rows(&text))
            .unwrap_or_default();

        // And then the two documents that override it, both the operator's own:
        // `estigia.local.md` beside the contract, and the one inside a
        // repository that answers for itself.
        //
        // This read the contract and stopped. `estigia config list` layers both
        // and reports the result, so the operator was told `Project board
        // acme/7` while the transport read `none` — the board mirror off for
        // everybody who configured it in their own file, which is the same
        // outcome, and the same silence, as the round that gave this function
        // its first line. That fix carried the contract; the level below it was
        // taken as covered.
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
            match config
                .iter_mut()
                .find(|(known, _)| known.eq_ignore_ascii_case(&label))
            {
                Some(row) => row.1 = value,
                None => config.push((label, value)),
            }
        }

        Self {
            skill_dir,
            repo_dir,
            config,
            repo,
        }
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
            .find(|(key, _)| key.to_lowercase().starts_with(&wanted))
            .map(|(_, value)| value.as_str())
    }
}
