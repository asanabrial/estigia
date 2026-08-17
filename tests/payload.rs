// An assertion that panics is the assertion working; threading `?` through a
// test buys nothing and costs the reader. Restated here because an integration
// test is a separate crate and the library's own allow does not reach it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What Estigia installs, checked against what it claims to install.
//!
//! This file ran **upstream's own test suite** — 328 checks written by the
//! people who wrote the payload — against the payload Estigia writes to disk.
//! That suite is Python, and it tested a transport that was Python. Both are
//! gone: this repository holds one transport, in one language, and the payload
//! it installs is prose.
//!
//! What is left is three interpreter-independent guards: nothing that is *test
//! material* may be installed beside the contract, every shipped runtime
//! companion must be reachable from another shipped file, and the tool Estigia
//! ships has to be able to read this repository's own changelog.
//!
//! Where the rest of the coverage went is written down rather than dropped: the
//! marker grammar, the ownership reducer and the exit-code contract are crossed
//! in `tests/differential.rs`, against answers recorded off that same Python
//! before it was deleted. See the README's honesty contract for what a recorded
//! answer proves and what it does not.

use std::path::{Path, PathBuf};

use estigia::skill::{CONTRACT, FILES, PHASE_AGENTS, SkillFile};

// This is a historical migration ledger, not a runtime document. Upstream's
// payload suite made the same sole exception.
const UNREACHABLE_BY_DESIGN: &[&str] = &["references/migration-inventory.md"];

/// Where this repository is.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn no_test_material_is_installed_beside_the_contract() {
    // It is a test, not payload. Shipping it would put somebody else's test
    // harness in every agent's skill directory, and the seam guard that asks
    // "does anything name this file?" would rightly fail on it.
    let installed = repository_root().join("skill").join("scripts");
    let Ok(entries) = std::fs::read_dir(&installed) else {
        // No `scripts/` at all is the strongest form of the same answer: the
        // directory existed to hold `github.py`, and nothing installs one now.
        return;
    };
    let stray: Vec<_> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("test_") || name == "__pycache__")
        .collect();
    assert!(
        stray.is_empty(),
        "{} holds test material that would be installed: {stray:?}",
        installed.display()
    );
}

#[test]
fn every_shipped_companion_is_reachable_from_another_shipped_file() {
    let shipped = FILES.iter().chain(PHASE_AGENTS);
    let mut unreachable = shipped
        .clone()
        .filter(|target| target.path != CONTRACT)
        .filter(|target| {
            !shipped.clone().any(|source| {
                source.path != target.path
                    && (links_from(source).iter().any(|link| link == target.path)
                        || selected_binding_is_named(source, target)
                        || phase_agent_is_named(source, target))
            })
        })
        .map(|file| file.path)
        .collect::<Vec<_>>();
    unreachable.sort_unstable();

    assert_eq!(
        unreachable, UNREACHABLE_BY_DESIGN,
        "every runtime companion must be reachable, and the exception list may neither grow nor go stale"
    );
}

fn links_from(file: &SkillFile) -> Vec<String> {
    let directory = Path::new(file.path).parent().unwrap_or(Path::new(""));
    markdown_link_targets(file.contents)
        .into_iter()
        .map(|target| {
            let target = target.split('#').next().unwrap_or_default();
            normalize_relative(directory, target)
        })
        .filter(|target| target.ends_with(".md"))
        .collect()
}

fn markdown_link_targets(document: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    let mut rest = document;
    while let Some(open) = rest.find("](") {
        rest = &rest[open + 2..];
        let Some(close) = rest.find(')') else { break };
        let target = rest[..close].trim();
        if !target.is_empty() && !target.contains("://") {
            targets.push(target);
        }
        rest = &rest[close + 1..];
    }
    targets
}

fn normalize_relative(directory: &Path, target: &str) -> String {
    let mut segments = directory
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other.to_owned()),
        }
    }
    segments.join("/")
}

fn selected_binding_is_named(source: &SkillFile, target: &SkillFile) -> bool {
    target.path.starts_with("bindings/")
        && source.path == CONTRACT
        && target
            .path
            .strip_prefix("bindings/")
            .and_then(|path| path.strip_suffix(".md"))
            .is_some_and(|name| source.contents.contains(&format!("`{name}`")))
}

fn phase_agent_is_named(source: &SkillFile, target: &SkillFile) -> bool {
    target.path.starts_with("agents/")
        && target
            .path
            .strip_prefix("agents/")
            .and_then(|path| path.strip_suffix(".md"))
            .is_some_and(|name| source.contents.contains(&format!("`{name}`")))
}

#[test]
fn the_changelog_is_readable_by_the_tool_estigia_ships() {
    // A closed loop worth keeping closed: `changelog_notes` is one of the tools
    // Estigia exposes, and this repository has a changelog it will one day be
    // asked to read. It fails closed on a missing or empty
    // entry, because a tag is immutable and notes invented at tag time are
    // permanent — so a format drift here is a release that cannot be cut.
    //
    // It used to run the shipped `github.py` for this. The tool that answers
    // now is the port, so the port is what is asked.
    let root = repository_root();
    let version = env!("CARGO_PKG_VERSION");
    let context = estigia::transport::Context {
        skill_dir: root.join("skill"),
        repo_dir: root.clone(),
        config: Vec::new(),
        repo: None,
    };

    let answer = estigia::transport::commands::changelog_notes(
        &context,
        &root.join("CHANGELOG.md"),
        version,
        false,
        None,
    )
    .unwrap_or_else(|failure| {
        panic!(
            "`changelog_notes` cannot read this repository's own changelog for {version}: {}",
            failure.envelope()
        )
    });

    assert_eq!(
        answer.get("ok").and_then(serde_json::Value::as_bool),
        Some(true),
        "the entry for {version} is missing or empty: {answer}"
    );
}
