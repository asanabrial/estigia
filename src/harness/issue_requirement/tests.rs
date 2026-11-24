use std::path::Path;
use std::time::Duration;

use super::*;
use crate::config::{Config, IssueRequirement};
use crate::harness::hook;
use crate::harness::hook::{Dialect, Event, Input};
use crate::harness::{Aside, Decision, GateContext, guard, session};
use crate::outcome::{NoCommandReason, Observed, Refusal, Resolution};

fn context(root: &Path, repo: &Path, requirement: IssueRequirement) -> GateContext {
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
        issue_requirement: requirement,
        repository_config_unreadable: None,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn git_in(repo: &Path, arguments: &[&str]) {
    let answer = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()
        .expect("a usable `git` on PATH");
    assert!(
        answer.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&answer.stderr)
    );
}

fn repository_on_main() -> tempfile::TempDir {
    let repo = crate::test_env::repository_with_commit();
    git_in(repo.path(), &["checkout", "-q", "-B", "main"]);
    crate::test_env::add_github_remote(repo.path());
    repo
}

fn rig(requirement: IssueRequirement) -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let repo = repository_on_main();
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path(), requirement);
    (repo, root, context)
}

fn rig_without_a_remote(
    requirement: IssueRequirement,
) -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let repo = crate::test_env::repository_with_no_remote();
    git_in(repo.path(), &["checkout", "-q", "-B", "main"]);
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path(), requirement);
    (repo, root, context)
}

fn unclaimed_edit(context: &GateContext, tool: &str, input: serde_json::Value) -> Decision {
    let (action, how, command) = crate::harness::classify_observing(tool, &input, &[]);
    let observed = Observed {
        command: Some(command),
        checkout: None,
        push_refs: Vec::new(),
    };
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    crate::harness::gate_observing(context, &mut run, &action, how, &observed)
}

fn edit(context: &GateContext) -> Decision {
    let target = context.repo_dir.join("src.rs");
    unclaimed_edit(
        context,
        "Edit",
        serde_json::json!({ "file_path": target.display().to_string() }),
    )
}

fn commit(context: &GateContext) -> Decision {
    unclaimed_edit(
        context,
        "Bash",
        serde_json::json!({ "command": "git commit -m x" }),
    )
}

fn hook_answer(context: &GateContext, dialect: Dialect, agent: &str) -> serde_json::Value {
    hook_answer_as(context, dialect, agent, None)
}

fn hook_answer_as(
    context: &GateContext,
    dialect: Dialect,
    agent: &str,
    agent_type: Option<&str>,
) -> serde_json::Value {
    let input = Input {
        session_id: "abcd1234".to_owned(),
        cwd: context.repo_dir.display().to_string(),
        tool_name: "Bash".to_owned(),
        tool_input: serde_json::json!({ "command": "git commit -m x" }),
        agent_type: agent_type.map(str::to_owned),
        ..Input::default()
    };
    crate::harness::hook::run_as(
        dialect,
        Some(agent),
        Event::PreToolUse,
        &input,
        Some(context),
    )
}

#[test]
fn required_refuses_an_unclaimed_commit_on_the_default_branch_and_names_what_clears_it() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let Decision::Deny(refusal) = commit(&context) else {
        panic!("an unclaimed commit on the default branch was not refused");
    };
    assert_eq!(refusal.code, CODE);
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("`claim`"), "{resolution}");
    assert!(resolution.contains("`create`"), "{resolution}");
    assert!(
        refusal.message.contains("default branch"),
        "{}",
        refusal.message
    );
}

#[test]
fn required_is_delivered_as_a_refusal_on_every_host() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for (dialect, agent) in [
        (Dialect::ClaudeCode, "claude-code"),
        (Dialect::ClaudeCode, "codex"),
        (Dialect::ClaudeCode, "continue"),
        (Dialect::GeminiCli, "gemini-cli"),
        (Dialect::Cursor, "cursor"),
        (Dialect::ExitCode, "windsurf"),
        (Dialect::Cline, "cline"),
    ] {
        let answer = hook_answer(&context, dialect, agent).to_string();
        assert!(!answer.contains("\"ask\""), "{agent}: {answer}");
        assert!(answer.contains("issue-required"), "{agent}: {answer}");
        assert!(
            answer.contains("deny") || answer.contains("review") || answer.contains("\"status\":2"),
            "{agent}: {answer}"
        );
    }
}

#[test]
fn an_unclaimed_commit_on_the_default_branch_through_the_gate_command_is_refused() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let input = serde_json::json!({ "command": "git commit -m x" });
    let (action, how, _) = crate::harness::classify_observing("Bash", &input, &[]);
    let readings = guard::Readings {
        literal: Some("git commit -m x"),
        ..guard::Readings::default()
    };
    let decision =
        guard::adjudicate_command(&context, &context.repo_dir, &action, how, &[], readings)
            .decision;
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn a_push_the_pre_push_hook_reports_is_held_by_its_destination() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let held = guard::decide_pushing(
        &context,
        &context.repo_dir,
        &["refs/heads/side".to_owned()],
        &["refs/heads/main".to_owned()],
    )
    .decision;
    assert!(held_for_an_issue(&held), "{held:?}");
    let passes = guard::decide_pushing(
        &context,
        &context.repo_dir,
        &["refs/heads/main".to_owned()],
        &["refs/heads/side".to_owned()],
    )
    .decision;
    assert!(
        matches!(&passes, Decision::Outside(_)),
        "a push of the default branch to a side branch was held: {passes:?}"
    );
    let unknown = guard::decide(&context, &context.repo_dir, &[]).decision;
    assert!(held_for_an_issue(&unknown), "{unknown:?}");
}

fn repository_file_row(value: &str) -> String {
    format!(
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Issue required | {value} | off |\n"
    )
}

fn a_commit_after_storing(stored: &str) -> (IssueRequirement, Decision) {
    let repo = repository_on_main();
    let path = crate::skill::repository_config_path(repo.path());
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    std::fs::write(&path, repository_file_row(stored)).expect("the repository file");
    let layered = crate::skill::layer_repository(&Config::default(), repo.path())
        .expect("the repository layers");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path(), layered.issue_requirement);
    (layered.issue_requirement, commit(&context))
}

#[test]
fn a_stored_watch_reads_as_on_and_refuses_an_unclaimed_commit_on_the_default_branch() {
    let (read, decision) = a_commit_after_storing("watch");
    assert_eq!(read, IssueRequirement::On);
    assert!(held_for_an_issue(&decision), "{decision:?}");
}

#[test]
fn a_stored_ask_reads_as_on_and_refuses_an_unclaimed_commit_on_the_default_branch() {
    let (read, decision) = a_commit_after_storing("ask");
    assert_eq!(read, IssueRequirement::On);
    assert!(held_for_an_issue(&decision), "{decision:?}");
}

const LEFTOVER_ROW: &str = "| Unclaimed GitHub writes | allowed | held |\n";

fn layered_with_the_leftover_row(
    value: &str,
) -> (tempfile::TempDir, tempfile::TempDir, Config, GateContext) {
    let repo = repository_on_main();
    let path = crate::skill::repository_config_path(repo.path());
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    std::fs::write(
        &path,
        format!("{}{LEFTOVER_ROW}", repository_file_row(value)),
    )
    .expect("the repository file");
    let layered = crate::skill::layer_repository(&Config::default(), repo.path())
        .expect("a repository file with the removed row stays readable");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path(), layered.issue_requirement);
    (repo, root, layered, context)
}

#[test]
fn a_leftover_unclaimed_github_writes_row_is_ignored_and_the_file_stays_readable() {
    let (_repo, _root, layered, _context) = layered_with_the_leftover_row("off");
    assert_eq!(layered.issue_requirement, IssueRequirement::Off);
}

#[test]
fn a_leftover_unclaimed_github_writes_row_does_not_let_an_unclaimed_merge_through() {
    let (_repo, _root, layered, context) = layered_with_the_leftover_row("on");
    assert_eq!(layered.issue_requirement, IssueRequirement::On);
    let decision = on_a_shell_line(&context, "gh pr merge 7 --merge");
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

fn line_decided(context: &GateContext, line: &str) -> Decision {
    unclaimed_edit(context, "Bash", serde_json::json!({ "command": line }))
}

#[test]
fn an_unclaimed_title_or_body_edit_passes_under_on() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for line in crate::harness::text_edit::tests::ADMITTED {
        let decision = line_decided(&context, line);
        assert!(
            matches!(decision, Decision::Outside(_)),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn an_unclaimed_api_mutation_that_is_more_than_text_is_still_held_under_on() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let api = crate::harness::text_edit::tests::EXCLUDED
        .iter()
        .filter(|line| line.starts_with("gh api"));
    for line in api {
        let decision = line_decided(&context, line);
        assert!(
            matches!(decision, Decision::Deny(_)),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn off_changes_nothing() {
    let (_repo, _root, context) = rig(IssueRequirement::Off);
    assert_eq!(commit(&context), Decision::Outside(Aside::NothingSworn));
}

#[test]
fn a_checkout_with_no_remote_is_not_held_for_an_issue() {
    let (_repo, _root, context) = rig_without_a_remote(IssueRequirement::On);
    for line in ["git commit -m x", "git merge topic", "git push origin main"] {
        let decision = on_a_shell_line(&context, line);
        assert_eq!(
            decision,
            Decision::Outside(Aside::NothingSworn),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn a_no_remote_checkout_answers_the_write_hook_with_nothing() {
    let (_repo, _root, context) = rig_without_a_remote(IssueRequirement::On);
    let answer = hook_answer(&context, Dialect::ClaudeCode, "claude-code");
    assert_eq!(answer, serde_json::json!({}), "{answer}");
}

#[test]
fn adding_a_remote_holds_the_same_write_again() {
    let (repo, _root, context) = rig_without_a_remote(IssueRequirement::On);
    assert_eq!(commit(&context), Decision::Outside(Aside::NothingSworn));
    crate::test_env::add_github_remote(repo.path());
    let decision = commit(&context);
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn a_named_repository_holds_the_commit_where_the_checkout_has_no_remote() {
    let (_repo, _root, mut context) = rig_without_a_remote(IssueRequirement::On);
    context.tracker = crate::config::Tracker::Github {
        repo: Some(crate::config::RepoRef::parse("asanabrial/estigia").expect("a repo ref")),
    };
    let decision = commit(&context);
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn a_commit_into_a_nested_checkout_that_has_a_remote_is_still_held() {
    let (repo, _root, context) = rig_without_a_remote(IssueRequirement::On);
    let nested = repo.path().join("nested");
    std::fs::create_dir(&nested).expect("a directory for the nested checkout");
    crate::test_env::repository_at(&nested);
    git_in(&nested, &["checkout", "-q", "-B", "main"]);
    crate::test_env::add_github_remote(&nested);
    let line = format!(
        "git -C {} commit -m x",
        crate::test_env::shell_path(&nested)
    );
    let decision = on_a_shell_line(&context, &line);
    assert!(held_for_an_issue(&decision), "{decision:?}");
}

fn hook_session_start(context: &GateContext) {
    let input = Input {
        session_id: "abcd1234".to_owned(),
        cwd: context.repo_dir.display().to_string(),
        ..Input::default()
    };
    hook::run_as(
        Dialect::ClaudeCode,
        Some("claude-code"),
        Event::SessionStart,
        &input,
        Some(context),
    );
}

fn told(answer: &serde_json::Value) -> Option<&str> {
    answer["hookSpecificOutput"]["additionalContext"].as_str()
}

#[test]
fn a_refused_unclaimed_write_carries_no_extra_text() {
    let (repo, _root, context) = rig_without_a_remote(IssueRequirement::On);
    hook_session_start(&context);
    crate::test_env::add_github_remote(repo.path());
    let answer = hook_answer(&context, Dialect::ClaudeCode, "claude-code");
    assert_eq!(
        answer["hookSpecificOutput"]["permissionDecision"], "deny",
        "{answer}"
    );
    assert_eq!(told(&answer), None, "{answer}");
}

#[test]
fn a_hold_the_session_start_already_announced_is_not_told_again() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    hook_session_start(&context);
    let answer = hook_answer(&context, Dialect::ClaudeCode, "claude-code");
    assert_eq!(
        answer["hookSpecificOutput"]["permissionDecision"], "deny",
        "{answer}"
    );
    assert_eq!(told(&answer), None, "{answer}");
}

#[test]
fn a_session_in_a_checkout_with_no_remote_is_told_nothing_is_gated() {
    let (_repo, _root, context) = rig_without_a_remote(IssueRequirement::On);
    std::fs::write(context.repo_dir.join("notes.txt"), "x\n").expect("a dirty file");
    let text = session_start_text(&context);
    assert!(text.contains("no git remote"), "{text}");
    for wording in [
        "on the default branch",
        "This checkout has uncommitted changes",
    ] {
        assert!(!text.contains(wording), "{text}");
    }
}

#[test]
fn a_failed_remote_read_is_not_zero_remotes() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let elsewhere = non_repository();
    let answer = std::process::Command::new("git")
        .arg("-C")
        .arg(elsewhere.path())
        .arg("remote")
        .output()
        .expect("a usable `git` on PATH");
    assert!(
        !answer.status.success(),
        "the fixture's `git remote` answered successfully, so it cannot stand for a failed read"
    );
    assert!(
        names_an_issue(&context, elsewhere.path()),
        "a failed remote read was read as zero remotes"
    );
}

#[test]
fn a_checkout_where_nothing_can_be_claimed_is_unaffected_under_every_value() {
    for requirement in IssueRequirement::all() {
        let (_repo, _root, mut context) = rig(requirement);
        std::fs::remove_file(context.skill_root.join(crate::skill::CONTRACT)).expect("contract");
        assert_eq!(
            commit(&context),
            Decision::Outside(Aside::NothingSworn),
            "{requirement:?}"
        );
        context.issue_requirement = requirement;
    }
}

#[test]
fn a_write_outside_the_checkout_is_not_a_repository_write() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let elsewhere = tempfile::tempdir().expect("another directory");
    let decision = unclaimed_edit(
        &context,
        "Write",
        serde_json::json!({ "file_path": elsewhere.path().join("notes.md").display().to_string() }),
    );
    assert_eq!(decision, Decision::Outside(Aside::NothingSworn));
}

#[test]
fn reads_and_estigias_own_commands_are_unaffected() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for (tool, input) in [
        ("Read", serde_json::json!({ "file_path": "src.rs" })),
        (
            "Bash",
            serde_json::json!({ "command": "git status && cargo test" }),
        ),
        ("Bash", serde_json::json!({ "command": "estigia claim 12" })),
    ] {
        let decision = unclaimed_edit(&context, tool, input.clone());
        assert!(
            !matches!(decision, Decision::Deny(_) | Decision::Ask(_)),
            "{tool} {input}: {decision:?}"
        );
    }
}

fn push(context: &GateContext) -> Decision {
    unclaimed_edit(
        context,
        "Bash",
        serde_json::json!({ "command": "git push origin main" }),
    )
}

#[test]
fn an_unclaimed_push_is_held_as_an_unclaimed_commit_is() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let decision = push(&context);
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
    let (_repo, _root, context) = rig(IssueRequirement::Off);
    assert_eq!(push(&context), Decision::Outside(Aside::NothingSworn));
}

fn on_a_shell_line(context: &GateContext, line: &str) -> Decision {
    unclaimed_edit(context, "Bash", serde_json::json!({ "command": line }))
}

const GITHUB_WRITES: &[&str] = &[
    "gh pr merge 3 --merge",
    "gh api -X POST repos/o/r/pulls/3/reviews -f event=COMMENT",
    "git tag -a v1 -m v1",
    "git merge topic",
    "gh release create v1",
];

const GITHUB_WRITES_THAT_PASS: &[&str] = &[
    "gh pr review 3 --approve",
    "gh pr comment 3 --body x",
    "gh pr edit 3 --title x",
    "gh pr create --title x --body y",
];

const GITHUB_READS: &[&str] = &[
    "gh pr view 3 --json state --jq .state",
    "gh api repos/o/r/pulls/3/reviews --jq '.[]|.state'",
];

#[test]
fn an_unclaimed_github_write_is_refused_under_required() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for line in GITHUB_WRITES {
        let decision = on_a_shell_line(&context, line);
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn a_github_write_that_is_no_integration_passes_under_every_value() {
    for requirement in IssueRequirement::all() {
        let (_repo, _root, context) = rig(requirement);
        for line in GITHUB_WRITES_THAT_PASS {
            let decision = on_a_shell_line(&context, line);
            assert!(passes(&decision), "{requirement:?} {line}: {decision:?}");
        }
    }
}

#[test]
fn an_unclaimed_github_write_passes_when_the_policy_is_off() {
    let (_repo, _root, context) = rig(IssueRequirement::Off);
    for line in GITHUB_WRITES {
        assert!(
            !matches!(
                on_a_shell_line(&context, line),
                Decision::Ask(_) | Decision::Deny(_)
            ),
            "{line}"
        );
    }
}

#[test]
fn a_github_read_passes_under_every_value() {
    for requirement in IssueRequirement::all() {
        let (_repo, _root, context) = rig(requirement);
        for line in GITHUB_READS {
            let decision = on_a_shell_line(&context, line);
            assert!(
                !matches!(&decision, Decision::Ask(_) | Decision::Deny(_)),
                "{requirement:?} {line}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_github_write_under_a_read_only_tracker_is_refused_under_required() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let context = under_linear(context);
    let decision = on_a_shell_line(&context, "gh pr merge 3 --merge");
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn an_unreadable_repository_file_requires_a_tracker_that_can_claim() {
    let (_repo, _root, mut context) = rig(IssueRequirement::Off);
    context.repository_config_unreadable = Some(Refusal::not_started(
        "config-local-unreadable",
        "the repository file: stream did not contain valid UTF-8",
        Resolution::no_command(NoCommandReason::OperatorKnowledge, "that file readable"),
    ));
    assert_eq!(
        in_force(&context),
        Some(IssueRequirement::On),
        "an unreadable repository file did not read as required"
    );

    let mut unadjudicated = context.clone();
    unadjudicated.tracker = crate::config::Tracker::Linear {
        team: "T".to_owned(),
    };
    unadjudicated.workflow = Some(crate::config::Workflow::Autonomous);
    assert_eq!(
        in_force(&unadjudicated),
        None,
        "a tracker that cannot adjudicate was turned into a lock"
    );

    std::fs::remove_file(context.skill_root.join(crate::skill::CONTRACT))
        .expect("the contract is removed");
    assert_eq!(
        in_force(&context),
        None,
        "a checkout with no installed contract was turned into a lock"
    );
}

#[test]
fn a_github_write_from_outside_every_repository_is_left_alone() {
    let (_repo, _root, mut context) = rig(IssueRequirement::On);
    let elsewhere = non_repository();
    context.repo_dir = elsewhere.path().to_path_buf();
    assert_eq!(
        on_a_shell_line(&context, "gh pr merge 3 --merge"),
        Decision::Outside(Aside::NothingSworn)
    );
}

#[test]
fn an_unclaimed_github_write_is_held_through_the_gate_command_too() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for line in ["git tag -a v1 -m v1", "gh pr merge 3 --merge"] {
        let input = serde_json::json!({ "command": line });
        let (action, how, _) = crate::harness::classify_observing("Bash", &input, &[]);
        let readings = guard::Readings {
            literal: Some(line),
            ..guard::Readings::default()
        };
        let decision =
            guard::adjudicate_command(&context, &context.repo_dir, &action, how, &[], readings)
                .decision;
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn a_run_holding_a_claim_is_not_held_for_a_github_write() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let input = serde_json::json!({ "command": "gh pr review 3 --approve" });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = Observed {
        command: Some(command),
        checkout: None,
        push_refs: Vec::new(),
    };
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(7));
    let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
    assert!(
        !matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn the_refusal_offers_only_the_requirement_row_as_a_way_in() {
    for (workflow, line) in [
        (None, "gh pr merge 3"),
        (
            Some(crate::config::Workflow::Assisted),
            "gh api -X DELETE repos/o/r/pulls/3/reviews/4",
        ),
        (None, "git commit -m x"),
        (None, "git push origin main"),
        (None, "gh pr merge 3; git commit -m x"),
    ] {
        let (_repo, _root, mut context) = rig(IssueRequirement::On);
        context.workflow = workflow;
        let decision = on_a_shell_line(&context, line);
        let Decision::Deny(refusal) = decision else {
            panic!("{line}: not refused: {decision:?}");
        };
        let text = refusal.to_string();
        assert!(!text.contains("Unclaimed GitHub writes"), "{line}: {text}");
        assert!(
            text.contains("`Issue required` set to `off`"),
            "{line}: {text}"
        );
    }
}

#[test]
fn the_operation_windows_refusal_still_decides_under_required() {
    let (_repo, _root, mut context) = rig(IssueRequirement::On);
    context.operation_window = crate::config::GitOperationWindow::Invalid;
    let decision = on_a_shell_line(&context, "gh pr review 3 --approve");
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == "git-operation-window-denied"),
        "{decision:?}"
    );
}

const MODIFICATIONS_BESIDE_A_BOUNDARY: &[&str] = &[
    "git pull; git worktree remove /some/path",
    "git pull && sleep 3; git -C ~/repo worktree remove \"$WT\" && echo removed",
    "gh pr create --title x; git branch new",
    "gh pr create --title x && git stash",
    "git pull && echo x > out.txt",
    "git pull; git rm src.rs",
];

const WORK_FOR_THE_DEFAULT_BRANCH_BESIDE_A_BOUNDARY: &[&str] = &[
    "git pull; git commit -m x",
    "gh pr create --title x && git commit -m x",
    "git push origin main; git branch x",
    "git tag -a v1 -m v1; git commit -m x",
];

#[test]
fn a_modification_beside_a_boundary_is_not_held_under_required() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for line in MODIFICATIONS_BESIDE_A_BOUNDARY {
        let decision = on_a_shell_line(&context, line);
        assert!(passes(&decision), "{line}: {decision:?}");
    }
}

#[test]
fn work_for_the_default_branch_beside_a_boundary_is_held_under_required() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for line in WORK_FOR_THE_DEFAULT_BRANCH_BESIDE_A_BOUNDARY {
        let decision = on_a_shell_line(&context, line);
        assert!(held_for_an_issue(&decision), "{line}: {decision:?}");
    }
}

#[test]
fn what_is_held_beside_a_boundary_passes_when_the_policy_is_off() {
    let (_repo, _root, context) = rig(IssueRequirement::Off);
    for line in MODIFICATIONS_BESIDE_A_BOUNDARY
        .iter()
        .chain(WORK_FOR_THE_DEFAULT_BRANCH_BESIDE_A_BOUNDARY)
    {
        let decision = on_a_shell_line(&context, line);
        assert!(
            matches!(&decision, Decision::Outside(Aside::NothingSworn)),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn a_boundary_with_only_reads_beside_it_is_held_as_a_boundary() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for line in [
        "gh pr merge 3 --merge",
        "gh pr merge 3 --merge 2>&1 | tail -1",
        "git tag -a v1 -m v1; git log -1",
    ] {
        let decision = on_a_shell_line(&context, line);
        assert!(
            matches!(&decision, Decision::Deny(_)),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn the_push_guards_refusal_is_still_the_one_a_guarded_commit_gets() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path(), IssueRequirement::On);
    let decision = unclaimed_edit(
        &context,
        "Bash",
        serde_json::json!({ "command": "git commit -m x" }),
    );
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == crate::harness::unclaimed::CODE),
        "{decision:?}"
    );
}

fn under_linear(mut context: GateContext) -> GateContext {
    context.tracker = crate::config::Tracker::Linear {
        team: "ENG".to_owned(),
    };
    context.workflow = Some(crate::config::Workflow::Assisted);
    context
}

#[test]
fn a_guarded_checkout_under_a_read_only_tracker_refuses_an_unsworn_commit() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = under_linear(context(root.path(), repo.path(), IssueRequirement::Off));
    for command in ["git commit -m x", "git push origin main"] {
        let decision = unclaimed_edit(&context, "Bash", serde_json::json!({ "command": command }));
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == crate::harness::unclaimed::CODE),
            "{command}: {decision:?}"
        );
    }
}

#[test]
fn required_refuses_an_unclaimed_commit_on_the_default_branch_under_a_read_only_tracker() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let context = under_linear(context);
    let Decision::Deny(refusal) = commit(&context) else {
        panic!("an unclaimed commit was not refused under a read-only tracker");
    };
    assert_eq!(refusal.code, CODE);
}

#[test]
fn a_read_only_tracker_without_an_assisted_workflow_refuses_nothing() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    for workflow in [None, Some(crate::config::Workflow::Autonomous)] {
        let mut context = under_linear(context(root.path(), repo.path(), IssueRequirement::On));
        context.workflow = workflow;
        assert!(in_force(&context).is_none(), "{workflow:?}");
        let decision = unclaimed_edit(
            &context,
            "Bash",
            serde_json::json!({ "command": "git commit -m x" }),
        );
        assert!(
            !matches!(&decision, Decision::Deny(refusal) if refusal.code == crate::harness::unclaimed::CODE),
            "{workflow:?}: {decision:?}"
        );
    }
}

#[test]
fn an_assisted_run_is_told_to_assist_not_to_claim() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = under_linear(context(root.path(), repo.path(), IssueRequirement::On));
    let Decision::Deny(refusal) = unclaimed_edit(
        &context,
        "Bash",
        serde_json::json!({ "command": "git commit -m x" }),
    ) else {
        panic!("an unclaimed commit was not refused");
    };
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("`assist`"), "{resolution}");
    assert!(!resolution.contains("start_branch"), "{resolution}");
    let Decision::Deny(refusal) = on_a_shell_line(&context, "gh pr merge 3") else {
        panic!("an unclaimed pull request merge was not refused");
    };
    assert_eq!(refusal.code, CODE);
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("`assist`"), "{resolution}");
    assert!(!resolution.contains("start_branch"), "{resolution}");
}

#[test]
fn an_assisted_run_under_github_is_told_to_assist_not_to_claim() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path(), repo.path(), IssueRequirement::On);
    context.workflow = Some(crate::config::Workflow::Assisted);
    let Decision::Deny(refusal) = unclaimed_edit(
        &context,
        "Bash",
        serde_json::json!({ "command": "git commit -m x" }),
    ) else {
        panic!("an unclaimed commit was not refused");
    };
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("`assist`"), "{resolution}");
    for tool in ["`claim`", "`create`", "`start_branch`"] {
        assert!(!resolution.contains(tool), "{resolution}");
    }
    let Decision::Deny(refusal) = on_a_shell_line(&context, "gh pr merge 3") else {
        panic!("an unclaimed pull request merge was not refused");
    };
    assert_eq!(refusal.code, CODE);
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("`assist`"), "{resolution}");
    for tool in ["`claim`", "`create`", "`start_branch`"] {
        assert!(!resolution.contains(tool), "{resolution}");
    }
}

fn session_start_text(context: &GateContext) -> String {
    let run = session::Run::new("opencode-abcd1234".to_owned());
    let response = hook::session_start_response(context, &run);
    response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context is injected")
        .to_owned()
}

#[test]
fn an_assisted_session_start_names_assist_and_not_the_refused_tools() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    // The third arm is reached with no push guard and nothing required: Estigia
    // gates nothing there, and the text is the only thing the agent reads.
    let unguarded = crate::test_env::repository_with_commit();
    let root = tempfile::tempdir().expect("a temporary root");
    for tracker in [
        crate::config::Tracker::Github { repo: None },
        crate::config::Tracker::Linear {
            team: "ENG".to_owned(),
        },
    ] {
        for (at, requirement) in [
            (repo.path(), IssueRequirement::Off),
            (repo.path(), IssueRequirement::On),
            (unguarded.path(), IssueRequirement::Off),
        ] {
            let mut context = context(root.path(), at, requirement);
            context.tracker = tracker.clone();
            context.workflow = Some(crate::config::Workflow::Assisted);
            let text = session_start_text(&context);
            assert!(
                text.contains("`assist`"),
                "{tracker:?} {requirement:?}: {text}"
            );
            for tool in ["`claim`", "`create`", "`start_branch`"] {
                assert!(
                    !text.contains(tool),
                    "{tracker:?} {requirement:?} names {tool}: {text}"
                );
            }
        }
    }
}

#[test]
fn a_claimed_run_is_unaffected_under_every_value() {
    for requirement in IssueRequirement::all() {
        let (_repo, _root, context) = rig(requirement);
        let target = context.repo_dir.join("src.rs");
        for (tool, input) in [
            (
                "Edit",
                serde_json::json!({ "file_path": target.display().to_string() }),
            ),
            (
                "Bash",
                serde_json::json!({ "command": "sed -i '' s/a/b/ src.rs" }),
            ),
            ("Bash", serde_json::json!({ "command": "git commit -m x" })),
        ] {
            let mut run = session::Run::new("claude-abcd1234".to_owned());
            run.issue = Some(crate::identity::Identity::Number(12));
            let (action, how, _) = crate::harness::classify_observing(tool, &input, &[]);
            let decision = crate::harness::gate(&context, &mut run, &action, how);
            assert!(
                !matches!(&decision, Decision::Ask(_))
                    && !matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
                "{requirement:?} {tool}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_stand_down_lets_a_refused_write_through() {
    let (_repo, _root, mut context) = rig(IssueRequirement::On);
    context.stand_down = Some(crate::harness::standdown::StandDown {
        declared_by: "operator".to_owned(),
        reason: "incident".to_owned(),
        declared_at: session::now_seconds().expect("a clock"),
        until: session::now_seconds().expect("a clock") + 600,
    });
    let decision = commit(&context);
    assert!(matches!(decision, Decision::Allow(_)), "{decision:?}");
}

#[test]
fn the_row_defaults_to_off_and_only_the_repository_file_sets_it() {
    assert_eq!(Config::default().issue_requirement, IssueRequirement::Off);
    let setting = crate::config::Setting::IssueRequirement;
    assert_eq!(setting.scope(), crate::config::Scope::Repository);
    assert!(!crate::config::AGENT_SETTINGS.contains(&setting));
    assert!(!crate::config::EVERYWHERE_SETTINGS.contains(&setting));
    assert!(!crate::config::MACHINE_SETTINGS.contains(&setting));
    for value in ["on", "off"] {
        let mut config = Config::default();
        setting.apply(&mut config, value).expect("an offered value");
        assert_eq!(setting.value_of(&config), value);
    }
    for retired in ["required", "watch", "ask", "sometimes"] {
        assert!(
            setting.apply(&mut Config::default(), retired).is_err(),
            "{retired}"
        );
    }
    for stored in ["required", "watch", "ask"] {
        let mut config = Config::default();
        setting
            .apply_stored(&mut config, stored)
            .expect("a stored value still reads");
        assert_eq!(config.issue_requirement, IssueRequirement::On);
    }
    assert!(
        setting
            .apply_stored(&mut Config::default(), "sometimes")
            .is_err()
    );
}

#[test]
fn the_repository_file_sets_it_and_the_contract_and_agent_files_do_not() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    let path = crate::skill::repository_config_path(repo.path());
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    std::fs::write(
        &path,
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Issue required | on | off |\n",
    )
    .expect("the repository file");
    let layered = crate::skill::layer_repository(&Config::default(), repo.path())
        .expect("the repository layers");
    assert_eq!(layered.issue_requirement, IssueRequirement::On);

    let (from_contract, owned) = Config::read_scope_over(
        &Config::default(),
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Issue required | on | off |\n",
        crate::config::Scope::Agent,
    )
    .expect("a read");
    assert_eq!(from_contract.issue_requirement, IssueRequirement::Off);
    assert!(owned.is_empty());
    let (from_contract, _) = Config::read_scope_over(
        &Config::default(),
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Issue required | on | off |\n",
        crate::config::Scope::Everywhere,
    )
    .expect("a read");
    assert_eq!(from_contract.issue_requirement, IssueRequirement::Off);
}

#[test]
fn a_contract_row_for_it_is_read_by_nobody_and_the_repository_file_renders_it() {
    let config = Config {
        issue_requirement: IssueRequirement::On,
        ..Config::default()
    };
    assert!(!config.render_rows().contains("Issue required"));
    let rendered = config.render_some_repository_rows(&[crate::config::Setting::IssueRequirement]);
    assert!(
        rendered.contains("| Issue required | on | off |"),
        "{rendered}"
    );
    let read = Config::read(
        &crate::config::CONFIG_FENCE.upsert("", &rendered),
        Some(&rendered),
    )
    .expect("a table");
    assert_eq!(read.issue_requirement, IssueRequirement::Off);
}

fn holder_of(context: &GateContext, worktree: &Path) {
    let mut holder = session::Run::new("opencode-aaaa1111".to_owned());
    holder.issue = Some(crate::identity::Identity::Number(12));
    holder.repo_dir = Some(context.repo_dir.clone());
    holder.worktree = Some(worktree.to_path_buf());
    session::store(&context.state_root, &holder).expect("the holder's pointer is stored");
}

#[test]
fn a_commit_in_the_checkout_a_claim_names_is_not_held_off_the_default_branch() {
    let (_repo, _root, mut context) = rig(IssueRequirement::On);
    let worktree = repository_on_main();
    git_in(worktree.path(), &["checkout", "-q", "-b", "side"]);
    holder_of(&context, worktree.path());
    context.repo_dir = worktree.path().to_path_buf();
    let decision = commit(&context);
    assert!(passes(&decision), "{decision:?}");
}

#[test]
fn the_primary_checkout_stays_refused_while_a_claim_names_another() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    let worktree = repository_on_main();
    holder_of(&context, worktree.path());
    assert!(held_for_an_issue(&commit(&context)));
}

fn shell_write(context: &GateContext, command: &str) -> Decision {
    unclaimed_edit(context, "Bash", serde_json::json!({ "command": command }))
}

const MODIFICATIONS_NO_PLACEMENT_CAN_BOUND: &[&str] = &[
    "sed -i '' s/a/b/ existing.txt",
    "perl -pi -e s/a/b/ existing.txt",
    "python3 -c \"open('existing.txt','w').write('x')\"",
    "cat > new.txt <<EOF\nx\nEOF",
    "cat > new.txt <<'EOF'\nx\nEOF",
    "bash -c 'echo x > src/lib.rs'",
    "echo x > $f",
    "echo x > out.txt",
    "echo x > /tmp/y",
    "cp a /tmp/b",
    "rg --pre /bin/rm x",
    "rg --pre=/tmp/s.sh x > /tmp/out",
    "echo 'echo x > src/f' | sh > /tmp/y",
    "git reset --hard",
    "git checkout -b x",
    "echo \"$(git stash)\"",
    "x=$(git reset --hard)",
    "python3 -c \"import os; os.system('/usr/bin/git stash')\"",
];

#[test]
fn a_modification_is_never_held_for_being_unplaceable() {
    let (_repo, _root, in_a_repository) = rig(IssueRequirement::On);
    let (_elsewhere, _other_root, outside) = in_a_non_repository(IssueRequirement::On);
    for context in [&in_a_repository, &outside] {
        for command in MODIFICATIONS_NO_PLACEMENT_CAN_BOUND {
            let decision = shell_write(context, command);
            assert!(passes(&decision), "{command}: {decision:?}");
        }
    }
}

#[test]
fn a_commit_at_an_unestablished_scope_is_held() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::On);
    std::fs::create_dir(launch.path().join(".git")).expect("a `.git` entry Git cannot use");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), launch.path(), IssueRequirement::On);
    assert!(
        matches!(scope_of(launch.path()), Scope::Unknown),
        "{} was established, so this test never reaches the unknown scope",
        launch.path().display()
    );
    for command in MODIFICATIONS_NO_PLACEMENT_CAN_BOUND {
        let decision = shell_write(&context, command);
        assert!(!held_for_an_issue(&decision), "{command}: {decision:?}");
    }
    let command = "echo x > $f; git commit -am x";
    let Decision::Deny(refusal) = shell_write(&context, command) else {
        panic!("{command} was not refused");
    };
    assert_eq!(refusal.code, CODE);
    assert!(
        refusal.message.contains("could not be established"),
        "{command}: {}",
        refusal.message
    );
    assert!(
        !refusal.message.contains("this repository"),
        "{}",
        refusal.message
    );
}

#[test]
fn a_git_write_inside_a_command_substitution_is_held_when_it_commits() {
    let (repo, _root, context) = rig(IssueRequirement::On);
    let own = crate::test_env::shell_path(repo.path());
    let other = repository_on_main();
    let clone = crate::test_env::shell_path(other.path());
    for command in [
        "x=\"$(git commit -m x)\"".to_owned(),
        "echo `git commit -m x`".to_owned(),
        format!("echo \"$(git -C {own} commit -m x)\""),
        format!("cd {clone} && echo \"$(git -C {own} commit -m x)\""),
    ] {
        let decision = shell_write(&context, &command);
        assert!(held_for_an_issue(&decision), "{command}: {decision:?}");
    }
    for command in [
        "echo \"$(git status)\"".to_owned(),
        "echo \"$(git log -1)\"".to_owned(),
    ] {
        let decision = shell_write(&context, &command);
        assert!(passes(&decision), "{command}: {decision:?}");
    }
}

#[test]
fn a_heredoc_is_a_modification_and_its_body_is_data_not_a_command() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for command in [
        "cat > /tmp/a <<'EOF'\nx\nEOF",
        "cat > out.md <<'EOF'\nx\nEOF",
        "cat > /tmp/a <<EOF\nx\nEOF",
        "cat > /tmp/a <<'EOF'\nrun git commit here\nEOF",
        "cat > /tmp/a <<'EOF'\nrun git push origin main\nEOF",
        "cat > /tmp/a <<'EOF'\n$(rm -rf src)\nEOF",
    ] {
        let decision = shell_write(&context, command);
        assert!(passes(&decision), "{command}: {decision:?}");
    }
}

#[test]
fn quoting_the_exemption_refuses_does_not_make_a_plain_read_a_write() {
    for command in [r#"grep "a\.b" src"#, "printf $'x\\n'"] {
        assert_eq!(
            crate::harness::shell::writes_a_file(command),
            None,
            "{command}"
        );
    }
}

#[test]
fn a_commit_or_a_merge_the_line_spells_out_is_held_beside_a_modification() {
    let (_repo, _root, context) = rig(IssueRequirement::On);
    for command in [
        "git commit -m x > /tmp/log",
        "echo x > $f && git commit -am x",
        "echo x > $f; gh pr merge 1",
        "git push origin main > /tmp/log",
        "git commit -m x 2>&1 | tail -1",
    ] {
        let decision = shell_write(&context, command);
        assert!(held_for_an_issue(&decision), "{command}: {decision:?}");
    }
}

#[test]
fn a_git_write_sent_into_another_checkout_is_read_in_that_checkout() {
    let (repo, _root, mut required) = rig(IssueRequirement::On);
    git_in(repo.path(), &["checkout", "-q", "-b", "side"]);
    let other = repository_on_main();
    let clone = crate::test_env::shell_path(other.path());
    let own = crate::test_env::shell_path(repo.path());
    for command in [
        format!("cd {clone} && git commit -m x"),
        format!("git -C {clone} commit -m x"),
        format!("git -C {own} commit -m x && git -C {clone} commit -m y"),
        "cd \"$DIR\" && git commit -m x".to_owned(),
        "cd clone && git commit -m x".to_owned(),
        format!("cd {clone} ; git commit -m x"),
        format!("cd {clone} && GIT_DIR={own}/.git git commit -m x"),
        format!("cd {clone} && git --git-dir={own}/.git commit -m x"),
        format!("cd {clone} && (cd {own}; git commit -m x)"),
        format!("cd {clone} && git push origin main"),
    ] {
        let decision = shell_write(&required, &command);
        assert!(held_for_an_issue(&decision), "{command}: {decision:?}");
    }
    for command in [
        format!("cd {own} && git commit -m x"),
        format!("git -C {own} commit -m x"),
        format!("git -C {own} push -u origin side"),
    ] {
        let decision = shell_write(&required, &command);
        assert!(passes(&decision), "{command}: {decision:?}");
    }
    required.repo_dir = other.path().to_path_buf();
    let decision = shell_write(&required, &format!("git -C {own} commit -m x"));
    assert!(passes(&decision), "{decision:?}");
}

#[test]
fn a_dirty_checkout_is_not_reported_at_session_start() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    let root = tempfile::tempdir().expect("a temporary root");
    let required = context(root.path(), repo.path(), IssueRequirement::On);
    std::fs::write(required.repo_dir.join("notes.txt"), "x\n").expect("a dirty file");
    let unclaimed = session::Run::new("opencode-abcd1234".to_owned());
    let quiet = hook::session_start_response(&required, &unclaimed);
    let text = quiet["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context is injected");
    assert!(!text.contains("notes.txt"), "{text}");
}

/// A directory Git answers `not a repository` for whose ancestors hold no `.git`
/// entry this process can read: the launch directory issue #152 was reproduced
/// from.
///
/// Asserted rather than assumed. A machine whose temporary directory sits inside
/// a checkout cannot measure this, and must fail loudly here rather than let
/// every test built on it report the opposite of what it ran.
fn non_repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let answer = std::process::Command::new("git")
        .arg("-C")
        .arg(directory.path())
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("a usable `git` on PATH");
    assert!(
        !answer.status.success(),
        "{} is inside a repository, so it cannot stand for a non-repository launch directory",
        directory.path().display()
    );
    for ancestor in directory.path().ancestors() {
        assert!(
            !ancestor.join(".git").exists(),
            "{} holds a `.git` entry, so {} is not a non-repository launch directory",
            ancestor.display(),
            directory.path().display()
        );
    }
    directory
}

fn in_a_non_repository(
    requirement: IssueRequirement,
) -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let launch = non_repository();
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), launch.path(), requirement);
    (launch, root, context)
}

fn patch_body(entries: &[(&str, &str)]) -> String {
    let mut body = String::from("*** Begin Patch\n");
    for (marker, path) in entries {
        body.push_str(&format!("{marker}: {path}\n"));
        body.push_str("+x\n");
    }
    body.push_str("*** End Patch");
    body
}

fn patch_write(context: &GateContext, body: &str) -> Decision {
    unclaimed_edit(context, "patch", serde_json::json!({ "patchText": body }))
}

fn held_for_an_issue(decision: &Decision) -> bool {
    matches!(decision, Decision::Deny(refusal) if refusal.code == CODE)
}

#[test]
fn a_patch_under_a_non_git_directory_is_not_refused_for_an_issue() {
    let (launch, _root, context) = in_a_non_repository(IssueRequirement::On);
    let target = launch.path().join("gpt-budget").join("package.json");
    let decision = patch_write(
        &context,
        &patch_body(&[("*** Add File", &target.display().to_string())]),
    );
    assert_eq!(decision, Decision::Outside(Aside::NothingSworn));
}

#[test]
fn an_absolute_write_under_a_non_git_directory_is_not_refused_for_an_issue() {
    let (launch, _root, context) = in_a_non_repository(IssueRequirement::On);
    for tool in ["Write", "Edit"] {
        let target = launch.path().join("gpt-budget").join("package.json");
        let decision = unclaimed_edit(
            &context,
            tool,
            serde_json::json!({ "file_path": target.display().to_string() }),
        );
        assert_eq!(
            decision,
            Decision::Outside(Aside::NothingSworn),
            "{tool}: {decision:?}"
        );
    }
}

#[test]
fn a_relative_write_under_a_non_git_directory_is_not_refused_for_an_issue() {
    let (_launch, _root, context) = in_a_non_repository(IssueRequirement::On);
    for target in ["notes.md", "gpt-budget/package.json"] {
        let decision = unclaimed_edit(
            &context,
            "Write",
            serde_json::json!({ "file_path": target }),
        );
        assert_eq!(
            decision,
            Decision::Outside(Aside::NothingSworn),
            "{target}: {decision:?}"
        );
    }
}

#[test]
fn a_shell_file_under_a_non_git_directory_is_not_refused_for_an_issue() {
    let (launch, _root, context) = in_a_non_repository(IssueRequirement::On);
    let target = crate::test_env::shell_path(&launch.path().join("notes.md"));
    for command in [
        format!("cat > {target} <<'EOF'\nx\nEOF"),
        format!("echo x > {target}"),
    ] {
        let decision = shell_write(&context, &command);
        assert!(
            matches!(decision, Decision::Outside(_)),
            "{command}: {decision:?}"
        );
    }
}

#[test]
fn a_file_write_or_a_patch_into_a_repository_is_not_held_from_anywhere() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::On);
    let repo = repository_on_main();
    let root = tempfile::tempdir().expect("a temporary root");
    let inside = repo.path().join("src.rs");
    let outside = launch.path().join("notes.md");
    let context = context(root.path(), launch.path(), IssueRequirement::On);
    let edit = unclaimed_edit(
        &context,
        "Write",
        serde_json::json!({ "file_path": inside.display().to_string() }),
    );
    assert!(passes(&edit), "edit: {edit:?}");
    let inside = inside.display().to_string();
    let outside = outside.display().to_string();
    for entries in [
        [("*** Update File", inside.as_str())],
        [("*** Delete File", inside.as_str())],
        [("*** Move to", inside.as_str())],
    ] {
        let patch = patch_write(&context, &patch_body(&entries));
        assert!(passes(&patch), "{entries:?}: {patch:?}");
    }
    let patch = patch_write(
        &context,
        &patch_body(&[
            ("*** Add File", outside.as_str()),
            ("*** Add File", inside.as_str()),
        ]),
    );
    assert!(passes(&patch), "{patch:?}");
}

#[test]
fn a_write_tool_whose_body_named_no_readable_path_is_not_held() {
    let (_launch, _root, context) = in_a_non_repository(IssueRequirement::On);
    let repo = repository_on_main();
    let inside = repo.path().join("src.rs").display().to_string();
    for (tool, input) in [
        (
            "apply_patch",
            serde_json::json!({ "input": format!("--- a{inside}\n+++ b{inside}\n") }),
        ),
        (
            "pre_write_code",
            serde_json::json!({ "file": inside.as_str() }),
        ),
        (
            "apply_patch",
            serde_json::json!({ "input": "*** Begin Patch\n*** End Patch" }),
        ),
    ] {
        let decision = unclaimed_edit(&context, tool, input.clone());
        assert!(passes(&decision), "{tool} {input}: {decision:?}");
    }
}

#[cfg(unix)]
#[test]
fn an_ancestor_this_process_cannot_open_is_held_rather_than_exempted() {
    use std::os::unix::fs::PermissionsExt;

    let (launch, root, _) = in_a_non_repository(IssueRequirement::On);
    let locked = launch.path().join("locked");
    std::fs::create_dir(&locked).expect("a directory");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000))
        .expect("a mode the process cannot read");
    // The premise, asserted rather than assumed: the arm under test is reached
    // only by a process that cannot open the ancestor. A root run reads straight
    // through the mode, and must fail here rather than report a write as held
    // that nothing held.
    assert!(
        std::fs::read_dir(&locked).is_err(),
        "a process that can open {} cannot measure the unreadable-ancestor arm",
        locked.display()
    );
    assert!(
        super::holds_a_git_entry(&locked).is_none(),
        "{} did not report an unreadable ancestor",
        locked.display()
    );
    let mut context = context(root.path(), launch.path(), IssueRequirement::On);
    context.repo_dir = locked.clone();
    let decision = commit(&context);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755))
        .expect("the mode is restored so the fixture can be cleaned up");
    let Decision::Deny(refusal) = &decision else {
        panic!("{decision:?}");
    };
    assert_eq!(refusal.code, CODE);
    assert!(
        refusal.message.contains("could not be established"),
        "{}",
        refusal.message
    );
}

#[test]
fn a_refusal_from_a_non_git_directory_does_not_call_it_this_repository() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::On);
    let repo = repository_on_main();
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), launch.path(), IssueRequirement::On);
    let from_here = commit(&context);
    assert!(passes(&from_here), "{from_here:?}");
    let line = format!(
        "git -C {} commit -m x",
        crate::test_env::shell_path(repo.path())
    );
    let decision = on_a_shell_line(&context, &line);
    let Decision::Deny(refusal) = &decision else {
        panic!("{decision:?}");
    };
    assert_eq!(refusal.code, CODE);
    assert!(
        !refusal.message.contains("this repository"),
        "{}",
        refusal.message
    );
}

#[test]
fn a_session_starting_in_a_non_git_directory_is_not_told_it_is_this_repository() {
    let (_launch, _root, context) = in_a_non_repository(IssueRequirement::On);
    let text = session_start_text(&context);
    assert!(text.contains("on the default branch"), "{text}");
    assert!(!text.contains("this repository"), "{text}");
}

fn hook_answer_in_mode(
    context: &GateContext,
    dialect: Dialect,
    agent: &str,
    mode: &serde_json::Value,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "session_id": "abcd1234",
        "cwd": context.repo_dir.display().to_string(),
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": "git commit -m x" },
    });
    for (key, value) in mode.as_object().expect("mode fields") {
        payload[key] = value.clone();
    }
    let input = hook::read_input(payload.to_string().as_bytes());
    assert!(!input.unreadable && !input.absent, "{payload}");
    hook::run_as(
        dialect,
        Some(agent),
        Event::PreToolUse,
        &input,
        Some(context),
    )
}

fn decision_word(answer: &serde_json::Value) -> String {
    answer["hookSpecificOutput"]["permissionDecision"]
        .as_str()
        .unwrap_or("none")
        .to_owned()
}

#[test]
fn a_host_automatic_mode_leaves_the_hook_answer_a_refusal() {
    let modes = [
        serde_json::json!({}),
        serde_json::json!({ "permission_mode": "bypassPermissions" }),
        serde_json::json!({ "permission_mode": "auto" }),
        serde_json::json!({ "permission_mode": "dontAsk" }),
        serde_json::json!({ "permission_mode": "acceptEdits" }),
        serde_json::json!({ "approval_policy": "never", "sandbox_mode": "danger-full-access" }),
        serde_json::json!({ "permission_mode": "bypassPermissions", "approval_policy": "never" }),
    ];

    let (_repo, _root, required) = rig(IssueRequirement::On);
    for (agent, mode) in [("claude-code", &modes[1..5]), ("codex", &modes[1..])]
        .into_iter()
        .flat_map(|(agent, set)| set.iter().map(move |mode| (agent, mode)))
    {
        let answer = hook_answer_in_mode(&required, Dialect::ClaudeCode, agent, mode);
        assert_eq!(decision_word(&answer), "deny", "{agent} {mode}: {answer}");
        assert!(
            answer.to_string().contains(CODE),
            "{agent} {mode}: {answer}"
        );
        let plain = hook_answer_in_mode(&required, Dialect::ClaudeCode, agent, &modes[0]);
        assert_eq!(answer, plain, "{agent} {mode}: the mode changed the answer");
    }
}

#[test]
fn an_unreadable_repository_file_holds_a_github_write() {
    let (_repo, _root, mut context) = rig(IssueRequirement::Off);
    context.repository_config_unreadable = Some(Refusal::not_started(
        "config-local-unreadable",
        "estigia.local.md: stream did not contain valid UTF-8",
        Resolution::no_command(NoCommandReason::OperatorKnowledge, "that file readable"),
    ));
    for line in ["git merge topic", "gh pr merge 3 --merge"] {
        let decision = on_a_shell_line(&context, line);
        assert!(
            matches!(&decision, Decision::Deny(refusal)
                if refusal.code == "config-local-unreadable"
                    && refusal.message.contains("estigia.local.md")
                    && refusal.message.contains("UTF-8")),
            "{line}: {decision:?}"
        );
    }
}

fn rig_on_the_default_branch(
    requirement: IssueRequirement,
) -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    rig(requirement)
}

fn rig_on_a_side_branch(
    requirement: IssueRequirement,
) -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let (repo, root, context) = rig_on_the_default_branch(requirement);
    git_in(repo.path(), &["checkout", "-q", "-b", "side"]);
    (repo, root, context)
}

fn passes(decision: &Decision) -> bool {
    !matches!(decision, Decision::Ask(_) | Decision::Deny(_))
}

#[test]
fn a_file_tool_write_and_a_commit_off_the_default_branch_are_not_held() {
    let (_repo, _root, context) = rig_on_a_side_branch(IssueRequirement::On);
    assert_eq!(edit(&context), Decision::Outside(Aside::NothingSworn));
    let decision = on_a_shell_line(&context, "git commit -m x");
    assert!(passes(&decision), "{decision:?}");
}

#[test]
fn what_puts_work_on_the_default_branch_is_held_without_a_claim() {
    let (_repo, _root, context) = rig_on_the_default_branch(IssueRequirement::On);
    for line in [
        "git commit -m x",
        "git cherry-pick abc",
        "git revert abc",
        "git am change.patch",
        "git merge topic",
        "git push origin main",
        "git push",
        "git push origin HEAD:main",
        "git push origin +main",
        "git push origin refs/heads/main",
        "gh pr merge 7 --merge",
    ] {
        let decision = on_a_shell_line(&context, line);
        assert!(held_for_an_issue(&decision), "{line}: {decision:?}");
    }
}

#[test]
fn everything_else_passes_without_a_claim_on_either_branch() {
    for (_repo, _root, context) in [
        rig_on_the_default_branch(IssueRequirement::On),
        rig_on_a_side_branch(IssueRequirement::On),
    ] {
        for line in [
            "git push -u origin side",
            "git pull",
            "gh pr create --title x --body y",
            "gh pr comment 3 --body x",
            "echo x > out.txt",
            "python3 -c \"open('a','w').write('x')\"",
            "git stash",
            "git rm src.rs",
            "git branch topic",
            "git worktree add ../wt",
        ] {
            let decision = on_a_shell_line(&context, line);
            assert!(passes(&decision), "{line}: {decision:?}");
        }
        assert_eq!(edit(&context), Decision::Outside(Aside::NothingSworn));
    }
}

#[test]
fn a_push_destination_the_line_does_not_establish_is_held() {
    let (_repo, _root, context) = rig_on_a_side_branch(IssueRequirement::On);
    for line in [
        "git push --all",
        "git push --mirror origin",
        "git push origin \"$BRANCH\"",
        "git push origin refs/heads/*:refs/heads/*",
    ] {
        let decision = on_a_shell_line(&context, line);
        assert!(held_for_an_issue(&decision), "{line}: {decision:?}");
    }
}

#[test]
fn a_failed_current_branch_read_is_held_under_a_commit_or_a_merge() {
    let (repo, _root, context) = rig_on_a_side_branch(IssueRequirement::On);
    std::fs::write(repo.path().join(".git").join("HEAD"), "garbage\n").expect("a broken HEAD");
    for line in ["git commit -m x", "git merge topic"] {
        let decision = on_a_shell_line(&context, line);
        assert!(held_for_an_issue(&decision), "{line}: {decision:?}");
    }
}

#[test]
fn a_tag_and_a_release_are_held_off_the_default_branch_too() {
    let (_repo, _root, context) = rig_on_a_side_branch(IssueRequirement::On);
    for line in ["git tag v1", "gh release create v1"] {
        let decision = on_a_shell_line(&context, line);
        assert!(held_for_an_issue(&decision), "{line}: {decision:?}");
    }
}

#[test]
fn a_detached_head_is_not_the_default_branch() {
    let (repo, _root, context) = rig_on_the_default_branch(IssueRequirement::On);
    git_in(repo.path(), &["checkout", "-q", "--detach"]);
    let decision = on_a_shell_line(&context, "git commit -m x");
    assert!(passes(&decision), "{decision:?}");
}

#[test]
fn the_default_branch_is_the_one_origin_head_names() {
    let (repo, _root, context) = rig_on_the_default_branch(IssueRequirement::On);
    git_in(
        repo.path(),
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/trunk",
        ],
    );
    let decision = on_a_shell_line(&context, "git commit -m x");
    assert!(passes(&decision), "{decision:?}");
    git_in(repo.path(), &["checkout", "-q", "-b", "trunk"]);
    let decision = on_a_shell_line(&context, "git commit -m x");
    assert!(held_for_an_issue(&decision), "{decision:?}");
    let decision = on_a_shell_line(&context, "git push origin trunk");
    assert!(held_for_an_issue(&decision), "{decision:?}");
    let decision = on_a_shell_line(&context, "git push origin main");
    assert!(passes(&decision), "{decision:?}");
}
