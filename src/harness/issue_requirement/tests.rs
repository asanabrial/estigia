use std::path::Path;
use std::time::Duration;

use super::*;
use crate::config::{Config, IssueRequirement};
use crate::harness::hook;
use crate::harness::hook::{Dialect, Event, Input};
use crate::harness::{Aside, Decision, GateContext, guard, session};
use crate::outcome::Observed;

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
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn rig(requirement: IssueRequirement) -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path(), requirement);
    (repo, root, context)
}

fn rig_without_a_remote(
    requirement: IssueRequirement,
) -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let repo = crate::test_env::repository_with_no_remote();
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

fn hook_answer(context: &GateContext, dialect: Dialect, agent: &str) -> serde_json::Value {
    let target = context.repo_dir.join("src.rs");
    let input = Input {
        session_id: "abcd1234".to_owned(),
        cwd: context.repo_dir.display().to_string(),
        tool_name: "Edit".to_owned(),
        tool_input: serde_json::json!({ "file_path": target.display().to_string() }),
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
fn required_refuses_an_unclaimed_edit_and_names_what_clears_it() {
    let (_repo, _root, context) = rig(IssueRequirement::Required);
    let Decision::Deny(refusal) = edit(&context) else {
        panic!("an unclaimed edit was not refused");
    };
    assert_eq!(refusal.code, CODE);
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("`claim`"), "{resolution}");
    assert!(resolution.contains("`create`"), "{resolution}");
}

#[test]
fn required_refuses_an_unclaimed_shell_commit() {
    let (_repo, _root, context) = rig(IssueRequirement::Required);
    let decision = unclaimed_edit(
        &context,
        "Bash",
        serde_json::json!({ "command": "git commit -m x" }),
    );
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn ask_puts_an_unclaimed_edit_to_the_person() {
    let (_repo, _root, context) = rig(IssueRequirement::Watch);
    let Decision::Ask(refusal) = edit(&context) else {
        panic!("an unclaimed edit was not put to the person");
    };
    assert_eq!(refusal.code, CODE);
}

#[test]
fn ask_is_delivered_as_ask_on_claude_code_and_nowhere_else() {
    let (_repo, _root, context) = rig(IssueRequirement::Watch);
    let claude = hook_answer(&context, Dialect::ClaudeCode, "claude-code");
    assert_eq!(
        claude["hookSpecificOutput"]["permissionDecision"], "ask",
        "{claude}"
    );
    for (dialect, agent) in [
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
fn an_unclaimed_edit_through_the_gate_command_is_refused_not_asked() {
    let (_repo, _root, context) = rig(IssueRequirement::Watch);
    let target = context.repo_dir.join("src.rs");
    let input = serde_json::json!({ "file_path": target.display().to_string() });
    let (action, how, _) = crate::harness::classify_observing("Edit", &input, &[]);
    let decision =
        guard::adjudicate_action(&context, &context.repo_dir, &action, how, &[]).decision;
    assert!(matches!(decision, Decision::Ask(_)), "{decision:?}");
    assert!(decision.without_ask().denies());
}

#[test]
fn off_changes_nothing() {
    let (_repo, _root, context) = rig(IssueRequirement::Off);
    assert_eq!(edit(&context), Decision::Outside(Aside::NothingSworn));
}

fn write_to(context: &GateContext, tool: &str, path: &Path) -> Decision {
    unclaimed_edit(
        context,
        tool,
        serde_json::json!({ "file_path": path.display().to_string() }),
    )
}

#[test]
fn a_checkout_with_no_remote_is_not_held_for_an_issue() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, _root, context) = rig_without_a_remote(requirement);
        for (tool, name) in [("Edit", "src.rs"), ("Write", "new.rs")] {
            let decision = write_to(&context, tool, &context.repo_dir.join(name));
            assert_eq!(
                decision,
                Decision::Outside(Aside::NothingSworn),
                "{requirement:?} {tool}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_no_remote_checkout_answers_the_write_hook_with_nothing() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, _root, context) = rig_without_a_remote(requirement);
        let answer = hook_answer(&context, Dialect::ClaudeCode, "claude-code");
        assert_eq!(answer, serde_json::json!({}), "{requirement:?}: {answer}");
    }
}

#[test]
fn adding_a_remote_holds_the_same_write_again() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (repo, _root, context) = rig_without_a_remote(requirement);
        assert_eq!(edit(&context), Decision::Outside(Aside::NothingSworn));
        crate::test_env::add_github_remote(repo.path());
        let decision = edit(&context);
        assert!(
            matches!(&decision, Decision::Ask(refusal) | Decision::Deny(refusal) if refusal.code == CODE),
            "{requirement:?}: {decision:?}"
        );
    }
}

#[test]
fn a_named_repository_holds_the_write_where_the_checkout_has_no_remote() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, _root, mut context) = rig_without_a_remote(requirement);
        context.tracker = crate::config::Tracker::Github {
            repo: Some(crate::config::RepoRef::parse("asanabrial/estigia").expect("a repo ref")),
        };
        let decision = edit(&context);
        assert!(
            matches!(&decision, Decision::Ask(refusal) | Decision::Deny(refusal) if refusal.code == CODE),
            "{requirement:?}: {decision:?}"
        );
    }
}

#[test]
fn a_write_into_a_nested_checkout_that_has_a_remote_is_still_held() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (repo, _root, context) = rig_without_a_remote(requirement);
        let nested = repo.path().join("nested");
        std::fs::create_dir(&nested).expect("a directory for the nested checkout");
        crate::test_env::repository_at(&nested);
        crate::test_env::add_github_remote(&nested);
        let decision = write_to(&context, "Write", &nested.join("notes.md"));
        assert!(
            matches!(&decision, Decision::Ask(refusal) | Decision::Deny(refusal) if refusal.code == CODE),
            "{requirement:?}: {decision:?}"
        );
    }
}

#[test]
fn a_session_in_a_checkout_with_no_remote_is_told_nothing_is_gated() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, _root, context) = rig_without_a_remote(requirement);
        std::fs::write(context.repo_dir.join("notes.txt"), "x\n").expect("a dirty file");
        let text = session_start_text(&context);
        assert!(text.contains("no git remote"), "{requirement:?}: {text}");
        for wording in [
            "each repository write is put to the person",
            "refuses every repository write",
            "This checkout has uncommitted changes",
        ] {
            assert!(!text.contains(wording), "{requirement:?}: {text}");
        }
    }
}

#[test]
fn a_failed_remote_read_is_not_zero_remotes() {
    let (_repo, _root, context) = rig(IssueRequirement::Required);
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
            edit(&context),
            Decision::Outside(Aside::NothingSworn),
            "{requirement:?}"
        );
        context.issue_requirement = requirement;
    }
}

#[test]
fn a_write_outside_the_checkout_is_not_a_repository_write() {
    let (_repo, _root, context) = rig(IssueRequirement::Required);
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
    let (_repo, _root, context) = rig(IssueRequirement::Required);
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
    let (_repo, _root, context) = rig(IssueRequirement::Watch);
    let decision = push(&context);
    assert!(
        matches!(&decision, Decision::Ask(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
    let (_repo, _root, context) = rig(IssueRequirement::Required);
    let decision = push(&context);
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
    let (_repo, _root, context) = rig(IssueRequirement::Off);
    assert_eq!(push(&context), Decision::Outside(Aside::NothingSworn));
}

#[test]
fn an_unclaimed_push_is_asked_under_a_read_only_tracker() {
    let (_repo, _root, context) = rig(IssueRequirement::Watch);
    let context = under_linear(context);
    assert!(matches!(push(&context), Decision::Ask(_)));
}

#[test]
fn a_boundary_other_than_a_push_is_left_to_the_checks_that_already_hold_it() {
    let (_repo, _root, context) = rig(IssueRequirement::Required);
    let decision = unclaimed_edit(
        &context,
        "Bash",
        serde_json::json!({ "command": "gh pr merge 3 --merge" }),
    );
    assert!(
        !matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn the_push_guards_refusal_is_still_the_one_a_guarded_commit_gets() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), repo.path(), IssueRequirement::Required);
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
fn required_refuses_an_unclaimed_edit_under_a_read_only_tracker() {
    let (_repo, _root, context) = rig(IssueRequirement::Required);
    let context = under_linear(context);
    let Decision::Deny(refusal) = edit(&context) else {
        panic!("an unclaimed edit was not refused under a read-only tracker");
    };
    assert_eq!(refusal.code, CODE);
}

#[test]
fn ask_puts_an_unclaimed_edit_to_the_person_under_a_read_only_tracker() {
    let (_repo, _root, context) = rig(IssueRequirement::Watch);
    let context = under_linear(context);
    assert!(matches!(edit(&context), Decision::Ask(_)));
}

#[test]
fn a_read_only_tracker_without_an_assisted_workflow_refuses_nothing() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    guard::install(repo.path(), Path::new("/usr/local/bin/estigia"), false)
        .expect("the guard installs");
    let root = tempfile::tempdir().expect("a temporary root");
    for workflow in [None, Some(crate::config::Workflow::Autonomous)] {
        let mut context = under_linear(context(
            root.path(),
            repo.path(),
            IssueRequirement::Required,
        ));
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
    let context = under_linear(context(
        root.path(),
        repo.path(),
        IssueRequirement::Required,
    ));
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
    let Decision::Deny(refusal) = edit(&context) else {
        panic!("an unclaimed edit was not refused");
    };
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
    let mut context = context(root.path(), repo.path(), IssueRequirement::Required);
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
    let Decision::Deny(refusal) = edit(&context) else {
        panic!("an unclaimed edit was not refused");
    };
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
            (repo.path(), IssueRequirement::Required),
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
fn a_stand_down_lets_an_asked_write_through() {
    let (_repo, _root, mut context) = rig(IssueRequirement::Watch);
    context.stand_down = Some(crate::harness::standdown::StandDown {
        declared_by: "operator".to_owned(),
        reason: "incident".to_owned(),
        declared_at: session::now_seconds().expect("a clock"),
        until: session::now_seconds().expect("a clock") + 600,
    });
    let decision = edit(&context);
    assert!(matches!(decision, Decision::Allow(_)), "{decision:?}");
}

#[test]
fn the_row_defaults_to_watch_and_only_the_repository_file_sets_it() {
    assert_eq!(Config::default().issue_requirement, IssueRequirement::Watch);
    let setting = crate::config::Setting::IssueRequirement;
    assert_eq!(setting.scope(), crate::config::Scope::Repository);
    assert!(!crate::config::AGENT_SETTINGS.contains(&setting));
    assert!(!crate::config::EVERYWHERE_SETTINGS.contains(&setting));
    assert!(!crate::config::MACHINE_SETTINGS.contains(&setting));
    for value in ["watch", "off", "required"] {
        let mut config = Config::default();
        setting.apply(&mut config, value).expect("an offered value");
        assert_eq!(setting.value_of(&config), value);
    }
    let mut stored = Config {
        issue_requirement: IssueRequirement::Required,
        ..Config::default()
    };
    setting
        .apply(&mut stored, "ask")
        .expect("a stored `ask` still reads");
    assert_eq!(stored.issue_requirement, IssueRequirement::Watch);
    assert!(setting.apply(&mut Config::default(), "sometimes").is_err());
}

#[test]
fn the_repository_file_sets_it_and_the_contract_and_agent_files_do_not() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    let path = crate::skill::repository_config_path(repo.path());
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    std::fs::write(&path, "| Setting | Value here | Skill default |\n|---|---|---|\n| Issue requirement | required | ask |\n")
        .expect("the repository file");
    let layered = crate::skill::layer_repository(&Config::default(), repo.path())
        .expect("the repository layers");
    assert_eq!(layered.issue_requirement, IssueRequirement::Required);

    let (from_contract, owned) = Config::read_scope_over(
        &Config::default(),
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Issue requirement | required | ask |\n",
        crate::config::Scope::Agent,
    )
    .expect("a read");
    assert_eq!(from_contract.issue_requirement, IssueRequirement::Watch);
    assert!(owned.is_empty());
    let (from_contract, _) = Config::read_scope_over(
        &Config::default(),
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Issue requirement | required | ask |\n",
        crate::config::Scope::Everywhere,
    )
    .expect("a read");
    assert_eq!(from_contract.issue_requirement, IssueRequirement::Watch);
}

#[test]
fn a_contract_row_for_it_is_read_by_nobody_and_the_repository_file_renders_it() {
    let config = Config {
        issue_requirement: IssueRequirement::Required,
        ..Config::default()
    };
    assert!(!config.render_rows().contains("Issue requirement"));
    let rendered = config.render_some_repository_rows(&[crate::config::Setting::IssueRequirement]);
    assert!(
        rendered.contains("| Unclaimed write policy | required | watch |"),
        "{rendered}"
    );
    let read = Config::read(
        &crate::config::CONFIG_FENCE.upsert("", &rendered),
        Some(&rendered),
    )
    .expect("a table");
    assert_eq!(read.issue_requirement, IssueRequirement::Watch);
}

fn holder_of(context: &GateContext, worktree: &Path) {
    let mut holder = session::Run::new("opencode-aaaa1111".to_owned());
    holder.issue = Some(crate::identity::Identity::Number(12));
    holder.repo_dir = Some(context.repo_dir.clone());
    holder.worktree = Some(worktree.to_path_buf());
    session::store(&context.state_root, &holder).expect("the holder's pointer is stored");
}

#[test]
fn a_session_without_a_claim_may_write_inside_a_checkout_a_claim_names() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, root, mut context) = rig(requirement);
        let worktree = crate::test_env::repository_with_commit();
        crate::test_env::add_github_remote(worktree.path());
        holder_of(&context, worktree.path());
        context.repo_dir = worktree.path().to_path_buf();
        for decision in [
            edit(&context),
            shell_write(&context, "sed -i '' s/a/b/ src.rs"),
        ] {
            assert!(
                !matches!(&decision, Decision::Ask(_))
                    && !matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
                "{requirement:?}: {decision:?}"
            );
        }
        drop(root);
    }
}

#[test]
fn a_holder_whose_worktree_is_its_repo_dir_grants_no_exemption() {
    let (_repo, _root, mut context) = rig(IssueRequirement::Watch);
    let worktree = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(worktree.path());
    let mut holder = session::Run::new("opencode-aaaa1111".to_owned());
    holder.issue = Some(crate::identity::Identity::Number(12));
    holder.repo_dir = Some(worktree.path().to_path_buf());
    holder.worktree = Some(worktree.path().to_path_buf());
    session::store(&context.state_root, &holder).expect("the holder's pointer is stored");
    context.repo_dir = worktree.path().to_path_buf();
    assert!(matches!(edit(&context), Decision::Ask(_)));
}

#[test]
fn the_primary_checkout_stays_asked_while_a_claim_names_another() {
    let (_repo, _root, context) = rig(IssueRequirement::Watch);
    let worktree = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(worktree.path());
    holder_of(&context, worktree.path());
    assert!(matches!(edit(&context), Decision::Ask(_)));
}

#[test]
fn an_unreadable_pointer_gives_no_exemption() {
    let (_repo, _root, mut context) = rig(IssueRequirement::Watch);
    let worktree = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(worktree.path());
    holder_of(&context, worktree.path());
    for entry in std::fs::read_dir(&context.state_root).expect("the state root") {
        std::fs::write(entry.expect("an entry").path(), "{").expect("corrupted");
    }
    context.repo_dir = worktree.path().to_path_buf();
    assert!(matches!(edit(&context), Decision::Ask(_)));
}

fn shell_write(context: &GateContext, command: &str) -> Decision {
    unclaimed_edit(context, "Bash", serde_json::json!({ "command": command }))
}

const UNPLACEABLE_SHELL_WRITES: &[&str] = &[
    "sed -i '' s/a/b/ existing.txt",
    "perl -pi -e s/a/b/ existing.txt",
    "python3 -c \"open('existing.txt','w').write('x')\"",
    "cat > new.txt <<EOF\nx\nEOF",
    "bash -c 'echo x > src/lib.rs'",
];

#[test]
fn an_unplaceable_shell_write_launched_in_a_repository_is_refused_under_required() {
    let (_repo, _root, required) = rig(IssueRequirement::Required);
    for command in UNPLACEABLE_SHELL_WRITES {
        assert!(
            matches!(
                shell_write(&required, command),
                Decision::Deny(refusal) if refusal.code == CODE
            ),
            "{command} was not refused under `required`"
        );
    }
}

#[test]
fn an_unplaceable_shell_write_runs_under_watch_and_the_next_call_asks_only_when_the_checkout_changed()
 {
    let (repo, _root, watching) = rig(IssueRequirement::Watch);
    let read = || {
        unclaimed_edit(
            &watching,
            "Read",
            serde_json::json!({ "file_path": repo.path().join("README.md").display().to_string() }),
        )
    };
    for command in UNPLACEABLE_SHELL_WRITES {
        assert!(
            matches!(shell_write(&watching, command), Decision::Allow(_)),
            "{command} was not let through under `watch`"
        );
        assert!(
            !matches!(read(), Decision::Ask(_)),
            "{command} changed nothing and the next call was still put to the person"
        );
    }
    assert!(matches!(
        shell_write(&watching, UNPLACEABLE_SHELL_WRITES[0]),
        Decision::Allow(_)
    ));
    std::fs::write(repo.path().join("written.txt"), "x").expect("the watched write lands");
    let Decision::Ask(refusal) = read() else {
        panic!("a changed checkout was not put to the person");
    };
    assert_eq!(refusal.code, CODE);
    assert!(
        refusal.message.contains("`written.txt`"),
        "{}",
        refusal.message
    );
    assert!(
        !matches!(read(), Decision::Ask(_)),
        "a change already put to the person was asked about again"
    );
    assert!(
        matches!(edit(&watching), Decision::Ask(_)),
        "a write the gate can place is not put to the person under `watch`"
    );
}

#[test]
fn the_shell_call_that_changed_a_watched_checkout_is_reported_as_soon_as_it_ran() {
    let (repo, _root, watching) = rig(IssueRequirement::Watch);
    let after = || {
        let input = Input {
            session_id: "abcd1234".to_owned(),
            cwd: repo.path().display().to_string(),
            tool_name: "Bash".to_owned(),
            tool_input: serde_json::json!({ "command": UNPLACEABLE_SHELL_WRITES[0] }),
            ..Input::default()
        };
        hook::run_as(
            Dialect::ClaudeCode,
            Some("claude-code"),
            Event::PostToolUse,
            &input,
            Some(&watching),
        )
    };
    assert!(matches!(
        shell_write(&watching, UNPLACEABLE_SHELL_WRITES[0]),
        Decision::Allow(_)
    ));
    assert_eq!(after(), serde_json::json!({}));
    std::fs::write(repo.path().join("written.txt"), "x").expect("the watched write lands");
    let answer = after();
    let note = answer["systemMessage"]
        .as_str()
        .expect("a note the person sees");
    assert!(note.contains("`written.txt`"), "{note}");
    assert_eq!(answer["hookSpecificOutput"]["additionalContext"], note);
    assert!(
        matches!(edit(&watching), Decision::Ask(refusal) if refusal.message.contains("`written.txt`")),
        "the next call was not put to the person"
    );
}

#[test]
fn an_unplaceable_shell_write_launched_outside_every_repository_is_still_allowed_under_watch() {
    let (_launch, _root, asking) = in_a_non_repository(IssueRequirement::Watch);
    for command in UNPLACEABLE_SHELL_WRITES {
        assert!(
            matches!(shell_write(&asking, command), Decision::Allow(_)),
            "{command} was not allowed outside every repository"
        );
    }
    let (_launch, _root, required) = in_a_non_repository(IssueRequirement::Required);
    for command in UNPLACEABLE_SHELL_WRITES {
        assert!(
            matches!(
                shell_write(&required, command),
                Decision::Deny(refusal) if refusal.code == CODE
            ),
            "{command} was not refused under `required`"
        );
    }
}

#[test]
fn an_unplaceable_shell_write_at_an_unestablished_scope_is_asked_under_ask() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::Watch);
    std::fs::create_dir(launch.path().join(".git")).expect("a `.git` entry Git cannot use");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), launch.path(), IssueRequirement::Watch);
    assert!(
        matches!(scope_of(launch.path()), Scope::Unknown),
        "{} was established, so this test never reaches the unknown scope",
        launch.path().display()
    );
    for command in UNPLACEABLE_SHELL_WRITES {
        let Decision::Ask(refusal) = shell_write(&context, command) else {
            panic!("{command} was not put to the person");
        };
        assert_eq!(refusal.code, CODE);
        assert!(
            refusal.message.contains("could not be established"),
            "{command}: {}",
            refusal.message
        );
    }
}

#[test]
fn a_redirect_to_an_absolute_path_outside_the_checkout_is_not_held() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, _root, context) = rig(requirement);
        for command in ["echo x > /tmp/y", "echo x >> /tmp/y"] {
            let decision = shell_write(&context, &crate::test_env::outside_the_checkout(command));
            assert!(
                matches!(decision, Decision::Outside(_)),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

fn held_or_watched(requirement: IssueRequirement, decision: &Decision) -> bool {
    match decision {
        Decision::Ask(refusal) | Decision::Deny(refusal) => refusal.code == CODE,
        Decision::Allow(note) => {
            requirement == IssueRequirement::Watch
                && note.contains("`Unclaimed write policy` is `watch`")
        }
        _ => false,
    }
}

#[test]
fn a_write_placed_inside_the_checkout_is_still_asked_under_watch() {
    let (repo, _root, context) = rig(IssueRequirement::Watch);
    let inside = format!(
        "echo x > {}/out.txt",
        crate::test_env::shell_path(repo.path())
    );
    for command in [
        "echo x > out.txt",
        "echo x > ./a",
        "git commit -m x",
        inside.as_str(),
    ] {
        let decision = shell_write(&context, command);
        assert!(
            matches!(&decision, Decision::Ask(r) if r.code == CODE),
            "{command}: {decision:?}"
        );
    }
    assert!(matches!(edit(&context), Decision::Ask(_)));
}

#[test]
fn rg_pre_runs_a_program_and_is_held() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, _root, context) = rig(requirement);
        for command in [
            "rg --pre /bin/rm x",
            "rg --pre=/tmp/s.sh x",
            "rg --pre /bin/rm x > /tmp/out",
        ] {
            let decision = shell_write(&context, command);
            assert!(
                matches!(&decision, Decision::Ask(r) | Decision::Deny(r) if r.code == CODE),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_plain_rg_search_keeps_todays_behaviour() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, _root, context) = rig(requirement);
        for command in ["rg pattern src", "rg pattern src > /tmp/out"] {
            let decision = shell_write(&context, &crate::test_env::outside_the_checkout(command));
            assert!(
                matches!(decision, Decision::Outside(_)),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_quoted_heredoc_written_outside_the_checkout_is_not_held() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (_repo, _root, context) = rig(requirement);
        for command in [
            "cat > /private/tmp/x/scratchpad/issue.md <<'EOF'\n## Problem\n... `model_entries` in `src/tui/app.rs` ...\nEOF",
            "cat > /tmp/a <<\"EOF\"\nx\nEOF",
            "cat >> /tmp/a <<'EOF'\nx\nEOF\n",
            "cat <<'EOF' > /tmp/a\nx\nEOF",
            "cat <<'EOF' >> /tmp/a\nx\nEOF",
            "cat \t > \t /tmp/a   <<'EOF'\nx\nEOF",
            "cat > /tmp/a <<'EOF'\n$(rm -rf src)\nEOF",
            "cat > /tmp/a <<'EOF'\n`rm -rf src`\n<<\n# note\nEOF",
            "cat > /tmp/a <<'EOF'\n\nEOF",
            "cat > /tmp/a <<'EOF'\nEOF",
        ] {
            let decision = shell_write(&context, &crate::test_env::outside_the_checkout(command));
            assert!(
                matches!(decision, Decision::Outside(_)),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_heredoc_that_is_not_the_one_exact_shape_is_held() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (repo, _root, context) = rig(requirement);
        let inside = format!(
            "cat > {}/out.md <<'EOF'\nx\nEOF",
            crate::test_env::shell_path(repo.path())
        );
        for command in [
            inside.as_str(),
            "cat > /tmp/a <<EOF\nx\nEOF",
            "cat > /tmp/a <<\\EOF\nx\nEOF",
            "cat > /tmp/a <<-'EOF'\n\tx\n\tEOF",
            "cat <<<'EOF' > /tmp/a\nx\nEOF",
            "cat > out.md <<'EOF'\nx\nEOF",
            "cat > ./out.md <<'EOF'\nx\nEOF",
            "cat > /tmp/../tmp/a <<'EOF'\nx\nEOF",
            "cat > $f <<'EOF'\nx\nEOF",
            "cat > /tmp/a <<'EOF'\nx\nEOF\nrm -rf src",
            "cat > /tmp/a <<'EOF'\nx\nEOF\n\n",
            "cat > /tmp/a <<'EOF' && rm x\nx\nEOF",
            "cat > /tmp/a <<'EOF'; rm x\nx\nEOF",
            "cat > /tmp/a <<'EOF' | sh\nx\nEOF",
            "cat > /tmp/a <<'EOF' &\nx\nEOF",
            "/bin/cat > /tmp/a <<'EOF'\nx\nEOF",
            "sh > /tmp/a <<'EOF'\nrm -rf src\nEOF",
            "tee /tmp/a <<'EOF'\nx\nEOF",
            "cat > /tmp/a <<'EOF'\nnever closed",
            "cat > /tmp/a <<'EOF'\nx\nEOF extra",
            "cat > /tmp/a <<'E-F'\nx\nE-F",
            "cat > /tmp/a <<''\nx\n",
            "cat > /tmp/a <<'EOF\nx\nEOF",
            "cat > /tmp/a <<'EOF'\nx\nEOF\r",
            "cat > /tmp/a b <<'EOF'\nx\nEOF",
            "cat > '/tmp/a' <<'EOF'\nx\nEOF",
            "cat > /tmp/a\\\n <<'EOF'\nx\nEOF",
            "echo $((1<<2)) > /tmp/a\ntouch src/x\n2",
            "(( x<<1 )) > /tmp/a\ntouch src/x\n1",
            "let y=1<<2 > /tmp/a\ntouch src/x\n2",
            "echo $[1<<2] > /tmp/a\ntouch src/x\n2",
            "echo hi > /tmp/a # <<EOF\nrm -rf src\nEOF",
            "echo $((1 +\ncat <<2 )) > /tmp/a\ntouch src/x\n2",
            "x=$(\ncat <<E\n)\ntouch src/x\nE",
            "cat <<E | sh > /tmp/a\ntouch src/x\nE",
            "cat <<E\\\nOF > src/x\nE\necho > /tmp/a",
            "echo a > /tmp/a; echo $'\\'' > src/x #'",
            "echo x > /tmp/a; echo \"a\\\"\" > src/x",
        ] {
            let decision = shell_write(&context, command);
            assert!(
                held_or_watched(requirement, &decision),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_heredoc_body_naming_a_repository_command_is_data_not_a_command() {
    let (_repo, _root, context) = rig(IssueRequirement::Watch);
    let decision = shell_write(
        &context,
        &crate::test_env::outside_the_checkout("cat > /tmp/a <<'EOF'\nrun git commit here\nEOF"),
    );
    assert!(
        matches!(&decision, Decision::Outside(_)),
        "a here-document body `cat` writes is data, and the target is outside the checkout: \
         {decision:?}"
    );
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
fn any_other_shell_write_is_still_held() {
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let (repo, _root, context) = rig(requirement);
        let inside = format!(
            "echo x > {}/out.txt",
            crate::test_env::shell_path(repo.path())
        );
        for command in [
            "echo x > out.txt",
            "echo x > ./a",
            "echo x > $f",
            "echo x > /tmp/a > out.txt",
            "cp a /tmp/b",
            "git commit -m x > /tmp/log",
            "git reset --hard > /tmp/log",
            "git checkout -b x > /tmp/log",
            "gh issue develop 3 > /tmp/log",
            "cat <<EOF > /tmp/x\n$(rm -rf src)\nEOF",
            "cp a out.txt > /tmp/b",
            "echo x > /tmp/a\" b\" > src/evil \"z\"",
            "echo 'echo x > src/f' | sh > /tmp/y",
            "sh -c \"echo x > src/f\" > /tmp/y",
            "echo x > /tmp/../tmp/y",
            "echo x >> \"/tmp/y\"",
            "echo x >&1x > /tmp/y",
            "sed -ibak s/a/b/ src/x.rs > /tmp/l",
            "cargo fmt > /tmp/l",
            "echo x > /tmp/y 2>&1",
            "echo x | tee /tmp/b > /tmp/c",
            inside.as_str(),
        ] {
            let decision = shell_write(&context, command);
            assert!(
                held_or_watched(requirement, &decision),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_git_write_sent_into_another_checkout_runs_watched_rather_than_asked() {
    let (repo, _root, watching) = rig(IssueRequirement::Watch);
    let other = crate::test_env::repository_with_commit();
    let clone = crate::test_env::shell_path(other.path());
    for command in [
        format!("cd {clone} && git stash -q -- lib"),
        format!("cd {clone} && git stash -q -- lib && cargo test ; git stash pop -q"),
        format!("git -C {clone} stash"),
        format!("git -C {clone} stash && git -C {clone} stash pop"),
        format!(
            "cd {clone} && python3 - <<'EOF'\nopen('t.dart','w').write('x')\nEOF\n\
             git stash -q -- lib && fvm flutter test t.dart 2>&1 | grep -E \"Some tests\" | head; \
             git stash pop -q && git diff --stat"
        ),
    ] {
        let decision = shell_write(&watching, &command);
        assert!(
            matches!(&decision, Decision::Allow(note) if note.contains("`watch`")),
            "{command}: {decision:?}"
        );
    }
    let (_repo, _root, required) = rig(IssueRequirement::Required);
    let decision = shell_write(&required, &format!("cd {clone} && git stash"));
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
    let own = crate::test_env::shell_path(repo.path());
    for command in [
        format!("cd {own} && git stash"),
        format!("git -C {own} stash"),
        "cd \"$DIR\" && git stash".to_owned(),
        "cd clone && git stash".to_owned(),
        format!("cd {clone} && git stash ; cd {own} && git stash pop"),
        format!("git -C {clone} stash && git stash pop"),
        format!("cd {clone} && git -C {own} stash"),
        format!("cd {clone} && GIT_DIR={own}/.git git stash"),
        format!("cd {clone} && git --git-dir={own}/.git stash"),
        format!("cd {clone} ; git stash"),
        format!("cd {clone} && (cd {own}; git stash)"),
        format!("cd {clone} && git stash && sh -c 'cd {own} && git stash'"),
    ] {
        let decision = shell_write(&watching, &command);
        assert!(
            matches!(&decision, Decision::Ask(refusal) if refusal.code == CODE),
            "{command}: {decision:?}"
        );
    }
}

#[test]
fn the_three_unplaceable_shapes_run_watched_under_watch_and_are_refused_under_required() {
    let scratch = "/private/tmp/estigia-719-scratchpad";
    let shapes = [
        format!("cd {scratch} && cat > c64.md <<'EOF'\n## Problem\nan issue body\nEOF"),
        format!("cd {scratch}/ga; bash -c 'grep -n src src/lib.rs; sed -n 1,10p src/lib.rs'"),
        format!("cd {scratch} && printf '%s\\n' an issue line >> a2.md"),
    ];
    let (_repo, _root, watching) = rig(IssueRequirement::Watch);
    for shape in &shapes {
        let decision = shell_write(&watching, shape);
        assert!(
            matches!(&decision, Decision::Allow(note) if note.contains("is `watch`")),
            "{shape}: {decision:?}"
        );
    }
    let (_repo, _root, required) = rig(IssueRequirement::Required);
    for shape in &shapes {
        let decision = shell_write(&required, shape);
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{shape}: {decision:?}"
        );
    }
}

#[test]
fn a_dirty_checkout_is_reported_at_session_start_for_an_unclaimed_asked_run() {
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    let root = tempfile::tempdir().expect("a temporary root");
    let asking = context(root.path(), repo.path(), IssueRequirement::Watch);
    std::fs::write(asking.repo_dir.join("notes.txt"), "x\n").expect("a dirty file");
    let unclaimed = session::Run::new("opencode-abcd1234".to_owned());
    let response = hook::session_start_response(&asking, &unclaimed);
    let injected = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context is injected");
    assert!(injected.contains("notes.txt"), "{injected}");

    let mut claimed = session::Run::new("opencode-abcd1234".to_owned());
    claimed.issue = Some(crate::identity::Identity::Number(12));
    let quiet = hook::session_start_response(&asking, &claimed);
    let text = quiet["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context is injected");
    assert!(!text.contains("notes.txt"), "{text}");

    let required = context(root.path(), repo.path(), IssueRequirement::Required);
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
    matches!(decision, Decision::Ask(refusal) | Decision::Deny(refusal) if refusal.code == CODE)
}

#[test]
fn a_patch_under_a_non_git_directory_is_not_refused_for_an_issue() {
    let (launch, _root, context) = in_a_non_repository(IssueRequirement::Watch);
    let target = launch.path().join("gpt-budget").join("package.json");
    let decision = patch_write(
        &context,
        &patch_body(&[("*** Add File", &target.display().to_string())]),
    );
    assert_eq!(decision, Decision::Outside(Aside::NothingSworn));
}

#[test]
fn an_absolute_write_under_a_non_git_directory_is_not_refused_for_an_issue() {
    let (launch, _root, context) = in_a_non_repository(IssueRequirement::Watch);
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
    let (_launch, _root, context) = in_a_non_repository(IssueRequirement::Watch);
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
    let (launch, _root, context) = in_a_non_repository(IssueRequirement::Watch);
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
fn a_write_from_a_non_git_directory_into_a_repository_is_still_held() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::Watch);
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    let root = tempfile::tempdir().expect("a temporary root");
    let inside = repo.path().join("src.rs");
    let outside = launch.path().join("notes.md");
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let context = context(root.path(), launch.path(), requirement);
        let edit = unclaimed_edit(
            &context,
            "Write",
            serde_json::json!({ "file_path": inside.display().to_string() }),
        );
        assert!(held_for_an_issue(&edit), "{requirement:?} edit: {edit:?}");
        for entries in [
            vec![("*** Update File", inside.display().to_string())],
            vec![
                ("*** Add File", outside.display().to_string()),
                ("*** Add File", inside.display().to_string()),
            ],
        ] {
            let borrowed: Vec<(&str, &str)> = entries
                .iter()
                .map(|(marker, path)| (*marker, path.as_str()))
                .collect();
            let patch = patch_write(&context, &patch_body(&borrowed));
            assert!(
                held_for_an_issue(&patch),
                "{requirement:?} {entries:?}: {patch:?}"
            );
        }
    }
}

#[test]
fn a_patch_that_deletes_or_moves_a_path_in_a_repository_is_still_held() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::Watch);
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), launch.path(), IssueRequirement::Watch);
    let inside = repo.path().join("src.rs").display().to_string();
    let outside = launch.path().join("notes.md").display().to_string();
    for entries in [
        [
            ("*** Add File", outside.as_str()),
            ("*** Delete File", inside.as_str()),
        ],
        [
            ("*** Add File", outside.as_str()),
            ("*** Move to", inside.as_str()),
        ],
        [
            ("*** Delete File", inside.as_str()),
            ("*** Delete File", outside.as_str()),
        ],
        [
            ("*** Move to", inside.as_str()),
            ("*** Move to", outside.as_str()),
        ],
    ] {
        let patch = patch_write(&context, &patch_body(&entries));
        assert!(held_for_an_issue(&patch), "{entries:?}: {patch:?}");
    }
}

#[test]
fn a_content_string_is_not_a_target_list_for_a_tool_that_is_not_a_patch() {
    let (_launch, _root, context) = in_a_non_repository(IssueRequirement::Watch);
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    // `pre_write_code` names its path in `file`, which `path_argument` does not
    // read, so this is a write whose destination the gate never read. A marker
    // in its `content` is body text, not a target list.
    for payload in [
        serde_json::json!({
            "file": repo.path().join("src.rs").display().to_string(),
            "content": "*** Begin Patch\n*** Add File: /tmp/elsewhere.md\n+x\n*** End Patch",
        }),
        serde_json::json!({
            "file": repo.path().join("src.rs").display().to_string(),
            "content": "*** Update File: /tmp/elsewhere.md\n@@\n-old\n+new\n",
        }),
    ] {
        let decision = unclaimed_edit(&context, "pre_write_code", payload.clone());
        assert!(held_for_an_issue(&decision), "{payload}: {decision:?}");
    }
}

#[cfg(unix)]
#[test]
fn an_ancestor_this_process_cannot_open_is_held_rather_than_exempted() {
    use std::os::unix::fs::PermissionsExt;

    let (launch, root, asking) = in_a_non_repository(IssueRequirement::Watch);
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
    let target = locked.join("notes.md");
    // Asserted so the coverage is conclusive rather than incidental: the arm is
    // reached only where `placed` answers for the path and the `.git` lookup
    // then fails. Were `placed` to answer `None` here, the hold would come from
    // `Unplaceable` and a mutation of the error arm would leave the suite green.
    assert!(
        crate::paths::placed(&target).is_some(),
        "{} is unplaceable, so this test never reaches the unreadable-ancestor arm",
        target.display()
    );
    assert!(
        super::holds_a_git_entry(&target).is_none(),
        "{} did not report an unreadable ancestor",
        target.display()
    );
    let decision = unclaimed_edit(
        &asking,
        "Write",
        serde_json::json!({ "file_path": target.display().to_string() }),
    );
    let refusing = unclaimed_edit(
        &context(root.path(), launch.path(), IssueRequirement::Required),
        "Write",
        serde_json::json!({ "file_path": target.display().to_string() }),
    );
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755))
        .expect("the mode is restored so the fixture can be cleaned up");
    let Decision::Ask(refusal) = &decision else {
        panic!("{decision:?}");
    };
    assert_eq!(refusal.code, CODE);
    assert!(
        matches!(&refusing, Decision::Deny(refusal) if refusal.code == CODE),
        "{refusing:?}"
    );
    assert!(
        refusal
            .message
            .contains("could not be placed outside every repository"),
        "{}",
        refusal.message
    );
}

#[test]
fn a_write_tool_whose_body_named_no_readable_path_is_still_held() {
    let (_launch, _root, context) = in_a_non_repository(IssueRequirement::Watch);
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
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
        assert!(held_for_an_issue(&decision), "{tool} {input}: {decision:?}");
    }
}

#[test]
fn a_write_from_a_non_git_directory_into_a_claimed_worktree_is_still_held() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::Watch);
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), launch.path(), IssueRequirement::Watch);
    let worktree = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(worktree.path());
    holder_of(&context, worktree.path());
    let decision = unclaimed_edit(
        &context,
        "Write",
        serde_json::json!({ "file_path": worktree.path().join("src.rs").display().to_string() }),
    );
    assert!(held_for_an_issue(&decision), "{decision:?}");
}

#[test]
fn a_repository_git_cannot_read_under_a_non_git_directory_is_still_held() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::Watch);
    std::fs::create_dir(launch.path().join(".git")).expect("a `.git` entry Git cannot use");
    let elsewhere = tempfile::tempdir().expect("somewhere else");
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path(), launch.path(), IssueRequirement::Watch);
    let decision = unclaimed_edit(
        &context,
        "Write",
        serde_json::json!({
            "file_path": elsewhere.path().join("notes.md").display().to_string()
        }),
    );
    let Decision::Ask(refusal) = &decision else {
        panic!("{decision:?}");
    };
    assert!(
        refusal.message.contains("could not be established"),
        "{}",
        refusal.message
    );
    assert!(
        !refusal.message.contains("this repository"),
        "{}",
        refusal.message
    );
}

#[test]
fn a_refusal_from_a_non_git_directory_does_not_call_it_this_repository() {
    let (launch, _root, _) = in_a_non_repository(IssueRequirement::Watch);
    let repo = crate::test_env::repository_with_commit();
    crate::test_env::add_github_remote(repo.path());
    let root = tempfile::tempdir().expect("a temporary root");
    for requirement in [IssueRequirement::Watch, IssueRequirement::Required] {
        let context = context(root.path(), launch.path(), requirement);
        let decision = unclaimed_edit(
            &context,
            "Write",
            serde_json::json!({ "file_path": repo.path().join("src.rs").display().to_string() }),
        );
        let refusal = match &decision {
            Decision::Ask(refusal) | Decision::Deny(refusal) => refusal,
            other => panic!("{requirement:?}: {other:?}"),
        };
        assert_eq!(refusal.code, CODE);
        assert!(
            !refusal.message.contains("this repository"),
            "{requirement:?}: {}",
            refusal.message
        );
    }
}

#[test]
fn a_session_starting_in_a_non_git_directory_is_not_told_it_is_this_repository() {
    let (_launch, _root, context) = in_a_non_repository(IssueRequirement::Watch);
    let text = session_start_text(&context);
    assert!(text.contains("ask"), "{text}");
    assert!(!text.contains("this repository"), "{text}");
}

fn hook_answer_in_mode(
    context: &GateContext,
    dialect: Dialect,
    agent: &str,
    mode: &serde_json::Value,
) -> serde_json::Value {
    let target = context.repo_dir.join("src.rs");
    let mut payload = serde_json::json!({
        "session_id": "abcd1234",
        "cwd": context.repo_dir.display().to_string(),
        "hook_event_name": "PreToolUse",
        "tool_name": "Edit",
        "tool_input": { "file_path": target.display().to_string() },
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
fn a_host_automatic_mode_leaves_the_hook_answer_a_refusal_or_a_question() {
    let modes = [
        serde_json::json!({}),
        serde_json::json!({ "permission_mode": "bypassPermissions" }),
        serde_json::json!({ "permission_mode": "auto" }),
        serde_json::json!({ "permission_mode": "dontAsk" }),
        serde_json::json!({ "permission_mode": "acceptEdits" }),
        serde_json::json!({ "approval_policy": "never", "sandbox_mode": "danger-full-access" }),
        serde_json::json!({ "permission_mode": "bypassPermissions", "approval_policy": "never" }),
    ];

    let (_repo, _root, required) = rig(IssueRequirement::Required);
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

    let (_repo, _root, asking) = rig(IssueRequirement::Watch);
    for mode in &modes[1..5] {
        let answer = hook_answer_in_mode(&asking, Dialect::ClaudeCode, "claude-code", mode);
        assert_eq!(decision_word(&answer), "ask", "{mode}: {answer}");
        let plain = hook_answer_in_mode(&asking, Dialect::ClaudeCode, "claude-code", &modes[0]);
        assert_eq!(answer, plain, "{mode}: the mode changed the answer");
    }
    for mode in &modes[1..] {
        let answer = hook_answer_in_mode(&asking, Dialect::ClaudeCode, "codex", mode);
        assert_eq!(decision_word(&answer), "deny", "codex {mode}: {answer}");
        assert!(answer.to_string().contains(CODE), "codex {mode}: {answer}");
    }
}
