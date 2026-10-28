use super::*;

/// An issue's identity and run id are recorded in the store, so a call with no
/// epoch composes no marker at all.
#[test]
fn a_call_with_no_epoch_composes_no_marker_whatever_identity_it_is_filed_under() {
    for (run_id, identity) in [
        (Some("claude-abcd1234"), "a-probe"),
        (None, "a-probe"),
        (Some("claude-abcd1234"), ""),
        (None, ""),
    ] {
        let marker = review_marker(run_id, identity, None, None)
            .expect("nothing here is invalid; there is nothing to render");
        assert_eq!(marker, None);
    }
}

#[test]
fn an_epoch_renders_a_review_marker_carrying_epoch_and_findings() {
    let marker = review_marker(
        Some("claude-abcd1234"),
        "a-probe",
        Some("0123456789abcdef0123456789abcdef"),
        Some("finding-one+finding-two"),
    )
    .expect("every attribute is in the allowed alphabet")
    .expect("an epoch renders a marker");
    assert_eq!(
        marker,
        "<!-- issue-flow: review encoding=pct run-id=claude-abcd1234 identity=a-probe \
         epoch=0123456789abcdef0123456789abcdef findings=finding-one+finding-two -->"
    );
}

#[test]
fn an_epoch_with_no_findings_still_renders_a_review_marker() {
    let marker = review_marker(
        Some("claude-abcd1234"),
        "a-probe",
        Some("0123456789abcdef0123456789abcdef"),
        None,
    )
    .expect("every attribute is in the allowed alphabet")
    .expect("an epoch renders a marker");
    assert!(
        marker.contains("epoch=0123456789abcdef0123456789abcdef -->"),
        "the review marker was not rendered with no findings: {marker:?}"
    );
    assert!(
        !marker.contains("findings="),
        "an absent findings was rendered as an attribute anyway: {marker:?}"
    );
}

/// A finding identity means nothing without the receipt it was recorded
/// against — refused rather than silently filed under no epoch at all.
#[test]
fn findings_without_an_epoch_is_refused() {
    let failure = review_marker(
        Some("claude-abcd1234"),
        "a-probe",
        None,
        Some("finding-one"),
    )
    .expect_err("a finding identity with no receipt must be refused");
    let Failure::ConfigDefect(payload) = failure else {
        panic!("expected a ConfigDefect, got {failure:?}");
    };
    assert_eq!(payload["reason"], "finding-without-transaction");
}

/// `check-closing-keywords` names the release step, not only `transition --to
/// done` — the field carries [`crate::transport::closing::FOLLOW_UP`]'s text.
#[test]
fn a_branch_link_names_release_after_done_in_the_mandatory_follow_up() {
    let repo = tempfile::tempdir().expect("a directory for the fixture repository");
    let git = |arguments: &[&str]| -> bool {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    assert!(git(&["init", "--quiet", "-b", "main"]));
    assert!(git(&["config", "user.email", "nobody@example.invalid"]));
    assert!(git(&["config", "user.name", "nobody"]));
    assert!(git(&["commit", "--allow-empty", "--quiet", "-m", "base"]));
    let base_sha = std::process::Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git names the head")
        .stdout;
    let base_sha = String::from_utf8_lossy(&base_sha).trim().to_owned();
    // Faked rather than fetched: the range `keywords_in_commits` reads only
    // needs `refs/remotes/origin/main` to resolve, and a local ref of that name
    // resolves it without a real remote.
    assert!(git(&["update-ref", "refs/remotes/origin/main", &base_sha]));

    let branch = "fix/12-branch-link";
    assert!(git(&["checkout", "--quiet", "-b", branch]));
    assert!(git(&[
        "commit",
        "--allow-empty",
        "--quiet",
        "-m",
        "a change that names no issue",
    ]));

    let issue = 12;
    let base = "main";
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [{
                        "number": 7,
                        "state": "OPEN",
                        "headRefName": branch,
                        "baseRefName": base,
                    }],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        { "matches": "json body", "stdout": "{\"body\":\"A body that names nothing.\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let bin = crate::test_env::scripted_gh();
    let context = Context {
        skill_dir: repo.path().to_path_buf(),
        repo_dir: repo.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let answer = crate::test_env::with_scripted_gh(bin.path(), &answers, || {
        check_closing_keywords(&context, issue, Some(base), Some(branch))
    })
    .expect("a branch-link autoclose is reported, not refused");

    assert_eq!(answer["cause"], "branch-link");
    let follow_up = answer["mandatory_follow_up"]
        .as_str()
        .expect("mandatory_follow_up is a string");
    assert_eq!(
        follow_up,
        crate::transport::closing::FOLLOW_UP,
        "the follow-up is not the shared constant, so it can drift from it silently: {follow_up}"
    );
}

/// A closing keyword naming an issue other than the delivered one is refused by
/// the standalone check too.
#[test]
fn a_closing_keyword_naming_another_issue_is_refused_by_the_standalone_check() {
    let repo = tempfile::tempdir().expect("a directory for the fixture repository");
    let git = |arguments: &[&str]| -> bool {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    assert!(git(&["init", "--quiet", "-b", "main"]));
    assert!(git(&["config", "user.email", "nobody@example.invalid"]));
    assert!(git(&["config", "user.name", "nobody"]));
    assert!(git(&["commit", "--allow-empty", "--quiet", "-m", "base"]));
    let base_sha = std::process::Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git names the head")
        .stdout;
    let base_sha = String::from_utf8_lossy(&base_sha).trim().to_owned();
    assert!(git(&["update-ref", "refs/remotes/origin/main", &base_sha]));

    let branch = "fix/575-closes-another";
    assert!(git(&["checkout", "--quiet", "-b", branch]));
    assert!(git(&[
        "commit",
        "--allow-empty",
        "--quiet",
        "-m",
        "a change that names no issue",
    ]));

    let issue = 575;
    let base = "main";
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [{
                        "number": 7,
                        "state": "OPEN",
                        "headRefName": branch,
                        "baseRefName": base,
                    }],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        { "matches": "json body", "stdout": "{\"body\":\"Closes #464, and names #469.\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let bin = crate::test_env::scripted_gh();
    let context = Context {
        skill_dir: repo.path().to_path_buf(),
        repo_dir: repo.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let failure = crate::test_env::with_scripted_gh(bin.path(), &answers, || {
        check_closing_keywords(&context, issue, Some(base), Some(branch))
    })
    .expect_err("a keyword naming another issue must be refused");
    let Failure::Stop(payload) = failure else {
        panic!("expected a Stop, got {failure:?}");
    };
    assert_eq!(payload["reason"], "closing-keyword-live");
    assert_eq!(payload["cause"], "closing-keyword-elsewhere");
    assert_eq!(payload["other_issue_keywords"][0]["text"], "Closes #464");
}

#[test]
fn only_the_general_partition_has_a_scale_the_binary_can_read() {
    let contract = include_str!("../../../skill/references/domain-composition.md");
    let identifier = contract
        .lines()
        .find_map(|line| line.strip_prefix("- Stable domain identifier: `"))
        .and_then(|rest| rest.split_once('`'))
        .map(|(value, _)| value)
        .expect("the autonomous contract declares a stable domain identifier");
    let scale = contract
        .lines()
        .find_map(|line| line.strip_prefix("- Scale: `"))
        .and_then(|rest| rest.split_once('`'))
        .map(|(value, _)| value)
        .expect("the autonomous contract declares a scale");
    assert_eq!(
        declared_scale(identifier),
        Some(scale),
        "the transport reads a different readable scale than {identifier:?} declares"
    );
    for domain in [
        "harness",
        "documentation",
        "review-contract",
        "unassigned",
        "",
    ] {
        assert_eq!(
            declared_scale(domain),
            None,
            "{domain} gained a readable scale with no contract behind it"
        );
    }
}

#[test]
fn a_scale_value_is_any_non_empty_value_and_its_meaning_is_not_judged() {
    let labels = ["status:ready".to_owned(), "priority:urgent".to_owned()];
    assert!(
        carries_scale_value(&labels, "priority"),
        "an unlisted value was read as no value at all"
    );
    for labels in [
        vec!["status:ready".to_owned(), "scale:high".to_owned()],
        vec!["status:ready".to_owned(), "priority:".to_owned()],
        vec!["status:ready".to_owned()],
    ] {
        assert!(
            !carries_scale_value(&labels, "priority"),
            "{labels:?} was read as a value on the priority scale"
        );
    }
}

#[test]
fn a_filed_priority_is_checked_only_on_a_readable_scale() {
    assert!(priority_scale_checked("priority:high", Some("general")));
    for (priority, domain) in [
        ("scale:high", Some("general")),
        ("high", Some("general")),
        ("priority:", Some("general")),
        ("priority:high", Some("harness")),
        ("priority:high", None),
    ] {
        assert!(
            !priority_scale_checked(priority, domain),
            "{priority:?} on {domain:?} was reported checked"
        );
    }
}

/// The five answers [`classify_base_movement`] emits, pinned whole — the issue
/// that revisited them had to leave `unknown` never compatible and never
/// leaving `integrate` false. Mutating the `None` arm's `integrate` to `false`,
/// or its `movement` to `"compatible"`, turns this red.
#[test]
fn unknown_is_never_compatible_and_only_none_and_compatible_leave_integrate_false() {
    let recorded = "a".repeat(40);
    let current = "b".repeat(40);
    let moved: std::collections::BTreeSet<String> = ["src/base.rs".to_owned()].into();
    let target: std::collections::BTreeSet<String> = ["src/delivery.rs".to_owned()].into();

    let answers = [
        (
            classify_base_movement(&recorded, &recorded, &moved, &target, Some(false)),
            "none",
            false,
        ),
        (
            classify_base_movement(&recorded, &current, &moved, &target, Some(false)),
            "compatible",
            false,
        ),
        (
            classify_base_movement(&recorded, &current, &moved, &moved, Some(false)),
            "overlapping",
            true,
        ),
        (
            classify_base_movement(&recorded, &current, &moved, &target, Some(true)),
            "conflicting",
            true,
        ),
        (
            classify_base_movement(&recorded, &current, &moved, &target, None),
            "unknown",
            true,
        ),
    ];
    for (answer, movement, integrate) in answers {
        assert_eq!(answer["movement"], movement, "{answer}");
        assert_eq!(answer["integrate"], integrate, "{answer}");
    }
}

/// The identity reader answers the key each open issue recorded in its
/// `analysis` marker — never the title — and answers `null` for an issue that
/// recorded none, which is not `""` and not an error.
///
/// Mutation: read the title instead of the marker, or answer `""` where the
/// marker carries none, and this reddens. The identity is the store's, recorded
/// by the import from the marker in the body, so a lying title cannot reach it.
#[test]
fn list_identities_reads_the_analysis_marker_and_never_the_title() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let repo = crate::transport::store::Repo {
        host: "github.com".to_owned(),
        owner: "o".to_owned(),
        name: "r".to_owned(),
    };
    let mut store =
        crate::transport::store::Store::open(&store_path).expect("the scratch store opens");
    let bodies = [
        (
            11,
            "## Problem\n\nThe key, in the body.\n\n\
             <!-- issue-flow: analysis encoding=pct run-id=claude-abcd1234 identity=a-probe -->",
        ),
        (12, "## Problem\n\nNo marker, only a title that lies."),
        (
            13,
            "<!-- issue-flow: analysis encoding=pct identity=held-probe -->",
        ),
        (14, "<!-- issue-flow: analysis encoding=pct identity= -->"),
    ];
    for (number, body) in bodies {
        store
            .import(
                &repo,
                number,
                &serde_json::json!({ "body": body, "labels": [], "comments": [] }),
                "2026-01-01T00:00Z",
            )
            .expect("the issue imports");
    }
    drop(store);

    let context = crate::transport::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: Some("o/r".to_owned()),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[serde_json::json!({
        "matches": "issues?state=open",
        "stdout": serde_json::json!([[
            { "number": 11, "title": "a title that is not the key",
              "labels": [], "created_at": "2026-01-01T00:00:00Z" },
            { "number": 12, "title": "a-probe: a title the reader must ignore",
              "labels": [], "created_at": "2026-01-01T00:00:00Z" },
            { "number": 13, "title": "held work is still a duplicate",
              "labels": [], "created_at": "2026-01-01T00:00:00Z" },
            { "number": 14, "title": "an empty key is no key",
              "labels": [], "created_at": "2026-01-01T00:00:00Z" }
        ]]).to_string(),
        "status": 0,
    })]);
    let answer = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            super::list_identities(&context, 200)
        })
    })
    .expect("the identities were read");

    let identities = answer["identities"]
        .as_array()
        .expect("an identities array");
    assert_eq!(answer["count"], 4, "{answer}");
    assert_eq!(
        identities[0]["identity"], "a-probe",
        "the key was not read from the analysis marker: {answer}"
    );
    assert!(
        identities[1]["identity"].is_null(),
        "an issue with no analysis marker was not reported as carrying no identity: {answer}"
    );
    assert_eq!(
        identities[2]["identity"], "held-probe",
        "an open issue was dropped, or its key was not read: {answer}"
    );
    assert!(
        identities[3]["identity"].is_null(),
        "an empty identity attribute was reported as a key rather than as no identity: {answer}"
    );
}

/// A filing made before the template stopped carrying its own `analysis` marker
/// holds two: the template's identity-less literal first, then the one `create`
/// composed. The reader must answer the identity the later marker carries, not
/// `null` because the first one has none.
///
/// Mutation: take the first `analysis` marker rather than the first carrying a
/// non-empty identity, and this reddens.
#[test]
fn an_identity_less_analysis_marker_does_not_shadow_the_one_carrying_the_identity() {
    let body = "## Problem\n\nA probe.\n\n\
                <!-- issue-flow: analysis encoding=pct run-id=claude-abcd1234 -->\n\n\
                <!-- issue-flow: analysis encoding=pct run-id=claude-abcd1234 identity=a-probe -->\n";
    assert_eq!(
        recorded_identity(body),
        Some("a-probe".to_owned()),
        "the identity-less marker shadowed the one carrying the identity: {body:?}"
    );
}

/// The queue read decides the state from the local store and never reads a
/// candidate. The scripted `gh` refuses any `issue view`, so a queue read that
/// touched one would fail rather than answer.
#[test]
fn list_state_decides_from_the_store_and_makes_no_issue_view() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let repo = crate::transport::store::Repo {
        host: "github.com".to_owned(),
        owner: "o".to_owned(),
        name: "r".to_owned(),
    };
    let mut store =
        crate::transport::store::Store::open(&store_path).expect("the scratch store opens");
    store
        .import(
            &repo,
            5,
            &serde_json::json!({
                "body": "Body.",
                "labels": [{ "name": "status:ready" }],
                "comments": [],
            }),
            "2026-01-01T00:00Z",
        )
        .expect("the issue imports");
    drop(store);

    let context = crate::transport::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: Some("o/r".to_owned()),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "issues?state=open",
            "stdout": serde_json::json!([[
                { "number": 5, "title": "ready work",
                  "labels": [{ "name": "domain:general" }],
                  "created_at": "2026-01-01T00:00:00Z" }
            ]]).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "issue view",
            "stdout": "",
            "stderr": "gh: the queue read must not read a candidate",
            "status": 1,
        }),
    ]);
    let answer = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            super::list_state(&context, "ready", "opencode-test", 200, "2026-01-01T00:00Z")
        })
    })
    .expect("the queue was read");
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(
        answer["count"], 1,
        "the store's ready issue was not in the queue: {answer}"
    );
    assert!(
        answer.get("unadmitted").is_none(),
        "an admitted issue was reported unadmitted: {answer}"
    );
}

/// An open issue the store has never admitted is named under `unadmitted` and
/// left out of the state's queue: a `status:` label never decides.
#[test]
fn list_state_reports_an_issue_the_store_never_admitted_as_unadmitted() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    // A store that exists but holds nothing: the repository is unknown.
    drop(crate::transport::store::Store::open(&store_path).expect("the scratch store opens"));

    let context = crate::transport::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: Some("o/r".to_owned()),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[serde_json::json!({
        "matches": "issues?state=open",
        "stdout": serde_json::json!([[
            { "number": 6, "title": "filed by hand",
              "labels": [{ "name": "status:ready" }, { "name": "domain:general" }],
              "created_at": "2026-01-01T00:00:00Z" }
        ]]).to_string(),
        "status": 0,
    })]);
    let answer = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            super::list_state(&context, "ready", "opencode-test", 200, "2026-01-01T00:00Z")
        })
    })
    .expect("the queue was read");
    assert_eq!(
        answer["count"], 0,
        "an issue the store never admitted was placed in the ready queue: {answer}"
    );
    assert_eq!(
        answer["unadmitted"][0]["number"], 6,
        "the unadmitted issue was not named: {answer}"
    );
}

fn local_context(root: &std::path::Path) -> crate::transport::Context {
    crate::transport::Context {
        skill_dir: root.to_path_buf(),
        repo_dir: root.to_path_buf(),
        config: Vec::new(),
        repo: Some("o/r".to_owned()),
    }
}

fn scratch_repo() -> crate::transport::store::Repo {
    crate::transport::store::Repo {
        host: "github.com".to_owned(),
        owner: "o".to_owned(),
        name: "r".to_owned(),
    }
}

fn admit(store: &mut crate::transport::store::Store, number: u64, state: &str, body: &str) {
    store
        .import(
            &scratch_repo(),
            number,
            &serde_json::json!({
                "body": body,
                "labels": [{ "name": format!("status:{state}") }],
                "comments": [],
            }),
            "2026-01-01T00:00Z",
        )
        .expect("the issue imports");
}

fn gh_writes(log: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| {
            [
                "issue comment",
                "issue edit",
                "issue create",
                "issue close",
                "label create",
                "--method",
                "-X",
            ]
            .iter()
            .any(|word| line.contains(word))
        })
        .map(ToOwned::to_owned)
        .collect()
}

/// A clause `blocked_until` records is counted by the blocked queue exactly as a
/// clause imported from a body is, a repeat records nothing twice, `clear`
/// removes them, and none of it reaches GitHub.
///
/// Mutation: record the clause under another body than `markers::render` gives
/// the importer, skip the duplicate check, or make `clear` a no-op, and this
/// reddens.
#[test]
fn a_clause_blocked_until_records_is_read_by_the_queue_as_an_imported_one_is() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut store =
        crate::transport::store::Store::open(&store_path).expect("the scratch store opens");
    admit(&mut store, 14, "done", "A clause target that is done.");
    admit(&mut store, 15, "blocked", "No clause in the body.");
    admit(
        &mut store,
        16,
        "blocked",
        "<!-- issue-flow: blocked-until issue=14 state=done -->",
    );
    drop(store);

    let context = local_context(root.path());
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[serde_json::json!({
        "matches": "issues?state=open",
        "stdout": serde_json::json!([[
            { "number": 15, "title": "recorded", "labels": [],
              "created_at": "2026-01-01T00:00:00Z" },
            { "number": 16, "title": "imported", "labels": [],
              "created_at": "2026-01-01T00:00:00Z" }
        ]]).to_string(),
        "status": 0,
    })]);
    let log = root.path().join("calls.log");
    let (first, again, queue, cleared, after_clear) =
        crate::transport::store::with_store_at(&store_path, || {
            crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
                let record = || {
                    super::blocked_until(
                        &context,
                        15,
                        Some(14),
                        Some("done"),
                        false,
                        "2026-01-02T00:00Z",
                    )
                    .expect("the clause records")
                };
                let first = record();
                let again = record();
                let queue = super::list_state(
                    &context,
                    "blocked",
                    "claude-abcd1234",
                    200,
                    "2026-01-02T00:00Z",
                )
                .expect("the blocked queue reads");
                let cleared =
                    super::blocked_until(&context, 15, None, None, true, "2026-01-02T00:00Z")
                        .expect("the clauses clear");
                let after_clear = super::list_state(
                    &context,
                    "blocked",
                    "claude-abcd1234",
                    200,
                    "2026-01-02T00:00Z",
                )
                .expect("the blocked queue reads");
                (first, again, queue, cleared, after_clear)
            })
        });

    assert_eq!(
        (first["recorded"].clone(), first["clauses"].clone()),
        (true.into(), 1.into())
    );
    assert_eq!(
        (again["recorded"].clone(), again["clauses"].clone()),
        (false.into(), 1.into()),
        "a repeated clause was recorded twice: {again}"
    );
    assert_eq!(queue["dischargeable"], 2, "{queue}");
    let entries = queue["partitions"]["unassigned"]
        .as_array()
        .expect("the blocked items are partitioned");
    assert_eq!(
        entries[0]["conditions"], entries[1]["conditions"],
        "a recorded clause and an imported one read differently: {queue}"
    );
    assert_eq!(cleared["cleared"], 1, "{cleared}");
    assert_eq!(after_clear["dischargeable"], 1, "{after_clear}");
    assert_eq!(after_clear["unconditioned"], 1, "{after_clear}");
    assert!(
        gh_writes(&log).is_empty(),
        "blocked_until reached GitHub: {:?}",
        gh_writes(&log)
    );
}

/// A clause is read only for an issue held in `blocked`, and only a well formed
/// one is recorded.
#[test]
fn blocked_until_refuses_an_issue_that_is_not_blocked_and_a_clause_that_is_not_one() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut store =
        crate::transport::store::Store::open(&store_path).expect("the scratch store opens");
    admit(&mut store, 5, "ready", "Ready work.");
    admit(&mut store, 6, "blocked", "Blocked work.");
    drop(store);
    let context = local_context(root.path());

    let reason = |issue: u64, until: Option<u64>, state: Option<&str>, clear: bool| {
        crate::transport::store::with_store_at(&store_path, || {
            super::blocked_until(&context, issue, until, state, clear, "2026-01-02T00:00Z")
        })
        .expect_err("the call was refused")
        .envelope()["reason"]
            .clone()
    };
    assert_eq!(reason(5, Some(6), Some("done"), false), "issue-not-blocked");
    assert_eq!(
        reason(99, Some(6), Some("done"), false),
        "issue-not-blocked"
    );
    assert_eq!(
        reason(6, Some(0), Some("done"), false),
        "invalid-blocked-clause"
    );
    assert_eq!(
        reason(6, Some(5), Some("finished"), false),
        "invalid-blocked-clause"
    );
    assert_eq!(reason(6, Some(5), None, false), "invalid-blocked-clause");
    assert_eq!(reason(6, None, None, false), "invalid-blocked-clause");
    assert_eq!(
        reason(6, Some(5), Some("done"), true),
        "invalid-blocked-clause"
    );
}

/// The checklist is written under the live claim, revised by id, read back in
/// the order it was first recorded, and never reaches GitHub for a write.
///
/// Mutation: drop the `verify_claim` call, let `done` through without a commit,
/// or reorder the rows on update, and this reddens.
#[test]
fn task_records_the_checklist_under_the_claim_and_tasks_reads_it_back() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut context = local_context(root.path());
    context.repo = Some("fixture/repo".to_owned());
    let bin = crate::test_env::scripted_gh();
    let held = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        5,
        "in-progress",
    )]);
    let log = root.path().join("calls.log");
    let call = |id: &'static str,
                text: Option<&'static str>,
                route: Option<&'static str>,
                status: Option<&'static str>,
                commit: Option<&'static str>| {
        super::task(
            &context,
            &super::TaskCall {
                issue: 5,
                run_id: "claude-abcd1234",
                expect_state: "in-progress",
                id,
                text,
                route,
                status,
                commit,
                evidence: None,
                compact: false,
                now: "2026-01-02T00:00Z",
            },
        )
    };
    let reason = |result: Result<serde_json::Value, Failure>| {
        result.expect_err("the call was refused").envelope()["reason"].clone()
    };

    let (refusals, last, read) = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_logging_gh(bin.path(), &held, &log, || {
            let refusals = vec![
                reason(call("T1", None, None, None, None)),
                reason(call("T 1", Some("a"), Some("inline"), None, None)),
                reason(call("T1", Some("a"), Some("sideways"), None, None)),
                reason(call("T1", Some("a"), Some("inline"), Some("done"), None)),
                reason(call("T1", Some("a"), Some("inline"), None, Some("XYZ"))),
            ];
            call("T1", Some("first"), Some("inline"), None, None).expect("T1 records");
            call("T2", Some("second"), Some("delegated"), None, None).expect("T2 records");
            let last =
                call("T1", None, None, Some("done"), Some("abcdef0")).expect("T1 is revised");
            let read = super::tasks(&context, 5).expect("the checklist reads");
            (refusals, last, read)
        })
    });
    assert_eq!(
        refusals,
        [
            "task-incomplete",
            "invalid-task",
            "invalid-task",
            "task-done-without-commit",
            "invalid-task"
        ]
    );
    assert_eq!(last["task"]["done"], true, "{last}");
    assert_eq!(last["task"]["text"], "first", "{last}");
    assert_eq!(last["task"]["commit"], "abcdef0", "{last}");
    assert_eq!(
        read,
        serde_json::json!({
            "ok": true,
            "issue": 5,
            "repo": "fixture/repo",
            "scope": "candidate",
            "count": 2,
            "tasks": [
                { "id": "T1", "text": "first", "route": "inline", "done": true, "commit": "abcdef0", "evidence": null },
                { "id": "T2", "text": "second", "route": "delegated", "done": false, "commit": null, "evidence": null },
            ],
        })
    );
    assert!(
        gh_writes(&log).is_empty(),
        "task reached GitHub for a write: {:?}",
        gh_writes(&log)
    );
}

/// A task whose work is not a commit reaches `done` by naming where it is
/// recorded, and the read-back carries that note beside a null commit.
///
/// Mutation: refuse a `done` row that has only `evidence`, or drop `evidence`
/// from the read-back, and this reddens.
#[test]
fn a_done_task_may_record_evidence_instead_of_a_commit() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut context = local_context(root.path());
    context.repo = Some("fixture/repo".to_owned());
    let bin = crate::test_env::scripted_gh();
    let held = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        5,
        "in-progress",
    )]);
    let log = root.path().join("calls.log");

    let (done, read) = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_logging_gh(bin.path(), &held, &log, || {
            let call = |status: Option<&str>, commit: Option<&str>, evidence: Option<&str>| {
                super::task(
                    &context,
                    &super::TaskCall {
                        issue: 5,
                        run_id: "claude-abcd1234",
                        expect_state: "in-progress",
                        id: "T1",
                        text: Some("a bookkeeping task"),
                        route: Some("inline"),
                        status,
                        commit,
                        evidence,
                        compact: false,
                        now: "2026-01-02T00:00Z",
                    },
                )
            };
            call(None, None, None).expect("T1 records");
            let done = call(
                Some("done"),
                None,
                Some("recorded in the issue close evidence; no change was required"),
            )
            .expect("a task done by evidence alone");
            let read = super::tasks(&context, 5).expect("the checklist reads");
            (done, read)
        })
    });

    assert_eq!(done["task"]["done"], true, "{done}");
    assert_eq!(done["task"]["commit"], serde_json::Value::Null, "{done}");
    assert_eq!(
        done["task"]["evidence"], "recorded in the issue close evidence; no change was required",
        "{done}"
    );
    assert_eq!(
        read["tasks"][0]["evidence"],
        "recorded in the issue close evidence; no change was required",
        "{read}"
    );
    assert_eq!(
        read["tasks"][0]["commit"],
        serde_json::Value::Null,
        "{read}"
    );
    assert!(
        gh_writes(&log).is_empty(),
        "task reached GitHub for a write: {:?}",
        gh_writes(&log)
    );
}

/// An `evidence` note that is blank or past the byte cap is refused the same
/// way `text` is, before anything is written.
#[test]
fn blank_or_oversized_evidence_is_refused() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut context = local_context(root.path());
    context.repo = Some("fixture/repo".to_owned());
    let bin = crate::test_env::scripted_gh();
    let held = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        5,
        "in-progress",
    )]);
    let log = root.path().join("calls.log");

    let refusals = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_logging_gh(bin.path(), &held, &log, || {
            let call = |evidence: &str| {
                super::task(
                    &context,
                    &super::TaskCall {
                        issue: 5,
                        run_id: "claude-abcd1234",
                        expect_state: "in-progress",
                        id: "T1",
                        text: Some("a task"),
                        route: Some("inline"),
                        status: Some("done"),
                        commit: None,
                        evidence: Some(evidence),
                        compact: false,
                        now: "2026-01-02T00:00Z",
                    },
                )
                .expect_err("the call was refused")
                .envelope()["reason"]
                    .clone()
            };
            vec![call("   "), call(&"x".repeat(4001))]
        })
    });

    assert_eq!(
        refusals,
        [
            serde_json::json!("invalid-task"),
            serde_json::json!("invalid-task")
        ]
    );
}

/// A run that does not hold the issue writes no row.
#[test]
fn task_is_refused_for_a_run_that_does_not_hold_the_issue() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut context = local_context(root.path());
    context.repo = Some("fixture/repo".to_owned());
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::not_current_live_holder_issue(5)]);
    let (refused, read) = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            let refused = super::task(
                &context,
                &super::TaskCall {
                    issue: 5,
                    run_id: "claude-abcd1234",
                    expect_state: "in-progress",
                    id: "T1",
                    text: Some("first"),
                    route: Some("inline"),
                    status: None,
                    commit: None,
                    evidence: None,
                    compact: false,
                    now: "2026-01-02T00:00Z",
                },
            );
            (
                refused,
                super::tasks(&context, 5).expect("the checklist reads"),
            )
        })
    });
    assert!(refused.is_err(), "a run holding nothing wrote a task");
    assert_eq!(read["count"], 0, "{read}");
}

/// The legacy acknowledgment carries the changed row beside the whole checklist,
/// so twenty inserts emit 230 row copies where twenty rows changed — the figure
/// a compact response is measured against. `tasks` is the recovery path and stays
/// complete and ordered.
///
/// Mutation: drop the changed row from the answer, or lose or reorder a row in
/// the read-back, and this reddens.
#[test]
fn the_legacy_task_acknowledgment_repeats_the_whole_checklist() {
    fn record(context: &crate::transport::Context, id: &str) -> Result<serde_json::Value, Failure> {
        super::task(
            context,
            &super::TaskCall {
                issue: 5,
                run_id: "claude-abcd1234",
                expect_state: "in-progress",
                id,
                text: Some("a task"),
                route: Some("inline"),
                status: None,
                commit: None,
                evidence: None,
                compact: false,
                now: "2026-01-02T00:00Z",
            },
        )
    }

    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut context = local_context(root.path());
    context.repo = Some("fixture/repo".to_owned());
    let bin = crate::test_env::scripted_gh();
    let held = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        5,
        "in-progress",
    )]);
    let log = root.path().join("calls.log");

    let (copies, last, read) = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_logging_gh(bin.path(), &held, &log, || {
            let mut copies = 0usize;
            let mut last = None;
            for n in 1..=20 {
                let answer = record(&context, &format!("T{n}")).expect("the task records");
                copies += 1 + rows(&answer["tasks"]);
                last = Some(answer);
            }
            let read = super::tasks(&context, 5).expect("the checklist reads");
            (copies, last.expect("twenty answers"), read)
        })
    });

    assert_eq!(
        copies, 230,
        "twenty inserts emitted {copies} row copies, where only twenty rows changed"
    );
    assert_eq!(last["task"]["id"], "T20", "{last}");
    assert_eq!(
        rows(&last["tasks"]),
        20,
        "the changed row travels beside the whole checklist: {last}"
    );
    assert_eq!(read["count"], 20, "{read}");
    assert_eq!(
        ids(&read["tasks"]),
        (1..=20).map(|n| format!("T{n}")).collect::<Vec<_>>(),
        "the read-back is complete and in the order the rows were first recorded"
    );
    assert!(
        gh_writes(&log).is_empty(),
        "task reached GitHub for a write: {:?}",
        gh_writes(&log)
    );
}

/// The compact acknowledgment carries the changed row alone, so twenty inserts
/// emit twenty row copies where the legacy answer emits 230. `tasks` is the
/// recovery path and still holds every row, in order.
///
/// Mutation: make the compact path read the checklist back, or answer without
/// the changed row, and this reddens.
#[test]
fn the_compact_task_acknowledgment_carries_only_the_changed_row() {
    fn record(context: &crate::transport::Context, id: &str) -> serde_json::Value {
        super::task(
            context,
            &super::TaskCall {
                issue: 5,
                run_id: "claude-abcd1234",
                expect_state: "in-progress",
                id,
                text: Some("a task"),
                route: Some("inline"),
                status: None,
                commit: None,
                evidence: None,
                compact: true,
                now: "2026-01-02T00:00Z",
            },
        )
        .expect("the task records")
    }

    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut context = local_context(root.path());
    context.repo = Some("fixture/repo".to_owned());
    let bin = crate::test_env::scripted_gh();
    let held = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        5,
        "in-progress",
    )]);
    let log = root.path().join("calls.log");

    let (copies, last, read) = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_logging_gh(bin.path(), &held, &log, || {
            let mut copies = 0usize;
            let mut last = None;
            for n in 1..=20 {
                let answer = record(&context, &format!("T{n}"));
                assert!(
                    answer.get("tasks").is_none(),
                    "the compact answer carried the checklist: {answer}"
                );
                assert_eq!(answer["task"]["id"], format!("T{n}"), "{answer}");
                copies += 1 + answer["tasks"].as_array().map_or(0, Vec::len);
                last = Some(answer);
            }
            let read = super::tasks(&context, 5).expect("the checklist reads");
            (copies, last.expect("twenty answers"), read)
        })
    });

    assert_eq!(
        copies, 20,
        "twenty compact inserts emitted {copies} row copies, where only twenty rows changed"
    );
    let last = &last;
    assert_eq!(last["task"]["id"], "T20", "{last}");
    assert_eq!(last["task"]["text"], "a task", "{last}");
    assert_eq!(last["task"]["route"], "inline", "{last}");
    assert_eq!(last["task"]["done"], false, "{last}");
    assert_eq!(read["count"], 20, "{read}");
    assert_eq!(
        ids(&read["tasks"]),
        (1..=20).map(|n| format!("T{n}")).collect::<Vec<_>>(),
        "the read-back is complete and in the order the rows were first recorded"
    );
    assert!(
        gh_writes(&log).is_empty(),
        "task reached GitHub for a write: {:?}",
        gh_writes(&log)
    );
}

/// A compact revision answers with the revised row alone and leaves the legacy
/// answer and the checklist as they were: the same id, a new status and commit.
///
/// Mutation: answer the row as it was stored rather than the one just written,
/// and this reddens.
#[test]
fn a_compact_revision_answers_the_revised_row_and_leaves_the_checklist_alone() {
    fn record(
        context: &crate::transport::Context,
        id: &str,
        text: Option<&str>,
        route: Option<&str>,
        status: Option<&str>,
        commit: Option<&str>,
        compact: bool,
    ) -> serde_json::Value {
        super::task(
            context,
            &super::TaskCall {
                issue: 5,
                run_id: "claude-abcd1234",
                expect_state: "in-progress",
                id,
                text,
                route,
                status,
                commit,
                evidence: None,
                compact,
                now: "2026-01-02T00:00Z",
            },
        )
        .expect("the task records")
    }

    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let mut context = local_context(root.path());
    context.repo = Some("fixture/repo".to_owned());
    let bin = crate::test_env::scripted_gh();
    let held = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        5,
        "in-progress",
    )]);
    let log = root.path().join("calls.log");

    let (revised, legacy, read) = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_logging_gh(bin.path(), &held, &log, || {
            record(
                &context,
                "T1",
                Some("first"),
                Some("inline"),
                None,
                None,
                true,
            );
            record(
                &context,
                "T2",
                Some("second"),
                Some("delegated"),
                None,
                None,
                true,
            );
            let revised = record(
                &context,
                "T1",
                None,
                None,
                Some("done"),
                Some("abcdef0"),
                true,
            );
            let legacy = record(&context, "T1", None, None, None, None, false);
            let read = super::tasks(&context, 5).expect("the checklist reads");
            (revised, legacy, read)
        })
    });

    assert_eq!(
        revised,
        serde_json::json!({
            "ok": true,
            "issue": 5,
            "task": {
                "id": "T1",
                "text": "first",
                "route": "inline",
                "done": true,
                "commit": "abcdef0",
                "evidence": null,
            },
        }),
        "{revised}"
    );
    assert!(
        revised.get("tasks").is_none(),
        "the compact answer carried the checklist: {revised}"
    );

    assert_eq!(legacy["task"]["done"], true, "{legacy}");
    assert_eq!(legacy["task"]["commit"], "abcdef0", "{legacy}");
    assert_eq!(rows(&legacy["tasks"]), 2, "{legacy}");
    assert_eq!(read["count"], 2, "{read}");
    assert_eq!(ids(&read["tasks"]), vec!["T1", "T2"], "{read}");
    assert_eq!(read["tasks"][0]["done"], true, "{read}");
    assert_eq!(read["tasks"][0]["commit"], "abcdef0", "{read}");
    assert!(
        gh_writes(&log).is_empty(),
        "task reached GitHub for a write: {:?}",
        gh_writes(&log)
    );
}

fn rows(value: &serde_json::Value) -> usize {
    value.as_array().expect("the checklist is an array").len()
}

fn ids(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .expect("the checklist is an array")
        .iter()
        .map(|row| {
            row["id"]
                .as_str()
                .expect("every row carries an id")
                .to_owned()
        })
        .collect()
}

/// The title search sends the whole title as one quoted phrase, so search syntax
/// inside a title cannot drop the issue the retry is looking for.
///
/// Mutation: interpolate the title unquoted, or leave an embedded `"` in the
/// phrase, and this reddens.
#[test]
fn the_title_search_quotes_the_title_as_one_phrase() {
    let root = tempfile::tempdir().expect("a directory for the fixture repository");
    let store_path = root.path().join("state.db");
    let store = crate::transport::store::Store::open(&store_path).expect("the scratch store opens");
    let context = local_context(root.path());
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[serde_json::json!({
        "matches": "issue list",
        "stdout": "[]",
        "status": 0,
    })]);
    let log = root.path().join("calls.log");
    let found = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        super::filed_under_title(
            &context,
            &store,
            &scratch_repo(),
            "fix -D warnings label:x \"q",
            None,
        )
    })
    .expect("the search answers");

    assert_eq!(found, None);
    let calls = std::fs::read_to_string(&log).expect("the search was logged");
    assert!(
        calls.contains("--search \"fix -D warnings label:x  q\" in:title"),
        "the title was not searched as one quoted phrase: {calls}"
    );
}

fn closed_window() -> crate::transport::window::Mode {
    crate::transport::window::Mode::Closed(
        crate::config::GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule"),
    )
}

fn the_six_labels(answer: &serde_json::Value) -> Vec<String> {
    answer["ensured"]
        .as_array()
        .expect("the six labels")
        .iter()
        .map(|label| label.as_str().expect("a label").to_owned())
        .collect()
}

#[test]
fn ensure_states_defers_while_the_git_operation_window_is_closed() {
    let root = tempfile::tempdir().expect("a directory");
    let log = root.path().join("calls.log");
    let bin = crate::test_env::scripted_gh();
    let context = local_context(root.path());

    let answer = crate::test_env::with_logging_gh(bin.path(), "", &log, || {
        crate::transport::window::with_mode(closed_window(), || super::ensure_states(&context))
    })
    .expect("a closed window is answered, not refused");

    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["deferred"], true, "{answer}");
    assert_eq!(answer["window"], "closed", "{answer}");
    assert_eq!(
        the_six_labels(&answer),
        crate::config::STATES
            .iter()
            .map(|state| format!("status:{state}"))
            .collect::<Vec<_>>(),
        "{answer}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        calls.is_empty(),
        "the deferred answer still started gh: {calls}"
    );
}

#[test]
fn ensure_states_creates_the_labels_while_the_window_is_open() {
    let root = tempfile::tempdir().expect("a directory");
    let log = root.path().join("calls.log");
    let bin = crate::test_env::scripted_gh();
    let context = local_context(root.path());

    let answer = crate::test_env::with_logging_gh(bin.path(), "", &log, || {
        crate::transport::window::with_mode(crate::transport::window::Mode::Open, || {
            super::ensure_states(&context)
        })
    })
    .expect("an open window creates the labels");

    let expected: Vec<String> = crate::config::STATES
        .iter()
        .map(|state| format!("status:{state}"))
        .collect();
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["deferred"], serde_json::Value::Null, "{answer}");
    assert_eq!(the_six_labels(&answer), expected, "{answer}");
    let calls = std::fs::read_to_string(&log).expect("the calls were logged");
    let created: Vec<String> = calls
        .lines()
        .filter_map(|line| line.strip_prefix("label create "))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        created, expected,
        "an open window did not create each label once, in order: {calls}"
    );
}

fn two_repository_store(path: &std::path::Path) {
    drop(crate::transport::store::Store::open(path).expect("the store opens"));
    rusqlite::Connection::open(path)
        .expect("the store opens")
        .execute_batch(
            "INSERT INTO repos (id, host, owner, name) VALUES
               (1, 'github.com', 'o', 'a'), (2, 'github.com', 'o', 'b'), (3, 'github.com', 'o', 'c');
             INSERT INTO issues (repo_id, number, state, history) VALUES
               (2, 77, 'in-progress', 'native'), (2, 5, 'ready', 'native'), (3, 5, 'ready', 'native');
             INSERT INTO tasks (repo_id, issue, id, text, route, done, at)
               VALUES (2, 77, 'T1', 'the checklist of b', 'inline', 0, '2026-01-01T00:00Z');",
        )
        .expect("the fixture seeds");
}

#[test]
fn tasks_for_a_number_held_only_in_another_repository_reads_that_repositorys_checklist() {
    let root = tempfile::tempdir().expect("a directory");
    let store_path = root.path().join("state.db");
    two_repository_store(&store_path);
    let mut context = local_context(root.path());
    context.repo = Some("o/a".to_owned());

    let read = crate::transport::store::with_store_at(&store_path, || super::tasks(&context, 77))
        .expect("the checklist reads");

    assert_eq!(read["count"], 1, "{read}");
    assert_eq!(read["repo"], "o/b", "{read}");
    assert_eq!(read["scope"], "candidate", "{read}");
}

#[test]
fn tasks_for_a_number_held_in_two_other_repositories_is_refused_with_both() {
    let root = tempfile::tempdir().expect("a directory");
    let store_path = root.path().join("state.db");
    two_repository_store(&store_path);
    let mut context = local_context(root.path());
    context.repo = Some("o/a".to_owned());

    let refused = crate::transport::store::with_store_at(&store_path, || super::tasks(&context, 5))
        .expect_err("two candidates");

    let envelope = refused.envelope();
    assert_eq!(envelope["reason"], "ambiguous-issue", "{envelope}");
    assert_eq!(envelope["candidates"], serde_json::json!(["o/b", "o/c"]));
}

#[test]
fn tasks_from_a_repository_the_store_never_saw_is_unavailable_and_not_an_empty_list() {
    let root = tempfile::tempdir().expect("a directory");
    let store_path = root.path().join("state.db");
    two_repository_store(&store_path);
    let mut context = local_context(root.path());
    context.repo = Some("never/seen".to_owned());

    let read = crate::transport::store::with_store_at(&store_path, || super::tasks(&context, 123))
        .expect("the read answers");

    assert!(read["unavailable"].is_string(), "{read}");
    assert!(
        read.get("count").is_none(),
        "an unknown repository counted tasks: {read}"
    );
}
