//! Declared checks run by a detached runner that outlives the call that asked.
//!
//! A job is a directory shaped like a reviewer's: a `manifest.json` written
//! before the launch, a `lock` the runner holds while it runs and a
//! `result.json` written before the lock drops. [`run`] is the runner's whole
//! body and what the hidden `estigia __check-job <dir>` subcommand calls. The
//! lock, the liveness reading and the detaching are `reviewer::job`'s, not
//! restated here. The runner never opens the ledger: what a result means is
//! decided by whoever collects it.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::outcome::Refusal;
use crate::paths::{replace_atomically, replace_bytes_atomically};
use crate::reviewer::job::{self, Detached, JobStatus};
use crate::transport::Failure;
use crate::transport::generated::checks::Check;

const FORMAT: u32 = 1;
const MANIFEST: &str = "manifest.json";

/// How long the whole job may run before its process group is killed.
pub const DEADLINE: Duration = Duration::from_secs(30 * 60);

/// How many trailing bytes of a check's combined output a result keeps.
pub const TAIL_BYTES: u64 = 2_000;

const POLL: Duration = Duration::from_millis(20);

/// What one check did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRun {
    /// The declared name.
    pub name: String,
    /// The exit code, or `None` when the check was killed or never started.
    pub exit: Option<i32>,
    /// Whole seconds it ran.
    pub seconds: u64,
    /// The last [`TAIL_BYTES`] of combined stdout and stderr, lossily UTF-8.
    pub tail: String,
}

/// What a finished job left in `result.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Whether the deadline killed the job before every check finished.
    pub timed_out: bool,
    /// What each check that ran did, in declaration order.
    pub runs: Vec<CheckRun>,
}

impl Outcome {
    /// Whether every declared check ran and exited zero.
    pub fn passed(&self, declared: usize) -> bool {
        !self.timed_out
            && self.runs.len() == declared
            && self.runs.iter().all(|run| run.exit == Some(0))
    }
}

fn refused(message: String) -> Refusal {
    job::refused(message)
}

fn unwritten(what: &str, error: impl std::fmt::Display) -> Failure {
    Failure::Read(format!("could not {what}: {error}"))
}

/// Writes a job's manifest into `dir`, creating it.
pub fn write_job(
    dir: &Path,
    worktree: &Path,
    digest: &str,
    commands: &str,
    checks: &[Check],
) -> Result<(), Failure> {
    std::fs::create_dir_all(dir).map_err(|e| unwritten("create a check job directory", e))?;
    let manifest = serde_json::json!({
        "format": FORMAT,
        "worktree": worktree.display().to_string(),
        "digest": digest,
        "commands": commands,
        "checks": checks
            .iter()
            .map(|check| serde_json::json!({
                "name": check.name,
                "command": check.command,
                "cwd": check.cwd,
            }))
            .collect::<Vec<_>>(),
    });
    replace_atomically(&dir.join(MANIFEST), &manifest.to_string())
        .map_err(|e| unwritten("write a check job manifest", e))
}

fn load(dir: &Path) -> Result<(PathBuf, Vec<Check>), Refusal> {
    let raw = std::fs::read(dir.join(MANIFEST))
        .map_err(|e| refused(format!("the check job manifest could not be read: {e}")))?;
    let manifest: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| refused(format!("the check job manifest is not JSON: {e}")))?;
    if manifest.get("format").and_then(serde_json::Value::as_u64) != Some(u64::from(FORMAT)) {
        return Err(refused(format!(
            "the check job manifest is not format {FORMAT}"
        )));
    }
    let missing = |what: &str| refused(format!("the check job manifest has no `{what}`"));
    let worktree = manifest
        .get("worktree")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| missing("worktree"))?;
    let listed = manifest
        .get("checks")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| missing("checks"))?;
    let mut checks = Vec::with_capacity(listed.len());
    for entry in listed {
        let name = entry
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| missing("name"))?;
        let command: Vec<String> = entry
            .get("command")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| missing("command"))?
            .iter()
            .map(|argument| argument.as_str().map(str::to_owned))
            .collect::<Option<_>>()
            .filter(|command: &Vec<String>| !command.is_empty())
            .ok_or_else(|| missing("command"))?;
        checks.push(Check {
            name: name.to_owned(),
            command,
            cwd: entry
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        });
    }
    Ok((PathBuf::from(worktree), checks))
}

fn tail_of(path: &Path) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map(|meta| meta.len()).unwrap_or_default();
    if file
        .seek(SeekFrom::Start(length.saturating_sub(TAIL_BYTES)))
        .is_err()
    {
        return String::new();
    }
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(unix)]
fn own_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn own_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

fn run_one(
    dir: &Path,
    position: usize,
    tree: &Path,
    check: &Check,
    remaining: Duration,
) -> (CheckRun, bool) {
    let began = Instant::now();
    let output = dir.join(format!("output-{position}"));
    let finish = |exit, timed_out, tail| {
        (
            CheckRun {
                name: check.name.clone(),
                exit,
                seconds: began.elapsed().as_secs(),
                tail,
            },
            timed_out,
        )
    };
    let attached = File::options()
        .create(true)
        .append(true)
        .open(&output)
        .and_then(|out| out.try_clone().map(|err| (out, err)));
    let Ok((out, err)) = attached else {
        return finish(None, false, "could not open an output file".to_owned());
    };
    let directory = check
        .cwd
        .as_deref()
        .map_or_else(|| tree.to_path_buf(), |cwd| tree.join(cwd));
    let mut command = Command::new(&check.command[0]);
    command
        .args(&check.command[1..])
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err);
    own_group(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return finish(None, false, format!("could not start: {error}")),
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if began.elapsed() >= remaining => break None,
            Ok(None) => std::thread::sleep(POLL),
            Err(_) => break None,
        }
    };
    let result = match status {
        Some(status) => finish(status.code(), false, tail_of(&output)),
        None => {
            job::kill_group(child.id());
            let _ = child.wait();
            finish(None, true, tail_of(&output))
        }
    };
    let _ = std::fs::remove_file(&output);
    result
}

fn execute(dir: &Path, tree: &Path, checks: &[Check], deadline: Duration) -> Outcome {
    let began = Instant::now();
    let mut runs = Vec::with_capacity(checks.len());
    for (position, check) in checks.iter().enumerate() {
        let remaining = deadline.saturating_sub(began.elapsed());
        let (run, timed_out) = run_one(dir, position, tree, check, remaining);
        runs.push(run);
        if timed_out {
            return Outcome {
                timed_out: true,
                runs,
            };
        }
    }
    Outcome {
        timed_out: false,
        runs,
    }
}

fn result_json(outcome: &Outcome) -> serde_json::Value {
    serde_json::json!({
        "format": FORMAT,
        "timed_out": outcome.timed_out,
        "checks": outcome
            .runs
            .iter()
            .map(|run| serde_json::json!({
                "name": run.name,
                "exit": run.exit,
                "seconds": run.seconds,
                "tail": run.tail,
            }))
            .collect::<Vec<_>>(),
    })
}

/// The runner: holds the job's lock while it runs each declared check in order
/// with no shell, a failing check not stopping the rest, and leaves
/// `result.json` before the lock drops. A manifest it cannot read leaves no
/// result, which reads as a runner that died.
pub fn run(dir: &Path) -> Result<(), Refusal> {
    run_within(dir, DEADLINE)
}

pub(crate) fn run_within(dir: &Path, deadline: Duration) -> Result<(), Refusal> {
    let _lock = job::begin(dir)?;
    let (tree, checks) = load(dir)?;
    let outcome = execute(dir, &tree, &checks, deadline);
    replace_bytes_atomically(
        &dir.join(job::RESULT),
        result_json(&outcome).to_string().as_bytes(),
    )
    .map_err(|e| refused(format!("the check job result could not be written: {e}")))
}

fn read_result(dir: &Path) -> Result<Option<Outcome>, Failure> {
    let raw = match std::fs::read(dir.join(job::RESULT)) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(unwritten("read a check job result", e)),
    };
    let unreadable = |what: &str| Failure::Read(format!("the check job result {what}"));
    let value: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|_| unreadable("is not JSON"))?;
    if value.get("format").and_then(serde_json::Value::as_u64) != Some(u64::from(FORMAT)) {
        return Err(unreadable("is a format this binary does not read"));
    }
    let timed_out = value
        .get("timed_out")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| unreadable("does not say whether it timed out"))?;
    let listed = value
        .get("checks")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| unreadable("lists no checks"))?;
    let mut runs = Vec::with_capacity(listed.len());
    for entry in listed {
        let field = |key: &str| entry.get(key);
        let (Some(name), Some(seconds), Some(tail)) = (
            field("name").and_then(serde_json::Value::as_str),
            field("seconds").and_then(serde_json::Value::as_u64),
            field("tail").and_then(serde_json::Value::as_str),
        ) else {
            return Err(unreadable("carries a check that is incomplete"));
        };
        runs.push(CheckRun {
            name: name.to_owned(),
            exit: field("exit")
                .and_then(serde_json::Value::as_i64)
                .and_then(|code| i32::try_from(code).ok()),
            seconds,
            tail: tail.to_owned(),
        });
    }
    Ok(Some(Outcome { timed_out, runs }))
}

/// Where a check job stands, read from its directory alone.
pub fn status(dir: &Path) -> Result<JobStatus<Outcome>, Failure> {
    job::job_status_with(dir, read_result)
}

/// Starts the runner for `dir` detached from this process and returns once it
/// holds the job's lock. A platform that refuses to detach answers
/// `review-job-detach-refused`; the checks never run in the foreground instead.
pub fn spawn(dir: &Path) -> Result<u32, Failure> {
    Detached::current().start_as("__check-job", dir)
}

/// Deletes a job's `result.json`, once what it carried has been recorded.
pub fn discard_result(dir: &Path) -> Result<(), Failure> {
    job::discard_result(dir)
}

#[cfg(test)]
mod tests;
