use super::*;
use crate::harness::{Action, Decision, Sensitivity, classify_with};

fn names(line: &str) -> bool {
    names_issue_creation(&super::super::without_leading_options(
        &line.to_ascii_lowercase(),
    ))
}

#[test]
fn every_spelling_that_files_an_issue_is_named() {
    for line in [
        "gh issue create --title x --body y",
        "gh -R o/r issue create --title x",
        "cd repo && gh issue create -t x",
        "gh api repos/o/r/issues -f title=x",
        "gh api repos/o/r/issues -X POST --input body.json",
        "gh api --method POST /repos/o/r/issues",
        "gh api \"repos/o/r/issues\" -F body=@note.md",
        "gh api graphql -f query='mutation { createIssue(input: {}) { clientMutationId } }'",
        "gh api -XGET repos/o/r/pulls; gh api repos/o/r/issues -f title=x",
        "gh api repos/o/r/issues -f title=fix-xgetter",
        "gh api -X POST ${REPO?}/issues -f title=x",
        "true & gh issue create -t x",
        "if x; then gh issue create; fi",
        "sudo -u root gh issue create",
        "timeout 30 gh issue create",
        "xargs -n 1 gh issue create",
        "echo it\\'s done && gh issue create",
        "echo \\\"; gh issue create",
        "case a in x) gh issue create;; esac",
        "echo \"$(gh issue create)\"",
    ] {
        assert!(names(line), "`{line}` files an issue and was not named");
    }
}

#[test]
fn a_graphql_call_whose_query_cannot_be_read_is_an_api_write() {
    for line in [
        "gh api graphql --input c.json",
        "gh api graphql -f query=@c.graphql",
    ] {
        assert!(
            super::super::mutates_through_the_api(line),
            "`{line}` was read as no write"
        );
    }
}

#[test]
fn the_alias_and_options_between_the_words_are_named() {
    for line in [
        "gh issue new --title x",
        "gh issue -R o/r create -t x",
        "gh issue --repo o/r new",
        "gh issue --repo=o/r create",
        "cd repo && gh -R o/r issue create",
        "cd repo && gh --hostname=h.example -R o/r issue new",
        "gh issue --hostname h.example create",
    ] {
        assert!(names(line), "`{line}` files an issue and was not named");
    }
}

#[test]
fn the_api_spellings_beyond_the_plain_ones_are_named() {
    for line in [
        "gh api repos/o/r/issues?x=1 -f title=x",
        "gh api \"repos/o/r/issues?x=1\" -f title=x",
        "gh api repos/o/r/issues -XPOST",
        "gh api repos/o/r/issues -X=POST",
        "gh api repos/o/r/issues --method=POST",
    ] {
        assert!(names(line), "`{line}` files an issue and was not named");
    }
}

#[test]
fn reads_and_other_issue_commands_with_options_are_not_named() {
    for line in [
        "gh api repos/o/r/issues?x=1",
        "gh issue -R o/r view 3",
        "gh issue -R o/r list",
        "gh -R o/r issue edit 3",
    ] {
        assert!(
            !names(line),
            "`{line}` does not file an issue and was named"
        );
    }
}

#[test]
fn reading_or_editing_issues_is_not_named() {
    for line in [
        "gh issue view 12",
        "gh issue list --state open",
        "gh issue edit 12 --add-label x",
        "gh issue comment 12 --body x",
        "gh api repos/o/r/issues",
        "gh api repos/o/r/issues/12 -X PATCH -f state=closed",
        "gh api repos/o/r/issues/12/comments -f body=x",
        "gh api -X GET repos/o/r/issues -f state=open",
        "gh api graphql -f query='mutation { closeIssue }'",
        "estigia create",
    ] {
        assert!(
            !names(line),
            "`{line}` does not file an issue and was named"
        );
    }
}

#[test]
fn the_refusal_names_the_create_tool_as_the_way_out() {
    let refusal = refusal(Some("gh issue create --title x")).expect("refused");
    assert_eq!(refusal.code, CODE);
    assert!(refusal.to_string().contains("`create` tool"));
    assert!(super::refusal(Some("gh issue view 1")).is_none());
    assert!(super::refusal(None).is_none());
}

#[test]
fn the_refusal_names_the_repo_argument_for_filing_in_another_repository() {
    for line in [
        "gh issue create --title x",
        "gh issue create --repo o/b --title x",
        "gh -R o/b issue create -t x",
    ] {
        let refusal = refusal(Some(line)).expect("refused");
        assert_eq!(refusal.code, CODE, "{line}");
        let said = refusal.to_string();
        assert!(
            said.contains("`create` tool")
                && said.contains("`repo`")
                && said.contains("owner/name"),
            "`{line}` was refused without naming the supported path to another repository: {said}"
        );
    }
}

#[test]
fn gh_issue_create_is_a_classified_write_not_an_untouched_call() {
    let (action, how) = classify_with(
        "Bash",
        &serde_json::json!({ "command": "gh issue create --title x" }),
        &[],
    );
    assert!(matches!(action, Action::Write { .. }), "{action:?}");
    assert_eq!(how, Sensitivity::Routine);
}

#[test]
fn a_run_with_no_claim_is_refused_too() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    let input = serde_json::json!({ "command": "gh issue create --title x" });
    let (action, how, observed) = crate::harness::classify_observing("Bash", &input, &[]);
    let literal = match &observed.literal {
        crate::outcome::Established::Known(text) => Some(text.as_str()),
        _ => None,
    };
    let decision = crate::harness::guard::adjudicate_command(
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
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn a_run_holding_an_issue_is_refused_too() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(7));
    let input = serde_json::json!({ "command": "gh issue create --title x" });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = crate::outcome::Observed {
        command: Some(command),
        ..Default::default()
    };
    let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
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
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: std::time::Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}
