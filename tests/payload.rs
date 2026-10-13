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
        unreachable,
        Vec::<&str>::new(),
        "every runtime companion must be reachable from another shipped file"
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
    // The changelog is read from a copy in a repository with no `changes/`: this
    // repository's own fragments are waiting to be assembled, and `changelog_notes`
    // refuses exactly that, so reading in place would measure the fragments.
    let root = repository_root();
    let version = env!("CARGO_PKG_VERSION");
    let copy = tempfile::tempdir().expect("a repository without fragments");
    let changelog = copy.path().join("CHANGELOG.md");
    std::fs::copy(root.join("CHANGELOG.md"), &changelog).expect("a copy of the changelog");
    let context = estigia::transport::Context {
        skill_dir: root.join("skill"),
        repo_dir: copy.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };

    let answer = estigia::transport::commands::changelog_notes(
        &context, &changelog, version, false, None,
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
fn one_stable_blind_reviewer_definition_is_in_the_agent_manifest_not_the_skill() {
    let root = repository_root();
    assert_eq!(estigia::skill::REVIEW_AGENT.path, "agents/review-blind.md");
    assert!(
        !estigia::skill::FILES
            .iter()
            .any(|file| file.path == estigia::skill::REVIEW_AGENT.path)
    );
    let reviewer = estigia::skill::REVIEW_AGENT.contents;
    for clause in ["model: inherit", "inert unless", "immutable target"] {
        assert!(reviewer.contains(clause));
    }
    // The asset is a **template**, and both halves of that are
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
        let rendered = estigia::setup::render_reviewer_agent(
            reviewer,
            evidence,
            estigia::config::Seal::Unsealed,
            estigia::harness::roles::RESERVED_REVIEWERS[0].0,
        );
        assert!(
            !rendered.contains("{{"),
            "{evidence:?} leaves a placeholder in the installed definition"
        );
    }

    // The lens is a template too, but not the judge's template: it carries
    // `{{TOOLS}}` and deliberately not `{{DISCIPLINE}}`, so it gets its own
    // version of the carries/leaves-none pair above rather than being forced
    // into the judge's assertion.
    let lens = estigia::skill::REVIEW_LENS.contents;
    let tools_placeholder = "{{TOOLS}}";
    assert!(
        lens.contains(tools_placeholder),
        "the lens asset no longer carries {tools_placeholder}, so the seal cannot reach it"
    );
    let discipline_placeholder = "{{DISCIPLINE}}";
    assert!(
        !lens.contains(discipline_placeholder),
        "the lens asset carries {discipline_placeholder}, which is judge verdict language \
         deliberately absent from a lens"
    );
    let lens_name = estigia::harness::roles::RESERVED_REVIEWERS
        .iter()
        .find(|&&(name, _, _)| estigia::harness::roles::reserved_lens(name))
        .expect("a reserved row renders from a template other than the judge's")
        .0;
    let lens_rendered = estigia::setup::render_reviewer_agent(
        lens,
        estigia::config::Evidence::Reading,
        estigia::config::Seal::Sealed,
        lens_name,
    );
    assert!(
        !lens_rendered.contains("{{"),
        "the sealed lens rendering leaves a placeholder behind: {lens_rendered}"
    );

    let mut definitions: Vec<String> = std::fs::read_dir(root.join("skill").join("agents"))
        .expect("skill/agents reads")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    definitions.sort();
    let expected = [
        "analyst.md",
        "developer.md",
        "review-blind.md",
        "review-lens.md",
        "review-readability.md",
        "review-readback.md",
        "review-refuter.md",
        "review-reliability.md",
        "review-risk.md",
        "review-validator.md",
    ];
    assert_eq!(definitions, expected);
    let mut manifest: Vec<_> = estigia::skill::AGENT_DEFINITIONS
        .iter()
        .map(|file| file.path.strip_prefix("agents/").expect("an agent path"))
        .collect();
    manifest.sort();
    assert_eq!(manifest, expected);
    // The delegated pair, each beside the `Model routing` key that installs it.
    // Held here because that pairing is what makes the key reach a file: a
    // name renamed on one side and not the other would leave a shipped
    // definition nothing can ever ask for.
    let mut delegated: Vec<_> = estigia::skill::DELEGATED_AGENTS
        .iter()
        .map(|(target, file)| {
            (
                *target,
                file.path.strip_prefix("agents/").expect("an agent path"),
            )
        })
        .collect();
    delegated.sort();
    assert_eq!(
        delegated,
        vec![("analyst", "analyst.md"), ("developer", "developer.md")]
    );
    for (target, _) in estigia::skill::DELEGATED_AGENTS {
        assert!(
            estigia::config::ModelRouting::targets().contains(target),
            "`{target}` installs a definition and the routing refuses the key"
        );
    }
    for file in estigia::skill::AGENT_DEFINITIONS {
        assert_eq!(
            std::fs::read_to_string(root.join("skill").join(file.path)).expect("an agent reads"),
            file.contents,
            "{} differs from its embedded bytes",
            file.path
        );
    }
    let notes = std::fs::read_to_string(root.join("skill/references/runtime-notes.md"))
        .expect("runtime notes ship");
    assert!(notes.contains("effective `judge` model"));
    let judges = std::fs::read_to_string(root.join("skill/policies/blind-judges.md"))
        .expect("blind judges ship");
    assert!(judges.contains("identical diff and criteria"));
}

/// Issue #35: what a judge's report must state was stated nowhere, and each launch
/// path that could have carried it was pinned against a literal in its own
/// test — the `Measuring` reviewer text in `src/setup/tests.rs`, the OpenCode
/// text nowhere at all. So the words could drift between hosts, or between a
/// host and the policy document that describes them, with every existing test
/// still green: each side only ever compared itself to itself.
///
/// Issue #775 retired the verdict a judge recorded, and with it the three carriers that
/// described the field it was recorded in — the MCP tool's `basis` argument, `skill/SKILL.md`'s
/// developer step 5 and `skill/bindings/github/operations.md`'s `Recording a verdict` section.
/// What remains is the requirement itself, which the judge definitions still carry, and this test
/// crosses all four places that state it: the three rendered host texts and the shipped policy
/// document.
///
/// Every carrier is crossed against the one constant they are all required to
/// carry verbatim, `estigia::skill::VERDICT_BASIS_REQUIREMENT`. Deleting the
/// requirement from any one of the four reddens this without touching the other
/// three.
///
/// What it does not hold: consistency. A carrier that keeps the requirement and
/// gains a sentence contradicting it beside stays green here. The crossing
/// catches a surface that stops carrying the words, not one that argues with them.
#[test]
fn the_verdict_basis_requirement_reaches_every_generated_launch_text() {
    // Compare the words, not the wrapping. The reviewer render, the OpenCode
    // block and the tool schema all carry the constant's exact bytes, but the
    // three markdown documents wrap their prose near 100 columns like every
    // paragraph around them. Comparing bytes forced a 296-character line into
    // one of them — in a file whose next-longest is 155 — and welded that
    // document's formatting to this test: rewrapping a paragraph would redden
    // an assertion whose message says the requirement is missing, which is
    // the most misleading failure this test could produce.
    fn words(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    let requirement = words(estigia::skill::VERDICT_BASIS_REQUIREMENT);

    let policy = std::fs::read_to_string(repository_root().join("skill/policies/judgment-day.md"))
        .expect("the judgment-day policy ships");
    assert!(
        words(&policy).contains(&requirement),
        "skill/policies/judgment-day.md no longer states what a verdict must report"
    );

    let measuring = estigia::setup::render_reviewer_agent(
        estigia::skill::REVIEW_AGENT.contents,
        estigia::config::Evidence::Measuring,
        estigia::config::Seal::Unsealed,
        estigia::harness::roles::RESERVED_REVIEWERS[0].0,
    );
    assert!(
        words(&measuring).contains(&requirement),
        "the `measuring` reviewer definition no longer asks a judge to say what it ran"
    );

    let reading = estigia::setup::render_reviewer_agent(
        estigia::skill::REVIEW_AGENT.contents,
        estigia::config::Evidence::Reading,
        estigia::config::Seal::Unsealed,
        estigia::harness::roles::RESERVED_REVIEWERS[0].0,
    );
    assert!(
        words(&reading).contains(&requirement),
        "the `reading` reviewer definition no longer asks a judge to state a basis"
    );

    let dir = tempfile::tempdir().expect("a temporary skill root");
    let opencode = dir.path().join("estigia.opencode.md");
    estigia::setup::write_agent_configuration_wholly(
        &opencode,
        "opencode",
        &estigia::config::Config::default(),
    )
    .expect("opencode's file writes");
    let opencode_text = std::fs::read_to_string(&opencode).expect("opencode's file reads");
    assert!(
        words(&opencode_text).contains(&requirement),
        "the OpenCode `## Review on this host` block no longer states it either"
    );
}

/// Issue #191, repair round 2: two blind panels rejected this change on a sentence copied from
/// another project without checking it against this repository, and both said independently that
/// the classification rule — the never-`severe` treatment of stale prose, and the carve-out that
/// bounds it — has three hand-maintained copies and nothing crosses them for meaning. This is
/// that crossing, for the two carriers the rule is actually duplicated across.
///
/// `estigia::skill::STALE_PROSE_NEVER_SEVERE` and `estigia::skill::CARVE_OUT_BOUND` are each
/// carried verbatim by `skill/agents/review-blind.md` and `skill/policies/blind-judges.md`.
/// Deleting either sentence from either file reddens this without touching the other.
///
/// The bound — and only the bound — is additionally carried by `CHANGELOG.md`'s 0.1.0 entry,
/// byte-identical under the same whitespace normalisation, and crossed below the same way:
/// deleting it there reddens this without touching either shipped carrier. The changelog does
/// not carry the never-`severe` rule itself, so only the bound is asserted of it.
///
/// `skill/SKILL.md` is deliberately not crossed here: developer step 5 no longer restates the
/// classification rule at all, and `skill/policies/blind-judges.md` states the duplication it means
/// to hold two carriers to, not three. Forcing a third carrier that says the same thing differently
/// into this assertion would either shrink the constant to a fragment too short to mean anything, or
/// fail a carrier that was never wrong. See `src/skill.rs`'s doc comment on both constants for the
/// same reasoning stated once, not twice.
///
/// What this does not hold, the same as the sibling test above: consistency. A carrier that keeps
/// both sentences and gains a third that contradicts them stays green here.
///
/// What it does hold past presence is placement: each assertion runs against
/// the classification section carrying the rule, not the whole file, so a
/// relocation that moves either sentence out of that section reddens naming
/// the carrier — the accident the sibling test's line selection refuses to
/// depend on.
#[test]
fn the_stale_prose_rule_and_its_carve_out_reach_both_carriers() {
    fn words(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    // The classification section: from its `## ` heading line to the next
    // level-2 heading or the end of the file. Subsections belong to it — the
    // rule sitting under one is still in the section — while a move to any
    // other section leaves it.
    fn section(text: &str, heading: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let start = lines
            .iter()
            .position(|line| *line == heading)
            .unwrap_or_else(|| panic!("{heading} is not a section heading of this carrier"));
        let rest = &lines[start + 1..];
        let end = rest
            .iter()
            .position(|line| line.starts_with("## "))
            .unwrap_or(rest.len());
        rest[..end].join("\n")
    }

    let never_severe = words(estigia::skill::STALE_PROSE_NEVER_SEVERE);
    let carve_out = words(estigia::skill::CARVE_OUT_BOUND);

    let definition = estigia::skill::REVIEW_AGENT.contents;
    let contract = section(definition, "## Review contract");
    assert!(
        words(&contract).contains(&never_severe),
        "skill/agents/review-blind.md's `Review contract` section no longer states the never-`severe` rule: moved out of the classification section or deleted"
    );
    assert!(
        words(&contract).contains(&carve_out),
        "skill/agents/review-blind.md's `Review contract` section no longer states the carve-out's bound: moved out of the classification section or deleted"
    );

    let policy = std::fs::read_to_string(repository_root().join("skill/policies/blind-judges.md"))
        .expect("the blind-judge policy ships");
    let finding = section(&policy, "## What a finding is");
    assert!(
        words(&finding).contains(&never_severe),
        "skill/policies/blind-judges.md's `What a finding is` section no longer states the never-`severe` rule: moved out of the classification section or deleted"
    );
    assert!(
        words(&finding).contains(&carve_out),
        "skill/policies/blind-judges.md's `What a finding is` section no longer states the carve-out's bound: moved out of the classification section or deleted"
    );

    let changelog = std::fs::read_to_string(repository_root().join("CHANGELOG.md"))
        .expect("the changelog ships");
    assert!(
        words(&changelog).contains(&carve_out),
        "CHANGELOG.md no longer states the carve-out's bound"
    );
}
