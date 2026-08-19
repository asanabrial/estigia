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

use estigia::skill::{AGENT_DEFINITIONS, CONTRACT, FILES, SkillFile};

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
    let shipped = FILES.iter().chain(AGENT_DEFINITIONS);
    let mut unreachable = shipped
        .clone()
        .filter(|target| target.path != CONTRACT)
        .filter(|target| {
            !shipped.clone().any(|source| {
                source.path != target.path
                    && (links_from(source).iter().any(|link| link == target.path)
                        || selected_binding_is_named(source, target)
                        || agent_definition_is_named(source, target))
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

fn agent_definition_is_named(source: &SkillFile, target: &SkillFile) -> bool {
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
        "review-blind` is operator-owned",
        "invalidates the panel",
        "a refused or unprovable launch contributes no judge",
        "never silently reduce or serialize the panel",
        "a repository whose evidence standard is mutation",
        "no two judges are ever pointed at one directory",
        "a directory nothing else writes for the duration of its review",
        // The scratch half, which the checkout phrases above do not reach. A
        // five-judge panel that shared one scratch directory lost two verdicts'
        // independence and nothing in the rule said it could not happen.
        "every location a judge writes, not only its checkout",
        // The scoping half, twice over. Reverting either to the blanket "All
        // judges are read-only" restores the contradiction this policy exists to
        // remove — a guarantee stated for a role no measuring panel could use —
        // and reverting the second to a blanket permission widens the grant past
        // the shell, which is the one widening no evidence standard makes
        // acceptable. What keeps a measuring judge from editing what it is
        // judging is the isolation rule, pinned by its own phrases below — not
        // this sentence, which a shell defeats.
        "cannot mutate the target even if its prompt asks it to",
        "the role gets a shell **and nothing else**",
        "stops and reports it rather than restoring it",
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
    // The asset is a **template** since issue 83, and both halves of that are
    // asserted. It has to carry the placeholders, or somebody has hardcoded a
    // grant back into it and the evidence standard decides nothing; and every
    // rendering has to have none left, because a surviving `{{TOOLS}}` parses as
    // an allowlist naming one tool of that name — which denies everything and
    // reads exactly like a gate that is working.
    for placeholder in ["{{TOOLS}}", "{{DISCIPLINE}}"] {
        assert!(
            reviewer.contains(placeholder),
            "the reviewer asset no longer carries {placeholder}, so the evidence standard cannot reach it"
        );
    }
    for evidence in estigia::config::Evidence::all() {
        let rendered = estigia::setup::render_reviewer_agent(reviewer, evidence);
        assert!(
            !rendered.contains("{{"),
            "{evidence:?} leaves a placeholder in the installed definition"
        );
    }

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
