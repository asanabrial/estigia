use super::*;

const RUN: &str = "claude-abcd1234";

fn published(
    context: &Context,
    branch: &str,
    expected_head: Option<&str>,
    wait_seconds: u64,
) -> Result<serde_json::Value, Failure> {
    published_with_sleep(
        context,
        &Published {
            pr: 7,
            branch,
            stages: &stages(TEAM),
            expected_head,
            wait_seconds,
            now_secs: 0,
        },
        &mut |_| {},
    )
}

fn reading_script(pr: &Pr) -> String {
    crate::test_env::answers(&[
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"owner":{"login":"example"},"name":"repo"}"#,
        }),
        serde_json::json!({ "matches": "api graphql", "stdout": graphql(pr) }),
    ])
}

#[test]
fn the_receipts_pull_request_is_read_by_number_with_no_position() {
    let (root, context) = rig();
    let bin = crate::test_env::scripted_gh();
    let log = root.path().join("log");
    let pr = Pr::open()
        .check("cubic", "COMPLETED", Some("FAILURE"))
        .thread("human", false);
    let answer = crate::test_env::with_logging_gh(bin.path(), &reading_script(&pr), &log, || {
        published(&context, "issue-12", Some("abc"), 0)
    })
    .unwrap();
    assert_eq!(answer["pr"], 7);
    assert_eq!(answer["head"], "abc");
    assert_eq!(answer["base"], "base");
    assert_eq!(answer["ci"], "failed");
    assert_eq!(answer["failing_checks"], serde_json::json!(["cubic"]));
    assert_eq!(answer["open_threads"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer["wait_timed_out"], false);
    for absent in ["position", "next"] {
        assert!(answer.get(absent).is_none(), "{absent} in {answer}");
    }
    assert!(
        !logged(&log).iter().any(|call| call.starts_with("pr list")),
        "{:?}",
        logged(&log)
    );
}

#[test]
fn a_pull_request_on_another_branch_or_head_is_not_read() {
    for (branch, head) in [("elsewhere", None), ("issue-12", Some("other"))] {
        let (root, context) = rig();
        let bin = crate::test_env::scripted_gh();
        let failure = crate::test_env::with_logging_gh(
            bin.path(),
            &reading_script(&Pr::open()),
            &root.path().join("log"),
            || published(&context, branch, head, 0),
        )
        .expect_err("a reading of another branch or head");
        assert_eq!(failure.envelope()["reason"], "assisted-review-head-moved");
    }
}

#[test]
fn waiting_on_a_published_reading_still_needs_a_head_and_a_bound() {
    let (_root, context) = rig();
    for (head, wait) in [(None, 5), (Some("abc"), 301)] {
        let failure = published(&context, "issue-12", head, wait).expect_err("an unbounded wait");
        assert_eq!(failure.envelope()["reason"], "review-wait-invalid");
    }
}

fn held(rig: &ThreadRig, pr: u64) -> thread::Held<'_> {
    thread::Held {
        issue: 12,
        run_id: RUN,
        expect_state: "review",
        pr,
        head: &rig.fix,
        now: "2026-11-21T10:00Z",
    }
}

fn held_script(
    rig: &ThreadRig,
    holder: serde_json::Value,
    readings: &[serde_json::Value],
) -> String {
    let mut entries: Vec<serde_json::Value> =
        serde_json::from_str(&rig.script(readings)).expect("a script");
    entries.insert(0, holder);
    crate::test_env::answers(&entries)
}

#[test]
fn a_held_claim_renews_before_the_reply_and_the_fix_resolves_the_thread() {
    let rig = ThreadRig::new();
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("log");
    let script = held_script(
        &rig,
        crate::test_env::live_holder_issue(RUN, 12, "review"),
        &[
            rig.pr(false, false),
            rig.pr(true, false),
            rig.pr(true, false),
            rig.pr(true, true),
        ],
    );
    let result = crate::test_env::with_counting_logging_gh(
        bin.path(),
        &script,
        &rig.root.path().join("counts"),
        &log,
        || {
            thread::respond_held(
                &rig.context,
                &rig.response(Some(&rig.fix), true),
                &held(&rig, 7),
            )
        },
    )
    .unwrap();
    assert_eq!(result["resolution_confirmed"], true);
    let calls = logged(&log);
    let at = |needle: &str| calls.iter().position(|call| call.contains(needle));
    assert!(at("issue view") < at("pr list"), "{calls:?}");
    assert!(
        at("issue view") < at("addPullRequestReviewThreadReply"),
        "{calls:?}"
    );
    assert!(at("resolveReviewThread").is_some(), "{calls:?}");
}

#[test]
fn a_held_claim_resolves_a_thread_with_a_reply_alone() {
    let rig = ThreadRig::new();
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("log");
    let script = held_script(
        &rig,
        crate::test_env::live_holder_issue(RUN, 12, "review"),
        &[
            rig.pr(false, false),
            rig.pr(true, false),
            rig.pr(true, false),
            rig.pr(true, true),
        ],
    );
    let result = crate::test_env::with_counting_logging_gh(
        bin.path(),
        &script,
        &rig.root.path().join("counts"),
        &log,
        || thread::respond_held(&rig.context, &rig.response(None, true), &held(&rig, 7)),
    )
    .unwrap();
    assert_eq!(result["resolution_confirmed"], true);
    assert!(result["fix_commit"].is_null());
    let calls = logged(&log);
    let at = |needle: &str| calls.iter().position(|call| call.contains(needle));
    assert!(
        at("issue view") < at("addPullRequestReviewThreadReply"),
        "{calls:?}"
    );
    assert!(at("resolveReviewThread").is_some(), "{calls:?}");
}

#[test]
fn a_claim_that_is_not_held_sends_nothing_and_lists_nothing() {
    let rig = ThreadRig::new();
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("log");
    let script = held_script(
        &rig,
        crate::test_env::not_current_live_holder_issue(12),
        &[rig.pr(false, false)],
    );
    let failure = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        let lapsed = thread::Held {
            expect_state: "in-progress",
            ..held(&rig, 7)
        };
        thread::respond_held(&rig.context, &rig.response(Some(&rig.fix), true), &lapsed)
    })
    .unwrap_err();
    assert_eq!(failure.envelope()["reason"], "not-current-live-holder");
    assert!(
        !logged(&log)
            .iter()
            .any(|call| call.starts_with("pr list") || call.contains("query=mutation")),
        "{:?}",
        logged(&log)
    );
}

#[test]
fn the_open_pull_request_must_be_the_one_the_claim_published() {
    let rig = ThreadRig::new();
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("log");
    let script = held_script(
        &rig,
        crate::test_env::live_holder_issue(RUN, 12, "review"),
        &[rig.pr(false, false)],
    );
    let failure = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        thread::respond_held(
            &rig.context,
            &rig.response(Some(&rig.fix), true),
            &held(&rig, 8),
        )
    })
    .unwrap_err();
    assert_eq!(failure.envelope()["reason"], "review-thread-pr-unconfined");
    assert!(
        !logged(&log)
            .iter()
            .any(|call| call.contains("query=mutation")),
        "{:?}",
        logged(&log)
    );
}

#[test]
fn a_head_the_claim_did_not_publish_is_refused_before_any_call() {
    let rig = ThreadRig::new();
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("log");
    let failure = crate::test_env::with_logging_gh(bin.path(), "[]", &log, || {
        let published = thread::Held {
            head: &rig.base,
            ..held(&rig, 7)
        };
        thread::respond_held(
            &rig.context,
            &rig.response(Some(&rig.fix), true),
            &published,
        )
    })
    .unwrap_err();
    assert_eq!(
        failure.envelope()["reason"],
        "review-thread-head-not-published"
    );
    assert_eq!(logged(&log), Vec::<String>::new());
}
