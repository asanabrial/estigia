use super::*;

fn git(directory: &std::path::Path, arguments: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

fn two_commits() -> (tempfile::TempDir, String, String) {
    let repo = repository_with_commit();
    let base = git(repo.path(), &["rev-parse", "HEAD"]);
    std::fs::write(repo.path().join("two.txt"), "two\n").expect("content");
    git(repo.path(), &["add", "two.txt"]);
    git(
        repo.path(),
        &[
            "-c",
            "user.email=estigia@example.invalid",
            "-c",
            "user.name=Estigia test",
            "commit",
            "--quiet",
            "-m",
            "head",
        ],
    );
    let head = git(repo.path(), &["rev-parse", "HEAD"]);
    (repo, base, head)
}

fn text_of(answer: &Result<Value, ToolFailure>) -> String {
    match answer {
        Ok(body) => body["content"][0]["text"]
            .as_str()
            .expect("a text answer")
            .to_owned(),
        Err(ToolFailure::Refused(refusal)) => format!("{} {refusal}", refusal.code),
        Err(ToolFailure::Malformed(message)) => message.clone(),
    }
}

#[test]
fn each_view_of_review_read_that_a_row_used_to_answer_answers_what_the_row_answers() {
    let (repo, base, head) = two_commits();
    let root = tempfile::tempdir().expect("a temporary root");
    let context = bare_context(&root, repo.path());

    let tier = json!({"base": base, "head": head});
    let by_row = run_tool("review_tier", &tier, Ok(&context));
    let by_view = run_tool(
        "review_read",
        &json!({"view": "tier", "base": base, "head": head}),
        Ok(&context),
    );
    assert!(by_row.is_ok(), "{}", text_of(&by_row));
    assert_eq!(text_of(&by_view), text_of(&by_row));
    assert!(
        text_of(&by_view).contains("\"lenses\""),
        "{}",
        text_of(&by_view)
    );

    let with_an_issue = run_tool(
        "review_read",
        &json!({"view": "tier", "issue": 5, "base": base, "head": head}),
        Ok(&context),
    );
    assert_eq!(text_of(&with_an_issue), text_of(&by_row));

    let narrowed = run_tool(
        "review_read",
        &json!({"view": "tier", "base": base, "head": head, "focus": "risk", "raise": "service-token"}),
        Ok(&context),
    );
    let by_the_narrowed_row = run_tool(
        "review_tier",
        &json!({"base": base, "head": head, "focus": "risk", "raise": "service-token"}),
        Ok(&context),
    );
    assert_eq!(text_of(&narrowed), text_of(&by_the_narrowed_row));

    let target_row = run_tool("target", &json!({"base": base}), Ok(&context));
    let target_view = run_tool(
        "review_read",
        &json!({"view": "target", "base": base}),
        Ok(&context),
    );
    assert!(target_row.is_ok(), "{}", text_of(&target_row));
    assert_eq!(text_of(&target_view), text_of(&target_row));

    let moved_row = run_tool(
        "target",
        &json!({"base": head, "recorded_base": base}),
        Ok(&context),
    );
    let moved_view = run_tool(
        "review_read",
        &json!({"view": "target", "base": head, "recorded_base": base}),
        Ok(&context),
    );
    assert_eq!(text_of(&moved_view), text_of(&moved_row));
}

#[test]
fn review_read_tier_and_target_are_read_where_the_old_names_were_under_assisted_linear() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = linear_context(&root, crate::config::Workflow::Assisted);

    let listed = tool_schemas(Ok(&context));
    let names: Vec<&str> = listed
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(names.contains(&"review_read"), "{names:?}");
    for hidden in [
        "collect_review_input",
        "deliver",
        "release_ci",
        "run_checks",
    ] {
        assert!(!names.contains(&hidden), "{hidden} is offered: {names:?}");
    }

    for view in ["tier", "target"] {
        let outcome = run_tool(
            "review_read",
            &json!({"view": view, "base": "main", "head": "HEAD"}),
            Ok(&context),
        );
        if let Err(failure) = outcome {
            assert!(
                refusal_code(&failure).is_none_or(|code| !TRANSPORT_REFUSALS.contains(&code)),
                "view {view} was refused for the tracking transport"
            );
        }
    }

    let refused = run_tool(
        "review_read",
        &json!({"view": "findings", "issue": 5}),
        Ok(&context),
    )
    .expect_err("a tracker read under a read-only tracker");
    assert_eq!(
        refusal_code(&refused),
        Some("read-only-tracker-admits-only-assisted")
    );
}

#[test]
fn a_view_still_needs_its_issue_and_takes_only_its_own_arguments() {
    for view in [
        "transaction",
        "findings",
        "verdict",
        "membership",
        "inspect",
        "spend",
    ] {
        let failure = super::super::read_view_step(&json!({"view": view}))
            .expect_err("a view that reads an issue's record was handed none");
        assert!(
            matches!(&failure, ToolFailure::Malformed(message) if message.contains("needs `issue`")),
            "{view}: {failure:?}"
        );
        assert!(
            super::super::read_view_step(&json!({"view": view, "issue": 5}))
                .expect("well formed")
                .is_none(),
            "{view} was forwarded"
        );
    }

    for (key, owner) in [("focus", "tier"), ("manifest", "target")] {
        let failure =
            super::super::read_view_step(&json!({"view": "findings", "issue": 5, key: "x"}))
                .expect_err("another view's argument was taken in silence");
        assert!(
            matches!(&failure, ToolFailure::Malformed(message)
                if message.contains(key) && message.contains(&format!("view `{owner}`"))),
            "{failure:?}"
        );
    }

    for arguments in [
        json!({"view": "tier", "base": "a", "head": "b", "epoch": "e"}),
        json!({"view": "target", "base": "a", "head": "b"}),
        json!({"view": "tier", "base": "a", "head": "b", "candidate": "f"}),
    ] {
        let failure = super::super::read_view_step(&arguments)
            .expect_err("a view took an argument its row does not");
        assert!(
            matches!(&failure, ToolFailure::Malformed(message) if message.contains("does not take")),
            "{arguments}: {failure:?}"
        );
    }

    let by_view = run_tool(
        "review_read",
        &json!({"view": "tier", "base": "a"}),
        Ok(&bare_context(
            &tempfile::tempdir().expect("a temporary root"),
            std::path::Path::new("."),
        )),
    );
    assert!(
        matches!(&by_view, Err(ToolFailure::Malformed(message))
            if message.starts_with("review_read view tier needs `head`")),
        "{by_view:?}"
    );

    let (row, view, rest) = super::super::read_view_step(
        &json!({"view": "target", "issue": 9, "base": "a", "compact": true}),
    )
    .expect("well formed")
    .expect("forwarded");
    assert_eq!((row, view), ("target", "target"));
    assert_eq!(rest, json!({"base": "a", "compact": true}));
}

#[test]
fn launching_forwards_the_named_slots_alone_and_nothing_launches_without_it() {
    let sent = json!({
        "issue": 5, "run_id": "claude-abcd1234", "launch": true,
        "names": ["review-risk", "review-readback"], "compact": true,
    });
    let (row, rest) = super::super::collect_step(&sent)
        .expect("well formed")
        .expect("forwarded");
    assert_eq!(row, "launch_review_input");
    assert_eq!(
        rest,
        json!({
            "issue": 5, "run_id": "claude-abcd1234",
            "names": ["review-risk", "review-readback"], "compact": true,
        })
    );
    let direct = json!({
        "issue": 5, "run_id": "claude-abcd1234", "names": ["review-risk", "review-readback"],
    });
    assert_eq!(
        super::super::flags_for(tools::find(row).expect("a row"), &rest).expect("flags"),
        super::super::flags_for(tools::find("launch_review_input").expect("a row"), &direct)
            .expect("flags")
    );

    let (row, rest) = super::super::collect_step(
        &json!({"issue": 5, "run_id": "claude-abcd1234", "launch": true, "name": "review-risk"}),
    )
    .expect("well formed")
    .expect("forwarded");
    assert_eq!(row, "launch_review_input");
    assert_eq!(
        rest,
        json!({"issue": 5, "run_id": "claude-abcd1234", "name": "review-risk"})
    );

    for abandoned_or_not in [
        json!({"issue": 5, "run_id": "claude-abcd1234", "name": "review-risk"}),
        json!({"issue": 5, "run_id": "claude-abcd1234", "names": ["review-risk"], "launch": false}),
        json!({"issue": 5, "run_id": "claude-abcd1234", "name": "review-risk", "wait_seconds": 30, "record_verdict": true}),
    ] {
        assert!(
            super::super::collect_step(&abandoned_or_not)
                .expect("well formed")
                .is_none(),
            "{abandoned_or_not} was carried out by a row"
        );
    }

    for refused in [
        json!({"issue": 5, "run_id": "claude-abcd1234", "launch": true}),
        json!({"issue": 5, "run_id": "claude-abcd1234", "launch": true, "name": "x", "wait_seconds": 5}),
        json!({"issue": 5, "run_id": "claude-abcd1234", "launch": true, "name": "x", "record_verdict": true}),
        json!({"issue": 5, "run_id": "claude-abcd1234", "launch": true, "checks": true, "name": "x"}),
    ] {
        assert!(
            matches!(
                super::super::collect_step(&refused),
                Err(ToolFailure::Malformed(_))
            ),
            "{refused} was carried out"
        );
    }
}

#[test]
fn checks_forward_with_their_wait_and_nothing_else() {
    let (row, rest) = super::super::collect_step(
        &json!({"issue": 5, "run_id": "claude-abcd1234", "checks": true, "wait_seconds": 40}),
    )
    .expect("well formed")
    .expect("forwarded");
    assert_eq!(row, "run_checks");
    assert_eq!(
        rest,
        json!({"issue": 5, "run_id": "claude-abcd1234", "wait_seconds": 40})
    );
    let direct = json!({"issue": 5, "run_id": "claude-abcd1234", "wait_seconds": 40});
    assert_eq!(
        super::super::flags_for(tools::find(row).expect("a row"), &rest).expect("flags"),
        super::super::flags_for(tools::find("run_checks").expect("a row"), &direct).expect("flags")
    );

    for refused in [
        json!({"issue": 5, "run_id": "claude-abcd1234", "checks": true, "name": "review-risk"}),
        json!({"issue": 5, "run_id": "claude-abcd1234", "checks": true, "record_verdict": true}),
    ] {
        assert!(
            matches!(
                super::super::collect_step(&refused),
                Err(ToolFailure::Malformed(_))
            ),
            "{refused} was carried out"
        );
    }
}

#[test]
fn a_status_that_offers_a_slot_to_start_says_to_collect_with_launch() {
    let offered = json!([
        {"name": "review-risk", "capture": "launch_review_input"},
        {"name": "review-readback", "capture": "collect_review_input", "job": "j1"},
        {"name": "review-reliability", "capture": null},
    ]);
    let status = json!({"ok": true, "next": {"kind": "collect", "inputs": offered}});
    let expected = json!([
        {"name": "review-risk", "capture": "collect_review_input", "launch": true},
        {"name": "review-readback", "capture": "collect_review_input", "job": "j1"},
        {"name": "review-reliability", "capture": null},
    ]);

    let mut direct = status.clone();
    super::super::capture_through_collect("review_status", &mut direct);
    assert_eq!(direct["next"]["inputs"], expected);

    for tool in ["publish_review", "republish_review", "collect_review_input"] {
        let mut embedded = json!({"ok": true, "review_status": status.clone()});
        super::super::capture_through_collect(tool, &mut embedded);
        assert_eq!(
            embedded["review_status"]["next"]["inputs"], expected,
            "{tool}"
        );
    }

    let mut unrelated = json!({"ok": true, "next": {"inputs": offered}});
    super::super::capture_through_collect("claim", &mut unrelated);
    assert_eq!(unrelated["next"]["inputs"], offered);

    let mut none = json!({"ok": true});
    super::super::capture_through_collect("review_status", &mut none);
    assert_eq!(none, json!({"ok": true}));
}

#[test]
fn a_retired_name_of_this_surface_answers_the_call_that_replaces_it() {
    for (name, replacement) in [
        ("review_tier", ["review_read", "view: tier"]),
        ("target", ["review_read", "view: target"]),
        (
            "launch_review_input",
            ["collect_review_input", "launch: true"],
        ),
        ("run_checks", ["collect_review_input", "checks: true"]),
        ("record_computed_verdict", ["deliver", "until: verdict"]),
        ("release_ci", ["deliver", "until: ci"]),
    ] {
        assert!(
            !TOOLS.iter().any(|tool| tool.name == name),
            "{name} is still advertised"
        );
        assert!(tools::find(name).is_some(), "{name} is no row any longer");
        let response = result(json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": name, "arguments": {}}
        }));
        assert_eq!(response["isError"], true, "{name}");
        let text = response["content"][0]["text"].as_str().expect("a text");
        assert!(text.contains("tool-retired"), "{name}: {text}");
        for part in replacement {
            assert!(text.contains(part), "{name} did not say `{part}`: {text}");
        }
    }
}

#[test]
fn a_shipped_line_that_tells_the_agent_to_call_a_retired_name_is_seen() {
    let hit = retired_calls("Call `launch_review_input` for the name once it is offered.");
    assert_eq!(hit.len(), 1, "{hit:?}");
    assert_eq!(hit[0].0, "launch_review_input");
    assert!(hit[0].1.contains("launch: true"));
    assert!(retired_calls("| `x` | Then call `release_ci` with the receipt. |").len() == 1);
    assert!(retired_calls("`release_ci` replays the live claim.").is_empty());
    assert!(retired_calls("Call `collect_review_input` with `launch`.").is_empty());
}

#[test]
fn the_arguments_that_choose_a_row_are_declared_and_carry_no_transport_flag() {
    assert_eq!(TOOLS.len(), 27);
    let columns = |name: &str| {
        tools::find(name)
            .expect("a row")
            .arguments
            .iter()
            .map(|argument| argument.name)
            .collect::<Vec<_>>()
    };
    for carried in ["launch", "checks"] {
        assert!(columns("collect_review_input").contains(&carried));
        assert!(tools::is_pointer_only("collect_review_input", carried));
    }
    for carried in ["until", "never_ran_evidence_file"] {
        assert!(columns("deliver").contains(&carried));
    }
    let issue = tools::find("review_read")
        .expect("a row")
        .arguments
        .iter()
        .find(|argument| argument.name == "issue")
        .expect("review_read takes an issue");
    assert!(!issue.required);
}
