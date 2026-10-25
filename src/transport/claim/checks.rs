//! Declared checks: the tool that runs them, the receipt that binds the result
//! to the reviewed digest, and the gate that refuses to clear without it.

use std::path::Path;
use std::time::{Duration, Instant};

use super::{Context, Failure, Ledger, ownership, protocol_body, verify_held};
use crate::reviewer::job::JobStatus;
use crate::transport::check_job::{self, CheckRun, Outcome};
use crate::transport::generated::checks::{Check, commands_digest, declared_at};
use crate::transport::{markers, target};

/// The longest a single `run_checks` call waits for a running job.
const MAX_WAIT_SECONDS: u64 = 50;

const ASK_AFTER_SECONDS: u64 = 30;
const POLL: Duration = Duration::from_millis(100);

/// What `run_checks` is asked.
#[derive(Debug, Clone)]
pub struct RunChecks<'a> {
    /// The issue whose live `review` claim authorises the call.
    pub issue: u64,
    /// The run holding that claim.
    pub run_id: &'a str,
    /// The base the published receipt names.
    pub base: Option<&'a str>,
    /// The checkout the checks run in; this repository's own when absent.
    pub worktree: Option<&'a Path>,
    /// Seconds to wait for the job, capped at `MAX_WAIT_SECONDS`.
    pub wait_seconds: u64,
    /// The moment the claim is judged.
    pub now: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChecksReceipt {
    passed: bool,
    results: String,
}

fn receipt_of(
    comments: &[ownership::Comment],
    digest: &str,
    commands: &str,
) -> Option<ChecksReceipt> {
    comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .flat_map(|comment| markers::parse(&comment.body))
        .find_map(|marker| {
            let is = |key: &str, value: &str| marker.get(key).map(String::as_str) == Some(value);
            if !(is("kind", "checks-receipt") && is("digest", digest) && is("commands", commands)) {
                return None;
            }
            let passed = match marker.get("passed").map(String::as_str)? {
                "true" => true,
                "false" => false,
                _ => return None,
            };
            Some(ChecksReceipt {
                passed,
                results: marker.get("results").cloned().unwrap_or_default(),
            })
        })
}

fn results_line(runs: &[CheckRun]) -> String {
    runs.iter()
        .map(|run| {
            let exit = run
                .exit
                .map_or_else(|| "none".to_owned(), |code| code.to_string());
            format!("{}={exit}:{}", run.name, run.seconds)
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn summaries(results: &str) -> Vec<serde_json::Value> {
    results
        .split(',')
        .filter_map(|entry| {
            let (name, rest) = entry.split_once('=')?;
            let (exit, seconds) = rest.split_once(':')?;
            Some(serde_json::json!({
                "name": name,
                "exit": exit.parse::<i32>().ok(),
                "seconds": seconds.parse::<u64>().ok(),
            }))
        })
        .collect()
}

pub(super) fn patience_for(wait_seconds: u64) -> Duration {
    Duration::from_secs(wait_seconds.min(MAX_WAIT_SECONDS))
}

fn base_unknown() -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "checks-base-unknown",
        "action": "publish the change with `publish_review` first: the checks run against the \
                   base its receipt names, and this run has published none",
    }))
}

fn runner_lost(detail: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "checks-runner-lost",
        "detail": detail,
        "action": "nothing was recorded; call `run_checks` again to start the checks afresh",
    }))
}

fn job_id(issue: u64, digest: &str, commands: &str) -> String {
    let short = |text: &str| text.chars().take(16).collect::<String>();
    format!("checks-{issue}-{}-{}", short(digest), short(commands))
}

fn record(
    ledger: &Ledger,
    request: &RunChecks<'_>,
    digest: &str,
    commands: &str,
    passed: bool,
    runs: &[CheckRun],
) -> Result<(), Failure> {
    let results = results_line(runs);
    let marker = markers::render(
        "checks-receipt",
        &[
            ("run-id", request.run_id),
            ("digest", digest),
            ("commands", commands),
            ("passed", if passed { "true" } else { "false" }),
            ("results", &results),
        ],
    )
    .ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    let body = protocol_body(
        &format!(
            "Declared checks {} for target `{digest}`, run by `{}`.",
            if passed { "passed" } else { "failed" },
            request.run_id
        ),
        &marker,
    );
    ledger.append(&body, request.now)
}

fn finished_answer(
    digest: &str,
    commands: &str,
    passed: bool,
    cached: bool,
    checks: Vec<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "declared": true,
        "cached": cached,
        "state": "finished",
        "passed": passed,
        "digest": digest,
        "commands": commands,
        "checks": checks,
    })
}

fn collected(
    ledger: &Ledger,
    request: &RunChecks<'_>,
    dir: &Path,
    digest: &str,
    commands: &str,
    declared: usize,
    outcome: &Outcome,
) -> Result<serde_json::Value, Failure> {
    let tails = |outcome: &Outcome| -> Vec<serde_json::Value> {
        outcome
            .runs
            .iter()
            .map(|run| {
                serde_json::json!({
                    "name": run.name,
                    "exit": run.exit,
                    "seconds": run.seconds,
                    "tail": run.tail,
                })
            })
            .collect()
    };
    if outcome.timed_out {
        check_job::discard_result(dir)?;
        let _ = std::fs::remove_dir_all(dir);
        return Ok(serde_json::json!({
            "ok": true,
            "declared": true,
            "cached": false,
            "state": "timed-out",
            "passed": false,
            "recorded": false,
            "digest": digest,
            "commands": commands,
            "checks": tails(outcome),
            "action": "the job ran past its deadline and its process group was killed; nothing \
                       was recorded, so `run_checks` starts the checks afresh",
        }));
    }
    let passed = outcome.passed(declared);
    record(ledger, request, digest, commands, passed, &outcome.runs)?;
    check_job::discard_result(dir)?;
    let _ = std::fs::remove_dir_all(dir);
    Ok(finished_answer(
        digest,
        commands,
        passed,
        false,
        tails(outcome),
    ))
}

/// Runs the checks the base declares against the clean target of the run's
/// checkout, in a detached job, and records a `checks-receipt` bound to that
/// target's digest and to the declared commands.
///
/// Nothing declared answers `declared: false` and records nothing. A receipt
/// already recorded for the digest and the commands is answered `cached`, and
/// nothing runs. A job already running for them is waited on, never started
/// twice. The wait never passes `MAX_WAIT_SECONDS`.
pub fn run_checks(
    context: &Context,
    request: &RunChecks<'_>,
) -> Result<serde_json::Value, Failure> {
    run_checks_with(context, request, &check_job::spawn)
}

pub(super) fn run_checks_with(
    context: &Context,
    request: &RunChecks<'_>,
    spawn: &dyn Fn(&Path) -> Result<u32, Failure>,
) -> Result<serde_json::Value, Failure> {
    let began = Instant::now();
    let patience = patience_for(request.wait_seconds);
    let base = request.base.ok_or_else(base_unknown)?;
    let ledger = Ledger::open(context, request.issue, request.now)?;
    verify_held(
        context,
        &ledger,
        request.issue,
        request.run_id,
        "review",
        request.now,
        None,
    )?;
    let tree = request.worktree.unwrap_or(&context.repo_dir);
    let clean = target::clean_target(context, base, request.worktree)?;
    let digest = clean
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Failure::Read("the clean target carries no digest".to_owned()))?
        .to_owned();
    let checks: Vec<Check> = declared_at(tree, base)?;
    if checks.is_empty() {
        return Ok(serde_json::json!({"ok": true, "declared": false}));
    }
    let commands = commands_digest(&checks);
    if let Some(recorded) = receipt_of(&ledger.timeline()?, &digest, &commands) {
        return Ok(finished_answer(
            &digest,
            &commands,
            recorded.passed,
            true,
            summaries(&recorded.results),
        ));
    }

    let id = job_id(request.issue, &digest, &commands);
    let dir = super::capture::jobs_dir()?.join(&id);
    let mut status = check_job::status(&dir)?;
    if status == JobStatus::Lost {
        let _ = std::fs::remove_dir_all(&dir);
        check_job::write_job(&dir, tree, &digest, &commands, &checks)?;
        if let Err(failure) = spawn(&dir) {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(failure);
        }
        status = check_job::status(&dir)?;
    }
    loop {
        match status {
            JobStatus::Finished(outcome) => {
                return collected(
                    &ledger,
                    request,
                    &dir,
                    &digest,
                    &commands,
                    checks.len(),
                    &outcome,
                );
            }
            JobStatus::Lost => {
                let detail = std::fs::read_to_string(dir.join("runner.err")).unwrap_or_default();
                let _ = std::fs::remove_dir_all(&dir);
                return Err(runner_lost(detail.trim()));
            }
            JobStatus::Running if began.elapsed() >= patience => {
                return Ok(serde_json::json!({
                    "ok": true,
                    "declared": true,
                    "state": "running",
                    "job": id,
                    "digest": digest,
                    "commands": commands,
                    "ask_after": ASK_AFTER_SECONDS,
                }));
            }
            JobStatus::Running => std::thread::sleep(POLL),
        }
        status = check_job::status(&dir)?;
    }
}

fn missing(receipt: &super::ReviewTransaction, commands: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "checks-receipt-missing",
        "digest": receipt.digest,
        "commands": commands,
        "action": "keep the PR draft; call `run_checks` for this exact published target and \
                   collect its answer, then record the verdict again",
    }))
}

/// Refuses to clear a receipt whose base declares checks unless a trusted
/// `checks-receipt` on the timeline names exactly its digest and the base's
/// current commands, and says they passed. A base that declares none is
/// untouched.
pub(super) fn require_receipt(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &super::ReviewTransaction,
    worktree: Option<&Path>,
) -> Result<(), Failure> {
    let checks = declared_at(worktree.unwrap_or(&context.repo_dir), &receipt.base)?;
    if checks.is_empty() {
        return Ok(());
    }
    let commands = commands_digest(&checks);
    match receipt_of(comments, &receipt.digest, &commands) {
        None => Err(missing(receipt, &commands)),
        Some(recorded) if !recorded.passed => Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "checks-failed",
            "digest": receipt.digest,
            "commands": commands,
            "checks": summaries(&recorded.results),
            "action": "keep the PR draft; correct what the failing checks name and publish again: \
                       a failed result stands for these exact bytes",
        }))),
        Some(_) => Ok(()),
    }
}
