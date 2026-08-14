//! What each agent's configuration file has to look like.
//!
//! Split from the adapter table because they answer different questions. That
//! one says *which* agent and *where*; this one says what its settings file
//! must contain and, just as importantly, what of somebody else's must survive
//! the edit. Keeping them together grew one file past a thousand lines and put
//! two unrelated reasons to change in the same place.
//!
//! Every function here obeys invariant two: it replaces what Estigia owns and
//! writes everything else back. Where the format has no room for a marker — a
//! JSON object, a TOML section — the entry is lifted out by name instead.

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use super::{AgentAdapter, InstructionFile};
use crate::harness::hook::Dialect;

/// How one agent registers a hook that can deny a tool call.
///
/// Three agents can; none spells it the same way. Two share the settings-file
/// envelope and differ only in the event name, so they share a renderer; Cursor
/// keeps its own file with its own shape and gets its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GateSpec {
    /// The events that fire before a tool runs.
    ///
    /// A slice, because one agent needs more than one. Windsurf has no matcher
    /// and a separate event per kind of action, so registering only
    /// `pre_write_code` would gate edits and leave every shell command through —
    /// which is X05 in the incident ledger, a shell walking around a write
    /// boundary, arriving by way of a table with room for one entry.
    pub events: &'static [&'static str],
    /// The tools worth waking a process for, when the agent filters.
    pub matcher: Option<&'static str>,
    /// How this agent reads a decision.
    pub dialect: Dialect,
    /// The shape of the file the entry goes in.
    pub envelope: Envelope,
    /// Whether the event name arrives where a tool name would.
    ///
    /// Most agents send a tool name and use the event only to say *when*.
    /// Windsurf has one event per kind of action and sends its name in that
    /// field, so the event **is** the tool as far as the classifier is
    /// concerned — and an event the classifier does not know is a gate that
    /// fires, reaches the classifier and stands aside.
    ///
    /// It sits on the spec rather than on the dialect because it describes how
    /// *this agent fills its payload*, not how it reads an answer. Crush shares
    /// the exit-code dialect and sends a real tool name; putting this on the
    /// dialect would have made the guard demand `PreToolUse` be a tool the
    /// classifier knows, which it is not and must not be.
    pub event_is_the_tool_name: bool,
    /// Whether this agent has the lifecycle events beyond the tool gate.
    ///
    /// Declared, not inferred. It was inferred — "settings envelope **and**
    /// Claude Code dialect" — which held only for as long as Codex happened to
    /// use a different envelope. The day Codex's envelope was corrected to the
    /// one it actually reads, that coincidence ended and Codex silently began
    /// receiving `SessionStart` and the rest: entries nobody runs, in somebody
    /// else's settings file.
    ///
    /// Two true facts multiplied into a third that was never true. A property
    /// of the agent belongs on the agent.
    pub lifecycle: bool,
}

/// The shapes a hooks file comes in.
///
/// Not one abstraction with branches — concrete files that happen to hold the
/// same idea. Naming the difference is what stops one agent's entry being
/// written into another's envelope, where it parses and never fires.
///
/// Codex used to be the argument for that and is now the argument against
/// guessing: it was given a shape of its own, `{"<Event>": [...]}` with no
/// wrapper, on a claim nobody crossed against Codex — which refuses that file
/// outright and will not start. It reads `Settings`, like Claude Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Envelope {
    /// `{"hooks": {"<Event>": [{matcher, hooks: [{type, command, timeout}]}]}}`
    /// — Claude Code and Gemini CLI.
    Settings,
    /// `{"version": 1, "hooks": {"<event>": [{command, timeout}]}}` — Cursor,
    /// with the command straight on the entry.
    Cursor,
    /// `{"hooks": {"<event>": [{command, powershell}]}}` — Windsurf, whose entry
    /// carries the command twice: `command` is run through `bash -c`, and
    /// `powershell` is what Windows uses when it is there.
    ///
    /// Both are written with the same string. Estigia's command is a quoted
    /// absolute path and flags, which both shells read the same way — and
    /// writing only one of them would leave the gate registered and silent on
    /// half the platforms.
    Windsurf,
    /// `{"hooks": {"<Event>": [{name, matcher, command, timeout}]}}` — Crush,
    /// which keeps its hooks inside the same `crush.json` as everything else and
    /// puts the matcher on the entry rather than beside it.
    Crush,
}

/// A file Estigia was asked to edit and cannot.
///
/// Typed rather than a message, because what it carries is a taxonomic
/// difference: nothing was written, the operator owns the file, and no command
/// Estigia can name will fix it. It used to arrive as a generic setup failure,
/// which is reported with an **unknown** outcome and answered with `estigia
/// status` — a command that reports the same thing again and discharges
/// nothing. Naming a dead end is the one thing the ratchet forbids.
#[derive(Debug, thiserror::Error)]
pub enum NotEditable {
    /// The file is not JSON at all.
    #[error("{path} is not JSON: {detail}")]
    Unparseable {
        /// The file that was read.
        path: String,
        /// What the parser said.
        detail: String,
    },
    /// The file is JSON, and not the shape this envelope needs.
    #[error("{path} must contain a JSON object")]
    NotAnObject {
        /// The file that was read.
        path: String,
    },
    /// A value *inside* the file is not the shape this envelope needs.
    ///
    /// The same situation as [`Self::NotAnObject`], one level down, and it used
    /// to be reported as a different kind of thing entirely: a generic setup
    /// failure, with an unknown outcome, answered with `estigia status`. An
    /// operator who wrote `"PreToolUse": "..."` by hand was told to run a
    /// command that reports the same file again and changes nothing — the dead
    /// end the variants above exist to avoid, reached by the path they did not
    /// cover.
    #[error("{what} in {path} must be {shape}")]
    WrongShape {
        /// The key, as it is written in the file.
        what: String,
        /// The file that was read.
        path: String,
        /// What that key has to hold.
        shape: &'static str,
    },
}

/// The JSON object in `existing`, or an empty one when there is nothing there.
///
/// One reader for the three envelopes. It was written out three times, and the
/// third copy is where a fourth would have gone.
///
/// Returns the map rather than a `Value`, so that no caller has to promise the
/// shape a second time — a promise is what an `expect` is, and this function
/// already made it.
fn object_at(path: &Path, existing: Option<&str>) -> Result<Map<String, Value>> {
    let root: Value = match existing {
        Some(text) if !text.trim().is_empty() => {
            serde_json::from_str(without_mark(text)).map_err(|error| NotEditable::Unparseable {
                path: path.display().to_string(),
                detail: error.to_string(),
            })?
        }
        _ => Value::Object(Map::new()),
    };
    match root {
        Value::Object(map) => Ok(map),
        _ => Err(NotEditable::NotAnObject {
            path: path.display().to_string(),
        }
        .into()),
    }
}

impl Envelope {
    /// The keys this envelope carries as scaffolding rather than as content.
    ///
    /// Cursor's file holds a `version` beside its hooks, and Estigia writes it —
    /// into a file it created, and into one that arrived without it. So removing
    /// the last hook leaves `{"version": 1, "hooks": {}}`, which read as
    /// substance and was kept: a file Estigia made, surviving the uninstall that
    /// is meant to be its exact inverse.
    ///
    /// Declared beside the code that writes it, so the two cannot drift.
    pub(super) fn scaffolding(self) -> &'static [&'static str] {
        match self {
            Self::Cursor => &["version"],
            Self::Windsurf | Self::Crush => &[],
            Self::Settings => &[],
        }
    }
}

impl AgentAdapter {
    /// How this agent registers a tool-call gate, if it can have one.
    ///
    /// Every entry comes from that agent's published hooks reference. An agent
    /// missing from here is one with no documented event that can deny — which
    /// has turned out to be a smaller set each time it was checked, so it is
    /// stated as what is known rather than as a property of the agent.
    pub(super) fn gate_spec(&self) -> Option<GateSpec> {
        match self.instructions {
            InstructionFile::ClaudeCode => Some(GateSpec {
                events: &["PreToolUse"],
                // `Update` is here because `WRITE_TOOLS` already says this
                // agent writes through it, and the two must not disagree: a
                // name the classifier judges and no matcher wakes for is a rule
                // this crate believes in and has arranged never to be asked
                // about. It was the only one — every other classifier name a
                // matcher leaves out belongs to Windsurf or OpenCode, which
                // carry no matcher at all.
                //
                // Named rather than dropped from the classifier, because the
                // two mistakes do not cost the same. If the tool does not
                // exist, a matcher naming it wakes for nothing and a classifier
                // knowing it judges nothing. If it does, leaving it out is
                // every write through it going ungated. The Continue entry
                // below drops `NotebookEdit` on *positive* knowledge that the
                // tool is absent; there is no such knowledge here, and absent
                // that, the answer that cannot open a hole wins.
                matcher: Some("Edit|Write|MultiEdit|NotebookEdit|Update|Bash"),
                dialect: Dialect::ClaudeCode,
                envelope: Envelope::Settings,
                event_is_the_tool_name: false,
                // The only one. See the field's note: this used to be worked
                // out from the envelope and the dialect, and stopped being true
                // the moment another agent shared both.
                lifecycle: true,
            }),
            // Verified from `codex-rs/hooks/src/schema.rs`: its
            // `PreToolUseHookSpecificOutputWire` is `hookEventName`,
            // `permissionDecision` and `permissionDecisionReason` under
            // `hookSpecificOutput` — **byte for byte Claude Code's shape**. So
            // it shares the dialect rather than carrying a second copy of one
            // fact; what differs is the file, and only the file.
            //
            // The envelope was `Bare` — the events at the top level, no
            // wrapper — on a claim recorded here confidently and never crossed
            // against the thing it described. Codex refuses that file outright,
            // and the refusal names the whole schema:
            //
            //   failed to parse hooks config ~/.codex/hooks.json:
            //   unknown field `PreToolUse`, expected `description` or `hooks`
            //
            // `codex-cli 0.146.0`'s own binary carries the serde field lists
            // that say the rest: the top level is `description` and `hooks`,
            // `hooks` maps an event to a `MatcherGroup` of `matcher` and
            // `hooks`, and each handler is internally tagged on `type` with
            // `command` and `timeout`. That is `Settings`, exactly — so Codex
            // shares Claude Code's envelope as well as its dialect, and this
            // adapter differs from it in nothing but which file it writes.
            //
            // Worse than a gate that never fires, which is what the note below
            // warns about: this one stopped **Codex itself** from starting.
            //
            // The matcher was `^Bash$` for one round, on a published claim that
            // `PreToolUse` intercepts the shell tool only. The source says
            // otherwise: `core/src/hook_runtime.rs` blocks `Bash`,
            // `apply_patch` **and any other tool by name**, and
            // `core/src/tools/hook_names.rs` gives the canonical set —
            // `apply_patch` carrying the aliases `Write` and `Edit`, which is
            // how Codex writes files. Gating shell alone would have installed a
            // gate that watches the one thing a delivery does least.
            InstructionFile::Codex => Some(GateSpec {
                events: &["PreToolUse"],
                matcher: Some("^(Bash|apply_patch|Write|Edit)$"),
                dialect: Dialect::ClaudeCode,
                envelope: Envelope::Settings,
                event_is_the_tool_name: false,
                lifecycle: false,
            }),
            InstructionFile::GeminiCli => Some(GateSpec {
                events: &["BeforeTool"],
                // Gemini's matcher is a regular expression over tool names, and
                // its tools are named differently from Claude Code's.
                matcher: Some("edit|write_file|replace|run_shell_command"),
                dialect: Dialect::GeminiCli,
                envelope: Envelope::Settings,
                event_is_the_tool_name: false,
                lifecycle: false,
            }),
            // Verified from `packages/core/src/hooks/types.ts` and its published
            // reference. A Gemini CLI fork that has **diverged where it counts**:
            // the event is `PreToolUse`, not Gemini's `BeforeTool`. Assuming the
            // fork inherited it — which is exactly what "probably shares it" would
            // have meant — would have written an event that never fires.
            //
            // `HookDecision` is `'ask' | 'block' | 'deny' | 'approve' | 'allow'`
            // with a `reason`, which is Gemini's dialect, so that is shared.
            // The matcher takes pipe-separated exact tool names.
            InstructionFile::Qwen => Some(GateSpec {
                events: &["PreToolUse"],
                matcher: Some("write_file|replace|edit|run_shell_command"),
                dialect: Dialect::GeminiCli,
                envelope: Envelope::Settings,
                event_is_the_tool_name: false,
                lifecycle: false,
            }),
            // Verified in `internal/hooks/hooks.go` and `internal/config/config.go`:
            // `PreToolUse`, a regex matcher against the tool name, and **exit 2
            // blocks the tool call**. Its payload is `{event, session_id, cwd,
            // tool_name, tool_input}` — the shape Estigia already reads, with no
            // renaming at all, which is why this adapter is a table entry rather
            // than a translation layer.
            // Its own hooks module says so: "these types match the exact
            // schemas from Claude Code so that any hook written for `claude`
            // works with `cn` out of the box". Checked piece by piece rather
            // than taken on that word — the events, the envelope, the dialect
            // and the tool names all match.
            //
            // `NotebookEdit` is left out because Continue has no such tool;
            // Claude Code's matcher would still work, and naming a tool that
            // does not exist is the kind of thing somebody later reads as
            // evidence that it does.
            InstructionFile::Continue => Some(GateSpec {
                events: &["PreToolUse"],
                matcher: Some("Edit|Write|MultiEdit|Bash"),
                dialect: Dialect::ClaudeCode,
                envelope: Envelope::Settings,
                event_is_the_tool_name: false,
                lifecycle: false,
            }),
            InstructionFile::Crush => Some(GateSpec {
                events: &["PreToolUse"],
                // Its own names, lower case: `bash`, `edit`, `write`,
                // `multiedit`. The classifier already knows all four, because
                // Claude Code spells them the same way in title case and the
                // classifier normalises.
                matcher: Some("edit|write|multiedit|bash"),
                dialect: Dialect::ExitCode,
                envelope: Envelope::Crush,
                event_is_the_tool_name: false,
                lifecycle: false,
            }),
            InstructionFile::Windsurf => Some(GateSpec {
                // Two, because Windsurf has no matcher and one event per kind of
                // action. Both, or a shell walks around the write boundary.
                events: &["pre_write_code", "pre_run_command"],
                matcher: None,
                dialect: Dialect::ExitCode,
                envelope: Envelope::Windsurf,
                event_is_the_tool_name: true,
                lifecycle: false,
            }),
            InstructionFile::Cursor => Some(GateSpec {
                events: &["preToolUse"],
                matcher: None,
                dialect: Dialect::Cursor,
                envelope: Envelope::Cursor,
                event_is_the_tool_name: false,
                lifecycle: false,
            }),
            _ => None,
        }
    }

    /// Why this agent has no tool-call gate, when it has none.
    ///
    /// The honesty contract, said where somebody reads it rather than only in a
    /// README they will not open while wondering why a write went through. Each
    /// line says what is missing and what would close it, because "not
    /// supported" is a dead end and this project does not ship those.
    pub fn gate_gap(&self) -> Option<&'static str> {
        if self.can_gate_tools() {
            return None;
        }
        Some(match self.instructions {
            InstructionFile::Neutral => {
                "the agent-neutral root is a convention for where skills live. It is not an agent, has no tool loop, and there is nothing here to gate — which is why it is the only entry without one."
            }
            _ => "no published reference for a deny-capable hook has been read for this agent.",
        })
    }
}

/// The name the server is registered under, in every dialect.
pub const SERVER_NAME: &str = crate::NAME;

/// How an agent spells an MCP server entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum McpFormat {
    /// `mcpServers: { estigia: { command, args } }`
    McpServers,
    /// `mcp: { estigia: { type: "stdio", command, args } }` — Crush, which uses
    /// OpenCode's key with Claude Code's entry plus a required `type`.
    CrushStdio,
    /// `mcp: { estigia: { type: "local", command: [...] } }`
    Local,
    /// `[mcp_servers.estigia]` in TOML.
    CodexToml,
}

impl AgentAdapter {
    /// The MCP dialect this agent's configuration file speaks.
    pub(super) fn mcp_format(&self) -> McpFormat {
        match self.instructions {
            InstructionFile::Codex => McpFormat::CodexToml,
            InstructionFile::OpenCode => McpFormat::Local,
            InstructionFile::Crush => McpFormat::CrushStdio,
            _ => McpFormat::McpServers,
        }
    }

    /// Whether `setup` can register the MCP server for this agent.
    ///
    /// Two say no for the same reason: there is no file Estigia has verified as
    /// the one this agent reads its servers from. The neutral root is not an
    /// agent at all, and Windsurf's location has not been checked against a real
    /// installation. An MCP entry written into the wrong file is a server that
    /// never starts under a `status` line saying it did — which is worse than
    /// saying plainly that the tools are not wired up here.
    pub fn supports_mcp(&self) -> bool {
        !matches!(
            self.instructions,
            InstructionFile::Neutral
                | InstructionFile::Windsurf
                | InstructionFile::Cline
                | InstructionFile::Continue
        )
    }
}

/// Writes Estigia's MCP entry, keeping every other server.
pub(super) fn render_mcp(
    path: &Path,
    existing: Option<&str>,
    executable: &Path,
    format: McpFormat,
) -> Result<String> {
    let command = executable.to_str().with_context(|| {
        format!(
            "the Estigia executable path is not valid UTF-8: {}",
            executable.display()
        )
    })?;

    if format == McpFormat::CodexToml {
        return Ok(render_codex_mcp(existing.unwrap_or(""), command));
    }

    let mut root = Value::Object(object_at(path, existing)?);
    let Value::Object(object) = &mut root else {
        unreachable!("object_at returns a map")
    };
    // Crush shares OpenCode's key and not its entry: same `mcp`, different
    // shape inside. Keeping the two apart is what stops one being written in
    // the other's form, which parses and never starts.
    let key = if matches!(format, McpFormat::Local | McpFormat::CrushStdio) {
        "mcp"
    } else {
        "mcpServers"
    };
    let servers = object
        .entry(key)
        .or_insert_with(|| Value::Object(Map::new()));
    if servers.is_null() {
        *servers = Value::Object(Map::new());
    }
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| NotEditable::WrongShape {
            what: key.to_string(),
            path: path.display().to_string(),
            shape: "an object",
        })?;

    servers.insert(
        SERVER_NAME.to_owned(),
        match format {
            McpFormat::Local => json!({
                "type": "local",
                "command": [command, "mcp"],
                "enabled": true,
            }),
            // `type` is required by its schema, and `command`/`args` are split
            // the way Claude Code splits them rather than joined the way
            // OpenCode joins them.
            McpFormat::CrushStdio => json!({
                "type": "stdio",
                "command": command,
                "args": ["mcp"],
            }),
            _ => json!({ "command": command, "args": ["mcp"] }),
        },
    );

    as_written(existing, &root)
}

/// Serialises a JSON document the way the file it came from was written.
///
/// `to_string_pretty` always indents with two spaces. An operator who indents
/// with four, or with tabs, got their settings file handed back reindented by a
/// tool that had gone in to add one hook — and formatting is theirs as much as
/// the keys are. The keys and their order already survived; this is the rest of
/// what "your file comes back yours" has to mean.
///
/// The indent is read off the original: the whitespace before the first key of
/// the outermost object. A file with nothing to read it from — empty, or written
/// on one line — gets two spaces, which is what it would have got anyway.
/// The same, for the uninstall path in [`super`], which has to keep an
/// operator's indentation exactly as `strip_hooks` does.
pub(super) fn as_written_public(existing: Option<&str>, root: &Value) -> Result<String> {
    as_written(existing, root)
}

/// The byte-order mark, named once.
const MARK: char = '\u{feff}';

/// The text with a leading byte-order mark set aside.
///
/// A BOM is not content, and `serde_json` refuses one. Windows is this crate's
/// own platform, and Notepad and `Set-Content` both write it by default — so
/// `estigia install` stopped on a perfectly good settings file with *"is not
/// JSON: expected value at line 1 column 1"*, and the way out it offered was
/// *"a JSON object in that file, or the file moved aside"*, about a file that
/// already held one. The message named the wrong cause and the remedy destroys
/// somebody's settings.
///
/// Four parse sites read these files; the mark is set aside in one place,
/// because three of them are the same question asked again.
pub(super) fn without_mark(text: &str) -> &str {
    text.strip_prefix(MARK).unwrap_or(text)
}

fn as_written(existing: Option<&str>, root: &Value) -> Result<String> {
    let indent = existing
        .and_then(indent_of)
        .unwrap_or_else(|| "  ".to_owned());
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut serialiser = serde_json::Serializer::with_formatter(&mut out, formatter);
    serde::Serialize::serialize(root, &mut serialiser).context("render the settings file")?;
    let mut text = String::from_utf8(out).context("the rendered settings are not UTF-8")?;
    text.push('\n');
    // Kept, not merely tolerated on the way in. The file was written with it,
    // and what this crate did not put into one of these files comes back byte
    // for byte — the same promise the indentation above is read for.
    if existing.is_some_and(|text| text.starts_with(MARK)) {
        text.insert(0, MARK);
    }
    Ok(text)
}

/// The whitespace this document indents with, if it says.
///
/// The first line that opens with whitespace and is not blank. Read from the
/// text rather than guessed, and `None` when the text does not say — which a
/// one-line document does not.
fn indent_of(existing: &str) -> Option<String> {
    existing.lines().skip(1).find_map(|line| {
        let white: String = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        if white.is_empty() || white.len() == line.len() {
            return None;
        }
        Some(white)
    })
}

/// Drops Estigia's MCP entry, keeping every other server.
pub(super) fn strip_mcp(path: &Path, existing: &str, format: McpFormat) -> Result<String> {
    if format == McpFormat::CodexToml {
        // Invariant two, which this path reached before the comparison that
        // holds it for the others: `strip_codex_mcp` rewrites unconditionally
        // — CRLF normalised to LF, trailing blank lines trimmed — so on
        // Windows, where an operator's `config.toml` is CRLF by default, an
        // uninstall that removed nothing still reported `update` and rewrote
        // every line ending in their file. The install side keeps the
        // normalisation, where a rewrite is what was asked for.
        if !existing.lines().any(opens_estigia_section) {
            return Ok(existing.to_owned());
        }
        return Ok(strip_codex_mcp(existing));
    }
    let mut root: Value =
        serde_json::from_str(without_mark(existing)).map_err(|error| NotEditable::Unparseable {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    // What it looked like before, so a file with nothing of Estigia's in it
    // comes back exactly as the operator wrote it. Reserialising unconditionally
    // reported `update` and rewrote their whitespace on an uninstall that
    // removed nothing — and invariant two is that a file which never mentioned
    // Estigia is *reported unchanged rather than touched*.
    let before = root.clone();
    for key in ["mcpServers", "mcp"] {
        let Some(servers) = root.get_mut(key).and_then(Value::as_object_mut) else {
            continue;
        };
        let ours = servers.remove(SERVER_NAME).is_some();
        // A container **this uninstall emptied** is a key that says nothing,
        // and it was not there before Estigia arrived. An operator whose
        // settings file had never held an MCP server used to get
        // `"mcpServers": {}` added to it by an *uninstall* — Estigia leaving
        // something of its own behind in their file on the way out, which is
        // the one thing taking it back out is for.
        //
        // `ours` is the half that took longest to arrive. This rule was copied
        // from `strip_hooks`, which asked whether the container was *empty*
        // rather than whether it had been emptied here — so an operator
        // holding an unfilled container had it removed by an uninstall that
        // took nothing out of it. Somebody else's server keeps the key alive;
        // so does nobody's, if it was theirs to keep.
        if ours
            && servers.is_empty()
            && let Some(object) = root.as_object_mut()
        {
            object.remove(key);
        }
    }
    if root == before {
        return Ok(existing.to_owned());
    }
    as_written(Some(existing), &root)
}

/// Codex keeps its servers in TOML, edited by the line rather than by a parser.
///
/// A round trip through a TOML library would reformat somebody else's file —
/// reordering their keys and dropping their comments — which breaks invariant
/// three as surely as rewriting it wholesale would.
pub(super) fn render_codex_mcp(existing: &str, command: &str) -> String {
    let kept = strip_codex_mcp(existing);
    let quoted = serde_json::to_string(command).unwrap_or_else(|_| format!("\"{command}\""));
    let block = format!("[mcp_servers.{SERVER_NAME}]\ncommand = {quoted}\nargs = [\"mcp\"]");
    let base = kept.trim_end();
    if base.is_empty() {
        format!("{block}\n")
    } else {
        format!("{base}\n\n{block}\n")
    }
}

/// Whether a line opens Estigia's own table, in either spelling TOML allows.
///
/// `[mcp_servers."estigia"]` is the same table as `[mcp_servers.estigia]` and
/// Codex reads both. Three readers in [`super`] spelled the bare form inline and
/// knew nothing of the quoted one, while this file — which writes the section
/// and lifts it back out — knew both. Measured through the binary on a
/// `config.toml` carrying the quoted spelling: `estigia status` printed **tools
/// off** and `doctor` said *no tool server registered*, over a server that was
/// registered and would have started. The uninstall, reading through this file,
/// took the same section out correctly.
///
/// A **sub-table** is deliberately not this question. `[mcp_servers.estigia.env]`
/// has to be lifted out with the rest — see [`opens_estigia_section`] — and does
/// not on its own register anything, so a file carrying only one is a file with
/// no server in it.
pub(super) fn opens_estigia_table(line: &str) -> bool {
    let line = line.trim();
    line == format!("[mcp_servers.{SERVER_NAME}]")
        || line == format!("[mcp_servers.\"{SERVER_NAME}\"]")
}

/// Whether a line opens a section of Estigia's, in any of its three spellings.
///
/// One predicate, because it answers two questions that must not drift apart:
/// which lines to lift out, and whether there is anything to lift out at all.
fn opens_estigia_section(line: &str) -> bool {
    opens_estigia_table(line)
        || line
            .trim()
            .starts_with(&format!("[mcp_servers.{SERVER_NAME}."))
}

/// The lines under Estigia's table, up to the next table header.
///
/// The one place that answers *what does this file say about Estigia's server*,
/// for the readers that need the keys rather than the fact. It was written out
/// twice in [`super`], identically, and both copies found the table by spelling
/// its bare form — so both answered `None` for a file Codex reads perfectly
/// well, and the rows built on them reported nothing rather than what was there.
pub(super) fn estigia_table_block(text: &str) -> Option<String> {
    let mut found: Vec<&str> = Vec::new();
    for line in text.lines() {
        if found.is_empty() {
            if opens_estigia_table(line) {
                found.push(line);
            }
            continue;
        }
        // Any table header ends it, including one of Estigia's own sub-tables:
        // `command` and `args` sit directly under the table, and a key found
        // below a `[mcp_servers.estigia.env]` header belongs to that one.
        if line.trim_start().starts_with('[') {
            break;
        }
        found.push(line);
    }
    (!found.is_empty()).then(|| found.join("\n"))
}

/// Lifts `[mcp_servers.estigia]` out, leaving every other section untouched.
///
/// The section ends at the next table header — **or at a comment**, and that
/// second boundary is the one that had to be added. Estigia's block is three
/// lines and none of them is a comment, so a `#` under that header was typed by
/// the operator; running to the next header swallowed it, and when the section
/// was last in the file it swallowed everything to the end.
///
/// Measured through the binary, one `~/.codex/config.toml`: `setup codex`, a
/// note written after the table, `uninstall` — and the note was gone. Which is
/// the one thing this crate promises about somebody else's file, and the same
/// promise `fence::locate` had just been fixed for.
///
/// A **key** under that header is left to go with it, deliberately: it was
/// written under a table that is being removed, and keeping it would leave it
/// attached to whichever table now precedes it — a value silently moved rather
/// than a value lost. A comment has no such attachment, so keeping one costs
/// nothing.
pub(super) fn strip_codex_mcp(existing: &str) -> String {
    let normalized = existing.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut kept = Vec::with_capacity(lines.len());
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if opens_estigia_section(line) {
            index += 1;
            while index < lines.len()
                && !(lines[index].trim().starts_with('[') && lines[index].trim().ends_with(']'))
                && !lines[index].trim_start().starts_with('#')
            {
                index += 1;
            }
            continue;
        }
        kept.push(lines[index]);
        index += 1;
    }
    let body = kept.join("\n");
    let body = body.trim_end();
    if body.is_empty() {
        String::new()
    } else {
        format!("{body}\n")
    }
}

/// Merges Estigia's lifecycle hooks into an agent settings file.
///
/// Only the entries Estigia owns are replaced. Every other setting and every
/// other tool's hooks are written back as they were — the same invariant as the
/// fenced blocks, applied to a structure that has no place to put a marker.
/// The command a hook entry runs, in any dialect.
///
/// Quoted unconditionally: these are handed to a shell, and on Windows bash
/// treats a bare backslash as an escape — the default install path breaks it
/// with no space anywhere in sight.
///
///
/// **Double** quotes here, and single quotes in the git hook, and that
/// difference is measured rather than chosen. Git bundles its own POSIX shell,
/// so `harness::guard`'s script is always read by `sh` — where single quotes
/// are strictly safer, because double quotes still expand `$(…)` and `` `…` ``
/// out of the path itself. An agent's hook command is read by whatever shell
/// that agent uses, and on this crate's own platform `cmd //c "'C:\…\estigia.exe'
/// hook …"` answers *el nombre de archivo … no son correctos*: `cmd` does not
/// quote with `'`.
///
/// So the path stays double-quoted here until somebody has watched each of the
/// ten agents run one. What that leaves open is named rather than closed: an
/// install directory containing `$(…)`, `` `…` `` or `${…}` misroutes the gate
/// under a shell that expands them, and nothing here can tell.
fn hook_command(executable: &Path, agent: &str, dialect: Dialect) -> String {
    format!(
        "\"{}\" {}",
        executable.display(),
        hook_arguments(agent, dialect)
    )
}

/// What every registered gate is invoked with, whoever writes the command line.
///
/// One function because there are two writers and they had drifted. This module
/// writes the settings hooks; [`super::plugin`] writes Cline's, in its own
/// quoting for two languages, and its copy said `hook pre-tool-use --dialect
/// cline` and nothing else. So the round that taught the ledger which agent sent
/// an ungated call taught it for eight agents and left the ninth — measured on a
/// `setup --all`, where Cline's was the one line of nine with no `--agent` in
/// it, and `doctor` reports its calls as *"from an agent no line names"*.
///
/// The quoting stays with each writer: a settings hook is read by whatever shell
/// that agent uses and takes double quotes, and Cline's script is `sh` or
/// PowerShell and takes theirs. What is shared is *which flags a gate is given*,
/// which is the part that went out of step.
pub(super) fn hook_arguments(agent: &str, dialect: Dialect) -> String {
    format!(
        "hook pre-tool-use --agent {} --dialect {}",
        agent,
        dialect.slug()
    )
}

/// Whether an entry runs Estigia's gate, whatever path the binary had.
fn is_estigia_gate(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.contains("hook pre-tool-use"))
}

/// Crush keeps its hooks in the same `crush.json` as everything else.
///
/// Its own renderer rather than Cursor's, which it superficially resembles.
/// Cursor's writes a `version` beside the hooks and Crush's schema has no such
/// field — putting one tool's scaffolding into another tool's configuration is
/// how a file ends up parsing, looking right, and failing validation somewhere
/// nobody connects back to this.
///
/// The matcher goes **on the entry**, which is the other difference. Without it
/// Crush wakes this process for every `view`, `ls` and `grep`: a cost paid
/// thousands of times to answer "not mine".
pub(super) fn render_crush_hooks(
    path: &Path,
    existing: Option<&str>,
    executable: &Path,
    agent: &str,
    spec: GateSpec,
) -> Result<String> {
    let mut root = Value::Object(object_at(path, existing)?);
    let Value::Object(object) = &mut root else {
        unreachable!("object_at returns a map")
    };
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    if hooks.is_null() {
        *hooks = Value::Object(Map::new());
    }
    let hooks = hooks
        .as_object_mut()
        .ok_or_else(|| NotEditable::WrongShape {
            what: "hooks".to_owned(),
            path: path.display().to_string(),
            shape: "an object",
        })?;

    for event in spec.events {
        let entries = hooks
            .entry((*event).to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if entries.is_null() {
            *entries = Value::Array(Vec::new());
        }
        let entries = entries
            .as_array_mut()
            .ok_or_else(|| NotEditable::WrongShape {
                what: format!("hooks.{event}"),
                path: path.display().to_string(),
                shape: "an array",
            })?;
        entries.retain(|entry| !is_estigia_gate(entry));
        let mut entry = Map::new();
        entry.insert("name".to_owned(), Value::String(crate::NAME.to_owned()));
        if let Some(matcher) = spec.matcher {
            entry.insert("matcher".to_owned(), Value::String(matcher.to_owned()));
        }
        entry.insert(
            "command".to_owned(),
            Value::String(hook_command(executable, agent, spec.dialect)),
        );
        entry.insert("timeout".to_owned(), json!(10));
        entries.push(Value::Object(entry));
    }

    as_written(existing, &root)
}

/// Windsurf keeps its hooks in a file of its own, keyed by event, with no
/// matcher and no wrapper.
///
/// The entry carries the command twice. `command` is run through `bash -c`;
/// `powershell` is what Windows uses when it is present. Estigia's command is a
/// quoted absolute path and its flags, which both shells read identically, so
/// both fields get the same string — writing one would leave the gate registered
/// and silent on the other half of the platforms, which is the failure this
/// adapter exists to avoid.
pub(super) fn render_windsurf_hooks(
    path: &Path,
    existing: Option<&str>,
    executable: &Path,
    agent: &str,
    spec: GateSpec,
) -> Result<String> {
    let mut root = Value::Object(object_at(path, existing)?);
    let Value::Object(object) = &mut root else {
        unreachable!("object_at returns a map")
    };
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    if hooks.is_null() {
        *hooks = Value::Object(Map::new());
    }
    let hooks = hooks
        .as_object_mut()
        .ok_or_else(|| NotEditable::WrongShape {
            what: "hooks".to_owned(),
            path: path.display().to_string(),
            shape: "an object",
        })?;

    let command = hook_command(executable, agent, spec.dialect);
    for event in spec.events {
        let entries = hooks
            .entry((*event).to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if entries.is_null() {
            *entries = Value::Array(Vec::new());
        }
        let entries = entries.as_array_mut().with_context(|| {
            format!("hooks.{event} in {} must contain an array", path.display())
        })?;
        entries.retain(|entry| !is_estigia_gate(entry));
        entries.push(json!({ "command": command, "powershell": command }));
    }

    as_written(existing, &root)
}

/// Takes Estigia's entries back out of a Windsurf hooks file.
pub(super) fn strip_windsurf_hooks(path: &Path, existing: &str) -> Result<String> {
    strip_cursor_hooks(path, existing)
}

/// Cursor keeps its hooks in a file of its own, with a version and a flatter
/// entry than the settings envelope the other two share.
pub(super) fn render_cursor_hooks(
    path: &Path,
    existing: Option<&str>,
    executable: &Path,
    agent: &str,
    spec: GateSpec,
) -> Result<String> {
    let mut root = Value::Object(object_at(path, existing)?);
    let Value::Object(object) = &mut root else {
        unreachable!("object_at returns a map")
    };
    // Only into a file with nothing of anybody's in it — one this run is
    // creating, or a blank one. Added to a file the operator already keeps
    // hooks in, it stays there after an uninstall, because nothing records
    // that Estigia was what put it there; and worse than the residue is what
    // adding it might do on the way in. If Cursor treats a version-less file
    // as inert, writing the key turns on *their* hooks as a side effect of
    // installing ours — a change to how their machine behaves that nobody
    // asked for and that uninstalling would not undo.
    if object.is_empty() {
        object.insert("version".to_owned(), json!(1));
    }
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    if hooks.is_null() {
        *hooks = Value::Object(Map::new());
    }
    let hooks = hooks
        .as_object_mut()
        .ok_or_else(|| NotEditable::WrongShape {
            what: "hooks".to_owned(),
            path: path.display().to_string(),
            shape: "an object",
        })?;

    for event in spec.events {
        let entries = hooks
            .entry((*event).to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if entries.is_null() {
            *entries = Value::Array(Vec::new());
        }
        let entries = entries.as_array_mut().with_context(|| {
            format!("hooks.{event} in {} must contain an array", path.display())
        })?;
        entries.retain(|entry| !is_estigia_gate(entry));
        entries.push(json!({
            "command": hook_command(executable, agent, spec.dialect),
            "timeout": 10,
        }));
    }

    as_written(existing, &root)
}

/// Drops Estigia's gate from Cursor's hooks file, keeping every other one.
pub(super) fn strip_cursor_hooks(path: &Path, existing: &str) -> Result<String> {
    let mut root: Value =
        serde_json::from_str(without_mark(existing)).map_err(|error| NotEditable::Unparseable {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    // What it looked like before, so a file with nothing of Estigia's in it
    // comes back exactly as the operator wrote it. Reserialising unconditionally
    // reported `update` and rewrote their whitespace on an uninstall that
    // removed nothing — and invariant two is that a file which never mentioned
    // Estigia is *reported unchanged rather than touched*.
    let before = root.clone();
    if let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) {
        // Emptied here, not merely empty — see `strip_hooks`, which had this
        // same hole. An operator's unfilled slot is theirs, and an uninstall
        // is entitled to Estigia's entries and to nothing else.
        let mut emptied: Vec<String> = Vec::new();
        for (name, entries) in hooks.iter_mut() {
            let Some(list) = entries.as_array_mut() else {
                continue;
            };
            let held = list.len();
            list.retain(|entry| !is_estigia_gate(entry));
            if list.is_empty() && list.len() != held {
                emptied.push(name.clone());
            }
        }
        hooks.retain(|name, _| !emptied.contains(name));
    }
    if root == before {
        return Ok(existing.to_owned());
    }
    as_written(Some(existing), &root)
}

pub(super) fn render_hooks(
    path: &Path,
    existing: Option<&str>,
    executable: &Path,
    agent: &str,
    spec: GateSpec,
) -> Result<String> {
    let mut root = Value::Object(object_at(path, existing)?);
    let Value::Object(object) = &mut root else {
        unreachable!("object_at returns a map")
    };
    // Every envelope this function serves wraps its events in `hooks`. Codex
    // was the exception for a while — the events at the top level, no wrapper —
    // and it was not an exception at all: the file it produced is one Codex
    // refuses to parse, so the agent would not start. Writing into the wrong
    // envelope usually produces a file that parses, looks right and never
    // fires; this one was louder, and only by luck.
    let hooks = {
        let hooks = object
            .entry("hooks")
            .or_insert_with(|| Value::Object(Map::new()));
        if hooks.is_null() {
            *hooks = Value::Object(Map::new());
        }
        hooks
            .as_object_mut()
            .ok_or_else(|| NotEditable::WrongShape {
                what: "hooks".to_owned(),
                path: path.display().to_string(),
                shape: "an object",
            })?
    };

    // Hook commands are handed to a shell, and always quoted. Leteo learned
    // this the expensive way: bash treats a backslash as an escape, so a bare
    // `C:\Users\me\...\estigia.exe` arrives with every separator eaten and
    // every hook fails with "command not found" — no space in sight.
    //
    // Double quotes, not single: see `hook_command`, which measured what `cmd`
    // does with `'…'` on this crate's own platform.
    let command_prefix = format!("\"{}\"", executable.display());

    // The gate first, in this agent's own events and dialect.
    for event in spec.events {
        let entries = hooks
            .entry((*event).to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if entries.is_null() {
            *entries = Value::Array(Vec::new());
        }
        let entries = entries
            .as_array_mut()
            .ok_or_else(|| NotEditable::WrongShape {
                what: format!("hooks.{event}"),
                path: path.display().to_string(),
                shape: "an array",
            })?;
        entries.retain_mut(|entry| !emptied_of_estigia(entry));
        let mut entry = Map::new();
        if let Some(matcher) = spec.matcher {
            entry.insert("matcher".to_owned(), Value::String(matcher.to_owned()));
        }
        entry.insert(
            "hooks".to_owned(),
            json!([{
                "type": "command",
                "command": hook_command(executable, agent, spec.dialect),
                "timeout": 10,
            }]),
        );
        entries.push(Value::Object(entry));
    }

    // The lifecycle events, which only Claude Code is known to have. Codex
    // shares its dialect *and* its envelope and not its lifecycle, so the
    // adapter says so itself rather than being deduced from those two.
    if !spec.lifecycle {
        return as_written(existing, &root);
    }

    for (event, _, _) in crate::harness::hook::EVENTS {
        // The gate was written above, in this agent's own event name. Clearing
        // it here would wipe what the block just added — the entry goes in and
        // comes straight back out, and the file ends up with a lifecycle hook
        // and no gate.
        if *event == crate::harness::hook::Event::PreToolUse {
            continue;
        }
        let entries = hooks
            .entry(event.agent_name().to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if entries.is_null() {
            *entries = Value::Array(Vec::new());
        }
        let entries = entries.as_array_mut().with_context(|| {
            format!(
                "hooks.{} in {} must contain an array",
                event.agent_name(),
                path.display()
            )
        })?;
        entries.retain_mut(|entry| !emptied_of_estigia(entry));
    }

    for (event, matcher, timeout) in crate::harness::hook::EVENTS {
        if *event == crate::harness::hook::Event::PreToolUse {
            continue;
        }
        let mut entry = Map::new();
        if let Some(matcher) = matcher {
            entry.insert("matcher".to_owned(), Value::String((*matcher).to_owned()));
        }
        entry.insert(
            "hooks".to_owned(),
            json!([{
                "type": "command",
                "command": format!("{command_prefix} hook {}", event.slug()),
                "timeout": timeout,
            }]),
        );
        if let Some(list) = hooks
            .get_mut(event.agent_name())
            .and_then(Value::as_array_mut)
        {
            list.push(Value::Object(entry));
        }
    }

    as_written(existing, &root)
}

/// Drops Estigia's hooks, keeping every other tool's.
pub(super) fn strip_hooks(path: &Path, existing: &str) -> Result<String> {
    let mut root: Value =
        serde_json::from_str(without_mark(existing)).map_err(|error| NotEditable::Unparseable {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    // What it looked like before, so a file with nothing of Estigia's in it
    // comes back exactly as the operator wrote it. Reserialising unconditionally
    // reported `update` and rewrote their whitespace on an uninstall that
    // removed nothing — and invariant two is that a file which never mentioned
    // Estigia is *reported unchanged rather than touched*.
    let before = root.clone();
    // Three envelopes again. Removing from the wrapper only would leave Codex's
    // entry in place — an uninstall that reports success and leaves the gate
    // running, which is the inverse of the failure the install side has.
    let bare = root.get("hooks").is_none();
    let events = if bare {
        root.as_object_mut()
    } else {
        root.get_mut("hooks").and_then(Value::as_object_mut)
    };
    // Which keys this uninstall emptied, as opposed to which ones are empty.
    // The two were treated as one, and they are not: an event left with no
    // hooks *by us* is a key that says nothing and was not there before Estigia
    // arrived, but one the operator wrote empty — a slot they meant to fill —
    // was, and taking it is editing their file on an uninstall that is only
    // entitled to Estigia's own entries. A settings file holding nothing but
    // their empty slots came back as `{}`.
    let mut emptied: Vec<String> = Vec::new();
    if let Some(events) = events {
        for (name, entries) in events.iter_mut() {
            let Some(list) = entries.as_array_mut() else {
                continue;
            };
            let held = list.len();
            list.retain_mut(|entry| !emptied_of_estigia(entry));
            if list.is_empty() && list.len() != held {
                emptied.push(name.clone());
            }
        }
        events.retain(|name, _| !emptied.contains(name));
    }
    // The same rule one level up, and it had the same hole: a wrapper is ours
    // to drop only when what emptied it was us. An operator who keeps a bare
    // `"hooks": {}` had it removed by an uninstall that took nothing out of it.
    if !emptied.is_empty()
        && root
            .get("hooks")
            .and_then(Value::as_object)
            .is_some_and(serde_json::Map::is_empty)
        && let Some(object) = root.as_object_mut()
    {
        object.remove("hooks");
    }
    if root == before {
        return Ok(existing.to_owned());
    }
    as_written(Some(existing), &root)
}

/// Recognises entries Estigia wrote, whatever path the binary had.
pub(super) fn is_estigia_hook(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| hooks.iter().any(is_estigia_command))
}

/// Whether one command inside a group is Estigia's.
fn is_estigia_command(hook: &Value) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| {
            crate::harness::hook::EVENTS
                .iter()
                .any(|(event, _, _)| command.contains(&format!("hook {}", event.slug())))
        })
}

/// Takes Estigia's commands out of one group, and says whether that emptied it.
///
/// The group was dropped whole whenever it held one of ours — and a group is a
/// **matcher and a list**, so an operator who added their own command beside
/// Estigia's lost it on the next uninstall or sync. Measured through the binary:
/// `setup claude-code`, a `mi-script-propio.sh` appended to the `PreToolUse`
/// group Estigia had written, `uninstall` — and the file came back with the
/// group gone and their command with it.
///
/// This module says the rule two levels up, twice, in its own words: *an
/// operator's unfilled slot is theirs*, and *a wrapper is ours to drop only
/// when what emptied it was us*. The innermost level is where it was not
/// applied, and it is the level holding the thing they typed.
///
/// The group survives carrying Estigia's matcher, which is the lesser of the
/// two: a matcher that is wider than they would have chosen costs them a hook
/// firing more often, and dropping the group costs them the hook.
fn emptied_of_estigia(entry: &mut Value) -> bool {
    let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
        return false;
    };
    let held = inner.len();
    inner.retain(|hook| !is_estigia_command(hook));
    inner.len() != held && inner.is_empty()
}
