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
//! What is left is the part of that guard which was never about the
//! interpreter: nothing that is *test material* may be installed beside the
//! contract, and the tool Estigia ships has to be able to read this
//! repository's own changelog.
//!
//! Where the rest of the coverage went is written down rather than dropped: the
//! marker grammar, the ownership reducer and the exit-code contract are crossed
//! in `tests/differential.rs`, against answers recorded off that same Python
//! before it was deleted. See the README's honesty contract for what a recorded
//! answer proves and what it does not.

use std::path::PathBuf;

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

#[test]
fn the_blind_panel_policy_requires_same_finding_quorum_without_erasing_dissent() {
    let policy = std::fs::read_to_string(repository_root().join("skill/policies/blind-judges.md"))
        .expect("the blind-judge policy ships");
    let policy = policy
        .lines()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    for required in [
        "five independent reviewer contexts",
        "contexts concurrently",
        "identical immutable target and criteria",
        "3-of-5",
        "same severe finding",
        "one or two confirmations",
        "suspicions",
        "preserve dissent",
        "ambiguous finding identities do not aggregate",
    ] {
        assert!(policy.contains(required), "missing {required:?}");
    }
}

#[test]
fn one_stable_blind_reviewer_definition_is_in_the_agent_manifest_not_the_skill() {
    let root = repository_root();
    assert_eq!(estigia::skill::REVIEW_AGENT.path, "agents/review-blind.md");
    assert!(
        !estigia::skill::FILES
            .iter()
            .any(|file| file.path == estigia::skill::REVIEW_AGENT.path)
    );
    let reviewer = estigia::skill::REVIEW_AGENT.contents;
    for clause in ["model: inherit", "inert unless", "exact publication"] {
        assert!(reviewer.contains(clause));
    }
    assert!(!reviewer.contains("{{"));

    let mut definitions: Vec<String> = std::fs::read_dir(root.join("skill").join("agents"))
        .expect("skill/agents reads")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    definitions.sort();
    let expected = [
        "review-blind.md",
        "sdd-design.md",
        "sdd-explore.md",
        "sdd-propose.md",
        "sdd-spec.md",
        "sdd-tasks.md",
    ];
    assert_eq!(definitions, expected);
    let mut manifest: Vec<_> = estigia::skill::AGENT_DEFINITIONS
        .iter()
        .map(|file| file.path.strip_prefix("agents/").expect("an agent path"))
        .collect();
    manifest.sort();
    assert_eq!(manifest, expected);
    let mut phases: Vec<_> = estigia::skill::PHASE_AGENTS
        .iter()
        .map(|file| file.path.strip_prefix("agents/").expect("an agent path"))
        .collect();
    phases.sort();
    let expected_phases: Vec<_> = expected
        .iter()
        .copied()
        .filter(|name| name.starts_with("sdd-"))
        .collect();
    assert_eq!(phases, expected_phases);
    for file in estigia::skill::AGENT_DEFINITIONS {
        assert_eq!(
            std::fs::read_to_string(root.join("skill").join(file.path)).expect("an agent reads"),
            file.contents,
            "{} differs from its embedded bytes",
            file.path
        );
    }
    let contract = std::fs::read_to_string(root.join("skill/SKILL.md")).expect("the skill ships");
    assert!(contract.contains("passes the effective `judge` model"));
    assert!(contract.contains("identical exact receipt and criteria"));
}
