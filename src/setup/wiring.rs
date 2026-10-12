//! Reading back the gate somebody else's settings file says is installed.
//!
//! The wiring is read back and checked rather than inferred from the presence
//! of an entry: **installed, looks installed, enforces nothing** is the exact
//! shape this project exists to refuse. Deliberately shape-agnostic — what
//! every dialect has in common is a command line naming this executable and an
//! event, so that is what is looked for.

use std::path::{Path, PathBuf};

use super::{AgentAdapter, SetupOptions};

/// One registered call into Estigia, as a settings file has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wire {
    /// The whole command, as written.
    pub command: String,
    /// The executable it names.
    pub executable: PathBuf,
    /// The event it names, if this build has one by that name.
    pub event: Option<&'static str>,
    /// The event name as written, whether or not this build knows it.
    pub named: String,
}

impl Wire {
    /// Whether this entry would actually gate anything; the same question
    /// [`Wire::fault`] answers, said in terms of it.
    pub fn is_live(&self) -> bool {
        self.fault().is_none()
    }

    /// What is wrong with it, if anything.
    pub fn fault(&self) -> Option<String> {
        if self.event.is_none() {
            return Some(format!(
                "names `{}`, which this build has no event for — the call is refused and the \
                 tool goes through ungated",
                self.named
            ));
        }
        if !self.executable.is_file() {
            return Some(format!(
                "runs `{}`, which is not there — the agent's call fails and the tool goes \
                 through ungated",
                self.executable.display()
            ));
        }
        None
    }
}

/// The tool names a matcher mentions, lowercased.
pub fn names_in(matcher: &str) -> Vec<String> {
    matcher
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

/// Estigia's own gate entries whose matcher wakes for nothing it can judge.
///
/// Structured, and only for the envelope where the shape is unambiguous: an
/// object carrying both `matcher` and a `hooks` array with an Estigia call in
/// it. Gemini, Windsurf and Crush spell their entries differently and are not
/// read here. Absent is not narrow: no `matcher` at all means *every tool*,
/// which is wide rather than broken.
pub fn narrowed(text: &str, judged: &[&str]) -> Vec<String> {
    let Some(root) = super::render::parse_settings(text) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    walk_entries(&root, judged, &mut found);
    found
}

fn walk_entries(value: &serde_json::Value, judged: &[&str], into: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                walk_entries(item, judged, into);
            }
        }
        serde_json::Value::Object(fields) => {
            if let Some(matcher) = fields.get("matcher").and_then(serde_json::Value::as_str)
                && let Some(hooks) = fields.get("hooks")
                && ours(hooks)
                && !matcher.trim().is_empty()
                && !names_in(matcher)
                    .iter()
                    .any(|name| judged.contains(&name.as_str()))
            {
                into.push(matcher.to_owned());
            }
            for field in fields.values() {
                walk_entries(field, judged, into);
            }
        }
        _ => {}
    }
}

/// Whether this `hooks` value carries a call of Estigia's.
fn ours(hooks: &serde_json::Value) -> bool {
    let mut lines = Vec::new();
    collect_strings(hooks, &mut lines);
    lines.iter().any(|line| wire_in(line).is_some())
}

/// The gate a plugin registers, for an agent that loads one rather than an
/// entry in a settings file.
///
/// [`wires`] reads command lines, and a plugin's call never has `hook` and an
/// event name adjacent, so that reader finds nothing — and `doctor` reported a
/// file naming a binary as *no wiring here to be wrong*. The event is this
/// build's `pre-tool-use`; `named` carries the host's own spelling.
fn plugin_wire(text: &str) -> Option<Wire> {
    if !text.contains(super::plugin::MARKER) {
        return None;
    }
    // The one line that names the binary, written by `plugin::source` as a
    // JSON string; read back the same way rather than by trimming quotes.
    let at = text.find("const ESTIGIA = ")?;
    let rest = &text[at + "const ESTIGIA = ".len()..];
    let end = rest.find(';')?;
    let executable: String = serde_json::from_str(rest[..end].trim()).ok()?;
    Some(Wire {
        command: format!("{executable} gate <tool> --input <args>"),
        executable: PathBuf::from(executable),
        event: Some(crate::harness::hook::Event::PreToolUse.slug()),
        named: if text.contains(&format!("pi.on(\"{}\"", super::plugin::PI_GATE_EVENT)) {
            super::plugin::PI_GATE_EVENT
        } else {
            "tool.execute.before"
        }
        .to_owned(),
    })
}

/// Every Estigia call one settings file registers.
///
/// Fed the text rather than a path, so the absent, unreadable and unparseable
/// cases can be shown to be handled.
pub fn wires(text: &str) -> Vec<Wire> {
    // JSON where it is JSON: the commands inside are escaped, and reading them
    // as text would hand back a doubled-backslash path that does not exist.
    let candidates: Vec<String> = match super::render::parse_settings(text) {
        Some(value) => {
            let mut found = Vec::new();
            collect_strings(&value, &mut found);
            found
        }
        None => super::render::without_mark(text)
            .lines()
            .map(str::to_owned)
            .collect(),
    };
    candidates.iter().filter_map(|line| wire_in(line)).collect()
}

/// Every string in a JSON document, however deep.
fn collect_strings(value: &serde_json::Value, into: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => into.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_strings(item, into);
            }
        }
        serde_json::Value::Object(fields) => {
            for field in fields.values() {
                collect_strings(field, into);
            }
        }
        _ => {}
    }
}

/// The Estigia call one command line makes, if it makes one.
fn wire_in(line: &str) -> Option<Wire> {
    let tokens = tokenise(line);
    if let Some(at) = tokens.iter().position(|token| token == "hook") {
        // The executable is the token immediately before `hook`, not the first
        // on the line: two dialects register a program that calls out to it.
        let executable = tokens.get(at.checked_sub(1)?)?;
        let named = tokens.get(at + 1)?.clone();
        // Ours, not somebody else's `hook` subcommand. Matched on the file name
        // so a moved copy still counts; a renamed one does not, though
        // `setup::is_gated` still sees it, by its `<program> hook <event>` shape.
        let stem = Path::new(executable)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !stem.contains("estigia") {
            return None;
        }
        return Some(Wire {
            command: line.trim().to_owned(),
            executable: PathBuf::from(executable),
            event: crate::harness::hook::Event::from_slug(&named).map(|event| event.slug()),
            named,
        });
    }
    // A whole command line sitting inside a quoted string, which is how a
    // plugin calls out to it. This terminates: the inner text has no quotes
    // left, so the next pass splits it into words.
    tokens
        .iter()
        .filter(|token| token.contains(" hook "))
        .find_map(|token| wire_in(token))
}

/// One command line, split into its words, honouring quotes: a quoted path
/// with a space in it would otherwise be reported as a fault that is not there.
pub(super) fn tokenise(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for character in line.chars() {
        match (quote, character) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), c) => current.push(c),
            (None, c @ ('"' | '\'')) => quote = Some(c),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            (None, c) => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// One adapter's registration: every file it uses, and what each one says.
pub type Registration = Vec<(PathBuf, Vec<Wire>)>;

/// Every file one adapter registers Estigia in, and what each one says. A file
/// that is absent contributes nothing rather than an error.
pub fn registered(adapter: &AgentAdapter, options: &SetupOptions) -> Registration {
    let Ok(paths) = super::resolve_paths(adapter, options) else {
        return Vec::new();
    };
    [paths.hooks, paths.plugin]
        .into_iter()
        .flatten()
        .filter_map(|file| {
            let text = std::fs::read_to_string(&file).ok()?;
            // The command-line reader first: Cline's hook **is** a command
            // line. The plugin reader is the fallback for the one shape those
            // two words never appear side by side in.
            let found = match wires(&text) {
                found if !found.is_empty() => found,
                _ => vec![plugin_wire(&text)?],
            };
            Some((file, found))
        })
        .collect()
}

#[cfg(test)]
mod tests;
