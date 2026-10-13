#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Drives the real `scripts/assemble-changelog.sh`, which folds `changes/`
//! fragments into the changelog when a version is cut. `sh` is part of the
//! platform: failing to spawn it is a broken fixture, never a skip.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CHANGELOG: &str = "# Changelog

## 0.2.0 — unreleased

Intro.

### The harness

- **Older.** Issue #1. Already there.

### Setup

- **Setup entry.** Issue #2.

### Empty

## 0.1.0 — 2026-01-01

### The harness

- **Released.** Issue #3.
";

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/assemble-changelog.sh")
}

fn assemble(version: &str, changelog: &Path, changes: &Path) -> Output {
    Command::new("sh")
        .arg(script())
        .arg(version)
        .arg(changelog)
        .arg(changes)
        .output()
        .expect("sh runs")
}

struct Rig {
    directory: tempfile::TempDir,
}

impl Rig {
    fn new(changelog: &str) -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(directory.path().join("changes")).expect("changes/");
        std::fs::write(directory.path().join("CHANGELOG.md"), changelog).expect("the changelog");
        Self { directory }
    }

    fn fragment(&self, name: &str, text: &str) {
        std::fs::write(self.directory.path().join("changes").join(name), text).expect("a fragment");
    }

    fn changelog(&self) -> PathBuf {
        self.directory.path().join("CHANGELOG.md")
    }

    fn changes(&self) -> PathBuf {
        self.directory.path().join("changes")
    }

    fn run(&self, version: &str) -> Output {
        assemble(version, &self.changelog(), &self.changes())
    }

    fn text(&self) -> String {
        std::fs::read_to_string(self.changelog()).expect("the changelog")
    }

    fn remaining(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.changes())
            .expect("changes/")
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn fragments_fold_into_their_sections_newest_issue_first_and_are_deleted() {
    let rig = Rig::new(CHANGELOG);
    rig.fragment(
        "9-nine.md",
        "section: The harness\n\n- **Nine.** Issue #9.\n",
    );
    rig.fragment(
        "10-ten.md",
        "section: The harness\n\n- **Ten.** Issue #10.\n  A second line.\n\n",
    );
    rig.fragment("4-four.md", "section: Setup\n- **Four.** Issue #4.\n");
    rig.fragment("5-five.md", "section: Empty\n- **Five.** Issue #5.\n");

    let output = rig.run("0.2.0");
    assert!(output.status.success(), "{}", stderr(&output));

    assert_eq!(
        rig.text(),
        "# Changelog

## 0.2.0 — unreleased

Intro.

### The harness

- **Ten.** Issue #10.
  A second line.
- **Nine.** Issue #9.
- **Older.** Issue #1. Already there.

### Setup

- **Four.** Issue #4.
- **Setup entry.** Issue #2.

### Empty

- **Five.** Issue #5.

## 0.1.0 — 2026-01-01

### The harness

- **Released.** Issue #3.
"
    );
    assert!(rig.remaining().is_empty(), "{:?}", rig.remaining());
}

#[test]
fn two_branches_that_each_add_a_fragment_assemble_to_the_same_text_in_either_order() {
    let first = Rig::new(CHANGELOG);
    first.fragment("20-a.md", "section: The harness\n\n- **A.** Issue #20.\n");
    first.fragment("2-b.md", "section: The harness\n\n- **B.** Issue #2.\n");
    let second = Rig::new(CHANGELOG);
    second.fragment("2-b.md", "section: The harness\n\n- **B.** Issue #2.\n");
    second.fragment("20-a.md", "section: The harness\n\n- **A.** Issue #20.\n");

    assert!(first.run("0.2.0").status.success());
    assert!(second.run("0.2.0").status.success());
    assert_eq!(first.text(), second.text());
}

#[test]
fn a_version_only_matches_a_heading_it_opens() {
    let rig = Rig::new(&CHANGELOG.replace("## 0.2.0", "## 0.2.01"));
    rig.fragment("4-four.md", "section: Setup\n\n- **Four.**\n");
    let output = rig.run("0.2.0");
    assert!(!output.status.success());
    assert!(stderr(&output).contains("found 0"), "{}", stderr(&output));
}

#[test]
fn a_section_the_version_lacks_leaves_the_changelog_and_the_fragments_untouched() {
    let rig = Rig::new(CHANGELOG);
    rig.fragment("4-four.md", "section: Setup\n\n- **Four.**\n");
    rig.fragment("6-six.md", "section: Nowhere\n\n- **Six.**\n");

    let output = rig.run("0.2.0");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("Nowhere"), "{}", stderr(&output));
    assert_eq!(rig.text(), CHANGELOG);
    assert_eq!(rig.remaining(), ["4-four.md", "6-six.md"]);
}

#[test]
fn a_version_with_no_heading_is_refused() {
    let rig = Rig::new(CHANGELOG);
    rig.fragment("4-four.md", "section: Setup\n\n- **Four.**\n");
    let output = rig.run("9.9.9");
    assert!(!output.status.success());
    assert_eq!(rig.text(), CHANGELOG);
}

#[test]
fn a_file_that_is_not_a_well_formed_fragment_is_refused_rather_than_skipped() {
    for (name, text) in [
        ("notes.md", "section: Setup\n\n- **Loose.**\n"),
        ("4-four.md", "- **No section line.**\n"),
        ("4-four.md", "section: Setup\n\nprose, not a bullet\n"),
        ("4-four.md", "section: Setup\n\n\n"),
    ] {
        let rig = Rig::new(CHANGELOG);
        rig.fragment(name, text);
        let output = rig.run("0.2.0");
        assert!(!output.status.success(), "{name}: {text:?}");
        assert_eq!(rig.text(), CHANGELOG, "{name}: {text:?}");
        assert_eq!(rig.remaining(), [name], "{name}: {text:?}");
    }
}

#[test]
fn an_empty_or_missing_directory_assembles_nothing_and_succeeds() {
    let rig = Rig::new(CHANGELOG);
    assert!(rig.run("0.2.0").status.success());
    assert_eq!(rig.text(), CHANGELOG);

    std::fs::remove_dir(rig.changes()).expect("an empty directory");
    assert!(rig.run("0.2.0").status.success());
    assert_eq!(rig.text(), CHANGELOG);
}

#[test]
fn the_script_and_the_rust_definition_of_a_fragment_agree_on_every_case() {
    let cases = [
        ("4-four.md", "section: Setup\n\n- **Four.**\n", true),
        ("123-a-b.md", "section: Setup\n\n- **A.**\n", true),
        ("1-.md", "section: Setup\n\n- **A.**\n", true),
        ("123.feature.md", "section: Setup\n\n- **A.**\n", false),
        ("123.md", "section: Setup\n\n- **A.**\n", false),
        ("notes.md", "section: Setup\n\n- **A.**\n", false),
        ("x-1.md", "section: Setup\n\n- **A.**\n", false),
        ("4-four.txt", "section: Setup\n\n- **A.**\n", false),
        ("4-four.md", "- **No section line.**\n", false),
        ("4-four.md", "section:Setup\n\n- **A.**\n", false),
        ("4-four.md", "section: \n\n- **A.**\n", false),
        ("4-four.md", " section: Setup\n\n- **A.**\n", false),
    ];
    for (name, text, expected) in cases {
        let rig = Rig::new(CHANGELOG);
        rig.fragment(name, text);
        let script_accepts = rig.run("0.2.0").status.success();
        let first_line = text.split('\n').next().unwrap();
        assert_eq!(
            estigia::transport::changelog::is_fragment(name, first_line),
            expected,
            "{name}: {text:?}"
        );
        assert_eq!(script_accepts, expected, "the script, for {name}: {text:?}");
    }
}

#[test]
fn this_repositorys_own_fragments_assemble_into_its_changelog() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rig = Rig::new(&std::fs::read_to_string(root.join("CHANGELOG.md")).expect("changelog"));
    let own = root.join("changes");
    let names: Vec<_> = match std::fs::read_dir(&own) {
        Ok(entries) => entries
            .map(|entry| entry.expect("an entry").file_name())
            .filter(|name| !name.to_string_lossy().starts_with('.'))
            .collect(),
        Err(error) => {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::NotFound,
                "changes/ is neither readable nor absent: {error}"
            );
            Vec::new()
        }
    };

    if names.is_empty() {
        assert!(
            !own.exists() || own.is_dir(),
            "changes/ exists and is not a directory"
        );
    } else {
        for name in &names {
            std::fs::copy(own.join(name), rig.changes().join(name)).expect("a copy");
        }
        let before = rig.text();

        let output = rig.run(env!("CARGO_PKG_VERSION"));

        assert!(output.status.success(), "{}", stderr(&output));
        assert!(rig.remaining().is_empty(), "{:?}", rig.remaining());
        assert_ne!(
            rig.text(),
            before,
            "{} fragments changed nothing",
            names.len()
        );
    }
}
