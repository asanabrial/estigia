use std::path::Path;
use std::time::Duration;

use super::*;
use crate::config::{
    DecisionMode, Evidence, Integration, IssueRequirement, RepositoryHost, Tracker, Workflow,
};
use crate::harness::{Decision, GateContext, Sensitivity, session};
use crate::outcome::Observed;

fn context(root: &Path) -> GateContext {
    context_at(&root.join("state"), &root.join("repo"))
}

fn context_at(state_root: &Path, repo_dir: &Path) -> GateContext {
    GateContext {
        stand_down: None,
        integration: Integration::Branch,
        evidence: Evidence::Reading,
        flag: None,
        skill_root: repo_dir.with_file_name("skill"),
        repo_dir: repo_dir.to_path_buf(),
        state_root: state_root.to_path_buf(),
        operation_window: Default::default(),
        issue_requirement: IssueRequirement::Off,
        repository_config_unreadable: None,
        decision_mode: DecisionMode::Ask,
        workflow: Some(Workflow::Autonomous),
        window: Duration::from_secs(120),
        tracker: Tracker::Linear {
            team: "ENG".to_owned(),
        },
        repository_host: RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn decide(
    context: &GateContext,
    action: &Action,
    how: Sensitivity,
    observed: &Observed,
) -> Decision {
    let mut run = session::Run::new("claude-abcd1234".to_owned());
    crate::harness::gate_observing(context, &mut run, action, how, observed)
}

#[test]
fn a_write_under_the_state_directory_is_refused_whether_or_not_the_run_holds_an_issue() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let target = root
        .path()
        .join("state")
        .join("jobs")
        .join("x")
        .join("result.json");
    let action = Action::Write {
        target: target.display().to_string(),
    };
    for issue in [None, Some(crate::identity::Identity::Number(7))] {
        let mut run = session::Run::new("claude-abcd1234".to_owned());
        run.issue = issue.clone();
        let decision = crate::harness::gate_observing(
            &context,
            &mut run,
            &action,
            Sensitivity::Routine,
            &Observed::default(),
        );
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{issue:?}: {decision:?}"
        );
    }
}

#[test]
fn the_store_and_the_run_pointers_are_inside_the_same_directory() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    for name in ["state.db", "jobs/x/result.json", "state/0001.json"] {
        let target = root.path().join("state").join(name);
        let action = Action::Write {
            target: target.display().to_string(),
        };
        let decision = decide(
            &context,
            &action,
            Sensitivity::Routine,
            &Observed::default(),
        );
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{name}: {decision:?}"
        );
    }
}

#[test]
fn a_run_pointer_directory_named_runs_reads_one_level_up() {
    let runs = context_at(
        Path::new("/home/x/.estigia/runs"),
        Path::new("/home/x/repo"),
    );
    assert_eq!(directory_of(&runs).as_path(), Path::new("/home/x/.estigia"));
    let named = context_at(Path::new("/tmp/state"), Path::new("/tmp/repo"));
    assert_eq!(directory_of(&named).as_path(), Path::new("/tmp/state"));
}

#[test]
fn a_directory_that_only_looks_similar_is_not_the_state_directory() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    for name in ["stateish/x", "repo/.estigiaignore", "state-db/x"] {
        let target = root.path().join(name);
        let action = Action::Write {
            target: target.display().to_string(),
        };
        let decision = decide(
            &context,
            &action,
            Sensitivity::Routine,
            &Observed::default(),
        );
        assert!(
            !matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{name}: {decision:?}"
        );
    }
}

#[test]
fn a_shell_line_writing_into_the_state_directory_is_refused_whichever_verb_names_it() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let state = root.path().join("state");
    for line in [
        format!("echo x > {}/jobs/a/result.json", state.display()),
        format!("tee {}/jobs/a/result.json <<'EOF'", state.display()),
        format!("cd {} && echo x > state.db", state.display()),
    ] {
        let input = serde_json::json!({ "command": line });
        let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
        let observed = Observed {
            command: Some(command),
            ..Default::default()
        };
        let decision = decide(&context, &action, how, &observed);
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{line}: {decision:?}"
        );
    }
}

#[test]
fn an_argv_payload_writing_into_the_state_directory_is_refused() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let destination = root
        .path()
        .join("state")
        .join("jobs")
        .join("a")
        .join("result.json");
    let input = serde_json::json!({
        "command": ["tee", destination.display().to_string()]
    });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = Observed {
        command: Some(command),
        ..Default::default()
    };
    let decision = decide(&context, &action, how, &observed);
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn a_shell_line_writing_elsewhere_is_not_refused_by_this_rule() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let input = serde_json::json!({ "command": "echo x > notes.txt" });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = Observed {
        command: Some(command),
        ..Default::default()
    };
    let decision = decide(&context, &action, how, &observed);
    assert!(
        !matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_write_that_lands_on_the_state_directory_through_a_link_is_refused() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let repo = root.path().join("repo");
    let jobs = root.path().join("state").join("jobs");
    std::fs::create_dir_all(&repo).expect("the checkout directory");
    std::fs::create_dir_all(&jobs).expect("the jobs directory");
    std::os::unix::fs::symlink(&jobs, repo.join("link")).expect("a link onto the state directory");
    let action = Action::Write {
        target: repo
            .join("link")
            .join("x")
            .join("result.json")
            .display()
            .to_string(),
    };
    let decision = decide(
        &context,
        &action,
        Sensitivity::Routine,
        &Observed::default(),
    );
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn the_home_relative_spelling_is_read_without_asking_the_home_that_spelled_it() {
    let state = Path::new("/home/someone/.estigia");
    for line in [
        "echo x > ~/.estigia/jobs/a/result.json",
        "echo x > $HOME/.estigia/jobs/a/result.json",
        "cd ~/.estigia && echo x > state.db",
        "echo x > /home/someone/.estigia/runs/0001.json",
    ] {
        assert!(names_state_directory(line, state), "{line}");
    }
    assert!(!names_state_directory(
        "echo x > /home/someone/notes.txt",
        state
    ));
}

#[test]
fn the_push_guard_route_refuses_it_too() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let line = format!(
        "echo x > {}/jobs/a/result.json",
        root.path().join("state").display()
    );
    let input = serde_json::json!({ "command": line });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let literal = command.literal.text();
    let readings = crate::harness::guard::Readings {
        literal,
        text: command.against.text(),
        same_tier_fragments: &command.same_tier_fragments,
        write_targets: &command.write_targets,
    };
    let decision = crate::harness::guard::adjudicate_command(
        &context,
        &context.repo_dir,
        &action,
        how,
        &[],
        readings,
    )
    .decision;
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}
