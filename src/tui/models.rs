//! Advisory model catalogs that require asking a coding-agent host.
//!
//! Only OpenCode is dynamic. Claude Code and Codex are curated on their
//! [`AgentAdapter`](crate::setup::AgentAdapter), and every other adapter says it
//! has no verified catalog rather than borrowing one. None of these lists
//! validates configuration: model IDs stay opaque and Estigia never runs them.

use std::io::Read;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::config::ModelRouting;
use crate::setup::{AgentAdapter, ModelCatalogSource};

const OPENCODE_ARGS: &[&str] = &["models"];
const OPENCODE_TIMEOUT: Duration = Duration::from_secs(5);
const PIPE_CLOSE_GRACE: Duration = Duration::from_millis(100);
const CLEANUP_GRACE: Duration = Duration::from_millis(500);
const CONTROLLER_GRACE: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_CAPTURED_OUTPUT: usize = 1024 * 1024;
/// Maximum production wait: five seconds resolving, validating, spawning and
/// running, 500 ms cleaning the tree and readers, then 100 ms for the
/// controller result. OS cleanup is best effort; this bound is enforced by the
/// caller rather than by joining worker threads.
const OPENCODE_OVERALL_BOUND: Duration = Duration::from_millis(5600);

/// Why an advisory catalog could not be read.
#[derive(Debug)]
pub(super) struct CatalogError(String);

impl std::fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Loads the advisory list owned by this adapter.
pub(super) fn load(adapter: &AgentAdapter) -> Result<Vec<String>, CatalogError> {
    match adapter.model_catalog() {
        ModelCatalogSource::Curated(models) => {
            Ok(models.iter().map(|model| (*model).to_owned()).collect())
        }
        ModelCatalogSource::None => Ok(Vec::new()),
        ModelCatalogSource::OpenCode => load_opencode_with(
            OPENCODE_TIMEOUT,
            || crate::setup::resolve_on_path("opencode"),
            |program, run_deadline, cleanup_deadline| {
                run_probe(
                    program,
                    OPENCODE_ARGS.iter().map(|arg| (*arg).to_owned()).collect(),
                    OPENCODE_TIMEOUT,
                    None,
                    run_deadline,
                    cleanup_deadline,
                )
            },
        ),
    }
}

fn load_opencode_with<R, L>(
    timeout: Duration,
    resolver: R,
    launch: L,
) -> Result<Vec<String>, CatalogError>
where
    R: FnOnce() -> Option<PathBuf> + Send + 'static,
    L: FnOnce(PathBuf, Instant, Instant) -> Result<Vec<String>, CatalogError> + Send + 'static,
{
    controlled(
        "`opencode` catalog",
        timeout,
        move |run_deadline, cleanup_deadline| {
            let program = resolver().ok_or_else(|| {
                CatalogError("`opencode` was not found on an absolute PATH entry".to_owned())
            })?;
            if Instant::now() >= run_deadline {
                return Err(CatalogError(format!(
                    "`opencode` PATH resolution did not finish within {}s",
                    timeout.as_secs_f64()
                )));
            }
            if !process_tree::launcher_can_be_contained(&program) {
                return Err(CatalogError(format!(
                    "{} is a script launcher, so its process tree cannot be bounded",
                    program.display()
                )));
            }
            launch(program, run_deadline, cleanup_deadline)
        },
    )
}

#[cfg(test)]
fn probe(program: &Path, args: &[&str], timeout: Duration) -> Result<Vec<String>, CatalogError> {
    probe_with_env(program, args, timeout, None)
}

#[cfg(test)]
fn probe_with_env(
    program: &Path,
    args: &[&str],
    timeout: Duration,
    environment: Option<(&str, &str)>,
) -> Result<Vec<String>, CatalogError> {
    let program = program.to_path_buf();
    let shown = program.display().to_string();
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let environment = environment.map(|(name, value)| (name.to_owned(), value.to_owned()));
    controlled(
        &format!("{shown} probe"),
        timeout,
        move |run_deadline, cleanup_deadline| {
            run_probe(
                program,
                args,
                timeout,
                environment,
                run_deadline,
                cleanup_deadline,
            )
        },
    )
}

fn controlled<F>(shown: &str, timeout: Duration, operation: F) -> Result<Vec<String>, CatalogError>
where
    F: FnOnce(Instant, Instant) -> Result<Vec<String>, CatalogError> + Send + 'static,
{
    let overall = if timeout == OPENCODE_TIMEOUT {
        OPENCODE_OVERALL_BOUND
    } else {
        timeout
            .saturating_add(CLEANUP_GRACE)
            .saturating_add(CONTROLLER_GRACE)
    };
    let started = Instant::now();
    let run_deadline = started + timeout;
    let cleanup_deadline = run_deadline + CLEANUP_GRACE;
    let controller_deadline = started + overall;
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = operation(run_deadline, cleanup_deadline);
        let _ = sender.send(result);
    });
    let remaining = controller_deadline.saturating_duration_since(Instant::now());
    receiver.recv_timeout(remaining).unwrap_or_else(|error| {
        Err(CatalogError(format!(
            "{shown} controller did not return within {:.1}s: {error}",
            overall.as_secs_f64()
        )))
    })
}

fn run_probe(
    program: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    environment: Option<(String, String)>,
    run_deadline: Instant,
    overall_deadline: Instant,
) -> Result<Vec<String>, CatalogError> {
    if Instant::now() >= run_deadline {
        return Err(CatalogError(format!(
            "{} did not start within {}s",
            program.display(),
            timeout.as_secs_f64()
        )));
    }
    let mut command = Command::new(&program);
    command.args(&args).stdin(Stdio::null());
    if let Some((name, value)) = environment {
        command.env(name, value);
    }
    let prepared = process_tree::prepare(&mut command).map_err(CatalogError)?;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| CatalogError(format!("{} would not start: {error}", program.display())))?;

    let (sender, receiver) = mpsc::channel();
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let mut failure = match (stdout_pipe.is_some(), stderr_pipe.is_some()) {
        (true, true) => None,
        (false, false) => Some("OpenCode stdout and stderr were not piped".to_owned()),
        (false, true) => Some("OpenCode stdout was not piped".to_owned()),
        (true, false) => Some("OpenCode stderr was not piped".to_owned()),
    };
    if let Some(stdout) = stdout_pipe {
        spawn_reader(Stream::Stdout, stdout, sender.clone());
    }
    if let Some(stderr) = stderr_pipe {
        spawn_reader(Stream::Stderr, stderr, sender);
    }

    let tree = match prepared.attach(&child) {
        Ok(tree) => Some(tree),
        Err(error) => {
            failure = Some(format!(
                "could not contain the OpenCode process tree: {error}"
            ));
            None
        }
    };
    let mut status = None;
    let mut exited_at = None;
    let mut stdout = None;
    let mut stderr = None;
    let mut cleanup_notes = Vec::new();
    let mut cleanup_started = false;
    let mut cleanup_deadline = overall_deadline;
    let mut reaped = false;
    let mut reap_error_recorded = false;
    let mut tree_terminated = false;

    loop {
        collect_reader_results(&receiver, &mut stdout, &mut stderr);
        if status.is_none() {
            match child.try_wait() {
                Ok(Some(found)) => {
                    status = Some(found);
                    exited_at = Some(Instant::now());
                    reaped = true;
                }
                Ok(None) => {}
                Err(error) => {
                    failure.get_or_insert_with(|| {
                        format!("{} could not be waited on: {error}", program.display())
                    });
                }
            }
        }

        if status.is_some() && !tree_terminated {
            tree_terminated = true;
            if let Some(tree) = &tree
                && let Err(error) = tree.terminate()
            {
                failure = Some(format!(
                    "could not terminate the completed OpenCode process tree: {error}"
                ));
            }
        }

        let now = Instant::now();
        if failure.is_none() && status.is_some() && stdout.is_some() && stderr.is_some() {
            break;
        }
        if failure.is_none() && reader_failed(&stdout, &stderr) {
            failure = Some("OpenCode output could not be read".to_owned());
        }
        if failure.is_none() && now >= run_deadline {
            failure = Some(if status.is_none() {
                format!(
                    "{} did not finish within {}s",
                    program.display(),
                    timeout.as_secs_f64()
                )
            } else {
                format!(
                    "{} left output streams open after its direct process exited",
                    program.display()
                )
            });
        }
        if failure.is_none()
            && exited_at.is_some_and(|exited| now >= exited + PIPE_CLOSE_GRACE)
            && (stdout.is_none() || stderr.is_none())
        {
            failure = Some(format!(
                "{} left output streams open after its direct process exited",
                program.display()
            ));
        }

        if failure.is_some() && !cleanup_started {
            cleanup_started = true;
            cleanup_deadline = (now + CLEANUP_GRACE).min(overall_deadline);
            if !tree_terminated
                && let Some(tree) = &tree
                && let Err(error) = tree.terminate()
            {
                cleanup_notes.push(format!("tree cleanup failed: {error}"));
            }
            tree_terminated = true;
            if !reaped && let Err(error) = child.kill() {
                cleanup_notes.push(format!("direct child kill failed: {error}"));
            }
        }
        if cleanup_started && !reaped {
            match child.try_wait() {
                Ok(Some(found)) => {
                    status = Some(found);
                    reaped = true;
                }
                Ok(None) => {}
                Err(error) if !reap_error_recorded => {
                    cleanup_notes.push(format!("direct child reap failed: {error}"));
                    reap_error_recorded = true;
                }
                Err(_) => {}
            }
        }
        if cleanup_started && now >= cleanup_deadline {
            break;
        }

        let next_deadline = if cleanup_started {
            cleanup_deadline
        } else {
            exited_at
                .map(|exited| (exited + PIPE_CLOSE_GRACE).min(run_deadline))
                .unwrap_or(run_deadline)
        };
        std::thread::sleep(POLL_INTERVAL.min(next_deadline.saturating_duration_since(now)));
    }

    collect_reader_results(&receiver, &mut stdout, &mut stderr);
    if !reaped {
        hand_off_reaping(child);
    }
    if let Some(mut failure) = failure {
        append_reader_diagnostics(&mut failure, &stdout, &stderr);
        for note in cleanup_notes {
            failure.push_str("; ");
            failure.push_str(&note);
        }
        return Err(CatalogError(failure));
    }

    let (stdout, stderr) = observed_outputs(stdout, stderr)?;
    let status =
        status.ok_or_else(|| CatalogError("OpenCode status was not observed".to_owned()))?;
    if !status.success() {
        return Err(CatalogError(format!(
            "{} failed ({}): {}",
            program.display(),
            status.code().unwrap_or(-1),
            safe_detail(&stderr.bytes)
        )));
    }
    if stdout.truncated {
        return Err(CatalogError(format!(
            "{} stdout exceeded the {} byte advisory catalog limit",
            program.display(),
            MAX_CAPTURED_OUTPUT
        )));
    }
    parse(&stdout.bytes)
}

#[derive(Debug)]
struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug, Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

type ReaderResult = std::io::Result<CapturedOutput>;

fn spawn_reader(
    stream: Stream,
    pipe: impl Read + Send + 'static,
    sender: mpsc::Sender<(Stream, ReaderResult)>,
) {
    std::thread::spawn(move || {
        let _ = sender.send((stream, read_all(pipe)));
    });
}

fn read_all(mut pipe: impl Read) -> ReaderResult {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut chunk = [0_u8; 8192];
    loop {
        let read = pipe.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        // Keep draining after the cap so the child cannot block on a full pipe,
        // but do not let an advisory command allocate without limit.
        let kept = read.min(MAX_CAPTURED_OUTPUT.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&chunk[..kept]);
        truncated |= kept < read;
    }
    Ok(CapturedOutput { bytes, truncated })
}

fn collect_reader_results(
    receiver: &mpsc::Receiver<(Stream, ReaderResult)>,
    stdout: &mut Option<ReaderResult>,
    stderr: &mut Option<ReaderResult>,
) {
    while let Ok((stream, result)) = receiver.try_recv() {
        match stream {
            Stream::Stdout => *stdout = Some(result),
            Stream::Stderr => *stderr = Some(result),
        }
    }
}

fn reader_failed(stdout: &Option<ReaderResult>, stderr: &Option<ReaderResult>) -> bool {
    stdout.as_ref().is_some_and(Result::is_err) || stderr.as_ref().is_some_and(Result::is_err)
}

fn append_reader_diagnostics(
    message: &mut String,
    stdout: &Option<ReaderResult>,
    stderr: &Option<ReaderResult>,
) {
    for (name, outcome) in [("stdout", stdout), ("stderr", stderr)] {
        match outcome {
            Some(Err(error)) => {
                message.push_str(&format!("; OpenCode {name} read failed: {error}"))
            }
            None => message.push_str(&format!("; OpenCode {name} did not close within the bound")),
            Some(Ok(_)) => {}
        }
    }
}

fn observed_outputs(
    stdout: Option<ReaderResult>,
    stderr: Option<ReaderResult>,
) -> Result<(CapturedOutput, CapturedOutput), CatalogError> {
    let mut errors = Vec::new();
    let stdout = match stdout {
        Some(Ok(output)) => Some(output),
        Some(Err(error)) => {
            errors.push(format!("OpenCode stdout could not be read: {error}"));
            None
        }
        None => {
            errors.push("OpenCode stdout was not observed".to_owned());
            None
        }
    };
    let stderr = match stderr {
        Some(Ok(output)) => Some(output),
        Some(Err(error)) => {
            errors.push(format!("OpenCode stderr could not be read: {error}"));
            None
        }
        None => {
            errors.push("OpenCode stderr was not observed".to_owned());
            None
        }
    };
    match (stdout, stderr) {
        (Some(stdout), Some(stderr)) => Ok((stdout, stderr)),
        _ => Err(CatalogError(errors.join("; "))),
    }
}

fn hand_off_reaping(mut child: Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

fn parse(stdout: &[u8]) -> Result<Vec<String>, CatalogError> {
    let text = std::str::from_utf8(stdout)
        .map_err(|error| CatalogError(format!("OpenCode stdout was not UTF-8: {error}")))?;
    let mut models: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.chars().any(char::is_control)
                && ModelRouting::accepts_model_id(line)
                && line
                    .split_once('/')
                    .is_some_and(|(provider, model)| !provider.is_empty() && !model.is_empty())
        })
        .map(str::to_owned)
        .collect();
    models.sort();
    models.dedup();
    if models.is_empty() {
        Err(CatalogError(
            "OpenCode stdout contained no model IDs".to_owned(),
        ))
    } else {
        Ok(models)
    }
}

fn safe_detail(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .chars()
        .filter_map(|character| {
            if character.is_control() {
                character.is_whitespace().then_some(' ')
            } else {
                Some(character)
            }
        })
        .take(240)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(unix)]
mod process_tree {
    use std::path::Path;
    use std::process::{Child, Command};

    use std::os::unix::process::CommandExt;

    pub(super) struct Prepared;

    pub(super) struct Owner {
        group: i32,
    }

    pub(super) fn prepare(command: &mut Command) -> Result<Prepared, String> {
        command.process_group(0);
        Ok(Prepared)
    }

    impl Prepared {
        pub(super) fn attach(self, child: &Child) -> Result<Owner, String> {
            let group = i32::try_from(child.id())
                .map_err(|_| format!("child PID {} does not fit a process group", child.id()))?;
            Ok(Owner { group })
        }
    }

    impl Owner {
        pub(super) fn terminate(&self) -> Result<(), String> {
            // SAFETY: the child was placed in a fresh process group whose ID is
            // its PID. A negative PID addresses that group and no other one.
            if unsafe { libc::kill(-self.group, libc::SIGKILL) } == 0 {
                return Ok(());
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(error.to_string())
            }
        }
    }

    pub(super) fn launcher_can_be_contained(_program: &Path) -> bool {
        true
    }
}

#[cfg(windows)]
mod process_tree {
    use std::ffi::c_void;
    use std::mem;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;
    use std::process::{Child, Command};
    use std::ptr;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    pub(super) struct Prepared {
        job: Job,
    }

    pub(super) struct Owner {
        job: Job,
    }

    struct Job(HANDLE);

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: this type exclusively owns the handle returned by
            // CreateJobObjectW and closes it exactly once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    pub(super) fn prepare(_command: &mut Command) -> Result<Prepared, String> {
        // SAFETY: null security attributes and name request an unnamed job with
        // the caller's default security descriptor.
        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let job = Job(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` has the exact layout and size required by the named
        // information class and remains alive for the duration of the call.
        let configured = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&limits).cast::<c_void>(),
                mem::size_of_val(&limits) as u32,
            )
        };
        if configured == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Prepared { job })
    }

    impl Prepared {
        pub(super) fn attach(self, child: &Child) -> Result<Owner, String> {
            // SAFETY: Child owns a live process handle for this call; the job
            // remains owned by `self` until assignment succeeds.
            let assigned = unsafe {
                AssignProcessToJobObject(self.job.0, child.as_raw_handle().cast::<c_void>())
            };
            if assigned == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Owner { job: self.job })
        }
    }

    impl Owner {
        pub(super) fn terminate(&self) -> Result<(), String> {
            // SAFETY: `job` is a live handle owned by this value.
            if unsafe { TerminateJobObject(self.job.0, 1) } != 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error().to_string())
            }
        }
    }

    pub(super) fn launcher_can_be_contained(program: &Path) -> bool {
        program
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("com")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FIXTURE: &str = "ESTIGIA_MODEL_CATALOG_FIXTURE";
    const FIXTURE_MARKER: &str = "ESTIGIA_MODEL_CATALOG_MARKER";
    const FIXTURE_TEST: &str = "tui::models::tests::catalog_process_fixture";
    const DESCENDANT_STARTUP_BOUND: Duration = Duration::from_secs(2);
    const DESCENDANT_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
    const DESCENDANT_SURVIVAL_PROOF_BOUND: Duration = Duration::from_secs(2);

    #[test]
    fn catalog_process_fixture() {
        let fixture_mode = std::env::var(FIXTURE).unwrap_or_default();
        let (mode, marker) = fixture_mode
            .split_once('|')
            .map_or((fixture_mode.as_str(), None), |(mode, marker)| {
                (mode, Some(marker))
            });
        match mode {
            "catalog-child-success" => {
                // Larger than a typical pipe, so waiting before draining would
                // hang this fixture instead of reaching the model lines.
                let mut stderr = std::io::stderr().lock();
                for _ in 0..2048 {
                    writeln!(
                        stderr,
                        "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                    )
                    .expect("fixture stderr");
                }
                println!("zeta/model-z");
                println!("alpha/model-a");
                println!("zeta/model-z");
                println!("not a model");
                println!("bad/\u{1b}[31m");
                println!("bad/model,other");
                println!("bad/model|other");
            }
            "catalog-child-nonzero" => std::process::exit(7),
            "catalog-child-timeout" => std::thread::sleep(Duration::from_secs(5)),
            "catalog-child-stdin-eof" => {
                let mut input = Vec::new();
                std::io::stdin()
                    .read_to_end(&mut input)
                    .expect("fixture stdin");
                println!("alpha/model-a");
            }
            "catalog-child-oversized" => {
                let mut stdout = std::io::stdout().lock();
                writeln!(stdout, "partial/model").expect("fixture stdout");
                let chunk = [b'x'; 8192];
                for _ in 0..=(MAX_CAPTURED_OUTPUT / chunk.len()) {
                    stdout.write_all(&chunk).expect("fixture stdout");
                }
            }
            "catalog-child-invalid-utf8" => {
                std::io::stdout()
                    .write_all(b"provider/\xff\n")
                    .expect("fixture stdout");
            }
            "catalog-child-success-with-descendant" => {
                spawn_descendant(marker, true);
                let mut stdout = std::io::stdout().lock();
                writeln!(stdout, "alpha/model-a").expect("fixture stdout");
                stdout.flush().expect("flush fixture stdout");
                std::process::exit(0);
            }
            "catalog-child-parent-exits" => spawn_descendant(marker, false),
            "catalog-child-parent-times-out" => {
                spawn_descendant(marker, false);
                std::thread::sleep(Duration::from_secs(10));
            }
            "catalog-child-descendant" => {
                let directory = std::env::var(FIXTURE_MARKER).expect("a descendant marker");
                let directory = std::path::Path::new(&directory);
                std::fs::write(directory.join("started"), "started")
                    .expect("write descendant start marker");
                let deadline = Instant::now() + Duration::from_secs(10);
                while !directory.join("prove-survival").exists() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(10));
                }
                if directory.join("prove-survival").exists() {
                    std::thread::sleep(Duration::from_millis(100));
                    std::fs::write(directory.join("survived"), "descendant survived")
                        .expect("write descendant survival marker");
                }
            }
            "catalog-child-stdin-driver" => {
                assert_eq!(
                    fixture("stdin-eof", Duration::from_secs(2))
                        .expect("the nested probe sees EOF"),
                    ["alpha/model-a"]
                );
            }
            _ => {}
        }
    }

    #[expect(
        clippy::zombie_processes,
        reason = "the fixture must let its parent exit while its owned descendant remains alive"
    )]
    fn spawn_descendant(marker: Option<&str>, close_output: bool) {
        let executable = std::env::current_exe().expect("the test executable");
        let marker = marker.expect("a descendant marker");
        let mut command = Command::new(executable);
        command
            .args(["--exact", FIXTURE_TEST, "--nocapture"])
            .env(FIXTURE, "catalog-child-descendant")
            .env(FIXTURE_MARKER, marker)
            .stdin(Stdio::null());
        if close_output {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        command.spawn().expect("spawn an owned descendant");

        let directory = std::path::Path::new(marker);
        let started = directory.join("started");
        let deadline = Instant::now() + DESCENDANT_STARTUP_BOUND;
        while !started.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(started.exists(), "the descendant did not start");
        std::fs::write(directory.join("ready"), "parent observed descendant")
            .expect("write parent readiness marker");
    }

    fn assert_descendant_ready(directory: &std::path::Path, mode: &str) {
        assert!(
            directory.join("started").exists(),
            "{mode} never started the descendant under test"
        );
        assert!(
            directory.join("ready").exists(),
            "{mode} reached cleanup before its parent observed descendant readiness"
        );
    }

    fn assert_descendant_did_not_survive(directory: &std::path::Path, mode: &str) {
        std::fs::write(directory.join("prove-survival"), "answer if alive")
            .expect("write the survival challenge");
        let deadline = Instant::now() + DESCENDANT_SURVIVAL_PROOF_BOUND;
        while Instant::now() < deadline {
            assert!(
                !directory.join("survived").exists(),
                "{mode} left a descendant alive after tree cleanup"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn fixture(mode: &str, timeout: Duration) -> Result<Vec<String>, CatalogError> {
        let executable = std::env::current_exe().expect("the test executable");
        let mode = format!("catalog-child-{mode}");
        probe_with_env(
            &executable,
            &["--exact", FIXTURE_TEST, "--nocapture"],
            timeout,
            Some((FIXTURE, &mode)),
        )
    }

    #[test]
    fn success_is_sorted_deduplicated_and_drains_both_pipes() {
        assert_eq!(
            fixture("success", Duration::from_secs(3)).expect("the fixture succeeds"),
            ["alpha/model-a", "zeta/model-z"]
        );
    }

    #[test]
    fn captured_output_is_bounded() {
        let output = vec![b'x'; MAX_CAPTURED_OUTPUT + 8192];
        let captured = read_all(output.as_slice()).expect("memory input is readable");
        assert_eq!(captured.bytes.len(), MAX_CAPTURED_OUTPUT);
        assert!(captured.truncated);
    }

    #[test]
    fn a_successful_catalog_larger_than_the_cap_is_unavailable_without_a_partial_id() {
        let started = Instant::now();
        let error = fixture("oversized", Duration::from_secs(2))
            .expect_err("a truncated catalog cannot be parsed as complete");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "draining excess output exceeded the probe bound: {error}"
        );
    }

    #[test]
    fn invalid_utf8_is_an_unavailable_catalog() {
        assert!(fixture("invalid-utf8", Duration::from_secs(2)).is_err());
    }

    #[test]
    fn a_blocked_resolver_returns_at_the_bound_without_launching_late() {
        let (resolved_sender, resolved_receiver) = mpsc::sync_channel(1);
        let (launch_sender, launch_receiver) = mpsc::sync_channel(1);
        let executable = std::env::current_exe().expect("the test executable");
        let started = Instant::now();
        let result = load_opencode_with(
            Duration::from_millis(100),
            move || {
                std::thread::sleep(Duration::from_millis(800));
                let _ = resolved_sender.send(());
                Some(executable)
            },
            move |_, _, _| {
                let _ = launch_sender.send(());
                Err(CatalogError("the expired resolver launched".to_owned()))
            },
        );
        assert!(result.is_err());
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "PATH resolution escaped the controller bound: {:?}",
            started.elapsed()
        );
        resolved_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("the resolver never completed after the controller returned");
        assert_eq!(
            launch_receiver.recv_timeout(Duration::from_secs(1)),
            Err(mpsc::RecvTimeoutError::Disconnected),
            "an expired resolver proceeded to launch OpenCode",
        );
    }

    #[test]
    fn stdin_is_closed_even_when_the_callers_stdin_stays_open() {
        let executable = std::env::current_exe().expect("the test executable");
        let mut driver = Command::new(executable)
            .args(["--exact", FIXTURE_TEST, "--nocapture"])
            .env(FIXTURE, "catalog-child-stdin-driver")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the stdin driver");
        let _kept_open = driver.stdin.take().expect("driver stdin is piped");
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            match driver.try_wait().expect("wait for the stdin driver") {
                Some(status) => {
                    assert!(status.success(), "the nested probe inherited open stdin");
                    break;
                }
                None if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                None => {
                    let _ = driver.kill();
                    let _ = driver.wait();
                    panic!("the nested probe did not return while stdin stayed open");
                }
            }
        }
    }

    #[test]
    fn a_successful_probe_terminates_descendants_that_close_both_pipes() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let executable = std::env::current_exe().expect("the test executable");
        let result = probe_with_env(
            &executable,
            &["--exact", FIXTURE_TEST, "--nocapture"],
            DESCENDANT_PROBE_TIMEOUT,
            Some((
                FIXTURE,
                &format!(
                    "catalog-child-success-with-descendant|{}",
                    directory.path().display()
                ),
            )),
        )
        .expect("the direct process returned a complete catalog");
        assert_eq!(result, ["alpha/model-a"]);
        assert_descendant_ready(directory.path(), "successful probe");
        assert_descendant_did_not_survive(directory.path(), "successful probe");
    }

    #[test]
    fn descendants_holding_both_pipes_cannot_outlive_the_probe_bound() {
        for mode in ["parent-exits", "parent-times-out"] {
            let directory = tempfile::tempdir().expect("a temporary directory");
            let executable = std::env::current_exe().expect("the test executable");
            let started = Instant::now();
            let result = probe_with_env(
                &executable,
                &["--exact", FIXTURE_TEST, "--nocapture"],
                DESCENDANT_PROBE_TIMEOUT,
                Some((
                    FIXTURE,
                    &format!("catalog-child-{mode}|{}", directory.path().display()),
                )),
            );
            assert!(result.is_err(), "{mode} was accepted as a complete catalog");
            assert!(
                started.elapsed() < Duration::from_secs(7),
                "{mode} kept the probe blocked for {:?}",
                started.elapsed()
            );
            assert_descendant_ready(directory.path(), mode);
            assert_descendant_did_not_survive(directory.path(), mode);
        }
    }

    #[test]
    fn missing_nonzero_and_timeout_are_unavailable_catalogs() {
        let missing = tempfile::tempdir()
            .expect("a temporary directory")
            .path()
            .join("not-opencode");
        assert!(probe(&missing, OPENCODE_ARGS, Duration::from_millis(20)).is_err());
        assert!(fixture("nonzero", Duration::from_secs(3)).is_err());
        let started = Instant::now();
        assert!(fixture("timeout", Duration::from_millis(50)).is_err());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the timed-out process was not killed and reaped"
        );
    }

    #[test]
    fn the_host_probe_never_refreshes_implicitly() {
        assert_eq!(OPENCODE_ARGS, ["models"]);
        assert_eq!(
            OPENCODE_OVERALL_BOUND,
            OPENCODE_TIMEOUT + CLEANUP_GRACE + CONTROLLER_GRACE
        );
    }

    #[cfg(windows)]
    #[test]
    fn script_launchers_are_refused_when_their_tree_cannot_be_owned() {
        assert!(!process_tree::launcher_can_be_contained(Path::new(
            "opencode.cmd"
        )));
        assert!(process_tree::launcher_can_be_contained(Path::new(
            "opencode.exe"
        )));
    }
}
