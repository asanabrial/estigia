//! Declared checks: `[[check]]` tables in the file `[[generated]]` lives in.

use std::path::Path;

use super::{argv_of, borrowed, declaration_text, relative, string_of};
use crate::transport::{Failure, isolated_diff_env};

#[cfg(test)]
mod tests;

/// One declared check: a name, the argv that runs it with no shell, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// Unique, lowercase `a-z0-9` groups joined by `-`.
    pub name: String,
    /// The argv run, with no shell.
    pub command: Vec<String>,
    /// Directory relative to the checked tree the command runs in.
    pub cwd: Option<String>,
}

fn malformed(detail: impl Into<String>) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "checks-declaration-malformed",
        "detail": detail.into(),
        "action": "correct `estigia.generated.toml` on the base branch; a declaration is read \
                   only from the base, so a fix in the head does not count",
    }))
}

fn well_named(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|group| {
            !group.is_empty()
                && group
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn check_of(table: &toml_edit::Table) -> Result<Check, String> {
    if let Some(unknown) = table
        .iter()
        .map(|(key, _)| key)
        .find(|key| !["name", "command", "cwd"].contains(key))
    {
        return Err(format!("`{unknown}` is not a known key"));
    }
    let name = string_of(table, "name")?.ok_or("a check needs `name`")?;
    if !well_named(&name) {
        return Err(format!(
            "`name` must be lowercase a-z0-9 groups joined by `-`, got `{name}`"
        ));
    }
    let cwd = string_of(table, "cwd")?;
    if let Some(cwd) = &cwd {
        relative("cwd", cwd)?;
    }
    Ok(Check {
        name,
        command: argv_of(table)?,
        cwd,
    })
}

/// Parses the `[[check]]` tables of the declaration file. A file with none
/// declares no checks; anything it cannot read as written is refused, never
/// read as empty.
pub fn parse(text: &str) -> Result<Vec<Check>, Failure> {
    let document: toml_edit::DocumentMut = text
        .parse()
        .map_err(|error| malformed(format!("not valid TOML: {error}")))?;
    let Some(item) = document.get("check") else {
        return Ok(Vec::new());
    };
    let tables = item
        .as_array_of_tables()
        .ok_or_else(|| malformed("`check` must be an array of tables, `[[check]]`"))?;
    let mut declared: Vec<Check> = Vec::new();
    for table in tables {
        let one = check_of(table).map_err(malformed)?;
        if declared.iter().any(|seen| seen.name == one.name) {
            return Err(malformed(format!("`{}` is declared twice", one.name)));
        }
        declared.push(one);
    }
    Ok(declared)
}

/// The checks the **base** commit declares; the working tree and the head are
/// never consulted.
pub fn declared_at(repo: &Path, base: &str) -> Result<Vec<Check>, Failure> {
    let (_guard, envs) = isolated_diff_env(repo)?;
    match declaration_text(repo, base, &borrowed(&envs))? {
        Some(text) => parse(&text),
        None => Ok(Vec::new()),
    }
}

/// SHA-256 over the checks in declaration order, every field length-prefixed so
/// no two declarations spell the same bytes.
pub fn commands_digest(checks: &[Check]) -> String {
    let mut bytes = Vec::new();
    let mut field = |text: &str| {
        bytes.extend_from_slice(format!("{}:", text.len()).as_bytes());
        bytes.extend_from_slice(text.as_bytes());
    };
    for check in checks {
        field(&check.name);
        field(check.cwd.as_deref().unwrap_or(""));
        field(&check.command.len().to_string());
        for argument in &check.command {
            field(argument);
        }
    }
    crate::transport::ownership::sha256_hex(&bytes)
}
