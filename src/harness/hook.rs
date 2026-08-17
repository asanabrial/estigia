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

use std::io::Read;

use serde::Deserialize;
use serde_json::{Value, json};

use super::{Decision, GateContext, RENEWAL_WINDOW, classify_with, session};
use crate::outcome::Refusal;

/// The lifecycle events Estigia registers for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Before a tool runs. The one that can say no.
    PreToolUse,
    /// A session begins or resumes. Mints the run identity.
    SessionStart,
    /// A session ends. Forgets the pointer; the claim on the tracker stays.
    SessionEnd,
    /// Git is about to push. **Not an agent event** — git's own, and the reason
    /// it is here is that it is the one gate no agent can route around.
    ///
    /// An agent hook only fires for the agent that installed it, and only one
    /// of the seven has an event that can deny. A `pre-push` hook sits under
    /// git: it refuses a push typed by any agent, by a person, or by a script,
    /// and it does it at the boundary that cannot be taken back.
    PrePush,
}

impl Event {
    /// The slug typed on the command line and written into the settings file.
    pub fn slug(self) -> &'static str {
        match self {
            Self::PreToolUse => "pre-tool-use",
            Self::SessionStart => "session-start",
            Self::SessionEnd => "session-end",
            Self::PrePush => "pre-push",
        }
    }

    /// The name the agent uses for this event.
    pub fn agent_name(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::PrePush => "pre-push",
        }
    }

    /// Whether this event can say **no**, which is what makes a payload matter.
    ///
    /// Only one of them does. A `session-start` mints an identity and a
    /// `session-end` forgets a pointer; neither adjudicates a write, so neither
    /// can let one through undecided.
    ///
    /// The ledger recorded `payload-absent` for every event and `silence`
    /// counts that verdict, so a session hook fired with nothing on standard
    /// input — the ordinary shape of that event for several agents — was
    /// reported as a call the gate had let through. The check whose whole
    /// subject is a silence was counting events that never asked it to decide,
    /// and sent the operator to repair a registration that was working.
    pub fn decides(self) -> bool {
        matches!(self, Self::PreToolUse | Self::PrePush)
    }

    /// Every event, checked by the compiler.
    pub fn all() -> [Self; 4] {
        [
            Self::PreToolUse,
            Self::SessionStart,
            Self::SessionEnd,
            Self::PrePush,
        ]
    }

    /// The event a name names, in either spelling.
    ///
    /// **Both** the slug Estigia writes and [`Self::agent_name`], the spelling
    /// the host's own documentation uses — and case and separators are ignored,
    /// so `PreToolUse`, `pre_tool_use` and `pre-tool-use` are one event.
    ///
    /// This is not politeness. A settings file is a thing people copy and
    /// hand-edit, and the name they will reach for is the one their agent's
    /// documentation prints. Refusing it exits non-zero — which for a
    /// `PreToolUse` hook in Claude Code is a *non-blocking* error: the tool call
    /// goes through ungated, with a line in the transcript. A gate that is
    /// installed, looks installed, and decides nothing is the exact failure this
    /// project exists to refuse, and both spellings were already in this file.
    ///
    /// Nothing is loosened by reading a name two ways. No two events collide
    /// under this normalisation — held by
    /// `every_event_name_resolves_to_exactly_one_event`.
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
    // No matcher here, and that is the honest shape: the gate's matcher is
    // **per agent** and lives on each adapter's `GateSpec`, because the tools
    // are named differently by each host. This table carried a second copy of
    // Claude Code's, which nothing installed — every writer of a hooks file
    // skips `PreToolUse` in this loop and writes the gate from the adapter — so
    // the copy in the table that calls itself *"every event Estigia registers,
    // with the matcher each needs"* was inert, and read as the source of truth
    // by the one test that crossed it against the classifier. It drifted the
    // moment the installed one gained a name.
    (Event::PreToolUse, None, 10),
    (Event::SessionStart, None, 5),
    (Event::SessionEnd, None, 5),
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
    /// Which sub-agent made this call, when one did.
    ///
    /// Claude Code sends `agent_type` on every tool event that fires **inside**
    /// a sub-agent — the name, `"Explore"` or `"security-reviewer"`. Absent
    /// means the main conversation made the call, which is a different fact
    /// from "an unnamed sub-agent did" and has to stay distinguishable.
    ///
    /// This is the field that lets a role's declared tool list stop being a
    /// request. Other harnesses write `tools:` into a sub-agent definition and
    /// rely on the host to honour it; a harness that knows which sub-agent is
    /// calling can refuse instead.
    #[serde(default, alias = "agent_name")]
    pub agent_type: Option<String>,
    /// The tool about to run, on `PreToolUse`.
    ///
    /// Windsurf sends the *event* name here rather than a tool name, which is
    /// the same fact in a different word: `pre_write_code` says as much about
    /// what is about to happen as `Write` does. Both are in the classifier's
    /// populations for that reason.
    #[serde(default, alias = "agent_action_name")]
    pub tool_name: String,
    /// That tool's arguments.
    #[serde(default, alias = "tool_info", alias = "tool_call")]
    pub tool_input: Value,
    /// Whether a body arrived and could not be read.
    ///
    /// Standing aside on an unreadable payload is the decision [`read_input`]
    /// documents, and it is the right one — a schema this build does not know
    /// could be wrapping `Read` as easily as `Write`, and denying it would deny
    /// reads. What was wrong was doing it *silently*: every call of a session
    /// whose agent changed its schema would stand aside, the ledger records only
    /// decisions and `Outside` is not one, and the gate would look installed
    /// while deciding nothing. That shape has cost this crate five defects.
    #[serde(skip)]
    pub unreadable: bool,
    /// Whether no body arrived at all.
    ///
    /// Apart from [`Self::unreadable`], and for the same reason that one
    /// exists. Both mean *this call was not gated*, and this one used to leave
    /// no trace whatever: a harness registered without piping the payload, or
    /// an event whose body this build never receives, gated nothing at all and
    /// said nothing about it — which is the quieter half of the shape that has
    /// cost this crate five defects.
    ///
    /// Two words rather than one because the fix is different. A payload that
    /// will not parse is a schema to teach the classifier; a payload that never
    /// arrives is a registration to repair.
    #[serde(skip)]
    pub absent: bool,
    /// What was wrong with a payload that would not parse.
    ///
    /// The two words above say the fixes differ — a schema to teach, or a
    /// registration to repair — and neither could be told apart from the
    /// record, because the parse error was discarded at the `Err(_)` that set
    /// the flag. `doctor` then reported the silence BROKEN and asked the
    /// operator "whether the agent that sent them is one this build knows",
    /// which is the question the payload answered and nothing kept.
    ///
    /// It cost a whole round of this session to work out that four such lines
    /// on one machine were Windows paths inside JSON strings.
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
/// Three agents can deny a tool call and none of them agrees on how to say so.
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
    /// **Not JSON at all.** The agent reads an exit code: `2` blocks the action
    /// and it is handed whatever went to standard error.
    ///
    /// Named for the shape rather than for the first agent that needed it —
    /// Windsurf and Crush both answer this way, and a dialect called `Windsurf`
    /// would have made the second one look like a special case of the first.
    ///
    /// The odd one out, and worth its own variant rather than a flag, because
    /// the mistake it prevents is the expensive one. A decision printed as JSON
    /// where an exit code is expected is a refusal that prints itself and lets
    /// the write through — the same failure the git hook is written against,
    /// arriving through an agent this time.
    ExitCode,
    /// `{"review": true, "context": ...}` — Cline, which has no "deny" at all.
    ///
    /// Its two stopping shapes are `cancel`, which kills the whole task, and
    /// `review`, which pauses for a person. Estigia uses `review`: a refusal
    /// means *this write is not covered by your claim*, and a person deciding
    /// what to do about that is the right next step. Cancelling the task would
    /// throw away work over a claim that could be renewed in one command.
    ///
    /// It does mean a person can approve past it, which is the same property
    /// `git push --no-verify` already has — a guard rail working as one.
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

    /// Every dialect, checked by the compiler.
    ///
    /// This was a hand-written list inside `from_slug`, and adding `Cline`
    /// without adding it here made `--dialect cline` fall back to the default —
    /// so Cline received Claude Code's JSON, ignored it, and the gate was
    /// registered and silent. The `match` below has no wildcard: a variant added
    /// to the enum and not to this list stops the build.
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

    /// Denying, in this dialect, carrying the whole refusal.
    fn deny(self, refusal: &Refusal) -> Value {
        let reason = deny_reason(refusal);
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
    }
}

/// The text an agent is shown when a write is refused.
fn deny_reason(refusal: &Refusal) -> String {
    format!(
        "estigia refused this write.\n\n{}\n({})\n\nWhat happened to the world: {}\nReplay: {}\n\n{}",
        refusal.message,
        refusal.code,
        refusal.outcome.what_happened(),
        refusal.replay.advice("command"),
        refusal.resolution,
    )
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
    };
    let mut entry = json!({
            "at": session::now_seconds(),
            "run_id": run_id,
            "tool": tool,
            "verdict": verdict,
            "detail": detail,
            "repo_dir": context.repo_dir.display().to_string(),
            // The flag this run declared, when it declared one.
            //
            // Estigia cannot read the code, so naming a flag proves nothing
            // about what the change is behind — `out_of_phase` says so itself.
            // What makes accepting an unverifiable self-declaration reasonable
            // is the second half of that sentence: it makes the claim "explicit
            // **and recorded**, so 'we thought it was flagged' stops being
            // something anybody can say afterwards."
            //
            // It was explicit and it was not recorded. A delivery that reached
            // trunk only because `ESTIGIA_FLAG` was set left `allow — issue #N
            // is held by <run>` and no trace of the flag or that one was named,
            // which is the whole of what the record was for.
            "flag": context.flag,
    });
    // What the decision was *about*: the file, or the command. A refusal already
    // carried it, folded into the message ahead of the reason — `git push: the
    // review was published against …` — and an allow carried nothing, so the
    // record named what was stopped and not what went through. See
    // [`super::Action::subject`] for the measurement.
    if let Some(subject) = subject.filter(|subject| !subject.trim().is_empty())
        && let Value::Object(fields) = &mut entry
    {
        fields.insert("subject".to_owned(), json!(subject));
    }
    // What a push was aimed at, when the boundary was a push.
    //
    // Added rather than always present: a key that is `[]` on every one of the
    // thousands of tool calls is noise in the file somebody reads to find the
    // call that mattered.
    //
    // It changes no decision — nothing adjudicates a destination, and the
    // honesty contract records that — but it is the difference between a
    // boundary that can be audited afterwards and one that can only be spoken
    // about in the present.
    if !aimed_at.is_empty() {
        entry["aimed_at"] = json!(aimed_at);
    }
    session::record(&context.state_root, &entry);
}

/// How a run's workflow state is named in the sentence a session opens with.
///
/// Pure and fed, because the interesting input is the one that is not there and
/// a function reaching for the pointer itself cannot be shown to handle it.
///
/// `None` is not `in-progress`. A pointer written by an earlier build carries no
/// `state` and still loads — `serde` reads a missing `Option` as absent, which
/// is the whole point of the tolerance — so a run can hold an issue and have
/// never recorded what state it was in. Naming the default there tells the agent
/// the tracker said something it never said, and the third rule this crate is
/// built on is that *an unknown result is not clearance* and the nearest named
/// state is never reported instead.
///
/// The neighbouring case was already fixed here: a pointer that will not open
/// says so rather than reading as "holds no issue". This is the same fault one
/// field over, on the same surface, and the fix did not reach it.
fn state_clause(state: Option<&str>) -> String {
    match state {
        Some(state) => format!("in `{state}`"),
        None => "in a state this run never recorded".to_owned(),
    }
}

/// The context injected when a session begins.
pub fn session_start_response(context: &GateContext, run: &session::Run) -> Value {
    // Read before the issue, because a pointer that will not open answers
    // neither question and `load` reports it as `issue: None`. Said as "holds
    // no issue" it was **two** false sentences on the first thing a run reads:
    // what it holds is unknown, not nothing, and Estigia is about to gate every
    // write it makes rather than none — the gate refuses this exact state by
    // name, and did so already. A run told it is unconstrained and then stopped
    // at every write learns why only by hitting the wall.
    //
    // The same rule the gate, the push guard, `status` and `doctor` all carry:
    // unreadable is not absent. This is the surface that *announces* the state,
    // and it was the one still reading them as the same.
    let held = if run.unreadable {
        format!(
            "This run's record exists and cannot be read, so whether it holds an issue is \
             unknown. Estigia refuses every repository write from this run until that is \
             settled: read what `{}` holds from the tracker and claim it again, or take its \
             pointer away.",
            run.run_id
        )
    } else {
        match run.issue {
            Some(issue) => format!(
                "This run holds issue #{issue} {}. Estigia verifies that claim before \
                 repository writes and at every irreversible boundary; a write refused here is \
                 the tracker answering, not a suggestion.",
                state_clause(run.state.as_deref())
            ),
            None => "This run holds no issue. Estigia gates nothing until one is claimed — the \
                     oath binds once sworn."
                .to_owned(),
        }
    };

    // `references/repository-delivery.md`: "Keep the base checkout read-only.
    // One issue uses one traceable branch and one isolated worktree."
    //
    // Estigia knows both paths and could refuse the base one outright. It does
    // not, because that reference calls itself a floor the repository's own
    // rules override, and because a write to the base checkout is sometimes the
    // right thing — the gate cannot tell which, and denying on a guess is a lock
    // rather than authority.
    //
    // What it can do is say so once, in the message the agent reads before it
    // touches anything. An invariant nobody states is one nobody keeps.
    let isolation = match run.worktree.as_ref() {
        Some(worktree) => format!(
            "\n\nThis run's isolated checkout is `{}`. The base checkout stays read-only: one issue, \
             one branch, one worktree. Estigia measures writes in both and refuses neither, so this \
             one is yours to keep.",
            worktree.display()
        ),
        None => String::new(),
    };
    // A name, or the fact that there is none. `session::run_id` answers a
    // session with no identity with `<runtime>-unknown`, and this line used to
    // hand that back as though it were one — so every unidentifiable session on
    // a machine was told the same run id, and two of them swearing under it
    // means the second overwrites the first's pointer while the gate goes on
    // measuring the first's writes against the second's issue.
    //
    // Saying so beats handing out a name that is not one: an agent told it has
    // no identity can ask for one, and an agent told it is `claude-unknown`
    // cannot know to.
    if session::is_nameless(&run.run_id) {
        return json!({
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": format!(
                    "## Estigia

This session carries no identity Estigia can derive a run id from, so nothing can be sworn here: a claim needs one run to bind to, and every session in this state would share the same name. Reads and writes go through untouched.{}",
                    drift_note(context)
                ),
            }
        });
    }
    json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": format!(
                "## Estigia

Run id `{}`. {held}{isolation}{}",
                run.run_id,
                drift_note(context)
            ),
        }
    })
}

/// What to say when the installed contract is not this binary's copy.
///
/// The binary and the skill are upgraded separately, so they drift: a new
/// `estigia` beside a contract the last one wrote means the agent reads one set
/// of rules while the gate enforces another. Nothing said so unless somebody
/// thought to run `status`, and the moment they would think to is after a
/// refusal they did not expect.
///
/// Said at `SessionStart` because that is the one message the agent is
/// guaranteed to read, and because it is the only point where saying it costs
/// nothing anybody notices.
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
/// to record the first under the name of the second. Eleven agents share five
/// dialects: a call from Codex, OpenCode or Continue was written down as
/// `claude-code`, and `doctor`'s silence row — whose whole subject is *which
/// agent sent a call that went through ungated* — said `from claude-code` and
/// sent the operator to somebody else's settings file. That row's own comment
/// argued against it: *"there are eleven agents and this check reports on all of
/// them"*.
///
/// Measured: `estigia hook pre-tool-use --dialect codex` with a payload that
/// would not parse wrote `"agent":"claude-code"`, and so did `opencode` and
/// `windsurf`.
///
/// `None` when the settings file was written by a build that did not send one.
/// The record then says nothing rather than naming the dialect and calling it an
/// agent — which is the case `doctor` already reports as *"from an agent no line
/// names"*.
pub fn run_as(
    dialect: Dialect,
    agent: Option<&str>,
    event: Event,
    input: &Input,
    context: Option<&GateContext>,
) -> Value {
    let run_id = session::run_id(session::DEFAULT_RUNTIME, &input.session_id);
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
    } else if input.tool_name.trim().is_empty() {
        // A payload that parses and names nothing is exactly as unusable as one
        // that does not parse, and this recorded the second and not the first.
        //
        // Which is the shape a client version bump takes: send `toolName` where
        // this reads `tool_name` and every call passes ungated, forever, while
        // `doctor` answers *"no call has reached the gate yet — there is no
        // ledger to read"*. Not *the gate let things through*: **the gate has
        // never been called**, on a machine where it was being called for every
        // edit. Measured on the binary — three such payloads, zero lines.
        //
        // Rule 3 is untouched: this still does not deny. A schema this build
        // does not know could be wrapping `Read` as easily as `Write`. What
        // changes is that somebody can find out afterwards.
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
                // Which agent, and which hook. `doctor` reports these lines and
                // then asks the operator to work out *"whether the agent that
                // sent them is one this build knows, and whether its hook is
                // registered to send its payload"* — both answerable here, and
                // both were dropped on the floor. What five such lines on this
                // crate's own development machine say instead is `"tool": ""`
                // and a run id of `claude-unknown`, which names nobody.
                //
                // The agent only when one was sent: see [`run_as`]. The dialect
                // goes down beside it under its own name, so a line from a
                // settings file that predates `--agent` still says something
                // about where it came from without claiming to name the agent.
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
            let run = session::load(&context.state_root, &run_id);
            session_start_response(context, &run)
        }
        Event::SessionEnd => {
            // The pointer goes; the claim does not. Releasing somebody's claim
            // because their terminal closed would be Estigia deciding something
            // that belongs to the run and to the tracker.
            //
            // Unless nobody said whose session this was. `session::run_id`
            // answers a session with no identity with `<runtime>-unknown`, and
            // that is a **name two runs share** rather than a run: a payload
            // this build cannot parse produces it, and this machine's own
            // ledger carries four entries under it.
            //
            // So a `SessionEnd` with no identity used to delete
            // `claude-unknown.json` — measured, with an issue in it. If the run
            // that wrote it is still working, the gate then reads `issue: None`
            // and every write it makes goes through ungated while the tracker
            // still shows the issue held. A run that swore, reading as one that
            // never did, is the failure this whole crate is written against.
            //
            // *An unknown result is not clearance*, and what is unknown here is
            // **which run this is** — with a destructive act behind it.
            if !input.session_id.trim().is_empty() {
                session::forget(&context.state_root, &run_id);
            }
            json!({})
        }
        Event::PreToolUse => {
            // A reserved reviewer is proved before either role or repository
            // classification. `agent_type` names the caller; only the nested
            // `tool_input.subagent_type` names what this call would launch.
            if is_prelaunch_tool(agent, &input.tool_name)
                && input.launch_target() == Some("review-blind")
                && let Err(refusal) = super::roles::authorize_review_blind_launch(
                    &context.repo_dir,
                    crate::paths::home_dir().ok().as_deref(),
                )
            {
                return dialect.deny(&refusal);
            }
            // Before anything else: a sub-agent reaching past the tool list its
            // own definition declares. Checked first because it is the cheapest
            // question and the least conditional — it does not depend on a
            // claim, a state, or a window, only on what the author wrote.
            if let Some(agent) = input.agent_type.as_deref() {
                if agent == "review-blind" {
                    if let Some(refusal) = super::roles::gate(
                        Some(agent),
                        &input.tool_name,
                        Some(crate::skill::REVIEW_AGENT.contents),
                    ) {
                        return dialect.deny(&refusal);
                    }
                } else {
                    match super::roles::definition_for(
                        &context.repo_dir,
                        crate::paths::home_dir().ok().as_deref(),
                        agent,
                    ) {
                        // A definition that is there and will not open is denied
                        // rather than stepped over. Stepping over it is how the
                        // search used to behave, and what it produced was *no
                        // policy*, which this gate reads as every tool allowed —
                        // an unknown result becoming clearance at the one boundary
                        // whose subject is what a sub-agent may not do.
                        // Through the stand-down, unlike the role refusal below.
                        //
                        // The two are not the same kind of no. `tool-outside-
                        // declared-role` enforces a list the sub-agent's *author*
                        // wrote, and an operator standing Estigia's gate down does
                        // not thereby grant permissions somebody else withheld — so
                        // that one is deliberately not wrapped, and this comment is
                        // where that is said.
                        //
                        // This one is Estigia's own *an unknown result is not
                        // clearance*, and it has the shape `decide_action`'s two
                        // refusals had until a round ago: a definition file that
                        // will not open denies every call this sub-agent makes, and
                        // the one command for getting past a gate that is wrong at a
                        // bad moment did not reach it. A file on a read-only mount,
                        // or one somebody else owns, left an operator with nothing
                        // to do but stop.
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

            let (action, how) =
                classify_with(&input.tool_name, &input.tool_input, &context.boundaries);
            let mut run = session::load(&context.state_root, &run_id);
            let decision = if input.session_id.trim().is_empty() {
                // No session to mint a run id from — the same position a git
                // hook is in, and the same answer: ask the checkout.
                super::guard::decide_action(context, &context.repo_dir, &action, how)
            } else {
                super::gate(context, &mut run, &action, how)
            };
            if matches!(decision, Decision::Allow(_)) {
                // Best effort: failing to record when we last asked costs one
                // extra read, and must not turn into a denial.
                let _ = session::store(&context.state_root, &run);
            }
            note(
                context,
                &run_id,
                &input.tool_name,
                action.subject(),
                &decision,
                &[],
            );
            response_in(dialect, &decision)
        }
    }
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

    /// An ungated call records which agent sent it and which hook it was.
    ///
    /// `doctor` reports these and then asks the operator *"whether the agent
    /// that sent them is one this build knows, and whether its hook is
    /// registered to send its payload"* — two questions the writer could
    /// answer and threw away. It holds both: the dialect it is answering in,
    /// and the event it is running.
    ///
    /// It is the same defect the `why` field's own note records being fixed one
    /// field over, and this half stayed. Five such lines on the machine this
    /// crate is developed on say `"tool": ""` and a run id of
    /// `claude-unknown`, which identifies nothing at all — and the check
    /// reporting them asks a question only they could have answered.
    /// A session hook with no payload is not a call that went through ungated.
    ///
    /// The ledger recorded `payload-absent` for **every** event, and `silence`
    /// counts that verdict — so a `session-end` fired with nothing on standard
    /// input, which is the ordinary shape of that event for several agents, was
    /// reported as a call the gate let through undecided. The check whose whole
    /// subject is a silence was counting events that never asked it to decide.
    ///
    /// It is still a fault, and still recorded: with no payload there is no
    /// session id, so the run identity falls back to the shared pointer every
    /// other unidentified session gets. That is a different thing from a write
    /// nobody adjudicated, and it is spelled differently.
    #[test]
    fn a_session_hook_with_no_payload_is_not_an_ungated_call() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            stand_down: None,
            integration: crate::config::Integration::Branch,
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");
        let ledger = crate::harness::session::ledger_path(&context.state_root);

        let verdicts = |event| {
            let _ = std::fs::remove_file(&ledger);
            let _ = run_in(
                Dialect::ClaudeCode,
                event,
                &Input {
                    absent: true,
                    ..Input::default()
                },
                Some(&context),
            );
            std::fs::read_to_string(&ledger)
                .unwrap_or_default()
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
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
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
            let _ = std::fs::remove_file(crate::harness::session::ledger_path(&context.state_root));
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
            let ledger = crate::harness::session::ledger_path(&context.state_root);
            let text = std::fs::read_to_string(&ledger).expect("the ledger was written");
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
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
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

        let ledger = std::fs::read_to_string(session::ledger_path(&context.state_root))
            .expect("the ledger was written");
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
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
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
            std::fs::read_to_string(session::ledger_path(&context.state_root))
                .expect("the ledger was written")
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
            flag: Some("ff.new-checkout".to_owned()),
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
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

        let ledger = std::fs::read_to_string(session::ledger_path(&context.state_root))
            .expect("the ledger was written");
        assert!(
            ledger.contains("ff.new-checkout"),
            "the flag the run declared is nowhere in what it was allowed: {ledger}"
        );

        // And a run that declared none says so, rather than carrying somebody
        // else's name or an empty string that reads like one.
        let quiet = GateContext {
            flag: None,
            state_root: root.path().join("quiet"),
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
        let ledger = std::fs::read_to_string(session::ledger_path(&quiet.state_root))
            .expect("the ledger was written");
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
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
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

        let ledger = std::fs::read_to_string(session::ledger_path(&context.state_root))
            .expect("the ledger was written");
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

    /// Every spelling this build says it accepts is one that reaches its field.
    ///
    /// `Input` accepts seven names other agents use for fields Estigia already
    /// has — `trajectory_id` for the session, `agent_action_name` for the tool,
    /// `tool_info` and `tool_call` for its arguments, and so on. Each is a
    /// hand-written claim about somebody **else's** schema, and five of the
    /// seven were exercised by no test at all.
    ///
    /// A wrong one is silent by construction: every field carries
    /// `#[serde(default)]`, so a misspelled alias does not fail to parse — it
    /// parses into an empty tool name, and an empty tool name classifies as
    /// `Untouched`. The gate fires, decides nothing, and the only trace is the
    /// ledger line the `silence` check exists to find.
    ///
    /// This cannot tell whether the *external* spelling is right — that needs
    /// the agent's own reference, and the Codex hooks envelope is this session's
    /// reminder of what an uncrossed claim about somebody else's schema costs.
    /// What it holds is the half that is checkable: a spelling this build claims
    /// to accept is one that arrives.
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
            flag: None,
            skill_root: root.path().join("skill"),
            repo_dir: repo,
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
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

    /// The event a response declares is the event it answers.
    ///
    /// `hookSpecificOutput.hookEventName` is how Claude Code — and Codex, whose
    /// wire shape is this one byte for byte — matches a response to the hook
    /// that produced it. Measured by mutation: changing the `PreToolUse` in a
    /// **denial** to `PostToolUse` left the whole suite green. That is a refusal
    /// that carries every word of its reason, filed under an event the agent
    /// was not asking about — a gate that decides and is not read, which looks
    /// exactly like a gate that allowed.
    ///
    /// Four sites carry the field and none of them was crossed: allow, deny,
    /// and both arms of the session-start answer.
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
            replay: Replayability::NotReplayable,
            resolution: Resolution::run("estigia status"),
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
            replay: Replayability::NotReplayable,
            resolution: Resolution::run("estigia status"),
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
            flag: None,
            skill_root: std::path::PathBuf::from("estigia-no-skill-here"),
            repo_dir: std::path::PathBuf::from("estigia-no-repo-here"),
            state_root: std::path::PathBuf::from("estigia-no-state-here"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
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

    /// A session opening on a pointer nobody can read is told that, not "none".
    ///
    /// The first thing a run reads, and it carried two false sentences: `load`
    /// answers an unparseable pointer with `issue: None`, so this said *this
    /// run holds no issue* and *Estigia gates nothing until one is claimed*.
    /// What it holds is unknown, and Estigia gates **everything** — the gate
    /// refuses this exact state by name, so the run was told it was
    /// unconstrained and then stopped at every write with no way to connect the
    /// two. Measured on the product before the fix, with a truncated pointer.
    ///
    /// The rule the gate, the push guard, `status` and `doctor` all carry, on
    /// the one surface that announces the state rather than acting on it.
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
        run.issue = Some(12);
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
        run.issue = Some(12);
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

    /// What the gate let through is named, not only what it stopped.
    ///
    /// Measured on the installed binary, under one live claim: `git tag v1.0`,
    /// `gh release create v1.0` and `git push --force origin main` left three
    /// ledger lines reading `tool=Bash verdict=allow detail=issue #12 is held by
    /// claude-abcd1234` — the same sentence three times, for a tag, a release
    /// and a force-push to trunk. The **refused** versions of those same three
    /// each named their command, because a refusal is prefixed with it.
    ///
    /// So the file an operator opens to find out what happened under a claim
    /// identified everything Estigia stopped and nothing it allowed, and the
    /// allowed ones are the ones that changed the world.
    #[test]
    fn an_allowed_boundary_says_which_one_it_was() {
        let root = tempfile::tempdir().expect("a temporary root");
        let context = GateContext {
            integration: crate::config::Integration::Branch,
            flag: None,
            stand_down: None,
            skill_root: root.path().join("skill"),
            repo_dir: root.path().join("repo"),
            state_root: root.path().join("state"),
            window: super::super::RENEWAL_WINDOW,
            tracker: crate::config::Tracker::Github { repo: None },
            boundaries: Vec::new(),
        };
        std::fs::create_dir_all(&context.state_root).expect("the state directory");

        let line = |subject: Option<&str>, decision: &crate::harness::Decision| {
            let ledger = session::ledger_path(&context.state_root);
            let _ = std::fs::remove_file(&ledger);
            note(&context, "claude-abcd1234", "Bash", subject, decision, &[]);
            let text = std::fs::read_to_string(&ledger).expect("the ledger was written");
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
}
