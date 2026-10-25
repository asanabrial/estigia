// An assertion that panics is the assertion working. See `tests/pipe.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Byte and paragraph ceilings on shipped `skill/*.md`.
//!
//! A registered per-document byte count that must not be exceeded, so growth
//! past it fails without a review-visible edit to `DOCUMENTS`, and a per-line
//! size that must not be exceeded except where `LINE_EXCEPTIONS` names the line
//! and why.
//!
//! Counts UTF-8 bytes after normalising newlines to `\n`, so a Windows
//! checkout and Linux CI agree. Does not read the prose for meaning.

use std::path::{Path, PathBuf};

const DOCUMENTS: &[(&str, usize)] = &[
    ("skill/SKILL.md", 25_528),
    ("skill/agents/analyst.md", 2_052),
    ("skill/agents/developer.md", 3_298),
    ("skill/agents/review-blind.md", 7_984),
    ("skill/agents/review-lens.md", 3_398),
    ("skill/agents/review-readability.md", 4_516),
    ("skill/agents/review-readback.md", 4_618),
    ("skill/agents/review-refuter.md", 2_438),
    ("skill/agents/review-reliability.md", 4_620),
    ("skill/agents/review-risk.md", 4_276),
    ("skill/agents/review-validator.md", 1_998),
    ("skill/assets/analyst-issue-template.md", 3_790),
    ("skill/bindings/github.md", 17_989),
    ("skill/bindings/github/delivery.md", 39_751),
    ("skill/bindings/github/operations-guarantees.md", 37_008),
    ("skill/bindings/github/operations.md", 52_474),
    ("skill/bindings/github/recovery.md", 6_571),
    ("skill/bindings/github/refusals.md", 54_453),
    ("skill/bindings/github/setup.md", 1_611),
    ("skill/bindings/linear.md", 5_999),
    ("skill/policies/blind-judges.md", 42_284),
    ("skill/policies/judgment-day.md", 15_969),
    ("skill/protocols/rdd.md", 7_863),
    ("skill/references/analyst.md", 3_183),
    ("skill/references/decision-gates.md", 4_069),
    ("skill/references/domain-composition.md", 9_798),
    ("skill/references/repository-delivery.md", 22_898),
    ("skill/references/runtime-notes.md", 20_633),
    ("skill/references/safety-incidents.md", 16_216),
];

const LINE_CEILING: usize = 2_000;

/// The repository's own instructions, and the bytes each may carry.
///
/// The same mechanism as `DOCUMENTS`, for the other audience: `skill/*.md` is shipped to somebody
/// else's agent, and `AGENTS.md` is read by every session that works here, so its size is paid per
/// session rather than per install. Inventory belongs in a document that answers its own question;
/// this is what fails when it comes back here.
const OWN_INSTRUCTIONS: &[(&str, usize)] = &[("AGENTS.md", 13_223)];

const LINE_EXCEPTIONS: &[LineException] = &[];

struct LineException {
    path: &'static str,
    line: usize,
    reason: &'static str,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn lf_bytes(path: &Path) -> Vec<u8> {
    let raw = std::fs::read(path)
        .unwrap_or_else(|error| panic!("{} must be readable: {error}", path.display()));
    String::from_utf8(raw)
        .unwrap_or_else(|_| panic!("{} must be UTF-8", path.display()))
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .into_bytes()
}

fn skill_markdown() -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    let mut pending = vec![root().join("skill")];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} must be readable: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                let rel = path
                    .strip_prefix(root())
                    .expect("skill files live in this crate")
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((rel, path));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn over_bytes(measured: usize, ceiling: usize) -> bool {
    measured > ceiling
}

fn registry_of(path: &str) -> &'static str {
    if DOCUMENTS.iter().any(|(name, _)| *name == path) {
        "DOCUMENTS"
    } else if OWN_INSTRUCTIONS.iter().any(|(name, _)| *name == path) {
        "OWN_INSTRUCTIONS"
    } else {
        "the table that registers it"
    }
}

fn byte_refusal(path: &str, measured: usize, ceiling: usize) -> String {
    let registry = registry_of(path);
    format!(
        "{path} is {measured} bytes, above the registered ceiling of {ceiling}. \
         If the growth is deliberate, raise the entry for {path} in \
         tests/instruction_ceiling.rs {registry} and let the review see the number move."
    )
}

fn line_refusal(path: &str, line: usize, measured: usize) -> String {
    format!(
        "{path}:{line} is {measured} bytes, above LINE_CEILING of {LINE_CEILING} \
         and not in LINE_EXCEPTIONS. Split the line, or add it to LINE_EXCEPTIONS \
         in tests/instruction_ceiling.rs with a reason."
    )
}

fn excepted(path: &str, line: usize) -> bool {
    LINE_EXCEPTIONS
        .iter()
        .any(|exception| exception.path == path && exception.line == line)
}

#[test]
fn every_shipped_skill_markdown_file_is_in_the_registry() {
    let found: Vec<String> = skill_markdown().into_iter().map(|(rel, _)| rel).collect();
    let registered: Vec<&str> = DOCUMENTS.iter().map(|(path, _)| *path).collect();
    assert_eq!(
        found, registered,
        "DOCUMENTS and skill/**/*.md disagree; add or remove the path in \
         tests/instruction_ceiling.rs DOCUMENTS"
    );
}

#[test]
fn documents_stay_under_their_registered_byte_ceilings() {
    let found = skill_markdown();
    for (rel, path) in &found {
        let Some((_, ceiling)) = DOCUMENTS.iter().find(|(name, _)| name == rel) else {
            panic!("{rel} is not in DOCUMENTS");
        };
        let measured = lf_bytes(path).len();
        assert!(
            !over_bytes(measured, *ceiling),
            "{}",
            byte_refusal(rel, measured, *ceiling)
        );
    }
}

#[test]
fn no_unexcepted_line_exceeds_the_paragraph_ceiling() {
    for (rel, path) in skill_markdown() {
        let text = String::from_utf8(lf_bytes(&path)).expect("UTF-8");
        for (index, line) in text.split('\n').enumerate() {
            let number = index + 1;
            let measured = line.len();
            if !over_bytes(measured, LINE_CEILING) {
                continue;
            }
            assert!(
                excepted(&rel, number),
                "{}",
                line_refusal(&rel, number, measured)
            );
        }
    }
}

#[test]
fn line_exceptions_still_exceed_the_ceiling() {
    for exception in LINE_EXCEPTIONS {
        let path = root().join(exception.path);
        let text = String::from_utf8(lf_bytes(&path)).expect("UTF-8");
        let line = text
            .split('\n')
            .nth(exception.line - 1)
            .unwrap_or_else(|| panic!("{} has no line {}", exception.path, exception.line));
        assert!(
            over_bytes(line.len(), LINE_CEILING),
            "{}:{} is {} bytes, at or under LINE_CEILING of {LINE_CEILING}; \
             remove the exception ({})",
            exception.path,
            exception.line,
            line.len(),
            exception.reason
        );
    }
}

/// The operations table's long cells route their detail to a companion named
/// from the cell, so a row that routes its detail needs no line exception, and
/// each cell names a section the companion actually carries. A row that cannot
/// route its detail — `reclaim`, whose cell is the whole statement — takes the
/// exception `LINE_EXCEPTIONS` records instead.
#[test]
fn the_operations_table_routes_its_long_cells_to_a_named_companion() {
    let table =
        String::from_utf8(lf_bytes(&root().join("skill/bindings/github.md"))).expect("UTF-8");
    let companion = String::from_utf8(lf_bytes(
        &root().join("skill/bindings/github/operations.md"),
    ))
    .expect("UTF-8");
    for operation in [
        "`claim`",
        "`publish_review`",
        "`review_finding`",
        "`review_disposition`",
        "`review_tier`",
    ] {
        let row = table
            .lines()
            .find(|line| line.starts_with(&format!("| {operation} |")))
            .unwrap_or_else(|| panic!("the operations table has no row for {operation}"));
        let section = row
            .split_once("see *")
            .and_then(|(_, rest)| rest.split_once("* in github/operations.md"))
            .map(|(title, _)| title)
            .unwrap_or_else(|| panic!("{operation}'s cell names no companion section"));
        assert!(
            companion.contains(&format!("## {section}")),
            "{operation}'s cell names the section {section:?}, which the companion does not carry"
        );
    }
}

fn docs_citations(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("docs/") {
        let tail = &rest[at..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.')))
            .unwrap_or(tail.len());
        let candidate = tail[..end].trim_end_matches('.');
        if candidate.ends_with(".md") {
            found.push(candidate.to_string());
        }
        rest = &tail[end.max(1)..];
    }
    found
}

#[test]
fn no_shipped_skill_file_names_a_docs_path_the_installed_skill_cannot_open() {
    let mut hits = Vec::new();
    for (rel, path) in skill_markdown() {
        let text = String::from_utf8(lf_bytes(&path)).expect("UTF-8");
        for (index, line) in text.lines().enumerate() {
            for citation in docs_citations(line) {
                hits.push(format!("{rel}:{} names {citation}", index + 1));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "the skill is installed without docs/; state the fact in a shipped companion instead:\n{}",
        hits.join("\n")
    );
}

#[test]
fn this_repositorys_own_instructions_stay_under_their_registered_ceilings() {
    assert!(
        !OWN_INSTRUCTIONS.is_empty(),
        "nothing is registered, so this guard would agree with everything"
    );
    for (rel, ceiling) in OWN_INSTRUCTIONS {
        let measured = lf_bytes(&root().join(rel)).len();
        assert!(
            !over_bytes(measured, *ceiling),
            "{}",
            byte_refusal(rel, measured, *ceiling)
        );
        assert!(
            measured * 2 > *ceiling,
            "{rel} is {measured} bytes against a ceiling of {ceiling}, so the entry measures \
             something other than the file it names"
        );
    }
}

#[test]
fn a_refusal_names_the_registry_that_holds_the_entry() {
    let shipped = byte_refusal("skill/SKILL.md", 1, 0);
    assert!(
        shipped.contains("instruction_ceiling.rs DOCUMENTS"),
        "a shipped skill file's refusal must send the reader to DOCUMENTS: {shipped}"
    );
    let own = byte_refusal("AGENTS.md", 1, 0);
    assert!(
        own.contains("instruction_ceiling.rs OWN_INSTRUCTIONS"),
        "an own-instruction entry's refusal must send the reader to OWN_INSTRUCTIONS: {own}"
    );
    assert!(
        !own.contains("DOCUMENTS"),
        "an own-instruction entry's refusal points at a table that does not hold it: {own}"
    );
    let unregistered = byte_refusal("nowhere/absent.md", 1, 0);
    assert!(
        !unregistered.contains("DOCUMENTS") && !unregistered.contains("OWN_INSTRUCTIONS"),
        "a path neither table holds is sent to a table that cannot hold it: {unregistered}"
    );
}
