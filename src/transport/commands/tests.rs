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
/// that revisited them (#124) had to leave `unknown` never compatible and never
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
            "count": 2,
            "tasks": [
                { "id": "T1", "text": "first", "route": "inline", "done": true, "commit": "abcdef0" },
                { "id": "T2", "text": "second", "route": "delegated", "done": false, "commit": null },
            ],
        })
    );
    assert!(
        gh_writes(&log).is_empty(),
        "task reached GitHub for a write: {:?}",
        gh_writes(&log)
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

const NOTES_CHANGELOG: &str = "# Changelog\n\n## 0.2.0 - unreleased\n\nThe entry.\n";

fn notes_of(root: &std::path::Path) -> Result<serde_json::Value, Failure> {
    changelog_notes(
        &local_context(root),
        std::path::Path::new("CHANGELOG.md"),
        "0.2.0",
        false,
        None,
    )
}

#[test]
fn notes_are_refused_while_a_fragment_waits_to_be_assembled() {
    let root = tempfile::tempdir().expect("a repository");
    std::fs::write(root.path().join("CHANGELOG.md"), NOTES_CHANGELOG).expect("the changelog");
    std::fs::create_dir(root.path().join("changes")).expect("changes/");
    for name in ["9-nine.md", "10-ten.md", ".gitkeep"] {
        std::fs::write(
            root.path().join("changes").join(name),
            "section: The harness\n\n- **A change.**\n",
        )
        .expect("a fragment");
    }

    let Err(failure) = notes_of(root.path()) else {
        panic!("notes were read past an unassembled fragment");
    };
    let answer = failure.envelope();

    assert_eq!(answer["reason"], "unassembled-changelog-fragments");
    assert_eq!(
        answer["fragments"],
        serde_json::json!(["10-ten.md", "9-nine.md"])
    );
    let action = answer["action"].as_str().expect("an action");
    assert!(
        action.contains("scripts/assemble-changelog.sh <version>"),
        "{action}"
    );
}

#[test]
fn notes_are_read_when_changes_is_absent_or_holds_no_fragment() {
    let root = tempfile::tempdir().expect("a repository");
    std::fs::write(root.path().join("CHANGELOG.md"), NOTES_CHANGELOG).expect("the changelog");

    let absent = notes_of(root.path()).expect("no changes/ directory");
    assert_eq!(absent["ok"], true);

    std::fs::create_dir(root.path().join("changes")).expect("changes/");
    std::fs::write(root.path().join("changes/.gitkeep"), "").expect("a placeholder");
    let empty = notes_of(root.path()).expect("an empty changes/ directory");
    assert_eq!(empty["notes"], "The entry.");
}

#[test]
fn another_tools_changes_directory_does_not_stop_the_notes() {
    let root = tempfile::tempdir().expect("a repository");
    std::fs::write(root.path().join("CHANGELOG.md"), NOTES_CHANGELOG).expect("the changelog");
    let changes = root.path().join("changes");
    std::fs::create_dir_all(changes.join("nested-1.md")).expect("a directory");
    std::fs::write(changes.join("123.feature.md"), "Adds a thing.\n").expect("towncrier");
    std::fs::write(changes.join("4-four.md"), "- no section line\n").expect("no section");
    std::fs::write(changes.join("notes.md"), "section: The harness\n- x\n").expect("no number");
    std::fs::write(changes.join(".gitkeep"), "").expect("a placeholder");

    let answer = notes_of(root.path()).expect("nothing here is this repository's fragment");

    assert_eq!(answer["notes"], "The entry.");
}
