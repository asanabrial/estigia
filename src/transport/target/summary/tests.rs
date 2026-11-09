use super::*;

fn manifests(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter_map(|line| {
            line.strip_prefix(MOVED_PREFIX)
                .or_else(|| line.strip_prefix(DELETED_PREFIX))
        })
        .map(|json| serde_json::from_str(json).expect("a manifest line carries JSON"))
        .collect()
}

fn diff(parts: &[String]) -> String {
    parts.concat()
}

fn block(tag: &str, lines: usize) -> String {
    (0..lines)
        .map(|line| format!("{tag} {line:04} padding padding padding padding\n"))
        .collect()
}

fn removed(text: &str) -> String {
    text.lines().map(|line| format!("-{line}\n")).collect()
}

fn added(text: &str) -> String {
    text.lines().map(|line| format!("+{line}\n")).collect()
}

#[test]
fn a_verbatim_move_between_two_files_is_one_manifest_line_on_each_side() {
    let moved = block("moved", 12);
    let raw = diff(&[
        "diff --git a/a.txt b/a.txt\n".to_owned(),
        "index 1111111..2222222 100644\n".to_owned(),
        "--- a/a.txt\n".to_owned(),
        "+++ b/a.txt\n".to_owned(),
        "@@ -1,14 +1,2 @@\n".to_owned(),
        " one\n".to_owned(),
        " two\n".to_owned(),
        removed(&moved),
        "diff --git a/b.txt b/b.txt\n".to_owned(),
        "index 3333333..4444444 100644\n".to_owned(),
        "--- a/b.txt\n".to_owned(),
        "+++ b/b.txt\n".to_owned(),
        "@@ -1,2 +1,14 @@\n".to_owned(),
        " x\n".to_owned(),
        added(&moved),
        " y\n".to_owned(),
    ]);
    let summary = summarise(&raw);
    assert_eq!(summary.moves, 1);
    assert_eq!(summary.deletions, 0);
    assert!(summary.saved > 0);
    assert!(
        !summary
            .text
            .lines()
            .any(|line| line.starts_with("-moved") || line.starts_with("+moved")),
        "{}",
        summary.text
    );
    assert_eq!(summary.text.matches(MOVED_PREFIX).count(), 2);
    let carried = manifests(&summary.text);
    assert_eq!(carried.len(), 2);
    let moved_line = &carried[0];
    assert_eq!(moved_line["from"], "a.txt");
    assert_eq!(moved_line["from-lines"], "3-14");
    assert_eq!(moved_line["to"], "b.txt");
    assert_eq!(moved_line["to-lines"], "2-13");
    assert_eq!(moved_line["bytes"], moved.len());
    assert_eq!(
        moved_line["sha256"],
        sha256_hex(moved.as_bytes()),
        "the manifest does not carry the digest of the bytes it elides"
    );
    assert_eq!(
        summary.text.matches("@@ -1,2 +1,2 @@\n").count(),
        2,
        "the elided hunks were not recounted: {}",
        summary.text
    );
}

#[test]
fn a_whole_file_deletion_is_one_manifest_line_carrying_the_deleted_digest() {
    let gone = block("gone", 12);
    let raw = diff(&[
        "diff --git a/gone.txt b/gone.txt\n".to_owned(),
        "deleted file mode 100644\n".to_owned(),
        "index 1111111..0000000\n".to_owned(),
        "--- a/gone.txt\n".to_owned(),
        "+++ /dev/null\n".to_owned(),
        "@@ -1,12 +0,0 @@\n".to_owned(),
        removed(&gone),
    ]);
    let summary = summarise(&raw);
    assert_eq!(summary.deletions, 1);
    assert!(summary.saved > 0);
    assert!(
        !summary.text.lines().any(|line| line.starts_with("-gone")),
        "{}",
        summary.text
    );
    let carried = manifests(&summary.text);
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0]["path"], "gone.txt");
    assert_eq!(carried[0]["lines"], "1-12");
    assert_eq!(carried[0]["bytes"], gone.len());
    assert_eq!(carried[0]["sha256"], sha256_hex(gone.as_bytes()));
    assert!(summary.text.contains("deleted file mode 100644\n"));
}

#[test]
fn a_whole_file_move_is_a_manifest_line_on_the_deleting_and_adding_sides() {
    let content = block("moved", 12);
    let raw = diff(&[
        "diff --git a/old.txt b/old.txt\n".to_owned(),
        "deleted file mode 100644\n".to_owned(),
        "index 1111111..0000000\n".to_owned(),
        "--- a/old.txt\n".to_owned(),
        "+++ /dev/null\n".to_owned(),
        "@@ -1,12 +0,0 @@\n".to_owned(),
        removed(&content),
        "diff --git a/new.txt b/new.txt\n".to_owned(),
        "new file mode 100644\n".to_owned(),
        "index 0000000..1111111\n".to_owned(),
        "--- /dev/null\n".to_owned(),
        "+++ b/new.txt\n".to_owned(),
        "@@ -0,0 +1,12 @@\n".to_owned(),
        added(&content),
    ]);
    let summary = summarise(&raw);
    assert_eq!(summary.moves, 1);
    assert_eq!(summary.deletions, 0);
    let carried = manifests(&summary.text);
    assert_eq!(carried.len(), 2);
    assert_eq!(carried[0]["from"], "old.txt");
    assert_eq!(carried[0]["to"], "new.txt");
    assert_eq!(carried[0]["from-lines"], "1-12");
    assert_eq!(carried[0]["to-lines"], "1-12");
    assert_eq!(carried[0]["bytes"], content.len());
    assert_eq!(carried[0]["sha256"], sha256_hex(content.as_bytes()));
    assert!(
        !summary
            .text
            .lines()
            .any(|line| line.starts_with("+moved") || line.starts_with("-moved"))
    );
}

#[test]
fn an_edited_move_stays_as_hunks() {
    let moved = block("moved", 12);
    let edited = moved.replacen("padding", "PADDING", 1);
    let raw = diff(&[
        "diff --git a/a.txt b/a.txt\n".to_owned(),
        "index 1111111..2222222 100644\n".to_owned(),
        "--- a/a.txt\n".to_owned(),
        "+++ b/a.txt\n".to_owned(),
        "@@ -1,14 +1,2 @@\n".to_owned(),
        " one\n".to_owned(),
        " two\n".to_owned(),
        removed(&moved),
        "diff --git a/b.txt b/b.txt\n".to_owned(),
        "index 3333333..4444444 100644\n".to_owned(),
        "--- a/b.txt\n".to_owned(),
        "+++ b/b.txt\n".to_owned(),
        "@@ -1,2 +1,14 @@\n".to_owned(),
        " x\n".to_owned(),
        added(&edited),
        " y\n".to_owned(),
    ]);
    let summary = summarise(&raw);
    assert_eq!(summary.moves, 0);
    assert_eq!(summary.deletions, 0);
    assert_eq!(summary.text, raw, "an edited move is authored text");
    assert!(!summary.text.contains(MOVED_PREFIX));
}

#[test]
fn an_ordinary_deletion_inside_a_modified_file_stays_a_hunk() {
    let gone = block("gone", 12);
    let raw = diff(&[
        "diff --git a/a.txt b/a.txt\n".to_owned(),
        "index 1111111..2222222 100644\n".to_owned(),
        "--- a/a.txt\n".to_owned(),
        "+++ b/a.txt\n".to_owned(),
        "@@ -1,14 +1,2 @@\n".to_owned(),
        " one\n".to_owned(),
        removed(&gone),
        " two\n".to_owned(),
    ]);
    let summary = summarise(&raw);
    assert_eq!(summary.deletions, 0);
    assert_eq!(summary.text, raw);
}

#[test]
fn a_move_too_small_to_pay_for_its_manifest_line_stays_as_hunks() {
    let raw = diff(&[
        "diff --git a/a.txt b/a.txt\n".to_owned(),
        "index 1111111..2222222 100644\n".to_owned(),
        "--- a/a.txt\n".to_owned(),
        "+++ b/a.txt\n".to_owned(),
        "@@ -1,2 +1,1 @@\n".to_owned(),
        " one\n".to_owned(),
        "-two\n".to_owned(),
        "diff --git a/b.txt b/b.txt\n".to_owned(),
        "index 3333333..4444444 100644\n".to_owned(),
        "--- a/b.txt\n".to_owned(),
        "+++ b/b.txt\n".to_owned(),
        "@@ -1,1 +1,2 @@\n".to_owned(),
        " x\n".to_owned(),
        "+two\n".to_owned(),
    ]);
    let summary = summarise(&raw);
    assert_eq!(summary.moves, 0);
    assert_eq!(summary.text, raw);
}

#[test]
fn a_move_whose_last_line_has_no_newline_matches_byte_for_byte() {
    let moved = block("moved", 12);
    let moved = moved.trim_end_matches('\n').to_owned();
    let raw = diff(&[
        "diff --git a/a.txt b/a.txt\n".to_owned(),
        "index 1111111..2222222 100644\n".to_owned(),
        "--- a/a.txt\n".to_owned(),
        "+++ b/a.txt\n".to_owned(),
        "@@ -1,14 +1,2 @@\n".to_owned(),
        " one\n".to_owned(),
        " two\n".to_owned(),
        removed(&moved),
        "\\ No newline at end of file\n".to_owned(),
        "diff --git a/b.txt b/b.txt\n".to_owned(),
        "index 3333333..4444444 100644\n".to_owned(),
        "--- a/b.txt\n".to_owned(),
        "+++ b/b.txt\n".to_owned(),
        "@@ -1,2 +1,14 @@\n".to_owned(),
        " x\n".to_owned(),
        added(&moved),
        "\\ No newline at end of file\n".to_owned(),
        " y\n".to_owned(),
    ]);
    let summary = summarise(&raw);
    assert_eq!(summary.moves, 1);
    let carried = manifests(&summary.text);
    assert_eq!(carried[0]["bytes"], moved.len());
    assert_eq!(carried[0]["sha256"], sha256_hex(moved.as_bytes()));
    assert!(!summary.text.contains("\\ No newline at end of file\n"));
}

#[test]
fn a_section_whose_paths_cannot_be_read_is_left_exactly_as_it_was() {
    let raw = diff(&[
        "diff --git \"a/a b.txt\" \"b/a b.txt\"\n".to_owned(),
        "index 1111111..2222222 100644\n".to_owned(),
        "--- \"a/a b.txt\"\n".to_owned(),
        "+++ \"b/a b.txt\"\n".to_owned(),
        "@@ -1,2 +1,1 @@\n".to_owned(),
        " one\n".to_owned(),
        "-two\n".to_owned(),
    ]);
    let summary = summarise(&raw);
    assert_eq!(summary.text, raw);
}

#[test]
fn a_diff_that_is_not_a_unified_diff_is_returned_whole() {
    for raw in ["", "not a diff\n", "Binary files a/x and b/x differ\n"] {
        let summary = summarise(raw);
        assert_eq!(summary.text, raw);
        assert_eq!(summary.moves, 0);
        assert_eq!(summary.deletions, 0);
    }
}

#[test]
fn context_lines_and_augmented_hunks_keep_their_start_and_trailing_text() {
    let moved = block("moved", 12);
    let raw = diff(&[
        "diff --git a/a.txt b/a.txt\n".to_owned(),
        "index 1111111..2222222 100644\n".to_owned(),
        "--- a/a.txt\n".to_owned(),
        "+++ b/a.txt\n".to_owned(),
        "@@ -10,14 +10,2 @@ fn main() {\n".to_owned(),
        " ten\n".to_owned(),
        " eleven\n".to_owned(),
        removed(&moved),
        "diff --git a/b.txt b/b.txt\n".to_owned(),
        "index 3333333..4444444 100644\n".to_owned(),
        "--- a/b.txt\n".to_owned(),
        "+++ b/b.txt\n".to_owned(),
        "@@ -1,1 +1,13 @@\n".to_owned(),
        " x\n".to_owned(),
        added(&moved),
    ]);
    let summary = summarise(&raw);
    assert!(
        summary.text.contains("@@ -10,2 +10,2 @@ fn main() {\n"),
        "{}",
        summary.text
    );
    assert!(summary.text.contains("@@ -1 +1 @@\n"), "{}", summary.text);
}
