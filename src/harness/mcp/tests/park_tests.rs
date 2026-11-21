use super::*;
use crate::config::GitOperationWindow;
use crate::identity::Identity;

fn evening() -> Option<u16> {
    Some(20 * 60)
}

fn noon() -> Option<u16> {
    Some(12 * 60)
}

fn context(root: &std::path::Path) -> GateContext {
    GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.join("skill"),
        repo_dir: root.join("repo"),
        state_root: root.join("state"),
        operation_window: GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule"),
        issue_requirement: crate::config::IssueRequirement::Off,
        repository_config_unreadable: None,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: crate::harness::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn holding(context: &GateContext, issue: u64, parked: &[u64]) -> session::Run {
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(Identity::Number(issue));
    run.worktree = Some(context.repo_dir.join(format!("wt-{issue}")));
    run.operation_id = Some(format!("op-{issue}"));
    run.parked = parked
        .iter()
        .map(|number| session::ParkedClaim {
            issue: Identity::Number(*number),
            state: Some("in-progress".to_owned()),
            worktree: Some(context.repo_dir.join(format!("wt-{number}"))),
            operation_id: Some(format!("op-{number}")),
            release_id: None,
            review_transaction: None,
            reviewed_head: None,
            horizon: None,
        })
        .collect();
    session::store(&context.state_root, &run).expect("the pointer writes");
    run
}

fn claim(context: &GateContext, issue: u64) -> ToolFailure {
    run_tool(
        "claim",
        &json!({"issue": issue, "run_id": "claude-abcd1234", "horizon": "2026-08-01T18:00Z",
                "park": true}),
        Ok(context),
    )
    .expect_err("there is no transport in this sandbox")
}

#[test]
fn a_closed_window_lets_a_run_park_its_held_issue_to_claim_another() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let before = holding(&context, 12, &[]);
    let failure =
        crate::harness::operation_window::with_now_override(Some(evening), || claim(&context, 34));
    assert!(
        !failure.to_string().contains("already-holding"),
        "{failure}"
    );
    let after = session::load(&context.state_root, "claude-abcd1234");
    assert_eq!(
        after.issue, before.issue,
        "a claim that did not land left the run parked"
    );
    assert_eq!(after.worktree, before.worktree);
    assert!(after.parked.is_empty(), "{:?}", after.parked);
}

#[test]
fn an_open_window_refuses_to_park_for_a_new_issue() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    holding(&context, 12, &[]);
    let failure =
        crate::harness::operation_window::with_now_override(Some(noon), || claim(&context, 34));
    let ToolFailure::Refused(refusal) = failure else {
        panic!("a well-formed call was reported malformed");
    };
    assert_eq!(refusal.code, "already-holding");
    assert!(refusal.message.contains("window"), "{}", refusal.message);
}

#[test]
fn a_parked_issue_is_taken_back_whatever_the_window() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    holding(&context, 34, &[12]);
    let failure =
        crate::harness::operation_window::with_now_override(Some(noon), || claim(&context, 12));
    assert!(
        !failure.to_string().contains("already-holding"),
        "{failure}"
    );
}

#[test]
fn without_park_a_closed_window_still_refuses_a_second_claim() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    holding(&context, 12, &[]);
    let failure = crate::harness::operation_window::with_now_override(Some(evening), || {
        run_tool(
            "claim",
            &json!({"issue": 34, "run_id": "claude-abcd1234", "horizon": "2026-08-01T18:00Z"}),
            Ok(&context),
        )
        .expect_err("a second oath is refused")
    });
    let ToolFailure::Refused(refusal) = failure else {
        panic!("a well-formed call was reported malformed");
    };
    assert_eq!(refusal.code, "already-holding");
}

#[test]
fn parking_swaps_the_active_claim_with_a_parked_one_and_back() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = holding(&context, 34, &[12]);
    run.park_for(12);
    assert_eq!(run.issue, Some(Identity::Number(12)));
    assert_eq!(run.operation_id.as_deref(), Some("op-12"));
    assert_eq!(run.worktree, Some(context.repo_dir.join("wt-12")));
    assert!(run.holds_parked(34) && !run.holds_parked(12));
    run.park_for(56);
    assert_eq!(run.issue, None);
    assert_eq!(run.worktree, None);
    assert!(run.holds_parked(34) && run.holds_parked(12));
}

#[test]
fn taking_a_claim_back_repeats_the_horizon_it_was_sworn_with() {
    let mut flags: Vec<String> = ["--issue", "12", "--horizon", "2026-08-01T23:00Z"]
        .map(str::to_owned)
        .to_vec();
    super::super::with_sworn_horizon(&mut flags, Some("2026-08-01T18:00Z"));
    assert_eq!(flags[3], "2026-08-01T18:00Z");
    super::super::with_sworn_horizon(&mut flags, None);
    assert_eq!(flags[3], "2026-08-01T18:00Z");
}

#[test]
fn parking_carries_the_sworn_horizon_with_the_claim() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = holding(&context, 34, &[]);
    run.horizon = Some("2026-08-01T18:00Z".to_owned());
    run.park_for(12);
    assert_eq!(run.horizon, None);
    run.park_for(34);
    assert_eq!(run.horizon.as_deref(), Some("2026-08-01T18:00Z"));
}

fn forget(context: &GateContext, run: &mut session::Run, issue: u64) -> session::Run {
    let release = tools::find("release").expect("the tool is listed");
    apply_effect(
        release,
        &json!({"issue": issue, "run_id": "claude-abcd1234"}),
        Some(&json!({"ok": true})),
        run,
        context,
    );
    session::load(&context.state_root, "claude-abcd1234")
}

#[test]
fn releasing_the_active_claim_keeps_the_parked_ones() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = holding(&context, 34, &[12, 56]);
    let after = forget(&context, &mut run, 34);
    assert!(
        session::pointer_path(&context.state_root, "claude-abcd1234").exists(),
        "the release removed a pointer that still names parked claims"
    );
    assert_eq!(after.issue, None);
    assert_eq!(after.worktree, None);
    assert_eq!(after.operation_id, None);
    assert!(after.holds_parked(12) && after.holds_parked(56));
    assert_eq!(session::parked_holdings(&context.state_root).len(), 1);
}

#[test]
fn releasing_a_parked_claim_keeps_the_active_one() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = holding(&context, 34, &[12, 56]);
    let after = forget(&context, &mut run, 12);
    assert_eq!(after.issue, Some(Identity::Number(34)));
    assert_eq!(after.operation_id.as_deref(), Some("op-34"));
    assert!(!after.holds_parked(12) && after.holds_parked(56));
}

#[test]
fn releasing_an_issue_the_run_does_not_hold_changes_nothing() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = holding(&context, 34, &[12]);
    let after = forget(&context, &mut run, 99);
    assert_eq!(after.issue, Some(Identity::Number(34)));
    assert_eq!(after.operation_id.as_deref(), Some("op-34"));
    assert!(after.holds_parked(12));
}

#[test]
fn releasing_the_only_claim_removes_the_pointer() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = holding(&context, 34, &[]);
    forget(&context, &mut run, 34);
    assert!(!session::pointer_path(&context.state_root, "claude-abcd1234").exists());
}

#[test]
fn releasing_the_last_parked_claim_with_none_active_removes_the_pointer() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = holding(&context, 34, &[12]);
    forget(&context, &mut run, 34);
    forget(&context, &mut run, 12);
    assert!(!session::pointer_path(&context.state_root, "claude-abcd1234").exists());
}
