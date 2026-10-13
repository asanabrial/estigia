use super::*;
use crate::transport::{fenced_git_env, run_with_env};

fn manifest(entries: &[(&str, &str, &str)]) -> manifest::Manifest {
    entries
        .iter()
        .map(|(path, mode, blob)| ((*path).to_owned(), ((*mode).to_owned(), (*blob).to_owned())))
        .collect()
}

#[test]
fn a_supplied_target_that_covers_less_than_the_branch_delivers_is_named_path_by_path() {
    // The whole point of `--native-start`, and the port had no comparison at
    // all while the MCP server offered agents the argument. The three lists are
    // not interchangeable: `unreviewed` is the one that ships without
    // authority, and reporting it as `differing` would read as a rebase.
    let target = manifest(&[
        ("src/a.rs", "100644", "aaa"),
        ("src/b.rs", "100644", "bbb"),
        ("src/c.rs", "100644", "ccc"),
    ]);
    let claimed = manifest(&[
        ("src/a.rs", "100644", "aaa"),
        ("src/b.rs", "100755", "bbb"),
        ("src/gone.rs", "100644", "ddd"),
    ]);
    let found = difference(&target, &claimed);
    assert_eq!(found.unreviewed, [&"src/c.rs".to_owned()]);
    assert_eq!(found.not_delivered, [&"src/gone.rs".to_owned()]);
    assert_eq!(
        found.differing,
        [&"src/b.rs".to_owned()],
        "a mode flip is a difference: an executable bit is half of what a delivery is"
    );

    // The same target twice is no difference at all, or every review is a stop.
    let same = difference(&target, &target);
    assert!(
        same.unreviewed.is_empty() && same.not_delivered.is_empty() && same.differing.is_empty(),
        "a target disagreed with itself"
    );
}

#[test]
fn an_unreadable_supplied_target_is_a_read_failure_and_never_a_mismatch() {
    // Both are refusals, and saying the wrong one is a lie about what happened:
    // a mismatch says the reviewer was shown the wrong thing. An unreadable
    // file says nothing was compared.
    let root = std::env::temp_dir().join("estigia-native-start-probe");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a directory");

    let absent = root.join("absent.json");
    assert!(matches!(supplied_manifest(&absent), Err(Failure::Read(_))));

    for (name, body) in [
        ("not-json.json", "{"),
        ("no-list.json", "{\"manifest\": {}}"),
        (
            "malformed.json",
            "[{\"path\": \"a\", \"mode\": \"100644\"}]",
        ),
    ] {
        let path = root.join(name);
        std::fs::write(&path, body).expect("the probe is written");
        assert!(
            matches!(supplied_manifest(&path), Err(Failure::Read(_))),
            "{name} was not read as a read failure"
        );
    }

    // Both shapes the transport accepts: a bare list, and one under `manifest`.
    let entry = "[{\"path\": \"src/a.rs\", \"mode\": \"100644\", \"blob\": \"aaa\"}]";
    for (name, body) in [
        ("bare.json", entry.to_owned()),
        ("wrapped.json", format!("{{\"manifest\": {entry}}}")),
    ] {
        let path = root.join(name);
        std::fs::write(&path, &body).expect("the probe is written");
        assert_eq!(
            supplied_manifest(&path).expect("a manifest"),
            manifest(&[("src/a.rs", "100644", "aaa")]),
            "{name} did not read back"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_dirty_target_cannot_be_bound_to_a_published_head() {
    let dirty = serde_json::json!({"uncommitted": ["src/lib.rs"]});
    assert!(
        dirty["uncommitted"]
            .as_array()
            .is_some_and(|paths| !paths.is_empty()),
        "the fixture stopped describing a dirty target"
    );

    let source = crate::source_text::code_of(include_str!("../target.rs"));
    let body = source
        .split_once("pub fn clean_target(")
        .expect("the clean publication boundary exists")
        .1;
    assert!(
        body.contains("dirty-review-target") && body.contains("expected_target("),
        "publication no longer derives the complete target and refuses its dirty suffix"
    );
}

#[test]
fn a_clean_publication_snapshot_is_bounded_by_the_same_head_and_empty_status() {
    let head = "a".repeat(40);
    let target = serde_json::json!({"head": head, "uncommitted": []});
    coherent_clean_snapshot(&head, "", &target, &head, "").expect("one clean snapshot");

    let moved = coherent_clean_snapshot(&head, "", &target, &"b".repeat(40), "")
        .expect_err("HEAD moved while the manifest was read");
    assert_eq!(
        moved.envelope()["reason"],
        "review-target-moved-during-read"
    );

    for (before, after) in [(" M src/lib.rs\0", ""), ("", "?? later\0")] {
        let dirty = coherent_clean_snapshot(&head, before, &target, &head, after)
            .expect_err("dirty at one boundary");
        assert_eq!(dirty.envelope()["reason"], "dirty-review-target");
    }

    let mixed = serde_json::json!({"head": head, "uncommitted": ["src/lib.rs"]});
    let dirty =
        coherent_clean_snapshot(&head, "", &mixed, &head, "").expect_err("the inner read saw dirt");
    assert_eq!(dirty.envelope()["reason"], "dirty-review-target");
}

fn git(at: &std::path::Path, args: &[&str]) {
    crate::transport::run(args, Some(at), crate::transport::How::write())
        .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
}

fn oid(at: &std::path::Path) -> String {
    crate::transport::run(
        &["git", "rev-parse", "HEAD"],
        Some(at),
        crate::transport::How::read(),
    )
    .expect("HEAD")
    .stdout
    .trim()
    .to_owned()
}

fn two_commit_repo(later: &str) -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    git(at, &["git", "init", "--quiet", "-b", "main", "."]);
    git(
        at,
        &["git", "config", "user.email", "estigia@example.invalid"],
    );
    git(at, &["git", "config", "user.name", "Estigia test"]);
    git(at, &["git", "config", "core.autocrlf", "false"]);
    std::fs::write(at.join("keep.txt"), "base\n").expect("the base file");
    git(at, &["git", "add", "keep.txt"]);
    git(at, &["git", "commit", "--quiet", "-m", "base"]);
    let base = oid(at);
    std::fs::write(at.join("keep.txt"), later).expect("the head file");
    git(at, &["git", "add", "keep.txt"]);
    git(at, &["git", "commit", "--quiet", "-m", "head"]);
    let head = oid(at);
    (root, base, head)
}

#[test]
fn an_oversize_review_payload_is_refused_not_truncated() {
    let later = format!("{}\n", "x".repeat(200));
    let (root, base, head) = two_commit_repo(&later);
    let ceiling = 64usize;
    let failure = review_payload_bounded(root.path(), &base, &head, ceiling)
        .expect_err("an oversize payload was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "review-payload-too-large");
    let bytes = envelope["bytes"]
        .as_u64()
        .expect("the refusal names the size");
    assert!(
        bytes > ceiling as u64,
        "the refusal named a size that fits: {bytes} <= {ceiling}"
    );
    assert_eq!(envelope["ceiling"], ceiling);
    assert!(
        envelope["action"]
            .as_str()
            .is_some_and(|action| action.contains("too large to review as one unit")),
        "the refusal must name the change as too large: {envelope}"
    );
}

#[test]
fn a_review_payload_is_the_unified_diff_of_the_receipt_ends() {
    let (root, base, head) = two_commit_repo("changed\n");
    let payload = review_payload(root.path(), &base, &head).expect("a payload");
    assert!(
        payload.text.contains("keep.txt") && payload.text.contains("changed"),
        "the payload is not the change: {}",
        payload.text
    );
    assert_eq!(
        payload.digest,
        crate::transport::ownership::sha256_hex(payload.text.as_bytes()),
        "a payload digest that is not the hash of its bytes cannot be crossed with a receipt"
    );
    let again = review_payload(root.path(), &base, &head).expect("the same ends");
    assert_eq!(
        again.digest, payload.digest,
        "the same receipt ends produced two payloads"
    );
}

/// An untracked `.git/info/attributes` reaches an unisolated `git diff` the
/// same way a committed `.gitattributes` does — issue #353's own measurement,
/// 41,334 bytes collapsing to 606 under one `* -diff` line. Reddens against
/// `review_payload_bounded` before it runs isolated: the payload it answers
/// is `Binary files a/keep.txt and b/keep.txt differ` instead of the real
/// hunk, which is exactly the collapse a sealed judge — holding no `Read` and
/// no way to tell a suppressed hunk from an honestly empty one — cannot see
/// happened.
#[test]
fn an_untracked_info_attributes_rule_does_not_collapse_the_review_payload() {
    let (root, base, head) = two_commit_repo("changed\nsecond line\nthird line\n");
    let at = root.path();
    std::fs::create_dir_all(at.join(".git").join("info")).expect("the info directory");
    std::fs::write(at.join(".git").join("info").join("attributes"), "* -diff\n")
        .expect("the untracked attributes file");

    let payload = review_payload(at, &base, &head).expect("a payload");
    assert!(
        !payload.text.contains("Binary files"),
        "an untracked `.git/info/attributes` collapsed the payload to a binary marker: {}",
        payload.text
    );
    assert!(
        payload.text.contains("+changed") && payload.text.contains("+third line"),
        "the payload lost its real hunk: {}",
        payload.text
    );
}

/// A regression guard, not the general claim: this and isolation agree only
/// where nothing ambient is in force, and the comparison is fenced with
/// [`fenced_git_env`] so "ambient" here is the repository's own committed
/// state rather than the machine's. The fixture sets no attribute source and
/// no diff-affecting config, so this holds only that isolating the payload
/// costs nothing over such a repository — it is not evidence about one whose
/// own config sets `diff.algorithm`, `diff.interHunkContext` or
/// `diff.indentHeuristic`; the test below states that case, which this one
/// cannot fail on by construction.
#[test]
fn an_isolated_payload_is_byte_identical_to_an_ambient_one_absent_any_attribute_source() {
    let (root, base, head) = two_commit_repo("changed\nsecond line\nthird line\n");
    let at = root.path();

    let ambient = run_with_env(
        &pinned_diff_args(&["--no-renames", "--unified=3", &base, &head]),
        Some(at),
        How::tolerated(),
        &fenced_git_env(),
    )
    .expect("an ambient diff")
    .stdout;

    let isolated = review_payload(at, &base, &head).expect("an isolated payload");

    assert_eq!(
        isolated.text, ambient,
        "isolating the payload changed its bytes over a repository with no attribute source"
    );
}

/// The fence [`an_isolated_payload_is_byte_identical_to_an_ambient_one_absent_any_attribute_source`]
/// relies on, held to its own claim. An attributes file marking every path
/// `-diff` — the shape `docs/honesty.md` records this repository's own
/// `.gitattributes` already approaches — collapses an unfenced ambient diff;
/// the fence restores it byte-identical to the isolated payload. Both ways such
/// a file reaches a diff are driven: through a global `core.attributesFile` and
/// through the default global attributes file, which is resolved from `HOME`
/// and `XDG_CONFIG_HOME` rather than from any config value the fence could
/// redirect. Without the fence that equality is decided by the machine rather
/// than by the code, and this test is what says so. Issue #359.
#[test]
fn the_ambient_fence_ignores_every_attribute_source_outside_the_repository() {
    let (root, base, head) = two_commit_repo("changed\nsecond line\n");
    let at = root.path();
    let hostile = tempfile::tempdir().expect("a hostile directory");
    let attrs = hostile.path().join("attrs");
    std::fs::write(&attrs, "* -diff\n").expect("the hostile attributes");

    // The config-set source: a global `core.attributesFile`, written through
    // `git config` rather than a hand-built line so a Windows path's
    // backslashes are quoted the way git parses them.
    let gitconfig = hostile.path().join("gitconfig");
    run(
        &[
            "git",
            "config",
            "--file",
            gitconfig.to_str().expect("a UTF-8 config path"),
            "core.attributesFile",
            attrs.to_str().expect("a UTF-8 attributes path"),
        ],
        None,
        How::write(),
    )
    .expect("the hostile global config is written");
    let via_config = [(
        "GIT_CONFIG_GLOBAL",
        gitconfig.to_str().expect("a UTF-8 config path"),
    )];

    // The default-global source: `$XDG_CONFIG_HOME/git/attributes`.
    let hostile_home = hostile.path().join("home");
    let default_attrs = hostile_home.join(".config").join("git").join("attributes");
    std::fs::create_dir_all(default_attrs.parent().expect("a parent")).expect("the directory");
    std::fs::write(&default_attrs, "* -diff\n").expect("the default attributes file");
    let xdg_config_home = hostile_home.join(".config");
    let via_home = [
        ("HOME", hostile_home.to_str().expect("a UTF-8 home")),
        (
            "XDG_CONFIG_HOME",
            xdg_config_home.to_str().expect("a UTF-8 config home"),
        ),
    ];

    let args = pinned_diff_args(&["--no-renames", "--unified=3", &base, &head]);
    let isolated = review_payload(at, &base, &head).expect("an isolated payload");

    for hostile_env in [via_config.as_slice(), via_home.as_slice()] {
        let unfenced = run_with_env(&args, Some(at), How::tolerated(), hostile_env)
            .expect("an unfenced ambient diff")
            .stdout;
        assert_ne!(
            unfenced, isolated.text,
            "a hostile attributes file did not move the unfenced ambient diff, so this test \
             cannot prove the fence is load-bearing"
        );

        let mut envs = hostile_env.to_vec();
        envs.extend(fenced_git_env());
        let fenced = run_with_env(&args, Some(at), How::tolerated(), &envs)
            .expect("a fenced ambient diff")
            .stdout;
        assert_eq!(
            fenced, isolated.text,
            "the fence did not hold: an attribute source outside the repository still reached \
             the ambient side"
        );
    }
}

/// Two commits over a twenty-line file whose only two edits sit ten lines
/// apart — close enough for `diff.interHunkContext=10` to merge what are
/// otherwise two separate hunks into one, changing the byte count. Built
/// apart from [`two_commit_repo`] because that fixture's single-line change
/// has no second hunk for a merge to have an effect on.
fn inter_hunk_context_repo() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    git(at, &["git", "init", "--quiet", "-b", "main", "."]);
    git(
        at,
        &["git", "config", "user.email", "estigia@example.invalid"],
    );
    git(at, &["git", "config", "user.name", "Estigia test"]);
    git(at, &["git", "config", "core.autocrlf", "false"]);
    let base_lines: String = (1..=20).map(|n| format!("l{n}\n")).collect();
    std::fs::write(at.join("keep.txt"), &base_lines).expect("the base file");
    git(at, &["git", "add", "keep.txt"]);
    git(at, &["git", "commit", "--quiet", "-m", "base"]);
    let base = oid(at);
    let head_lines: String = (1..=20)
        .map(|n| match n {
            2 => "CHANGED2\n".to_owned(),
            13 => "CHANGED13\n".to_owned(),
            other => format!("l{other}\n"),
        })
        .collect();
    std::fs::write(at.join("keep.txt"), &head_lines).expect("the head file");
    git(at, &["git", "add", "keep.txt"]);
    git(at, &["git", "commit", "--quiet", "-m", "head"]);
    let head = oid(at);
    (root, base, head)
}

/// The true property behind the equality above, which that test cannot see:
/// isolation makes the payload **independent** of ambient diff configuration,
/// not "identical to ambient output" in general — the two claims coincide
/// only where nothing ambient is in force. `diff.interHunkContext` is not an
/// attribute source and reaches a plain `git diff` regardless of
/// `pinned_diff_args`, which pins prefixes and quoting only. The first
/// assertion proves the knob is genuinely live over this fixture: without it,
/// a fixture the knob no longer moved would make the equality below vacuous,
/// the same blindness the equality test above has on this exact axis. It is
/// not what catches a stop-isolating regression — the second assertion does
/// that alone, since an unisolated payload answers the knob's own rendering
/// and not the baseline it is compared against.
#[test]
fn a_local_diff_inter_hunk_context_moves_the_ambient_payload_but_not_the_isolated_one() {
    let (root, base, head) = inter_hunk_context_repo();
    let at = root.path();

    let ambient_default = run_with_env(
        &pinned_diff_args(&["--no-renames", "--unified=3", &base, &head]),
        Some(at),
        How::tolerated(),
        &fenced_git_env(),
    )
    .expect("an ambient diff")
    .stdout;

    git(at, &["git", "config", "diff.interHunkContext", "10"]);

    let ambient_with_knob = run_with_env(
        &pinned_diff_args(&["--no-renames", "--unified=3", &base, &head]),
        Some(at),
        How::tolerated(),
        &fenced_git_env(),
    )
    .expect("an ambient diff under the knob")
    .stdout;

    assert_ne!(
        ambient_default, ambient_with_knob,
        "diff.interHunkContext did not move the ambient diff over this fixture — the knob is not \
         live, so this test cannot prove anything about isolation"
    );

    let isolated = review_payload(at, &base, &head).expect("an isolated payload");

    assert_eq!(
        isolated.text, ambient_default,
        "a local diff.interHunkContext reached the isolated payload: {}",
        isolated.text
    );
}

#[test]
fn publish_review_materialises_the_payload_from_the_receipt_ends_before_any_write() {
    let source = crate::source_text::code_of(include_str!("../claim.rs"));
    let body = source
        .split_once("fn publish_with(")
        .expect("publication exists")
        .1;
    let payload = body
        .find("review_payload(")
        .expect("publish_with no longer materialises the payload");
    let push = body
        .find("push_to_origin(")
        .expect("publication still pushes");
    assert!(
        payload < push,
        "an oversize payload would be noticed only after the branch was already on the remote"
    );
    let published = source
        .split_once("fn published(")
        .expect("the receipt writer exists")
        .1;
    assert!(
        published.contains("\"payload-digest\"") && published.contains("payload.text"),
        "the receipt no longer binds the payload it handed the judges"
    );
}

/// Issue #306: the base-ancestry refusal precedes the payload materialisation,
/// and this binds that order. The binding is source-text rather than
/// behavioural because the property is the call order itself: the behavioural
/// alternative — a base advance larger than `REVIEW_PAYLOAD_CEILING` whose
/// refusal must stay `publication-base-not-ancestor` rather than
/// `review-payload-too-large` — tests the consequence but costs a 262 KB
/// fixture and still cannot name the order when it fails. Reading through
/// `source_text::code_of` means the comment above the call cannot move the
/// boundary, so the assertion holds the code and not the prose. Run as a
/// mutation for #306: moving the `is_ancestor` block below `review_payload`
/// reddens this test with the message below.
#[test]
fn the_base_ancestry_refusal_precedes_the_payload_materialisation() {
    let source = crate::source_text::code_of(include_str!("../claim.rs"));
    let body = source
        .split_once("fn publish_with(")
        .expect("publication exists")
        .1;
    let ancestor = body
        .find("is_ancestor(")
        .expect("publish_with no longer checks base ancestry");
    let payload = body
        .find("review_payload(")
        .expect("publish_with no longer materialises the payload");
    assert!(
        ancestor < payload,
        "the base-ancestry refusal must be checked before `review_payload` materialises the \
         corrupted two-dot diff, or an oversize diff is measured against \
         `review-payload-too-large` before it is refused"
    );
}

fn context(root: &std::path::Path) -> Context {
    Context {
        skill_dir: root.to_path_buf(),
        repo_dir: root.to_path_buf(),
        config: Vec::new(),
        repo: None,
    }
}

#[cfg(unix)]
#[test]
fn a_changed_file_that_cannot_be_hashed_is_refused_and_not_reported_as_deleted() {
    use std::os::unix::fs::PermissionsExt;
    let (root, base, _) = two_commit_repo("head\n");
    let at = root.path();
    std::fs::write(at.join("keep.txt"), "changed and unreadable\n").expect("a change");
    std::fs::set_permissions(at.join("keep.txt"), std::fs::Permissions::from_mode(0o000))
        .expect("chmod 000");
    let answer = expected_target(&context(at), &base, None, None, true);
    std::fs::set_permissions(at.join("keep.txt"), std::fs::Permissions::from_mode(0o644))
        .expect("chmod back");
    match answer {
        Err(Failure::Read(detail)) => assert!(detail.contains("keep.txt"), "{detail}"),
        Err(other) => panic!("a refusal of the wrong class: {}", other.detail()),
        Ok(answer) => panic!("an unhashable change was dropped from the target: {answer}"),
    }
}

/// The narrow mode is the same target with one key removed, not a second
/// derivation: the digest is computed over the manifest before either answer
/// is rendered, so a caller cannot tell the two modes apart by any field but
/// `manifest` itself.
#[test]
fn a_narrow_expected_target_omits_the_manifest_but_matches_the_full_answer_otherwise() {
    let (root, base, _head) = two_commit_repo("changed\n");
    let context = context(root.path());

    let full = expected_target(&context, &base, None, None, true).expect("a full target");
    let narrow = expected_target(&context, &base, None, None, false).expect("a narrow target");

    assert!(
        full.get("manifest")
            .is_some_and(serde_json::Value::is_array),
        "the default answer stopped carrying a manifest: {full}"
    );
    assert!(
        narrow.get("manifest").is_none(),
        "the narrow answer still carries a manifest: {narrow}"
    );
    assert_eq!(
        full["digest"], narrow["digest"],
        "omitting the manifest changed the digest computed over it"
    );
    for field in ["ok", "base", "head", "paths", "uncommitted"] {
        assert_eq!(
            full[field], narrow[field],
            "the narrow mode disagreed with the full one on {field:?}"
        );
    }
}

/// `--native-start` compares the internal target, not the rendered answer, so
/// the narrow mode must refuse a mismatch exactly as the full one does — same
/// reason, same digests, same path lists.
#[test]
fn review_target_mismatch_still_fires_over_the_narrow_modes_inputs() {
    let (root, base, _head) = two_commit_repo("changed\n");
    let context = context(root.path());

    let claimed_path = root.path().join("claimed.json");
    std::fs::write(&claimed_path, "[]").expect("a supplied manifest of nothing");

    let full = expected_target(&context, &base, None, Some(&claimed_path), true)
        .expect_err("an empty supplied manifest cannot match a real delivery");
    let narrow = expected_target(&context, &base, None, Some(&claimed_path), false)
        .expect_err("the narrow mode must refuse identically");

    assert_eq!(full.envelope()["reason"], "review-target-mismatch");
    assert_eq!(narrow.envelope()["reason"], "review-target-mismatch");
    assert_eq!(
        full.envelope()["unreviewed"],
        narrow.envelope()["unreviewed"],
        "the narrow mode compared a different internal target"
    );
    assert_eq!(
        full.envelope()["expected_digest"],
        narrow.envelope()["expected_digest"],
        "the narrow mode derived a different digest for the same tree"
    );
}

/// One header per path, whichever way that path changed — added, deleted,
/// mode-only or binary — and never a false count from a diff whose *content*
/// is itself a patch.
#[test]
fn changed_file_count_reads_one_header_per_path_and_not_diff_shaped_content() {
    assert_eq!(changed_file_count(""), 0, "no diff, no paths");

    // Every fixture below is one `\n`-escaped literal rather than backslash
    // line-continuation: continuation strips the *following* line's leading
    // whitespace, which would silently eat the one leading space this test
    // exists to prove `changed_file_count` sees.
    let two_text_files = "diff --git a/one.txt b/one.txt\nindex aaa..bbb 100644\n--- a/one.txt\n+++ b/one.txt\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/two.txt b/two.txt\nindex ccc..ddd 100644\n--- a/two.txt\n+++ b/two.txt\n@@ -1 +1 @@\n-old\n+new\n";
    assert_eq!(changed_file_count(two_text_files), 2);

    // Mode-only and binary changes carry a header with no hunk beneath it —
    // undercounting these is exactly what counting `@@` hunks instead would do.
    let mode_and_binary = "diff --git a/run.sh b/run.sh\nold mode 100644\nnew mode 100755\ndiff --git a/pic.png b/pic.png\nindex eee..fff 100644\nBinary files a/pic.png and b/pic.png differ\n";
    assert_eq!(changed_file_count(mode_and_binary), 2);

    // A diff whose changed *content* is itself a patch must not inflate the
    // count: the content line below is a **context** line, prefixed with the
    // single leading space unified diff gives every unchanged line, so it
    // never starts at column zero the way a real header does.
    let diff_of_a_diff = "diff --git a/patch.diff b/patch.diff\nindex 111..222 100644\n--- a/patch.diff\n+++ b/patch.diff\n@@ -1,2 +1,2 @@\n diff --git a/x b/x\n-index 111..222 100644\n+index 333..444 100644\n";
    assert_eq!(
        changed_file_count(diff_of_a_diff),
        1,
        "a content line starting with the header's own words must not count as a second path"
    );
}

#[test]
fn merge_base_is_the_fork_point_until_the_branch_merges_it() {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    git(at, &["git", "init", "--quiet", "-b", "main", "."]);
    git(
        at,
        &["git", "config", "user.email", "estigia@example.invalid"],
    );
    git(at, &["git", "config", "user.name", "Estigia test"]);
    std::fs::write(at.join("root.txt"), "root\n").expect("the root file");
    git(at, &["git", "add", "root.txt"]);
    git(at, &["git", "commit", "--quiet", "-m", "root"]);
    let fork_point = oid(at);

    git(at, &["git", "checkout", "--quiet", "-b", "branch"]);
    std::fs::write(at.join("feature.txt"), "feature\n").expect("the branch file");
    git(at, &["git", "add", "feature.txt"]);
    git(at, &["git", "commit", "--quiet", "-m", "feature"]);
    let branch_head = oid(at);

    git(at, &["git", "checkout", "--quiet", "main"]);
    std::fs::write(at.join("advance.txt"), "advance\n").expect("main's own advance");
    git(at, &["git", "add", "advance.txt"]);
    git(at, &["git", "commit", "--quiet", "-m", "advance"]);
    let main_tip = oid(at);

    assert_eq!(
        merge_base(at, &main_tip, &branch_head).expect("a merge base"),
        fork_point,
        "an unmerged advance moved the merge base off the branch's own fork point"
    );

    git(at, &["git", "checkout", "--quiet", "branch"]);
    git(
        at,
        &[
            "git",
            "-c",
            "user.email=estigia@example.invalid",
            "-c",
            "user.name=Estigia test",
            "merge",
            "--quiet",
            "--no-edit",
            "main",
        ],
    );
    let merged_head = oid(at);
    assert_eq!(
        merge_base(at, &main_tip, &merged_head).expect("a merge base"),
        main_tip,
        "main's own tip is an ancestor of the branch once merged, and must be its own merge base"
    );
}

#[test]
fn is_ancestor_answers_true_false_and_refuses_an_unresolvable_ref() {
    let (root, base, head) = two_commit_repo("changed\n");
    let at = root.path();
    assert_eq!(is_ancestor(at, &base, &head), Ok(true));
    assert_eq!(is_ancestor(at, &head, &base), Ok(false));
    assert!(
        is_ancestor(at, "not-a-real-rev", &head).is_err(),
        "an unresolvable ref must refuse rather than silently answer false"
    );
}

const BULK_ROWS: usize = 7000;

fn bulk(rows: usize) -> String {
    (0..rows)
        .map(|row| format!("line {row:05} padding padding padding padding\n"))
        .collect()
}

fn regenerate_bulk(path: &str) -> String {
    format!(
        "[[generated]]\npath = \"{path}\"\ncommand = [\"sh\", \"-c\", \"i=0; while [ $i -lt {BULK_ROWS} ]; do printf 'line %05d padding padding padding padding\\\\n' $i; i=$((i+1)); done > {path}\"]\n"
    )
}

fn commit_all(at: &std::path::Path, files: &[(&str, Option<&str>)], message: &str) -> String {
    for (name, content) in files {
        match content {
            Some(content) => std::fs::write(at.join(name), content).expect("content"),
            None => std::fs::remove_file(at.join(name)).expect("removal"),
        }
    }
    git(at, &["git", "add", "-A"]);
    git(at, &["git", "commit", "--quiet", "-m", message]);
    oid(at)
}

fn generated_repo() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    git(at, &["git", "init", "--quiet", "-b", "main", "."]);
    git(
        at,
        &["git", "config", "user.email", "estigia@example.invalid"],
    );
    git(at, &["git", "config", "user.name", "Estigia test"]);
    git(at, &["git", "config", "core.autocrlf", "false"]);
    root
}

fn manifest_lines(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter_map(|line| line.strip_prefix("# estigia-generated "))
        .map(|json| serde_json::from_str(json).expect("a manifest line carries JSON"))
        .collect()
}

#[test]
fn a_declared_file_too_large_to_read_is_reviewed_under_the_ceiling_by_manifest() {
    let declared = generated_repo();
    let at = declared.path();
    let base = commit_all(
        at,
        &[
            ("estigia.generated.toml", Some(&regenerate_bulk("lock.txt"))),
            ("lock.txt", Some("old\n")),
        ],
        "base",
    );
    let head = commit_all(at, &[("lock.txt", Some(&bulk(BULK_ROWS)))], "head");

    let payload = review_payload(at, &base, &head).expect("a payload under the ceiling");

    assert!(
        payload.text.len() < REVIEW_PAYLOAD_CEILING,
        "{}",
        payload.text.len()
    );
    assert!(payload.text.contains("diff --git a/lock.txt b/lock.txt\n"));
    assert!(!payload.text.contains("+line 00100"), "the hunks were kept");
    let lines = manifest_lines(&payload.text);
    assert_eq!(lines.len(), 1, "{}", payload.text);
    assert_eq!(lines[0]["path"], "lock.txt");
    assert_eq!(lines[0]["head-bytes"], bulk(BULK_ROWS).len());
    assert_eq!(lines[0]["cwd"], serde_json::Value::Null);
    assert_eq!(lines[0]["command"][0], "sh");
    assert_eq!(
        lines[0]["base-blob"].as_str().map(str::len),
        lines[0]["head-blob"].as_str().map(str::len)
    );
}

#[test]
fn the_same_change_without_a_base_declaration_is_refused_as_too_large() {
    let undeclared = generated_repo();
    let at = undeclared.path();
    let base = commit_all(at, &[("lock.txt", Some("old\n"))], "base");
    let head = commit_all(
        at,
        &[
            ("estigia.generated.toml", Some(&regenerate_bulk("lock.txt"))),
            ("lock.txt", Some(&bulk(BULK_ROWS))),
        ],
        "head",
    );

    let failure = review_payload(at, &base, &head).expect_err("a payload over the ceiling");

    assert_eq!(failure.envelope()["reason"], "review-payload-too-large");
}

#[test]
fn a_declaration_added_by_the_change_gives_hunks_and_no_manifest() {
    let undeclared = generated_repo();
    let at = undeclared.path();
    let base = commit_all(at, &[("lock.txt", Some("old\n"))], "base");
    let head = commit_all(
        at,
        &[
            ("estigia.generated.toml", Some(&regenerate_bulk("lock.txt"))),
            ("lock.txt", Some("new\n")),
        ],
        "head",
    );

    let payload = review_payload(at, &base, &head).expect("a payload");

    assert!(manifest_lines(&payload.text).is_empty());
    assert!(payload.text.contains("+new\n"), "{}", payload.text);
}

#[test]
fn a_newly_added_declared_file_is_a_manifest_with_an_empty_base_blob() {
    let declared = generated_repo();
    let at = declared.path();
    let base = commit_all(
        at,
        &[("estigia.generated.toml", Some(&regenerate_bulk("lock.txt")))],
        "base",
    );
    let head = commit_all(at, &[("lock.txt", Some(&bulk(BULK_ROWS)))], "head");

    let payload = review_payload(at, &base, &head).expect("a payload");

    let lines = manifest_lines(&payload.text);
    assert_eq!(lines.len(), 1, "{}", payload.text);
    assert!(
        lines[0]["base-blob"]
            .as_str()
            .is_some_and(|blob| blob.chars().all(|c| c == '0')),
        "{}",
        lines[0]
    );
    assert!(!payload.text.contains("new file mode"), "{}", payload.text);
}

#[test]
fn substituting_a_manifest_does_not_change_the_changed_file_count() {
    let declared = generated_repo();
    let at = declared.path();
    let base = commit_all(
        at,
        &[
            ("estigia.generated.toml", Some(&regenerate_bulk("lock.txt"))),
            ("lock.txt", Some("old\n")),
            ("README.md", Some("one\n")),
        ],
        "base",
    );
    let head = commit_all(
        at,
        &[
            ("lock.txt", Some(&bulk(BULK_ROWS))),
            ("README.md", Some("two\n")),
        ],
        "head",
    );

    let payload = review_payload(at, &base, &head).expect("a payload");

    assert_eq!(manifest_lines(&payload.text).len(), 1);
    assert_eq!(changed_file_count(&payload.text), 2, "{}", payload.text);
    assert!(payload.text.contains("+two\n"));
}

#[test]
fn a_deleted_declared_file_keeps_its_hunks() {
    let declared = generated_repo();
    let at = declared.path();
    let base = commit_all(
        at,
        &[
            ("estigia.generated.toml", Some(&regenerate_bulk("lock.txt"))),
            ("lock.txt", Some("old\n")),
        ],
        "base",
    );
    let head = commit_all(at, &[("lock.txt", None)], "head");

    let payload = review_payload(at, &base, &head).expect("a payload");

    assert!(manifest_lines(&payload.text).is_empty());
    assert!(
        payload.text.contains("deleted file mode"),
        "{}",
        payload.text
    );
}

#[test]
fn a_malformed_base_declaration_refuses_the_payload() {
    let broken = generated_repo();
    let at = broken.path();
    let base = commit_all(
        at,
        &[
            ("estigia.generated.toml", Some("not [toml")),
            ("lock.txt", Some("old\n")),
        ],
        "base",
    );
    let head = commit_all(at, &[("lock.txt", Some("new\n"))], "head");

    let failure = review_payload(at, &base, &head).expect_err("a refusal");

    assert_eq!(
        failure.envelope()["reason"],
        "generated-declaration-malformed"
    );
}

#[test]
fn a_delta_payload_trusts_only_the_declaration_the_receipt_base_holds() {
    let declared = generated_repo();
    let at = declared.path();
    let base = commit_all(at, &[("lock.txt", Some("old\n"))], "base");
    let parent = commit_all(
        at,
        &[
            ("estigia.generated.toml", Some(&regenerate_bulk("lock.txt"))),
            ("lock.txt", Some("mid\n")),
        ],
        "parent",
    );
    let head = commit_all(at, &[("lock.txt", Some(&bulk(BULK_ROWS)))], "head");

    let by_the_base = review_payload_declared_at(at, &parent, &head, &base);
    let by_the_parent = review_payload_declared_at(at, &parent, &head, &parent);

    assert_eq!(
        by_the_base.expect_err("hunks past the ceiling").envelope()["reason"],
        "review-payload-too-large"
    );
    assert_eq!(
        manifest_lines(&by_the_parent.expect("a manifest").text).len(),
        1
    );
}

#[test]
fn two_payloads_differing_only_in_a_declared_head_blob_have_different_digests() {
    let declared = generated_repo();
    let at = declared.path();
    let base = commit_all(
        at,
        &[
            ("estigia.generated.toml", Some(&regenerate_bulk("lock.txt"))),
            ("lock.txt", Some("old\n")),
        ],
        "base",
    );
    let first_head = commit_all(at, &[("lock.txt", Some(&bulk(BULK_ROWS)))], "first");
    git(at, &["git", "reset", "--quiet", "--hard", &base]);
    let second_head = commit_all(
        at,
        &[(
            "lock.txt",
            Some(&bulk(BULK_ROWS).replacen("line", "lone", 1)),
        )],
        "second",
    );

    let first = review_payload(at, &base, &first_head).expect("a payload");
    let second = review_payload(at, &base, &second_head).expect("a payload");

    assert_eq!(first.text.len(), second.text.len());
    assert_ne!(first_head, second_head);
    assert_ne!(first.text, second.text);
    assert_ne!(first.digest, second.digest);
}

fn dispatched_target(context: &Context, flags: &[&str]) -> serde_json::Value {
    let flags: Vec<String> = flags.iter().map(|flag| (*flag).to_owned()).collect();
    crate::transport::dispatch::dispatch(context, "target", &flags, "2026-10-13T00:00:00Z")
        .expect("a target")
}

#[test]
fn the_target_operation_answers_the_brief_form_unless_the_manifest_is_asked_for() {
    let (root, base, _head) = two_commit_repo("changed\n");
    let context = context(root.path());

    let brief = dispatched_target(&context, &["--base", &base]);
    assert!(
        brief.get("manifest").is_none(),
        "the default answer carries the manifest: {brief}"
    );

    let full = dispatched_target(&context, &["--base", &base, "--manifest"]);
    assert!(
        full.get("manifest")
            .is_some_and(serde_json::Value::is_array),
        "asking for the manifest did not return it: {full}"
    );
    assert_eq!(brief["digest"], full["digest"]);
}
