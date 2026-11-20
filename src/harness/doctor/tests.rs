fn track(repo: &Path, files: &[(&str, &str)]) {
    for (name, text) in files {
        let path = repo.join(name);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(&path, text).expect("a file");
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["add", "--all"])
        .status()
        .expect("git add");
    assert!(status.success());
}

#[test]
fn an_undeclared_tracked_lockfile_is_named_with_its_path_and_no_command() {
    let repo = git_repo();
    track(
        repo.path(),
        &[("Cargo.lock", "x"), ("web/package-lock.json", "y")],
    );
    let check = lockfiles(repo.path()).expect("a row for undeclared lockfiles");
    assert_eq!(check.name, "lockfiles");
    let Health::Skipped { detail } = check.health else {
        panic!("an advisory row is not broken: {:?}", check.health);
    };
    for line in [
        "[[generated]]",
        "path = \"Cargo.lock\"",
        "path = \"web/package-lock.json\"",
        "cwd = \"web\"",
    ] {
        assert!(
            detail.lines().any(|found| found.trim() == line),
            "{line:?} is not a line of {detail}"
        );
    }
    for command in ["generate-lockfile", "npm install", "package-lock-only"] {
        assert!(
            !detail.contains(command),
            "{command:?} is named in {detail}"
        );
    }
    assert!(
        !detail
            .lines()
            .any(|line| line.trim().starts_with("command =")),
        "{detail}"
    );
}

#[test]
fn an_unreadable_declaration_is_said_so_and_names_no_lockfile_as_undeclared() {
    let repo = git_repo();
    track(
        repo.path(),
        &[
            ("Cargo.lock", "x"),
            ("estigia.generated.toml", "[[generated]\npath = "),
        ],
    );
    let Some(Check {
        name,
        health: Health::Skipped { detail },
        ..
    }) = lockfiles(repo.path())
    else {
        panic!("a skipped row for the unreadable declaration");
    };
    assert_eq!(name, "lockfiles");
    assert!(detail.contains("could not be read"), "{detail}");
    assert!(detail.contains("not valid TOML"), "{detail}");
    for absent in ["[[generated]]", "path =", "not declared", "Cargo.lock"] {
        assert!(!detail.contains(absent), "{absent:?} is in {detail}");
    }
}

#[test]
fn a_declaration_that_cannot_be_opened_is_said_so() {
    let repo = git_repo();
    track(repo.path(), &[("Cargo.lock", "x")]);
    std::fs::create_dir(repo.path().join("estigia.generated.toml")).expect("a directory");
    let Some(Check {
        health: Health::Skipped { detail },
        ..
    }) = lockfiles(repo.path())
    else {
        panic!("a skipped row");
    };
    assert!(detail.contains("could not be read"), "{detail}");
    assert!(!detail.contains("not declared"), "{detail}");
}

#[test]
fn an_unreadable_declaration_with_no_lockfile_has_no_row() {
    let repo = git_repo();
    track(
        repo.path(),
        &[
            ("src/main.rs", "x"),
            ("estigia.generated.toml", "[[generated]"),
        ],
    );
    assert_eq!(lockfiles(repo.path()), None);
}

#[test]
fn the_undeclared_row_says_no_command_is_suggested() {
    let repo = git_repo();
    track(repo.path(), &[("Cargo.lock", "x")]);
    let Some(Check {
        health: Health::Skipped { detail },
        ..
    }) = lockfiles(repo.path())
    else {
        panic!("a skipped row");
    };
    assert!(
        detail.contains(
            "none is suggested here, because the usual ones re-resolve dependencies instead"
        ),
        "{detail}"
    );
}

#[test]
fn a_declared_lockfile_has_no_row() {
    let repo = git_repo();
    track(
        repo.path(),
        &[
            ("Cargo.lock", "x"),
            (
                "estigia.generated.toml",
                "[[generated]]\npath = \"Cargo.lock\"\ncommand = [\"cargo\", \"generate-lockfile\"]\n",
            ),
        ],
    );
    assert_eq!(lockfiles(repo.path()), None);
}

#[test]
fn a_repository_with_no_lockfile_has_no_row() {
    let repo = git_repo();
    track(repo.path(), &[("src/main.rs", "fn main() {}")]);
    assert_eq!(lockfiles(repo.path()), None);
}

#[test]
fn an_untracked_lockfile_has_no_row() {
    let repo = git_repo();
    std::fs::write(repo.path().join("Cargo.lock"), "x").expect("a file");
    assert_eq!(lockfiles(repo.path()), None);
}

#[test]
fn only_the_undeclared_lockfile_is_named() {
    let repo = git_repo();
    track(
        repo.path(),
        &[
            ("Cargo.lock", "x"),
            ("go.sum", "y"),
            (
                "estigia.generated.toml",
                "[[generated]]\npath = \"Cargo.lock\"\ncommand = [\"cargo\", \"generate-lockfile\"]\n",
            ),
        ],
    );
    let Some(Check {
        health: Health::Skipped { detail },
        ..
    }) = lockfiles(repo.path())
    else {
        panic!("a row for go.sum");
    };
    assert!(
        detail.contains("go.sum") && !detail.contains("Cargo.lock"),
        "{detail}"
    );
}

#[test]
fn an_undeclared_tracked_lockfile_has_no_row_when_the_directory_is_not_a_repository() {
    let directory = tempfile::tempdir().expect("a directory");
    std::fs::write(directory.path().join("Cargo.lock"), "x").expect("a file");
    assert_eq!(lockfiles(directory.path()), None);
}

#[test]
fn an_older_binary_than_the_checkout_is_reported_broken_with_a_reinstall() {
    let health = gate_age_verdict(
        Path::new("/some/checkout"),
        Some("1111111"),
        false,
        Some("cf9ab5a"),
        Some(false),
        Some(3),
    );
    let Health::Broken { detail, resolution } = health else {
        panic!("a stale gate was not reported broken: {health:?}");
    };
    assert!(
        detail.contains("not established as current"),
        "the staleness was not the headline of the detail: {detail}"
    );
    assert!(
        detail.contains("cf9ab5a"),
        "the checkout's head should be named so the operator sees what they are behind: {detail}"
    );
    let Resolution::Run { command } = resolution else {
        panic!("a stale gate needs a command to repair it, not a reason it cannot have one");
    };
    // The command must replace the executable from the checkout the row is
    // about rather than the process's current directory, and — under the
    // Windows constraint the issue hands down — must stop Estigia first,
    // through a shell that can actually run a cmdlet.
    assert!(
        command.contains("cargo install --path '/some/checkout' --force"),
        "the command must reinstall from the checkout the row is about: {command}"
    );
    if cfg!(windows) {
        assert!(
            command.starts_with("powershell"),
            "a cmdlet is being printed for a shell that cannot run one: {command}"
        );
    } else {
        assert_eq!(
            command, "cargo install --path '/some/checkout' --force",
            "a non-Windows reinstall command should not open with a PowerShell cmdlet nothing \
                 there can run: {command}"
        );
    }
}

/// A `doctor` resolution is a `Health::Broken`, not a `Refusal`, so the
/// refusal ratchet never sees it; and this one is built from a value rather
/// than a literal, so the source scan cannot read it either. Its first
/// token must still be a program this machine can run.
#[test]
fn the_reinstall_command_names_a_program_that_runs() {
    let command = reinstall_command(Path::new("/some/checkout"));
    let program = command.split_whitespace().next().expect("a first token");
    assert!(
        std::process::Command::new(program)
            .arg("--version")
            .output()
            .is_ok(),
        "the reinstall command opens with `{program}`, which is not on the path: {command}"
    );
}

#[test]
fn a_broken_verdict_names_the_distance_when_it_is_known() {
    let health = gate_age_verdict(
        Path::new("."),
        Some("1111111"),
        false,
        Some("cf9ab5a"),
        Some(false),
        Some(7),
    );
    let Health::Broken { detail, .. } = health else {
        panic!("a stale gate was not reported broken: {health:?}");
    };
    assert!(
        detail.contains('7') && detail.contains("commit"),
        "the count of commits the checkout holds that the build lacks should be on the record: {detail}"
    );
}

#[test]
fn a_broken_verdict_says_the_distance_could_not_be_read_rather_than_current() {
    let health = gate_age_verdict(
        Path::new("."),
        Some("1111111"),
        false,
        Some("cf9ab5a"),
        Some(false),
        None,
    );
    let Health::Broken { detail, .. } = health else {
        panic!("a stale gate was not reported broken: {health:?}");
    };
    assert!(
        detail.contains("could not be read"),
        "an unread distance should say so rather than be silently omitted: {detail}"
    );
    assert!(
        !detail.contains("is current"),
        "an unread distance must never render as though the build were current: {detail}"
    );
}

#[test]
fn a_dirty_build_is_reported_skipped_not_stale() {
    let health = gate_age_verdict(
        Path::new("."),
        Some("cf9ab5a"),
        true,
        Some("cf9ab5a"),
        Some(true),
        None,
    );
    let Health::Skipped { detail } = health else {
        panic!("a dirty build must be cannot-tell, not a verdict: {health:?}");
    };
    assert!(
        detail.contains("uncommitted changes"),
        "the reason the comparison cannot be made should be on the record: {detail}"
    );
}

#[test]
fn a_binary_with_no_commit_is_reported_skipped_not_current() {
    let health = gate_age_verdict(
        Path::new("."),
        None,
        false,
        Some("cf9ab5a"),
        Some(false),
        None,
    );
    let Health::Skipped { detail } = health else {
        panic!("a binary with no commit must be cannot-tell: {health:?}");
    };
    assert!(
        detail.contains("carries no commit"),
        "the reason the comparison cannot be made should be on the record: {detail}"
    );
}

#[test]
fn a_checkout_with_no_head_is_reported_skipped_not_stale() {
    let health = gate_age_verdict(
        Path::new("."),
        Some("cf9ab5a"),
        false,
        None,
        Some(false),
        None,
    );
    let Health::Skipped { detail } = health else {
        panic!("a directory with no head must be cannot-tell: {health:?}");
    };
    assert!(
        detail.contains("could not be read"),
        "the reason should say the directory has no head to compare to: {detail}"
    );
}

#[test]
fn an_unreadable_ancestry_is_reported_skipped_not_stale() {
    let health = gate_age_verdict(
        Path::new("."),
        Some("cf9ab5a"),
        false,
        Some("1111111"),
        None,
        None,
    );
    let Health::Skipped { detail } = health else {
        panic!("an ancestry git could not answer must be cannot-tell, not a verdict: {health:?}");
    };
    assert!(
        detail.contains("cf9ab5a") && detail.contains("1111111"),
        "the reason should name both commits it could not compare: {detail}"
    );
}

#[test]
fn a_binary_current_with_the_checkout_is_reported_fine() {
    let health = gate_age_verdict(
        Path::new("."),
        Some("cf9ab5a"),
        false,
        Some("cf9ab5a"),
        Some(true),
        None,
    );
    let Health::Fine { detail } = health else {
        panic!("a current binary was not reported fine: {health:?}");
    };
    assert!(
        detail.contains("current against the checkout"),
        "the fine report should say it is current: {detail}"
    );
}

#[test]
fn gate_version_reports_a_row_against_a_real_checkout() {
    let repo = git_repo();
    let check = gate_version(repo.path());
    assert_eq!(check.name, "gate-version");
    assert!(
        matches!(check.health, Health::Skipped { .. }),
        "a repository with no commits cannot be compared against, so the row must say so \
             rather than pass or break: {:?}",
        check.health
    );
}

#[test]
fn the_row_still_explains_a_commit_the_repository_does_not_hold() {
    let repo = git_repo();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    };
    assert!(
        git(&["config", "user.email", "gate-version@estigia.test"])
            && git(&["config", "user.name", "estigia"])
            && git(&["commit", "--quiet", "--allow-empty", "-m", "first"]),
        "the fixture could not commit"
    );
    let foreign = "1111111111111111111111111111111111111111";
    let check = gate_version_for(repo.path(), Some(foreign), false);
    assert!(
        matches!(&check.health, Health::Skipped { detail } if detail.contains("different repository")),
        "{:?}",
        check.health
    );
    assert!(commit_is_foreign_to(repo.path(), Some(foreign), false));
    assert!(!commit_is_foreign_to(repo.path(), Some(foreign), true));
    assert!(!commit_is_foreign_to(repo.path(), None, false));
}

#[test]
fn git_is_ancestor_answers_a_real_ancestor_pair() {
    let repo = git_repo();
    let git = |args: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    };
    let rev_parse_head = || -> String {
        String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git runs")
                .stdout,
        )
        .expect("git prints utf-8")
        .trim()
        .to_owned()
    };
    let configured = git(&["config", "user.email", "gate-version@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "first"]);
    assert!(
        configured,
        "this fixture needs a usable `git` that can commit in a disposable repository, so \
             the row under test can be measured at all; a test that reports `ok` without running \
             is a defect this repository has filed against itself"
    );
    let first = rev_parse_head();
    assert!(
        git(&["commit", "--quiet", "--allow-empty", "-m", "second"]),
        "the second commit could not be made"
    );
    let second = rev_parse_head();

    assert_eq!(
        git_is_ancestor(repo.path(), &first, &second),
        Some(true),
        "the first commit is a real ancestor of the second, and the check must say so"
    );
    assert_eq!(
        git_is_ancestor(repo.path(), &second, &first),
        Some(false),
        "the second commit is not an ancestor of the first, and the check must say so"
    );
}

#[test]
fn git_commits_ahead_counts_real_commits_and_answers_none_off_the_repository() {
    let repo = git_repo();
    let git = |args: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    };
    let rev_parse_head = || -> String {
        String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git runs")
                .stdout,
        )
        .expect("git prints utf-8")
        .trim()
        .to_owned()
    };
    let configured = git(&["config", "user.email", "gate-version@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "first"]);
    assert!(
        configured,
        "this fixture needs a usable `git` that can commit in a disposable repository, so \
             the row under test can be measured at all; a test that reports `ok` without running \
             is a defect this repository has filed against itself"
    );
    let first = rev_parse_head();
    assert!(
        git(&["commit", "--quiet", "--allow-empty", "-m", "second"])
            && git(&["commit", "--quiet", "--allow-empty", "-m", "third"]),
        "the two commits ahead of `first` could not be made"
    );

    assert_eq!(
        git_commits_ahead(repo.path(), &first),
        Some(2),
        "HEAD is two real commits ahead of `first`, and the count must say so"
    );
    assert_eq!(
        git_commits_ahead(repo.path(), "0000000000000000000000000000000000000000"),
        None,
        "a commit the repository does not hold makes the command fail, and a failure must not \
             be reported as a distance"
    );
}

#[test]
fn gate_version_reports_fine_and_genuinely_stale_against_real_commits() {
    let Some(commit) = crate::build_info::commit() else {
        eprintln!("SKIPPED: this build carries no commit.");
        return;
    };
    if crate::build_info::dirty() {
        eprintln!("SKIPPED: this build came from an uncommitted tree.");
        return;
    }
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let fine = gate_version(manifest_dir);
    assert!(
        matches!(fine.health, Health::Fine { .. }),
        "this binary's own checkout was not reported current: {:?}",
        fine.health
    );

    let Ok(scratch) = tempfile::tempdir() else {
        eprintln!("SKIPPED: no scratch directory available.");
        return;
    };
    let cloned = Command::new("git")
        .args(["clone", "--quiet"])
        .arg(manifest_dir)
        .arg(scratch.path())
        .status();
    assert!(
        matches!(cloned, Ok(status) if status.success()),
        "this fixture needs a usable `git` that can clone the checkout, so the row under \
             test can be measured at all; a test that reports `ok` without running is the defect \
             this repository has filed against itself"
    );
    let git = |args: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(scratch.path())
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    };
    // A local `git clone` of a shallow source (CI checks out this crate
    // itself at `fetch-depth: 1`) disables the hardlink optimisation and
    // can leave the clone without this binary's own commit object at all
    // — the same *cannot tell* this test exists to rule out, reached
    // through the clone rather than through `git_is_ancestor`. Ask the
    // question the assertion below assumes rather than let `gate_version`
    // answer `Skipped` where the test expects `Broken`.
    if !git(&["cat-file", "-e", commit]) {
        eprintln!(
            "SKIPPED: the clone does not hold this binary's own commit ({commit}), most \
                 likely because the source checkout is itself shallow."
        );
        return;
    }
    let configured = git(&["config", "user.email", "gate-version@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "a real commit ahead of the binary under test",
        ]);
    assert!(
        configured,
        "this fixture needs a usable `git` that can commit in the clone, so the row under \
             test can be measured at all; a test that reports `ok` without running is the defect \
             this repository has filed against itself"
    );

    let stale = gate_version(scratch.path());
    let Health::Broken { detail, .. } = stale.health else {
        panic!(
            "a checkout genuinely ahead of this binary's own commit was not reported stale: {:?}",
            stale.health
        );
    };
    assert!(
        detail.contains(commit),
        "the stale report should name this binary's own commit: {detail}"
    );
    // The clone made exactly one commit past the binary's own, over a real
    // `git rev-list --count` rather than a fed value.
    assert!(
        detail.contains("1 commit"),
        "a checkout genuinely one real commit ahead should have that distance named: {detail}"
    );
}

#[test]
fn a_broken_verdict_over_a_real_git_commits_ahead_names_the_distance() {
    // Unlike the test above, this does not depend on this binary's own
    // commit or dirty state: it feeds `gate_age_verdict` the real
    // `git_commits_ahead` answer over a disposable repository rather than
    // this checkout's own build.
    let repo = git_repo();
    let git = |args: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    };
    let rev_parse_head = || -> String {
        String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git runs")
                .stdout,
        )
        .expect("git prints utf-8")
        .trim()
        .to_owned()
    };
    let configured = git(&["config", "user.email", "gate-version@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "first"]);
    assert!(
        configured,
        "this fixture needs a usable `git` that can commit in a disposable repository, so \
             the row under test can be measured at all; a test that reports `ok` without running \
             is a defect this repository has filed against itself"
    );
    let first = rev_parse_head();
    assert!(
        git(&["commit", "--quiet", "--allow-empty", "-m", "second"]),
        "the commit ahead of `first` could not be made"
    );
    let second = rev_parse_head();

    let ahead = git_commits_ahead(repo.path(), &first);
    let health = gate_age_verdict(
        Path::new("."),
        Some(&first),
        false,
        Some(&second),
        Some(false),
        ahead,
    );
    let Health::Broken { detail, .. } = health else {
        panic!("a stale gate was not reported broken: {health:?}");
    };
    assert!(
        detail.contains("1 commit"),
        "one real commit ahead, read through the real git half, should be named: {detail}"
    );
}

#[test]
fn no_holdings_is_reported_fine() {
    let root = tempfile::tempdir().expect("a temporary root");
    let check = stale_run_pointers(Some(root.path()), root.path(), &github(), &[]);
    assert_eq!(check.name, "stale-run-pointer");
    assert!(
        matches!(check.health, Health::Fine { .. }),
        "a machine holding nothing was reported as something else: {:?}",
        check.health
    );
}

#[test]
fn a_missing_skill_root_is_skipped_for_stale_pointers() {
    let root = tempfile::tempdir().expect("a temporary root");
    let check = stale_run_pointers(None, root.path(), &github(), &[]);
    assert_eq!(check.name, "stale-run-pointer");
    assert!(
        matches!(check.health, Health::Skipped { .. }),
        "a machine with no skill root to ask the tracker through was reported as something \
             else: {:?}",
        check.health
    );
}

#[test]
fn a_readable_stale_pointer_is_reported_and_names_the_release_command() {
    let repo = git_repo();
    install_contract(repo.path());
    let run = holder("claude-stale0000", 12, repo.path());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::closed_issue(12)]);
    let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[run])
    });

    let Health::Broken { detail, resolution } = &check.health else {
        panic!(
            "a stale pointer was not reported broken: {:?}",
            check.health
        );
    };
    assert!(
        detail.contains("claude-stale0000") && detail.contains("#12"),
        "the row does not name the stale pointer: {detail}"
    );
    assert!(
        resolution.to_string().contains("estigia release --run-id"),
        "the row does not name the command that clears it: {resolution}"
    );
}

#[test]
fn a_readable_live_pointer_is_reported_fine() {
    let repo = git_repo();
    install_contract(repo.path());
    let run = holder("claude-live00000", 34, repo.path());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::open_but_unmatched_issue(34)]);
    let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[run])
    });

    assert!(
        matches!(check.health, Health::Fine { .. }),
        "a pointer whose issue is open was reported stale: {:?}",
        check.health
    );
}

fn assisted_holder(repo: &Path, worktree: &Path) -> super::super::session::Run {
    let mut run = holder("claude-assist0000", 12, repo);
    run.assisted = true;
    run.branch = Some("issue-12".to_owned());
    run.worktree = Some(worktree.to_path_buf());
    run
}

#[test]
fn an_assisted_pointer_is_never_sent_to_the_tracker_and_one_whose_worktree_is_gone_is_flagged() {
    let repo = git_repo();
    install_contract(repo.path());
    let present = tempfile::tempdir().expect("a worktree directory");
    let bin = crate::test_env::scripted_gh();
    let log = repo.path().join("gh.log");
    let script = crate::test_env::answers(&[crate::test_env::closed_issue(12)]);
    let check = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        stale_run_pointers(
            Some(repo.path()),
            repo.path(),
            &github(),
            &[assisted_holder(repo.path(), present.path())],
        )
    });
    assert!(
        matches!(check.health, Health::Fine { .. }),
        "an assisted pointer was read against the tracker: {:?}",
        check.health
    );
    assert!(
        std::fs::read_to_string(&log)
            .unwrap_or_default()
            .trim()
            .is_empty(),
        "the tracker was asked about a pointer it never held"
    );

    let gone = repo.path().join("no-such-worktree");
    let check = stale_run_pointers(
        Some(repo.path()),
        repo.path(),
        &github(),
        &[assisted_holder(repo.path(), &gone)],
    );
    let Health::Broken { detail, resolution } = &check.health else {
        panic!("a gone worktree was not reported: {:?}", check.health);
    };
    assert!(
        detail.contains("claude-assist0000")
            && detail.contains("gone")
            && detail.contains("renewal window"),
        "{detail}"
    );
    assert!(
        !resolution.to_string().contains("estigia release"),
        "the way out is a command assisted refuses: {resolution}"
    );
}

#[test]
fn a_tracker_read_that_fails_is_reported_as_unread_not_stale() {
    let repo = git_repo();
    install_contract(repo.path());
    let run = holder("claude-unknown00", 56, repo.path());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::unreachable_tracker_answer()]);
    let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[run])
    });

    let Health::Broken { detail, .. } = &check.health else {
        panic!(
            "a pointer this could not ask about was reported fine, which reads a read that \
                 failed as an issue that is open: {:?}",
            check.health
        );
    };
    assert!(
        !detail.contains("closed"),
        "a read that failed was reported as though the tracker said the issue was closed: \
             {detail}"
    );
    assert!(
        detail.contains("could not be checked"),
        "a read that failed does not say so: {detail}"
    );
}

#[test]
fn a_pointer_for_another_repository_is_not_named() {
    let repo = git_repo();
    let elsewhere = git_repo();
    install_contract(repo.path());
    // Closed, if it were ever asked about — the shape most likely to be
    // silently believed rather than caught, since a false "stale" here
    // reads as an ordinary broken row rather than as an obviously wrong
    // machine-wide answer.
    let foreign = holder("claude-foreign00", 12, elsewhere.path());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::closed_issue(12)]);
    let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[foreign])
    });

    assert!(
        matches!(check.health, Health::Fine { .. }),
        "a pointer for a different repository was asked about and reported: {:?}",
        check.health
    );
}

#[test]
fn a_genuinely_passing_pointer_is_reported_fine() {
    let repo = git_repo();
    install_contract(repo.path());
    let run = holder("claude-passing00", 78, repo.path());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-passing00",
        78,
        "in-progress",
    )]);
    let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[run])
    });

    let Health::Fine { detail } = &check.health else {
        panic!(
            "a run pointer this run genuinely, currently holds \u{2014} the tracker's own \
                 code-0 answer \u{2014} was reported as something other than fine: {:?}",
            check.health
        );
    };
    assert!(
        detail.contains("1 of them read as still open and held by this run"),
        "the pointer's own code-0 answer was not counted as read; the row said: {detail}"
    );
}

/// The `Fine` sentence names exactly the row's three buckets, and denies
/// neither state that reaches `Fine` with a closed issue behind it: an aged
/// stranded pointer, and a label-mismatched own-delivery close. Neither is
/// drivable through the scripted-`gh` fixtures, so the sentence is what this
/// pins.
#[test]
fn the_fine_sentence_claims_only_the_buckets_the_row_empties() {
    let root = tempfile::tempdir().expect("a temporary root");
    let check = stale_run_pointers(Some(root.path()), root.path(), &github(), &[]);
    let Health::Fine { detail } = &check.health else {
        panic!(
            "a machine holding nothing was reported as something else: {:?}",
            check.health
        );
    };
    assert!(
        detail.contains("issue-not-open"),
        "the fine sentence does not name the stale bucket's own reason: {detail}"
    );
    assert!(
        !detail.contains("names an issue the tracker reports closed"),
        "the fine sentence denies a closed issue the row never asked about: {detail}"
    );
    assert!(
        !detail.contains("still holds one closed behind this run's own delivery"),
        "the fine sentence denies an own-delivery close a swallowed answer never \
             established: {detail}"
    );
}

#[test]
fn own_delivery_close_reads_the_transports_own_prefix_and_nothing_else() {
    let answer = |checked: serde_json::Value| super::super::tracker::Answer {
        code: 0,
        body: Some(serde_json::json!({ "checked": checked })),
    };
    assert!(
        super::own_delivery_close(&answer(serde_json::json!([
            format!(
                "{}deadbeef:epoch",
                crate::transport::claim::OWN_DELIVERY_CLOSE_PREFIX
            ),
            "single-expected-state",
        ]))),
        "the transport's own `checked[0]` was not read as its own-delivery close"
    );
    assert!(
        !super::own_delivery_close(&answer(serde_json::json!(["issue-open"]))),
        "an ordinary open answer was read as an own-delivery close"
    );
    for body in [
        serde_json::json!({}),
        serde_json::json!({ "checked": "not an array" }),
        serde_json::json!({ "checked": [7, "single-expected-state"] }),
    ] {
        assert!(
            !super::own_delivery_close(&super::super::tracker::Answer {
                code: 0,
                body: Some(body.clone()),
            }),
            "a body with no readable `checked[0]` was read as an own-delivery close: {body}"
        );
    }
}

#[test]
fn a_stale_pointer_does_not_hide_an_unread_one_in_the_same_row() {
    let repo = git_repo();
    install_contract(repo.path());
    let stale = holder("claude-stale1111", 21, repo.path());
    let unread = holder("claude-unread111", 22, repo.path());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::closed_issue(21),
        crate::test_env::unreachable_tracker_answer(),
    ]);
    let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        stale_run_pointers(Some(repo.path()), repo.path(), &github(), &[stale, unread])
    });

    let Health::Broken { detail, resolution } = &check.health else {
        panic!(
            "one stale pointer and one unread pointer covering the same checkout were not \
                 reported broken: {:?}",
            check.health
        );
    };
    assert!(
        detail.contains("claude-stale1111") && detail.contains("#21"),
        "the stale pointer dropped out of the row once an unread one joined it: {detail}"
    );
    assert!(
        detail.contains("claude-unread111"),
        "the unread pointer was hidden behind the stale one \u{2014} the early-return \
             defect this test exists to catch: {detail}"
    );
    assert!(
        resolution.to_string().contains("estigia release --run-id")
            && resolution.to_string().contains("claude-stale1111"),
        "the row still owes the release command for the stale pointer: {resolution}"
    );
}

#[test]
fn the_silence_says_what_the_last_undecided_call_said_about_itself() {
    // The counts say *which repair*. This says *which agent* — and it is
    // the half that was missing while four such lines sat on this session's
    // own machine, unexplained, with `doctor` asking the operator the
    // question the payload had already answered.
    let told = silence(&Ungated {
        calls: vec![("payload-unreadable".to_owned(), 30)],
        latest: Some(
            "a payload arrived and could not be parsed, so this call was not gated: \
                 valid JSON this build does not know, whose top-level keys are [hook_event, \
                 tool_call]; the read stopped at line 1 column 44 (Data)"
                .to_owned(),
        ),
        ..Ungated::default()
    });
    let Health::Broken { detail, .. } = &told.health else {
        panic!("reported fine");
    };
    assert!(
        detail.contains("hook_event") && detail.contains("column 44"),
        "the reason the last call was not gated is not in the check: {detail}"
    );

    // And a line from a build that had nothing to say still reads as one
    // sentence plus the one this check adds. Half of any real ledger is
    // older than this feature, and a check that appended an empty reason to
    // every one of them would be noise on exactly the machines that already
    // have a problem.
    let bare = silence(&Ungated {
        calls: vec![("payload-unreadable".to_owned(), 30)],
        latest: Some(
            "a payload arrived and could not be parsed, so this call was not gated".to_owned(),
        ),
        ..Ungated::default()
    });
    let Health::Broken { detail, .. } = &bare.health else {
        panic!("reported fine");
    };
    let lines: Vec<&str> = detail.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "an old line with nothing to add grew more than the aged-out sentence: {detail:?}"
    );
    assert!(
        lines[1].contains("ages out"),
        "the only added line is not when the call ages out: {detail:?}"
    );
}

#[test]
fn the_reason_is_read_off_the_ledger_and_not_merely_carried() {
    // The check above hands `latest` in. Nothing yet says the reader takes
    // it *out of the file*, which is how a field ends up declared, consumed
    // and never written — the shape this crate has already found four times.
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    std::fs::create_dir_all(&state).expect("the state directory");
    for entry in [
        serde_json::json!({"at":10,"verdict":"payload-unreadable","detail":"x: an older reason"}),
        serde_json::json!({"at":40,"verdict":"payload-unreadable","detail":"x: the newest reason"}),
        serde_json::json!({"at":20,"verdict":"payload-absent","detail":"x: a middling reason"}),
    ] {
        crate::harness::session::record(&state, &entry);
    }

    let found = ungated_calls(&state);
    assert_eq!(found.calls.len(), 3, "not every undecided call was read");
    // The newest by `at`, not the last line: runs append concurrently.
    assert_eq!(
        found.latest.as_deref(),
        Some("x: the newest reason"),
        "the reason read back is not the most recent one"
    );

    // One kind alone says only that kind.
    let absent = silence(&Ungated {
        calls: vec![("payload-absent".to_owned(), 5)],
        ..Ungated::default()
    });
    let Health::Broken { detail, .. } = &absent.health else {
        panic!("reported fine");
    };
    assert!(
        !detail.contains("could not be parsed"),
        "a fault nobody hit was reported: {detail}"
    );
}

fn github() -> crate::config::Tracker {
    crate::config::Tracker::Github { repo: None }
}

#[test]
fn a_gate_that_is_registered_and_would_not_run_is_reported_broken() {
    use crate::setup::wiring::Wire;

    // The failure this exists for. `status` says `gate on` for any agent
    // whose settings file carries an entry — and the entry on the machine
    // this was written on named a debug build inside a working tree.
    let root = tempfile::tempdir().expect("a temporary root");
    let file = root.path().join("settings.json");
    let gone = root.path().join("estigia.exe");
    let dead = Wire {
        command: format!("{} hook pre-tool-use", gone.display()),
        executable: gone.clone(),
        event: Some("pre-tool-use"),
        named: "pre-tool-use".to_owned(),
        blocks_on_failure: Some(true),
    };
    let checks = gates(
        &[("claude-code", vec![(file.clone(), vec![dead])], true)],
        None,
        &[],
    );
    assert_eq!(checks.len(), 1);
    assert!(checks[0].health.is_broken(), "a dead gate passed");
    let Health::Broken { detail, resolution } = &checks[0].health else {
        unreachable!("just asserted")
    };
    // Names the agent, what is wrong, and the file it is wrong in — a
    // report that said only "broken" sends somebody to read four files.
    assert!(detail.contains("claude-code"), "{detail}");
    assert!(detail.contains("ungated"), "{detail}");
    assert!(detail.contains("settings.json"), "{detail}");
    // And the way out is the command that rewrites the entry with this
    // executable's real path.
    assert!(
        format!("{resolution}").contains("estigia setup claude-code"),
        "{resolution}"
    );

    // The same entry, with the executable actually there, is fine.
    std::fs::write(&gone, "").expect("the executable");
    let live = Wire {
        command: format!("{} hook pre-tool-use", gone.display()),
        executable: gone,
        event: Some("pre-tool-use"),
        named: "pre-tool-use".to_owned(),
        blocks_on_failure: Some(true),
    };
    let checks = gates(
        &[("claude-code", vec![(file, vec![live])], true)],
        None,
        &[],
    );
    assert!(
        !checks[0].health.is_broken(),
        "a live gate was called broken"
    );
}

#[test]
fn a_gate_that_lets_a_timed_out_hook_through_is_reported_broken() {
    use crate::setup::wiring;

    let root = tempfile::tempdir().expect("a temporary root");
    let executable = root.path().join("estigia");
    std::fs::write(&executable, "").expect("the executable");
    let file = root.path().join("settings.json");
    let settings = |on_failure: Option<&str>| {
        let mut handler = serde_json::json!({
            "type": "command",
            "command": format!(
                "\"{}\" hook pre-tool-use --agent claude-code --dialect claude-code",
                executable.display()
            ),
            "timeout": 10,
        });
        if let Some(value) = on_failure {
            handler["onFailure"] = serde_json::json!(value);
        }
        serde_json::json!({
            "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [handler]}]},
        })
        .to_string()
    };
    let registration = |text: &str, file: &std::path::Path| {
        std::fs::write(file, text).expect("the settings file");
        vec![(file.to_path_buf(), wiring::wires(text))]
    };

    for (label, on_failure, broken) in [
        ("no field", None, true),
        ("`, \"onFailure\": \"block\"`", Some("block"), false),
    ] {
        let text = settings(on_failure);
        let checks = gates(
            &[("claude-code", registration(&text, &file), true)],
            None,
            &[],
        );
        assert_eq!(checks.len(), 1);
        assert_eq!(
            checks[0].health.is_broken(),
            broken,
            "{label}: {:?}",
            checks[0].health
        );
        if !broken {
            continue;
        }
        let Health::Broken { detail, resolution } = &checks[0].health else {
            unreachable!("just asserted")
        };
        assert!(detail.contains("claude-code"), "{detail}");
        assert!(detail.contains("onFailure"), "{detail}");
        assert!(detail.contains("settings.json"), "{detail}");
        assert!(
            format!("{resolution}").contains("estigia setup claude-code"),
            "{resolution}"
        );
    }

    // Codex reads the same envelope and declares no failure policy of its
    // own, so an entry without the field is not wrong there.
    let text = settings(None);
    let checks = gates(&[("codex", registration(&text, &file), true)], None, &[]);
    assert!(
        !checks[0].health.is_broken(),
        "codex was held to another host's field: {:?}",
        checks[0].health
    );
}

#[test]
fn a_gate_codex_will_skip_is_not_reported_live_and_names_what_clears_it() {
    use crate::setup::Skipped;
    use crate::setup::wiring::Wire;

    let root = tempfile::tempdir().expect("a temporary root");
    let executable = root.path().join("estigia");
    std::fs::write(&executable, "").expect("the executable");
    let wire = || Wire {
        command: format!("{} hook pre-tool-use", executable.display()),
        executable: executable.clone(),
        event: Some("pre-tool-use"),
        named: "pre-tool-use".to_owned(),
        blocks_on_failure: Some(true),
    };
    let registered = || vec![(root.path().join("hooks.json"), vec![wire()])];

    let live = gates(&[("codex", registered(), true)], None, &[]);
    assert!(matches!(&live[0].health, Health::Fine { detail } if detail.contains("1 live")));

    let untrusted = Skipped {
        detail: "Codex skips the gate: config.toml holds no trust for it".to_owned(),
        cleared_by_setup: true,
        advice: "remove the `enabled = false` line".to_owned(),
    };
    let skipped = gates(
        &[("codex", registered(), true)],
        None,
        &[("codex", untrusted)],
    );
    let Health::Broken { detail, resolution } = &skipped[0].health else {
        unreachable!(
            "a gate Codex skips was not reported broken: {:?}",
            skipped[0].health
        )
    };
    assert!(
        detail.contains("codex") && detail.contains("skips"),
        "{detail}"
    );
    assert!(!detail.contains("live"), "{detail}");
    assert_eq!(*resolution, Resolution::run("estigia setup codex"));

    let disabled = Skipped {
        detail: "Codex skips the gate: `enabled = false`".to_owned(),
        cleared_by_setup: false,
        advice: "remove the `enabled = false` line".to_owned(),
    };
    let skipped = gates(
        &[("codex", registered(), true)],
        None,
        &[("codex", disabled)],
    );
    let Health::Broken { resolution, .. } = &skipped[0].health else {
        unreachable!("a disabled gate was not reported broken")
    };
    assert!(
        matches!(resolution, Resolution::NoCommand { .. }),
        "a command was named that does not clear `enabled = false`: {resolution}"
    );

    let elsewhere = gates(
        &[("claude-code", registered(), true)],
        None,
        &[(
            "codex",
            Skipped {
                detail: String::new(),
                cleared_by_setup: true,
                advice: "remove the `enabled = false` line".to_owned(),
            },
        )],
    );
    assert!(
        !elsewhere[0].health.is_broken(),
        "another agent's trust was read"
    );
}

#[test]
fn full_reports_a_codex_gate_live_only_while_codex_trusts_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    let repo = tempfile::tempdir().expect("a temporary repository");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::find_agent("codex").expect("a declared agent");
    crate::setup::setup(adapter, &crate::config::Config::default(), &options).expect("setup runs");
    let paths = crate::setup::resolve_paths(adapter, &options).expect("paths resolve");
    let skill = Some(paths.skill_root.as_path());
    let tracker = crate::config::Tracker::Github { repo: None };
    let row = || {
        full(skill, repo.path(), &tracker, &options)
            .into_iter()
            .find(|check| check.name == "gate" && format!("{:?}", check.health).contains("codex"))
            .map(|check| check.health)
            .expect("the codex gate row is on the report")
    };

    assert!(
        matches!(row(), Health::Fine { ref detail } if detail.contains("1 live")),
        "{:?}",
        row()
    );

    let config = paths.mcp_config.expect("codex reads a config");
    let held = std::fs::read_to_string(&config).expect("read");
    std::fs::write(
        &config,
        held.replace("trusted_hash = \"sha256:", "trusted_hash = \"sha256:0"),
    )
    .expect("drift the hash");
    let Health::Broken { detail, resolution } = row() else {
        unreachable!("a gate Codex skips was reported live")
    };
    assert!(detail.contains("another hash"), "{detail}");
    assert_eq!(resolution, Resolution::run("estigia setup codex"));

    crate::setup::setup(adapter, &crate::config::Config::default(), &options)
        .expect("setup runs again");
    assert!(matches!(row(), Health::Fine { .. }), "{:?}", row());
}

#[test]
fn an_agent_with_no_gate_registered_is_skipped_rather_than_broken() {
    // Some adapters have no gate, by design.
    // Reporting a deliberate choice as damage teaches people to ignore the
    // report — which is the one thing a health report cannot survive.
    let checks = gates(&[("codex", Vec::new(), false)], None, &[]);
    assert_eq!(checks.len(), 1);
    assert!(matches!(checks[0].health, Health::Skipped { .. }));
    assert!(!checks[0].health.is_broken());
}

#[test]
fn an_entry_this_build_cannot_read_is_not_an_agent_gated_somewhere_else() {
    // Settings-gated, registered, and nothing parsed out of it.
    let unreadable = gates(&[("claude-code", Vec::new(), true)], None, &[]);
    assert!(
        unreadable[0].health.is_broken(),
        "an entry nothing could read was reported as nothing to read: {:?}",
        unreadable[0].health
    );
    let Health::Broken { resolution, .. } = &unreadable[0].health else {
        panic!("just asserted broken");
    };
    assert!(
        matches!(resolution, Resolution::Run { command, .. } if command.contains("claude-code")),
        "the fault names no command that re-registers it: {resolution:?}"
    );

    // The sentence it used to give belongs to an agent that really does keep
    // its gate in a file of its own, and it keeps it.
    let plugin = gates(&[("cline", Vec::new(), true)], None, &[]);
    let Health::Skipped { detail } = &plugin[0].health else {
        panic!(
            "a plugin-gated agent was reported broken: {:?}",
            plugin[0].health
        );
    };
    assert!(
        detail.contains("its own file"),
        "a plugin-gated agent stopped being told apart: {detail}"
    );

    // And the floor at the other end: an agent with no gate at all is still
    // skipped, not broken. Without this, refusing everything would pass the
    // assertion above.
    let none = gates(&[("codex", Vec::new(), false)], None, &[]);
    assert!(
        !none[0].health.is_broken(),
        "an agent that was never gated was reported as damage: {:?}",
        none[0].health
    );
}

#[test]
fn a_missing_skill_is_the_only_thing_reported_because_nothing_after_it_means_anything() {
    let checks = examine(None, Path::new("."));
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].name, "skill");
    assert!(checks[0].health.is_broken());
}

#[test]
fn every_broken_check_names_a_way_out() {
    // The ratchet applied to an environment. A report that says what is
    // wrong and not what to do is the dead end it exists to prevent.
    let root = tempfile::tempdir().expect("a temporary root");
    for check in examine(Some(root.path()), root.path()) {
        if check.health.is_broken() {
            assert!(
                check.health.resolution().is_some(),
                "{} is broken and says nothing about what to do",
                check.name
            );
        }
    }
}

#[test]
fn a_directory_that_is_not_a_repository_is_skipped_rather_than_failed() {
    // Running `doctor` from a home directory must not report that git is
    // broken. A report people learn to ignore is a report that has stopped
    // working.
    let root = tempfile::tempdir().expect("a temporary root");
    let checks = examine(Some(root.path()), root.path());
    let remote = checks
        .iter()
        .find(|check| check.name == "remote")
        .expect("the remote is checked");
    assert!(
        matches!(remote.health, Health::Skipped { .. }),
        "a non-repository was reported as a failure: {:?}",
        remote.health
    );
}

#[test]
fn every_check_says_what_it_is_for() {
    let root = tempfile::tempdir().expect("a temporary root");
    for check in examine(Some(root.path()), root.path()) {
        assert!(
            check.about.len() > 20,
            "{} does not say what it is for",
            check.name
        );
        assert!(!check.name.is_empty());
    }
}

#[test]
fn nothing_on_the_list_asks_for_an_interpreter() {
    let root = tempfile::tempdir().expect("a temporary root");
    let checks = examine(Some(root.path()), root.path());
    assert!(
        !checks.iter().any(|check| check.name == "python"),
        "the health report asks for an interpreter this release does not use"
    );
    // The floor: the walk produced the rows it should, so a list that came
    // back empty — or came back without the row this one replaced — cannot
    // pass for a clean one.
    for wanted in ["skill", "gh"] {
        assert!(
            checks.iter().any(|check| check.name == wanted),
            "the walk produced no `{wanted}` row, so its silence about `python` says nothing"
        );
    }
}
#[test]
fn each_agent_is_reported_by_the_path_it_actually_reads() {
    let root = tempfile::tempdir().expect("a temporary root");
    let present = root.path().join("own/SKILL.md");
    std::fs::create_dir_all(present.parent().expect("a parent")).expect("the directory");
    std::fs::write(&present, "# contract").expect("the contract");
    let absent = root.path().join("shared/SKILL.md");

    fn entry(agent: &'static str, path: std::path::PathBuf, own_root: bool) -> Contract {
        Contract {
            agent,
            path,
            own_root,
            directive: None,
            directive_absent: None,
            stale: false,
            unreadable: None,
            ignored: Vec::new(),
            cut_short: Vec::new(),
            shadowed_local: None,
            duplicated: Vec::new(),
            unknown: Vec::new(),
            inherited: Vec::new(),
        }
    }

    // Nothing configured is a broken check, not an empty report: a doctor
    // that says nothing about agents reads as "all fine" to somebody who
    // installed none.
    let none = contracts(&[]);
    assert_eq!(none.len(), 1);
    assert!(matches!(none[0].health, Health::Broken { .. }));

    let checks = contracts(&[
        entry("codex", present.clone(), true),
        entry("claude-code", absent.clone(), false),
    ]);
    assert_eq!(checks.len(), 2);

    // The one that is there reports the full path, so an operator can hand
    // back an answer nobody had to guess at.
    let Health::Fine { detail } = &checks[0].health else {
        panic!("a contract that exists was reported broken");
    };
    assert!(detail.contains(&present.display().to_string()));
    assert!(
        detail.contains("verified"),
        "a verified root is not said so"
    );

    // The shared root says plainly what has to be confirmed. Promoting an
    // adapter by reading its documentation is the unchecked claim
    // `setup::tests` forbids; this is the only honest alternative.
    let shared = contracts(&[entry("claude-code", present.clone(), false)]);
    let Health::Fine { detail } = &shared[0].health else {
        panic!("the shared root was reported broken");
    };
    assert!(
        detail.contains("confirm your agent reads it"),
        "the operator is not told what to check: {detail}"
    );

    // A file that is there is not a file a run can use. Three documents
    // layer into what the agent reads, two of them hand-editable, and one
    // bad row in any of them refuses every command that reads
    // configuration — while this check said `ok` because the contract
    // existed.
    let unreadable = contracts(&[Contract {
        agent: "codex",
        path: present,
        own_root: true,
        directive: None,
        directive_absent: None,
        stale: false,
        unreadable: Some("`Integration` is \"octopus\" in estigia.local.md".to_owned()),
        ignored: Vec::new(),
        cut_short: Vec::new(),
        shadowed_local: None,
        duplicated: Vec::new(),
        unknown: Vec::new(),
        inherited: Vec::new(),
    }]);
    let Health::Broken {
        detail, resolution, ..
    } = &unreadable[0].health
    else {
        panic!("a contract no run can read was reported fine");
    };
    assert!(detail.contains("octopus"), "{detail}");
    assert!(
        detail.contains("estigia.local.md"),
        "it does not say which file: {detail}"
    );
    // And it names no command: two of the three files are the operator's,
    // and Estigia does not edit those.
    assert!(
        matches!(resolution, crate::outcome::Resolution::NoCommand { .. }),
        "a row in the operator's own file was given a command to run"
    );

    // A contract that is not there is broken, with the command that fixes
    // it — not a bare "missing".
    let Health::Broken { detail, resolution } = &checks[1].health else {
        panic!("a missing contract was reported fine");
    };
    assert!(detail.contains(&absent.display().to_string()));
    assert!(
        format!("{resolution:?}").contains("claude-code"),
        "the way out does not name the agent to fix"
    );
}

#[test]
fn an_ungated_line_is_attributed_to_its_agent_and_never_to_its_dialect() {
    let source_of = |line: &str| {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("state");
        std::fs::create_dir_all(&state).expect("the state directory");
        let entry: serde_json::Value = serde_json::from_str(line).expect("a record");
        crate::harness::session::record(&state, &entry);
        ungated_calls(&state).source
    };

    // What the hook writes now. Codex speaks Claude Code's dialect, and
    // the line has to come back as Codex.
    let both = source_of(
        r#"{"at":1,"verdict":"payload-absent","agent":"codex","dialect":"claude-code","event":"pre-tool-use"}"#,
    );
    assert_eq!(
        both.as_deref(),
        Some("codex's pre-tool-use hook"),
        "an ungated call was attributed to the dialect its agent happens to speak"
    );

    // A line from a settings file written before `--agent` existed. Half an
    // answer, offered as half — several agents share each dialect, and
    // naming one of them is the defect this pair was split to fix.
    let dialect_only = source_of(
        r#"{"at":1,"verdict":"payload-absent","dialect":"claude-code","event":"pre-tool-use"}"#,
    );
    let dialect_only = dialect_only.expect("a line that knows its dialect says so");
    assert!(
        dialect_only.contains("claude-code") && dialect_only.contains("dialect"),
        "the one fact such a line does carry was thrown away: {dialect_only}"
    );
    assert!(
        !dialect_only.starts_with("claude-code'"),
        "a dialect is being read out as the agent's name: {dialect_only}"
    );

    // And a line from before either: nothing, which `silence` reports as
    // *from an agent no line names*.
    assert_eq!(
        source_of(r#"{"at":1,"verdict":"payload-absent"}"#),
        None,
        "a line naming nothing was attributed to somebody"
    );
}

#[test]
fn a_gate_deciding_by_rows_the_agent_never_reads_is_reported() {
    use super::{Divergence, canonical};

    let root = std::path::PathBuf::from("/home/somebody/.agents/skills/flow");

    // A comparison that was not made is never reported as agreement.
    assert!(matches!(
        canonical(Some(&root), None).health,
        super::Health::Skipped { .. }
    ));
    assert!(matches!(
        canonical(None, Some(&[])).health,
        super::Health::Skipped { .. }
    ));

    // Agreement is quiet, and still says where the deciding happens.
    let agreed = canonical(Some(&root), Some(&[]));
    assert_eq!(agreed.name, "canonical");
    match agreed.health {
        super::Health::Fine { detail } => assert!(
            detail.contains("flow"),
            "the row reporting agreement does not say where: {detail}"
        ),
        other => panic!("agreement was reported as {other:?}"),
    }

    // A row about the repository differing is the fault: those answer the
    // same whichever agent asks, so two answers means one agent is being
    // decided for by the other's file.
    let diverged = canonical(
        Some(&root),
        Some(&[
            Divergence {
                agent: "claude-code",
                rows: vec![(
                    crate::config::Setting::Integration,
                    "trunk".to_owned(),
                    "branch".to_owned(),
                )],
            },
            Divergence {
                agent: "codex",
                rows: vec![(
                    crate::config::Setting::ChangeSize,
                    "120".to_owned(),
                    "800".to_owned(),
                )],
            },
        ]),
    );
    match diverged.health {
        super::Health::Broken { detail, resolution } => {
            for expected in ["claude-code", "Integration", "trunk", "branch"] {
                assert!(
                    detail.contains(expected),
                    "the divergence does not name {expected:?}: {detail}"
                );
            }
            assert!(
                detail.contains("one other agent diverges too"),
                "a second diverging agent was dropped without a word: {detail}"
            );
            // The plain form propagates a repository row, so it is the one
            // named here — and it is named because running it clears this.
            let way_out = format!("{resolution}");
            assert!(
                way_out.contains("with no `--agent`"),
                "a repository row was not offered the command that clears it: {way_out}"
            );
        }
        other => panic!("a gate deciding by rows nobody reads was reported as {other:?}"),
    }

    // A row about the machine is the fault too, and it now has the same way
    // out. It did not: two rounds of review measured both forms leaving the
    // row exactly as red, because the plain one wrote the canonical
    // contract alone and the per-agent one could not hold a machine row in
    // a shared root at all. Both halves are fixed, so this
    // asserts the command is named — and, below, that the sentence saying
    // nothing clears it is gone. A report that goes on describing a closed
    // gap sends an operator away from the command that works.
    let machine = canonical(
        Some(&root),
        Some(&[Divergence {
            agent: "claude-code",
            rows: vec![(
                crate::config::Setting::Summary,
                "Spanish".to_owned(),
                "English".to_owned(),
            )],
        }]),
    );
    match machine.health {
        super::Health::Broken { detail, resolution } => {
            assert!(detail.contains("Summary language"), "{detail}");
            let way_out = format!("{resolution}");
            assert!(
                way_out.contains("config set \"<row>\""),
                "a machine row was not offered the command that now propagates it: {way_out}"
            );
            assert!(
                way_out.contains("with no `--agent`"),
                "the resolution does not say which form of the command holds it: {way_out}"
            );
            assert!(
                !way_out.contains("no command does that yet"),
                "the report still describes a gap that is closed: {way_out}"
            );
        }
        other => panic!("a row about the machine differing was reported as {other:?}"),
    }

    // And a per-agent row differing is the feature. `config set --agent
    // opencode "Delivery authorisation" ask` is a documented command, and
    // calling the machine broken for having run it names a fault with no way
    // out — measured by both halves of the first review of this row, one on
    // `Model routing` and one on `Delivery authorisation`. Named, never
    // broken.
    let per_agent = canonical(
        Some(&root),
        Some(&[Divergence {
            agent: "opencode",
            rows: vec![
                (
                    crate::config::Setting::Delivery,
                    "ask".to_owned(),
                    "auto".to_owned(),
                ),
                (
                    crate::config::Setting::Models,
                    "developer=opus".to_owned(),
                    "unset".to_owned(),
                ),
            ],
        }]),
    );
    match per_agent.health {
        super::Health::Fine { detail } => {
            for expected in [
                "opencode",
                "Delivery authorisation",
                "Model routing",
                "by design",
            ] {
                assert!(
                    detail.contains(expected),
                    "a row that differs by design was not named: {detail}"
                );
            }
        }
        other => panic!("a supported per-agent configuration was reported as {other:?}"),
    }

    // And it is still named when something else is broken. It was not: the
    // broken branch built its sentence from the faulty rows alone, so a row
    // somebody had set on purpose disappeared from the report exactly when
    // the report grew a second subject.
    let both = canonical(
        Some(&root),
        Some(&[Divergence {
            agent: "claude-code",
            rows: vec![
                (
                    crate::config::Setting::ChangeSize,
                    "120".to_owned(),
                    "800".to_owned(),
                ),
                (
                    crate::config::Setting::Delivery,
                    "ask".to_owned(),
                    "auto".to_owned(),
                ),
            ],
        }]),
    );
    match both.health {
        super::Health::Broken { detail, .. } => {
            assert!(detail.contains("Change size"), "{detail}");
            assert!(
                detail.contains("Delivery authorisation") && detail.contains("by design"),
                "the deliberate per-agent row vanished as soon as a fault appeared: {detail}"
            );
        }
        other => panic!("a repository row at odds was reported as {other:?}"),
    }
}

/// `tracker_in_force` reads the checkout's own layer, not the contract alone.
///
/// The test that held this was deleted with the tracker whose *kind* it
/// switched, and the kind is what became unposable — not the value. With one
/// variant left, the observable difference is the repository the tracker
/// names, and it is load-bearing: `stale_run_pointers` branches on
/// `named_repo()`, so a doctor reading the contract alone asks the wrong
/// repository whether a pointed-at issue is still open.
///
/// The contract is given a named repository of its own, `contract/one`, so
/// the floor below is not `None` — a `tracker_in_force` short-circuited to
/// `Config::default().tracker` would answer `None` for **both**
/// assertions, passing the second and reading the first as vacuous. With a
/// contract that actually names something, the floor requires the contract
/// to be read at all, and the second assertion still requires the
/// checkout's own layer to override it.
#[test]
fn the_tracker_in_force_is_the_one_this_checkout_names_not_the_contracts() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(crate::setup::Platform::Unix),
        skip_harness: true,
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::find_agent("agents").expect("a declared agent");
    crate::setup::setup(
        adapter,
        &crate::config::Config {
            tracker: crate::config::Tracker::Github {
                repo: Some(
                    crate::config::RepoRef::parse("contract/one").expect("a valid repo ref"),
                ),
            },
            ..crate::config::Config::default()
        },
        &options,
    )
    .expect("the install writes");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("paths")
        .skill_root;

    let repo = tempfile::tempdir().expect("a checkout");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout with a git directory");

    // The floor: with no layer of its own, the checkout takes the
    // contract's answer — `contract/one`, and not `None`, so a function
    // that never read the contract at all fails here rather than passing
    // vacuously.
    assert_eq!(
        super::tracker_in_force(Some(&root), repo.path()).named_repo(),
        Some("contract/one".to_owned()),
        "the contract's own repository was not read"
    );

    let path = crate::skill::repository_config_path(repo.path());
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    std::fs::write(
        &path,
        "| Tracker | github acme/repo |
",
    )
    .expect("the layer is writable");

    assert_eq!(
        super::tracker_in_force(Some(&root), repo.path()).named_repo(),
        Some("acme/repo".to_owned()),
        "the checkout's own row was not read: {}",
        path.display()
    );
}

/// The wiring, not only the function: `tracker_in_force`'s answer has to
/// actually reach the check that reads it, the way `cli::doctor` feeds one
/// into the other at its own call site rather than each reading the
/// contract on its own.
///
/// The test above asserts on `tracker_in_force`'s return value directly —
/// a correct answer nobody downstream consumes would still pass it. This
/// composes it with `stale_run_pointers` the same way `cli::doctor` does,
/// and reads back which repository `gh` was actually asked about through
/// `GH_REPO`, the variable `tracker_answer_for_pointer` sets from
/// `named_repo()` before every read. Confirmed red against the
/// contract-only mutation named above: `tracker_in_force` short-circuited
/// to `Config::default().tracker` sends this to `gh` asking about neither
/// repository the fixture names, and the assertion for `acme/repo` fails.
#[test]
fn the_stale_run_pointer_check_asks_gh_about_the_checkout_s_own_repository() {
    let repo = git_repo();

    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(crate::setup::Platform::Unix),
        skip_harness: true,
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::find_agent("agents").expect("a declared agent");
    crate::setup::setup(
        adapter,
        &crate::config::Config {
            tracker: crate::config::Tracker::Github {
                repo: Some(
                    crate::config::RepoRef::parse("contract/one").expect("a valid repo ref"),
                ),
            },
            ..crate::config::Config::default()
        },
        &options,
    )
    .expect("the install writes");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("paths")
        .skill_root;

    // The checkout's own layer names a different repository — the one
    // this call has to ask `gh` about.
    let path = crate::skill::repository_config_path(repo.path());
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    std::fs::write(&path, "| Tracker | github acme/repo |\n").expect("the layer is writable");

    // Exactly the seam `cli::doctor` wires at its own call site: the
    // tracker `tracker_in_force` derives, fed straight into the check
    // that reads it.
    let tracker = super::tracker_in_force(Some(&root), repo.path());
    let run = holder("claude-wiring0000", 91, repo.path());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::closed_issue(91)]);
    let log = home.path().join("gh-env.log");
    let logged = crate::test_env::with_env_logging_gh(bin.path(), &script, &log, || {
        let _ = super::stale_run_pointers(Some(&root), repo.path(), &tracker, &[run]);
        std::fs::read_to_string(&log).unwrap_or_default()
    });

    assert!(
        logged.contains("GH_REPO=acme/repo"),
        "the checkout's own repository was not the one gh was asked about: {logged}"
    );
    assert!(
        !logged.contains("GH_REPO=contract/one"),
        "the contract's repository reached gh instead of the checkout's: {logged}"
    );
}

#[test]
fn the_report_crosses_the_two_roots_rather_than_reporting_each_alone() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(crate::setup::Platform::Unix),
        skip_harness: true,
        ..crate::setup::SetupOptions::default()
    };
    let config = crate::config::Config::default();
    for slug in ["agents", "claude-code"] {
        let adapter = crate::setup::find_agent(slug).expect("a declared agent");
        crate::setup::setup(adapter, &config, &options).expect("the install writes");
    }
    let claude = crate::setup::resolve_paths(
        crate::setup::find_agent("claude-code").expect("a declared agent"),
        &options,
    )
    .expect("paths")
    .skill_root;

    let named = |checks: &[super::Check]| {
        checks
            .iter()
            .find(|check| check.name == "canonical")
            .map(|check| check.health.clone())
            .expect("the report carries the row")
    };
    let report = |options: &crate::setup::SetupOptions| {
        let root = crate::harness::discover_skill_root_in(options).expect("a canonical root");
        super::full(
            Some(&root),
            home.path(),
            &crate::config::Tracker::Github { repo: None },
            options,
        )
    };

    // Two identical installs and no operator file: nothing diverges.
    assert!(
        matches!(named(&report(&options)), super::Health::Fine { .. }),
        "a machine whose roots agree is being told they do not"
    );

    // Their own file, in one root, setting a per-agent row the other cannot
    // see. Named, and still `Fine`: that row is allowed to differ.
    let theirs = claude.join(crate::config::LOCAL_FILE);
    std::fs::write(
        &theirs,
        "| Setting | Value here |\n|---|---|\n| Model routing | developer=opus |\n",
    )
    .expect("the operator's own file");
    match named(&report(&options)) {
        super::Health::Fine { detail } => assert!(
            detail.contains("Model routing") && detail.contains("by design"),
            "a row that may differ by agent was not named: {detail}"
        ),
        other => panic!("a supported per-agent configuration was reported as {other:?}"),
    }

    // A machine-wide row, which may not: one of these two agents is being
    // decided for by a file it does not read.
    std::fs::write(
        &theirs,
        "| Setting | Value here |\n|---|---|\n| Change size | 120 |\n",
    )
    .expect("the operator's own file");
    match named(&report(&options)) {
        super::Health::Broken { detail, .. } => assert!(
            detail.contains("Change size"),
            "the divergence does not name the row: {detail}"
        ),
        other => panic!("a silent divergence stayed silent: {other:?}"),
    }
}

const COUNTED_AT: u64 = 1_800_000_000;

fn counted(
    kind: &str,
    count: u64,
    last_at: Option<u64>,
    last_error: Option<&str>,
) -> crate::transport::store::RecordFailure {
    crate::transport::store::RecordFailure {
        kind: kind.to_owned(),
        count,
        last_at,
        last_error: last_error.map(str::to_owned),
    }
}

#[test]
fn a_counted_failure_inside_the_retention_is_broken_with_its_date() {
    let at = COUNTED_AT - 86_400;
    let health = super::counted_failures(&[counted("decisions", 2, Some(at), None)], COUNTED_AT);
    let super::Health::Broken { detail, .. } = health else {
        panic!("a counted failure from yesterday was reported fine");
    };
    assert!(
        detail.contains("2 decision record(s)"),
        "how many is not said: {detail}"
    );
    assert!(
        detail.contains(&crate::harness::session::stamp_of(at)),
        "when the last one was is not said: {detail}"
    );
}

#[test]
fn a_counted_failure_older_than_the_retention_stops_being_broken() {
    let days = crate::harness::session::RECORD_RETENTION_DAYS;
    let boundary = COUNTED_AT - days * 86_400;
    assert!(
        super::counted_failures(&[counted("decisions", 1, Some(boundary), None)], COUNTED_AT)
            .is_broken(),
        "a failure exactly at the retention boundary was read as older than it"
    );
    let old = COUNTED_AT - (days + 1) * 86_400;
    assert!(
        !super::counted_failures(&[counted("decisions", 1, Some(old), None)], COUNTED_AT)
            .is_broken(),
        "a failure whose records have already aged out is still reported as a current fault"
    );
}

#[test]
fn a_counted_failure_with_no_recorded_time_stays_broken() {
    assert!(
        super::counted_failures(&[counted("decisions", 1, None, None)], COUNTED_AT).is_broken(),
        "a failure whose age is unknown was reported as clearance"
    );
}

#[test]
fn a_failed_snapshot_is_described_as_one_and_keeps_its_reason() {
    let health = super::counted_failures(
        &[counted(
            "snapshot",
            1,
            Some(COUNTED_AT - 60),
            Some("could not create /home/x/.estigia/snapshots: the disk was full"),
        )],
        COUNTED_AT,
    );
    let super::Health::Broken { detail, .. } = health else {
        panic!("a failed snapshot was reported fine");
    };
    assert!(
        detail.contains("snapshot"),
        "the kind is not named: {detail}"
    );
    assert!(
        !detail.contains("decision"),
        "a snapshot failure is described as a decision: {detail}"
    );
    assert!(
        detail.contains("the disk was full"),
        "the reason the snapshot failed is not in the report: {detail}"
    );
}

#[test]
fn the_silence_says_when_the_newest_undecided_call_ages_out() {
    let at = 1_785_904_685;
    let check = silence(&Ungated {
        calls: vec![("payload-absent".to_owned(), at)],
        ..Ungated::default()
    });
    assert!(
        check.health.is_broken(),
        "an ungated call stopped being broken"
    );
    let super::Health::Broken { detail, .. } = &check.health else {
        panic!("an ungated call stopped being broken");
    };
    let aged_out = at + crate::harness::session::RECORD_RETENTION_DAYS * 24 * 60 * 60;
    assert!(
        detail.contains(&crate::harness::session::stamp_of(aged_out)),
        "when the newest one ages out is not said: {detail}"
    );
}

#[test]
fn an_undated_undecided_call_is_not_given_a_date() {
    let check = silence(&Ungated {
        calls: vec![("payload-absent".to_owned(), 0)],
        ..Ungated::default()
    });
    let super::Health::Broken { detail, .. } = &check.health else {
        panic!("an ungated call stopped being broken");
    };
    assert!(
        detail.contains("did not record") && detail.contains("carries no date"),
        "an undated call is not reported as undated: {detail}"
    );
    assert!(
        !detail.contains("1970"),
        "an undated call was given the epoch as its date: {detail}"
    );
}

const STALLED_FIXTURE: &str = "harness::doctor::tests::a_stalled_program_fixture";

#[test]
#[ignore = "the child a_program_that_stalls_is_broken_within_its_limit launches"]
fn a_stalled_program_fixture() {
    std::thread::sleep(std::time::Duration::from_secs(5));
}

#[test]
fn a_program_that_stalls_is_broken_within_its_limit() {
    let executable = std::env::current_exe().expect("the test executable");
    let resolution = Resolution::run("estigia setup --all");
    let started = std::time::Instant::now();
    let health = super::program_within(
        &executable.display().to_string(),
        &["--ignored", "--exact", STALLED_FIXTURE, "--nocapture"],
        |output| output.to_owned(),
        resolution.clone(),
        std::time::Duration::from_millis(300),
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(3),
        "the check waited {:?} on a stalled program",
        started.elapsed()
    );
    match health {
        super::Health::Broken {
            detail,
            resolution: carried,
        } => {
            assert_eq!(carried, resolution);
            assert!(detail.contains("did not finish within"), "{detail}");
        }
        other => panic!("a stalled program was reported as {other:?}"),
    }
}
