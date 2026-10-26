use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Workflow;
use crate::harness::{
    Action, Aside, CommitTarget, Decision, GateContext, RunsIn, Sensitivity, guard, session,
};
use crate::outcome::Observed;

const RUN: &str = "claude-abcd1234";
const BRANCH: &str = "issue-7";

struct Rig {
    repo: tempfile::TempDir,
    trees: tempfile::TempDir,
    root: tempfile::TempDir,
    worktree: PathBuf,
    context: GateContext,
    run: session::Run,
}

fn git(directory: &Path, arguments: &[&str]) {
    let done = std::process::Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        done.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&done.stderr)
    );
}

fn write_marker(worktree: &Path, run_id: &str, issue: u64, branch: &str) {
    let marker = crate::transport::branch::ownership_path(worktree).expect("an admin directory");
    std::fs::write(
        marker,
        serde_json::json!({ "run_id": run_id, "issue": issue, "branch": branch }).to_string(),
    )
    .expect("the marker is written");
}

fn new_rig() -> Rig {
    let repo = crate::test_env::repository_with_commit();
    let trees = tempfile::tempdir().expect("a directory for worktrees");
    let worktree = trees.path().join(BRANCH);
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            BRANCH,
            worktree.to_str().expect("a path"),
        ],
    );
    write_marker(&worktree, RUN, 7, BRANCH);
    let root = tempfile::tempdir().expect("a state root");
    let skill_root = root.path().join("skill");
    std::fs::create_dir_all(&skill_root).expect("a skill root");
    std::fs::write(skill_root.join(crate::skill::CONTRACT), "the contract\n")
        .expect("the contract is installed");
    let context = GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root,
        repo_dir: worktree.clone(),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(Workflow::Assisted),
        window: Duration::ZERO,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let mut run = session::Run::new(RUN.to_owned());
    run.issue = Some(crate::identity::Identity::Number(7));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.path().to_path_buf());
    run.worktree = Some(worktree.clone());
    run.branch = Some(BRANCH.to_owned());
    run.assisted = true;
    Rig {
        repo,
        trees,
        root,
        worktree,
        context,
        run,
    }
}

fn write(target: &Path) -> Action {
    Action::Write {
        target: target.display().to_string(),
    }
}

fn boundary(command: &str) -> Action {
    Action::Boundary {
        command: command.to_owned(),
        pr: None,
        pr_unidentified_reason: None,
        local_fast_forward_target: None,
        is_exact_integration_merge: false,
        runs_in: RunsIn::Session,
        target: CommitTarget::Checkout,
    }
}

fn decide(rig: &mut Rig, action: &Action, how: Sensitivity) -> Decision {
    decide_pushing(rig, action, how, &[])
}

fn decide_pushing(
    rig: &mut Rig,
    action: &Action,
    how: Sensitivity,
    refs: &[(&str, &str)],
) -> Decision {
    let observed = Observed {
        push_refs: refs
            .iter()
            .map(|(local, remote)| ((*local).to_owned(), (*remote).to_owned()))
            .collect(),
        ..Observed::default()
    };
    crate::harness::gate_observing(&rig.context, &mut rig.run, action, how, &observed)
}

fn denied_as(decision: &Decision) -> &'static str {
    match decision {
        Decision::Deny(refusal) => refusal.code,
        other => panic!("not denied: {other:?}"),
    }
}

#[test]
fn a_write_inside_the_worktree_is_admitted() {
    let mut rig = new_rig();
    let target = rig.worktree.join("src.rs");
    let decision = decide(&mut rig, &write(&target), Sensitivity::Routine);
    assert!(matches!(decision, Decision::Allow(_)), "{decision:?}");
}

#[test]
fn a_keyed_assisted_run_under_a_read_only_tracker_is_still_admitted_in_its_worktree() {
    let mut rig = new_rig();
    rig.context.tracker = crate::config::Tracker::Linear {
        team: "ENG".to_owned(),
    };
    rig.context.issue_requirement = crate::config::IssueRequirement::Required;
    guard::install(rig.repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let marker = crate::transport::branch::ownership_path(&rig.worktree).expect("a directory");
    std::fs::write(
        marker,
        serde_json::json!({ "run_id": RUN, "issue": "ENG-12", "branch": BRANCH }).to_string(),
    )
    .expect("the marker is written");
    rig.run.issue = Some(crate::identity::Identity::Key("ENG-12".to_owned()));
    let decision = write_in_worktree(&mut rig);
    assert!(matches!(decision, Decision::Allow(_)), "{decision:?}");
}

#[test]
fn a_pointer_is_not_authority_once_the_row_is_not_assisted() {
    for workflow in [Some(Workflow::Autonomous), None] {
        let mut rig = new_rig();
        rig.context.workflow = workflow;
        let decision = write_in_worktree(&mut rig);
        assert_eq!(denied_as(&decision), "assisted-claim-not-in-force");
    }
}

#[test]
fn a_pointer_with_no_worktree_confines_nothing_and_admits_nothing() {
    let mut rig = new_rig();
    rig.run.worktree = None;
    rig.context.repo_dir = rig.repo.path().to_path_buf();
    let decision = decide(&mut rig, &boundary("git pull"), Sensitivity::Boundary);
    assert_eq!(denied_as(&decision), "assisted-worktree-not-started");
}

#[test]
fn a_write_from_the_base_checkout_is_outside_the_worktree() {
    let mut rig = new_rig();
    rig.context.repo_dir = rig.repo.path().to_path_buf();
    let decision = write_in_base(&mut rig);
    assert_eq!(denied_as(&decision), "assisted-outside-the-worktree");
}

#[test]
fn a_write_aimed_at_the_base_checkout_from_inside_the_worktree_is_outside_it() {
    let mut rig = new_rig();
    let decision = write_in_base(&mut rig);
    assert_eq!(denied_as(&decision), "assisted-outside-the-worktree");
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_worktree_does_not_carry_a_write_past_it() {
    let mut rig = new_rig();
    let link = rig.worktree.join("link");
    std::os::unix::fs::symlink(rig.repo.path(), &link).expect("a symlink");
    let decision = decide(&mut rig, &write(&link.join("x")), Sensitivity::Routine);
    assert_eq!(denied_as(&decision), "assisted-outside-the-worktree");
}

#[test]
fn a_write_to_no_repository_at_all_is_outside_the_claim() {
    let mut rig = new_rig();
    let elsewhere = tempfile::tempdir().expect("a directory");
    let decision = decide(
        &mut rig,
        &write(&elsewhere.path().join("note")),
        Sensitivity::Routine,
    );
    assert_eq!(decision, Decision::Outside(Aside::OutsideTheClaim));
}

#[test]
fn work_in_another_repository_is_not_this_claims_to_judge() {
    let mut rig = new_rig();
    let other = crate::test_env::repository_with_commit();
    rig.context.repo_dir = other.path().to_path_buf();
    let decision = decide(
        &mut rig,
        &write(&other.path().join("x")),
        Sensitivity::Routine,
    );
    assert_eq!(decision, Decision::Outside(Aside::AnotherCheckout));
}

#[test]
fn a_marker_naming_another_run_leaves_the_pointer_without_authority() {
    let mut rig = new_rig();
    write_marker(&rig.worktree, "claude-ffff0000", 7, BRANCH);
    let decision = write_in_worktree(&mut rig);
    assert_eq!(denied_as(&decision), "assisted-claim-stale");
}

#[test]
fn a_marker_naming_another_issue_or_branch_leaves_the_pointer_without_authority() {
    for (issue, branch) in [(8, BRANCH), (7, "issue-8")] {
        let mut rig = new_rig();
        write_marker(&rig.worktree, RUN, issue, branch);
        let decision = decide(&mut rig, &boundary("git pull"), Sensitivity::Boundary);
        assert_eq!(denied_as(&decision), "assisted-claim-stale");
    }
}

#[test]
fn a_worktree_that_is_gone_leaves_the_pointer_without_authority() {
    let mut rig = new_rig();
    git(
        rig.repo.path(),
        &[
            "worktree",
            "remove",
            "--force",
            rig.worktree.to_str().expect("a path"),
        ],
    );
    rig.context.repo_dir = rig.repo.path().to_path_buf();
    let decision = decide(&mut rig, &boundary("git pull"), Sensitivity::Boundary);
    assert!(
        matches!(decision, Decision::Deny(_)),
        "a pointer whose worktree is gone admitted a boundary: {decision:?}"
    );
}

#[test]
fn a_branch_switched_inside_the_worktree_leaves_the_pointer_without_authority() {
    let mut rig = new_rig();
    git(&rig.worktree, &["checkout", "-b", "elsewhere"]);
    let decision = write_in_worktree(&mut rig);
    assert_eq!(denied_as(&decision), "assisted-claim-stale");
}

#[test]
fn a_pointer_that_names_no_branch_is_not_taken_on_trust() {
    let mut rig = new_rig();
    rig.run.branch = None;
    let decision = decide(&mut rig, &boundary("git pull"), Sensitivity::Boundary);
    assert_eq!(denied_as(&decision), "assisted-claim-stale");
}

#[test]
fn a_routine_write_may_ride_the_renewal_window_and_a_boundary_may_not() {
    let mut rig = new_rig();
    rig.context.window = Duration::from_secs(600);
    assert_eq!(rig.run.verified_at, None);
    let first = write_in_worktree(&mut rig);
    assert!(matches!(first, Decision::Allow(_)), "{first:?}");
    assert!(
        rig.run.verified_at.is_some(),
        "the local check left no stamp"
    );
    write_marker(&rig.worktree, "claude-ffff0000", 7, BRANCH);
    let routine = write_in_worktree(&mut rig);
    assert!(matches!(routine, Decision::Allow(_)), "{routine:?}");
    let delivered = decide(&mut rig, &boundary("git pull"), Sensitivity::Boundary);
    assert_eq!(denied_as(&delivered), "assisted-claim-stale");
}

#[test]
fn an_assisted_run_delivers_nothing() {
    for command in [
        "git merge",
        "git tag",
        "gh pr merge",
        "gh release create",
        "gh release edit",
    ] {
        let mut rig = new_rig();
        let decision = decide(&mut rig, &boundary(command), Sensitivity::Boundary);
        assert_eq!(
            denied_as(&decision),
            "assisted-does-not-deliver",
            "{command}"
        );
    }
}

#[test]
fn a_shell_pull_request_is_refused_and_names_the_draft_pr_tool() {
    let mut rig = new_rig();
    let decision = decide(&mut rig, &boundary("gh pr create"), Sensitivity::Boundary);
    assert_eq!(denied_as(&decision), "assisted-publishes-through-draft-pr");
    let Decision::Deny(refusal) = decision else {
        unreachable!("denied above");
    };
    let text = refusal.to_string();
    assert!(text.contains("`draft_pr`"), "{text}");
    assert!(!text.contains("a person opening"), "{text}");
}

#[test]
fn an_assisted_api_mutation_cannot_bypass_pushed_fix_verification() {
    let mut rig = new_rig();
    let decision = decide(&mut rig, &boundary("gh api"), Sensitivity::Boundary);
    assert_eq!(denied_as(&decision), "assisted-api-mutation-unverified");
}

#[test]
fn a_pre_execution_push_names_the_effective_missing_hook_and_supported_publisher() {
    let mut rig = new_rig();
    git(&rig.worktree, &["config", "core.hooksPath", ".husky/_"]);
    let decision = decide(&mut rig, &boundary("git push"), Sensitivity::Boundary);
    let Decision::Deny(refusal) = decision else {
        panic!("not refused");
    };
    let text = refusal.to_string();
    assert!(
        text.contains(".husky/_/pre-push") || text.contains(".husky\\_\\pre-push"),
        "{text}"
    );
    assert!(
        text.contains("Absent") && text.contains("Git has not run"),
        "{text}"
    );
    assert!(
        text.contains("draft_pr") && text.contains("push-ready"),
        "{text}"
    );
}

#[test]
fn a_shell_command_the_gate_cannot_place_is_not_blamed_on_a_write_tool() {
    let mut rig = new_rig();
    let decision = write_to(&mut rig, "patch");
    let Decision::Deny(refusal) = decision else {
        panic!("not denied: {decision:?}");
    };
    let text = refusal.to_string();
    assert!(text.contains("assisted-write-unplaced"), "{text}");
    assert!(text.contains("shell command"), "{text}");
    assert!(text.contains("write tool"), "{text}");
}

#[test]
fn a_push_line_with_no_remote_ref_is_not_blamed_on_a_silent_hook() {
    let own = format!("refs/heads/{BRANCH}");
    let mut rig = new_rig();
    let decision = decide_pushing(
        &mut rig,
        &boundary("git push"),
        Sensitivity::Boundary,
        &[(&own, "")],
    );
    let Decision::Deny(refusal) = decision else {
        panic!("not denied: {decision:?}");
    };
    let text = refusal.to_string();
    assert!(text.contains("assisted-push-unconfined"), "{text}");
    assert!(!text.contains("has not spoken"), "{text}");
    assert!(text.contains("no remote ref"), "{text}");
}

#[cfg(unix)]
#[test]
fn git_hands_the_hook_head_and_not_the_branch_for_a_push_of_head() {
    use std::os::unix::fs::PermissionsExt;

    let mut rig = new_rig();
    let remote = tempfile::tempdir().expect("a remote");
    git(remote.path(), &["init", "--bare"]);
    git(
        &rig.worktree,
        &[
            "remote",
            "add",
            "origin",
            remote.path().to_str().expect("a path"),
        ],
    );
    let record = rig.root.path().join("stdin.txt");
    let hook = rig.repo.path().join(".git/hooks/pre-push");
    std::fs::create_dir_all(hook.parent().expect("a directory")).expect("a hooks directory");
    std::fs::write(&hook, format!("#!/bin/sh\ncat > '{}'\n", record.display()))
        .expect("the hook is written");
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("executable");
    git(&rig.worktree, &["push", "origin", "HEAD"]);

    let line = std::fs::read_to_string(&record).expect("the hook ran");
    let fields: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(fields[0], "HEAD", "{line}");
    assert_eq!(fields[2], format!("refs/heads/{BRANCH}"), "{line}");

    let decision = decide_pushing(
        &mut rig,
        &boundary("git push"),
        Sensitivity::Boundary,
        &[(fields[0], fields[2])],
    );
    let Decision::Deny(refusal) = decision else {
        panic!("not denied: {decision:?}");
    };
    assert!(refusal.to_string().contains("by name"), "{refusal}");
}

#[test]
fn a_push_is_admitted_only_for_the_runs_own_branch_and_only_when_its_refs_are_known() {
    let own = format!("refs/heads/{BRANCH}");
    let mut rig = new_rig();
    let unknown = decide(&mut rig, &boundary("git push"), Sensitivity::Boundary);
    assert_eq!(denied_as(&unknown), "assisted-push-unconfined");

    let mut rig = new_rig();
    let foreign = decide_pushing(
        &mut rig,
        &boundary("git push"),
        Sensitivity::Boundary,
        &[(&own, &own), ("refs/heads/main", "refs/heads/main")],
    );
    assert_eq!(denied_as(&foreign), "assisted-push-unconfined");

    let mut rig = new_rig();
    let deletion = decide_pushing(
        &mut rig,
        &boundary("git push"),
        Sensitivity::Boundary,
        &[("(delete)", &own)],
    );
    assert_eq!(denied_as(&deletion), "assisted-push-unconfined");

    let mut rig = new_rig();
    let admitted = decide_pushing(
        &mut rig,
        &boundary("git push"),
        Sensitivity::Boundary,
        &[(&own, &own)],
    );
    assert!(matches!(admitted, Decision::Allow(_)), "{admitted:?}");
}

#[test]
fn a_push_of_the_runs_branch_to_another_remote_ref_is_unconfined() {
    let own = format!("refs/heads/{BRANCH}");
    for destination in ["refs/heads/main", "refs/tags/v1", ""] {
        let mut rig = new_rig();
        let decision = decide_pushing(
            &mut rig,
            &boundary("git push"),
            Sensitivity::Boundary,
            &[(&own, destination)],
        );
        assert_eq!(denied_as(&decision), "assisted-push-unconfined");
    }
}

#[test]
fn the_hook_hands_the_pushed_refs_to_the_gate() {
    let rig = new_rig();
    session::store(&rig.context.state_root, &rig.run).expect("the pointer writes");
    let own = format!("refs/heads/{BRANCH}");
    let push = |local: &str, remote: &str| {
        guard::decide_pushing(
            &rig.context,
            &rig.worktree,
            &[local.to_owned()],
            &[remote.to_owned()],
        )
        .decision
    };
    let decision = push(&own, &own);
    assert!(matches!(decision, Decision::Allow(_)), "{decision:?}");
    assert_eq!(
        denied_as(&push("refs/heads/main", "refs/heads/main")),
        "assisted-push-unconfined"
    );
    assert_eq!(
        denied_as(&push(&own, "refs/heads/main")),
        "assisted-push-unconfined"
    );
    assert_eq!(
        denied_as(&push(&own, "refs/tags/v1")),
        "assisted-push-unconfined"
    );
}

#[test]
fn an_assisted_pointer_holds_its_worktree_and_not_the_base_checkout() {
    let rig = new_rig();
    session::store(&rig.context.state_root, &rig.run).expect("the pointer writes");
    let state = &rig.context.state_root;
    assert_eq!(guard::holders_of(state, &rig.worktree).len(), 1);
    assert!(
        guard::holders_of(state, rig.repo.path()).is_empty(),
        "the person's own checkout was read as the agent's"
    );
    let mut sworn = rig.run.clone();
    sworn.assisted = false;
    assert!(
        sworn.lands_in().len() == 2,
        "a tracker claim stopped covering the checkout it was made in"
    );
    assert_eq!(rig.run.lands_in(), vec![&rig.worktree]);
}

#[test]
fn an_assisted_pointer_never_sends_the_tracker_a_verify_claim() {
    let mut rig = new_rig();
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("gh.log");
    let script = serde_json::json!([]).to_string();
    let decision = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        let target = rig.worktree.join("x");
        let routine = decide(&mut rig, &write(&target), Sensitivity::Routine);
        let pulled = decide(&mut rig, &boundary("git pull"), Sensitivity::Boundary);
        (routine, pulled)
    });
    assert!(matches!(decision.0, Decision::Allow(_)), "{:?}", decision.0);
    assert!(matches!(decision.1, Decision::Allow(_)), "{:?}", decision.1);
    let asked = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(asked.trim().is_empty(), "the gate asked gh: {asked}");
    assert!(
        crate::harness::tracker_answer_for_pointer(
            &rig.context.skill_root,
            &rig.worktree,
            &rig.context.tracker,
            &rig.run
        )
        .is_none(),
        "a pointer with no tracker behind it was sent to the tracker"
    );
    let _ = &rig.trees;
}

#[test]
fn a_live_tracker_claim_on_the_same_issue_does_not_admit_an_assisted_pointer() {
    let mut rig = new_rig();
    write_marker(&rig.worktree, "claude-ffff0000", 7, BRANCH);
    let bin = crate::test_env::scripted_gh();
    let script =
        crate::test_env::answers(&[crate::test_env::live_holder_issue(RUN, 7, "in-progress")]);
    let decision =
        crate::test_env::with_scripted_gh(bin.path(), &script, || write_in_worktree(&mut rig));
    assert_eq!(denied_as(&decision), "assisted-claim-stale");
}

fn write_in_worktree(rig: &mut Rig) -> Decision {
    let action = write(&rig.worktree.join("x"));
    decide(rig, &action, Sensitivity::Routine)
}

fn write_to(rig: &mut Rig, target: &str) -> Decision {
    let action = Action::Write {
        target: target.to_owned(),
    };
    decide(rig, &action, Sensitivity::Routine)
}

#[test]
fn a_relative_target_is_placed_from_the_worktree_before_it_is_admitted() {
    let mut rig = new_rig();
    let inside = write_to(&mut rig, "src/lib.rs");
    assert!(matches!(inside, Decision::Allow(_)), "{inside:?}");

    let climbing = format!(
        "{}{}/x",
        "../".repeat(rig.worktree.components().count() - 1),
        rig.repo
            .path()
            .display()
            .to_string()
            .trim_start_matches('/')
    );
    let escaped = write_to(&mut rig, &climbing);
    assert_eq!(denied_as(&escaped), "assisted-outside-the-worktree");
}

#[test]
fn a_write_tool_that_names_no_path_cannot_be_placed_and_is_refused() {
    for tool in ["apply_patch", "patch", "Apply_Patch"] {
        let mut rig = new_rig();
        let decision = write_to(&mut rig, tool);
        assert_eq!(denied_as(&decision), "assisted-write-unplaced");
    }
}

fn write_in_base(rig: &mut Rig) -> Decision {
    let action = write(&rig.repo.path().join("x"));
    decide(rig, &action, Sensitivity::Routine)
}

#[test]
fn a_boundary_from_the_base_checkout_is_outside_the_worktree() {
    let mut rig = new_rig();
    rig.context.repo_dir = rig.repo.path().to_path_buf();
    let decision = decide(&mut rig, &boundary("git pull"), Sensitivity::Boundary);
    assert_eq!(denied_as(&decision), "assisted-outside-the-worktree");
}
