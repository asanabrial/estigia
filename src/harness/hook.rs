//! The agent's lifecycle protocol, and the only place a decision becomes a
//! `deny`.
//!
//! Hooks sit on the critical path of every edit. Three consequences run through
//! everything here:
//!
//! 1. **Fast.** No async runtime, no database, no network unless the gate has to
//!    ask. A process that takes half a second is half a second on every write.
//! 2. **Unknown fields are ignored.** The payload schema belongs to the agent
//!    and it will grow; a hook that fails to parse is a hook that blocks work
//!    for a field it never read.
//! 3. **A hook that breaks does not deny.** Estigia failing is not the tracker
//!    saying stop, and turning one into the other would make a bug in this file
//!    indistinguishable from a lost claim race. Only the gate denies.

use std::io::{BufRead, BufReader, Read};

use serde::Deserialize;
use serde_json::{Value, json};

use super::{Action, Decision, GateContext, RENEWAL_WINDOW, classify_observing, session};
use crate::outcome::{CheckoutObserved, CheckoutSource, Observed, Refusal};
use crate::transport::store;

/// The lifecycle events Estigia registers for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Before a tool runs. The one that can say no.
    PreToolUse,
    /// After a shell tool ran. Says nothing and decides nothing.
    PostToolUse,
    /// A session begins or resumes. Mints the run identity.
    SessionStart,
    /// A session ends. Forgets the pointer; the claim on the tracker stays.
    SessionEnd,
    /// A turn is about to end. Continues it while the run still holds its
    /// issue, and stands aside otherwise.
    Stop,
    /// Git is about to push. **Not an agent event** — git's own, and the one
    /// gate no agent can route around: it refuses a push typed by any agent,
    /// by a person, or by a script.
    PrePush,
}

impl Event {
    /// The slug typed on the command line and written into the settings file.
    pub fn slug(self) -> &'static str {
        match self {
            Self::PreToolUse => "pre-tool-use",
            Self::PostToolUse => "post-tool-use",
            Self::SessionStart => "session-start",
            Self::SessionEnd => "session-end",
            Self::Stop => "stop",
            Self::PrePush => "pre-push",
        }
    }

    /// The name the agent uses for this event.
    pub fn agent_name(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::Stop => "Stop",
            Self::PrePush => "pre-push",
        }
    }

    /// Whether this event can say **no**, which is what makes a payload matter.
    /// The session hooks adjudicate no write; the turn hook continues one.
    pub fn decides(self) -> bool {
        matches!(self, Self::PreToolUse | Self::PrePush | Self::Stop)
    }

    /// Every event, checked by the compiler.
    pub fn all() -> [Self; 6] {
        [
            Self::PreToolUse,
            Self::PostToolUse,
            Self::SessionStart,
            Self::SessionEnd,
            Self::Stop,
            Self::PrePush,
        ]
    }

    /// The event a name names, in either spelling: **both** the slug Estigia
    /// writes and [`Self::agent_name`], with case and separators ignored. A
    /// settings file is a thing people copy and hand-edit, and the name they
    /// reach for is the one their agent's documentation prints — refusing it
    /// exits non-zero, which for a `PreToolUse` hook is a *non-blocking* error
    /// and the tool call goes through ungated.
    pub fn from_slug(slug: &str) -> Option<Self> {
        let wanted = spelling(slug);
        if wanted.is_empty() {
            return None;
        }
        Self::all().into_iter().find(|event| {
            spelling(event.slug()) == wanted || spelling(event.agent_name()) == wanted
        })
    }
}

/// One event name, reduced to the letters in it.
///
/// Case and separators are the two things that differ between a name typed by a
/// person, a name written by `setup`, and a name copied out of somebody else's
/// documentation. Nothing else is dropped, so a genuine misspelling is still a
/// misspelling and still says so.
fn spelling(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// Every **lifecycle** event Estigia registers in an agent, with the matcher each
/// needs and the timeout its host is given.
///
/// `PrePush` is deliberately absent: it belongs to git, not to an agent, and it
/// is installed by `estigia guard` into a repository rather than by `setup`
/// into a settings file.
///
/// `PreToolUse` is absent too, and it is the gate rather than a lifecycle event:
/// its matcher, timeout and failure policy differ per agent, so they are
/// `AgentAdapter::gate_spec`'s to describe. [`Event::all`] is every event this
/// build answers.
pub const EVENTS: &[(Event, Option<&str>, u64)] = &[
    (Event::PostToolUse, Some("Bash"), 10),
    (Event::SessionStart, None, 5),
    (Event::SessionEnd, None, 5),
    (Event::Stop, None, 5),
];

/// What the agent sends on standard input.
///
/// Unknown fields are ignored so the agent can extend its schema without
/// breaking the hook.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Input {
    /// The agent's session identity, from which the run-id is minted.
    #[serde(default, alias = "trajectory_id")]
    pub session_id: String,
    /// The working directory the agent is running in.
    #[serde(default)]
    pub cwd: String,
    /// Which sub-agent made this call, when one did. Claude Code sends
    /// `agent_type` on every tool event fired **inside** a sub-agent; absent
    /// means the main conversation made the call.
    #[serde(default, alias = "agent_name")]
    pub agent_type: Option<String>,
    /// The tool about to run, on `PreToolUse`.
    #[serde(default, alias = "agent_action_name")]
    pub tool_name: String,
    /// That tool's arguments.
    #[serde(default, alias = "tool_info", alias = "tool_call")]
    pub tool_input: Value,
    /// Whether the agent is already stopping after an earlier block. The host
    /// caps consecutive blocks, then sets this; answering anything but aside
    /// here would wedge the session.
    #[serde(default)]
    pub stop_hook_active: bool,
    /// Where the host keeps this session's transcript, when it offers one.
    ///
    /// `Stop` uses it to tell a run that is waiting on a background launch from
    /// one that is unfinished.
    #[serde(default, alias = "transcriptPath", deserialize_with = "text_or_null")]
    pub transcript_path: String,
    /// Whether a body arrived and could not be read: standing aside is right —
    /// denying it would deny reads — but doing it silently would leave the gate
    /// looking installed while deciding nothing.
    #[serde(skip)]
    pub unreadable: bool,
    /// Whether no body arrived at all. Two words rather than one because the
    /// fix is different: a payload that will not parse is a schema to teach
    /// the classifier; a payload that never arrives is a registration to
    /// repair.
    #[serde(skip)]
    pub absent: bool,
    /// What was wrong with a payload that would not parse.
    #[serde(skip)]
    pub why: Option<String>,
}

impl Input {
    /// The role an `Agent` or legacy `Task` call asks Claude to launch.
    pub fn launch_target(&self) -> Option<&str> {
        self.tool_input
            .get("subagent_type")
            .and_then(Value::as_str)
            .filter(|target| !target.is_empty())
    }

    /// The model an `Agent` or legacy `Task` call named, when it named one.
    ///
    /// A launch declaration, not a check: this is what the launcher asked for,
    /// and nothing here proves the host ran it.
    pub fn launch_model(&self) -> Option<&str> {
        self.tool_input
            .get("model")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|model| !model.is_empty())
    }

    /// The effort an `Agent` or legacy `Task` call named, when it named one.
    ///
    /// The same declaration [`Self::launch_model`] is, and absent on a host
    /// whose launch surface carries no effort field.
    pub fn launch_effort(&self) -> Option<&str> {
        self.tool_input
            .get("effort")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|effort| !effort.is_empty())
    }
}

fn text_or_null<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(<Option<String> as Deserialize>::deserialize(deserializer)?.unwrap_or_default())
}

/// Whether this tool is Claude's current or legacy sub-agent launch surface.
pub fn is_prelaunch_tool(agent: Option<&str>, tool: &str) -> bool {
    agent == Some("claude-code")
        && ["Agent", "Task"]
            .iter()
            .any(|surface| surface.eq_ignore_ascii_case(tool.trim()))
}

/// Why a payload would not parse, in terms an operator can act on.
///
/// Names, never values. This goes to a file on disk, and a payload is a tool
/// call: `tool_input` can carry a token, a diff, or somebody's private branch.
/// Top-level **keys** are the schema, and the schema is the whole of what there
/// is to teach — so the keys go in and nothing else does.
fn why_unreadable(body: &str, error: &serde_json::Error) -> String {
    let shape = match serde_json::from_str::<Value>(body) {
        Ok(Value::Object(map)) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            // Bounded: a ledger line is read in a terminal, and an agent that
            // sends forty keys has already been identified by its first twelve.
            let shown = keys.len().min(12);
            format!(
                "valid JSON this build does not know, whose top-level keys are [{}{}]",
                keys[..shown].join(", "),
                if keys.len() > shown { ", …" } else { "" }
            )
        }
        Ok(_) => "valid JSON, but not an object".to_owned(),
        Err(_) => format!("{} bytes that are not JSON at all", body.len()),
    };
    format!(
        "a payload arrived and could not be parsed, so this call was not gated: {shape}; \
         the read stopped at line {} column {} ({:?})",
        error.line(),
        error.column(),
        error.classify()
    )
}

/// Reads a payload, tolerating an empty body.
pub fn read_input(mut reader: impl Read) -> Input {
    let mut body = String::new();
    if reader.read_to_string(&mut body).is_err() || body.trim().is_empty() {
        return Input {
            absent: true,
            ..Input::default()
        };
    }
    // A payload that will not parse is a schema this build does not know. That
    // is not grounds to stop somebody's edit — but it is grounds to say so, or
    // the gate goes quiet in a way nobody can see. See `Input::unreadable`.
    let mut input: Input = match serde_json::from_str(&body) {
        Ok(input) => input,
        Err(error) => Input {
            unreadable: true,
            why: Some(why_unreadable(&body, &error)),
            ..Input::default()
        },
    };
    normalise(&mut input);
    input
}

/// Folds the spellings a rename cannot reach into the common shape.
///
/// `serde(alias)` handles a field with another name. It cannot reach a field
/// that lives one level down, and Windsurf puts both the working directory and
/// the command inside `tool_info`. Left alone, the gate would read an empty cwd
/// and a shell command with no command in it — and classify a push as something
/// it had never heard of.
fn normalise(input: &mut Input) {
    // Cline puts the tool one level down and sends no session id and no working
    // directory at all. The first is a rename a `serde` alias cannot reach; the
    // other two are absences, and `run_in` already knows what to do with a
    // missing session — ask which oath covers this checkout, exactly as the git
    // hook does.
    if input.tool_name.trim().is_empty()
        && let Some(name) = input.tool_input.get("name").and_then(Value::as_str)
        && let Some(arguments) = input.tool_input.get("input").cloned()
    {
        input.tool_name = name.to_owned();
        input.tool_input = arguments;
    }

    if input.cwd.trim().is_empty()
        && let Some(cwd) = input.tool_input.get("cwd").and_then(Value::as_str)
    {
        input.cwd = cwd.to_owned();
    }
    // Windsurf's `command_line` was copied to `command` here, and that copy is
    // gone: `classify_with` reads a key by its letters, so the two are one
    // argument to it. The copy was not merely redundant — it made the two doors
    // disagree. `estigia gate` reaches the classifier **without** passing
    // through this function, and it is the door OpenCode's plugin uses, so the
    // spelling this line translated was invisible through the other one.
}

/// How one agent spells a decision.
///
/// Agents can deny a tool call and none of them agrees on how to say so.
/// The differences are small and total: the key names, the verb, and where the
/// reason goes. Getting one wrong produces a hook that runs, decides correctly,
/// and is ignored — the worst outcome available, because it reports success and
/// enforces nothing.
///
/// Each is taken from that agent's published reference, and the seam is held by
/// `every_dialect_denies_in_its_own_words`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// `hookSpecificOutput.permissionDecision`, with the reason beside it.
    #[default]
    ClaudeCode,
    /// `{"decision": "deny", "reason": ...}`, where `reason` is required when
    /// denied and is delivered to the agent as a tool error.
    GeminiCli,
    /// `{"permission": "deny", "agent_message": ...}`, with a separate
    /// `user_message` for the person watching.
    Cursor,
    /// The odd one out, and worth its own variant rather than a flag: a
    /// decision printed as JSON where an exit code is expected is a refusal
    /// that prints itself and lets the write through.
    ExitCode,
    /// `{"review": true, "context": ...}` — Cline, which has no "deny" at all;
    /// Estigia uses `review`, which pauses for a person rather than killing the
    /// task.
    Cline,
}

impl Dialect {
    /// The name typed on the command line and written into the hook command.
    pub fn slug(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::GeminiCli => "gemini-cli",
            Self::Cursor => "cursor",
            Self::ExitCode => "exit-code",
            Self::Cline => "cline",
        }
    }

    /// Whether this dialect answers with a process status rather than JSON.
    ///
    /// The one question `cli::hook` has to ask before it prints anything.
    pub fn answers_with_status(self) -> bool {
        matches!(self, Self::ExitCode)
    }

    /// The dialect a slug names, or the default.
    ///
    /// Falls back rather than refusing: a settings file naming a dialect this
    /// build does not have is a hook that would otherwise block every edit
    /// until somebody worked out why.
    pub fn from_slug(slug: &str) -> Self {
        Self::all()
            .into_iter()
            .find(|dialect| dialect.slug() == slug)
            .unwrap_or_default()
    }

    /// Every dialect, checked by the compiler: the `match` below has no
    /// wildcard, so a variant added to the enum and not to this list stops the
    /// build.
    pub fn all() -> Vec<Self> {
        let every = vec![
            Self::ClaudeCode,
            Self::GeminiCli,
            Self::Cursor,
            Self::ExitCode,
            Self::Cline,
        ];
        for dialect in &every {
            match dialect {
                Self::ClaudeCode
                | Self::GeminiCli
                | Self::Cursor
                | Self::ExitCode
                | Self::Cline => {}
            }
        }
        every
    }

    /// Standing aside, in this dialect.
    ///
    /// Every one of them treats an empty object as "no opinion", which is the
    /// one thing they do agree on — and it matters, because an explicit allow
    /// would override the operator's own permission settings.
    fn aside(self) -> Value {
        json!({})
    }

    /// Allowing, in this dialect.
    fn allow(self, reason: &str) -> Value {
        match self {
            Self::ClaudeCode => json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "permissionDecisionReason": format!("estigia: {reason}"),
                }
            }),
            Self::GeminiCli => json!({ "decision": "allow" }),
            Self::Cursor => json!({ "permission": "allow" }),
            // Nothing to say: an exit code of zero is the whole answer, and
            // stdout is shown to a person rather than parsed.
            Self::ExitCode | Self::Cline => json!({}),
        }
    }

    /// Claude Code is the only dialect with an ask this process can rely on; the rest deny.
    fn ask(self, refusal: &Refusal) -> Value {
        match self {
            Self::ClaudeCode => json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "ask",
                    "permissionDecisionReason": format!(
                        "estigia asks before this write.\n\n{}\n({})\n\n{}",
                        refusal.message, refusal.code, refusal.resolution
                    ),
                }
            }),
            Self::GeminiCli | Self::Cursor | Self::ExitCode | Self::Cline => self.deny(refusal),
        }
    }

    /// Denying, in this dialect, carrying the whole refusal.
    fn deny(self, refusal: &Refusal) -> Value {
        let reason = reason_text("estigia refused this write.", refusal);
        match self {
            Self::ClaudeCode => json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": reason,
                }
            }),
            // `reason` is required when denied, and is delivered to the agent
            // as a tool error it can respond to.
            Self::GeminiCli => json!({ "decision": "deny", "reason": reason }),
            // Two messages: one for the agent and one for the person watching.
            // The person gets the short form; the agent gets everything, since
            // it is the one that has to act.
            Self::Cursor => json!({
                "permission": "deny",
                "agent_message": reason,
                "user_message": format!("estigia: {} ({})", refusal.message, refusal.code),
            }),
            // Carried, not printed. `cli::hook` writes this to standard error
            // and exits 2, which is the only thing Windsurf reads. Returning it
            // as a value keeps one decision path for every dialect and puts the
            // difference in one place — how the answer leaves the process.
            Self::ExitCode => json!({ "status": 2, "stderr": reason }),
            Self::Cline => json!({ "review": true, "context": reason }),
        }
    }
}

/// The JSON a `PreToolUse` hook returns to allow, deny, or stand aside.
///
/// `deny` carries the whole refusal — the code, what happened to the world, and
/// the resolution — because the agent reads this text and it is the only place
/// the agent learns what to do next. A denial that says "not allowed" is the
/// dead end the ratchet forbids.
pub fn pre_tool_use_response(decision: &Decision) -> Value {
    response_in(Dialect::ClaudeCode, decision)
}

/// The same decision, spelled the way one agent reads it.
pub fn response_in(dialect: Dialect, decision: &Decision) -> Value {
    match decision {
        Decision::Outside(_) => dialect.aside(),
        Decision::Allow(reason) => dialect.allow(reason),
        Decision::Deny(refusal) => dialect.deny(refusal),
        Decision::Ask(refusal) => dialect.ask(refusal),
    }
}

const ASKS_ON_PRE_TOOL_USE: &str = "claude-code";

fn can_ask(dialect: Dialect, agent: Option<&str>) -> bool {
    dialect == Dialect::ClaudeCode && agent == Some(ASKS_ON_PRE_TOOL_USE)
}

/// The text an agent is shown when a write is refused or put to a person.
fn reason_text(lead_in: &str, refusal: &Refusal) -> String {
    format!("{lead_in}\n\n{}", refusal.agent_text("command"))
}

/// Writes one decision to the ledger, when there was a decision to write.
///
/// `Outside` is not one: it is Estigia standing aside, and a line for every tool
/// call of every session that never swore would bury the calls that mattered
/// under the ones that never involved it.
pub(crate) fn note(
    context: &GateContext,
    run_id: &str,
    tool: &str,
    subject: Option<&str>,
    decision: &Decision,
    aimed_at: &[String],
) {
    let (verdict, detail) = match decision {
        Decision::Outside(_) => return,
        Decision::Allow(reason) => ("allow", reason.clone()),
        Decision::Deny(refusal) => ("deny", format!("{}: {}", refusal.code, refusal.message)),
        Decision::Ask(refusal) => ("ask", format!("{}: {}", refusal.code, refusal.message)),
    };
    let repository = crate::skill::repository_key(&context.repo_dir);
    let mut entry = json!({
            "at": session::now_seconds(),
            "run_id": run_id,
            "tool": tool,
            "verdict": verdict,
            "detail": detail,
            "repo_dir": context.repo_dir.display().to_string(),
            // The flag this run declared, when it declared one: accepting an
            // unverifiable self-declaration is what makes the claim explicit
            // **and recorded**.
            "flag": context.flag,
    });
    if let Some(repository) = &repository {
        entry["repo"] = json!(repository);
    }
    // What the decision was *about*: the target, or the command.
    if let Some(subject) = subject.filter(|subject| !subject.trim().is_empty())
        && let Value::Object(fields) = &mut entry
    {
        fields.insert("subject".to_owned(), json!(subject));
    }
    // What a push was aimed at, when the boundary was a push. Added rather
    // than always present: a key that is `[]` on every call is noise.
    if !aimed_at.is_empty() {
        entry["aimed_at"] = json!(aimed_at);
    }
    session::record(&context.state_root, &entry);
    if let Decision::Ask(refusal) = decision {
        record_ask(context, run_id, refusal.code, repository.as_deref());
    }
}

/// The same ask for the same run is a new need once this many seconds have passed.
const ASK_DEDUP_SECONDS: u64 = 1800;

fn ask_key(run_id: &str, code: &str, now: u64) -> String {
    format!("ask:{run_id}:{code}:{}", now / ASK_DEDUP_SECONDS)
}

fn record_ask(context: &GateContext, run_id: &str, code: &str, repository: Option<&str>) {
    let Some(now) = session::now_seconds() else {
        return;
    };
    let repo = repository.map_or_else(|| context.repo_dir.display().to_string(), str::to_owned);
    let recorded =
        crate::transport::store::Store::open(&crate::transport::store::beside(&context.state_root))
            .and_then(|store| {
                let policy = crate::transport::Context::live(
                    context.skill_root.clone(),
                    context.repo_dir.clone(),
                    None,
                )
                .get("notifications")
                .and_then(crate::config::Notifications::parse)
                .unwrap_or_default();
                crate::notify::record(
                    &store,
                    &crate::transport::store::Need {
                        kind: "ask",
                        repo: &repo,
                        issue: None,
                        run_id: Some(run_id),
                        dedup_key: &ask_key(run_id, code, now),
                    },
                    &policy,
                    now,
                )
            });
    if let Err(failure) = recorded {
        session::record(
            &context.state_root,
            &json!({
                "at": now,
                "run_id": run_id,
                "tool": "needs-person",
                "verdict": "unrecorded",
                "detail": failure.detail(),
                "repo_dir": context.repo_dir.display().to_string(),
                "repo": repo,
            }),
        );
    }
}

/// How a run's workflow state is named in the sentence a session opens with.
///
/// Pure and fed. `None` is not `in-progress`: naming the default would tell
/// the agent the tracker said something it never said.
fn state_clause(state: Option<&str>) -> String {
    match state {
        Some(state) => format!("in `{state}`"),
        None => "in a state this run never recorded".to_owned(),
    }
}

fn with_queue_note(context: &GateContext, mut answer: Value) -> Value {
    let stops_nothing = answer.get("decision").is_none();
    if stops_nothing
        && let (Some(note), Some(fields)) = (queue_note(context, true), answer.as_object_mut())
    {
        fields.insert("systemMessage".to_owned(), json!(note));
    }
    answer
}

pub(crate) fn with_decisions(context: &GateContext, run_id: &str, mut answer: Value) -> Value {
    if answer.get("decision").is_some() {
        return answer;
    }
    let Some(report) = super::decision_mode::report(&context.state_root, run_id) else {
        return answer;
    };
    if let Some(fields) = answer.as_object_mut() {
        let message = match fields.get("systemMessage").and_then(Value::as_str) {
            Some(before) => format!("{before}\n\n{report}"),
            None => report,
        };
        fields.insert("systemMessage".to_owned(), json!(message));
    }
    answer
}

std::thread_local! {
    #[cfg(test)]
    static FLUSH_REQUESTS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

fn start_background_flush() -> bool {
    #[cfg(test)]
    {
        FLUSH_REQUESTS.with(|requests| requests.set(requests.get() + 1));
        true
    }
    #[cfg(not(test))]
    {
        std::env::current_exe()
            .ok()
            .and_then(|executable| crate::transport::outbox::flush::detach(&executable).ok())
            .is_some()
    }
}

/// What to say about the GitHub writes the closed window held back, or nothing
/// when none wait. Reads only the local queue count; when the window is open and
/// writes wait, it also starts the flush that sends them.
fn queue_note(context: &GateContext, with_count: bool) -> Option<String> {
    let queued = crate::transport::outbox::queued_writes();
    if queued == 0 {
        return None;
    }
    let writes = match (with_count, queued) {
        (false, _) => "GitHub writes are".to_owned(),
        (true, 1) => "1 GitHub write is".to_owned(),
        (true, _) => format!("{queued} GitHub writes are"),
    };
    let window = &context.operation_window;
    if super::operation_window::permits_now(window, super::operation_window::now_minutes()) {
        return Some(if start_background_flush() {
            format!(
                "{writes} queued from a closed git operation window; `estigia flush` was started \
                 in the background to send them in order."
            )
        } else {
            format!(
                "{writes} queued from a closed git operation window and the flush could not be \
                 started: run `estigia flush`."
            )
        });
    }
    let now = super::operation_window::now_minutes();
    let until = super::operation_window::permitted_period(window)
        .map_or_else(String::new, |period| format!(" ({period})"));
    let until = match super::operation_window::next_opening(window, now) {
        Some(at) => format!("{until}, at {at} (`opens_at`)"),
        None => until,
    };
    Some(format!(
        "{writes} queued; nothing reaches GitHub until the git operation window opens{until}. \
         Claims, comments and transitions keep working against the local store and `estigia \
         flush` sends them in order. {}.",
        super::operation_window::LOCAL_WORK
    ))
}

fn queue_paragraph(context: &GateContext, with_count: bool) -> String {
    queue_note(context, with_count).map_or_else(String::new, |note| format!("\n\n{note}"))
}

fn orphan_paragraph(context: &GateContext) -> String {
    let transport =
        crate::transport::Context::live(context.skill_root.clone(), context.repo_dir.clone(), None);
    let mut holdings = session::holdings(&context.state_root);
    holdings.extend(session::review_holdings(&context.state_root));
    super::changes::orphan_notice(&transport, &holdings)
        .map_or_else(String::new, |note| format!("\n\n{note}"))
}

/// The context injected when a session begins.
pub fn session_start_response(context: &GateContext, run: &session::Run) -> Value {
    session_start_body(context, run, true)
}

/// What a run holding no claim is told about the issue requirement in force here.
fn unclaimed_hold(context: &GateContext, requirement: crate::config::IssueRequirement) -> String {
    use super::issue_requirement::Scope;
    let scope = super::issue_requirement::scope_of(&context.repo_dir);
    if let Scope::Repository(root) = &scope
        && !super::issue_requirement::names_an_issue(context, root)
    {
        return "This run holds no issue. The tracker names no repository and this checkout \
                has no git remote, so there is nothing here to file or claim one in: Estigia \
                gates no write here, and no commit or push is refused for want of a claim. \
                Reads are not gated."
            .to_owned();
    }
    let way_in = if context.workflow == Some(crate::config::Workflow::Assisted) {
        "Take a local claim with `assist` in a worktree first, which gives the isolated checkout \
         to write in."
    } else {
        "Claim an issue first, filing one with `create` when none covers the change, then work \
         through `claim` with `branch` and `base`."
    };
    let held = "Estigia refuses every repository write until one is claimed";
    match scope {
        Scope::Repository(_) => format!(
            "This run holds no issue, and `Issue required` is `{}` in this \
             repository: {held}. {way_in} Reads are not gated.",
            requirement.as_value()
        ),
        Scope::Outside => format!(
            "This run holds no issue. This directory is not inside a repository Estigia \
             can place, so no issue requirement applies to a write that lands here: \
             `Issue required` is `{}` for a write that lands in a repository Estigia \
             can place, and there {held}. Reads are not gated.",
            requirement.as_value()
        ),
        Scope::Unknown => format!(
            "This run holds no issue. `Issue required` is `{}` here, but whether \
             this directory is inside a repository could not be established, so every \
             write from it is held rather than exempted: {held}. Reads are not gated.",
            requirement.as_value()
        ),
    }
}

fn session_start_body(context: &GateContext, run: &session::Run, with_count: bool) -> Value {
    // Unreadable is not absent: what the run holds is unknown, not nothing.
    let held = if run.unreadable {
        format!(
            "This run's record exists and cannot be read, so whether it holds an issue is \
             unknown. Estigia refuses every repository write from this run until that is \
             settled: read what `{}` holds from the tracker and claim it again, or take its \
             pointer away.",
            run.run_id
        )
    } else {
        match run.issue.as_ref() {
            Some(issue) => format!(
                "This run holds issue #{issue} {}. Estigia verifies that claim before \
                 repository writes and at every irreversible boundary; a write refused here is \
                 the tracker answering, not a suggestion.",
                state_clause(run.state.as_deref())
            ),
            None if let Some(requirement) = super::issue_requirement::in_force(context) => {
                unclaimed_hold(context, requirement)
            }
            None if super::unclaimed::applies(context, &context.repo_dir) => {
                let way_in = if context.workflow == Some(crate::config::Workflow::Assisted) {
                    "take a local claim with `assist` in a worktree, which gives the isolated \
                     checkout to commit and push from"
                } else {
                    "claim an issue, then work through `claim` with `branch` and `base`, which \
                     gives the isolated checkout to commit in"
                };
                format!(
                    "This run holds no issue. The push guard is installed in this checkout, so \
                     Estigia refuses every commit and every push until one is claimed: {way_in}. \
                     Reads, edits and tests are not gated before then."
                )
            }
            None => {
                let way_in = if context.workflow == Some(crate::config::Workflow::Assisted) {
                    "taken with `assist` in a worktree"
                } else {
                    "claimed"
                };
                format!(
                    "This run holds no issue. Estigia gates nothing until one is {way_in} — the \
                     oath binds once sworn."
                )
            }
        }
    };

    // The base checkout stays read-only by convention; Estigia measures writes
    // in both and refuses neither, but says so once, in the message the agent
    // reads before it touches anything.
    let isolation = match run.worktree.as_ref() {
        Some(worktree) => format!(
            "\n\nThis run's isolated checkout is `{}`. The base checkout stays read-only: one issue, \
             one branch, one worktree. Estigia measures writes in both and refuses neither, so this \
             one is yours to keep.",
            worktree.display()
        ),
        None => String::new(),
    };
    let children = several_claims_paragraph(context, run);
    let parked = parked_paragraph(run);
    // Which roles this run's delegated contexts actually ran as, from the
    // pointer. Absent when the set is empty, and that absence is deliberately
    // **not** spelled as "no contexts were delegated": a host that never sent
    // `agent_type` also leaves it empty.
    let delegated = if run.roles.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nDelegated contexts this run has already gated ran as: {}. That is what the gate \
             saw on their tool calls, not what a launch declared \u{2014} a context refused at the \
             role gate never reached it, and a host that does not name the role leaves it empty.",
            run.roles
                .iter()
                .map(|role| format!("`{role}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    // The targets this run's own context wrote inline once a delegated one had
    // already run. Floor, not a full count: a write before that first
    // delegated call is recorded as neither, and `SKILL.md`'s Direct Work
    // Delegation threshold stays stated, not adjudicated, either way.
    let inline = if run.inline_writes.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nThis run's own context wrote {} target{} inline after a delegated context had \
             already run: {}. That is a floor \u{2014} a write before this run's first delegated \
             call is recorded as neither this run's own nor a delegate's.",
            run.inline_writes.len(),
            if run.inline_writes.len() == 1 {
                ""
            } else {
                "s"
            },
            run.inline_writes
                .iter()
                .map(|target| format!("`{target}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    // A name, or the fact that there is none: `<runtime>-unknown` is a name
    // two runs share, and saying so beats handing out a name that is not one.
    if session::is_nameless(&run.run_id) {
        return json!({
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": format!(
                    "## Estigia

This session carries no identity Estigia can derive a run id from, so nothing can be sworn here: a claim needs one run to bind to, and every session in this state would share the same name. Reads go through untouched, and a write is held to `Issue required` where the repository sets one.{}{}{}{}",
                    drift_note(context),
                    build_note(context),
                    queue_paragraph(context, with_count),
                    orphan_paragraph(context)
                ),
            }
        });
    }
    let handoff = handoff_paragraph(context, run);
    json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": format!(
                "## Estigia

Run id `{}`. {held}{handoff}{isolation}{children}{parked}{delegated}{inline}{}{}{}{}",
                run.run_id,
                drift_note(context),
                build_note(context),
                queue_paragraph(context, with_count),
                orphan_paragraph(context)
            ),
        }
    })
}

fn parked_paragraph(run: &session::Run) -> String {
    if run.parked.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = run
        .parked
        .iter()
        .map(|parked| {
            format!(
                "- #{} in `{}`",
                parked.issue,
                parked.worktree.as_ref().map_or_else(
                    || "no checkout".to_owned(),
                    |path| path.display().to_string()
                )
            )
        })
        .collect();
    format!(
        "\n\nThis run has parked claims, still held, whose worktrees the gate does not cover until \
         one is taken back with `claim` and `park: true`:\n{}",
        lines.join("\n")
    )
}

fn several_claims_paragraph(context: &GateContext, run: &session::Run) -> String {
    let children = session::children(&context.state_root, &run.run_id);
    if children.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = children
        .iter()
        .map(|child| {
            format!(
                "- `{}` holds issue #{} in `{}`",
                child.run_id,
                child
                    .issue
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                child
                    .worktree
                    .as_ref()
                    .map(|worktree| worktree.display().to_string())
                    .unwrap_or_default()
            )
        })
        .collect();
    format!(
        "\n\nThis session holds several assisted claims. A call is decided under the one it lands \
         in: a write tool by its absolute path, a shell call by one `git -C <worktree> …` or the \
         directory it runs in; anything else by this run's own policy. Publish with each claim's \
         own run id.\n{}",
        lines.join("\n")
    )
}

/// The issues another run handed to this runtime in this repository.
///
/// A handoff is a request, not a claim: the store records it so a session that
/// starts later can see it, and taking one still goes through `claim`. A store
/// that exists and cannot be read says so, because silence would be read as
/// nothing having been handed over.
fn handoff_paragraph(context: &GateContext, run: &session::Run) -> String {
    let session::RunIdShape::Named { runtime } = session::run_id_shape(&run.run_id) else {
        return String::new();
    };
    let held = match store::open_for_read(&store::beside(&context.state_root)) {
        Ok(Some(opened)) => opened.handoffs(),
        Ok(None) => return String::new(),
        Err(failure) => {
            return format!(
                "\n\nThe handoffs recorded for `{runtime}` in this repository could not be read, \
                 so whether an issue waits for this session is unknown: {}",
                failure.detail()
            );
        }
    };
    let Ok(rows) = held else {
        return format!(
            "\n\nThe handoffs recorded for `{runtime}` in this repository could not be read, so \
             whether an issue waits for this session is unknown"
        );
    };
    let waiting: Vec<&store::HandoffRow> = rows
        .iter()
        .filter(|row| {
            row.runtime == runtime
                && crate::paths::same_directory(
                    std::path::Path::new(&row.repo_dir),
                    &context.repo_dir,
                )
        })
        .collect();
    if waiting.is_empty() {
        return String::new();
    }
    let named = waiting
        .iter()
        .map(|row| {
            let by = row
                .from_run_id
                .as_deref()
                .map_or_else(|| "a person".to_owned(), |run| format!("`{run}`"));
            let label = if row.projection == "landed" {
                ""
            } else {
                ", its `dev:` label queued and not yet read back on the tracker"
            };
            format!("#{} by {by} at {}{label}", row.issue, row.created_at)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "\n\nIssues handed to `{runtime}` in this repository: {named}. A handoff is a request, \
         never a claim: take one through `claim` when it is yours."
    )
}

/// The session a new one was started from, as the host reported it.
pub struct StartedFrom<'a> {
    /// Why the new session began: `fork` and `new` are the two that leave the
    /// person looking at a session that holds nothing.
    pub reason: &'a str,
    /// The run the previous session was.
    pub run_id: String,
}

/// The text `SessionStart` injects, for a host that has no hook to carry it.
///
/// Built from the same body as [`session_start_response`], so the two never
/// describe a run differently, except that the queue note carries no count: Pi
/// serves this text as a system section, and a count that grows with every
/// queued write would rewrite the leading prompt each turn.
///
/// A session started from another one holding an issue is told so, and told the
/// one command that puts that issue down. The claim is never moved.
pub fn session_start_text(
    context: &GateContext,
    run: &session::Run,
    from: Option<&StartedFrom<'_>>,
) -> String {
    let mut text =
        session_start_body(context, run, false)["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
    if let Some(from) = from
        && matches!(from.reason, "fork" | "new")
        && !run.unreadable
        && run.issue.is_none()
        && from.run_id != run.run_id
    {
        let previous = session::load(&context.state_root, &from.run_id);
        if !previous.unreadable
            && let Some(issue) = previous.issue.as_ref()
        {
            text.push_str(&format!(
                "\n\nThe session this one was started from (`{}`) holds issue #{issue}, and this \
                 session holds nothing. Estigia never moves a claim between sessions: \
                 `estigia release --run-id {}` puts that issue down, after which this session \
                 can claim it.",
                from.run_id, from.run_id
            ));
        }
    }
    text
}

/// What to say when the installed contract is not this binary's copy. Said at
/// `SessionStart` because that is the one message the agent is guaranteed to
/// read.
fn drift_note(context: &GateContext) -> String {
    match crate::skill::presence_of(&context.skill_root) {
        crate::skill::Presence::Current => String::new(),
        crate::skill::Presence::Stale => format!(
            "\n\nThe installed contract in `{}` is not this binary's copy, so what you read and \
             what the gate enforces may differ. Run `estigia sync`.",
            context.skill_root.display()
        ),
        // `sync` is the wrong command here and refuses: the contract is this
        // binary's copy, carrying one configured value it does not recognise.
        // The gate falls back to its defaults meanwhile, which is the part that
        // has to be said out loud — `doctor` names the row.
        crate::skill::Presence::Unreadable => format!(
            "\n\nThe operator table in `{}` carries a value this build does not recognise, so the \
             gate is running on its defaults rather than on what is written there. Run `estigia \
             doctor` for the row.",
            context.skill_root.display()
        ),
        crate::skill::Presence::Absent => format!(
            "\n\nNo contract is installed in `{}`. Run `estigia setup --all`.",
            context.skill_root.display()
        ),
    }
}

/// What to say about the binary answering this session against the
/// checkout it runs in, beside [`drift_note`] and for the same reason. Silent
/// where the build's commit is no object of that repository: no project but
/// Estigia's holds it, and `doctor`'s `gate-version` row keeps the explanation.
fn build_note(context: &GateContext) -> String {
    build_note_for(
        &context.repo_dir,
        crate::build_info::commit(),
        crate::build_info::dirty(),
    )
}

fn build_note_for(repo_dir: &std::path::Path, commit: Option<&str>, dirty: bool) -> String {
    if super::doctor::commit_is_foreign_to(repo_dir, commit, dirty) {
        return String::new();
    }
    render_build_note(&super::doctor::gate_version_for(repo_dir, commit, dirty).health)
}

/// The pure half of [`build_note`]: a current build says nothing, so this
/// stays silent for [`super::doctor::Health::Fine`] and speaks for
/// [`super::doctor::Health::Broken`] and [`super::doctor::Health::Skipped`]
/// alike — never a claim of currency for a verdict that could not be read.
fn render_build_note(health: &super::doctor::Health) -> String {
    match health {
        super::doctor::Health::Fine { .. } => String::new(),
        super::doctor::Health::Broken { detail, .. } => {
            format!("\n\nThe gate answering this session is not established as current: {detail}")
        }
        super::doctor::Health::Skipped { detail } => format!(
            "\n\nWhether the gate answering this session is current cannot be told: {detail}"
        ),
    }
}

/// Runs one lifecycle event and returns the JSON to print.
///
/// `context` is `None` when the harness is not installed, which is not an
/// error: somebody who ran `estigia setup --skill-only`, or who removed the
/// skill and left the hook, gets a hook that stands aside rather than one that
/// blocks every edit until they notice.
pub fn run(event: Event, input: &Input, context: Option<&GateContext>) -> Value {
    run_in(Dialect::ClaudeCode, event, input, context)
}

/// The same, answering in one agent's dialect.
pub fn run_in(
    dialect: Dialect,
    event: Event,
    input: &Input,
    context: Option<&GateContext>,
) -> Value {
    run_as(dialect, None, event, input, context)
}

/// The same again, knowing which agent registered the hook.
///
/// A dialect is a protocol shape and an agent is a program, and the ledger used
/// to record the first under the name of the second. `None` when the settings
/// file was written by a build that did not send one — the record then says
/// nothing rather than naming the dialect and calling it an agent.
///
/// The agent also decides the runtime word of the run id, through
/// [`session::runtime_of_agent`].
pub fn run_as(
    dialect: Dialect,
    agent: Option<&str>,
    event: Event,
    input: &Input,
    context: Option<&GateContext>,
) -> Value {
    let host = agent;
    let run_id = session::run_id(session::runtime_of_agent(agent), &input.session_id);
    let Some(context) = context else {
        return json!({});
    };

    // `note` records decisions and this is the absence of one, so it would drop
    // this on the floor — which is exactly how the silence stayed invisible.
    if let Some((verdict, detail)) = if !event.decides() {
        // Still a fault, and still recorded — with no payload there is no
        // session id, so the run identity falls back to the pointer every
        // other unidentified session on this machine shares. That is a
        // different thing from a write nobody adjudicated, and `silence`
        // counts the other two by name, so spelling it apart is what takes it
        // out of a count it was never part of.
        (input.unreadable || input.absent).then(|| {
            (
                "identity-unminted",
                format!(
                    "the {} hook arrived with no readable payload, so this session shares the \
                     unidentified run pointer instead of holding its own",
                    event.slug()
                ),
            )
        })
    } else if input.unreadable {
        Some((
            "payload-unreadable",
            // The diagnosis when there is one. A payload can only be unreadable
            // by way of `read_input`, which always writes one — the fallback is
            // for a caller that set the flag by hand, and says exactly as much
            // as the line said before.
            input.why.clone().unwrap_or_else(|| {
                "a payload arrived and could not be parsed, so this call was not gated".to_owned()
            }),
        ))
    } else if input.absent {
        Some((
            "payload-absent",
            "no payload arrived, so this call was not gated \u{2014} the hook is registered \
             without one being sent"
                .to_owned(),
        ))
    } else if input.tool_name.trim().is_empty() && !matches!(event, Event::Stop) {
        // A payload that parses and names nothing is exactly as unusable as one
        // that does not parse. Rule 3 is untouched: this still does not deny.
        Some((
            "tool-unnamed",
            "a payload arrived, parsed, and named no tool, so this call was not gated \u{2014} \
             the agent may be sending a shape this build does not read"
                .to_owned(),
        ))
    } else {
        None
    } {
        session::record(
            &context.state_root,
            &json!({
                "at": session::now_seconds(),
                "run_id": run_id,
                "tool": "",
                // Which agent, and which hook — only the agent when one was
                // sent; the dialect goes down beside it under its own name.
                "agent": agent.unwrap_or_default(),
                "dialect": dialect.slug(),
                "event": event.slug(),
                "verdict": verdict,
                "detail": detail,
            }),
        );
    }

    match event {
        // Git's, not an agent's: the caller runs the gate itself and turns the
        // decision into an exit code. Reaching here would mean it did not.
        Event::PrePush => json!({}),
        Event::PostToolUse => json!({}),
        Event::SessionStart => {
            session::maintain(&context.state_root);
            if !input.session_id.trim().is_empty() {
                super::registry::record_start(
                    &context.state_root,
                    session::runtime_of_agent(host),
                    &input.session_id,
                    &context.repo_dir,
                    None,
                );
            }
            let run = session::load(&context.state_root, &run_id);
            session_start_response(context, &run)
        }
        Event::SessionEnd => {
            // The pointer goes; the claim does not. But only when somebody said
            // whose session this was: `<runtime>-unknown` is a name two runs
            // share rather than a run, and deleting it would be a destructive
            // act on an unknown identity.
            if !input.session_id.trim().is_empty() {
                super::registry::record_end(
                    &context.state_root,
                    session::runtime_of_agent(host),
                    &input.session_id,
                    None,
                );
                for child in session::children(&context.state_root, &run_id) {
                    session::forget(&context.state_root, &child.run_id);
                }
                session::forget(&context.state_root, &run_id);
            }
            json!({})
        }
        Event::Stop => {
            if input.unreadable || input.absent {
                return json!({});
            }
            if input.stop_hook_active {
                return with_decisions(context, &run_id, json!({}));
            }
            let answer = 'stop: {
                let mut run = session::load(&context.state_root, &run_id);
                let Some(issue) = run.issue.clone() else {
                    break 'stop json!({});
                };
                let live = if run.assisted {
                    false
                } else if run.within_window(context.window) {
                    true
                } else if super::tracker_answer_for_pointer(
                    &context.skill_root,
                    &context.repo_dir,
                    &context.tracker,
                    &run,
                )
                .is_some_and(|answer| answer.standing() == super::tracker::Standing::Passed)
                {
                    run.mark_verified();
                    let _ = session::store(&context.state_root, &run);
                    true
                } else {
                    false
                };
                if live {
                    if background_work_outstanding(&input.transcript_path)
                        || waiting_on_a_person(context, &run)
                    {
                        json!({})
                    } else {
                        stop_block(&issue, &run_id)
                    }
                } else {
                    json!({})
                }
            };
            with_decisions(context, &run_id, with_queue_note(context, answer))
        }
        Event::PreToolUse => {
            // Before the role road, which decides a call carrying an `agent_type` and
            // would otherwise answer it, through a stand-down, without this rule asked.
            let (action, how, command_observed) =
                classify_observing(&input.tool_name, &input.tool_input, &context.boundaries);
            if let Some(denied) = super::person_only::deny(&action) {
                note(
                    context,
                    &run_id,
                    &input.tool_name,
                    action.subject().as_deref(),
                    &denied,
                    &[],
                );
                return response_in(dialect, &denied);
            }
            if super::decision_mode::QUESTION_TOOLS.contains(&input.tool_name.as_str()) {
                let Some(decided) = super::decision_mode::refuse(
                    context,
                    &run_id,
                    &input.tool_name,
                    &input.tool_input,
                ) else {
                    return dialect.aside();
                };
                note(context, &run_id, &input.tool_name, None, &decided, &[]);
                return response_in(dialect, &decided);
            }
            // A reserved reviewer is proved before either role or repository
            // classification. `agent_type` names the caller; only the nested
            // `tool_input.subagent_type` names what this call would launch.
            if is_prelaunch_tool(agent, &input.tool_name)
                && let Some(target) = input.launch_target()
                && super::roles::reserved_reviewer(target).is_some()
                && let Err(refusal) = super::roles::authorize_review_blind_launch(
                    &context.repo_dir,
                    crate::paths::home_dir().ok().as_deref(),
                    target,
                )
            {
                return dialect.deny(&refusal);
            }
            // Before anything else: a sub-agent reaching past the tool list its
            // own definition declares. Checked first because it is the cheapest
            // question and the least conditional — it does not depend on a
            // claim, a state, or a window, only on what the author wrote.
            if let Some(agent) = input.agent_type.as_deref() {
                if let Some((seal, template)) = super::roles::reserved_reviewer_row(agent) {
                    // Rendered with the effective row, exactly as the other door
                    // does: handing this the raw bytes makes `declared_policy`
                    // read an allowlist naming one tool called `{{TOOLS}}`.
                    let reviewer = crate::setup::render_reviewer_agent(
                        template,
                        context.evidence,
                        seal,
                        agent,
                    );
                    if let Some(refusal) =
                        super::roles::gate(Some(agent), &input.tool_name, Some(&reviewer))
                    {
                        return dialect.deny(&refusal);
                    }
                } else {
                    match super::roles::definition_for(
                        &context.repo_dir,
                        crate::paths::home_dir().ok().as_deref(),
                        agent,
                    ) {
                        // A definition that is there and will not open is denied
                        // rather than stepped over: stepping over it produces
                        // *no policy*, which this gate reads as every tool
                        // allowed. Through the stand-down, unlike the role
                        // refusal below — that one enforces a list the
                        // sub-agent's *author* wrote, and a stand-down does not
                        // grant permissions somebody else withheld.
                        Err(refusal) => {
                            return response_in(
                                dialect,
                                &super::standdown::over(
                                    Decision::Deny(Box::new(refusal)),
                                    context.stand_down.as_ref(),
                                    session::now_seconds(),
                                ),
                            );
                        }
                        Ok(Some(definition)) => {
                            if let Some(refusal) =
                                super::roles::gate(Some(agent), &input.tool_name, Some(&definition))
                            {
                                return dialect.deny(&refusal);
                            }
                        }
                        Ok(None) => {}
                    }
                }
            }

            // This dialect resolves a call's checkout from `input.cwd` alone
            // (see `hook` in `cli::mod`, which lifts it before `gate_context`
            // ever runs) — no clamped `workdir` or write-target candidate is
            // read here the way `estigia gate` reads one, so there is nothing
            // to drop.
            let mut observed = Observed {
                command: Some(command_observed),
                checkout: Some(CheckoutObserved {
                    adjudicated: context.repo_dir.display().to_string(),
                    source: if input.cwd.trim().is_empty() {
                        CheckoutSource::LaunchDirectory
                    } else {
                        CheckoutSource::PayloadCwd
                    },
                    dropped: Vec::new(),
                }),
                push_refs: Vec::new(),
            };
            let mut run = session::load(&context.state_root, &run_id);
            let mut run_id = run_id;
            let landed = (dialect == Dialect::ClaudeCode
                && run.issue.is_none()
                && !run.unreadable
                && !input.session_id.trim().is_empty())
            .then(|| {
                let literal = match observed.command.as_ref().map(|command| &command.literal) {
                    Some(crate::outcome::Established::Known(text)) => Some(text.as_str()),
                    _ => None,
                };
                super::assisted::landed_child(
                    &context.state_root,
                    &run_id,
                    &context.repo_dir,
                    &action,
                    literal,
                )
            })
            .flatten();
            let mut narrowed = None;
            if let Some(landed) = landed {
                run = landed.run;
                run_id = run.run_id.clone();
                if let Some(runs_in) = landed.runs_in {
                    if let Some(checkout) = observed.checkout.as_mut() {
                        checkout.adjudicated = runs_in.display().to_string();
                    }
                    narrowed = Some(GateContext {
                        repo_dir: runs_in,
                        ..context.clone()
                    });
                }
            }
            let context = narrowed.as_ref().unwrap_or(context);
            let declared_launch = is_prelaunch_tool(agent, &input.tool_name)
                .then(|| {
                    input
                        .launch_target()
                        .map(|target| (target, input.launch_model(), input.launch_effort()))
                })
                .flatten()
                .filter(|(_, model, effort)| model.is_some() || effort.is_some());
            if let Some((target, model, effort)) = declared_launch
                && run.roles.insert(target.to_owned())
            {
                let mut entry = json!({
                    "at": session::now_seconds(),
                    "run_id": run_id,
                    "agent_type": target,
                    "repo_dir": context.repo_dir.display().to_string(),
                });
                if let Some(model) = model {
                    entry["model"] = json!(model);
                }
                if let Some(effort) = effort {
                    entry["effort"] = json!(effort);
                }
                if let Some(issue) = run.issue.as_ref() {
                    entry["issue"] = json!(issue);
                }
                session::record_role(&context.state_root, &entry);
                let _ = session::store(&context.state_root, &run);
            }
            // The role a delegated context ran as, recorded for the length of
            // this run. Here rather than beside the role gate above, and that
            // placement is the semantics: a context refused at the role gate
            // returns before it and leaves no trace.
            // Absent, not present-and-blank: a blank `agent_type` is not proof
            // of this run's own context, so only a missing field is attributed.
            let agent_absent = input.agent_type.is_none();
            let agent = input
                .agent_type
                .as_deref()
                .map(str::trim)
                .filter(|agent| !agent.is_empty());
            let saw_new_role = agent.is_some_and(|agent| run.roles.insert(agent.to_owned()));
            // Written before the decision, so an `Outside` call — what a
            // delegated context's reads produce — reaches it though `note` drops it.
            if let Some(role) = agent.filter(|_| saw_new_role) {
                let mut entry = json!({
                    "at": session::now_seconds(),
                    "run_id": run_id,
                    "agent_type": role,
                    "repo_dir": context.repo_dir.display().to_string(),
                });
                if let Some(issue) = run.issue.as_ref() {
                    entry["issue"] = json!(issue);
                }
                session::record_role(&context.state_root, &entry);
            }
            let (decision, recorded) = if input.session_id.trim().is_empty() {
                // No session to mint a run id from — the same position a git
                // hook is in, and the same answer: ask the checkout.
                let literal = match observed.command.as_ref().map(|command| &command.literal) {
                    Some(crate::outcome::Established::Known(text)) => Some(text.as_str()),
                    _ => None,
                };
                let same_tier_fragments = observed
                    .command
                    .as_ref()
                    .map(|command| command.same_tier_fragments.as_slice())
                    .unwrap_or_default();
                let write_targets = observed
                    .command
                    .as_ref()
                    .map(|command| command.write_targets.as_slice())
                    .unwrap_or_default();
                let readings = super::guard::Readings {
                    literal,
                    same_tier_fragments,
                    write_targets,
                };
                let adjudication = super::guard::adjudicate_command(
                    context,
                    &context.repo_dir,
                    &action,
                    how,
                    &[],
                    readings,
                );
                (
                    adjudication.decision,
                    adjudication.holder.unwrap_or_default(),
                )
            } else {
                (
                    super::gate_observing(context, &mut run, &action, how, &observed),
                    run_id,
                )
            };
            let decision = if can_ask(dialect, host) {
                decision
            } else {
                decision.without_ask()
            };
            let subject = action.subject();
            // A write from this run's own context: `agent_type` absent on a
            // call the gate allowed, after `roles` had already gained a role —
            // the same absence `session_start_response` reads as orchestrating
            // rather than unreported. A write before that first delegated call
            // leaves `roles` empty and is recorded as neither.
            let saw_new_write = agent_absent
                && !run.roles.is_empty()
                && matches!(decision, Decision::Allow(_))
                && matches!(action, Action::Write { .. })
                && subject
                    .as_ref()
                    .is_some_and(|target| run.inline_writes.insert(target.clone()));
            // `saw_new_role` and not only `Allow`, and the difference is the
            // point: the role record is about what the gate **saw**, and a
            // delegated context reading files makes calls this gate answers
            // `Outside` rather than `Allow`. Still best effort, and still never
            // a denial.
            if saw_new_role || saw_new_write || matches!(decision, Decision::Allow(_)) {
                let _ = session::store(&context.state_root, &run);
            }
            note(
                context,
                &recorded,
                &input.tool_name,
                subject.as_deref(),
                &decision,
                &[],
            );
            let answer = response_in(dialect, &decision);
            let Some(recipient) = mail_recipient(dialect, host, input) else {
                return answer;
            };
            with_mail(context, dialect, &recipient, answer)
        }
    }
}

/// The run whose mail a hook may deliver: the main conversation of a named
/// session, in the one dialect that carries mail. `None` for a sub-agent's
/// call or a payload that names no session.
pub fn mail_recipient(dialect: Dialect, agent: Option<&str>, input: &Input) -> Option<String> {
    (dialect == Dialect::ClaudeCode
        && input.agent_type.is_none()
        && !input.session_id.trim().is_empty())
    .then(|| session::run_id(session::runtime_of_agent(agent), &input.session_id))
}

fn with_mail(context: &GateContext, dialect: Dialect, run_id: &str, mut answer: Value) -> Value {
    let decided = answer["hookSpecificOutput"]["permissionDecision"].as_str();
    if dialect != Dialect::ClaudeCode || matches!(decided, Some("deny" | "ask")) {
        return answer;
    }
    let Some(text) = super::mailbox::deliver(&context.state_root, run_id) else {
        return answer;
    };
    answer["hookSpecificOutput"]["hookEventName"] = json!("PreToolUse");
    answer["hookSpecificOutput"]["additionalContext"] = json!(text);
    answer
}

/// The JSON a `Stop` hook returns to keep the turn going.
pub fn stop_block(issue: &crate::identity::Identity, run_id: &str) -> Value {
    json!({
        "decision": "block",
        "reason": super::loop_text::held(&issue.to_string(), run_id),
    })
}

fn waiting_on_a_person(context: &GateContext, run: &session::Run) -> bool {
    if run.state.as_deref() == Some("blocked") {
        return true;
    }
    let Some(issue) = run.issue_number() else {
        return false;
    };
    let transport = crate::transport::Context::live(
        context.skill_root.clone(),
        context.repo_dir.clone(),
        context.tracker.named_repo(),
    );
    let Ok(repo) = crate::transport::claim::repo_of(&transport) else {
        return false;
    };
    let Ok(path) = crate::transport::store::store_path() else {
        return false;
    };
    let Ok(comments) = crate::transport::store::Ledger::read_timeline_at(&path, &repo, issue)
    else {
        return false;
    };
    let newest = comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .flat_map(|comment| crate::transport::markers::parse(&comment.body))
        .rfind(|marker| match marker.get("kind").map(String::as_str) {
            Some("published") => true,
            Some("review-receipt") => marker.contains_key("terminal-state"),
            _ => false,
        });
    newest.is_some_and(|marker| {
        marker.get("kind").map(String::as_str) == Some("review-receipt")
            && marker.get("terminal-state").map(String::as_str) == Some("escalated")
    })
}

/// Whether a transcript names a background launch that has not reported yet.
///
/// A run waiting on a background command or subagent has nothing to do until
/// the host resumes it, and blocking the stop buys a turn that is usually spent
/// polling — a turn whose context every later turn then pays for again. What
/// counts as a launch is the host's own record that it started — never the text
/// of a result, which ordinary output can quote. A `toolUseResult` carrying a
/// non-empty `backgroundTaskId` is the host's notice for a background command,
/// and one whose `status` is `async_launched` is how Claude Code 2.1.x reports
/// a background subagent, whose tool_use carries no flag at all. A call the
/// host refused or never answered reports neither and is not a launch, so a run
/// whose launch never ran keeps the block. A launch is reported when a later
/// record carries a `<task-notification>` naming its `<tool-use-id>`; a
/// sidechain record belongs to a sub-agent's own conversation and never answers
/// for the main one.
///
/// An absent or unreadable transcript answers `false` — the absence of evidence
/// is not evidence of a launch — so such a run is kept going exactly as it was
/// before this scan existed. A line that is not JSON is skipped; a read that
/// stops partway answers `false` for the same reason an absent file does.
pub(crate) fn background_work_outstanding(transcript: &str) -> bool {
    if transcript.trim().is_empty() {
        return false;
    }
    let Ok(file) = std::fs::File::open(transcript) else {
        return false;
    };
    let mut launches: Vec<String> = Vec::new();
    let mut reported: Vec<String> = Vec::new();
    let mut read_to_the_end = true;
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else {
            // A read that stopped partway cannot tell an answered launch from an
            // unanswered one, and answering `true` there would release the turn
            // on a launch this scan never finished reading. The caller's
            // conservative answer is the block.
            read_to_the_end = false;
            break;
        };
        // Cheap gates first: a transcript is large, and most lines carry
        // neither a launch nor a notification.
        if line.contains("backgroundTaskId") || line.contains("async_launched") {
            let record: Value = match serde_json::from_str(&line) {
                Ok(record) => record,
                Err(_) => continue,
            };
            if record
                .get("isSidechain")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                continue;
            }
            // The host's own record of the start, never the text of a result:
            // ordinary output can quote the notice a launch writes, and a
            // quoted copy must not become a launch that never reports.
            let started = record.get("toolUseResult").is_some_and(|result| {
                result
                    .get("backgroundTaskId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| !id.trim().is_empty())
                    || result.get("status").and_then(Value::as_str) == Some("async_launched")
            });
            if !started {
                continue;
            }
            let blocks = record
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(Value::as_array);
            for block in blocks.into_iter().flatten() {
                if block.get("type").and_then(Value::as_str) == Some("tool_result")
                    && let Some(id) = block.get("tool_use_id").and_then(Value::as_str)
                {
                    launches.push(id.to_owned());
                }
            }
        }
        // A report is a JSON record of this conversation and nothing else: a
        // sidechain record answers for a sub-agent, and a line that is not a
        // record is not a notification.
        if line.contains("<task-notification>")
            && let Ok(record) = serde_json::from_str::<Value>(&line)
            && !record
                .get("isSidechain")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            for piece in line.split("<tool-use-id>").skip(1) {
                if let Some((id, _)) = piece.split_once("</tool-use-id>")
                    && launches.iter().any(|launch| launch == id)
                {
                    reported.push(id.to_owned());
                }
            }
        }
    }
    if !read_to_the_end {
        return false;
    }
    launches
        .iter()
        .any(|launch| !reported.iter().any(|id| id == launch))
}

/// The renewal window a gate context uses when nothing overrides it.
pub fn default_window() -> std::time::Duration {
    RENEWAL_WINDOW
}

/// Whether this event's decision can stop what is about to happen.
pub fn can_deny(event: Event) -> bool {
    matches!(event, Event::PreToolUse | Event::PrePush)
}

/// Whether this event is git's rather than an agent's.
///
/// The two answer differently: an agent event returns JSON describing a
/// decision, and a git hook returns an **exit code**. Confusing them would make
/// a refusal print itself and let the push through.
pub fn is_git(event: Event) -> bool {
    matches!(event, Event::PrePush)
}

#[cfg(test)]
mod tests;
