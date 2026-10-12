use super::*;

struct Correction {
    repo: tempfile::TempDir,
    parent: String,
    head: String,
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

fn correction() -> Correction {
    let repo = crate::test_env::repository_with_commit();
    let at = repo.path();
    std::fs::write(at.join("a.txt"), "one\ntwo\n").expect("write a");
    std::fs::write(at.join("keep.txt"), "untouched\n").expect("write keep");
    git(at, &["add", "."]);
    git(at, &["commit", "--quiet", "-m", "parent"]);
    let parent = git(at, &["rev-parse", "HEAD"]);
    std::fs::write(at.join("a.txt"), "one\nthree\n").expect("edit a");
    std::fs::write(at.join("b.txt"), "new\n").expect("write b");
    git(at, &["add", "."]);
    git(at, &["commit", "--quiet", "-m", "correction"]);
    let head = git(at, &["rev-parse", "HEAD"]);
    Correction { repo, parent, head }
}

fn ask(fixture: &Correction, request: Inspect) -> Result<String, Refusal> {
    inspect(
        fixture.repo.path(),
        &InspectReceipt {
            parent_head: &fixture.parent,
            head: &fixture.head,
        },
        &request,
    )
}

#[test]
fn the_summaries_name_only_what_the_correction_changed() {
    let fixture = correction();
    let status = ask(&fixture, Inspect::NameStatus).expect("name-status");
    assert_eq!(status, "M\ta.txt\nA\tb.txt\n");
    let numstat = ask(&fixture, Inspect::Numstat).expect("numstat");
    assert_eq!(numstat, "1\t1\ta.txt\n1\t0\tb.txt\n");
    let stat = ask(&fixture, Inspect::Stat).expect("stat");
    assert!(stat.contains("a.txt") && stat.contains("b.txt") && !stat.contains("keep.txt"));
}

#[test]
fn a_patch_is_the_whole_correction_or_one_of_its_paths() {
    let fixture = correction();
    let all = ask(&fixture, Inspect::Patch { path: None }).expect("the patch");
    assert!(all.contains("-two\n+three\n") && all.contains("+new\n"));
    let one = ask(
        &fixture,
        Inspect::Patch {
            path: Some("b.txt".to_owned()),
        },
    )
    .expect("one path");
    assert!(one.contains("+new\n") && !one.contains("three"));
}

#[test]
fn an_object_is_read_from_the_side_asked_for() {
    let fixture = correction();
    let read = |side| {
        ask(
            &fixture,
            Inspect::Object {
                path: "a.txt".to_owned(),
                side,
            },
        )
    };
    assert_eq!(read(Side::Parent).expect("parent"), "one\ntwo\n");
    assert_eq!(read(Side::Candidate).expect("candidate"), "one\nthree\n");
    let absent = ask(
        &fixture,
        Inspect::Object {
            path: "b.txt".to_owned(),
            side: Side::Parent,
        },
    )
    .expect_err("b.txt did not exist before the correction");
    assert_eq!(absent.code, "reviewer-inspect-absent-side");
}

#[test]
fn a_changed_path_absent_on_a_side_is_told_apart_from_an_unreadable_one() {
    let fixture = correction();
    std::fs::remove_file(fixture.repo.path().join("a.txt")).expect("delete a");
    git(fixture.repo.path(), &["add", "."]);
    git(fixture.repo.path(), &["commit", "--quiet", "-m", "delete"]);
    let head = git(fixture.repo.path(), &["rev-parse", "HEAD"]);
    let gone = inspect(
        fixture.repo.path(),
        &InspectReceipt {
            parent_head: &fixture.head,
            head: &head,
        },
        &Inspect::Object {
            path: "a.txt".to_owned(),
            side: Side::Candidate,
        },
    )
    .expect_err("deleted on the candidate side");
    assert_eq!(gone.code, "reviewer-inspect-absent-side");
    let added = ask(
        &fixture,
        Inspect::Object {
            path: "b.txt".to_owned(),
            side: Side::Parent,
        },
    )
    .expect_err("added by the correction");
    assert_eq!(added.code, "reviewer-inspect-absent-side");
}

#[test]
fn a_path_the_correction_did_not_change_is_refused() {
    let fixture = correction();
    for request in [
        Inspect::Patch {
            path: Some("keep.txt".to_owned()),
        },
        Inspect::Object {
            path: "keep.txt".to_owned(),
            side: Side::Candidate,
        },
        Inspect::Object {
            path: "../outside".to_owned(),
            side: Side::Candidate,
        },
        Inspect::Patch {
            path: Some("*.txt".to_owned()),
        },
    ] {
        let refusal = ask(&fixture, request.clone()).expect_err("outside the correction");
        assert_eq!(
            refusal.code, "reviewer-inspect-outside-correction",
            "{request:?}"
        );
    }
}

#[test]
fn a_receipt_that_does_not_name_its_commits_is_refused() {
    let fixture = correction();
    let at = fixture.repo.path();
    let short = &fixture.head[..12];
    let missing = "0".repeat(40);
    for (parent, head) in [
        (fixture.parent.as_str(), short),
        (short, fixture.head.as_str()),
        (fixture.parent.as_str(), missing.as_str()),
        ("HEAD", fixture.head.as_str()),
        (fixture.parent.as_str(), "--all"),
    ] {
        let refusal = inspect(
            at,
            &InspectReceipt {
                parent_head: parent,
                head,
            },
            &Inspect::NameStatus,
        )
        .expect_err("a receipt that does not resolve");
        assert_eq!(
            refusal.code, "reviewer-inspect-transaction-unresolved",
            "{parent} {head}"
        );
    }
}

#[test]
fn a_tree_standing_for_the_parent_side_is_read_like_a_commit() {
    let fixture = correction();
    let tree = git(
        fixture.repo.path(),
        &["rev-parse", &format!("{}^{{tree}}", fixture.parent)],
    );

    let status = inspect(
        fixture.repo.path(),
        &InspectReceipt {
            parent_head: &tree,
            head: &fixture.head,
        },
        &Inspect::NameStatus,
    )
    .expect("a tree names the parent side");
    let object = inspect(
        fixture.repo.path(),
        &InspectReceipt {
            parent_head: &tree,
            head: &fixture.head,
        },
        &Inspect::Object {
            path: "a.txt".to_owned(),
            side: Side::Parent,
        },
    )
    .expect("the parent side of a changed path");

    assert_eq!(status, "M\ta.txt\nA\tb.txt\n");
    assert_eq!(object, "one\ntwo\n");
}

#[test]
fn an_inspection_past_the_ceiling_is_refused_not_truncated() {
    let fixture = correction();
    let big = "x".repeat(INSPECT_CEILING + 1);
    std::fs::write(fixture.repo.path().join("big.txt"), &big).expect("write big");
    git(fixture.repo.path(), &["add", "."]);
    git(fixture.repo.path(), &["commit", "--quiet", "-m", "big"]);
    let head = git(fixture.repo.path(), &["rev-parse", "HEAD"]);
    let refusal = inspect(
        fixture.repo.path(),
        &InspectReceipt {
            parent_head: &fixture.head,
            head: &head,
        },
        &Inspect::Object {
            path: "big.txt".to_owned(),
            side: Side::Candidate,
        },
    )
    .expect_err("over the ceiling");
    assert_eq!(refusal.code, "reviewer-input-too-large");
}
