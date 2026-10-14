use std::process::Command;
use std::time::Duration;

use super::*;

/// A real repository, because every path here goes through `git rev-parse` and
/// a fixture would only restate the assumption.
///
/// It panics rather than answering `None`, the repair it was given: this
/// is a git workflow harness, so a machine without a usable `git` cannot
/// measure the push guard at all, and a `None` the caller turns into `return;`
/// after an `eprintln!` reports an unmeasured test as `ok` to every ordinary
/// run. `docs/honesty.md` records the shape and what it cost.
fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let git = |arguments: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory.path())
            .args(arguments)
            .output()
            .unwrap_or_else(|error| {
                panic!(
                    "this fixture needs a usable `git` on PATH and could not run \
                     `git {}`: {error}. Install git, or fix the command, because a test \
                     that reports `ok` without running is a defect this repository has filed against itself.",
                    arguments.join(" ")
                )
            });
        assert!(
            output.status.success(),
            "`git {}` failed: {}. This fixture needs a usable `git` on PATH — \
             `docs/honesty.md` records why an unmeasured test must not report `ok`.",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    };
    git(&["init", "--quiet"]);
    directory
}

/// Writes a hook the way an operator's own tooling would leave it: runnable.
///
/// `fs::write` alone produces a file without the execute bit, and on a system
/// where git consults the mode that is an **`Inert`** hook — present, readable,
/// and silently never run. `state` says so correctly, which is the whole reason
/// that variant exists.
///
/// Two tests wrote a hook with `fs::write` and asserted `Chained`. On Windows
/// there is no mode to consult, so `runnable` is true and both passed; on Linux
/// and macOS they asserted that an unrunnable hook is a running gate, and failed.
/// The tests were describing a state the file was not in — the crate was right
/// both times.
///
/// Use this wherever a test means *a hook git will actually run*. A test that
/// means the opposite should write the file directly and say so.
fn write_runnable_hook(path: &Path, text: &str) {
    std::fs::write(path, text).expect("the hook is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut mode = std::fs::metadata(path)
            .expect("the hook is there")
            .permissions();
        mode.set_mode(0o755);
        std::fs::set_permissions(path, mode).expect("the hook is made runnable");
    }
}

fn context(root: &Path, repo: &Path) -> GateContext {
    let skill_root = root.join("skill");
    std::fs::create_dir_all(&skill_root).expect("a skill root");
    std::fs::write(
        skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root,
        repo_dir: repo.to_path_buf(),
        state_root: root.join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

use crate::test_env::{
    answers, closed_issue, live_holder_issue, not_current_live_holder_issue,
    open_but_unmatched_issue, repository_with_commit, scripted_gh, unreachable_tracker_answer,
};

/// Registers a real worktree of `repo` on a fresh branch — the shape
/// `narrow_to_pushed_branch` reads back through `git worktree list`, so a
/// fixture that only wrote a directory would not exercise it.
fn add_worktree(repo: &Path, path: &Path, branch: &str) {
    let added = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "-b", branch])
        .arg(path)
        .output()
        .is_ok_and(|output| output.status.success());
    assert!(added, "git could not add the worktree this fixture needs");
}

#[test]
fn a_stand_down_does_not_launder_a_pointer_nobody_could_read() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = tempfile::tempdir().expect("a checkout");
    let state = root.path().join("state");
    std::fs::create_dir_all(&state).expect("the state directory");
    let pointer = state.join("claude-torn.json");
    let corrupt = "{\"run_id\": \"claude-torn\", \"issue\":";
    std::fs::write(&pointer, corrupt).expect("a torn pointer");

    let mut context = context(root.path(), repo.path());
    context.state_root = state.clone();
    context.tracker = crate::config::Tracker::Github { repo: None };
    let action = crate::harness::Action::Write {
        target: "src/main.rs".to_owned(),
    };

    // Refused while nothing is standing down.
    let mut run = crate::harness::session::load(&state, "claude-torn");
    assert!(run.unreadable, "the fixture's pointer parses after all");
    let plain = crate::harness::gate(
        &context,
        &mut run,
        &action,
        crate::harness::Sensitivity::Routine,
    );
    let Decision::Deny(refusal) = &plain else {
        panic!("an unreadable pointer stopped being a refusal: {plain:?}");
    };
    assert_eq!(refusal.code, "run-pointer-unreadable");

    // Stood down: the write goes through, which is what a stand-down is for.
    context.stand_down = Some(crate::harness::standdown::StandDown {
        reason: "the pointer is corrupt and I have to work".to_owned(),
        // A stand-down `declare` could actually issue. This was
        // `declared_at: 0, until: u64::MAX` — a window of five hundred billion
        // years — and it passed only because the cap was checked on the way out
        // and never on the way back in. A fixture posing a record no operator
        // can obtain is a fixture measuring a path nobody reaches.
        declared_at: crate::harness::session::now_seconds().expect("a clock"),
        until: crate::harness::session::now_seconds().expect("a clock")
            + crate::harness::standdown::LONGEST,
        declared_by: "asanabrial".to_owned(),
    });
    let mut during = crate::harness::session::load(&state, "claude-torn");
    let allowed = crate::harness::gate(
        &context,
        &mut during,
        &action,
        crate::harness::Sensitivity::Routine,
    );
    assert!(
        matches!(allowed, Decision::Allow(_)),
        "the stand-down stopped reaching this refusal: {allowed:?}"
    );
    // The store the hook makes on every allowance.
    let _ = crate::harness::session::store(&state, &during);

    assert_eq!(
        std::fs::read_to_string(&pointer).expect("the pointer is still there"),
        corrupt,
        "the allowance rewrote the pointer nobody could read, so what it held is now gone"
    );

    // And when the window is over, the refusal is back — which is the whole of
    // what "bounded" means here.
    context.stand_down = None;
    let mut after = crate::harness::session::load(&state, "claude-torn");
    assert!(
        matches!(
            crate::harness::gate(
                &context,
                &mut after,
                &action,
                crate::harness::Sensitivity::Routine
            ),
            Decision::Deny(_)
        ),
        "the window expired and the gate did not come back"
    );

    // The floor: a run whose pointer reads is still stored, or the hot path has
    // stopped recording when it last asked and this test guards a dead branch.
    let mut whole = crate::harness::session::Run::new("claude-whole".to_owned());
    whole.issue = Some(crate::identity::Identity::Number(12));
    assert_eq!(
        crate::harness::session::store(&state, &whole),
        Ok(true),
        "a readable run stopped being recorded"
    );
}

#[test]
fn a_stand_down_reaches_the_refusals_the_push_path_raises_itself() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = tempfile::tempdir().expect("a checkout");
    let state = root.path().join("state");
    std::fs::create_dir_all(&state).expect("the state directory");
    // On disk, under a run's name, and not parseable.
    std::fs::write(
        state.join("claude-torn.json"),
        "{\"run_id\": \"claude-torn\",",
    )
    .expect("a torn pointer");

    let mut context = context(root.path(), repo.path());
    context.state_root = state;
    let action = Action::Write {
        target: "src/main.rs".to_owned(),
    };

    // Refused, and rightly: whether a claim covers this checkout is unknown.
    let refused = decide_action(&context, repo.path(), &action, Sensitivity::Routine);
    let Decision::Deny(refusal) = &refused else {
        panic!("an unreadable pointer stopped being a refusal: {refused:?}");
    };
    assert_eq!(refusal.code, "run-pointers-unreadable");

    // And with a stand-down in force it goes through, saying what it overrode —
    // which is the whole difference between this and a switch.
    context.stand_down = Some(crate::harness::standdown::StandDown {
        reason: "the pointer is somebody else's and they are away".to_owned(),
        // A stand-down `declare` could actually issue. This was
        // `declared_at: 0, until: u64::MAX` — a window of five hundred billion
        // years — and it passed only because the cap was checked on the way out
        // and never on the way back in. A fixture posing a record no operator
        // can obtain is a fixture measuring a path nobody reaches.
        declared_at: crate::harness::session::now_seconds().expect("a clock"),
        until: crate::harness::session::now_seconds().expect("a clock")
            + crate::harness::standdown::LONGEST,
        declared_by: "asanabrial".to_owned(),
    });
    let allowed = decide_action(&context, repo.path(), &action, Sensitivity::Routine);
    let Decision::Allow(why) = &allowed else {
        panic!("a declared stand-down did not reach this refusal: {allowed:?}");
    };
    assert!(
        why.contains("run-pointers-unreadable") && why.contains("stood down"),
        "the allowance does not name what it overrode: {why}"
    );

    // And it is not a switch: a window that has expired changes nothing.
    context.stand_down = Some(crate::harness::standdown::StandDown {
        reason: "over".to_owned(),
        declared_at: 0,
        until: 1,
        declared_by: "asanabrial".to_owned(),
    });
    assert!(
        matches!(
            decide_action(&context, repo.path(), &action, Sensitivity::Routine),
            Decision::Deny(_)
        ),
        "an expired stand-down still opened the push path"
    );
}

#[test]
fn a_pre_push_hook_nothing_can_read_is_not_a_repository_with_no_hook() {
    // `state` read the hook with `.ok()`, so a `pre-push` that is there and will
    // not open arrived as `Absent` — and `Absent` falls into the arm that
    // **writes**. The distinction between `Foreign` and `Absent` exists for one
    // reason: not replacing a file somebody relies on. A file nothing can read
    // is the case where whose it is cannot be established at all, and it was the
    // one classified as nobody's.
    //
    // The `Chained` arm above records what that costs, reached by the other
    // door: "this fell into the `_` arm and was overwritten … the `npx
    // lint-staged` line it told you to keep is gone."
    //
    // What an operator saw before: `estigia guard` blaming write permission on
    // the hooks *directory* — which is writable — and `doctor` reporting the
    // push guard simply not installed.
    let repository = repository();
    let repo = repository.path();
    let hooks = hooks_directory(repo).expect("a real repository has a hooks directory");
    let hook = hooks.join(HOOK);
    let executable = Path::new("estigia");

    // The floor first: with nothing there, this still installs. A fix that made
    // every repository unreadable would pass every assertion below.
    assert_eq!(state(repo), State::Absent, "an empty repository had a hook");
    assert_eq!(
        install(repo, executable, false).expect("an empty repository takes the guard"),
        State::Installed
    );
    assert_eq!(state(repo), State::Installed);
    std::fs::remove_file(&hook).expect("clear the way");

    // A directory in its place fails the read with something other than
    // `NotFound`, on every platform — and it carries something, so a write that
    // went through would be visible.
    std::fs::create_dir_all(&hook).expect("something unreadable in its place");
    std::fs::write(hook.join("theirs.txt"), "somebody else's").expect("their file");

    assert_eq!(
        state(repo),
        State::Unreadable,
        "a hook nothing can read was reported as no hook at all"
    );

    let refusal =
        install(repo, executable, false).expect_err("a hook nothing can identify was written over");
    assert_eq!(refusal.code, "push-hook-unreadable");
    assert!(
        refusal.to_string().contains("human-authority"),
        "the decision was not left with the person who owns the file: {refusal}"
    );
    assert!(
        hook.join("theirs.txt").exists(),
        "what was there did not survive the install that refused"
    );

    // And it is not taken out either: nothing read it, so nothing can say it was
    // ever ours.
    assert_eq!(
        uninstall(repo, false).expect("uninstall does not fail on somebody else's file"),
        Removal::LeftAlone(State::Unreadable)
    );
    assert!(
        hook.join("theirs.txt").exists(),
        "an uninstall removed a hook it could not identify"
    );
}

#[test]
fn a_directory_that_is_not_a_repository_says_so_and_names_what_is_wanted() {
    let root = tempfile::tempdir().expect("a temporary root");
    let refusal = hooks_directory(root.path()).unwrap_err();
    assert_eq!(refusal.code, "not-a-repository");
    assert!(refusal.to_string().contains("operator-knowledge"));
}

#[test]
fn installing_writes_a_hook_that_defers_everything_to_the_binary() {
    let repo = repository();
    assert_eq!(state(repo.path()), State::Absent);

    let executable = Path::new("/usr/local/bin/estigia");
    assert_eq!(
        install(repo.path(), executable, false).expect("the guard installs"),
        State::Installed
    );
    assert_eq!(state(repo.path()), State::Installed);

    let written = std::fs::read_to_string(
        hooks_directory(repo.path())
            .expect("a hooks directory")
            .join(HOOK),
    )
    .expect("the hook is readable");
    assert!(written.contains(MARKER), "the hook says who wrote it");
    assert!(
        written.contains("hook pre-push"),
        "the hook defers to the binary rather than deciding for itself"
    );
    // Everything it decides is *whether the binary answered*; everything it
    // knows about claims is nothing, so a rule that changes needs no reinstall.
    //
    // Measured on the lines that do something rather than on the file's length.
    // The count used to include comments, which punished explaining why the
    // script exists — a proxy that cannot tell prose from logic will eventually
    // be satisfied by deleting the prose.
    let doing: Vec<&str> = written
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert!(
        doing.len() < 10,
        "the hook grew logic:
{}",
        doing.join(
            "
"
        )
    );
    // And none of it is the gate's. A word from this list in here means a
    // decision lives in two places, and the shell copy is the one that goes
    // stale.
    for word in [
        "claim", "issue", "worktree", "allow", "deny", "verify", "tracker",
    ] {
        assert!(
            !doing.iter().any(|line| line.contains(word)),
            "the hook names `{word}`, so it is deciding rather than asking"
        );
    }
}

#[test]
fn somebody_else_s_hook_is_never_replaced() {
    // A `pre-push` is somebody's test runner or secret scanner as often as it
    // is nothing. Replacing it takes away a check they rely on to add one they
    // did not ask for.
    let repo = repository();
    let hooks = hooks_directory(repo.path()).expect("a hooks directory");
    std::fs::create_dir_all(&hooks).expect("create the hooks directory");
    let theirs = "#!/bin/sh\nexec ./scripts/scan-for-secrets\n";
    std::fs::write(hooks.join(HOOK), theirs).expect("write their hook");

    assert_eq!(state(repo.path()), State::Foreign);
    let refusal = install(repo.path(), Path::new("/usr/local/bin/estigia"), false).unwrap_err();
    assert_eq!(refusal.code, "push-hook-belongs-to-somebody-else");
    assert!(
        refusal.to_string().contains("chain"),
        "it must name the way out: {refusal}"
    );
    assert_eq!(
        std::fs::read_to_string(hooks.join(HOOK)).expect("their hook survives"),
        theirs
    );
}

#[test]
fn uninstalling_removes_ours_and_leaves_theirs() {
    let repo = repository();
    let hooks = hooks_directory(repo.path()).expect("a hooks directory");

    install(repo.path(), Path::new("/usr/local/bin/estigia"), false).expect("installs");
    assert_eq!(
        uninstall(repo.path(), false).expect("uninstalls"),
        Removal::Taken
    );
    assert!(!hooks.join(HOOK).exists());

    let theirs = "#!/bin/sh\nexit 0\n";
    std::fs::write(hooks.join(HOOK), theirs).expect("write their hook");
    // Reported, not refused: an uninstall that stops on somebody else's file
    // has not failed.
    assert_eq!(
        uninstall(repo.path(), false).expect("uninstalls"),
        Removal::LeftAlone(State::Foreign)
    );
    assert_eq!(
        std::fs::read_to_string(hooks.join(HOOK)).expect("their hook survives"),
        theirs
    );
}

#[test]
fn a_dry_run_writes_no_hook() {
    let repo = repository();
    install(repo.path(), Path::new("/usr/local/bin/estigia"), true).expect("the plan is produced");
    assert_eq!(state(repo.path()), State::Absent);
}

#[test]
fn a_push_from_a_checkout_nobody_holds_is_not_estigia_s_business() {
    // Refusing here would make the guard a lock on the operator's own work,
    // which is not workflow authority.
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    assert_eq!(
        decide(&context(root.path(), &repo), &repo, &[]).decision,
        Decision::Outside(crate::harness::Aside::NothingSworn)
    );
}

#[test]
fn a_push_from_a_checkout_one_run_holds_is_verified_at_the_boundary() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.repo_dir = Some(repo.clone());
    // Inside the renewal window, which a boundary must never ride on.
    run.mark_verified();
    session::store(&context.state_root, &run).expect("the pointer writes");

    let bin = scripted_gh();
    let script = answers(&[closed_issue(12)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    let Decision::Deny(refusal) = decision else {
        panic!("a push rode on a cached answer: {decision:?}");
    };
    assert_eq!(
        refusal.code, "issue-not-open",
        "the boundary refusal was not the tracker's own answer: {refusal:?}"
    );
    assert!(
        refusal.message.contains("#12"),
        "the refusal did not name the issue it read: {refusal:?}"
    );
}

#[test]
fn a_push_the_worktree_of_a_claim_is_covered_by_it() {
    // The delivery is pushed from the isolated checkout, not from where the
    // claim was made.
    let root = tempfile::tempdir().expect("a temporary root");
    let worktree = root.path().join("trees").join("issue-12");
    std::fs::create_dir_all(&worktree).expect("create the worktree");
    let context = context(root.path(), &worktree);

    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.repo_dir = Some(root.path().join("repo"));
    run.worktree = Some(worktree.clone());
    session::store(&context.state_root, &run).expect("the pointer writes");

    // It reaches the tracker rather than standing aside: the transport is
    // dispatched in-process, so the deny is the tracker's own answer about
    // issue #12 — which is the proof it looked.
    let bin = scripted_gh();
    let script = answers(&[closed_issue(12)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &worktree, &[]).decision
    });
    let Decision::Deny(refusal) = decision else {
        panic!("a covered worktree push did not reach the tracker: {decision:?}");
    };
    assert_eq!(
        refusal.code, "issue-not-open",
        "the worktree refusal was not the tracker's own answer: {refusal:?}"
    );
    assert!(
        refusal.message.contains("#12"),
        "the refusal did not name the issue it read: {refusal:?}"
    );
}

#[test]
fn two_runs_holding_one_checkout_are_refused_rather_than_guessed_between() {
    // Picking one would be guessing which claim a push belongs to, and a wrong
    // guess authorises the wrong delivery.
    //
    // Reconciliation asks the tracker about both before this refusal is built
    // — see the `_ =>` arm's own note — so this now drives a `gh` that
    // answers `OPEN` for both issues, un-matched to whatever state each run
    // recorded. That is a real, successful read, and not the one answer this
    // reconciliation drops a holder on: two genuinely live holders stay two,
    // and the refusal still names both.
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    for (id, issue) in [("claude-first000", 12), ("claude-second00", 34)] {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo.clone());
        session::store(&context.state_root, &run).expect("the pointer writes");
    }

    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    match decision {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "several-runs-hold-this-checkout");
            assert!(refusal.message.contains("#12"));
            assert!(refusal.message.contains("#34"));
            // Real ambiguity: the tracker says both are live, so nothing here
            // may be offered `estigia release` for — releasing either would
            // put down a claim that is not this refusal's to end. There is no
            // CLI verb for isolating a checkout the way `start_branch` does
            // (that tool is MCP-only), so the honest resolution names no
            // command rather than a guess.
            assert!(
                !refusal.resolution.to_string().contains("estigia release"),
                "two genuinely live claims were offered a release command: {}",
                refusal.resolution
            );
        }
        other => panic!("two live claims were guessed between: {other:?}"),
    }
}

#[test]
fn an_assisted_holder_among_the_refused_leaves_the_refusal_naming_no_release() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);
    for (id, issue, assisted) in [
        ("claude-first000", 12, true),
        ("claude-second00", 34, false),
    ] {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo.clone());
        run.worktree = Some(repo.clone());
        run.assisted = assisted;
        session::store(&context.state_root, &run).expect("the pointer writes");
    }
    let bin = scripted_gh();
    let script = answers(&[not_current_live_holder_issue(34)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    let Decision::Deny(refusal) = decision else {
        panic!("two holders were guessed between: {decision:?}");
    };
    assert_eq!(refusal.code, "several-runs-hold-this-checkout");
    assert!(
        !refusal.resolution.to_string().contains("estigia release"),
        "a command assisted refuses was named: {}",
        refusal.resolution
    );
}

/// Two holders, each isolated into its own worktree on its own branch — the
/// shape in question, and the fixture every test below narrows or
/// fails to narrow.
fn two_holders_with_worktrees() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    GateContext,
    PathBuf,
    PathBuf,
) {
    let repo = repository_with_commit();
    let root = tempfile::tempdir().expect("a state root");
    let context = context(root.path(), repo.path());

    let worktree_a = root.path().join("wt-a");
    let worktree_b = root.path().join("wt-b");
    add_worktree(repo.path(), &worktree_a, "branch-a");
    add_worktree(repo.path(), &worktree_b, "branch-b");

    let mut run_a = session::Run::new("claude-aaaa1111".to_owned());
    run_a.issue = Some(crate::identity::Identity::Number(12));
    run_a.repo_dir = Some(repo.path().to_path_buf());
    run_a.worktree = Some(worktree_a.clone());
    session::store(&context.state_root, &run_a).expect("the pointer writes");

    let mut run_b = session::Run::new("claude-bbbb2222".to_owned());
    run_b.issue = Some(crate::identity::Identity::Number(34));
    run_b.repo_dir = Some(repo.path().to_path_buf());
    run_b.worktree = Some(worktree_b.clone());
    session::store(&context.state_root, &run_b).expect("the pointer writes");

    (repo, root, context, worktree_a, worktree_b)
}

#[test]
fn a_push_naming_one_holders_branch_is_decided_on_that_holders_claim() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();

    let bin = scripted_gh();

    // Red today: no refs reach the decision, so both holders survive and the
    // push is refused rather than guessed between.
    let ambiguous = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);
    let refused = crate::test_env::with_scripted_gh(bin.path(), &ambiguous, || {
        decide(&context, repo.path(), &[]).decision
    });
    match refused {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => panic!("two live holders with no refs did not refuse: {other:?}"),
    }

    // Green after: the local ref names holder A's branch, so the checkout's
    // worktree selects A and only A — and the decision is then made on *that
    // holder's own claim*: run A was never marked verified, so `Allow` here
    // can only come from a fresh dispatch to the tracker, at the boundary,
    // with no renewal window to ride on. The script answers only for issue
    // #12, the one holder A actually holds; a decision that reached holder
    // B's claim instead would find nothing scripted for issue #34 and deny.
    let refs = vec!["refs/heads/branch-a".to_owned()];
    crate::transport::store::forget_scratch_store();
    let narrowed = answers(&[live_holder_issue("claude-aaaa1111", 12, "in-progress")]);
    let adjudication = crate::test_env::with_scripted_gh(bin.path(), &narrowed, || {
        decide(&context, repo.path(), &refs)
    });
    assert_eq!(
        adjudication.holder.as_deref(),
        Some("claude-aaaa1111"),
        "the push naming branch-a was not resolved to the holder whose worktree is on it"
    );
    assert!(
        matches!(adjudication.decision, Decision::Allow(_)),
        "the narrowed push was not decided on holder A's own claim at the boundary: {:?}",
        adjudication.decision
    );
}

#[test]
fn a_push_naming_a_branch_neither_worktree_is_on_still_refuses() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);

    let refs = vec!["refs/heads/branch-c".to_owned()];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "several-runs-hold-this-checkout");
            assert!(
                refusal.message.contains("refs/heads/branch-c"),
                "the refusal did not name the ref it tried and failed to resolve on: {}",
                refusal.message
            );
        }
        other => panic!("a ref naming nobody's branch was guessed between anyway: {other:?}"),
    }
}

#[test]
fn a_push_with_no_refs_at_all_refuses_exactly_as_before_narrowing_existed() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);

    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &[]).decision
    });
    match decision {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "several-runs-hold-this-checkout");
            assert!(
                !refusal.message.contains("this push named"),
                "a refusal with no refs to try claimed it tried some: {}",
                refusal.message
            );
        }
        other => panic!("two live holders with no refs did not refuse: {other:?}"),
    }
}

#[test]
fn a_delete_push_names_no_branch_and_selects_nobody() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);

    // Git's own literal for a branch deletion: the local side of the line is
    // `(delete)`, not a ref, so it must not be read as one.
    let refs = vec!["(delete)".to_owned()];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => panic!("a `(delete)` local ref selected a holder: {other:?}"),
    }
}

#[test]
fn an_unreadable_worktree_listing_does_not_narrow() {
    // `repo_dir` is not a git repository at all, so `git worktree list` fails
    // and the fail-closed rule in `narrow_to_pushed_branch` applies: an
    // unreadable control surface permits no narrowing, so both holders stay
    // counted and the refusal stands, exactly as it would with no refs.
    let root = tempfile::tempdir().expect("a state root");
    let repo = root.path().join("not-a-repository");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    for (id, issue) in [("claude-aaaa1111", 12), ("claude-bbbb2222", 34)] {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo.clone());
        session::store(&context.state_root, &run).expect("the pointer writes");
    }

    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);
    let refs = vec!["refs/heads/branch-a".to_owned()];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => panic!("an unreadable worktree listing narrowed a refusal anyway: {other:?}"),
    }
}

/// A second worktree checked out on a branch some other worktree already has
/// — `git worktree add --force` is one of the two ways a severe finding
/// names to reach this, and this fixture reaches it directly.
fn add_worktree_forced(repo: &Path, path: &Path, branch: &str) {
    let added = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "--force"])
        .arg(path)
        .arg(branch)
        .output()
        .is_ok_and(|output| output.status.success());
    assert!(
        added,
        "git could not check {branch} out a second time — this fixture needs `--force` to work"
    );
}

/// The branch `repository_with_commit`'s own base checkout is on — read
/// rather than assumed, because `init.defaultBranch` is local git
/// configuration and this crate does not steer it.
fn default_branch(repo: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .expect("git answers for the fixture's own repository");
    assert!(output.status.success(), "the base checkout has no branch");
    String::from_utf8(output.stdout)
        .expect("a branch name is UTF-8")
        .trim()
        .to_owned()
}

#[test]
fn a_ref_that_resolves_to_nobody_still_refuses_alongside_one_that_would_narrow_alone() {
    // The defect demonstrated against the built binary: `refs/heads/main`
    // alone refused, and `refs/heads/main` *plus* a line naming a holder's
    // own branch let the push through — a union over whatever resolved,
    // judged only by the surviving count. The base checkout's own branch is
    // a real registered worktree (`repository_with_commit` starts on it) but
    // nobody in `holders` owns it, so this ref has to fail the whole
    // narrowing closed rather than being dropped while `branch-a` decides
    // alone.
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let base_branch = default_branch(repo.path());

    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);
    let refs = vec![
        format!("refs/heads/{base_branch}"),
        "refs/heads/branch-a".to_owned(),
    ];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => panic!(
            "a ref naming nobody's worktree let a second, resolving ref decide anyway: {other:?}"
        ),
    }
}

#[test]
fn two_refs_landing_on_different_holders_still_refuses() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);
    let refs = vec![
        "refs/heads/branch-a".to_owned(),
        "refs/heads/branch-b".to_owned(),
    ];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => {
            panic!(
                "two refs naming two different holders' own branches were guessed between: {other:?}"
            )
        }
    }
}

#[test]
fn two_refs_landing_on_the_same_holder_narrows() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let bin = scripted_gh();
    let script = answers(&[live_holder_issue("claude-aaaa1111", 12, "in-progress")]);
    let refs = vec![
        "refs/heads/branch-a".to_owned(),
        "refs/heads/branch-a".to_owned(),
    ];
    let adjudication = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs)
    });
    assert_eq!(
        adjudication.holder.as_deref(),
        Some("claude-aaaa1111"),
        "two refs both naming holder A's branch did not narrow to A"
    );
    assert!(
        matches!(adjudication.decision, Decision::Allow(_)),
        "two refs agreeing on one holder did not reach that holder's own claim: {:?}",
        adjudication.decision
    );
}

#[test]
fn a_tag_ref_alongside_a_branch_ref_still_refuses() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);
    let refs = vec!["refs/tags/v1".to_owned(), "refs/heads/branch-a".to_owned()];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => {
            panic!(
                "a tag ref alongside a resolving branch ref let narrowing through anyway: {other:?}"
            )
        }
    }
}

#[test]
fn a_delete_ref_alongside_a_branch_ref_still_refuses() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);
    let refs = vec!["(delete)".to_owned(), "refs/heads/branch-a".to_owned()];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => panic!(
            "a `(delete)` ref alongside a resolving branch ref let narrowing through anyway: \
             {other:?}"
        ),
    }
}

#[test]
fn a_branch_checked_out_in_two_worktrees_does_not_narrow() {
    // Severe 2: the registry itself no longer names one worktree for
    // `branch-a`, reached with `git worktree add --force` exactly as the
    // finding names.
    let (repo, root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();
    let worktree_c = root.path().join("wt-c");
    add_worktree_forced(repo.path(), &worktree_c, "branch-a");

    let bin = scripted_gh();
    let script = answers(&[open_but_unmatched_issue(12), open_but_unmatched_issue(34)]);
    let refs = vec!["refs/heads/branch-a".to_owned()];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => panic!("a branch checked out in two worktrees narrowed to one anyway: {other:?}"),
    }
}

#[test]
fn two_holders_recording_one_worktree_does_not_narrow() {
    // Severe 2's other shape: `parse_worktree_listing` refuses a repeated
    // **path** from git's own registry, but nothing stops two of this
    // machine's own pointers from recording one worktree — a fact about
    // Estigia's own pointer writes, not git's registry, and reproduced
    // directly here on the pointer this crate actually reads, per
    // `skill/references/repository-delivery.md`'s "registration is not
    // ownership".
    let (repo, _root, context, worktree_a, _worktree_b) = two_holders_with_worktrees();
    let mut run_c = session::Run::new("claude-cccc3333".to_owned());
    run_c.issue = Some(crate::identity::Identity::Number(56));
    run_c.repo_dir = Some(repo.path().to_path_buf());
    run_c.worktree = Some(worktree_a);
    session::store(&context.state_root, &run_c).expect("the pointer writes");

    let bin = scripted_gh();
    let script = answers(&[
        open_but_unmatched_issue(12),
        open_but_unmatched_issue(34),
        open_but_unmatched_issue(56),
    ]);
    let refs = vec!["refs/heads/branch-a".to_owned()];
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, repo.path(), &refs).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "several-runs-hold-this-checkout"),
        other => {
            panic!("two holders recording one worktree narrowed to one anyway: {other:?}")
        }
    }
}

#[test]
fn release_is_offered_by_what_the_tracker_says_not_by_a_pointers_worktree() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    // Live, and claimed before `start_branch` ever ran: no worktree recorded.
    let mut live_no_worktree = session::Run::new("claude-live00000".to_owned());
    live_no_worktree.issue = Some(crate::identity::Identity::Number(12));
    live_no_worktree.repo_dir = Some(repo.clone());
    session::store(&context.state_root, &live_no_worktree).expect("the pointer writes");

    // A worktree on disk from a session the timeline holds no acquisition
    // for at all — the shape a crash right after `start_branch` leaves.
    let worktree = root.path().join("wt-dead");
    std::fs::create_dir_all(&worktree).expect("create the worktree");
    let mut dead_with_worktree = session::Run::new("claude-dead000000".to_owned());
    dead_with_worktree.issue = Some(crate::identity::Identity::Number(34));
    dead_with_worktree.repo_dir = Some(repo.clone());
    dead_with_worktree.worktree = Some(worktree);
    session::store(&context.state_root, &dead_with_worktree).expect("the pointer writes");

    let bin = scripted_gh();
    let script = answers(&[
        open_but_unmatched_issue(12),
        not_current_live_holder_issue(34),
    ]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    match decision {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "several-runs-hold-this-checkout");
            let resolution = refusal.resolution.to_string();
            assert!(
                resolution.contains("claude-dead000000"),
                "the run the timeline holds no acquisition for was not offered for release, \
                 even though it names a worktree: {resolution}"
            );
            assert!(
                !resolution.contains("claude-live00000"),
                "a live run with no worktree of its own was offered for release: {resolution}"
            );
        }
        other => panic!("the tracker's own answer was not what decided this: {other:?}"),
    }
}

#[test]
fn a_stale_holder_is_dropped_and_the_live_one_s_single_holder_path_runs() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    for (id, issue) in [("claude-stale0000", 12), ("claude-live00000", 34)] {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo.clone());
        session::store(&context.state_root, &run).expect("the pointer writes");
    }

    let bin = scripted_gh();
    let script = answers(&[closed_issue(12), open_but_unmatched_issue(34)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    match decision {
        Decision::Deny(refusal) => {
            assert_ne!(
                refusal.code, "several-runs-hold-this-checkout",
                "a stale holder was not dropped, so the survivor never reached its own path: \
                 {refusal:?}"
            );
            // The survivor's own path, through `gate`: read, and answered
            // `unexpected-state` rather than the several-holder refusal — the
            // proof this is `1 =>`'s call into `gate`, not `_ =>`'s.
            assert_eq!(refusal.code, "unexpected-state");
        }
        other => panic!("the live holder's own path did not run: {other:?}"),
    }
}

#[test]
fn a_stale_holder_dropped_to_one_live_holder_lets_gate_allow() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    for (id, issue) in [("claude-stale0000", 12), ("claude-live00000", 34)] {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo.clone());
        session::store(&context.state_root, &run).expect("the pointer writes");
    }

    let bin = scripted_gh();
    let script = answers(&[
        closed_issue(12),
        live_holder_issue("claude-live00000", 34, "in-progress"),
    ]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    assert!(
        matches!(decision, Decision::Allow(_)),
        "a stale holder dropped to a fully verified single survivor did not allow the write: \
         {decision:?}"
    );
}

#[test]
fn both_holders_stale_leaves_nothing_holding_the_checkout() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    for (id, issue) in [("claude-stale0001", 12), ("claude-stale0002", 34)] {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo.clone());
        session::store(&context.state_root, &run).expect("the pointer writes");
    }

    let bin = scripted_gh();
    let script = answers(&[closed_issue(12), closed_issue(34)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    assert_eq!(
        decision,
        Decision::Outside(crate::harness::Aside::NothingSworn),
        "two pointers whose issues are both closed still refused by name: {decision:?}"
    );
}

/// Same two stale holders as `both_holders_stale_leaves_nothing_holding_the_checkout`,
/// but reached with a local ref naming one holder's branch instead of no refs
/// at all. Narrowing picks that holder before the `ClosedIssue` reconciliation
/// loop ever runs, so its own staleness is never classified there; the
/// survivor is decided on `gate`'s own read instead, and denies rather than
/// leaving the checkout unheld.
#[test]
fn narrowing_before_the_tracker_can_deny_what_reconciliation_would_have_allowed() {
    let (repo, _root, context, _worktree_a, _worktree_b) = two_holders_with_worktrees();

    let bin = scripted_gh();
    let closed = answers(&[closed_issue(12), closed_issue(34)]);
    let unnarrowed = crate::test_env::with_scripted_gh(bin.path(), &closed, || {
        decide(&context, repo.path(), &[]).decision
    });
    assert_eq!(
        unnarrowed,
        Decision::Outside(crate::harness::Aside::NothingSworn),
        "two closed holders with no refs to narrow on did not read as nobody's: {unnarrowed:?}"
    );

    // The same two answers the unnarrowed call was given, so what changes
    // between the two halves is the decision and not how far the script got.
    let refs = vec!["refs/heads/branch-a".to_owned()];
    let narrowed = crate::test_env::with_scripted_gh(bin.path(), &closed, || {
        decide(&context, repo.path(), &refs).decision
    });
    match narrowed {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "issue-not-open"),
        other => panic!(
            "narrowing to a stale holder's branch did not reach that holder's own refusal: \
             {other:?}"
        ),
    }
}

#[test]
fn a_tracker_read_that_fails_keeps_every_holder_counted() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    for (id, issue) in [("claude-first000", 12), ("claude-second00", 34)] {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo.clone());
        session::store(&context.state_root, &run).expect("the pointer writes");
    }

    let bin = scripted_gh();
    let script = answers(&[unreachable_tracker_answer()]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    match decision {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "several-runs-hold-this-checkout");
            assert!(refusal.message.contains("#12"));
            assert!(refusal.message.contains("#34"));
            // Nothing here was ever read, let alone confirmed live — the
            // resolution must not claim otherwise.
            assert!(
                !refusal.resolution.to_string().contains("live"),
                "a read that failed was described as though it had confirmed a claim live: {}",
                refusal.resolution
            );
        }
        other => panic!("a read that failed dropped a holder instead of keeping it: {other:?}"),
    }
}

#[test]
fn a_single_stale_holder_still_refuses_through_its_own_issue_not_open() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    let mut run = session::Run::new("claude-alone0000".to_owned());
    run.issue = Some(crate::identity::Identity::Number(99));
    run.repo_dir = Some(repo.clone());
    session::store(&context.state_root, &run).expect("the pointer writes");

    let bin = scripted_gh();
    let script = answers(&[closed_issue(99)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "issue-not-open"),
        other => panic!("a single stale holder no longer refuses on its own answer: {other:?}"),
    }
}

#[test]
fn doctor_reports_the_guard_without_ever_installing_it() {
    // A health report that quietly wrote a hook into whatever repository
    // somebody was standing in would be the opposite of what read-only means.
    let repo = repository();
    let before = state(repo.path());
    let checks = crate::harness::doctor::examine(Some(repo.path()), repo.path());
    assert_eq!(state(repo.path()), before, "doctor changed something");

    let guard = checks
        .iter()
        .find(|check| check.name == "push-guard")
        .expect("the push guard is reported");
    // Absent is not broken: a repository where nobody works under Estigia has
    // no reason to carry it, and reporting every checkout as broken teaches
    // people to ignore the report.
    assert!(!guard.health.is_broken());
    assert!(guard.about.contains("pre-push"));
}

#[test]
fn doctor_tells_an_installed_guard_from_an_absent_one() {
    let repo = repository();
    let describe = |repo: &Path| {
        crate::harness::doctor::examine(Some(repo), repo)
            .into_iter()
            .find(|check| check.name == "push-guard")
            .map(|check| format!("{:?}", check.health))
            .unwrap_or_default()
    };
    let absent = describe(repo.path());
    install(repo.path(), Path::new("/usr/local/bin/estigia"), false).expect("installs");
    let installed = describe(repo.path());

    assert!(absent.contains("not installed"), "{absent}");
    assert!(installed.contains("refused"), "{installed}");
    assert_ne!(absent, installed);
}

#[test]
fn a_path_the_shell_could_read_as_a_command_is_not_read_as_one() {
    // The hook is a `sh` script and the binary's path is interpolated into it.
    // It was interpolated inside **double** quotes, where `sh` still expands
    // `$(…)`, `` `…` `` and `${…}` — so a checkout whose binary lived under a
    // directory named with one of those ran it on every push and then looked
    // for the gate at whatever came back. `$` and a backtick are legal in a
    // directory name on every platform this installs on, Windows included.
    //
    // Run rather than read: the question is what a shell does with the line,
    // so a shell is what answers it.
    let root = tempfile::tempdir().expect("a temporary root");
    let marker = root.path().join("substitution-ran");
    let hostile = format!(
        "/nonexistent/$(touch {})/estigia",
        marker.display().to_string().replace('\\', "/")
    );

    let script = super::script(Path::new(&hostile));
    let file = root.path().join("pre-push");
    std::fs::write(&file, &script).expect("the script is written");

    let Ok(shell) = std::process::Command::new("sh")
        .arg(&file)
        .current_dir(root.path())
        .output()
    else {
        eprintln!("SKIPPED: no POSIX shell here.");
        return;
    };
    assert!(
        !marker.exists(),
        "the hook ran a command out of the binary's own path: {script}"
    );
    // And it still let the push through, because a hook that breaks does not
    // deny — the binary is not there either way.
    assert!(shell.status.success(), "{:?}", shell.status);

    // A quote in the path closes nothing: this is the one character single
    // quoting has to handle itself.
    let awkward = super::script(Path::new("/tmp/it's here/estigia"));
    let file = root.path().join("quoted");
    std::fs::write(&file, &awkward).expect("the script is written");
    let Ok(shell) = std::process::Command::new("sh")
        .arg(&file)
        .current_dir(root.path())
        .output()
    else {
        return;
    };
    let said = String::from_utf8_lossy(&shell.stderr);
    assert!(
        !said.contains("unexpected EOF") && !said.contains("syntax error"),
        "a quote in the path broke the script: {said}\n{awkward}"
    );
    assert!(shell.status.success(), "{:?}: {said}", shell.status);
}

#[test]
fn a_push_is_not_blocked_by_estigia_being_gone() {
    // The module says "a hook that breaks does not deny". At this one boundary
    // it did: the script used `exec`, so git was handed whatever the shell
    // produced — and a shell that cannot find the binary produces 127, which
    // git blocks on. Deleting Estigia, moving it, or a `cargo clean` left a
    // repository nobody could push from, with a fix nobody would guess.
    //
    // Read from the installed script rather than asserted about it: the shell is
    // what runs, so the shell is what has to be checked.
    let repo = repository();
    let missing = repo.path().join("estigia-is-not-here");
    install(repo.path(), &missing, false).expect("the guard installs");
    let hooks = hooks_directory(repo.path()).expect("the hooks directory resolves");

    let script = std::fs::read_to_string(hooks.join(HOOK)).expect("the hook is written");
    assert!(
        !script.contains("exec "),
        "the script still hands git the shell's own status"
    );
    // Only the two codes Estigia defines are decisions.
    assert!(script.contains("1|2) exit"), "{script}");
    assert!(script.contains("exit 0"), "{script}");
    // And the code that means "it did not answer" is told apart from the one
    // that means "it answered, and the answer was yes". Both let the push
    // through and only one of them is a measurement.
    assert!(script.contains("  0) exit 0 ;;"), "{script}");
    assert!(
        script.contains("echo "),
        "the script passes unchecked in silence: {script}"
    );

    // And it actually behaves that way, run as the shell git would run it.
    let Ok(shell) = std::process::Command::new("sh")
        .arg(hooks.join(HOOK))
        .current_dir(repo.path())
        .output()
    else {
        // No POSIX shell here; the assertions above still hold the contract.
        return;
    };
    assert!(
        shell.status.success(),
        "a missing binary blocked the push: {:?}",
        shell.status
    );
    // Let through **and said so**. `git_hook` states the rule in those words
    // for the working directory it cannot read — "doing it without a word is
    // not the same stance; it is the silence the ledger check exists to find"
    // — and this, the case the script exists for, was the silent one. A push
    // that went out unmeasured looked exactly like one the gate had approved.
    let said = String::from_utf8_lossy(&shell.stderr);
    assert!(
        said.contains("estigia:") && said.contains("unchecked"),
        "the push went out unmeasured without a word: {said:?}"
    );
}

#[test]
fn what_installing_the_guard_says_is_narrower_than_every_push() {
    // The install used to print "a push this repository cannot justify is now
    // refused, whoever types it", which overstated what was installed. The claim
    // is pinned to the behaviour that backs it: a live claim, or no claim at all.
    let source = crate::source_text::code_of(include_str!("../../cli/mod.rs"));
    let doctor = crate::source_text::code_of(include_str!("../doctor.rs"));

    for (where_it_is, text) in [("the install", &source), ("doctor", &doctor)] {
        // The sentence that overstated it is gone, and cannot come back by
        // somebody shortening the honest one.
        assert!(
            !text.contains("a push this repository cannot justify is now refused")
                && !text.contains("a push this repository cannot justify is refused"),
            "{where_it_is} claims every push is checked"
        );
    }
    // And what replaced it names the condition: a live claim.
    assert!(
        source.contains(
            "with no claim on a checkout, this refuses every push and the agent gate refuses"
        ),
        "the install does not say when it refuses nothing"
    );
    assert!(
        doctor.contains(
            "with no claim, the hook refuses every push here and the agent gate every commit"
        ),
        "doctor does not say when the guard refuses nothing"
    );
}

#[test]
fn an_unclaimed_checkout_is_outside_the_guard_rather_than_denied() {
    // The behaviour the message now describes, held here so the two cannot
    // drift apart: with no run holding this checkout, the push is not this
    // gate's business and goes through.
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = tempfile::tempdir().expect("a checkout");
    let context = context(root.path(), repo.path());
    assert!(
        matches!(
            decide(&context, repo.path(), &[]).decision,
            crate::harness::Decision::Outside(crate::harness::Aside::NothingSworn)
        ),
        "an unclaimed checkout was decided rather than left alone"
    );
}

#[test]
fn the_hooks_path_is_printed_in_this_platforms_own_separators() {
    // Git answers in its own, always forward slashes: `git rev-parse --git-path
    // hooks` returns `.git/hooks`. Joined onto a Windows checkout that gave
    // `C:\work\repo\.git/hooks\pre-push` — which every API here accepts, and
    // which `estigia guard` prints. A path somebody is shown is one they will
    // paste into a shell.
    let repo = repository();
    let hooks = hooks_directory(repo.path()).expect("a repository has a hooks directory");
    let shown = hooks.join(HOOK).display().to_string();
    if cfg!(windows) {
        assert!(
            !shown.contains('/'),
            "the path shown to an operator mixes separators: {shown}"
        );
    }
    // And it is still the directory git named, not a guess.
    assert!(shown.ends_with(HOOK), "{shown}");
    assert!(hooks.ends_with("hooks"), "{}", hooks.display());
}

#[test]
fn a_hook_somebody_else_wrote_that_calls_estigia_is_the_guard_running() {
    // Estigia's own answer to finding a hook it did not write is *chain
    // `estigia hook pre-push` from it*. An operator who does that has the guard
    // in force on every push — and was told, by `doctor` and by `status`, that
    // a pre-push hook was there which Estigia had not written, with the same
    // advice to chain it. The instruction and the report disagreed about work
    // the operator had already done.
    //
    // The predicate rather than `state`, because the population is *which hook
    // texts hand the push over*; reading the file is the easy half.
    for call in [
        "estigia hook pre-push",
        "\"C:\\Program Files\\estigia.exe\" hook pre-push || exit $?",
        "/usr/local/bin/estigia hook pre-push",
        "exec estigia hook pre-push \"$@\"",
        // The shape Estigia writes itself, quoted by `paths::shell_quoted`,
        // for a home directory holding an apostrophe. Dropping quoted runs to
        // tell a mention from a call must not drop the call.
        r"'C:\Users\O'\''Brien\.cargo\bin\estigia.exe' hook pre-push",
    ] {
        assert!(
            super::calls_estigia(&format!("#!/bin/sh\necho mine\n{call}\n")),
            "{call} is the guard running"
        );
    }

    // And the same text through `state`, which is what the reports read. The
    // predicate being right is half of it: removing the arm that calls it left
    // this test passing, which is exactly the gap this second half closes.
    let repo = repository();
    {
        let hooks = repo.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks).expect("the hooks directory");
        write_runnable_hook(
            &hooks.join(super::HOOK),
            "#!/bin/sh
echo mine
estigia hook pre-push
",
        );
        assert_eq!(super::state(repo.path()), super::State::Chained);
        write_runnable_hook(
            &hooks.join(super::HOOK),
            "#!/bin/sh
echo mine
",
        );
        assert_eq!(super::state(repo.path()), super::State::Foreign);

        // The same text, unrunnable. `Inert` is the state this enum was added
        // for -- installed, looks installed, decides nothing -- and until this
        // line nothing exercised it on a platform that has a file mode.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = hooks.join(super::HOOK);
            std::fs::write(
                &path,
                "#!/bin/sh
echo mine
estigia hook pre-push
",
            )
            .expect("their hook, chained but not runnable");
            // The mode has to be cleared explicitly. `fs::write` truncates an
            // existing file and leaves its permissions alone, so writing over
            // the runnable hook above kept 0o755 and this read `Chained` —
            // measured on the first CI run after the assertion was added.
            let mut mode = std::fs::metadata(&path).expect("the hook").permissions();
            mode.set_mode(0o644);
            std::fs::set_permissions(&path, mode).expect("the hook is made unrunnable");
            assert_eq!(
                super::state(repo.path()),
                super::State::Inert,
                "a hook git will not run is not a gate, however it reads"
            );
        }
    }

    for quiet in [
        "#!/bin/sh\necho mine\n",
        // Talking about it is not doing it.
        "#!/bin/sh\n# could chain estigia hook pre-push here one day\necho mine\n",
        // Another event of Estigia's is not this one.
        "#!/bin/sh\nestigia hook session-start\n",
        // Talking about it out loud is not doing it either, and this is the
        // shape that was counted. A team hook printing the instruction had
        // `doctor` answer `ok push-guard — in force from a hook Estigia did not
        // write` while nothing ran the gate: the push boundary reported as
        // gated, on the line an operator reads to decide their machine is in
        // order. The commented case above was caught by the `#` filter and
        // this one has no `#` anywhere.
        "#!/bin/sh\necho \"to enable the gate: estigia hook pre-push\" >&2\n",
        "#!/bin/sh\nprintf '%s\\n' 'estigia hook pre-push'\n",
        // Outside quotes, so the words are there — and `echo` runs the gate
        // exactly as much as the quoted spelling does.
        "#!/bin/sh\necho estigia hook pre-push\n",
        "#!/bin/sh\n/bin/echo estigia hook pre-push\n",
        // The other builtin that evaluates its arguments and does nothing.
        "#!/bin/sh\n: estigia hook pre-push\n",
        // And the shapes no list of printing commands can cover: the words
        // held as data. Whatever this line does, it is not running the gate.
        "#!/bin/sh\nADVICE=\"run estigia hook pre-push\"\n",
        "#!/bin/sh\ngrep -q 'estigia hook pre-push' .git/hooks/other || exit 1\n",
    ] {
        assert!(
            !super::calls_estigia(quiet),
            "{quiet:?} does not hand the push to Estigia"
        );
    }

    // And through `state`, because the predicate being right is half of it —
    // the sentence beside this one in the test above says so, and the same
    // reasoning applies to the population it now excludes.
    let repo = repository();
    {
        let hooks = repo.path().join(".git").join("hooks");
        std::fs::create_dir_all(&hooks).expect("the hooks directory");
        std::fs::write(
            hooks.join(super::HOOK),
            "#!/bin/sh\necho \"to enable: estigia hook pre-push\"\n",
        )
        .expect("their hook, only talking about it");
        assert_eq!(
            super::state(repo.path()),
            super::State::Foreign,
            "a hook that only prints the words was reported as the guard running"
        );
    }
}

#[test]
fn a_pointer_nothing_can_read_does_not_make_a_checkout_nobody_holds() {
    // `holdings` drops a pointer it cannot open or parse, and an empty list is
    // how this guard is told no claim covers the checkout — so one corrupt file
    // turned a claimed checkout into an unclaimed one and the push left through
    // `Outside`. Reachable across an upgrade: a pointer carrying a field an
    // older build has no default for fails to parse, and every push from that
    // checkout stops being checked.
    //
    // The rule is the project's own, from the single-run path: "this run's
    // record exists and cannot be read, so whether it holds an issue is
    // unknown". The all-runs path never got it.
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);
    std::fs::create_dir_all(&context.state_root).expect("the state directory");

    // The premise: with nothing there at all, this checkout is nobody's
    // business and stays that way.
    assert_eq!(
        decide(&context, &repo, &[]).decision,
        Decision::Outside(crate::harness::Aside::NothingSworn)
    );

    // A pointer that will not parse. Not empty, not absent — there.
    let pointer = context.state_root.join("claude-abcd1234.json");
    std::fs::write(&pointer, "{\"run_id\": ").expect("a torn pointer");
    assert_eq!(
        session::unreadable_holdings(&context.state_root).len(),
        1,
        "a pointer that will not parse was counted as one that says nothing"
    );
    let decision = decide(&context, &repo, &[]).decision;
    assert!(
        decision.denies(),
        "a push went out unchecked because a pointer would not open: {decision:?}"
    );

    // And it names the file, because "somewhere on this machine" is not a
    // thing anybody can act on.
    let Decision::Deny(refusal) = decision else {
        panic!("checked above");
    };
    assert_eq!(refusal.code, "run-pointers-unreadable");
    assert!(
        refusal.message.contains("claude-abcd1234.json"),
        "the file that cannot be read is not named: {}",
        refusal.message
    );

    // But only for something the harness watches. `estigia gate <tool>` reaches
    // this same function for whatever tool a caller names — a person or a
    // script, not only an agent's own plugin — so with the torn pointer still
    // there, denying whatever it did not recognise denied `estigia gate Read`
    // and every other unwatched tool until somebody found the file. The rule
    // is stated next door, about a payload rather than a pointer: *denying it
    // would deny reads*.
    assert_eq!(
        decide_action(
            &context,
            &repo,
            &crate::harness::Action::Untouched,
            crate::harness::Sensitivity::Routine,
        ),
        Decision::Outside(crate::harness::Aside::NotWatched),
        "a read was refused because a pointer belonging to some other run would not open"
    );

    // Readable again: back to nobody's business, so this refuses a fault and
    // not a checkout.
    std::fs::remove_file(&pointer).expect("take it away");
    assert_eq!(
        decide(&context, &repo, &[]).decision,
        Decision::Outside(crate::harness::Aside::NothingSworn)
    );
}

#[test]
fn installing_over_a_hook_that_already_chains_estigia_leaves_it_alone() {
    let repo = repository();
    let hooks = repo.path().join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).expect("the hooks directory");
    let path = hooks.join(super::HOOK);

    // Exactly the shape the refusal asks for: their checks, then Estigia.
    let theirs = "#!/bin/sh
npx lint-staged || exit 1
\"C:/estigia.exe\" hook pre-push || exit $?
";
    write_runnable_hook(&path, theirs);
    assert_eq!(super::state(repo.path()), super::State::Chained);

    let after = super::install(repo.path(), std::path::Path::new("C:/estigia.exe"), false)
        .expect("installing over a chained hook is not a failure");
    assert_eq!(
        after,
        super::State::Chained,
        "it reported something other than what is there"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("readable"),
        theirs,
        "`estigia guard` replaced the hook its own refusal told them to write"
    );

    // And the round trip leaves it theirs, which is the promise `uninstall`
    // already kept on its own.
    super::uninstall(repo.path(), false).expect("uninstalling is not a failure");
    assert_eq!(
        std::fs::read_to_string(&path).expect("readable"),
        theirs,
        "uninstall took away a hook Estigia never wrote"
    );

    // A dry run says the same and writes nothing either.
    let dry = super::install(repo.path(), std::path::Path::new("C:/estigia.exe"), true)
        .expect("a dry run is not a failure");
    assert_eq!(dry, super::State::Chained);
    assert_eq!(
        std::fs::read_to_string(&path).expect("readable"),
        theirs,
        "a dry run wrote"
    );
}

#[test]
fn a_verification_through_the_checkout_path_is_written_down() {
    let home = tempfile::tempdir().expect("a temporary home");
    let repo = tempfile::tempdir().expect("a checkout");
    let context = context(home.path(), repo.path());
    let root = context.state_root.clone();

    // Already inside the window, which is the one answer `gate` can give
    // without a transport. The *full* effect — a fresh tracker verification
    // surviving to the next call — needs a real binding and is not unit-tested
    // here; what is, is that an allowed decision through this path is written
    // down at all, which is what it was not.
    let mut run = super::session::Run::new("claude-probe".to_owned());
    run.issue = Some(crate::identity::Identity::Number(7));
    run.repo_dir = Some(repo.path().to_path_buf());
    run.mark_verified();
    super::session::store(&root, &run).expect("the pointer is written");
    let before = super::session::load(&root, "claude-probe").revision;

    // A routine write, decided through the checkout rather than through a
    // session — the path the plugin and the push hook share.
    let _ = super::decide_action(
        &context,
        repo.path(),
        &crate::harness::Action::Write {
            target: "src/x.rs".to_owned(),
        },
        crate::harness::Sensitivity::Routine,
    );

    let after = super::session::load(&root, "claude-probe");
    assert!(
        after.revision > before,
        "an allowed decision here was never written down, so the next write pays again"
    );
    assert!(after.verified_at.is_some(), "the run came back unverified");
}

#[test]
fn a_hook_git_will_not_run_is_not_an_installed_one() {
    // Git skips a hook without the execute bit silently: no warning, no exit
    // code, nothing anybody reads. So `estigia guard` said the guard was in,
    // `doctor` said it was in force, and every push went through ungated —
    // installed, looks installed, decides nothing.
    //
    // Fed rather than found, because the case cannot be produced on the
    // platform this is being written on: git for Windows never consults the
    // mode, so `git_would_run` is always true there and the state is
    // unreachable. A decision nobody can reach is a decision nobody can check.
    let ours = format!("#!/bin/sh\n# {MARKER}\nestigia hook pre-push\n");
    let theirs = "#!/bin/sh\nexec estigia hook pre-push \"$@\"\n";
    let strangers = "#!/bin/sh\nnpx some-linter\n";

    assert_eq!(state_of(Some(&ours), true), State::Installed);
    assert_eq!(state_of(Some(theirs), true), State::Chained);

    // Both of those, unrunnable, are the same thing: present and inert.
    assert_eq!(state_of(Some(&ours), false), State::Inert);
    assert_eq!(
        state_of(Some(theirs), false),
        State::Inert,
        "a chained hook git will not run was reported as running"
    );

    // Somebody else's hook is theirs either way — Estigia is not in force in
    // it, so there is nothing of ours to be inert.
    assert_eq!(state_of(Some(strangers), true), State::Foreign);
    assert_eq!(state_of(Some(strangers), false), State::Foreign);

    // And no file at all is absent, not inert.
    assert_eq!(state_of(None, true), State::Absent);
    assert_eq!(state_of(None, false), State::Absent);
}

#[test]
fn an_inert_hook_of_estigias_is_still_estigias_to_remove() {
    let ours = format!("#!/bin/sh\n{MARKER}\nestigia hook pre-push\n");
    // Theirs, and chained: `calls_estigia` is true and the marker is not, so
    // without the bit this is `Inert` as well — and it stays.
    let chained = "#!/bin/sh\n./lint.sh\nestigia hook pre-push\n";
    let theirs = "#!/bin/sh\nexec ./lint.sh\n";

    assert_eq!(
        state_of(Some(&ours), false),
        State::Inert,
        "the fixture is wrong"
    );
    assert!(
        removable(State::Inert, Some(&ours)),
        "a hook carrying Estigia's own marker was left on the machine because git would not run it"
    );
    assert!(
        !removable(State::Inert, Some(chained)),
        "somebody's own hook was removed because it mentions Estigia and lost its execute bit"
    );

    // The states around it, so the change is bounded.
    assert!(removable(State::Installed, Some(&ours)));
    for state in [
        State::Absent,
        State::Foreign,
        State::Chained,
        State::Unreadable,
    ] {
        let named = format!("{state:?}");
        assert!(
            !removable(state, Some(theirs)),
            "{named} was treated as Estigia's to delete"
        );
    }
    // And an inert hook nothing could read is not removed on a guess.
    assert!(!removable(State::Inert, None));
}

#[test]
fn reinstalling_over_a_hook_somebody_extended_is_refused() {
    let repo = tempfile::tempdir().expect("a repository");
    crate::transport::run(
        &["git", "init", "--quiet", "."],
        Some(repo.path()),
        crate::transport::How::write(),
    )
    .expect("a repository is made");
    let estigia = std::path::Path::new("/usr/bin/estigia");

    super::install(repo.path(), estigia, false).expect("the guard installs");
    let path = super::hooks_directory(repo.path())
        .expect("a hooks directory")
        .join(super::HOOK);

    // Re-running over an untouched hook is the ordinary upgrade and must keep
    // working, or the refusal below is bought by breaking the command.
    super::install(repo.path(), estigia, false).expect("an untouched hook is reinstalled");

    let held = std::fs::read_to_string(&path).expect("the hook reads");
    std::fs::write(
        &path,
        format!(
            "{held}
# my own check
echo 'the changelog'
"
        ),
    )
    .expect("the operator adds a line");

    let refusal = super::install(repo.path(), estigia, false)
        .expect_err("a hook carrying somebody's lines was rewritten");
    assert_eq!(refusal.code, "push-hook-carries-your-lines");
    assert!(
        std::fs::read_to_string(&path)
            .expect("the hook reads")
            .contains("my own check"),
        "the added line did not survive the refusal"
    );
    // And the way out is named, because there is one: two of them.
    let said = refusal.to_string();
    assert!(
        said.contains("chain") && said.contains("take them out"),
        "the refusal does not name what to do about the additions: {said}"
    );
}

#[test]
fn a_hook_somebody_extended_is_left_where_it_is() {
    let untouched = super::script(std::path::Path::new("/usr/bin/estigia"));
    assert!(
        super::removable(super::State::Installed, Some(&untouched)),
        "an untouched hook stopped being Estigia's to remove, so the command removes nothing"
    );

    let extended = format!("{untouched}\n# my own check\necho 'the changelog'\n");
    assert!(
        !super::removable(super::State::Installed, Some(&extended)),
        "a hook carrying somebody's own lines was still removable"
    );
    // The same file without the execute bit is the same file. `Inert` was the
    // pair this rule had to be applied to as well, and applying it to one and
    // not the other is how the two answers drift.
    assert!(
        !super::removable(super::State::Inert, Some(&extended)),
        "an inert hook carrying somebody's own lines was still removable"
    );

    // And a hook written by an Estigia that has since moved is **not** edited:
    // the script names the executable's path, so comparing whole files would
    // call every relocated install somebody else's work.
    let elsewhere = super::script(std::path::Path::new("/opt/estigia/bin/estigia"));
    assert!(
        super::removable(super::State::Installed, Some(&elsewhere)),
        "a hook installed by an Estigia at another path was treated as edited"
    );
}

#[test]
fn the_holder_of_a_checkout_holds_it_from_inside_the_checkout() {
    let state = tempfile::tempdir().expect("a state root");
    let checkout = tempfile::tempdir().expect("a checkout");
    let deep = checkout.path().join("src").join("deep");
    std::fs::create_dir_all(&deep).expect("a subdirectory");

    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.repo_dir = Some(checkout.path().to_path_buf());
    crate::harness::session::store(state.path(), &run).expect("the pointer writes");

    for from in [checkout.path(), deep.as_path()] {
        let holders = holders_of(state.path(), from);
        assert_eq!(
            holders.len(),
            1,
            "nobody was found holding {} — the claim covers {}",
            from.display(),
            checkout.path().display()
        );
    }

    // The floor: a checkout nobody claimed still has no holder, or the answer
    // above would mean nothing.
    let elsewhere = tempfile::tempdir().expect("another checkout");
    assert!(
        holders_of(state.path(), elsewhere.path()).is_empty(),
        "a claim was found holding a checkout it was never made over"
    );
}

#[test]
fn the_closest_claim_holds_a_directory_two_runs_both_cover() {
    let state = tempfile::tempdir().expect("a state root");
    let checkout = tempfile::tempdir().expect("a checkout");
    let mine = checkout.path().join("wt-a");
    let theirs = checkout.path().join("wt-b");
    std::fs::create_dir_all(mine.join("src")).expect("my worktree");
    std::fs::create_dir_all(&theirs).expect("their worktree");

    let pointer = |run_id: &str, issue: u64, worktree: Option<&std::path::Path>| {
        let mut run = crate::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(checkout.path().to_path_buf());
        run.worktree = worktree.map(std::path::Path::to_path_buf);
        crate::harness::session::store(state.path(), &run).expect("the pointer writes");
    };
    pointer("claude-aaaa1111", 1, Some(&mine));
    pointer("claude-bbbb2222", 2, Some(&theirs));

    for from in [mine.as_path(), &mine.join("src")] {
        let holders = holders_of(state.path(), from);
        assert_eq!(
            holders.len(),
            1,
            "{} is one run's isolated worktree and {} run(s) were found holding it",
            from.display(),
            holders.len()
        );
        assert_eq!(
            holders[0].issue,
            Some(crate::identity::Identity::Number(1)),
            "the wrong run was found holding {}",
            from.display()
        );
    }

    // The floor, and the reason the many-holders refusal exists: at the base
    // checkout neither claim is closer, so the ambiguity is real and both are
    // still holders. A rule that always picked one would pass the assertion
    // above and take that refusal with it.
    assert_eq!(
        holders_of(state.path(), checkout.path()).len(),
        2,
        "two runs holding one checkout stopped being an ambiguity"
    );
}

#[test]
fn sibling_selection_binds_pr_before_head_and_attributes_no_ambiguous_holder() {
    let repo = repository();
    for arguments in [
        &["config", "user.email", "nobody@example.invalid"][..],
        &["config", "user.name", "nobody"][..],
        &["commit", "--allow-empty", "--quiet", "-m", "base"][..],
    ] {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(arguments)
                .output()
                .is_ok_and(|output| output.status.success())
        );
    }
    let sibling_root = tempfile::tempdir().expect("a sibling parent");
    let sibling = sibling_root.path().join("reviewed");
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "--quiet", "--detach"])
            .arg(&sibling)
            .output()
            .is_ok_and(|output| output.status.success())
    );
    let head = super::super::head_of(&sibling).expect("the sibling has a head");
    let state = tempfile::tempdir().expect("a state root");

    let store = |id: &str, pr: u64, receipt_head: &str| {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(pr));
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.path().to_path_buf());
        run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
            epoch: format!("{pr:032x}"),
            pr,
            head: receipt_head.to_owned(),
            base: "b".repeat(40),
            digest: "c".repeat(64),
        });
        session::store(state.path(), &run).expect("the pointer writes");
    };
    let stale = "d".repeat(40);
    store("claude-pr54", 54, &stale);
    store("claude-pr55", 55, &head);

    let action = Action::Boundary {
        command: "gh pr merge".to_owned(),
        pr: Some(54),
        pr_unidentified_reason: None,
        local_fast_forward_target: None,
        is_exact_integration_merge: false,
        runs_in: crate::harness::RunsIn::Session,
        target: crate::harness::CommitTarget::Checkout,
    };
    let selected = holders_for_action(state.path(), &sibling, &action);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].run_id, "claude-pr54");

    store("claude-pr54-too", 54, &stale);
    let root = tempfile::tempdir().expect("a gate root");
    let mut gate_context = context(root.path(), &sibling);
    gate_context.state_root = state.path().to_path_buf();
    // Two receipt-matched siblings now reconcile the same as two ordinary
    // holders: `holders.len() >= 2` dispatches `holder_standing` once per
    // name, both asking about issue #54. `context()` above builds a
    // `Tracker::Github { repo: None }`, whose `transport()` is `Some`, so an
    // unscripted call here would spawn the machine's real `gh` twice — wrapped
    // the same way its sibling reconciliation tests are, with an answer that
    // fails to land, matching the fail-closed standing an unauthenticated `gh`
    // in this environment would actually reach: an unknown result keeps both
    // holders counted rather than dropping either.
    let bin = scripted_gh();
    let script = answers(&[unreachable_tracker_answer()]);
    let adjudication = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        adjudicate_action(&gate_context, &sibling, &action, Sensitivity::Boundary, &[])
    });
    assert!(adjudication.decision.denies());
    assert_eq!(adjudication.holder, None);
}

#[test]
fn an_unreadable_sibling_pointer_selects_no_readable_holder() {
    let repo = repository();
    for arguments in [
        &["config", "user.email", "nobody@example.invalid"][..],
        &["config", "user.name", "nobody"][..],
        &["commit", "--allow-empty", "--quiet", "-m", "base"][..],
    ] {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(arguments)
                .output()
                .is_ok_and(|output| output.status.success())
        );
    }
    let sibling_root = tempfile::tempdir().expect("a sibling parent");
    let sibling = sibling_root.path().join("reviewed");
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "--quiet", "--detach"])
            .arg(&sibling)
            .output()
            .is_ok_and(|output| output.status.success())
    );
    let head = super::super::head_of(&sibling).expect("the sibling has a head");
    let state = tempfile::tempdir().expect("a state root");
    let mut run = session::Run::new("claude-readable".to_owned());
    run.issue = Some(crate::identity::Identity::Number(54));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.path().to_path_buf());
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head,
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    session::store(state.path(), &run).expect("the readable pointer writes");
    std::fs::write(state.path().join("claude-torn.json"), "{").expect("a torn pointer");

    let root = tempfile::tempdir().expect("a gate root");
    let mut gate_context = context(root.path(), &sibling);
    gate_context.state_root = state.path().to_path_buf();
    let adjudication = adjudicate_action(
        &gate_context,
        &sibling,
        &Action::Boundary {
            command: "gh pr merge".to_owned(),
            pr: Some(54),
            pr_unidentified_reason: None,
            local_fast_forward_target: None,
            is_exact_integration_merge: false,
            runs_in: crate::harness::RunsIn::Session,
            target: crate::harness::CommitTarget::Checkout,
        },
        Sensitivity::Boundary,
        &[],
    );
    let Decision::Deny(refusal) = adjudication.decision else {
        panic!("an unreadable pointer did not refuse");
    };
    assert_eq!(refusal.code, "run-pointers-unreadable");
    assert_eq!(adjudication.holder, None);
}

/// The script answers for one of the two holders and not the other on purpose:
/// that asymmetry is the subject, and making it symmetric moves the decision to
/// a different arm that has its own test.
#[test]
fn an_unanswered_tracker_read_names_the_read_and_not_the_contract() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("create the checkout");
    let context = context(root.path(), &repo);

    for (id, issue) in [("claude-stale0001", 12), ("claude-stale0002", 34)] {
        let mut run = session::Run::new(id.to_owned());
        run.issue = Some(crate::identity::Identity::Number(issue));
        run.repo_dir = Some(repo.clone());
        session::store(&context.state_root, &run).expect("the pointer writes");
    }

    let bin = scripted_gh();
    let script = answers(&[closed_issue(34)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        decide(&context, &repo, &[]).decision
    });

    let Decision::Deny(refusal) = decision else {
        panic!("an unanswerable read did not refuse: {decision:?}");
    };
    assert_eq!(
        refusal.code, "read-failed",
        "the refusal named something other than the read that failed: {refusal:?}"
    );
    assert_eq!(
        refusal.message, "git push: gh issue view 12 returned nothing",
        "the refusal did not name the read it could not make: {refusal:?}"
    );
    assert_eq!(
        refusal.resolution.to_string(),
        "[world-action] the tracker could not be read; write nothing and retry the read",
        "the refusal did not carry the resolution the transport already had: {refusal:?}"
    );
}
