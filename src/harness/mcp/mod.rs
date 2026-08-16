//! The MCP server: the agent calls tools instead of composing shell.
//!
//! # Why this is hand-written
//!
//! Leteo uses `rmcp`, and copying that would have been the low-risk choice for
//! a long-lived server. Estigia is not one. The same binary answers a
//! `PreToolUse` hook on **every edit**, so a `tokio` runtime spun up per process
//! is a cost paid thousands of times to move a few lines of JSON across a pipe.
//! Keeping the crate synchronous is what keeps the hook cheap, and stdio MCP
//! with a fixed tool list is a small, well-bounded subset:
//! newline-delimited JSON-RPC, four methods, one request at a time.
//!
//! What that buys is a gate that starts in milliseconds. What it costs is
//! written down in [`UNIMPLEMENTED`] rather than discovered.
//!
//! # What it does not do
//!
//! Estigia holds the tools; it does not hold authority. Every tool here reaches
//! the tracker through the transport `bindings/github.md` already names, and the
//! answer an agent gets is the tracker's, translated — never Estigia's opinion
//! about what the tracker would have said.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use super::{GateContext, session, tracker};
use crate::outcome::{Refusal, Resolution};

pub mod tools;

pub use tools::{NOT_EXPOSED, NOT_RUN, PointerEffect, TOOLS, Tool};

/// The protocol revision this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Parts of MCP this server does not implement, and what that costs.
///
/// The honesty contract for the protocol subset. An axis list that names what is
/// not covered is the difference between "it works" and "it works for what we
/// tried".
pub const UNIMPLEMENTED: &[(&str, &str)] = &[
    (
        "resources",
        "no resource is exposed; everything an agent needs arrives as a tool result",
    ),
    (
        "prompts",
        "the contract lives in the skill, which the agent reads directly",
    ),
    (
        "sampling",
        "Estigia never asks the client to run a model — it holds tools, it does not think",
    ),
    (
        "notifications beyond initialized",
        "the tool list is a compile-time constant, so there is never a list-changed to send",
    ),
    (
        "concurrent requests",
        "requests are served one at a time in arrival order; a slow tracker read blocks the next \
         call rather than interleaving with it",
    ),
];

/// Why a tool call did not produce a result.
///
/// Two shapes, kept apart because callers act on them differently: a malformed
/// call is the caller's own defect and a refusal is the world's answer. Folding
/// them together is what made `estigia claim --json` report `not_started` for
/// an ambiguous write — the taxonomy was intact one layer down and fabricated
/// one layer up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolFailure {
    /// The call could not be made: a missing argument, an unknown tool.
    Malformed(String),
    /// The call was made, or deliberately not made, and this is the answer.
    Refused(Box<Refusal>),
}

impl std::fmt::Display for ToolFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(message) => formatter.write_str(message),
            Self::Refused(refusal) => write!(formatter, "{refusal}"),
        }
    }
}

/// A JSON-RPC error code.
mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
}

/// Serves stdio until the client closes it.
pub fn serve(
    input: impl BufRead,
    mut output: impl Write,
    context: Result<GateContext, Refusal>,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Some(response) = handle_line(&line, context.as_ref()) else {
            // A notification. JSON-RPC says answer nothing at all.
            continue;
        };
        writeln!(output, "{response}")?;
        output.flush()?;
    }
    Ok(())
}

/// Answers one line, or nothing when it was a notification.
pub fn handle_line(line: &str, context: Result<&GateContext, &Refusal>) -> Option<Value> {
    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(error) => {
            return Some(error_response(
                Value::Null,
                code::PARSE_ERROR,
                &format!("{error}"),
            ));
        }
    };

    // Valid JSON is not the same as a request. `Value::get` on an array, a
    // string or a number answers `None` for every key, so a batch — or a bare
    // `"hello"` — used to reach the notification rule below and be answered
    // with silence. Silence is the one answer a client cannot act on: it waits
    // for a reply to an id it did send, and nothing ever comes.
    //
    // Batching was removed from the protocol this server speaks, so refusing
    // one is right. Refusing it *audibly* is the part that was missing.
    if !request.is_object() {
        return Some(error_response(
            Value::Null,
            code::INVALID_REQUEST,
            "a request must be a JSON object; this server does not take batches",
        ));
    }

    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or(json!({}));

    // No id means a notification: it is answered with silence, not with an
    // empty result. A client that gets a response to a notification is a client
    // that has to decide what to do with it.
    let id = id?;

    Some(match method {
        "initialize" => success(id, initialize(&params)),
        "tools/list" => success(id, json!({ "tools": tool_schemas() })),
        "tools/call" => match call(&params, context) {
            Ok(result) => success(id, result),
            Err(message) => error_response(id, code::INVALID_PARAMS, &message),
        },
        "ping" => success(id, json!({})),
        other => error_response(
            id,
            code::METHOD_NOT_FOUND,
            &format!("{other} is not a method this server implements"),
        ),
    })
}

/// The handshake.
///
/// This server answers with the one revision it implements, whatever was asked.
///
/// The doc here used to describe a negotiation — *the client's protocol version
/// is echoed back when this server knows it* — and the code under it was
/// `if requested == PROTOCOL_VERSION { requested } else { PROTOCOL_VERSION }`,
/// whose two arms are the same string. There is one version, so there is
/// nothing to choose between; the two tests below have always pinned that, and
/// it was the sentence that was wrong.
///
/// Left as one on purpose rather than widened: claiming a revision nobody has
/// run this against is the unchecked claim `setup::tests` forbids for adapters,
/// and the cost of being wrong is the same — a client that speaks it and finds
/// the tools do not work.
///
/// A client that asked for a different one is **told on stderr**. It is entitled
/// to hang up on this answer, and when it does the agent simply has no tools:
/// registered, reported on, and silent. Stderr because stdout is the protocol.
fn initialize(params: &Value) -> Value {
    if let Some(requested) = params.get("protocolVersion").and_then(Value::as_str)
        && requested != PROTOCOL_VERSION
    {
        eprintln!(
            "estigia: the client asked for MCP {requested} and this server implements \
             {PROTOCOL_VERSION}; if it hangs up, that is why and the agent has no tools"
        );
    }
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": crate::NAME,
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "Estigia holds the workflow authority. Call `claim` before any repository \
                         write; every write afterwards is verified against that claim, and every \
                         irreversible boundary re-reads the tracker timeline. A refusal here is \
                         the tracker answering, not a suggestion.",
    })
}

fn tool_schemas() -> Vec<Value> {
    TOOLS.iter().map(Tool::schema).collect()
}

/// Runs one tool from a `tools/call` request.
fn call(params: &Value, context: Result<&GateContext, &Refusal>) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call needs a name".to_owned())?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    match run_tool(name, &arguments, context) {
        Ok(result) => Ok(result),
        // A refusal is an answer, so it is a *result* an agent can act on — not
        // a protocol error, which is a code it cannot.
        Err(ToolFailure::Refused(refusal)) => Ok(refusal_result(&refusal)),
        Err(ToolFailure::Malformed(message)) => Err(message),
    }
}

/// The command line one tool call becomes, before the harness adds its own.
///
/// Its own function because it is the seam that was wrong and nothing could
/// watch: `run_tool` builds these and hands them straight to the transport, so
/// the only way to see what went out was to run one. `--fix true` went out for
/// a flag that takes no value, and every test here still passed.
/// The isolated checkout to name on a publish, when this run has one and the
/// caller did not.
///
/// **Pure and fed**, so the case can be exercised without a tracker.
///
/// `publish-review` reads the head it binds the review to from `--worktree`
/// **or, failing that, from its own working directory**, and the gate later
/// compares that head against the checkout this run covers. Two ends reading
/// two directories is a delivery refused for a verdict that never went stale —
/// the worst thing a gate can do, because the agent is stopped and nothing it
/// can do will help: the mismatch is ours.
///
/// Filled rather than overridden. A caller that named a worktree meant it, and
/// Estigia adding its own on top would be deciding where somebody else's work
/// lives.
fn worktree_to_name(tool: &tools::Tool, flags: &[String], run: &session::Run) -> Option<String> {
    // `republish-review` is here because it reads the head the same way, and
    // because it reaches it **after a force-push**. Left out, an agent calling it
    // the way it calls `publish_review` — the argument is optional on both — got
    // no worktree, `publish_with` fell back to the base checkout, and the target
    // was derived from whatever that checkout had checked out. Refs are shared
    // between git worktrees, so the branch still pushed: history was rewritten
    // and *then* the readback disagreed, answering that somebody else must have
    // pushed. Nobody had. That is the failure this function's own comment names,
    // arriving on the one route where the step before it cannot be undone.
    if !matches!(
        tool.operation,
        "publish-review" | "republish-review" | "release-ci"
    ) {
        return None;
    }
    if flags.iter().any(|flag| flag == "--worktree") {
        return None;
    }
    Some(run.worktree.as_ref()?.display().to_string())
}

fn flags_for(tool: &tools::Tool, arguments: &Value) -> Result<Vec<String>, ToolFailure> {
    // What the caller sent that this tool does not take. Checked first, because
    // this loop reads the *declared* arguments and looks each one up — so a key
    // the caller sent and this tool never heard of was not rejected, it was
    // never looked at.
    //
    // For a required argument the misspelling is caught by its own absence. For
    // an optional one nothing was caught at all: the call ran **without the
    // thing the agent asked for**, and succeeded. `list_state` is the
    // measurement, because its optional argument exists to prevent exactly this
    // — `limt: 500` runs with the default ceiling of 200 and answers as though
    // it were the whole queue, which that argument's own note calls *a partial
    // answer read as the state, the failure this crate is named for*.
    //
    // Refused rather than ignored, and the direction is the point: a schema is
    // the contract this server publishes, and a key outside it is not a lax
    // spelling of one inside it. The refusal names what the tool does take,
    // because an agent told only *no* guesses again.
    if let Some(sent) = arguments.as_object() {
        let unknown: Vec<&str> = sent
            .keys()
            .map(String::as_str)
            .filter(|key| !tool.arguments.iter().any(|argument| argument.name == *key))
            .collect();
        if !unknown.is_empty() {
            return Err(ToolFailure::Malformed(format!(
                "{} does not take {}; it takes {}",
                tool.name,
                unknown
                    .iter()
                    .map(|key| format!("`{key}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                tool.arguments
                    .iter()
                    .map(|argument| format!("`{}`", argument.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }
    let mut flags: Vec<String> = Vec::new();
    for argument in tool.arguments {
        let Some(value) = arguments.get(argument.name) else {
            if argument.required {
                return Err(ToolFailure::Malformed(format!(
                    "{} needs `{}`: {}",
                    tool.name, argument.name, argument.description
                )));
            }
            continue;
        };
        // A boolean is a flag that is there or is not there. The transport
        // declares these with `action="store_true"`, which takes no value, so
        // rendering one sent `audit-board --fix true` — and argparse answered
        // `unrecognized arguments: true` and rejected the whole call, for
        // `false` exactly as for `true`. The one argument this covers was
        // therefore unusable in both of its settings, and the agent read the
        // failure as a configuration defect.
        //
        // Absent when false, because that is what false means: `store_true`
        // has no way to be told `no` other than not being told at all.
        if argument.json_type == "boolean" {
            let Some(set) = value.as_bool() else {
                return Err(ToolFailure::Malformed(format!(
                    "`{}` is true or false, not {value}",
                    argument.name
                )));
            };
            if set && !tools::is_pointer_only(tool.name, argument.name) {
                flags.push(argument.as_flag());
            }
            continue;
        }
        // And an integer is a whole number, not a string that looks like one.
        // This was unchecked while `boolean` above was checked, so `"twelve"`
        // — and `"12"`, and `12.5` — were rendered verbatim and handed to the
        // transport, where argparse refused the whole call. The note above says
        // what that costs, in the same words and for the same reason: *the
        // agent read the failure as a configuration defect*.
        //
        // The schema is the contract this server publishes. Publishing one and
        // enforcing a looser one makes it a description, and the agent that
        // believed it gets its error from two processes away.
        // And a value the transport constrains is one of the values it takes.
        // Refused here so the agent is told which word is wrong by the server
        // that published the vocabulary, rather than by argparse two processes
        // away rejecting the whole call.
        if let Some(choices) = argument.choices
            && !value.as_str().is_some_and(|text| choices.contains(&text))
        {
            return Err(ToolFailure::Malformed(format!(
                "`{}` is one of {}, not {value}",
                argument.name,
                choices.join(", ")
            )));
        }
        if argument.json_type == "integer" && !value.is_i64() && !value.is_u64() {
            return Err(ToolFailure::Malformed(format!(
                "`{}` is a whole number, not {value}",
                argument.name
            )));
        }
        // And a whole number is not yet an issue number. The value goes into the
        // transport's argv, where a negative one is read as a **flag**:
        // `issue: -5` came back `gh issue view failed (1): unknown shorthand
        // flag: '5' in -5`. Refused here, by the server that published the
        // `minimum`, rather than arriving as a broken transport two processes
        // away — which is the cause an agent retries against.
        if let Some(least) = argument.least
            && value.as_i64().is_some_and(|number| number < least)
        {
            return Err(ToolFailure::Malformed(format!(
                "`{}` counts from {least}, and {value} is below it",
                argument.name
            )));
        }
        let rendered = match value {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            Value::Bool(flag) => flag.to_string(),
            other => {
                return Err(ToolFailure::Malformed(format!(
                    "`{}` cannot be {other}",
                    argument.name
                )));
            }
        };
        if tools::is_pointer_only(tool.name, argument.name) {
            continue;
        }
        flags.push(argument.as_flag());
        flags.push(rendered);
    }
    Ok(flags)
}

/// Runs one workflow operation.
///
/// The single implementation behind both front doors. `estigia claim` and
/// `mcp__estigia__claim` reach the world through this function, so the two
/// cannot answer differently — which they would, eventually, as two copies of
/// argument assembly, idempotency keys and pointer bookkeeping.
pub fn run_tool(
    name: &str,
    arguments: &Value,
    context: Result<&GateContext, &Refusal>,
) -> Result<Value, ToolFailure> {
    let tool = tools::find(name).ok_or_else(|| {
        let known = TOOLS
            .iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>()
            .join(", ");
        ToolFailure::Malformed(format!(
            "{name} is not a tool this server exposes; it has: {known}"
        ))
    })?;

    // Arguments are checked before anything else. A call missing a required
    // argument is malformed whether or not the harness is installed, and
    // answering it with "the harness is not installed" sends the agent to fix
    // the wrong thing.
    let mut flags = flags_for(tool, arguments)?;

    let context = match context {
        Ok(context) => context,
        // The harness is not installed. That is a tool result rather than a
        // protocol error: the agent asked a well-formed question and deserves
        // the refusal with its resolution, not a JSON-RPC code it cannot act on.
        Err(refusal) => return Err(ToolFailure::Refused(Box::new((*refusal).clone()))),
    };

    // A binding with no executable cannot be reached, and reaching for the one
    // that does exist would issue `gh` calls to a tracker that is not there —
    // answered as though they were about it. The binding's own rule: *"bindings
    // MUST declare unsupported capabilities and fail closed."*
    if context.tracker.transport().is_none() {
        return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
            "tracker-has-no-transport",
            format!(
                "`{}` has a binding the agent reads and no executable, so `{name}` cannot run against it",
                context.tracker.as_value()
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "either a tracker with a scripted binding — `estigia config set Tracker github` — or the operations run by hand from the binding this one ships",
            ),
        ))));
    }

    // A name every unidentifiable session shares is not a run to bind anything
    // to. `session::run_id` answers a session with no identity with
    // `<runtime>-unknown`, and `SessionStart` used to hand that back as though
    // it were one — so two such sessions swearing under it means the second
    // overwrites the first's pointer, and the gate goes on measuring the
    // first's writes against the second's issue.
    //
    // **Every tool that takes a `run_id`**, not only the ones that swear. The
    // first cut of this guard sat inside the `Swear` branch, and `release`
    // walked straight past it to the tracker: measured, and it failed only
    // because that fixture had no credentials. A release under a name no run
    // owns takes an issue away from whoever actually holds it.
    //
    // Which tools those are comes from the tool's own arguments, so one added
    // later is covered the day it is added rather than the day somebody
    // remembers this.
    if tool
        .arguments
        .iter()
        .any(|argument| argument.name == "run_id")
        && let Some(named) = arguments.get("run_id").and_then(Value::as_str)
        && session::is_nameless(named)
    {
        return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
            "run-id-names-no-run",
            format!(
                "{named} is the name every session without an identity is given, so `{}` under it \
                 is bound to no run",
                tool.name
            ),
            // Two situations, and one of them had no way out at all.
            //
            // A **live** session with no identity is asking for one, and
            // `status` is where it looks. But a pointer already written under
            // this name is an orphan: nothing will ever release it, because
            // this refusal is what stops that — and meanwhile it declares a
            // checkout, so the guard gates every write in there against an
            // issue nobody holds.
            //
            // `status` prints that run and tells the operator *`estigia release
            // --run-id <id>` puts one down*; this used to answer that by
            // sending them back to `status`. Two commands pointing at each
            // other and neither discharging anything, which is the one thing
            // the ratchet forbids.
            //
            // So when the pointer is there, the way out is the file — Estigia's
            // own state, and no claim of anybody's, the same words
            // `run-pointer-unreadable` already uses. The tracker's side is
            // separate and stays the tracker's: whoever holds the issue there
            // is whoever the timeline says, and this name is not evidence
            // about that.
            {
                let orphan = crate::harness::session::pointer_path(&context.state_root, named);
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    if orphan.is_file() {
                        format!(
                            "that pointer taken away \u{2014} {} is Estigia's own state and no \
                             claim of anybody's; who holds the issue is read from the tracker",
                            orphan.display()
                        )
                    } else {
                        "a session Estigia can derive a run id from \u{2014} `estigia status` \
                         says which agents it holds the tools for"
                            .to_owned()
                    },
                )
            },
        ))));
    }

    // A pointer that is on disk and will not parse says a run under this name
    // existed; it does not say what it swore. The gate refuses every write in
    // that state, on the directive every agent is given — *an unknown result is
    // not clearance* — and this refuses every tool call in it for the same
    // reason.
    //
    // It was here already and only for the swearing tools, inside the branch
    // below. That is the shape the guard three screens up records having been
    // fixed out of, in its own words: *the first cut of this guard sat inside
    // the `Swear` branch, and `release` walked straight past it to the tracker.*
    // The lesson was written down and not carried to the guard beside it, so a
    // run whose record cannot be read could still put an issue down — and what
    // it was putting down is the fact nothing could read.
    //
    // Derived from the tool's own arguments, like the guard above it, so a tool
    // added later is covered the day it is added.
    let names_a_run = tool
        .arguments
        .iter()
        .any(|argument| argument.name == "run_id");
    if names_a_run {
        let named = arguments
            .get("run_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let holding = session::load(&context.state_root, named);
        if holding.unreadable {
            return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
                "run-pointer-unreadable",
                format!(
                    "{named}: this run's record exists and cannot be read, so whether it holds \
                     an issue is unknown"
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "what that run holds, read from the tracker \u{2014} then claimed again, or \
                     its pointer removed",
                ),
            ))));
        }
    }

    // Incident I02: a five-minute dev loop re-entered selection while already
    // holding an issue and nearly started additional work on every tick. The
    // contract answers it in prose — *"Resume this run's held issue before
    // consulting queues"* — and prose is what the loop had. A run that holds
    // one issue cannot swear to a second.
    //
    // Re-claiming the *same* issue is a renewal and passes: the transport
    // dedupes it on the operation id this run already minted.
    if matches!(tool.effect, PointerEffect::Swear) {
        let holder = arguments
            .get("run_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let wanted = arguments.get("issue").and_then(Value::as_u64);
        let holding = session::load(&context.state_root, holder);
        // The unreadable case is refused above, for every tool rather than only
        // for these: reading `.issue` straight off a record that will not parse
        // answers `None`, and the one-issue-at-a-time rule below would simply
        // not run.
        let held = holding.issue;
        if let (Some(held), Some(wanted)) = (held, wanted)
            && held != wanted
        {
            return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
                "already-holding",
                format!("{holder} already holds issue #{held} and cannot also swear to #{wanted}"),
                Resolution::run(format!("estigia release --run-id {holder}")),
            ))));
        }
    }

    // The operation-scoped writes need an idempotency key the transport can
    // dedupe on: `claim`, `reclaim`, and `unassign` below.
    let run_id = arguments
        .get("run_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let issue = arguments.get("issue").and_then(Value::as_u64).unwrap_or(0);
    let mut run = session::load(&context.state_root, run_id);

    // The run id is the only identity a tool call carries, and the caller
    // supplies it. `estigia mcp` takes no arguments and the protocol brings no
    // session, so nothing here knows *which* run is asking — it knows only the
    // name it was handed. A name is an assertion, and this product's first rule
    // is that a claim is adjudicated rather than asserted.
    //
    // The guard above covers the one name every unidentified session shares. It
    // says what the general case costs, in its own words: *a release under a
    // name no run owns takes an issue away from whoever actually holds it.* An
    // id belonging to another live run does exactly that, and it is discoverable
    // — the ledger carries run ids, and so does every claim comment.
    //
    // What can be checked is the same fact the gate already checks one file
    // over: a claim covers the checkout it was made in and the isolated one this
    // run was given, and a call from anywhere else is not covered by it. The
    // rule was stated there and not carried here, so a tool call was measured
    // against a claim a write in the same directory would not have been.
    //
    // Only when the pointer names a checkout: a run that has claimed nothing yet
    // has nothing to be outside of, which is the `claim` call itself.
    if run.covered().count() > 0
        && !run
            .covered()
            .any(|covered| crate::paths::covers(covered, &context.repo_dir))
    {
        return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
            "run-id-names-another-checkout",
            format!(
                "{run_id} holds a claim over {}, and this server is running in {} \u{2014} so \
                 `{}` under that name is not this run's to make",
                run.covered()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" and "),
                context.repo_dir.display(),
                tool.name
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "this run's own id, the one `SessionStart` reported \u{2014} a call under \
                 another run's name ends that run's claim rather than this one's",
            ),
        ))));
    }

    if matches!(tool.operation, "claim" | "reclaim") {
        let key = match run.operation_id.clone() {
            // Reused on retry, exactly as the transport documents: a fresh key
            // would turn a retry into a second claim event.
            Some(existing) if run.issue == Some(issue) => existing,
            _ => {
                let fresh = session::mint_operation_id(run_id, issue);
                let key = fresh.clone();
                // Through `update`, because a dropped write here mints a new key
                // next time — and a retry under a new key is the second claim
                // event the key exists to prevent.
                run = session::update(&context.state_root, run_id, move |run| {
                    run.operation_id = Some(key.clone());
                });
                fresh
            }
        };
        flags.push("--operation-id".to_owned());
        flags.push(key);
        flags.push("--runtime".to_owned());
        flags.push(session::DEFAULT_RUNTIME.to_owned());
    }
    if tool.operation == "unassign" {
        // Reused on retry, for the reason the `claim` arm above gives: the
        // transport answers a repeated release from the marker already on the
        // issue, and only if the id is the same one.
        //
        // Kept **only** when it belongs to the issue the pointer holds. The
        // field is one key, not one per issue, so storing a key minted for a
        // different number wrote over the one the held issue's retry depends on
        // — measured: a run holding #12 asked to release #99 came back with its
        // #12 key replaced, before the transport had been reached at all. A
        // later, legitimate retry of the real release then arrives under a new
        // id, and the dedupe the comment above describes cannot see it as a
        // repeat.
        //
        // Not refused on the pointer, though: the pointer is a note about what
        // was last read and **none of it is authority** — the timeline decides
        // whether this run may release that issue, and refusing here would be
        // local state overruling the tracker. So the call goes through with a
        // fresh key that is simply not written down.
        let holds_it = run.issue == Some(issue);
        let key = match run.release_id.clone() {
            Some(existing) if holds_it => existing,
            _ => {
                let fresh = session::mint_operation_id(run_id, issue);
                if holds_it {
                    let key = fresh.clone();
                    run = session::update(&context.state_root, run_id, move |run| {
                        run.release_id = Some(key.clone());
                    });
                }
                fresh
            }
        };
        flags.push("--operation-id".to_owned());
        flags.push(key);
        flags.push("--runtime".to_owned());
        flags.push(session::DEFAULT_RUNTIME.to_owned());
    }
    if tool.operation == "handoff-review" {
        let field = |name: &str| {
            arguments
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or_default()
        };
        let pr = arguments
            .get("pr")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            .to_string();
        let key = crate::transport::claim::review_operation_id(
            "review-handoff",
            &[
                run_id,
                field("target_operation"),
                field("epoch"),
                &pr,
                field("head"),
                field("base"),
                field("digest"),
                field("blocker"),
                field("discharger"),
            ],
        );
        flags.push("--operation-id".to_owned());
        flags.push(key);
        flags.push("--runtime".to_owned());
        flags.push(session::DEFAULT_RUNTIME.to_owned());
    }
    if tool.operation == "review-verdict" {
        let field = |name: &str| {
            arguments
                .get(name)
                .and_then(Value::as_str)
                .unwrap_or_default()
        };
        let pr = arguments
            .get("pr")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            .to_string();
        let key = crate::transport::claim::review_operation_id(
            "review-verdict",
            &[
                run_id,
                field("reviewer"),
                field("epoch"),
                &pr,
                field("head"),
                field("base"),
                field("digest"),
                field("outcome"),
            ],
        );
        flags.push("--operation-id".to_owned());
        flags.push(key);
    }

    // The isolated checkout, when this run has one and the caller did not name
    // it. `publish-review` reads the head it binds the review to from
    // `--worktree` **or, failing that, from its own working directory** — and
    // the gate later compares that head against the checkout this run covers.
    // Two ends reading two directories is a delivery refused for a verdict that
    // never went stale, which is the worst thing a gate can do: the agent is
    // stopped and nothing it can do will help, because the mismatch is ours.
    //
    // Filled rather than overridden. A caller that named a worktree meant it,
    // and Estigia adding its own on top would be deciding where somebody else's
    // work lives.
    if let Some(named) = worktree_to_name(tool, &flags, &run) {
        flags.push("--worktree".to_owned());
        flags.push(named);
    }

    // Answered in this process. Every one of these used to build a command line
    // and spawn `python <skill>/scripts/github.py`, so an agent's call went
    // agent → tool → interpreter → `gh`, with two implementations of the same
    // decisions alive at once and only one of them running. The flags are
    // unchanged — they were already the transport's own — and what moved is who
    // reads them.
    //
    // `GH_REPO` is set the way the spawn set it, and for the same reason: `gh`
    // infers the repository from the checkout's remote, which is right until an
    // operator says otherwise.
    let transport = crate::transport::Context::live(
        context.skill_root.clone(),
        context.repo_dir.clone(),
        context.tracker.named_repo(),
    );
    // Told to `gh` the way the spawn told it, and only when the operator named
    // one. It was an environment variable on the child; here the child is every
    // `gh` this call runs, so it is set on this process instead.
    //
    // Safe in this one place and nowhere else: this server is a process of its
    // own that serves **one request at a time in arrival order** — its own
    // `UNIMPLEMENTED` list says so — and the value it writes is the same on
    // every call, read from a configuration that does not change while it runs.
    if let Some(named) = transport.repo.as_deref() {
        // SAFETY: single-threaded server, one request at a time, idempotent value.
        unsafe { std::env::set_var("GH_REPO", named) };
    }
    let answered = crate::transport::dispatch::dispatch(
        &transport,
        tool.operation,
        &flags,
        &session::stamp_of(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default(),
        ),
    );
    let answer = match answered {
        Ok(value) => tracker::Answer {
            code: 0,
            body: Some(value),
        },
        Err(failure) => tracker::Answer {
            code: failure.code(),
            body: Some(failure.envelope()),
        },
    };
    if let Some(refusal) = tracker::translate(&answer, tool.name) {
        return Err(ToolFailure::Refused(Box::new(refusal)));
    }

    // A pointer keyed by nothing is a file nobody reads and a state the real run
    // never learns. `transition` used to land here with an empty run-id: the
    // issue moved, the pointer for `""` recorded it, and the run that made the
    // move went on believing the old state — which is what `verify_claim
    // --expect-state` is then measured against.
    // Whether what just happened on the tracker is also on disk here. It is not
    // a reason to call the operation failed — the tracker write landed and
    // saying otherwise would send an agent to repeat a claim that exists — but
    // it has to be said, because the gate reads the pointer and not the tracker.
    // With the pointer unwritten, a run that swore reads as a run that swore
    // nothing, and every write after it goes through ungated.
    //
    // `session::update` answered the same whether the store worked or not, so
    // this was reported as an ordinary success. An unknown result is not
    // clearance, and this one was not even unknown: it was known and dropped.
    let recorded = if run_id.trim().is_empty() {
        None
    } else {
        Some(apply_effect(
            tool,
            arguments,
            answer.body.as_ref(),
            &mut run,
            context,
        ))
    };
    let mut text = serde_json::to_string_pretty(&answer.body.unwrap_or(json!({"ok": true})))
        .unwrap_or_else(|_| "{}".to_owned());
    if recorded == Some(false) && tool.effect != PointerEffect::None {
        text.push_str(&format!(
            "

estigia: this happened on the tracker and could not be written to this run's              pointer at {}. The gate reads that pointer, so until it is written the gate              adjudicates as though this call had not been made.",
            session::pointer_path(&context.state_root, run_id).display()
        ));
    }
    Ok(json!({
        "content": [{
            "type": "text",
            "text": text,
        }],
        "isError": false,
    }))
}

/// Moves the run pointer to follow what just became true.
fn apply_effect(
    tool: &Tool,
    arguments: &Value,
    body: Option<&Value>,
    run: &mut session::Run,
    context: &GateContext,
) -> bool {
    // A two-step operation answers its first call with `write_performed: false`
    // and asks to be repeated naming what it found. Nothing happened, so nothing
    // about the run changed — and `release` moved on it: the discovery call
    // forgot the pointer, taking the issue, the worktree the gate watches and
    // the last verification with it, while the tracker still showed the run
    // holding the issue and no release had been written at all.
    //
    // `reclaim` is the same door and the worse side of it: its effect is
    // `Swear`, so the read-only discovery call set the issue, the state and the
    // verification stamp the renewal window rides on. A run that had taken over
    // nothing read as the *verified* holder of an issue somebody else still
    // held, and its repository writes went through the gate on it.
    if body.is_some_and(|body| body.get("write_performed") == Some(&json!(false))) {
        // Nothing was asked of the pointer, so nothing failed to be written to
        // it. `true` here means *the record agrees with what happened*, which
        // for a call that did nothing it does.
        return true;
    }
    let effect = tool.effect;
    if effect == PointerEffect::Forget {
        // Removing the file is the record: if it is gone, the run holds
        // nothing, which is what was asked for. A removal that fails leaves a
        // pointer claiming an issue that was put down — reported through the
        // ordinary reader rather than here, because `release` is the one path
        // where a stale pointer is *tighter* than the truth and the gate errs
        // toward asking.
        session::forget(&context.state_root, &run.run_id);
        return true;
    }
    let issue = arguments.get("issue").and_then(Value::as_u64);
    let state = arguments
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("in-progress")
        .to_owned();
    let to = arguments
        .get("to")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let published_head = body
        .and_then(|body| body.get("head"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let worktree = body
        .and_then(|body| body.get("worktree"))
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from);
    let repo_dir = context.repo_dir.clone();

    // Against the newest pointer on disk rather than the one this call loaded.
    // A hook storing in between used to make this write vanish, taking the issue
    // or the isolated checkout with it — and a run whose worktree the gate has
    // forgotten delivers through it ungated.
    let (updated, on_disk) =
        session::updated(&context.state_root, &run.run_id, |run| match effect {
            PointerEffect::None | PointerEffect::Forget => {}
            PointerEffect::Swear => {
                run.issue = issue;
                run.state = Some(state.clone());
                run.repo_dir = Some(repo_dir.clone());
                run.mark_verified();
            }
            PointerEffect::Moved => {
                if let Some(to) = &to {
                    run.state = Some(to.clone());
                }
                run.mark_verified();
            }
            PointerEffect::Renew => run.mark_verified(),
            PointerEffect::Published => {
                // Kept only when the answer names one. A publish that came back
                // without a head is not a reason to forget the head this run had.
                if let Some(head) = &published_head {
                    run.reviewed_head = Some(head.clone());
                }
                run.mark_verified();
            }
            PointerEffect::Isolated => {
                // Where the work actually happens. Without this the gate watches the
                // checkout the claim was made in, which is the one directory the run
                // does not edit — and the whole delivery goes through ungated.
                run.worktree = worktree.clone();
                run.mark_verified();
            }
        });
    *run = updated;
    on_disk
}

/// A refusal as a tool result, carrying everything needed to act on it.
///
/// `isError` is set so the agent knows the operation did not happen, and the
/// text carries the code, what happened to the world, whether a replay is safe,
/// and the resolution — the same four things the hook's denial carries, because
/// an agent reading either one needs the same four things.
fn refusal_result(refusal: &Refusal) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": format!(
                "{}\n({})\n\nWhat happened to the world: {}\nReplay: {}\n\n{}",
                refusal.message,
                refusal.code,
                refusal.outcome.what_happened(),
                refusal.replay.advice("call"),
                refusal.resolution,
            ),
        }],
        "isError": true,
    })
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests;
