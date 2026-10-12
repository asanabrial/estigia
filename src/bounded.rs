//! Runs one program under a deadline and returns what it printed.
//!
//! The caller gets an answer or a reason within the bound, never a wait on the
//! program: the run happens on a worker thread, the process tree is contained
//! and terminated, and a controller that does not hear back in time answers
//! for it. Shared by the advisory model catalog and the doctor's `gh` check.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::reviewer::process_tree;

const PIPE_CLOSE_GRACE: Duration = Duration::from_millis(100);
pub(crate) const CLEANUP_GRACE: Duration = Duration::from_millis(500);
pub(crate) const CONTROLLER_GRACE: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
pub(crate) const MAX_CAPTURED_OUTPUT: usize = 1024 * 1024;

/// What a program that finished in time printed, and how it ended.
#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) stdout_truncated: bool,
}

/// Runs `program` and returns its output, or why there is none.
pub(crate) fn run(
    shown: &str,
    program: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    environment: Option<(String, String)>,
) -> Result<Output, String> {
    #[cfg(test)]
    let (program, overridden) = match program
        .to_str()
        .and_then(crate::transport::overridden_program)
    {
        Some((resolved, envs)) => (resolved, envs),
        None => (program, Vec::new()),
    };
    #[cfg(not(test))]
    let overridden = Vec::new();
    controlled(shown, timeout, move |run_deadline, cleanup_deadline| {
        run_process_with(
            program,
            args,
            timeout,
            environment,
            overridden,
            run_deadline,
            cleanup_deadline,
        )
    })
}

/// Runs `operation` on a worker thread and answers for it at the bound.
pub(crate) fn controlled<T, F>(shown: &str, timeout: Duration, operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(Instant, Instant) -> Result<T, String> + Send + 'static,
{
    let overall = timeout
        .saturating_add(CLEANUP_GRACE)
        .saturating_add(CONTROLLER_GRACE);
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
        Err(format!(
            "{shown} controller did not return within {:.1}s: {error}",
            overall.as_secs_f64()
        ))
    })
}

pub(crate) fn run_process(
    program: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    environment: Option<(String, String)>,
    run_deadline: Instant,
    overall_deadline: Instant,
) -> Result<Output, String> {
    run_process_with(
        program,
        args,
        timeout,
        environment,
        Vec::new(),
        run_deadline,
        overall_deadline,
    )
}

fn run_process_with(
    program: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    environment: Option<(String, String)>,
    inherited: Vec<(String, std::ffi::OsString)>,
    run_deadline: Instant,
    overall_deadline: Instant,
) -> Result<Output, String> {
    if Instant::now() >= run_deadline {
        return Err(format!(
            "{} did not start within {}s",
            program.display(),
            timeout.as_secs_f64()
        ));
    }
    let mut command = Command::new(&program);
    command.args(&args).stdin(Stdio::null());
    for (name, value) in inherited {
        command.env(name, value);
    }
    if let Some((name, value)) = environment {
        command.env(name, value);
    }
    let prepared = process_tree::prepare(&mut command)?;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{} would not start: {error}", program.display()))?;

    let (sender, receiver) = mpsc::channel();
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let mut failure = match (stdout_pipe.is_some(), stderr_pipe.is_some()) {
        (true, true) => None,
        (false, false) => Some("stdout and stderr were not piped".to_owned()),
        (false, true) => Some("stdout was not piped".to_owned()),
        (true, false) => Some("stderr was not piped".to_owned()),
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
            failure = Some(format!("could not contain the process tree: {error}"));
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
                failure = Some(tree_termination_failure(&program, error));
            }
        }

        let now = Instant::now();
        if failure.is_none() && status.is_some() && stdout.is_some() && stderr.is_some() {
            break;
        }
        if failure.is_none() && reader_failed(&stdout, &stderr) {
            failure = Some("output could not be read".to_owned());
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
        return Err(failure);
    }

    let (stdout, stderr) = observed_outputs(stdout, stderr)?;
    let status = status.ok_or_else(|| "status was not observed".to_owned())?;
    Ok(Output {
        status,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_truncated: stdout.truncated,
    })
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
            Some(Err(error)) => message.push_str(&format!("; {name} read failed: {error}")),
            None => message.push_str(&format!("; {name} did not close within the bound")),
            Some(Ok(_)) => {}
        }
    }
}

fn observed_outputs(
    stdout: Option<ReaderResult>,
    stderr: Option<ReaderResult>,
) -> Result<(CapturedOutput, CapturedOutput), String> {
    let mut errors = Vec::new();
    let stdout = match stdout {
        Some(Ok(output)) => Some(output),
        Some(Err(error)) => {
            errors.push(format!("stdout could not be read: {error}"));
            None
        }
        None => {
            errors.push("stdout was not observed".to_owned());
            None
        }
    };
    let stderr = match stderr {
        Some(Ok(output)) => Some(output),
        Some(Err(error)) => {
            errors.push(format!("stderr could not be read: {error}"));
            None
        }
        None => {
            errors.push("stderr was not observed".to_owned());
            None
        }
    };
    match (stdout, stderr) {
        (Some(stdout), Some(stderr)) => Ok((stdout, stderr)),
        _ => Err(errors.join("; ")),
    }
}

fn hand_off_reaping(mut child: Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

fn tree_termination_failure(program: &Path, error: impl std::fmt::Display) -> String {
    format!(
        "could not terminate the completed {} process tree: {error}",
        program.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tree_that_would_not_terminate_names_the_program() {
        let message = tree_termination_failure(Path::new("/stand/in/gh"), "denied");
        assert_eq!(
            message,
            "could not terminate the completed /stand/in/gh process tree: denied"
        );
    }

    #[test]
    fn captured_output_is_bounded() {
        let output = vec![b'x'; MAX_CAPTURED_OUTPUT + 8192];
        let captured = read_all(output.as_slice()).expect("memory input is readable");
        assert_eq!(captured.bytes.len(), MAX_CAPTURED_OUTPUT);
        assert!(captured.truncated);
    }
}
