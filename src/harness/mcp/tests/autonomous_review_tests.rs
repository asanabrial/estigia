use super::*;

const RUN: &str = "claude-abcd1234";

struct Held {
    root: tempfile::TempDir,
    _repo: tempfile::TempDir,
    worktree: std::path::PathBuf,
    context: GateContext,
    base: String,
    head: String,
    bin: &'static tempfile::TempDir,
}

fn held() -> Held {
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = repository_with_commit();
    let worktree = root.path().join("tree");
    register_adopted_worktree(repo.path(), &worktree, "issue-12", RUN, 12);
    let base = git_at(&worktree, &["rev-parse", "HEAD"]);
    git_at(
        &worktree,
        &["commit", "--allow-empty", "--quiet", "-m", "fix"],
    );
    let head = git_at(&worktree, &["rev-parse", "HEAD"]);
    let context = bare_context(&root, &worktree);
    let mut run = session::Run::new(RUN.to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.worktree = Some(worktree.clone());
    run.repo_dir = Some(repo.path().to_path_buf());
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 7,
        head: head.clone(),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    session::store(&context.state_root, &run).expect("the pointer writes");
    Held {
        root,
        _repo: repo,
        worktree,
        context,
        base,
        head,
        bin: crate::test_env::scripted_gh(),
    }
}

impl Held {
    fn snapshot(&self, replied: bool) -> Value {
        let mut comments = vec![json!({
            "id": "c-1", "viewerDidAuthor": false, "author": {"login": "reviewer"},
            "body": "Rename this.", "path": "src/lib.rs", "line": 3,
            "originalCommit": {"oid": self.base},
        })];
        if replied {
            comments.push(json!({"id": "reply-1", "viewerDidAuthor": true, "body": "Words."}));
        }
        json!({
            "number": 7, "state": "OPEN", "isDraft": false, "headRefName": "issue-12",
            "headRefOid": self.head, "baseRefOid": "b".repeat(40),
            "reviewRequests": {"pageInfo": {"hasNextPage": false}, "nodes": []},
            "latestReviews": {"pageInfo": {"hasNextPage": false}, "nodes": []},
            "reviewThreads": {"pageInfo": {"hasNextPage": false}, "nodes": [{
                "id": "thread-1", "isResolved": false,
                "comments": {"pageInfo": {"hasNextPage": false}, "nodes": comments},
            }]},
            "commits": {"nodes": [{"commit": {"oid": self.head, "statusCheckRollup": {
                "contexts": {"pageInfo": {"hasNextPage": false}, "nodes": [
                    {"__typename": "CheckRun", "name": "ci", "status": "COMPLETED",
                     "conclusion": "SUCCESS"},
                ]},
            }}}]},
        })
    }

    fn reading(&self, replied: bool) -> String {
        json!({"data": {"repository": {"pullRequest": self.snapshot(replied)}}}).to_string()
    }

    fn reply(&self, head: &str) -> Value {
        let body = self.root.path().join("reply.md");
        std::fs::write(&body, "Words.").expect("the reply file");
        json!({
            "run_id": RUN, "thread_id": "thread-1", "expected_head": head,
            "body_file": body.to_str().unwrap(),
        })
    }

    fn silent<T>(&self, body: impl FnOnce() -> T) -> (T, String) {
        let log = self.root.path().join("silent.log");
        let out = crate::test_env::with_logging_gh(self.bin.path(), "[]", &log, body);
        (out, std::fs::read_to_string(&log).unwrap_or_default())
    }

    fn pointer(&self) -> session::Run {
        session::load(&self.context.state_root, RUN)
    }
}

#[test]
fn review_stages_under_an_autonomous_claim_answers_threads_and_no_position() {
    let held = held();
    let script = crate::test_env::answers(&[
        json!({"matches": "repo view", "stdout": r#"{"owner":{"login":"o"},"name":"r"}"#}),
        json!({"matches": "api graphql", "stdout": held.reading(false)}),
    ]);
    let answer = crate::test_env::with_scripted_gh(held.bin.path(), &script, || {
        run_tool_body("review_stages", &json!({"run_id": RUN}), Ok(&held.context))
    })
    .unwrap_or_else(|failure| panic!("an autonomous reading was refused: {failure}"));
    assert_eq!(answer["pr"], 7, "{answer}");
    assert_eq!(answer["head"], held.head.as_str());
    assert_eq!(answer["ci"], "passed");
    assert_eq!(answer["open_threads"].as_array().map(Vec::len), Some(1));
    assert_eq!(answer["open_threads"][0]["id"], "thread-1");
    for absent in ["position", "next"] {
        assert!(answer.get(absent).is_none(), "{absent} in {answer}");
    }
}

#[test]
fn review_stages_under_an_autonomous_claim_refuses_to_request_a_review() {
    let held = held();
    let arguments = json!({"run_id": RUN, "request_review": true});
    let (failure, calls) = held.silent(|| {
        run_tool("review_stages", &arguments, Ok(&held.context)).expect_err("a request")
    });
    assert_eq!(refusal_code(&failure), Some("workflow-not-assisted"));
    assert_eq!(calls, "");
}

#[test]
fn review_thread_names_only_the_head_the_receipt_published() {
    let held = held();
    let other = "d".repeat(40);
    let (failure, calls) = held.silent(|| {
        run_tool("review_thread", &held.reply(&other), Ok(&held.context)).expect_err("a head")
    });
    assert_eq!(
        refusal_code(&failure),
        Some("review-thread-head-not-published"),
        "{failure}"
    );
    assert_eq!(calls, "", "a head the claim never published reached GitHub");
}

#[test]
fn both_tools_need_a_complete_receipt_under_an_autonomous_claim() {
    let held = held();
    session::update(&held.context.state_root, RUN, |run| {
        run.review_transaction = None;
    });
    for (tool, arguments) in [
        ("review_stages", json!({"run_id": RUN})),
        ("review_thread", held.reply(&held.head)),
    ] {
        let (failure, calls) =
            held.silent(|| run_tool(tool, &arguments, Ok(&held.context)).expect_err("no receipt"));
        assert_eq!(
            refusal_code(&failure),
            Some("published-transaction-missing"),
            "{tool}: {failure}"
        );
        assert_eq!(calls, "", "{tool}");
    }
}

#[test]
fn a_marker_for_another_run_or_a_worktree_on_another_branch_refuses_both_tools() {
    for spoil in ["another run", "another branch"] {
        let held = held();
        match spoil {
            "another run" => {
                let marker = crate::transport::branch::ownership_path(&held.worktree).unwrap();
                std::fs::write(
                    marker,
                    json!({"run_id": "claude-ffff0000", "issue": 12, "branch": "issue-12"})
                        .to_string(),
                )
                .unwrap();
            }
            _ => {
                git_at(&held.worktree, &["checkout", "--quiet", "-b", "elsewhere"]);
            }
        }
        for (tool, arguments) in [
            ("review_stages", json!({"run_id": RUN})),
            ("review_thread", held.reply(&held.head)),
        ] {
            let (failure, calls) =
                held.silent(|| run_tool(tool, &arguments, Ok(&held.context)).expect_err(spoil));
            assert_eq!(
                refusal_code(&failure),
                Some("assisted-claim-stale"),
                "{spoil}, {tool}: {failure}"
            );
            assert_eq!(calls, "", "{spoil}, {tool}");
        }
    }
}

#[test]
fn another_open_pull_request_on_the_branch_is_not_the_receipts() {
    for (listed, expected) in [
        (r#"[{"number":8}]"#, "review-thread-pr-unconfined"),
        // Two of this repository's pull requests are ambiguous, and naming
        // both is the refusal publication and this path share.
        (r#"[{"number":7},{"number":8}]"#, "ambiguous-open-prs"),
        ("[]", "review-thread-pr-unconfined"),
    ] {
        let held = held();
        let script = crate::test_env::answers(&[
            crate::test_env::live_holder_issue(RUN, 12, "review"),
            json!({"matches": "pr list", "stdout": listed}),
            json!({"matches": "repo view", "stdout": r#"{"owner":{"login":"o"},"name":"r"}"#}),
            json!({"matches": "query=mutation", "stdout": r#"{"data":{}}"#}),
            json!({"matches": "query=query", "stdout": held.reading(false)}),
        ]);
        let log = held.root.path().join("log");
        let failure = crate::test_env::with_logging_gh(held.bin.path(), &script, &log, || {
            run_tool("review_thread", &held.reply(&held.head), Ok(&held.context))
        })
        .expect_err(listed);
        assert_eq!(
            refusal_code(&failure),
            Some(expected),
            "{listed}: {failure}"
        );
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(!calls.contains("query=mutation"), "{listed}: {calls}");
    }
}

#[test]
fn a_reply_under_an_autonomous_claim_renews_the_claim_and_reads_the_reply_back() {
    let held = held();
    let script = crate::test_env::answers(&[
        crate::test_env::live_holder_issue(RUN, 12, "review"),
        json!({"matches": "pr list", "stdout": r#"[{"number":7}]"#}),
        json!({"matches": "repo view", "stdout": r#"{"owner":{"login":"o"},"name":"r"}"#}),
        json!({"matches": "query=mutation", "stdout": r#"{"data":{}}"#}),
        json!({"matches": "query=query", "nth": 1, "stdout": held.reading(false)}),
        json!({"matches": "query=query", "stdout": held.reading(true)}),
    ]);
    let log = held.root.path().join("log");
    let answer = crate::test_env::with_counting_logging_gh(
        held.bin.path(),
        &script,
        &held.root.path().join("counts"),
        &log,
        || run_tool_body("review_thread", &held.reply(&held.head), Ok(&held.context)),
    )
    .unwrap_or_else(|failure| panic!("the reply was refused: {failure}"));
    assert_eq!(answer["reply_id"], "reply-1", "{answer}");
    assert_eq!(answer["pr"], 7);
    let calls = std::fs::read_to_string(&log).unwrap();
    let renewal = calls.find("issue view").expect("the claim was renewed");
    let mutation = calls.find("query=mutation").expect("the reply was sent");
    assert!(renewal < mutation, "{calls}");
    assert_eq!(held.pointer().review_transaction.unwrap().pr, 7);
}

#[test]
fn a_claim_someone_else_holds_stops_a_reply_before_any_mutation() {
    let held = held();
    let script = crate::test_env::answers(&[
        crate::test_env::not_current_live_holder_issue(12),
        json!({"matches": "pr list", "stdout": r#"[{"number":7}]"#}),
    ]);
    let log = held.root.path().join("log");
    let failure = crate::test_env::with_logging_gh(held.bin.path(), &script, &log, || {
        run_tool("review_thread", &held.reply(&held.head), Ok(&held.context))
    })
    .expect_err("a lapsed claim replied");
    assert!(
        failure.to_string().contains("not-current-live-holder")
            || failure.to_string().contains("unexpected-state"),
        "{failure}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("query=mutation") && !calls.contains("pr list"),
        "{calls}"
    );
}
