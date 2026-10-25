//! `deliver`: the mechanical tail from publication to close, in one call.
//!
//! This file is the sequence and the reads it makes; every write goes through a
//! tool (`record_computed_verdict`, `release_ci`, `release`) or through the
//! gate's own decision for the merge, so nothing here widens what a run may do.
//! `Hands` is the seam: the live one calls those tools and the gate, and the
//! tests script them. The steps and their hand-backs are in the binding's
//! *CI, merge and delivery* section.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::{NamedRun, ToolFailure};
use crate::harness::{Action, CommitTarget, Decision, GateContext, RunsIn, Sensitivity, session};
use crate::transport::claim::ReviewTransaction;
use crate::transport::{self, Failure, How, Output};

const CONFLICT_LINES: usize = 8;
const DATE_CHARS: usize = 10;
const SHA_CHARS: usize = 12;
const MAX_WAIT_SECONDS: u64 = 50;
const DEFAULT_WAIT_SECONDS: u64 = 20;
const EMPTY_ROLLUP_WINDOW_SECONDS: u64 = 600;
const ASK_AFTER_SECONDS: u64 = 15;
const POLL: Duration = Duration::from_secs(2);
const READBACK_ATTEMPTS: u32 = 4;
const READBACK_PAUSE: Duration = Duration::from_millis(750);
const RED: [&str; 5] = [
    "FAILURE",
    "CANCELLED",
    "TIMED_OUT",
    "ACTION_REQUIRED",
    "STARTUP_FAILURE",
];
const SOUND: [&str; 3] = ["SUCCESS", "NEUTRAL", "SKIPPED"];
const PR_FIELDS: &str = "statusCheckRollup,mergeStateStatus,headRefOid,baseRefOid,baseRefName";
const MERGED_FIELDS: &str = "state,mergedAt,mergeCommit";

pub(super) struct Settings {
    pub(super) merge_by_agent: bool,
    pub(super) delivery_auto: bool,
    pub(super) summary_language: String,
}

pub(super) trait Hands {
    fn tool(&self, name: &str, arguments: Value) -> Result<Value, Value>;
    fn exec(&self, args: &[&str], how: How) -> Result<Output, Failure>;
    fn merge_decision(&self, pr: u64) -> Result<(), Value>;
    fn receipt(&self) -> Option<ReviewTransaction>;
    fn worktree(&self) -> Option<String>;
    fn settings(&self) -> Settings;
    fn release_stamp(&self, receipt: &ReviewTransaction) -> Result<Option<String>, Value>;
    fn lane(&self, receipt: &ReviewTransaction) -> Value;
    fn checks_receipt(&self, receipt: &ReviewTransaction) -> Result<Option<Value>, Value>;
    fn scratch(&self) -> PathBuf;
    fn now(&self) -> u64;
}

pub(super) struct Request {
    pub(super) issue: u64,
    pub(super) run_id: String,
    pub(super) wait: Duration,
    pub(super) readback_pause: Duration,
    pub(super) spend: Option<String>,
    pub(super) appendix: Option<String>,
}

enum Halt {
    Stopped(Value),
    Pending(Value),
}

type Step<T> = Result<T, Halt>;

enum Position {
    Open,
    Delivered,
}

struct Merged {
    sha: String,
    parents: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Rollup {
    Green(Vec<String>),
    Pending(Vec<String>),
    Red(Vec<String>),
    Empty,
}

fn stop_answer(step: &str, reason: &str, detail: Value) -> Value {
    let mut stopped = json!({"step": step, "reason": reason});
    if let Value::Object(extra) = detail {
        for (key, value) in extra {
            stopped[key.as_str()] = value;
        }
    }
    json!({"ok": true, "delivered": false, "stopped": stopped})
}

fn stop(step: &str, reason: &str, detail: Value) -> Halt {
    Halt::Stopped(stop_answer(step, reason, detail))
}

fn refused(step: &str, refusal: Value) -> Halt {
    let reason = refusal["code"].as_str().unwrap_or("refused").to_owned();
    stop(step, &reason, json!({"refusal": refusal}))
}

fn failed(step: &str, reason: &str, failure: &Failure) -> Halt {
    stop(step, reason, json!({"failure": failure.envelope()}))
}

fn with_merged(halt: Halt, merged: &Merged) -> Halt {
    match halt {
        Halt::Stopped(mut answer) => {
            answer["merged"] = json!({"sha": merged.sha, "parents": merged.parents});
            Halt::Stopped(answer)
        }
        pending => pending,
    }
}

fn pending(step: &str, reason: &str, ask_after: u64) -> Halt {
    Halt::Pending(json!({
        "ok": true,
        "delivered": false,
        "pending": {"step": step, "reason": reason, "ask_after": ask_after},
    }))
}

pub(super) fn classify(rollup: &Value) -> Rollup {
    let checks = rollup.as_array().map(Vec::as_slice).unwrap_or_default();
    if checks.is_empty() {
        return Rollup::Empty;
    }
    let (mut sound, mut waiting, mut red) = (Vec::new(), Vec::new(), Vec::new());
    for check in checks {
        let name = check["name"]
            .as_str()
            .or_else(|| check["context"].as_str())
            .unwrap_or("unnamed");
        let verdict = if let Some(state) = check["state"].as_str() {
            match state {
                "SUCCESS" => Ok(state),
                "PENDING" | "EXPECTED" => Err(None),
                other => Err(Some(other)),
            }
        } else {
            match check["status"].as_str() {
                Some("COMPLETED") => {
                    let conclusion = check["conclusion"].as_str().unwrap_or_default();
                    if SOUND.contains(&conclusion) {
                        Ok(conclusion)
                    } else if RED.contains(&conclusion) {
                        Err(Some(conclusion))
                    } else {
                        Err(Some("UNREADABLE"))
                    }
                }
                Some(_) => Err(None),
                None => Err(Some("UNREADABLE")),
            }
        };
        match verdict {
            Ok(conclusion) => sound.push(format!("{name} {conclusion}")),
            Err(None) => waiting.push(name.to_owned()),
            Err(Some(conclusion)) => red.push(format!("{name} {conclusion}")),
        }
    }
    if !red.is_empty() {
        Rollup::Red(red)
    } else if !waiting.is_empty() {
        Rollup::Pending(waiting)
    } else {
        Rollup::Green(sound)
    }
}

pub(super) fn cargo_version(text: &str) -> Option<String> {
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "version" => return Some(value.trim().trim_matches('"').to_owned()),
            "version.workspace" => return Some("workspace".to_owned()),
            _ => {}
        }
    }
    None
}

pub(super) fn package_version(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()?
        .get("version")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn manifest_version(path: &str, text: &str) -> Result<Option<String>, ()> {
    match path.rsplit('/').next() {
        Some("Cargo.toml") => text
            .parse::<toml_edit::DocumentMut>()
            .map(|_| cargo_version(text))
            .map_err(|_| ()),
        Some("package.json") => serde_json::from_str::<Value>(text)
            .map(|_| package_version(text))
            .map_err(|_| ()),
        _ => Ok(None),
    }
}

struct Deliver<'a, H: Hands> {
    hands: &'a H,
    request: &'a Request,
    deadline: Instant,
    released_at: u64,
    done: Vec<&'static str>,
}

pub(super) fn run<H: Hands>(hands: &H, request: &Request) -> Value {
    let mut deliver = Deliver {
        hands,
        request,
        deadline: Instant::now() + request.wait,
        released_at: 0,
        done: Vec::new(),
    };
    let outcome = deliver.sequence();
    let (Ok(mut answer) | Err(Halt::Stopped(mut answer) | Halt::Pending(mut answer))) = outcome;
    answer["done"] = json!(deliver.done);
    answer
}

impl<H: Hands> Deliver<'_, H> {
    fn sequence(&mut self) -> Step<Value> {
        let hands = self.hands;
        let (issue, run_id) = (self.request.issue, self.request.run_id.as_str());
        hands
            .tool(
                "verify_claim",
                json!({"issue": issue, "run_id": run_id, "expect_state": "review", "compact": true}),
            )
            .map_err(|refusal| refused("hold", refusal))?;
        let Some(receipt) = hands.receipt() else {
            return Err(stop(
                "receipt",
                "receipt-missing",
                json!({"detail": "this run holds no complete review transaction in its pointer"}),
            ));
        };
        let answer = match self.review(&receipt)? {
            Position::Delivered => self.landed(&receipt)?,
            Position::Open => {
                self.authorise()?;
                let lane = self.release(&receipt)?;
                let (view, ci) = self.ci(&receipt, &lane)?;
                self.base(&receipt, &view)?;
                self.version(&receipt)?;
                let merged = self.merge(&receipt, &view)?;
                self.close(&receipt, &merged, &ci)?
            }
        };
        Ok(answer)
    }

    fn call(&self, step: &str, tool: &str, arguments: Value) -> Step<Value> {
        self.hands
            .tool(tool, arguments)
            .map_err(|refusal| refused(step, refusal))
    }

    fn receipt_arguments(&self, receipt: &ReviewTransaction) -> Value {
        json!({
            "issue": self.request.issue,
            "run_id": self.request.run_id,
            "epoch": receipt.epoch,
            "pr": receipt.pr,
            "head": receipt.head,
            "base": receipt.base,
            "digest": receipt.digest,
            "compact": true,
        })
    }

    fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }

    fn wait(&self, step: &str, reason: &str, ask_after: Option<u64>) -> Step<()> {
        if self.expired() {
            let ask = ask_after.unwrap_or(self.hands.now() + ASK_AFTER_SECONDS);
            return Err(pending(step, reason, ask));
        }
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(POLL.min(remaining));
        Ok(())
    }

    fn review(&mut self, receipt: &ReviewTransaction) -> Step<Position> {
        let mut recorded = false;
        loop {
            let status = self.call(
                "review",
                "review_status",
                json!({"issue": self.request.issue, "compact": true}),
            )?;
            let seen = &status["receipt"];
            let same = seen["epoch"] == receipt.epoch
                && seen["pr"] == receipt.pr
                && seen["head"] == receipt.head
                && seen["base"] == receipt.base
                && seen["digest"] == receipt.digest;
            if !same {
                return Err(stop(
                    "receipt",
                    "receipt-moved",
                    json!({"observed": seen, "held": {
                        "epoch": receipt.epoch, "pr": receipt.pr, "head": receipt.head,
                        "base": receipt.base, "digest": receipt.digest,
                    }}),
                ));
            }
            let next = status["next"].clone();
            let operation = next["operation"].as_str().unwrap_or_default();
            let reason = next["reason_code"].as_str().unwrap_or_default();
            match next["kind"].as_str().unwrap_or_default() {
                "execute" if operation == "record_computed_verdict" && !recorded => {
                    let mut arguments = self.receipt_arguments(receipt);
                    if let Some(worktree) = self.hands.worktree() {
                        arguments["worktree"] = json!(worktree);
                    }
                    self.call("review", "record_computed_verdict", arguments)?;
                    self.done.push("record_computed_verdict");
                    recorded = true;
                }
                "execute" if operation == "release_ci" => return Ok(Position::Open),
                "stop" if reason == "released" => return Ok(Position::Open),
                "stop" if reason == "delivered" => return Ok(Position::Delivered),
                "collect" if all_running(&next) => {
                    self.wait("review", "inputs-running", next["ask_after"].as_u64())?;
                }
                _ => return Err(stop("review", reason, json!({"next": next}))),
            }
        }
    }

    fn authorise(&self) -> Step<()> {
        let settings = self.hands.settings();
        if !settings.merge_by_agent {
            return Err(stop(
                "authorisation",
                "merge-by-person",
                json!({"detail": "`Merge by` is `person`: a person merges this pull request"}),
            ));
        }
        if !settings.delivery_auto {
            return Err(stop(
                "authorisation",
                "delivery-authorisation-ask",
                json!({"detail": "`Delivery authorisation` is not `auto`: ask the person first"}),
            ));
        }
        Ok(())
    }

    fn release(&mut self, receipt: &ReviewTransaction) -> Step<Value> {
        let stamp = self
            .hands
            .release_stamp(receipt)
            .map_err(|refusal| refused("release", refusal))?;
        match stamp {
            Some(stamp) => {
                self.released_at = crate::transport::ownership::parse_stamp(&stamp)
                    .and_then(|seconds| u64::try_from(seconds).ok())
                    .unwrap_or(0);
            }
            None => {
                self.call("release", "release_ci", self.receipt_arguments(receipt))?;
                self.done.push("release_ci");
                self.released_at = self.hands.now();
            }
        }
        Ok(self.hands.lane(receipt))
    }

    fn read_pr(&self, step: &str, receipt: &ReviewTransaction, fields: &str) -> Step<Value> {
        let pr = receipt.pr.to_string();
        let output = self
            .hands
            .exec(&["gh", "pr", "view", &pr, "--json", fields], How::read())
            .map_err(|failure| failed(step, "pull-request-unreadable", &failure))?;
        serde_json::from_str(output.stdout.trim()).map_err(|error| {
            stop(
                step,
                "pull-request-unreadable",
                json!({"detail": format!("gh answered no JSON for the pull request: {error}")}),
            )
        })
    }

    fn ci(&mut self, receipt: &ReviewTransaction, lane: &Value) -> Step<(Value, Value)> {
        let state = lane["state"].as_str().unwrap_or_default();
        let cannot_run = matches!(state, "disabled" | "never-ran");
        if cannot_run && self.request.appendix.is_none() {
            return Err(stop(
                "ci",
                "appendix-required",
                json!({
                    "lane": lane,
                    "detail": "no CI can run here: pass `appendix_file` with the local suite \
                               command and its result",
                }),
            ));
        }
        loop {
            let view = self.read_pr("ci", receipt, PR_FIELDS)?;
            if view["headRefOid"] != receipt.head {
                return Err(stop(
                    "ci",
                    "head-moved",
                    json!({"observed": view["headRefOid"], "reviewed": receipt.head}),
                ));
            }
            if cannot_run {
                let summary = json!({"lane": lane, "rollup": "not consulted"});
                return Ok((view, summary));
            }
            match classify(&view["statusCheckRollup"]) {
                Rollup::Red(checks) => {
                    return Err(stop("ci", "ci-red", json!({"checks": checks})));
                }
                Rollup::Green(checks) => {
                    let summary = json!({
                        "lane": lane,
                        "rollup": checks,
                        "merge_state": view["mergeStateStatus"],
                    });
                    return Ok((view, summary));
                }
                Rollup::Pending(_) => {
                    self.wait("ci", "checks-running", None)?;
                }
                Rollup::Empty if state == "not-consulted" => {
                    let summary = json!({"lane": lane, "rollup": "none"});
                    return Ok((view, summary));
                }
                Rollup::Empty => {
                    if self.hands.now() > self.released_at + EMPTY_ROLLUP_WINDOW_SECONDS {
                        return Err(stop(
                            "ci",
                            "ci-absent",
                            json!({"detail": "no check appeared within ten minutes of the release"}),
                        ));
                    }
                    self.wait("ci", "checks-not-started", None)?;
                }
            }
        }
    }

    fn git(&self, step: &str, reason: &str, args: &[&str]) -> Step<String> {
        let mut full = vec!["git"];
        full.extend_from_slice(args);
        self.hands
            .exec(&full, How::read())
            .map(|output| output.stdout.trim().to_owned())
            .map_err(|failure| failed(step, reason, &failure))
    }

    fn base(&self, receipt: &ReviewTransaction, view: &Value) -> Step<()> {
        let Some(branch) = view["baseRefName"].as_str() else {
            return Err(stop("base", "base-unreadable", json!({})));
        };
        self.git("base", "base-fetch-failed", &["fetch", "origin", branch])?;
        let tip = self.git("base", "base-fetch-failed", &["rev-parse", "FETCH_HEAD"])?;
        if tip == receipt.base {
            return Ok(());
        }
        let merge = self
            .hands
            .exec(
                &["git", "merge-tree", "--write-tree", &tip, &receipt.head],
                How::tolerated(),
            )
            .map_err(|failure| failed("base", "base-unreadable", &failure))?;
        if merge.status == 0 {
            return Ok(());
        }
        let reason = if merge.status == 1 && !merge.stdout.trim().is_empty() {
            "base-advanced"
        } else {
            "base-unreadable"
        };
        Err(stop(
            "base",
            reason,
            json!({
                "tip": tip,
                "reviewed_base": receipt.base,
                "detail": merge.stdout.lines().take(CONFLICT_LINES).collect::<Vec<_>>().join("\n"),
            }),
        ))
    }

    fn version(&self, receipt: &ReviewTransaction) -> Step<()> {
        let changed = self.git(
            "version",
            "version-unreadable",
            &["diff", "--name-status", &receipt.base, &receipt.head],
        )?;
        let mut moved = Vec::new();
        for line in changed.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            let (Some(status), Some(old), Some(new)) =
                (fields.first(), fields.get(1), fields.last())
            else {
                continue;
            };
            let at = |revision: &str, path: &str| {
                if !matches!(path.rsplit('/').next(), Some("Cargo.toml" | "package.json")) {
                    return Ok(None);
                }
                let spec = format!("{revision}:{path}");
                let shown = self.hands.exec(&["git", "show", &spec], How::tolerated());
                let read = match &shown {
                    Ok(output) if output.status == 0 => manifest_version(path, &output.stdout),
                    _ => Err(()),
                };
                read.map_err(|()| stop("version", "version-unreadable", json!({"path": path})))
            };
            let before = if status.starts_with('A') {
                None
            } else {
                at(&receipt.base, old)?
            };
            let after = if status.starts_with('D') {
                None
            } else {
                at(&receipt.head, new)?
            };
            if before != after {
                moved.push(json!({"path": new, "from": before, "to": after}));
            }
        }
        if moved.is_empty() {
            return Ok(());
        }
        Err(stop(
            "version",
            "version-changed",
            json!({
                "manifests": moved,
                "detail": "a version changed between the reviewed base and head: the tag and \
                           release are the agent's, per the binding's Version tags",
            }),
        ))
    }

    fn merge(&mut self, receipt: &ReviewTransaction, view: &Value) -> Step<Merged> {
        self.hands
            .merge_decision(receipt.pr)
            .map_err(|refusal| refused("merge", refusal))?;
        let pr = receipt.pr.to_string();
        let attempted = self.hands.exec(
            &[
                "gh",
                "pr",
                "merge",
                &pr,
                "--merge",
                "--match-head-commit",
                &receipt.head,
            ],
            How::tolerated(),
        );
        let refusal = match &attempted {
            Ok(output) if output.status == 0 => None,
            Ok(output) => Some(output.stderr.trim().to_owned()),
            Err(failure) => Some(failure.detail()),
        };
        let branch = view["baseRefName"].as_str().unwrap_or_default().to_owned();
        let mut read = false;
        for attempt in 0..READBACK_ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(self.request.readback_pause);
            }
            let Ok(seen) = self.read_pr("merge", receipt, MERGED_FIELDS) else {
                continue;
            };
            read = true;
            if let (Some("MERGED"), Some(sha)) =
                (seen["state"].as_str(), seen["mergeCommit"]["oid"].as_str())
            {
                self.done.push("merge");
                let parents = self.parents_of(sha, &branch);
                return Ok(Merged {
                    sha: sha.to_owned(),
                    parents,
                });
            }
        }
        if !read {
            let detail = "the outcome could not be read and may have landed: deliver again re-reads it and never re-merges";
            return Err(stop(
                "merge",
                "merge-unknown",
                json!({"landed": null, "detail": detail}),
            ));
        }
        Err(match refusal {
            Some(said) => stop(
                "merge",
                "merge-refused",
                json!({"detail": said, "landed": false}),
            ),
            None => stop(
                "merge",
                "merge-unconfirmed",
                json!({"detail": "the merge command answered and the pull request does not read merged: a merge queue or an approval rule may own it"}),
            ),
        })
    }

    fn parents_of(&self, sha: &str, branch: &str) -> Vec<String> {
        if !branch.is_empty() {
            let _ = self
                .hands
                .exec(&["git", "fetch", "origin", branch], How::tolerated());
        }
        self.hands
            .exec(
                &["git", "rev-list", "--parents", "-n", "1", sha],
                How::tolerated(),
            )
            .ok()
            .filter(|output| output.status == 0)
            .map(|output| {
                output
                    .stdout
                    .split_whitespace()
                    .skip(1)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn landed(&mut self, receipt: &ReviewTransaction) -> Step<Value> {
        let seen = self.read_pr("merge", receipt, "state,mergeCommit,baseRefName")?;
        let Some(sha) = seen["mergeCommit"]["oid"].as_str() else {
            return Err(stop(
                "merge",
                "merge-unreadable",
                json!({"detail": "the pull request reads merged and names no merge commit"}),
            ));
        };
        let branch = seen["baseRefName"].as_str().unwrap_or_default().to_owned();
        let parents = self.parents_of(sha, &branch);
        let merged = Merged {
            sha: sha.to_owned(),
            parents,
        };
        self.close(receipt, &merged, &json!({}))
    }

    fn close(&mut self, receipt: &ReviewTransaction, merged: &Merged, ci: &Value) -> Step<Value> {
        let facts = self.facts(receipt, merged, ci);
        let evidence = evidence(&facts);
        let scratch = self.hands.scratch();
        let file = scratch.join(format!("{}-{}.md", self.request.run_id, receipt.pr));
        let written = std::fs::create_dir_all(&scratch)
            .and_then(|()| crate::paths::replace_atomically(&file, &evidence));
        if let Err(error) = written {
            return Err(with_merged(
                stop(
                    "close",
                    "evidence-unwritable",
                    json!({"detail": format!("{}: {error}", file.display())}),
                ),
                merged,
            ));
        }
        let mut arguments = json!({
            "issue": self.request.issue,
            "run_id": self.request.run_id,
            "evidence_file": file.display().to_string(),
            "compact": true,
        });
        if let Some(spend) = &self.request.spend {
            arguments["spend"] = json!(spend);
        }
        let released = self.hands.tool("release", arguments);
        let _ = std::fs::remove_file(&file);
        let released =
            released.map_err(|refusal| with_merged(refused("close", refusal), merged))?;
        if released["ok"] == false {
            let mut answer = released;
            answer["delivered"] = json!(false);
            answer["merged"] = json!({"sha": merged.sha, "parents": merged.parents});
            return Err(Halt::Stopped(answer));
        }
        self.done.push("release");
        let issue_state = self.close_issue();
        let delivered = matches!(&issue_state, Ok(state) if state == "CLOSED");
        let mut answer = json!({
            "ok": delivered,
            "delivered": delivered,
            "issue": self.request.issue,
            "pr": receipt.pr,
            "merged": {"sha": merged.sha, "parents": merged.parents},
            "release": released,
        });
        match issue_state {
            Ok(state) => answer["github_issue"] = json!({"state": state}),
            Err(failure) => {
                answer["delivery_incomplete"] =
                    json!({"step": "issue-close", "failure": failure.envelope()});
            }
        }
        Ok(answer)
    }

    fn close_issue(&mut self) -> Result<String, Failure> {
        let issue = self.request.issue.to_string();
        let read = |hands: &H| -> Result<String, Failure> {
            let output = hands.exec(
                &["gh", "issue", "view", &issue, "--json", "state"],
                How::read(),
            )?;
            let seen: Value = serde_json::from_str(output.stdout.trim()).map_err(|error| {
                Failure::Read(format!("gh answered no JSON for issue {issue}: {error}"))
            })?;
            Ok(seen["state"].as_str().unwrap_or_default().to_owned())
        };
        let mut state = read(self.hands)?;
        if state != "CLOSED" {
            self.hands
                .exec(&["gh", "issue", "close", &issue], How::write())?;
            self.done.push("issue-close");
            state = read(self.hands)?;
        }
        Ok(state)
    }

    fn facts(&self, receipt: &ReviewTransaction, merged: &Merged, ci: &Value) -> Facts {
        let read = |view: &str, extra: Value| -> Value {
            let mut arguments = json!({
                "issue": self.request.issue,
                "view": view,
                "epoch": receipt.epoch,
                "compact": true,
            });
            if let (Value::Object(into), Value::Object(from)) = (&mut arguments, extra) {
                into.extend(from);
            }
            self.hands
                .tool("review_read", arguments)
                .unwrap_or_else(|refusal| json!({"unavailable": refusal["code"]}))
        };
        Facts {
            issue: self.request.issue,
            run_id: self.request.run_id.clone(),
            receipt: receipt.clone(),
            merge_sha: merged.sha.clone(),
            parents: merged.parents.clone(),
            transaction: read("transaction", json!({})),
            findings: read("findings", json!({})),
            verdict: read(
                "verdict",
                json!({"pr": receipt.pr, "head": receipt.head, "base": receipt.base, "digest": receipt.digest}),
            ),
            checks: self
                .hands
                .checks_receipt(receipt)
                .unwrap_or_else(|refusal| Some(json!({"unavailable": refusal["code"]}))),
            ci: ci.clone(),
            spend: self.request.spend.clone(),
            appendix: self.request.appendix.clone(),
            language: self.hands.settings().summary_language,
            date: session::stamp_of(self.hands.now())
                .chars()
                .take(DATE_CHARS)
                .collect(),
        }
    }
}

fn all_running(next: &Value) -> bool {
    next["inputs"].as_array().is_some_and(|inputs| {
        !inputs.is_empty() && inputs.iter().all(|input| input["capture"].is_null())
    })
}

pub(super) struct Facts {
    pub(super) issue: u64,
    pub(super) run_id: String,
    pub(super) receipt: ReviewTransaction,
    pub(super) merge_sha: String,
    pub(super) parents: Vec<String>,
    pub(super) transaction: Value,
    pub(super) findings: Value,
    pub(super) verdict: Value,
    pub(super) checks: Option<Value>,
    pub(super) ci: Value,
    pub(super) spend: Option<String>,
    pub(super) appendix: Option<String>,
    pub(super) language: String,
    pub(super) date: String,
}

fn unavailable(read: &Value) -> Option<String> {
    read.get("unavailable")
        .map(|code| format!("unavailable ({})", code.as_str().unwrap_or("unread")))
}

fn lens_roles(transaction: &Value) -> String {
    use crate::transport::tier::{ReviewFocus, RiskLevel};
    let Some(level) = transaction["computed_level"]
        .as_str()
        .and_then(RiskLevel::parse)
    else {
        return "not recorded".to_owned();
    };
    let focus = ReviewFocus::parse(transaction["focus"].as_str()).unwrap_or_default();
    let roles = level.lens_roles(focus);
    if roles.is_empty() {
        "none (no lens licensed)".to_owned()
    } else {
        roles.join(", ")
    }
}

fn findings_by_class(findings: &Value) -> String {
    let Some(list) = findings["findings"].as_array() else {
        return unavailable(findings).unwrap_or_else(|| "not recorded".to_owned());
    };
    if list.is_empty() {
        return "none recorded".to_owned();
    }
    ["severe", "warning", "suggestion"]
        .iter()
        .filter_map(|class| {
            let ids: Vec<&str> = list
                .iter()
                .filter(|finding| finding["class"] == *class)
                .filter_map(|finding| finding["id"].as_str())
                .collect();
            (!ids.is_empty()).then(|| format!("{class}: {}", ids.join(", ")))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn ci_line(ci: &Value) -> String {
    match ci["rollup"].as_array() {
        Some(checks) => format!(
            "{} checks, all passing: {}",
            checks.len(),
            join_strings(checks)
        ),
        None => {
            let lane = &ci["lane"];
            match lane["state"].as_str() {
                Some(state) => {
                    let said = lane["annotation"]
                        .as_str()
                        .or_else(|| lane["read"].as_str())
                        .map(|text| format!(" ({text})"))
                        .unwrap_or_default();
                    format!("no CI ran: the publication lane reads `{state}`{said}")
                }
                None => "read before the merge".to_owned(),
            }
        }
    }
}

fn join_strings(values: &[Value]) -> String {
    values
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn summary(facts: &Facts) -> String {
    let verdict = facts.verdict["outcome"].as_str().unwrap_or("approved");
    let sha: String = facts.merge_sha.chars().take(SHA_CHARS).collect();
    match facts.language.as_str() {
        "Español" => format!(
            "Entregado #{} mediante el PR #{} en el commit de fusión {sha}, con veredicto calculado {verdict}.",
            facts.issue, facts.receipt.pr
        ),
        _ => format!(
            "Delivered #{} through PR #{} at merge commit {sha}, computed verdict {verdict}.",
            facts.issue, facts.receipt.pr
        ),
    }
}

pub(super) fn evidence(facts: &Facts) -> String {
    let receipt = &facts.receipt;
    let verdict = unavailable(&facts.verdict).unwrap_or_else(|| {
        format!(
            "{} (open: {}; follow-ups: {})",
            facts.verdict["outcome"].as_str().unwrap_or("not recorded"),
            facts.verdict["open"].as_array().map_or(0, Vec::len),
            facts.verdict["follow_ups"].as_array().map_or(0, Vec::len),
        )
    });
    let tier = unavailable(&facts.transaction).unwrap_or_else(|| {
        facts.transaction["computed_level"]
            .as_str()
            .unwrap_or("not recorded")
            .to_owned()
    });
    let checks = match &facts.checks {
        Some(recorded) if recorded.get("unavailable").is_some() => {
            "could not be read; unknown whether one was recorded".to_owned()
        }
        Some(recorded) => format!(
            "{} ({})",
            if recorded["passed"] == true {
                "passed"
            } else {
                "failed"
            },
            recorded["checks"]
                .as_array()
                .map(|runs| {
                    runs.iter()
                        .map(|run| {
                            format!(
                                "{} exit {}",
                                run["name"].as_str().unwrap_or("?"),
                                run["exit"]
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default()
        ),
        None => "none declared or none recorded".to_owned(),
    };
    let parents = match facts.parents.as_slice() {
        [first, second] => format!("first parent `{first}`, second parent `{second}`"),
        [] => "parents unread".to_owned(),
        other => format!("parents `{}`", other.join("`, `")),
    };
    let spend = match &facts.spend {
        Some(spend) => format!("recorded with this close: `{spend}`"),
        None => "none recorded with this close".to_owned(),
    };
    let mut note = format!(
        "{summary}\n\n\
         ## Delivery\n\n\
         - Issue #{issue}, pull request #{pr}, run `{run}`, {date} UTC\n\
         - Delivering commit: `{sha}` ({parents})\n\
         - Receipt: epoch `{epoch}`, head `{head}`, base `{base}`, digest `{digest}`\n\
         - Review tier: {tier}; lens roles: {roles}\n\
         - Computed verdict: {verdict}\n\
         - Findings: {findings}\n\
         - Declared checks receipt: {checks}\n\
         - CI: {ci}\n\
         - Spend: {spend}\n\
         - Version tags or releases published by this close: none\n",
        summary = summary(facts),
        issue = facts.issue,
        pr = receipt.pr,
        run = facts.run_id,
        date = facts.date,
        sha = facts.merge_sha,
        epoch = receipt.epoch,
        head = receipt.head,
        base = receipt.base,
        digest = receipt.digest,
        roles = lens_roles(&facts.transaction),
        findings = findings_by_class(&facts.findings),
        ci = ci_line(&facts.ci),
    );
    if let Some(appendix) = &facts.appendix {
        note.push_str("\n## Appendix\n\n");
        note.push_str(appendix.trim_end());
        note.push('\n');
    }
    note
}

struct Live<'a> {
    context: &'a GateContext,
    run_id: &'a str,
    transport: transport::Context,
}

impl Live<'_> {
    fn pinned<T>(&self, body: impl FnOnce() -> T) -> T {
        let _pin = crate::harness::gh_repo_pin();
        crate::harness::apply_gh_repo_locked(self.transport.repo.as_deref());
        body()
    }
}

impl Hands for Live<'_> {
    fn tool(&self, name: &str, arguments: Value) -> Result<Value, Value> {
        match super::run_tool(name, &arguments, Ok(self.context)) {
            Ok(result) => {
                let text = result["content"][0]["text"].as_str().unwrap_or("{}");
                Ok(serde_json::from_str(text).unwrap_or_else(|_| json!({"text": text})))
            }
            Err(ToolFailure::Refused(refusal)) => Err(json!({
                "code": refusal.code,
                "message": refusal.message,
                "resolution": refusal.resolution.to_string(),
            })),
            Err(ToolFailure::Malformed(message)) => {
                Err(json!({"code": "malformed-argument", "message": message}))
            }
        }
    }

    fn exec(&self, args: &[&str], how: How) -> Result<Output, Failure> {
        self.pinned(|| transport::run(args, Some(&self.transport.repo_dir), how))
    }

    fn merge_decision(&self, pr: u64) -> Result<(), Value> {
        let action = Action::Boundary {
            command: "gh pr merge".to_owned(),
            pr: Some(pr),
            pr_unidentified_reason: None,
            local_fast_forward_target: None,
            is_exact_integration_merge: false,
            runs_in: RunsIn::Session,
            target: CommitTarget::Checkout,
        };
        match crate::harness::guard::decide_action(
            self.context,
            &self.context.repo_dir,
            &action,
            Sensitivity::Boundary,
        ) {
            Decision::Allow(_) => Ok(()),
            Decision::Deny(refusal) | Decision::Ask(refusal) => Err(json!({
                "code": refusal.code,
                "message": refusal.message,
                "resolution": refusal.resolution.to_string(),
            })),
            Decision::Outside(aside) => Err(json!({
                "code": "merge-not-covered",
                "message": format!("the merge decision stood aside ({aside:?}): no claim of this run covers it"),
            })),
        }
    }

    fn receipt(&self) -> Option<ReviewTransaction> {
        session::load(&self.context.state_root, self.run_id)
            .review_transaction
            .filter(ReviewTransaction::is_complete)
    }

    fn worktree(&self) -> Option<String> {
        session::load(&self.context.state_root, self.run_id)
            .worktree
            .map(|path| path.display().to_string())
    }

    fn settings(&self) -> Settings {
        let layered = self.transport.layered();
        Settings {
            merge_by_agent: layered.merge_by() == crate::config::Party::Agent,
            delivery_auto: layered.delivery.is_autonomous(),
            summary_language: layered.summary_language.as_str().to_owned(),
        }
    }

    fn release_stamp(&self, receipt: &ReviewTransaction) -> Result<Option<String>, Value> {
        let issue = session::load(&self.context.state_root, self.run_id)
            .issue_number()
            .unwrap_or_default();
        transport::claim::ci_release_stamp(&self.transport, issue, receipt)
            .map_err(|failure| failure.envelope())
    }

    fn lane(&self, receipt: &ReviewTransaction) -> Value {
        self.pinned(|| transport::claim::ci_outlook(&self.transport, &receipt.head, receipt.pr))
            .0
    }

    fn checks_receipt(&self, receipt: &ReviewTransaction) -> Result<Option<Value>, Value> {
        let held = session::load(&self.context.state_root, self.run_id);
        let Some(issue) = held.issue_number() else {
            return Ok(None);
        };
        let worktree = held.worktree.as_deref();
        transport::claim::checks_receipt_summary(&self.transport, issue, receipt, worktree)
            .map_err(|failure| failure.envelope())
    }

    fn scratch(&self) -> PathBuf {
        self.context.state_root.join("deliver")
    }

    fn now(&self) -> u64 {
        session::now_seconds().unwrap_or_default()
    }
}

pub(super) fn deliver(
    arguments: &Value,
    context: &GateContext,
    named: Option<&NamedRun<'_>>,
) -> Result<Value, ToolFailure> {
    let Some(named) = named else {
        return Err(ToolFailure::Malformed(
            "deliver needs `run_id`: this run's id, as SessionStart reported it".to_owned(),
        ));
    };
    let issue = arguments["issue"].as_u64().unwrap_or_default();
    let seconds = match arguments.get("wait_seconds") {
        None => DEFAULT_WAIT_SECONDS,
        Some(sent) => match sent.as_u64() {
            Some(seconds) if (1..=MAX_WAIT_SECONDS).contains(&seconds) => seconds,
            _ => {
                return Err(ToolFailure::Malformed(format!(
                    "`wait_seconds` is a whole number from 1 to {MAX_WAIT_SECONDS}, not {sent}"
                )));
            }
        },
    };
    let compact = arguments.get("compact").and_then(Value::as_bool) == Some(true);
    let appendix = match arguments.get("appendix_file").and_then(Value::as_str) {
        None => None,
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(error) => {
                let answer = stop_answer(
                    "appendix",
                    "appendix-unreadable",
                    json!({"detail": format!("{path}: {error}")}),
                );
                return Ok(result_of(answer, compact));
            }
        },
    };
    let request = Request {
        issue,
        run_id: named.id.to_owned(),
        wait: Duration::from_secs(seconds),
        readback_pause: READBACK_PAUSE,
        spend: arguments
            .get("spend")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        appendix,
    };
    let live = Live {
        context,
        run_id: named.id,
        transport: transport::Context::live(
            context.skill_root.clone(),
            context.repo_dir.clone(),
            context.repository_host.named_repo(),
        ),
    };
    Ok(result_of(run(&live, &request), compact))
}

fn result_of(answer: Value, compact: bool) -> Value {
    json!({
        "content": [{"type": "text", "text": super::answer_text(Some(answer), compact)}],
        "isError": false,
    })
}

#[cfg(test)]
mod tests;
