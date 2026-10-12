use super::*;

const TEMPLATE_SOURCE: &str = include_str!("../../../skill/assets/analyst-issue-template.md");

fn filled() -> String {
    let fenced = TEMPLATE_SOURCE
        .split("```markdown\n")
        .nth(1)
        .and_then(|rest| rest.split("\n```").next())
        .expect("the template carries a fenced markdown block");
    fenced.to_owned()
}

fn problem_of(body: &str) -> (String, String) {
    let Err(Failure::ConfigDefect(payload)) = check(body) else {
        panic!("the body was not refused as a template violation: {body:?}");
    };
    assert_eq!(payload["reason"], "body-violates-template");
    (
        payload["heading"].as_str().unwrap_or_default().to_owned(),
        payload["problem"].as_str().unwrap_or_default().to_owned(),
    )
}

#[test]
fn the_template_filled_as_written_is_accepted() {
    check(&filled()).expect("the template itself conforms");
}

/// The shipped template carries no control marker: the identity is recorded in
/// the local store, not in the body.
///
/// Mutation: restore `<!-- issue-flow: analysis run-id=<run-id> -->` to the
/// template's fenced block, and this reddens.
#[test]
fn the_shipped_template_carries_no_control_marker() {
    let body = filled().replace("<run-id>", "claude-abcd1234");
    let markers = crate::transport::markers::parse(&body).len();
    assert_eq!(
        markers, 0,
        "the shipped template carries {markers} markers: {body:?}"
    );
}

#[test]
fn the_required_headings_are_the_templates_own() {
    let from_the_file: Vec<String> = filled()
        .lines()
        .filter(|line| line.starts_with("## "))
        .map(str::to_owned)
        .collect();
    assert_eq!(from_the_file, TEMPLATE_HEADINGS);
    assert_eq!(TEMPLATE_HEADINGS[0], "## Description for dumb humans");
    assert!(TEMPLATE_HEADINGS.contains(&"## Out of scope"));
}

#[test]
fn a_body_without_the_fixed_first_heading_is_refused_naming_it() {
    let body = filled().replacen("## Description for dumb humans", "## Summary", 1);
    assert_eq!(problem_of(&body).0, "## Description for dumb humans");
}

#[test]
fn a_body_whose_first_heading_is_another_is_refused() {
    let body = format!("## Problem\n\nx\n\n{}", filled());
    let (heading, problem) = problem_of(&body);
    assert_eq!(heading, "## Description for dumb humans");
    assert_eq!(problem, "not the first heading");
}

#[test]
fn a_summary_without_the_note_alert_is_refused() {
    let body = filled().replacen("> [!NOTE]", "> A plain quote", 1);
    assert_eq!(problem_of(&body).1, "not followed by a `> [!NOTE]` alert");
}

#[test]
fn blank_lines_between_the_heading_and_the_alert_are_allowed() {
    let body = filled().replacen(
        "## Description for dumb humans\n\n",
        "## Description for dumb humans\n\n\n\n",
        1,
    );
    check(&body).expect("blank lines before the alert are allowed");
}

#[test]
fn every_required_heading_is_named_when_it_is_missing() {
    for required in &TEMPLATE_HEADINGS[1..] {
        let body = filled().replacen(&format!("{required}\n"), "plain text\n", 1);
        assert_eq!(
            problem_of(&body),
            ((*required).to_owned(), "missing".to_owned())
        );
    }
}

#[test]
fn headings_out_of_template_order_are_refused_naming_the_first_displaced() {
    let body = filled()
        .replacen("## Problem\n", "## @@\n", 1)
        .replacen("## Why it matters\n", "## Problem\n", 1)
        .replacen("## @@\n", "## Why it matters\n", 1);
    assert_eq!(
        problem_of(&body),
        (
            "## Why it matters".to_owned(),
            "out of template order".to_owned()
        )
    );
}

#[test]
fn extra_headings_beyond_the_required_ones_are_allowed() {
    let body = format!("{}\n\n## Appendix\n\nmore\n", filled());
    check(&body).expect("an extra heading is allowed");
    let between = filled().replacen("## Evidence", "## Notes\n\nx\n\n## Evidence", 1);
    check(&between).expect("an extra heading between required ones is allowed");
}

#[test]
fn a_heading_inside_a_code_fence_does_not_count() {
    let body = filled().replacen("## Evidence\n", "```\n## Evidence\n```\n", 1);
    assert_eq!(
        problem_of(&body),
        ("## Evidence".to_owned(), "missing".to_owned())
    );
}

#[test]
fn a_refused_create_files_nothing_and_asks_the_tracker_nothing() {
    let directory = tempfile::tempdir().expect("a directory");
    let body = directory.path().join("body.md");
    std::fs::write(&body, "## Problem\n\nNo summary.\n").expect("the body is written");
    let context = crate::transport::Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let failure = crate::transport::commands::create(
        &context,
        "an-identity",
        "A title",
        &body,
        "ready",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        "2026-01-01T00:00Z",
    )
    .expect_err("a body that ignores the template is refused");
    let Failure::ConfigDefect(payload) = failure else {
        panic!("expected the template refusal, which precedes every `gh` call: {failure:?}");
    };
    assert_eq!(payload["reason"], "body-violates-template");
}
