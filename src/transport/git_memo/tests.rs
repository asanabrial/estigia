use std::path::Path;

use super::*;
use crate::transport::{How, isolated_diff_env, run, run_with_isolated_env};

fn git(at: &Path, args: &[&str]) -> String {
    let mut full = vec!["git"];
    full.extend_from_slice(args);
    run(&full, Some(at), How::write())
        .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"))
        .stdout
        .trim()
        .to_owned()
}

fn two_commits() -> (tempfile::TempDir, String, String) {
    let root = crate::test_env::repository_with_commit();
    let base = git(root.path(), &["rev-parse", "HEAD"]);
    git(
        root.path(),
        &["commit", "--allow-empty", "--quiet", "-m", "head"],
    );
    let head = git(root.path(), &["rev-parse", "HEAD"]);
    (root, base, head)
}

fn spawns_of(needle: &str) -> usize {
    spawned()
        .iter()
        .filter(|args| args.iter().any(|arg| arg.contains(needle)))
        .count()
}

fn verify(at: &Path, spec: &str) -> Result<crate::transport::Output, crate::transport::Failure> {
    run(
        &["git", "rev-parse", "--verify", spec],
        Some(at),
        How::read(),
    )
}

#[test]
fn inside_a_scope_a_repeated_fixed_question_is_asked_once() {
    let (root, base, _) = two_commits();
    let spec = format!("{base}^{{commit}}");
    forget_spawns();

    let _scope = scope();
    let first = verify(root.path(), &spec).expect("first");
    let second = verify(root.path(), &spec).expect("second");

    assert_eq!(first.stdout, second.stdout);
    assert_eq!(spawns_of(&spec), 1);
}

#[test]
fn outside_a_scope_nothing_is_memoised() {
    let (root, base, _) = two_commits();
    let spec = format!("{base}^{{commit}}");
    forget_spawns();

    verify(root.path(), &spec).expect("first");
    verify(root.path(), &spec).expect("second");

    assert_eq!(spawns_of(&spec), 2);
}

#[test]
fn a_scope_that_closed_leaves_nothing_behind() {
    let (root, base, _) = two_commits();
    let spec = format!("{base}^{{commit}}");
    forget_spawns();

    {
        let _outer = scope();
        {
            let _inner = scope();
            verify(root.path(), &spec).expect("inner");
        }
        verify(root.path(), &spec).expect("outer still open");
    }
    verify(root.path(), &spec).expect("closed");

    assert_eq!(spawns_of(&spec), 2);
}

#[test]
fn head_stays_live_inside_a_scope_after_a_commit() {
    let (root, _, head) = two_commits();
    forget_spawns();

    let _scope = scope();
    let before = verify(root.path(), "HEAD^{commit}").expect("before");
    git(
        root.path(),
        &["commit", "--allow-empty", "--quiet", "-m", "later"],
    );
    let after = verify(root.path(), "HEAD^{commit}").expect("after");

    assert_eq!(before.stdout.trim(), head);
    assert_ne!(after.stdout, before.stdout);
    assert_eq!(spawns_of("HEAD^{commit}"), 2);
}

#[test]
fn a_failure_is_never_memoised() {
    let (root, _, _) = two_commits();
    let missing = format!("{}^{{commit}}", "0".repeat(40));
    forget_spawns();

    let _scope = scope();
    assert!(verify(root.path(), &missing).is_err());
    assert!(verify(root.path(), &missing).is_err());

    assert_eq!(spawns_of(&missing), 2);
}

#[test]
fn two_isolated_environments_over_one_repository_share_an_answer() {
    let (root, base, head) = two_commits();
    forget_spawns();

    let _scope = scope();
    let mut answers = Vec::new();
    for _ in 0..2 {
        let (_guard, envs) = isolated_diff_env(root.path()).expect("an isolated environment");
        let envs: Vec<(&str, &str)> = envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        answers.push(
            run_with_isolated_env(
                &["git", "diff", "--no-color", "--name-only", &base, &head],
                Some(root.path()),
                How::read(),
                &envs,
            )
            .expect("a diff")
            .stdout,
        );
    }

    assert_eq!(answers[0], answers[1]);
    assert_eq!(spawns_of("--name-only"), 1);
    assert_eq!(spawns_of("--show-object-format"), 1);
}

#[test]
fn only_a_question_fixed_by_full_object_ids_is_memoisable() {
    let oid = "a".repeat(40);
    let other = "b".repeat(40);
    let yes: Vec<Vec<String>> = vec![
        vec![
            "git",
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
            "--show-object-format",
        ],
        vec!["git", "rev-parse", "--verify", &format!("{oid}^{{commit}}")],
        vec![
            "git",
            "ls-tree",
            "--name-only",
            &oid,
            "--",
            "estigia.generated.toml",
        ],
        vec![
            "git",
            "-c",
            "core.quotePath=false",
            "diff",
            "--numstat",
            "-z",
            &oid,
            &other,
            "--",
        ],
        vec![
            "git",
            "diff",
            "--unified=0",
            &oid,
            &other,
            "--",
            ":(literal)a.rs",
        ],
        vec![
            "git",
            "cat-file",
            "blob",
            &format!("{oid}:estigia.generated.toml"),
        ],
    ]
    .into_iter()
    .map(|args| args.into_iter().map(str::to_owned).collect())
    .collect();
    let no: Vec<Vec<String>> = vec![
        vec!["git", "rev-parse", "--verify", "HEAD^{commit}"],
        vec!["git", "rev-parse", "--verify", "main^{commit}"],
        vec!["git", "rev-parse", "HEAD"],
        vec!["git", "ls-tree", "-r", "-z", "--full-tree", "HEAD"],
        vec!["git", "diff", &oid],
        vec!["git", "diff", &oid, "HEAD"],
        vec!["git", "status", "--porcelain=v1"],
        vec!["git", "commit", "-m", "x"],
        vec!["git", "cat-file", "blob", "HEAD:a.rs"],
        vec!["git", "cat-file", "--batch"],
        vec!["gh", "api", "user"],
    ]
    .into_iter()
    .map(|args| args.into_iter().map(str::to_owned).collect())
    .collect();

    for args in yes {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        assert!(memoisable(&refs), "{args:?}");
    }
    for args in no {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        assert!(!memoisable(&refs), "{args:?}");
    }
}
