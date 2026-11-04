use super::*;
use crate::config::Party;

fn reviewing_script(head: &str, drafts_first: usize) -> String {
    let mut entries = vec![
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"defaultBranchRef":{"name":"main"}}"#,
        }),
        serde_json::json!({ "matches": "pr list", "stdout": "[]" }),
        serde_json::json!({ "matches": "pr create", "stdout": PR_URL }),
        serde_json::json!({ "matches": "pr ready", "stdout": "" }),
    ];
    for nth in 1..=drafts_first {
        entries.push(serde_json::json!({
            "matches": "pr view", "nth": nth, "stdout": view(head, true).to_string(),
        }));
    }
    entries.push(serde_json::json!({
        "matches": "pr view", "stdout": view(head, false).to_string(),
    }));
    crate::test_env::answers(&entries)
}

fn delivered(
    rig: &Rig,
    answers: &str,
    ready_by: Party,
    self_review: bool,
    corrections: Corrections,
) -> (Result<serde_json::Value, Failure>, Vec<String>) {
    let body = rig.body("Words.");
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("gh.log");
    let outcome = crate::test_env::with_counting_logging_gh(
        bin.path(),
        answers,
        &rig.root.path().join("counts"),
        &log,
        || rig.call_delivering("A title", &body, corrections, ready_by, self_review),
    );
    (outcome, logged(rig, &log))
}

fn names(next: &serde_json::Value, words: &[&str]) {
    let next = next.as_str().expect("a next step");
    for word in words {
        assert!(next.contains(word), "{word} is not in {next}");
    }
}

#[test]
fn a_draft_stays_a_draft_where_a_person_marks_it_or_the_review_is_still_to_come() {
    for (ready_by, self_review) in [
        (Party::Agent, true),
        (Party::Person, true),
        (Party::Person, false),
    ] {
        let rig = rig();
        let answers = reviewing_script(&rig.head, 9);
        let (outcome, calls) =
            delivered(&rig, &answers, ready_by, self_review, Corrections::Refuse);
        let answer = outcome.expect("the draft is published");
        assert_eq!(answer["draft"], true, "{answer}");
        assert!(answer.get("marked_ready").is_none(), "{answer}");
        assert!(
            !calls.iter().any(|call| call.starts_with("pr ready")),
            "{calls:?}"
        );
        assert!(
            calls
                .iter()
                .filter(|call| call.starts_with("pr create"))
                .all(|call| call.contains("--draft")),
            "{calls:?}"
        );
    }
}

#[test]
fn a_draft_waiting_on_the_self_review_names_the_review_and_ready_pr_with_the_head() {
    let rig = rig();
    let answers = reviewing_script(&rig.head, 9);
    let (outcome, _) = delivered(&rig, &answers, Party::Agent, true, Corrections::Refuse);
    let answer = outcome.expect("the draft is published");
    names(&answer["next"], &["ready_pr", "reviewed_head", &rig.head]);
}

#[test]
fn without_a_self_review_an_agent_publishes_a_pull_request_that_reads_ready_at_the_head() {
    let rig = rig();
    let answers = reviewing_script(&rig.head, 1);
    let (outcome, calls) = delivered(&rig, &answers, Party::Agent, false, Corrections::Refuse);
    let answer = outcome.expect("published and marked");
    assert_eq!(answer["draft"], false, "{answer}");
    assert_eq!(answer["marked_ready"], true, "{answer}");
    assert_eq!(answer["head"], rig.head.as_str());
    assert_eq!(answer["tracker_written"], false);
    names(&answer["next"], &["review_stages", &rig.head]);
    let created: Vec<&String> = calls
        .iter()
        .filter(|call| call.starts_with("pr create"))
        .collect();
    assert!(
        created.len() == 1 && created[0].contains("--draft"),
        "{calls:?}"
    );
    assert_eq!(
        calls.iter().filter(|call| *call == "pr ready 7").count(),
        1,
        "{calls:?}"
    );
}

#[test]
fn a_reused_draft_is_marked_ready_too_without_a_self_review() {
    let rig = rig();
    let existing = serde_json::json!([view(&rig.head, true)]);
    let answers = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"defaultBranchRef":{"name":"main"}}"#,
        }),
        serde_json::json!({ "matches": "pr list", "stdout": existing.to_string() }),
        serde_json::json!({ "matches": "pr ready", "stdout": "" }),
        serde_json::json!({
            "matches": "pr view", "nth": 1, "stdout": view(&rig.head, true).to_string(),
        }),
        serde_json::json!({ "matches": "pr view", "stdout": view(&rig.head, false).to_string() }),
    ]);
    let (outcome, calls) = delivered(&rig, &answers, Party::Agent, false, Corrections::Refuse);
    let answer = outcome.expect("the reused draft is marked");
    assert_eq!(answer["created"], false);
    assert_eq!(answer["draft"], false);
    assert_eq!(answer["marked_ready"], true);
    assert!(calls.iter().any(|call| call == "pr ready 7"), "{calls:?}");
}

#[test]
fn a_pull_request_already_ready_gets_a_correction_and_the_reading_is_named() {
    let rig = rig();
    let existing = serde_json::json!([view(&rig.head, false)]);
    let answers = script(&existing, &view(&rig.head, false));
    let (outcome, calls) = delivered(&rig, &answers, Party::Agent, false, Corrections::PushReady);
    let answer = outcome.expect("the correction is pushed");
    assert_eq!(answer["draft"], false);
    assert!(answer.get("marked_ready").is_none(), "{answer}");
    assert!(
        !calls.iter().any(|call| call.starts_with("pr ready")),
        "{calls:?}"
    );
    names(&answer["next"], &["review_stages", &rig.head]);
}

#[test]
fn a_mark_that_fails_after_the_push_reports_the_published_head_and_asks_for_no_second_publication()
{
    for failing in ["errors", "never-reads-ready"] {
        let rig = rig();
        let ready = if failing == "errors" {
            serde_json::json!({ "matches": "pr ready", "stdout": "", "status": 1 })
        } else {
            serde_json::json!({ "matches": "pr ready", "stdout": "" })
        };
        let answers = crate::test_env::answers(&[
            serde_json::json!({
                "matches": "repo view",
                "stdout": r#"{"defaultBranchRef":{"name":"main"}}"#,
            }),
            serde_json::json!({ "matches": "pr list", "stdout": "[]" }),
            serde_json::json!({ "matches": "pr create", "stdout": PR_URL }),
            ready,
            serde_json::json!({ "matches": "pr view", "stdout": view(&rig.head, true).to_string() }),
        ]);
        let (outcome, _) = delivered(&rig, &answers, Party::Agent, false, Corrections::Refuse);
        let failure = outcome.expect_err("a mark nobody read back was reported");
        let envelope = failure.envelope();
        assert_eq!(
            envelope["reason"], "draft-pr-ready-not-read-back",
            "{failing}: {envelope}"
        );
        assert_eq!(envelope["world"], "committed");
        assert_eq!(envelope["published_head"], rig.head.as_str());
        assert_eq!(envelope["pr"], 7);
        let action = envelope["action"].as_str().expect("an action");
        assert!(
            action.contains("ready_pr") && action.contains("do not publish again"),
            "{action}"
        );
        assert!(rig.on_origin());
    }
}

fn ready_with(
    rig: &Rig,
    self_review: bool,
    reviewed: Option<&str>,
) -> (Result<serde_json::Value, Failure>, Vec<String>) {
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("gh.log");
    let answers = ready_script(
        &serde_json::json!([view(&rig.head, true)]),
        &view(&rig.head, false),
    );
    let outcome = crate::test_env::with_logging_gh(bin.path(), &answers, &log, || {
        rig.ready_reviewing(Party::Agent, self_review, reviewed)
    });
    (outcome, logged(rig, &log))
}

#[test]
fn ready_under_a_self_review_refuses_a_head_that_was_not_the_one_reviewed() {
    let rig = rig();
    let reviewed = "0".repeat(40);
    let (outcome, calls) = ready_with(&rig, true, Some(&reviewed));
    let envelope = outcome
        .expect_err("a head other than the published one was opened")
        .envelope();
    assert_eq!(
        envelope["reason"], "ready-pr-review-not-this-head",
        "{envelope}"
    );
    assert_eq!(envelope["published_head"], rig.head.as_str());
    assert_eq!(envelope["reviewed_head"], reviewed.as_str());
    assert!(
        !calls.iter().any(|call| call.starts_with("pr ready")),
        "{calls:?}"
    );
}

#[test]
fn ready_under_a_self_review_names_the_argument_it_needs() {
    let rig = rig();
    let (outcome, calls) = ready_with(&rig, true, None);
    let failure = outcome.expect_err("a ready mark with no review stated");
    assert_eq!(reason(&failure), "ready-pr-reviewed-head-required");
    assert!(
        failure.envelope()["action"]
            .as_str()
            .unwrap_or_default()
            .contains("reviewed_head"),
        "{}",
        failure.envelope()
    );
    assert!(calls.is_empty(), "{calls:?}");
}

#[test]
fn ready_under_a_self_review_opens_the_reviewed_head_and_names_the_reading() {
    let rig = rig();
    let reviewed = rig.head.clone();
    let (outcome, _) = ready_with(&rig, true, Some(&reviewed));
    let answer = outcome.expect("the reviewed head opens");
    assert_eq!(answer["draft"], false);
    names(&answer["next"], &["review_stages", &rig.head]);
}

#[test]
fn without_a_self_review_ready_needs_no_reviewed_head() {
    let rig = rig();
    let (outcome, _) = ready_with(&rig, false, None);
    outcome.expect("nothing was reviewed, so nothing is stated");
}

#[test]
fn a_new_head_past_the_row_is_refused_before_anything_is_pushed() {
    let rig = rig();
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("gh.log");
    let answers = script(&serde_json::json!([]), &view(&rig.head, true));
    let outcome = crate::test_env::with_logging_gh(bin.path(), &answers, &log, || {
        rig.call_after(&"0".repeat(40), 1)
    });
    let envelope = outcome.expect_err("a correction past the row").envelope();
    assert_eq!(
        envelope["reason"], "draft-pr-corrections-spent",
        "{envelope}"
    );
    assert_eq!(envelope["corrections_allowed"], 1, "{envelope}");
    assert!(!rig.on_origin());
    assert!(
        !logged(&rig, &log)
            .iter()
            .any(|call| call.starts_with("pr create") || call.starts_with("pr list")),
        "the refusal came after a host call"
    );
}

#[test]
fn the_row_stops_only_a_new_head_and_only_once_it_is_spent() {
    for (same_head, spent) in [(true, 1), (false, 0)] {
        let rig = rig();
        let bin = crate::test_env::scripted_gh();
        let answers = script(&serde_json::json!([]), &view(&rig.head, true));
        let published = if same_head {
            rig.head.clone()
        } else {
            "0".repeat(40)
        };
        crate::test_env::with_scripted_gh(bin.path(), &answers, || {
            rig.call_after(&published, spent)
        })
        .unwrap_or_else(|failure| panic!("{same_head}/{spent}: {}", failure.detail()));
        assert!(rig.on_origin(), "{same_head}/{spent}");
    }
}
