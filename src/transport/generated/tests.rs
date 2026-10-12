use std::path::{Path, PathBuf};

use super::*;
use crate::transport::{How, run};

fn git(at: &Path, arguments: &[&str]) -> String {
    let mut command = vec!["git"];
    command.extend_from_slice(arguments);
    run(&command, Some(at), How::write())
        .unwrap_or_else(|failure| panic!("git {arguments:?} failed: {}", failure.detail()))
        .stdout
}

fn reason(failure: Failure) -> String {
    match failure {
        Failure::Stop(payload) => payload["reason"].as_str().unwrap_or_default().to_owned(),
        other => panic!("expected a named refusal, got {}", other.detail()),
    }
}

struct Fixture {
    _root: tempfile::TempDir,
    repo: PathBuf,
    base: String,
    head: String,
}

fn declaration(command: &str) -> String {
    format!(
        "[[generated]]\npath = \"sub/lock.txt\"\ncwd = \"sub\"\ncommand = [\"sh\", \"-c\", \"{command}\"]\n"
    )
}

const REGENERATE: &str = "printf 'locked\\\\n' > lock.txt";

fn commit(repo: &Path, files: &[(&str, Option<&str>)], message: &str) -> String {
    for (name, content) in files {
        let target = repo.join(name);
        match content {
            Some(content) => {
                std::fs::create_dir_all(target.parent().expect("a parent")).expect("directories");
                std::fs::write(&target, content).expect("content");
            }
            None => std::fs::remove_file(&target).expect("removal"),
        }
    }
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "--quiet", "-m", message]);
    git(repo, &["rev-parse", "HEAD"]).trim().to_owned()
}

fn fixture(base: &[(&str, Option<&str>)], head: &[(&str, Option<&str>)]) -> Fixture {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the repository directory");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    let base = commit(&repo, base, "base");
    let head = commit(&repo, head, "head");
    Fixture {
        _root: root,
        repo,
        base,
        head,
    }
}

fn declared_base(command: &str) -> Vec<(&'static str, Option<String>)> {
    vec![
        ("estigia.generated.toml", Some(declaration(command))),
        ("sub/lock.txt", Some("old\n".to_owned())),
    ]
}

fn borrowed<'a>(
    files: &'a [(&'static str, Option<String>)],
) -> Vec<(&'static str, Option<&'a str>)> {
    files
        .iter()
        .map(|(name, content)| (*name, content.as_deref()))
        .collect()
}

#[test]
fn a_regenerated_file_with_the_head_bytes_matches() {
    let base = declared_base(REGENERATE);
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("locked\n"))]);

    let outcomes = reproduce(&world.repo, &world.base, &world.head).expect("a reproduction");

    assert_eq!(
        outcomes,
        vec![PathOutcome {
            path: "sub/lock.txt".to_owned(),
            outcome: Outcome::Match
        }]
    );
}

#[test]
fn a_hand_edited_file_is_a_mismatch_not_a_match() {
    let base = declared_base(REGENERATE);
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("hand edited\n"))]);

    let outcomes = reproduce(&world.repo, &world.base, &world.head).expect("a reproduction");

    assert_eq!(outcomes[0].outcome, Outcome::Mismatch);
}

#[test]
fn a_command_that_exits_non_zero_is_unavailable_not_a_pass() {
    let base = declared_base("exit 3");
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("locked\n"))]);

    let outcomes = reproduce(&world.repo, &world.base, &world.head).expect("a reproduction");

    assert!(
        matches!(outcomes[0].outcome, Outcome::Unavailable(_)),
        "{outcomes:?}"
    );
}

#[test]
fn a_command_that_cannot_start_is_unavailable() {
    let base = declared_base(REGENERATE)
        .into_iter()
        .map(|(name, content)| {
            (
                name,
                content.map(|text| text.replace("\"sh\"", "\"estigia-no-such-program\"")),
            )
        });
    let base: Vec<_> = base.collect();
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("locked\n"))]);

    let outcomes = reproduce(&world.repo, &world.base, &world.head).expect("a reproduction");

    assert!(
        matches!(outcomes[0].outcome, Outcome::Unavailable(_)),
        "{outcomes:?}"
    );
}

#[test]
fn a_declaration_present_only_in_the_head_declares_nothing() {
    let world = fixture(
        &[("sub/lock.txt", Some("old\n"))],
        &[
            ("estigia.generated.toml", Some(&declaration(REGENERATE))),
            ("sub/lock.txt", Some("locked\n")),
        ],
    );

    assert!(
        declared_at(&world.repo, &world.base)
            .expect("a read")
            .is_empty()
    );
    assert!(
        reproduce(&world.repo, &world.base, &world.head)
            .expect("a reproduction")
            .is_empty()
    );
}

#[test]
fn a_declared_path_the_change_does_not_touch_is_not_reproduced() {
    let base = declared_base("exit 3");
    let world = fixture(&borrowed(&base), &[("README.md", Some("prose\n"))]);

    assert!(
        reproduce(&world.repo, &world.base, &world.head)
            .expect("a reproduction")
            .is_empty()
    );
}

#[test]
fn a_deleted_declared_path_falls_back_to_hunks() {
    let base = declared_base(REGENERATE);
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", None)]);

    let outcomes = reproduce(&world.repo, &world.base, &world.head).expect("a reproduction");

    assert!(
        matches!(outcomes[0].outcome, Outcome::FallsBackToHunks(_)),
        "{outcomes:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_mode_change_of_a_declared_path_falls_back_to_hunks() {
    use std::os::unix::fs::PermissionsExt;

    let base = declared_base(REGENERATE);
    let world = fixture(&borrowed(&base), &[("README.md", Some("prose\n"))]);
    let lock = world.repo.join("sub/lock.txt");
    std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o755)).expect("a mode");
    git(&world.repo, &["add", "-A"]);
    git(&world.repo, &["commit", "--quiet", "-m", "mode"]);
    let head = git(&world.repo, &["rev-parse", "HEAD"]).trim().to_owned();

    let outcomes = reproduce(&world.repo, &world.base, &head).expect("a reproduction");

    assert!(
        matches!(outcomes[0].outcome, Outcome::FallsBackToHunks(_)),
        "{outcomes:?}"
    );
}

#[test]
fn a_malformed_declaration_is_a_named_refusal() {
    let world = fixture(
        &[("estigia.generated.toml", Some("[[generated]\npath = "))],
        &[("README.md", Some("prose\n"))],
    );

    let failure = declared_at(&world.repo, &world.base).expect_err("a refusal");

    assert_eq!(reason(failure), "generated-declaration-malformed");
}

#[test]
fn a_declaration_that_cannot_run_is_refused_when_parsed() {
    let cases = [
        "[[generated]]\npath = \"a\"\ncommand = []\n",
        "[[generated]]\npath = \"a\"\ncommand = [\"\"]\n",
        "[[generated]]\npath = \"/etc/passwd\"\ncommand = [\"true\"]\n",
        "[[generated]]\npath = \"../a\"\ncommand = [\"true\"]\n",
        "[[generated]]\npath = \"a\"\ncwd = \"../x\"\ncommand = [\"true\"]\n",
        "[[generated]]\npath = \"a\"\ncommand = \"npm install\"\n",
        "[[generated]]\npath = \"a\"\ncommand = [\"true\"]\nextra = 1\n",
        "[[generated]]\npath = \"a\\nb\"\ncommand = [\"true\"]\n",
        "[[generated]]\ncommand = [\"true\"]\n",
        "[[generated]]\npath = \"a\"\ncommand = [\"true\"]\n[[generated]]\npath = \"a\"\ncommand = [\"true\"]\n",
    ];
    for case in cases {
        let failure = parse(case).expect_err(case);
        assert_eq!(reason(failure), "generated-declaration-malformed", "{case}");
    }
}

#[test]
fn a_well_formed_declaration_is_parsed() {
    let parsed = parse(&declaration(REGENERATE)).expect("a declaration");

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].path, "sub/lock.txt");
    assert_eq!(parsed[0].cwd.as_deref(), Some("sub"));
    assert_eq!(parsed[0].command[0], "sh");
}

#[test]
fn reproduction_leaves_the_callers_worktree_alone() {
    let command = "printf 'locked\\\\n' > lock.txt; printf stray > ../stray.txt";
    let base = declared_base(command);
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("locked\n"))]);
    let before = git(&world.repo, &["status", "--porcelain", "--ignored"]);
    let listing = |repo: &Path| {
        let mut names: Vec<_> = walk(repo);
        names.sort();
        names
    };
    let files_before = listing(&world.repo);

    reproduce(&world.repo, &world.base, &world.head).expect("a reproduction");

    assert_eq!(
        before,
        git(&world.repo, &["status", "--porcelain", "--ignored"])
    );
    assert_eq!(files_before, listing(&world.repo));
    assert!(!world.repo.join("sub/stray.txt").exists());
    assert!(!world.repo.join("stray.txt").exists());
}

fn walk(directory: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(directory).expect("a directory") {
        let path = entry.expect("an entry").path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push((path.clone(), std::fs::read(&path).expect("content")));
        }
    }
    found
}

#[test]
fn the_report_carries_each_outcome_with_its_code() {
    let base = declared_base(REGENERATE);
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("hand edited\n"))]);

    let report = report(&world.repo, &world.base, &world.head, true);

    assert_eq!(report["outcomes"][0]["path"], "sub/lock.txt");
    assert_eq!(report["outcomes"][0]["outcome"], "mismatch");
    assert_eq!(
        report["outcomes"][0]["reason"],
        "generated-reproduction-mismatch"
    );
}

#[test]
fn the_report_carries_a_malformed_declaration_without_failing() {
    let world = fixture(
        &[("estigia.generated.toml", Some("not [toml"))],
        &[("README.md", Some("prose\n"))],
    );

    let report = report(&world.repo, &world.base, &world.head, true);

    assert_eq!(
        report["refusal"]["reason"],
        "generated-declaration-malformed"
    );
    assert_eq!(report["outcomes"], serde_json::json!([]));
}

#[test]
fn a_command_that_writes_nothing_is_unavailable_never_a_match() {
    for command in ["true", ":"] {
        let base = declared_base(command);
        let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("anything\n"))]);

        let outcomes = reproduce(&world.repo, &world.base, &world.head).expect("a reproduction");

        assert!(
            matches!(outcomes[0].outcome, Outcome::Unavailable(_)),
            "{command}: {outcomes:?}"
        );
    }
}

#[test]
fn one_command_regenerating_two_declared_files_is_compared_on_each() {
    let both = "[[generated]]\npath = \"a.txt\"\ncommand = [\"sh\", \"-c\", \"printf 'a\\\\n' > a.txt; printf 'b\\\\n' > b.txt\"]\n\
                [[generated]]\npath = \"b.txt\"\ncommand = [\"true\"]\n";
    let world = fixture(
        &[
            ("estigia.generated.toml", Some(both)),
            ("a.txt", Some("old\n")),
            ("b.txt", Some("old\n")),
        ],
        &[("a.txt", Some("a\n")), ("b.txt", Some("b\n"))],
    );

    let outcomes = reproduce(&world.repo, &world.base, &world.head).expect("a reproduction");

    assert_eq!(outcomes[0].outcome, Outcome::Match, "{outcomes:?}");
    assert!(
        matches!(outcomes[1].outcome, Outcome::Unavailable(_)),
        "{outcomes:?}"
    );
}

fn marking_command(marker: &Path) -> String {
    format!("printf ran > {}; {REGENERATE}", marker.display())
}

#[test]
fn a_report_without_the_opt_in_runs_nothing_and_says_so() {
    let outside = tempfile::tempdir().expect("a directory outside the repository");
    let marker = outside.path().join("ran");
    let base = declared_base(&marking_command(&marker));
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("locked\n"))]);

    let report = report(&world.repo, &world.base, &world.head, false);

    assert_eq!(report["outcomes"][0]["path"], "sub/lock.txt");
    assert_eq!(report["outcomes"][0]["outcome"], "not-run");
    assert!(!marker.exists(), "a command ran without the opt-in");
}

#[test]
fn a_report_with_the_opt_in_runs_the_command() {
    let outside = tempfile::tempdir().expect("a directory outside the repository");
    let marker = outside.path().join("ran");
    let base = declared_base(&marking_command(&marker));
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("locked\n"))]);

    let report = report(&world.repo, &world.base, &world.head, true);

    assert_eq!(report["outcomes"][0]["outcome"], "match");
    assert!(marker.exists(), "the opt-in did not run the command");
}

#[test]
fn a_newly_added_declared_file_is_reproduced_like_any_other() {
    let declared = vec![("estigia.generated.toml", Some(declaration(REGENERATE)))];
    let matching = fixture(&borrowed(&declared), &[("sub/lock.txt", Some("locked\n"))]);
    let edited = fixture(&borrowed(&declared), &[("sub/lock.txt", Some("edited\n"))]);

    let matched = reproduce(&matching.repo, &matching.base, &matching.head).expect("a run");
    let mismatched = reproduce(&edited.repo, &edited.base, &edited.head).expect("a run");

    assert_eq!(matched[0].outcome, Outcome::Match, "{matched:?}");
    assert_eq!(mismatched[0].outcome, Outcome::Mismatch, "{mismatched:?}");
}

#[test]
fn a_declared_path_that_is_a_symlink_falls_back_to_hunks() {
    let base = declared_base(REGENERATE);
    let world = fixture(&borrowed(&base), &[("README.md", Some("prose\n"))]);
    let lock = world.repo.join("sub/lock.txt");
    std::fs::remove_file(&lock).expect("the file goes");
    #[cfg(unix)]
    std::os::unix::fs::symlink("../README.md", &lock).expect("a link");
    git(&world.repo, &["add", "-A"]);
    git(&world.repo, &["commit", "--quiet", "-m", "link"]);
    let head = git(&world.repo, &["rev-parse", "HEAD"]).trim().to_owned();

    let outcomes = reproduce(&world.repo, &world.base, &head).expect("a reproduction");

    assert!(
        matches!(outcomes[0].outcome, Outcome::FallsBackToHunks(_)),
        "{outcomes:?}"
    );
}

fn declaration_of_many(entries: &[(&str, &str)]) -> String {
    entries
        .iter()
        .map(|(path, command)| {
            format!(
                "[[generated]]\npath = \"{path}\"\ncommand = [\"sh\", \"-c\", \"{command}\"]\n\n"
            )
        })
        .collect()
}

#[test]
fn several_declared_paths_report_every_failure_and_a_mismatch_outranks_an_unavailable() {
    let declared = declaration_of_many(&[
        ("a.txt", "printf a > a.txt"),
        ("b.txt", "exit 3"),
        ("c.txt", "printf c > c.txt"),
        ("d.txt", "printf d > d.txt"),
    ]);
    let world = fixture(
        &[("estigia.generated.toml", Some(&declared))],
        &[
            ("a.txt", Some("hand edited")),
            ("b.txt", Some("b")),
            ("c.txt", Some("c")),
            ("d.txt", Some("tampered")),
        ],
    );

    let failure = require_reproduction(&world.repo, &world.base, &world.head)
        .expect_err("a mismatch was approved");

    let Failure::Stop(payload) = failure else {
        panic!("expected a named refusal");
    };
    assert_eq!(payload["reason"], "generated-reproduction-mismatch");
    let paths = payload["paths"].as_array().expect("a list of paths");
    let seen: Vec<(&str, &str)> = paths
        .iter()
        .map(|each| {
            (
                each["path"].as_str().expect("a path"),
                each["outcome"].as_str().expect("an outcome"),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("a.txt", "mismatch"),
            ("b.txt", "unavailable"),
            ("d.txt", "mismatch")
        ]
    );
}

#[test]
fn several_declared_paths_that_are_only_unavailable_are_refused_as_unavailable() {
    let declared = declaration_of_many(&[("b.txt", "exit 3"), ("c.txt", "printf c > c.txt")]);
    let world = fixture(
        &[("estigia.generated.toml", Some(&declared))],
        &[("b.txt", Some("b")), ("c.txt", Some("c"))],
    );

    let failure = require_reproduction(&world.repo, &world.base, &world.head)
        .expect_err("an unrun command was approved");

    let Failure::Stop(payload) = failure else {
        panic!("expected a named refusal");
    };
    assert_eq!(payload["reason"], "generated-reproduction-unavailable");
    assert_eq!(payload["paths"].as_array().map(Vec::len), Some(1));
    assert_eq!(payload["paths"][0]["path"], "b.txt");
}

fn scratch_left_behind(repo: &Path) -> Vec<String> {
    std::fs::read_dir(repo.join(".git"))
        .expect("the git directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.starts_with(".estigia-isolated-diff-"))
        .collect()
}

#[test]
fn a_command_past_the_deadline_is_unavailable_at_the_gate() {
    let base = declared_base("sleep 30; printf 'locked\\\\n' > lock.txt");
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("locked\n"))]);

    let started = std::time::Instant::now();
    let failure = require_reproduction_within(
        &world.repo,
        &world.base,
        &world.head,
        Duration::from_millis(300),
    )
    .expect_err("a command that never finished was read as a match");

    let Failure::Stop(payload) = failure else {
        panic!("expected a named refusal");
    };
    assert_eq!(payload["reason"], "generated-reproduction-unavailable");
    assert_eq!(payload["paths"][0]["outcome"], "unavailable");
    assert!(started.elapsed() < Duration::from_secs(20));
    assert!(scratch_left_behind(&world.repo).is_empty());
}

#[test]
fn an_approved_reproduction_leaves_no_scratch_directory_behind() {
    let base = declared_base(REGENERATE);
    let world = fixture(&borrowed(&base), &[("sub/lock.txt", Some("locked\n"))]);

    let regenerated = require_reproduction(&world.repo, &world.base, &world.head)
        .expect("the regenerated bytes match");

    assert_eq!(regenerated, 1);
    assert!(scratch_left_behind(&world.repo).is_empty());
}
