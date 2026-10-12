//! The two reviewer CLIs, each run with no tools in an empty directory.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use super::run::{self, Finished, Launch, MAX_OUTPUT};
use super::scratch::Scratch;
use super::{Adapter, Invocation, ReviewerCode};
use crate::outcome::Refusal;

pub(super) const ISOLATED_SYSTEM_PROMPT: &str = "You are a read-only code reviewer with no tools. \
Answer only from the text you are given and reply with exactly what that text asks for.";

pub(super) const CODEX_DISABLED_FEATURES: &[&str] = &[
    "shell_tool",
    "unified_exec",
    "unified_exec_tty",
    "write_stdin_approval",
    "sleep_tool",
    "code_mode_host",
    "view_image",
    "image_generation",
    "apps",
    "plugins",
    "plugin_sharing",
    "remote_plugin",
    "skill_mcp_dependency_install",
    "skill_search",
    "tool_suggest",
    "browser_use",
    "browser_use_external",
    "browser_use_full_cdp_access",
    "computer_use",
    "in_app_browser",
    "in_app_local_automation",
    "hooks",
    "multi_agent",
];

pub(crate) const DEFAULT_DEADLINE: Duration = Duration::from_secs(15 * 60);
const SHORT_STDOUT: usize = 512;
const ANCHOR_BYTES: usize = 256;
const MIN_ANCHOR_BYTES: usize = 32;
const UNAVAILABLE_WINDOW: usize = 2 * 1024;
const EXCERPT_CHARS: usize = 240;
const LAST_MESSAGE: &str = "last-message.txt";

const QUOTA_MARKERS: &[&str] = &[
    "quota",
    "credit",
    "billing",
    "usage limit",
    "rate limit",
    "rate_limit",
];

const AUTH_MARKERS: &[&str] = &[
    "authenticat",
    "unauthorized",
    "not logged in",
    "log in",
    "login",
    "api key",
];

fn mentions(text: &str, markers: &[&str]) -> bool {
    let lowered = text.to_lowercase();
    markers.iter().any(|marker| lowered.contains(marker))
}

pub(super) fn indicates_unavailable(text: &str) -> bool {
    mentions(text, QUOTA_MARKERS) || mentions(text, AUTH_MARKERS)
}

fn is_authentication_failure(text: &str) -> bool {
    mentions(text, AUTH_MARKERS) && !mentions(text, QUOTA_MARKERS)
}

fn launch_value(environment: &[(OsString, OsString)], name: &str) -> String {
    environment.iter().find(|(key, _)| key == name).map_or_else(
        || format!("{name} is unset"),
        |(_, value)| format!("{name}={}", value.to_string_lossy()),
    )
}

fn unauthenticated_message(
    lead: &str,
    detail: &str,
    environment: &[(OsString, OsString)],
    config_var: &str,
) -> String {
    format!(
        "{lead} and reported itself unauthenticated in the environment it was launched with \
         ({}, {}): {detail}. A login from a shell with a different environment may not change \
         this; compare that environment with the operator's before asking for a login",
        launch_value(environment, "HOME"),
        launch_value(environment, config_var),
    )
}

pub(super) fn without_prompt(text: &[u8], prompt: &[u8]) -> String {
    let bounded = &text[text.len().saturating_sub(run::MAX_STDERR)..];
    if prompt.len() < MIN_ANCHOR_BYTES {
        return String::from_utf8_lossy(bounded).into_owned();
    }
    let anchor = &prompt[prompt.len().saturating_sub(ANCHOR_BYTES)..];
    let kept = bounded
        .windows(anchor.len())
        .rposition(|window| window == anchor)
        .map_or(bounded, |at| &bounded[at + anchor.len()..]);
    String::from_utf8_lossy(kept).into_owned()
}

fn tail_chars(text: &str, count: usize) -> String {
    let kept = text.chars().count().saturating_sub(count);
    text.chars().skip(kept).collect()
}

fn excerpt(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    tail_chars(&flat, EXCERPT_CHARS)
}

fn is_blank(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_whitespace)
}

fn require_success(
    program: &std::path::Path,
    finished: Finished,
    prompt: &[u8],
    environment: &[(OsString, OsString)],
    config_var: &str,
) -> Result<Finished, Refusal> {
    if finished.code == Some(0) {
        return Ok(finished);
    }
    let shown = program.display();
    let code = run::exit_description(finished.code);
    let stderr = without_prompt(&finished.stderr, prompt);
    let detail = excerpt(&stderr);
    let haystack = if stderr.trim().is_empty() && finished.stdout.len() <= SHORT_STDOUT {
        String::from_utf8_lossy(&finished.stdout).into_owned()
    } else {
        tail_chars(&stderr, UNAVAILABLE_WINDOW)
    };
    if indicates_unavailable(&haystack) {
        let lead = format!("{shown} exited with {code}");
        let message = if is_authentication_failure(&haystack) {
            unauthenticated_message(&lead, &detail, environment, config_var)
        } else {
            format!("{lead} and reported that it is unavailable: {detail}")
        };
        return Err(ReviewerCode::Unavailable.refusal(message));
    }
    Err(ReviewerCode::ExitedNonzero.refusal(format!("{shown} exited with {code}: {detail}")))
}

enum ClaudeResult {
    Answer(String),
    Error {
        subtype: Option<String>,
        detail: String,
    },
}

fn parse_result(stdout: &[u8]) -> Option<ClaudeResult> {
    let value: serde_json::Value = serde_json::from_slice(stdout).ok()?;
    if value.get("type")?.as_str()? != "result" {
        return None;
    }
    let result = value.get("result").and_then(serde_json::Value::as_str);
    if value.get("is_error")?.as_bool()? {
        let mut parts: Vec<String> = result.map(str::to_owned).into_iter().collect();
        for error in value
            .get("errors")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let text = error
                .as_str()
                .or_else(|| error.get("message").and_then(serde_json::Value::as_str));
            parts.extend(text.map(str::to_owned));
        }
        Some(ClaudeResult::Error {
            subtype: value
                .get("subtype")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            detail: parts.join(" "),
        })
    } else {
        Some(ClaudeResult::Answer(result?.to_owned()))
    }
}

fn push_model(args: &mut Vec<OsString>, model: Option<&str>) {
    if let Some(model) = model {
        args.push("--model".into());
        args.push(model.into());
    }
}

pub(super) fn claude_args(model: Option<&str>) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "--print",
        "--output-format",
        "json",
        "--tools",
        "",
        "--permission-mode",
        "dontAsk",
        "--no-session-persistence",
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--system-prompt",
        ISOLATED_SYSTEM_PROMPT,
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    push_model(&mut args, model);
    args
}

pub(super) fn codex_args(
    scratch: &std::path::Path,
    last_message: &std::path::Path,
    model: Option<&str>,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "exec",
        "--skip-git-repo-check",
        "--ignore-user-config",
        "--sandbox",
        "read-only",
        "-C",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push(scratch.into());
    args.push("--output-last-message".into());
    args.push(last_message.into());
    for feature in CODEX_DISABLED_FEATURES {
        args.push("--disable".into());
        args.push((*feature).into());
    }
    args.push("-c".into());
    args.push("shell_environment_policy.inherit=none".into());
    push_model(&mut args, model);
    args
}

/// Runs the reviewer through `claude --print --output-format json`, taking its
/// answer from the `result` of the single result object on standard output.
#[derive(Debug, Clone)]
pub struct ClaudeCode {
    program: PathBuf,
    deadline: Duration,
}

impl ClaudeCode {
    /// An adapter for the `claude` executable at `program`.
    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            deadline: DEFAULT_DEADLINE,
        }
    }

    /// The same adapter with a different deadline for the whole process tree.
    pub fn with_deadline(self, deadline: Duration) -> Self {
        Self { deadline, ..self }
    }
}

impl Adapter for ClaudeCode {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        let scratch = Scratch::new()
            .map_err(|error| run::launch_failed(format!("no scratch directory: {error}")))?;
        let args = claude_args(invocation.model.as_deref());
        let environment = run::child_environment();
        let finished = run::execute(&Launch {
            program: &self.program,
            args: &args,
            cwd: scratch.path(),
            stdin: &invocation.prompt,
            environment: &environment,
            deadline: self.deadline,
        })?;
        let shown = self.program.display();
        let mut finished = finished;
        if finished.code != Some(0)
            && is_blank(&finished.stderr)
            && let Some(ClaudeResult::Error { detail: text, .. } | ClaudeResult::Answer(text)) =
                parse_result(&finished.stdout)
        {
            finished.stderr = text.into_bytes();
        }
        let stdout = require_success(
            &self.program,
            finished,
            &invocation.prompt,
            &environment,
            "CLAUDE_CONFIG_DIR",
        )?
        .stdout;
        if is_blank(&stdout) {
            return Err(
                ReviewerCode::NoAnswer.refusal(format!("{shown} exited 0 with an empty answer"))
            );
        }
        match parse_result(&stdout) {
            Some(ClaudeResult::Answer(text)) if is_blank(text.as_bytes()) => {
                Err(ReviewerCode::NoAnswer
                    .refusal(format!("{shown} exited 0 with an empty answer")))
            }
            Some(ClaudeResult::Answer(text)) => Ok(text.into_bytes()),
            Some(ClaudeResult::Error { subtype, detail }) => {
                let text = without_prompt(detail.as_bytes(), &invocation.prompt);
                let window = tail_chars(&text, UNAVAILABLE_WINDOW);
                let lead = format!(
                    "{shown} exited 0 but reported an error result (subtype {})",
                    subtype.as_deref().unwrap_or("none")
                );
                let detail = excerpt(&text);
                if is_authentication_failure(&window) {
                    return Err(ReviewerCode::Unavailable.refusal(unauthenticated_message(
                        &lead,
                        &detail,
                        &environment,
                        "CLAUDE_CONFIG_DIR",
                    )));
                }
                let code = if indicates_unavailable(&window) {
                    ReviewerCode::Unavailable
                } else {
                    ReviewerCode::ErrorResult
                };
                Err(code.refusal(format!("{lead}: {detail}")))
            }
            None => Err(run::launch_failed(format!(
                "{shown} exited 0 and the reviewer's answer could not be read as the JSON \
                 result object: {}",
                excerpt(&without_prompt(&stdout, &invocation.prompt))
            ))),
        }
    }
}

/// Runs the reviewer through `codex exec`, taking its answer from the
/// `--output-last-message` file and from nowhere else.
#[derive(Debug, Clone)]
pub struct Codex {
    program: PathBuf,
    deadline: Duration,
}

impl Codex {
    /// An adapter for the `codex` executable at `program`.
    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            deadline: DEFAULT_DEADLINE,
        }
    }

    /// The same adapter with a different deadline for the whole process tree.
    pub fn with_deadline(self, deadline: Duration) -> Self {
        Self { deadline, ..self }
    }
}

impl Adapter for Codex {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        let scratch = Scratch::new()
            .map_err(|error| run::launch_failed(format!("no scratch directory: {error}")))?;
        let last_message = scratch.path().join(LAST_MESSAGE);
        let args = codex_args(scratch.path(), &last_message, invocation.model.as_deref());
        let environment = run::child_environment();
        let finished = run::execute(&Launch {
            program: &self.program,
            args: &args,
            cwd: scratch.path(),
            stdin: &invocation.prompt,
            environment: &environment,
            deadline: self.deadline,
        })?;
        require_success(
            &self.program,
            finished,
            &invocation.prompt,
            &environment,
            "CODEX_HOME",
        )?;
        let shown = self.program.display();
        let size = match std::fs::metadata(&last_message) {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ReviewerCode::NoAnswer
                    .refusal(format!("{shown} exited 0 but wrote no last message")));
            }
            Err(error) => {
                return Err(run::launch_failed(format!(
                    "{shown} last message could not be inspected: {error}"
                )));
            }
        };
        if size > MAX_OUTPUT as u64 {
            return Err(run::too_large(&shown, None));
        }
        let answer = std::fs::read(&last_message).map_err(|error| {
            run::launch_failed(format!("{shown} last message could not be read: {error}"))
        })?;
        if is_blank(&answer) {
            return Err(ReviewerCode::NoAnswer
                .refusal(format!("{shown} exited 0 with an empty last message")));
        }
        Ok(answer)
    }
}
