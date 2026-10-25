//! A reviewer run that outlives the call that asked for it.
//!
//! A job is a directory: a manifest and a prompt written before the launch, a
//! `lock` the runner holds for as long as it lives, a `result.json` it leaves
//! behind, and a `usage.json` beside it when the reviewer's CLI reported what
//! it spent. [`run`] is the runner's whole body and is what the hidden
//! `estigia __review-job <dir>` subcommand calls; [`spawn_review_job`] starts
//! that subcommand detached from the caller. Liveness is the lock and the result
//! file, never a process id: see [`job_status`].
//!
//! The runner never opens the ledger. What a result means is decided by whoever
//! collects it, not by the process that produced it.

use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{Adapter, Invocation, Receipt, Usage, run as run_cli};
use crate::outcome::Refusal;
use crate::paths::{replace_atomically, replace_bytes_atomically};
use crate::transport::Failure;

/// The version stamped into a manifest and into a result. A runner or a reader
/// that meets another version refuses rather than guess, because the binary
/// that wrote a job may not be the one that reads it.
pub const FORMAT: u32 = 1;

const MANIFEST: &str = "manifest.json";
const PROMPT: &str = "prompt.bin";
const LOCK: &str = "lock";
const STARTED: &str = "started";
pub(crate) const RESULT: &str = "result.json";
const USAGE: &str = "usage.json";
const RUNNER_OUT: &str = "runner.out";
const RUNNER_ERR: &str = "runner.err";
const STARTUP: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(10);

/// A reviewer refusal as a job records it, enough to rebuild what the caller
/// shows without the runner holding any of the caller's types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRefusal {
    /// The stable refusal code.
    pub code: String,
    /// The refusal class word.
    pub class: String,
    /// What went wrong.
    pub message: String,
    /// The way out, as the refusal serialises it.
    pub resolution: serde_json::Value,
}

/// What a finished job left in `result.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobResult {
    /// The reviewer's raw answer, unread.
    Answer(Vec<u8>),
    /// The reviewer refused.
    Refused(RecordedRefusal),
}

/// Where a job stands, read from its directory alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus<T = JobResult> {
    /// The runner holds the lock.
    Running,
    /// The runner left a result.
    Finished(T),
    /// Nothing holds the lock and no result exists: the runner died.
    Lost,
}

/// Starts a job's runner. The real one re-executes this binary detached; a
/// test's runs the same body on a thread.
pub trait Spawner {
    /// Starts the runner for `dir` and returns once it holds the lock.
    fn start(&self, dir: &Path) -> Result<u32, Failure>;
}

/// The spawner that re-executes a binary as `__review-job <dir>`, detached from
/// the caller's session and standard streams. It is this process's own binary
/// unless [`Detached::of`] names another.
pub struct Detached {
    executable: Option<std::path::PathBuf>,
    environment: Vec<(std::ffi::OsString, std::ffi::OsString)>,
}

impl Detached {
    /// Re-executes the binary this process is running.
    pub fn current() -> Self {
        Self {
            executable: None,
            environment: Vec::new(),
        }
    }

    /// Re-executes `executable`, for a caller that is not the `estigia` binary.
    pub fn of(executable: impl Into<std::path::PathBuf>) -> Self {
        Self {
            executable: Some(executable.into()),
            environment: Vec::new(),
        }
    }

    /// Sets one variable in the runner's environment.
    pub fn with_env(
        mut self,
        name: impl Into<std::ffi::OsString>,
        value: impl Into<std::ffi::OsString>,
    ) -> Self {
        self.environment.push((name.into(), value.into()));
        self
    }
}

/// The relay spawner: the host runs the reviewer, so no detached runner starts
/// and no lock is taken. The job is recorded and its prompt written, and
/// `estigia review admit` records the answer the host hands back.
pub struct Relayed;

impl Spawner for Relayed {
    fn start(&self, _dir: &Path) -> Result<u32, Failure> {
        Ok(std::process::id())
    }
}

fn unwritten(what: &str, error: impl std::fmt::Display) -> Failure {
    Failure::Read(format!("could not {what}: {error}"))
}

pub(crate) fn refused(message: String) -> Refusal {
    run_cli::launch_failed(message)
}

/// Writes a job's manifest and prompt into `dir`, creating it. The manifest goes
/// last and atomically, so a runner never reads half of one.
pub fn write_job(dir: &Path, invocation: &Invocation) -> Result<(), Failure> {
    std::fs::create_dir_all(dir).map_err(|e| unwritten("create a review job directory", e))?;
    replace_bytes_atomically(&dir.join(PROMPT), &invocation.prompt)
        .map_err(|e| unwritten("write a review job prompt", e))?;
    let receipt = &invocation.receipt;
    let manifest = serde_json::json!({
        "format": FORMAT,
        "role": invocation.role,
        "model": invocation.model,
        "receipt": {
            "epoch": receipt.epoch,
            "pr": receipt.pr,
            "head": receipt.head,
            "base": receipt.base,
            "digest": receipt.digest,
            "payload_digest": receipt.payload_digest,
        },
    });
    replace_atomically(&dir.join(MANIFEST), &manifest.to_string())
        .map_err(|e| unwritten("write a review job manifest", e))
}

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, Refusal> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| refused(format!("the review job manifest has no `{key}`")))
}

fn load(dir: &Path) -> Result<Invocation, Refusal> {
    let raw = std::fs::read(dir.join(MANIFEST))
        .map_err(|e| refused(format!("the review job manifest could not be read: {e}")))?;
    let manifest: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|e| refused(format!("the review job manifest is not JSON: {e}")))?;
    let format = manifest.get("format").and_then(serde_json::Value::as_u64);
    if format != Some(u64::from(FORMAT)) {
        return Err(refused(format!(
            "the review job manifest is format {format:?} and this runner reads {FORMAT}"
        )));
    }
    let prompt = std::fs::read(dir.join(PROMPT))
        .map_err(|e| refused(format!("the review job prompt could not be read: {e}")))?;
    let found = manifest
        .get("receipt")
        .ok_or_else(|| refused("the review job manifest has no receipt".to_owned()))?;
    Ok(Invocation {
        role: text(&manifest, "role")?.to_owned(),
        model: manifest
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        prompt,
        receipt: Receipt {
            epoch: text(found, "epoch")?.to_owned(),
            pr: found
                .get("pr")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| refused("the review job receipt has no `pr`".to_owned()))?,
            head: text(found, "head")?.to_owned(),
            base: text(found, "base")?.to_owned(),
            digest: text(found, "digest")?.to_owned(),
            payload_digest: text(found, "payload_digest")?.to_owned(),
        },
    })
}

fn hex(bytes: &[u8]) -> String {
    crate::paths::hex_digest(bytes)
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

fn result_json(outcome: &Result<Vec<u8>, Refusal>) -> serde_json::Value {
    match outcome {
        Ok(answer) => serde_json::json!({ "format": FORMAT, "answer_hex": hex(answer) }),
        Err(refusal) => serde_json::json!({
            "format": FORMAT,
            "refusal": {
                "code": refusal.code,
                "class": refusal.class.word(),
                "message": refusal.message,
                "resolution": serde_json::to_value(&refusal.resolution).unwrap_or_default(),
            },
        }),
    }
}

fn usage_json(usage: &Usage) -> serde_json::Value {
    serde_json::json!({
        "format": FORMAT,
        "provider": usage.provider,
        "counters": usage.counters,
    })
}

/// Writes the provider's own usage, or nothing when there is none to preserve.
/// An empty counter set writes nothing too, so an artifact this writer leaves is
/// always one [`read_usage`] accepts.
fn write_usage(dir: &Path, usage: Option<&Usage>) -> std::io::Result<()> {
    let Some(usage) = usage.filter(|usage| !usage.counters.is_empty()) else {
        return Ok(());
    };
    replace_atomically(&dir.join(USAGE), &usage_json(usage).to_string())
}

fn report_usage_failure(dir: &Path, error: &std::io::Error) {
    use std::io::Write as _;
    let _ = writeln!(
        std::io::stderr(),
        "the review job usage artifact {} could not be written: {error}; the reviewer's answer is \
         still the job's outcome, and its counters are unknown",
        dir.join(USAGE).display()
    );
}

pub(crate) fn take_lock(dir: &Path) -> Result<File, Refusal> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(LOCK))
        .map_err(|e| refused(format!("the review job lock could not be opened: {e}")))?;
    file.try_lock()
        .map_err(|e| refused(format!("the review job lock is held or unavailable: {e}")))?;
    Ok(file)
}

/// Takes a job's lock and marks the job started, for a runner of any kind.
pub(crate) fn begin(dir: &Path) -> Result<File, Refusal> {
    let lock = take_lock(dir)?;
    replace_atomically(&dir.join(STARTED), "")
        .map_err(|e| refused(format!("the review job could not mark itself started: {e}")))?;
    Ok(lock)
}

/// The runner: holds the job's lock for as long as it runs, asks the selected
/// adapter for the reviewer's answer exactly as a synchronous launch would, and
/// leaves `result.json` with the provider's own usage in `usage.json` beside it
/// when the reviewer's CLI reported any. The answer is the outcome whatever
/// happens to the usage: a usage write that fails is reported on the runner's
/// standard error rather than turning a review that succeeded into a refusal,
/// and the result is then written without the artifact. The usage is written
/// before the result and never inside it, so the admitted answer's bytes do not
/// move with it, and a reader of a result with no usage beside it must read the
/// counters as unknown, never as zero. It writes nothing else outside `dir`, and
/// never the ledger. A manifest it cannot read, or an adapter it cannot select,
/// is recorded as a refusal rather than left as a silent absence.
pub fn run(
    dir: &Path,
    select: &dyn Fn() -> Result<Box<dyn Adapter>, Refusal>,
) -> Result<(), Refusal> {
    let _lock = begin(dir)?;
    let outcome = load(dir).and_then(|invocation| {
        let review = select()?.review_with_usage(&invocation)?;
        if let Err(error) = write_usage(dir, review.usage.as_ref()) {
            report_usage_failure(dir, &error);
        }
        Ok(review.answer)
    });
    replace_atomically(&dir.join(RESULT), &result_json(&outcome).to_string())
        .map_err(|e| refused(format!("the review job result could not be written: {e}")))
}

fn read_result(dir: &Path) -> Result<Option<JobResult>, Failure> {
    let raw = match std::fs::read(dir.join(RESULT)) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(unwritten("read a review job result", e)),
    };
    let unreadable = |what: &str| Failure::Read(format!("the review job result {what}"));
    let value: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|_| unreadable("is not JSON"))?;
    if value.get("format").and_then(serde_json::Value::as_u64) != Some(u64::from(FORMAT)) {
        return Err(unreadable("is a format this binary does not read"));
    }
    if let Some(answer) = value.get("answer_hex").and_then(serde_json::Value::as_str) {
        return unhex(answer)
            .map(|bytes| Some(JobResult::Answer(bytes)))
            .ok_or_else(|| unreadable("carries an answer that is not hex"));
    }
    let refusal = value
        .get("refusal")
        .ok_or_else(|| unreadable("holds neither an answer nor a refusal"))?;
    let field = |key: &str| refusal.get(key).and_then(serde_json::Value::as_str);
    match (field("code"), field("class"), field("message")) {
        (Some(code), Some(class), Some(message)) => Ok(Some(JobResult::Refused(RecordedRefusal {
            code: code.to_owned(),
            class: class.to_owned(),
            message: message.to_owned(),
            resolution: refusal.get("resolution").cloned().unwrap_or_default(),
        }))),
        _ => Err(unreadable("carries a refusal that is incomplete")),
    }
}

/// Reads the provider's own usage a job preserved in `usage.json`, or `None`
/// when the directory holds no artifact — which is the unknown state: either the
/// reviewer's CLI reported no usage or the counters could not be stored, and
/// neither is a measurement. An artifact that exists but is not JSON, is another
/// format, names no provider, holds no counters or carries a counter that is not
/// a whole number is an `Err`: it is corrupt rather than unknown. An empty
/// counter set is refused the same way, because the writer never emits one. It is
/// written beside `result.json` and never inside it, so reading it never moves
/// the admitted answer.
pub fn read_usage(dir: &Path) -> Result<Option<Usage>, Failure> {
    let raw = match std::fs::read(dir.join(USAGE)) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(unwritten("read a review job usage", e)),
    };
    let unreadable = |what: &str| Failure::Read(format!("the review job usage {what}"));
    let value: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|_| unreadable("is not JSON"))?;
    if value.get("format").and_then(serde_json::Value::as_u64) != Some(u64::from(FORMAT)) {
        return Err(unreadable("is a format this binary does not read"));
    }
    let provider = value
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| unreadable("names no provider"))?;
    let counters = value
        .get("counters")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| unreadable("holds no counters"))?;
    let mut counted = std::collections::BTreeMap::new();
    for (name, count) in counters {
        let count = count
            .as_u64()
            .ok_or_else(|| unreadable("carries a counter that is not a whole number"))?;
        counted.insert(name.clone(), count);
    }
    if counted.is_empty() {
        return Err(unreadable("carries no counters"));
    }
    Ok(Some(Usage {
        provider: provider.to_owned(),
        counters: counted,
    }))
}

/// Reads where a job stands. A result file decides first. Otherwise a held lock
/// is a live runner, and a free one with no result is a runner that died,
/// checked once more after taking the lock so a runner that finished between
/// the two reads is not called lost.
pub fn job_status(dir: &Path) -> Result<JobStatus, Failure> {
    job_status_with(dir, read_result)
}

/// [`job_status`] over a result file another reader decodes.
pub(crate) fn job_status_with<T>(
    dir: &Path,
    read: impl Fn(&Path) -> Result<Option<T>, Failure>,
) -> Result<JobStatus<T>, Failure> {
    if let Some(result) = read(dir)? {
        return Ok(JobStatus::Finished(result));
    }
    let lock = match File::options().read(true).write(true).open(dir.join(LOCK)) {
        Ok(lock) => lock,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(JobStatus::Lost),
        Err(e) => return Err(unwritten("open a review job lock", e)),
    };
    match lock.try_lock() {
        Err(std::fs::TryLockError::WouldBlock) => Ok(JobStatus::Running),
        Err(std::fs::TryLockError::Error(e)) => Err(unwritten("test a review job lock", e)),
        Ok(()) => Ok(match read(dir)? {
            Some(result) => JobStatus::Finished(result),
            None => JobStatus::Lost,
        }),
    }
}

pub(crate) fn await_started(
    dir: &Path,
    mut dead: impl FnMut() -> Option<String>,
) -> Result<(), Failure> {
    let began = Instant::now();
    loop {
        if dir.join(STARTED).exists() || dir.join(RESULT).exists() {
            return Ok(());
        }
        if let Some(reason) = dead() {
            if dir.join(STARTED).exists() || dir.join(RESULT).exists() {
                return Ok(());
            }
            return Err(Failure::Read(format!(
                "the review runner stopped before it started ({reason}): {}",
                std::fs::read_to_string(dir.join(RUNNER_ERR))
                    .unwrap_or_default()
                    .trim()
            )));
        }
        if began.elapsed() > STARTUP {
            return Err(Failure::Write(format!(
                "the review runner did not report in within {STARTUP:?}; it may still be starting"
            )));
        }
        std::thread::sleep(POLL);
    }
}

fn detach_refused(detail: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "review-job-detach-refused",
        "detail": detail,
        "action": "this process may not start a detached child, so the reviewer was not launched: \
                   run the review from a session that is not confined to a job, and do not \
                   expect it to run in the foreground instead",
    }))
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn refused_to_detach(error: &std::io::Error) -> bool {
    cfg!(windows) && error.raw_os_error() == Some(5)
}

impl Spawner for Detached {
    fn start(&self, dir: &Path) -> Result<u32, Failure> {
        self.start_as("__review-job", dir)
    }
}

impl Detached {
    /// Starts `subcommand <dir>` of the re-executed binary, detached, and
    /// returns once it reports in.
    pub(crate) fn start_as(&self, subcommand: &str, dir: &Path) -> Result<u32, Failure> {
        let executable = match &self.executable {
            Some(named) => named.clone(),
            None => std::env::current_exe().map_err(|e| unwritten("find this executable", e))?,
        };
        let open = |name: &str| {
            File::options()
                .create(true)
                .append(true)
                .open(dir.join(name))
                .map_err(|e| unwritten("open a runner log", e))
        };
        let mut command = Command::new(executable);
        command
            .arg(subcommand)
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(open(RUNNER_OUT)?)
            .stderr(open(RUNNER_ERR)?)
            .envs(self.environment.iter().map(|(k, v)| (k, v)));
        detach(&mut command);
        let mut child = command.spawn().map_err(|e| {
            if refused_to_detach(&e) {
                detach_refused(&e.to_string())
            } else {
                unwritten("start the review runner", e)
            }
        })?;
        let pid = child.id();
        let started = await_started(dir, || {
            child
                .try_wait()
                .ok()
                .flatten()
                .map(|status| status.to_string())
        });
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        started.map(|()| pid)
    }
}

/// The prompt bytes a job holds, for a host that will run the reviewer itself.
pub fn prompt_of(dir: &Path) -> Result<Vec<u8>, Failure> {
    std::fs::read(dir.join(PROMPT)).map_err(|e| unwritten("read a review job prompt", e))
}

/// The model a job's launch recorded, or `None` when the `reviewer` routing row
/// named none and the host's own model runs the reviewer.
pub fn model_of(dir: &Path) -> Result<Option<String>, Failure> {
    let raw = std::fs::read(dir.join(MANIFEST))
        .map_err(|e| unwritten("read a review job manifest", e))?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&raw).map_err(|e| unwritten("parse a review job manifest", e))?;
    Ok(manifest
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

/// Deletes a job's `result.json`, once what it carried has been recorded. It
/// leaves `usage.json` and every other file in the directory: the usage holds no
/// answer bytes, and leaving it is what keeps it readable after admission.
pub fn discard_result(dir: &Path) -> Result<(), Failure> {
    match std::fs::remove_file(dir.join(RESULT)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(unwritten("delete a review job result", e)),
    }
}

/// Kills the process group a runner leads: its id is its group's, because the
/// runner started a session of its own.
#[cfg(unix)]
pub fn kill_group(pid: u32) {
    if let Ok(pid) = i32::try_from(pid)
        && pid > 1
    {
        unsafe { libc::kill(-pid, libc::SIGKILL) };
    }
}

/// Kills the runner and what it started.
#[cfg(windows)]
pub fn kill_group(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Starts the runner for `dir` detached from this process and returns its
/// process id once the runner holds the job's lock. The id is for a later kill
/// and is never evidence the runner is alive.
pub fn spawn_review_job(dir: &Path) -> Result<u32, Failure> {
    Detached::current().start(dir)
}

/// A spawner that runs the runner's body on a thread of this process, for tests
/// that cannot cross a process boundary.
#[cfg(test)]
pub(crate) struct OnThread<F>(pub(crate) F);

#[cfg(test)]
impl<F> Spawner for OnThread<F>
where
    F: Fn() -> Result<Box<dyn Adapter>, Refusal> + Clone + Send + 'static,
{
    fn start(&self, dir: &Path) -> Result<u32, Failure> {
        let select = self.0.clone();
        let owned = dir.to_path_buf();
        let handle = std::thread::spawn(move || run(&owned, &select));
        await_started(dir, || handle.is_finished().then(|| "finished".to_owned()))?;
        Ok(std::process::id())
    }
}

#[cfg(test)]
mod tests;
