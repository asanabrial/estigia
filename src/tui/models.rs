//! Advisory model catalogs that require asking a coding-agent host.
//!
//! Only OpenCode is dynamic. Claude Code and Codex are curated on their
//! [`AgentAdapter`](crate::setup::AgentAdapter), and every other adapter says it
//! has no verified catalog rather than borrowing one. None of these lists
//! validates configuration: model IDs stay opaque and Estigia never runs them.

#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::bounded::{self, MAX_CAPTURED_OUTPUT, Output, controlled};
use crate::config::ModelRouting;
use crate::reviewer::process_tree;
use crate::setup::{AgentAdapter, ModelCatalogSource};

const OPENCODE_ARGS: &[&str] = &["models"];
const OPENCODE_TIMEOUT: Duration = Duration::from_secs(5);

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
            let program = resolver()
                .ok_or_else(|| "`opencode` was not found on an absolute PATH entry".to_owned())?;
            if Instant::now() >= run_deadline {
                return Err(format!(
                    "`opencode` PATH resolution did not finish within {}s",
                    timeout.as_secs_f64()
                ));
            }
            if !process_tree::launcher_can_be_contained(&program) {
                return Err(format!(
                    "{} is a script launcher, so its process tree cannot be bounded",
                    program.display()
                ));
            }
            launch(program, run_deadline, cleanup_deadline).map_err(|error| error.0)
        },
    )
    .map_err(CatalogError)
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
            .map_err(|error| error.0)
        },
    )
    .map_err(CatalogError)
}

fn run_probe(
    program: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    environment: Option<(String, String)>,
    run_deadline: Instant,
    cleanup_deadline: Instant,
) -> Result<Vec<String>, CatalogError> {
    let shown = program.display().to_string();
    let Output {
        status,
        stdout,
        stderr,
        stdout_truncated,
    } = bounded::run_process(
        program,
        args,
        timeout,
        environment,
        run_deadline,
        cleanup_deadline,
    )
    .map_err(CatalogError)?;
    if !status.success() {
        return Err(CatalogError(format!(
            "{shown} failed ({}): {}",
            status.code().unwrap_or(-1),
            safe_detail(&stderr)
        )));
    }
    if stdout_truncated {
        return Err(CatalogError(format!(
            "{shown} stdout exceeded the {MAX_CAPTURED_OUTPUT} byte advisory catalog limit",
        )));
    }
    parse(&stdout)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;

    const FIXTURE: &str = "ESTIGIA_MODEL_CATALOG_FIXTURE";
    const FIXTURE_MARKER: &str = "ESTIGIA_MODEL_CATALOG_MARKER";
    const FIXTURE_TEST: &str = "tui::models::tests::catalog_process_fixture";
    const DESCENDANT_STARTUP_BOUND: Duration = Duration::from_secs(2);
    const DESCENDANT_PROBE_TIMEOUT: Duration = Duration::from_secs(3);
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
    fn descendants_after_a_parent_exit_cannot_outlive_the_probe_bound() {
        assert_dead_descendant_after_probe_bound("parent-exits");
    }

    #[test]
    fn descendants_after_a_parent_timeout_cannot_outlive_the_probe_bound() {
        assert_dead_descendant_after_probe_bound("parent-times-out");
    }

    fn assert_dead_descendant_after_probe_bound(mode: &str) {
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
            started.elapsed() < DESCENDANT_PROBE_TIMEOUT + Duration::from_secs(2),
            "{mode} kept the probe blocked for {:?}",
            started.elapsed()
        );
        assert_descendant_ready(directory.path(), mode);
        assert_descendant_did_not_survive(directory.path(), mode);
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
