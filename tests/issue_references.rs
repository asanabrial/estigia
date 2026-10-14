#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The shipped text cites no issue number from before the repository was public.

use std::path::{Path, PathBuf};

const PUBLIC_ISSUES_THROUGH: u64 = 93;

const NOT_ISSUE_REFERENCES: &[(&str, u64, &str)] = &[
    ("CHANGELOG.md", 464, "`close #464, #469, #473 and #474`"),
    ("CHANGELOG.md", 469, "`close #464, #469, #473 and #474`"),
    ("CHANGELOG.md", 473, "`close #464, #469, #473 and #474`"),
    ("CHANGELOG.md", 474, "`close #464, #469, #473 and #474`"),
    ("CHANGELOG.md", 464, "GitHub closed #464 on"),
    ("CHANGELOG.md", 464, "skipping #464's `done`"),
    ("CHANGELOG.md", 318, "another repository's #318"),
    ("docs/parity.md", 4967, "removed SDD and OpenSpec in #4967"),
    ("docs/honesty.md", 464, "`close #464`"),
    ("docs/honesty.md", 464, "closed #464 on merge"),
    ("docs/honesty.md", 318, "another repository's #318"),
    ("skill/bindings/github/delivery.md", 464, "`close #464`"),
    (
        "skill/bindings/github/delivery.md",
        464,
        "closes #464 on merge",
    ),
];

fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("cannot read an entry of {}: {error}", dir.display()))
            .path();
        if path.is_dir() {
            files_under(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn shipped_text(root: &Path) -> Vec<(String, String)> {
    let mut paths = vec![
        root.join("AGENTS.md"),
        root.join("README.md"),
        root.join("CHANGELOG.md"),
    ];
    for dir in ["docs", "skill", "changes", "src"] {
        let path = root.join(dir);
        if dir == "changes" && !path.exists() {
            continue;
        }
        files_under(&path, &mut paths);
    }
    let mut texts = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .expect("under the root")
            .to_string_lossy()
            .replace('\\', "/");
        let is_rust = relative.ends_with(".rs");
        let is_text = relative.ends_with(".md") || relative.ends_with(".txt");
        if !is_rust && !is_text {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let text = if is_rust {
            if relative.ends_with("/tests.rs") || relative.contains("/tests/") {
                continue;
            }
            let code = estigia::source_text::code_of(&text);
            estigia::source_text::shipped_part(&code).to_string()
        } else {
            text
        };
        texts.push((relative, text));
    }
    for prefix in ["docs/", "skill/", "src/"] {
        assert!(
            texts.iter().any(|(path, _)| path.starts_with(prefix)),
            "the scan read nothing under {prefix} of {}",
            root.display()
        );
    }
    texts
}

fn references_in(text: &str) -> Vec<(u64, &str)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte != b'#' {
            continue;
        }
        let digits: String = text[at + 1..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if digits.is_empty() {
            continue;
        }
        if let Some(before) = text[..at].chars().next_back() {
            let named_repository = text[..at].ends_with("estigia");
            if (before.is_alphanumeric() || before == '_' || before == '&' || before == '/')
                && !named_repository
            {
                continue;
            }
        }
        if let Ok(number) = digits.parse() {
            let start = text[..at].rfind('\n').map_or(0, |newline| newline + 1);
            let end = text[at..]
                .find('\n')
                .map_or(text.len(), |newline| at + newline);
            found.push((number, &text[start..end]));
        }
    }
    found
}

#[test]
fn shipped_text_cites_no_issue_number_beyond_the_public_ones() {
    let mut stray = Vec::new();
    for (path, text) in shipped_text(&PathBuf::from(env!("CARGO_MANIFEST_DIR"))) {
        for (number, line) in references_in(&text) {
            let allowed = NOT_ISSUE_REFERENCES.iter().any(|(file, allowed, context)| {
                *file == path && *allowed == number && line.contains(context)
            });
            if number > PUBLIC_ISSUES_THROUGH && !allowed {
                stray.push(format!("{path}: #{number}"));
            }
        }
    }
    stray.sort();
    stray.dedup();
    assert!(
        stray.is_empty(),
        "these cite an old issue reference, or an issue newer than PUBLIC_ISSUES_THROUGH \
         ({PUBLIC_ISSUES_THROUGH}); drop the number, or raise the constant by hand once the public \
         tracker has passed it, or list a non-issue use in NOT_ISSUE_REFERENCES:\n{}",
        stray.join("\n")
    );
}

#[test]
fn the_reference_scanner_reads_issue_numbers_and_nothing_else() {
    let numbers = |text| {
        references_in(text)
            .into_iter()
            .map(|(number, _)| number)
            .collect::<Vec<_>>()
    };
    assert_eq!(numbers("issue #117 and (#50)"), vec![117, 50]);
    assert_eq!(numbers("estigia#482"), vec![482]);
    assert!(numbers("color #fff, ## 3, a/b#13045, &#123;, x_#9").is_empty());
}

#[test]
fn every_allowed_non_issue_use_still_matches_a_line_of_its_file() {
    let texts = shipped_text(&PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let mut stale = Vec::new();
    for (file, number, context) in NOT_ISSUE_REFERENCES {
        let matched = texts.iter().any(|(path, text)| {
            path == file
                && references_in(text)
                    .iter()
                    .any(|(found, line)| found == number && line.contains(context))
        });
        if !matched {
            stale.push(format!("{file}: #{number} in {context:?}"));
        }
    }
    assert!(
        stale.is_empty(),
        "these NOT_ISSUE_REFERENCES entries match no line any more; delete them:\n{}",
        stale.join("\n")
    );
}

#[test]
fn no_allowed_non_issue_use_is_at_or_below_the_ceiling() {
    for (file, number, _) in NOT_ISSUE_REFERENCES {
        assert!(
            *number > PUBLIC_ISSUES_THROUGH,
            "{file}: #{number} is at or below PUBLIC_ISSUES_THROUGH and needs no entry"
        );
    }
}

#[test]
#[should_panic(expected = "AGENTS.md")]
fn a_root_without_the_anchor_files_is_refused() {
    let root = tempfile::tempdir().unwrap();
    for dir in ["docs", "skill", "src"] {
        std::fs::create_dir_all(root.path().join(dir)).unwrap();
    }
    shipped_text(root.path());
}

#[test]
#[should_panic(expected = "the scan read nothing under docs/")]
fn a_root_whose_directories_hold_nothing_is_refused() {
    let root = tempfile::tempdir().unwrap();
    for dir in ["docs", "skill", "src"] {
        std::fs::create_dir_all(root.path().join(dir)).unwrap();
    }
    for anchor in ["AGENTS.md", "README.md", "CHANGELOG.md"] {
        std::fs::write(root.path().join(anchor), "").unwrap();
    }
    shipped_text(root.path());
}

#[test]
#[should_panic(expected = "cannot read")]
fn a_file_that_exists_and_cannot_be_read_fails_the_scan() {
    let root = tempfile::tempdir().unwrap();
    for dir in ["docs", "skill", "src"] {
        std::fs::create_dir_all(root.path().join(dir)).unwrap();
    }
    for anchor in ["AGENTS.md", "README.md", "CHANGELOG.md"] {
        std::fs::write(root.path().join(anchor), "").unwrap();
    }
    std::fs::write(root.path().join("docs").join("bad.md"), [0xff, 0xfe, 0xfd]).unwrap();
    shipped_text(root.path());
}
