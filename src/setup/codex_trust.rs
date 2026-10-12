use std::path::Path;

use anyhow::Result;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use toml_edit::{DocumentMut, Item, Table, value};

use super::render::{NotEditable, event_arguments, without_mark};
use crate::harness::hook::{Dialect, Event};

const DEFAULT_TIMEOUT_SECONDS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Entry {
    pub key: String,
    pub hash: String,
}

/// What Codex will do with Estigia's gate, when it will not run it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// What is wrong, for the row that reports it.
    pub detail: String,
    /// Whether `estigia setup` writes what clears it.
    pub cleared_by_setup: bool,
    /// What to do instead, when setup does not clear it.
    pub advice: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct State {
    pub trusted_hash: Option<String>,
    pub enabled: Option<bool>,
}

fn event_label(event: &str) -> String {
    let mut label = String::new();
    for (index, character) in event.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                label.push('_');
            }
            label.push(character.to_ascii_lowercase());
        } else {
            label.push(character);
        }
    }
    label
}

fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|key| (key.clone(), canonical(&map[key])))
                    .collect::<Map<_, _>>(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

fn hook_hash(label: &str, matcher: Option<&str>, handler: &Value) -> Option<String> {
    if handler.get("type").and_then(Value::as_str) != Some("command")
        || matches!(label, "session_end" | "interrupt")
    {
        return None;
    }
    let command = handler.get("command").and_then(Value::as_str)?;
    let timeout = handler
        .get("timeout")
        .map_or(Some(DEFAULT_TIMEOUT_SECONDS), Value::as_u64)?
        .max(1);
    let mut normalised = Map::new();
    normalised.insert("type".to_owned(), Value::String("command".to_owned()));
    normalised.insert("command".to_owned(), Value::String(command.to_owned()));
    normalised.insert("timeout".to_owned(), Value::from(timeout));
    normalised.insert(
        "async".to_owned(),
        Value::Bool(handler.get("async").and_then(Value::as_bool) == Some(true)),
    );
    if let Some(message) = handler.get("statusMessage").and_then(Value::as_str) {
        normalised.insert(
            "statusMessage".to_owned(),
            Value::String(message.to_owned()),
        );
    }
    let mut identity = Map::new();
    identity.insert("event_name".to_owned(), Value::String(label.to_owned()));
    identity.insert(
        "hooks".to_owned(),
        Value::Array(vec![Value::Object(normalised)]),
    );
    let matcher = matcher.filter(|_| !matches!(label, "user_prompt_submit" | "stop"));
    if let Some(matcher) = matcher {
        identity.insert("matcher".to_owned(), Value::String(matcher.to_owned()));
    }
    let bytes = serde_json::to_vec(&canonical(&Value::Object(identity))).ok()?;
    let digest = Sha256::digest(bytes);
    Some(format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

pub(super) fn owned(hooks_path: &Path, hooks_text: &str) -> Vec<Entry> {
    let Ok(root) = serde_json::from_str::<Value>(without_mark(hooks_text)) else {
        return Vec::new();
    };
    let Some(events) = root.get("hooks").and_then(Value::as_object) else {
        return Vec::new();
    };
    let tails = [Event::PreToolUse, Event::Stop, Event::SessionStart].map(|event| {
        (
            event_label(event.agent_name()),
            format!(" {}", event_arguments(event, "codex", Dialect::ClaudeCode)),
        )
    });
    let mut found = Vec::new();
    for (event, groups) in events {
        let label = event_label(event);
        let Some(groups) = groups.as_array() else {
            continue;
        };
        for (group_index, group) in groups.iter().enumerate() {
            let matcher = group.get("matcher").and_then(Value::as_str);
            let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
                continue;
            };
            for (handler_index, handler) in handlers.iter().enumerate() {
                let command = handler.get("command").and_then(Value::as_str);
                if !command.is_some_and(|command| {
                    tails
                        .iter()
                        .any(|(owned, tail)| *owned == label && command.ends_with(tail))
                }) {
                    continue;
                }
                let Some(hash) = hook_hash(&label, matcher, handler) else {
                    continue;
                };
                found.push(Entry {
                    key: format!(
                        "{}:{label}:{group_index}:{handler_index}",
                        hooks_path.display()
                    ),
                    hash,
                });
            }
        }
    }
    found
}

/// The trust Estigia owns in `hooks_text`, by key: the one condition the
/// uninstall's parse guard and its parse step both read.
pub(super) fn trusted_keys(hooks_path: &Path, hooks_text: &str) -> Vec<String> {
    owned(hooks_path, hooks_text)
        .into_iter()
        .map(|entry| entry.key)
        .collect()
}

/// Parse the file Estigia is about to read or edit.
///
/// Empty and whitespace-only text is the empty document, which is how `setup`
/// meets a config it has not written yet.
fn parse(path: &Path, config: &str) -> Result<DocumentMut> {
    if config.trim().is_empty() {
        return Ok(DocumentMut::new());
    }
    config.parse::<DocumentMut>().map_err(|error| {
        NotEditable::UnparseableToml {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
        .into()
    })
}

/// Refuse a config Estigia cannot parse, without editing it.
///
/// The uninstall removes its trust from this file and so parses it; checking
/// here lets the caller refuse before the first removal rather than after it.
pub(super) fn ensure_parseable(path: &Path, config: &str) -> Result<()> {
    parse(path, config).map(|_| ())
}

/// The trust Codex would read for `key`, by TOML's rules rather than by lines.
///
/// `["hooks.state"]` is a different key and is not read as one.
fn state_of_doc(doc: &DocumentMut, key: &str) -> State {
    let Some(entry) = doc
        .get("hooks")
        .and_then(|hooks| hooks.get("state"))
        .and_then(|state| state.get(key))
    else {
        return State::default();
    };
    State {
        trusted_hash: entry
            .get("trusted_hash")
            .and_then(Item::as_str)
            .map(str::to_owned),
        enabled: entry.get("enabled").and_then(Item::as_bool),
    }
}

#[cfg(test)]
pub(super) fn state_of(config: &str, key: &str) -> State {
    match config.parse::<DocumentMut>() {
        Ok(doc) => state_of_doc(&doc, key),
        Err(_) => State::default(),
    }
}

pub(super) fn skipped(
    hooks: &Path,
    config: &Path,
    entries: &[Entry],
    text: &str,
) -> Option<Skipped> {
    if entries.is_empty() {
        return None;
    }
    let doc = match parse(config, text) {
        Ok(doc) => doc,
        Err(_) => {
            return Some(Skipped {
                detail: format!(
                    "whether Codex trusts the gate cannot be told: {} is not TOML",
                    config.display()
                ),
                cleared_by_setup: false,
                advice: "make it valid TOML, then run `estigia setup codex`".to_owned(),
            });
        }
    };
    if unmergeable_doc(config, &doc).is_err() {
        return Some(Skipped {
            detail: format!(
                "whether Codex trusts the gate cannot be told: {} defines hooks or hooks.state in a \
                 shape setup refuses to edit",
                config.display()
            ),
            cleared_by_setup: false,
            advice: "move those entries into [hooks.state.\"<key>\"] tables, then run `estigia setup codex`".to_owned(),
        });
    }
    entries.iter().find_map(|entry| {
        let state = state_of_doc(&doc, &entry.key);
        let ours = format!("`{}` in {}", entry.key, hooks.display());
        if state.enabled == Some(false) {
            return Some(Skipped {
                detail: format!(
                    "Codex skips the gate: `enabled = false` under [hooks.state.\"{}\"] in {}",
                    entry.key,
                    config.display()
                ),
                cleared_by_setup: false,
                advice: "remove the `enabled = false` line, which is a choice made in Codex's own configuration".to_owned(),
            });
        }
        match state.trusted_hash.as_deref() {
            Some(held) if held == entry.hash => None,
            Some(_) => Some(Skipped {
                detail: format!(
                    "Codex skips the gate: {} trusts another hash than the current one for {ours}, \
                     so the entry changed after it was trusted",
                    config.display()
                ),
                cleared_by_setup: true,
                advice: String::new(),
            }),
            None => Some(Skipped {
                detail: format!(
                    "Codex skips the gate: {} holds no trust for {ours}, so a Codex that enforces hook \
                     trust (0.159 measured; the field exists from 0.129) runs nothing from it",
                    config.display()
                ),
                cleared_by_setup: true,
                advice: String::new(),
            }),
        }
    })
}

/// Refuse a file whose `hooks` or `hooks.state` cannot hold a sub-table.
fn unmergeable_doc(path: &Path, doc: &DocumentMut) -> Result<()> {
    let named = [
        ("hooks", doc.get("hooks")),
        (
            "hooks.state",
            doc.get("hooks").and_then(|hooks| hooks.get("state")),
        ),
    ];
    for (what, item) in named {
        if item.is_some_and(|item| item.as_table().is_none()) {
            return Err(NotEditable::WrongShape {
                what: what.to_owned(),
                path: path.display().to_string(),
                shape: "a table, not an inline table",
            }
            .into());
        }
    }
    Ok(())
}

pub(super) fn unmergeable(path: &Path, config: &str) -> Result<()> {
    let doc = parse(path, config)?;
    unmergeable_doc(path, &doc)
}

/// Put back the line ending and byte-order mark toml_edit normalises away.
fn restore_envelope(config: &str, text: &str) -> String {
    let mut text = if config.contains("\r\n") {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_owned()
    };
    if config.starts_with('\u{feff}') && !text.starts_with('\u{feff}') {
        text.insert(0, '\u{feff}');
    }
    text
}

fn ensure_table<'a>(table: &'a mut Table, key: &str) -> Option<&'a mut Table> {
    if !table.contains_key(key) {
        let mut created = Table::new();
        // Implicit, so the table it only exists to hold does not print an empty
        // header of its own above the real one.
        created.set_implicit(true);
        table.insert(key, Item::Table(created));
    }
    table.get_mut(key).and_then(Item::as_table_mut)
}

fn set_trust(path: &Path, doc: &mut DocumentMut, entry: &Entry) -> Result<()> {
    let Some(hooks) = ensure_table(doc.as_table_mut(), "hooks") else {
        return Err(NotEditable::WrongShape {
            what: "hooks".to_owned(),
            path: path.display().to_string(),
            shape: "a table of hooks",
        }
        .into());
    };
    let Some(state) = ensure_table(hooks, "state") else {
        return Err(NotEditable::WrongShape {
            what: "hooks.state".to_owned(),
            path: path.display().to_string(),
            shape: "a table of hook states",
        }
        .into());
    };
    if state
        .get(&entry.key)
        .and_then(|held| held.get("trusted_hash"))
        .and_then(Item::as_str)
        .is_some_and(|held| held == entry.hash)
    {
        return Ok(());
    }
    if !state.contains_key(&entry.key) {
        state.insert(&entry.key, Item::Table(Table::new()));
    }
    let Some(entry_table) = state.get_mut(&entry.key) else {
        return Err(NotEditable::WrongShape {
            what: entry.key.clone(),
            path: path.display().to_string(),
            shape: "a [hooks.state.\"<key>\"] table",
        }
        .into());
    };
    if entry_table.as_table_mut().is_none() && entry_table.as_inline_table_mut().is_none() {
        return Err(NotEditable::WrongShape {
            what: entry.key.clone(),
            path: path.display().to_string(),
            shape: "a [hooks.state.\"<key>\"] table",
        }
        .into());
    }
    entry_table["trusted_hash"] = value(entry.hash.as_str());
    Ok(())
}

fn remove_trust(doc: &mut DocumentMut, key: &str) {
    let Some(hooks) = doc.get_mut("hooks").and_then(Item::as_table_mut) else {
        return;
    };
    let Some(state) = hooks.get_mut("state").and_then(Item::as_table_mut) else {
        return;
    };
    let emptied = {
        let Some(entry) = state.get_mut(key) else {
            return;
        };
        if let Some(table) = entry.as_table_mut() {
            table.remove("trusted_hash");
            table.is_empty()
        } else if let Some(table) = entry.as_inline_table_mut() {
            table.remove("trusted_hash");
            table.is_empty()
        } else {
            false
        }
    };
    if emptied {
        state.remove(key);
    }
}

/// Refuse a text that does not parse, or does not read back `entries`' trust.
pub(super) fn verify(path: &Path, text: &str, entries: &[Entry]) -> Result<()> {
    let doc = parse(path, text)?;
    for entry in entries {
        if state_of_doc(&doc, &entry.key).trusted_hash.as_deref() != Some(entry.hash.as_str()) {
            return Err(NotEditable::TrustNotWritten {
                path: path.display().to_string(),
                key: entry.key.clone(),
            }
            .into());
        }
    }
    Ok(())
}

pub(super) fn with_trust(
    path: &Path,
    config: &str,
    stale: &[String],
    entries: &[Entry],
) -> Result<String> {
    let mut doc = parse(path, config)?;
    unmergeable_doc(path, &doc)?;
    for key in stale
        .iter()
        .filter(|key| !entries.iter().any(|entry| &entry.key == *key))
    {
        remove_trust(&mut doc, key);
    }
    for entry in entries {
        set_trust(path, &mut doc, entry)?;
    }
    let text = restore_envelope(config, &doc.to_string());
    verify(path, &text, entries)?;
    Ok(text)
}

pub(super) fn without_trust(path: &Path, config: &str, keys: &[String]) -> Result<String> {
    if keys.is_empty() {
        return Ok(config.to_owned());
    }
    let mut doc = parse(path, config)?;
    for key in keys {
        remove_trust(&mut doc, key);
    }
    Ok(restore_envelope(config, &doc.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hash_of(label: &str, matcher: Option<&str>, handler: Value) -> String {
        hook_hash(label, matcher, &handler).expect("a command handler")
    }

    #[test]
    fn hashes_equal_the_ones_codex_0_159_2_reports_for_the_same_hooks() {
        let estigia =
            "\"/usr/local/bin/estigia\" hook pre-tool-use --agent codex --dialect claude-code";
        let vectors = [
            (
                "pre_tool_use",
                Some("^(Bash|apply_patch|Write|Edit)$"),
                json!({"type": "command", "command": estigia, "timeout": 10}),
                "sha256:88df9b97a2a7fd4a13776a34da9bc93f77018b2a5911b48f08269fdaf1a8002f",
            ),
            (
                "pre_tool_use",
                Some("Bash"),
                json!({"type": "command", "command": "echo \"héllo\" \\ 'x'"}),
                "sha256:2472b86b8508797edb8a9b84193ff23dbc0ae4308965bf291d2ec33a409be943",
            ),
            (
                "pre_tool_use",
                Some("Bash"),
                json!({"type": "command", "command": "b", "timeout": 0}),
                "sha256:00ebda7daecd6bedc53e6c5770909f8a4045f8c4a90ca3c34a0b39e91788c0c8",
            ),
            (
                "pre_tool_use",
                None,
                json!({"type": "command", "command": "c", "statusMessage": "checking", "async": true, "timeout": 7}),
                "sha256:dd474d980772eb7dd52275c2da68ae60f08aa576cd6af2e422eb5496fcbaa79f",
            ),
            (
                "session_start",
                Some("startup|resume"),
                json!({"type": "command", "command": "t"}),
                "sha256:5d0ce724c3c50dc95f3fb3f39646ad007bc215937edebd50fcd9386e62995580",
            ),
            (
                "stop",
                Some("ignored"),
                json!({"type": "command", "command": "s", "timeout": 3}),
                "sha256:5e5da534cd9e1bb877f926af5591b1a5e3486bced1781f0bcaacb35b4e01cf1d",
            ),
        ];
        for (label, matcher, handler, expected) in vectors {
            assert_eq!(
                hash_of(label, matcher, handler.clone()),
                expected,
                "{handler}"
            );
        }
    }

    #[test]
    fn a_handler_codex_would_not_hash_as_a_command_has_no_hash() {
        assert_eq!(
            hook_hash("pre_tool_use", None, &json!({"type": "agent"})),
            None
        );
        assert_eq!(
            hook_hash(
                "session_end",
                None,
                &json!({"type": "command", "command": "x"})
            ),
            None
        );
    }

    #[test]
    fn event_labels_are_the_snake_case_of_the_event_name() {
        for (name, label) in [
            ("PreToolUse", "pre_tool_use"),
            ("UserPromptSubmit", "user_prompt_submit"),
            ("Stop", "stop"),
            ("SubagentStart", "subagent_start"),
        ] {
            assert_eq!(event_label(name), label);
        }
    }

    fn hooks_file() -> String {
        json!({"hooks": {"PreToolUse": [
            {"matcher": "Bash", "hooks": [{"type": "command", "command": "theirs"}]},
            {"matcher": "Bash", "hooks": [
                {"type": "command", "command": "theirs"},
                {"type": "command", "command": "\"/bin/estigia\" hook pre-tool-use --agent codex --dialect claude-code", "timeout": 10}
            ]}
        ]}})
        .to_string()
    }

    #[test]
    fn the_key_is_the_hooks_file_the_event_the_group_and_the_handler() {
        let found = owned(Path::new("/home/me/.codex/hooks.json"), &hooks_file());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].key, "/home/me/.codex/hooks.json:pre_tool_use:1:1");
        assert!(found[0].hash.starts_with("sha256:"));
    }

    #[test]
    fn a_foreign_handler_that_merely_names_a_hook_event_is_not_ours() {
        let text = json!({"hooks": {
            "Stop": [{"hooks": [{"type": "command", "command": "/usr/bin/githook stop"}]}],
            "PreToolUse": [{"hooks": [
                {"type": "command", "command": "/usr/bin/githook pre-tool-use --agent codex --dialect claude-code"},
                {"type": "command", "command": "\"/x/estigia\" hook pre-tool-use --agent codex --dialect claude-code"}
            ]}]
        }})
        .to_string();
        let found = owned(Path::new("/h/hooks.json"), &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].key.ends_with(":pre_tool_use:0:1"), "{found:?}");
    }

    #[test]
    fn only_the_stop_and_session_start_commands_setup_writes_are_ours() {
        let ours = |event: &str| {
            format!("\"/x/estigia\" hook {event} --agent codex --dialect claude-code")
        };
        let text = json!({"hooks": {
            "Stop": [{"hooks": [
                {"type": "command", "command": ours("stop"), "timeout": 180},
                {"type": "command", "command": ours("session-start")},
                {"type": "command", "command": "\"/x/estigia\" hook stop --agent claude-code --dialect claude-code"},
                {"type": "command", "command": "\"/x/estigia\" hook stop --agent codex --dialect gemini-cli"},
                {"type": "command", "command": "/usr/bin/githook stop --agent codex --dialect claude-code"}
            ]}],
            "SessionStart": [{"hooks": [
                {"type": "command", "command": ours("session-start"), "timeout": 5},
                {"type": "command", "command": ours("session-end"), "timeout": 5}
            ]}],
            "PostToolUse": [{"hooks": [{"type": "command", "command": ours("stop")}]}]
        }})
        .to_string();
        let keys: Vec<String> = owned(Path::new("/h/hooks.json"), &text)
            .into_iter()
            .map(|entry| entry.key)
            .collect();
        assert_eq!(
            keys,
            ["/h/hooks.json:stop:0:0", "/h/hooks.json:session_start:0:0"],
            "{keys:?}"
        );
    }

    #[test]
    fn a_quoted_enabled_key_is_read() {
        let text = "[hooks.state.\"k\"]\n\"enabled\" = false\n";
        assert_eq!(state_of(text, "k").enabled, Some(false));
    }

    fn entry(key: &str, hash: &str) -> Entry {
        Entry {
            key: key.to_owned(),
            hash: hash.to_owned(),
        }
    }

    #[test]
    fn trust_is_appended_as_the_table_codex_writes_and_nothing_else_moves() {
        let path = Path::new("config.toml");
        let theirs = "model = \"gpt\"\n\n[mcp_servers.leteo]\ncommand = \"leteo\"\n";
        let after = with_trust(
            path,
            theirs,
            &[],
            &[entry("/h/hooks.json:pre_tool_use:0:0", "sha256:aa")],
        )
        .expect("the table is appended");
        assert_eq!(
            after,
            format!(
                "{theirs}\n[hooks.state.\"/h/hooks.json:pre_tool_use:0:0\"]\ntrusted_hash = \"sha256:aa\"\n"
            )
        );
        assert_eq!(
            with_trust(
                path,
                &after,
                &[],
                &[entry("/h/hooks.json:pre_tool_use:0:0", "sha256:aa")]
            )
            .expect("a second pass reads what the first wrote"),
            after,
            "a second pass rewrote it"
        );
        assert_eq!(
            without_trust(path, &after, &["/h/hooks.json:pre_tool_use:0:0".to_owned()])
                .expect("the trust is removed"),
            theirs
        );
    }

    #[test]
    fn a_changed_hash_is_replaced_in_place_and_their_other_keys_stay() {
        let path = Path::new("config.toml");
        let theirs = "[hooks.state.\"k\"]\n# mine\nenabled = true\ntrusted_hash = \"sha256:old\"\n\n[other]\nx = 1\n";
        let after = with_trust(path, theirs, &[], &[entry("k", "sha256:new")])
            .expect("the hash is replaced");
        assert_eq!(
            after,
            "[hooks.state.\"k\"]\n# mine\nenabled = true\ntrusted_hash = \"sha256:new\"\n\n[other]\nx = 1\n"
        );
        let removed = without_trust(path, &after, &["k".to_owned()]).expect("the trust is removed");
        assert_eq!(
            removed, "[hooks.state.\"k\"]\n# mine\nenabled = true\n\n[other]\nx = 1\n",
            "the operator's own keys under the table went with the trust"
        );
    }

    #[test]
    fn a_table_with_no_hash_gets_one_beside_their_keys() {
        let path = Path::new("config.toml");
        let theirs = "[hooks.state.\"k\"]\nenabled = false\n";
        assert_eq!(
            with_trust(path, theirs, &[], &[entry("k", "sha256:aa")]).expect("the hash is added"),
            "[hooks.state.\"k\"]\nenabled = false\ntrusted_hash = \"sha256:aa\"\n"
        );
    }

    #[test]
    fn other_hooks_trust_survives_and_a_stale_key_of_ours_does_not() {
        let path = Path::new("config.toml");
        let theirs = "[hooks.state.\"theirs\"]\ntrusted_hash = \"sha256:t\"\n\n[hooks.state.\"old\"]\ntrusted_hash = \"sha256:o\"\n";
        let after = with_trust(
            path,
            theirs,
            &["old".to_owned()],
            &[entry("new", "sha256:n")],
        )
        .expect("their trust and ours coexist");
        assert_eq!(
            state_of(&after, "theirs").trusted_hash.as_deref(),
            Some("sha256:t"),
            "{after}"
        );
        assert_eq!(
            state_of(&after, "new").trusted_hash.as_deref(),
            Some("sha256:n"),
            "{after}"
        );
        assert_eq!(state_of(&after, "old").trusted_hash, None, "{after}");
    }

    #[test]
    fn a_key_with_backslashes_and_the_literal_spelling_are_the_same_key() {
        let path = Path::new("config.toml");
        let key = "C:\\Users\\me\\.codex\\hooks.json:pre_tool_use:0:0";
        let written =
            with_trust(path, "", &[], &[entry(key, "sha256:aa")]).expect("the key is written");
        assert_eq!(
            state_of(&written, key).trusted_hash.as_deref(),
            Some("sha256:aa")
        );
        let literal = format!("[hooks.state.'{key}']\ntrusted_hash = 'sha256:aa'\n");
        assert_eq!(
            state_of(&literal, key).trusted_hash.as_deref(),
            Some("sha256:aa")
        );
    }

    #[test]
    fn windows_line_endings_are_kept() {
        let path = Path::new("config.toml");
        let theirs = "model = \"gpt\"\r\n";
        let after = with_trust(path, theirs, &[], &[entry("k", "sha256:aa")])
            .expect("the trust is appended");
        assert_eq!(
            after,
            "model = \"gpt\"\r\n\r\n[hooks.state.k]\r\ntrusted_hash = \"sha256:aa\"\r\n"
        );
    }

    #[test]
    fn a_shape_the_table_cannot_be_merged_into_is_refused() {
        let path = Path::new("config.toml");
        for refused in [
            "[hooks] # note\nstate = {}\n",
            "[hooks] # https://x/?a=b\nstate = { trusted_hash = \"x\" }\n",
            "hooks = { state = {} }\n",
            "[hooks]\nstate = {}\n",
        ] {
            assert!(unmergeable(path, refused).is_err(), "{refused}");
        }
        for merged in [
            "[hooks.state] # note\nk = 1\n",
            "[hooks.state] # see ]\nk = 1\n",
            "[ hooks.state ]\nk = 1\n",
            "[\"hooks\".\"state\"]\nk = 1\n",
            "hooks.state.k.trusted_hash = \"x\"\n",
            "[hooks.state]\nk = { trusted_hash = \"x\" }\n",
            "[hooks]\n\n[hooks.state.\"k\"]\ntrusted_hash = \"x\"\n",
            "model = \"x\"\n[mcp_servers.a]\nhooks = 1\n",
        ] {
            assert!(unmergeable(path, merged).is_ok(), "{merged}");
        }
    }

    #[test]
    fn a_hook_codex_skips_is_named_by_why() {
        let hooks = Path::new("/h/hooks.json");
        let config = Path::new("/h/config.toml");
        let ours = [entry("k", "sha256:now")];
        assert_eq!(
            skipped(
                hooks,
                config,
                &ours,
                "hooks.state.k.trusted_hash = \"sha256:now\"\n",
            ),
            None,
            "a dotted definition the parser reads was reported as refused"
        );
        assert!(skipped(hooks, config, &ours, "").is_some_and(|s| s.cleared_by_setup));
        let drifted = "[hooks.state.\"k\"]\ntrusted_hash = \"sha256:then\"\n";
        let drift = skipped(hooks, config, &ours, drifted).expect("drift is reported");
        assert!(drift.detail.contains("another hash"), "{}", drift.detail);
        assert!(drift.cleared_by_setup);
        let trusted = "[hooks.state.\"k\"]\ntrusted_hash = \"sha256:now\"\n";
        assert_eq!(skipped(hooks, config, &ours, trusted), None);
        let off = "[hooks.state.\"k\"]\ntrusted_hash = \"sha256:now\"\nenabled = false\n";
        let disabled = skipped(hooks, config, &ours, off).expect("disabled is reported");
        assert!(disabled.detail.contains("enabled = false"));
        assert!(!disabled.cleared_by_setup);
    }
}
