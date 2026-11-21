use super::*;
use crate::harness::Decision;
use crate::transport::store::{OutboxKind, Repo, Store};

fn repo() -> Repo {
    Repo {
        host: "github.com".to_owned(),
        owner: "o".to_owned(),
        name: "r".to_owned(),
    }
}

fn store() -> Store {
    Store::open(&crate::transport::store::store_path().expect("a store path")).expect("opens")
}

fn queued() -> u64 {
    let mut held = store();
    let provisional = held.allocate_provisional(&repo()).expect("allocates");
    crate::transport::outbox::queue_in(
        &mut held,
        &repo(),
        provisional,
        OutboxKind::Create,
        &serde_json::json!({"title": "x"}),
        "2026-11-17T08:00Z",
    )
    .expect("queues");
    provisional
}

#[test]
fn the_reads_and_writes_of_one_issue_are_named() {
    for line in [
        "gh issue view 9000000000032",
        "gh issue view #9000000000032 --json body",
        "gh -R o/r issue comment 9000000000032 -b x",
        "cd x && gh issue edit 9000000000032 --add-label y",
        "gh issue close https://github.com/o/r/issues/9000000000032",
        "gh api graphql -f query='{ repository(owner:\"o\",name:\"r\") { issue(number: 9000000000032) { id } } }'",
    ] {
        assert_eq!(
            named_provisional(line).map(|(number, _)| number),
            Some(9_000_000_000_032),
            "`{line}`"
        );
    }
    for line in [
        "gh issue view 32",
        "gh issue list",
        "echo 9000000000032",
        "gh pr view 9000000000032",
    ] {
        assert_eq!(named_provisional(line), None, "`{line}`");
    }
}

#[test]
fn a_queued_number_answers_queued_from_the_store() {
    let provisional = queued();
    let answer = refusal(Some(&format!("gh issue view {provisional}"))).expect("answered");
    assert_eq!(answer.code, CODE);
    let said = answer.to_string();
    assert!(said.contains("waits in this machine's queue"), "{said}");
    assert!(said.contains("queue entry"), "{said}");
    assert!(!said.contains("not found"), "{said}");
}

#[test]
fn a_sent_number_answers_the_real_one() {
    let provisional = queued();
    let mut held = store();
    held.import(
        &repo(),
        provisional,
        &serde_json::json!({"body": "", "labels": [], "comments": []}),
        "2026-11-17T08:00Z",
    )
    .expect("held");
    held.rekey(&repo(), provisional, 480, None).expect("rekeys");
    let said = refusal(Some(&format!("gh issue view {provisional}")))
        .expect("answered")
        .to_string();
    assert!(said.contains("GitHub filed it as #480"), "{said}");
    assert!(said.contains("naming #480"), "{said}");
}

#[test]
fn a_number_no_store_here_allocated_says_so() {
    let said = refusal(Some("gh issue view 9000000000999"))
        .expect("answered")
        .to_string();
    assert!(said.contains("never allocated"), "{said}");
}

#[test]
fn another_repository_named_with_r_is_left_to_github() {
    let provisional = queued();
    assert!(refusal(Some(&format!("gh issue view {provisional} -R other/repo"))).is_none());
    assert!(refusal(Some(&format!("gh issue view {provisional} -R o/r"))).is_some());
}

#[test]
fn the_gate_answers_before_the_command_reaches_github() {
    let provisional = queued();
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    let input = serde_json::json!({ "command": format!("gh issue view {provisional}") });
    let (action, how, observed) = crate::harness::classify_observing("Bash", &input, &[]);
    let literal = match &observed.literal {
        crate::outcome::Established::Known(text) => Some(text.as_str()),
        _ => None,
    };
    let unclaimed = crate::harness::guard::adjudicate_command(
        &context,
        root.path(),
        &action,
        how,
        &[],
        crate::harness::guard::Readings {
            literal,
            same_tier_fragments: &observed.same_tier_fragments,
            write_targets: &observed.write_targets,
        },
    )
    .decision;
    assert!(
        matches!(&unclaimed, Decision::Deny(answer) if answer.code == CODE),
        "{unclaimed:?}"
    );
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(7));
    let seen = crate::outcome::Observed {
        command: Some(observed),
        ..Default::default()
    };
    let held = crate::harness::gate_observing(&context, &mut run, &action, how, &seen);
    assert!(
        matches!(&held, Decision::Deny(answer) if answer.code == CODE),
        "{held:?}"
    );
}

fn test_context(root: &std::path::Path) -> crate::harness::GateContext {
    let skill_root = root.join("skill");
    std::fs::create_dir_all(&skill_root).expect("a skill root");
    crate::harness::GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root,
        repo_dir: root.to_path_buf(),
        state_root: root.join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        repository_config_unreadable: None,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: std::time::Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}
