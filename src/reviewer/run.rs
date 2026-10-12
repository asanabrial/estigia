//! Running one reviewer CLI with its output drained as it arrives and its whole
//! process tree killed at a deadline.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{ReviewerCode, process_tree};
use crate::outcome::Refusal;

pub(super) const MAX_OUTPUT: usize = 4 * 1024 * 1024;
pub(super) const MAX_STDERR: usize = 64 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const READER_GRACE: Duration = Duration::from_secs(2);

pub(super) struct Launch<'a> {
    pub(super) program: &'a Path,
    pub(super) args: &'a [OsString],
    pub(super) cwd: &'a Path,
    pub(super) stdin: &'a [u8],
    pub(super) environment: &'a [(OsString, OsString)],
    pub(super) deadline: Duration,
}

pub(super) struct Finished {
    pub(super) code: Option<i32>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stream {
    Stdout,
    Stderr,
}

pub(super) type Drained = (Stream, std::io::Result<Vec<u8>>);

pub(super) fn launch_failed(message: impl Into<String>) -> Refusal {
    ReviewerCode::LaunchFailed.refusal(message)
}

const INHERITED: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "TMPDIR",
    "TEMP",
    "TMP",
    "SYSTEMROOT",
    "LANG",
    "LC_ALL",
    "TERM",
    "CLAUDE_CONFIG_DIR",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_AUTH_TOKEN",
    "CODEX_HOME",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
];

pub(super) fn child_environment() -> Vec<(OsString, OsString)> {
    std::env::vars_os()
        .filter(|(name, _)| {
            let name = name.to_string_lossy();
            INHERITED.contains(&name.as_ref()) || (cfg!(test) && name.starts_with("ESTIGIA_FAKE_"))
        })
        .collect()
}

pub(super) fn drain(
    stream: Stream,
    mut pipe: impl Read + Send + 'static,
    cap: usize,
    overflow: Option<Arc<AtomicBool>>,
    sender: mpsc::Sender<Drained>,
) {
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut chunk = [0_u8; 8192];
        let outcome = loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break Ok(kept),
                Ok(read) => match &overflow {
                    Some(overflow) => {
                        let room = cap.saturating_sub(kept.len()).min(read);
                        kept.extend_from_slice(&chunk[..room]);
                        if room < read {
                            overflow.store(true, Ordering::Relaxed);
                        }
                    }
                    None => {
                        kept.extend_from_slice(&chunk[..read]);
                        if kept.len() > cap {
                            kept.drain(..kept.len() - cap);
                        }
                    }
                },
                Err(error) => break Err(error),
            }
        };
        let _ = sender.send((stream, outcome));
    });
}

pub(super) fn execute(launch: &Launch<'_>) -> Result<Finished, Refusal> {
    let shown = launch.program.display();
    let mut command = Command::new(launch.program);
    command
        .args(launch.args)
        .current_dir(launch.cwd)
        .env_clear()
        .envs(launch.environment.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let prepared = process_tree::prepare(&mut command)
        .map_err(|error| launch_failed(format!("{shown} could not be contained: {error}")))?;
    let mut child = command.spawn().map_err(|error| {
        let code = if error.kind() == std::io::ErrorKind::NotFound {
            ReviewerCode::CliMissing
        } else {
            ReviewerCode::LaunchFailed
        };
        code.refusal(format!("{shown} would not start: {error}"))
    })?;
    let tree = match prepared.attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(launch_failed(format!(
                "{shown} could not be contained: {error}"
            )));
        }
    };

    let overflow = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        drain(
            Stream::Stdout,
            stdout,
            MAX_OUTPUT,
            Some(overflow.clone()),
            sender.clone(),
        );
    }
    if let Some(stderr) = child.stderr.take() {
        drain(Stream::Stderr, stderr, MAX_STDERR, None, sender);
    }
    let writer = child.stdin.take().map(|mut stdin| {
        let prompt = launch.stdin.to_vec();
        std::thread::spawn(move || stdin.write_all(&prompt))
    });

    let limit = Instant::now() + launch.deadline;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let note = stop(&tree, &mut child);
                return Err(launch_failed(format!(
                    "{shown} could not be waited on: {error}{}",
                    kill_note(note.as_deref())
                )));
            }
        }
        if overflow.load(Ordering::Relaxed) {
            let note = stop(&tree, &mut child);
            return Err(too_large(&shown, note.as_deref()));
        }
        if Instant::now() >= limit {
            let note = stop(&tree, &mut child);
            return Err(ReviewerCode::Deadline.refusal(format!(
                "{shown} did not finish within {}s{}",
                launch.deadline.as_secs_f64(),
                match note {
                    None => " and its process tree was killed".to_owned(),
                    Some(error) => format!(" and killing its process tree failed: {error}"),
                }
            )));
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    if let Err(error) = tree.terminate() {
        return Err(launch_failed(format!(
            "{shown} exited with {} and killing its process tree failed: {error}",
            exit_description(status.code())
        )));
    }

    let mut stdout = None;
    let mut stderr = None;
    let grace = Instant::now() + READER_GRACE;
    while stdout.is_none() || stderr.is_none() {
        let wait = grace.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(wait) {
            Ok((Stream::Stdout, bytes)) => stdout = Some(bytes),
            Ok((Stream::Stderr, bytes)) => stderr = Some(bytes),
            Err(_) => {
                return Err(launch_failed(format!(
                    "{shown} exited with {} and a descendant kept its output open past the \
                     cleanup bound",
                    exit_description(status.code())
                )));
            }
        }
    }
    if overflow.load(Ordering::Relaxed) {
        return Err(too_large(&shown, None));
    }
    let read = |bytes: Option<std::io::Result<Vec<u8>>>, name: &str| match bytes {
        Some(Ok(bytes)) => Ok(bytes),
        Some(Err(error)) => Err(launch_failed(format!(
            "{shown} {name} could not be read: {error}"
        ))),
        None => Err(launch_failed(format!("{shown} {name} was not captured"))),
    };
    let delivered = writer.map_or(Ok(()), |writer| {
        writer
            .join()
            .unwrap_or_else(|_| Err(std::io::Error::other("the prompt writer panicked")))
    });
    if let Err(error) = delivered
        && status.code() == Some(0)
    {
        return Err(launch_failed(format!(
            "{shown} exited 0 without taking the whole prompt: {error}"
        )));
    }
    Ok(Finished {
        code: status.code(),
        stdout: read(stdout, "stdout")?,
        stderr: read(stderr, "stderr")?,
    })
}

pub(super) fn exit_description(code: Option<i32>) -> String {
    code.map_or_else(|| "a signal".to_owned(), |code| code.to_string())
}

fn stop(tree: &process_tree::Owner, child: &mut std::process::Child) -> Option<String> {
    let failure = tree.terminate().err();
    let _ = child.kill();
    let _ = child.wait();
    failure
}

fn kill_note(failure: Option<&str>) -> String {
    failure.map_or_else(String::new, |error| {
        format!(" and killing its process tree failed: {error}")
    })
}

pub(super) fn too_large(shown: &impl std::fmt::Display, kill_failure: Option<&str>) -> Refusal {
    ReviewerCode::OutputTooLarge.refusal(format!(
        "{shown} produced more than {MAX_OUTPUT} bytes of output{}",
        kill_note(kill_failure)
    ))
}
