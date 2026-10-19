use super::*;
use crate::harness::{Action, Decision, Sensitivity, classify_with};

fn names(line: &str) -> bool {
    named_in(&line.to_ascii_lowercase()).is_some_and(|command| command.verb == "stand-down")
}

const TARGET: &str = "estigia stand-down";
const CODE: &str = "stand-down-is-for-a-person";

#[test]
fn every_spelling_that_runs_the_stand_down_is_named() {
    for line in [
        "estigia stand-down --reason x --minutes 5",
        "estigia --json stand-down --reason x",
        "cd repo && estigia stand-down --reason x",
        "/usr/local/bin/estigia stand-down --reason x",
        "ESTIGIA.EXE stand-down",
        "true; estigia stand-down --reason x",
        "sudo estigia stand-down --reason x",
        "echo ok && (estigia stand-down --reason x)",
        "cd repo;estigia stand-down --reason x",
        "FOO=bar estigia stand-down --reason x",
        "timeout 5 estigia stand-down --reason x",
        "nice -n 5 estigia stand-down --reason x",
        "env X=1 estigia stand-down --reason x",
        "sudo -u x estigia stand-down --reason x",
        "xargs -0 estigia stand-down --reason x",
        "if estigia stand-down --reason x; then :; fi",
        "{ estigia stand-down --reason x; }",
        "! estigia stand-down --reason x",
        "while true; do estigia stand-down --reason x; done",
        "bash -c \"estigia stand-down --reason x\"",
        "sh -lc 'estigia stand-down --reason x'",
        ".\\estigia.exe stand-down --reason x",
        "a&&estigia stand-down --reason x",
        "x=$(estigia stand-down --reason x)",
        "`estigia` stand-down --reason x",
        "echo \"$(estigia stand-down --reason x)\"",
        "estigia stand\\-down --reason x",
        "estigia st''and-down --reason x",
        "es''tigia stand-down --reason x",
        "estigia \"stand\"-down --reason x",
    ] {
        assert!(
            names(line),
            "`{line}` runs the stand-down and was not named"
        );
    }
}

#[test]
fn a_lookalike_is_not_named() {
    for line in [
        "estigia stand-downx --reason x",
        "estigia status",
        "estigia config set Tracker github",
        "git log --grep stand-down",
        "echo stand-down",
        "estigia-stand-down --reason x",
        "cat estigia/stand-down",
        "estigia status --note stand-down",
        "estigia doctor stand-down",
        "cd estigia/stand-down",
        "estigia doctor",
    ] {
        assert!(
            !names(line),
            "`{line}` does not run the stand-down and was named"
        );
    }
}

#[test]
fn the_refusal_gives_the_person_the_command_and_names_the_bang() {
    let action = Action::Write {
        target: TARGET.to_owned(),
    };
    let refusal = refusal(&action).expect("refused");
    assert_eq!(refusal.code, CODE);
    let text = refusal.to_string();
    assert!(text.contains("estigia stand-down --reason"), "{text}");
    assert!(text.contains("`!`"), "{text}");
    assert!(
        super::refusal(&Action::Write {
            target: "estigia config set".to_owned()
        })
        .is_none()
    );
}

#[test]
fn the_stand_down_is_a_classified_write_not_an_untouched_call() {
    let (action, how) = classify_with(
        "Bash",
        &serde_json::json!({ "command": "estigia stand-down --reason x" }),
        &[],
    );
    assert!(matches!(action, Action::Write { .. }), "{action:?}");
    assert_eq!(how, Sensitivity::Routine);
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
        workflow: Some(crate::config::Workflow::Autonomous),
        window: std::time::Duration::from_secs(120),
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

#[test]
fn an_agent_run_is_denied_whether_or_not_it_holds_an_issue() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    let input = serde_json::json!({ "command": "estigia stand-down --reason x" });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = crate::outcome::Observed {
        command: Some(command),
        ..Default::default()
    };
    for issue in [None, Some(crate::identity::Identity::Number(7))] {
        let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
        run.issue = issue;
        let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{decision:?}"
        );
    }
}

#[test]
fn the_push_guard_denies_it_too() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    let input = serde_json::json!({ "command": "estigia stand-down --reason x" });
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
fn an_argv_payload_with_no_literal_command_is_denied_everywhere() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    let input = serde_json::json!({ "command": ["bash", "-lc", "estigia stand-down --reason x"] });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    assert!(
        matches!(&action, Action::Write { target } if target == TARGET),
        "{action:?}"
    );
    assert!(
        !matches!(command.literal, crate::outcome::Established::Known(_)),
        "the payload carried a literal, so this proves nothing: {:?}",
        command.literal
    );
    let observed = crate::outcome::Observed {
        command: Some(command.clone()),
        ..Default::default()
    };
    for issue in [None, Some(crate::identity::Identity::Number(7))] {
        let mut run = crate::harness::session::Run::new("codex-abcd1234".to_owned());
        run.issue = issue;
        let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{decision:?}"
        );
    }
    let decision = crate::harness::guard::adjudicate_command(
        &context,
        root.path(),
        &action,
        how,
        &[],
        crate::harness::guard::Readings {
            literal: None,
            same_tier_fragments: &command.same_tier_fragments,
            write_targets: &command.write_targets,
        },
    )
    .decision;
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "{decision:?}"
    );
}

#[test]
fn a_commit_message_that_names_the_stand_down_is_refused_too_by_design() {
    for line in [
        "git commit -m \"fix(estigia): stand-down the focus\"",
        "git commit -m 'estigia: stand-down the focus'",
    ] {
        let (action, _) = classify_with("Bash", &serde_json::json!({ "command": line }), &[]);
        assert!(
            matches!(&action, Action::Write { target } if target == TARGET),
            "`{line}` was not refused: {action:?}"
        );
    }
}

#[test]
fn a_stand_down_beside_a_disarm_fragment_or_a_boundary_is_still_denied() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    for line in [
        "estigia setup --help; estigia stand-down --reason r",
        "echo core.hookspath; estigia stand-down --reason r",
        "git config --get core.hookspath; estigia stand-down --reason x",
        "gh pr merge 5 --merge && estigia stand-down --reason x",
    ] {
        let input = serde_json::json!({ "command": line });
        let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
        assert!(
            matches!(&action, Action::Write { target } if target == TARGET),
            "`{line}` was classified {action:?}"
        );
        let observed = crate::outcome::Observed {
            command: Some(command.clone()),
            ..Default::default()
        };
        for issue in [None, Some(crate::identity::Identity::Number(7))] {
            let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
            run.issue = issue;
            let decision =
                crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
            assert!(
                matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
                "`{line}`: {decision:?}"
            );
        }
        let literal = match &command.literal {
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
                same_tier_fragments: &command.same_tier_fragments,
                write_targets: &command.write_targets,
            },
        )
        .decision;
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "push guard, `{line}`: {decision:?}"
        );
    }
}

fn stood_down(context: &mut crate::harness::GateContext) {
    let now = crate::harness::session::now_seconds().expect("a readable clock");
    context.stand_down = Some(crate::harness::standdown::StandDown {
        reason: "a fixture".to_owned(),
        declared_at: now,
        until: now + 600,
        declared_by: "an agent".to_owned(),
    });
}

#[test]
fn a_stand_down_does_not_lift_its_own_deny_but_still_lifts_another() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = test_context(root.path());
    stood_down(&mut context);
    let input = serde_json::json!({ "command": "estigia stand-down --reason x" });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = crate::outcome::Observed {
        command: Some(command.clone()),
        ..Default::default()
    };
    for issue in [None, Some(crate::identity::Identity::Number(7))] {
        let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
        run.issue = issue;
        let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
        assert!(
            matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
            "{decision:?}"
        );
    }
    let literal = match &command.literal {
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
            same_tier_fragments: &command.same_tier_fragments,
            write_targets: &command.write_targets,
        },
    )
    .decision;
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == CODE),
        "push guard: {decision:?}"
    );

    let filing = serde_json::json!({ "command": format!("gh issue {} --title x", "create") });
    let (action, how, command) = crate::harness::classify_observing("Bash", &filing, &[]);
    let observed = crate::outcome::Observed {
        command: Some(command),
        ..Default::default()
    };
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
    assert!(
        matches!(&decision, Decision::Allow(why) if why.contains("stood down")),
        "the stand-down no longer lifts anything: {decision:?}"
    );
}

fn denied_by_every_road(context: &crate::harness::GateContext, root: &std::path::Path, line: &str) {
    denied_by_every_road_with(
        context,
        root,
        "Bash",
        &serde_json::json!({ "command": line }),
        TARGET,
    );
}

fn denied_by_every_road_with(
    context: &crate::harness::GateContext,
    root: &std::path::Path,
    tool: &str,
    input: &serde_json::Value,
    target: &str,
) {
    let line = input.to_string();
    let code = refusal(&Action::Write {
        target: target.to_owned(),
    })
    .expect("a person-only target")
    .code;
    let (action, how, command) = crate::harness::classify_observing(tool, input, &[]);
    assert!(
        matches!(&action, Action::Write { target: named } if named == target),
        "`{line}` was classified {action:?}"
    );
    let observed = crate::outcome::Observed {
        command: Some(command.clone()),
        ..Default::default()
    };
    for issue in [None, Some(crate::identity::Identity::Number(7))] {
        let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
        run.issue = issue;
        for decision in [
            crate::harness::gate_observing(context, &mut run, &action, how, &observed),
            crate::harness::gate(context, &mut run, &action, how),
        ] {
            assert!(
                matches!(&decision, Decision::Deny(refusal) if refusal.code == code),
                "`{line}`: {decision:?}"
            );
        }
    }
    let literal = match &command.literal {
        crate::outcome::Established::Known(text) => Some(text.as_str()),
        _ => None,
    };
    let decision = crate::harness::guard::adjudicate_command(
        context,
        root,
        &action,
        how,
        &[],
        crate::harness::guard::Readings {
            literal,
            same_tier_fragments: &command.same_tier_fragments,
            write_targets: &command.write_targets,
        },
    )
    .decision;
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == code),
        "push guard, `{line}`: {decision:?}"
    );
}

#[test]
fn a_stand_down_on_a_compound_line_is_denied_by_every_road_under_a_stand_down() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = test_context(root.path());
    stood_down(&mut context);
    let command = "estigia stand-down --reason r";
    for line in [
        format!("gh issue {}ate --title x; {command}", "cre"),
        format!("estigia setup --help; {command}"),
        format!("git push origin b && {command}"),
    ] {
        denied_by_every_road(&context, root.path(), &line);
    }
}

#[test]
fn a_quoted_or_escaped_verb_is_denied_by_every_road() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = test_context(root.path());
    stood_down(&mut context);
    for line in [
        "estigia stand\\-down --reason x",
        "estigia st''and-down --reason x",
        "es''tigia stand-down --reason x",
        "estigia \"stand\"-down --reason x",
    ] {
        denied_by_every_road(&context, root.path(), line);
    }
}

#[test]
fn the_issue_filing_line_alone_still_has_its_own_deny() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    let line = format!("gh issue {}ate --title x", "cre");
    let input = serde_json::json!({ "command": line });
    let (action, how, command) = crate::harness::classify_observing("Bash", &input, &[]);
    let observed = crate::outcome::Observed {
        command: Some(command),
        ..Default::default()
    };
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
    assert!(
        matches!(&decision, Decision::Deny(refusal) if refusal.code == crate::harness::issue_creation::CODE),
        "{decision:?}"
    );
}

#[test]
fn the_classifier_reads_every_path_and_option_spelling_as_the_stand_down() {
    for line in [
        "/usr/local/bin/estigia --json stand-down --reason x",
        ".\\estigia.exe stand-down --reason x",
        "ESTIGIA.EXE --json stand-down --reason x",
        "C:\\tools\\estigia.exe stand-down --reason x",
    ] {
        let (action, _) = classify_with("Bash", &serde_json::json!({ "command": line }), &[]);
        assert!(
            matches!(&action, Action::Write { target } if target == TARGET),
            "`{line}` was classified {action:?}"
        );
    }
}

const RECORD_AT: &str = "/home/someone/.estigia/stand-down.json";

fn writes_to_the_record() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        (
            "Write",
            serde_json::json!({ "file_path": RECORD_AT, "content": "{}" }),
        ),
        (
            "Edit",
            serde_json::json!({ "file_path": RECORD_AT, "old_string": "a", "new_string": "b" }),
        ),
        (
            "MultiEdit",
            serde_json::json!({ "file_path": RECORD_AT, "edits": [] }),
        ),
        (
            "NotebookEdit",
            serde_json::json!({ "notebook_path": RECORD_AT, "new_source": "x" }),
        ),
        (
            "apply_patch",
            serde_json::json!({ "input": format!("*** Begin Patch\n*** Update File: {RECORD_AT}\n@@\n-a\n+b\n*** End Patch") }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": format!("echo '{{}}' > {RECORD_AT}") }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "echo x > ~/.estigia/stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "echo x >> $HOME/.estigia/stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "echo x | tee ~/.estigia/stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cp /tmp/x ~/.estigia/stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "mv /tmp/x ~/.estigia/stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "rm ~/.estigia/stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": ["bash", "-lc", "echo x > ~/.estigia/stand-down.json"] }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd ~/.estigia && echo '{}' > stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd $HOME/.estigia && echo x > stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd .estigia && echo x | tee stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": ["bash", "-lc", "cd ~/.estigia && echo x > stand-down.json"] }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd -P ~/.estigia && echo x > stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd ~/.estigia&&echo x>stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd; cd ~/.estigia && echo x > stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "echo x > ~/.estigia/stand-down.json." }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd ~/.estigia && echo x > stand-down.json." }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "pushd ~/.estigia && echo x > stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd ~/.estigia. && echo x > stand-down.json" }),
        ),
    ]
}

#[test]
fn every_write_to_the_record_is_the_person_only_target() {
    for (tool, input) in writes_to_the_record() {
        let (action, _, _) = crate::harness::classify_observing(tool, &input, &[]);
        assert!(
            matches!(&action, Action::Write { target } if target == RECORD.target),
            "{tool} {input} was classified {action:?}"
        );
    }
}

#[test]
fn a_read_of_the_record_or_a_lookalike_is_not_the_person_only_target() {
    for (tool, input) in [
        ("Read", serde_json::json!({ "file_path": RECORD_AT })),
        (
            "Bash",
            serde_json::json!({ "command": "cat ~/.estigia/stand-down.json" }),
        ),
        (
            "Write",
            serde_json::json!({ "file_path": "/home/x/.estigia/runs/stand-down.json" }),
        ),
        (
            "Write",
            serde_json::json!({ "file_path": "/home/x/notes/stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "echo x > ~/.estigiaignore" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "echo x > stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd ~/.estigia && echo x > notes.txt" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "cd ~/.estigia && cat stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "echo .estigia > /tmp/notes; echo y > stand-down.json" }),
        ),
        (
            "Bash",
            serde_json::json!({ "command": "echo x > ~/.estigia/stand-down.json.bak" }),
        ),
    ] {
        let (action, _, _) = crate::harness::classify_observing(tool, &input, &[]);
        assert!(
            !matches!(&action, Action::Write { target } if target == RECORD.target),
            "{tool} {input} was classified {action:?}"
        );
    }
}

#[test]
fn a_backup_of_the_record_is_not_the_record() {
    assert!(
        !is_the_record("/home/x/.estigia/stand-down.json.bak"),
        "a backup was read as the record, so it is refused with a way out that declares"
    );
    assert!(
        !matches!(
            classified("echo x > ~/.estigia/stand-down.json.bak"),
            Action::Write { target } if target == RECORD.target
        ),
        "a shell write to a backup was classified as the record"
    );
    // The floor: the record itself is still the record, on both roads.
    assert!(is_the_record("/home/x/.estigia/stand-down.json"));
    assert!(matches!(
        classified("echo x > ~/.estigia/stand-down.json"),
        Action::Write { target } if target == RECORD.target
    ));
}

#[test]
fn a_write_to_the_record_is_denied_by_every_road_with_or_without_a_stand_down() {
    let root = tempfile::tempdir().expect("a temporary root");
    let plain = test_context(root.path());
    let mut standing = test_context(root.path());
    stood_down(&mut standing);
    for context in [&plain, &standing] {
        for (tool, input) in writes_to_the_record() {
            denied_by_every_road_with(context, root.path(), tool, &input, RECORD.target);
        }
    }
}

#[test]
fn a_person_only_denial_carries_the_code_and_a_way_out_that_declares() {
    let refusal = refusal(&Action::Write {
        target: RECORD.target.to_owned(),
    })
    .expect("refused");
    assert_eq!(refusal.code, CODE);
    assert!(refusal.to_string().contains("estigia stand-down --reason"));
}

#[test]
fn a_declaration_that_mentions_the_lift_is_still_refused() {
    for line in [
        "estigia stand-down --reason x",
        "estigia stand-down --reason \"tidy --lift\"",
        "estigia stand-down --reason --lift",
        "estigia stand-down --reason=--lift",
        "estigia stand-down --minutes 5 --reason x --lift",
        "estigia stand-down --reason lift",
        "estigia stand-down; echo --lift",
        "estigia stand-down --lift --reason a; estigia stand-down --reason b",
        "echo --lift; estigia stand-down --reason b",
    ] {
        assert!(names(line), "`{line}` declares and was not named");
    }
}

const LIFTS: &[&str] = &[
    "estigia stand-down --rea\\son=, #--lift",
    "estigia stand-down --lift",
    "estigia stand-down --lift --reason cleared",
    "estigia stand-down --reason cleared --lift",
    "estigia --json stand-down --lift --reason x",
];

fn classified(line: &str) -> Action {
    classify_with("Bash", &serde_json::json!({ "command": line }), &[]).0
}

#[test]
fn an_agents_lift_is_refused_with_a_way_out_that_lifts() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = test_context(root.path());
    for line in LIFTS {
        let action = classified(line);
        let refusal = refusal(&action).unwrap_or_else(|| panic!("`{line}` was {action:?}"));
        assert_eq!(refusal.code, CODE, "{line}");
        denied_by_every_road_with(
            &context,
            root.path(),
            "Bash",
            &serde_json::json!({ "command": line }),
            LIFT.target,
        );
    }
    let text = refusal(&classified("estigia stand-down --reason a --lift"))
        .expect("refused")
        .to_string();
    assert!(text.contains("--lift --reason"), "{text}");
    let declaring = refusal(&classified("estigia stand-down --reason a"))
        .expect("refused")
        .to_string();
    assert!(!declaring.contains("--lift"), "{declaring}");
}

#[test]
fn the_lift_the_doctor_prints_is_refused_to_an_agent() {
    assert!(matches!(
        classified("estigia stand-down --lift --reason cleared"),
        Action::Write { target } if target == LIFT.target
    ));
}

#[test]
fn a_quoted_or_escaped_record_spelling_is_still_the_record() {
    for line in [
        "echo x > ~/.estigia/\"stand-down\".json",
        "echo x > ~/.estigia/stand\\-down.json",
        "echo x > ~/.estigia/'stand-down.json'",
    ] {
        assert!(
            matches!(classified(line), Action::Write { target } if target == RECORD.target),
            "`{line}`"
        );
    }
    assert!(is_the_record("/home/x/.estigia/\"stand-down\".json"));
    assert!(is_the_record("/home/x/.estigia/stand\\-down.json"));
}

#[test]
fn reading_the_record_stays_allowed() {
    for (tool, input) in [
        ("Read", serde_json::json!({ "file_path": RECORD_AT })),
        (
            "Bash",
            serde_json::json!({ "command": "cat ~/.estigia/stand-down.json" }),
        ),
    ] {
        let (action, _) = classify_with(tool, &input, &[]);
        assert!(matches!(action, Action::Untouched), "{tool}: {action:?}");
    }
}

const RECOVER: &str = "estigia recover";
const RECOVER_CODE: &str = "recover-is-for-a-person";

fn recovers(line: &str) -> bool {
    named_in(&line.to_ascii_lowercase()).is_some_and(|command| command.verb == "recover")
}

#[test]
fn every_spelling_that_runs_the_recovery_is_named() {
    for line in [
        "estigia recover 716 --epoch abc --authorized-by someone --reason x",
        "estigia --json recover 716 --epoch abc",
        "estigia recover 716 --reason \"why\"",
        "estigia recover 716",
        "cd repo && estigia recover 716 --epoch abc",
        "/usr/local/bin/estigia recover 716",
        "ESTIGIA.EXE recover 716",
        "true; estigia recover 716",
        "sudo estigia recover 716",
        "echo ok && (estigia recover 716)",
        "cd repo;estigia recover 7",
        "FOO=bar estigia recover 7",
        "timeout 5 estigia recover 7",
        "nice -n 5 estigia recover 7",
        "env X=1 estigia recover 7",
        "sudo -u x estigia recover 7",
        "xargs -0 estigia recover 7",
        "if estigia recover 7; then :; fi",
        "{ estigia recover 7; }",
        "! estigia recover 7",
        "while true; do estigia recover 7; done",
        "bash -c \"estigia recover 7\"",
        "sh -lc 'estigia recover 7'",
        ".\\estigia.exe recover 7",
        "a&&estigia recover 7",
        "x=$(estigia recover 7)",
        "`estigia` recover 7",
        "echo \"$(estigia recover 7)\"",
    ] {
        assert!(
            recovers(line),
            "`{line}` runs the recovery and was not named"
        );
    }
}

#[test]
fn a_recovery_lookalike_is_not_named() {
    for line in [
        "estigia recovery 716",
        "estigia status",
        "git log --grep recover",
        "echo recover",
        "estigia-recover 716",
        "cat estigia/recover",
        "estigia status --note recover",
        "estigia doctor recover",
        "cd estigia/recover",
    ] {
        assert!(!recovers(line), "`{line}` does not run the recovery");
    }
}

#[test]
fn the_recovery_refusal_gives_the_person_the_command_and_names_the_bang() {
    let refusal = refusal(&Action::Write {
        target: RECOVER.to_owned(),
    })
    .expect("refused");
    assert_eq!(refusal.code, RECOVER_CODE);
    let text = refusal.to_string();
    assert!(
        text.contains("estigia recover <issue> --reason \"<why>\""),
        "{text}"
    );
    assert!(!text.contains("--epoch"), "{text}");
    assert!(text.contains("`!`"), "{text}");
    assert!(text.contains("maintainer"), "{text}");
}

#[test]
fn the_lift_refinement_never_reaches_the_recovery() {
    let action = classified("estigia recover 7 --epoch e --reason x --lift");
    assert!(
        matches!(&action, Action::Write { target } if target == RECOVER),
        "{action:?}"
    );
}

#[test]
fn a_recovery_on_any_line_is_denied_by_every_road_with_or_without_a_stand_down() {
    let root = tempfile::tempdir().expect("a temporary root");
    let plain = test_context(root.path());
    let mut standing = test_context(root.path());
    stood_down(&mut standing);
    let recovery = "estigia recover 7 --epoch e --authorized-by m --reason r";
    for context in [&plain, &standing] {
        for line in [
            recovery.to_owned(),
            format!("gh issue {}ate --title x; {recovery}", "cre"),
            format!("estigia setup --help; {recovery}"),
            format!("echo core.hookspath; {recovery}"),
            format!("git push origin b && {recovery}"),
            format!("gh pr merge 5 --merge && {recovery}"),
        ] {
            denied_by_every_road_with(
                context,
                root.path(),
                "Bash",
                &serde_json::json!({ "command": line }),
                RECOVER,
            );
        }
        denied_by_every_road_with(
            context,
            root.path(),
            "Bash",
            &serde_json::json!({ "command": ["bash", "-lc", recovery] }),
            RECOVER,
        );
    }
}

#[test]
fn a_stand_down_still_lifts_another_deny_beside_a_recovery_refusal() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = test_context(root.path());
    stood_down(&mut context);
    let filing = serde_json::json!({ "command": format!("gh issue {} --title x", "create") });
    let (action, how, command) = crate::harness::classify_observing("Bash", &filing, &[]);
    let observed = crate::outcome::Observed {
        command: Some(command),
        ..Default::default()
    };
    let mut run = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    let decision = crate::harness::gate_observing(&context, &mut run, &action, how, &observed);
    assert!(
        matches!(&decision, Decision::Allow(why) if why.contains("stood down")),
        "{decision:?}"
    );
}
