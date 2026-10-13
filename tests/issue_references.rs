#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The shipped text cites no issue number from before the repository was public.

use std::path::{Path, PathBuf};

const PUBLIC_ISSUES_THROUGH: u64 = 100;

const NOT_ISSUE_REFERENCES: &[(&str, u64, &str)] = &[
    ("CHANGELOG.md", 464, "`close #464, #469, #473 and #474`"),
    ("CHANGELOG.md", 469, "`close #464, #469, #473 and #474`"),
    ("CHANGELOG.md", 473, "`close #464, #469, #473 and #474`"),
    ("CHANGELOG.md", 474, "`close #464, #469, #473 and #474`"),
    ("CHANGELOG.md", 464, "GitHub closed #464 on"),
    ("CHANGELOG.md", 464, "skipping #464's `done`"),
    ("CHANGELOG.md", 318, "another repository's #318"),
    ("CHANGELOG.md", 167, "(PR #167)"),
    ("docs/parity.md", 4967, "removed SDD and OpenSpec in #4967"),
    ("docs/honesty.md", 70, "another repository's #70"),
    ("docs/honesty.md", 464, "`close #464`"),
    ("docs/honesty.md", 464, "closed #464 on merge"),
    ("docs/honesty.md", 318, "another repository's #318"),
    (
        "skill/references/safety-incidents.md",
        92,
        "another repo's #92",
    ),
    ("skill/references/safety-incidents.md", 167, "PR #167"),
    ("skill/bindings/github/delivery.md", 464, "`close #464`"),
    (
        "skill/bindings/github/delivery.md",
        464,
        "closes #464 on merge",
    ),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_under(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn shipped_text() -> Vec<(String, String)> {
    let root = root();
    let mut paths = vec![
        root.join("AGENTS.md"),
        root.join("README.md"),
        root.join("CHANGELOG.md"),
    ];
    for dir in ["docs", "skill", "changes", "src"] {
        files_under(&root.join(dir), &mut paths);
    }
    let mut texts = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(&root)
            .expect("under the root")
            .to_string_lossy()
            .replace('\\', "/");
        let is_rust = relative.ends_with(".rs");
        let is_text = relative.ends_with(".md") || relative.ends_with(".txt");
        if !is_rust && !is_text {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
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
    for (path, text) in shipped_text() {
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
         ({PUBLIC_ISSUES_THROUGH}); drop the number, or raise the constant by hand if the issue \
         is public, or list a non-issue use in NOT_ISSUE_REFERENCES:\n{}",
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
