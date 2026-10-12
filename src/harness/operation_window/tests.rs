use std::cell::RefCell;
use std::path::Path;

use super::*;
use crate::config::GitOperationWindow;
use crate::harness::{Decision, Sensitivity, session::Run};

std::thread_local! {
    static TEST_MINUTES: RefCell<u16> = const { RefCell::new(0) };
}

fn test_clock() -> Option<u16> {
    TEST_MINUTES.with(|cell| Some(*cell.borrow()))
}

fn at(minutes: u16, run: impl FnOnce()) {
    TEST_MINUTES.with(|cell| *cell.borrow_mut() = minutes);
    with_now_override(Some(test_clock), run);
}

fn context(root: &Path, window: GitOperationWindow) -> GateContext {
    GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: root.join("skill"),
        repo_dir: root.join("repo"),
        state_root: root.join("state"),
        window: crate::harness::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
        operation_window: window,
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
    }
}

fn installed(context: &GateContext) {
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
}

fn sworn(repo_dir: &Path) -> Run {
    let mut run = Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo_dir.to_path_buf());
    run.mark_verified();
    run
}

fn commit() -> Action {
    Action::Write {
        target: "git commit".to_owned(),
    }
}

fn edit() -> Action {
    Action::Write {
        target: "src/main.rs".to_owned(),
    }
}

fn push() -> Action {
    Action::Boundary {
        command: "git push".to_owned(),
        pr: None,
        pr_unidentified_reason: None,
        local_fast_forward_target: None,
        is_exact_integration_merge: false,
        runs_in: crate::harness::RunsIn::Session,
    }
}

#[test]
fn a_disabled_schedule_preserves_current_behaviour() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), GitOperationWindow::Disabled);
    installed(&context);
    let mut run = sworn(&context.repo_dir);
    at(3 * 60, || {
        match crate::harness::gate(&context, &mut run, &commit(), Sensitivity::Routine) {
            Decision::Allow(_) => {}
            other => panic!("a disabled schedule denied a commit: {other:?}"),
        }
    });
}

#[test]
fn a_commit_outside_the_period_is_denied_before_the_renewal_window() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut run = sworn(&context.repo_dir);
    at(20 * 60, || {
        let Decision::Deny(refusal) =
            crate::harness::gate(&context, &mut run, &commit(), Sensitivity::Routine)
        else {
            panic!("a commit at 20:00 rode the renewal window past 09:00-17:00");
        };
        assert_eq!(refusal.code, "git-operation-window-denied");
    });
}

#[test]
fn a_commit_inside_the_period_still_rides_the_renewal_window() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut run = sworn(&context.repo_dir);
    at(12 * 60, || {
        match crate::harness::gate(&context, &mut run, &commit(), Sensitivity::Routine) {
            Decision::Allow(_) => {}
            other => panic!("a commit at noon was not decided by the other checks: {other:?}"),
        }
    });
}

fn comment_on_github() -> Action {
    Action::Remote {
        command: "gh issue comment".to_owned(),
    }
}

#[test]
fn a_remote_write_outside_the_period_is_denied() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut run = sworn(&context.repo_dir);
    at(20 * 60, || {
        let Decision::Deny(refusal) = crate::harness::gate(
            &context,
            &mut run,
            &comment_on_github(),
            Sensitivity::Routine,
        ) else {
            panic!("a comment on GitHub at 20:00 passed 09:00-17:00");
        };
        assert_eq!(refusal.code, "git-operation-window-denied");
    });
}

#[test]
fn the_gh_writes_outside_the_old_verb_table_are_remote_and_denied_while_closed() {
    let lines = [
        "gh cache delete --all",
        "gh ssh-key add k.pub",
        "gh ssh-key delete 1",
        "gh gpg-key add k.asc",
        "gh gpg-key delete 1",
        "gh repo deploy-key add k.pub",
        "gh repo deploy-key delete 1",
        "gh repo autolink create JIRA- https://x/ref",
        "gh repo autolink delete 1",
    ];
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    for line in lines {
        let (action, _) = crate::harness::classify("Bash", &serde_json::json!({ "command": line }));
        assert!(
            matches!(action, Action::Remote { .. }),
            "{line}: {action:?}"
        );
        let mut run = sworn(&context.repo_dir);
        at(20 * 60, || {
            let decision = crate::harness::gate(&context, &mut run, &action, Sensitivity::Routine);
            let Decision::Deny(refusal) = decision else {
                panic!("{line} passed a closed window: {decision:?}");
            };
            assert_eq!(refusal.code, "git-operation-window-denied", "{line}");
        });
    }
}

#[test]
fn a_remote_write_inside_the_period_or_with_no_schedule_is_not_watched() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule");
    for (window, minutes) in [(window, 12 * 60), (GitOperationWindow::Disabled, 3 * 60)] {
        let context = context(root.path(), window);
        installed(&context);
        let mut run = sworn(&context.repo_dir);
        at(minutes, || {
            let decision = crate::harness::gate(
                &context,
                &mut run,
                &comment_on_github(),
                Sensitivity::Routine,
            );
            assert!(
                matches!(decision, Decision::Outside(_)),
                "{window:?} at {minutes}: {decision:?}"
            );
        });
    }
}

#[test]
fn the_schedule_never_grants_a_push_the_other_checks_deny() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 00:00-23:59").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut run = sworn(&context.repo_dir);
    at(12 * 60, || {
        let decision = crate::harness::gate(&context, &mut run, &push(), Sensitivity::Boundary);
        assert!(
            decision.denies(),
            "a push inside the period was granted by the schedule: {decision:?}"
        );
        if let Decision::Deny(refusal) = &decision {
            assert_ne!(
                refusal.code, "git-operation-window-denied",
                "the schedule denied what it should leave to the other checks"
            );
        }
    });
}

#[test]
fn an_inverted_schedule_permits_only_the_complement() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 09:00-17:00 inverted").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut noon = sworn(&context.repo_dir);
    let mut evening = sworn(&context.repo_dir);
    at(12 * 60, || {
        assert!(
            crate::harness::gate(&context, &mut noon, &commit(), Sensitivity::Routine).denies(),
            "midday passed an inverted 09:00-17:00"
        );
    });
    at(20 * 60, || {
        match crate::harness::gate(&context, &mut evening, &commit(), Sensitivity::Routine) {
            Decision::Allow(_) => {}
            other => panic!("the evening did not pass an inverted 09:00-17:00: {other:?}"),
        }
    });
}

#[test]
fn endpoints_are_half_open_through_the_gate() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut opening = sworn(&context.repo_dir);
    let mut closing = sworn(&context.repo_dir);
    at(9 * 60, || {
        match crate::harness::gate(&context, &mut opening, &commit(), Sensitivity::Routine) {
            Decision::Allow(_) => {}
            other => panic!("the inclusive start denied: {other:?}"),
        }
    });
    at(17 * 60, || {
        assert!(
            crate::harness::gate(&context, &mut closing, &commit(), Sensitivity::Routine).denies(),
            "the exclusive end permitted"
        );
    });
}

#[test]
fn a_midnight_crossing_window_permits_after_midnight() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 22:00-06:00").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut night = sworn(&context.repo_dir);
    let mut day = sworn(&context.repo_dir);
    at(2 * 60, || {
        match crate::harness::gate(&context, &mut night, &commit(), Sensitivity::Routine) {
            Decision::Allow(_) => {}
            other => panic!("02:00 did not pass 22:00-06:00: {other:?}"),
        }
    });
    at(12 * 60, || {
        assert!(
            crate::harness::gate(&context, &mut day, &commit(), Sensitivity::Routine).denies(),
            "midday passed 22:00-06:00"
        );
    });
}

#[test]
fn an_invalid_schedule_denies_protected_operations_and_nothing_else() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), GitOperationWindow::Invalid);
    installed(&context);
    let mut committing = sworn(&context.repo_dir);
    let mut editing = sworn(&context.repo_dir);
    at(12 * 60, || {
        let Decision::Deny(refusal) =
            crate::harness::gate(&context, &mut committing, &commit(), Sensitivity::Routine)
        else {
            panic!("an unparsable schedule fell back to unrestricted");
        };
        assert_eq!(refusal.code, "git-operation-window-denied");
        match crate::harness::gate(&context, &mut editing, &edit(), Sensitivity::Routine) {
            Decision::Allow(_) => {}
            other => panic!("an invalid schedule stopped an ordinary edit: {other:?}"),
        }
    });
}

#[test]
fn an_unreadable_clock_denies_protected_operations_and_nothing_else() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut committing = sworn(&context.repo_dir);
    let mut editing = sworn(&context.repo_dir);
    with_now_override(Some(|| None), || {
        let Decision::Deny(refusal) =
            crate::harness::gate(&context, &mut committing, &commit(), Sensitivity::Routine)
        else {
            panic!("an unreadable clock fell back to unrestricted");
        };
        assert_eq!(refusal.code, "git-operation-window-denied");
        match crate::harness::gate(&context, &mut editing, &edit(), Sensitivity::Routine) {
            Decision::Allow(_) => {}
            other => panic!("an unreadable clock stopped an ordinary edit: {other:?}"),
        }
    });
}

#[test]
fn the_push_guard_denies_without_a_holder_outside_the_period() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = tempfile::tempdir().expect("a checkout");
    let mut context = context(
        root.path(),
        GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule"),
    );
    context.repo_dir = repo.path().to_path_buf();
    context.state_root = root.path().join("state");
    std::fs::create_dir_all(&context.state_root).expect("a state directory");
    at(20 * 60, || {
        let adjudication = crate::harness::guard::decide(&context, repo.path(), &[]);
        let Decision::Deny(refusal) = &adjudication.decision else {
            panic!("an unclaimed push at 20:00 stood aside as nothing-sworn");
        };
        assert_eq!(refusal.code, "git-operation-window-denied");
    });
}

#[test]
fn the_push_guard_stands_aside_inside_the_period() {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = tempfile::tempdir().expect("a checkout");
    let mut context = context(
        root.path(),
        GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule"),
    );
    context.repo_dir = repo.path().to_path_buf();
    context.state_root = root.path().join("state");
    std::fs::create_dir_all(&context.state_root).expect("a state directory");
    at(12 * 60, || {
        let adjudication = crate::harness::guard::decide(&context, repo.path(), &[]);
        assert!(
            matches!(
                adjudication.decision,
                Decision::Outside(crate::harness::Aside::NothingSworn)
            ),
            "an unclaimed push at noon was decided by the schedule: {:?}",
            adjudication.decision
        );
    });
}

fn table(value: &str) -> String {
    format!(
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Git operation window | {value} | disabled |\n"
    )
}

#[test]
fn the_effective_schedule_layers_contract_local_and_repository() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join("skill");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&skill).expect("a skill root");
    let fallback = GitOperationWindow::Disabled;
    assert_eq!(
        effective_in(&skill, &repo, &fallback),
        GitOperationWindow::Disabled,
        "an unconfigured checkout reads as the fallback"
    );
    std::fs::write(
        skill.join(crate::skill::CONTRACT),
        table("enabled 09:00-17:00"),
    )
    .expect("a contract");
    assert_eq!(
        effective_in(&skill, &repo, &fallback),
        GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule"),
        "the contract row was not read"
    );
    std::fs::write(skill.join(crate::config::LOCAL_FILE), table("disabled"))
        .expect("a local override");
    assert_eq!(
        effective_in(&skill, &repo, &fallback),
        GitOperationWindow::Disabled,
        "the operator override did not win over the contract"
    );
    std::fs::create_dir_all(
        crate::skill::repository_config_path(&repo)
            .parent()
            .expect("a parent"),
    )
    .expect("the repository directory");
    std::fs::write(
        crate::skill::repository_config_path(&repo),
        table("enabled 22:00-06:00 inverted"),
    )
    .expect("a repository override");
    assert_eq!(
        effective_in(&skill, &repo, &fallback),
        GitOperationWindow::parse("enabled 22:00-06:00 inverted").expect("a schedule"),
        "the repository row did not win over the operator file"
    );
}

#[test]
fn a_present_but_unparsable_row_is_invalid_not_disabled() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join("skill");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&skill).expect("a skill root");
    std::fs::write(
        skill.join(crate::skill::CONTRACT),
        table("enabled 09:00-09:00"),
    )
    .expect("a contract");
    assert_eq!(
        effective_in(&skill, &repo, &GitOperationWindow::Disabled),
        GitOperationWindow::Invalid,
        "equal endpoints fell back to disabled instead of failing closed"
    );
}

#[test]
fn a_commit_on_a_day_outside_the_set_is_denied_and_the_denial_names_the_days() {
    let root = tempfile::tempdir().expect("a temporary root");
    let window = GitOperationWindow::parse("enabled mon-fri 09:00-17:00").expect("a schedule");
    let context = context(root.path(), window);
    installed(&context);
    let mut run = sworn(&context.repo_dir);
    at(5 * 1440 + 12 * 60, || {
        let Decision::Deny(refusal) =
            crate::harness::gate(&context, &mut run, &commit(), Sensitivity::Routine)
        else {
            panic!("a Saturday noon commit passed mon-fri 09:00-17:00");
        };
        assert_eq!(refusal.code, "git-operation-window-denied");
        assert!(
            refusal.to_string().contains("mon-fri 09:00-17:00"),
            "the denial does not name the days: {refusal}"
        );
    });
    at(2 * 1440 + 12 * 60, || {
        match crate::harness::gate(&context, &mut run, &commit(), Sensitivity::Routine) {
            Decision::Allow(_) => {}
            other => panic!("a Wednesday noon commit was not left to the other checks: {other:?}"),
        }
    });
}

#[test]
fn a_midnight_crossing_range_permits_the_morning_after_its_start_day_only() {
    let window = GitOperationWindow::parse("enabled fri 22:00-06:00").expect("a schedule");
    assert!(permits_now(&window, Some(5 * 1440 + 3 * 60)));
    assert!(!permits_now(&window, Some(4 * 1440 + 3 * 60)));
    assert!(!permits_now(&window, Some(5 * 1440 + 23 * 60)));
}
