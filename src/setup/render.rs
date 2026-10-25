//! What each agent's configuration file has to look like.
//!
//! Every function here obeys invariant two: it replaces what Estigia owns and
//! writes everything else back. Where the format has no room for a marker — a
//! JSON object, a TOML section — the entry is lifted out by name instead.

use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use super::{AgentAdapter, InstructionFile, PiExposure};
use crate::harness::hook::{Dialect, Event};

/// How one agent registers a hook that can deny a tool call. Several agents
/// can; none spells it the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GateSpec {
    /// The events that fire before a tool runs.
    ///
    /// A slice, because one agent needs more than one: registering only
    /// `pre_write_code` would gate edits and leave every shell command through.
    pub events: &'static [&'static str],
    /// The tools worth waking a process for, when the agent filters.
    pub matcher: Option<&'static str>,
    /// How this agent reads a decision.
    pub dialect: Dialect,
    /// The shape of the file the entry goes in.
    pub envelope: Envelope,
    /// Whether this agent has the lifecycle events beyond the tool gate.
    ///
    /// Declared, not inferred: inferring it from "settings envelope **and**
    /// Claude Code dialect" stopped being true the day another agent shared
    /// both, and Codex silently began receiving entries nobody runs.
    pub lifecycle: bool,
}

/// The shapes a hooks file comes in.
///
/// Not one abstraction with branches — concrete files that happen to hold the
/// same idea. Naming the difference is what stops one agent's entry being
/// written into another's envelope, where it parses and never fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Envelope {
    /// `{"hooks": {"<Event>": [{matcher, hooks: [{type, command, timeout}]}]}}`
    /// — Claude Code and Gemini CLI.
    Settings,
    /// `{"version": 1, "hooks": {"<event>": [{command, timeout}]}}` — Cursor,
    /// with the command straight on the entry.
    Cursor,
    /// `{"hooks": {"<event>": [{command, powershell}]}}` — Windsurf. Uninstall
    /// only: nothing writes it any more.
    Windsurf,
    /// `{"hooks": {"<Event>": [{name, matcher, command, timeout}]}}` — Crush,
    /// inside its `crush.json`. Uninstall only: nothing writes it any more.
    Crush,
}

/// A file Estigia was asked to edit and cannot.
///
/// Typed rather than a message: nothing was written, the operator owns the
/// file, and no command Estigia can name will fix it.
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
    #[error("{what} in {path} must be {shape}")]
    WrongShape {
        /// The key, as it is written in the file.
        what: String,
        /// The file that was read.
        path: String,
        /// What that key has to hold.
        shape: &'static str,
    },
    /// The file is not TOML at all, so there is no safe edit of it.
    #[error("{path} is not TOML: {detail}")]
    UnparseableToml {
        /// The file that was read.
        path: String,
        /// What the parser said.
        detail: String,
    },
    /// The edit did not read back as the trust Estigia meant to write.
    #[error("{path} does not read back the trust Estigia wrote for {key}")]
    TrustNotWritten {
        /// The file that was read.
        path: String,
        /// The trust key the write was meant to establish.
        key: String,
    },
}

/// The JSON object in `existing`, or an empty one when there is nothing there.
///
/// Returns the map rather than a `Value`, so that no caller has to promise the
/// shape a second time.
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
    /// Cursor's file holds a `version` beside its hooks; removing the last hook
    /// must not leave it behind in a file Estigia made. Declared beside the
    /// code that writes it, so the two cannot drift.
    pub(super) fn scaffolding(self) -> &'static [&'static str] {
        match self {
            Self::Cursor => &["version"],
            Self::Windsurf | Self::Crush => &[],
            Self::Settings => &[],
        }
    }
}

impl AgentAdapter {
    /// How this agent registers a tool-call gate, if it can have one. Every
    /// entry comes from that agent's published hooks reference.
    ///
    /// `const` so the canonical table's `gate` column is crossed against this
    /// `match` at compile time, not merely declared beside it.
    pub(super) const fn gate_spec(&self) -> Option<GateSpec> {
        match self.instructions {
            InstructionFile::ClaudeCode => Some(GateSpec {
                events: &["PreToolUse"],
                // `Update` is named because `WRITE_TOOLS` already says this
                // agent writes through it, and the two must not disagree.
                // `Agent` and legacy `Task` are not repository writes; they wake
                // the dedicated Claude prelaunch check for the reserved reviewer.
                // `AskUserQuestion` is read only under `Decision mode: auto`.
                matcher: Some(
                    "Edit|Write|MultiEdit|NotebookEdit|Update|Bash|Agent|Task|AskUserQuestion",
                ),
                dialect: Dialect::ClaudeCode,
                envelope: Envelope::Settings,
                // The only one; see the field's note on why it is declared.
                lifecycle: true,
            }),
            // Verified from `codex-rs/hooks/src/schema.rs`: byte for byte
            // Claude Code's shape, so it shares the dialect and the envelope,
            // and differs from Claude Code in nothing but which file it writes.
            // The matcher covers `apply_patch` and its aliases `Write` and
            // `Edit` — `core/src/tools/hook_names.rs` gives the canonical set —
            // not shell alone.
            InstructionFile::Codex => Some(GateSpec {
                events: &["PreToolUse"],
                matcher: Some("^(Bash|apply_patch|Write|Edit)$"),
                dialect: Dialect::ClaudeCode,
                envelope: Envelope::Settings,
                lifecycle: false,
            }),
            InstructionFile::GeminiCli => Some(GateSpec {
                events: &["BeforeTool"],
                // Gemini's matcher is a regular expression over tool names, and
                // its tools are named differently from Claude Code's.
                matcher: Some("edit|write_file|replace|run_shell_command"),
                dialect: Dialect::GeminiCli,
                envelope: Envelope::Settings,
                lifecycle: false,
            }),
            // Verified from `packages/core/src/hooks/types.ts`: a Gemini CLI
            // fork that has diverged where it counts — the event is
            // `PreToolUse`, not Gemini's `BeforeTool`. `HookDecision` and the
            // pipe-separated matcher are Gemini's, so those are shared.
            InstructionFile::Qwen => Some(GateSpec {
                events: &["PreToolUse"],
                matcher: Some("write_file|replace|edit|run_shell_command"),
                dialect: Dialect::GeminiCli,
                envelope: Envelope::Settings,
                lifecycle: false,
            }),
            // Verified in `internal/hooks/hooks.go` and `internal/config/config.go`:
            // `PreToolUse`, a regex matcher against the tool name, and **exit 2
            // blocks the tool call**; the payload is the shape Estigia already
            // reads, with no renaming. Its own hooks module says the types match
            // Claude Code's; checked piece by piece rather than taken on that word.
            //
            // `NotebookEdit` is left out because Continue has no such tool;
            // naming a tool that does not exist is the kind of thing somebody
            // later reads as evidence that it does.
            InstructionFile::Continue => Some(GateSpec {
                events: &["PreToolUse"],
                matcher: Some("Edit|Write|MultiEdit|Bash"),
                dialect: Dialect::ClaudeCode,
                envelope: Envelope::Settings,
                lifecycle: false,
            }),
            InstructionFile::Crush => Some(GateSpec {
                events: &["PreToolUse"],
                // Its own names, lower case; the classifier already knows all
                // four, normalising Claude Code's title case.
                matcher: Some("edit|write|multiedit|bash"),
                dialect: Dialect::ExitCode,
                envelope: Envelope::Crush,
                lifecycle: false,
            }),
            InstructionFile::Windsurf => Some(GateSpec {
                // Two, because Windsurf has no matcher and one event per kind of
                // action. Both, or a shell walks around the write boundary.
                events: &["pre_write_code", "pre_run_command"],
                matcher: None,
                dialect: Dialect::ExitCode,
                envelope: Envelope::Windsurf,
                lifecycle: false,
            }),
            InstructionFile::Cursor => Some(GateSpec {
                events: &["preToolUse"],
                matcher: None,
                dialect: Dialect::Cursor,
                envelope: Envelope::Cursor,
                lifecycle: false,
            }),
            // Spelled out rather than left to a wildcard: a wildcard hands a
            // new variant a default nobody chose. OpenCode and Cline still gate
            // through a file Estigia owns whole, which `can_gate_tools` adds back.
            InstructionFile::Neutral
            | InstructionFile::OpenCode
            | InstructionFile::Pi
            | InstructionFile::Cline
            | InstructionFile::ZCode
            | InstructionFile::DeepSeekHarness => None,
        }
    }

    /// Why this agent has no tool-call gate, when it has none. Each line says
    /// what is missing and what would close it.
    pub fn gate_gap(&self) -> Option<&'static str> {
        if self.can_gate_tools() {
            return None;
        }
        Some(match self.instructions {
            InstructionFile::Neutral => {
                "the agent-neutral root is a convention for where skills live. It is not an agent, has no tool loop, and there is nothing here to gate."
            }
            // Not the generic arm below: ZCode's reference documents a
            // `PreToolUse` event, so saying nothing has been read would be
            // false. The phrase this returns is pinned by
            // `the_only_agents_without_a_gate_are_the_neutral_root_zcode_and_deepseek_harness`.
            InstructionFile::ZCode => {
                "ZCode's own reference documents a PreToolUse event, but a configuration-file hook there decides nothing until `hooks.enabled` is true, and neither the payload the host sends nor whether a hook can deny has been crossed against a running installation."
            }
            InstructionFile::DeepSeekHarness => {
                "the harness exposes an execution hook, but whether a plugin can deny a tool call, and under what payload, has not been crossed against a shipped harness."
            }
            _ => "no published reference for a deny-capable hook has been read for this agent.",
        })
    }

    /// Whether a hook response can start or continue a turn for this agent.
    ///
    /// The gate half, not the tool half: `can_gate_tools` says a write is
    /// weighed, this says a turn is continuable. One dialect demonstrated one
    /// surface; the rest are unable with the reason, or unknown pending the
    /// reference that would settle them.
    pub fn continuation(&self) -> &'static str {
        match self.instructions {
            InstructionFile::ClaudeCode => "able: stop-hook",
            InstructionFile::GeminiCli | InstructionFile::Qwen => {
                "none: a denial surfaces as a tool error"
            }
            InstructionFile::Windsurf => "none: exit 2 feeds back into the current turn",
            InstructionFile::Crush => "none: exit 2 blocks the tool, exit 49 halts the turn",
            InstructionFile::Cline => "none: exits 0 and routes to a person",
            InstructionFile::OpenCode => "able: loop plugin, when installed",
            InstructionFile::Pi => "none: the extension gates tool calls and starts no turn",
            InstructionFile::Codex => "able: stop-hook, when the Loop plugin setting is on",
            InstructionFile::Continue => "unknown: hooks reference not retrieved",
            InstructionFile::Cursor => "unknown: stop hook exists, powers unverified",
            InstructionFile::Neutral
            | InstructionFile::ZCode
            | InstructionFile::DeepSeekHarness => "none: no tool-call gate",
        }
    }
}

const PI_SERVER_DESCRIPTION: &str = "Estigia workflow authority: claim an issue, record tasks and reviews, transition state, release";

/// The name the server is registered under, in every dialect.
pub const SERVER_NAME: &str = crate::NAME;

/// How an agent spells an MCP server entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum McpFormat {
    /// `mcpServers: { estigia: { command, args } }`
    McpServers,
    /// `McpServers` plus an `exposure`: Pi's default, `codemode`, registers the
    /// tools without declaring them to the model.
    PiMcpServers,
    /// `mcp: { estigia: { type: "local", command: [...] } }`
    Local,
    /// `[mcp_servers.estigia]` in TOML.
    CodexToml,
    /// `mcp: { servers: { estigia: { type: "stdio", command, args } } }` —
    /// ZCode, the only dialect nested two levels deep.
    ZCode,
}

impl AgentAdapter {
    /// The MCP dialect this agent's configuration file speaks.
    pub(super) fn mcp_format(&self) -> McpFormat {
        match self.instructions {
            InstructionFile::Codex => McpFormat::CodexToml,
            InstructionFile::OpenCode => McpFormat::Local,
            InstructionFile::ZCode => McpFormat::ZCode,
            InstructionFile::Pi => McpFormat::PiMcpServers,
            InstructionFile::Neutral
            | InstructionFile::ClaudeCode
            | InstructionFile::GeminiCli
            | InstructionFile::Cursor
            | InstructionFile::Qwen
            | InstructionFile::Crush
            | InstructionFile::Windsurf
            | InstructionFile::Cline
            | InstructionFile::Continue
            | InstructionFile::DeepSeekHarness => McpFormat::McpServers,
        }
    }

    /// Whether `setup` can register the MCP server for this agent.
    ///
    /// Read from the canonical table's `tool_server` column, which is itself
    /// crossed against `paths_in`'s `mcp_config` resolution — an entry written
    /// into the wrong file is a server that never starts under a `status` line
    /// saying it did.
    pub fn supports_mcp(&self) -> bool {
        self.capabilities.tool_server
    }

    /// Why this agent's workflow tools can never be registered, when they
    /// cannot be. `None` exactly when [`supports_mcp`](Self::supports_mcp), so
    /// the two are crossed by `the_tools_gap_and_the_tools_never_disagree`
    /// rather than by a `_` arm a new variant would fall through.
    pub fn tools_gap(&self) -> Option<&'static str> {
        match self.instructions {
            InstructionFile::Neutral => {
                Some("the agent-neutral root has no MCP client of its own.")
            }
            InstructionFile::Windsurf => {
                Some("where Windsurf reads MCP servers from has not been verified.")
            }
            InstructionFile::Cline => Some(
                "where Cline reads MCP servers from has not been verified; the gate is what this adapter is for.",
            ),
            InstructionFile::Continue => {
                Some("where Continue reads MCP servers from has not been verified.")
            }
            InstructionFile::DeepSeekHarness => Some(
                "where DeepSeek Harness reads MCP servers from has not been crossed against a real installation.",
            ),
            InstructionFile::ClaudeCode
            | InstructionFile::Codex
            | InstructionFile::OpenCode
            | InstructionFile::Pi
            | InstructionFile::GeminiCli
            | InstructionFile::Cursor
            | InstructionFile::Qwen
            | InstructionFile::Crush
            | InstructionFile::ZCode => None,
        }
    }
}

/// Writes Estigia's MCP entry, keeping every other server.
pub(super) fn render_mcp(
    path: &Path,
    existing: Option<&str>,
    executable: &Path,
    format: McpFormat,
    pi_exposure: PiExposure,
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
    // ZCode shares OpenCode's key and not its entry: same `mcp`, different
    // shape inside.
    let key = if matches!(format, McpFormat::Local | McpFormat::ZCode) {
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

    // ZCode nests one level further: `mcp.servers.<name>`, not `mcp.<name>`.
    let servers = if format == McpFormat::ZCode {
        let nested = servers
            .entry("servers")
            .or_insert_with(|| Value::Object(Map::new()));
        if nested.is_null() {
            *nested = Value::Object(Map::new());
        }
        nested
            .as_object_mut()
            .ok_or_else(|| NotEditable::WrongShape {
                what: format!("{key}.servers"),
                path: path.display().to_string(),
                shape: "an object",
            })?
    } else {
        servers
    };

    servers.insert(
        SERVER_NAME.to_owned(),
        match format {
            McpFormat::Local => json!({
                "type": "local",
                "command": [command, "mcp"],
                "enabled": true,
            }),
            McpFormat::ZCode => json!({
                "type": "stdio",
                "command": command,
                "args": ["mcp"],
            }),
            McpFormat::McpServers => json!({ "command": command, "args": ["mcp"] }),
            McpFormat::PiMcpServers => match pi_exposure {
                PiExposure::Direct => {
                    json!({ "command": command, "args": ["mcp"], "exposure": "direct" })
                }
                PiExposure::Deferred => json!({
                    "command": command,
                    "args": ["mcp"],
                    "exposure": "deferred",
                    "description": PI_SERVER_DESCRIPTION,
                }),
            },
            McpFormat::CodexToml => unreachable!("CodexToml returns before this match"),
        },
    );

    as_written(existing, &root)
}

/// Serialises a JSON document the way the file it came from was written.
///
/// The indent is read off the original — the whitespace before the first key
/// of the outermost object — so an operator's formatting comes back theirs. A
/// file with nothing to read it from gets two spaces. The same, for the
/// uninstall path in [`super`], which has to keep an operator's indentation
/// exactly as `strip_hooks` does.
pub(super) fn as_written_public(existing: Option<&str>, root: &Value) -> Result<String> {
    as_written(existing, root)
}

/// The byte-order mark, named once.
const MARK: char = '\u{feff}';

/// The text with a leading byte-order mark set aside.
///
/// A BOM is not content, and `serde_json` refuses one; Windows editors write
/// it by default. The mark is set aside here, and readers go through
/// [`parse_settings`].
pub(super) fn without_mark(text: &str) -> &str {
    text.strip_prefix(MARK).unwrap_or(text)
}

/// An agent's JSON settings text parsed with a leading byte-order mark set aside.
pub(super) fn parse_settings(text: &str) -> Option<Value> {
    serde_json::from_str(without_mark(text)).ok()
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
    // Kept, not merely tolerated: what this crate did not put into the file
    // comes back byte for byte.
    if existing.is_some_and(|text| text.starts_with(MARK)) {
        text.insert(0, MARK);
    }
    Ok(text)
}

/// The whitespace this document indents with, if it says: the first line that
/// opens with whitespace and is not blank.
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
        // Invariant two: `strip_codex_mcp` rewrites unconditionally (CRLF
        // normalised to LF, trailing blank lines trimmed), so on Windows an
        // uninstall that removed nothing still rewrote every line ending.
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
    // comes back exactly as the operator wrote it.
    let before = root.clone();
    if format == McpFormat::ZCode {
        strip_zcode_mcp(&mut root);
    } else {
        for key in ["mcpServers", "mcp"] {
            let Some(servers) = root.get_mut(key).and_then(Value::as_object_mut) else {
                continue;
            };
            let ours = servers.remove(SERVER_NAME).is_some();
            // A container **this uninstall emptied** is removed; one that was
            // already empty is the operator's unfilled slot, and an uninstall is
            // entitled to Estigia's entries and nothing else. See `strip_hooks`.
            if ours
                && servers.is_empty()
                && let Some(object) = root.as_object_mut()
            {
                object.remove(key);
            }
        }
    }
    if root == before {
        return Ok(existing.to_owned());
    }
    as_written(Some(existing), &root)
}

/// Removes Estigia's entry from ZCode's two-level `mcp.servers.<name>`. See
/// `strip_hooks` for the empty-container rule, carried through both levels
/// here.
fn strip_zcode_mcp(root: &mut Value) {
    let mut emptied_servers = false;
    if let Some(mcp) = root.get_mut("mcp").and_then(Value::as_object_mut) {
        if let Some(servers) = mcp.get_mut("servers").and_then(Value::as_object_mut) {
            let ours = servers.remove(SERVER_NAME).is_some();
            emptied_servers = ours && servers.is_empty();
        }
        if emptied_servers {
            mcp.remove("servers");
        }
    }
    let mcp_emptied = emptied_servers
        && root
            .get("mcp")
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty);
    if mcp_emptied && let Some(object) = root.as_object_mut() {
        object.remove("mcp");
    }
}

/// Codex keeps its servers in TOML, edited by the line; the caller's
/// parse-before-write guard refuses a removal that breaks the file.
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
/// Codex reads both. A **sub-table** such as `[mcp_servers.estigia.env]` is
/// deliberately not this question — see [`opens_estigia_section`].
pub(super) fn opens_estigia_table(line: &str) -> bool {
    let line = line.trim();
    line == format!("[mcp_servers.{SERVER_NAME}]")
        || line == format!("[mcp_servers.\"{SERVER_NAME}\"]")
}

/// Whether a line opens a section of Estigia's, in any of its three spellings:
/// one predicate, so which lines to lift out and whether there is anything to
/// lift out cannot drift apart.
fn opens_estigia_section(line: &str) -> bool {
    opens_estigia_table(line)
        || line
            .trim()
            .starts_with(&format!("[mcp_servers.{SERVER_NAME}."))
}

/// The lines under Estigia's table, up to the next table header: the one place
/// that answers *what does this file say about Estigia's server*.
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

fn is_table_header(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('[') && line.ends_with(']')
}

/// Lifts `[mcp_servers.estigia]` out, leaving every other section untouched.
///
/// The section ends at the next table header **or at a comment that no key
/// follows**: a `#` after the last key was typed by the operator, and running to
/// the next header swallowed it. A **key** under that header goes with it
/// deliberately — it was written under a table being removed — and so does a
/// comment sitting between the header and a later key, since leaving it would
/// strand the keys under whichever table precedes.
pub(super) fn strip_codex_mcp(existing: &str) -> String {
    let normalized = existing.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut kept = Vec::with_capacity(lines.len());
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if opens_estigia_section(line) {
            index += 1;
            while index < lines.len() && !is_table_header(lines[index]) {
                if lines[index].trim_start().starts_with('#') {
                    let next = (index..lines.len()).find(|&at| {
                        let text = lines[at].trim();
                        !text.is_empty() && !text.starts_with('#')
                    });
                    match next {
                        Some(at) if !is_table_header(lines[at]) => index = at,
                        _ => break,
                    }
                } else {
                    index += 1;
                }
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
///
/// The command a hook entry runs, in any dialect, is double-quoted: these are
/// handed to a shell, and bash treats a bare backslash as an escape. **Double**
/// quotes here and single quotes in the git hook is measured rather than chosen
/// — an agent's hook is read by whatever shell that agent uses, and `cmd` does
/// not quote with `'`. What that leaves open is named rather than closed: an
/// install directory containing `$(…)`, `` `…` `` or `${…}` misroutes the gate
/// under a shell that expands them.
fn hook_command(executable: &Path, agent: &str, dialect: Dialect) -> String {
    format!(
        "\"{}\" {}",
        executable.display(),
        hook_arguments(agent, dialect)
    )
}

/// What every registered gate is invoked with, whoever writes the command line.
///
/// One function because there were two writers and they had drifted: what is
/// shared is *which flags a gate is given*; the quoting stays with each writer.
pub(super) fn hook_arguments(agent: &str, dialect: Dialect) -> String {
    event_arguments(Event::PreToolUse, agent, dialect)
}

pub(super) fn event_arguments(event: Event, agent: &str, dialect: Dialect) -> String {
    format!(
        "hook {} --agent {} --dialect {}",
        event.slug(),
        agent,
        dialect.slug()
    )
}

const LOOP_STOP_TIMEOUT: u64 = 180;

const LOOP_EVENTS: [(Event, u64); 2] = [(Event::SessionStart, 5), (Event::Stop, LOOP_STOP_TIMEOUT)];

/// Whether an entry runs Estigia's gate, whatever path the binary had.
fn is_estigia_gate(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| runs_estigia_hook(command, Some(Event::PreToolUse)))
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
    // creating, or a blank one. Added to a file the operator already keeps,
    // it would survive an uninstall; and if Cursor treats a version-less file
    // as inert, writing the key turns on *their* hooks as a side effect of
    // installing ours.
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
    // comes back exactly as the operator wrote it.
    let before = root.clone();
    if let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) {
        // Emptied here, not merely empty — see `strip_hooks`.
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
    loop_events: bool,
) -> Result<String> {
    let mut root = Value::Object(object_at(path, existing)?);
    let Value::Object(object) = &mut root else {
        unreachable!("object_at returns a map")
    };
    // Every envelope this function serves wraps its events in `hooks`; writing
    // into the wrong envelope usually produces a file that parses, looks right
    // and never fires.
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

    // Hook commands are handed to a shell, and always quoted: bash treats a
    // backslash as an escape, so bare separators arrive eaten. Double quotes,
    // not single — see `hook_command`.
    let command_prefix = format!("\"{}\"", executable.display());

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
        let mut slot = None;
        let mut kept = Vec::with_capacity(entries.len());
        for mut entry in std::mem::take(entries) {
            if emptied_of_estigia(&mut entry) {
                slot.get_or_insert(kept.len());
            } else {
                kept.push(entry);
            }
        }
        *entries = kept;
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
        entries.insert(slot.unwrap_or(entries.len()), Value::Object(entry));
    }

    // The lifecycle events, which only Claude Code is known to have.
    if !spec.lifecycle {
        settle_loop_events(hooks, path, executable, agent, spec.dialect, loop_events)?;
        return as_written(existing, &root);
    }

    for (event, _, _) in crate::harness::hook::EVENTS {
        // The gate was written above; clearing it here would wipe what the
        // block just added — a lifecycle hook and no gate.
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
        // Claude Code's `Stop` carries no agent by default; with the loop on it
        // names its agent and dialect so the decision can tell the loop from the
        // ordinary claim-holding stop, exactly as Codex's own loop entry does.
        let (command, timeout) = if loop_events && *event == crate::harness::hook::Event::Stop {
            (
                format!(
                    "{command_prefix} {}",
                    event_arguments(*event, agent, spec.dialect)
                ),
                LOOP_STOP_TIMEOUT,
            )
        } else {
            (format!("{command_prefix} hook {}", event.slug()), *timeout)
        };
        entry.insert(
            "hooks".to_owned(),
            json!([{
                "type": "command",
                "command": command,
                "timeout": timeout,
            }]),
        );
        if let Some(list) = hooks
            .get_mut(event.agent_name())
            .and_then(Value::as_array_mut)
        {
            list.push(Value::Object(entry));
            if *event == crate::harness::hook::Event::Stop {
                list.push(json!({
                    "hooks": [{
                        "type": "command",
                        "command": format!(
                            "{command_prefix} {} --wait-mail",
                            event_arguments(*event, agent, spec.dialect)
                        ),
                        "asyncRewake": true,
                        "timeout": crate::harness::mailbox::WAIT_TIMEOUT_SECONDS,
                    }],
                }));
            }
        }
    }

    as_written(existing, &root)
}

fn settle_loop_events(
    hooks: &mut Map<String, Value>,
    path: &Path,
    executable: &Path,
    agent: &str,
    dialect: Dialect,
    on: bool,
) -> Result<()> {
    for (event, timeout) in LOOP_EVENTS {
        let name = event.agent_name();
        if !on && !hooks.contains_key(name) {
            continue;
        }
        let entries = hooks
            .entry(name.to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        if entries.is_null() {
            *entries = Value::Array(Vec::new());
        }
        let entries = entries
            .as_array_mut()
            .ok_or_else(|| NotEditable::WrongShape {
                what: format!("hooks.{name}"),
                path: path.display().to_string(),
                shape: "an array",
            })?;
        let tail = format!(" {}", event_arguments(event, agent, dialect));
        let mut slot = None;
        let mut kept = Vec::with_capacity(entries.len());
        for mut entry in std::mem::take(entries) {
            if emptied_of_loop_command(&mut entry, &tail) {
                slot.get_or_insert(kept.len());
            } else {
                kept.push(entry);
            }
        }
        let removed = slot.is_some();
        *entries = kept;
        if on {
            entries.insert(
                slot.unwrap_or(entries.len()),
                json!({
                    "hooks": [{
                        "type": "command",
                        "command": format!("\"{}\"{tail}", executable.display()),
                        "timeout": timeout,
                    }],
                }),
            );
        } else if entries.is_empty() && removed {
            hooks.remove(name);
        }
    }
    Ok(())
}

fn emptied_of_loop_command(entry: &mut Value, tail: &str) -> bool {
    let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
        return false;
    };
    let held = inner.len();
    inner.retain(|hook| {
        !hook
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| command.ends_with(tail))
    });
    inner.len() != held && inner.is_empty()
}

/// Drops Estigia's hooks, keeping every other tool's.
pub(super) fn strip_hooks(path: &Path, existing: &str) -> Result<String> {
    let mut root: Value =
        serde_json::from_str(without_mark(existing)).map_err(|error| NotEditable::Unparseable {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    // What it looked like before, so a file with nothing of Estigia's in it
    // comes back exactly as the operator wrote it.
    let before = root.clone();
    // Three envelopes again. Removing from the wrapper only would leave Codex's
    // entry in place — an uninstall that reports success and leaves the gate
    // running.
    let bare = root.get("hooks").is_none();
    let events = if bare {
        root.as_object_mut()
    } else {
        root.get_mut("hooks").and_then(Value::as_object_mut)
    };
    // Which keys this uninstall emptied, as opposed to which ones are empty:
    // an event left with no hooks *by us* was not there before Estigia arrived,
    // but an operator's own empty slot — one they meant to fill — was.
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
    // The same rule one level up: a wrapper is ours to drop only when what
    // emptied it was us.
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
pub(super) fn is_estigia_command(hook: &Value) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| runs_estigia_hook(command, None))
}

/// Whether a command line is `<program> hook <event>`: the shape `hook_command`
/// writes, whatever the program is called, `only` narrowing it to one event.
pub(super) fn runs_estigia_hook(command: &str, only: Option<Event>) -> bool {
    let words = super::wiring::tokenise(command);
    matches!(words.as_slice(), [_, hook, slug, ..] if hook == "hook"
        && Event::from_slug(slug).is_some_and(|event| only.is_none_or(|only| only == event)))
}

/// Takes Estigia's commands out of one group, and says whether that emptied it.
///
/// The group is a **matcher and a list**, so it is dropped whole only when this
/// removal emptied it — an operator who added their own command beside Estigia's
/// keeps it. The group survives carrying Estigia's matcher, the lesser of the
/// two costs: a wider matcher costs a hook firing more often, dropping the group
/// costs the hook.
fn emptied_of_estigia(entry: &mut Value) -> bool {
    let Some(inner) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
        return false;
    };
    let held = inner.len();
    inner.retain(|hook| !is_estigia_command(hook));
    inner.len() != held && inner.is_empty()
}
