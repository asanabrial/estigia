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
        workflow: Some(crate::config::Workflow::Autonomous),
        window: Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn rig(requirement: IssueRequirement) -> (tempfile::TempDir, tempfile::TempDir, GateContext) {
    let repo = crate::test_env::repository_with_commit();
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
    let (_repo, _root, context) = rig(IssueRequirement::Ask);
    let Decision::Ask(refusal) = edit(&context) else {
        panic!("an unclaimed edit was not put to the person");
    };
    assert_eq!(refusal.code, CODE);
}

#[test]
fn ask_is_delivered_as_ask_on_claude_code_and_nowhere_else() {
    let (_repo, _root, context) = rig(IssueRequirement::Ask);
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
    let (_repo, _root, context) = rig(IssueRequirement::Ask);
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

#[test]
fn a_boundary_write_is_left_to_the_checks_that_already_hold_it() {
    let (_repo, _root, context) = rig(IssueRequirement::Required);
    let decision = unclaimed_edit(
        &context,
        "Bash",
        serde_json::json!({ "command": "git push origin main" }),
    );
    assert_eq!(decision, Decision::Outside(Aside::NothingSworn));
}

#[test]
fn the_push_guards_refusal_is_still_the_one_a_guarded_commit_gets() {
    let repo = crate::test_env::repository_with_commit();
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
    let (_repo, _root, context) = rig(IssueRequirement::Ask);
    let context = under_linear(context);
    assert!(matches!(edit(&context), Decision::Ask(_)));
}

#[test]
fn a_read_only_tracker_without_an_assisted_workflow_refuses_nothing() {
    let repo = crate::test_env::repository_with_commit();
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
        let mut run = session::Run::new("claude-abcd1234".to_owned());
        run.issue = Some(crate::identity::Identity::Number(12));
        let target = context.repo_dir.join("src.rs");
        let input = serde_json::json!({ "file_path": target.display().to_string() });
        let (action, how, _) = crate::harness::classify_observing("Edit", &input, &[]);
        let decision = crate::harness::gate(&context, &mut run, &action, how);
        assert!(
            !matches!(&decision, Decision::Ask(_))
                && !matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{requirement:?}: {decision:?}"
        );
    }
}

#[test]
fn a_stand_down_lets_an_asked_write_through() {
    let (_repo, _root, mut context) = rig(IssueRequirement::Ask);
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
fn the_row_defaults_to_ask_and_only_the_repository_file_sets_it() {
    assert_eq!(Config::default().issue_requirement, IssueRequirement::Ask);
    let setting = crate::config::Setting::IssueRequirement;
    assert_eq!(setting.scope(), crate::config::Scope::Repository);
    assert!(!crate::config::AGENT_SETTINGS.contains(&setting));
    assert!(!crate::config::EVERYWHERE_SETTINGS.contains(&setting));
    assert!(!crate::config::MACHINE_SETTINGS.contains(&setting));
    for value in ["ask", "off", "required"] {
        let mut config = Config::default();
        setting.apply(&mut config, value).expect("an offered value");
        assert_eq!(setting.value_of(&config), value);
    }
    assert!(setting.apply(&mut Config::default(), "sometimes").is_err());
}

#[test]
fn the_repository_file_sets_it_and_the_contract_and_agent_files_do_not() {
    let repo = crate::test_env::repository_with_commit();
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
    assert_eq!(from_contract.issue_requirement, IssueRequirement::Ask);
    assert!(owned.is_empty());
    let (from_contract, _) = Config::read_scope_over(
        &Config::default(),
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Issue requirement | required | ask |\n",
        crate::config::Scope::Everywhere,
    )
    .expect("a read");
    assert_eq!(from_contract.issue_requirement, IssueRequirement::Ask);
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
        rendered.contains("| Unclaimed write policy | required | ask |"),
        "{rendered}"
    );
    let read = Config::read(
        &crate::config::CONFIG_FENCE.upsert("", &rendered),
        Some(&rendered),
    )
    .expect("a table");
    assert_eq!(read.issue_requirement, IssueRequirement::Ask);
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
    for requirement in [IssueRequirement::Ask, IssueRequirement::Required] {
        let (_repo, root, mut context) = rig(requirement);
        let worktree = crate::test_env::repository_with_commit();
        holder_of(&context, worktree.path());
        context.repo_dir = worktree.path().to_path_buf();
        let decision = edit(&context);
        assert!(
            !matches!(&decision, Decision::Ask(_))
                && !matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{requirement:?}: {decision:?}"
        );
        drop(root);
    }
}

#[test]
fn a_holder_whose_worktree_is_its_repo_dir_grants_no_exemption() {
    let (_repo, _root, mut context) = rig(IssueRequirement::Ask);
    let worktree = crate::test_env::repository_with_commit();
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
    let (_repo, _root, context) = rig(IssueRequirement::Ask);
    let worktree = crate::test_env::repository_with_commit();
    holder_of(&context, worktree.path());
    assert!(matches!(edit(&context), Decision::Ask(_)));
}

#[test]
fn an_unreadable_pointer_gives_no_exemption() {
    let (_repo, _root, mut context) = rig(IssueRequirement::Ask);
    let worktree = crate::test_env::repository_with_commit();
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

#[test]
fn a_redirect_to_an_absolute_path_outside_the_checkout_is_not_held() {
    for requirement in [IssueRequirement::Ask, IssueRequirement::Required] {
        let (_repo, _root, context) = rig(requirement);
        for command in ["echo x > /tmp/y", "echo x >> /tmp/y"] {
            let decision = shell_write(&context, command);
            assert!(
                matches!(decision, Decision::Outside(_)),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn rg_pre_runs_a_program_and_is_held() {
    for requirement in [IssueRequirement::Ask, IssueRequirement::Required] {
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
    for requirement in [IssueRequirement::Ask, IssueRequirement::Required] {
        let (_repo, _root, context) = rig(requirement);
        for command in ["rg pattern src", "rg pattern src > /tmp/out"] {
            let decision = shell_write(&context, command);
            assert!(
                matches!(decision, Decision::Outside(_)),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_quoted_heredoc_written_outside_the_checkout_is_not_held() {
    for requirement in [IssueRequirement::Ask, IssueRequirement::Required] {
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
            let decision = shell_write(&context, command);
            assert!(
                matches!(decision, Decision::Outside(_)),
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_heredoc_that_is_not_the_one_exact_shape_is_held() {
    for requirement in [IssueRequirement::Ask, IssueRequirement::Required] {
        let (repo, _root, context) = rig(requirement);
        let inside = format!("cat > {}/out.md <<'EOF'\nx\nEOF", repo.path().display());
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
            // Only the absolute-target heredoc is placed inside the checkout; every other shape
            // here is unplaceable, which `ask` passes and `required` still holds.
            let placed_inside = command == inside.as_str();
            let held = matches!(&decision, Decision::Ask(r) | Decision::Deny(r) if r.code == CODE);
            assert_eq!(
                held,
                requirement == IssueRequirement::Required || placed_inside,
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn a_heredoc_body_naming_a_repository_command_is_data_not_a_command() {
    let (_repo, _root, context) = rig(IssueRequirement::Ask);
    let decision = shell_write(&context, "cat > /tmp/a <<'EOF'\nrun git commit here\nEOF");
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
    for requirement in [IssueRequirement::Ask, IssueRequirement::Required] {
        let (repo, _root, context) = rig(requirement);
        let inside = format!("echo x > {}/out.txt", repo.path().display());
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
            // A shell write is placed inside only by its literal redirect targets or by naming a
            // repository operation; the rest are unplaceable, which `ask` passes.
            let placed_inside = command == inside.as_str()
                || matches!(
                    command,
                    "echo x > out.txt"
                        | "echo x > ./a"
                        | "echo x > /tmp/a > out.txt"
                        | "git commit -m x > /tmp/log"
                        | "git reset --hard > /tmp/log"
                        | "git checkout -b x > /tmp/log"
                        | "gh issue develop 3 > /tmp/log"
                );
            let held = matches!(&decision, Decision::Ask(r) | Decision::Deny(r) if r.code == CODE);
            assert_eq!(
                held,
                requirement == IssueRequirement::Required || placed_inside,
                "{requirement:?} {command}: {decision:?}"
            );
        }
    }
}

#[test]
fn the_three_prompting_shapes_pass_under_ask_without_a_claim() {
    let scratch = "/private/tmp/estigia-719-scratchpad";
    let shapes = [
        format!("cd {scratch} && cat > c64.md <<'EOF'\n## Problem\nan issue body\nEOF"),
        format!("cd {scratch}/ga; bash -c 'grep -n src src/lib.rs; sed -n 1,10p src/lib.rs'"),
        format!("cd {scratch} && printf '%s\\n' an issue line >> a2.md"),
    ];
    let (_repo, _root, ask) = rig(IssueRequirement::Ask);
    for shape in &shapes {
        let decision = shell_write(&ask, shape);
        assert!(
            !matches!(&decision, Decision::Ask(_) | Decision::Deny(_)),
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
fn a_write_the_gate_cannot_place_is_reported_at_session_start_rather_than_asked() {
    let (_repo, _root, ask) = rig(IssueRequirement::Ask);
    assert!(matches!(
        shell_write(&ask, "echo x > src/lib.rs"),
        Decision::Ask(_)
    ));
    assert!(matches!(
        shell_write(&ask, "bash -c 'echo x > src/lib.rs'"),
        Decision::Allow(_)
    ));

    let repo = crate::test_env::repository_with_commit();
    let root = tempfile::tempdir().expect("a temporary root");
    let asking = context(root.path(), repo.path(), IssueRequirement::Ask);
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
