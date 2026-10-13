//! The MCP server: the agent calls tools instead of composing shell.
//!
//! Synchronous stdio JSON-RPC, one request at a time, because this binary
//! also answers a `PreToolUse` hook on every edit. Estigia holds the tools,
//! not the authority — every tool reaches the tracker through the transport,
//! and the answer is the tracker's, translated. What this protocol subset
//! does not implement is in [`UNIMPLEMENTED`].

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use super::{GateContext, session, tracker};
use crate::outcome::{Refusal, Resolution};

pub mod tools;

pub use tools::{NOT_EXPOSED, NOT_RUN, PointerEffect, TOOLS, Tool};

/// The protocol revision this server implements.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Parts of MCP this server does not implement, and what that costs.
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
/// A malformed call is the caller's own defect; a refusal is the world's
/// answer. Folding the two together once made `estigia claim --json` report
/// `not_started` for an ambiguous write.
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
            // A notification: JSON-RPC says answer nothing.
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

    // A batch, or any non-object value, would otherwise fall through to the
    // notification rule below and get silence — the one answer a client
    // waiting on an id it sent can never act on.
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

    // No id means a notification: silence, not an empty result — there is no
    // id for a client to match a response against.
    let id = id?;

    Some(match method {
        "initialize" => success(id, initialize(&params)),
        "tools/list" => success(id, json!({ "tools": tool_schemas(context) })),
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

/// The handshake. Answers with the one revision this server implements,
/// whatever was asked — there is nothing to negotiate. A client that asked
/// for something else is told on stderr, since stdout is the protocol; it
/// is entitled to hang up, and the agent then simply has no tools.
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
            "version": crate::build_info::VERSION,
        },
        "instructions": "Estigia holds the workflow authority. Call `claim` before any repository \
                         write; every write afterwards is verified against that claim, and every \
                         irreversible boundary re-verifies it against the local store. A refusal here \
                         is the store answering, not a suggestion.",
    })
}

/// The tools this process can call. The context is fixed for the process, so
/// the list never changes under a client; a context that cannot be read lists
/// everything, and a hidden name still dispatches and refuses by its own rule.
fn tool_schemas(context: Result<&GateContext, &Refusal>) -> Vec<Value> {
    let workflow = context.ok().and_then(|context| context.workflow);
    TOOLS
        .iter()
        .filter(|tool| match workflow {
            Some(crate::config::Workflow::Assisted) => !tools::writes_the_tracker(tool.name),
            Some(_) => !tools::is_assisted_only(tool.name),
            None => true,
        })
        .map(Tool::schema)
        .collect()
}

fn call(params: &Value, context: Result<&GateContext, &Refusal>) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call needs a name".to_owned())?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    match run_tool(name, &arguments, context) {
        Ok(result) => Ok(result),
        Err(ToolFailure::Refused(refusal)) => Ok(refusal_result(&refusal)),
        Err(ToolFailure::Malformed(message)) => Err(message),
    }
}

/// The isolated checkout to name on a publish, when this run has one and the
/// caller did not — filled rather than overridden, because a caller that
/// named a worktree meant it.
fn worktree_to_name(tool: &tools::Tool, flags: &[String], run: &session::Run) -> Option<String> {
    if !matches!(
        tool.operation,
        "publish-review" | "republish-review" | "release-ci" | "unassign"
    ) {
        return None;
    }
    if flags.iter().any(|flag| flag == "--worktree") {
        return None;
    }
    Some(run.worktree.as_ref()?.display().to_string())
}

fn flags_for(tool: &tools::Tool, arguments: &Value) -> Result<Vec<String>, ToolFailure> {
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
        if let Value::Array(items) = value {
            let mut rendered: Vec<&str> = Vec::with_capacity(items.len());
            for item in items {
                let Some(text) = item.as_str() else {
                    return Err(ToolFailure::Malformed(format!(
                        "`{}` is a list of strings, not {item}",
                        argument.name
                    )));
                };
                if text.trim().is_empty() {
                    return Err(ToolFailure::Malformed(format!(
                        "`{}` lists an empty name, and every slot it names has to say something",
                        argument.name
                    )));
                }
                rendered.push(text);
            }
            if rendered.is_empty() {
                return Err(ToolFailure::Malformed(format!(
                    "`{}` is empty; name at least one slot",
                    argument.name
                )));
            }
            if !tools::is_pointer_only(tool.name, argument.name) {
                flags.push(argument.as_flag());
                flags.push(rendered.join(","));
            }
            continue;
        }
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

/// The run a tool call names, read from the argument rather than from the
/// flag it renders to.
struct NamedRun<'a> {
    id: &'a str,
    /// The runtime `session::run_id`'s own shape says this id names — the
    /// value every `--runtime` below projects, derived from the validated id
    /// rather than split out of it again.
    runtime: &'a str,
}

/// Reads the run id a call carries, refusing every shape [`session::run_id`]
/// could not have minted. `create`'s declared runtime is held to the same
/// component check by [`declared_runtime`], which [`run_tool`] calls beside
/// this one rather than inside it.
///
/// The type is part of the shape: a `run_id` sent as a JSON number used to
/// skip all four refusals at once, because every one of them read
/// `Value::as_str` and answered `None` for it. A refusal read from the
/// argument, not from the string a flag happens to render, cannot be skipped
/// by a type.
fn named_run<'a>(
    tool: &Tool,
    arguments: &'a Value,
    context: &GateContext,
) -> Result<Option<NamedRun<'a>>, ToolFailure> {
    if !tool
        .arguments
        .iter()
        .any(|argument| argument.name == "run_id")
    {
        return Ok(None);
    }
    let Some(sent) = arguments.get("run_id") else {
        return Ok(None);
    };
    let Some(id) = sent.as_str() else {
        return Err(ToolFailure::Malformed(format!(
            "`run_id` is this run's id, a string \u{2014} not {sent}"
        )));
    };
    match session::run_id_shape(id) {
        session::RunIdShape::Named { runtime } => Ok(Some(NamedRun { id, runtime })),
        session::RunIdShape::Nameless { .. } => Err(run_id_names_no_run(
            id,
            format!(
                "{id} is the name every session without an identity is given, so `{}` under it \
                 is bound to no run",
                tool.name
            ),
            context,
        )),
        session::RunIdShape::Unminted(why) => Err(run_id_names_no_run(
            id,
            unminted_run_id(id, why, tool),
            context,
        )),
    }
}

/// Why an id is not a shape [`session::run_id`] mints, in the words the
/// caller is given.
fn unminted_run_id(id: &str, why: session::Unminted, tool: &Tool) -> String {
    match why {
        session::Unminted::NoRuntime => format!(
            "{id} carries no `-`, so it names no runtime to attribute a `dev:` label to \u{2014} \
             `{}` under it is bound to no run",
            tool.name
        ),
        session::Unminted::Runtime => format!(
            "{id} does not carry a runtime of lowercase `a-z0-9` groups joined by single `-` \
             before its dash, so it cannot be attributed to any runtime \u{2014} `{}` under it \
             is bound to no run",
            tool.name
        ),
        session::Unminted::Suffix => format!(
            "{id} does not end in what `SessionStart` mints after a runtime \u{2014} a dashless \
             suffix of up to eight alphanumerics followed by eight lowercase hex digits \u{2014} \
             so it is not an id this crate could have minted and `{}` under it is bound to no run",
            tool.name
        ),
    }
}

/// The one refusal every unmintable id gets, with the way out that fits this
/// machine's state.
fn run_id_names_no_run(id: &str, what: String, context: &GateContext) -> ToolFailure {
    ToolFailure::Refused(Box::new(Refusal::not_started(
        "run-id-names-no-run",
        what,
        {
            let orphan = session::pointer_path(&context.state_root, id);
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                // A live session with no identity should ask `status`; a
                // pointer already written under this name is an orphan
                // nothing will ever release, so the way out there is
                // deleting Estigia's own file — never a claim about who
                // holds the issue on the tracker.
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
    )))
}

/// The refusal `create` gets for a runtime that is not a component its
/// `analyst:` label can name.
pub(crate) fn runtime_names_no_runtime(runtime: &str) -> Refusal {
    Refusal::not_started(
        "runtime-names-no-runtime",
        format!(
            "{runtime} is not a runtime a label can name \u{2014} `dev:` and `analyst:` \
             labels are lowercase `a-z0-9` groups joined by single `-`, and this one would \
             be a repository-level label nobody's query finds"
        ),
        Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "this run's own runtime word \u{2014} the one `SessionStart`'s run id begins \
             with, before its last dash",
        ),
    )
}

/// Holds `create`'s declared runtime to [`session::runtime_component`] — the
/// same predicate that validates the runtime derived from a run id — and
/// refuses a value that is not one, rather than minting a repository-level
/// `analyst:` label from caller text.
fn declared_runtime(arguments: &Value) -> Result<(), ToolFailure> {
    let Some(sent) = arguments.get("runtime") else {
        return Ok(());
    };
    let Some(runtime) = sent.as_str() else {
        return Err(ToolFailure::Malformed(format!(
            "`runtime` is the runtime word filing this finding, a string \u{2014} not {sent}"
        )));
    };
    session::runtime_component(runtime)
        .map_err(|_| ToolFailure::Refused(Box::new(runtime_names_no_runtime(runtime))))
}

/// The refusal a deliberately unexposed operation answers with.
///
/// It carries both halves the agent needs: why no tool exists, and where the
/// contract carries the operation — so a boundary reads as a boundary rather
/// than as the answer for a typo.
pub fn operation_has_no_tool(name: &str, reason: &str) -> Refusal {
    Refusal::not_started(
        "operation-has-no-tool",
        format!("`{name}` is not a tool this server exposes, by design: {reason}"),
        Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "the operation itself, performed by hand from the contract section named above \
             rather than through this server",
        ),
    )
}

/// Runs one workflow operation — the single implementation behind both
/// `estigia claim` and `mcp__estigia__claim`, so the two front doors cannot
/// answer differently.
pub fn run_tool(
    name: &str,
    arguments: &Value,
    context: Result<&GateContext, &Refusal>,
) -> Result<Value, ToolFailure> {
    if let Some(reason) = tools::unexposed_reason(name) {
        return Err(ToolFailure::Refused(Box::new(operation_has_no_tool(
            name, reason,
        ))));
    }
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

    let mut flags = flags_for(tool, arguments)?;

    let context = match context {
        Ok(context) => context,
        Err(refusal) => return Err(ToolFailure::Refused(Box::new((*refusal).clone()))),
    };

    if context.workflow == Some(crate::config::Workflow::Assisted)
        && tools::writes_the_tracker(name)
    {
        return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
            "workflow-assisted-writes-no-tracker",
            format!(
                "`{name}` writes the tracker, and the `Workflow` row is `assisted` in this \
                 repository's own file: the team coordinates who takes an issue, and Estigia \
                 writes nothing to it"
            ),
            Resolution::run("estigia config set Workflow autonomous --repo"),
        ))));
    }

    // Per operation, not per installation: a tracker without a
    // transport must not refuse a host-bound tool, and the reverse. The two
    // assisted-only tools are exempt for a read-only tracker: they
    // read the issue through `transport::linear` and write nothing to it, so
    // `Tracker linear`'s absent tracking transport is not in their way.
    match tool.binds() {
        tools::TransportBind::Tracker
            if context.tracker.transport().is_none()
                && !(context.tracker.is_read_only() && tools::ASSISTED_ONLY.contains(&name)) =>
        {
            if context.tracker.is_read_only() {
                return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
                    "read-only-tracker-admits-only-assisted",
                    format!(
                        "`{}` is read-only and admits only assisted work, so `{name}` cannot run \
                         against it",
                        context.tracker.as_value()
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::OperatorKnowledge,
                        "`Tracker linear` is supported only under `Workflow: assisted`, where the \
                         issue is read and nothing is written; autonomous work needs a claim \
                         Linear cannot provide",
                    ),
                ))));
            }
            return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
                "tracker-has-no-transport",
                format!(
                    "`{}` has a binding the agent reads and no executable, so `{name}` cannot run \
                     against it",
                    context.tracker.as_value()
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "either a tracker with a scripted binding — `estigia config set Tracker \
                     github` — or the operations run by hand from the binding this one ships",
                ),
            ))));
        }
        tools::TransportBind::RepositoryHost if context.repository_host.transport().is_none() => {
            return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
                "repository-host-has-no-transport",
                format!(
                    "`{}` is the repository host and has no executable, so `{name}` cannot run against it",
                    context.repository_host.as_value()
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "a repository host with a scripted binding — `estigia config set \"Repository host\" github`",
                ),
            ))));
        }
        _ => {}
    }

    // Every tool that takes a `run_id` is read here, typed and held to the
    // one shape `session::run_id` mints — the tool table decides *which*
    // tools, so a tool added later is covered the day it is added, and the
    // runtime below is derived from this answer rather than split again.
    let named = named_run(tool, arguments, context)?;
    if tool.operation == "create" {
        declared_runtime(arguments)?;
    }

    let names_a_run = tool
        .arguments
        .iter()
        .any(|argument| argument.name == "run_id");
    if names_a_run {
        let named = named.as_ref().map_or("", |run| run.id);
        let holding = session::load(&context.state_root, named);
        // A pointer that is on disk and will not parse says a run under this
        // name existed; it does not say what it swore. Refused rather than
        // read past, on the same rule every agent is given: an unknown
        // result is not clearance.
        if holding.unreadable {
            let what = match holding.unreadable_reason.as_deref() {
                Some(reason) => format!(
                    "{named}: this run's record exists and cannot be read, so whether it holds \
                     an issue is unknown \u{2014} {reason}"
                ),
                None => format!(
                    "{named}: this run's record exists and cannot be read, so whether it holds \
                     an issue is unknown"
                ),
            };
            return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
                "run-pointer-unreadable",
                what,
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "what that run holds, read from the tracker \u{2014} then claimed again, or \
                     its pointer removed",
                ),
            ))));
        }
    }

    if tools::is_assisted_only(name) && context.workflow != Some(crate::config::Workflow::Assisted)
    {
        return Err(ToolFailure::Refused(Box::new(match context.workflow {
            None => Refusal::not_started(
                "workflow-not-assisted",
                format!(
                    "`{name}` is for repositories whose `Workflow` row is `assisted`, and the \
                     contract could not be read, so no row is known to say so"
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "the contract readable again \u{2014} `estigia doctor` names the row it \
                     could not parse",
                ),
            ),
            Some(_) => Refusal::not_started(
                "workflow-not-assisted",
                format!(
                    "`{name}` is for repositories whose `Workflow` row is `assisted`, and this \
                     one is not: work here goes through a claim"
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "the person who owns this repository setting `Workflow` to `assisted` \
                     with `estigia config set Workflow assisted --repo`",
                ),
            ),
        })));
    }

    // The tracker decides where the issue is read. `assist`'s
    // issue is a string, so a number and a key both arrive; a shape that does
    // not match the configured tracker is refused here rather than letting
    // `assist` pick its source from the argument alone, and an argument that is
    // not an identity at all is refused rather than passed through unread.
    if name == "assist"
        && let Some(value) = arguments.get("issue")
    {
        let not_matching = |detail: String| {
            ToolFailure::Refused(Box::new(Refusal::not_started(
                "assist-issue-does-not-match-the-tracker",
                detail,
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "an issue number under `Tracker github`, or a Linear key such as `ENG-123` \
                     under `Tracker linear`",
                ),
            )))
        };
        match serde_json::from_value::<crate::identity::Identity>(value.clone()) {
            Ok(identity) if identity.is_key() != context.tracker.is_read_only() => {
                return Err(not_matching(format!(
                    "`assist` was handed {}, and this repository's `Tracker` is `{}`",
                    if identity.is_key() {
                        "a Linear key"
                    } else {
                        "an issue number"
                    },
                    context.tracker.as_value()
                )));
            }
            Ok(_) => {}
            Err(_) => {
                return Err(not_matching(format!(
                    "`assist` was handed {value}, which is not an issue identity"
                )));
            }
        }
    }

    // A run that holds one issue cannot swear to a second. Re-claiming the
    // *same* issue is a renewal and passes: the transport dedupes it on the
    // operation id this run already minted.
    if matches!(tool.effect, PointerEffect::Swear | PointerEffect::Assisted) {
        let holder = named.as_ref().map_or("", |run| run.id);
        let wanted = arguments.get("issue").and_then(|value| {
            serde_json::from_value::<crate::identity::Identity>(value.clone()).ok()
        });
        let holding = session::load(&context.state_root, holder);
        let held = holding.issue;
        if let (Some(held), Some(wanted)) = (held, wanted)
            && held != wanted
        {
            let way_out = if tool.effect == PointerEffect::Assisted {
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    format!(
                        "that run's pointer taken away \u{2014} {} is Estigia's own state, and \
                         `release` writes the tracker, which this repository does not",
                        session::pointer_path(&context.state_root, holder).display()
                    ),
                )
            } else {
                Resolution::run(format!("estigia release --run-id {holder}"))
            };
            return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
                "already-holding",
                format!("{holder} already holds issue #{held} and cannot also swear to #{wanted}"),
                way_out,
            ))));
        }
    }

    let run_id = named.as_ref().map_or("", |run| run.id);
    let runtime = named.as_ref().map_or("", |run| run.runtime);
    let issue = arguments.get("issue").and_then(Value::as_u64).unwrap_or(0);
    let mut run = session::load(&context.state_root, run_id);

    // `repo_dir`, not `covered().count()` — the worktree is the *additional*
    // directory a claim covers, never the one that says where it was sworn.
    // A run that has claimed nothing yet has nothing to be outside of, which
    // is the `claim` call itself, so this only fires once a pointer names a
    // checkout.
    if run.repo_dir.is_some()
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

    let mut claim_key_reused = false;
    // Recorded here rather than searched for later by matching the
    // `"--operation-id"` string in `flags`, which could also match a flag
    // *value* equal to that string.
    let mut operation_id_flag_index = None;
    if matches!(tool.operation, "claim" | "reclaim") {
        let key = match run.operation_id.clone() {
            // Reused on retry: a fresh key would turn a retry into a second
            // claim event.
            Some(existing) if run.issue_number() == Some(issue) => {
                claim_key_reused = true;
                existing
            }
            _ => {
                let fresh = session::mint_operation_id(run_id, issue);
                let key = fresh.clone();
                run = session::update(&context.state_root, run_id, move |run| {
                    run.operation_id = Some(key.clone());
                });
                fresh
            }
        };
        operation_id_flag_index = Some(flags.len());
        flags.push("--operation-id".to_owned());
        flags.push(key);
        flags.push("--runtime".to_owned());
        flags.push(runtime.to_owned());
    }
    if tool.operation == "unassign" {
        // The stored key is kept only when it belongs to the issue the
        // pointer holds — the field is one key, not one per issue, so a key
        // minted for a different number would overwrite the one the held
        // issue's retry depends on. Not refused on the pointer, though: it
        // is a note about what was last read, not authority, so the call
        // still goes through with a fresh key that is simply not recorded.
        let holds_it = run.issue_number() == Some(issue);
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
        flags.push(runtime.to_owned());
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
        flags.push(runtime.to_owned());
    }
    // The finding's own identity (`id`, `class`) is in the key, so two
    // different findings against one receipt mint two operations rather than
    // one replay — and re-recording the *same* finding after an unreadable
    // answer mints the same key and reads back what it already wrote.
    if tool.operation == "review-finding" {
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
        let receipt = crate::transport::claim::ReviewTransaction {
            epoch: field("epoch").to_owned(),
            pr: pr.parse().unwrap_or_default(),
            head: field("head").to_owned(),
            base: field("base").to_owned(),
            digest: field("digest").to_owned(),
        };
        let key = crate::transport::claim::finding_operation_id(
            run_id,
            field("reviewer"),
            &receipt,
            field("id"),
            field("class"),
        );
        flags.push("--operation-id".to_owned());
        flags.push(key);
    }
    // The batch's own body (`outcomes`) is deliberately left out of the key,
    // the same reason `review-finding`'s `evidence`/`impact` are: one batch
    // per receipt, so a retry that supplies a corrected batch reads back the
    // first one rather than minting a second operation over it.
    if tool.operation == "review-refutation" {
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
            "review-refutation",
            &[
                run_id,
                field("refuter"),
                field("epoch"),
                &pr,
                field("head"),
                field("base"),
                field("digest"),
            ],
        );
        flags.push("--operation-id".to_owned());
        flags.push(key);
    }
    if tool.operation == "review-lens-report" {
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
        let receipt = crate::transport::claim::ReviewTransaction {
            epoch: field("epoch").to_owned(),
            pr: pr.parse().unwrap_or_default(),
            head: field("head").to_owned(),
            base: field("base").to_owned(),
            digest: field("digest").to_owned(),
        };
        let key =
            crate::transport::claim::lens_report_operation_id(run_id, field("role"), &receipt);
        flags.push("--operation-id".to_owned());
        flags.push(key);
    }
    if tool.operation == "review-validator-result" {
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
        let attempt = match field("inspection") {
            "unavailable" => field("attempt").to_owned(),
            _ => "conclusive".to_owned(),
        };
        let key = crate::transport::claim::review_operation_id(
            "review-validator-result",
            &[
                run_id,
                field("epoch"),
                &pr,
                field("head"),
                field("base"),
                field("digest"),
                &attempt,
            ],
        );
        flags.push("--operation-id".to_owned());
        flags.push(key);
    }
    if tool.operation == "review-computed-verdict" {
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
            "review-computed-verdict",
            &[
                run_id,
                field("epoch"),
                &pr,
                field("head"),
                field("base"),
                field("digest"),
            ],
        );
        flags.push("--operation-id".to_owned());
        flags.push(key);
    }
    // The population is the key's last field, unlike `review-refutation`
    // above: a batch answering a **wider** set of findings than one already
    // recorded is a new operation, so a finding recorded after the first batch
    // can still be answered; the words themselves are out, so a retry of one
    // population reads back the first batch's answers rather than minting a
    // second event over it.
    if tool.operation == "review-disposition" {
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
        let answers = crate::transport::claim::disposition_answers(field("dispositions"));
        let key = crate::transport::claim::review_operation_id(
            "review-disposition",
            &[
                run_id,
                field("epoch"),
                &pr,
                field("head"),
                field("base"),
                field("digest"),
                &answers,
            ],
        );
        flags.push("--operation-id".to_owned());
        flags.push(key);
    }
    // `conditions` joins the key for the same reason `id` does above: two
    // measurements against one receipt that vary only the conditions must
    // mint two operations, never a replay of one another.
    if tool.operation == "spend" {
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
            "spend",
            &[
                run_id,
                field("phase"),
                field("epoch"),
                &pr,
                field("head"),
                field("base"),
                field("digest"),
                field("conditions"),
            ],
        );
        flags.push("--operation-id".to_owned());
        flags.push(key);
    }

    if tool.operation == "draft-pr" {
        let (Some(branch), Some(worktree), true) = (&run.branch, &run.worktree, run.assisted)
        else {
            return Err(ToolFailure::Refused(Box::new(Refusal::not_started(
                "draft-pr-run-not-assisted",
                format!(
                    "{run_id} has no local claim from `assist` naming a branch and a checkout, so \
                     there is no branch of this run's to publish"
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "`assist` called first, under this run's own id, for the issue the person named",
                ),
            ))));
        };
        flags.push("--branch".to_owned());
        flags.push(branch.clone());
        flags.push("--worktree".to_owned());
        flags.push(worktree.display().to_string());
    }

    if let Some(named) = worktree_to_name(tool, &flags, &run) {
        flags.push("--worktree".to_owned());
        flags.push(named);
    }

    let named_repo = match tool.binds() {
        tools::TransportBind::RepositoryHost => context.repository_host.named_repo(),
        tools::TransportBind::Tracker => context.tracker.named_repo(),
    };
    let transport = crate::transport::Context::live(
        context.skill_root.clone(),
        context.repo_dir.clone(),
        named_repo,
    );
    // Tracker and host may name different repos; a prior tool's
    // pin must not stick when this one has none. Held
    // through the dispatch so a concurrent clear cannot land mid-spawn.
    let _gh_pin = super::gh_repo_pin();
    super::apply_gh_repo_locked(transport.repo.as_deref());
    let now_stamp = || {
        session::stamp_of(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default(),
        )
    };
    let mut answered =
        crate::transport::dispatch::dispatch(&transport, tool.operation, &flags, &now_stamp());

    // A reused key that names a lapsed claim of this run's own is not a
    // conflict to report — it is the case `claim` exists to serve, one call
    // away. Minting a fresh key and reissuing once turns
    // `claim-operation-expired` into the result the caller actually wants.
    if should_reissue_claim(tool.operation, claim_key_reused, &answered) {
        let fresh = session::mint_operation_id(run_id, issue);
        let Some(position) = operation_id_flag_index else {
            unreachable!(
                "should_reissue_claim answered true for a \"claim\" call with no \
                 --operation-id flag recorded to rewrite"
            );
        };
        flags[position + 1] = fresh.clone();
        run = session::update(&context.state_root, run_id, move |run| {
            run.operation_id = Some(fresh.clone());
        });
        answered =
            crate::transport::dispatch::dispatch(&transport, tool.operation, &flags, &now_stamp());
    }
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
    if let Some(failure) = refusal_from_answer(tool, &answer, &mut run, context) {
        return Err(failure);
    }

    // A pointer keyed by nothing is a file nobody reads and a state the real
    // run never learns — `transition` used to land here with an empty
    // run-id, and the pointer for `""` recorded a move the real run went on
    // believing had not happened.
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
    let mut text = answer_text(answer.body);
    // The tracker write landed, so this is not a reason to call the
    // operation failed — but the gate reads the pointer and not the tracker,
    // so an unwritten one has to be said: `session::update` used to answer
    // the same whether the store worked or not, reporting this as an
    // ordinary success when it was a known and dropped failure.
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

/// The text of a successful tool answer.
///
/// Compact, not indented: the model reading a tool result pays for every
/// byte. The CLI's non-`--json` road re-indents this same string for a
/// terminal rather than relying on this one; see
/// [`crate::cli::indented_for_a_person`].
fn answer_text(body: Option<serde_json::Value>) -> String {
    let mut body = body.unwrap_or(json!({"ok": true}));
    if let Value::Object(fields) = &mut body {
        fields.insert(
            "build".to_owned(),
            json!({
                "commit": crate::build_info::commit(),
                "dirty": crate::build_info::dirty(),
            }),
        );
    }
    serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned())
}

/// Whether a `claim` that just answered `claim-operation-expired` is worth
/// reissuing under a freshly minted key.
///
/// `tests.rs`'s `a_fresh_key_is_never_reissued_as_though_it_had_lost_a_race`
/// is what turns a dropped `claim_key_reused` conjunct here red.
fn should_reissue_claim(
    operation: &str,
    claim_key_reused: bool,
    answered: &Result<Value, crate::transport::Failure>,
) -> bool {
    operation == "claim"
        && claim_key_reused
        && matches!(
            answered,
            Err(failure)
                if failure.envelope().get("reason").and_then(Value::as_str)
                    == Some("claim-operation-expired")
        )
}

/// Turns the transport's dispatch answer into the MCP refusal and applies
/// the local consequence of an uncertain publication.
fn refusal_from_answer(
    tool: &Tool,
    answer: &tracker::Answer,
    run: &mut session::Run,
    context: &GateContext,
) -> Option<ToolFailure> {
    let refusal = tracker::translate(answer, tool.name)?;
    // A write that landed or may have landed can have minted a new epoch over
    // the same PR and HEAD. Keeping the old receipt would let that stale
    // epoch be spent before the remote is reconciled.
    if tool.effect == PointerEffect::Published && !refusal.outcome.is_clean() {
        let (updated, _) = session::updated(&context.state_root, &run.run_id, |run| {
            run.review_transaction = None;
            run.reviewed_head = None;
        });
        *run = updated;
    }
    Some(ToolFailure::Refused(Box::new(refusal)))
}

/// Moves the run pointer to follow what just became true.
pub(crate) fn apply_effect(
    tool: &Tool,
    arguments: &Value,
    body: Option<&Value>,
    run: &mut session::Run,
    context: &GateContext,
) -> bool {
    // A two-step operation answers its first call with `write_performed:
    // false` and asks to be repeated naming what it found — nothing
    // happened, so nothing about the run changed. `reclaim`'s discovery call
    // used to fall through this and write the renewal effect anyway, making
    // a run that had taken over nothing read as the verified holder of an
    // issue somebody else still held.
    if body.is_some_and(|body| body.get("write_performed") == Some(&json!(false))) {
        return true;
    }
    let effect = tool.effect;
    if effect == PointerEffect::Forget {
        // Removing the file is the record: if it is gone, the run holds
        // nothing, which is what was asked for.
        session::forget(&context.state_root, &run.run_id);
        return true;
    }
    let issue = arguments
        .get("issue")
        .and_then(|value| serde_json::from_value::<crate::identity::Identity>(value.clone()).ok());
    // Kept as an `Option` beside the defaulted `state` below, because
    // `release_ci` carries neither. Writing the default down for it would stamp
    // a pointer with a state nobody named.
    let named_state = arguments
        .get("transition_to")
        .or_else(|| arguments.get("state"))
        .or_else(|| arguments.get("expect_state"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let state = named_state
        .clone()
        .unwrap_or_else(|| "in-progress".to_owned());
    let to = arguments
        .get("to")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let receipt_from = |value: &Value| {
        let receipt = crate::transport::claim::ReviewTransaction {
            epoch: value.get("epoch")?.as_str()?.to_owned(),
            pr: value.get("pr")?.as_u64()?,
            head: value.get("head")?.as_str()?.to_owned(),
            base: value.get("base")?.as_str()?.to_owned(),
            digest: value.get("digest")?.as_str()?.to_owned(),
        };
        receipt.is_complete().then_some(receipt)
    };
    let published_receipt = body.and_then(receipt_from);
    let supplied_receipt = receipt_from(arguments);
    let claimed_receipt = body
        .and_then(|body| body.get("review_transaction"))
        .and_then(receipt_from);
    let worktree = body
        .and_then(|body| body.get("worktree"))
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from);
    let branch = arguments
        .get("branch")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let repo_dir = context.repo_dir.clone();

    // The checkout a resumed run adopted rather than built with
    // `start_branch`, resolved once out here — because it runs `git` and
    // reads a file, and `session::updated`'s closure has to stay pure: it
    // may run twice, against whatever a racing writer left on disk.
    let discovered_worktree = if run.worktree.is_none()
        && matches!(effect, PointerEffect::Swear | PointerEffect::Renew)
    {
        // A renewal is a repair, so it trusts the pointer's own issue over
        // the call's arguments; a claim is what says what the run is about
        // *now*, so it trusts the arguments first.
        match effect {
            PointerEffect::Swear => issue.clone().or_else(|| run.issue.clone()),
            _ => run.issue.clone().or_else(|| issue.clone()),
        }
        .and_then(|target| {
            let transport = crate::transport::Context::live(
                context.skill_root.clone(),
                repo_dir.clone(),
                context.tracker.named_repo(),
            );
            crate::transport::branch::discover_adopted_worktree(&transport, &target)
                .ok()
                .flatten()
        })
        // `RegisteredWorktree::path` is git's own porcelain spelling,
        // unresolved; `start_branch` records a different string for the same
        // directory. Put through the same helper before being written down,
        // so the two never disagree about how one checkout is spelled.
        .and_then(|discovered| crate::transport::branch::canonical_worktree_path(&discovered).ok())
    } else {
        None
    };

    // Against the newest pointer on disk rather than the one this call
    // loaded — a hook storing in between would otherwise make this write
    // vanish, taking the issue or the isolated checkout with it.
    let (updated, on_disk) = session::updated(&context.state_root, &run.run_id, |run| {
        match effect {
            PointerEffect::None | PointerEffect::Forget => {}
            PointerEffect::Swear => {
                run.issue = issue.clone();
                run.state = Some(state.clone());
                run.repo_dir = Some(repo_dir.clone());
                run.assisted = false;
                run.branch = None;
                // `get_or_insert_with`, never assignment: a pointer that
                // already names a checkout is describing something this call
                // has no business replacing.
                if let Some(discovered) = &discovered_worktree {
                    run.worktree.get_or_insert_with(|| discovered.clone());
                }
                if let Some(receipt) = &claimed_receipt {
                    run.review_transaction = Some(receipt.clone());
                    run.reviewed_head = None;
                }
                run.mark_verified();
            }
            PointerEffect::Moved => {
                if issue.is_some() && issue == run.issue {
                    if let Some(to) = &to {
                        run.state = Some(to.clone());
                    }
                    run.mark_verified();
                }
            }
            PointerEffect::Renew => {
                // A reviewer claiming after a handoff receives the receipt in
                // the CI-release call; preserved here so the
                // recreated pointer can gate delivery from the inherited
                // worktree after the publisher's own pointer was removed.
                if tool.name == "release_ci"
                    && let Some(receipt) = &supplied_receipt
                {
                    run.review_transaction = Some(receipt.clone());
                    run.reviewed_head = None;
                }
                // Every field below is filled and none overwritten: this is
                // the way back for a run that has already been stranded, and
                // a pointer that already names an issue, a state or a
                // checkout is describing something this call has no
                // business replacing.
                if run.issue.is_none() {
                    run.issue = issue.clone();
                }
                if run.state.is_none()
                    && let Some(named) = &named_state
                {
                    run.state = Some(named.clone());
                }
                run.repo_dir.get_or_insert_with(|| repo_dir.clone());
                if let Some(discovered) = &discovered_worktree {
                    run.worktree.get_or_insert_with(|| discovered.clone());
                }
                run.mark_verified();
            }
            PointerEffect::Published => {
                // Supersedes old authority even when the answer is too
                // incomplete to manufacture a new receipt.
                run.review_transaction = None;
                run.reviewed_head = None;
                if let Some(receipt) = &published_receipt {
                    run.review_transaction = Some(receipt.clone());
                }
                run.mark_verified();
            }
            PointerEffect::Assisted => {
                run.issue = issue.clone();
                run.state = Some(state.clone());
                run.repo_dir = Some(repo_dir.clone());
                run.worktree = worktree.clone();
                run.branch = branch.clone();
                run.assisted = true;
            }
            PointerEffect::Isolated => {
                run.worktree = worktree.clone();
                // Also fills `repo_dir` when the pointer does not already
                // name one: a `claim` whose write landed but whose readback
                // failed returns before this runs, leaving a pointer with no
                // `repo_dir` — and from there `start_branch` would narrow
                // this run's coverage to only the new worktree, refusing
                // every later call `run-id-names-another-checkout` with no
                // way back.
                run.repo_dir.get_or_insert_with(|| repo_dir.clone());
                run.mark_verified();
            }
        }
        if run.assisted {
            run.verified_at = None;
        }
    });
    *run = updated;
    on_disk
}

/// A refusal as a tool result, carrying the same four things the hook's
/// denial carries: the code, what happened to the world, whether a replay is
/// safe, and the resolution.
fn refusal_result(refusal: &Refusal) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": refusal.agent_text("call"),
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
