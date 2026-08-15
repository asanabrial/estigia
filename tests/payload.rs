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
    // Estigia exposes, and this repository has a changelog it
    // will one day be asked to read. It fails closed on a missing or empty
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
