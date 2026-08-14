//! Reading back the gate somebody else's settings file says is installed.
//!
//! # The gap this closes
//!
//! `estigia status` reports `gate on` for an agent whose settings file carries a
//! hook entry. It does not read what that entry *says*. On the machine this was
//! written on, the registered command was:
//!
//! ```text
//! H:\REPO\estigia\target\debug\estigia.exe hook pre-tool-use
//! ```
//!
//! — a debug build inside a working tree. `cargo clean` deletes it. Moving the
//! checkout moves it. Either way the agent goes on calling a command that is not
//! there, the call fails, the tool proceeds, and Estigia keeps saying `gate on`.
//!
//! That is the exact shape this project exists to refuse: **installed, looks
//! installed, enforces nothing.** So the wiring is read back and checked, rather
//! than inferred from the presence of an entry.
//!
//! Deliberately shape-agnostic. Four dialects write JSON of three different
//! shapes and two write a program; parsing each properly would be five parsers
//! to keep in step with five upstreams. What every one of them has in common is
//! a **command line naming this executable and an event**, so that is what is
//! looked for.

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
    ///
    /// `None` is a settings file naming something this binary will refuse —
    /// which for a `PreToolUse` hook is a non-blocking error, so the call goes
    /// through ungated with one line in a transcript nobody reads.
    pub event: Option<&'static str>,
    /// The event name as written, whether or not this build knows it.
    pub named: String,
}

impl Wire {
    /// Whether this entry would actually gate anything.
    ///
    /// The same question [`Wire::fault`] answers, and said in terms of it. It
    /// used to be a second copy of the conditions — `event.is_some() &&
    /// executable.is_file()` — sitting in this impl block beside the one the
    /// doctor reads. They agreed; nothing made them, and the tests exercised
    /// the copy while `doctor::gates` ran the original.
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
///
/// One reader, because two questions ask it: whether the gate this crate is
/// about to install wakes for tools the classifier judges, and whether the one
/// already on disk still does. `setup::tests` had its own copy of this split
/// and this is now the only one.
pub fn names_in(matcher: &str) -> Vec<String> {
    matcher
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

/// Estigia's own gate entries whose matcher wakes for nothing it can judge.
///
/// The `gate` row promises *whether the gate this agent registers would
/// actually run*, and the matcher is the field that decides which tools it runs
/// **for** — the one thing `Wire` could not see, because [`wires`] reads command
/// lines and a matcher is not one. So a settings file whose matcher had been
/// narrowed to a tool that does not exist reported `3 live` and never fired for
/// a single write: installed, looking installed, deciding nothing, which is the
/// failure this whole crate is written against.
///
/// Structured rather than by line, and only for the envelope where the shape is
/// unambiguous: an object carrying both `matcher` and a `hooks` array with an
/// Estigia call in it. Gemini, Windsurf and Crush spell their entries
/// differently and are not read here — a matcher this cannot see is one it says
/// nothing about, which is the same stance `wires` takes about a plugin.
///
/// Absent is not narrow. No `matcher` at all means *every tool*, which is wide
/// rather than broken, and the honesty contract already carries what that costs.
pub fn narrowed(text: &str, judged: &[&str]) -> Vec<String> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(text) else {
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
/// [`wires`] reads **command lines**: it looks for `hook` and an event name
/// next to each other. OpenCode's plugin is JavaScript and calls `estigia gate
/// <tool> --input <payload>` through a template literal, so those two words are
/// never adjacent and that reader found nothing at all. `doctor` then reported
///
/// > opencode: gated by its own file rather than a settings entry, **so there
/// > is no wiring here to be wrong**
///
/// which is a sentence about a file naming a binary. Measured: with the plugin
/// pointed at a path that does not exist, `cline` answered *the gate is
/// registered and would not run* and `opencode` answered that there was nothing
/// to be wrong.
///
/// The event is this build's `pre-tool-use` because that is the decision the
/// plugin asks for; `named` carries OpenCode's own spelling, so the report says
/// what the file says.
fn plugin_wire(text: &str) -> Option<Wire> {
    if !text.contains(super::plugin::MARKER) {
        return None;
    }
    // The one line that names the binary, written by `plugin::source` as a
    // JSON string so a Windows path's backslashes survive into the module —
    // and read back the same way rather than by trimming quotes.
    let at = text.find("const ESTIGIA = ")?;
    let rest = &text[at + "const ESTIGIA = ".len()..];
    let end = rest.find(';')?;
    let executable: String = serde_json::from_str(rest[..end].trim()).ok()?;
    Some(Wire {
        command: format!("{executable} gate <tool> --input <args>"),
        executable: PathBuf::from(executable),
        event: Some(crate::harness::hook::Event::PreToolUse.slug()),
        named: "tool.execute.before".to_owned(),
    })
}

/// Every Estigia call one settings file registers.
///
/// Fed the text rather than a path, for the usual reason: the interesting cases
/// are a file that is absent, one that is unreadable and one that holds JSON
/// this build cannot parse, and a function that goes looking cannot be shown to
/// handle any of them.
pub fn wires(text: &str) -> Vec<Wire> {
    // JSON where it is JSON, because the commands inside are escaped and
    // reading them as text would hand back `H:\\REPO\\...` — a path that does
    // not exist, reported as a fault that is not there.
    let candidates: Vec<String> = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(value) => {
            let mut found = Vec::new();
            collect_strings(&value, &mut found);
            found
        }
        Err(_) => text.lines().map(str::to_owned).collect(),
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
        // The executable is the token **immediately** before `hook`, not the
        // first on the line: two dialects register a program that calls out to
        // it, and there the line starts with somebody else's function.
        let executable = tokens.get(at.checked_sub(1)?)?;
        let named = tokens.get(at + 1)?.clone();
        // Ours, not somebody else's `hook` subcommand. Matched on the file name
        // so a **moved** copy still counts — an operator who put it somewhere of
        // their own still installed this.
        //
        // A *renamed* one does not, and the comment here said it did. The guard
        // is what stops Estigia claiming another program's `hook` subcommand, so
        // it stays; what changed is that the disagreement it produces is now
        // reported. `setup::is_gated` recognises the same entry by `hook
        // pre-tool-use` alone, so a copy called something else is plain to that
        // reader and invisible to this one — and `doctor`'s gate row said *"there
        // is no wiring here to be wrong"* about exactly that entry.
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
    // plugin calls out to it. Re-read that string as the command line it is.
    // This terminates: the inner text has no quotes left to hold it together,
    // so the next pass splits it into words.
    tokens
        .iter()
        .filter(|token| token.contains(" hook "))
        .find_map(|token| wire_in(token))
}

/// One command line, split into its words, honouring quotes.
///
/// A path with a space in it is written quoted, and splitting on whitespace
/// alone would report the executable as its own first directory — a fault that
/// is not there, on a machine where everything works.
fn tokenise(line: &str) -> Vec<String> {
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

/// Every file one adapter registers Estigia in, and what each one says.
///
/// A file that is absent contributes nothing rather than an error: an adapter
/// Estigia was never installed into has no wiring to be wrong, and reporting
/// that as a fault would make a clean machine look broken.
pub fn registered(adapter: &AgentAdapter, options: &SetupOptions) -> Registration {
    let Ok(paths) = super::resolve_paths(adapter, options) else {
        return Vec::new();
    };
    [paths.hooks, paths.plugin]
        .into_iter()
        .flatten()
        .filter_map(|file| {
            let text = std::fs::read_to_string(&file).ok()?;
            // The command-line reader first, because Cline's hook **is** a
            // command line and matches it. The plugin reader is the fallback,
            // for the one shape those two words never appear side by side in.
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
