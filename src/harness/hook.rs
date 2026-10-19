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

/// The lifecycle events Estigia registers for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Before a tool runs. The one that can say no.
    PreToolUse,
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
    pub fn all() -> [Self; 5] {
        [
            Self::PreToolUse,
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

/// Every event Estigia registers **in an agent**, with the matcher each needs.
///
/// `PrePush` is deliberately absent: it belongs to git, not to an agent, and it
/// is installed by `estigia guard` into a repository rather than by `setup`
/// into a settings file.
///
/// `PreToolUse` matches only the tools the gate can act on. A matcher of `*`
/// would wake this process for every `Read` and every `Grep`, which is a cost
/// paid thousands of times to answer "not mine".
pub const EVENTS: &[(Event, Option<&str>, u64)] = &[
    (Event::PreToolUse, None, 10),
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
                    "permissionDecisionReason": reason_text("estigia asks before this write.", refusal),
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

/// The dirty paths of a checkout an unclaimed run under `ask` may have written without the gate
/// being able to place the write. Empty for a claimed run, another requirement, or a clean tree.
fn unclaimed_dirt_note(context: &GateContext, run: &session::Run) -> String {
    if run.issue.is_some()
        || super::issue_requirement::in_force(context) != Some(crate::config::IssueRequirement::Ask)
    {
        return String::new();
    }
    let Some(answer) = super::proof_git_command(
        &context.repo_dir,
        &["status", "--porcelain=v1", "--untracked-files=normal"],
    )
    .output()
    .ok()
    .filter(|answer| answer.status.success()) else {
        return String::new();
    };
    let listing = String::from_utf8_lossy(&answer.stdout);
    let paths: Vec<&str> = listing
        .lines()
        .filter_map(|line| line.get(3..))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .collect();
    if paths.is_empty() {
        return String::new();
    }
    let shown = paths.len().min(8);
    let named = paths[..shown]
        .iter()
        .map(|path| format!("`{path}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let more = if paths.len() > shown {
        format!(", and {} more", paths.len() - shown)
    } else {
        String::new()
    };
    format!(
        "\n\nThis checkout has uncommitted changes: {named}{more}. A run holding no issue under \
         `ask` is allowed a write the gate cannot place, so one of these may be its residue; \
         inspect them before claiming."
    )
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
    let until = super::operation_window::permitted_period(window)
        .map_or_else(String::new, |period| format!(" ({period})"));
    Some(format!(
        "{writes} queued; nothing reaches GitHub until the git operation window opens{until}. \
         Claims, comments and transitions keep working against the local store and `estigia \
         flush` sends them in order."
    ))
}

fn queue_paragraph(context: &GateContext, with_count: bool) -> String {
    queue_note(context, with_count).map_or_else(String::new, |note| format!("\n\n{note}"))
}

fn orphan_paragraph(context: &GateContext) -> String {
    let transport =
        crate::transport::Context::live(context.skill_root.clone(), context.repo_dir.clone(), None);
    let holdings = session::holdings(&context.state_root);
    super::changes::orphan_notice(&transport, &holdings)
        .map_or_else(String::new, |note| format!("\n\n{note}"))
}

/// The context injected when a session begins.
pub fn session_start_response(context: &GateContext, run: &session::Run) -> Value {
    session_start_body(context, run, true)
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
                let way_in = if context.workflow == Some(crate::config::Workflow::Assisted) {
                    "Take a local claim with `assist` in a worktree first, which gives the \
                     isolated checkout to write in."
                } else {
                    "Claim an issue first, filing one with `create` when none covers the \
                     change, then work through `start_branch`."
                };
                let held = match requirement {
                    crate::config::IssueRequirement::Required => {
                        "Estigia refuses every repository write until one is claimed"
                    }
                    _ => {
                        "each repository write is put to the person until one is claimed, \
                          and refused on a host that cannot ask"
                    }
                };
                match super::issue_requirement::scope_of(&context.repo_dir) {
                    super::issue_requirement::Scope::Repository(_) => format!(
                        "This run holds no issue, and `Issue requirement` is `{}` in this \
                         repository: {held}. {way_in} Reads are not gated.",
                        requirement.as_value()
                    ),
                    super::issue_requirement::Scope::Outside => format!(
                        "This run holds no issue. This directory is not inside a repository Estigia \
                         can place, so no issue requirement applies to a write that lands here: \
                         `Issue requirement` is `{}` for a write that lands in a repository Estigia \
                         can place, and there {held}. Reads are not gated.",
                        requirement.as_value()
                    ),
                    super::issue_requirement::Scope::Unknown => format!(
                        "This run holds no issue. `Issue requirement` is `{}` here, but whether \
                         this directory is inside a repository could not be established, so every \
                         write from it is held rather than exempted: {held}. Reads are not gated.",
                        requirement.as_value()
                    ),
                }
            }
            None if super::unclaimed::applies(context, &context.repo_dir) => {
                let way_in = if context.workflow == Some(crate::config::Workflow::Assisted) {
                    "take a local claim with `assist` in a worktree, which gives the isolated \
                     checkout to commit and push from"
                } else {
                    "claim an issue, then work through `start_branch`, which gives the isolated \
                     checkout to commit in"
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
    let dirt = unclaimed_dirt_note(context, run);
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

This session carries no identity Estigia can derive a run id from, so nothing can be sworn here: a claim needs one run to bind to, and every session in this state would share the same name. Reads go through untouched, and a write is held to `Issue requirement` where the repository sets one.{}{}{}{}",
                    drift_note(context),
                    build_note(context),
                    queue_paragraph(context, with_count),
                    orphan_paragraph(context)
                ),
            }
        });
    }
    json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": format!(
                "## Estigia

Run id `{}`. {held}{isolation}{dirt}{delegated}{inline}{}{}{}{}",
                run.run_id,
                drift_note(context),
                build_note(context),
                queue_paragraph(context, with_count),
                orphan_paragraph(context)
            ),
        }
    })
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
        Event::SessionStart => {
            session::maintain(&context.state_root);
            if !input.session_id.trim().is_empty() {
                super::registry::record_start(
                    &context.state_root,
                    session::runtime_of_agent(host),
                    &input.session_id,
                    &context.repo_dir,
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
                session::forget(&context.state_root, &run_id);
            }
            json!({})
        }
        Event::Stop => {
            if input.unreadable || input.absent || input.stop_hook_active {
                return json!({});
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
            with_queue_note(context, answer)
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
            let observed = Observed {
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
            if mail_recipient(dialect, host, input).is_some() {
                return with_mail(context, dialect, &recorded, answer);
            }
            answer
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
mod tests {
    use super::*;
    use crate::outcome::{MutationOutcome, Replayability, Resolution};

    /// Every decision the store holds for a state root, one JSON object per
    /// line, so a test can read it the way it read the ledger file before.
    fn decisions(state_root: &std::path::Path) -> String {
        crate::harness::session::decision_records(state_root)
            .iter()
            .map(|entry| serde_json::to_string(entry).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Forgets every decision and role, so a test that poses two worlds gets a
    /// clean one. These tests do not use claims or timelines.
    fn clear_ledger(state_root: &std::path::Path) {
        let path = crate::transport::store::beside(state_root);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn an_ask_does_not_say_the_write_was_refused() {
        let refusal =
            Refusal::not_started("a-code", "a message", Resolution::run("estigia status"));
        let ask = response_in(
            Dialect::ClaudeCode,
            &Decision::Ask(Box::new(refusal.clone())),
        );
        let deny = response_in(Dialect::ClaudeCode, &Decision::Deny(Box::new(refusal)));
        let reason = |value: &Value| {
            value["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .expect("a reason")
                .to_owned()
        };
        assert!(reason(&ask).starts_with("estigia asks before this write."));
        assert!(reason(&deny).starts_with("estigia refused this write."));
    }

    #[test]
    fn a_session_hook_with_no_payload_is_not_an_ungated_call() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");

        let verdicts = |event| {
            clear_ledger(&context.state_root);
            let _ = run_in(
                Dialect::ClaudeCode,
                event,
                &Input {
                    absent: true,
                    ..Input::default()
                },
                Some(&context),
            );
            decisions(&context.state_root)
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .filter_map(|entry| {
                    entry
                        .get("verdict")
                        .and_then(|value| value.as_str())
                        .map(ToOwned::to_owned)
                })
                .collect::<Vec<_>>()
        };

        // The floor: the one event that can say no still records the silence.
        // Without this, a fix that stopped recording anything would pass.
        assert_eq!(
            verdicts(Event::PreToolUse),
            vec!["payload-absent".to_owned()],
            "a write the gate could not decide on is no longer recorded as one"
        );

        for event in [Event::SessionStart, Event::SessionEnd] {
            let recorded = verdicts(event);
            assert_eq!(recorded.len(), 1, "{event:?} recorded nothing at all");
            assert_ne!(
                recorded[0], "payload-absent",
                "{event:?} decides nothing, and its missing payload is being counted as a write \
                 that went through ungated"
            );
        }
    }

    #[test]
    fn an_ungated_call_records_which_agent_and_which_hook() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");

        // An agent whose dialect is **not** its own name, which is what this
        // test could not tell apart before. It asked `run_in(Dialect::GeminiCli,
        // ..)` and asserted the record said `gemini-cli` — true of the dialect
        // slug and true of the agent at once, so it passed while the code wrote
        // the dialect. Eleven agents share five dialects, and Codex speaks
        // Claude Code's.
        let line = |agent: Option<&str>| {
            clear_ledger(&context.state_root);
            let _ = run_as(
                Dialect::ClaudeCode,
                agent,
                Event::PreToolUse,
                &Input {
                    absent: true,
                    ..Input::default()
                },
                Some(&context),
            );
            let text = decisions(&context.state_root);
            serde_json::from_str::<serde_json::Value>(text.lines().next().expect("a line"))
                .expect("the line is JSON")
        };

        let record = line(Some("codex"));
        assert_eq!(
            record.get("verdict").and_then(|value| value.as_str()),
            Some("payload-absent"),
            "the floor moved: this is no longer the line under test — {record}"
        );
        assert_eq!(
            record.get("agent").and_then(|value| value.as_str()),
            Some("codex"),
            "the record names the dialect where the agent belongs: {record}"
        );
        // Beside it, under its own name: worth knowing, and not the same fact.
        assert_eq!(
            record.get("dialect").and_then(|value| value.as_str()),
            Some("claude-code"),
            "the dialect was lost: {record}"
        );
        assert_eq!(
            record.get("event").and_then(|value| value.as_str()),
            Some("pre-tool-use"),
            "the record does not say which hook it was: {record}"
        );

        // A settings file written before `--agent` existed. Saying nothing is
        // the answer: `doctor` reports that as *"from an agent no line names"*,
        // and naming the dialect there would be this defect again.
        let old = line(None);
        assert_eq!(
            old.get("agent").and_then(|value| value.as_str()),
            Some(""),
            "a line that knows no agent named one anyway: {old}"
        );
        assert_eq!(
            old.get("dialect").and_then(|value| value.as_str()),
            Some("claude-code"),
            "the dialect is what such a line does know, and it is gone: {old}"
        );
    }

    #[test]
    fn every_event_round_trips_through_its_slug() {
        for (event, _, _) in EVENTS {
            assert_eq!(Event::from_slug(event.slug()), Some(*event));
        }
    }

    #[test]
    fn a_codex_payload_with_a_null_transcript_path_is_still_readable() {
        let input = read_input(
            br#"{"session_id":"abc","cwd":"/r","hook_event_name":"Stop","stop_hook_active":false,"transcript_path":null}"#
                .as_slice(),
        );
        assert!(!input.unreadable, "{:?}", input.why);
        assert_eq!(input.session_id, "abc");
        assert_eq!(input.transcript_path, "");
    }

    #[test]
    fn a_payload_that_will_not_parse_says_so_without_denying() {
        // An empty body is the ordinary case — a hook fired with nothing to say —
        // and must stay quiet. A body that arrived and could not be read is the
        // one that has to leave a trace.
        assert!(!read_input(std::io::Cursor::new("")).unreadable);
        assert!(!read_input(std::io::Cursor::new("   \n")).unreadable);
        assert!(!read_input(std::io::Cursor::new(r#"{"session_id":"x"}"#)).unreadable);

        // The exact shape that fooled this session's own probe three times: a
        // hand-built payload whose backslash is not a JSON escape.
        let broken = read_input(std::io::Cursor::new(
            r#"{"tool_input":{"command":"del src\x.rs"}}"#,
        ));
        assert!(broken.unreadable);
        // And it is still not a denial: the command is simply unknown.
        assert!(broken.tool_name.is_empty());
    }

    #[test]
    fn an_unreadable_payload_reaches_the_ledger() {
        // The half that matters. `note` records decisions, and standing aside is
        // not one — so before this, an agent whose schema drifted took the gate
        // out of the loop for every call and left nothing anywhere to see it.
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");

        let input = read_input(std::io::Cursor::new(r#"{"command":"del src\x.rs"}"#));
        assert!(input.unreadable);
        let answer = run_in(
            Dialect::ClaudeCode,
            Event::PreToolUse,
            &input,
            Some(&context),
        );
        assert_eq!(answer, json!({}), "an unreadable payload denied a call");

        let ledger = decisions(&context.state_root);
        assert!(
            ledger.contains("payload-unreadable"),
            "standing aside left no trace: {ledger}"
        );
    }

    #[test]
    fn the_trace_of_a_call_nobody_gated_says_enough_to_act_on() {
        // A trace that only says "something was wrong" is the shape this file
        // has spent five defects on, one level up: `doctor` reports the silence
        // BROKEN and refuses to let a run swear, and the resolution it offers
        // is `[operator-knowledge] whether the agent that sent them is one this
        // build knows` — a question the payload answered and nothing kept.
        //
        // Two kinds, because the doc on `Input::absent` says the two fixes
        // differ: a schema to teach, or bytes that were never JSON.
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");
        let ledger = |body: &'static str| {
            let input = read_input(std::io::Cursor::new(body));
            assert!(input.unreadable, "{body} was expected not to parse");
            run_in(
                Dialect::ClaudeCode,
                Event::PreToolUse,
                &input,
                Some(&context),
            );
            decisions(&context.state_root)
        };

        // Not JSON: the four real lines on this session's own machine, whose
        // cause took a round to find and is one substring away in this one.
        let bytes = ledger(r#"{"tool_input":{"command":"del C:\Users\x"}}"#);
        assert!(
            bytes.contains("bytes that are not JSON at all") && bytes.contains("column"),
            "nothing in the line says where the read stopped: {bytes}"
        );

        // JSON this build does not know: the keys are the schema, and the
        // schema is the fix.
        let schema = ledger(r#"{"session_id":7,"agent_token":"sk-not-in-the-ledger"}"#);
        assert!(
            schema.contains("agent_token") && schema.contains("session_id"),
            "the keys that would identify the agent are not in the line: {schema}"
        );
        // The half that is a rule rather than a convenience: this file sits on
        // disk, and a tool call carries tokens, diffs and private branch names.
        assert!(
            !schema.contains("sk-not-in-the-ledger"),
            "a payload value reached the ledger: {schema}"
        );
    }

    #[test]
    fn the_flag_a_run_declared_is_in_the_record_of_what_it_was_allowed() {
        // `out_of_phase` accepts a named feature flag in place of a review on
        // trunk, and says plainly why that is not a proof: "Estigia cannot read
        // the code." The reason it is nonetheless reasonable is the rest of the
        // sentence — the claim becomes "explicit **and recorded**, so 'we
        // thought it was flagged' stops being something anybody can say
        // afterwards."
        //
        // It was explicit. It was not recorded: the ledger line for the allow
        // named the run and the issue and nothing about the flag, so the one
        // thing that made an unverifiable declaration acceptable did not exist.
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Trunk,
            evidence: crate::config::Evidence::Reading,
            flag: Some("ff.new-checkout".to_owned()),
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");

        note(
            &context,
            "claude-abcd1234",
            "Bash",
            Some("git push"),
            &crate::harness::Decision::Allow("issue #12 is held by claude-abcd1234".to_owned()),
            &[],
        );

        let ledger = decisions(&context.state_root);
        assert!(
            ledger.contains("ff.new-checkout"),
            "the flag the run declared is nowhere in what it was allowed: {ledger}"
        );

        // And a run that declared none says so, rather than carrying somebody
        // else's name or an empty string that reads like one.
        let quiet = GateContext {
            flag: None,
            state_root: root.path().join("quiet"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            ..context
        };
        std::fs::create_dir_all(&quiet.state_root).expect("the state directory");
        note(
            &quiet,
            "claude-abcd1234",
            "Bash",
            Some("git push"),
            &crate::harness::Decision::Allow("issue #12 is held by claude-abcd1234".to_owned()),
            &[],
        );
        let ledger = decisions(&quiet.state_root);
        assert!(
            ledger.contains("\"flag\":null"),
            "a run that named no flag should say so plainly: {ledger}"
        );
    }

    #[test]
    fn a_payload_that_never_arrives_reaches_the_ledger_too() {
        // The quieter half of the same shape. A payload that will not parse was
        // recorded; one that never came left nothing at all — and a harness
        // registered without piping its body gates every call of every session
        // and says so nowhere. Both mean *this was not gated*; they are two
        // records because the repair is different. A schema this build cannot
        // read is something to teach the classifier. A body that never arrives
        // is a registration to fix.
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");

        for body in ["", "   \n\t "] {
            let input = read_input(std::io::Cursor::new(body));
            assert!(input.absent, "{body:?} is not a payload");
            assert!(
                !input.unreadable,
                "{body:?} did not arrive, so there was nothing to fail to read"
            );
            let answer = run_in(
                Dialect::ClaudeCode,
                Event::PreToolUse,
                &input,
                Some(&context),
            );
            assert_eq!(answer, json!({}), "a missing payload denied a call");
        }

        let ledger = decisions(&context.state_root);
        assert!(
            ledger.contains("payload-absent"),
            "standing aside left no trace: {ledger}"
        );
        // Under its own name, so the two are countable apart in a ledger where
        // a session may hold both.
        assert!(
            !ledger.contains("payload-unreadable"),
            "a payload that never arrived was filed as one that would not parse"
        );
    }

    #[test]
    fn only_pre_tool_use_can_deny() {
        assert!(can_deny(Event::PreToolUse));
        assert!(!can_deny(Event::SessionStart));
        assert!(!can_deny(Event::SessionEnd));
    }

    fn stop_context(root: &std::path::Path) -> GateContext {
        GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.join("skill"),
            repo_dir: root.join("repo"),
            state_root: root.join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        }
    }

    #[test]
    fn a_stop_hook_blocks_while_its_run_holds_a_live_claim() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        std::fs::create_dir_all(&context.state_root).expect("the state directory");
        std::fs::create_dir_all(&context.repo_dir).expect("the checkout the tracker reads in");
        let session_id = "stop-live-1";
        let run_id = session::run_id(session::DEFAULT_RUNTIME, session_id);
        let mut run = session::Run::new(run_id.clone());
        run.issue = Some(crate::identity::Identity::Number(7));
        run.state = Some("in-progress".to_owned());
        session::store(&context.state_root, &run).expect("the pointer writes");

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
            &run_id,
            7,
            "in-progress",
        )]);
        let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            run_as(
                Dialect::ClaudeCode,
                Some("claude-code"),
                Event::Stop,
                &Input {
                    session_id: session_id.to_owned(),
                    ..Input::default()
                },
                Some(&context),
            )
        });
        assert_eq!(
            answer.get("decision").and_then(Value::as_str),
            Some("block"),
            "a run holding a live claim was let go: {answer}"
        );
        let reason = answer
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            reason.contains("#7") && reason.contains("still open"),
            "the block names neither the issue nor what remains of it: {reason}"
        );
    }

    fn stop_waiting(state: &str, recorded: &[(&str, &str)]) -> Value {
        crate::transport::store::forget_scratch_store();
        let root = tempfile::tempdir().expect("a temporary root");
        let mut context = stop_context(root.path());
        context.tracker = crate::config::Tracker::Github {
            repo: Some(crate::config::RepoRef::parse("owner/name").expect("a repository")),
        };
        let (_run_id, mut run) = stop_run(&context, "stop-waiting-1");
        run.state = Some(state.to_owned());
        run.mark_verified();
        session::store(&context.state_root, &run).expect("the pointer writes");
        if !recorded.is_empty() {
            let comments: Vec<Value> = recorded
                .iter()
                .enumerate()
                .map(|(index, (kind, terminal))| {
                    let mut fields = vec![
                        ("run-id", "claude-a"),
                        ("epoch", "55555555555555555555555555555555"),
                        ("pr", "9"),
                        ("head", "1111111111111111111111111111111111111111"),
                        ("base", "2222222222222222222222222222222222222222"),
                        (
                            "digest",
                            "3333333333333333333333333333333333333333333333333333333333333333",
                        ),
                    ];
                    if *kind == "review-receipt" {
                        fields.push(("terminal-state", terminal));
                        fields.push(("escalated-to", "Alexis"));
                        fields.push(("reason", "the correction was spent"));
                    }
                    let body = crate::transport::markers::render(kind, &fields).expect("a marker");
                    json!({
                        "id": format!("IC_{index}"),
                        "createdAt": format!("2026-01-01T00:0{index}Z"),
                        "viewerDidAuthor": true,
                        "includesCreatedEdit": false,
                        "body": format!("{body}\n"),
                    })
                })
                .collect();
            let repo = crate::transport::store::Repo {
                host: "github.com".to_owned(),
                owner: "owner".to_owned(),
                name: "name".to_owned(),
            };
            let mut store = crate::transport::store::Store::open(
                &crate::transport::store::store_path().expect("a store path"),
            )
            .expect("the store opens");
            store
                .import(
                    &repo,
                    7,
                    &json!({"body": "Body.", "labels": [], "comments": comments}),
                    "2026-01-01T00:09Z",
                )
                .expect("the issue imports");
        }
        run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::Stop,
            &stop_input("stop-waiting-1"),
            Some(&context),
        )
    }

    #[test]
    fn a_stop_hook_lets_a_blocked_run_stop() {
        let answer = stop_waiting("blocked", &[]);
        assert_eq!(
            answer,
            json!({}),
            "a run waiting on a person was held: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_lets_a_run_with_an_escalated_receipt_stop() {
        let answer = stop_waiting("review", &[("review-receipt", "escalated")]);
        assert_eq!(answer, json!({}), "an escalated run was held: {answer}");
    }

    #[test]
    fn a_stop_hook_holds_a_run_whose_escalation_was_since_republished() {
        let answer = stop_waiting(
            "in-progress",
            &[("review-receipt", "escalated"), ("published", "")],
        );
        assert_eq!(
            answer.get("decision").and_then(Value::as_str),
            Some("block"),
            "a republished run was let go: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_holds_an_in_progress_run_with_no_escalation() {
        let answer = stop_waiting("in-progress", &[]);
        assert_eq!(
            answer,
            stop_block(
                &crate::identity::Identity::Number(7),
                &session::run_id(session::DEFAULT_RUNTIME, "stop-waiting-1")
            )
        );
    }

    fn stop_input(session_id: &str) -> Input {
        Input {
            session_id: session_id.to_owned(),
            ..Input::default()
        }
    }

    fn stop_run(context: &GateContext, session_id: &str) -> (String, session::Run) {
        std::fs::create_dir_all(&context.state_root).expect("the state directory");
        std::fs::create_dir_all(&context.repo_dir).expect("the checkout the tracker reads in");
        let run_id = session::run_id(session::DEFAULT_RUNTIME, session_id);
        let mut run = session::Run::new(run_id.clone());
        run.issue = Some(crate::identity::Identity::Number(7));
        run.state = Some("in-progress".to_owned());
        (run_id, run)
    }

    fn gh_calls(log: &std::path::Path, checkout: &std::path::Path) -> usize {
        let from = checkout.display().to_string();
        std::fs::read_to_string(log).map_or(0, |text| {
            text.lines().filter(|line| line.ends_with(&from)).count()
        })
    }

    #[test]
    fn a_stop_hook_inside_the_renewal_window_asks_the_tracker_nothing() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        let (run_id, mut run) = stop_run(&context, "stop-window-1");
        run.mark_verified();
        session::store(&context.state_root, &run).expect("the pointer writes");

        let bin = crate::test_env::scripted_gh();
        let log = root.path().join("gh.log");
        let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
            &run_id,
            7,
            "in-progress",
        )]);
        let answer = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            run_as(
                Dialect::ClaudeCode,
                Some("claude-code"),
                Event::Stop,
                &stop_input("stop-window-1"),
                Some(&context),
            )
        });
        assert_eq!(
            gh_calls(&log, &context.repo_dir),
            0,
            "a Stop inside the renewal window of a successful verify read the tracker again"
        );
        assert_eq!(
            answer.get("decision").and_then(Value::as_str),
            Some("block"),
            "reusing the recent verify changed what the Stop hook decides: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_never_holds_an_assisted_run_inside_the_window() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        let (_run_id, mut run) = stop_run(&context, "stop-assisted-1");
        run.assisted = true;
        run.mark_verified();
        session::store(&context.state_root, &run).expect("the pointer writes");
        let answer = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::Stop,
            &stop_input("stop-assisted-1"),
            Some(&context),
        );
        assert_eq!(
            answer,
            json!({}),
            "an assisted run was kept working: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_records_the_verify_it_paid_for() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        let (run_id, run) = stop_run(&context, "stop-window-2");
        session::store(&context.state_root, &run).expect("the pointer writes");

        let bin = crate::test_env::scripted_gh();
        let log = root.path().join("gh.log");
        let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
            &run_id,
            7,
            "in-progress",
        )]);
        let mut answers = Vec::new();
        let mut counts = Vec::new();
        for _ in 0..2 {
            answers.push(crate::test_env::with_logging_gh(
                bin.path(),
                &script,
                &log,
                || {
                    run_as(
                        Dialect::ClaudeCode,
                        Some("claude-code"),
                        Event::Stop,
                        &stop_input("stop-window-2"),
                        Some(&context),
                    )
                },
            ));
            counts.push(gh_calls(&log, &context.repo_dir));
        }
        assert!(
            counts[0] > 0,
            "the first Stop never read the tracker: {counts:?}"
        );
        assert_eq!(
            counts[1], counts[0],
            "a second Stop inside the window read the tracker again: {counts:?}"
        );
        assert!(
            answers
                .iter()
                .all(|answer| answer.get("decision").and_then(Value::as_str) == Some("block")),
            "{answers:?}"
        );
    }

    #[test]
    fn a_stop_hook_never_records_a_failed_read_as_a_verify() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        let (run_id, run) = stop_run(&context, "stop-window-3");
        session::store(&context.state_root, &run).expect("the pointer writes");

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::unreachable_tracker_answer()]);
        let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            run_as(
                Dialect::ClaudeCode,
                Some("claude-code"),
                Event::Stop,
                &stop_input("stop-window-3"),
                Some(&context),
            )
        });
        assert_eq!(answer, json!({}));
        assert_eq!(
            session::load(&context.state_root, &run_id).verified_at,
            None,
            "a read that failed was recorded as a verification"
        );
    }

    /// A `Stop` for a session holding #7 live, with the tracker scripted to
    /// say so — the state every background-launch test starts from.
    fn stop_with_transcript(session_id: &str, transcript: &std::path::Path) -> Value {
        crate::transport::store::forget_scratch_store();
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        std::fs::create_dir_all(&context.state_root).expect("the state directory");
        std::fs::create_dir_all(&context.repo_dir).expect("the checkout the tracker reads in");
        let run_id = session::run_id(session::DEFAULT_RUNTIME, session_id);
        let mut run = session::Run::new(run_id.clone());
        run.issue = Some(crate::identity::Identity::Number(7));
        run.state = Some("in-progress".to_owned());
        session::store(&context.state_root, &run).expect("the pointer writes");

        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
            &run_id,
            7,
            "in-progress",
        )]);
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            run_as(
                Dialect::ClaudeCode,
                Some("claude-code"),
                Event::Stop,
                &Input {
                    session_id: session_id.to_owned(),
                    transcript_path: transcript.display().to_string(),
                    ..Input::default()
                },
                Some(&context),
            )
        })
    }

    /// The measured shape of a background launch and its notification, read out
    /// of a real Claude Code 2.1.x transcript.
    const BACKGROUND_LAUNCH: &str = r#"{"type":"assistant","isSidechain":false,"message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_pending","name":"Bash","input":{"command":"sleep 595","run_in_background":true}}]}}"#;
    const BACKGROUND_RUNNING: &str = r#"{"type":"user","isSidechain":false,"toolUseResult":{"stdout":"","stderr":"","interrupted":false,"backgroundTaskId":"abc123"},"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_pending","content":"Command running in background with ID: abc123. You will be notified when it completes."}]}}"#;
    const BACKGROUND_REPORTED: &str = r#"{"type":"user","isSidechain":false,"message":{"role":"user","content":"<task-notification>\n<task-id>abc123</task-id>\n<tool-use-id>toolu_pending</tool-use-id>\n<status>completed</status>\n</task-notification>"}}"#;

    /// The second launch shape the same host writes: a subagent launched in the
    /// background carries **no** `run_in_background` flag on its tool_use, and
    /// the async answer arrives in the record's `toolUseResult`.
    const AGENT_LAUNCH: &str = r#"{"type":"assistant","isSidechain":false,"message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_async","name":"Agent","input":{"subagent_type":"developer","description":"Implement the slice","prompt":"work"}}]}}"#;
    const AGENT_ASYNC_RESULT: &str = r#"{"type":"user","isSidechain":false,"toolUseResult":{"isAsync":true,"status":"async_launched","agentId":"afbc591fd5b67eaff"},"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_async","content":[{"type":"text","text":"Async agent launched successfully. You will be notified automatically when it completes."}]}]}}"#;
    const AGENT_REPORTED: &str = r#"{"type":"user","isSidechain":false,"message":{"role":"user","content":"<task-notification>\n<task-id>afbc591fd5b67eaff</task-id>\n<tool-use-id>toolu_async</tool-use-id>\n<status>completed</status>\n</task-notification>"}}"#;

    /// A launch inside a sub-agent's own conversation, which never answers for
    /// the main one — here in the shape that would otherwise be a launch.
    const SIDECHAIN_RESULT: &str = r#"{"type":"user","isSidechain":true,"toolUseResult":{"isAsync":true,"status":"async_launched"},"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_sidechain","content":[{"type":"text","text":"Async agent launched successfully."}]}]}}"#;
    /// A launch the host refused: no notice, no notification, and never a
    /// launch — the turn must stay blocked.
    const DENIED_RESULT: &str = r#"{"type":"user","isSidechain":false,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_pending","is_error":true,"content":"Permission denied"}]}}"#;
    /// Ordinary output quoting the notice a launch writes — reading this file,
    /// for one. It starts nothing and must not become a launch.
    const QUOTED_NOTICE: &str = r#"{"type":"user","isSidechain":false,"toolUseResult":{"stdout":"Command running in background with ID: abc123","stderr":"","interrupted":false},"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_quote","content":"1\tCommand running in background with ID: abc123"}]}}"#;
    /// The same exclusion for a report: a sub-agent's notification naming the
    /// main conversation's launch answers nothing.
    const SIDECHAIN_NOTIFICATION: &str = r#"{"type":"user","isSidechain":true,"message":{"role":"user","content":"<task-notification>\n<task-id>abc123</task-id>\n<tool-use-id>toolu_pending</tool-use-id>\n<status>completed</status>\n</task-notification>"}}"#;

    fn transcript(records: &[&str]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("a temporary transcript directory");
        let mut body = records.join("\n");
        body.push('\n');
        std::fs::write(directory.path().join("session.jsonl"), body)
            .expect("the transcript writes");
        directory
    }

    #[test]
    fn a_stop_hook_stands_aside_while_a_background_launch_has_not_reported() {
        let directory = transcript(&[BACKGROUND_LAUNCH, BACKGROUND_RUNNING]);
        let answer =
            stop_with_transcript("stop-background-1", &directory.path().join("session.jsonl"));
        assert_eq!(
            answer,
            json!({}),
            "a run whose only remaining work is a background launch was kept polling: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_keeps_the_turn_going_once_a_background_launch_has_reported() {
        let directory = transcript(&[
            BACKGROUND_LAUNCH,
            BACKGROUND_RUNNING,
            SIDECHAIN_RESULT,
            BACKGROUND_REPORTED,
        ]);
        let answer =
            stop_with_transcript("stop-background-2", &directory.path().join("session.jsonl"));
        assert_eq!(
            answer.get("decision").and_then(Value::as_str),
            Some("block"),
            "a reported launch — or a sidechain launch that never answers for this one — let a run \
             with a live claim go: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_is_not_fooled_by_output_quoting_the_notice() {
        let directory = transcript(&[BACKGROUND_LAUNCH, QUOTED_NOTICE]);
        let answer = stop_with_transcript("stop-quoted-1", &directory.path().join("session.jsonl"));
        assert_eq!(
            answer.get("decision").and_then(Value::as_str),
            Some("block"),
            "output that quoted the notice was read as a launch that will never report: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_keeps_the_turn_going_when_a_background_launch_never_started() {
        let directory = transcript(&[BACKGROUND_LAUNCH, DENIED_RESULT]);
        let answer = stop_with_transcript("stop-denied-1", &directory.path().join("session.jsonl"));
        assert_eq!(
            answer.get("decision").and_then(Value::as_str),
            Some("block"),
            "a refused background call was read as a launch that will never report: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_is_not_answered_by_a_sidechain_notification() {
        let directory = transcript(&[
            BACKGROUND_LAUNCH,
            BACKGROUND_RUNNING,
            SIDECHAIN_NOTIFICATION,
        ]);
        let answer =
            stop_with_transcript("stop-sidechain-1", &directory.path().join("session.jsonl"));
        assert_eq!(
            answer,
            json!({}),
            "a sub-agent's notification answered for the main conversation's launch: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_stands_aside_for_an_async_agent_launch_too() {
        let pending = transcript(&[AGENT_LAUNCH, AGENT_ASYNC_RESULT]);
        let answer = stop_with_transcript("stop-async-1", &pending.path().join("session.jsonl"));
        assert_eq!(
            answer,
            json!({}),
            "a run waiting on a background agent launch was kept polling: {answer}"
        );
        let reported = transcript(&[AGENT_LAUNCH, AGENT_ASYNC_RESULT, AGENT_REPORTED]);
        let answer = stop_with_transcript("stop-async-2", &reported.path().join("session.jsonl"));
        assert_eq!(
            answer.get("decision").and_then(Value::as_str),
            Some("block"),
            "a reported agent launch let a run with a live claim go: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_keeps_the_turn_going_when_the_transcript_cannot_be_read() {
        let directory = tempfile::tempdir().expect("a temporary transcript directory");
        let answer =
            stop_with_transcript("stop-background-3", &directory.path().join("absent.jsonl"));
        assert_eq!(
            answer.get("decision").and_then(Value::as_str),
            Some("block"),
            "an unreadable transcript was read as a launch that has not reported: {answer}"
        );
    }

    #[test]
    fn a_stop_hook_stands_aside_when_its_run_holds_nothing() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        std::fs::create_dir_all(&context.state_root).expect("the state directory");
        let answer = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::Stop,
            &Input {
                session_id: "stop-free-1".to_owned(),
                ..Input::default()
            },
            Some(&context),
        );
        assert_eq!(answer, json!({}), "a run holding nothing was kept working");
        let ledger = decisions(&context.state_root);
        assert!(
            !ledger.contains("tool-unnamed"),
            "a turn hook naming no tool was filed as a call that named none: {ledger}"
        );
    }

    #[test]
    fn a_stop_hook_never_wedges_a_session_that_is_already_stopping() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        std::fs::create_dir_all(&context.state_root).expect("the state directory");
        let session_id = "stop-capped-1";
        let mut run = session::Run::new(session::run_id(session::DEFAULT_RUNTIME, session_id));
        run.issue = Some(crate::identity::Identity::Number(7));
        run.state = Some("in-progress".to_owned());
        session::store(&context.state_root, &run).expect("the pointer writes");
        let answer = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::Stop,
            &Input {
                session_id: session_id.to_owned(),
                stop_hook_active: true,
                ..Input::default()
            },
            Some(&context),
        );
        assert_eq!(
            answer,
            json!({}),
            "a capped session was blocked again instead of being let go"
        );
    }

    #[test]
    fn a_stop_hook_with_no_readable_payload_stands_aside() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = stop_context(root.path());
        std::fs::create_dir_all(&context.state_root).expect("the state directory");
        let input = read_input(std::io::Cursor::new(
            r#"{"tool_input":{"command":"del src\x.rs"}}"#,
        ));
        assert!(input.unreadable);
        let answer = run_in(Dialect::ClaudeCode, Event::Stop, &input, Some(&context));
        assert_eq!(answer, json!({}), "an unreadable payload blocked a turn");
    }

    #[test]
    fn every_spelling_this_build_accepts_reaches_the_field_it_is_for() {
        let read = |body: &str| super::read_input(std::io::Cursor::new(body.to_owned()));

        // The session, under both names.
        for body in [r#"{"session_id": "s-1"}"#, r#"{"trajectory_id": "s-1"}"#] {
            assert_eq!(read(body).session_id, "s-1", "{body}");
        }

        // Which sub-agent is calling, under its top-level spellings. Absent stays absent:
        // "the main conversation" and "an unnamed sub-agent" are different
        // facts and the role gate reads them differently.
        for body in [
            r#"{"agent_type": "Explore"}"#,
            r#"{"agent_name": "Explore"}"#,
        ] {
            assert_eq!(read(body).agent_type.as_deref(), Some("Explore"), "{body}");
        }
        assert_eq!(read(r#"{"session_id": "s"}"#).agent_type, None);
        let launch = read(r#"{"tool_name":"Agent","tool_input":{"subagent_type":"review-blind"}}"#);
        assert_eq!(launch.agent_type, None);
        assert_eq!(launch.launch_target(), Some("review-blind"));
        let nested = read(
            r#"{"agent_type":"builder","tool_name":"Task","tool_input":{"subagent_type":"review-blind"}}"#,
        );
        assert_eq!(nested.agent_type.as_deref(), Some("builder"));
        assert_eq!(nested.launch_target(), Some("review-blind"));
        assert_eq!(read(r#"{"subagent_type":"review-blind"}"#).agent_type, None);

        // The tool, under both names, and reaching the classifier as a write.
        for body in [
            r#"{"tool_name": "Write", "tool_input": {"file_path": "src/x.rs"}}"#,
            r#"{"agent_action_name": "Write", "tool_input": {"file_path": "src/x.rs"}}"#,
        ] {
            let input = read(body);
            assert_eq!(input.tool_name, "Write", "{body}");
            let (action, _) = crate::harness::classify(&input.tool_name, &input.tool_input);
            assert!(
                matches!(action, crate::harness::Action::Write { .. }),
                "{body} reached the classifier as {action:?}, so the gate would stand aside"
            );
        }

        // Its arguments, under all three names — including the two that nest
        // the tool one level down, which `normalise` unwraps.
        for body in [
            r#"{"tool_name": "Write", "tool_input": {"file_path": "src/x.rs"}}"#,
            r#"{"tool_name": "Write", "tool_info": {"file_path": "src/x.rs"}}"#,
            r#"{"tool_call": {"name": "Write", "input": {"file_path": "src/x.rs"}}}"#,
        ] {
            let input = read(body);
            let (action, _) = crate::harness::classify(&input.tool_name, &input.tool_input);
            assert_eq!(
                action,
                crate::harness::Action::Write {
                    target: "src/x.rs".to_owned()
                },
                "{body} did not reach the classifier as a write to the file it names"
            );
        }

        // The working directory, when it arrives nested. This is the field that
        // decides **which oath covers this checkout**, and a gate that reads an
        // empty one asks about the wrong directory — the shape that already
        // cost this crate a push going through unweighed.
        let input =
            read(r#"{"tool_name": "Bash", "tool_info": {"cwd": "H:/repo", "command_line": "ls"}}"#);
        assert_eq!(
            input.cwd, "H:/repo",
            "a nested working directory never reached the field the gate reads"
        );
        // And an outer one is not overwritten by a nested one.
        let input =
            read(r#"{"cwd": "H:/outer", "tool_name": "Bash", "tool_input": {"cwd": "H:/inner"}}"#);
        assert_eq!(
            input.cwd, "H:/outer",
            "the nested value won over the real one"
        );

        // Windsurf's name for the shell line, which the classifier reads as
        // `command`. A shell that arrives without it is a push nobody weighed.
        let input = read(
            r#"{"tool_name": "Bash", "tool_input": {"command_line": "git push origin main"}}"#,
        );
        let (action, _) = crate::harness::classify(&input.tool_name, &input.tool_input);
        // A boundary, not merely a write: `git push` is one of the commands the
        // classifier weighs hardest, and that is the answer this asserts —
        // anything but `Untouched` means the line arrived.
        assert!(
            matches!(action, crate::harness::Action::Boundary { .. }),
            "a shell line under `command_line` reached the classifier as {action:?}"
        );
    }

    #[test]
    fn an_empty_payload_does_not_panic() {
        let input = read_input(std::io::empty());
        assert_eq!(input.session_id, "");
    }

    #[test]
    fn an_unparseable_payload_does_not_deny() {
        // A schema this build does not know must not stop somebody's edit.
        let input = read_input("this is not json".as_bytes());
        assert_eq!(input.tool_name, "");
        let response = run(Event::PreToolUse, &input, None);
        assert_eq!(response, json!({}));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let input = read_input(
            r#"{"session_id":"abc","tool_name":"Edit","some_future_field":{"a":1}}"#.as_bytes(),
        );
        assert_eq!(input.session_id, "abc");
        assert_eq!(input.tool_name, "Edit");
    }

    #[test]
    fn an_uninstalled_harness_stands_aside_instead_of_blocking_every_edit() {
        let input = Input {
            session_id: "abc".into(),
            tool_name: "Edit".into(),
            tool_input: json!({"file_path": "src/x.rs"}),
            ..Input::default()
        };
        assert_eq!(run(Event::PreToolUse, &input, None), json!({}));
    }

    #[test]
    fn a_running_review_blind_uses_embedded_policy_over_hostile_project_bytes() {
        let root = tempfile::tempdir().expect("a root");
        let repo = root.path().join("repo");
        let definition = repo.join(".claude/agents/review-blind.md");
        std::fs::create_dir_all(definition.parent().expect("a parent")).expect("agents directory");
        std::fs::write(
            definition,
            "---\nname: review-blind\ntools: Read, Write, Edit, Bash, Agent, Task\n---\n",
        )
        .expect("hostile project bytes");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: repo,
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };
        for tool in ["Write", "Edit", "Bash", "Agent", "Task"] {
            let input = Input {
                agent_type: Some("review-blind".to_owned()),
                tool_name: tool.to_owned(),
                // A different target isolates the running reviewer's policy
                // from the reserved-target prelaunch check.
                tool_input: json!({"subagent_type": "other"}),
                ..Input::default()
            };

            let answer = run_as(
                Dialect::ClaudeCode,
                Some("claude-code"),
                Event::PreToolUse,
                &input,
                Some(&context),
            );
            let reason = answer["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap_or_default();
            assert!(
                reason.contains("tool-outside-declared-role"),
                "{tool}: {answer}"
            );
        }
    }

    #[test]
    fn the_role_a_delegated_context_ran_as_reaches_the_pointer() {
        let root = tempfile::tempdir().expect("a state root");
        let repo = root.path().join("repo");
        std::fs::create_dir_all(&repo).expect("a repository directory");
        let state_root = root.path().join("state");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: repo,
            state_root: state_root.clone(),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };

        let roles_after = |agent: Option<&str>| -> std::collections::BTreeSet<String> {
            let _ = std::fs::remove_dir_all(&state_root);
            let input = Input {
                agent_type: agent.map(str::to_owned),
                tool_name: "Read".to_owned(),
                tool_input: json!({"file_path": "src/x.rs"}),
                session_id: "abcd1234".to_owned(),
                ..Input::default()
            };
            let answer = run_as(
                Dialect::ClaudeCode,
                Some("claude-code"),
                Event::PreToolUse,
                &input,
                Some(&context),
            );
            assert_ne!(
                answer["hookSpecificOutput"]["permissionDecision"], "deny",
                "a read this role declares should not be refused: {answer}"
            );
            session::load(
                &state_root,
                &session::run_id(session::DEFAULT_RUNTIME, "abcd1234"),
            )
            .roles
        };

        let delegated = roles_after(Some("review-blind"));
        assert!(
            delegated.contains("review-blind"),
            "the gate saw a call fired inside `review-blind` and the pointer does not carry it, so \
             nothing can read which role the context ran as, not even this run on a resume: \
             {delegated:?}"
        );

        let ordinary = roles_after(None);
        assert!(
            ordinary.is_empty(),
            "a call carrying no `agent_type` recorded a role anyway, so the record says a context \
             was delegated where none was: {ordinary:?}"
        );

        // A host that sends the field and leaves it empty. Distinct from not
        // sending it at all, and a judge measured that dropping the filter left
        // the whole suite green: the set would gain an empty name and
        // `SessionStart` would announce a role spelled as nothing.
        let blank = roles_after(Some("   "));
        assert!(
            blank.is_empty(),
            "an empty `agent_type` was recorded as a role, so a later reader is told a context ran \
             as something with no name: {blank:?}"
        );

        // And the surface. A record kept where nothing reads it aloud is the
        // shape this repository refuses everywhere else.
        let mut run = session::Run::new("claude-abcd1234".to_owned());
        run.roles.insert("review-blind".to_owned());
        let announced = session_start_response(&context, &run);
        let text = announced["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default();
        assert!(
            text.contains("review-blind"),
            "the first message a run reads does not name the roles its delegated contexts ran as: \
             {text}"
        );

        let silent =
            session_start_response(&context, &session::Run::new("claude-abcd1234".to_owned()));
        let quiet = silent["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default();
        assert!(
            !quiet.contains("Delegated contexts"),
            "a run that has gated no delegated context announced one: {quiet}"
        );
    }

    fn durable_roles_context(root: &std::path::Path) -> GateContext {
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).expect("a repository directory");
        GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.join("skill"),
            repo_dir: repo,
            state_root: root.join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        }
    }

    #[test]
    fn an_outside_call_records_the_role_where_the_decision_ledger_will_not() {
        let root = tempfile::tempdir().expect("a state root");
        let context = durable_roles_context(root.path());

        let answer = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &Input {
                agent_type: Some("review-blind".to_owned()),
                tool_name: "Read".to_owned(),
                tool_input: json!({"file_path": "src/x.rs"}),
                session_id: "abcd1234".to_owned(),
                ..Input::default()
            },
            Some(&context),
        );
        assert_eq!(
            answer,
            json!({}),
            "a delegated context's read was not answered Outside, so this is not the case it \
             claims: {answer}"
        );
        assert!(
            session::decision_records(&context.state_root).is_empty(),
            "the decision ledger recorded an Outside call, so the durable store is not the road \
             this claims it is"
        );
        let recorded = session::role_records(&context.state_root);
        assert!(
            recorded
                .iter()
                .any(|record| record["agent_type"].as_str() == Some("review-blind")),
            "an Outside call's role reached no durable record: {recorded:?}"
        );
    }

    #[test]
    fn a_role_one_run_recorded_outlives_its_session_and_a_second_run_reads_it() {
        let root = tempfile::tempdir().expect("a state root");
        let context = durable_roles_context(root.path());

        let first_session = "abcd1234";
        let first_run = session::run_id(session::DEFAULT_RUNTIME, first_session);
        let answer = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &Input {
                agent_type: Some("review-blind".to_owned()),
                tool_name: "Read".to_owned(),
                tool_input: json!({"file_path": "src/x.rs"}),
                session_id: first_session.to_owned(),
                ..Input::default()
            },
            Some(&context),
        );
        assert_eq!(
            answer,
            json!({}),
            "the fixture did not drive an Outside call: {answer}"
        );

        let recorded = session::role_records(&context.state_root);
        assert_eq!(
            recorded.len(),
            1,
            "the gate did not record the role durably: {recorded:?}"
        );
        assert_eq!(recorded[0]["run_id"].as_str(), Some(first_run.as_str()));
        assert_eq!(recorded[0]["agent_type"].as_str(), Some("review-blind"));

        // The session ends: the pointer, and with it the within-run half, goes.
        session::forget(&context.state_root, &first_run);
        assert!(!session::pointer_path(&context.state_root, &first_run).exists());

        // A second run — a different identity — reads what the first one saw.
        let second_session = "efgh5678";
        let second_run = session::run_id(session::DEFAULT_RUNTIME, second_session);
        assert_ne!(first_run, second_run);
        let second = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &Input {
                tool_name: "Read".to_owned(),
                tool_input: json!({"file_path": "src/x.rs"}),
                session_id: second_session.to_owned(),
                ..Input::default()
            },
            Some(&context),
        );
        assert_eq!(second, json!({}));
        let still = session::role_records(&context.state_root);
        assert!(
            still.iter().any(|record| {
                record["run_id"].as_str() == Some(first_run.as_str())
                    && record["agent_type"].as_str() == Some("review-blind")
            }),
            "the durable record did not survive the first session's end: {still:?}"
        );
        // The second run's own pointer does not carry it: the durable log is
        // the half that crossed, and the within-run half did not.
        assert!(
            session::load(&context.state_root, &second_run)
                .roles
                .is_empty()
        );
        let announced = session_start_response(&context, &session::Run::new(second_run));
        let text = announced["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default();
        assert!(
            !text.contains("review-blind"),
            "the second run was announced a role its pointer never held: {text}"
        );
    }

    #[test]
    fn a_launch_that_declares_a_model_records_it_and_absent_when_it_declared_nothing() {
        let root = tempfile::tempdir().expect("a state root");
        let context = durable_roles_context(root.path());

        let launch = |tool_input: serde_json::Value| {
            run_as(
                Dialect::ClaudeCode,
                Some("claude-code"),
                Event::PreToolUse,
                &Input {
                    tool_name: "Agent".to_owned(),
                    tool_input,
                    session_id: "abcd1234".to_owned(),
                    ..Input::default()
                },
                Some(&context),
            );
        };

        launch(json!({"subagent_type": "developer", "model": "opus", "effort": "high"}));
        let recorded = session::role_records(&context.state_root);
        let entry = recorded
            .iter()
            .find(|record| record["agent_type"].as_str() == Some("developer"))
            .unwrap_or_else(|| panic!("the launch declaration reached no record: {recorded:?}"));
        assert_eq!(entry["model"].as_str(), Some("opus"));
        assert_eq!(entry["effort"].as_str(), Some("high"));

        launch(json!({"subagent_type": "analyst", "model": "haiku"}));
        let recorded = session::role_records(&context.state_root);
        let analyst = recorded
            .iter()
            .find(|record| record["agent_type"].as_str() == Some("analyst"))
            .unwrap_or_else(|| {
                panic!("the model-only declaration reached no record: {recorded:?}")
            });
        assert_eq!(analyst["model"].as_str(), Some("haiku"));
        assert!(
            analyst.get("effort").is_none(),
            "an undeclared effort arrived as a field: {analyst:?}"
        );

        let before = session::role_records(&context.state_root).len();
        launch(json!({"subagent_type": "reviewer"}));
        assert_eq!(
            session::role_records(&context.state_root).len(),
            before,
            "a launch that declared nothing recorded a declaration anyway"
        );

        run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &Input {
                agent_type: Some("developer".to_owned()),
                tool_name: "Read".to_owned(),
                tool_input: json!({"file_path": "src/x.rs"}),
                session_id: "abcd1234".to_owned(),
                ..Input::default()
            },
            Some(&context),
        );
        assert_eq!(
            session::role_records(&context.state_root).len(),
            before,
            "a declared role was recorded a second time when its context ran"
        );
    }

    #[test]
    fn a_codex_write_is_adjudicated_against_the_claim_its_run_holds() {
        let root = tempfile::tempdir().expect("a state root");
        let repo = root.path().join("repo");
        std::fs::create_dir_all(&repo).expect("a repository directory");
        let skill_root = root.path().join("skill");
        std::fs::create_dir_all(&skill_root).expect("a skill root");
        std::fs::write(skill_root.join(crate::skill::CONTRACT), "the contract\n")
            .expect("the contract is installed");
        std::fs::create_dir_all(root.path().join("state")).expect("a state root");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root,
            repo_dir: repo.clone(),
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };

        let run_id = session::run_id("codex", "abcd1234");
        let mut run = session::Run::new(run_id.clone());
        run.issue = Some(crate::identity::Identity::Number(12));
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo);
        session::store(&context.state_root, &run).expect("the pointer writes");

        let write = Input {
            tool_name: "Write".to_owned(),
            tool_input: json!({"file_path": "src/x.rs"}),
            session_id: "abcd1234".to_owned(),
            ..Input::default()
        };
        let ask = |script: &str| {
            let bin = crate::test_env::scripted_gh();
            crate::test_env::with_scripted_gh(bin.path(), script, || {
                run_as(
                    Dialect::ClaudeCode,
                    Some("codex"),
                    Event::PreToolUse,
                    &write,
                    Some(&context),
                )
            })
        };

        let refused = ask(&crate::test_env::answers(&[
            crate::test_env::not_current_live_holder_issue(12),
        ]));
        assert_eq!(
            refused["hookSpecificOutput"]["permissionDecision"], "deny",
            "a Codex run whose claim the tracker no longer shows was treated as holding nothing: \
             {refused}"
        );

        crate::transport::store::forget_scratch_store();
        let allowed = ask(&crate::test_env::answers(&[
            crate::test_env::live_holder_issue(&run_id, 12, "in-progress"),
        ]));
        assert_eq!(
            allowed["hookSpecificOutput"]["permissionDecision"], "allow",
            "a Codex run holding a live claim was refused: {allowed}"
        );
        let ledger = decisions(&context.state_root);
        assert!(
            ledger.contains(&run_id),
            "the decision was recorded under another run: {ledger}"
        );
    }

    #[test]
    fn each_agent_derives_the_runtime_its_run_id_begins_with() {
        assert_eq!(session::runtime_of_agent(Some("codex")), "codex");
        assert_eq!(session::runtime_of_agent(Some("claude-code")), "claude");
        assert_eq!(session::runtime_of_agent(Some("gemini-cli")), "claude");
        assert_eq!(session::runtime_of_agent(None), session::DEFAULT_RUNTIME);
        for (slug, runtime) in session::AGENT_RUNTIMES {
            assert!(
                crate::setup::every_adapter().any(|adapter| adapter.slug == *slug),
                "{slug} names no adapter"
            );
            assert!(session::runtime_component(runtime).is_ok(), "{runtime}");
        }
    }

    #[test]
    fn an_inline_write_is_recorded_only_after_a_delegated_context_has_run() {
        let root = tempfile::tempdir().expect("a state root");
        let repo = root.path().join("repo");
        std::fs::create_dir_all(&repo).expect("a repository directory");
        let skill_root = root.path().join("skill");
        std::fs::create_dir_all(&skill_root).expect("a skill root");
        std::fs::write(skill_root.join(crate::skill::CONTRACT), "the contract\n")
            .expect("the contract is installed");
        let state_root = root.path().join("state");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root,
            repo_dir: repo.clone(),
            state_root: state_root.clone(),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };

        let run_id = session::run_id(session::DEFAULT_RUNTIME, "abcd1234");
        let mut run = session::Run::new(run_id.clone());
        run.issue = Some(crate::identity::Identity::Number(12));
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo);
        run.mark_verified();
        session::store(&state_root, &run).expect("the pointer writes");

        let write_input = |target: &str| Input {
            agent_type: None,
            tool_name: "Write".to_owned(),
            tool_input: json!({"file_path": target}),
            session_id: "abcd1234".to_owned(),
            ..Input::default()
        };

        // Before any delegated context has ever been gated, an inline write is
        // an unknown origin, not this run's own.
        let early = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &write_input("src/early.rs"),
            Some(&context),
        );
        assert_eq!(
            early["hookSpecificOutput"]["permissionDecision"], "allow",
            "the fixture's own write was refused, so the case under test was never reached: {early}"
        );
        let before = session::load(&state_root, &run_id);
        assert!(
            before.inline_writes.is_empty(),
            "a write before this run ever gated a delegated context was recorded as this run's \
             own: {:?}",
            before.inline_writes
        );

        let delegated = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &Input {
                agent_type: Some("review-blind".to_owned()),
                tool_name: "Read".to_owned(),
                tool_input: json!({"file_path": "src/x.rs"}),
                session_id: "abcd1234".to_owned(),
                ..Input::default()
            },
            Some(&context),
        );
        assert_ne!(
            delegated["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );

        // A present-but-blank `agent_type` is not proof of this run's own
        // context: the same payload that leaves `roles` untouched must not turn
        // a delegate's write into a claim that this run wrote it.
        let blank = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &Input {
                agent_type: Some("   ".to_owned()),
                tool_name: "Write".to_owned(),
                tool_input: json!({"file_path": "src/blank.rs"}),
                session_id: "abcd1234".to_owned(),
                ..Input::default()
            },
            Some(&context),
        );
        assert_ne!(blank["hookSpecificOutput"]["permissionDecision"], "deny");
        let between = session::load(&state_root, &run_id);
        assert!(
            between.inline_writes.is_empty(),
            "a present-but-blank `agent_type` was read as this run's own context: {:?}",
            between.inline_writes
        );

        // Now this run's own context writes again, after a delegated one ran.
        let late = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &write_input("src/late.rs"),
            Some(&context),
        );
        assert_eq!(
            late["hookSpecificOutput"]["permissionDecision"], "allow",
            "the fixture's own write was refused, so the case under test was never reached: {late}"
        );
        // A shell write names the matched fragment, not a path, and is still a
        // target: the message must say target for it too.
        let shell = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &Input {
                agent_type: None,
                tool_name: "Bash".to_owned(),
                tool_input: json!({"command": "git commit -m x"}),
                session_id: "abcd1234".to_owned(),
                ..Input::default()
            },
            Some(&context),
        );
        assert_eq!(
            shell["hookSpecificOutput"]["permissionDecision"], "allow",
            "the fixture's own shell write was refused, so the case under test was never reached: \
             {shell}"
        );
        let after = session::load(&state_root, &run_id);
        assert_eq!(
            after.inline_writes,
            std::collections::BTreeSet::from(["git commit".to_owned(), "src/late.rs".to_owned()]),
            "a shell write from this run's own context was not recorded as a target beside the \
             path: {:?}",
            after.inline_writes
        );

        // And the surface. A record kept where nothing reads it aloud is the
        // shape this repository refuses everywhere else.
        let announced = session_start_response(&context, &after);
        let text = announced["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default();
        assert!(
            text.contains("2 targets inline")
                && text.contains("`git commit`")
                && text.contains("`src/late.rs`")
                && !text.contains("src/early.rs")
                && !text.contains("src/blank.rs"),
            "the first message a run reads does not name the targets this run wrote inline, or \
             names one it never recorded: {text}"
        );

        let silent = session_start_response(&context, &session::Run::new(run_id));
        let quiet = silent["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default();
        assert!(
            !quiet.contains("wrote") || !quiet.contains("inline"),
            "a run that wrote nothing inline announced that it had: {quiet}"
        );
    }

    #[test]
    fn a_write_outside_the_claim_is_not_an_inline_write_inside_the_renewal_window() {
        let root = tempfile::tempdir().expect("a state root");
        let repo = root.path().join("repo");
        std::fs::create_dir_all(&repo).expect("a repository directory");
        let skill_root = root.path().join("skill");
        std::fs::create_dir_all(&skill_root).expect("a skill root");
        std::fs::write(skill_root.join(crate::skill::CONTRACT), "the contract\n")
            .expect("the contract is installed");
        let state_root = root.path().join("state");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root,
            repo_dir: repo.clone(),
            state_root: state_root.clone(),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };

        let run_id = session::run_id(session::DEFAULT_RUNTIME, "abcd1234");
        let mut run = session::Run::new(run_id.clone());
        run.issue = Some(crate::identity::Identity::Number(12));
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.clone());
        run.roles.insert("review-blind".to_owned());
        run.mark_verified();
        session::store(&state_root, &run).expect("the pointer writes");

        let write_input = |target: &std::path::Path| Input {
            agent_type: None,
            tool_name: "Write".to_owned(),
            tool_input: json!({"file_path": target.display().to_string()}),
            session_id: "abcd1234".to_owned(),
            ..Input::default()
        };

        let scratch = root.path().join("scratch").join("pr-body.md");
        let outside = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &write_input(&scratch),
            Some(&context),
        );
        assert_ne!(
            outside["hookSpecificOutput"]["permissionDecision"], "deny",
            "a scratch write was refused: {outside}"
        );
        let after_scratch = session::load(&state_root, &run_id);
        assert!(
            after_scratch.within_window(super::super::RENEWAL_WINDOW),
            "the fixture left the renewal window, so the case under test was never reached"
        );
        assert!(
            after_scratch.inline_writes.is_empty(),
            "a write outside the claimed checkout was recorded as inline: {:?}",
            after_scratch.inline_writes
        );

        let claimed = repo.join("src").join("in_claim.rs");
        let inside = run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PreToolUse,
            &write_input(&claimed),
            Some(&context),
        );
        assert_eq!(
            inside["hookSpecificOutput"]["permissionDecision"], "allow",
            "the fixture's own write was refused, so the case under test was never reached: {inside}"
        );
        let after_claimed = session::load(&state_root, &run_id);
        assert_eq!(
            after_claimed.inline_writes,
            std::collections::BTreeSet::from([claimed.display().to_string()]),
            "a write inside the claimed checkout after a delegated context was not recorded"
        );
    }

    #[test]
    fn an_inline_target_is_named_a_target_whether_it_is_a_path_or_a_shell_command() {
        let run = session::Run {
            inline_writes: std::collections::BTreeSet::from([
                "git commit".to_owned(),
                "src/late.rs".to_owned(),
            ]),
            ..session::Run::new("claude-abcd1234".to_owned())
        };
        let announced = session_start_response(&nowhere(), &run);
        let text = announced["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default();
        assert!(
            text.contains("2 targets inline")
                && text.contains("`git commit`")
                && text.contains("`src/late.rs`"),
            "the message does not name a shell command and a path as the targets this run wrote: \
             {text}"
        );
        assert!(
            !text.contains("file inline") && !text.contains("files inline"),
            "the message still calls a shell command a file: {text}"
        );
    }

    #[test]
    fn the_event_a_response_declares_is_the_one_it_answers() {
        let named = |response: &Value| -> String {
            response["hookSpecificOutput"]["hookEventName"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        };

        let refusal = Refusal {
            code: "not-current-live-holder",
            message: "src/x.rs: claude-aaaa lost the race".into(),
            outcome: MutationOutcome::NotStarted,
            class: crate::harness::refusal_class::class_of_code("not-current-live-holder"),
            replay: Replayability::NotReplayable,
            resolution: Resolution::run("estigia status"),
            observed: None,
        };
        for (what, response) in [
            (
                "a denial",
                pre_tool_use_response(&Decision::Deny(Box::new(refusal))),
            ),
            (
                "an allowance",
                pre_tool_use_response(&Decision::Allow("inside the window".to_owned())),
            ),
        ] {
            assert_eq!(
                named(&response),
                "PreToolUse",
                "{what} is filed under an event the agent is not asking about: {response}"
            );
        }

        // And both session-start answers, which are the other two sites: a run
        // with a name, and one whose session carries no identity at all.
        for run in [
            session::Run::new("claude-aaaaaaaaaaaa".to_owned()),
            session::Run::new("claude-unknown".to_owned()),
        ] {
            let response = session_start_response(&nowhere(), &run);
            assert_eq!(
                named(&response),
                "SessionStart",
                "a session-start answer declares the wrong event: {response}"
            );
        }

        // The floor: the reader above answers with an empty string for a
        // response that has no such field, so every assertion would pass
        // against a response carrying none.
        assert_eq!(
            named(&json!({"hookSpecificOutput": {}})),
            "",
            "the reader does not distinguish a missing name from a wrong one"
        );
    }

    #[test]
    fn a_denial_carries_the_whole_refusal() {
        let refusal = Refusal {
            code: "not-current-live-holder",
            message: "src/x.rs: claude-aaaa lost the race".into(),
            outcome: MutationOutcome::NotStarted,
            class: crate::harness::refusal_class::class_of_code("not-current-live-holder"),
            replay: Replayability::NotReplayable,
            resolution: Resolution::run("estigia status"),
            observed: None,
        };
        let response = pre_tool_use_response(&Decision::Deny(Box::new(refusal)));
        let reason = response["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .expect("a denial carries a reason");

        assert_eq!(response["hookSpecificOutput"]["permissionDecision"], "deny");
        // Everything the agent needs to act, in the one place it will read.
        assert!(reason.contains("not-current-live-holder"));
        assert!(reason.contains("nothing was written"));
        assert!(reason.contains("do not repeat this command"));
        assert!(reason.contains("estigia status"));
    }

    #[test]
    fn standing_aside_is_an_empty_object_not_an_allow() {
        // An explicit `allow` overrides the user's own permission settings.
        // Estigia has no business doing that for a tool it does not gate.
        assert_eq!(
            pre_tool_use_response(&Decision::Outside(crate::harness::Aside::NothingSworn)),
            json!({})
        );
    }

    /// A context pointing at nothing, for the responses that do not read it.
    fn nowhere() -> GateContext {
        GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: std::path::PathBuf::from("estigia-no-skill-here"),
            repo_dir: std::path::PathBuf::from("estigia-no-repo-here"),
            state_root: std::path::PathBuf::from("estigia-no-state-here"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        }
    }

    #[test]
    fn a_session_that_holds_nothing_says_so_rather_than_implying_a_claim() {
        let run = session::Run::new("claude-abcd1234".to_owned());
        let response = session_start_response(&nowhere(), &run);
        let context = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(context.contains("claude-abcd1234"));
        assert!(context.contains("holds no issue"));
    }

    #[test]
    fn an_unclaimed_session_in_a_guarded_checkout_is_told_commits_and_pushes_are_refused() {
        let repo = crate::test_env::repository_with_commit();
        crate::harness::guard::install(
            repo.path(),
            std::path::Path::new("/usr/local/bin/estigia"),
            false,
        )
        .expect("the guard installs");
        let skill = tempfile::tempdir().expect("a skill root");
        std::fs::write(skill.path().join(crate::skill::CONTRACT), "the contract\n")
            .expect("the contract is installed");
        let context = GateContext {
            repo_dir: repo.path().to_path_buf(),
            skill_root: skill.path().to_path_buf(),
            ..nowhere()
        };
        let run = session::Run::new("claude-abcd1234".to_owned());
        let response = session_start_response(&context, &run);
        let text = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(
            text.contains("refuses every commit and every push") && text.contains("start_branch"),
            "{text}"
        );
        assert!(!text.contains("gates nothing"), "{text}");
    }

    #[test]
    fn a_session_opening_on_an_unreadable_pointer_is_told_the_truth_about_it() {
        let run = session::Run {
            unreadable: true,
            ..session::Run::new("claude-abcd1234".to_owned())
        };
        let response = session_start_response(&nowhere(), &run);
        let context = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(
            !context.contains("holds no issue"),
            "an unknown was announced as an absence: {context}"
        );
        assert!(
            !context.contains("gates nothing"),
            "a run about to be refused every write was told nothing is gated: {context}"
        );
        assert!(
            context.contains("cannot be read") && context.contains("unknown"),
            "the state is not named: {context}"
        );
        // And what to do about it, because the gate's own refusal on this state
        // carries one and two surfaces describing one machine must not differ.
        assert!(
            context.contains("claim it again") || context.contains("pointer away"),
            "the run is told it is stuck and not how to get out: {context}"
        );
    }

    #[test]
    fn a_session_that_holds_an_issue_names_it_and_its_state() {
        let mut run = session::Run::new("claude-abcd1234".to_owned());
        run.issue = Some(crate::identity::Identity::Number(12));
        run.state = Some("review".to_owned());
        let response = session_start_response(&nowhere(), &run);
        let context = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(context.contains("#12"));
        assert!(context.contains("review"));
    }

    #[test]
    fn a_session_whose_pointer_never_recorded_a_state_is_not_told_one() {
        // Reachable, and by the tolerance that is there on purpose: a pointer
        // written by an earlier build carries no `state` and still loads, so a
        // run can hold an issue and have never recorded a state. This surface
        // used to fill that hole with `in-progress` — the nearest named state,
        // reported instead of the unknown, on the first sentence a run reads.
        let mut run = session::Run::new("claude-abcd1234".to_owned());
        run.issue = Some(crate::identity::Identity::Number(12));
        assert!(
            run.state.is_none(),
            "the pointer already carries a state, so this test measures nothing"
        );
        let response = session_start_response(&nowhere(), &run);
        let context = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(context.contains("#12"), "{context}");
        assert!(
            !context.contains("in-progress"),
            "a state the pointer never recorded was announced as fact: {context}"
        );
        assert!(context.contains("never recorded"), "{context}");
    }

    #[test]
    fn a_session_is_told_when_the_installed_contract_is_not_this_binary_s_copy() {
        // The binary and the skill upgrade separately. A new `estigia` beside an
        // old contract means the agent reads one set of rules while the gate
        // enforces another, and nothing said so unless somebody ran `status` —
        // which nobody does until a refusal they did not expect.
        let run = session::Run::new("claude-abcd1234".to_owned());
        let response = session_start_response(&nowhere(), &run);
        let injected = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(injected.contains("No contract is installed"), "{injected}");
        // And it names a command that discharges the block, which is the ratchet.
        assert!(injected.contains("estigia setup --all"), "{injected}");

        // A contract that is current says nothing: a note repeated every session
        // for a state that is fine is a note people learn to skip.
        let root = tempfile::tempdir().expect("a temporary root");
        let config = crate::config::Config::default();
        crate::skill::install(root.path(), &config, false).expect("the skill installs");
        let current = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            skill_root: root.path().to_path_buf(),
            ..nowhere()
        };
        let response = session_start_response(&current, &run);
        let injected = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(!injected.contains("estigia sync"), "{injected}");
        assert!(!injected.contains("No contract"), "{injected}");
    }

    fn commit_in(repo: &std::path::Path, message: &str) -> String {
        let git = |args: &[&str]| -> String {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(args)
                .output()
                .expect("this fixture needs a usable `git` on PATH");
            assert!(
                output.status.success(),
                "`git {args:?}` failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        };
        git(&["config", "user.email", "build-note@estigia.test"]);
        git(&["config", "user.name", "estigia"]);
        git(&["commit", "--quiet", "--allow-empty", "-m", message]);
        git(&["rev-parse", "HEAD"])
    }

    fn repository_with_one_commit() -> (tempfile::TempDir, String) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let init = std::process::Command::new("git")
            .arg("-C")
            .arg(directory.path())
            .args(["init", "--quiet"])
            .status()
            .expect("this fixture needs a usable `git` on PATH");
        assert!(init.success(), "`git init` failed");
        let head = commit_in(directory.path(), "first");
        (directory, head)
    }

    const FOREIGN_COMMIT: &str = "1111111111111111111111111111111111111111";

    #[test]
    fn a_build_commit_this_repository_does_not_hold_says_nothing_at_session_start() {
        let (repo, _) = repository_with_one_commit();
        assert_eq!(
            build_note_for(repo.path(), Some(FOREIGN_COMMIT), false),
            "",
            "a commit that is not an object here cannot be acted on in this project"
        );
    }

    #[test]
    fn a_build_older_than_the_checkout_it_belongs_to_is_still_reported() {
        let (repo, first) = repository_with_one_commit();
        commit_in(repo.path(), "second");
        let note = build_note_for(repo.path(), Some(&first), false);
        assert!(note.contains("not established as current"), "{note}");
    }

    #[test]
    fn a_dirty_build_is_reported_even_where_its_commit_is_not_held() {
        let (repo, _) = repository_with_one_commit();
        let note = build_note_for(repo.path(), Some(FOREIGN_COMMIT), true);
        assert!(note.contains("cannot be told"), "{note}");
    }

    #[test]
    fn a_build_without_a_commit_is_reported_in_any_repository() {
        let (repo, _) = repository_with_one_commit();
        let note = build_note_for(repo.path(), None, false);
        assert!(note.contains("cannot be told"), "{note}");
    }

    #[test]
    fn a_directory_that_is_not_a_repository_is_still_reported() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let note = build_note_for(directory.path(), Some(FOREIGN_COMMIT), false);
        assert!(note.contains("cannot be told"), "{note}");
    }

    #[test]
    fn a_fine_build_health_produces_no_note() {
        let health = super::super::doctor::Health::Fine {
            detail: "this build (abc1234) is current against the checkout".to_owned(),
        };
        assert_eq!(render_build_note(&health), "");
    }

    #[test]
    fn a_broken_build_health_names_what_is_wrong_in_the_note() {
        let health = super::super::doctor::Health::Broken {
            detail: "this build (abc1234) is not established as current against the checkout's \
                      head (def5678): the checkout is 3 commit(s) ahead of this build"
                .to_owned(),
            resolution: crate::outcome::Resolution::run("cargo install --path . --force"),
        };
        let note = render_build_note(&health);
        assert!(note.contains("not established as current"), "{note}");
        assert!(
            !note.contains("is not current"),
            "the note claimed staleness where the verdict establishes only that currency could \
             not be shown: {note}"
        );
        assert!(note.contains("abc1234"), "{note}");
    }

    #[test]
    fn a_skipped_build_health_says_cannot_tell_never_current() {
        let health = super::super::doctor::Health::Skipped {
            detail: "this build came from a working tree with uncommitted changes, so its exact \
                      bytes cannot be compared to the checkout"
                .to_owned(),
        };
        let note = render_build_note(&health);
        assert!(note.contains("cannot be told"), "{note}");
        assert!(
            !note.contains("current against the checkout"),
            "a build that could not be compared must never render as though it were current: {note}"
        );
    }

    #[test]
    fn a_session_start_carries_the_build_note_when_the_checkout_cannot_be_compared() {
        let run = session::Run::new("claude-abcd1234".to_owned());
        let response = session_start_response(&nowhere(), &run);
        let injected = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(
            injected.contains("cannot be told"),
            "a checkout the build could not be compared against was not named at session start: \
             {injected}"
        );
    }

    #[test]
    fn a_nameless_session_start_carries_the_build_note_too() {
        // `is_nameless` answers to a `<runtime>-unknown` tail, and that is what
        // selects the other of the two return sites this note was added to.
        let run = session::Run::new(format!("claude-{}", session::NAMELESS));
        let response = session_start_response(&nowhere(), &run);
        let injected = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(
            injected.contains("nothing can be sworn here"),
            "this run id did not reach the nameless branch, so the assertion below would have \
             been about the other one: {injected}"
        );
        assert!(
            injected.contains("cannot be told"),
            "the nameless branch dropped the build note: {injected}"
        );
    }

    #[test]
    fn a_session_start_says_nothing_about_the_build_when_it_is_current() {
        let Some(commit) = crate::build_info::commit() else {
            eprintln!("SKIPPED: this build carries no commit.");
            return;
        };
        if crate::build_info::dirty() {
            eprintln!("SKIPPED: this build came from an uncommitted tree.");
            return;
        }
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let context = GateContext {
            repo_dir: manifest_dir.to_path_buf(),
            ..nowhere()
        };
        let run = session::Run::new("claude-abcd1234".to_owned());
        let response = session_start_response(&context, &run);
        let injected = response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected");
        assert!(
            !injected.contains("not established as current")
                && !injected.contains("cannot be told"),
            "this binary's own checkout, at this binary's own commit ({commit}), spoke about the \
             build when it should have said nothing: {injected}"
        );
    }

    #[test]
    fn an_allowed_boundary_says_which_one_it_was() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            integration: crate::config::Integration::Branch,
            evidence: crate::config::Evidence::Reading,
            flag: None,
            stand_down: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            operation_window: Default::default(),
            issue_requirement: crate::config::IssueRequirement::Off,
            workflow: Some(crate::config::Workflow::Autonomous),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            repository_host: crate::config::RepositoryHost::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");

        let line = |subject: Option<&str>, decision: &crate::harness::Decision| {
            clear_ledger(&context.state_root);
            note(&context, "claude-abcd1234", "Bash", subject, decision, &[]);
            let text = decisions(&context.state_root);
            serde_json::from_str::<Value>(text.lines().next().expect("a line"))
                .expect("the line is JSON")
        };
        let allowed = crate::harness::Decision::Allow("issue #12 is held by claude-a".to_owned());

        // Three different boundaries have to leave three different lines.
        let mut seen = std::collections::BTreeSet::new();
        for command in ["git tag", "gh release create", "git push"] {
            let record = line(Some(command), &allowed);
            assert_eq!(
                record.get("subject").and_then(Value::as_str),
                Some(command),
                "an allowed boundary did not say which one it was: {record}"
            );
            seen.insert(record.to_string());
        }
        assert_eq!(
            seen.len(),
            3,
            "three irreversible steps left fewer than three distinguishable lines"
        );

        // A write says which file, on the same key: the question *what did this
        // run touch* is the same question.
        let write = line(Some("src/main.rs"), &allowed);
        assert_eq!(
            write.get("subject").and_then(Value::as_str),
            Some("src/main.rs")
        );

        // The floor: a line with nothing to name does not carry an empty key.
        // A key that is `""` on every line is the noise `aimed_at` is kept off
        // the file for.
        let bare = line(None, &allowed);
        assert!(
            bare.get("subject").is_none(),
            "a decision about nothing in particular invented a subject: {bare}"
        );
        let blank = line(Some("   "), &allowed);
        assert!(
            blank.get("subject").is_none(),
            "a blank subject was written down as one: {blank}"
        );
    }

    fn queue_writes(count: usize) {
        use crate::transport::store::{OutboxKind, Repo, Store, store_path};
        let repo = Repo {
            host: "github.com".to_owned(),
            owner: "fixture".to_owned(),
            name: "repo".to_owned(),
        };
        let mut held = Store::open(&store_path().expect("a path")).expect("opens");
        for number in 0..count {
            crate::transport::outbox::queue_in(
                &mut held,
                &repo,
                12,
                OutboxKind::Comment,
                &json!({"kind": "note", "body": format!("note {number}")}),
                "2026-06-01T00:00Z",
            )
            .expect("queues");
        }
    }

    fn closed_context() -> GateContext {
        GateContext {
            operation_window: crate::config::GitOperationWindow::parse("enabled 09:00-17:00")
                .expect("a window"),
            ..nowhere()
        }
    }

    fn at_minutes(minutes: u16, body: impl FnOnce()) {
        std::thread_local! {
            static AT: std::cell::Cell<u16> = const { std::cell::Cell::new(0) };
        }
        AT.with(|at| at.set(minutes));
        super::super::operation_window::with_now_override(
            Some(|| AT.with(|at| Some(at.get()))),
            body,
        );
    }

    fn announced(context: &GateContext) -> String {
        let run = session::Run::new("claude-abcd1234".to_owned());
        session_start_response(context, &run)["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("context is injected")
            .to_owned()
    }

    fn flush_requests() -> u32 {
        FLUSH_REQUESTS.with(std::cell::Cell::get)
    }

    #[test]
    fn the_session_start_text_names_a_dirty_orphan_checkout_and_the_command_that_lists_it() {
        let root = tempfile::tempdir().expect("a root");
        let main = root.path().join("main");
        std::fs::create_dir_all(&main).unwrap();
        let git = |at: &std::path::Path, arguments: &[&str]| {
            let done = std::process::Command::new("git")
                .args(arguments)
                .current_dir(at)
                .status()
                .expect("git runs");
            assert!(done.success(), "git {arguments:?}");
        };
        git(&main, &["init", "--quiet", "-b", "main"]);
        git(&main, &["config", "user.email", "e@example.invalid"]);
        git(&main, &["config", "user.name", "E"]);
        git(&main, &["commit", "--quiet", "--allow-empty", "-m", "one"]);
        let tree = root.path().join("issue-9-parked");
        git(
            &main,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                "issue-9-parked",
                tree.to_str().unwrap(),
            ],
        );
        let context = GateContext {
            repo_dir: main,
            state_root: root.path().join("state"),
            skill_root: root.path().join("skill"),
            ..nowhere()
        };
        let run = session::Run::new("claude-abcd1234".to_owned());
        let clean = session_start_text(&context, &run, None);
        assert!(!clean.contains("estigia changes"), "{clean}");

        std::fs::write(tree.join("parked.txt"), "x\n").unwrap();
        for text in [
            session_start_text(&context, &run, None),
            session_start_response(&context, &run)["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .expect("context")
                .to_owned(),
        ] {
            assert!(text.contains("issue-9-parked"), "{text}");
            assert!(text.contains("`estigia changes`"), "{text}");
        }
    }

    #[test]
    fn an_empty_queue_leaves_the_session_start_text_as_it_was() {
        at_minutes(20 * 60, || {
            let text = announced(&closed_context());
            assert!(!text.contains("GitHub write"), "{text}");
            assert_eq!(flush_requests(), 0);
        });
    }

    #[test]
    fn a_closed_window_says_what_is_queued_and_starts_nothing() {
        queue_writes(2);
        at_minutes(20 * 60, || {
            let text = announced(&closed_context());
            assert!(
                text.contains("2 GitHub writes are queued; nothing reaches GitHub until"),
                "{text}"
            );
            assert!(text.contains("inside 09:00-17:00 local"), "{text}");
            assert_eq!(
                flush_requests(),
                0,
                "a flush started while the window was closed"
            );
        });
    }

    #[test]
    fn the_text_served_to_pi_does_not_change_as_writes_queue() {
        let run = session::Run::new("pi-abcd1234".to_owned());
        at_minutes(20 * 60, || {
            queue_writes(1);
            let one = session_start_text(&closed_context(), &run, None);
            queue_writes(4);
            let five = session_start_text(&closed_context(), &run, None);
            assert_eq!(one, five);
            assert!(
                one.contains("GitHub writes are queued; nothing reaches"),
                "{one}"
            );
            assert!(!one.contains("1 GitHub"), "{one}");
        });
    }

    #[test]
    fn the_claude_session_start_text_still_names_the_count() {
        queue_writes(5);
        at_minutes(20 * 60, || {
            let text = announced(&closed_context());
            assert!(text.contains("5 GitHub writes are queued"), "{text}");
        });
    }

    #[test]
    fn an_open_window_with_a_queue_starts_the_flush_and_says_so() {
        queue_writes(1);
        at_minutes(12 * 60, || {
            let text = announced(&closed_context());
            assert!(
                text.contains("1 GitHub write is queued") && text.contains("was started"),
                "{text}"
            );
            assert_eq!(flush_requests(), 1);
        });
    }

    #[test]
    fn the_text_served_to_pi_names_no_count_while_the_window_is_open_either() {
        let run = session::Run::new("pi-abcd1234".to_owned());
        at_minutes(12 * 60, || {
            queue_writes(1);
            let one = session_start_text(&closed_context(), &run, None);
            assert_eq!(flush_requests(), 1);
            queue_writes(4);
            let five = session_start_text(&closed_context(), &run, None);
            assert_eq!(flush_requests(), 2);
            assert_eq!(one, five);
            assert!(
                one.contains(
                    "GitHub writes are queued from a closed git operation window; `estigia flush` \
                     was started"
                ),
                "{one}"
            );
            assert!(!one.contains("1 GitHub"), "{one}");
        });
    }

    fn counted_note_is_singular_then_plural(minutes: u16, tail: &'static str) {
        at_minutes(minutes, || {
            queue_writes(1);
            let one = queue_note(&closed_context(), true).expect("one write waits");
            assert!(
                one.starts_with(&format!("1 GitHub write is {tail}")),
                "{one}"
            );
            queue_writes(2);
            let three = queue_note(&closed_context(), true).expect("three writes wait");
            assert!(
                three.starts_with(&format!("3 GitHub writes are {tail}")),
                "{three}"
            );
        });
    }

    #[test]
    fn a_counted_queue_note_is_singular_for_one_write_while_the_window_is_closed() {
        counted_note_is_singular_then_plural(20 * 60, "queued; nothing reaches GitHub");
    }

    #[test]
    fn a_counted_queue_note_is_singular_for_one_write_while_the_window_is_open() {
        counted_note_is_singular_then_plural(12 * 60, "queued from a closed git operation window");
    }

    #[test]
    fn the_stop_hook_names_the_queue_and_never_blocks_for_it() {
        queue_writes(1);
        let input = Input {
            session_id: "abcd1234".to_owned(),
            ..Input::default()
        };
        at_minutes(20 * 60, || {
            let answer = run(Event::Stop, &input, Some(&closed_context()));
            assert!(answer.get("decision").is_none(), "{answer}");
            assert!(
                answer["systemMessage"]
                    .as_str()
                    .is_some_and(|text| text.contains("1 GitHub write is queued")),
                "{answer}"
            );
            assert_eq!(flush_requests(), 0);
        });
        at_minutes(12 * 60, || {
            let answer = run(Event::Stop, &input, Some(&closed_context()));
            assert!(answer.get("decision").is_none(), "{answer}");
            assert_eq!(flush_requests(), 1);
        });
    }

    #[test]
    fn the_stop_hook_with_an_empty_queue_answers_as_before() {
        let input = Input {
            session_id: "abcd1234".to_owned(),
            ..Input::default()
        };
        at_minutes(20 * 60, || {
            assert_eq!(run(Event::Stop, &input, Some(&closed_context())), json!({}));
        });
    }

    #[test]
    fn mail_rides_only_on_an_answer_that_decides_nothing_and_a_refusal_keeps_it_waiting() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let context = GateContext {
            state_root: directory.path().join("runs"),
            ..nowhere()
        };
        let path = crate::transport::store::beside(&context.state_root);
        let store = crate::transport::store::Store::open(&path).expect("the store opens");
        store
            .message_sent(
                &crate::transport::store::MessageSent {
                    sender_run_id: None,
                    recipient_run_id: "claude-aaaa",
                    body: "wait for me",
                },
                10,
            )
            .expect("the message is recorded");

        let refusal = crate::outcome::Refusal::not_started(
            "unexpected-state",
            "not now",
            crate::outcome::Resolution::run("estigia status"),
        );
        for refused in [
            Dialect::ClaudeCode.deny(&refusal),
            Dialect::ClaudeCode.ask(&refusal),
        ] {
            assert_eq!(
                with_mail(
                    &context,
                    Dialect::ClaudeCode,
                    "claude-aaaa",
                    refused.clone()
                ),
                refused
            );
        }
        assert_eq!(
            with_mail(&context, Dialect::Cursor, "claude-aaaa", json!({})),
            json!({})
        );
        assert_eq!(
            store
                .messages_waiting_for()
                .expect("counts")
                .get("claude-aaaa"),
            Some(&1)
        );

        let allowed = with_mail(
            &context,
            Dialect::ClaudeCode,
            "claude-aaaa",
            Dialect::ClaudeCode.allow("inside the claim"),
        );
        assert_eq!(allowed["hookSpecificOutput"]["permissionDecision"], "allow");
        assert!(
            allowed["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .is_some_and(|text| text.contains("wait for me")),
            "{allowed}"
        );
    }
}
