use std::path::Path;
use std::time::Duration;

use super::*;
use crate::harness::{Decision, GateContext, Sensitivity, guard, session};
use crate::outcome::Observed;

fn context(root: &Path, repo: &Path) -> GateContext {
    let skill_root = root.join("skill");
    std::fs::create_dir_all(&skill_root).expect("a skill root");
    std::fs::write(skill_root.join(crate::skill::CONTRACT), "the contract\n")
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
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn guarded() -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path());
    (repo, root, context)
}

fn unguarded() -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let repo = crate::test_env::repository_with_commit();
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path());
    (repo, root, context)
}

fn shell(context: &GateContext, repo: &Path, command: &str) -> Decision {
    let input = serde_json::json!({ "command": command });
    let (action, how, observed) = crate::harness::classify_observing("Bash", &input, &[]);
    let literal = match &observed.literal {
        crate::outcome::Established::Known(text) => Some(text.as_str()),
        _ => None,
    };
    guard::adjudicate_command(
        context,
        repo,
        &action,
        how,
        &[],
        guard::Readings {
            literal,
            same_tier_fragments: &observed.same_tier_fragments,
            write_targets: &observed.write_targets,
        },
    )
    .decision
}

fn refused_as_unclaimed(decision: &Decision) -> bool {
    matches!(decision, Decision::Deny(refusal) if refusal.code == CODE)
}

#[test]
fn a_commit_without_a_claim_is_refused_where_the_guard_is_installed() {
    let (repo, _root, context) = guarded();
    for command in [
        "git commit -m x",
        "git add . && git commit -m x",
        "git -C . commit --amend",
        "git cherry-pick abc123",
        "git revert abc123",
        "git merge topic",
        "git rebase main",
        "git am patch.mbox",
        "git restore x && git revert abc123",
    ] {
        let decision = shell(&context, repo.path(), command);
        assert!(refused_as_unclaimed(&decision), "{command}: {decision:?}");
    }
}

#[test]
fn a_commit_run_after_a_here_document_is_still_refused_without_a_claim() {
    let (repo, _root, context) = guarded();
    for command in [
        "cat <<EOF\nhello\nEOF\nsh -c 'git commit -m x'",
        "cat <<EOF\nhello\nEOF\nsh -c 'git push origin main'",
    ] {
        let decision = shell(&context, repo.path(), command);
        assert!(refused_as_unclaimed(&decision), "{command}: {decision:?}");
    }
}

#[test]
fn a_push_without_a_claim_is_refused_on_any_branch_where_the_guard_is_installed() {
    let (repo, _root, context) = guarded();
    for command in [
        "git push",
        "git push origin main",
        "git push origin topic",
        "git push -u origin HEAD",
    ] {
        let decision = shell(&context, repo.path(), command);
        assert!(refused_as_unclaimed(&decision), "{command}: {decision:?}");
    }
}

#[test]
fn the_pre_push_hook_refuses_an_unclaimed_push_where_the_guard_is_installed() {
    let (repo, _root, context) = guarded();
    let decision = guard::decide(&context, repo.path(), &[]).decision;
    assert!(refused_as_unclaimed(&decision), "{decision:?}");
}

#[test]
fn a_guarded_checkout_with_no_remote_refuses_no_commit() {
    let repo = crate::test_env::repository_with_no_remote();
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path());
    for command in ["git commit -m x", "git push origin main"] {
        let decision = shell(&context, repo.path(), command);
        assert_eq!(
            decision,
            Decision::Outside(crate::harness::Aside::NothingSworn),
            "{command}: {decision:?}"
        );
    }
    let hook = guard::decide(&context, repo.path(), &[]).decision;
    assert_eq!(hook, Decision::Outside(crate::harness::Aside::NothingSworn));
}

#[test]
fn a_named_repository_refuses_a_commit_where_the_checkout_has_no_remote() {
    let repo = crate::test_env::repository_with_no_remote();
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path(), repo.path());
    context.tracker = crate::config::Tracker::Github {
        repo: Some(crate::config::RepoRef::parse("asanabrial/estigia").expect("a repo ref")),
    };
    for command in ["git commit -m x", "git push origin main"] {
        let decision = shell(&context, repo.path(), command);
        assert!(refused_as_unclaimed(&decision), "{command}: {decision:?}");
    }
}

#[test]
fn the_refusal_names_the_claim_and_the_branch_start() {
    let (repo, _root, context) = guarded();
    let Decision::Deny(refusal) = shell(&context, repo.path(), "git commit -m x") else {
        panic!("an unclaimed commit was not refused");
    };
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("`claim`"), "{resolution}");
    assert!(resolution.contains("start_branch"), "{resolution}");
}

#[test]
fn a_session_with_no_issue_is_refused_a_commit_where_the_guard_is_installed() {
    let (_repo, _root, context) = guarded();
    let input = serde_json::json!({ "command": "git commit -m x" });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = Observed {
        command: Some(command),
        checkout: None,
        push_refs: Vec::new(),
    };
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
    assert!(refused_as_unclaimed(&decision), "{decision:?}");
}

#[test]
fn an_unclaimed_checkout_without_the_guard_stays_outside() {
    let (repo, _root, context) = unguarded();
    for command in ["git commit -m x", "git push origin main", "git merge topic"] {
        let decision = shell(&context, repo.path(), command);
        assert_eq!(
            decision,
            Decision::Outside(crate::harness::Aside::NothingSworn),
            "{command}"
        );
    }
    let hook = guard::decide(&context, repo.path(), &[]).decision;
    assert_eq!(hook, Decision::Outside(crate::harness::Aside::NothingSworn));
}

#[test]
fn reads_edits_and_tests_stay_outside_in_an_unclaimed_guarded_checkout() {
    let (repo, _root, context) = guarded();
    for command in [
        "git status",
        "git log --oneline",
        "git diff",
        "git merge-base main HEAD",
        "git pull",
        "git rebase --abort",
        "git merge --abort",
        "git cherry-pick --abort",
        "git revert --abort",
        "git am --abort",
        "git am --skip",
        "git rebase --quit",
        "git -C . rebase --abort",
        "cargo test",
        "ls -la",
        "echo done > notes.txt",
    ] {
        let decision = shell(&context, repo.path(), command);
        assert!(!refused_as_unclaimed(&decision), "{command}: {decision:?}");
    }
    let edit = crate::harness::Action::Write {
        target: "src/main.rs".to_owned(),
    };
    let decision =
        guard::adjudicate_action(&context, repo.path(), &edit, Sensitivity::Routine, &[]).decision;
    assert_eq!(
        decision,
        Decision::Outside(crate::harness::Aside::NothingSworn)
    );
}

#[test]
fn a_claimed_run_is_not_refused_as_unclaimed() {
    let (repo, _root, context) = guarded();
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.repo_dir = Some(repo.path().to_path_buf());
    session::store(&context.state_root, &run).expect("the pointer writes");
    for command in ["git commit -m x", "git push origin topic"] {
        let decision = shell(&context, repo.path(), command);
        assert!(!refused_as_unclaimed(&decision), "{command}: {decision:?}");
    }
    let hook = guard::decide(&context, repo.path(), &[]).decision;
    assert!(!refused_as_unclaimed(&hook), "{hook:?}");
}

#[test]
fn a_guarded_checkout_with_no_contract_installed_is_not_locked() {
    let (repo, _root, context) = guarded();
    std::fs::remove_file(context.skill_root.join(crate::skill::CONTRACT))
        .expect("the contract is removed");
    for command in ["git commit -m x", "git push origin topic"] {
        let decision = shell(&context, repo.path(), command);
        assert_eq!(
            decision,
            Decision::Outside(crate::harness::Aside::NothingSworn),
            "{command}"
        );
    }
}

#[test]
fn a_here_document_terminator_is_compared_in_the_case_bash_reads() {
    assert!(commits_or_pushes(
        "cat <<EOF <<'X'\neof\n$(git push origin main)\nEOF\nX"
    ));
}
