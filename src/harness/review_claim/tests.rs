use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Workflow;
use crate::harness::{Action, Aside, Decision, GateContext, Sensitivity, guard, session};
use crate::outcome::Observed;

const RUN: &str = "claude-abcd1234";
const PR: u64 = 7;

struct Rig {
    repo: tempfile::TempDir,
    _trees: tempfile::TempDir,
    _root: tempfile::TempDir,
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

fn new_rig() -> Rig {
    let repo = crate::test_env::repository_with_commit();
    let trees = tempfile::tempdir().expect("a directory for worktrees");
    let worktree = trees.path().join("review-pr-7");
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "--detach",
            worktree.to_str().expect("a path"),
        ],
    );
    let root = tempfile::tempdir().expect("a state root");
    let skill_root = root.path().join("skill");
    std::fs::create_dir_all(&skill_root).expect("a skill root");
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
        unclaimed_github_writes: crate::config::UnclaimedGithubWrites::Held,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(Workflow::Assisted),
        window: Duration::ZERO,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let mut run = session::Run::new(RUN.to_owned());
    run.kind = session::RunKind::ReviewOnly;
    run.reviewed_pr = Some(PR);
    run.repo_dir = Some(repo.path().to_path_buf());
    run.worktree = Some(worktree.clone());
    Rig {
        repo,
        _trees: trees,
        _root: root,
        worktree,
        context,
        run,
    }
}

fn decide_line(rig: &mut Rig, line: &str) -> Decision {
    let input = serde_json::json!({ "command": line });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = Observed {
        command: Some(command),
        checkout: None,
        push_refs: Vec::new(),
    };
    crate::harness::gate_observing(&rig.context, &mut rig.run, &action, how, &observed)
}

fn write_to(rig: &mut Rig, target: &Path) -> Decision {
    let action = Action::Write {
        target: target.display().to_string(),
    };
    crate::harness::gate_observing(
        &rig.context,
        &mut rig.run,
        &action,
        Sensitivity::Routine,
        &Observed::default(),
    )
}

fn denied_as(decision: &Decision) -> &'static str {
    match decision {
        Decision::Deny(refusal) => refusal.code,
        other => panic!("expected a denial, got {other:?}"),
    }
}

#[test]
fn every_host_write_and_delivery_a_review_claim_could_reach_is_refused() {
    for line in [
        "git push origin HEAD",
        "git push origin main",
        "gh pr merge 7 --merge",
        "gh pr review 7 --approve",
        "gh pr review 7 --request-changes --body x",
        "gh pr comment 7 --body x",
        "gh pr create --title x --body y",
        "gh pr ready 7",
        "gh pr close 7",
        "gh api -X POST repos/o/r/pulls/7/reviews -f event=APPROVE",
        "gh api --method PATCH repos/o/r/pulls/7 -f title=x",
        "git commit -m x; gh pr review 7 --approve",
    ] {
        let mut rig = new_rig();
        let decision = decide_line(&mut rig, line);
        assert_eq!(
            denied_as(&decision),
            "review-claim-admits-no-write",
            "{line}"
        );
    }
}

#[test]
fn a_host_write_is_refused_from_a_directory_the_claim_does_not_cover() {
    let elsewhere = tempfile::tempdir().expect("a directory");
    let mut rig = new_rig();
    rig.context.repo_dir = elsewhere.path().to_path_buf();
    let decision = decide_line(&mut rig, "gh pr merge 7 --merge");
    assert_eq!(denied_as(&decision), "review-claim-admits-no-write");
}

#[test]
fn no_shell_worktree_command_is_admitted() {
    for line in ["git worktree add ../x", "git worktree remove ../x"] {
        let mut rig = new_rig();
        let decision = decide_line(&mut rig, line);
        assert_eq!(
            denied_as(&decision),
            "review-claim-admits-no-write",
            "{line}"
        );
    }
}

#[test]
fn a_file_edit_is_refused_in_the_reviewed_worktree_and_in_the_base_checkout() {
    let mut rig = new_rig();
    let inside = rig.worktree.join("README.md");
    assert_eq!(
        denied_as(&write_to(&mut rig, &inside)),
        "review-claim-admits-no-write"
    );
    let base = rig.repo.path().join("README.md");
    assert_eq!(
        denied_as(&write_to(&mut rig, &base)),
        "review-claim-admits-no-write"
    );
}

#[test]
fn a_commit_and_a_shell_edit_are_refused() {
    for line in ["git commit -m x", "sed -i '' s/a/b/ README.md"] {
        let mut rig = new_rig();
        let decision = decide_line(&mut rig, line);
        assert_eq!(
            denied_as(&decision),
            "review-claim-admits-no-write",
            "{line}"
        );
    }
}

#[test]
fn a_note_outside_every_repository_stands_aside() {
    let elsewhere = tempfile::tempdir().expect("a directory");
    let mut rig = new_rig();
    let decision = write_to(&mut rig, &elsewhere.path().join("note.md"));
    assert!(
        matches!(
            decision,
            Decision::Outside(Aside::OutsideTheClaim | Aside::AnotherCheckout)
        ),
        "{decision:?}"
    );
}

#[test]
fn reading_is_not_gated() {
    for line in ["git diff HEAD~1", "gh pr view 7", "cat README.md"] {
        let mut rig = new_rig();
        let decision = decide_line(&mut rig, line);
        assert!(
            matches!(decision, Decision::Outside(Aside::NotWatched)),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn a_pointer_written_with_an_unknown_kind_is_unreadable_rather_than_empty() {
    let root = tempfile::tempdir().expect("a state root");
    std::fs::create_dir_all(root.path()).expect("the root");
    std::fs::write(
        session::pointer_path(root.path(), RUN),
        serde_json::json!({ "run_id": RUN, "issue": null, "state": null, "repo_dir": null,
                             "verified_at": null, "kind": "something-new" })
        .to_string(),
    )
    .expect("the pointer writes");
    assert!(session::load(root.path(), RUN).unreadable);
}

#[test]
fn a_review_claim_is_never_a_holder_a_merge_could_find() {
    let rig = new_rig();
    let state = rig.context.state_root.clone();
    let mut run = rig.run.clone();
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "e".to_owned(),
        pr: PR,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    session::store(&state, &run).expect("the pointer writes");
    let merge = crate::harness::classify_observing(
        "Bash",
        &serde_json::json!({ "command": "gh pr merge 7 --merge" }),
        &[],
    )
    .0;
    assert!(guard::holders_for_action(&state, rig.repo.path(), &merge).is_empty());
    assert!(guard::holders_for_action(&state, &rig.worktree, &merge).is_empty());
    assert!(session::holdings(&state).is_empty());
    assert_eq!(session::review_holdings(&state).len(), 1);
}
