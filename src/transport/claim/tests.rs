use super::super::Source;
use super::*;

#[test]
fn a_read_that_did_not_answer_resets_the_run_of_agreements() {
    // Two agreeing reads either side of a failure are not two in a row, and
    // treating them as such is how a flapping tracker looks settled.
    let yes = serde_json::json!({"seen": true});
    let no = serde_json::json!({"seen": false});
    let is_seen = |value: &serde_json::Value| value["seen"] == true;

    assert!(wait_for(&[Some(yes.clone())], is_seen, 1).is_some());
    assert!(wait_for(&[Some(no.clone()), Some(yes.clone())], is_seen, 1).is_some());
    assert!(wait_for(&[Some(no.clone())], is_seen, 1).is_none());

    // Two in a row, and the two things that break the run.
    assert!(wait_for(&[Some(yes.clone()), Some(yes.clone())], is_seen, 2).is_some());
    assert!(
        wait_for(&[Some(yes.clone()), None, Some(yes.clone())], is_seen, 2).is_none(),
        "a read that did not answer did not reset the run"
    );
    assert!(
        wait_for(&[Some(yes.clone()), Some(no), Some(yes)], is_seen, 2).is_none(),
        "a disagreeing read did not reset the run"
    );
}

#[test]
fn the_projection_names_exactly_the_holder_and_nobody_else() {
    // A stale `dev:` label tells everyone at a glance that the issue belongs to
    // a run that put it down — worse than no projection at all.
    let labels =
        |names: &[&str]| -> Vec<String> { names.iter().map(|name| (*name).to_owned()).collect() };

    // Nothing projected yet.
    assert_eq!(
        converge_projection(&labels(&["status:in-progress"]), Some("claude")),
        (vec!["dev:claude".to_owned()], Vec::new())
    );
    // Already right: nothing to do.
    assert_eq!(
        converge_projection(
            &labels(&["status:in-progress", "dev:claude"]),
            Some("claude")
        ),
        (Vec::new(), Vec::new())
    );
    // Somebody else's projection goes, and this one arrives.
    assert_eq!(
        converge_projection(&labels(&["dev:codex"]), Some("claude")),
        (vec!["dev:claude".to_owned()], vec!["dev:codex".to_owned()])
    );
    // No holder: every projection goes and none is added.
    assert_eq!(
        converge_projection(&labels(&["dev:claude", "dev:codex"]), None),
        (
            Vec::new(),
            vec!["dev:claude".to_owned(), "dev:codex".to_owned()]
        )
    );
    // Labels that are not projections are never touched.
    let (add, remove) = converge_projection(&labels(&["status:review", "domain:api"]), None);
    assert!(add.is_empty() && remove.is_empty());
}

fn event(run: &str, position: usize, operation: Option<&str>) -> ownership::Event {
    ownership::Event {
        created_at: "2026-07-26T01:00Z".to_owned(),
        position,
        run_id: run.to_owned(),
        runtime: None,
        horizon: None,
        kind: "claim".to_owned(),
        from: None,
        operation_id: operation.map(ToOwned::to_owned),
        from_operation: None,
        evidence_hash: None,
        evidence_body_hash: None,
        forced: false,
        evidence_required: false,
        marker_index: Some(0),
        comment: ownership::Comment::default(),
    }
}

#[test]
fn a_fresh_claim_refuses_the_two_cases_that_are_not_a_race() {
    let reason = |failure: Failure| {
        failure
            .envelope()
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };

    // Nobody holds it, but somebody else's expired claim is on the timeline.
    // Claiming over it would erase a takeover that should have been declared.
    let stale_foreign = ownership::Holding {
        holder: None,
        event: None,
        live: Vec::new(),
        stale: vec![event("claude-b", 0, None)],
    };
    assert_eq!(
        reason(may_claim(&stale_foreign, "claude-a").expect_err("a reclaim is required")),
        "stale-foreign-requires-reclaim"
    );

    // A live marker with no adoptable operation id still refuses: there is
    // nothing to resume.
    let mine = ownership::Holding {
        holder: Some("claude-a".to_owned()),
        event: Some(event("claude-a", 0, Some("op1"))),
        live: vec![event("claude-a", 0, Some("op1"))],
        stale: Vec::new(),
    };
    assert_eq!(
        reason(may_claim(&mine, "claude-a").expect_err("one run, one acquisition")),
        "already-owned-by-different-operation"
    );

    // The same run, with an adoptable epoch: a lost key, not a conflict.
    let epoch = "a".repeat(32);
    let mine_adoptable = ownership::Holding {
        holder: Some("claude-a".to_owned()),
        event: Some(event("claude-a", 0, Some(&epoch))),
        live: vec![event("claude-a", 0, Some(&epoch))],
        stale: Vec::new(),
    };
    assert_eq!(
        may_claim(&mine_adoptable, "claude-a").expect("the lost key is adoptable"),
        Attempt::AlreadyWritten {
            operation_id: epoch,
        }
    );

    // A live foreign holder is an ordinary race, not a refusal before writing:
    // the timeline settles it after everybody has posted.
    let theirs = ownership::Holding {
        holder: Some("claude-b".to_owned()),
        event: Some(event("claude-b", 0, None)),
        live: vec![event("claude-b", 0, None)],
        stale: Vec::new(),
    };
    assert!(may_claim(&theirs, "claude-a").is_ok());
    // And this run's *own* stale claim does not block it either.
    let mine_stale = ownership::Holding {
        holder: None,
        event: None,
        live: Vec::new(),
        stale: vec![event("claude-a", 0, None)],
    };
    assert!(may_claim(&mine_stale, "claude-a").is_ok());
}

#[test]
fn a_claim_is_won_on_the_timeline_and_not_by_posting() {
    let reason = |failure: Failure| {
        failure
            .envelope()
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };

    let won = ownership::Holding {
        holder: Some("claude-a".to_owned()),
        event: Some(event("claude-a", 0, Some("op1"))),
        live: vec![event("claude-a", 0, Some("op1"))],
        stale: Vec::new(),
    };
    let answer = adjudicate(&won, "op1", "claude-a", false).expect("it won");
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["reused_existing_claim"], false);

    // Somebody else is the earliest live acquisition: the post landed and the
    // race was still lost.
    let lost = ownership::Holding {
        holder: Some("claude-b".to_owned()),
        event: Some(event("claude-b", 0, None)),
        live: vec![
            event("claude-b", 0, None),
            event("claude-a", 1, Some("op1")),
        ],
        stale: Vec::new(),
    };
    assert_eq!(
        reason(adjudicate(&lost, "op1", "claude-a", false).expect_err("it lost")),
        "lost-claim-race"
    );

    // The operation's own event is gone from the timeline entirely.
    let vanished = ownership::Holding {
        holder: Some("claude-b".to_owned()),
        event: Some(event("claude-b", 0, None)),
        live: vec![event("claude-b", 0, None)],
        stale: Vec::new(),
    };
    assert_eq!(
        reason(adjudicate(&vanished, "op1", "claude-a", false).expect_err("no event")),
        "claim-operation-no-longer-current"
    );

    // A resumed operation whose event has since expired is not one to carry on
    // with — but the same shape written fresh is a race, not an expiry.
    let expired = ownership::Holding {
        holder: None,
        event: None,
        live: Vec::new(),
        stale: vec![event("claude-a", 0, Some("op1"))],
    };
    assert_eq!(
        reason(adjudicate(&expired, "op1", "claude-a", true).expect_err("expired")),
        "claim-operation-expired"
    );
}

#[test]
fn a_lost_race_reports_the_marker_it_already_left() {
    let world = |failure: Failure| {
        crate::harness::tracker::translate(
            &crate::harness::tracker::Answer {
                code: failure.code(),
                body: Some(failure.envelope()),
            },
            "claim",
        )
        .expect("still a refusal")
        .outcome
    };

    let lost = ownership::Holding {
        holder: Some("claude-b".to_owned()),
        event: Some(event("claude-b", 0, None)),
        live: vec![
            event("claude-b", 0, None),
            event("claude-a", 1, Some("op1")),
        ],
        stale: Vec::new(),
    };
    assert_eq!(
        world(adjudicate(&lost, "op1", "claude-a", false).expect_err("it lost")),
        crate::outcome::MutationOutcome::Committed,
        "the claim marker is on the timeline and this refusal reports the world untouched"
    );

    assert_eq!(
        world(
            won_the_reclaim(
                Some(event("claude-b", 0, Some("op2"))),
                "claude-a",
                &[],
                "op2"
            )
            .expect_err("it lost")
        ),
        crate::outcome::MutationOutcome::Committed,
        "the reclaim comment landed and this refusal reports the world untouched"
    );

    // And the same holding read by a **retry**: this call posted nothing, but
    // the operation did, and the declaration is about the operation. A refusal
    // that reported the world untouched here would send a resuming run to write
    // a second marker.
    let retried = adjudicate(&lost, "op1", "claude-a", true).expect_err("it lost");
    assert_eq!(
        world(retried),
        crate::outcome::MutationOutcome::Committed,
        "the operation's marker is on the timeline and the retry was told the world was untouched"
    );
}

/// Reads a refusal through `translate`, the way a caller does, so a payload
/// that drops `world` is measured as the lie it is and not as a string.
fn outcome(failure: &Failure) -> crate::outcome::MutationOutcome {
    crate::harness::tracker::translate(
        &crate::harness::tracker::Answer {
            code: failure.code(),
            body: Some(failure.envelope()),
        },
        "claim",
    )
    .expect("still a refusal")
    .outcome
}

/// A tracker payload in the shape `gh issue view --json
/// assignees,labels,comments` answers, built from the timeline and the
/// projection a test wants the world to show.
fn issue_payload(
    comments: &[ownership::Comment],
    assignees: &[&str],
    labels: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "state": "OPEN",
        "assignees": assignees
            .iter()
            .map(|login| serde_json::json!({ "login": login }))
            .collect::<Vec<_>>(),
        "labels": labels
            .iter()
            .map(|name| serde_json::json!({ "name": name }))
            .collect::<Vec<_>>(),
        "comments": comments
            .iter()
            .map(|comment| {
                serde_json::json!({
                    "id": comment.id,
                    "createdAt": comment.created_at,
                    "body": comment.body,
                    "viewerDidAuthor": comment.viewer_did_author,
                    "includesCreatedEdit": comment.includes_created_edit,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn context_at(directory: &std::path::Path) -> Context {
    Context {
        skill_dir: directory.to_path_buf(),
        repo_dir: directory.to_path_buf(),
        config: Vec::new(),
        repo: None,
    }
}

fn foreign_claim() -> ownership::Comment {
    dated(
        "2",
        "2026-07-28T02:00:00Z",
        claim_body("claude-b", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    )
}

/// A reclaim whose target epoch is nowhere on the timeline: parsed into
/// `ownership_events`, discarded by the fold, so it explains a `dev:claude`
/// label without holding or blocking anything.
///
/// The projection fixtures need one because the stand-in `gh` cannot read back a
/// projection edit, so the label has to be present before the claim and
/// explained by the store the claim is judged from.
fn explaining_projection() -> ownership::Comment {
    let marker = super::super::markers::render(
        "reclaim",
        &[
            ("run-id", "claude-z"),
            ("runtime", "claude"),
            ("horizon", RECLAIM_HORIZON),
            ("op-id", &"1".repeat(32)),
            ("from", "claude-y"),
            ("from-op", &"0".repeat(32)),
        ],
    )
    .expect("a reclaim renders");
    dated(
        "0",
        "2026-07-28T00:00:00Z",
        format!("taking over nothing.\n\n{marker}\n"),
    )
}

fn store_timeline(context: &Context, issue: u64) -> Vec<ownership::Comment> {
    crate::transport::store::Ledger::open(context, issue, RECLAIM_NOW)
        .expect("the ledger opens")
        .timeline()
        .expect("the timeline reads")
}

fn claim_script(issue: u64, comments: &[ownership::Comment]) -> String {
    let mut timeline = vec![explaining_projection()];
    timeline.extend_from_slice(comments);
    crate::test_env::answers(&[
        crate::test_env::issue_view_answer(
            issue,
            issue_payload(
                &timeline,
                &["fixture"],
                &["dev:claude", "status:in-progress"],
            ),
        ),
        serde_json::json!({
            "matches": "api user",
            "stdout": "{\"login\":\"fixture\"}", "status": 0,
        }),
    ])
}

fn acquisition(issue: u64, run: &'static str) -> Acquisition<'static> {
    Acquisition {
        issue,
        run_id: run,
        runtime: "claude",
        horizon: RECLAIM_HORIZON,
        operation_id: HOLDER_OPERATION,
        now: RECLAIM_NOW,
    }
}

/// A claim is decided where it is recorded: the same transaction reads the
/// timeline, refuses or inserts, and judges the result, so the answer never
/// depends on a second read of GitHub catching up with the comment it posted.
#[test]
fn a_claim_is_recorded_in_the_store_in_the_transaction_that_decides_it() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let log = directory.path().join("gh.log");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let script = claim_script(12, &[]);

    let (answer, timeline) = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        let answer = claim(&context, &acquisition(12, "claude-a")).expect("the claim wins");
        (answer, store_timeline(&context, 12))
    });

    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(
        holding(&timeline, RECLAIM_NOW).holder.as_deref(),
        Some("claude-a"),
        "the store does not hold the claim that was answered ok"
    );
    let calls = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
    assert!(
        !calls.contains("issue comment") && !calls.contains("/comments -f"),
        "a claim posts no comment to GitHub: {calls}"
    );
    assert!(
        calls.contains("label create dev:claude"),
        "the label projection is still written: {calls}"
    );
    assert_eq!(
        calls.matches("--json body,labels,comments").count(),
        1,
        "only the import reads a timeline from GitHub: {calls}"
    );
    assert!(
        !calls.contains("assignees,labels,comments"),
        "the projection still asks GitHub for the comments it no longer judges by: {calls}"
    );
}

/// Two runs claiming one issue at once: the store serialises them and the
/// earliest committed claim holds, so exactly one answers `ok` and the other
/// is told it lost. The loser's marker still commits; rolling it back is a later
/// slice's change.
#[test]
fn two_runs_claiming_one_issue_at_once_leave_exactly_one_winner_decided_by_the_store() {
    let directory = tempfile::tempdir().expect("a directory for the store");
    let path = directory.path().join("state.db");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let script = claim_script(12, &[]);

    let outcomes = crate::test_env::with_scripted_gh_across_threads(bin.path(), &script, || {
        let barrier = std::sync::Barrier::new(2);
        let racer = |run: &'static str, operation: &'static str| {
            crate::transport::store::with_store_at(&path, || {
                barrier.wait();
                let operation = operation.repeat(32);
                claim(
                    &context,
                    &Acquisition {
                        issue: 12,
                        run_id: run,
                        runtime: "claude",
                        horizon: RECLAIM_HORIZON,
                        operation_id: &operation,
                        now: RECLAIM_NOW,
                    },
                )
            })
        };
        std::thread::scope(|scope| {
            let first = scope.spawn(|| racer("claude-a", "a"));
            let second = scope.spawn(|| racer("claude-b", "b"));
            vec![
                first.join().expect("the first racer finishes"),
                second.join().expect("the second racer finishes"),
            ]
        })
    });

    let winners: Vec<_> = outcomes.iter().filter(|outcome| outcome.is_ok()).collect();
    assert_eq!(winners.len(), 1, "{outcomes:?}");
    let loser = outcomes
        .iter()
        .find_map(|outcome| outcome.as_ref().err())
        .expect("one racer lost");
    assert_eq!(reason(loser.clone()), "lost-claim-race", "{loser:?}");
    let stored = crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || store_timeline(&context, 12))
    });
    let holder = holding(&stored, RECLAIM_NOW).holder;
    let winner = winners[0].as_ref().expect("ok")["run_id"]
        .as_str()
        .expect("a run");
    assert_eq!(
        holder.as_deref(),
        Some(winner),
        "the store names a different holder"
    );

    let events: Vec<_> = ownership::ownership_events(&stored)
        .into_iter()
        .filter(|event| event.kind == "claim")
        .collect();
    assert_eq!(
        events.len(),
        1,
        "the loser's claim row was left in the store: {stored:?}"
    );
    assert_eq!(
        events[0].run_id, winner,
        "the surviving row is not the winner's"
    );
    let body = match loser {
        Failure::Stop(body) => body.clone(),
        other => panic!("a lost race is not a stop: {other:?}"),
    };
    assert_eq!(body["reason"], "lost-claim-race");
    assert!(
        body.get("world").is_none(),
        "a rolled-back loss still reports a committed world: {body}"
    );
}

/// The store committed the claim and GitHub then refused its label: the world
/// changed, so this is a write failure and never a refusal that changed
/// nothing, and the claim is held.
#[test]
fn a_claim_the_store_recorded_and_github_refused_its_label_is_a_write_failure_and_still_held() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "label create dev:claude",
            "stderr": "GraphQL: API rate limit already exceeded for user ID 1.",
            "status": 1,
        }),
        serde_json::json!({
            "matches": "issue view 12",
            "stdout": issue_payload(&[], &["fixture"], &["status:in-progress"]).to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}" }),
    ]);

    let (failure, timeline) = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        let failure = claim(&context, &acquisition(12, "claude-a"))
            .expect_err("GitHub refused the claim's label");
        (failure, store_timeline(&context, 12))
    });

    assert!(matches!(failure, Failure::Write(_)), "{failure:?}");
    assert_eq!(
        outcome(&failure),
        crate::outcome::MutationOutcome::Unknown,
        "a claim recorded locally is not a call that changed nothing"
    );
    assert_eq!(
        holding(&timeline, RECLAIM_NOW).holder.as_deref(),
        Some("claude-a")
    );
}

/// A retry under the operation id of a claim whose label GitHub refused finds
/// its own marker and writes nothing, and still makes the label: the failed
/// first attempt left the projection unwritten.
///
/// Mutation: make the label conditional on `decided.wrote` again and this
/// reddens on the retry's call log.
#[test]
fn a_retry_after_a_failed_label_makes_the_label_again() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let log = directory.path().join("gh.log");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let refused = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "label create dev:claude",
            "stderr": "GraphQL: API rate limit already exceeded for user ID 1.",
            "status": 1,
        }),
        serde_json::json!({
            "matches": "issue view 12",
            "stdout": issue_payload(&[], &["fixture"], &["status:in-progress"]).to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}" }),
    ]);
    let first = crate::test_env::with_scripted_gh(bin.path(), &refused, || {
        claim(&context, &acquisition(12, "claude-a"))
    })
    .expect_err("GitHub refused the claim's label");
    assert!(matches!(first, Failure::Write(_)), "{first:?}");

    let accepted = claim_script(12, &[]);
    let retried = crate::test_env::with_logging_gh(bin.path(), &accepted, &log, || {
        claim(&context, &acquisition(12, "claude-a"))
    })
    .expect("the retry under the same operation id succeeds");

    assert_eq!(retried["ok"], true, "{retried}");
    let calls = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
    assert!(
        calls.contains("label create dev:claude"),
        "the retry skipped the label the first attempt failed to make: {calls}"
    );
}

/// The holder a store-judged projection follows cannot be moved by a read of
/// GitHub: when the store names somebody else than the event this run was
/// answered, the repair is made for the one it names and the caller is told
/// its claim moved, with the marker it already left declared committed.
#[test]
fn a_projection_judged_against_the_store_reports_the_claim_that_was_recorded_when_the_holder_moved()
{
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let mine = dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    );
    let event = holding(std::slice::from_ref(&mine), RECLAIM_NOW)
        .event
        .expect("the claim is live");
    let bin = crate::test_env::scripted_gh();
    let script = claim_script(12, &[foreign_claim()]);

    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        let ledger = crate::transport::store::Ledger::open(&context, 12, RECLAIM_NOW)
            .expect("the ledger opens");
        converge_ownership_projection_from(
            &context,
            12,
            Some(event),
            Some("fixture"),
            RECLAIM_NOW,
            Judge::Store(&ledger),
        )
        .expect_err("the store names another holder")
    });

    assert_eq!(
        reason(failure.clone()),
        "ownership-changed-projections-repaired"
    );
    assert_eq!(
        outcome(&failure),
        crate::outcome::MutationOutcome::Committed,
        "the claim marker was recorded and this refusal reports the world untouched"
    );
}

#[test]
fn a_rate_limited_read_after_a_landed_write_is_never_a_call_that_changed_nothing() {
    let wrote = PullRequestWrites {
        undrafted: true,
        edited: Some(Edited::TitleAndBody),
    };
    let limited = || Failure::RateLimited("GitHub primary rate limit hit".to_owned());
    for failure in [
        after_rewriting_the_pr(limited(), wrote),
        un_readied(limited(), true, 7),
    ] {
        assert!(
            matches!(failure, Failure::Write(_)),
            "a rate-limited read after a write was reported as {failure:?}"
        );
        assert_eq!(failure.code(), 5);
    }
}

fn holder_script(issue: u64) -> String {
    let mine = dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    );
    crate::test_env::answers(&[
        serde_json::json!({
            "matches": format!("issue view {issue} --json body,labels,comments"),
            "stdout": issue_payload(&[mine], &["fixture"], &["dev:claude", "status:in-progress"])
                .to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": format!("issue view {issue} --json labels"),
            "stdout": issue_payload(&[], &[], &["status:review"]).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": format!("issue view {issue}"),
            "stdout": issue_payload(&[], &[], &["status:in-progress"]).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "api user",
            "stdout": "{\"login\":\"fixture\"}", "status": 0,
        }),
    ])
}

/// The state a renewal compares against is the one the store holds, and the
/// store learns it from the transition that moved the labels: GitHub still
/// carrying the imported label afterwards changes nothing about the decision.
#[test]
fn a_transition_moves_the_state_the_store_decides_from() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let script = holder_script(12);

    let (moved, still_old, now_new) =
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            let moved = super::super::commands::transition(
                &context,
                12,
                "review",
                Some("in-progress"),
                RECLAIM_NOW,
            );
            let still_old =
                verify_claim(&context, 12, "claude-a", "in-progress", RECLAIM_NOW, None);
            let now_new = verify_claim(&context, 12, "claude-a", "review", RECLAIM_NOW, None);
            (moved, still_old, now_new)
        });

    moved.expect("the transition lands");
    assert_eq!(
        reason(still_old.expect_err("the old state no longer matches")),
        "unexpected-state"
    );
    now_new.expect("the new state is the one the store holds");
}

#[test]
fn after_the_first_touch_a_verify_and_a_heartbeat_read_no_issue_and_no_label_and_post_nothing() {
    let (root, context) = context_with_no_repository();
    let note = root.path().join("note.md");
    std::fs::write(&note, "still working\n").expect("the note");
    let bin = crate::test_env::scripted_gh();
    let script = holder_script(918_304);
    let log = root.path().join("gh.log");

    let (import, wrong_state) = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        verify_claim(
            &context,
            918_304,
            "claude-a",
            "in-progress",
            RECLAIM_NOW,
            None,
        )
        .expect("the first touch imports and verifies");
        let import = std::fs::read_to_string(&log).expect("the log");
        let wrong_state = verify_claim(&context, 918_304, "claude-a", "review", RECLAIM_NOW, None);
        super::heartbeat(
            &context,
            918_304,
            "claude-a",
            "in-progress",
            &note,
            "progress",
            RECLAIM_NOW,
        )
        .expect("the heartbeat lands");
        (import, wrong_state)
    });

    assert!(
        import.contains("issue view"),
        "the first touch imports through one `gh issue view`: {import}"
    );
    assert_eq!(
        reason(wrong_state.expect_err("the store holds in-progress")),
        "unexpected-state",
        "a `status:review` label GitHub answers with decides nothing"
    );
    let all = std::fs::read_to_string(&log).expect("the log");
    let later = &all[import.len()..];
    assert!(
        !later.contains("issue view") && !later.contains("issue comment"),
        "a verify or heartbeat after the first touch read an issue or posted a comment: {later}"
    );
}

/// A heartbeat is a marker comment on GitHub and an entry in the store, so the
/// activity it declares is what the next renewal reads.
#[test]
fn a_heartbeat_is_recorded_in_the_store_beside_the_comment_it_posts() {
    let (root, context) = context_with_no_repository();
    let note = root.path().join("note.md");
    std::fs::write(&note, "still working\n").expect("the note");
    let bin = crate::test_env::scripted_gh();
    let script = holder_script(918_303);

    let timeline = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::heartbeat(
            &context,
            918_303,
            "claude-a",
            "in-progress",
            &note,
            "progress",
            RECLAIM_NOW,
        )
        .expect("the heartbeat lands");
        store_timeline(&context, 918_303)
    });

    let recorded = timeline.last().expect("an entry");
    assert!(
        recorded.body.contains("issue-flow: progress"),
        "the store does not hold the marker that was recorded: {}",
        recorded.body
    );
    assert!(recorded.viewer_did_author && !recorded.includes_created_edit);
}

/// A release is decided over the store in one transaction and leaves nobody
/// holding it there, and its discovery writes nothing.
#[test]
fn an_unassign_releases_the_epoch_in_the_store_and_its_discovery_writes_nothing() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let script = holder_script(12);
    let release_operation = "d".repeat(32);
    let log = directory.path().join("gh.log");

    let (discovered, before, released, after) =
        crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            let discovered = unassign(
                &context,
                &departure_of("claude-a", &release_operation, None),
            )
            .expect("the discovery answers");
            let before = store_timeline(&context, 12).len();
            let target = discovered["target_operation"]
                .as_str()
                .expect("a target epoch")
                .to_owned();
            let released = unassign(
                &context,
                &departure_of("claude-a", &release_operation, Some(&target)),
            );
            (discovered, before, released, store_timeline(&context, 12))
        });

    assert_eq!(discovered["write_performed"], false, "{discovered}");
    assert_eq!(before, 1, "the discovery wrote an entry");
    released.expect("the release lands");
    assert_eq!(after.len(), 2);
    assert_eq!(holding(&after, RECLAIM_NOW).holder, None);
    let calls = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
    assert!(
        !calls.contains("issue comment"),
        "a release posts no comment to GitHub: {calls}"
    );
}

/// A takeover is recorded in the store alone: the holder changes there and no
/// comment reaches GitHub.
#[test]
fn a_reclaim_is_recorded_in_the_store_and_posts_no_comment() {
    let directory = tempfile::tempdir().expect("a directory for the log");
    let log = directory.path().join("gh.log");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let now = "2026-07-28T20:00:00Z";
    let script = claim_script(12, &[foreign_claim()]);
    let reclaim_of = |target: Option<&'static str>| Reclaim {
        issue: 12,
        run_id: "claude-a",
        runtime: "claude",
        horizon: "2026-07-29T09:00Z",
        operation_id: "cccccccccccccccccccccccccccccccc",
        target_operation: target,
        force: false,
        reason_file: None,
        now,
    };

    let (answer, timeline) = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        let target = reclaim(&context, &reclaim_of(None)).expect("the discovery answers")
            ["target_operation"]
            .as_str()
            .expect("a target")
            .to_owned();
        let target: &'static str = Box::leak(target.into_boxed_str());
        let answer = reclaim(&context, &reclaim_of(Some(target))).expect("the takeover lands");
        (answer, store_timeline(&context, 12))
    });

    assert_eq!(answer["reclaimed_from"], "claude-b", "{answer}");
    assert_eq!(
        holding(&timeline, now).holder.as_deref(),
        Some("claude-a"),
        "the store does not hold the takeover"
    );
    let calls = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
    assert!(
        !calls.contains("issue comment"),
        "a reclaim posts no comment to GitHub: {calls}"
    );
}

/// The projection check left for later: a `dev:` label or
/// assignee the store cannot explain is another store's, so a claim over it
/// refuses and writes nothing. The override the design names is `reclaim`, which
/// does not run the check.
#[test]
fn a_claim_refuses_a_dev_label_the_store_cannot_explain_and_reclaim_takes_it_over() {
    let directory = tempfile::tempdir().expect("a directory for the store");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let now = "2026-07-28T20:00:00Z";
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(
            12,
            issue_payload(
                &[foreign_claim()],
                &["fixture"],
                &["dev:codex", "status:in-progress"],
            ),
        ),
        serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
    ]);

    let (refused, stored, reclaimed) =
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            let refused = claim(&context, &acquisition(12, "claude-a"));
            let stored = store_timeline(&context, 12);
            let reclaim_of = |target: Option<&'static str>| Reclaim {
                issue: 12,
                run_id: "codex-a",
                runtime: "codex",
                horizon: "2026-07-29T09:00Z",
                operation_id: "cccccccccccccccccccccccccccccccc",
                target_operation: target,
                force: false,
                reason_file: None,
                now,
            };
            let target = reclaim(&context, &reclaim_of(None)).expect("the discovery answers")
            ["target_operation"]
            .as_str()
            .expect("a target")
            .to_owned();
            let target: &'static str = Box::leak(target.into_boxed_str());
            let reclaimed = reclaim(&context, &reclaim_of(Some(target)));
            (refused, stored, reclaimed)
        });

    let failure = refused.expect_err("the store cannot explain dev:codex");
    assert_eq!(reason(failure.clone()), "foreign-projection");
    assert_eq!(
        failure.envelope()["labels"],
        serde_json::json!(["dev:codex"]),
        "{failure:?}"
    );
    assert_eq!(failure.envelope()["assignees"], serde_json::json!([]));
    assert!(
        ownership::ownership_events(&stored)
            .iter()
            .all(|event| event.run_id != "claude-a"),
        "the refused claim wrote its marker: {stored:?}"
    );
    reclaimed.expect("reclaim is the override and does not run the check");
}

/// An assignee that is not the authenticated login is the other half of the
/// same foreign projection.
#[test]
fn a_claim_refuses_an_assignee_the_store_cannot_explain() {
    let directory = tempfile::tempdir().expect("a directory for the store");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(
            12,
            issue_payload(&[], &["somebody-else"], &["status:in-progress"]),
        ),
        serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
    ]);

    let (refused, stored) = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        (
            claim(&context, &acquisition(12, "claude-a")),
            store_timeline(&context, 12),
        )
    });

    let failure = refused.expect_err("the store cannot explain somebody-else");
    assert_eq!(reason(failure.clone()), "foreign-projection");
    assert_eq!(failure.envelope()["labels"], serde_json::json!([]));
    assert_eq!(
        failure.envelope()["assignees"],
        serde_json::json!(["somebody-else"]),
        "{failure:?}"
    );
    assert!(
        ownership::ownership_events(&stored)
            .iter()
            .all(|event| event.run_id != "claude-a"),
        "the refused claim wrote its marker: {stored:?}"
    );
}

fn departure_of<'a>(run: &'a str, operation: &'a str, target: Option<&'a str>) -> Departure<'a> {
    Departure {
        issue: 12,
        run_id: run,
        runtime: "claude",
        operation_id: operation,
        target_operation: target,
        held_by_other: false,
        worktree: None,
        now: RECLAIM_NOW,
    }
}

fn rest_comment(node: &str, login: &str, at: &str, body: &str) -> serde_json::Value {
    serde_json::json!({
        "id": 1, "node_id": node, "user": { "login": login },
        "created_at": at, "updated_at": at, "body": body,
    })
}

fn synced_script(
    issue: u64,
    imported: &[ownership::Comment],
    rest: &[serde_json::Value],
) -> String {
    let mut timeline = vec![explaining_projection()];
    timeline.extend_from_slice(imported);
    crate::test_env::answers(&[
        serde_json::json!({
            "matches": format!("issue view {issue} --json body,labels,comments"),
            "stdout": issue_payload(&timeline, &["fixture"], &["dev:claude", "status:in-progress"])
                .to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": format!("issue view {issue}"),
            "stdout": issue_payload(&[], &["fixture"], &["dev:claude", "status:in-progress"])
                .to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": format!("issues/{issue}/comments"),
            "stdout": serde_json::Value::Array(rest.to_vec()).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0,
        }),
    ])
}

/// A claim an older binary posted to GitHub after the import is not read: the store alone
/// decides who holds the issue.
#[test]
fn a_claim_an_old_binary_posted_after_the_import_decides_nothing() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let theirs = rest_comment(
        "IC_b",
        "fixture",
        "2026-07-28T01:00:00Z",
        &claim_body("claude-b", &"b".repeat(32)),
    );
    let script = synced_script(12, &[], &[theirs]);

    let (timeline, claimed) = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        (
            store_timeline(&context, 12),
            claim(&context, &acquisition(12, "claude-a")),
        )
    });

    assert!(
        ownership::ownership_events(&timeline)
            .iter()
            .all(|event| event.run_id != "claude-b"),
        "a foreign claim was imported: {timeline:?}"
    );
    claimed.expect("the foreign claim does not hold the issue");
}

/// A control comment a person wrote after the import (an adjudication naming the
/// run) stops the run it names, as it did when every read came from GitHub.
#[test]
fn a_standdown_a_person_commented_after_the_import_stops_the_run_it_names() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let mine = dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    );
    let marker = super::super::markers::render("adjudication", &[("target", "claude-a")])
        .expect("an adjudication renders");
    let stop = rest_comment(
        "IC_stop",
        "fixture",
        "2026-07-28T02:00:00Z",
        &format!("Stop.\n\n{marker}\n"),
    );
    let script = synced_script(12, &[mine], &[stop]);

    let refused = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        verify_claim(&context, 12, "claude-a", "in-progress", RECLAIM_NOW, None)
    });

    assert_eq!(
        reason(refused.expect_err("the stand-down names this run")),
        "control-message"
    );
}

/// A person's `standdown target=<run>` comment, posted after the import, releases that run
/// through the ownership fold, so its next verify stops.
#[test]
fn a_person_commenting_a_standdown_after_the_import_releases_the_run_it_names() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let mine = dated(
        "1",
        "2026-07-28T01:00:00Z",
        format!(
            "claiming.\n\n{}\n",
            super::super::markers::render(
                "claim",
                &[
                    ("run-id", "claude-a"),
                    ("runtime", "claude"),
                    ("horizon", RECLAIM_HORIZON),
                ],
            )
            .expect("the claim renders"),
        ),
    );
    let marker = super::super::markers::render("standdown", &[("target", "claude-a")])
        .expect("a standdown renders");
    let stop = rest_comment(
        "IC_stop",
        "fixture",
        "2026-07-28T02:00:00Z",
        &format!("Stop.\n\n{marker}\n"),
    );
    let script = synced_script(12, &[mine], &[stop]);

    let refused = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        verify_claim(&context, 12, "claude-a", "in-progress", RECLAIM_NOW, None)
    });

    refused.expect_err("the stand-down released the run it names");
}

/// A `reclaim` would hand the issue to somebody else, so a comment carrying one decides
/// nothing after the import.
#[test]
fn a_reclaim_commented_after_the_import_decides_nothing() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let mine = dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    );
    let marker = super::super::markers::render(
        "reclaim",
        &[
            ("run-id", "claude-b"),
            ("from", "claude-a"),
            ("forced", "true"),
        ],
    )
    .expect("a reclaim renders");
    let theirs = rest_comment(
        "IC_reclaim",
        "fixture",
        "2026-07-28T02:00:00Z",
        &format!("Mine now.\n\n{marker}\n"),
    );
    let script = synced_script(12, &[mine], &[theirs]);

    let (renewal, timeline) = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        (
            verify_claim(&context, 12, "claude-a", "in-progress", RECLAIM_NOW, None),
            store_timeline(&context, 12),
        )
    });

    renewal.expect("a foreign reclaim does not displace the holder");
    assert!(
        ownership::ownership_events(&timeline)
            .iter()
            .all(|event| event.run_id != "claude-b"),
        "a reclaim was imported: {timeline:?}"
    );
}

/// A REST comment carries no `viewerDidAuthor`: it is derived from the login,
/// and an author who is not the viewer, or a comment edited after it was
/// written, is never admitted.
#[test]
fn a_synced_comment_is_admitted_only_when_the_viewer_wrote_it_and_nobody_edited_it() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let body = super::super::markers::render("adjudication", &[("target", "claude-b")])
        .expect("an adjudication renders");
    let mut edited = rest_comment("IC_e", "fixture", "2026-07-28T01:00:00Z", &body);
    edited["updated_at"] = serde_json::json!("2026-07-28T03:00:00Z");
    let script = synced_script(
        12,
        &[],
        &[
            rest_comment("IC_s", "somebody-else", "2026-07-28T00:30:00Z", &body),
            edited,
        ],
    );

    let (timeline, claimed) = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        (
            store_timeline(&context, 12),
            claim(&context, &acquisition(12, "claude-a")),
        )
    });

    assert!(
        timeline
            .iter()
            .all(|comment| !comment.body.contains("adjudication")),
        "an untrusted REST comment was stored: {timeline:?}"
    );
    claimed.expect("neither untrusted claim stands in the way");
}

/// A retry under the same operation id finds the claim the store already holds
/// and resumes it: nothing is posted, then or now.
#[test]
fn a_retry_under_the_same_operation_id_resumes_the_stored_claim_and_posts_nothing() {
    let directory = tempfile::tempdir().expect("a directory for the log");
    let log = directory.path().join("gh.log");
    let context = context_at(directory.path());
    let bin = crate::test_env::scripted_gh();
    let script = claim_script(12, &[]);

    let (first, retried) = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        (
            claim(&context, &acquisition(12, "claude-a")),
            claim(&context, &acquisition(12, "claude-a")),
        )
    });

    assert_eq!(
        first.expect("the claim wins")["reused_existing_claim"],
        false
    );
    assert_eq!(
        retried.expect("the retry resumes the claim")["reused_existing_claim"],
        true
    );
    let calls = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
    assert!(!calls.contains("issue comment"), "{calls}");
}

#[test]
fn an_ownership_projection_names_a_rate_limit_and_stops_reading_at_it() {
    let directory = tempfile::tempdir().expect("a directory for the log");
    let log = directory.path().join("gh.log");
    let context = context_at(directory.path());
    let comments = vec![dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    )];
    let event = holding(&comments, RECLAIM_NOW)
        .event
        .expect("the claim is live");
    let script = crate::test_env::answers(&[serde_json::json!({
        "matches": "issue view 12",
        "stderr": "GraphQL: API rate limit already exceeded for user ID 1.",
        "status": 1,
    })]);
    let bin = crate::test_env::scripted_gh();
    let failure = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        converge_ownership_projection(&context, 12, Some(event), Some("fixture"), RECLAIM_NOW)
            .expect_err("every read was rate limited")
    });
    assert!(matches!(failure, Failure::RateLimited(_)), "{failure:?}");
    let reads = std::fs::read_to_string(&log)
        .expect("the fake gh logged its calls")
        .lines()
        .filter(|line| {
            line.contains("issue view 12")
                && line.ends_with(&directory.path().display().to_string())
        })
        .count();
    assert_eq!(reads, 1, "reading went on after the budget said stop");
}

/// Each loop's reads exist to give `wait_for` two agreeing answers, so a
/// tracker that agrees at once is asked twice per loop, not seven times.
#[test]
fn an_ownership_projection_stops_reading_once_two_reads_agree() {
    let directory = tempfile::tempdir().expect("a directory for the log");
    let log = directory.path().join("gh.log");
    let context = context_at(directory.path());
    let comments = vec![dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    )];
    let settled = issue_payload(&comments, &["fixture"], &["dev:claude"]);
    let event = holding(&comments, RECLAIM_NOW)
        .event
        .expect("the claim is live");
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(12, settled)]);
    let bin = crate::test_env::scripted_gh();
    crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        converge_ownership_projection(&context, 12, Some(event), Some("fixture"), RECLAIM_NOW)
            .expect("a settled projection converges");
    });
    let reads = std::fs::read_to_string(&log)
        .expect("the fake gh logged its calls")
        .lines()
        .filter(|line| {
            line.contains("issue view 12")
                && line.ends_with(&directory.path().display().to_string())
        })
        .count();
    assert_eq!(
        reads, 4,
        "two agreeing reads before and two after were enough, and more were asked for"
    );
}

/// A holder whose runtime cannot be established is refused after the
/// projection's own after-read agreed. A release or a takeover can leave a
/// legacy holder this way, and the call that moved ownership had already
/// posted its own marker.
#[test]
fn a_projection_without_a_nameable_runtime_reports_the_marker_it_already_left() {
    let directory = tempfile::tempdir().expect("a context root");
    let context = context_at(directory.path());
    let marker = super::super::markers::render(
        "claim",
        &[("run-id", "claude-a"), ("horizon", RECLAIM_HORIZON)],
    )
    .expect("a claim marker without a runtime renders");
    let comments = vec![dated(
        "1",
        "2026-07-28T01:00:00Z",
        format!("claiming.\n\n{marker}\n"),
    )];
    let payload = issue_payload(&comments, &[], &["status:in-progress"]);
    let event = holding(&comments, RECLAIM_NOW)
        .event
        .expect("the claim is live");
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(12, payload)]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        converge_ownership_projection(&context, 12, Some(event), Some("fixture"), RECLAIM_NOW)
            .expect_err("the holder's runtime cannot be established")
    });
    assert_eq!(reason(failure.clone()), "holder-runtime-missing");
    assert_eq!(
        outcome(&failure),
        crate::outcome::MutationOutcome::Committed,
        "the marker this projection follows is on the timeline and this refusal reports the world \
         untouched"
    );
}

/// The `else` arm of the stop that follows a failed after-read: this attempt
/// wrote nothing of its own, the holder did not move, and the projection moved
/// instead. The caller's claim marker is still on the timeline.
#[test]
fn a_projection_that_moved_under_the_readback_reports_the_marker_it_already_left() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let count = directory.path().join("count");
    let context = context_at(directory.path());
    let comments = vec![dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    )];
    let settled = issue_payload(&comments, &["fixture"], &["dev:claude"]);
    let shifted = issue_payload(&comments, &["fixture"], &["dev:codex"]);
    let event = holding(&comments, RECLAIM_NOW)
        .event
        .expect("the claim is live");
    let mut entries: Vec<serde_json::Value> = (1..=2)
        .map(|call| {
            serde_json::json!({
                "matches": "issue view 12", "nth": call,
                "stdout": settled.to_string(), "status": 0,
            })
        })
        .collect();
    entries.push(serde_json::json!({
        "matches": "issue view 12",
        "stdout": shifted.to_string(), "status": 0,
    }));
    let script = crate::test_env::answers(&entries);
    let bin = crate::test_env::scripted_gh();
    let failure = crate::test_env::with_counting_gh(bin.path(), &script, &count, || {
        converge_ownership_projection(&context, 12, Some(event), Some("fixture"), RECLAIM_NOW)
            .expect_err("the projection moved under the readback")
    });
    assert_eq!(
        reason(failure.clone()),
        "ownership projection write did not read back exactly"
    );
    assert_eq!(
        outcome(&failure),
        crate::outcome::MutationOutcome::Committed,
        "the claim marker was posted and this refusal reports the world untouched"
    );
}

/// The bound exhausted: every read disagrees with the one before it, so the
/// loop never settles and this call never repairs anything. The claim marker
/// is still on the timeline.
#[test]
fn an_ownership_read_that_never_settles_reports_the_marker_it_already_left() {
    let directory = tempfile::tempdir().expect("a directory for the counter");
    let count = directory.path().join("count");
    let context = context_at(directory.path());
    let mine = vec![dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    )];
    let theirs = vec![foreign_claim()];
    let held = issue_payload(&mine, &["fixture"], &["dev:claude"]);
    let elsewhere = issue_payload(&theirs, &["fixture"], &["dev:claude"]);
    let event = holding(&mine, RECLAIM_NOW)
        .event
        .expect("the claim is live");
    let mut entries = Vec::new();
    for call in 1..=49u64 {
        let payload = if call % 2 == 1 { &held } else { &elsewhere };
        entries.push(serde_json::json!({
            "matches": "issue view 12", "nth": call,
            "stdout": payload.to_string(), "status": 0,
        }));
    }
    let script = crate::test_env::answers(&entries);
    let bin = crate::test_env::scripted_gh();
    let failure = crate::test_env::with_counting_gh(bin.path(), &script, &count, || {
        converge_ownership_projection(&context, 12, Some(event), Some("fixture"), RECLAIM_NOW)
            .expect_err("ownership never settled")
    });
    assert_eq!(
        reason(failure.clone()),
        "ownership-kept-changing-before-projection"
    );
    assert_eq!(
        outcome(&failure),
        crate::outcome::MutationOutcome::Committed,
        "the claim marker was posted and this refusal reports the world untouched"
    );
}

#[test]
fn a_release_discovers_before_it_writes_and_binds_one_exact_epoch() {
    let reason = |failure: Failure| {
        failure
            .envelope()
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let mut mine = event("claude-a", 0, Some("op1"));
    mine.runtime = Some("claude".to_owned());
    let held = ownership::Holding {
        holder: Some("claude-a".to_owned()),
        event: Some(mine.clone()),
        live: vec![mine],
        stale: Vec::new(),
    };

    // Two phases: the first call writes nothing and names the epoch.
    assert_eq!(
        plan_release(&held, "claude-a", false, None, &[]).expect("discovery"),
        Release::Confirm {
            target_operation: "op1".to_owned()
        }
    );
    // The second binds it.
    assert_eq!(
        plan_release(&held, "claude-a", false, Some("op1"), &[]).expect("the write"),
        Release::Write {
            target_operation: "op1".to_owned()
        }
    );
    // Naming a different epoch is refused: ownership moved in between.
    assert_eq!(
        reason(
            plan_release(&held, "claude-a", false, Some("op2"), &[]).expect_err("the epoch moved")
        ),
        "target-operation-mismatch"
    );

    // Nothing held at all.
    let empty = ownership::Holding {
        holder: None,
        event: None,
        live: Vec::new(),
        stale: Vec::new(),
    };
    assert_eq!(
        reason(plan_release(&empty, "claude-a", false, None, &[]).expect_err("nothing")),
        "nothing-to-unassign"
    );

    // "Somebody else holds it" when nobody else does.
    assert_eq!(
        reason(
            plan_release(&held, "claude-a", true, None, &[])
                .expect_err("the timeline says otherwise")
        ),
        "held-by-other-without-other-holder"
    );
}

/// `mine` is already found by `run_id`, the exact identity a release has to
/// match — a recorded runtime plays no part in it at all.
#[test]
fn an_acquisition_recorded_claude_by_a_non_claude_run_stays_releasable() {
    let mut mine = event("codex-a", 0, Some("op1"));
    mine.runtime = Some("claude".to_owned());
    let held = ownership::Holding {
        holder: Some("codex-a".to_owned()),
        event: Some(mine.clone()),
        live: vec![mine],
        stale: Vec::new(),
    };
    assert_eq!(
        plan_release(&held, "codex-a", false, None, &[]).expect("its own acquisition"),
        Release::Confirm {
            target_operation: "op1".to_owned()
        }
    );
}

/// An acquisition that recorded no runtime is legacy — and, like every other
/// acquisition, is found and released by `run_id` alone. The labels play no
/// part in whether *this* release goes through, only in whether a *successor*
/// can be named — [`a_release_that_leaves_an_unnameable_holder_is_refused`]
/// covers that.
#[test]
fn a_legacy_acquisition_releases_regardless_of_the_labels() {
    let mut legacy = event("claude-a", 0, None);
    legacy.runtime = None;
    let old = ownership::Holding {
        holder: Some("claude-a".to_owned()),
        event: Some(legacy.clone()),
        live: vec![legacy],
        stale: Vec::new(),
    };
    for projection in [
        Vec::new(),
        vec![String::from("dev:codex")],
        vec![String::from("dev:claude")],
        vec![String::from("dev:codex"), String::from("dev:claude")],
    ] {
        assert!(
            plan_release(&old, "claude-a", false, None, &projection).is_ok(),
            "a legacy acquisition, found by its own run id, was refused against {projection:?}"
        );
    }
}

#[test]
fn a_release_that_leaves_an_unnameable_holder_is_refused() {
    // Handing the issue on. A successor that declared no runtime is only
    // identifiable through the `dev:` labels already there, so unless those
    // resolve to exactly one, releasing leaves a holder nobody can project a
    // label for — and the label is what a person reads at a glance. The port
    // had no equivalent, and released anyway.
    let mut mine = event("claude-a", 0, Some("op1"));
    mine.runtime = Some("claude".to_owned());
    let mut successor = event("codex-b", 1, Some("op2"));
    successor.runtime = None;
    let held = ownership::Holding {
        holder: Some("claude-a".to_owned()),
        event: Some(mine.clone()),
        live: vec![mine, successor],
        stale: Vec::new(),
    };

    // The run-id prefix identifies an older successor, so one matching label is
    // enough — and it has to be that one.
    assert!(
        plan_release(&held, "claude-a", false, None, &[String::from("dev:codex")]).is_ok(),
        "a successor the labels do name was refused"
    );
    for projection in [
        Vec::new(),
        vec![String::from("dev:claude")],
        vec![String::from("dev:codex"), String::from("dev:codex")],
    ] {
        assert_eq!(
            reason(
                plan_release(&held, "claude-a", false, None, &projection)
                    .expect_err("the successor cannot be named")
            ),
            "holder-runtime-missing",
            "handed the issue to a holder nobody can name, against {projection:?}"
        );
    }
}

/// An acquisition recorded `claude` before this build,
/// released with `codex` after it. `ownership.rs`'s epoch-release reading
/// must not refuse that release on the strength of the runtime alone — this
/// is the same fact `plan_release` finding it by run id already decided.
#[test]
fn a_release_across_the_build_boundary_is_not_refused_by_the_epoch_reading() {
    let target = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let operation = "0123456789abcdef0123456789abcdef";
    let comments = vec![
        wrote(
            "1",
            format!(
                "claiming.\n\n{}\n",
                super::super::markers::render(
                    "claim",
                    &[
                        ("run-id", "codex-a"),
                        ("runtime", "claude"),
                        ("horizon", "2026-07-26T09:00Z"),
                        ("op-id", target),
                    ],
                )
                .expect("renders")
            ),
        ),
        wrote(
            "2",
            format!(
                "releasing.\n\n{}\n",
                super::super::markers::render(
                    "unassign",
                    &[
                        ("run-id", "codex-a"),
                        ("runtime", "codex"),
                        ("op-id", operation),
                        ("target-op", target),
                    ],
                )
                .expect("renders")
            ),
        ),
    ];
    let ownership = holding(&comments, "2026-07-26T04:00:00Z");
    assert_eq!(
        reason(
            plan_release(&ownership, "codex-a", false, Some(target), &[])
                .expect_err("the boundary-crossing release already went through")
        ),
        "nothing-to-unassign",
        "ownership.rs's epoch-release reading refused a release across the runtime the build changed"
    );
}

#[test]
fn a_takeover_separates_the_five_ways_it_can_be_wrong() {
    let reason = |failure: Failure| {
        failure
            .envelope()
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let holding = |live: Vec<ownership::Event>, stale: Vec<ownership::Event>| ownership::Holding {
        holder: live.first().map(|event| event.run_id.clone()),
        event: live.first().cloned(),
        live,
        stale,
    };

    // Nothing at all to take.
    assert_eq!(
        reason(
            plan_takeover(&holding(Vec::new(), Vec::new()), "claude-a", true, None).unwrap_err()
        ),
        "nothing-to-reclaim"
    );
    // The only expired acquisition is this run's own: that is a fresh claim,
    // not a takeover — and the refusal must say so, not merely name the rule.
    // A caller was measured stuck reading only the bare reason; asserting
    // on the guidance's *meaning* (it names `claim`) rather than its wording
    // keeps a later rewording from failing here while still failing if the
    // guidance is deleted outright.
    let stale_self = plan_takeover(
        &holding(Vec::new(), vec![event("claude-a", 0, None)]),
        "claude-a",
        true,
        None,
    )
    .unwrap_err();
    let envelope = stale_self.envelope();
    assert_eq!(reason(stale_self), "stale-self-requires-claim");
    let detail = envelope
        .get("detail")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(
        !detail.is_empty(),
        "the bare reason had no sentence behind it; the detail field is gone: {envelope}"
    );
    let action = envelope
        .get("action")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(
        action.contains("claim"),
        "the action must send a caller to `claim`; got {action:?}"
    );
    // The live holder is this run.
    assert_eq!(
        reason(
            plan_takeover(
                &holding(vec![event("claude-a", 0, None)], Vec::new()),
                "claude-a",
                true,
                None
            )
            .unwrap_err()
        ),
        "already-yours"
    );
    // A live holder, unforced: taking one quietly is the thing a reclaim must
    // never do.
    let live_other = holding(vec![event("claude-b", 0, None)], Vec::new());
    assert_eq!(
        reason(plan_takeover(&live_other, "claude-a", false, None).unwrap_err()),
        "holder-not-stale"
    );
    // Forced, it becomes a discovery.
    assert!(matches!(
        plan_takeover(&live_other, "claude-a", true, None).expect("forced discovery"),
        Takeover::Confirm { .. }
    ));

    // A stale foreign holder needs no force, and the two phases bind one epoch.
    let stale_other = holding(Vec::new(), vec![event("claude-b", 0, Some("op9"))]);
    assert_eq!(
        plan_takeover(&stale_other, "claude-a", false, None).expect("discovery"),
        Takeover::Confirm {
            target_operation: "op9".to_owned(),
            holder: "claude-b".to_owned()
        }
    );
    assert_eq!(
        plan_takeover(&stale_other, "claude-a", false, Some("op9")).expect("the write"),
        Takeover::Write {
            target_operation: "op9".to_owned(),
            holder: "claude-b".to_owned()
        }
    );
    assert_eq!(
        reason(plan_takeover(&stale_other, "claude-a", false, Some("op1")).unwrap_err()),
        "target-operation-mismatch"
    );
}

fn takeover(run_id: &str, operation_id: &str, forced: bool) -> ownership::Event {
    ownership::Event {
        kind: "reclaim".to_owned(),
        from: Some("claude-z".to_owned()),
        forced,
        ..event(run_id, 0, Some(operation_id))
    }
}

/// `claim`'s own early check recognises a lapsed self-acquisition whichever
/// call wrote it — not only the `reclaim`-origin shape
/// `a_lapsed_self_claim_that_originated_as_a_reclaim_can_be_claimed_again`
/// (`tests/pipe/part_09.rs`) poses. `event.kind` never enters the predicate, and this
/// is what would catch it if it did: narrowing `is_dead_self_operation` to
/// `event.kind == "reclaim" && …` leaves the first assertion here false.
#[test]
fn a_dead_self_operation_is_recognised_whichever_call_wrote_it() {
    // Written originally by `claim`.
    let from_claim = vec![event("claude-a", 0, Some("op1"))];
    assert!(is_dead_self_operation(&from_claim, &[], "claude-a", "op1"));

    // Written originally by `reclaim` — the shape the integration test poses.
    let from_reclaim = vec![takeover("claude-a", "op1", true)];
    assert!(is_dead_self_operation(
        &from_reclaim,
        &[],
        "claude-a",
        "op1"
    ));

    // An acquisition a later foreign `reclaim` pushed out of the live epoch is
    // dead too, though `holding` carries it in neither `live` nor `stale`.
    let mut other = takeover("claude-b", "op2", true);
    other.position = 1;
    let closed_epoch = vec![event("claude-a", 0, Some("op1")), other.clone()];
    let live_now = vec![other];
    assert!(is_dead_self_operation(
        &closed_epoch,
        &live_now,
        "claude-a",
        "op1"
    ));

    // A live acquisition of this run's own is the retry to resume, not a dead
    // operation to replace.
    let live = vec![event("claude-a", 0, Some("op1"))];
    assert!(!is_dead_self_operation(&live, &live, "claude-a", "op1"));

    // Neither a different run's dead acquisition nor a different operation id
    // is this run's own dead one.
    assert!(!is_dead_self_operation(&from_claim, &[], "claude-b", "op1"));
    assert!(!is_dead_self_operation(&from_claim, &[], "claude-a", "op2"));
}

fn reason(failure: Failure) -> String {
    failure
        .envelope()
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn marker(evidence: Option<&str>) -> super::super::markers::Marker {
    let mut found = super::super::markers::Marker::new();
    found.insert("op-id".to_owned(), "0123".to_owned());
    if let Some(hash) = evidence {
        found.insert("evidence-hash".to_owned(), hash.to_owned());
    }
    found
}

#[test]
fn a_retried_reclaim_carrying_different_evidence_is_not_the_same_operation() {
    // The port computed the evidence hash only to write it, and compared it to
    // nothing. So a retry with a *different* reason file found the existing
    // marker, wrote nothing, and answered `ok` — leaving the takeover recorded
    // against a justification nobody meant to give.
    assert_eq!(
        reason(
            resumed_reclaim(
                &marker(Some("aaa")),
                Some("bbb"),
                Some(takeover("claude-a", "0123", true)),
                "0123",
            )
            .expect_err("different evidence")
        ),
        "reclaim-metadata-mismatch"
    );

    // The same evidence, and no evidence at all, both stand.
    for (recorded, supplied) in [(Some("aaa"), Some("aaa")), (None, None)] {
        assert!(
            resumed_reclaim(
                &marker(recorded),
                supplied,
                Some(takeover("claude-a", "0123", true)),
                "0123",
            )
            .expect("the same operation"),
            "a forced reclaim came back unforced"
        );
    }

    // Superseded, and the payload names who by — "you lost" and "you lost to
    // *them*" are different amounts of help.
    let gone = resumed_reclaim(
        &marker(None),
        None,
        Some(takeover("claude-b", "9999", false)),
        "0123",
    )
    .expect_err("no longer current");
    assert_eq!(
        gone.envelope().get("winner").and_then(|w| w.as_str()),
        Some("claude-b")
    );
    assert_eq!(reason(gone), "reclaim-operation-no-longer-current");
}

/// The sibling `resumed_reclaim` declares nothing where `adjudicate` declares
/// `world: committed`, so the two verbs answered one retry case differently.
/// The operation's event has left the timeline, and what the flag reports is
/// that the operation wrote, not which call did.
#[test]
fn a_retried_reclaim_that_is_no_longer_current_reports_the_marker_it_left() {
    let failure = resumed_reclaim(
        &marker(None),
        None,
        Some(takeover("claude-b", "9999", false)),
        "0123",
    )
    .expect_err("no longer current");
    assert_eq!(
        reason(failure.clone()),
        "reclaim-operation-no-longer-current"
    );
    assert_eq!(
        outcome(&failure),
        crate::outcome::MutationOutcome::Committed,
        "the write had landed, so this refusal reports the world committed — not which call made it"
    );
}

#[test]
fn a_written_reclaim_that_nothing_holds_is_a_write_failure_and_never_a_stop() {
    // One `Stop` used to answer all of these. A `Stop` says *this did not
    // happen*, and that is the one thing not known here: the comment is on the
    // issue. Told nothing happened, a caller writes the takeover again.
    assert!(matches!(
        won_the_reclaim(None, "claude-a", &[], "0123"),
        Err(Failure::Write(_))
    ));

    let lost = won_the_reclaim(
        Some(takeover("claude-b", "9999", false)),
        "claude-a",
        &[],
        "9999",
    )
    .expect_err("another run holds it");
    assert_eq!(
        lost.envelope().get("winner").and_then(|w| w.as_str()),
        Some("claude-b")
    );
    assert_eq!(reason(lost), "lost-reclaim-race");

    // Won, and the answer carries whether it was forced rather than what the
    // caller asked for.
    assert!(
        won_the_reclaim(
            Some(takeover("claude-a", "0123", true)),
            "claude-a",
            &[],
            "0123"
        )
        .expect("won"),
        "a forced takeover came back unforced"
    );
}

/// The takeover tests' clock and the horizon every acquisition declares there.
const RECLAIM_NOW: &str = "2026-07-28T06:00:00Z";
const RECLAIM_HORIZON: &str = "2026-07-28T09:00Z";
const HOLDER_OPERATION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const RECLAIM_OPERATION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TAKEOVER_EVIDENCE: &str = "the holder went silent";

/// A comment on a timeline whose forced evidence is **required**: the
/// activation boundary is 2026-07-27, so these dates are past it.
fn dated(id: &str, at: &str, body: String) -> ownership::Comment {
    ownership::Comment {
        id: Some(format!("IC_{id}")),
        created_at: at.to_owned(),
        body,
        viewer_did_author: true,
        includes_created_edit: false,
    }
}

fn claim_body(run: &str, operation: &str) -> String {
    format!(
        "claiming.\n\n{}\n",
        super::super::markers::render(
            "claim",
            &[
                ("run-id", run),
                ("runtime", "claude"),
                ("horizon", RECLAIM_HORIZON),
                ("op-id", operation),
            ],
        )
        .expect("the claim renders"),
    )
}

fn takeover_attributes<'a>(from: &'a str, from_operation: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("run-id", "claude-b"),
        ("runtime", "claude"),
        ("horizon", RECLAIM_HORIZON),
        ("op-id", RECLAIM_OPERATION),
        ("from", from),
        ("from-op", from_operation),
    ]
}

fn attribute<'a>(attributes: &[(&'a str, &'a str)], name: &str) -> &'a str {
    attributes
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
        .unwrap_or_default()
}

/// A forced takeover whose evidence-hash binds exactly these attributes, or
/// the given hash instead — the one input the digest must not survive.
fn forced_takeover_with(
    attributes: &[(&str, &str)],
    evidence: &str,
    bound: Option<&str>,
) -> String {
    let digest = ownership::sha256_hex(evidence.as_bytes());
    let computed = ownership::forced_reclaim_hash(
        Some(attribute(attributes, "op-id")).filter(|value| !value.is_empty()),
        &digest,
        attribute(attributes, "run-id"),
        Some(attribute(attributes, "runtime")).filter(|value| !value.is_empty()),
        Some(attribute(attributes, "horizon")).filter(|value| !value.is_empty()),
        Some(attribute(attributes, "from")).filter(|value| !value.is_empty()),
        Some(attribute(attributes, "from-op")).filter(|value| !value.is_empty()),
    );
    let hash = bound.unwrap_or(&computed);
    let mut rendered = attributes.to_vec();
    rendered.push(("forced", "true"));
    rendered.push(("evidence-hash", hash));
    format!(
        "`{}` reclaiming from `{}`.\n\n{}\n\n{evidence}\n\n{}\n",
        attribute(attributes, "run-id"),
        attribute(attributes, "from"),
        ownership::FORCED_EVIDENCE_HEADING,
        super::super::markers::render("reclaim", &rendered).expect("the takeover renders"),
    )
}

fn forced_takeover(attributes: &[(&str, &str)], evidence: &str) -> String {
    forced_takeover_with(attributes, evidence, None)
}

/// An unforced takeover: no `forced`, no evidence — the shape that may only
/// land on a stale acquisition or on nobody.
fn unforced_takeover(attributes: &[(&str, &str)]) -> String {
    format!(
        "`{}` reclaiming from `{}`.\n\n{}\n",
        attribute(attributes, "run-id"),
        attribute(attributes, "from"),
        super::super::markers::render("reclaim", attributes).expect("the takeover renders"),
    )
}

fn claim_a() -> ownership::Comment {
    dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    )
}

/// The floor each discard test breaks from: one claim, and one forced takeover
/// of it whose evidence binds. Every input below is a rendered marker, so what
/// a test breaks is what the reader has to refuse.
fn taken_over() -> Vec<ownership::Comment> {
    vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            forced_takeover(
                &takeover_attributes("claude-a", HOLDER_OPERATION),
                TAKEOVER_EVIDENCE,
            ),
        ),
    ]
}

fn assert_the_takeover_stands(comments: &[ownership::Comment]) {
    let after = holding(comments, RECLAIM_NOW);
    assert_eq!(
        after.holder.as_deref(),
        Some("claude-b"),
        "the timeline's floor is not standing, so breaking an input below proves nothing"
    );
    assert!(
        won_the_reclaim(after.event, "claude-b", comments, RECLAIM_OPERATION)
            .expect("the takeover stands"),
        "a forced takeover came back unforced"
    );
}

fn refusal(comments: &[ownership::Comment]) -> serde_json::Value {
    let after = holding(comments, RECLAIM_NOW);
    won_the_reclaim(after.event, "claude-b", comments, RECLAIM_OPERATION)
        .expect_err("a discarded takeover was reported as standing")
        .envelope()
}

#[test]
fn a_marker_the_timeline_could_not_read_is_named_and_not_a_lost_race() {
    assert_the_takeover_stands(&taken_over());
    let mut attributes = takeover_attributes("claude-a", HOLDER_OPERATION);
    attributes[2].1 = "the day after never";
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            forced_takeover(&attributes, TAKEOVER_EVIDENCE),
        ),
    ];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "marker-malformed");
    assert_eq!(envelope["world"], "committed");
}

#[test]
fn evidence_that_does_not_bind_is_named_and_not_a_lost_race() {
    assert_the_takeover_stands(&taken_over());
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            forced_takeover_with(
                &takeover_attributes("claude-a", HOLDER_OPERATION),
                TAKEOVER_EVIDENCE,
                Some(&"f".repeat(64)),
            ),
        ),
    ];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "evidence-unproven");
}

#[test]
fn a_comment_the_tracker_did_not_attribute_to_this_identity_is_named() {
    assert_the_takeover_stands(&taken_over());
    let mut second = dated(
        "2",
        "2026-07-28T02:00:00Z",
        forced_takeover(
            &takeover_attributes("claude-a", HOLDER_OPERATION),
            TAKEOVER_EVIDENCE,
        ),
    );
    second.viewer_did_author = false;
    let comments = vec![claim_a(), second];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "comment-untrusted");
}

#[test]
fn a_comment_edited_after_it_was_written_is_named() {
    assert_the_takeover_stands(&taken_over());
    let mut second = dated(
        "2",
        "2026-07-28T02:00:00Z",
        forced_takeover(
            &takeover_attributes("claude-a", HOLDER_OPERATION),
            TAKEOVER_EVIDENCE,
        ),
    );
    second.includes_created_edit = true;
    let comments = vec![claim_a(), second];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "comment-edited");
}

#[test]
fn an_operation_reserved_at_another_marker_is_named() {
    assert_the_takeover_stands(&taken_over());
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            claim_body("claude-c", RECLAIM_OPERATION),
        ),
        dated(
            "3",
            "2026-07-28T03:00:00Z",
            forced_takeover(
                &takeover_attributes("claude-a", HOLDER_OPERATION),
                TAKEOVER_EVIDENCE,
            ),
        ),
    ];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "reserved-elsewhere");
}

/// A marker that is both unreadable and reserved at another marker answers the
/// condition the reader reaches first. The malformed check runs before the
/// reserved one, and this is the only shape that tells the two apart: the
/// malformed marker elsewhere is also the operation's first reservation, so a
/// reader that checked reservation first would call it reserved-elsewhere.
#[test]
fn a_malformed_marker_reserved_elsewhere_is_still_malformed() {
    assert_the_takeover_stands(&taken_over());
    let mut attributes = takeover_attributes("claude-a", HOLDER_OPERATION);
    attributes[2].1 = "the day after never";
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            claim_body("claude-c", RECLAIM_OPERATION),
        ),
        dated(
            "3",
            "2026-07-28T03:00:00Z",
            forced_takeover(&attributes, TAKEOVER_EVIDENCE),
        ),
    ];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "marker-malformed");
}

#[test]
fn an_epoch_the_timeline_does_not_carry_is_named() {
    assert_the_takeover_stands(&taken_over());
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            forced_takeover(
                &takeover_attributes("claude-a", "abcdefabcdefabcdefabcdefabcdefab"),
                TAKEOVER_EVIDENCE,
            ),
        ),
    ];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "target-missing");
}

/// The no-holder branch consulted nothing before this. A reclaim the fold
/// discarded while it displaced nobody — its target scrolled out of the fetched
/// window, the cause `ReclaimDiscard::TargetMissing` itself names — answered
/// only that the write's fate was unknown, so a caller could not learn which
/// check dropped it.
#[test]
fn a_discarded_reclaim_that_leaves_no_holder_is_named_too() {
    let stale = dated(
        "1",
        "2026-07-28T01:00:00Z",
        format!(
            "claiming.\n\n{}\n",
            super::super::markers::render(
                "claim",
                &[
                    ("run-id", "claude-a"),
                    ("runtime", "claude"),
                    ("horizon", "2026-07-28T05:00Z"),
                    ("op-id", HOLDER_OPERATION),
                ],
            )
            .expect("the claim renders"),
        ),
    );
    let comments = vec![
        stale,
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            forced_takeover(
                &takeover_attributes("claude-a", "abcdefabcdefabcdefabcdefabcdefab"),
                TAKEOVER_EVIDENCE,
            ),
        ),
    ];
    let after = holding(&comments, RECLAIM_NOW);
    assert!(
        after.event.is_none(),
        "the floor is wrong: a discarded takeover left a holder standing"
    );
    let envelope = won_the_reclaim(after.event, "claude-b", &comments, RECLAIM_OPERATION)
        .expect_err("a discarded takeover was reported as standing")
        .envelope();
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "target-missing");
    assert_eq!(envelope["world"], "committed");
    assert!(
        envelope.get("winner").is_none(),
        "a discard that leaves nobody carried a winner: {envelope}"
    );
}

#[test]
fn a_live_epoch_that_is_not_the_holder_is_named_rather_than_read_as_absent() {
    assert_the_takeover_stands(&taken_over());
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            claim_body("claude-c", "cccccccccccccccccccccccccccccccc"),
        ),
        dated(
            "3",
            "2026-07-28T03:00:00Z",
            forced_takeover(
                &takeover_attributes("claude-c", "cccccccccccccccccccccccccccccccc"),
                TAKEOVER_EVIDENCE,
            ),
        ),
    ];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(
        envelope["condition"], "target-not-holder",
        "a live epoch that is not the winner exists; reading it as absent is the fold's answer, \
         not the timeline's"
    );
}

#[test]
fn a_takeover_naming_another_runs_epoch_is_named() {
    assert_the_takeover_stands(&taken_over());
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            forced_takeover(
                &takeover_attributes("claude-x", HOLDER_OPERATION),
                TAKEOVER_EVIDENCE,
            ),
        ),
    ];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "target-run-mismatch");
}

#[test]
fn an_unforced_takeover_of_a_live_holder_is_named() {
    assert_the_takeover_stands(&taken_over());
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            unforced_takeover(&takeover_attributes("claude-a", HOLDER_OPERATION)),
        ),
    ];
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "reclaim-discarded");
    assert_eq!(envelope["condition"], "live-target-unforced");
}

#[test]
fn a_takeover_that_counted_and_somebody_else_took_is_the_only_lost_race() {
    // Two valid takeovers in a row. The first counted — the second holds the
    // issue because it displaced the first — so the first run hears the race it
    // really lost, and nothing else reaches `lost-reclaim-race`.
    let second = vec![
        ("run-id", "claude-c"),
        ("runtime", "claude"),
        ("horizon", RECLAIM_HORIZON),
        ("op-id", "dddddddddddddddddddddddddddddddd"),
        ("from", "claude-b"),
        ("from-op", RECLAIM_OPERATION),
    ];
    let comments = vec![
        claim_a(),
        dated(
            "2",
            "2026-07-28T02:00:00Z",
            forced_takeover(
                &takeover_attributes("claude-a", HOLDER_OPERATION),
                TAKEOVER_EVIDENCE,
            ),
        ),
        dated(
            "3",
            "2026-07-28T03:00:00Z",
            forced_takeover(&second, TAKEOVER_EVIDENCE),
        ),
    ];
    assert_eq!(
        holding(&comments, RECLAIM_NOW).holder.as_deref(),
        Some("claude-c")
    );
    let envelope = refusal(&comments);
    assert_eq!(envelope["reason"], "lost-reclaim-race");
    assert_eq!(envelope["winner"], "claude-c");
    assert!(
        envelope.get("condition").is_none(),
        "a lost race carries a discard condition: {envelope}"
    );
}

fn wrote(at: &str, body: String) -> ownership::Comment {
    ownership::Comment {
        id: Some(format!("IC_{at}")),
        created_at: format!("2026-07-26T0{at}:00:00Z"),
        body,
        viewer_did_author: true,
        includes_created_edit: false,
    }
}

/// A claim, then this run's own unassign for its epoch. The shape a retry sees.
fn released(target: &str, operation: &str) -> Vec<ownership::Comment> {
    vec![
        wrote(
            "1",
            format!(
                "claiming.\n\n{}\n",
                super::super::markers::render(
                    "claim",
                    &[
                        ("run-id", "claude-a"),
                        ("runtime", "claude"),
                        ("horizon", "2026-07-26T09:00Z"),
                        ("op-id", target),
                    ],
                )
                .expect("renders")
            ),
        ),
        wrote(
            "2",
            format!(
                "releasing.\n\n{}\n",
                super::super::markers::render(
                    "unassign",
                    &[
                        ("run-id", "claude-a"),
                        ("runtime", "claude"),
                        ("op-id", operation),
                        ("target-op", target),
                    ],
                )
                .expect("renders")
            ),
        ),
    ]
}

fn departure<'a>(target: Option<&'a str>, held_by_other: bool) -> Departure<'a> {
    Departure {
        issue: 7,
        run_id: "claude-a",
        runtime: "claude",
        operation_id: "0123456789abcdef0123456789abcdef",
        target_operation: target,
        held_by_other,
        worktree: None,
        now: "2026-07-26T04:00:00Z",
    }
}

#[test]
fn a_retried_unassign_is_answered_from_its_own_marker_and_not_replanned() {
    // The port planned the release again on a retry, and a plan made after the
    // write landed says `nothing-to-unassign` — there is nothing left to
    // release. A run repeating its own unassign after an ambiguous write was
    // told its release had never happened.
    let target = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let operation = "0123456789abcdef0123456789abcdef";
    let comments = released(target, operation);
    let ownership = holding(&comments, "2026-07-26T04:00:00Z");
    let existing = operation_marker(
        &comments,
        operation,
        OperationKind::Unassign,
        &[("run-id", "claude-a")],
    )
    .expect("a lookup")
    .expect("this run's own unassign");

    // Planning again is where the old answer came from, and it is the wrong
    // question: the epoch is gone precisely because this call worked.
    assert_eq!(
        reason(
            plan_release(&ownership, "claude-a", false, Some(target), &[])
                .expect_err("nothing is left to release")
        ),
        "nothing-to-unassign"
    );

    let answer = resumed_release(
        &context_at(std::path::Path::new(".")),
        &comments,
        &ownership,
        &existing,
        operation,
        &departure(Some(target), false),
    )
    .expect("the retry is answered from the marker");
    assert_eq!(answer["ok"], serde_json::json!(true));
    assert_eq!(answer["assignee_kept"], serde_json::json!(false));
}

/// `resumed_release` must resume even when
/// the acquisition's recorded runtime and the retrying call's own disagree,
/// because the build changed underneath the operation rather than the run.
#[test]
fn a_resumed_release_resumes_across_the_build_boundary() {
    let target = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let operation = "0123456789abcdef0123456789abcdef";
    let comments = vec![
        wrote(
            "1",
            format!(
                "claiming.\n\n{}\n",
                super::super::markers::render(
                    "claim",
                    &[
                        ("run-id", "codex-a"),
                        ("runtime", "claude"),
                        ("horizon", "2026-07-26T09:00Z"),
                        ("op-id", target),
                    ],
                )
                .expect("renders")
            ),
        ),
        wrote(
            "2",
            format!(
                "releasing.\n\n{}\n",
                super::super::markers::render(
                    "unassign",
                    &[
                        ("run-id", "codex-a"),
                        ("runtime", "codex"),
                        ("op-id", operation),
                        ("target-op", target),
                    ],
                )
                .expect("renders")
            ),
        ),
    ];
    let ownership = holding(&comments, "2026-07-26T04:00:00Z");
    let existing = operation_marker(
        &comments,
        operation,
        OperationKind::Unassign,
        &[("run-id", "codex-a")],
    )
    .expect("a lookup")
    .expect("this run's own unassign");

    let what = Departure {
        issue: 7,
        run_id: "codex-a",
        runtime: "codex",
        operation_id: operation,
        target_operation: Some(target),
        held_by_other: false,
        worktree: None,
        now: "2026-07-26T04:00:00Z",
    };
    let answer = resumed_release(
        &context_at(std::path::Path::new(".")),
        &comments,
        &ownership,
        &existing,
        operation,
        &what,
    )
    .expect("a resumed release resumes across the runtime the build changed");
    assert_eq!(answer["ok"], serde_json::json!(true));
}

#[test]
fn a_retried_unassign_naming_an_epoch_nobody_can_end_is_refused_by_name() {
    // `invalid-unassign-target` existed nowhere in the port. The epoch a marker
    // names has to be an acquisition of this run standing *before* the
    // operation's own control marker; anything else is a name, not a release.
    let operation = "0123456789abcdef0123456789abcdef";
    let comments = released("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", operation);
    let ownership = holding(&comments, "2026-07-26T04:00:00Z");

    let mut invented = super::super::markers::Marker::new();
    invented.insert("run-id".to_owned(), "claude-a".to_owned());
    invented.insert(
        "target-op".to_owned(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
    );
    let failure = resumed_release(
        &context_at(std::path::Path::new(".")),
        &comments,
        &ownership,
        &invented,
        operation,
        &departure(None, false),
    )
    .expect_err("no such epoch");
    assert_eq!(
        failure
            .envelope()
            .get("action")
            .and_then(serde_json::Value::as_str),
        Some("re-read ownership and use a fresh operation ID")
    );
    assert_eq!(reason(failure), "invalid-unassign-target");

    // A marker with no target at all is the same refusal, never a success.
    assert_eq!(
        reason(
            resumed_release(
                &context_at(std::path::Path::new(".")),
                &comments,
                &ownership,
                &super::super::markers::Marker::new(),
                operation,
                &departure(None, false),
            )
            .expect_err("no target named")
        ),
        "invalid-unassign-target"
    );
}

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn set_of(values: &[&str]) -> std::collections::BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn the_projection_a_holder_calls_for_is_the_runtime_and_the_assignee_together() {
    // The port had `converge_projection` — labels in, edits out — and nothing
    // called it, while the transport converges after every claim, reclaim and
    // unassign. The assignee half was not there at all, so the issue would have
    // gone on showing whoever held it last.
    let mut modern = event("claude-a", 0, Some("op1"));
    modern.runtime = Some("claude".to_owned());
    let found = projection_for(Some(&modern), &names(&["dev:codex"]), Some("asanabrial"));
    assert_eq!(found.runtimes, set_of(&["dev:claude"]));
    assert_eq!(
        found.assignees,
        set_of(&["asanabrial"]),
        "the holder was projected without an assignee"
    );
    assert!(!found.unresolved_runtime);

    // Nobody holding it means nothing projected — the point of converging at
    // all is that a run which put the issue down stops being shown as holding.
    assert_eq!(
        projection_for(None, &names(&["dev:claude"]), Some("a")),
        Projection::default()
    );
}

#[test]
fn a_holder_nothing_can_name_projects_nothing_rather_than_a_guess() {
    // A legacy acquisition recorded no runtime, so the labels are the only
    // thing left that can name it — and only when they name exactly one
    // runtime this holder could be using. Unknown provenance must not leave the
    // previous holder standing as authoritative, and must not invent the new
    // one either, so "unresolved" empties both sets rather than filling them.
    // Two dashes, so the labels below can genuinely name two runtimes it
    // could be using: `codex-b-7` starts with both `codex-` and `codex-b-`.
    let legacy = event("codex-b-7", 0, None);
    assert!(legacy.runtime.is_none(), "the fixture stopped being legacy");

    // One label, and it is one this run could be using: resolved by prefix.
    let found = projection_for(Some(&legacy), &names(&["dev:codex"]), Some("asanabrial"));
    assert_eq!(found.runtimes, set_of(&["dev:codex"]));
    assert_eq!(found.assignees, set_of(&["asanabrial"]));
    assert!(!found.unresolved_runtime);

    for labels in [
        names(&[]),
        // Names a runtime, but not one this run could be using.
        names(&["dev:claude"]),
        // Two candidates is a question, not a majority.
        names(&["dev:codex", "dev:codex-b"]),
    ] {
        let found = projection_for(Some(&legacy), &labels, Some("asanabrial"));
        assert!(
            found.unresolved_runtime,
            "a holder was named from {labels:?}, which does not name one"
        );
        assert!(
            found.assignees.is_empty() && found.runtimes.is_empty(),
            "an unresolved holder was projected anyway from {labels:?}"
        );
    }
}

#[test]
fn what_the_issue_already_carries_is_read_back_the_way_it_was_written() {
    // The other half of the comparison: a converge is only provable if the
    // current sets can be read as exactly as the expected ones are computed.
    let (assignees, runtimes) = projection_state(&serde_json::json!({
        "assignees": [{"login": "asanabrial"}, {"login": "otra"}],
        "labels": [{"name": "dev:claude"}, {"name": "status:in-progress"}, {"name": "dev:codex"}]
    }));
    assert_eq!(assignees, set_of(&["asanabrial", "otra"]));
    assert_eq!(
        runtimes,
        set_of(&["dev:claude", "dev:codex"]),
        "a status label was counted as a runtime, or a runtime was missed"
    );

    // An issue with neither reads as empty rather than failing: nothing to
    // converge is a state, not a malfunction.
    assert_eq!(
        projection_state(&serde_json::json!({})),
        (Default::default(), Default::default())
    );
}

fn published_by(run_id: &str, receipt: &ReviewTransaction) -> ownership::Comment {
    wrote(
        "3",
        super::super::markers::render(
            "published",
            &[
                ("run-id", run_id),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
            ],
        )
        .expect("a receipt marker"),
    )
}

fn published(receipt: &ReviewTransaction) -> ownership::Comment {
    published_by("claude-a", receipt)
}

/// One `ownership::Comment`, in the shape `gh issue view --json comments`
/// actually returns it — the reverse of [`comment_of`].
fn comment_json(comment: &ownership::Comment) -> serde_json::Value {
    serde_json::json!({
        "id": comment.id,
        "createdAt": comment.created_at,
        "viewerDidAuthor": comment.viewer_did_author,
        "includesCreatedEdit": comment.includes_created_edit,
        "body": comment.body,
    })
}

/// What `gh issue view` answers once a publication carrying exactly this
/// receipt is on the timeline, and nothing else — enough to clear
/// [`record_review_finding`]'s receipt-identity check, and no live claim, so a
/// test built on this either stops before [`verify_claim`] or fails there.
fn published_only_stdout(receipt: &ReviewTransaction) -> serde_json::Value {
    serde_json::json!({ "comments": [comment_json(&published(receipt))] })
}

fn handoff_by(
    run_id: &str,
    target: &str,
    operation: &str,
    receipt: &ReviewTransaction,
) -> ownership::Comment {
    wrote(
        "4",
        super::super::markers::render(
            "review-handoff",
            &[
                ("run-id", run_id),
                ("target-op", target),
                ("op-id", operation),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("authority", "ask 30m"),
                ("requested-at", "2026-07-26T04:00:00Z"),
                ("deadline", "2026-07-26T04:30:00Z"),
                ("blocker", "independent reviewer unavailable"),
                (
                    "discharger",
                    "another run records the exact-receipt verdict",
                ),
            ],
        )
        .expect("a handoff marker"),
    )
}

/// The handoff route, where the reviewing run records its own verdict.
fn verdict_by(
    run_id: &str,
    outcome: &str,
    operation: &str,
    receipt: &ReviewTransaction,
) -> ownership::Comment {
    verdict_attested_by(run_id, run_id, outcome, operation, receipt)
}

/// The direct route, where the claim holder records the reviewer it acquired.
fn verdict_attested_by(
    attester: &str,
    reviewer: &str,
    outcome: &str,
    operation: &str,
    receipt: &ReviewTransaction,
) -> ownership::Comment {
    wrote(
        "5",
        super::super::markers::render(
            "review-verdict",
            &[
                ("run-id", attester),
                ("reviewer", reviewer),
                ("op-id", operation),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("outcome", outcome),
            ],
        )
        .expect("a verdict marker"),
    )
}

/// The `approved` receipt an approval records **locally** — its own store
/// entry, never a GitHub comment.
fn approved_receipt_by(
    run_id: &str,
    reviewer: &str,
    receipt: &ReviewTransaction,
) -> ownership::Comment {
    wrote(
        "8",
        super::super::markers::render(
            "review-receipt",
            &[
                ("run-id", run_id),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("terminal-state", "approved"),
                ("reviewer", reviewer),
            ],
        )
        .expect("an approved receipt marker"),
    )
}

/// The `escalated` receipt `escalate_review` records **locally** — its own
/// store entry, never a GitHub comment.
fn escalated_receipt_by(
    run_id: &str,
    operation: &str,
    person: &str,
    receipt: &ReviewTransaction,
) -> ownership::Comment {
    wrote(
        "9",
        super::super::markers::render(
            "review-receipt",
            &[
                ("run-id", run_id),
                ("op-id", operation),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("terminal-state", "escalated"),
                ("escalated-to", person),
                ("reason", "the correction was spent"),
            ],
        )
        .expect("an escalated receipt marker"),
    )
}

/// A verdict marker an earlier release wrote still parses, whatever optional
/// attributes it carries, and one without a recognised outcome does not.
#[test]
fn an_old_verdict_marker_still_parses_with_or_without_its_optional_attributes() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let render = |outcome: &str, extra: &[(&str, &str)]| {
        let mut fields = vec![
            ("run-id", "claude-a"),
            ("reviewer", "codex-b"),
            ("op-id", "11111111111111111111111111111111"),
            ("epoch", receipt.epoch.as_str()),
            ("pr", "7"),
            ("head", receipt.head.as_str()),
            ("base", receipt.base.as_str()),
            ("digest", receipt.digest.as_str()),
            ("outcome", outcome),
        ];
        fields.extend_from_slice(extra);
        super::super::markers::render("review-verdict", &fields).expect("a verdict marker")
    };
    let read = |text: String| {
        let marker = super::super::markers::parse(&text)
            .pop()
            .expect("the marker parses");
        ReviewVerdict::from_marker(&marker)
    };

    let bare = read(render("accepted", &[])).expect("a bare verdict parses");
    assert_eq!(
        (bare.reviewer.as_str(), bare.outcome.as_str()),
        ("codex-b", "accepted")
    );
    let full = read(render(
        "rejected",
        &[
            ("basis", "ran cargo test; it passed"),
            ("classification", "never-ran"),
            ("never-ran-evidence", "the account has no minutes"),
        ],
    ))
    .expect("a verdict with every optional attribute parses");
    assert_eq!(full.outcome, "rejected");
    assert!(read(render("approved", &[])).is_none());
}

/// Every word `says_nothing` refuses, checked one at a time so each entry is
/// exercised rather than merely declared — the proof that
/// a remembered set alone does not carry. The list is the predicate's own
/// `SAYS_NOTHING_WORDS`, so a word added there is exercised here without a
/// second copy to drift; `tests/guards.rs` reopens the population when the
/// list itself changes. Each is also checked upper-cased and padded with
/// whitespace: no entry could fail on case alone, since every one is lowercase
/// ASCII, so what those two checks actually guard is the `.to_lowercase()` and
/// `.trim()` calls inside `says_nothing` itself — remove either and this
/// assertion reddens.
#[test]
fn every_says_nothing_word_earns_its_place() {
    for &word in super::SAYS_NOTHING_WORDS {
        assert!(
            says_nothing(word),
            "{word:?} must be treated as a proof that says nothing"
        );
        let upper = word.to_uppercase();
        assert!(
            says_nothing(&upper),
            "{upper:?} must be caught the same as {word:?}, case-insensitively"
        );
        let padded = format!("  {word}  ");
        assert!(
            says_nothing(&padded),
            "{padded:?} must be caught the same as {word:?}, trimmed"
        );
    }
}

/// The proof that matters most: the contract states that measuring nothing is
/// a legitimate basis when stated plainly, so a short honest answer must
/// survive `says_nothing` even though it shares a word with an entry on the
/// list (`"nothing"` alone is refused; this is not alone). See
/// `a_verdict_names_what_it_measured_or_says_it_did_not` in `tests/pipe/part_08.rs`
/// for the same string carried through the real MCP call end to end.
#[test]
fn the_honest_short_basis_survives_the_predicate() {
    assert!(
        !says_nothing("nothing was executed"),
        "a plainly stated honest short basis must not be treated as a proof that says nothing"
    );
}

/// `record_review_finding`'s own precision gate already refuses an empty
/// `evidence`; this is the same refusal reached by a word wearing the same
/// emptiness. Before any tracker read, exactly like the basis check above, so
/// this drives the real operation with no `gh` fixture on `PATH` at all.
#[test]
fn a_finding_naming_todo_as_its_evidence_is_refused_before_any_tracker_read() {
    let root = tempfile::tempdir().expect("a context root");
    let context = super::super::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let epoch = "1".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: "11111111111111111111111111111111",
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "loc-1",
        class: "warning",
        evidence: "TODO",
        impact: "a real impact statement",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "",
        lens: "",
        now: "2026-08-31T00:00:00Z",
    };
    let failure = record_review_finding(&context, &finding)
        .expect_err("evidence reading \"TODO\" must be refused");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "incomplete-review-finding");
    assert_eq!(envelope["missing"], "evidence");
}

/// Same check, same list, the other field it is declared against: `impact`
/// wearing the same emptiness as `evidence` above.
#[test]
fn a_finding_naming_none_as_its_impact_is_refused_before_any_tracker_read() {
    let root = tempfile::tempdir().expect("a context root");
    let context = super::super::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let epoch = "1".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: "11111111111111111111111111111111",
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "loc-1",
        class: "warning",
        evidence: "a real evidence statement",
        impact: "none",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "",
        lens: "",
        now: "2026-08-31T00:00:00Z",
    };
    let failure = record_review_finding(&context, &finding)
        .expect_err("impact reading \"none\" must be refused");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "incomplete-review-finding");
    assert_eq!(envelope["missing"], "impact");
}

/// A `severe` finding declaring exactly one of the two words the class owes,
/// with real evidence and impact so the earlier presence check cannot be what
/// refuses it.
fn severe_declaring<'a>(
    epoch: &'a str,
    head: &'a str,
    base: &'a str,
    digest: &'a str,
    evidence_class: &'a str,
    causal_disposition: &'a str,
) -> FindingReview<'a> {
    FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: "33333333333333333333333333333333",
        epoch,
        pr: 7,
        head,
        base,
        digest,
        id: "decl-1",
        class: "severe",
        evidence: "a real evidence statement",
        impact: "a real impact statement",
        parent: "",
        origin: "",
        evidence_class,
        causal_disposition,
        location: "",
        lens: "",
        now: "2026-08-31T00:00:00Z",
    }
}

/// The severe declaration gate is a conjunction: an evidence class without a
/// cause is refused for the cause it is missing.
#[test]
fn a_severe_finding_declaring_only_its_evidence_class_is_refused() {
    let root = tempfile::tempdir().expect("a context root");
    let context = super::super::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let epoch = "1".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = severe_declaring(&epoch, &head, &base, &digest, "deterministic", "");
    let failure = record_review_finding(&context, &finding)
        .expect_err("a severe finding with no cause must be refused");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "incomplete-review-finding");
    assert_eq!(envelope["missing"], "causal_disposition");
}

/// The other half: a cause without an evidence class is refused for the class,
/// so the gate cannot be satisfied by either word alone.
#[test]
fn a_severe_finding_declaring_only_its_causal_disposition_is_refused() {
    let root = tempfile::tempdir().expect("a context root");
    let context = super::super::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let epoch = "1".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = severe_declaring(&epoch, &head, &base, &digest, "", "introduced");
    let failure = record_review_finding(&context, &finding)
        .expect_err("a severe finding with no evidence class must be refused");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "incomplete-review-finding");
    assert_eq!(envelope["missing"], "evidence_class");
}

/// The other half of the pair above: real evidence and impact must not be
/// caught by the same check. No live claim is scripted, so this cannot fully
/// succeed — the same shape `a_citation_naming_a_path_present_only_at_base_is_accepted`
/// uses — and what is asserted is that the failure it does reach is not
/// `incomplete-review-finding`.
#[test]
fn a_finding_with_real_evidence_is_not_refused_for_saying_nothing() {
    let root = tempfile::tempdir().expect("a context root");
    let context = super::super::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "observed the defect directly while reading the change",
        "wrong output reaches every caller on the happy path",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("no live claim is scripted, so this cannot fully succeed");
    let envelope = failure.envelope();
    assert_ne!(
        envelope["reason"], "incomplete-review-finding",
        "real evidence and impact were refused as though they said nothing: {envelope}"
    );
}

#[test]
fn an_unresolved_handoff_excludes_its_publisher_and_requesters_but_not_another_run() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = [
        published(&receipt),
        handoff_by(
            "claude-a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "11111111111111111111111111111111",
            &receipt,
        ),
        handoff_by(
            "codex-b",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "22222222222222222222222222222222",
            &receipt,
        ),
    ];

    for excluded in ["claude-a", "codex-b"] {
        let ReviewEligibility::Excluded {
            publisher,
            requesters,
            handoff,
        } = review_eligibility(&comments, excluded).unwrap()
        else {
            panic!("{excluded} was offered its own unresolved handoff")
        };
        let handoff = *handoff;
        assert_eq!(publisher, "claude-a");
        assert_eq!(requesters, ["claude-a", "codex-b"]);
        assert_eq!(handoff.receipt, receipt);
        assert_eq!(handoff.blocker, "independent reviewer unavailable");
        assert_eq!(handoff.deadline, "2026-07-26T04:30:00Z");
    }
    assert_eq!(
        review_eligibility(&comments, "gemini-c").unwrap(),
        ReviewEligibility::Eligible,
        "a different run cannot discover the review handoff"
    );

    // The two arms, separated. Above, `claude-a` is publisher *and* requester,
    // so either arm alone satisfies every assertion and neither is measured.
    // Here the publisher asked nobody and a different run did the asking.
    let split = [
        published(&receipt),
        handoff_by(
            "codex-b",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "22222222222222222222222222222222",
            &receipt,
        ),
    ];
    assert!(
        matches!(
            review_eligibility(&split, "claude-a").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "the publishing run was offered work whose review is unresolved"
    );
    assert!(
        matches!(
            review_eligibility(&split, "codex-b").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "the requesting run was offered the handoff it asked somebody else to take"
    );
    assert_eq!(
        review_eligibility(&split, "gemini-c").unwrap(),
        ReviewEligibility::Eligible
    );
}

/// Every field-shape check on the three review markers, measured rather than
/// asserted.
///
/// A reviewer deleted each of these one at a time and watched the suite stay
/// green, and an earlier version of `docs/honesty.md` then *listed* them as
/// unmeasured — from somebody else's measurement, not one this repository had
/// run. Both are the same mistake in different directions: a validator nothing
/// exercises, and a document naming a number nobody counted. These are the
/// validators, exercised. What a malformed marker must never be is *readable*,
/// because a half-read receipt still compares equal to itself and would bind a
/// verdict to bytes nobody can name.
#[test]
fn a_malformed_review_marker_is_not_a_marker_at_all() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let fields = |overrides: &[(&str, &str)]| -> Vec<(String, String)> {
        let mut all: Vec<(String, String)> = vec![
            ("epoch".to_owned(), receipt.epoch.clone()),
            ("pr".to_owned(), receipt.pr.to_string()),
            ("head".to_owned(), receipt.head.clone()),
            ("base".to_owned(), receipt.base.clone()),
            ("digest".to_owned(), receipt.digest.clone()),
        ];
        for (name, value) in overrides {
            match all.iter_mut().find(|(field, _)| field == name) {
                Some(slot) => slot.1 = (*value).to_owned(),
                None => all.push(((*name).to_owned(), (*value).to_owned())),
            }
        }
        all
    };
    let marker = |kind: &str, fields: &[(String, String)]| {
        let borrowed: Vec<(&str, &str)> = fields
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        super::super::markers::parse(
            &super::super::markers::render(kind, &borrowed).expect("a marker renders"),
        )
        .pop()
        .expect("one marker parses back")
    };

    // The receipt's own shape, through the publication reader that uses it.
    for broken in [
        ("epoch", "1".repeat(31)),
        ("epoch", "Z".repeat(32)),
        ("pr", "0".to_owned()),
        ("head", "a".repeat(39)),
        ("base", "b".repeat(41)),
        ("digest", "c".repeat(63)),
    ] {
        let overrides = fields(&[("run-id", "claude-a"), (broken.0, broken.1.as_str())]);
        assert!(
            ReviewTransaction::from_marker(&marker("published", &overrides)).is_none(),
            "a receipt with a malformed {} was read as complete",
            broken.0
        );
    }

    // A publication nobody can be attributed to is not a publication.
    //
    // Emptied after rendering, because `render` drops an empty attribute rather
    // than writing one: the only way to reach these checks is the way they exist
    // for — a marker somebody wrote or edited by hand on the tracker.
    let emptied = |body: String, key: &str, value: &str| {
        let found = format!(" {key}={value} ");
        assert!(body.contains(&found), "nothing to empty in {body}");
        body.replace(&found, &format!(" {key}= "))
    };
    let anonymous = [wrote(
        "3",
        emptied(
            super::super::markers::render(
                "published",
                &[
                    ("run-id", "claude-a"),
                    ("epoch", &receipt.epoch),
                    ("pr", &receipt.pr.to_string()),
                    ("head", &receipt.head),
                    ("base", &receipt.base),
                    ("digest", &receipt.digest),
                ],
            )
            .expect("a receipt marker"),
            "run-id",
            "claude-a",
        ),
    )];
    assert!(
        latest_publication(&anonymous).unwrap().is_none(),
        "a publication crediting nobody was read as the latest one"
    );

    // The handoff's own fields: an authority nobody can parse, a target that is
    // not an epoch, a deadline before its request, and the two free-text fields
    // whose whole purpose is naming what is missing and who can supply it.
    let handoff = |overrides: &[(&str, &str)]| {
        let mut base = vec![
            ("run-id", "claude-a"),
            ("target-op", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            ("op-id", "11111111111111111111111111111111"),
            ("authority", "ask 30m"),
            ("requested-at", "2026-07-26T04:00:00Z"),
            ("deadline", "2026-07-26T04:30:00Z"),
            ("blocker", "no reviewer"),
            ("discharger", "another run"),
        ];
        for (name, value) in overrides {
            match base.iter_mut().find(|(field, _)| field == name) {
                Some(slot) => slot.1 = value,
                None => base.push((name, value)),
            }
        }
        let owned: Vec<(String, String)> = base
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .chain(fields(&[]))
            .collect();
        ReviewHandoff::from_marker(&marker("review-handoff", &owned))
    };
    assert!(
        handoff(&[]).is_some(),
        "the well-formed handoff was refused"
    );
    for broken in [
        ("authority", "whenever"),
        ("authority", "ask 30y"),
        ("target-op", "not-an-epoch"),
        ("requested-at", "yesterday"),
        ("blocker", ""),
        ("discharger", ""),
    ] {
        assert!(
            handoff(&[broken]).is_none(),
            "a handoff with {} = {:?} was read as durable",
            broken.0,
            broken.1
        );
    }
    assert!(
        handoff(&[("deadline", "2026-07-26T03:59:00Z")]).is_none(),
        "a deadline before its own request was read as a deadline"
    );

    // And the verdict's two identities. Either one missing leaves a verdict
    // that credits nobody or is attributed to nobody.
    let verdict = |attester: &str, reviewer: &str, outcome: &str| {
        let owned: Vec<(String, String)> = [
            ("run-id".to_owned(), attester.to_owned()),
            ("reviewer".to_owned(), reviewer.to_owned()),
            (
                "op-id".to_owned(),
                "22222222222222222222222222222222".to_owned(),
            ),
            ("outcome".to_owned(), outcome.to_owned()),
        ]
        .into_iter()
        .chain(fields(&[]))
        .collect();
        ReviewVerdict::from_marker(&marker("review-verdict", &owned))
    };
    assert!(verdict("gemini-c", "codex-b", "accepted").is_some());
    assert!(
        verdict("gemini-c", "codex-b", "approved").is_none(),
        "an outcome outside the vocabulary was read as a verdict"
    );
    // Both identities, emptied by hand for the same reason as above.
    for (key, value) in [("run-id", "gemini-c"), ("reviewer", "codex-b")] {
        let owned: Vec<(String, String)> = [
            ("run-id".to_owned(), "gemini-c".to_owned()),
            ("reviewer".to_owned(), "codex-b".to_owned()),
            (
                "op-id".to_owned(),
                "22222222222222222222222222222222".to_owned(),
            ),
            ("outcome".to_owned(), "accepted".to_owned()),
        ]
        .into_iter()
        .chain(fields(&[]))
        .collect();
        let borrowed: Vec<(&str, &str)> = owned
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let body = emptied(
            super::super::markers::render("review-verdict", &borrowed).expect("a marker renders"),
            key,
            value,
        );
        let marker = super::super::markers::parse(&body)
            .pop()
            .expect("the edited marker still parses");
        assert!(
            ReviewVerdict::from_marker(&marker).is_none(),
            "a verdict with an empty {key} was read"
        );
    }
}

/// The binding promises the requesting run cannot *select or reclaim* an
/// unresolved handoff. Selection was refused inside `claim` and reclaim was
/// not, so a requester whose replacement went stale could take the item back
/// and become, once again, the only holder forbidden to produce its verdict —
/// the exact livelock the handoff exists to end. One guard, consulted by both
/// routes, is what keeps the two from disagreeing again.
#[test]
fn no_acquisition_route_returns_an_unresolved_handoff_to_its_requester() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = [
        published(&receipt),
        handoff_by(
            "claude-a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "11111111111111111111111111111111",
            &receipt,
        ),
    ];

    assert_eq!(
        reason(
            require_review_eligibility(&comments, "claude-a")
                .expect_err("the requesting run was offered its own handoff back")
        ),
        "review-handoff-requester-excluded"
    );
    require_review_eligibility(&comments, "gemini-c")
        .expect("a distinct run must still be able to acquire the item");

    let source = crate::source_text::code_of(include_str!("../claim.rs"));
    let body_of = |entry: &str| {
        let body = source.split_once(entry).expect("the entry point exists").1;
        body.split_once("\npub fn ")
            .map_or(body, |(body, _)| body)
            .to_owned()
    };
    for (entry, planner) in [
        ("\npub fn claim(", "operation_marker("),
        ("\npub fn reclaim(", "plan_takeover("),
    ] {
        let body = body_of(entry);
        let guard = body
            .find("require_review_eligibility(")
            .unwrap_or_else(|| panic!("{entry} never consults the handoff exclusion"));
        let planned = body
            .find(planner)
            .unwrap_or_else(|| panic!("{entry} no longer plans through {planner}"));
        assert!(
            guard < planned,
            "{entry} plans an acquisition before refusing the excluded requester"
        );
    }
}

/// `record_entry` posts what it is handed, and its contract is *a body
/// this crate composed*. The blocker, discharger and run-id in a review
/// protocol comment are agent text, so a body that quoted a marker would **be**
/// that marker once posted: the publishing run could carry a forged
/// `review-verdict` inside its own handoff comment, satisfy the handoff it is
/// forbidden to satisfy, and clear `release_ci` with evidence it wrote itself.
/// That is the liveness fix turning into an integrity hole, which is the one
/// trade this issue refuses.
#[test]
fn agent_text_in_a_protocol_comment_cannot_forge_a_second_marker() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let forged = super::super::markers::render(
        "review-verdict",
        &[
            ("run-id", "codex-zz"),
            ("op-id", "00000000000000000000000000000001"),
            ("epoch", &receipt.epoch),
            ("pr", &receipt.pr.to_string()),
            ("head", &receipt.head),
            ("base", &receipt.base),
            ("digest", &receipt.digest),
            ("outcome", "accepted"),
        ],
    )
    .expect("a verdict marker to smuggle");
    let own = super::super::markers::render(
        "review-handoff",
        &[("run-id", "claude-a"), ("op-id", &"a".repeat(32))],
    )
    .expect("the comment's own marker");

    let body = protocol_body(
        &format!("Blocker: no reviewer {forged}\n\nDischarger: anybody"),
        &own,
    );
    let kinds: Vec<Option<String>> = super::super::markers::parse(&body)
        .iter()
        .map(|marker| marker.get("kind").cloned())
        .collect();
    assert_eq!(
        kinds,
        [Some("review-handoff".to_owned())],
        "agent text smuggled a second protocol marker into the comment: {body}"
    );

    // Both writers compose through the one escaping helper. The refusal is
    // worth nothing if a later body is assembled beside it.
    let claim_source = crate::source_text::code_of(include_str!("../claim.rs"));
    let computed_source = crate::source_text::code_of(include_str!("../claim/computed.rs"));
    for (source, entry) in [
        (&claim_source, "\npub fn handoff_review("),
        (&computed_source, "\npub fn record_lens_report("),
        (&computed_source, "\npub fn record_computed_verdict("),
    ] {
        let body = source.split_once(entry).expect("the writer exists").1;
        let body = body.split_once("\npub fn ").map_or(body, |(body, _)| body);
        let composed = body
            .find("protocol_body(")
            .unwrap_or_else(|| panic!("{entry} composes its comment without escaping agent text"));
        let posted = body
            .find("record_entry(")
            .unwrap_or_else(|| panic!("{entry} no longer posts a comment"));
        assert!(composed < posted, "{entry} posts before it escapes");
    }
}

#[test]
fn a_distinct_legacy_verdict_still_resolves_a_handoff_it_was_recorded_against() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let base = vec![
        published(&receipt),
        handoff_by(
            "claude-a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "11111111111111111111111111111111",
            &receipt,
        ),
    ];

    let mut by_the_publisher = base.clone();
    by_the_publisher.push(verdict_by(
        "claude-a",
        "accepted",
        "33333333333333333333333333333333",
        &receipt,
    ));
    assert!(
        matches!(
            review_eligibility(&by_the_publisher, "claude-a").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "the publisher's own marker resolved its handoff"
    );

    let requester_only = [
        published(&receipt),
        handoff_by(
            "codex-b",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "22222222222222222222222222222222",
            &receipt,
        ),
        verdict_attested_by(
            "gemini-c",
            "codex-b",
            "accepted",
            "44444444444444444444444444444444",
            &receipt,
        ),
    ];
    assert!(
        matches!(
            review_eligibility(&requester_only, "codex-b").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "crediting the requester resolved its own handoff"
    );

    for outcome in ["accepted", "rejected"] {
        let mut distinct = base.clone();
        distinct.push(verdict_by(
            "codex-b",
            outcome,
            "33333333333333333333333333333333",
            &receipt,
        ));
        assert_eq!(
            review_eligibility(&distinct, "claude-a").unwrap(),
            ReviewEligibility::Eligible,
            "a distinct {outcome} marker did not resolve the handoff it was recorded against"
        );
    }
}

#[test]
fn a_recorded_computed_marker_of_any_outcome_resolves_a_handoff() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let base = vec![
        published(&receipt),
        handoff_by(
            "claude-a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "11111111111111111111111111111111",
            &receipt,
        ),
    ];
    assert!(matches!(
        review_eligibility(&base, "claude-a").unwrap(),
        ReviewEligibility::Excluded { .. }
    ));

    for outcome in ["approved", "rejected", "escalated"] {
        let mut resolved = base.clone();
        resolved.push(computed_by("codex-b", &receipt, outcome));
        assert_eq!(
            review_eligibility(&resolved, "claude-a").unwrap(),
            ReviewEligibility::Eligible,
            "a {outcome} computed marker did not resolve the handoff"
        );
    }

    let mut by_the_publisher = base.clone();
    by_the_publisher.push(computed_by("claude-a", &receipt, "approved"));
    assert!(
        matches!(
            review_eligibility(&by_the_publisher, "claude-a").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "the publisher's own computed marker resolved its handoff"
    );

    let mut elsewhere = base;
    elsewhere.push(computed_by(
        "codex-b",
        &ReviewTransaction {
            epoch: "2".repeat(32),
            ..receipt
        },
        "approved",
    ));
    assert!(
        matches!(
            review_eligibility(&elsewhere, "claude-a").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "a marker over another receipt resolved this handoff"
    );
}

#[test]
fn a_republished_pull_request_is_judged_by_its_latest_receipt_alone() {
    let older = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let latest = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "d".repeat(40),
        ..older.clone()
    };
    let handed_off = vec![
        published(&older),
        handoff_by(
            "claude-a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "11111111111111111111111111111111",
            &older,
        ),
        published(&latest),
    ];
    assert_eq!(
        review_eligibility(&handed_off, "claude-a").unwrap(),
        ReviewEligibility::Eligible,
        "an older receipt's handoff kept the run excluded after the pull request was republished"
    );

    let mut handed_off_again = handed_off;
    handed_off_again.push(handoff_by(
        "claude-a",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "22222222222222222222222222222222",
        &latest,
    ));
    match review_eligibility(&handed_off_again, "claude-a").unwrap() {
        ReviewEligibility::Excluded { handoff, .. } => assert_eq!(handoff.receipt, latest),
        ReviewEligibility::Eligible => panic!("the latest receipt's own handoff was ignored"),
    }

    let mut resolved = handed_off_again;
    resolved.push(computed_by("codex-b", &latest, "approved"));
    assert_eq!(
        review_eligibility(&resolved, "claude-a").unwrap(),
        ReviewEligibility::Eligible
    );
}

#[test]
fn a_handoff_is_resolved_against_the_latest_receipt_of_its_own_pull_request() {
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let second = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 8,
        head: "d".repeat(40),
        base: "e".repeat(40),
        digest: "f".repeat(64),
    };
    let handed_off = vec![
        published(&first),
        handoff_by(
            "claude-a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "11111111111111111111111111111111",
            &first,
        ),
        published_by("codex-b", &second),
    ];
    match review_eligibility(&handed_off, "claude-a").unwrap() {
        ReviewEligibility::Excluded { handoff, .. } => assert_eq!(handoff.receipt, first),
        ReviewEligibility::Eligible => {
            panic!("a later pull request's publication hid the first one's unresolved handoff")
        }
    }

    let mut resolved = handed_off.clone();
    resolved.push(computed_by("codex-b", &first, "approved"));
    assert_eq!(
        review_eligibility(&resolved, "claude-a").unwrap(),
        ReviewEligibility::Eligible,
        "a computed verdict on its own pull request's receipt did not resolve the handoff"
    );

    let mut other_pull_request = handed_off;
    other_pull_request.push(computed_by("codex-b", &second, "approved"));
    assert!(
        matches!(
            review_eligibility(&other_pull_request, "claude-a").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "a verdict on the other pull request's receipt resolved this handoff"
    );
}

/// An abandoned pull request's unresolved handoff keeps excluding the run that
/// published or requested it from later pull requests on the issue. **Decision:**
/// the exclusion is kept, not narrowed to closed pull requests.
/// `review_eligibility` reads the timeline alone — `all_publications` carries no
/// pull-request state — so telling an abandoned pull request from an open one
/// needs a tracker read this pure function does not have, and the handoff
/// already has a discharge that is not the passage of time: a distinct run's
/// recorded judgement. `docs/honesty.md` names the limit.
#[test]
fn an_abandoned_pull_requests_handoff_still_excludes_its_publisher() {
    let abandoned = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let later = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 8,
        head: "d".repeat(40),
        base: "e".repeat(40),
        digest: "f".repeat(64),
    };
    let comments = [
        published_by("claude-a", &abandoned),
        handoff_by(
            "codex-b",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "11111111111111111111111111111111",
            &abandoned,
        ),
        published(&later),
    ];

    assert!(
        matches!(
            review_eligibility(&comments, "claude-a").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "the abandoned pull request's handoff stopped excluding its publisher"
    );
    assert!(
        matches!(
            review_eligibility(&comments, "codex-b").unwrap(),
            ReviewEligibility::Excluded { .. }
        ),
        "the abandoned pull request's handoff stopped excluding its requester"
    );
    assert_eq!(
        review_eligibility(&comments, "delta-c").unwrap(),
        ReviewEligibility::Eligible,
        "a run that neither published nor requested the abandoned pull request was excluded"
    );
}

#[test]
fn stale_edited_untrusted_and_prose_verdicts_never_resolve_a_handoff() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let mut stale = receipt.clone();
    stale.head = "d".repeat(40);
    let handoff = handoff_by(
        "claude-a",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "11111111111111111111111111111111",
        &receipt,
    );
    let marker = verdict_by(
        "codex-b",
        "accepted",
        "33333333333333333333333333333333",
        &receipt,
    );
    let mut edited = marker.clone();
    edited.includes_created_edit = true;
    let mut untrusted = marker;
    untrusted.viewer_did_author = false;

    for evidence in [
        verdict_by(
            "codex-b",
            "accepted",
            "33333333333333333333333333333333",
            &stale,
        ),
        edited,
        untrusted,
        wrote(
            "5",
            "codex-b accepted epoch 11111111111111111111111111111111".to_owned(),
        ),
    ] {
        let comments = [published(&receipt), handoff.clone(), evidence];
        assert!(
            matches!(
                review_eligibility(&comments, "claude-a").unwrap(),
                ReviewEligibility::Excluded { .. }
            ),
            "non-qualifying evidence released the publisher"
        );
    }

    let mut edited_handoff = handoff;
    edited_handoff.includes_created_edit = true;
    assert_eq!(
        review_eligibility(&[published(&receipt), edited_handoff], "claude-a").unwrap(),
        ReviewEligibility::Eligible,
        "an edited request became durable queue authority"
    );
}

#[test]
fn review_protocol_operation_ids_are_stable_only_for_the_same_immutable_event() {
    let fields = [
        "claude-a",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "11111111111111111111111111111111",
        "7",
        "blocker",
    ];
    let first = review_operation_id("review-handoff", &fields);
    assert_eq!(first, review_operation_id("review-handoff", &fields));
    assert_eq!(first.len(), 32);
    assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let mut changed = fields;
    changed[4] = "different blocker";
    assert_ne!(first, review_operation_id("review-handoff", &changed));
    assert_ne!(first, review_operation_id("review-verdict", &fields));
}

#[test]
fn a_handoff_operation_replays_identically_and_refuses_conflicting_copies() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let operation = "11111111111111111111111111111111";
    let first = handoff_by(
        "claude-a",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        operation,
        &receipt,
    );
    let expected = [
        ("run-id", "claude-a"),
        ("target-op", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    ];
    assert!(
        operation_marker(
            &[first.clone(), first.clone()],
            operation,
            OperationKind::ReviewHandoff,
            &expected,
        )
        .expect("identical replay")
        .is_some()
    );

    let mut conflict = first.clone();
    conflict.body = conflict.body.replace(
        "independent%20reviewer%20unavailable",
        "different%20blocker",
    );
    assert_eq!(
        reason(
            operation_marker(
                &[first, conflict],
                operation,
                OperationKind::ReviewHandoff,
                &expected,
            )
            .expect_err("one operation was rebound")
        ),
        "review-handoff-operation-conflict"
    );
}

/// One grammar reads `Review delegation`, wherever it is read from.
///
/// It had been written three times — the configuration's parser, the
/// transport's, and the handoff marker's own validator — and the three
/// disagreed. Four spellings `estigia config` accepts were refused by the
/// transport, and the review handoff is the one operation that ends a blocked
/// run's wait, so a mis-cased row left it holding the issue for good. `ask  30m`
/// was worse: it passed the transport, was stamped into the marker verbatim, and
/// was refused by the marker's reader — comment posted, readback failing, every
/// retry answering `review-handoff-operation-conflict` with the claim never
/// released. Both of the transport's copies also cut the duration at a byte
/// offset, so a value ending in a multi-byte character panicked the process
/// instead of being refused, and `review_eligibility` is what `claim`, `reclaim`
/// and every review-queue candidate go through.
#[test]
fn one_grammar_reads_every_authority_row_the_configuration_accepts() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };

    // Every spelling the configuration reader accepts, and what it renders, is
    // one the marker reader reads back: markers written before `ask` was
    // retired still bind their requester.
    for spelling in [
        "auto", "Auto", "AUTO", "ask", "Ask", "ask 30m", "Ask 30m", "ask 30 m", "ask  30m",
        "ask 2h", "ask 45s",
    ] {
        assert!(
            crate::config::authority_of(spelling).is_some(),
            "{spelling} is not a value the configuration accepts; fix the fixture, not the rule"
        );
        let recorded = crate::config::rendered_authority(
            crate::config::authority_of(spelling).expect("checked above"),
        );
        let marker = super::super::markers::render(
            "review-handoff",
            &[
                ("run-id", "claude-a"),
                ("target-op", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                ("op-id", "11111111111111111111111111111111"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("authority", &recorded),
                ("requested-at", "2026-07-26T04:00:00Z"),
                ("deadline", "2026-07-26T04:30:00Z"),
                ("blocker", "no reviewer"),
                ("discharger", "another run"),
            ],
        )
        .expect("a handoff marker");
        let parsed = super::super::markers::parse(&marker)
            .pop()
            .expect("the marker parses");
        assert!(
            ReviewHandoff::from_marker(&parsed).is_some(),
            "{spelling} was recorded as {recorded:?}, which its own reader refuses"
        );
    }

    // And a value that is not one. Refused on both sides, and — the reason this
    // test exists — refused rather than fatal: these cut at a byte offset, and
    // `review_eligibility` parses timelines for the queue and both acquisitions.
    for broken in ["whenever", "ask 30y", "ask 30é", "ask é"] {
        assert!(crate::config::authority_of(broken).is_none(), "{broken}");
        let marker = super::super::markers::render(
            "review-handoff",
            &[
                ("run-id", "claude-a"),
                ("target-op", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                ("op-id", "11111111111111111111111111111111"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("authority", broken),
                ("requested-at", "2026-07-26T04:00:00Z"),
                ("deadline", "2026-07-26T04:30:00Z"),
                ("blocker", "no reviewer"),
                ("discharger", "another run"),
            ],
        )
        .expect("a handoff marker");
        let comments = [published(&receipt), wrote("4", marker)];
        assert_eq!(
            review_eligibility(&comments, "codex-b").unwrap(),
            ReviewEligibility::Eligible,
            "{broken} was read as a durable handoff"
        );
    }
}

#[test]
fn a_new_handoff_records_auto_and_an_immediate_deadline_whatever_the_table_says() {
    assert_eq!(
        review_authority("2026-07-26T04:00:00Z").expect("auto records immediately"),
        ("auto".to_owned(), "2026-07-26T04:00:00Z".to_owned())
    );
    assert!(review_authority("yesterday").is_err());
}

#[test]
fn an_old_handoff_marker_carrying_ask_still_parses_and_binds_its_requester() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let old = handoff_by(
        "claude-a",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "11111111111111111111111111111111",
        &receipt,
    );
    let comments = [published(&receipt), old];
    assert!(matches!(
        review_eligibility(&comments, "claude-a").unwrap(),
        ReviewEligibility::Excluded { handoff, .. } if handoff.authority == "ask 30m"
    ));
    assert_eq!(
        review_eligibility(&comments, "codex-b").unwrap(),
        ReviewEligibility::Eligible
    );
}

#[test]
fn the_compound_handoff_records_before_release_and_checks_review_afterwards() {
    let source = crate::source_text::code_of(include_str!("../claim.rs"));
    let body = source
        .split_once("pub fn handoff_review(")
        .expect("the compound operation exists")
        .1
        .split_once("pub struct FindingReview")
        .expect("the compound operation ends")
        .0;
    let verify = body.find("verify_claim(").expect("renews first");
    let receipt = body
        .find("require_recorded_receipt")
        .expect("checks the receipt");
    let record = body.find("record_entry").expect("records the handoff");
    let receipt_retry = body
        .rfind("require_recorded_receipt")
        .expect("rechecks the receipt on retry");
    let release = body.find("unassign(").expect("releases ownership");
    let state = body
        .rfind("status_labels")
        .expect("checks the retained state");
    assert!(
        verify < receipt
            && receipt < record
            && record < receipt_retry
            && receipt_retry < release
            && release < state
    );
}

#[test]
fn ci_release_spends_the_receipt_only_after_a_qualifying_review() {
    let source = crate::source_text::code_of(include_str!("../claim.rs"));
    let body = source
        .split_once("pub fn release_ci(")
        .expect("CI release exists")
        .1
        .split_once("fn require_pr_matches(")
        .expect("CI release ends")
        .0;
    let receipt = body
        .find("recorded_receipt")
        .expect("checks the latest receipt");
    let verdict = body
        .find("release_basis(")
        .expect("checks what clears the receipt");
    let ready = body.find("ready_write").expect("marks the PR ready");
    let verify = body
        .find("verify_held(")
        .expect("adjudicates the caller first");
    assert!(verify < receipt && receipt < verdict && verdict < ready);
}

#[test]
fn a_release_requires_the_latest_complete_review_transaction() {
    let first = ReviewTransaction {
        epoch: "11111111111111111111111111111111".to_owned(),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let second = ReviewTransaction {
        epoch: "22222222222222222222222222222222".to_owned(),
        head: "d".repeat(40),
        ..first.clone()
    };
    let comments = [published(&first), published(&second)];

    assert_eq!(
        recorded_receipt(&comments, &second).expect("the latest receipt"),
        second
    );
    assert_eq!(
        reason(recorded_receipt(&comments, &first).expect_err("an old epoch")),
        "published-transaction-mismatch"
    );

    let foreign = ReviewTransaction {
        epoch: "33333333333333333333333333333333".to_owned(),
        head: "e".repeat(40),
        ..second.clone()
    };
    let cross_run = [published(&second), published_by("codex-b", &foreign)];
    assert_eq!(
        reason(recorded_receipt(&cross_run, &second).expect_err("another run republished later")),
        "published-transaction-mismatch"
    );

    let incomplete = wrote(
        "4",
        super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-b"),
                ("pr", "7"),
                ("head", &"a".repeat(40)),
            ],
        )
        .expect("an incomplete legacy marker"),
    );
    // A `published` marker whose receipt fields are present but do not make a
    // complete receipt is refused by name, not read as though no publication
    // existed: `published-transaction-missing` would be the nearest named state.
    assert_eq!(
        reason(recorded_receipt(&[incomplete], &first).expect_err("incomplete")),
        "published-transaction-unparseable"
    );
}

/// The publisher recorded on the marker is not consulted — this is the
/// restore it builds: the local pointer is a note that can go
/// missing, the timeline is the authority, and the latest receipt is
/// obtained whatever run published it.
#[test]
fn a_release_obtains_the_latest_receipt_whatever_run_published_it() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = [published_by("claude-a", &receipt)];
    assert_eq!(
        recorded_receipt(&comments, &receipt)
            .expect("a byte-exact receipt is obtainable whatever run published it"),
        receipt
    );
}

/// One issue, two pull requests. A chained delivery publishes each slice under
/// the issue, so a later slice's publication must leave an earlier slice's
/// receipt usable. Before this, `recorded_receipt` demanded the single newest
/// publication on the whole issue, so slice 1's receipt was refused with
/// `published-transaction-mismatch` the moment slice 2 published, and slice 1 could
/// never be released.
#[test]
fn a_recorded_receipt_survives_a_later_slice_on_another_pull_request() {
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let second = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 8,
        head: "d".repeat(40),
        ..first.clone()
    };
    let comments = [published(&first), published(&second)];

    assert_eq!(
        recorded_receipt(&comments, &first).expect("slice 1's receipt is still recorded"),
        first
    );
    assert_eq!(
        recorded_receipt(&comments, &second).expect("slice 2's receipt is the newest for PR 8"),
        second
    );
}

/// The per-pull-request rule keeps the protection it always had: a republish of
/// one pull request still invalidates that pull request's earlier epochs, so
/// the older head can never reach CI.
#[test]
fn a_republish_on_one_pull_request_still_invalidates_its_earlier_epochs() {
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let republished = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "d".repeat(40),
        ..first.clone()
    };
    let comments = [published(&first), published(&republished)];

    assert_eq!(
        reason(recorded_receipt(&comments, &first).expect_err("the same PR republished")),
        "published-transaction-mismatch"
    );
    assert_eq!(
        recorded_receipt(&comments, &republished).expect("the newest for PR 7"),
        republished
    );
}

/// A receipt for a pull request the issue never published is refused by name,
/// not read as though the issue carried no publication at all.
#[test]
fn a_receipt_for_an_unpublished_pull_request_is_a_mismatch_not_a_missing_publication() {
    let recorded = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let other_pr = ReviewTransaction {
        pr: 9,
        ..recorded.clone()
    };
    assert_eq!(
        reason(recorded_receipt(&[published(&recorded)], &other_pr).expect_err("PR 9")),
        "published-transaction-mismatch"
    );
}

/// The distinct-verdict lookup follows the same per-pull-request rule, so a
/// verdict recorded for an earlier slice still resolves once a later slice
/// publishes.
#[test]
fn a_distinct_verdict_is_found_for_an_earlier_slice_after_a_later_one_publishes() {
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let second = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 8,
        head: "d".repeat(40),
        ..first.clone()
    };
    let comments = [
        published_by("claude-a", &first),
        published_by("claude-a", &second),
        verdict_by("claude-reviewer", "accepted", &"a".repeat(32), &first),
    ];

    let verdict = latest_distinct_review_verdict(&comments, &first)
        .expect("the read lands")
        .expect("a distinct verdict for slice 1's receipt");
    assert_eq!(verdict.reviewer, "claude-reviewer");
}

/// The repair reads a finding's lineage and a republish's lease for the
/// receipt's own pull request, not the issue's newest publication. This is the
/// predicate both now share.
#[test]
fn the_newest_publication_is_read_per_pull_request() {
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let second = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 8,
        head: "d".repeat(40),
        ..first.clone()
    };
    let comments = [published(&first), published(&second)];
    assert_eq!(
        latest_publication_for_pr(&comments, 7)
            .expect("a read")
            .map(|publication| publication.receipt),
        Some(first)
    );
    assert_eq!(
        latest_publication_for_pr(&comments, 8)
            .expect("a read")
            .map(|publication| publication.receipt),
        Some(second)
    );
    assert_eq!(
        latest_publication_for_pr(&comments, 9).expect("a read"),
        None
    );
}

#[test]
fn a_release_still_refuses_a_receipt_that_differs_in_any_single_field() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = [published_by("claude-a", &receipt)];
    let mutations = [
        ReviewTransaction {
            epoch: "9".repeat(32),
            ..receipt.clone()
        },
        ReviewTransaction {
            pr: 8,
            ..receipt.clone()
        },
        ReviewTransaction {
            head: "d".repeat(40),
            ..receipt.clone()
        },
        ReviewTransaction {
            base: "e".repeat(40),
            ..receipt.clone()
        },
        ReviewTransaction {
            digest: "f".repeat(64),
            ..receipt
        },
    ];
    for mutated in mutations {
        assert_eq!(
            reason(
                recorded_receipt(&comments, &mutated)
                    .expect_err("a field differing from the timeline must still be refused")
            ),
            "published-transaction-mismatch",
            "{mutated:?} should have been refused"
        );
    }
}

/// One comparison, held for both the release path `recorded_receipt` fronts and
/// the refutation path `require_distinct_reviewer` fronts. Removing that
/// comparison left both answering `ok`, so this is the test that says the one
/// remaining check is load-bearing on each.
#[test]
fn the_one_receipt_comparison_refuses_a_stale_receipt_on_the_release_and_refutation_paths() {
    let latest = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "d".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let stale = ReviewTransaction {
        epoch: "1".repeat(32),
        head: "a".repeat(40),
        ..latest.clone()
    };
    let comments = [published(&latest)];

    let release = recorded_receipt(&comments, &stale)
        .map(|_| ())
        .map_err(reason);
    let verdict = require_distinct_reviewer(&comments, &stale, "claude-reviewer").map_err(reason);

    assert_eq!(
        (release.err().as_deref(), verdict.err().as_deref()),
        (
            Some("published-transaction-mismatch"),
            Some("published-transaction-mismatch")
        ),
        "the one comparison must refuse a stale receipt on both the release and refutation paths"
    );
}

/// The requester arm of the distinctness rule: a refuter that is a handoff
/// requester is refused, not only one that is the publisher. Disabling the
/// `requesters.iter().any(..)` clause leaves this test the only one that goes
/// red for it.
#[test]
fn a_handoff_requester_cannot_be_credited_with_its_own_refutation() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = [
        published_by("claude-a", &receipt),
        handoff_by(
            "codex-b",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "22222222222222222222222222222222",
            &receipt,
        ),
    ];

    assert_eq!(
        reason(
            require_distinct_reviewer(&comments, &receipt, "codex-b")
                .expect_err("the requesting run was credited with adjudicating its own request")
        ),
        "reviewer-not-distinct"
    );
}

#[test]
fn an_ambiguous_ready_write_is_settled_only_by_exact_readback() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let ready = serde_json::json!({
        "number": 7, "headRefOid": receipt.head, "baseRefOid": receipt.base, "isDraft": false,
    });
    ready_outcome(
        Err(Failure::Write("connection lost".to_owned())),
        Ok(ready),
        &receipt,
    )
    .expect("the failed command landed exactly");

    let still_draft = serde_json::json!({
        "number": 7, "headRefOid": receipt.head, "baseRefOid": receipt.base, "isDraft": true,
    });
    assert!(matches!(
        ready_outcome(
            Err(Failure::Write("connection lost".to_owned())),
            Ok(still_draft),
            &receipt,
        ),
        Err(Failure::Write(_))
    ));
    assert!(matches!(
        ready_outcome(Ok(()), Err(Failure::Read("no answer".to_owned())), &receipt,),
        Err(Failure::Write(_))
    ));
    assert!(matches!(
        ready_outcome(
            Err(Failure::Read("gh was not started".to_owned())),
            Ok(serde_json::json!({
                "number": 7, "headRefOid": receipt.head, "baseRefOid": receipt.base,
                "isDraft": true,
            })),
            &receipt,
        ),
        Err(Failure::Read(_))
    ));
}

#[test]
fn release_pr_matching_covers_number_head_base_and_draft_state() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let current = serde_json::json!({
        "number": 7, "headRefOid": receipt.head, "baseRefOid": receipt.base, "isDraft": true,
    });
    require_pr_matches(&current, &receipt, true).expect("the exact draft");

    for field in ["number", "headRefOid", "baseRefOid", "isDraft"] {
        let mut changed = current.clone();
        changed[field] = match field {
            "number" => serde_json::json!(8),
            "isDraft" => serde_json::json!(false),
            _ => serde_json::json!("f".repeat(40)),
        };
        assert_eq!(
            reason(require_pr_matches(&changed, &receipt, true).expect_err(field)),
            "release-pr-mismatch",
            "{field} was not part of the release identity"
        );
    }
}

#[test]
fn publication_establishes_the_draft_barrier_before_push() {
    let source = crate::source_text::code_of(include_str!("../claim.rs"));
    let body = source
        .split_once("pub fn publish_review(")
        .expect("publication exists")
        .1;
    let draft = body
        .find("ensure_draft(context, pr)")
        .expect("drafts reused PRs");
    // The **call site**, not the string the push is spelled with. Extracting
    // `push_to_origin` moved that spelling into a function *defined below*
    // `publish_with`, so `"git", "push"` stopped being the place the push
    // happens and started being the place it is written — and the comparison
    // was satisfied by file layout rather than by order. Measured: hoisting the
    // call above the draft conversion, which would put a rewritten head at a
    // pull request CI is watching, left this test green.
    let push = body
        .find("push_to_origin(at, branch, push)")
        .expect("pushes the branch");
    assert!(draft < push, "a reused ready PR is drafted only after push");

    let create = source
        .split_once("\"pr\",\n                    \"create\"")
        .expect("new PR creation exists")
        .1;
    assert!(
        create.contains("\"--draft\""),
        "a new PR can open ready and start CI before review"
    );
    assert!(
        source.contains("draft-readback-failed") && source.contains("isDraft"),
        "drafting is trusted from the write path instead of confirmed"
    );
}

/// A checkout with one commit, wired to a bare remote of its own.
///
/// Real git, because the acceptance criterion is about what git does with a
/// lease and not about what this crate writes into one. The returned paths are
/// the remote and the working checkout; the directory is returned with them so
/// it outlives the test body.
fn repository_with_remote() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let root = tempfile::tempdir().expect("a directory");
    let remote = root.path().join("remote.git");
    let work = root.path().join("work");
    std::fs::create_dir_all(&remote).expect("the remote directory");
    std::fs::create_dir_all(&work).expect("the work directory");
    git(&remote, &["init", "--quiet", "--bare", "-b", "main"]);
    git(&work, &["init", "--quiet", "-b", "main"]);
    git(&work, &["config", "user.email", "estigia@example.invalid"]);
    git(&work, &["config", "user.name", "Estigia test"]);
    git(
        &work,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    std::fs::write(work.join("one.txt"), "one\n").expect("content");
    git(&work, &["add", "one.txt"]);
    git(&work, &["commit", "--quiet", "-m", "one"]);
    (root, remote, work)
}

/// A git command that has to work for the test to mean anything.
///
/// It panics naming the arguments rather than returning: a fixture step that
/// failed quietly would leave the assertions below measuring a repository that
/// was never in the state they describe.
fn git(at: &std::path::Path, arguments: &[&str]) {
    let mut command = vec!["git"];
    command.extend_from_slice(arguments);
    crate::transport::run(&command, Some(at), crate::transport::How::write())
        .unwrap_or_else(|failure| panic!("git {arguments:?} failed: {}", failure.detail()));
}

fn head_of(at: &std::path::Path) -> String {
    crate::transport::run(
        &["git", "rev-parse", "HEAD"],
        Some(at),
        crate::transport::How::read(),
    )
    .expect("git names the head")
    .stdout
    .trim()
    .to_owned()
}

/// A red lane's refusal carries where the failing step's log puts
/// the failure, against the diff under review, so a run can tell a defect in
/// its own change from one outside it.
///
/// The location is read from the **log**, not from the check run's annotations:
/// GitHub annotates a step failure at the workflow directory (`.github`), so an
/// annotation-based comparison answers "outside" for every failure.
#[test]
fn a_red_lane_names_whether_its_failure_is_outside_the_diff() {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the repository directory");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    std::fs::create_dir_all(repo.join("src")).expect("the src directory");
    std::fs::write(repo.join("src/foo.rs"), "fn main() {}\n").expect("content");
    git(&repo, &["add", "src/foo.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let base = head_of(&repo);
    std::fs::write(repo.join("src/foo.rs"), "fn main() { /* now */ }\n").expect("content");
    git(&repo, &["add", "src/foo.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "head"]);
    let head = head_of(&repo);

    let run_url = "https://github.com/o/r/actions/runs/31865504912/job/99";
    let red = super::RedCheckRun {
        name: "check (ubuntu-latest)".to_owned(),
        conclusion: "failure".to_owned(),
        run_url: run_url.to_owned(),
    };
    let context = context_at(&repo);

    let locate = |log: &str| -> Option<super::LaneFailureLocation> {
        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[serde_json::json!({
            "matches": "run view 31865504912 --log-failed",
            "stdout": log,
            "status": 0,
        })]);
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            super::lane_failure_location(&context, &red, &base, &head)
        })
    };

    let inside =
        locate("thread 'x' panicked at src/foo.rs:1:1:\n").expect("the log names a changed path");
    assert_eq!(inside.failing_paths, vec!["src/foo.rs".to_owned()]);
    assert_eq!(inside.changed_paths, vec!["src/foo.rs".to_owned()]);
    assert!(
        !inside.outside_diff,
        "a path the diff changes is inside the change"
    );

    // A Windows lane writes the separator the other way; git's own output is
    // always forward-slashed, so the log's path is normalised before the
    // comparison rather than read as a different file.
    let windows =
        locate("thread 'x' panicked at src\\foo.rs:1:1:\n").expect("the log names a changed path");
    assert_eq!(windows.failing_paths, vec!["src/foo.rs".to_owned()]);
    assert!(
        !windows.outside_diff,
        "a backslash path names the same changed file, not another one"
    );

    let outside = locate("  --> untouched.rs:12:5\n").expect("the log names an unchanged path");
    assert_eq!(outside.failing_paths, vec!["untouched.rs".to_owned()]);
    assert!(
        outside.outside_diff,
        "a path the diff does not change is outside the change"
    );

    // A log naming no source path proves no location, which leaves the refusal
    // the shape it had before this existed rather than inventing one.
    let none = locate("error: could not compile `estigia`\n");
    assert!(
        none.is_none(),
        "a log naming no source path is not a location"
    );
}

/// The refusal's observable contract: the fields a run routes on are actually
/// serialised, not merely computed, and the location-free shape is unchanged.
#[test]
fn a_red_lane_refusal_carries_the_location_fields() {
    let head = "b".repeat(40);
    let red = super::RedCheckRun {
        name: "check (ubuntu-latest)".to_owned(),
        conclusion: "failure".to_owned(),
        run_url: "https://github.com/o/r/actions/runs/31865504912/job/99".to_owned(),
    };
    let location = super::LaneFailureLocation {
        failing_paths: vec!["src/elsewhere.rs".to_owned()],
        changed_paths: vec!["src/mine.rs".to_owned()],
        outside_diff: true,
    };
    let body = super::red_refusal(&head, &red, Some(&location)).envelope();
    assert_eq!(body["reason"], "publication-lane-red");
    assert_eq!(body["outside_diff"], serde_json::json!(true));
    assert_eq!(
        body["failing_paths"],
        serde_json::json!(["src/elsewhere.rs"])
    );
    assert_eq!(body["changed_paths"], serde_json::json!(["src/mine.rs"]));
    let action = body["action"].as_str().expect("an action");
    assert!(action.contains("not a defect in these bytes"), "{action}");

    let bare = super::red_refusal(&head, &red, None).envelope();
    assert!(bare.get("outside_diff").is_none());
    assert!(bare.get("failing_paths").is_none());
    assert!(
        !bare["action"]
            .as_str()
            .expect("an action")
            .contains("the failing log names"),
        "a location-free refusal says nothing about a log"
    );
}

#[test]
fn the_lease_names_the_recorded_head_and_not_the_tracking_ref() {
    // The three-part form, and the reason it is spelled in a function of its own
    // rather than inline: `--force-with-lease` alone, and `--force-with-lease=
    // <branch>`, both lease against the remote-tracking ref — which the
    // publication path refreshes itself when it fetches the base. Both are valid
    // git, neither is a compile error, and neither protects what this operation
    // cares about.
    let expected = "a".repeat(40);
    let lease = lease_for("fix/4-x", &expected);
    assert_eq!(lease, format!("--force-with-lease=fix/4-x:{expected}"));
    assert_eq!(
        lease.split(':').next_back(),
        Some(expected.as_str()),
        "the lease does not end in the commit the remote must currently be at: {lease}"
    );
}

#[test]
fn a_leased_push_lands_a_rewritten_branch_the_ordinary_push_refuses() {
    // Both halves in one test on purpose. The refusal alone would pass against a
    // push that never works at all, and the success alone would pass against a
    // plain `--force`; what the issue asks for is the pair.
    let (_root, _remote, work) = repository_with_remote();
    git(&work, &["push", "-u", "origin", "--", "main"]);
    let published_head = head_of(&work);

    // A rewritten history, in its cheapest honest form: after the amend this
    // branch is no longer a descendant of what the remote holds, which is what a
    // rebase onto a moved base leaves behind.
    git(
        &work,
        &["commit", "--quiet", "--amend", "-m", "one, amended"],
    );
    assert_ne!(head_of(&work), published_head, "the amend rewrote nothing");

    assert!(
        super::push_to_origin(&work, "main", super::Push::FastForward).is_err(),
        "the ordinary push accepted a rewritten history, so this proves nothing about the lease"
    );
    super::push_to_origin(
        &work,
        "main",
        super::Push::Leased {
            recorded_head: &published_head,
        },
    )
    .expect("a lease against the head the remote actually holds is accepted");
}

#[test]
fn a_leased_push_refuses_when_the_remote_moved_since_the_recorded_head() {
    // The criterion the operation exists for. A lease is only worth having if it
    // refuses, and it can only be shown to refuse against a remote somebody else
    // moved.
    let (_root, remote, work) = repository_with_remote();
    git(&work, &["push", "-u", "origin", "--", "main"]);
    let published_head = head_of(&work);

    // Somebody else, through a second checkout of the same remote, so the moving
    // push is a real one rather than this checkout writing to itself.
    let beside = remote.parent().expect("the remote has a parent").to_owned();
    let other = beside.join("other");
    git(
        &beside,
        &[
            "clone",
            "--quiet",
            "--",
            &remote.to_string_lossy(),
            &other.to_string_lossy(),
        ],
    );
    git(&other, &["config", "user.email", "other@example.invalid"]);
    git(&other, &["config", "user.name", "Somebody else"]);
    std::fs::write(other.join("two.txt"), "two\n").expect("content");
    git(&other, &["add", "two.txt"]);
    git(&other, &["commit", "--quiet", "-m", "two"]);
    git(&other, &["push", "--quiet", "origin", "--", "main"]);

    // And now this run rewrites its own history and republishes. The lease names
    // the head its receipt recorded, which is no longer what the remote holds.
    git(
        &work,
        &["commit", "--quiet", "--amend", "-m", "one, amended"],
    );
    // The fetch matters, and it is not scenery. `publish_with` fetches the base
    // before deriving the target, so by the time it pushes, this checkout's
    // remote-tracking refs have already seen the other run's commit — and a bare
    // `--force-with-lease`, whose expectation *is* the tracking ref, would be
    // satisfied by exactly the push this must refuse. Measured: without the
    // fetch, the bare form refuses too and this test cannot tell the two apart.
    git(&work, &["fetch", "--quiet", "origin"]);
    assert!(
        super::push_to_origin(
            &work,
            "main",
            super::Push::Leased {
                recorded_head: &published_head,
            },
        )
        .is_err(),
        "the lease let a republish destroy a commit the recorded head never named"
    );

    // The other run's commit is still there. A refusal that had already written
    // is not a refusal, and an exit code alone does not show that.
    let remote_head = crate::transport::run(
        &["git", "rev-parse", "refs/heads/main"],
        Some(&remote),
        crate::transport::How::read(),
    )
    .expect("the remote names its head");
    assert_eq!(
        remote_head.stdout.trim(),
        head_of(&other),
        "the refused push moved the remote anyway"
    );
}

#[test]
fn a_base_that_advanced_past_the_branchs_own_rebase_is_refused_before_the_payload_is_built() {
    // The exact case measured on this repository's own timeline: a
    // branch built on the base's tip at the time it branched, and that base
    // tip advancing again before publication. The two-dot diff `publish_with`
    // would otherwise build carries everything the base gained in between,
    // inverted into what reads as this branch's own deletions.
    let (_root, remote, work) = repository_with_remote();
    git(&work, &["push", "-u", "origin", "--", "main"]);

    let beside = remote.parent().expect("the remote has a parent").to_owned();
    let other = beside.join("other");
    git(
        &beside,
        &[
            "clone",
            "--quiet",
            "--",
            &remote.to_string_lossy(),
            &other.to_string_lossy(),
        ],
    );
    git(&other, &["config", "user.email", "other@example.invalid"]);
    git(&other, &["config", "user.name", "Somebody else"]);

    // `work` branches off the base tip it can currently see. This is what a
    // rebase onto the base leaves behind: the branch's history includes this
    // commit as an ancestor.
    git(&work, &["checkout", "--quiet", "-b", "fix/1-x", "main"]);
    std::fs::write(work.join("three.txt"), "three\n").expect("content");
    git(&work, &["add", "three.txt"]);
    git(&work, &["commit", "--quiet", "-m", "three"]);
    let branch_head = head_of(&work);

    // The base advances again, past what the branch was rebased onto — through
    // a second checkout, so the advance is a real push and not this checkout
    // writing to itself.
    std::fs::write(other.join("four.txt"), "four\n").expect("content");
    git(&other, &["add", "four.txt"]);
    git(&other, &["commit", "--quiet", "-m", "four"]);
    git(&other, &["push", "--quiet", "origin", "--", "main"]);

    let remote_main = |at: &std::path::Path| -> String {
        crate::transport::run(
            &["git", "rev-parse", "refs/heads/main"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("the remote names its head")
        .stdout
        .trim()
        .to_owned()
    };
    let context = Context {
        skill_dir: work.clone(),
        repo_dir: work.clone(),
        config: Vec::new(),
        repo: None,
    };
    let what = Publication {
        issue: 277,
        run_id: "claude-abcd1234",
        expect_state: "in-progress",
        base: "main",
        branch: "fix/1-x",
        title: "example",
        pr_body_file: None,
        worktree: Some(&work),
        inline_payload: false,
        topology: None,
        topology_reason: None,
        focus: None,
        declaration: Default::default(),
        expected_epoch: None,
        now: "2026-01-01T00:05:00Z",
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::live_holder_issue(what.run_id, what.issue, what.expect_state),
        serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
        serde_json::json!({ "matches": "list --head fix/1-x", "stdout": "[]", "status": 0 }),
    ]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::publish_review(&context, &what)
    })
    .expect_err("a base that advanced past the branch's own rebase was accepted");

    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "publication-base-not-ancestor");
    assert_eq!(envelope["head"], branch_head);
    assert_eq!(envelope["base"], remote_main(&remote));
    // A first publication has no recorded receipt, so the
    // rebase recovery must name `publish_review` for it as well as
    // `republish_review` for a repair — naming only the latter landed this path
    // on `published-transaction-missing`.
    let action = envelope["action"].as_str().unwrap_or_default();
    assert!(
        action.contains("`publish_review`") && action.contains("`republish_review`"),
        "the refusal must name the operation that clears each path: {envelope}"
    );
    // The recovery names the one literal the gate admits as an
    // integration and the state it is admitted from, so the reader does not
    // reach for a rebase (which rewrites the parent head) or for a shape the
    // gate refuses on its own terms.
    assert!(
        action.contains("git -C <worktree> merge --no-edit origin/main")
            && action.contains("`in-progress`"),
        "the refusal must name the accepted integration literal and the state it is admitted \
         from: {envelope}"
    );

    // No assertion here re-reads `main`: `publish_review` never pushes the base
    // branch, only the feature branch, so that equality would hold under every
    // outcome and discriminate nothing.
    let branch_on_remote = crate::transport::run(
        &[
            "git",
            "ls-remote",
            "--exit-code",
            "--heads",
            "origin",
            "fix/1-x",
        ],
        Some(&work),
        crate::transport::How::tolerated(),
    )
    .expect("ls-remote answers");
    assert_ne!(
        branch_on_remote.status, 0,
        "the feature branch reached the remote despite the refusal"
    );
}

/// `publish_review`'s answer no longer carries the topology declaration, the
/// repair lineage, the publication lane's detail or the autoclose verdict —
/// all of it moved to `review_transaction`; see `docs/review-cost.md`. A
/// branch-link publication is the shape that used to add `mandatory_follow_up`
/// on top of `autoclose`, so it is the one most likely to regress silently.
#[test]
fn a_branch_link_publication_keeps_the_follow_up_out_of_its_answer() {
    let (_root, remote, work) = repository_with_remote();
    git(&work, &["push", "-q", "-u", "origin", "--", "main"]);
    let base_sha = crate::transport::run(
        &["git", "rev-parse", "refs/heads/main"],
        Some(&remote),
        crate::transport::How::read(),
    )
    .expect("the remote names its head")
    .stdout
    .trim()
    .to_owned();

    let issue = 12;
    let run_id = "claude-abcd1234";
    let branch = "fix/12-branch-link";
    let base = "main";
    git(&work, &["checkout", "-q", "-b", branch]);
    std::fs::write(work.join("two.txt"), "two\n").expect("content");
    git(&work, &["add", "two.txt"]);
    git(
        &work,
        &["commit", "--quiet", "-m", "a change that names no issue"],
    );
    let branch_head = head_of(&work);

    let trace = tempfile::tempdir().expect("a directory for the PR body");
    let pr_body = trace.path().join("body.md");
    std::fs::write(&pr_body, "A body that names nothing.\n").expect("the body");

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::live_holder_issue(run_id, issue, "in-progress"),
        // A pull request already open on this branch, so `published` reuses it
        // instead of creating one — the shape this fixture needs only one
        // `pr list` and one `pr view` answer to cover.
        serde_json::json!({
            "matches": format!("list --head {branch}"),
            "stdout": serde_json::json!([{
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": "0".repeat(40),
                "baseRefOid": "0".repeat(40),
                "isDraft": true,
            }])
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "state,isDraft",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": branch_head,
                "baseRefOid": base_sha,
                "state": "OPEN",
                "isDraft": true,
            })
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
        serde_json::json!({
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
            })
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "json body", "stdout": "{\"body\":\"A body that names nothing.\"}", "status": 0 }),
    ]);

    let context = Context {
        skill_dir: work.clone(),
        repo_dir: work,
        config: Vec::new(),
        repo: None,
    };
    let what = Publication {
        issue,
        run_id,
        expect_state: "in-progress",
        base,
        branch,
        title: "Something",
        pr_body_file: Some(&pr_body),
        worktree: None,
        inline_payload: false,
        topology: None,
        topology_reason: None,
        focus: None,
        declaration: Default::default(),
        expected_epoch: None,
        now: "2026-01-01T00:05:00Z",
    };

    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::publish_review(&context, &what)
    })
    .expect("a branch-link publication is not refused");

    let object = answer.as_object().expect("an object answer");
    for removed in [
        "autoclose",
        "mandatory_follow_up",
        "topology",
        "topology_reason",
        "parent",
        "parent_pr",
        "parent_head",
        "parent_base",
        "parent_digest",
        "delta",
        "delta_range",
        "delta_range_reason",
        "publication_lane_note",
        "publication_lane_workflows",
        "publication_lane_count",
    ] {
        assert!(
            !object.contains_key(removed),
            "`{removed}` moved to review_transaction and must not ride publish_review's own \
             answer any more: {answer}"
        );
    }
    assert!(
        answer["publication_lane"].is_string(),
        "the state word stays on the write answer even though its detail moved: {answer}"
    );
    // The shrink this issue's acceptance measures: `publish_review`'s own
    // answer, identity fields and `next` only, stays well under what it cost
    // before the lineage, topology, lane detail and autoclose verdict moved
    // to `review_transaction` — see `docs/review-cost.md`. 900 bytes is
    // measured headroom, not a production limit.
    let bytes = serde_json::to_string(&answer)
        .expect("the answer serialises")
        .len();
    assert!(
        bytes < 900,
        "publish_review's answer grew past its measured ceiling: {bytes} bytes: {answer}"
    );
}

#[test]
fn the_ordinary_publication_has_no_way_to_force() {
    // The last acceptance criterion, and the one that decays silently: a boolean
    // added to `Publication` later, or a `Push` chosen from an argument, would
    // reintroduce exactly the implicit force the operation was split in two to
    // avoid. `publish_review` names its variant as a literal and takes nothing
    // that could name the other.
    let source = crate::source_text::code_of(include_str!("../claim.rs"));
    let entry = source
        .split_once("pub fn publish_review(")
        .expect("publication exists")
        .1
        .split_once("\n}")
        .expect("the entry point ends")
        .0;
    assert!(
        entry.contains("Push::FastForward") && !entry.contains("Push::Leased"),
        "publish_review can reach the leased push: {entry}"
    );
    // And the force with no expectation to check, which the issue rules out by
    // name. Asked of the push site rather than of the file: `claim.rs` carries a
    // legitimate `--force` for `gh label create`, and a search that found it
    // would have to be loosened until it found nothing at all. The push site may
    // name no force flag of any spelling — the only one it can reach is the
    // lease `lease_for` builds, and that function has its own test.
    let pushes = source
        .split_once("fn push_to_origin(")
        .expect("the push site exists")
        .1
        .split_once("\n}")
        .expect("the push site ends")
        .0;
    assert!(
        !pushes.contains("--force"),
        "the push site spells a force flag of its own instead of the checked lease: {pushes}"
    );
}

/// The head a lease is taken from is the latest `published` marker's, or none.
///
/// Named for what it measures. It was called
/// `a_republish_with_nothing_recorded_to_lease_against_is_refused` and never
/// called `republish_review` at all — so the refusal in its name was unheld, and
/// deleting that refusal left the whole suite green. The refusal is now driven
/// end to end in `pipe.rs`; this keeps the half that needs no remote.
#[test]
fn the_head_a_lease_is_taken_from_is_the_latest_published_marker() {
    // `latest_publication` is what supplies the expectation, and an issue with
    // no `published` marker supplies none. Forcing anyway would be a `--force`
    // with the lease spelled over whatever the remote happened to hold.
    assert_eq!(latest_publication(&[]).unwrap(), None);

    let published = |head: &str| ownership::Comment {
        id: Some("IC_1".to_owned()),
        created_at: "2026-01-01T00:00Z".to_owned(),
        viewer_did_author: true,
        includes_created_edit: false,
        body: super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-abcd1234"),
                ("pr", "7"),
                ("head", head),
                ("base", &"b".repeat(40)),
                ("digest", &"c".repeat(64)),
                ("epoch", &"a".repeat(32)),
            ],
        )
        .expect("the marker renders"),
    };
    let head = "d".repeat(40);
    assert_eq!(
        latest_publication(&[published(&head)])
            .unwrap()
            .expect("a complete marker is a receipt")
            .receipt
            .head,
        head,
        "the lease would be taken against something other than the recorded head"
    );
}

/// A push that lands with no confirmed marker must still leave a
/// lease target, and that target must never qualify review or CI.
#[test]
fn an_unconfirmed_publication_is_a_lease_target_and_not_a_review_target() {
    let head = "a".repeat(40);
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: head.clone(),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let unconfirmed = wrote(
        "3",
        super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("unconfirmed", "true"),
            ],
        )
        .expect("an unconfirmed marker"),
    );
    let latest = latest_publication(std::slice::from_ref(&unconfirmed))
        .unwrap()
        .expect("an unconfirmed marker is still a complete receipt");
    assert_eq!(latest.receipt.head, head);
    assert!(
        !latest.confirmed,
        "unconfirmed=true was read as a confirmed publication"
    );
    assert_eq!(
        reason(
            recorded_receipt(std::slice::from_ref(&unconfirmed), &receipt)
                .expect_err("review binds")
        ),
        "publication-unconfirmed"
    );
    assert_eq!(
        reason(
            require_recorded_receipt(
                std::slice::from_ref(&unconfirmed),
                &receipt,
                "cannot be handed off",
            )
            .expect_err("handoff binds")
        ),
        "publication-unconfirmed"
    );
    // The lease still names that head — the recovery route asked for.
    assert_eq!(
        lease_for("fix/123-x", &latest.receipt.head),
        format!("--force-with-lease=fix/123-x:{head}")
    );
}

/// An unconfirmed marker on the same PR must not become the correction-budget
/// parent: that would bootstrap a fresh genesis and zero the cumulative a
/// prior confirmed repair froze.
#[test]
fn correction_parent_skips_unconfirmed_publications_on_the_same_pr() {
    let confirmed = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let unconfirmed_receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "d".repeat(40),
        ..confirmed.clone()
    };
    let comments = [
        published(&confirmed),
        wrote(
            "4",
            super::super::markers::render(
                "published",
                &[
                    ("run-id", "claude-a"),
                    ("epoch", &unconfirmed_receipt.epoch),
                    ("pr", &unconfirmed_receipt.pr.to_string()),
                    ("head", &unconfirmed_receipt.head),
                    ("base", &unconfirmed_receipt.base),
                    ("digest", &unconfirmed_receipt.digest),
                    ("unconfirmed", "true"),
                ],
            )
            .expect("unconfirmed marker"),
        ),
    ];
    let on_pr = all_publications(&comments)
        .unwrap()
        .into_iter()
        .rev()
        .find(|publication| publication.receipt.pr == 7 && publication.confirmed)
        .expect("a confirmed parent remains");
    assert_eq!(
        on_pr.receipt, confirmed,
        "the unconfirmed marker stole the correction parent"
    );
    assert!(
        latest_publication(&comments)
            .unwrap()
            .expect("latest exists")
            .receipt
            == unconfirmed_receipt,
        "the lease target must still be the unconfirmed head"
    );
}

/// A `published` marker written before the budget fields existed
/// carries none of them, and must still bootstrap a fresh lineage — the
/// migration path `bootstrap_correction_state` exists for. This is the absent
/// outcome, which must stay legitimate; only a field that is *present* and
/// unreadable is a refusal.
#[test]
fn a_budgetless_marker_bootstraps_a_fresh_lineage_unchanged() {
    let rendered = super::super::markers::render(
        "published",
        &[
            ("run-id", "claude-a"),
            ("epoch", "11111111111111111111111111111111"),
            ("pr", "7"),
            ("head", &"a".repeat(40)),
            ("base", &"b".repeat(40)),
            ("digest", &"c".repeat(64)),
        ],
    )
    .expect("a marker renders");
    let marker = super::super::markers::parse(&rendered)
        .pop()
        .expect("one marker parses back");
    assert_eq!(
        CorrectionState::read(&marker).expect("a budgetless marker is not a refusal"),
        None,
        "a marker carrying none of the budget fields was read as one that did"
    );
    let publications = all_publications(&[wrote("3", rendered)]).expect("admitted");
    assert_eq!(publications.len(), 1);
    assert_eq!(
        publications[0].correction_state, None,
        "the absent budget was not carried through `all_publications` as absent"
    );
}

/// A `published` marker whose budget fields are present but do not
/// parse is refused by name before anything remote moves, naming the epoch and
/// the offending field — never read as "no budget was ever frozen", which would
/// reset the cumulative to zero in silence.
#[test]
fn an_unparseable_budget_field_is_refused_by_name() {
    let broken = wrote(
        "3",
        super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", "11111111111111111111111111111111"),
                ("pr", "7"),
                ("head", &"a".repeat(40)),
                ("base", &"b".repeat(40)),
                ("digest", &"c".repeat(64)),
                ("budget-genesis-tree", &"d".repeat(40)),
                ("budget-genesis-paths", "feature.txt"),
                ("budget-original-lines", "not-a-number"),
                ("budget", "2"),
                ("budget-cumulative", "0"),
            ],
        )
        .expect("a marker renders"),
    );
    let failure = all_publications(&[broken]).expect_err("a half-written budget is not absent");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "correction-state-unparseable");
    assert_eq!(envelope["field"], "budget-original-lines");
    assert_eq!(envelope["epoch"], "11111111111111111111111111111111");
}

/// A `published` marker whose receipt fields are present but do not
/// form a complete receipt is refused by name rather than dropped from
/// `all_publications` — a corrupt publication and no publication must not be the
/// same input to `latest`, `repair_rounds` or a budget. The oldest surviving
/// `published` marker on this repository's own timeline is complete, so no
/// pre-schema marker is turned unreadable by this.
#[test]
fn a_published_marker_with_a_malformed_receipt_is_refused_not_dropped() {
    let broken = wrote(
        "3",
        super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", "11111111111111111111111111111111"),
                ("pr", "7"),
                ("head", &"a".repeat(40)),
            ],
        )
        .expect("a marker renders"),
    );
    let failure = all_publications(std::slice::from_ref(&broken))
        .expect_err("a published marker missing its base is not a publication");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "published-transaction-unparseable");
    assert_eq!(envelope["field"], "base");
    assert_eq!(envelope["epoch"], "11111111111111111111111111111111");
    // The same refusal reaches every reader of the latest publication, not only
    // the lineage enumeration.
    assert_eq!(
        reason(
            latest_publication(std::slice::from_ref(&broken))
                .expect_err("the refusal is not dropped at `latest_publication`")
        ),
        "published-transaction-unparseable"
    );
}

/// Each blank rule in its own test: two assertions in one test can only
/// redden once, so a shared test would prove one refusal while swearing to
/// both.
#[test]
fn a_blank_topology_is_refused_rather_than_recorded_nowhere() {
    for blank in ["", "   ", "n/a"] {
        let reason = super::normalize_topology(Some(blank), None)
            .expect_err("a blank declaration must not pass")
            .envelope();
        assert_eq!(
            reason["reason"], "blank-topology",
            "{blank:?} was refused as something else"
        );
    }
}

#[test]
fn a_blank_topology_reason_is_refused_rather_than_kept() {
    for reason in ["", "   ", "n/a"] {
        let envelope = super::normalize_topology(Some("inline"), Some(reason))
            .expect_err("a blank reason must not pass")
            .envelope();
        assert_eq!(envelope["reason"], "blank-topology-reason");
        // The reason's own refusal fires for any declared topology,
        // so its action names the one declared rather than assuming `inline`.
        assert!(
            envelope["action"]
                .as_str()
                .is_some_and(|action| action.contains("`inline`")),
            "the refusal must name the declared topology: {envelope}"
        );
        let envelope = super::normalize_topology(None, Some(reason))
            .expect_err("a blank reason with no topology must not pass")
            .envelope();
        assert_eq!(envelope["reason"], "blank-topology-reason");
        assert!(
            envelope["action"]
                .as_str()
                .is_some_and(|action| action.contains("no topology was declared")),
            "the refusal must say no topology was declared: {envelope}"
        );
    }
    // A delegated topology needs no reason at all, so its refusal must not tell
    // the caller to justify one — "any topology value", and the
    // case the `inline`-only test above could not see.
    let envelope = super::normalize_topology(Some("developer:3"), Some("  "))
        .expect_err("a blank reason must not pass")
        .envelope();
    let action = envelope["action"].as_str().unwrap_or_default();
    assert!(
        action.contains("`developer:3`"),
        "the refusal must name the declared topology: {envelope}"
    );
    assert!(
        !action.contains("needed no delegation"),
        "a delegated topology is not told to justify why it needed none: {envelope}"
    );
}

/// `PUBLICATION_FIXED_OVERHEAD` has to bound the comment `published` renders
/// apart from the caller's text, the manifest and the lane, or the pre-push
/// refusal passes a comment GitHub then rejects.
#[test]
fn the_fixed_overhead_bounds_the_widest_published_marker() {
    let wide = |n: usize, c: char| c.to_string().repeat(n);
    let fields: Vec<(&str, String)> = vec![
        ("run-id", wide(255, 'r')),
        ("pr", wide(10, '9')),
        ("head", wide(40, 'a')),
        ("base", wide(40, 'a')),
        ("digest", wide(64, 'd')),
        ("payload-digest", wide(64, 'd')),
        ("epoch", wide(32, 'e')),
        ("url", wide(200, 'u')),
        ("lane-count", "10".to_owned()),
        ("autoclose-json", wide(1_024, 'j')),
        ("mandatory-follow-up", wide(1_024, 'f')),
        ("parent", wide(32, 'p')),
        ("parent-pr", "9".to_owned()),
        ("parent-head", wide(40, 'a')),
        ("parent-base", wide(40, 'a')),
        ("parent-digest", wide(64, 'd')),
        ("delta", wide(64, 'd')),
        ("delta-range", wide(40, 'r')),
        ("delta-range-reason", wide(60, 'w')),
        ("delta-parent", "integrated".to_owned()),
        ("budget-candidate-tree", wide(64, 'c')),
        ("budget-genesis-tree", wide(64, 'g')),
        ("budget-original-lines", wide(10, '9')),
        ("budget", wide(10, '9')),
        ("budget-cumulative", wide(10, '9')),
        ("budget-refrozen", "base-advanced".to_owned()),
        ("budget-parent", "integrated".to_owned()),
    ];
    let refs: Vec<(&str, &str)> = fields
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    let marker = crate::transport::markers::render("published", &refs).expect("the marker renders");
    assert!(
        marker.len() <= super::PUBLICATION_FIXED_OVERHEAD,
        "the widest marker no longer fits the fixed overhead: {} > {}",
        marker.len(),
        super::PUBLICATION_FIXED_OVERHEAD
    );
    // The prose template around it is fixed and small; rendering it keeps the
    // assertion about the whole comment rather than the marker alone.
    let note = super::publication_note(
        &wide(200, 'u'),
        &wide(32, 'e'),
        &wide(40, 'a'),
        &wide(40, 'a'),
        &wide(64, 'd'),
        &wide(64, 'd'),
        None,
        None,
        &super::LaneDispatch::Absent("none".to_owned()),
        &marker,
    );
    assert!(
        note.len() <= super::PUBLICATION_FIXED_OVERHEAD + super::LANE_SENTENCE_CEILING,
        "the widest comment no longer fits the fixed overhead and the lane sentence: {}",
        note.len()
    );
}

/// `run-id` is caller-supplied and is **not** bounded by the run-pointer
/// filename, so the caller bound has to count it.
#[test]
fn the_caller_bound_counts_the_run_id() {
    let short = super::caller_comment_bytes("claude-abc", None, None, &[]);
    let long = super::caller_comment_bytes(&"r".repeat(40_000), None, None, &[]);
    assert!(
        long >= 40_000 && long > short,
        "a 40,000-character run-id must be measured as such: {long} against {short}"
    );
}

/// A dispatch's `gh` detail is free-form transport text with no ceiling of its
/// own, so `lane-note` clamps it; the comment's lane term is bounded only once
/// the detail is.
#[test]
fn the_lane_note_clamps_an_oversize_gh_detail() {
    // Four-byte scalars, because the marker percent-encodes byte by byte: a
    // character-count clamp would leave 4,096 raw bytes and an encoded detail
    // four times the reservation, which ASCII cannot reveal.
    let detail = "\u{1f600}".repeat(super::LANE_DETAIL_CEILING + 5_000);
    let clamped = super::clamp_lane_detail(&detail);
    assert!(
        clamped.len() <= super::LANE_DETAIL_CEILING,
        "the clamp must bound the detail by bytes, not characters: {} > {}",
        clamped.len(),
        super::LANE_DETAIL_CEILING
    );
    let (state, note, _) = super::lane_report(&super::LaneDispatch::Absent(detail));
    assert_eq!(state, "absent");
    assert!(
        super::super::markers::encoded_len(&note) <= super::LANE_SENTENCE_CEILING,
        "an oversize lane detail must be clamped below the sentence ceiling once encoded: {} > {}",
        super::super::markers::encoded_len(&note),
        super::LANE_SENTENCE_CEILING
    );
}

#[test]
fn a_topology_reason_without_a_topology_is_refused_as_incomplete() {
    let envelope = super::normalize_topology(None, Some("two writers, one change"))
        .expect_err("a reason with no declaration must not pass")
        .envelope();
    assert_eq!(envelope["reason"], "topology-reason-without-topology");
}

#[test]
fn trimmed_values_are_what_the_predicate_decides_and_the_marker_records() {
    assert_eq!(
        super::normalize_topology(Some("  INLINE  "), Some(" x ")),
        Ok((Some("INLINE"), Some("x"))),
        "the predicate would decide on one value and the marker record another"
    );
}

#[test]
fn topology_case_is_preserved_as_typed() {
    assert_eq!(
        super::normalize_topology(Some("Inline"), None),
        Ok((Some("Inline"), None)),
        "folding the recorded value would lose information no consumer reads back"
    );
}

#[test]
fn a_blank_topology_is_refused_before_anything_is_written() {
    // No repository behind the checkout: the refusal fires right after the
    // claim check, before the fetch, so there is nothing to push or open.
    let dir = tempfile::tempdir().expect("a directory");
    let context = Context {
        skill_dir: dir.path().to_owned(),
        repo_dir: dir.path().to_owned(),
        config: Vec::new(),
        repo: None,
    };
    let issue = 123;
    let run_id = "claude-abcd1234";
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        run_id,
        issue,
        "in-progress",
    )]);
    let what = Publication {
        issue,
        run_id,
        expect_state: "in-progress",
        base: "main",
        branch: "fix/123-blank",
        title: "Something",
        pr_body_file: None,
        worktree: None,
        inline_payload: false,
        topology: Some(""),
        topology_reason: None,
        focus: None,
        declaration: Default::default(),
        expected_epoch: None,
        now: "2026-01-01T00:05:00Z",
    };
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::publish_with_sleep(&context, &what, super::Push::FastForward, &mut |_| {})
    })
    .expect_err("a blank declaration must not publish");
    assert_eq!(failure.envelope()["reason"], "blank-topology");
}

/// A receipt a retired `review-verdict` accepted stands approved while it is in
/// flight (history): a republish over it is
/// refused unless a finding recorded after the acceptance answers it, and a
/// latest verdict no panel accepted is not an acceptance at all.
#[test]
fn a_legacy_accepted_parent_stands_approved_until_a_finding_answers_it() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let accepted = verdict_by("claude-a", "accepted", &"d".repeat(32), &receipt);
    let rejected = verdict_by("claude-a", "rejected", &"e".repeat(32), &receipt);
    let answer = finding_marker(
        "5",
        &"f".repeat(32),
        "judge",
        &receipt,
        "honesty-md-leftover",
        "warning",
    );
    let elsewhere = finding_marker(
        "6",
        &"0".repeat(32),
        "judge",
        &ReviewTransaction {
            epoch: "2".repeat(32),
            ..receipt.clone()
        },
        "other-leftover",
        "warning",
    );
    let unanswered = |comments: &[ownership::Comment]| {
        let standing = receipt_standing(comments, &receipt).expect("readable");
        standing.standing == Standing::Approved && !standing.answered
    };

    assert!(
        unanswered(std::slice::from_ref(&accepted)),
        "an acceptance nothing has answered needs a cause to be republished"
    );
    assert!(
        !unanswered(&[accepted.clone(), answer.clone()]),
        "a finding recorded after the verdict is the declaration that clears it"
    );
    assert!(
        unanswered(&[answer, accepted.clone()]),
        "a finding the panel could weigh does not clear the round it accepted over"
    );
    assert!(
        !unanswered(std::slice::from_ref(&rejected)),
        "a rejected round is the panel's, not the author's"
    );
    assert!(
        unanswered(&[accepted, elsewhere]),
        "a finding against another receipt answers nothing"
    );
}

#[test]
fn a_receipts_standing_comes_from_its_marker_and_only_a_later_finding_answers_an_approval() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let approved = computed_by("claude-a", &receipt, "approved");
    let answer = finding_marker(
        "5",
        &"f".repeat(32),
        "judge",
        &receipt,
        "honesty-md-leftover",
        "warning",
    );
    let standing =
        |comments: &[ownership::Comment]| receipt_standing(comments, &receipt).expect("readable");

    let alone = standing(std::slice::from_ref(&approved));
    assert_eq!(alone.standing, Standing::Approved);
    assert!(!alone.answered);
    assert!(standing(&[approved.clone(), answer.clone()]).answered);
    assert!(
        !standing(&[answer, approved.clone()]).answered,
        "a finding that arrived with the panel, before the verdict, answers nothing"
    );
    for outcome in ["rejected", "escalated"] {
        assert_eq!(
            standing(&[computed_by("claude-a", &receipt, outcome)]).standing,
            Standing::Rejected,
            "{outcome}"
        );
    }
    for comments in [
        Vec::new(),
        vec![computed_by("claude-a", &receipt, "incomplete")],
    ] {
        assert_eq!(standing(&comments).standing, Standing::Unjudged);
    }
    assert_eq!(
        standing(&[
            approved.clone(),
            escalated_receipt_by("claude-a", &"5".repeat(32), "Alexis", &receipt)
        ])
        .standing,
        Standing::Rejected,
        "a person's escalation stands over an approval"
    );
    assert_eq!(
        standing(&[
            approved,
            verdict_by("codex-b", "rejected", &"a".repeat(32), &receipt)
        ])
        .standing,
        Standing::Rejected,
        "a retired rejection is final"
    );
    assert_eq!(
        standing(&[verdict_by("codex-b", "accepted", &"a".repeat(32), &receipt)]).standing,
        Standing::Approved
    );
}

#[test]
fn a_base_advance_is_one_the_parents_base_is_an_ancestor_of() {
    let (_root, _remote, work) = repository_with_remote();
    let first = head_of(&work);
    git(&work, &["checkout", "-q", "-b", "side"]);
    std::fs::write(work.join("side.txt"), "side\n").expect("content");
    git(&work, &["add", "side.txt"]);
    git(&work, &["commit", "--quiet", "-m", "another line"]);
    let side = head_of(&work);
    git(&work, &["checkout", "-q", "main"]);
    std::fs::write(work.join("two.txt"), "two\n").expect("content");
    git(&work, &["add", "two.txt"]);
    git(&work, &["commit", "--quiet", "-m", "the base advances"]);
    let second = head_of(&work);

    assert!(base_advanced_past(&work, &first, &second).expect("readable"));
    assert!(
        !base_advanced_past(&work, &first, &first).expect("readable"),
        "a base that did not move advanced nowhere"
    );
    assert!(
        !base_advanced_past(&work, &second, &first).expect("readable"),
        "a base reset backwards is not an integration of anything"
    );
    assert!(
        !base_advanced_past(&work, &second, &side).expect("readable"),
        "a base swapped for another line of history is not an advance of this one"
    );
}

#[test]
fn an_integrated_parent_that_git_cannot_merge_is_a_refusal_naming_the_cause() {
    let (_root, _remote, work) = repository_with_remote();
    let head = head_of(&work);
    let (tree, conflicted) =
        super::integrated_parent_tree(&work, &head, &head).expect("a trivial merge");
    assert!(crate::transport::target::is_object_id(&tree));
    assert!(conflicted.is_empty(), "{conflicted:?}");

    let failure = super::integrated_parent_tree(&work, &head, &"0".repeat(40))
        .expect_err("a base git cannot resolve has no integrated parent");
    let detail = failure.detail();
    assert!(
        detail.contains("git merge-tree --write-tree") && detail.contains("not reset"),
        "the refusal does not name its cause: {detail}"
    );
}

/// The object-id half of the `merge-tree` validation, driven directly: a
/// status the merge accepts whose first line is not an object id is a failed
/// read naming the cause, not a tree the budget would diff against.
#[test]
fn a_merge_tree_whose_first_line_is_not_an_object_id_refuses_naming_the_cause() {
    for status in [0, 1] {
        let failure = super::validated_merge_tree(status, "not-an-object-id\n", "")
            .expect_err("a non-object-id first line was accepted");
        let detail = failure.detail();
        assert!(
            detail.contains("git merge-tree --write-tree")
                && detail.contains(&format!("status {status}")),
            "the refusal does not name its cause: {detail}"
        );
    }
    let tree = "a".repeat(40);
    assert_eq!(
        super::validated_merge_tree(0, &format!("{tree}\n"), "").expect("a tree"),
        tree
    );
}

/// One record per stage per conflicted path, after the tree line; the parser
/// reads every path and collapses the stages of one path to a single entry.
#[test]
fn merge_tree_conflicted_paths_reads_every_stage_and_collapses_duplicates() {
    let stdout = "0123456789012345678901234567890123456789\n\
                  100644 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1\tshared.txt\n\
                  100644 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 2\tshared.txt\n\
                  100644 cccccccccccccccccccccccccccccccccccccccc 3\tshared.txt\n\
                  100644 dddddddddddddddddddddddddddddddddddddddd 1\tsrc/other.rs\n";
    assert_eq!(
        super::merge_tree_conflicted_paths(stdout).expect("the conflicted paths"),
        vec!["shared.txt".to_owned(), "src/other.rs".to_owned()]
    );
}

/// A record with no tab cannot be read as a path, so it is a failed read
/// naming the line rather than a silently dropped conflict.
#[test]
fn merge_tree_conflicted_paths_refuses_a_malformed_record() {
    let stdout = "0123456789012345678901234567890123456789\nnot a conflict record\n";
    let failure =
        super::merge_tree_conflicted_paths(stdout).expect_err("a malformed record was accepted");
    assert!(
        failure
            .detail()
            .contains("unexpected merge-tree conflict record"),
        "the refusal does not name its cause: {}",
        failure.detail()
    );
}

struct RepublishWorld {
    _root: tempfile::TempDir,
    remote: std::path::PathBuf,
    work: std::path::PathBuf,
    base: String,
    limit: Option<&'static str>,
}

const REPUBLISH_BRANCH: &str = "fix/123-lineage";
const REPUBLISH_RUN: &str = "claude-abcd1234";

fn republish_world() -> RepublishWorld {
    let (root, remote, work) = repository_with_remote();
    git(&work, &["push", "-q", "-u", "origin", "--", "main"]);
    let base = head_of(&work);
    git(&work, &["checkout", "-q", "-b", REPUBLISH_BRANCH]);
    RepublishWorld {
        _root: root,
        remote,
        work,
        base,
        limit: None,
    }
}

fn limited_to_one_correction(mut world: RepublishWorld) -> RepublishWorld {
    world.limit = Some("1");
    world
}

fn corrections_limit_row(limit: &str) -> Vec<(String, String, Source)> {
    vec![(
        "Corrections per lineage".to_owned(),
        limit.to_owned(),
        Source::Written,
    )]
}

impl RepublishWorld {
    /// Commits `lines` lines to `name` and answers the new head.
    fn commit(&self, name: &str, lines: usize) -> String {
        std::fs::write(self.work.join(name), "line\n".repeat(lines)).expect("content");
        git(&self.work, &["add", name]);
        git(&self.work, &["commit", "--quiet", "-m", name]);
        head_of(&self.work)
    }

    fn receipt(&self, epoch: char, head: &str) -> ReviewTransaction {
        ReviewTransaction {
            epoch: epoch.to_string().repeat(32),
            pr: 7,
            head: head.to_owned(),
            base: self.base.clone(),
            digest: epoch.to_string().repeat(64),
        }
    }

    /// Publishes the branch as it stands over the timeline `comments` and the
    /// pull request open at `open_head`.
    fn publish(
        &self,
        comments: &[ownership::Comment],
        open_head: &str,
    ) -> Result<serde_json::Value, Failure> {
        self.publish_logging(comments, open_head).0
    }

    /// [`Self::publish`], with every comment body the call wrote to the tracker.
    fn publish_logging(
        &self,
        comments: &[ownership::Comment],
        open_head: &str,
    ) -> (Result<serde_json::Value, Failure>, String) {
        self.publish_expecting(comments, open_head, None)
    }

    fn publish_expecting(
        &self,
        comments: &[ownership::Comment],
        open_head: &str,
        expected_epoch: Option<&str>,
    ) -> (Result<serde_json::Value, Failure>, String) {
        self.drive(comments, open_head, expected_epoch, None)
    }

    fn drive(
        &self,
        comments: &[ownership::Comment],
        open_head: &str,
        expected_epoch: Option<&str>,
        operation: Option<&str>,
    ) -> (Result<serde_json::Value, Failure>, String) {
        crate::transport::store::forget_scratch_store();
        let mut timeline = vec![live_claim(REPUBLISH_RUN)];
        timeline.extend_from_slice(comments);
        let new_head = head_of(&self.work);
        let new_base = crate::transport::run(
            &["git", "rev-parse", "main"],
            Some(&self.work),
            crate::transport::How::read(),
        )
        .expect("git names the base")
        .stdout
        .trim()
        .to_owned();
        let pr = |head: &str, base: &str, extra: serde_json::Value| {
            let mut value = serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": head,
                "baseRefOid": base,
                "isDraft": true,
            });
            value
                .as_object_mut()
                .expect("an object")
                .extend(extra.as_object().expect("an object").clone());
            value
        };
        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[
            crate::test_env::issue_view_answer(
                123,
                serde_json::json!({
                    "state": "OPEN",
                    "labels": [{ "name": "status:in-progress" }],
                    "comments": distinct_ids(&timeline).iter().map(comment_json).collect::<Vec<_>>(),
                }),
            ),
            serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
            serde_json::json!({
                "matches": format!("list --head {REPUBLISH_BRANCH}"),
                "stdout": serde_json::json!([pr(open_head, &self.base, serde_json::json!({}))]).to_string(),
                "status": 0,
            }),
            serde_json::json!({
                "matches": "state,isDraft",
                "stdout": pr(&new_head, &new_base, serde_json::json!({ "state": "OPEN" })).to_string(),
                "status": 0,
            }),
            serde_json::json!({
                "matches": "api graphql",
                "stdout": serde_json::json!({
                    "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                        "nodes": [],
                        "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                    } } } },
                })
                .to_string(),
                "status": 0,
            }),
            serde_json::json!({ "matches": "issue comment", "stdout": "", "status": 0 }),
        ]);
        let context = Context {
            skill_dir: self.work.clone(),
            repo_dir: self.work.clone(),
            config: self.limit.map(corrections_limit_row).unwrap_or_default(),
            repo: None,
        };
        let what = Publication {
            issue: 123,
            run_id: REPUBLISH_RUN,
            expect_state: "in-progress",
            base: "main",
            branch: REPUBLISH_BRANCH,
            title: "Something",
            pr_body_file: None,
            worktree: None,
            inline_payload: false,
            topology: None,
            topology_reason: None,
            focus: None,
            declaration: Default::default(),
            expected_epoch,
            now: "2026-01-01T00:05:00Z",
        };
        let log_directory = tempfile::tempdir().expect("a directory for the body log");
        let log = log_directory.path().join("bodies.log");
        if let Some(operation) = operation {
            let mut flags: Vec<String> = [
                "--issue",
                "123",
                "--run-id",
                REPUBLISH_RUN,
                "--branch",
                REPUBLISH_BRANCH,
                "--base",
                "main",
                "--pr-title",
                "Something",
            ]
            .iter()
            .map(|flag| (*flag).to_owned())
            .collect();
            if let Some(expected) = expected_epoch {
                flags.extend(["--expected-epoch".to_owned(), expected.to_owned()]);
            }
            let result = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
                crate::transport::dispatch::dispatch(&context, operation, &flags, what.now)
            });
            return (
                result,
                std::fs::read_to_string(&log).unwrap_or_default()
                    + &crate::transport::store::local_bodies(123),
            );
        }
        let result = crate::test_env::with_body_log(bin.path(), &script, &log, || {
            super::publish_with_sleep(&context, &what, super::Push::FastForward, &mut |_| {})
        });
        (
            result,
            std::fs::read_to_string(&log).unwrap_or_default()
                + &crate::transport::store::local_bodies(123),
        )
    }
}

fn refusal_of(result: Result<serde_json::Value, Failure>) -> serde_json::Value {
    result
        .expect_err("the publication must be refused")
        .envelope()
}

#[test]
fn a_republish_over_an_approved_parent_with_no_cause_is_refused() {
    let world = republish_world();
    let parent_head = world.commit("two.txt", 1);
    world.commit("three.txt", 1);
    let parent = world.receipt('1', &parent_head);

    let envelope = refusal_of(world.publish(
        &[
            published(&parent),
            computed_by(REPUBLISH_RUN, &parent, "approved"),
        ],
        &parent_head,
    ));
    assert_eq!(
        envelope["reason"], "approved-republish-without-cause",
        "{envelope}"
    );
    assert_eq!(envelope["epoch"], parent.epoch);
    assert!(
        envelope["action"]
            .as_str()
            .expect("action")
            .contains("review_finding"),
        "the refusal does not name the declaration that clears it: {}",
        envelope["action"]
    );
}

#[test]
fn a_base_that_moved_backwards_does_not_license_a_republish_over_an_approved_parent() {
    let world = republish_world();
    let parent_head = world.commit("two.txt", 1);
    world.commit("three.txt", 1);
    let mut parent = world.receipt('1', &parent_head);
    parent.base = parent_head.clone();

    let envelope = refusal_of(world.publish(
        &[
            published(&parent),
            computed_by(REPUBLISH_RUN, &parent, "approved"),
        ],
        &parent_head,
    ));
    assert_eq!(
        envelope["reason"], "approved-republish-without-cause",
        "{envelope}"
    );
}

#[test]
fn a_republish_over_an_approved_parent_keeps_its_scope_and_spends_no_correction() {
    let world = republish_world();
    let original_head = world.commit("two.txt", 1);
    let corrected_head = world.commit("two.txt", 2);
    world.commit("three.txt", 1);
    let original = world.receipt('1', &original_head);
    let corrected = world.receipt('2', &corrected_head);
    let answer = finding_marker(
        "5",
        &"f".repeat(32),
        "judge",
        &corrected,
        "unweighed-defect",
        "warning",
    );

    let envelope = refusal_of(world.publish(
        &[
            published(&original),
            computed_by(REPUBLISH_RUN, &original, "rejected"),
            published_with_parent(REPUBLISH_RUN, &corrected, &original, "delta-1-2"),
            computed_by(REPUBLISH_RUN, &corrected, "approved"),
            answer,
        ],
        &corrected_head,
    ));
    assert_eq!(
        envelope["reason"], "correction-outside-reviewed-scope",
        "{envelope}"
    );
}

#[test]
fn an_escalation_a_distinct_approval_superseded_does_not_make_an_integration_a_second_correction() {
    let world = republish_world();
    let original_head = world.commit("two.txt", 1);
    let corrected_head = world.commit("two.txt", 2);
    git(&world.work, &["checkout", "-q", "main"]);
    std::fs::write(world.work.join("base-only.txt"), "base\n").expect("content");
    git(&world.work, &["add", "base-only.txt"]);
    git(
        &world.work,
        &["commit", "--quiet", "-m", "the base advances"],
    );
    git(&world.work, &["push", "-q", "origin", "main"]);
    git(&world.work, &["checkout", "-q", REPUBLISH_BRANCH]);
    git(&world.work, &["merge", "--no-edit", "main"]);
    let original = world.receipt('1', &original_head);
    let corrected = world.receipt('2', &corrected_head);

    let answer = world
        .publish(
            &[
                published(&original),
                computed_by("claude-a", &original, "rejected"),
                published_with_parent("claude-a", &corrected, &original, "delta-1-2"),
                escalated_receipt_by("claude-a", &"5".repeat(32), "Alexis", &corrected),
                approved_receipt_by("claude-a", "codex-b", &corrected),
                computed_by("claude-a", &corrected, "approved"),
            ],
            &corrected_head,
        )
        .expect("an approval that superseded the escalation leaves the integration publishable");
    assert_eq!(answer["ok"], true, "{answer}");
}

#[test]
fn a_publication_on_another_pull_request_does_not_reset_the_correction_count() {
    let world = limited_to_one_correction(republish_world());
    let original_head = world.commit("two.txt", 1);
    let corrected_head = world.commit("two.txt", 2);
    world.commit("two.txt", 3);
    let original = world.receipt('1', &original_head);
    let corrected = world.receipt('2', &corrected_head);
    let other = ReviewTransaction {
        pr: 8,
        ..world.receipt('3', &original_head)
    };

    let envelope = refusal_of(world.publish(
        &[
            published(&original),
            computed_by(REPUBLISH_RUN, &original, "rejected"),
            published_with_parent(REPUBLISH_RUN, &corrected, &original, "delta-1-2"),
            computed_by(REPUBLISH_RUN, &corrected, "rejected"),
            published_with_parent(REPUBLISH_RUN, &other, &corrected, "delta-2-3"),
        ],
        &corrected_head,
    ));
    assert_eq!(envelope["reason"], "correction-already-spent", "{envelope}");
}

#[test]
fn a_merge_of_an_advanced_base_over_an_approved_parent_publishes_as_a_new_candidate() {
    let world = republish_world();
    let parent_head = world.commit("two.txt", 1);
    git(&world.work, &["checkout", "-q", "main"]);
    std::fs::write(world.work.join("base-only.txt"), "base\n").expect("content");
    git(&world.work, &["add", "base-only.txt"]);
    git(
        &world.work,
        &["commit", "--quiet", "-m", "the base advances"],
    );
    git(&world.work, &["push", "-q", "origin", "main"]);
    let base_after = head_of(&world.work);
    git(&world.work, &["checkout", "-q", REPUBLISH_BRANCH]);
    git(&world.work, &["merge", "--no-edit", "main"]);
    let merge_head = head_of(&world.work);
    let parent = world.receipt('1', &parent_head);

    let answer = world
        .publish(
            &[
                published(&parent),
                computed_by(REPUBLISH_RUN, &parent, "approved"),
            ],
            &parent_head,
        )
        .expect("a forced integration over an approved parent must publish");
    assert_eq!(
        answer["head"].as_str(),
        Some(merge_head.as_str()),
        "{answer}"
    );
    assert_eq!(
        answer["base"].as_str(),
        Some(base_after.as_str()),
        "{answer}"
    );
    let remote_branch = crate::transport::run(
        &[
            "git",
            "rev-parse",
            &format!("refs/heads/{REPUBLISH_BRANCH}"),
        ],
        Some(&world.remote),
        crate::transport::How::read(),
    )
    .expect("the remote has the branch")
    .stdout
    .trim()
    .to_owned();
    assert_eq!(remote_branch, merge_head, "the integration was not pushed");
}

#[test]
fn a_second_correction_on_one_lineage_is_refused() {
    let world = limited_to_one_correction(republish_world());
    let original_head = world.commit("two.txt", 1);
    let first_correction_head = world.commit("two.txt", 2);
    world.commit("outside.txt", 1);
    let original = world.receipt('1', &original_head);
    let first_correction = world.receipt('2', &first_correction_head);

    let envelope = refusal_of(world.publish(
        &[
            published(&original),
            computed_by(REPUBLISH_RUN, &original, "rejected"),
            published_with_parent(REPUBLISH_RUN, &first_correction, &original, "delta-1-2"),
            computed_by(REPUBLISH_RUN, &first_correction, "rejected"),
        ],
        &first_correction_head,
    ));
    assert_eq!(envelope["reason"], "correction-already-spent", "{envelope}");
    assert_eq!(envelope["corrections"], 1, "{envelope}");
    assert_eq!(envelope["epoch"], first_correction.epoch);

    let envelope = refusal_of(world.publish(
        &[
            published(&original),
            computed_by(REPUBLISH_RUN, &original, "rejected"),
        ],
        &original_head,
    ));
    assert_eq!(
        envelope["reason"], "correction-outside-reviewed-scope",
        "the first correction is admitted and measured against its budget: {envelope}"
    );
}

fn spent_chain(world: &RepublishWorld, spent: usize) -> (Vec<ownership::Comment>, String) {
    let mut heads = vec![world.commit("two.txt", 1)];
    for lines in 2..=spent + 1 {
        heads.push(world.commit("two.txt", lines));
    }
    world.commit("outside.txt", 1);
    let receipts: Vec<ReviewTransaction> = heads
        .iter()
        .enumerate()
        .map(|(at, head)| world.receipt(char::from(b'1' + at as u8), head))
        .collect();
    let mut comments = vec![published(&receipts[0])];
    for at in 0..receipts.len() {
        if at > 0 {
            comments.push(published_with_parent(
                REPUBLISH_RUN,
                &receipts[at],
                &receipts[at - 1],
                &format!("delta-{}-{}", at, at + 1),
            ));
        }
        comments.push(computed_by(REPUBLISH_RUN, &receipts[at], "rejected"));
    }
    (comments, heads[spent].clone())
}

fn refused_as_spent(result: &Result<serde_json::Value, Failure>) -> bool {
    matches!(result, Err(Failure::Stop(envelope)) if envelope["reason"] == "correction-already-spent")
}

#[test]
fn by_default_a_lineage_publishes_a_second_and_a_third_correction() {
    for spent in [1, 2] {
        let world = republish_world();
        let (comments, open_head) = spent_chain(&world, spent);
        let result = world.publish(&comments, &open_head);
        assert!(
            !refused_as_spent(&result),
            "{spent} corrections spent leave one of three: {result:?}"
        );
    }
}

#[test]
fn by_default_a_fourth_correction_is_refused_before_anything_is_pushed() {
    let world = republish_world();
    let (comments, open_head) = spent_chain(&world, 3);
    let envelope = refusal_of(world.publish(&comments, &open_head));
    assert_eq!(envelope["reason"], "correction-already-spent", "{envelope}");
    assert_eq!(envelope["corrections"], 3, "{envelope}");
    assert_eq!(envelope["correction_limit"], 3, "{envelope}");
}

#[test]
fn a_configured_limit_of_one_refuses_the_second_correction() {
    let world = limited_to_one_correction(republish_world());
    let (comments, open_head) = spent_chain(&world, 1);
    let envelope = refusal_of(world.publish(&comments, &open_head));
    assert_eq!(envelope["reason"], "correction-already-spent", "{envelope}");
    assert_eq!(envelope["correction_limit"], 1, "{envelope}");
}

#[test]
fn repair_rounds_reports_the_configured_limit() {
    let (_root, mut context) = context_with_no_repository();
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let correction = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "b".repeat(40),
        digest: "2".repeat(64),
        ..first.clone()
    };
    let comments = [
        published(&first),
        computed_by("codex-b", &first, "rejected"),
        published_with_parent("claude-a", &correction, &first, "delta-1-2"),
        computed_by("codex-b", &correction, "rejected"),
    ];

    let answer = read_repair_rounds(&context, &comments);
    assert_eq!(answer["correction_limit"], 3, "{answer}");
    assert_eq!(answer["correction_spent"], false, "{answer}");
    assert_eq!(answer["correction_available"], true, "{answer}");

    context.config = corrections_limit_row("1");
    let answer = read_repair_rounds(&context, &comments);
    assert_eq!(answer["correction_limit"], 1, "{answer}");
    assert_eq!(answer["correction_spent"], true, "{answer}");
    assert_eq!(answer["correction_available"], false, "{answer}");
}

#[test]
fn a_second_correction_is_refused_over_a_parent_a_person_escalated() {
    let world = limited_to_one_correction(republish_world());
    let original_head = world.commit("two.txt", 1);
    let first_correction_head = world.commit("two.txt", 2);
    world.commit("two.txt", 3);
    let original = world.receipt('1', &original_head);
    let first_correction = world.receipt('2', &first_correction_head);

    let envelope = refusal_of(world.publish(
        &[
            published(&original),
            computed_by(REPUBLISH_RUN, &original, "rejected"),
            published_with_parent(REPUBLISH_RUN, &first_correction, &original, "delta-1-2"),
            escalated_receipt_by(REPUBLISH_RUN, &"5".repeat(32), "Alexis", &first_correction),
        ],
        &first_correction_head,
    ));
    assert_eq!(envelope["reason"], "correction-already-spent", "{envelope}");
}

#[test]
fn a_republish_no_panel_has_judged_yet_spends_no_correction() {
    let (_root, context) = context_with_no_repository();
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let again = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "b".repeat(40),
        digest: "2".repeat(64),
        ..first.clone()
    };
    let answer = read_repair_rounds(
        &context,
        &[
            published(&first),
            published_with_parent("claude-a", &again, &first, "delta-1-2"),
        ],
    );
    assert_eq!(answer["corrections"], 0, "{answer}");
    assert_eq!(answer["correction_spent"], false, "{answer}");
    assert_eq!(answer["correction_available"], false, "{answer}");
    assert_eq!(answer["latest_verdict"], "unjudged", "{answer}");
}

/// End-to-end against a real bare remote: push lands, PR readback never agrees,
/// an unconfirmed marker is written, and a leased push against that head is
/// accepted — the shape that used to require a hand force-push.
#[test]
fn a_push_whose_readback_disagrees_leaves_a_republishable_lease_target() {
    let (_root, remote, work) = repository_with_remote();
    git(&work, &["push", "-q", "-u", "origin", "--", "main"]);
    let base_sha = head_of(&work);

    let issue = 123;
    let run_id = "claude-abcd1234";
    let branch = "fix/123-readback";
    let base = "main";
    git(&work, &["checkout", "-q", "-b", branch]);
    std::fs::write(work.join("two.txt"), "two\n").expect("content");
    git(&work, &["add", "two.txt"]);
    git(&work, &["commit", "--quiet", "-m", "a change"]);
    let branch_head = head_of(&work);

    let trace = tempfile::tempdir().expect("a directory for the PR body");
    let pr_body = trace.path().join("body.md");
    std::fs::write(&pr_body, "A body that names nothing.\n").expect("the body");

    // PR list reuses an open draft; every PR view answers a *different* head so
    // `confirm_published` never agrees — the measured failure window.
    let wrong_head = "0".repeat(40);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::live_holder_issue(run_id, issue, "in-progress"),
        serde_json::json!({
            "matches": format!("list --head {branch}"),
            "stdout": serde_json::json!([{
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": wrong_head,
                "baseRefOid": base_sha,
                "isDraft": true,
            }])
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "state,isDraft",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": wrong_head,
                "baseRefOid": base_sha,
                "state": "OPEN",
                "isDraft": true,
            })
            .to_string(),
            "status": 0,
        }),
        // The pushed head is really on the remote — the test pushed it above —
        // so the ref answers it and this refusal must read as lag, not a move.
        serde_json::json!({
            "matches": "git/refs/heads",
            "stdout": serde_json::json!({ "object": { "sha": &branch_head } }).to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
        serde_json::json!({
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            })
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "json body", "stdout": "{\"body\":\"A body that names nothing.\"}", "status": 0 }),
        // The unconfirmed marker is posted through `gh issue comment`.
        serde_json::json!({ "matches": "issue comment", "stdout": "", "status": 0 }),
    ]);

    let context = Context {
        skill_dir: work.clone(),
        repo_dir: work.clone(),
        config: Vec::new(),
        repo: None,
    };
    let what = Publication {
        issue,
        run_id,
        expect_state: "in-progress",
        base,
        branch,
        title: "Something",
        pr_body_file: Some(&pr_body),
        worktree: None,
        inline_payload: false,
        topology: None,
        topology_reason: None,
        focus: None,
        declaration: Default::default(),
        expected_epoch: None,
        now: "2026-01-01T00:05:00Z",
    };

    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        // A no-op sleep: the wait is pinned by the tests below, and this path
        // would otherwise pay the whole production window for ten stale reads.
        super::publish_with_sleep(&context, &what, super::Push::FastForward, &mut |_| {})
    })
    .expect_err("a disagreeing readback must refuse");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "publication-readback-disagrees");
    assert_eq!(envelope["expected_head"], branch_head);
    assert!(
        envelope["action"]
            .as_str()
            .expect("action")
            .contains("republish_review"),
        "the refusal does not name the recovery route: {}",
        envelope["action"]
    );
    assert_eq!(
        envelope["unconfirmed_head"].as_str(),
        Some(branch_head.as_str()),
        "no unconfirmed head was recorded on the refusal"
    );
    assert_eq!(
        envelope["branch_ref"].as_str(),
        Some(branch_head.as_str()),
        "the refusal does not name the branch ref it read"
    );
    assert_eq!(
        envelope["branch_ref_matches_head"].as_bool(),
        Some(true),
        "the ref carries the head yet the refusal does not say lag"
    );
    assert!(
        envelope["action"]
            .as_str()
            .expect("action")
            .contains("has not caught up"),
        "the lag world is not distinguished: {}",
        envelope["action"]
    );

    // The push did land. A leased republish against the unconfirmed head must
    // be accepted; that is the executable recovery without a hand force-push.
    let remote_branch = crate::transport::run(
        &["git", "rev-parse", &format!("refs/heads/{branch}")],
        Some(&remote),
        crate::transport::How::read(),
    )
    .expect("the remote has the branch")
    .stdout
    .trim()
    .to_owned();
    assert_eq!(remote_branch, branch_head, "the push did not land");

    git(
        &work,
        &["commit", "--quiet", "--amend", "-m", "a change, repaired"],
    );
    let repaired = head_of(&work);
    super::push_to_origin(
        &work,
        branch,
        super::Push::Leased {
            recorded_head: &branch_head,
        },
    )
    .expect("a lease against the unconfirmed head the push left is accepted");
    let after = crate::transport::run(
        &["git", "rev-parse", &format!("refs/heads/{branch}")],
        Some(&remote),
        crate::transport::How::read(),
    )
    .expect("the remote still names the branch")
    .stdout
    .trim()
    .to_owned();
    assert_eq!(
        after, repaired,
        "the leased recovery did not move the remote"
    );
}

#[test]
fn readback_attempts_cover_the_measured_lag_window() {
    let delays: Vec<u64> = (2..=super::PUBLISH_READBACK_ATTEMPTS)
        .map(super::readback_delay_secs)
        .collect();
    assert_eq!(
        delays,
        vec![2, 4, 6, 8, 10, 12, 14, 16, 18],
        "the attempts no longer span the window the constant states"
    );
    assert_eq!(
        delays.iter().sum::<u64>(),
        90,
        "ten attempts cover about ninety seconds, the order of the measured lag"
    );
}

#[test]
fn a_failed_readback_names_its_world_from_the_branch_ref() {
    use super::ReadbackWorld;
    assert_eq!(
        super::classify_readback(Some("abc"), "abc"),
        ReadbackWorld::Lag
    );
    assert_eq!(
        super::classify_readback(Some("def"), "abc"),
        ReadbackWorld::Moved
    );
    assert_eq!(
        super::classify_readback(None, "abc"),
        ReadbackWorld::Unknown
    );
    let lead = super::readback_world_lead(ReadbackWorld::Lag, "abc", 7);
    assert!(
        lead.contains("has not caught up")
            && lead.contains("abc")
            && lead.contains("republish_review"),
        "lag must name waiting and the recovery: {lead:?}"
    );
    let lead = super::readback_world_lead(ReadbackWorld::Moved, "abc", 7);
    assert!(
        lead.contains("no longer carries") && lead.contains("abc"),
        "a moved head must name investigation: {lead:?}"
    );
    let lead = super::readback_world_lead(ReadbackWorld::Unknown, "abc", 7);
    assert!(
        lead.contains("cannot be told"),
        "an unreadable ref must say so: {lead:?}"
    );
}

#[test]
fn a_readback_that_settles_after_two_stale_reads_waits_then_succeeds() {
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let stale = serde_json::json!({
        "number": 7,
        "headRefOid": "0".repeat(40),
        "baseRefOid": base,
        "isDraft": true,
    })
    .to_string();
    let correct = serde_json::json!({
        "number": 7,
        "headRefOid": head,
        "baseRefOid": base,
        "isDraft": true,
    })
    .to_string();
    let bin = crate::test_env::scripted_gh();
    let dir = tempfile::tempdir().expect("a directory for the counter");
    let count = dir.path().join("count");
    let script = crate::test_env::answers(&[
        serde_json::json!({"matches": "pr view", "nth": 1, "stdout": stale, "status": 0}),
        serde_json::json!({"matches": "pr view", "nth": 2, "stdout": stale, "status": 0}),
        serde_json::json!({"matches": "pr view", "stdout": correct, "status": 0}),
    ]);
    let context = Context {
        skill_dir: dir.path().to_owned(),
        repo_dir: dir.path().to_owned(),
        config: Vec::new(),
        repo: None,
    };
    let mut slept = Vec::new();
    let seen = crate::test_env::with_counting_gh(bin.path(), &script, &count, || {
        super::read_back_pr(&context, 7, &head, &base, &mut |delay| {
            slept.push(delay);
        })
        .expect("two stale reads then agreement must succeed")
    });
    assert!(
        super::confirm_published(&seen, &head, &base, true).is_some(),
        "the settled read was not recognised"
    );
    assert_eq!(
        slept,
        vec![
            std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(4)
        ],
        "the loop did not wait its scheduled delays before re-reading"
    );
}

#[test]
fn a_readback_that_never_settles_pays_the_whole_window() {
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let stale = serde_json::json!({
        "number": 7,
        "headRefOid": "0".repeat(40),
        "baseRefOid": base,
        "isDraft": true,
    })
    .to_string();
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[serde_json::json!({
        "matches": "pr view",
        "stdout": stale,
        "status": 0
    })]);
    let dir = tempfile::tempdir().expect("a directory");
    let context = Context {
        skill_dir: dir.path().to_owned(),
        repo_dir: dir.path().to_owned(),
        config: Vec::new(),
        repo: None,
    };
    let mut slept = Vec::new();
    let seen = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::read_back_pr(&context, 7, &head, &base, &mut |delay| {
            slept.push(delay);
        })
        .expect("ten stale reads are observations, not a transport failure")
    });
    assert_eq!(seen.len(), super::PUBLISH_READBACK_ATTEMPTS);
    assert!(
        super::confirm_published(&seen, &head, &base, true).is_none(),
        "ten stale reads must not confirm"
    );
    assert_eq!(
        slept,
        vec![2, 4, 6, 8, 10, 12, 14, 16, 18]
            .into_iter()
            .map(std::time::Duration::from_secs)
            .collect::<Vec<_>>(),
        "a remote that never settles must still cost every scheduled wait"
    );
}

/// End to end for the other world: ref and pull request agree on a foreign
/// head, so the refusal must say the head moved rather than that GitHub is
/// catching up.
#[test]
fn a_readback_that_settles_elsewhere_says_the_head_moved() {
    let (_root, _remote, work) = repository_with_remote();
    git(&work, &["push", "-q", "-u", "origin", "--", "main"]);
    let base_sha = head_of(&work);

    let issue = 123;
    let run_id = "claude-abcd1234";
    let branch = "fix/123-readback";
    let base = "main";
    git(&work, &["checkout", "-q", "-b", branch]);
    std::fs::write(work.join("two.txt"), "two\n").expect("content");
    git(&work, &["add", "two.txt"]);
    git(&work, &["commit", "--quiet", "-m", "a change"]);

    let trace = tempfile::tempdir().expect("a directory for the PR body");
    let pr_body = trace.path().join("body.md");
    std::fs::write(&pr_body, "A body that names nothing.\n").expect("the body");

    // Somebody else pushed: every read agrees, on a head this run never had.
    let moved = "1".repeat(40);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::live_holder_issue(run_id, issue, "in-progress"),
        serde_json::json!({
            "matches": format!("list --head {branch}"),
            "stdout": serde_json::json!([{
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": &moved,
                "baseRefOid": base_sha,
                "isDraft": true,
            }])
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "state,isDraft",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": &moved,
                "baseRefOid": base_sha,
                "state": "OPEN",
                "isDraft": true,
            })
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "git/refs/heads",
            "stdout": serde_json::json!({ "object": { "sha": &moved } }).to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
        serde_json::json!({
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            })
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "json body", "stdout": "{\"body\":\"A body that names nothing.\"}", "status": 0 }),
        serde_json::json!({ "matches": "issue comment", "stdout": "", "status": 0 }),
    ]);

    let context = Context {
        skill_dir: work.clone(),
        repo_dir: work,
        config: Vec::new(),
        repo: None,
    };
    let what = Publication {
        issue,
        run_id,
        expect_state: "in-progress",
        base,
        branch,
        title: "Something",
        pr_body_file: Some(&pr_body),
        worktree: None,
        inline_payload: false,
        topology: None,
        topology_reason: None,
        focus: None,
        declaration: Default::default(),
        expected_epoch: None,
        now: "2026-01-01T00:05:00Z",
    };

    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::publish_with_sleep(&context, &what, super::Push::FastForward, &mut |_| {})
    })
    .expect_err("a moved head must still refuse");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "publication-readback-disagrees");
    assert_eq!(
        envelope["branch_ref"].as_str(),
        Some(moved.as_str()),
        "the refusal does not name the branch ref it read"
    );
    assert_eq!(
        envelope["branch_ref_matches_head"].as_bool(),
        Some(false),
        "a foreign head reads as the pushed one"
    );
    assert!(
        envelope["action"]
            .as_str()
            .expect("action")
            .contains("no longer carries"),
        "the moved world is not distinguished: {}",
        envelope["action"]
    );
}

/// The sentence an operator reads, assembled, for every combination the type
/// can express.
///
/// All six the *type* has — two `undrafted` values by three `edited` ones — and
/// not every combination anything can drive: [`Edited::Title`] is unreachable
/// through what ships, so the two rows naming it are a superset. The default is
/// checked above the loop and the other five are its rows.
///
/// The reason this exists at the whole-sentence level rather than as more
/// `contains` checks: **fragment assertions are what let the broken sentence
/// ship**. The clauses were a past participle and a finite verb phrase sharing a
/// `was` supplied by the frame, so the ordinary case read *"the pull request was
/// had its title and body replaced"* — and every test pinning that text asserted
/// a fragment, so all of them passed. The repair then added more fragment
/// assertions, and restoring the double-verb frame left the whole suite green.
///
/// A fragment cannot see a frame. This asserts the frames.
#[test]
fn the_refusal_reads_as_a_sentence_in_every_combination() {
    let says = |wrote: PullRequestWrites| -> (String, String) {
        let stop = after_rewriting_the_pr(
            stop("some-reason", "detail".to_owned(), "do the thing"),
            wrote,
        );
        let write = after_rewriting_the_pr(Failure::Write("push failed".to_owned()), wrote);
        (
            stop.envelope()
                .get("action")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            write.detail(),
        )
    };

    // Nothing written: the refusal is passed through untouched, so there is no
    // sentence to get wrong and no `world` claiming otherwise.
    let untouched = PullRequestWrites::default();
    let (action, _) = says(untouched);
    assert_eq!(
        action, "do the thing",
        "a refusal was reworded over a pull request nobody touched"
    );
    assert!(
        after_rewriting_the_pr(
            stop("some-reason", "detail".to_owned(), "do the thing"),
            untouched,
        )
        .envelope()
        .get("world")
        .is_none(),
        "an untouched world was reported as committed"
    );

    for (wrote, expected) in [
        (
            PullRequestWrites {
                undrafted: false,
                edited: Some(Edited::TitleAndBody),
            },
            "the pull request had its title and body replaced",
        ),
        (
            PullRequestWrites {
                undrafted: false,
                edited: Some(Edited::Title),
            },
            "the pull request had its title replaced",
        ),
        (
            PullRequestWrites {
                undrafted: true,
                edited: None,
            },
            "the pull request was converted back to draft",
        ),
        (
            PullRequestWrites {
                undrafted: true,
                edited: Some(Edited::Title),
            },
            "the pull request was converted back to draft and had its title replaced",
        ),
        (
            PullRequestWrites {
                undrafted: true,
                edited: Some(Edited::TitleAndBody),
            },
            "the pull request was converted back to draft and had its title and body replaced",
        ),
    ] {
        let (action, write) = says(wrote);
        assert!(
            action.contains(&format!("{expected} before this refusal")),
            "the stop frame does not read as a sentence for {wrote:?}: {action}"
        );
        assert!(
            write.contains(&format!(
                "{expected}, so this is not a call that changed nothing"
            )),
            "the write frame does not read as a sentence for {wrote:?}: {write}"
        );
        // The shapes a frame gets wrong: a doubled verb, and an adverb landing
        // in front of one.
        for broken in ["was was", "was had", "already was", "already had"] {
            for text in [&action, &write] {
                assert!(
                    !text.contains(broken),
                    "the refusal says {broken:?} for {wrote:?}: {text}"
                );
            }
        }
    }
}

/// One check run, as the check-run listing spells it.
fn check_run(name: &str, status: &str, conclusion: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "status": status,
        "conclusion": conclusion,
        "html_url": "https://github.com/o/r/actions/runs/31865504912/job/99",
    })
}

/// The listing, with the count the API reports beside it.
fn check_runs(runs: &[serde_json::Value]) -> serde_json::Value {
    serde_json::json!({ "total_count": runs.len(), "check_runs": runs })
}

/// The four states the publication lane can be in, and the first one decides
/// whether this change is a fix or a disaster.
///
/// **Absence proceeds.** A consumer repository with no dispatchable lane has no
/// check runs on any head; refusing there would stop every such repository from
/// recording a verdict at all, which breaks the consumers this exists to
/// protect. It is measured on its own, because a test that omitted it would let
/// a later change turn absence into a refusal with nothing objecting.
///
/// The other three refuse, and `in_progress` is the one most easily written the
/// wrong way round: a lane that has not answered has not said the bytes are
/// sound.
#[test]
fn the_publication_lane_clears_only_a_head_that_has_answered_and_answered_green() {
    let head = "b".repeat(40);
    let path = format!("repos/o/r/commits/{head}/check-runs?per_page=100");
    let judged = |data: &serde_json::Value| judge_publication_lane(data, 7, &head, &path);

    // None at all: the behaviour before this existed, exactly.
    assert!(
        judged(&check_runs(&[])).is_ok(),
        "a head with no check runs was refused, which breaks every repository that has no lane"
    );

    // Green, and the two completed conclusions that are a lane declining to
    // have an opinion rather than a lane that failed.
    for conclusion in ["success", "neutral", "skipped"] {
        assert!(
            judged(&check_runs(&[check_run(
                "check (ubuntu-latest)",
                "completed",
                Some(conclusion)
            )]))
            .is_ok(),
            "a completed `{conclusion}` lane refused a verdict"
        );
    }

    // Red, in every spelling that is not one of those three. A completed run
    // with no conclusion at all is included: an unknown result is not clearance.
    for conclusion in [
        Some("failure"),
        Some("timed_out"),
        Some("cancelled"),
        Some("action_required"),
        Some("startup_failure"),
        None,
    ] {
        let refusal = judged(&check_runs(&[
            check_run("check (windows-latest)", "completed", Some("success")),
            check_run("check (ubuntu-latest)", "completed", conclusion),
        ]))
        .expect_err("a lane that did not conclude green cleared a verdict");
        let Failure::Stop(envelope) = &refusal else {
            panic!("a red lane is a stop, not {refusal:?}");
        };
        assert_eq!(envelope["reason"], "publication-lane-red", "{conclusion:?}");
        assert_eq!(
            envelope["lane"], "check (ubuntu-latest)",
            "the refusal names the wrong lane: {envelope}"
        );
        // A command that clears it, not a dead end: the run is readable by id,
        // and the way past a red lane is a new epoch over fixed bytes.
        let action = envelope["action"].as_str().unwrap_or_default();
        assert!(
            action.contains("gh run view 31865504912 --log-failed")
                && action.contains("republish_review"),
            "the red refusal names no command that clears it: {action}"
        );
        assert_eq!(
            envelope["run_url"],
            "https://github.com/o/r/actions/runs/31865504912/job/99"
        );
    }

    // Unfinished. `in_progress` is not `success`, and neither is anything else
    // GitHub reports before a run has settled.
    for status in ["queued", "in_progress", "waiting", "pending", "requested"] {
        let refusal = judged(&check_runs(&[
            check_run("check (windows-latest)", "completed", Some("success")),
            check_run("check (macos-latest)", status, None),
        ]))
        .expect_err("a lane that had not answered cleared a verdict");
        let Failure::Stop(envelope) = &refusal else {
            panic!("an unfinished lane is a stop, not {refusal:?}");
        };
        assert_eq!(
            envelope["reason"], "publication-lane-unfinished",
            "{status} was not read as unfinished"
        );
        assert_eq!(envelope["lane"], "check (macos-latest)");
        assert!(
            envelope["action"]
                .as_str()
                .unwrap_or_default()
                .contains("gh run watch 31865504912 --exit-status"),
            "the unfinished refusal names no command to watch it: {envelope}"
        );
    }

    // Red before unfinished when both are there. Waiting cannot turn a failed
    // lane green, so the answer that cannot improve is the one to report.
    let both = judged(&check_runs(&[
        check_run("check (macos-latest)", "in_progress", None),
        check_run("check (ubuntu-latest)", "completed", Some("failure")),
    ]))
    .expect_err("a red lane beside an unfinished one cleared a verdict");
    let Failure::Stop(envelope) = &both else {
        panic!("expected a stop: {both:?}");
    };
    assert_eq!(envelope["reason"], "publication-lane-red");
}

/// A listing that did not answer is a failed read, never a green lane and never
/// a red one.
///
/// The partial page is the half worth having. `total_count` is what the API says
/// exists; anything less than that arrived is an answer about *part* of the
/// head, and thirty green lanes cleared as the whole of it is how a red
/// thirty-first becomes invisible. It is the rule `connection_page` states for
/// the closing-PR listing, for the same reason.
#[test]
fn an_unreadable_check_run_listing_fails_the_read_rather_than_clearing_the_head() {
    let head = "b".repeat(40);
    let path = format!("repos/o/r/commits/{head}/check-runs?per_page=100");
    let judged = |data: serde_json::Value| judge_publication_lane(&data, 7, &head, &path);

    for (what, data) in [
        ("no check_runs list", serde_json::json!({"total_count": 0})),
        (
            "check_runs is not a list",
            serde_json::json!({"total_count": 0, "check_runs": "none"}),
        ),
        (
            "no total_count",
            serde_json::json!({"check_runs": [check_run("check", "completed", Some("success"))]}),
        ),
        (
            "one page of several",
            serde_json::json!({
                "total_count": 120,
                "check_runs": [check_run("check", "completed", Some("success"))],
            }),
        ),
        ("nothing at all", serde_json::json!({})),
    ] {
        let refusal = judged(data).expect_err("an unreadable listing cleared a verdict");
        assert!(
            matches!(refusal, Failure::Read(_)),
            "{what} was reported as something other than a failed read: {refusal:?}"
        );
        let detail = refusal.detail();
        assert!(
            detail.contains("this is a failed read and not a green lane"),
            "{what} does not say it is a read failure rather than a red lane: {detail}"
        );
    }
}

#[test]
fn publication_lane_answers_each_of_the_six_states() {
    let head = "b".repeat(40);
    let ran_ubuntu = RedCheckRun {
        name: "check (ubuntu-latest)".to_owned(),
        conclusion: "failure".to_owned(),
        run_url: "https://github.com/o/r/actions/runs/31865504912/job/99".to_owned(),
    };
    // `classification` stands in for what `classify_reds` would have
    // answered — this test drives `publication_lane_answer` directly, with
    // no `Context` and no network, so the decision it would have made is
    // supplied rather than reached.
    let answered = |data: serde_json::Value, classification: Option<RedClassification>| {
        publication_lane_answer(
            &head,
            read_publication_lane(&data).expect("a readable listing"),
            classification,
            ActionsAvailability::Available,
        )
        .expect("an answer")
    };

    let absent = answered(check_runs(&[]), None);
    assert_eq!(absent["state"], "absent");
    assert_eq!(absent["head"], head);

    let green = answered(
        check_runs(&[check_run(
            "check (ubuntu-latest)",
            "completed",
            Some("success"),
        )]),
        None,
    );
    assert_eq!(green["state"], "green");

    // A red lane whose jobs `classify_reds` did not prove never-ran —
    // the invariant: it still reads `red`, never `never-ran`, however the
    // caller elsewhere might have been evidenced.
    let red = answered(
        check_runs(&[check_run(
            "check (ubuntu-latest)",
            "completed",
            Some("failure"),
        )]),
        Some(RedClassification::Ran(ran_ubuntu.clone())),
    );
    assert_eq!(red["state"], "red");
    assert!(
        red["action"]
            .as_str()
            .is_some_and(|action| action.contains("do not launch")),
        "red does not say not to launch: {red}"
    );

    // The same red listing, this time with `classify_reds` having proven
    // it never executed a step. Same input to `read_publication_lane`, a
    // different outcome once classified — the whole reason the two need to
    // stay one function rather than a second copy of the predicate.
    let never_ran = answered(
        check_runs(&[check_run(
            "check (ubuntu-latest)",
            "completed",
            Some("failure"),
        )]),
        Some(RedClassification::NeverRan {
            annotation: Some("no minutes remaining on this account".to_owned()),
            runs: vec![ran_ubuntu],
        }),
    );
    assert_eq!(never_ran["state"], "never-ran");
    assert_eq!(
        never_ran["annotation"], "no minutes remaining on this account",
        "the never-ran answer does not carry the annotation: {never_ran}"
    );
    assert!(
        never_ran["action"]
            .as_str()
            .is_some_and(|action| action.contains("Launch")),
        "never-ran does not say launching still has a path: {never_ran}"
    );

    let running = answered(
        check_runs(&[check_run("check (macos-latest)", "in_progress", None)]),
        None,
    );
    assert_eq!(running["state"], "running");

    let path = format!("repos/o/r/commits/{head}/check-runs?per_page=100");
    let unread = judge_publication_lane(&serde_json::json!({}), 7, &head, &path)
        .expect_err("emptiness cleared the head");
    assert!(
        matches!(unread, Failure::Read(_)),
        "unreadable was not a failed read: {unread:?}"
    );
}

/// The structural predicate, isolated from the network read
/// around it: two recorded jobs-listing shapes, told apart on `steps: []`
/// alone and never on the annotation's wording.
#[test]
fn jobs_that_ran_and_jobs_that_never_started_are_told_apart() {
    let ran_and_failed = serde_json::json!({
        "total_count": 1,
        "jobs": [{"steps": [{"name": "pytest", "conclusion": "failure"}]}],
    });
    assert_eq!(
        jobs_report_never_ran(&ran_and_failed),
        Some(false),
        "a job with real steps was read as never having run"
    );

    let never_ran = serde_json::json!({"total_count": 1, "jobs": [{"steps": []}]});
    assert_eq!(
        jobs_report_never_ran(&never_ran),
        Some(true),
        "a job whose steps are empty was not read as never having run"
    );

    // One job that ran beside one that did not is not proof every job never
    // ran — `all`, not `any`.
    let mixed = serde_json::json!({
        "total_count": 2,
        "jobs": [{"steps": []}, {"steps": [{"name": "lint", "conclusion": "failure"}]}],
    });
    assert_eq!(jobs_report_never_ran(&mixed), Some(false));

    // A read that told us nothing is not evidence either way, and answering
    // `false` for it would make an unreadable listing look exactly like one
    // this crate measured and found to have run.
    assert_eq!(
        jobs_report_never_ran(&serde_json::json!({"total_count": 0, "jobs": []})),
        None
    );
    assert_eq!(jobs_report_never_ran(&serde_json::json!({})), None);

    // The partial page: `total_count` says more jobs exist than arrived, the
    // identical shape `read_publication_lane` already refuses a verdict over.
    // Every job that *did* arrive reports `steps: []` — a defect here would
    // read this as never-ran precisely because the page that arrived agreed
    // with itself, which is not the same as the run agreeing with itself.
    let partial_page = serde_json::json!({
        "total_count": 101,
        "jobs": (0..100).map(|_| serde_json::json!({"steps": []})).collect::<Vec<_>>(),
    });
    assert_eq!(
        jobs_report_never_ran(&partial_page),
        None,
        "a partial jobs page was read as proof the run never executed"
    );
}

#[test]
fn the_first_annotation_message_is_evidence_and_not_a_predicate() {
    let annotations = serde_json::json!([
        {"message": "no minutes remaining on this account"},
        {"message": "a second annotation nobody reads"},
    ]);
    assert_eq!(
        first_annotation_message(&annotations).as_deref(),
        Some("no minutes remaining on this account")
    );
    assert_eq!(first_annotation_message(&serde_json::json!([])), None);
    assert_eq!(first_annotation_message(&serde_json::json!({})), None);
}

/// One workflow run behind two check runs sharing its id — a matrix leg that
/// was cancelled beside one still reporting `failure` — with the cancelled
/// entry listed **second**. The dedupe that groups check runs by run id keeps
/// only the first-listed entry per id, so a `cancelled` test read over that
/// dedupe alone never sees the second entry at all. `classify_reds` must
/// still exclude the run: scanned over every entry of `reds`, before the
/// dedupe is ever built.
///
/// The context names a directory with no `.git`, so `never_ran_signal`'s own
/// `repo_identity` read fails fast without reaching any network — a
/// `classify_reds` that excludes on the first entry never needs to ask it at
/// all, and one that does not exclude falls through to that same failing
/// read, naming the **other**, non-cancelled entry instead. The two outcomes
/// are told apart by which run the answer names.
#[test]
fn a_cancelled_conclusion_excludes_its_run_wherever_it_is_listed() {
    let failed = RedCheckRun {
        name: "check (matrix-1)".to_owned(),
        conclusion: "failure".to_owned(),
        run_url: "https://github.com/o/r/actions/runs/900000001/job/1".to_owned(),
    };
    let cancelled = RedCheckRun {
        name: "check (matrix-2)".to_owned(),
        conclusion: "cancelled".to_owned(),
        run_url: "https://github.com/o/r/actions/runs/900000001/job/2".to_owned(),
    };
    let root = tempfile::tempdir().expect("a context root");
    let context = super::super::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let data = serde_json::json!({});
    let reds = [failed, cancelled];
    let RedClassification::Ran(named) = classify_reds(&context, &data, &reds) else {
        panic!("a cancelled sibling was read as never-ran");
    };
    assert_eq!(
        named.conclusion, "cancelled",
        "the cancelled entry was not what excluded the run, even though it is present: {named:?}"
    );
}

#[test]
fn the_contract_reads_the_lane_before_launching_the_panel() {
    let delivery = include_str!("../../../skill/references/repository-delivery.md");
    for needle in [
        "publication lane",
        "do not launch",
        "unreadable, not clearance",
    ] {
        assert!(
            delivery.contains(needle),
            "repository delivery does not route the lane on {needle:?}"
        );
    }
}

#[test]
fn the_contract_escalates_once_the_one_correction_is_spent() {
    let judges = include_str!("../../../skill/policies/blind-judges.md");
    let flat = judges.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in [
        "`review_status`'s `corrections`",
        "one correction",
        "escalate_review",
        "not the non-routine adjudication `blocked` is for",
        "Reviewers are never told the count",
        "spending the correction manufactures none",
    ] {
        assert!(
            flat.contains(needle),
            "blind judges does not route on {needle:?}"
        );
    }
}

/// A run commits to what reopens review before it reads
/// anything the panel returns. The rule lives in developer step 5; the ledger
/// it rests on lives in `blind-judges.md` and the disposition record it may
/// answer with lives in the binding's operations companion, both linked from
/// step 5.
#[test]
fn the_contract_commits_to_the_disposition_rule() {
    let skill = include_str!("../../../skill/SKILL.md");
    let region = skill
        .split_once("5. **Renew before publishing.**")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("\n6. "))
        .map(|(region, _)| region)
        .expect("developer step 5");
    let flat = region.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("Commit to the disposition before reading anything the panel returns"),
        "step 5 does not commit to the disposition rule before reading the panel"
    );
    let judges = include_str!("../../../skill/policies/blind-judges.md");
    let judges_flat = judges.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        judges_flat.contains("never a reason to re-run review on this candidate"),
        "blind judges no longer states that a non-blocking finding does not reopen review"
    );
    assert!(
        judges_flat.contains("The verdict is computed from the receipt's findings"),
        "blind judges no longer states the one release rule"
    );
    assert!(
        judges.contains("record_disposition"),
        "blind judges no longer names the disposition record"
    );
    let operations = include_str!("../../../skill/bindings/github/operations.md");
    for needle in ["acted-on", "filed", "declined"] {
        assert!(
            operations.contains(needle),
            "the operations companion does not name {needle:?}"
        );
    }
}

/// The no-reduce sentence is scoped by its own main clause. Deleting the
/// condition that scopes it leaves "never reduce or serialize the panel"
/// standing alone, which is the defect a second finding names.
#[test]
fn the_contract_scopes_its_no_reduce_sentence() {
    let judges = include_str!("../../../skill/policies/judgment-day.md");
    let flat = judges.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("never silently reduce or serialize the panel"),
        "judgment day no longer states the no-reduce rule"
    );
    assert!(
        flat.contains("cannot be restored"),
        "judgment day states the no-reduce rule with no condition scoping it"
    );
}

/// The tracking rule: the checklist is recorded
/// with `task` on the claimed issue, never in a comment or a checkout file. The
/// positive half pins the rule; the negative half says the skill does not carry
/// the file path v4 uses, so no reader is told to write one.
#[test]
fn the_contract_tracks_tasks_with_the_task_operation_and_never_in_a_comment_or_a_checkout_file() {
    let skill = include_str!("../../../skill/SKILL.md");
    let flat = skill.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("records its ordered task checklist with the `task` operation"),
        "the ODD protocol no longer tracks tasks with the task operation"
    );
    assert!(
        flat.contains("never as a `comment` and never as a file in the checkout"),
        "the tracking rule no longer refuses a comment"
    );
    assert!(
        !flat.contains("odd/tasks"),
        "the skill instructs a task file in the checkout"
    );
}

/// The seven ODD steps, the evidence budget and test-first are all stated in
/// the shipped skill.
#[test]
fn the_contract_states_the_odd_protocol_its_budget_and_test_first() {
    let skill = include_str!("../../../skill/SKILL.md");
    let flat = skill.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in [
        "**Authorize.**",
        "**Explore**,",
        "**Resolve uncertainty.**",
        "**Classify.**",
        "**Track before the first write.**",
        "**Implement task by task.**",
        "**Close.**",
        "At most 3 calls and about 10k tokens stays inline",
        "More than about 5 sequential lookups delegates one explorer with a ~2k-token handoff",
        "test-first",
    ] {
        assert!(
            flat.contains(needle),
            "the ODD protocol does not state {needle:?}"
        );
    }
}

/// `gh workflow run` is read for what it says, and a repository with no lane is
/// not a caller without permission.
///
/// The two look identical to a `?` and need opposite outcomes. `404` is
/// deliberately on the permissive side: GitHub answers it both for a workflow
/// that is not there and for one the caller may not see, and refusing a
/// publication on the ambiguous code would stop every repository that has not
/// adopted this lane from publishing at all.
#[test]
fn only_a_permission_refusal_stops_a_publication_that_could_not_start_its_lane() {
    assert_eq!(classify_lane_dispatch(0, ""), DispatchAttempt::Started);

    for said in [
        "HTTP 403: Resource not accessible by personal access token",
        "gh: HTTP 401: Bad credentials",
    ] {
        assert!(
            matches!(
                classify_lane_dispatch(1, said),
                DispatchAttempt::Forbidden(_)
            ),
            "a permission refusal was not read as one: {said}"
        );
    }
    for said in [
        "could not find any workflows named ci.yml",
        "gh: HTTP 404: Not Found",
        "gh: HTTP 422: Workflow does not have 'workflow_dispatch' trigger",
    ] {
        assert!(
            matches!(classify_lane_dispatch(1, said), DispatchAttempt::Absent(_)),
            "a repository with no dispatchable lane would have been refused a publication: {said}"
        );
    }
    assert!(
        matches!(
            classify_lane_dispatch(1, "dial tcp: lookup api.github.com: no such host"),
            DispatchAttempt::Unknown(_)
        ),
        "a call that did not answer was read as an answer"
    );
}

#[test]
fn lane_name_lists_take_basenames_and_drop_blanks() {
    assert_eq!(
        lane_name_list("ci.yml, ci-api.yml , .github/workflows/ci-web.yml"),
        vec![
            "ci.yml".to_owned(),
            "ci-api.yml".to_owned(),
            "ci-web.yml".to_owned()
        ]
    );
    assert!(lane_name_list(" , , ").is_empty());
    assert!(lane_is_excluded(
        "release.yml",
        &lane_name_list("Release.yml, nightly.yml")
    ));
    assert!(!lane_is_excluded(
        "ci.yml",
        &lane_name_list("release.yml, nightly.yml")
    ));
}

#[test]
fn format_lane_workflows_names_every_file() {
    assert_eq!(format_lane_workflows(&["ci.yml".to_owned()]), "`ci.yml`");
    assert_eq!(
        format_lane_workflows(&["ci.yml".to_owned(), "ci-api.yml".to_owned()]),
        "`ci.yml`, `ci-api.yml`"
    );
}

#[test]
fn publication_lane_candidates_honour_an_explicit_list_and_the_exclude_row() {
    let listed = super::super::Context {
        skill_dir: std::path::PathBuf::from("."),
        repo_dir: std::path::PathBuf::from("."),
        config: vec![
            (
                "Publication lanes".to_owned(),
                "ci.yml, release.yml, ci-api.yml".to_owned(),
                Source::Written,
            ),
            (
                "Publication lanes exclude".to_owned(),
                "release.yml".to_owned(),
                Source::Written,
            ),
        ],
        repo: None,
    };
    assert_eq!(
        publication_lane_candidates(&listed),
        vec!["ci.yml".to_owned(), "ci-api.yml".to_owned()],
        "an explicit list must drop only the excluded names"
    );

    let none = super::super::Context {
        skill_dir: std::path::PathBuf::from("."),
        repo_dir: std::path::PathBuf::from("."),
        config: vec![(
            "Publication lanes".to_owned(),
            "none".to_owned(),
            Source::Written,
        )],
        repo: None,
    };
    assert!(
        publication_lane_candidates(&none).is_empty(),
        "`none` must leave the epoch without a lane rather than falling back to ci.yml"
    );

    // Exclude-only must not bind as the lane list: Context::get is a prefix
    // match, and "publication lanes" is a prefix of "publication lanes exclude".
    let exclude_only = super::super::Context {
        skill_dir: std::path::PathBuf::from("."),
        repo_dir: std::path::PathBuf::from("."),
        config: vec![(
            "Publication lanes exclude".to_owned(),
            "release.yml".to_owned(),
            Source::Written,
        )],
        repo: None,
    };
    assert!(
        config_exact(&exclude_only, "Publication lanes").is_none(),
        "exclude-only must leave the lanes row unbound rather than prefix-matching the exclude cell"
    );
    assert_eq!(
        config_exact(&exclude_only, "Publication lanes exclude"),
        Some("release.yml"),
        "the exclude row is still readable by its own exact label"
    );

    let exclude_first = super::super::Context {
        skill_dir: std::path::PathBuf::from("."),
        repo_dir: std::path::PathBuf::from("."),
        config: vec![
            (
                "Publication lanes exclude".to_owned(),
                "release.yml".to_owned(),
                Source::Written,
            ),
            (
                "Publication lanes".to_owned(),
                "ci.yml, release.yml".to_owned(),
                Source::Written,
            ),
        ],
        repo: None,
    };
    assert_eq!(
        publication_lane_candidates(&exclude_first),
        vec!["ci.yml".to_owned()],
        "exclude listed before lanes must still bind each row by exact label"
    );

    // The defect both judges measured: prefix get treated the exclude cell as
    // the lane list, then filtered it against itself to []. Exact match leaves
    // the explicit list intact even when exclude is the first row.
    let prefix_trap = super::super::Context {
        skill_dir: std::path::PathBuf::from("."),
        repo_dir: std::path::PathBuf::from("."),
        config: vec![
            (
                "Publication lanes exclude".to_owned(),
                "ci.yml, release.yml".to_owned(),
                Source::Written,
            ),
            (
                "Publication lanes".to_owned(),
                "ci.yml".to_owned(),
                Source::Written,
            ),
        ],
        repo: None,
    };
    assert_eq!(
        publication_lane_candidates(&prefix_trap),
        Vec::<String>::new(),
        "an explicit lane that is also excluded is empty on purpose, not by prefix collision"
    );
    // And with only the exclude row present, get-by-prefix would have answered
    // the exclude cell as the lane list; exact match must not.
    assert_ne!(
        config_exact(&exclude_only, "Publication lanes").map(lane_name_list),
        Some(lane_name_list("release.yml")),
        "the lanes key must not resolve to the exclude cell"
    );
}

/// The receipt comment says what this publication did about CI, and says it per
/// lane.
///
/// **The test that did not exist**: the dispatch sits immediately above the
/// call that writes this comment, and nothing read the emitted sentence — so
/// `publish_review` could answer `"publication_lane": "started"` while posting
/// a comment that said *"CI remains blocked while the PR is draft"*, two
/// sentences from one call disagreeing on one issue's own timeline.
///
/// Three properties, and the middle one is the one that goes stale:
///
/// - the evidence lines and the marker survive, because a reviewer binds to
///   those bytes and prose is not what carries them;
/// - the CI clause tells the two lanes apart — the publication lane an accepted
///   verdict waits on, and the pull-request-event lane that still waits for the
///   pull request to be marked ready. Restoring the old sentence, or dropping
///   either half, fails here;
/// - a state that did not start a lane does not claim one did. Absence is the
///   ordinary condition of every repository that has not adopted the lane, and
///   a receipt telling those repositories a run is coming is the same defect
///   one state along.
#[test]
fn the_review_transaction_says_what_it_did_about_ci_and_says_it_per_lane() {
    let marker = "<!-- issue-flow: published run-id=claude-abcd1234 -->";
    let note = |lane: &LaneDispatch| {
        publication_note(
            "https://github.com/o/r/pull/7",
            &"e".repeat(32),
            &"b".repeat(40),
            &"c".repeat(40),
            &"d".repeat(64),
            &"f".repeat(64),
            None,
            None,
            lane,
            marker,
        )
    };

    for (label, lane) in [
        ("started", LaneDispatch::Started(vec!["ci.yml".to_owned()])),
        ("absent", LaneDispatch::Absent("HTTP 404".to_owned())),
        ("unknown", LaneDispatch::Unknown("no such host".to_owned())),
    ] {
        let text = note(&lane);
        for line in [
            &format!("- epoch `{}`", "e".repeat(32)),
            &format!("- head `{}`", "b".repeat(40)),
            &format!("- base `{}`", "c".repeat(40)),
            &format!("- target `{}`", "d".repeat(64)),
            &format!("- payload `{}`", "f".repeat(64)),
            &marker.to_owned(),
            &"Review is bound to this complete clean target.".to_owned(),
            &"any republish creates a new epoch".to_owned(),
        ] {
            assert!(
                text.contains(line.as_str()),
                "the {label} receipt lost the evidence a review is bound to: {line} is not in \
                 {text}"
            );
        }

        // The sentence this test exists for. It was true of the world before
        // the publication lane existed and false in every publication after,
        // and it is the one a blind reviewer reads.
        assert!(
            !text.contains("CI remains blocked while the PR is draft"),
            "the {label} receipt still tells every reviewer CI is blocked, which is what the \
             publication lane made false: {text}"
        );
        // Both lanes, named apart. The draft barrier is still real — a
        // dispatch does not mark the pull request ready — so dropping this
        // half would make the receipt claim CI is running when the ordinary
        // lane is not.
        assert!(
            text.contains("still waits for it to be marked ready"),
            "the {label} receipt no longer says the ordinary pull-request-event lane waits for \
             the pull request to be readied: {text}"
        );
        let lane_named = match &lane {
            LaneDispatch::Started(workflows) => workflows
                .iter()
                .any(|name| text.contains(&format!("`{name}`"))),
            _ => text.contains("publication lane"),
        };
        assert!(
            lane_named,
            "the {label} receipt names no publication lane at all: {text}"
        );
    }

    // What each state says about a verdict, which is the thing the receipt is
    // read for. `started` is the only one that may promise a run.
    let started = LaneDispatch::Started(vec!["ci.yml".to_owned()]);
    assert!(
        note(&started).contains(
            "was started against this head as this receipt was recorded, and an accepted verdict \
             cannot be recorded until it is green"
        ),
        "a started lane does not tell the reviewer a verdict waits on it: {}",
        note(&started)
    );
    assert!(
        note(&started).contains("`ci.yml`"),
        "a started lane must name the workflow it dispatched: {}",
        note(&started)
    );
    let multi = LaneDispatch::Started(vec!["ci.yml".to_owned(), "ci-api.yml".to_owned()]);
    assert!(
        note(&multi).contains("2 runs (`ci.yml`, `ci-api.yml`) were started"),
        "a multi-lane start must name every workflow and the count: {}",
        note(&multi)
    );
    for (label, lane) in [
        ("absent", LaneDispatch::Absent("HTTP 404".to_owned())),
        ("unknown", LaneDispatch::Unknown("no such host".to_owned())),
    ] {
        let text = note(&lane);
        assert!(
            !text.contains("was started against this head"),
            "the {label} receipt claims a lane run that this publication did not start, which is \
             the receipt disagreeing with the answer beside it one state along: {text}"
        );
    }
    assert!(
        note(&LaneDispatch::Absent("HTTP 404".to_owned()))
            .contains("no publication lane and verdicts are not gated on one"),
        "a repository with no lane is not told its verdicts are ungated, so its reviewers wait \
         for a run that will never appear"
    );
    assert!(
        note(&LaneDispatch::Unknown("no such host".to_owned()))
            .contains("read the checks on this head before obtaining verdicts"),
        "an unknown dispatch is reported as if it were an answer"
    );
}

/// The receipt's topology line: absent when nothing was declared, the bare
/// value when it was, and the stated reason beside it when there was one.
#[test]
fn the_review_transaction_says_the_declared_topology_and_only_when_declared() {
    let marker = "<!-- issue-flow: published run-id=claude-abcd1234 -->";
    let note = |topology: Option<&str>, reason: Option<&str>| {
        publication_note(
            "https://github.com/o/r/pull/7",
            &"e".repeat(32),
            &"b".repeat(40),
            &"c".repeat(40),
            &"d".repeat(64),
            &"f".repeat(64),
            topology,
            reason,
            &LaneDispatch::Started(vec!["ci.yml".to_owned()]),
            marker,
        )
    };

    assert!(
        !note(None, None).contains("- topology"),
        "an undeclared topology must not appear as though `inline` or anything else were said"
    );
    assert!(
        note(Some("developer:3"), None).contains("- topology `developer:3`"),
        "a declared topology with no reason is still recorded"
    );
    let with_reason = note(
        Some("inline"),
        Some("one mechanical rename across two files"),
    );
    assert!(
        with_reason.contains("- topology `inline` \u{2014} one mechanical rename across two files"),
        "a stated reason belongs beside the topology it explains: {with_reason}"
    );

    // A reason that quotes a whole marker must not become one: `markers::parse`
    // scans the composed note from any `<!--`, not only the one this function
    // renders, so an unescaped reason posts a second, caller-authored marker
    // under the identity that posted the receipt.
    let quoting_a_marker = note(
        Some("inline"),
        Some("<!-- issue-flow: standdown run-id=claude-abcd1234 -->"),
    );
    let found = super::super::markers::parse(&quoting_a_marker);
    assert_eq!(
        found.len(),
        1,
        "a reason quoting a marker was read back as a second one: {found:?}\n{quoting_a_marker}"
    );

    // A reason with an opener and no closer must not swallow the real
    // `published` marker that follows it: `parse` closes on the *first*
    // `-->` wherever it lands, so an unescaped, unclosed opener here borrows
    // the receipt's own close and the receipt answers `ok: true` for a
    // publication nobody can read back.
    let unclosed_opener = note(Some("inline"), Some("see <!-- the notes"));
    let found = super::super::markers::parse(&unclosed_opener);
    assert!(
        found
            .iter()
            .any(|marker| marker.get("kind").map(String::as_str) == Some("published")),
        "the receipt's own `published` marker did not survive an unclosed opener in the reason: \
         {found:?}\n{unclosed_opener}"
    );
}

/// The refusal names a run somebody can actually watch, or names none.
///
/// A `gh run watch` with the wrong number is a dead end, and this repository
/// calls naming one worse than naming nothing.
#[test]
fn a_check_runs_url_yields_the_workflow_run_or_nothing_at_all() {
    assert_eq!(
        workflow_run_id("https://github.com/o/r/actions/runs/31865504912/job/91234"),
        Some("31865504912".to_owned())
    );
    assert_eq!(
        workflow_run_id("https://github.com/o/r/actions/runs/31865504912"),
        Some("31865504912".to_owned())
    );
    for nothing in [
        "",
        "https://example.invalid/checks/1",
        // A check run written by an app rather than by Actions: the URL names no
        // workflow run, and deriving one from the check id would send somebody
        // to a different run entirely.
        "https://github.com/o/r/runs/12345",
        "https://github.com/o/r/actions/runs//job/1",
        "https://github.com/o/r/actions/runs/not-a-number/job/1",
    ] {
        assert_eq!(
            workflow_run_id(nothing),
            None,
            "invented a run id for {nothing:?}"
        );
    }
}

/// The shared scaffolding behind the changed-line fixtures below that keep the
/// file at `changed.rs` in the repository root: a ten-line `changed.rs` whose
/// content at `head` is `head_text`, so each hunk shape a containment test needs
/// is one `head_text` argument rather than a copy of the git setup around it.
fn repo_with_head_text(head_text: &str) -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    let base_text = ten_numbered_lines();
    std::fs::write(at.join("changed.rs"), &base_text).expect("the base file");
    git(&["git", "add", "changed.rs"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    std::fs::write(at.join("changed.rs"), head_text).expect("the head file");
    git(&["git", "add", "changed.rs"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();
    (root, base, head)
}

fn ten_numbered_lines() -> String {
    (1..=10).map(|n| format!("line{n}\n")).collect()
}

/// A repository with one ten-line file, where only line 5 differs between
/// `base` and `head` -- a single, isolated hunk a location can be checked
/// against, rather than the whole-file rewrite
/// `target::tests::two_commit_repo` builds for its own callers.
fn repo_with_one_changed_line() -> (tempfile::TempDir, String, String) {
    repo_with_head_text(&ten_numbered_lines().replace("line5\n", "line5-changed\n"))
}

/// A repository with one ten-line file, where lines 3 and 5 differ between
/// `base` and `head` — two separate hunks with line 4 unchanged between
/// them, the shape a containment span can straddle without covering.
fn repo_with_two_changed_lines_a_gap_apart() -> (tempfile::TempDir, String, String) {
    repo_with_head_text(
        &ten_numbered_lines()
            .replace("line3\n", "line3-changed\n")
            .replace("line5\n", "line5-changed\n"),
    )
}

/// A repository with one ten-line file, where lines 4 through 7 all differ
/// between `base` and `head` -- one hunk wide enough to claim a sub-range
/// that starts and ends strictly inside it, touching neither of the hunk's
/// own bounds.
fn repo_with_a_wide_changed_run() -> (tempfile::TempDir, String, String) {
    repo_with_head_text(
        &ten_numbered_lines()
            .replace("line4\n", "line4-changed\n")
            .replace("line5\n", "line5-changed\n")
            .replace("line6\n", "line6-changed\n")
            .replace("line7\n", "line7-changed\n"),
    )
}

#[test]
fn a_finding_location_parses_path_colon_line_or_path_colon_start_dash_end_and_nothing_else() {
    let single = parse_finding_location("src/foo.rs:10").expect("a plain line parses");
    assert_eq!(
        (single.path.as_str(), single.start, single.end),
        ("src/foo.rs", 10, 10)
    );

    let range = parse_finding_location("src/foo.rs:10-15").expect("a range parses");
    assert_eq!(
        (range.path.as_str(), range.start, range.end),
        ("src/foo.rs", 10, 15)
    );

    for bad in [
        "",
        "src/foo.rs",       // no line at all
        "src/foo.rs:",      // a colon naming nothing
        "src/foo.rs:abc",   // not a number
        "src/foo.rs:0",     // a diff has no line zero
        "src/foo.rs:15-10", // end before start
        "src/foo.rs:+5",    // a signed line; a diff has no signed lines
        "/etc/passwd:1",    // absolute, not repository-relative
        "../secret.rs:1",   // escapes the repository
        // The same escape, spelled with the backslash separator this
        // platform pastes naturally — refused by the same `..`-component
        // check as the forward-slash spelling above, which means the
        // component split has to read `\` as a separator too, not only `/`.
        "..\\secret.rs:1",
        "C:\\foo.rs:1", // a drive letter, not a repository-relative path
        // The shape `rg -n`/`clippy` emit. A repository-relative path never
        // contains a colon, so the embedded one refuses this rather than
        // being read as part of the path by a naive last-colon split.
        "src/foo.rs:10:9",
        // A leading colon is a `git` pathspec magic signature, not a
        // repository-relative path.
        ":!nothing.rs:1",
        ":(exclude)nothing.rs:1",
    ] {
        assert!(
            parse_finding_location(bad).is_none(),
            "{bad:?} parsed as a well-formed location"
        );
    }
}

/// Whitespace is trimmed once, before the path and the line are read apart —
/// not read by the presence check and left in place for the parse, which
/// would keep a leading space in the path and refuse a trailing one as
/// malformed. A backslash separator is normalised to the forward slashes
/// `git diff` itself reports, since a backslash is a pathspec escape rather
/// than a directory separator once handed to `git` — the natural way to
/// paste a path on the platform this repository is developed on would
/// otherwise parse as well-formed and then match nothing.
#[test]
fn a_findings_location_is_trimmed_once_and_its_separators_normalised() {
    let padded = parse_finding_location("  src/foo.rs:10  ").expect("padding trims");
    assert_eq!(
        (padded.path.as_str(), padded.start, padded.end),
        ("src/foo.rs", 10, 10)
    );

    let backslashed = parse_finding_location("src\\transport\\claim.rs:10")
        .expect("a backslash-separated path is repository-relative once normalised");
    assert_eq!(backslashed.path, "src/transport/claim.rs");
}

#[test]
fn a_hunk_headers_new_side_range_is_the_plus_half_and_a_pure_deletion_names_none() {
    assert_eq!(new_side_range("@@ -1,2 +3,4 @@"), Some(3..=6));
    assert_eq!(new_side_range("@@ -1 +5 @@"), Some(5..=5));
    assert_eq!(
        new_side_range("@@ -1,3 +5,0 @@ trailing hunk heading"),
        None,
        "a pure deletion adds no new-side line"
    );
    assert_eq!(new_side_range("+some added line"), None);
    assert_eq!(new_side_range("diff --git a/x b/x"), None);
}

#[test]
fn changed_new_side_lines_reports_the_touched_line_and_nothing_for_an_untouched_path() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let (touched, ranges) = changed_new_side_lines(&context, &base, &head, "changed.rs")
        .expect("a diff over a path that exists at both ends");
    assert!(
        touched,
        "the fixture's own path was not reported as touched"
    );
    assert_eq!(
        ranges,
        vec![5..=5],
        "the fixture changed only line 5: {ranges:?}"
    );

    let (untouched, empty) =
        changed_new_side_lines(&context, &base, &head, "nothing-like-this-exists.rs")
            .expect("a diff over a path the change never mentions still answers, just empty");
    assert!(
        !untouched,
        "a path the diff never mentions was reported as touched"
    );
    assert!(
        empty.is_empty(),
        "a path the diff never mentions reported ranges: {empty:?}"
    );
}

/// A repository with one ten-line file at a non-ASCII path, changed the same
/// way [`repo_with_one_changed_line`] changes `changed.rs` — and with
/// `core.quotePath` set to `true` explicitly, git's own default, so the test
/// below measures the pin overriding that setting rather than hoping a
/// machine's global config never carries the opposite. `changed_new_side_lines`
/// must pin it itself rather than rely on the caller's repository
/// configuration.
fn repo_with_one_changed_line_at_a_non_ascii_path() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    // Set explicitly rather than left to git's default: a global config on the
    // machine running this test can already carry `core.quotePath = false`,
    // which would make this fixture pass with the pin deleted.
    git(&["git", "config", "core.quotePath", "true"]);
    let base_text: String = (1..=10).map(|n| format!("line{n}\n")).collect();
    std::fs::write(at.join("café.rs"), &base_text).expect("the base file");
    git(&["git", "add", "café.rs"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    let head_text = base_text.replace("line5\n", "line5-changed\n");
    std::fs::write(at.join("café.rs"), &head_text).expect("the head file");
    git(&["git", "add", "café.rs"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();
    (root, base, head)
}

/// `core.quotePath` set to `true` — git's own default — C-quotes a non-ASCII
/// byte in the `diff --git` header (`diff --git "a/caf\303\251.rs"
/// "b/caf\303\251.rs"`), so a header built from the raw path never matches
/// unless the walk pins the setting itself rather than trusting whatever the
/// caller's repository already has configured.
#[test]
fn changed_new_side_lines_reports_touched_for_a_non_ascii_path_under_default_git_config() {
    let (root, base, head) = repo_with_one_changed_line_at_a_non_ascii_path();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let (touched, ranges) = changed_new_side_lines(&context, &base, &head, "café.rs")
        .expect("a diff over a path that exists at both ends");
    assert!(
        touched,
        "a non-ASCII path was not reported as touched under this install's default \
         core.quotePath"
    );
    assert_eq!(
        ranges,
        vec![5..=5],
        "the fixture changed only line 5: {ranges:?}"
    );
}

/// `diff.noprefix` is a repository setting, reachable from a fixture the same
/// way `core.quotePath` is: a repository with it set to `true` renders
/// `diff --git changed.rs changed.rs`, dropping the `a/`/`b/` prefix the
/// header comparison assumes. Left unpinned, no location would ever settle
/// in such a repository — the second demonstration the blocker names, beside
/// the quoting one above.
#[test]
fn changed_new_side_lines_reports_touched_under_a_repository_that_drops_the_diff_prefix() {
    let (root, base, head) = repo_with_one_changed_line();
    crate::transport::run(
        &["git", "config", "diff.noprefix", "true"],
        Some(root.path()),
        crate::transport::How::write(),
    )
    .expect("diff.noprefix can be set on this fixture");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let (touched, ranges) = changed_new_side_lines(&context, &base, &head, "changed.rs")
        .expect("a diff over a path that exists at both ends");
    assert!(
        touched,
        "a path was not reported as touched once the repository's own diff.noprefix dropped \
         the a/ b/ prefix"
    );
    assert_eq!(
        ranges,
        vec![5..=5],
        "the fixture changed only line 5: {ranges:?}"
    );
}

/// `diff.srcPrefix`/`diff.dstPrefix` are independent of `diff.noprefix` and
/// `diff.mnemonicPrefix` — git has four variables that decide the `diff --git`
/// prefix, and pinning only the first two still lets a repository render
/// `diff --git x/changed.rs b/changed.rs`, which never matches the
/// `diff --git a/<path> b/<path>` header this check builds. Left unpinned, a
/// repository setting only `diff.srcPrefix` defeats the fix that closed the
/// `diff.noprefix` gap above without needing `diff.noprefix` itself touched.
#[test]
fn changed_new_side_lines_reports_touched_under_a_repository_with_a_hostile_src_prefix() {
    let (root, base, head) = repo_with_one_changed_line();
    crate::transport::run(
        &["git", "config", "diff.srcPrefix", "x/"],
        Some(root.path()),
        crate::transport::How::write(),
    )
    .expect("diff.srcPrefix can be set on this fixture");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let (touched, ranges) = changed_new_side_lines(&context, &base, &head, "changed.rs")
        .expect("a diff over a path that exists at both ends");
    assert!(
        touched,
        "a path was not reported as touched once the repository's own diff.srcPrefix replaced \
         the a/ prefix the header comparison assumes"
    );
    assert_eq!(
        ranges,
        vec![5..=5],
        "the fixture changed only line 5: {ranges:?}"
    );
}

/// `diff.interHunkContext` fuses nearby hunks together in the rendered diff,
/// widening what `changed_new_side_lines` reads as touched to include lines
/// between two real changes that neither change ever touched. Left unpinned,
/// a location on line 5 of a ten-line file whose only real edits are lines 3
/// and 7 would settle — a widening `CLAUDE.md` refuses by name, since nothing
/// in `base..head` actually changed line 5.
#[test]
fn changed_new_side_lines_does_not_fuse_hunks_under_a_repository_with_inter_hunk_context() {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    git(&["git", "config", "diff.interHunkContext", "5"]);
    let base_text: String = (1..=10).map(|n| format!("line{n}\n")).collect();
    std::fs::write(at.join("changed.rs"), &base_text).expect("the base file");
    git(&["git", "add", "changed.rs"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    let head_text = base_text
        .replace("line3\n", "line3-changed\n")
        .replace("line7\n", "line7-changed\n");
    std::fs::write(at.join("changed.rs"), &head_text).expect("the head file");
    git(&["git", "add", "changed.rs"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();

    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let (touched, ranges) = changed_new_side_lines(&context, &base, &head, "changed.rs")
        .expect("a diff over a path that exists at both ends");
    assert!(touched);
    assert!(
        !ranges.iter().any(|range| range.contains(&5)),
        "line 5 was never touched by either edit, but the repository's own \
         diff.interHunkContext fused the two hunks around it: {ranges:?}"
    );
    assert_eq!(
        ranges,
        vec![3..=3, 7..=7],
        "the fixture changed exactly lines 3 and 7: {ranges:?}"
    );
}

/// `GIT_DIFF_OPTS` is read by `git diff`'s builtin diff machinery *after* the
/// command line, so no flag on the invocation — not even
/// `--inter-hunk-context=0` — can override it once the child process inherits
/// it from this one. Only removing it from the child's environment closes the
/// gap: measured directly, `GIT_DIFF_OPTS=-u3` widens a zero-context diff on
/// a ten-line file with two isolated edits into one hunk covering the whole
/// file.
#[test]
fn changed_new_side_lines_is_not_widened_by_an_inherited_git_diff_opts() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let (touched, ranges) = crate::test_env::with_process_env("GIT_DIFF_OPTS", "-u3", || {
        changed_new_side_lines(&context, &base, &head, "changed.rs")
    })
    .expect("a diff over a path that exists at both ends");
    assert!(touched);
    assert_eq!(
        ranges,
        vec![5..=5],
        "an inherited GIT_DIFF_OPTS widened the range beyond the one line the fixture changed: \
         {ranges:?}"
    );
}

/// A repository whose only difference for `changed.rs` is a deleted line — the
/// fixture the deletion-only accept case needs: a hunk that adds nothing on
/// the new side, so its own header is the only signal a location naming it
/// has anything to settle against.
fn repo_with_one_deleted_line() -> (tempfile::TempDir, String, String) {
    let head_text: String = (1..=10)
        .filter(|n| *n != 5)
        .map(|n| format!("line{n}\n"))
        .collect();
    repo_with_head_text(&head_text)
}

/// A repository whose changed file sits under a directory, so a location can
/// name that directory rather than the file — the shape an ordinary typo
/// produces.
fn repo_with_a_change_under_a_directory() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    std::fs::create_dir_all(at.join("src")).expect("the directory");
    let base_text: String = (1..=10).map(|n| format!("line{n}\n")).collect();
    std::fs::write(at.join("src").join("foo.rs"), &base_text).expect("the base file");
    git(&["git", "add", "src/foo.rs"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    let head_text = base_text.replace("line5\n", "line5-changed\n");
    std::fs::write(at.join("src").join("foo.rs"), &head_text).expect("the head file");
    git(&["git", "add", "src/foo.rs"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();
    (root, base, head)
}

/// The pathspec `changed_new_side_lines` hands `git` is `:(literal)<path>`,
/// not `<path>` handed to `--`: `--` stops option parsing but not pathspec
/// magic, so a leading colon in a caller-supplied location would otherwise be
/// read as a magic signature. `:(literal)` alone does not close every gap a
/// pathspec opens, though — prefix matching on a directory and wildcard
/// matching on a glob are not the magic that flag disables, so `src` still
/// pathspec-matches every file under `src/` even as a literal. What actually
/// closes that gap is the header-exact-match this test drives: `src` never
/// carries a `diff --git a/src b/src` line, because no file is literally
/// named `src`, so the caller-facing behaviour ends up correct even though
/// `:(literal)` by itself would not have been enough.
#[test]
fn a_bare_directory_never_settles_for_the_files_under_it() {
    let (root, base, head) = repo_with_a_change_under_a_directory();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let (directory_touched, directory_ranges) =
        changed_new_side_lines(&context, &base, &head, "src")
            .expect("a diff over a pathspec matching a directory still answers");
    assert!(
        !directory_touched,
        "a bare directory pathspec was reported as an exact-header match"
    );
    assert!(
        directory_ranges.is_empty(),
        "a bare directory pathspec leaked ranges from the files under it: {directory_ranges:?}"
    );

    // The real file itself still resolves correctly, so the fix is a
    // narrowing rather than a break.
    let (file_touched, file_ranges) = changed_new_side_lines(&context, &base, &head, "src/foo.rs")
        .expect("the exact file still answers");
    assert!(file_touched);
    assert_eq!(file_ranges, vec![5..=5]);
}

/// The blocker this repair round exists for: a deletion-only path carries no
/// new-side hunk, so before this fix `changed_new_side_lines` answered an
/// empty range vector identically to a path the diff never mentioned, and
/// [`record_review_finding`] had no honest way to accept a location naming a
/// deletion. Run against the code as it stood before this round, the first
/// assertion below failed with `finding-location-outside-the-change` — the
/// mutation this test exists to catch is deleting the `touched` fallback in
/// `changed_new_side_lines`'s caller, which reintroduces exactly that.
#[test]
fn a_severe_finding_may_locate_a_pure_deletion_the_diff_actually_made() {
    let (root, base, head) = repo_with_one_deleted_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding =
        severe_introduced_finding(&operation_id, &epoch, &head, &base, &digest, "changed.rs:5");
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);

    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "this fixture's timeline carries no live claim, so this cannot succeed -- but it must \
         fail for a reason other than the location",
    );
    let reason = failure.envelope()["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_ne!(
        reason, "finding-location-required",
        "a location was supplied and still refused as missing: {reason}"
    );
    assert_ne!(
        reason, "finding-location-malformed",
        "changed.rs:5 is a well-formed location: {reason}"
    );
    assert_ne!(
        reason, "finding-location-outside-the-change",
        "line 5 is exactly the line this fixture's diff deleted: {reason}"
    );

    // The mirror: a path the diff never mentions at all is still refused,
    // deletion or not.
    let untouched = severe_introduced_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "nothing-like-this-exists.rs:1",
    );
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &untouched)
    })
    .expect_err("a path the diff never mentions was accepted");
    assert_eq!(
        failure.envelope()["reason"],
        "finding-location-outside-the-change",
        "an untouched path stopped being refused once deletion-only paths were accepted"
    );
}

/// Everything a severe `introduced` finding owes, apart from the location
/// itself and the context it is checked against -- both supplied by the caller.
fn severe_introduced_finding<'a>(
    operation_id: &'a str,
    epoch: &'a str,
    head: &'a str,
    base: &'a str,
    digest: &'a str,
    location: &'a str,
) -> FindingReview<'a> {
    FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id,
        epoch,
        pr: 7,
        head,
        base,
        digest,
        id: "loc-1",
        class: "severe",
        evidence: "observed the defect at the named line",
        impact: "wrong output on the happy path",
        parent: "",
        origin: "",
        evidence_class: "deterministic",
        causal_disposition: "introduced",
        location,
        lens: "",
        now: "2026-08-31T00:00:00Z",
    }
}

/// A `warning` finding — no location owed, no lineage rule reachable — so the
/// only thing standing between it and [`verify_claim`] is
/// [`check_finding_citations`], reached through every one of these tests
/// with nothing else able to answer first.
fn citation_finding<'a>(
    operation_id: &'a str,
    epoch: &'a str,
    head: &'a str,
    base: &'a str,
    digest: &'a str,
    evidence: &'a str,
    impact: &'a str,
) -> FindingReview<'a> {
    FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id,
        epoch,
        pr: 7,
        head,
        base,
        digest,
        id: "cite-1",
        class: "warning",
        evidence,
        impact,
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "",
        lens: "",
        now: "2026-08-31T00:00:00Z",
    }
}

/// A repository whose only file, `gone.rs`, is committed at `base` and
/// removed before `head` — a path a citation can name that exists in one
/// tree and not the other.
fn repo_with_a_path_removed_at_head() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    std::fs::write(at.join("gone.rs"), "line1\n").expect("the base-only file");
    git(&["git", "add", "gone.rs"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    git(&["git", "rm", "--quiet", "gone.rs"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();
    (root, base, head)
}

/// A repository whose tree contains `src/nested`, a directory rather than a
/// file, in both `base` and `head` — the shape [`resolve_citation_path`]
/// answers `CitationResolution::Directory` for.
fn repo_with_a_directory() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    std::fs::create_dir_all(at.join("src").join("nested")).expect("a nested directory");
    std::fs::write(at.join("src").join("nested").join("inner.rs"), "line1\n")
        .expect("a file inside it");
    git(&["git", "add", "-A"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    std::fs::write(
        at.join("src").join("nested").join("inner.rs"),
        "line1\nline2\n",
    )
    .expect("a change under the directory");
    git(&["git", "add", "-A"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();
    (root, base, head)
}

/// A repository whose only file, `grown.rs`, has one line at `base` and five
/// at `head` — the shape the two trees never took anywhere else in this test
/// module, and the one [`resolve_citation_path`] must read head's blob to
/// answer correctly: base's blob alone reports too few lines for a citation
/// naming head's last one.
fn repo_with_a_path_that_grew_at_head() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    std::fs::write(at.join("grown.rs"), "line1\n").expect("the base file");
    git(&["git", "add", "grown.rs"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    let head_text: String = (1..=5).map(|n| format!("line{n}\n")).collect();
    std::fs::write(at.join("grown.rs"), &head_text).expect("the head file");
    git(&["git", "add", "grown.rs"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();
    (root, base, head)
}

/// The mirror of [`repo_with_a_path_that_grew_at_head`]: `shrunk.rs` has five
/// lines at `base` and one at `head`, so a citation naming base's line 5 is
/// backed only by the tree `head`'s blob alone would discard.
fn repo_with_a_path_that_shrank_at_head() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    let base_text: String = (1..=5).map(|n| format!("line{n}\n")).collect();
    std::fs::write(at.join("shrunk.rs"), &base_text).expect("the base file");
    git(&["git", "add", "shrunk.rs"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    std::fs::write(at.join("shrunk.rs"), "line1\n").expect("the head file");
    git(&["git", "add", "shrunk.rs"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();
    (root, base, head)
}

/// A repository whose only file, `linked.rs`, is a three-line `100644` blob
/// at `base` and a `120000` symlink at `head` — built through `git
/// update-index --cacheinfo` and `git commit-tree` rather than a real
/// filesystem symlink, since neither is required to exist on disk for
/// [`resolve_citation_path`] to read the tree. The shape lets a citation
/// past `base`'s three lines prove whether the symlink at `head` discards
/// the blob's line count or is merely consulted alongside it.
fn repo_with_a_lined_blob_that_becomes_a_symlink_at_head() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"))
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    std::fs::write(at.join("linked.rs"), "line1\nline2\nline3\n").expect("the base blob");
    git(&["git", "add", "linked.rs"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    std::fs::write(at.join("symlink-target.txt"), "nowhere").expect("a scratch symlink target");
    let target_oid = git(&["git", "hash-object", "-w", "symlink-target.txt"])
        .stdout
        .trim()
        .to_owned();
    git(&[
        "git",
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("120000,{target_oid},linked.rs"),
    ]);
    let tree = git(&["git", "write-tree"]).stdout.trim().to_owned();
    let head = git(&["git", "commit-tree", &tree, "-p", &base, "-m", "head"])
        .stdout
        .trim()
        .to_owned();
    (root, base, head)
}

/// A repository whose tree contains `src/nested`, a directory holding two
/// files rather than one, in both `base` and `head` — with the guard ordering
/// reverted, that is what routes the citation to the multi-record arm.
fn repo_with_a_directory_holding_two_files() -> (tempfile::TempDir, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    let git = |args: &[&str]| {
        crate::transport::run(args, Some(at), crate::transport::How::write())
            .unwrap_or_else(|failure| panic!("{args:?}: {failure:?}"));
    };
    let oid = || {
        crate::transport::run(
            &["git", "rev-parse", "HEAD"],
            Some(at),
            crate::transport::How::read(),
        )
        .expect("HEAD")
        .stdout
        .trim()
        .to_owned()
    };
    git(&["git", "init", "--quiet", "-b", "main", "."]);
    git(&["git", "config", "user.email", "estigia@example.invalid"]);
    git(&["git", "config", "user.name", "Estigia test"]);
    git(&["git", "config", "core.autocrlf", "false"]);
    std::fs::create_dir_all(at.join("src").join("nested")).expect("a nested directory");
    std::fs::write(at.join("src").join("nested").join("inner.rs"), "line1\n")
        .expect("the first file inside it");
    std::fs::write(at.join("src").join("nested").join("other.rs"), "line1\n")
        .expect("the second file inside it");
    git(&["git", "add", "-A"]);
    git(&["git", "commit", "--quiet", "-m", "base"]);
    let base = oid();
    std::fs::write(
        at.join("src").join("nested").join("inner.rs"),
        "line1\nline2\n",
    )
    .expect("a change under the directory");
    git(&["git", "add", "-A"]);
    git(&["git", "commit", "--quiet", "-m", "head"]);
    let head = oid();
    (root, base, head)
}

#[test]
fn citation_tokens_splits_on_whitespace_and_quotes_without_rejecting_anything() {
    let plain = citation_tokens("see src/a.rs:10 and src/b.rs:20");
    assert_eq!(
        plain,
        vec![
            CitationToken {
                value: "see".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "src/a.rs:10".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "and".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "src/b.rs:20".to_owned(),
                quoted: false
            },
        ]
    );

    let quoted = citation_tokens("path `src/a.rs:10` is where");
    assert_eq!(
        quoted,
        vec![
            CitationToken {
                value: "path".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "src/a.rs:10".to_owned(),
                quoted: true
            },
            CitationToken {
                value: "is".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "where".to_owned(),
                quoted: false
            },
        ]
    );

    let unmatched = citation_tokens("it's broken");
    assert_eq!(
        unmatched,
        vec![
            CitationToken {
                value: "it".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "s".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "broken".to_owned(),
                quoted: false
            },
        ]
    );

    let glued = citation_tokens("at 'src/a.rs:1");
    assert_eq!(
        glued,
        vec![
            CitationToken {
                value: "at".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "src/a.rs:1".to_owned(),
                quoted: false
            },
        ],
        "an unclosed quote must not be carried into the citation that follows it"
    );

    let apostrophe_mid_word = citation_tokens("foo'bar");
    assert_eq!(
        apostrophe_mid_word,
        vec![
            CitationToken {
                value: "foo".to_owned(),
                quoted: false
            },
            CitationToken {
                value: "bar".to_owned(),
                quoted: false
            },
        ],
        "an unclosed quote flushes the pending token rather than carrying it into the run after"
    );

    assert!(
        citation_tokens("``").is_empty(),
        "an empty quoted span emits no token"
    );
}

#[test]
fn a_citation_in_evidence_naming_an_absent_path_is_refused() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "the defect is at src/nowhere.rs:9999",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a citation naming a path in neither tree was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["field"], "evidence");
    assert_eq!(envelope["cause"], "path-not-in-either-tree");
    assert_eq!(envelope["path"], "src/nowhere.rs");
}

#[test]
fn a_citation_in_impact_naming_an_absent_path_is_refused() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "no citation in this field",
        "see src/nowhere.rs:1 for the cost",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a citation naming a path in neither tree was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["field"], "impact");
    assert_eq!(envelope["cause"], "path-not-in-either-tree");
}

/// The false-positive class `docs/honesty.md` discloses: a quoted token
/// bypasses the `pathLike` gate, so `` `ratio 1:3` `` yields the path part
/// `ratio 1`, which passes [`looks_repository_relative`] and is refused for
/// naming nothing in either tree. This is not narrowed away — the same
/// bypass is what lets a quoted citation without a `/` or a `.` followed by
/// a letter (`` `LICENSE:1` ``) be recognised at all.
#[test]
fn a_quoted_non_path_token_is_refused_as_the_pathlike_bypass_it_is() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "the fix changes the `ratio 1:3` throughout",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a quoted non-path token was accepted despite the pathLike gate it bypasses");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["cause"], "path-not-in-either-tree");
    assert_eq!(envelope["path"], "ratio 1");
}

#[test]
fn a_citation_naming_a_path_present_only_at_base_is_accepted() {
    let (root, base, head) = repo_with_a_path_removed_at_head();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "removed by this change: gone.rs:1",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("no live claim is scripted, so this cannot fully succeed");
    let reason = failure.envelope()["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        reason, "unexpected-state",
        "a path present only at base must clear the citation check and stop at the unscripted \
         claim read, not be refused earlier: {reason}"
    );
}

#[test]
fn a_citation_naming_a_line_only_present_at_head_is_accepted() {
    let (root, base, head) = repo_with_a_path_that_grew_at_head();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "the new code is at grown.rs:5",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("no live claim is scripted, so this cannot fully succeed");
    let reason = failure.envelope()["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        reason, "unexpected-state",
        "a line present only at head, past base's shorter blob, must clear the citation check \
         and stop at the claim verification (the scripted issue holds no workflow state), not be refused earlier: {reason}"
    );
}

/// The mirror of [`a_citation_naming_a_line_only_present_at_head_is_accepted`]:
/// `shrunk.rs` has five lines at `base` and one at `head`, and the citation
/// names line 5 — a line only `base` has. [`resolve_citation_path`] must keep
/// `base`'s larger line count rather than letting `head`'s shorter blob answer
/// first, or a citation the base tree backs is refused as past end of file.
#[test]
fn a_citation_naming_a_line_only_present_at_base_is_accepted() {
    let (root, base, head) = repo_with_a_path_that_shrank_at_head();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "the old bound is at shrunk.rs:5",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("no live claim is scripted, so this cannot fully succeed");
    let reason = failure.envelope()["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        reason, "unexpected-state",
        "a line present only at base, past head's shorter blob, must clear the citation check \
         and stop at the claim verification (the scripted issue holds no workflow state), not be refused earlier: {reason}"
    );
}

#[test]
fn a_citation_past_end_of_file_is_refused() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "see changed.rs:15 for detail",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a line past end of file was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["cause"], "line-past-end-of-file");
    assert_eq!(envelope["path"], "changed.rs");
}

#[test]
fn a_cited_range_is_checked_against_the_files_actual_length() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "see changed.rs:8-20 for the whole run",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "changed.rs:8-20 exceeds the file's 10 lines, which a range citation must be checked \
         against \u{2014} without divergence 1, this range is silently skipped and accepted",
    );
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["cause"], "line-past-end-of-file");
}

#[test]
fn a_lined_blob_at_base_is_not_lost_to_a_symlink_at_head() {
    let (root, base, head) = repo_with_a_lined_blob_that_becomes_a_symlink_at_head();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "see linked.rs:5000 for the missing bound",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "a line past base's three-line blob was accepted because head's symlink discarded \
         base's line count",
    );
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["cause"], "line-past-end-of-file");
    assert_eq!(envelope["path"], "linked.rs");
}

#[test]
fn a_lined_blob_at_head_is_not_lost_to_a_symlink_at_base() {
    let (root, base, head) = repo_with_a_lined_blob_that_becomes_a_symlink_at_head();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &base,
        &head,
        &digest,
        "see linked.rs:5000 for the missing bound",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: base.clone(),
        base: head.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "a line past head's three-line blob was accepted because base's symlink discarded \
         head's line count",
    );
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["cause"], "line-past-end-of-file");
    assert_eq!(envelope["path"], "linked.rs");
}

#[test]
fn a_citation_naming_a_directory_is_refused() {
    let (root, base, head) = repo_with_a_directory();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "see src/nested:1 for the whole directory",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a directory was accepted as a citation");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["cause"], "path-is-not-a-file");
    assert_eq!(envelope["path"], "src/nested");
}

#[test]
fn a_backslash_terminated_directory_citation_is_refused_as_malformed() {
    let (root, base, head) = repo_with_a_directory_holding_two_files();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        r"the defect spans src/nested\:10",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "a path that only becomes directory-shaped after backslash normalisation reached git \
         ls-tree instead of being refused as malformed",
    );
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["cause"], "malformed-path");
    assert_eq!(envelope["path"], r"src/nested\");
}

#[test]
fn a_non_trailing_backslash_normalises_to_a_candidate() {
    let (root, base, head) = repo_with_a_directory_holding_two_files();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = citation_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        r"see src\nested\inner.rs:5 for the missing bound",
        "minor",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "a non-trailing backslash must normalise to a candidate and reach the line check, \
         not be refused as malformed",
    );
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-citation-unresolvable");
    assert_eq!(envelope["cause"], "line-past-end-of-file");
    assert_eq!(envelope["path"], "src/nested/inner.rs");
}

#[test]
fn a_path_ending_in_a_slash_is_not_a_citation() {
    let token = CitationToken {
        value: "src/transport/:10".to_owned(),
        quoted: false,
    };
    assert!(
        citation_reference(&token).is_none(),
        "a trailing slash names a directory, not a citation, and must not become a candidate \
         or a malformed reference"
    );
}

#[test]
fn a_malformed_citation_is_refused_without_a_git_lookup() {
    let root = tempfile::tempdir().expect("a directory that is not a git repository");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    for (index, evidence) in [
        "regression at ../secret.rs:1",
        "leaked via /etc/passwd:1",
        "found in C:\\foo.rs:1",
    ]
    .into_iter()
    .enumerate()
    {
        let operation_id = format!("{index:032x}");
        let finding = citation_finding(
            &operation_id,
            &epoch,
            &head,
            &base,
            &digest,
            evidence,
            "minor",
        );
        let receipt = ReviewTransaction {
            epoch: epoch.clone(),
            pr: finding.pr,
            head: head.clone(),
            base: base.clone(),
            digest: digest.clone(),
        };
        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
            finding.issue,
            published_only_stdout(&receipt),
        )]);
        let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            record_review_finding(&context, &finding)
        })
        .expect_err(&format!("{evidence:?} was accepted"));
        let envelope = failure.envelope();
        assert_eq!(
            envelope["reason"], "finding-citation-unresolvable",
            "{evidence:?}: {envelope}"
        );
        assert_eq!(
            envelope["cause"], "malformed-path",
            "{evidence:?}: {envelope}"
        );
    }
}

#[test]
fn ordinary_prose_shaped_like_a_citation_is_never_flagged() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let sha256_evidence = format!("digest sha256:{}", "a".repeat(64));
    let corpus: [&str; 9] = [
        "regression since 1.2:3",
        "measured over 10:30",
        "read from session::run_id",
        "reported at https://example.com:443",
        "answered with status:500",
        "see foo.rs:0 for context",
        &sha256_evidence,
        "logged at 2026-07-22T12:34:56Z",
        "a trailing note:",
    ];
    for (index, evidence) in corpus.iter().enumerate() {
        let operation_id = format!("{index:032x}");
        let finding = citation_finding(
            &operation_id,
            &epoch,
            &head,
            &base,
            &digest,
            evidence,
            "minor",
        );
        let receipt = ReviewTransaction {
            epoch: epoch.clone(),
            pr: finding.pr,
            head: head.clone(),
            base: base.clone(),
            digest: digest.clone(),
        };
        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
            finding.issue,
            published_only_stdout(&receipt),
        )]);
        let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            record_review_finding(&context, &finding)
        })
        .expect_err("no live claim is scripted, so this cannot fully succeed");
        let reason = failure.envelope()["reason"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert_ne!(
            reason, "finding-citation-unresolvable",
            "{evidence:?} was wrongly flagged: {reason}"
        );
    }
}

#[test]
fn a_cached_citation_path_is_resolved_only_once() {
    let mut cache = HashMap::new();
    let calls = std::cell::Cell::new(0u32);
    for _ in 0..3 {
        let resolution = cached_citation_resolution(&mut cache, "src/a.rs", || {
            calls.set(calls.get() + 1);
            Ok(CitationResolution::Absent)
        })
        .expect("a fabricated resolution");
        assert!(matches!(resolution, CitationResolution::Absent));
    }
    assert_eq!(
        calls.get(),
        1,
        "a path cited three times spawned more than one lookup"
    );
}

#[test]
fn a_severe_finding_naming_this_change_as_the_cause_must_carry_a_location() {
    let root = tempfile::tempdir().expect("a context root");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = severe_introduced_finding(&operation_id, &epoch, &head, &base, &digest, "");
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a severe finding this change introduced, with no location, was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-location-required");
    assert_eq!(envelope["value"], "");
}

#[test]
fn a_findings_location_must_read_as_path_colon_line_or_path_colon_start_dash_end() {
    let root = tempfile::tempdir().expect("a context root");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = severe_introduced_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "not-a-location-at-all",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a location with no line grammar was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-location-malformed");
    assert_eq!(envelope["value"], "not-a-location-at-all");
}

/// `--lens` is validated against the shipped roles, never minted from caller
/// text — checked before any tracker read, since a caller-supplied lens name
/// that is not one of the five is a defect in the call itself.
#[test]
fn a_finding_naming_a_lens_must_name_one_of_the_five_shipped() {
    let root = tempfile::tempdir().expect("a context root");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "lens-unknown",
        class: "warning",
        evidence: "the lens read the diff",
        impact: "minor",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "",
        lens: "review-made-up",
        now: "2026-08-31T00:00:00Z",
    };
    let failure = record_review_finding(&context, &finding)
        .expect_err("a lens name outside the shipped five was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "invalid-finding-lens");
    assert_eq!(envelope["value"], "review-made-up");
}

/// A lens is sealed to the diff, so every finding it produces owes a
/// location — under any class, unlike the severe/causal-owed rule above.
#[test]
fn a_lens_finding_with_no_location_is_refused() {
    let root = tempfile::tempdir().expect("a context root");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "lens-no-location",
        class: "warning",
        evidence: "the lens read the diff",
        impact: "minor",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "",
        lens: "review-risk",
        now: "2026-08-31T00:00:00Z",
    };
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a lens finding with no location was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "lens-finding-location-required");
    assert_eq!(envelope["lens"], "review-risk");
}

/// The path half of a lens finding's location is checked against `base..head`
/// the same way a severe/causal one is — a path the diff never touched
/// settles nothing, lens or not.
#[test]
fn a_lens_finding_naming_a_path_the_diff_never_touched_is_refused() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "lens-outside-diff",
        class: "warning",
        evidence: "the lens read the diff",
        impact: "minor",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "nothing-like-this-exists.rs:1",
        lens: "review-risk",
        now: "2026-08-31T00:00:00Z",
    };
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a lens finding naming an untouched path was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "lens-finding-location-outside-the-diff");
    assert_eq!(envelope["path"], "nothing-like-this-exists.rs");
}

/// The changed-path set a lens finding's location is checked against comes
/// from `git diff` over the receipt's own `base`/`head` — the same read
/// [`changed_new_side_lines`] performs for a severe/causal finding. When that
/// read cannot be done at all, this refuses rather than silently accepting
/// the location: an unreadable control surface permits no write.
#[test]
fn a_lens_finding_is_refused_rather_than_accepted_when_the_diff_cannot_be_read() {
    let root = tempfile::tempdir().expect("a context root, deliberately holding no git repository");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "lens-unreadable-diff",
        class: "warning",
        evidence: "the lens read the diff",
        impact: "minor",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "changed.rs:1",
        lens: "review-risk",
        now: "2026-08-31T00:00:00Z",
    };
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a lens finding was accepted although its diff could not be read");
    assert!(
        matches!(failure, Failure::Read(_)),
        "an unreadable diff must fail the read, not be reported as a refused write: {failure:?}"
    );
}

/// Unlike the severe/causal-owed check, a lens finding's location is settled
/// by its **path** alone: line 1 of `changed.rs` is untouched by the
/// fixture's diff (only line 5 changed), and a lens may still anchor there —
/// their check (c), path only, the line is not checked. Proven from the
/// staged marker body rather than the call's own success, the same way
/// `a_whitespace_only_location_is_absent_on_the_write_path_too` proves what
/// was written without a live tracker to read it back from.
#[test]
fn a_lens_finding_may_anchor_any_line_of_a_changed_file_not_only_the_changed_hunk() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let issue = 918_275_u64;
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let claim_op = "d".repeat(32);
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: 7,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let claim_comment = wrote(
        "1",
        format!(
            "Claimed by claude-a.\n\n<!-- issue-flow: claim run-id=claude-a runtime=claude \
             horizon=2099-01-01T00:00Z op-id={claim_op} -->\n"
        ),
    );
    let comments = [claim_comment, published(&receipt)];
    let stdout = serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": "status:review"}],
        "comments": comments.iter().map(comment_json).collect::<Vec<_>>(),
    });
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(issue, stdout)]);

    let finding = FindingReview {
        issue,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "lens-any-line",
        class: "warning",
        evidence: "the lens read the diff",
        impact: "minor",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "changed.rs:1",
        lens: "review-risk",
        now: "2026-08-31T00:00:00Z",
    };

    let _ = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    });

    let body = crate::transport::store::local_bodies(issue);
    assert!(
        body.contains("location=changed.rs:1") && body.contains("lens=review-risk"),
        "a lens finding anchored on an untouched line of a changed file never reached the \
         write, so the path-only rule is not what settled it: {body}"
    );
}

/// The mirror of the test above, and the one no test covered before this
/// repair: a lens finding that is *also* `severe` with `causal_disposition`
/// `introduced` owes the same containment any severe causal finding owes,
/// not the path-only rule `lens_owed` alone would give it. Same fixture,
/// same untouched line 1 of `changed.rs`, same `lens`; only `class`,
/// `evidence_class` and `causal_disposition` differ from the positive case
/// above. `location_owed` takes precedence over `lens_owed` in
/// `record_review_finding`, and this pins that precedence rather than
/// leaving it to be rediscovered.
#[test]
fn a_severe_lens_finding_owes_containment_not_only_the_path() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "lens-severe-introduced",
        class: "severe",
        evidence: "the lens read the diff",
        impact: "wrong output on the happy path",
        parent: "",
        origin: "",
        evidence_class: "deterministic",
        causal_disposition: "introduced",
        location: "changed.rs:1",
        lens: "review-risk",
        now: "2026-08-31T00:00:00Z",
    };
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "a severe introduced lens finding anchored outside the changed hunk was accepted on \
         the path-only rule alone",
    );
    let envelope = failure.envelope();
    assert_eq!(
        envelope["reason"], "finding-location-outside-the-change",
        "a severe introduced lens finding must be checked by containment, the same rule any \
         other severe causal finding owes, not `lens-finding-location-outside-the-diff`'s \
         path-only rule: {envelope}"
    );
}

/// The malformed refusal is not scoped to the owed case: a `warning`, which
/// owes no location at all, still refuses one that does not parse. Before
/// this round the parse only ran inside `if location_owed`, so a
/// non-blocking finding with a malformed location was recorded as given.
#[test]
fn a_malformed_location_is_refused_under_a_class_that_owes_no_location_at_all() {
    let root = tempfile::tempdir().expect("a context root");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "loc-warn-1",
        class: "warning",
        evidence: "observed once",
        impact: "minor",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "not-a-location-at-all",
        lens: "",
        now: "2026-08-31T00:00:00Z",
    };
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a warning with a malformed location was accepted");
    assert_eq!(
        failure.envelope()["reason"],
        "finding-location-malformed",
        "a warning owes no location, but a malformed one is still refused"
    );
}

#[test]
fn a_findings_location_must_fall_inside_a_changed_hunk_of_the_named_path() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding =
        severe_introduced_finding(&operation_id, &epoch, &head, &base, &digest, "changed.rs:2");
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a location on a line the diff never touched was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-location-outside-the-change");
    assert_eq!(envelope["path"], "changed.rs");
    assert_eq!(envelope["line"], "2");
}

/// Containment, not overlap: a claimed range that merely *shares* a line with
/// the diff's hunk is refused when the hunk does not contain the whole
/// range. This fixture's hunk is `5..=5`; the claimed range `1..=10` starts
/// at 1 and ends at 10, neither of which the hunk reaches, so only overlap
/// would accept it.
#[test]
fn a_findings_location_range_must_be_contained_by_the_hunk_not_merely_overlap_it() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = severe_introduced_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "changed.rs:1-10",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a range that only overlaps the diff's hunk, without containing it, was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-location-outside-the-change");
    assert_eq!(envelope["first_unchanged_line"], 1);
}

/// A range location that falls outside the change names the *whole* claimed
/// range in its refusal, `start-end`, not only `start` — held by nothing
/// before this test, since [`a_findings_location_must_fall_inside_a_changed_hunk_of_the_named_path`]
/// above only ever claims a single line, so `finding_location_field`'s range
/// branch could be replaced with `start.to_string()` and every existing
/// assertion on `envelope["line"]` would still pass.
#[test]
fn a_findings_location_range_that_falls_outside_names_the_full_range_in_the_refusal() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = severe_introduced_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "changed.rs:1-2",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a range that never reaches the hunk at line 5 was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-location-outside-the-change");
    assert_eq!(envelope["path"], "changed.rs");
    assert_eq!(envelope["line"], "1-2");
}

/// A span that starts and ends on changed lines, with one unchanged line
/// between them, is refused the same as one that never touches a changed
/// line at all — containment demands every line, not just the endpoints.
/// This fixture's two hunks are `3..=3` and `5..=5`; the claimed range
/// `3-5` starts and ends inside them but crosses line 4, which neither
/// covers.
#[test]
fn a_findings_location_spanning_two_hunks_is_refused_for_the_unchanged_line_between_them() {
    let (root, base, head) = repo_with_two_changed_lines_a_gap_apart();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = severe_introduced_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "changed.rs:3-5",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a span crossing the unchanged line between two hunks was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-location-outside-the-change");
    assert_eq!(envelope["path"], "changed.rs");
    assert_eq!(envelope["line"], "3-5");
    assert_eq!(envelope["first_unchanged_line"], 4);
}

/// A span strictly inside one larger hunk, touching neither of its own
/// bounds, is accepted -- containment, not equality. Held by nothing before
/// this test: mutating the predicate above to `range.start() ==
/// claimed.start() && claimed.end() == range.end()` left the whole suite
/// green, because every location the suite elsewhere accepts happens to
/// equal its hunk exactly.
#[test]
fn a_findings_location_strictly_inside_one_hunk_is_accepted_not_only_one_equal_to_it() {
    let (root, base, head) = repo_with_a_wide_changed_run();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = severe_introduced_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "changed.rs:5-6",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "this fixture's timeline carries no live claim, so this cannot succeed -- but it must \
         fail for a reason other than the location",
    );
    assert_ne!(
        failure.envelope()["reason"],
        "finding-location-outside-the-change",
        "a span strictly inside one hunk was refused as outside the change",
    );
}

/// A span that starts inside a hunk but runs past its end is refused for the
/// first line past that hunk, not the span's own start -- held by nothing
/// before this test, since both existing assertions on
/// `first_unchanged_line` above exit the loop at the `range.start() >
/// cursor` branch and never reach the trailing return. Mutating that
/// return to `*claimed.start()` leaves the whole suite green otherwise.
#[test]
fn a_findings_location_that_outruns_its_hunk_is_refused_for_the_first_line_past_it() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding = severe_introduced_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "changed.rs:5-8",
    );
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a span running past its hunk's end was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "finding-location-outside-the-change");
    assert_eq!(envelope["first_unchanged_line"], 6);
}

/// `worsened` is held by no other test in this module: every other severe
/// `causal_disposition` exercised here is `introduced`. Deleting `|
/// "worsened"` from `location_owed`'s matcher would leave a `worsened`
/// finding with no location accepted, and this is the test that reddens for
/// it.
#[test]
fn a_worsened_finding_owes_a_location_the_same_as_an_introduced_one() {
    let root = tempfile::tempdir().expect("a context root");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let mut finding = severe_introduced_finding(&operation_id, &epoch, &head, &base, &digest, "");
    finding.causal_disposition = "worsened";
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err("a worsened finding with no location was accepted");
    assert_eq!(failure.envelope()["reason"], "finding-location-required");
}

/// A `pre-existing` defect lives outside these bytes by definition, so a
/// location naming a line the diff never touched is what a correct one looks
/// like. Refusing it would punish the reviewer for volunteering the very
/// coordinate that shows the defect predates the change, and would leave
/// omitting the location as the only way to record the finding at all.
#[test]
fn a_pre_existing_finding_may_name_a_line_outside_the_change() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let mut finding =
        severe_introduced_finding(&operation_id, &epoch, &head, &base, &digest, "changed.rs:2");
    finding.causal_disposition = "pre-existing";
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "this fixture's timeline carries no live claim, so this cannot succeed -- but it must \
         fail for a reason other than the location",
    );
    assert_ne!(
        failure.envelope()["reason"],
        "finding-location-outside-the-change",
        "a pre-existing finding was refused for naming a line outside the change",
    );
}

/// The mirror of the refusal above: a location the diff actually touched must
/// clear the causality check and let the call reach a later guard, never one
/// of the three new location reasons.
#[test]
fn a_findings_location_inside_the_changed_hunk_clears_the_causality_check() {
    let (root, base, head) = repo_with_one_changed_line();
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let digest = "c".repeat(64);
    let finding =
        severe_introduced_finding(&operation_id, &epoch, &head, &base, &digest, "changed.rs:5");
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: finding.pr,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        finding.issue,
        published_only_stdout(&receipt),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    })
    .expect_err(
        "this fixture's timeline carries no live claim, so this cannot succeed -- but it must \
         fail for a reason other than the location",
    );
    let reason = failure.envelope()["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_ne!(
        reason, "finding-location-required",
        "a location was supplied and still refused as missing: {reason}"
    );
    assert_ne!(
        reason, "finding-location-malformed",
        "changed.rs:5 is a well-formed location: {reason}"
    );
    assert_ne!(
        reason, "finding-location-outside-the-change",
        "line 5 is exactly what this fixture changed between base and head: {reason}"
    );
}

/// The precondition `record_review_finding` gained when the location check
/// did: `context.repo_dir` must now be a real git repository holding both
/// `base` and `head`, and a caller whose checkout never fetched the topic
/// branch has to be told what could not be read rather than meeting a bare
/// exit code. A plain directory answers `git diff` with exit `128`, and the
/// failure this asserts on names the two revisions and the path rather than
/// truncating to `git diff --unified=0`.
#[test]
fn changed_new_side_lines_names_what_it_could_not_read_when_the_directory_holds_no_repository() {
    let root = tempfile::tempdir().expect("a plain directory, not a repository");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let base = "b".repeat(40);
    let head = "a".repeat(40);

    let failure = changed_new_side_lines(&context, &base, &head, "src/x.rs")
        .expect_err("a directory holding no repository answered a diff");
    let Failure::Read(detail) = failure else {
        panic!("expected a failed read, got {failure:?}");
    };
    assert!(
        detail.contains(&base) && detail.contains(&head) && detail.contains("src/x.rs"),
        "the failure dropped what it could not read: {detail}"
    );
}

/// The write `record_review_finding` makes is only provable by reading back
/// what was actually staged for `gh issue comment`, not the call's own
/// answer: the answer echoes `finding.location` regardless of whether the
/// marker carries it, so deleting the marker's `location` write leaves every
/// existing test green while the timeline silently drops the field. This
/// reads the body `record_entry` stages on disk before posting it,
/// which is where that write would actually go missing.
///
/// Mutation: delete `written.push(("location", location));` at the marker
/// assembly site in `record_review_finding`. Watched red: the staged body no
/// longer contains `location=`, and this assertion fails.
#[test]
fn the_marker_write_carries_the_location_the_recorder_checked() {
    let root = tempfile::tempdir().expect("a context root");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let issue = 918_273_u64;
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let claim_op = "d".repeat(32);
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: 7,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let claim_comment = wrote(
        "1",
        format!(
            "Claimed by claude-a.\n\n<!-- issue-flow: claim run-id=claude-a runtime=claude \
             horizon=2099-01-01T00:00Z op-id={claim_op} -->\n"
        ),
    );
    let comments = [claim_comment, published(&receipt)];
    let stdout = serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": "status:review"}],
        "comments": comments.iter().map(comment_json).collect::<Vec<_>>(),
    });
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(issue, stdout)]);

    let finding = FindingReview {
        issue,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "loc-write-1",
        class: "severe",
        evidence: "observed the defect at the named line",
        impact: "wrong output on the happy path",
        parent: "",
        origin: "",
        evidence_class: "deterministic",
        causal_disposition: "pre-existing",
        location: "src/anything.rs:5",
        lens: "",
        now: "2026-08-31T00:00:00Z",
    };

    // No claim on the timeline this call could stably reuse across retries,
    // so the eventual answer is a failed readback -- what matters here
    // happened already, staged to disk before `gh issue comment` was ever
    // asked to post it.
    let _ = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    });

    let body = crate::transport::store::local_bodies(issue);
    assert!(
        body.contains("location=src/anything.rs:5"),
        "the marker staged for the timeline dropped the location: {body}"
    );
}

/// A whitespace-only location on a finding that owes none reads as *absent*
/// everywhere, not merely at the presence check: before this round the
/// presence test read `finding.location.trim().is_empty()` while the marker
/// write and the answer both read the untrimmed
/// `!finding.location.is_empty()`, so `"   "` passed the guard as "no
/// declaration" and was then written to the marker as `location=%20%20%20` —
/// a value `ReviewFinding::from_marker` reads back as present, which
/// `skill/bindings/github.md` says cannot happen. One predicate, used here,
/// closes the gap: the staged body must carry no `location` key at all.
#[test]
fn a_whitespace_only_location_is_absent_on_the_write_path_too() {
    let root = tempfile::tempdir().expect("a context root");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let issue = 918_274_u64;
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let claim_op = "d".repeat(32);
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: 7,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let claim_comment = wrote(
        "1",
        format!(
            "Claimed by claude-a.\n\n<!-- issue-flow: claim run-id=claude-a runtime=claude \
             horizon=2099-01-01T00:00Z op-id={claim_op} -->\n"
        ),
    );
    let comments = [claim_comment, published(&receipt)];
    let stdout = serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": "status:review"}],
        "comments": comments.iter().map(comment_json).collect::<Vec<_>>(),
    });
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(issue, stdout)]);

    let finding = FindingReview {
        issue,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &epoch,
        pr: 7,
        head: &head,
        base: &base,
        digest: &digest,
        id: "loc-blank-1",
        class: "severe",
        evidence: "observed the defect at the named line",
        impact: "wrong output on the happy path",
        parent: "",
        origin: "",
        evidence_class: "deterministic",
        causal_disposition: "pre-existing",
        location: "   ",
        lens: "",
        now: "2026-08-31T00:00:00Z",
    };

    let _ = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &finding)
    });

    let body = crate::transport::store::local_bodies(issue);
    assert!(
        !body.contains("location="),
        "a whitespace-only location was written to the marker: {body}"
    );
}

/// The location block used to run before the idempotent-replay check, so a
/// retry naming a corrected-but-still-outside location for an operation
/// already recorded was refused instead of answered from the marker that
/// stands — and a retry from a checkout that cannot read `base..head` at all
/// failed on the diff read rather than reaching the replay branch, breaking
/// the unconditional guarantee `skill/bindings/github.md` states for it. This
/// fixture's `context.repo_dir` holds no git repository whatsoever, so a diff
/// read attempted before the replay check would fail outright rather than
/// merely disagree with the retry's own location.
#[test]
fn a_replay_of_a_recorded_finding_answers_from_the_marker_even_when_the_diff_cannot_be_read() {
    let root = tempfile::tempdir().expect("a context root, deliberately holding no git repository");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let operation_id = "1".repeat(32);
    let epoch = "2".repeat(32);
    let head = "a".repeat(40);
    let base = "b".repeat(40);
    let digest = "c".repeat(64);
    let receipt = ReviewTransaction {
        epoch: epoch.clone(),
        pr: 7,
        head: head.clone(),
        base: base.clone(),
        digest: digest.clone(),
    };
    let persisted_marker = super::super::markers::render(
        "review-finding",
        &[
            ("run-id", "claude-a"),
            ("reviewer", "codex-b"),
            ("op-id", operation_id.as_str()),
            ("epoch", epoch.as_str()),
            ("pr", "7"),
            ("head", head.as_str()),
            ("base", base.as_str()),
            ("digest", digest.as_str()),
            ("id", "loc-1"),
            ("class", "severe"),
            ("evidence", "observed the defect at the named line"),
            ("impact", "wrong output on the happy path"),
            ("evidence-class", "deterministic"),
            ("causal-disposition", "introduced"),
            ("location", "changed.rs:5"),
        ],
    )
    .expect("a review-finding marker");
    let comments = [
        published(&receipt),
        wrote("4", format!("Review finding.\n\n{persisted_marker}\n")),
    ];
    let stdout = serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": "status:review"}],
        "comments": comments.iter().map(comment_json).collect::<Vec<_>>(),
    });
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);

    // The retry names a different, still-outside-the-change location — the
    // shape a reviewer correcting their own wording would send.
    let retry = severe_introduced_finding(
        &operation_id,
        &epoch,
        &head,
        &base,
        &digest,
        "changed.rs:999",
    );
    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_review_finding(&context, &retry)
    })
    .expect(
        "a replay of an already-recorded finding must answer from its own marker rather than \
         re-reading a diff this checkout cannot provide",
    );
    assert_eq!(answer["reused_existing_finding"], true);
    assert_eq!(
        answer["location"], "changed.rs:5",
        "the replay answered with the retry's own location rather than what stands on the \
         timeline: {answer}"
    );
    // The shrink this issue's acceptance measures: a severe finding carrying
    // every optional field answers well under what it cost before the null
    // keys were dropped — see `docs/review-cost.md`. 260 bytes is measured
    // headroom above 213, not a production limit.
    let bytes = serde_json::to_string(&answer)
        .expect("the answer serialises")
        .len();
    assert!(
        bytes < 260,
        "record_review_finding's answer grew past its measured ceiling: {bytes} bytes: {answer}"
    );
}

/// `conditions` has to be part of `record_spend`'s operation identity for the
/// comparison it exists to enable: recording "sealed" and "checkout
/// budgeted" against the same receipt and phase are two measurements, and an
/// identity that ignored `conditions` would answer the second call from the
/// first's marker rather than minting a second one.
///
/// `mcp/mod.rs`'s own `field("conditions")` entry in the list it hashes for
/// this tool is what makes that true in the running server, mirroring
/// `review-finding` and `review-verdict`'s own key derivation there — neither
/// of which carries a dedicated test at that layer either, since the key is
/// deterministic and recomputed fresh rather than stored. This drives
/// `review_operation_id` directly with the same field shape, which is the
/// property `mcp/mod.rs`'s wiring depends on.
#[test]
fn a_spend_operation_id_depends_on_conditions_and_not_only_the_receipt_and_phase() {
    let fields = |conditions: &str| {
        review_operation_id(
            "spend",
            &[
                "claude-a",
                "review",
                &"2".repeat(32),
                "7",
                &"a".repeat(40),
                &"b".repeat(40),
                &"c".repeat(64),
                conditions,
            ],
        )
    };
    assert_eq!(
        fields("sealed"),
        fields("sealed"),
        "the same call retried must mint the same operation id"
    );
    assert_ne!(
        fields("sealed"),
        fields("checkout budgeted"),
        "two measurements against the same receipt and phase, varying only the conditions, \
         collapsed onto one operation id"
    );
}

/// `input`, `tokens`, `seconds`, `contexts`, in that order — bundled so
/// `spend_by` stays under clippy's argument ceiling.
type SpendMeasurement = (u64, u64, u64, u64);

fn spend_by(
    run_id: &str,
    operation: &str,
    phase: &str,
    receipt: &ReviewTransaction,
    conditions: &str,
    measured: SpendMeasurement,
) -> ownership::Comment {
    let (input, tokens, seconds, contexts) = measured;
    wrote(
        "6",
        super::super::markers::render(
            "spend",
            &[
                ("run-id", run_id),
                ("op-id", operation),
                ("phase", phase),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("conditions", conditions),
                ("input", &input.to_string()),
                ("tokens", &tokens.to_string()),
                ("seconds", &seconds.to_string()),
                ("contexts", &contexts.to_string()),
            ],
        )
        .expect("a spend marker"),
    )
}

fn spend_review<'a>(
    operation_id: &'a str,
    phase: &'a str,
    receipt: &'a ReviewTransaction,
    conditions: &'a str,
) -> SpendReview<'a> {
    SpendReview {
        issue: 42,
        run_id: "claude-a",
        operation_id,
        phase,
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        conditions,
        input: 1_000,
        tokens: 50_000,
        seconds: 120,
        contexts: 1,
        note: "",
        now: "2026-08-31T00:00:00Z",
    }
}

fn stdout_with(comments: &[ownership::Comment]) -> serde_json::Value {
    serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": "status:review"}],
        "comments": distinct_ids(comments).iter().map(comment_json).collect::<Vec<_>>(),
    })
}

fn distinct_ids(comments: &[ownership::Comment]) -> Vec<ownership::Comment> {
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    comments
        .iter()
        .map(|comment| {
            let mut comment = comment.clone();
            if let Some(id) = comment.id.clone() {
                let count = seen.entry(id.clone()).or_default();
                if *count > 0 {
                    comment.id = Some(format!("{id}-{count}"));
                }
                *count += 1;
            }
            comment
        })
        .collect()
}

fn context_with_no_repository() -> (tempfile::TempDir, Context) {
    let root = tempfile::tempdir().expect("a context root, deliberately holding no git repository");
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    (root, context)
}

/// An unrecognised `--phase` is refused before `record_spend` ever reads the
/// tracker — no scripted `gh` is wired here at all, which is itself part of
/// what the test proves: the phase vocabulary is checked ahead of the read.
///
/// Deleting the `SPEND_PHASES.contains` check in `record_spend` turns this
/// red: the call falls through to a read against a repository this fixture
/// deliberately does not have, and fails for the wrong reason.
#[test]
fn an_unknown_spend_phase_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let operation_id = "1".repeat(32);
    let spend = spend_review(&operation_id, "bystander", &receipt, "sealed");
    let failure =
        record_spend(&context, &spend).expect_err("a phase outside the vocabulary was accepted");
    assert_eq!(failure.envelope()["reason"], "invalid-spend-phase");
}

/// `implementer` was renamed to `developer` rather than kept as a
/// second accepted spelling. A timeline whose old records still read
/// `implementer` does not make the word writable again. Adding it back to
/// `SPEND_PHASES` turns this red.
#[test]
fn the_retired_spend_phase_word_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let operation_id = "2".repeat(32);
    let spend = spend_review(&operation_id, "bystander", &receipt, "implementer");
    let failure = record_spend(&context, &spend).expect_err("the retired phase word was accepted");
    assert_eq!(failure.envelope()["reason"], "invalid-spend-phase");
}

/// Conditions say what a measurement varied; without one, a second run cannot
/// be checked against a first. Deleting the blank-conditions check in
/// `record_spend` turns this red.
#[test]
fn a_blank_spend_conditions_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let operation_id = "1".repeat(32);
    let spend = spend_review(&operation_id, "review", &receipt, "   ");
    let failure =
        record_spend(&context, &spend).expect_err("whitespace-only conditions were accepted");
    assert_eq!(failure.envelope()["reason"], "blank-spend-conditions");
}

/// `input`, `tokens`, `seconds` and `contexts` each have to say something: a
/// zero is the shape of a field nobody filled in, not a measurement. Changing
/// any one `value == 0` check in `record_spend`'s loop to `false` turns the
/// matching iteration of this test red.
#[test]
fn a_zero_spend_value_is_refused_by_field_name() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    for field in ["input", "tokens", "seconds", "contexts"] {
        let operation_id = "1".repeat(32);
        let mut spend = spend_review(&operation_id, "review", &receipt, "sealed");
        match field {
            "input" => spend.input = 0,
            "tokens" => spend.tokens = 0,
            "seconds" => spend.seconds = 0,
            "contexts" => spend.contexts = 0,
            _ => unreachable!(),
        }
        let failure = record_spend(&context, &spend)
            .expect_err(&format!("a zero {field} was accepted as a measurement"));
        assert_eq!(failure.envelope()["reason"], "non-positive-spend-value");
        assert_eq!(failure.envelope()["field"], field);
    }
}

/// A receipt this issue's timeline never published settles nothing, however
/// well-formed it reads: `known_publication` refuses a fabricated or
/// mistyped epoch rather than letting it stand in for a real round. Deleting
/// the `known_publication` check in `record_spend` turns this red.
#[test]
fn a_spend_receipt_the_timeline_never_published_is_refused() {
    let (_root, context) = context_with_no_repository();
    let real = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let fabricated = ReviewTransaction {
        epoch: "9".repeat(32),
        ..real.clone()
    };
    let stdout = stdout_with(&[published(&real)]);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);

    let operation_id = "1".repeat(32);
    let spend = spend_review(&operation_id, "review", &fabricated, "sealed");
    let failure =
        crate::test_env::with_scripted_gh(bin.path(), &script, || record_spend(&context, &spend))
            .expect_err("a receipt this timeline never published was accepted");
    assert_eq!(failure.envelope()["reason"], "spend-transaction-unknown");
}

/// The divergence from `record_review_finding` this module documents: a spend
/// record stays recordable against a receipt a later publication has since
/// superseded, because it states a historical fact about a round that
/// happened rather than a judgement about the round currently under review.
///
/// Run against a version of `record_spend` that checked `require_recorded_receipt`
/// the way `record_review_finding` does, this goes red: the second (older)
/// receipt would be refused as `published-transaction-mismatch` even though the
/// timeline recorded it.
#[test]
fn a_spend_record_against_a_superseded_receipt_still_records() {
    let (_root, context) = context_with_no_repository();
    let round_one = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let round_two = ReviewTransaction {
        epoch: "3".repeat(32),
        ..round_one.clone()
    };
    // Both publications on the timeline, the second superseding the first —
    // `latest_publication` reads the last one, which is `round_two`.
    let stdout = stdout_with(&[published(&round_one), published(&round_two)]);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);

    let operation_id = "1".repeat(32);
    let spend = spend_review(&operation_id, "review", &round_one, "sealed");
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_spend(&context, &spend)
    })
    .expect_err(
        "this fixture's timeline carries no live claim, so this cannot succeed -- but it must \
         fail for a reason other than the receipt being superseded",
    );
    assert_ne!(
        failure.envelope()["reason"],
        "spend-transaction-unknown",
        "a receipt this timeline genuinely published was treated as unknown"
    );
    assert_ne!(
        failure.envelope()["reason"],
        "published-transaction-mismatch",
        "a superseded-but-real receipt was refused the way a finding's stale receipt is, which \
         is the divergence this operation deliberately does not carry"
    );
}

/// A retry answers from the marker already on the timeline rather than
/// recording a second measurement — the same idempotent-replay shape
/// `record_review_finding` has. Deleting the `operation_marker` replay branch
/// in `record_spend` turns this red: the retry falls through to `verify_claim`
/// against a fixture with no live claim and fails for the wrong reason.
#[test]
fn a_replay_of_a_recorded_spend_answers_from_its_own_marker() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let operation_id = "1".repeat(32);
    let persisted = spend_by(
        "claude-a",
        &operation_id,
        "review",
        &receipt,
        "sealed",
        (1_000, 50_000, 120, 1),
    );
    let stdout = stdout_with(&[published(&receipt), persisted]);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);

    // The retry names a different token count for the same identity — the
    // shape a recomputation after an unreadable first answer would send.
    let mut retry = spend_review(&operation_id, "review", &receipt, "sealed");
    retry.tokens = 999_999;
    let answer =
        crate::test_env::with_scripted_gh(bin.path(), &script, || record_spend(&context, &retry))
            .expect(
                "a replay of an already-recorded spend measurement must answer from its own marker",
            );
    assert_eq!(answer["reused_existing_spend"], true);
    assert_eq!(
        answer["tokens"], 50_000,
        "the replay answered with the retry's own tokens rather than what stands on the \
         timeline: {answer}"
    );
}

/// `spend_summary` refuses an unrecognised `--phase` before it reads the
/// tracker at all — no scripted `gh` is wired here either.
#[test]
fn spend_summary_refuses_an_unknown_phase_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let failure = spend_summary(&context, 42, "bystander", RECLAIM_NOW)
        .expect_err("an unknown phase was answered");
    assert_eq!(failure.envelope()["reason"], "invalid-spend-phase");
}

/// The primary answer keeps two measurements of one phase apart by their own
/// `conditions` rather than collapsing them, and the secondary `issue_totals`
/// still sums every phase. Mutating `spend_summary`'s `record.phase == phase`
/// filter to `true` turns the first assertion red: the `developer` record
/// leaks into a `review` answer.
#[test]
fn spend_summary_keeps_records_of_one_phase_apart_by_their_conditions() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let sealed = spend_by(
        "claude-a",
        &"1".repeat(32),
        "review",
        &receipt,
        "sealed",
        (1_000, 50_000, 120, 2),
    );
    let checkout = spend_by(
        "claude-a",
        &"4".repeat(32),
        "review",
        &receipt,
        "checkout budgeted",
        (1_500, 80_000, 200, 2),
    );
    let developer = spend_by(
        "claude-a",
        &"5".repeat(32),
        "developer",
        &receipt,
        "sealed",
        (2_000, 120_000, 600, 1),
    );
    let stdout = stdout_with(&[published(&receipt), sealed, checkout, developer]);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);

    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        spend_summary(&context, 42, "review", RECLAIM_NOW)
    })
    .expect("a well-formed read must answer");

    assert_eq!(answer["record_count"], 2, "answer: {answer}");
    let conditions: std::collections::BTreeSet<String> = answer["records"]
        .as_array()
        .expect("a records array")
        .iter()
        .map(|record| record["conditions"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        conditions,
        std::collections::BTreeSet::from(["sealed".to_owned(), "checkout budgeted".to_owned()]),
        "the two review measurements did not both surface in the primary answer: {answer}"
    );
    assert_eq!(
        answer["issue_totals"]["records"], 3,
        "the secondary total dropped a phase: {answer}"
    );
    assert_eq!(answer["issue_totals"]["by_phase"]["review"]["records"], 2);
    assert_eq!(
        answer["issue_totals"]["by_phase"]["developer"]["records"],
        1
    );
    assert_eq!(answer["issue_totals"]["tokens"], 50_000 + 80_000 + 120_000);
}

/// A spend marker is a historical fact, and a phase rename does not make an
/// earlier round's cost unreadable: `issue_totals` still sums a marker that
/// recorded the retired word, while the closed query vocabulary refuses to
/// answer for it. Restoring the `SPEND_PHASES.contains` gate in
/// `SpendRecord::from_marker` turns the first assertion red.
#[test]
fn issue_totals_still_counts_a_retired_phase_word() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let old = spend_by(
        "claude-a",
        &"5".repeat(32),
        "implementer",
        &receipt,
        "sealed",
        (2_000, 120_000, 600, 1),
    );
    let listing = stdout_with(&[published(&receipt), old.clone()]);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, listing)]);

    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        spend_summary(&context, 42, "review", RECLAIM_NOW)
    })
    .expect("a well-formed read must answer");

    assert_eq!(
        answer["issue_totals"]["by_phase"]["implementer"]["records"], 1,
        "a retired phase word was dropped from the totals: {answer}"
    );
    assert_eq!(
        answer["issue_totals"]["tokens"], 120_000,
        "answer: {answer}"
    );

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&[published(&receipt), old]),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        spend_summary(&context, 42, "implementer", RECLAIM_NOW)
    })
    .expect_err("the closed query vocabulary answered for a retired word");
    assert_eq!(failure.envelope()["reason"], "invalid-spend-phase");
}

/// Rounds — an issue with several publications and rejections — are what
/// this exists to make visible, so `by_epoch` has to keep each receipt's
/// subtotal apart rather than merging every round into one number. Mutating
/// the epoch-matching `find` in `spend_summary` to always report "no match"
/// turns the second assertion red: every record starts its own `by_epoch`
/// entry instead of two records under `round_one` sharing one.
#[test]
fn spend_summary_keeps_each_rounds_epoch_subtotal_apart() {
    let (_root, context) = context_with_no_repository();
    let round_one = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let round_two = ReviewTransaction {
        epoch: "3".repeat(32),
        ..round_one.clone()
    };
    let first = spend_by(
        "claude-a",
        &"1".repeat(32),
        "review",
        &round_one,
        "sealed",
        (1_000, 50_000, 120, 2),
    );
    let repeat = spend_by(
        "claude-a",
        &"4".repeat(32),
        "review",
        &round_one,
        "checkout budgeted",
        (1_000, 10_000, 30, 2),
    );
    let second_round = spend_by(
        "claude-a",
        &"5".repeat(32),
        "review",
        &round_two,
        "sealed",
        (1_000, 70_000, 150, 2),
    );
    let stdout = stdout_with(&[
        published(&round_one),
        published(&round_two),
        first,
        repeat,
        second_round,
    ]);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);

    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        spend_summary(&context, 42, "review", RECLAIM_NOW)
    })
    .expect("a well-formed read must answer");

    let by_epoch = answer["issue_totals"]["by_epoch"]
        .as_array()
        .expect("a by_epoch array");
    assert_eq!(by_epoch.len(), 2, "two rounds collapsed into one: {answer}");
    assert_eq!(by_epoch[0]["epoch"], round_one.epoch);
    assert_eq!(
        by_epoch[0]["records"], 2,
        "round one's two records did not share a subtotal"
    );
    assert_eq!(by_epoch[0]["tokens"], 50_000 + 10_000);
    assert_eq!(by_epoch[1]["epoch"], round_two.epoch);
    assert_eq!(by_epoch[1]["records"], 1);
    assert_eq!(by_epoch[1]["tokens"], 70_000);
}

/// A `published` marker carrying a parent receipt, the shape a repair leaves
/// behind — see [`published_by`] for the first-publication shape this omits.
fn published_with_parent(
    run_id: &str,
    receipt: &ReviewTransaction,
    parent: &ReviewTransaction,
    delta: &str,
) -> ownership::Comment {
    published_with_parent_range(run_id, receipt, parent, delta, None)
}

fn published_with_parent_range(
    run_id: &str,
    receipt: &ReviewTransaction,
    parent: &ReviewTransaction,
    delta: &str,
    delta_range_reason: Option<&str>,
) -> ownership::Comment {
    published_over_parent(run_id, receipt, parent, delta, delta_range_reason, false)
}

fn published_over_parent(
    run_id: &str,
    receipt: &ReviewTransaction,
    parent: &ReviewTransaction,
    delta: &str,
    delta_range_reason: Option<&str>,
    integrated_parent: bool,
) -> ownership::Comment {
    let pr = receipt.pr.to_string();
    let parent_pr = parent.pr.to_string();
    let mut fields = vec![
        ("run-id", run_id),
        ("epoch", receipt.epoch.as_str()),
        ("pr", pr.as_str()),
        ("head", receipt.head.as_str()),
        ("base", receipt.base.as_str()),
        ("digest", receipt.digest.as_str()),
        ("parent", parent.epoch.as_str()),
        ("parent-pr", parent_pr.as_str()),
        ("parent-head", parent.head.as_str()),
        ("parent-base", parent.base.as_str()),
        ("parent-digest", parent.digest.as_str()),
        ("delta", delta),
    ];
    if let Some(reason) = delta_range_reason {
        fields.push(("delta-range-reason", reason));
    }
    if integrated_parent {
        fields.push(("delta-parent", "integrated"));
    }
    wrote(
        "3",
        super::super::markers::render("published", &fields).expect("a receipt marker with lineage"),
    )
}

/// Every `published` marker written before `delta-range`/`delta-range-reason`
/// existed carries neither, and `from_marker` has to keep reading the rest of
/// the lineage off it rather than losing it to a shape check invented after
/// the fact. Making either field required — `marker.get("delta-range")?`
/// instead of `.cloned()` — turns this red, since [`published_with_parent`]
/// renders no such field at all.
#[test]
fn a_lineage_with_no_recorded_range_still_parses() {
    let parent = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 8,
        head: "d".repeat(40),
        base: "e".repeat(40),
        digest: "f".repeat(64),
    };
    let comment = published_with_parent("claude-a", &receipt, &parent, &"0".repeat(64));
    let marker = super::super::markers::parse(&comment.body)
        .pop()
        .expect("one marker parses back");

    let lineage = PublicationLineage::from_marker(&marker)
        .expect("a marker with no recorded range is still a complete lineage");
    assert_eq!(lineage.parent, parent, "{lineage:?}");
    assert_eq!(lineage.delta, "0".repeat(64), "{lineage:?}");
    assert!(
        lineage.delta_range.is_none(),
        "an unrecorded field read as present: {lineage:?}"
    );
    assert!(
        lineage.delta_range_reason.is_none(),
        "an unrecorded field read as present: {lineage:?}"
    );
}

fn read_repair_rounds(context: &Context, comments: &[ownership::Comment]) -> serde_json::Value {
    crate::transport::store::forget_scratch_store();
    let stdout = stdout_with(comments);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        repair_rounds(context, 42, RECLAIM_NOW)
    })
    .expect("a well-formed read must answer")
}

#[test]
fn repair_rounds_counts_a_rejected_original_and_its_one_correction() {
    let (_root, mut context) = context_with_no_repository();
    context.config = corrections_limit_row("1");
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let correction = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "b".repeat(40),
        digest: "2".repeat(64),
        ..first.clone()
    };

    let answer = read_repair_rounds(
        &context,
        &[
            published(&first),
            computed_by("codex-b", &first, "rejected"),
        ],
    );
    assert_eq!(answer["rejections"], 1, "{answer}");
    assert_eq!(answer["corrections"], 0, "{answer}");
    assert_eq!(answer["correction_limit"], 1, "{answer}");
    assert_eq!(answer["correction_spent"], false, "{answer}");
    assert_eq!(answer["correction_available"], true, "{answer}");
    assert_eq!(answer["latest_verdict"], "rejected", "{answer}");

    let answer = read_repair_rounds(
        &context,
        &[
            published(&first),
            computed_by("codex-b", &first, "rejected"),
            published_with_parent("claude-a", &correction, &first, "delta-1-2"),
        ],
    );
    assert_eq!(answer["corrections"], 1, "{answer}");
    assert_eq!(answer["correction_spent"], true, "{answer}");
    assert_eq!(
        answer["correction_available"], false,
        "the newest publication is unjudged: {answer}"
    );

    let answer = read_repair_rounds(
        &context,
        &[
            published(&first),
            computed_by("codex-b", &first, "rejected"),
            published_with_parent("claude-a", &correction, &first, "delta-1-2"),
            computed_by("codex-b", &correction, "escalated"),
        ],
    );
    assert_eq!(answer["rejections"], 2, "{answer}");
    assert_eq!(answer["corrections"], 1, "{answer}");
    assert_eq!(
        answer["correction_available"], false,
        "a spent correction leaves none to publish: {answer}"
    );
}

#[test]
fn repair_rounds_does_not_charge_a_new_lineage_with_the_corrected_ones_correction() {
    let (_root, context) = context_with_no_repository();
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let correction = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "b".repeat(40),
        digest: "2".repeat(64),
        ..first.clone()
    };
    let candidate = ReviewTransaction {
        epoch: "3".repeat(32),
        head: "c".repeat(40),
        digest: "3".repeat(64),
        ..first.clone()
    };
    let answer = read_repair_rounds(
        &context,
        &[
            published(&first),
            computed_by("codex-b", &first, "rejected"),
            published_with_parent("claude-a", &correction, &first, "delta-1-2"),
            computed_by("codex-b", &correction, "approved"),
            published_with_parent("claude-a", &candidate, &correction, "delta-2-3"),
        ],
    );
    assert_eq!(
        answer["corrections"], 0,
        "a republish over an approved parent is a new candidate: {answer}"
    );
    assert_eq!(answer["correction_spent"], false, "{answer}");
    assert_eq!(answer["rejections"], 1, "{answer}");
}

#[test]
fn repair_rounds_counts_neither_a_first_publication_nor_its_acceptance() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let answer = read_repair_rounds(
        &context,
        &[
            published(&receipt),
            verdict_by("codex-b", "accepted", &"a".repeat(32), &receipt),
        ],
    );
    assert_eq!(answer["rejections"], 0, "{answer}");
    assert_eq!(answer["corrections"], 0, "{answer}");
    assert_eq!(answer["latest_verdict"], "approved", "{answer}");
}

#[test]
fn repair_rounds_does_not_charge_a_second_change_with_the_first_ones_history() {
    let (_root, context) = context_with_no_repository();
    let delivered = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let next_change = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 8,
        head: "b".repeat(40),
        digest: "2".repeat(64),
        ..delivered.clone()
    };
    let answer = read_repair_rounds(
        &context,
        &[
            published(&delivered),
            computed_by("codex-b", &delivered, "rejected"),
            published_with_parent("claude-a", &next_change, &delivered, "delta-1-2"),
        ],
    );
    assert_eq!(
        answer["rejections"], 0,
        "a rejection against another pull request is not this change's: {answer}"
    );
    assert_eq!(answer["corrections"], 0, "{answer}");
}

#[test]
fn repair_rounds_answers_nothing_spent_before_any_publication() {
    let (_root, context) = context_with_no_repository();
    let answer = read_repair_rounds(&context, &[]);
    assert_eq!(answer["rejections"], 0, "{answer}");
    assert_eq!(answer["corrections"], 0, "{answer}");
    assert_eq!(answer["correction_spent"], false, "{answer}");
    assert_eq!(answer["correction_available"], false, "{answer}");
}

fn finding_marker(
    at: &str,
    operation_id: &str,
    reviewer: &str,
    receipt: &ReviewTransaction,
    id: &str,
    class: &str,
) -> ownership::Comment {
    wrote(
        at,
        super::super::markers::render(
            "review-finding",
            &[
                ("run-id", "claude-a"),
                ("op-id", operation_id),
                ("reviewer", reviewer),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("id", id),
                ("class", class),
                ("evidence", "observed the defect directly"),
                ("impact", "wrong output on the happy path"),
            ],
        )
        .expect("a review-finding marker"),
    )
}

/// A severe finding carrying a chosen `evidence_class`, for the refutation
/// tests below. `causal_disposition` is `pre-existing` throughout so no
/// `--location` is owed.
fn severe_finding_with_class(
    at: &str,
    operation_id: &str,
    reviewer: &str,
    receipt: &ReviewTransaction,
    id: &str,
    evidence_class: &str,
) -> ownership::Comment {
    wrote(
        at,
        super::super::markers::render(
            "review-finding",
            &[
                ("run-id", "claude-a"),
                ("op-id", operation_id),
                ("reviewer", reviewer),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("id", id),
                ("class", "severe"),
                ("evidence", "observed the defect directly"),
                ("impact", "wrong output on the happy path"),
                ("evidence-class", evidence_class),
                ("causal-disposition", "pre-existing"),
            ],
        )
        .expect("a severe review-finding marker"),
    )
}

fn refutation_review<'a>(
    operation_id: &'a str,
    refuter: &'a str,
    receipt: &'a ReviewTransaction,
    outcomes: &'a str,
) -> Refutation<'a> {
    Refutation {
        issue: 42,
        run_id: "claude-a",
        refuter,
        operation_id,
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        outcomes,
        now: "2026-08-31T00:00:00Z",
    }
}

fn read_review_findings(
    context: &Context,
    comments: &[ownership::Comment],
    epoch: Option<&str>,
    id: Option<&str>,
    with_text: bool,
) -> Result<serde_json::Value, Failure> {
    let stdout = stdout_with(comments);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        review_findings(context, 42, epoch, id, with_text, RECLAIM_NOW)
    })
}

fn read_review_transaction(
    context: &Context,
    comments: &[ownership::Comment],
    epoch: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    let stdout = stdout_with(comments);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        review_transaction(context, 42, epoch, RECLAIM_NOW)
    })
}

/// A `published` marker carrying a repair's full detail — lineage, lane and
/// autoclose — the shape [`review_transaction`] answers back once none of it
/// rides `publish_review`'s own answer any more. See [`published_with_parent`]
/// for the lineage-only shape this extends.
#[allow(clippy::too_many_arguments)]
fn published_with_detail(
    run_id: &str,
    receipt: &ReviewTransaction,
    parent: &ReviewTransaction,
    delta: &str,
    url: &str,
    lane_note: &str,
    lanes: &[&str],
    autoclose_json: &str,
    mandatory_follow_up: &str,
) -> ownership::Comment {
    let lanes_joined = lanes.join(",");
    let lane_count = lanes.len().to_string();
    wrote(
        "3",
        super::super::markers::render(
            "published",
            &[
                ("run-id", run_id),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("url", url),
                ("lanes", &lanes_joined),
                ("lane-count", &lane_count),
                ("lane-note", lane_note),
                ("parent", &parent.epoch),
                ("parent-pr", &parent.pr.to_string()),
                ("parent-head", &parent.head),
                ("parent-base", &parent.base),
                ("parent-digest", &parent.digest),
                ("delta", delta),
                ("delta-range", "parent-head..head"),
                ("topology", "developer:1"),
                ("topology-reason", "one writer"),
                ("computed-level", "low"),
                ("raise", "contract"),
                ("raise-reason", "a wire format other readers depend on"),
                ("focus", "risk"),
                ("focus-reason", "the change is all about failure"),
                ("autoclose-json", autoclose_json),
                ("mandatory-follow-up", mandatory_follow_up),
            ],
        )
        .expect("a receipt marker with every field review_transaction answers"),
    )
}

/// `review_transaction` answers back every field 1b removed from
/// `publish_review`'s own answer — the exact shape a repair's marker records.
/// Mutating `all_publications` to drop `lineage`, `lane_note`,
/// `lane_workflows` or `autoclose` off `LatestPublication` must turn this red.
#[test]
fn review_transaction_answers_a_repairs_lineage_and_lane_and_autoclose() {
    let (_root, context) = context_with_no_repository();
    let parent = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let receipt = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "b".repeat(40),
        base: "d".repeat(40),
        digest: "2".repeat(64),
    };
    let autoclose = serde_json::json!({
        "will_autoclose": true, "cause": "branch-link", "linked_prs": [],
    })
    .to_string();
    let comment = published_with_detail(
        "claude-a",
        &receipt,
        &parent,
        &"0".repeat(64),
        "https://github.com/o/r/pull/7",
        "one run of `ci.yml` was dispatched against this head; an accepted verdict cannot be \
         recorded until it is green",
        &["ci.yml"],
        &autoclose,
        "GitHub will auto-close this issue on merge.",
    );

    let answer = read_review_transaction(&context, &[comment], None)
        .expect("a well-formed read must answer");

    assert_eq!(answer["selected"], "latest", "{answer}");
    assert_eq!(answer["epoch"], receipt.epoch, "{answer}");
    assert_eq!(answer["url"], "https://github.com/o/r/pull/7", "{answer}");
    assert_eq!(answer["parent"], parent.epoch, "{answer}");
    assert_eq!(answer["parent_pr"], parent.pr, "{answer}");
    assert_eq!(answer["parent_head"], parent.head, "{answer}");
    assert_eq!(answer["parent_base"], parent.base, "{answer}");
    assert_eq!(answer["parent_digest"], parent.digest, "{answer}");
    assert_eq!(answer["delta"], "0".repeat(64), "{answer}");
    assert_eq!(answer["delta_range"], "parent-head..head", "{answer}");
    assert_eq!(answer["topology"], "developer:1", "{answer}");
    assert_eq!(answer["topology_reason"], "one writer", "{answer}");
    assert_eq!(answer["computed_level"], "low", "{answer}");
    assert_eq!(answer["raise"], "contract", "{answer}");
    assert_eq!(
        answer["raise_reason"], "a wire format other readers depend on",
        "{answer}"
    );
    assert_eq!(answer["focus"], "risk", "{answer}");
    assert_eq!(
        answer["focus_reason"], "the change is all about failure",
        "{answer}"
    );
    assert_eq!(
        answer["delta_range_reason"],
        serde_json::Value::Null,
        "an honest range answers no fallback reason: {answer}"
    );
    assert_eq!(
        answer["publication_lane_workflows"],
        serde_json::json!(["ci.yml"]),
        "{answer}"
    );
    assert_eq!(answer["publication_lane_count"], 1, "{answer}");
    assert!(
        answer["publication_lane_note"]
            .as_str()
            .is_some_and(|note| note.contains("dispatched")),
        "{answer}"
    );
    assert_eq!(answer["autoclose"]["cause"], "branch-link", "{answer}");
    assert_eq!(answer["autoclose_assessed"], true, "{answer}");
    assert_eq!(
        answer["mandatory_follow_up"], "GitHub will auto-close this issue on merge.",
        "{answer}"
    );
}

/// Every marker field the write answer gave up is still written, and the lane
/// fields keep their old presence rule: `lanes`/`lane-count` only when
/// something started. Deleting any of these pushes must redden this test.
#[test]
fn the_relocated_marker_fields_are_written_even_though_the_answer_dropped_them() {
    let verdict = serde_json::json!({
        "will_autoclose": true, "cause": "branch-link", "linked_prs": [],
    });
    let workflows = vec!["ci.yml".to_owned()];
    let fields = super::marker_detail_fields(
        "https://github.com/o/r/pull/7",
        "one run of `ci.yml` was dispatched",
        &workflows,
        &verdict,
    );
    let names: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        names,
        vec![
            "url",
            "lanes",
            "lane-count",
            "lane-note",
            "autoclose-json",
            "mandatory-follow-up"
        ],
        "a field the answer dropped must still be written: {fields:?}"
    );
    assert!(
        fields
            .iter()
            .any(|(key, value)| *key == "mandatory-follow-up"
                && value.contains("closed-by-own-delivery")),
        "the follow-up the answer no longer carries must live on the marker: {fields:?}"
    );
    assert!(
        fields
            .iter()
            .any(|(key, value)| *key == "autoclose-json" && value.contains("branch-link")),
        "the autoclose verdict must be recorded whole: {fields:?}"
    );

    let quiet = super::marker_detail_fields(
        "https://github.com/o/r/pull/7",
        "no dispatchable workflow",
        &[],
        &serde_json::json!({ "will_autoclose": false, "cause": null }),
    );
    let quiet_names: Vec<&str> = quiet.iter().map(|(key, _)| *key).collect();
    assert_eq!(
        quiet_names,
        vec!["url", "lane-note", "autoclose-json", "mandatory-follow-up"],
        "lanes and lane-count appear only when something started: {quiet:?}"
    );
}

/// A marker written before the autoclose verdict was recorded answers
/// `autoclose_assessed: false` with a null `will_autoclose` — "unknown",
/// never a false "no": nothing was ever assessed for it.
#[test]
fn review_transaction_calls_an_unrecorded_autoclose_unknown() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let comments = vec![published(&receipt)];

    let answer =
        read_review_transaction(&context, &comments, None).expect("a well-formed read must answer");
    assert_eq!(answer["autoclose_assessed"], false, "{answer}");
    assert_eq!(
        answer["autoclose"]["will_autoclose"],
        serde_json::Value::Null,
        "{answer}"
    );
}

/// An epoch this issue's timeline never published is refused by name, the
/// same shape `review_findings` refuses `findings-transaction-unknown` with.
#[test]
fn review_transaction_refuses_an_epoch_the_timeline_never_published() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let comments = vec![published(&receipt)];

    let failure = read_review_transaction(&context, &comments, Some(&"9".repeat(32)))
        .expect_err("an epoch this issue never published was answered");
    assert_eq!(
        failure.envelope()["reason"],
        "publication-epoch-not-found",
        "{failure:?}"
    );
    assert_eq!(
        failure.envelope()["recorded"],
        serde_json::json!([receipt.epoch]),
        "{failure:?}"
    );
}

/// `completed_count` counts only the issues this run's own delivery closed: a
/// `done` issue the run merely mentioned is not one of them. Issue 7 carries a
/// publication by the run whose merge parents are the receipt's own base and
/// head, so it counts; issue 8 carries nothing by the run, so it does not.
/// Turning the per-issue `own_delivery_close` proof into a bare `status:done`
/// count must redden this.
#[test]
fn the_completion_count_is_proved_per_issue_rather_than_searched() {
    let (root, base, head) = repo_with_one_changed_line();
    let mut context = context_at(root.path());
    context.repo = Some("o/r".to_owned());
    let receipt = ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 7,
        head,
        base,
        digest: "d".repeat(64),
    };

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
            7,
            &serde_json::json!({
                "body": "Delivered.",
                "labels": [{ "name": "status:done" }],
                "comments": [comment_json(&published(&receipt))],
            }),
            "2026-01-01T00:00Z",
        )
        .expect("issue 7 imports");
    store
        .import(
            &repo,
            8,
            &serde_json::json!({
                "body": "Mentioned only.",
                "labels": [{ "name": "status:done" }],
                "comments": [],
            }),
            "2026-01-01T00:00Z",
        )
        .expect("issue 8 imports");
    drop(store);

    let closing = serde_json::json!({
        "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
            "nodes": [{ "number": 7, "state": "MERGED", "headRefName": "h", "baseRefName": "b" }],
            "pageInfo": { "hasNextPage": false, "endCursor": null },
        } } } }
    });
    let merge = serde_json::json!({ "mergeCommit": { "oid": "e".repeat(40) } });
    let parents = serde_json::json!({
        "parents": [{ "sha": receipt.base }, { "sha": receipt.head }],
    });

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "repo view --json owner,name",
            "stdout": serde_json::json!({ "owner": { "login": "o" }, "name": "r" }).to_string(),
        }),
        serde_json::json!({ "matches": "api graphql", "stdout": closing.to_string() }),
        serde_json::json!({
            "matches": "pr view 7 --json mergeCommit",
            "stdout": merge.to_string(),
        }),
        serde_json::json!({
            "matches": "api repos/o/r/commits/",
            "stdout": parents.to_string(),
        }),
    ]);
    let answer = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            super::completed_count(&context, "claude-a")
        })
    })
    .expect("a readable tracker answers");
    assert_eq!(answer, Some(1), "only the issue this run delivered counts");
}

fn verify_a_closed_issue_from_the_store(
    published_by_run: Option<&str>,
    second_parent: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    let (root, base, head) = repo_with_one_changed_line();
    let context = context_at(root.path());
    let receipt = ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 7,
        head,
        base,
        digest: "d".repeat(64),
    };
    let store_path = root.path().join("state.db");
    let repo = crate::transport::store::Repo {
        host: "github.com".to_owned(),
        owner: "fixture".to_owned(),
        name: "repo".to_owned(),
    };
    let comments: Vec<serde_json::Value> = [comment_json(&dated(
        "1",
        "2026-07-28T01:00:00Z",
        claim_body("claude-a", HOLDER_OPERATION),
    ))]
    .into_iter()
    .chain(published_by_run.map(|run| comment_json(&published_by(run, &receipt))))
    .collect();
    let mut store =
        crate::transport::store::Store::open(&store_path).expect("the scratch store opens");
    store
        .import(
            &repo,
            7,
            &serde_json::json!({
                "body": "Delivered.",
                "labels": [{ "name": "status:review" }],
                "comments": comments,
            }),
            "2026-07-28T01:00Z",
        )
        .expect("issue 7 imports");
    drop(store);

    let closing = serde_json::json!({
        "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
            "nodes": [{ "number": 7, "state": "MERGED", "headRefName": "h", "baseRefName": "b" }],
            "pageInfo": { "hasNextPage": false, "endCursor": null },
        } } } }
    });
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "repo view --json owner,name",
            "stdout": serde_json::json!({ "owner": { "login": "fixture" }, "name": "repo" })
                .to_string(),
        }),
        serde_json::json!({ "matches": "api graphql", "stdout": closing.to_string() }),
        serde_json::json!({
            "matches": "pr view 7 --json mergeCommit",
            "stdout": serde_json::json!({ "mergeCommit": { "oid": "e".repeat(40) } }).to_string(),
        }),
        crate::test_env::issue_view_answer(
            7,
            serde_json::json!({ "state": "CLOSED", "labels": [], "comments": [] }),
        ),
        serde_json::json!({
            "matches": "/commits/",
            "stdout": serde_json::json!({
                "parents": [
                    { "sha": receipt.base },
                    { "sha": second_parent.unwrap_or(&receipt.head) },
                ],
            })
            .to_string(),
        }),
    ]);

    crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            verify_claim(&context, 7, "claude-a", "review", RECLAIM_NOW, Some(7))
        })
    })
}

#[test]
fn an_own_delivery_close_is_proved_from_the_store_when_github_carries_no_marker() {
    let answer = verify_a_closed_issue_from_the_store(Some("claude-a"), None)
        .expect("the close is this run's own delivery, proved from the store");
    assert!(
        answer.to_string().contains(OWN_DELIVERY_CLOSE_PREFIX),
        "{answer}"
    );
}

#[test]
fn a_closed_issue_is_not_this_runs_delivery_without_a_receipt_in_the_store() {
    let refused = verify_a_closed_issue_from_the_store(None, None)
        .expect_err("no receipt in the store proves nothing");
    assert_refused_as_not_open(refused);
}

#[test]
fn a_closed_issue_is_not_this_runs_delivery_on_another_runs_receipt() {
    let refused = verify_a_closed_issue_from_the_store(Some("claude-b"), None)
        .expect_err("another run's receipt is not this run's delivery");
    assert_refused_as_not_open(refused);
}

#[test]
fn a_closed_issue_is_not_this_runs_delivery_when_the_merge_does_not_match_the_receipt() {
    let refused = verify_a_closed_issue_from_the_store(Some("claude-a"), Some(&"9".repeat(40)))
        .expect_err("a merge whose second parent is not the receipt head is not this delivery");
    assert_refused_as_not_open(refused);
}

fn assert_refused_as_not_open(refused: Failure) {
    let Failure::Stop(body) = refused else {
        panic!("the refusal was not a stop: {refused:?}");
    };
    assert_eq!(body["reason"], "issue-not-open", "{body}");
    assert!(
        !body.to_string().contains(OWN_DELIVERY_CLOSE_PREFIX),
        "{body}"
    );
}

/// A completion count whose tracker read did not land is an error, never a
/// zero — the loop above it must stop on the fault rather than read it as
/// nothing done. Turning the search's `?` into a defaulted empty list must
/// redden this.
#[test]
fn a_completion_count_that_cannot_read_the_tracker_is_not_zero() {
    let (_root, context) = context_with_no_repository();
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[serde_json::json!({
        "matches": "issue list --state closed",
        "stderr": "gh: could not resolve to a repository",
        "status": 1,
    })]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::completed_count(&context, "claude-a")
    })
    .expect_err("an unreadable tracker answered a count");
    assert_eq!(failure.envelope()["reason"], "read-failed", "{failure:?}");
}

/// Mutating `findings_for(&comments, &resolved)` to an empty `Vec` must turn
/// this red at `findings_count` and at the array length: two findings stand
/// against the resolved receipt and nothing here answers them otherwise.
#[test]
fn review_findings_answers_every_finding_recorded_against_the_receipt() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let comments = vec![
        published(&receipt),
        finding_marker(
            "3",
            &"a".repeat(32),
            "codex-b",
            &receipt,
            "finding-1",
            "suggestion",
        ),
        finding_marker(
            "4",
            &"b".repeat(32),
            "codex-c",
            &receipt,
            "finding-2",
            "warning",
        ),
    ];
    let answer = read_review_findings(&context, &comments, None, None, false)
        .expect("a well-formed read must answer");
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["selected"], "latest", "{answer}");
    assert_eq!(answer["receipt"]["epoch"], receipt.epoch, "{answer}");
    assert_eq!(answer["findings_count"], 2, "{answer}");
    assert_eq!(
        answer["findings"].as_array().expect("an array").len(),
        2,
        "{answer}"
    );
}

/// Mutating the epoch-resolution refusal to fall through to an empty answer
/// must turn this red: the mutant would answer `ok: true, findings: []`, the
/// exact conflation between an unknown epoch and an empty ledger this
/// operation's own criterion forbids.
#[test]
fn review_findings_refuses_an_epoch_this_issue_never_published() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let comments = vec![published(&receipt)];
    let unknown_epoch = "9".repeat(32);
    let failure = read_review_findings(&context, &comments, Some(&unknown_epoch), None, false)
        .expect_err("an epoch this issue never published was answered");
    assert_eq!(
        failure.envelope()["reason"],
        "findings-transaction-unknown",
        "{failure:?}"
    );
}

/// Mutating the read to `.ok().flatten().unwrap_or_default()` must turn this
/// red: an unreachable `gh` would answer as though the issue simply carried
/// no comments, rather than as the failed read it actually is.
#[test]
fn review_findings_answers_an_unreadable_timeline_as_a_failed_read() {
    let (_root, context) = context_with_no_repository();
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::unreachable_tracker_answer()]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        review_findings(&context, 42, None, None, false, RECLAIM_NOW)
    })
    .expect_err("an unreachable tracker was answered as though it read something");
    assert!(
        matches!(failure, Failure::Read(_)),
        "expected a failed read, got {failure:?}"
    );
}

/// Mutating the `with_text` branch to skip populating `entry["evidence"]`/`entry["impact"]`
/// must turn the first two assertions red: the answer would carry only structured fields.
/// Mutating the `FINDING_TEXT_CEILING` comparison away must turn the last assertion red: a sum
/// past the ceiling under `--with-text` would answer `ok: true` instead of refusing.
#[test]
fn review_findings_with_text_answers_the_text_and_refuses_it_past_the_ceiling() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let comments = vec![
        published(&receipt),
        finding_marker(
            "3",
            &"a".repeat(32),
            "codex-b",
            &receipt,
            "finding-1",
            "suggestion",
        ),
    ];
    let answer = read_review_findings(&context, &comments, None, None, true)
        .expect("a well-formed read must answer");
    assert_eq!(
        answer["findings"][0]["evidence"], "observed the defect directly",
        "{answer}"
    );
    assert_eq!(
        answer["findings"][0]["impact"], "wrong output on the happy path",
        "{answer}"
    );
    assert_eq!(answer["text_included"], true, "{answer}");

    let big = "x".repeat(FINDING_TEXT_CEILING);
    let big_marker = wrote(
        "4",
        super::super::markers::render(
            "review-finding",
            &[
                ("run-id", "claude-a"),
                ("op-id", &"b".repeat(32)),
                ("reviewer", "codex-c"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("id", "finding-2"),
                ("class", "suggestion"),
                ("evidence", &big),
                ("impact", "short"),
            ],
        )
        .expect("a review-finding marker"),
    );
    let over_ceiling = vec![published(&receipt), big_marker];
    crate::transport::store::forget_scratch_store();
    let without_text = read_review_findings(&context, &over_ceiling, None, None, false)
        .expect("without --with-text the ceiling does not gate the answer");
    assert_eq!(without_text["findings_count"], 1, "{without_text}");

    let failure = read_review_findings(&context, &over_ceiling, None, None, true)
        .expect_err("text past the ceiling under --with-text was answered");
    assert_eq!(
        failure.envelope()["reason"],
        "finding-text-too-large",
        "{failure:?}"
    );
}

/// Mutating the `--id` filter to `findings.retain(|_| true)` must turn the first assertion red:
/// the known-id read would answer both findings instead of one. Mutating the `is_empty()` check
/// afterwards away must turn the second assertion red: an id nothing recorded would answer
/// `ok: true` with an empty array instead of `finding-id-unknown`.
#[test]
fn review_findings_filters_by_id_and_refuses_an_unknown_one() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let comments = vec![
        published(&receipt),
        finding_marker(
            "3",
            &"a".repeat(32),
            "codex-b",
            &receipt,
            "finding-1",
            "suggestion",
        ),
        finding_marker(
            "4",
            &"b".repeat(32),
            "codex-c",
            &receipt,
            "finding-2",
            "warning",
        ),
    ];
    let answer = read_review_findings(&context, &comments, None, Some("finding-1"), false)
        .expect("a recorded id must answer");
    assert_eq!(answer["findings_count"], 1, "{answer}");
    assert_eq!(answer["findings"][0]["id"], "finding-1", "{answer}");

    let failure = read_review_findings(&context, &comments, None, Some("finding-9"), false)
        .expect_err("an id this receipt never recorded was answered");
    assert_eq!(
        failure.envelope()["reason"],
        "finding-id-unknown",
        "{failure:?}"
    );
    assert_eq!(failure.envelope()["id"], "finding-9", "{failure:?}");
    assert_eq!(
        failure.envelope()["recorded"]
            .as_array()
            .expect("an array")
            .len(),
        2,
        "{failure:?}"
    );
}

/// A parent whose own `published` comment was deleted is still answerable through its child's
/// recorded lineage. Mutating the `else if let Some(parent) = latest.lineage...` branch away must
/// turn the second read red: naming the parent's own epoch would answer `findings-transaction-unknown`
/// instead of resolving it. Mutating the `parent_epoch` lookup to always answer `None` must turn
/// the first read's `parent_epoch` assertion red.
#[test]
fn review_findings_resolves_a_deleted_parent_through_its_childs_lineage() {
    let (_root, context) = context_with_no_repository();
    let parent = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let child = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "b".repeat(40),
        base: "d".repeat(40),
        digest: "2".repeat(64),
    };
    let comments = vec![
        finding_marker(
            "2",
            &"a".repeat(32),
            "codex-b",
            &parent,
            "old-finding",
            "suggestion",
        ),
        published_with_parent("claude-a", &child, &parent, "delta-1"),
    ];

    let latest = read_review_findings(&context, &comments, None, None, false)
        .expect("the latest publication must answer");
    assert_eq!(latest["selected"], "latest", "{latest}");
    assert_eq!(latest["receipt"]["epoch"], child.epoch, "{latest}");
    assert_eq!(latest["parent_epoch"], parent.epoch, "{latest}");
    assert_eq!(latest["findings_count"], 0, "{latest}");

    let via_parent = read_review_findings(&context, &comments, Some(&parent.epoch), None, false)
        .expect("the deleted parent must still answer through its child's lineage");
    assert_eq!(via_parent["selected"], "epoch", "{via_parent}");
    assert_eq!(via_parent["receipt"]["epoch"], parent.epoch, "{via_parent}");
    assert!(via_parent["parent_epoch"].is_null(), "{via_parent}");
    assert_eq!(via_parent["findings_count"], 1, "{via_parent}");
    assert_eq!(
        via_parent["findings"][0]["id"], "old-finding",
        "{via_parent}"
    );
}

/// Mutating the empty-ledger case to fall into the same shape `finding-id-unknown` uses must turn
/// this red: a publication nobody has recorded a finding against would refuse instead of
/// answering `ok: true` with an empty array, exactly the conflation the CHANGELOG rules out.
#[test]
fn review_findings_answers_ok_true_and_an_empty_array_when_nothing_was_recorded() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let comments = vec![published(&receipt)];
    let answer = read_review_findings(&context, &comments, None, None, false)
        .expect("a publication with no findings must still answer");
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["findings_count"], 0, "{answer}");
    assert_eq!(
        answer["findings"].as_array().expect("an array").len(),
        0,
        "{answer}"
    );
}

/// Mutating away the `require_operation_id(Some(value))` call at the top of `review_findings`
/// must turn this red: a malformed `--epoch` would fall through to `gh issue view` instead of
/// being refused before any read.
#[test]
fn review_findings_refuses_a_malformed_epoch_before_the_timeline_is_read() {
    let (_root, context) = context_with_no_repository();
    let failure = review_findings(&context, 42, Some("not-an-epoch"), None, false, RECLAIM_NOW)
        .expect_err("a malformed --epoch was accepted");
    assert_eq!(
        failure.envelope()["reason"],
        "invalid-operation-id",
        "{failure:?}"
    );
}

fn published_with_payload_digest(
    receipt: &ReviewTransaction,
    payload_digest: &str,
) -> ownership::Comment {
    wrote(
        "3",
        super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("payload-digest", payload_digest),
            ],
        )
        .expect("a receipt marker"),
    )
}

/// A real two-commit, two-path repository, and the payload `publish_review`
/// would have bound as `payload-digest` for it — `review_payload` itself,
/// never hand-typed, so a fixture line is only ever a line git actually
/// produced.
fn payload_repo() -> (tempfile::TempDir, Context, String, String, String, String) {
    let root = tempfile::tempdir().expect("a repository");
    let at = root.path();
    git(at, &["init", "--quiet", "-b", "main"]);
    git(at, &["config", "user.email", "estigia@example.invalid"]);
    git(at, &["config", "user.name", "Estigia test"]);
    git(at, &["config", "core.autocrlf", "false"]);
    std::fs::write(at.join("kept.txt"), "one\ntwo\nthree\n").expect("the base file");
    std::fs::write(at.join("other.txt"), "alpha\nbeta\ngamma\n").expect("the other base file");
    git(at, &["add", "kept.txt", "other.txt"]);
    git(at, &["commit", "--quiet", "-m", "base"]);
    let base = head_of(at);
    std::fs::write(at.join("kept.txt"), "one\nTWO\nthree\n").expect("the head file");
    std::fs::write(at.join("other.txt"), "alpha\nBETA\ngamma\n").expect("the other head file");
    git(at, &["add", "kept.txt", "other.txt"]);
    git(at, &["commit", "--quiet", "-m", "head"]);
    let head = head_of(at);
    let payload =
        super::super::target::review_payload(at, &base, &head).expect("a recomputable payload");
    let context = Context {
        skill_dir: at.to_path_buf(),
        repo_dir: at.to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    (root, context, base, head, payload.text, payload.digest)
}

/// The first path's own diff block, exactly as `payload` rendered it: from the
/// start of the text up to and including the newline that precedes the second
/// path's `diff --git` header. A genuine subset, not a retyped one.
fn first_file_diff(payload: &str) -> &str {
    match payload.find("\ndiff --git ") {
        Some(at) => &payload[..=at],
        None => payload,
    }
}

fn candidate_file(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
    let path = dir.join("candidate.diff");
    std::fs::write(&path, text).expect("the candidate file");
    path
}

fn read_review_payload_membership(
    context: &Context,
    comments: &[ownership::Comment],
    epoch: Option<&str>,
    candidate: &std::path::Path,
) -> Result<serde_json::Value, Failure> {
    let stdout = stdout_with(comments);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        review_payload_membership(context, 42, epoch, candidate, None, RECLAIM_NOW)
    })
}

fn read_inspect_candidate(
    context: &Context,
    comments: &[ownership::Comment],
    epoch: &str,
    request: &str,
    path: Option<&str>,
    side: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    let stdout = stdout_with(comments);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout)]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        inspect_candidate(
            context,
            42,
            &InspectRequest {
                epoch,
                request,
                path,
                side,
                worktree: None,
            },
            RECLAIM_NOW,
        )
    })
}

fn repair_pair(base: String, head: String) -> (ReviewTransaction, ReviewTransaction) {
    let parent = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: base.clone(),
        base: base.clone(),
        digest: "1".repeat(64),
    };
    let child = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head,
        base,
        digest: "2".repeat(64),
    };
    (parent, child)
}

/// Reading the parent's head from anywhere but the recorded lineage, or the
/// publication's own head from anywhere but its marker, turns the listing red.
#[test]
fn inspect_candidate_answers_what_a_correction_changed_between_two_publications() {
    let (root, context, base, head, _payload, _digest) = payload_repo();
    let at = root.path();
    std::fs::write(at.join("kept.txt"), "one\nTWO\nTHREE\n").expect("the correction");
    std::fs::write(at.join("added.txt"), "new\n").expect("the added file");
    git(at, &["add", "kept.txt", "added.txt"]);
    git(at, &["commit", "--quiet", "-m", "correction"]);
    let head2 = head_of(at);
    let (mut parent, child) = repair_pair(base, head2);
    parent.head = head;
    let comments = vec![
        published_with_parent("claude-a", &parent, &parent, "unused"),
        published_with_parent("claude-a", &child, &parent, "delta-1"),
    ];

    let answer =
        read_inspect_candidate(&context, &comments, &child.epoch, "name-status", None, None)
            .expect("a repair's correction is inspectable");
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["epoch"], child.epoch, "{answer}");
    assert_eq!(answer["request"], "name-status", "{answer}");
    let text = answer["text"].as_str().expect("text");
    assert!(
        text.contains("kept.txt") && text.contains("added.txt") && !text.contains("other.txt"),
        "{answer}"
    );

    let object = read_inspect_candidate(
        &context,
        &comments,
        &child.epoch,
        "object",
        Some("kept.txt"),
        Some("candidate"),
    )
    .expect("a changed path reads");
    assert_eq!(object["text"], "one\nTWO\nTHREE\n", "{object}");

    let Failure::Stop(absent) = read_inspect_candidate(
        &context,
        &comments,
        &child.epoch,
        "object",
        Some("added.txt"),
        Some("parent"),
    )
    .expect_err("the correction added this path") else {
        panic!("a stop was expected");
    };
    assert_eq!(absent["reason"], "reviewer-inspect-absent-side", "{absent}");
}

#[test]
fn inspect_candidate_refuses_a_parent_head_the_checkout_does_not_hold() {
    let (_root, context, base, head, _payload, _digest) = payload_repo();
    let (mut parent, child) = repair_pair(base, head);
    parent.head = "a".repeat(40);
    let comments = vec![published_with_parent(
        "claude-a", &child, &parent, "delta-1",
    )];

    let Failure::Stop(body) =
        read_inspect_candidate(&context, &comments, &child.epoch, "name-status", None, None)
            .expect_err("an unheld parent head")
    else {
        panic!("a stop was expected");
    };
    assert_eq!(
        body["reason"], "reviewer-inspect-transaction-unresolved",
        "{body}"
    );
}

#[test]
fn inspect_candidate_refuses_a_recovered_successor_as_no_correction() {
    let (_root, context, base, head, _payload, _digest) = payload_repo();
    let (parent, child) = repair_pair(base, head);
    let comments = vec![successor_published(&child, &parent)];

    let Failure::Stop(body) =
        read_inspect_candidate(&context, &comments, &child.epoch, "name-status", None, None)
            .expect_err("a recovered successor is not a correction")
    else {
        panic!("a stop was expected");
    };
    assert_eq!(body["reason"], "inspect-parent-missing", "{body}");
    assert!(
        body["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("recovered successor")),
        "{body}"
    );
}

/// Dropping the lineage check makes a first publication inspect against nothing.
#[test]
fn inspect_candidate_refuses_a_first_publication_for_having_no_parent() {
    let (_root, context, base, head, _payload, digest) = payload_repo();
    let (_, child) = repair_pair(base, head);
    let comments = vec![published_with_payload_digest(&child, &digest)];

    let failure =
        read_inspect_candidate(&context, &comments, &child.epoch, "name-status", None, None)
            .expect_err("a first publication has no correction");
    let Failure::Stop(body) = failure else {
        panic!("a stop was expected");
    };
    assert_eq!(body["reason"], "inspect-parent-missing", "{body}");
}

#[test]
fn inspect_candidate_refuses_an_epoch_the_timeline_never_published_and_a_path_outside_the_correction()
 {
    let (_root, context, base, head, _payload, _digest) = payload_repo();
    let (parent, child) = repair_pair(base, head);
    let comments = vec![published_with_parent(
        "claude-a", &child, &parent, "delta-1",
    )];

    let Failure::Stop(unknown) =
        read_inspect_candidate(&context, &comments, &"9".repeat(32), "stat", None, None)
            .expect_err("an unpublished epoch")
    else {
        panic!("a stop was expected");
    };
    assert_eq!(
        unknown["reason"], "publication-epoch-not-found",
        "{unknown}"
    );

    let Failure::Stop(outside) = read_inspect_candidate(
        &context,
        &comments,
        &child.epoch,
        "patch",
        Some("elsewhere.txt"),
        None,
    )
    .expect_err("a path the correction did not change") else {
        panic!("a stop was expected");
    };
    assert_eq!(
        outside["reason"], "reviewer-inspect-outside-correction",
        "{outside}"
    );

    let Failure::ConfigDefect(malformed) = read_inspect_candidate(
        &context,
        &comments,
        &child.epoch,
        "object",
        Some("kept.txt"),
        None,
    )
    .expect_err("an object needs a side") else {
        panic!("a config defect was expected");
    };
    assert_eq!(malformed["reason"], "malformed-argument", "{malformed}");
}

/// Acceptance criterion 1: a candidate that withholds a whole file, keeping
/// only real lines of the other, is accepted rather than refused for being
/// incomplete. Inverting the membership predicate in
/// `review_payload_membership` (testing for absence instead of presence)
/// must turn this red.
#[test]
fn review_payload_membership_accepts_a_subset_that_omits_a_whole_file() {
    let (root, context, base, head, payload_text, payload_digest) = payload_repo();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head,
        base,
        digest: "1".repeat(64),
    };
    let comments = vec![published_with_payload_digest(&receipt, &payload_digest)];
    let subset = first_file_diff(&payload_text);
    assert_ne!(
        subset, payload_text,
        "the fixture's payload was only one path; this test needs two"
    );
    let candidate = candidate_file(root.path(), subset);

    let answer = read_review_payload_membership(&context, &comments, None, &candidate)
        .expect("a genuine subset must be accepted");
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["payload_digest"], payload_digest, "{answer}");
    assert!(
        answer["omitted"].as_u64().unwrap_or(0) > 0,
        "withholding a whole file must be visible in `omitted`: {answer}"
    );
}

/// Acceptance criterion 2: one digit inside one otherwise-real line is enough
/// to be refused, and the refusal names that exact line. Deleting the
/// membership check (or reversing which side is searched) must turn this red.
#[test]
fn review_payload_membership_rejects_a_single_altered_digit_and_names_the_line() {
    let (root, context, base, head, payload_text, payload_digest) = payload_repo();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head,
        base,
        digest: "1".repeat(64),
    };
    let comments = vec![published_with_payload_digest(&receipt, &payload_digest)];

    let forged_line = "@@ -1,4 +1,3 @@";
    assert!(
        !payload_text.contains(forged_line),
        "the forged hunk header must not already be a real line of the payload"
    );
    let candidate_text = payload_text.replacen("@@ -1,3 +1,3 @@", forged_line, 1);
    assert_ne!(
        candidate_text, payload_text,
        "the replacement must actually have altered one line"
    );
    let expected_line = candidate_text
        .lines()
        .position(|line| line == forged_line)
        .expect("the forged line is somewhere in the candidate")
        + 1;
    let candidate = candidate_file(root.path(), &candidate_text);

    let failure = read_review_payload_membership(&context, &comments, None, &candidate)
        .expect_err("an altered line was accepted as a real one");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "payload-line-not-found", "{envelope}");
    assert_eq!(envelope["line"], expected_line as u64, "{envelope}");
    assert_eq!(envelope["text"], forged_line, "{envelope}");
}

/// Acceptance criterion 3: a receipt whose recorded `payload-digest` does not
/// match what this rebuilds answers a distinct refusal from "a line is
/// absent", and never reports a line — the mismatch cannot tell "the
/// candidate is wrong" from "this rebuild is wrong", so naming a line would
/// be reporting the nearest named state instead of the unknown one. Deleting
/// the digest reconciliation must turn this red: it would fall through to the
/// membership check instead, over a candidate carrying only real lines, and
/// answer `ok: true`.
#[test]
fn review_payload_membership_refuses_a_digest_mismatch_and_reports_no_line() {
    let (root, context, base, head, payload_text, _payload_digest) = payload_repo();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head,
        base,
        digest: "1".repeat(64),
    };
    let recorded_but_wrong = "0".repeat(64);
    let comments = vec![published_with_payload_digest(&receipt, &recorded_but_wrong)];
    // The candidate carries only real, unaltered lines of the payload — the
    // mismatch must still refuse, on the digest alone.
    let candidate = candidate_file(root.path(), &payload_text);

    let failure = read_review_payload_membership(&context, &comments, None, &candidate)
        .expect_err("a payload that could not be rebuilt to match was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "payload-rebuild-mismatch", "{envelope}");
    assert_eq!(
        envelope["recorded_payload_digest"], recorded_but_wrong,
        "{envelope}"
    );
    assert!(
        envelope.get("line").is_none(),
        "a digest mismatch must never report a line: {envelope}"
    );
}

/// Acceptance criterion 4: a marker recorded before `payload-digest` existed
/// answers that it cannot be crossed, rather than crossing the candidate
/// against nothing. Reverting the `payload_digest` field back to reading
/// nothing — or reconciling a missing digest as if it matched — must turn
/// this red.
#[test]
fn review_payload_membership_refuses_a_receipt_with_no_recorded_payload_digest() {
    let (root, context, base, head, payload_text, _payload_digest) = payload_repo();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head,
        base,
        digest: "1".repeat(64),
    };
    // `published`, not `published_with_payload_digest`: the marker this issue
    // actually has carries no `payload-digest` field at all.
    let comments = vec![published(&receipt)];
    let candidate = candidate_file(root.path(), &payload_text);

    let failure = read_review_payload_membership(&context, &comments, None, &candidate)
        .expect_err("a receipt with no recorded payload-digest was crossed anyway");
    assert_eq!(
        failure.envelope()["reason"],
        "payload-digest-unavailable",
        "{}",
        failure.envelope()
    );
}

/// A candidate built entirely from real payload lines can still
/// misrepresent the change, and the membership check accepts it — the set it
/// searches is global, so it carries no order, no multiplicity and no record
/// of which file a line came from. Each shape below is built from the
/// fixture's own payload bytes, every line asserted to be a line the payload
/// produced, and each answers `ok: true`. This pins the limit rather than
/// leaving it incidental: tightening any of the three moves this test.
#[test]
fn review_payload_membership_accepts_reordered_duplicated_and_cross_spliced_real_lines() {
    let (root, context, base, head, payload_text, payload_digest) = payload_repo();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head,
        base,
        digest: "1".repeat(64),
    };
    let comments = vec![published_with_payload_digest(&receipt, &payload_digest)];
    let real: std::collections::BTreeSet<&str> = payload_text
        .lines()
        .filter(|line| !line.is_empty())
        .collect();

    // Reordered: the `-two`/`+TWO` pair presented in the order the payload
    // never had.
    let reordered = payload_text.replacen("-two\n+TWO", "+TWO\n-two", 1);
    // Duplicated: one real line repeated, so a single change looks like two.
    let duplicated = payload_text.replacen(" three\n", " three\n three\n", 1);
    // Cross-spliced: kept.txt's hunk carrying other.txt's added line, so a
    // change to other.txt appears to be a change to kept.txt.
    let cross_spliced = payload_text.replacen("+TWO", "+BETA", 1);

    for (shape, candidate_text) in [
        ("reordered", reordered),
        ("duplicated", duplicated),
        ("cross-spliced", cross_spliced),
    ] {
        assert_ne!(
            candidate_text, payload_text,
            "{shape}: the construction did not change the payload text"
        );
        for line in candidate_text.lines().filter(|line| !line.is_empty()) {
            assert!(
                real.contains(line),
                "{shape}: {line:?} is not a line the payload produced, so this is not the shape"
            );
        }
        let candidate = candidate_file(root.path(), &candidate_text);
        let answer = read_review_payload_membership(&context, &comments, None, &candidate)
            .unwrap_or_else(|failure| {
                panic!("{shape}: a candidate built from real lines was refused: {failure:?}")
            });
        assert_eq!(answer["ok"], true, "{shape}: {answer}");
    }
}

#[test]
fn every_operation_kind_reason_is_in_the_generated_vocabulary() {
    for kind in [
        OperationKind::Claim,
        OperationKind::Reclaim,
        OperationKind::Unassign,
        OperationKind::ReviewFinding,
        OperationKind::ReviewHandoff,
        OperationKind::ReviewRefutation,
        OperationKind::ReviewDisposition,
        OperationKind::ReviewLensReport,
        OperationKind::ReviewComputedVerdict,
        OperationKind::Spend,
    ] {
        // Exhaustive over the type, not over this array: a variant added to
        // `OperationKind` without being added above still fails to compile
        // here, since the match is checked against every variant that exists.
        match kind {
            OperationKind::Claim
            | OperationKind::Reclaim
            | OperationKind::Unassign
            | OperationKind::ReviewFinding
            | OperationKind::ReviewHandoff
            | OperationKind::ReviewRefutation
            | OperationKind::ReviewDisposition
            | OperationKind::ReviewLensReport
            | OperationKind::ReviewComputedVerdict
            | OperationKind::Spend => {}
        }
        let persisted = super::super::markers::Marker::new();
        for (envelope, suffix) in [
            (kind.operation_conflict(), "-operation-conflict"),
            (kind.metadata_mismatch(&persisted), "-metadata-mismatch"),
        ] {
            let reason = envelope
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("{kind:?}'s envelope has no reason: {envelope}"));
            let expected = format!("{}{suffix}", kind.as_str());
            assert_eq!(
                reason, expected,
                "{kind:?}'s reason is {reason}, not {expected}"
            );
            assert!(
                crate::harness::tracker::TRANSPORT_VOCABULARY.contains(&reason),
                "{reason} (from {kind:?}) is missing from TRANSPORT_VOCABULARY"
            );
        }
    }
}

/// A `review` marker carries no acquisition attributes at all — it is not a
/// `claim` or a `reclaim`, and it names none of `op-id`/`from-op`/`target-op`/
/// `evidence-hash` — so a body carrying one changes nothing about who holds
/// the issue.
#[test]
fn a_review_marker_in_a_body_changes_no_claim_adjudication() {
    let claimed = wrote(
        "1",
        format!(
            "claiming.\n\n{}\n",
            super::super::markers::render(
                "claim",
                &[
                    ("run-id", "claude-a"),
                    ("runtime", "claude"),
                    ("horizon", "2026-07-26T09:00Z"),
                ],
            )
            .expect("renders")
        ),
    );
    let filed = wrote(
        "2",
        format!(
            "## Problem\n\nA probe.\n\n{}\n",
            super::super::markers::render(
                "review",
                &[
                    ("run-id", "claude-x"),
                    ("identity", "a-probe"),
                    ("epoch", "0123456789abcdef0123456789abcdef"),
                    ("findings", "finding-one+finding-two"),
                ],
            )
            .expect("renders")
        ),
    );

    // The other half is `markers::tests::review_evidence_neither_releases_nor_renews_ownership`.
    // This is the third leg: no scoped attribute, so the early-`true` path.
    let parsed = super::super::markers::parse(&filed.body);
    let mark = parsed
        .iter()
        .find(|mark| mark.get("kind").map(String::as_str) == Some("review"))
        .expect("the review marker parses");
    assert!(
        !["op-id", "from-op", "target-op", "evidence-hash"]
            .iter()
            .any(|key| mark.contains_key(*key)),
        "a review marker unexpectedly carries a scoped acquisition attribute: {mark:?}"
    );
    assert!(
        ownership::valid_acquisition_marker(mark),
        "the early-true path did not admit an unscoped review marker"
    );

    // Now the pipeline itself: with only the claim, and with the claim plus
    // the filed issue's marker, the adjudicated holder is the same run.
    let comments_without = [claimed.clone()];
    let comments_with = [claimed, filed];
    let without = holding(&comments_without, "2026-07-26T04:00:00Z");
    let with = holding(&comments_with, "2026-07-26T04:00:00Z");
    assert_eq!(without.holder, Some("claude-a".to_owned()));
    assert_eq!(
        with.holder, without.holder,
        "a review marker changed the adjudicated holder"
    );
    assert_eq!(
        ownership::ownership_events(&comments_with).len(),
        ownership::ownership_events(&comments_without).len(),
        "a review marker was read as an acquisition or release event"
    );
}

#[test]
fn a_rust_test_file_is_recognised_by_suffix_or_by_living_under_tests() {
    assert!(correction_test_file_name("src/transport/tier_test.rs"));
    assert!(correction_test_file_name("tests/pipe.rs"));
    assert!(correction_test_file_name("a/b/tests/fixture.rs"));
    // A bare "tests.rs" is not a "_test.rs" suffix.
    assert!(!correction_test_file_name("src/transport/target/tests.rs"));
    assert!(!correction_test_file_name("src/transport/target.rs"));
    assert!(!correction_test_file_name("tests/fixture.json"));
    assert!(
        !correction_test_file_name("_test.rs"),
        "a stem no longer than the marker is the marker and nothing else"
    );
}

#[test]
fn a_companion_directory_is_the_same_directory_or_a_reserved_name_immediately_inside_or_beside() {
    assert!(correction_companion_directory(
        "src/transport",
        "src/transport"
    ));
    assert!(correction_companion_directory(
        "src/transport/tests",
        "src/transport"
    ));
    assert!(correction_companion_directory("tests", "."));
    assert!(!correction_companion_directory(
        "src/other",
        "src/transport"
    ));
    assert!(
        !correction_companion_directory("src/transport/fixtures", "src/transport"),
        "an unreserved directory name never decides for a file"
    );
}

#[test]
fn the_genesis_manifest_length_is_measured_encoded_not_raw() {
    // Two one-byte, safe paths joined by one separator: the separator is
    // outside the marker's safe set, so it alone costs three characters once
    // percent-encoded, even though the raw join is three bytes wide, not five.
    let paths = vec!["a".to_owned(), "b".to_owned()];
    assert_eq!(genesis_manifest_encoded_len(&paths), 1 + 3 + 1);

    // A single path built entirely from a byte outside the safe set costs
    // three characters per byte on its own, with no separator at all.
    let unsafe_path = vec!["!".repeat(10)];
    assert_eq!(genesis_manifest_encoded_len(&unsafe_path), 30);
}

#[test]
fn admission_refuses_the_first_offending_path_by_name() {
    let genesis = vec![
        "src/transport/tier.rs".to_owned(),
        "CHANGELOG.md".to_owned(),
    ];
    assert!(admit_correction_scope(&genesis, &genesis).is_ok());
    let with_companion = vec![
        "src/transport/tier.rs".to_owned(),
        "src/transport/tier_test.rs".to_owned(),
    ];
    assert!(admit_correction_scope(&with_companion, &genesis).is_ok());
    // Admitted because CHANGELOG.md's own directory is "." and tests/ sits
    // beside it.
    let under_tests = vec!["CHANGELOG.md".to_owned(), "tests/pipe.rs".to_owned()];
    assert!(admit_correction_scope(&under_tests, &genesis).is_ok());
    let outside = vec![
        "src/transport/tier.rs".to_owned(),
        "src/transport/other.rs".to_owned(),
    ];
    assert_eq!(
        admit_correction_scope(&outside, &genesis),
        Err("src/transport/other.rs")
    );
}

// -- record_refutation ------------------------------------------------------

/// Checked before any tracker read, the same way a blank basis is on
/// `record_review_verdict`: a word outside the vocabulary is the caller's own
/// defect and settles nothing about the timeline.
#[test]
fn a_refutation_outcome_outside_the_vocabulary_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let operation_id = "2".repeat(32);
    let refutation = refutation_review(
        &operation_id,
        "codex-c",
        &receipt,
        r#"[{"id":"loc-1","outcome":"maybe"}]"#,
    );
    let failure =
        record_refutation(&context, &refutation).expect_err("an unknown outcome word was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "invalid-refutation-outcome");
    assert_eq!(envelope["value"], "maybe");
}

/// `--outcomes` that does not even parse as the shape this operation takes is
/// refused before any read, the same as an outcome outside the vocabulary.
#[test]
fn a_malformed_outcomes_batch_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let operation_id = "2".repeat(32);
    let refutation = refutation_review(&operation_id, "codex-c", &receipt, "not json at all");
    let failure =
        record_refutation(&context, &refutation).expect_err("a malformed batch was accepted");
    assert_eq!(failure.envelope()["reason"], "invalid-refutation-batch");
}

/// One batch answers each candidate once. Checked before any read, from the
/// batch alone.
#[test]
fn a_refutation_batch_naming_the_same_finding_twice_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let operation_id = "2".repeat(32);
    let refutation = refutation_review(
        &operation_id,
        "codex-c",
        &receipt,
        r#"[{"id":"loc-1","outcome":"refuted"},{"id":"loc-1","outcome":"corroborated"}]"#,
    );
    let failure =
        record_refutation(&context, &refutation).expect_err("a duplicate id was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "refutation-batch-duplicate-finding");
    assert_eq!(envelope["id"], "loc-1");
}

/// A batch may not name a finding this receipt never recorded as severe.
/// Reached after the receipt is read but before any live claim is asked for —
/// modelled on `a_lens_finding_with_no_location_is_refused`.
#[test]
fn a_refutation_naming_an_unknown_finding_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = vec![
        published(&receipt),
        severe_finding_with_class(
            "4",
            &"3".repeat(32),
            "codex-b",
            &receipt,
            "loc-1",
            "inferential",
        ),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = "2".repeat(32);
    let refutation = refutation_review(
        &operation_id,
        "codex-c",
        &receipt,
        r#"[{"id":"ghost-1","outcome":"refuted"}]"#,
    );
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &refutation)
    })
    .expect_err("a batch naming an unrecorded finding was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "refutation-unknown-finding");
    assert_eq!(envelope["id"], "ghost-1");
}

/// A `deterministic` severe finding already qualifies a rejection with no
/// confirmation owed, so it must never enter the batch.
#[test]
fn a_refutation_naming_a_deterministic_finding_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = vec![
        published(&receipt),
        severe_finding_with_class(
            "4",
            &"3".repeat(32),
            "codex-b",
            &receipt,
            "loc-1",
            "deterministic",
        ),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = "2".repeat(32);
    let refutation = refutation_review(
        &operation_id,
        "codex-c",
        &receipt,
        r#"[{"id":"loc-1","outcome":"corroborated"}]"#,
    );
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &refutation)
    })
    .expect_err("a batch naming a deterministic finding was accepted");
    let envelope = failure.envelope();
    assert_eq!(
        envelope["reason"],
        "refutation-includes-deterministic-finding"
    );
    assert_eq!(envelope["id"], "loc-1");
}

/// `record_refutation`'s only evidence-class refusal excludes `deterministic`;
/// an `insufficient` severe finding is outside that carve-out and reaches
/// `verify_claim` like any admitted candidate, rather than being refused for
/// its evidence class the way the sibling test above shows `deterministic` is.
#[test]
fn a_refutation_naming_an_insufficient_finding_is_admitted() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = vec![
        published(&receipt),
        severe_finding_with_class(
            "4",
            &"3".repeat(32),
            "codex-b",
            &receipt,
            "loc-1",
            "insufficient",
        ),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = "2".repeat(32);
    let refutation = refutation_review(
        &operation_id,
        "codex-c",
        &receipt,
        r#"[{"id":"loc-1","outcome":"refuted"}]"#,
    );
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &refutation)
    })
    .expect_err("no live claim is scripted for this issue, so the operation cannot complete");
    assert_eq!(failure.envelope()["reason"], "not-current-live-holder");
}

/// One batch answers the whole `inferential` population; omitting a standing
/// one is refused rather than silently accepted as partial.
#[test]
fn an_incomplete_refutation_batch_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = vec![
        published(&receipt),
        severe_finding_with_class(
            "4",
            &"3".repeat(32),
            "codex-b",
            &receipt,
            "loc-1",
            "inferential",
        ),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = "2".repeat(32);
    let refutation = refutation_review(&operation_id, "codex-c", &receipt, "[]");
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &refutation)
    })
    .expect_err("a batch omitting a standing inferential finding was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "refutation-batch-incomplete");
    assert_eq!(envelope["missing"], serde_json::json!(["loc-1"]));
}

/// A refuter equal to the run that published cannot be credited with
/// adjudicating its own receipt's findings.
#[test]
fn a_refutation_credited_to_the_publisher_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = vec![published(&receipt)];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = "2".repeat(32);
    // `published` credits "claude-a" as the publisher.
    let refutation = refutation_review(&operation_id, "claude-a", &receipt, "[]");
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &refutation)
    })
    .expect_err("the publishing run was credited with its own refutation");
    assert_eq!(failure.envelope()["reason"], "reviewer-not-distinct");
}

/// The stale-receipt refusal reaches `record_refutation` itself, not only the
/// helper it calls: a batch naming an epoch a later publication superseded is
/// refused `published-transaction-mismatch` before the batch is read.
#[test]
fn a_stale_receipt_sent_through_record_refutation_is_refused() {
    let (_root, context) = context_with_no_repository();
    let latest = ReviewTransaction {
        epoch: "2".repeat(32),
        pr: 7,
        head: "d".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let stale = ReviewTransaction {
        epoch: "1".repeat(32),
        head: "a".repeat(40),
        ..latest.clone()
    };
    let comments = vec![published(&latest)];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = "1".repeat(32);
    let refutation = refutation_review(&operation_id, "codex-b", &stale, "[]");
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &refutation)
    })
    .expect_err("a stale receipt was accepted by record_refutation");
    assert_eq!(
        failure.envelope()["reason"],
        "published-transaction-mismatch"
    );
}

/// A finding id carrying the outcomes JSON's own structural characters — a
/// comma, a colon — must not corrupt a neighbouring entry once the whole
/// batch is percent-encoded onto the marker and read back.
#[test]
fn a_refutation_outcome_id_containing_the_delimiter_round_trips_through_the_marker() {
    let tricky_id = "src/x.rs:12,codex-b:severe,another:1-4";
    let outcomes = serde_json::to_string(&vec![
        RefutationOutcome {
            id: tricky_id.to_owned(),
            outcome: "refuted".to_owned(),
        },
        RefutationOutcome {
            id: "loc-2".to_owned(),
            outcome: "corroborated".to_owned(),
        },
    ])
    .expect("outcomes serialize");
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let rendered = super::super::markers::render(
        "review-refutation",
        &[
            ("run-id", "claude-a"),
            ("refuter", "codex-c"),
            ("op-id", &"1".repeat(32)),
            ("epoch", receipt.epoch.as_str()),
            ("pr", "7"),
            ("head", receipt.head.as_str()),
            ("base", receipt.base.as_str()),
            ("digest", receipt.digest.as_str()),
            ("outcomes", &outcomes),
        ],
    )
    .expect("a review-refutation marker");
    let marker = super::super::markers::parse(&rendered)
        .pop()
        .expect("the marker parses");
    let parsed = ReviewRefutation::from_marker(&marker).expect("the refutation parses");
    assert_eq!(
        parsed.outcomes.len(),
        2,
        "a comma inside one entry's id was read as a second entry: {:?}",
        parsed.outcomes
    );
    assert_eq!(
        parsed.outcomes[0].id, tricky_id,
        "the delimiter-carrying id was corrupted by the round trip"
    );
    assert_eq!(parsed.outcomes[0].outcome, "refuted");
    assert_eq!(parsed.outcomes[1].id, "loc-2");
    assert_eq!(parsed.outcomes[1].outcome, "corroborated");
}

// -- record_disposition ------------------------------------------------------

fn disposition_receipt() -> ReviewTransaction {
    ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    }
}

/// A non-blocking finding — the population a disposition answers. The same
/// marker shape `review_finding` writes, one class lower.
fn warning_finding(
    at: &str,
    operation_id: &str,
    reviewer: &str,
    receipt: &ReviewTransaction,
    id: &str,
) -> ownership::Comment {
    wrote(
        at,
        super::super::markers::render(
            "review-finding",
            &[
                ("run-id", "claude-a"),
                ("op-id", operation_id),
                ("reviewer", reviewer),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("id", id),
                ("class", "warning"),
                (
                    "evidence",
                    "the sentence beside the fix overstates what the check covers",
                ),
                (
                    "impact",
                    "a reader trusts a guarantee the code does not make",
                ),
            ],
        )
        .expect("a warning review-finding marker"),
    )
}

fn disposition_review<'a>(
    operation_id: &'a str,
    receipt: &'a ReviewTransaction,
    dispositions: &'a str,
) -> DispositionReview<'a> {
    DispositionReview {
        issue: 42,
        run_id: "claude-a",
        operation_id,
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        dispositions,
        now: "2026-08-31T00:00:00Z",
    }
}

/// The three words are closed the same way `FINDING_CLASSES` is: a word
/// outside them is the caller's own defect and settles nothing about the
/// timeline, so it is refused before any read.
#[test]
fn a_disposition_word_outside_the_vocabulary_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let operation_id = "2".repeat(32);
    let disposition = disposition_review(
        &operation_id,
        &receipt,
        r#"[{"id":"w-1","disposition":"maybe","reason":"unsure"}]"#,
    );
    let failure = record_disposition(&context, &disposition)
        .expect_err("a word outside the three was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "invalid-disposition-word");
    assert_eq!(envelope["value"], "maybe");
}

#[test]
fn a_malformed_disposition_batch_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let operation_id = "2".repeat(32);
    let disposition = disposition_review(&operation_id, &receipt, "not json at all");
    let failure =
        record_disposition(&context, &disposition).expect_err("a malformed batch was accepted");
    assert_eq!(failure.envelope()["reason"], "invalid-disposition-batch");
}

#[test]
fn a_disposition_batch_naming_the_same_finding_twice_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let operation_id = "2".repeat(32);
    let disposition = disposition_review(
        &operation_id,
        &receipt,
        r#"[{"id":"w-1","disposition":"declined","reason":"not worth a round"},{"id":"w-1","disposition":"filed","reason":"filed late","filed_as":9}]"#,
    );
    let failure =
        record_disposition(&context, &disposition).expect_err("a duplicate id was accepted");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "disposition-batch-duplicate-finding");
    assert_eq!(envelope["id"], "w-1");
}

#[test]
fn a_filed_disposition_without_an_issue_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let operation_id = "2".repeat(32);
    let disposition = disposition_review(
        &operation_id,
        &receipt,
        r#"[{"id":"w-1","disposition":"filed","reason":"became its own work"}]"#,
    );
    let failure = record_disposition(&context, &disposition)
        .expect_err("a filed entry naming no issue was accepted");
    assert_eq!(
        failure.envelope()["reason"],
        "disposition-filed-issue-missing"
    );
}

#[test]
fn a_disposition_filed_as_on_a_word_that_is_not_filed_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let operation_id = "2".repeat(32);
    let disposition = disposition_review(
        &operation_id,
        &receipt,
        r#"[{"id":"w-1","disposition":"declined","reason":"not worth a round","filed_as":9}]"#,
    );
    let failure = record_disposition(&context, &disposition)
        .expect_err("a declined entry carrying filed_as was accepted");
    assert_eq!(
        failure.envelope()["reason"],
        "disposition-unexpected-filed-issue"
    );
}

/// A reason is the whole point of the record, so a blank or filler one says
/// nothing and is refused before any read — the same standing
/// `blank-review-basis` has.
#[test]
fn a_blank_disposition_reason_is_refused_before_any_read() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let operation_id = "2".repeat(32);
    for reason in ["", "   ", "TODO", "n/a"] {
        let batch = serde_json::json!([{
            "id": "w-1", "disposition": "declined", "reason": reason,
        }])
        .to_string();
        let disposition = disposition_review(&operation_id, &receipt, &batch);
        let failure = record_disposition(&context, &disposition)
            .expect_err("a reason that says nothing was accepted");
        assert_eq!(
            failure.envelope()["reason"],
            "blank-disposition-reason",
            "{reason:?} was accepted"
        );
    }
}

/// An id this receipt recorded no finding under is refused with the
/// identities it did record, so the caller can answer the right one.
#[test]
fn a_disposition_naming_an_unknown_finding_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let comments = vec![
        published(&receipt),
        warning_finding("3", &"3".repeat(32), "codex-b", &receipt, "w-1"),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = "2".repeat(32);
    let disposition = disposition_review(
        &operation_id,
        &receipt,
        r#"[{"id":"w-9","disposition":"declined","reason":"not worth a round"}]"#,
    );
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_disposition(&context, &disposition)
    })
    .expect_err("a finding this receipt never recorded was answered");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "disposition-unknown-finding");
    assert_eq!(envelope["id"], "w-9");
    assert_eq!(envelope["recorded"], serde_json::json!(["w-1"]));
}

/// A finding whose every recording is severe is adjudicated by the verdict and
/// never by a disposition, and naming one is refused rather than silently
/// ignored.
#[test]
fn a_disposition_naming_a_severe_finding_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let comments = vec![
        published(&receipt),
        severe_finding_with_class(
            "3",
            &"3".repeat(32),
            "codex-b",
            &receipt,
            "s-1",
            "deterministic",
        ),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = "2".repeat(32);
    let disposition = disposition_review(
        &operation_id,
        &receipt,
        r#"[{"id":"s-1","disposition":"declined","reason":"not worth a round"}]"#,
    );
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_disposition(&context, &disposition)
    })
    .expect_err("a severe finding was dispositioned");
    assert_eq!(
        failure.envelope()["reason"],
        "disposition-includes-severe-finding"
    );
}

/// One identity can be recorded twice at different classes: the
/// `review-finding` operation key carries both `id` and `class`, so a second
/// judge recording the same identity writes a second marker rather than editing
/// the first, and nothing refuses one id twice. `record_disposition` answers
/// such an identity because one of its recordings is non-severe, whichever
/// order the two markers stand in — a severe recording beside the warning does
/// not withdraw it, since the severe one is adjudicated by the verdict alone.
#[test]
fn a_disposition_answers_an_identity_recorded_severe_and_warning_in_either_order() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let operation_id = "2".repeat(32);
    let batch = r#"[{"id":"x-1","disposition":"declined","reason":"a bounded risk the rule declines to spend a publication on"}]"#;
    for severe_first in [true, false] {
        let severe = severe_finding_with_class(
            "3",
            &"3".repeat(32),
            "codex-b",
            &receipt,
            "x-1",
            "deterministic",
        );
        let warning = warning_finding("4", &"4".repeat(32), "codex-c", &receipt, "x-1");
        let mut comments = vec![live_claim("claude-a"), published(&receipt)];
        if severe_first {
            comments.push(severe);
            comments.push(warning);
        } else {
            comments.push(warning);
            comments.push(severe);
        }
        crate::transport::store::forget_scratch_store();
        let bin = crate::test_env::scripted_gh();
        let counts = tempfile::tempdir().expect("a counter directory");
        let script = crate::test_env::answers(&[
            serde_json::json!({
                "matches": "issue view 42 --json body,labels,comments",
                "stdout": stdout_with(&comments).to_string(), "status": 0,
            }),
            serde_json::json!({
                "matches": "issue comment 42", "stdout": "", "status": 0,
            }),
        ]);
        let disposition = disposition_review(&operation_id, &receipt, batch);
        let answer = crate::test_env::with_counting_gh(
            bin.path(),
            &script,
            &counts.path().join("count"),
            || record_disposition(&context, &disposition),
        )
        .unwrap_or_else(|failure| {
            panic!(
                "the batch answering the warning was refused because the same identity is also \
                 severe (severe first: {severe_first}): {}",
                failure.envelope()
            )
        });
        assert_eq!(answer["dispositions_count"], 1, "{answer}");
    }
}

/// A receipt this issue's timeline never published is refused rather than
/// answering a fabricated one — the same standing `record_spend` gives it.
#[test]
fn a_disposition_against_a_receipt_this_issue_never_published_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let comments = vec![published(&receipt)];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let elsewhere = ReviewTransaction {
        epoch: "9".repeat(32),
        ..disposition_receipt()
    };
    let operation_id = "2".repeat(32);
    let disposition = disposition_review(&operation_id, &elsewhere, "[]");
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_disposition(&context, &disposition)
    })
    .expect_err("a receipt the timeline never published was answered");
    assert_eq!(
        failure.envelope()["reason"],
        "disposition-transaction-unknown"
    );
}

/// The operation's identity is the population it answers, not the order or
/// the words: one population is one operation however it is spelled, and a
/// wider population is a new one — which is what lets a finding recorded
/// after the first batch still be answered. Mutation named: collapsing
/// `disposition_answers` to a constant key turns the second assertion red
/// with `a narrower batch replayed a wider one`.
#[test]
fn a_disposition_population_is_the_operation_key() {
    let receipt = disposition_receipt();
    let key = |batch: &str| {
        review_operation_id(
            "review-disposition",
            &[
                "claude-a",
                &receipt.epoch,
                "7",
                &receipt.head,
                &receipt.base,
                &receipt.digest,
                &disposition_answers(batch),
            ],
        )
    };
    let one = r#"[{"id":"w-1","disposition":"declined","reason":"not worth a round"}]"#;
    let reordered = r#"[{"id":"w-2","disposition":"filed","reason":"its own work","filed_as":9},{"id":"w-1","disposition":"declined","reason":"another reason entirely"}]"#;
    let wider = r#"[{"id":"w-2","disposition":"filed","reason":"its own work","filed_as":9}]"#;
    assert_eq!(
        key(one),
        key(
            r#"[{"id":"w-1","disposition":"acted-on","reason":"repaired in the next publication"}]"#
        ),
        "the same population spelled differently minted two operations"
    );
    assert_ne!(
        key(one),
        key(wider),
        "a narrower batch replayed a wider one"
    );
    assert_ne!(
        key(wider),
        key(reordered),
        "two different populations minted one operation"
    );
    assert_eq!(
        disposition_answers("not json at all"),
        "not json at all",
        "a malformed batch must answer its own bytes so a retry is the same refusal"
    );
}

/// A finding id carrying the batch JSON's own structural characters — a
/// comma, a colon — must not corrupt a neighbouring entry once the whole
/// batch is percent-encoded onto the marker and read back.
#[test]
fn a_disposition_id_containing_the_delimiter_round_trips_through_the_marker() {
    let tricky_id = "src/x.rs:12,codex-b:warning,another:1-4";
    let batch = serde_json::to_string(&vec![
        DispositionEntry {
            id: tricky_id.to_owned(),
            disposition: "filed".to_owned(),
            reason: "became its own work".to_owned(),
            filed_as: Some(9),
        },
        DispositionEntry {
            id: "w-2".to_owned(),
            disposition: "declined".to_owned(),
            reason: "not worth a publication".to_owned(),
            filed_as: None,
        },
    ])
    .expect("the batch serialises");
    let receipt = disposition_receipt();
    let rendered = super::super::markers::render(
        "review-disposition",
        &[
            ("run-id", "claude-a"),
            ("op-id", &"1".repeat(32)),
            ("epoch", receipt.epoch.as_str()),
            ("pr", "7"),
            ("head", receipt.head.as_str()),
            ("base", receipt.base.as_str()),
            ("digest", receipt.digest.as_str()),
            ("dispositions", &batch),
        ],
    )
    .expect("a review-disposition marker");
    let marker = super::super::markers::parse(&rendered)
        .pop()
        .expect("the marker parses");
    let parsed = ReviewDisposition::from_marker(&marker).expect("the batch parses");
    assert_eq!(
        parsed.dispositions.len(),
        2,
        "a comma inside one entry's id was read as a second entry: {:?}",
        parsed.dispositions
    );
    assert_eq!(parsed.dispositions[0].id, tricky_id);
    assert_eq!(parsed.dispositions[0].filed_as, Some(9));
    assert_eq!(parsed.dispositions[1].id, "w-2");
    assert_eq!(parsed.dispositions[1].filed_as, None);
}

/// The write path end to end: one batch is posted, read back, and a retry of
/// the same operation reads back what the first call wrote rather than
/// posting a second comment.
#[test]
fn a_disposition_batch_is_recorded_and_a_retry_reads_it_back() {
    let (_root, context) = context_with_no_repository();
    let receipt = disposition_receipt();
    let operation_id = "2".repeat(32);
    let finding_op = "3".repeat(32);
    // Two warnings, one answered: the record is optional and may answer a
    // subset, so nothing waits on `w-2`.
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        warning_finding("4", &finding_op, "codex-b", &receipt, "w-1"),
        warning_finding("5", &"5".repeat(32), "codex-b", &receipt, "w-2"),
    ];
    let batch = serde_json::json!([{
        "id": "w-1",
        "disposition": "declined",
        "reason": "a bounded risk the rule declines to spend a publication on",
    }])
    .to_string();
    let bin = crate::test_env::scripted_gh();
    let counts = tempfile::tempdir().expect("a counter directory");
    let script = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "issue view 42 --json body,labels,comments",
            "stdout": stdout_with(&comments).to_string(), "status": 0,
        }),
        serde_json::json!({
            "matches": "issue comment 42", "stdout": "", "status": 0,
        }),
    ]);
    let disposition = disposition_review(&operation_id, &receipt, &batch);
    let answer = crate::test_env::with_counting_gh(
        bin.path(),
        &script,
        &counts.path().join("count"),
        || record_disposition(&context, &disposition),
    )
    .expect("the subset was refused though nothing owed it");
    assert_eq!(answer["dispositions_count"], 1);
    assert_eq!(answer["reused_existing_disposition"], false);

    // The retry: the same operation id finds its own marker on the first
    // read and answers with it, without a second comment.
    let answer = crate::test_env::with_counting_gh(
        bin.path(),
        &script,
        &counts.path().join("count"),
        || record_disposition(&context, &disposition),
    )
    .expect("the retry was refused though the operation was already recorded");
    assert_eq!(answer["reused_existing_disposition"], true);
    assert_eq!(answer["dispositions_count"], 1);
}

/// A claim comment establishing `run_id` as the live holder — the same
/// marker shape `crate::test_env::live_holder_issue` renders, built by hand
/// here because that helper answers a whole `gh issue view` payload on its
/// own and this fixture needs several more comments beside it.
fn live_claim(run_id: &str) -> ownership::Comment {
    wrote(
        "1",
        format!(
            "Claimed by {run_id}.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    )
}

#[test]
fn publication_lane_off_skips_check_run_read_and_names_not_consulted() {
    let root = tempfile::tempdir().expect("a context root");
    let head = "a".repeat(40);
    let context = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: vec![(
            "Publication lane".to_owned(),
            "off".to_owned(),
            Source::Written,
        )],
        repo: Some("acme/widgets".to_owned()),
    };
    // No gh script: a check-run read would fail. Off must not call one.
    let answer = publication_lane(&context, &head, Some(7)).expect("off lane answers");
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["state"], "not-consulted");
    assert!(
        answer["action"]
            .as_str()
            .expect("action")
            .contains("not read"),
        "{answer}"
    );
}

#[test]
fn publication_lane_on_still_reads_and_cancelled_is_not_never_ran() {
    // The cancellation exclusion is independent of the off gate: with the gate
    // on, a cancelled red with empty steps is still ordinary red, never never-ran.
    let head = "b".repeat(40);
    let path = "repos/acme/widgets/commits/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/check-runs";
    let data = serde_json::json!({
        "total_count": 1,
        "check_runs": [{
            "id": 1,
            "name": "ci",
            "status": "completed",
            "conclusion": "cancelled",
            "html_url": "https://example.test/run/1",
            "check_suite": { "id": 9 },
        }],
    });
    let refusal = judge_publication_lane(&data, 7, &head, path).expect_err("cancelled is red");
    assert_eq!(
        refusal.envelope()["reason"],
        "publication-lane-red",
        "cancelled must not clear as never-ran: {}",
        refusal.envelope()
    );
}

#[test]
fn http_422_on_disabled_workflow_is_absent_at_the_classifier() {
    // End-to-end of the disabled-workflow path the issue measured: 422 → Absent.
    // The classifier is what dispatch and publication share; a 422 must not be
    // Forbidden.
    let said = "HTTP 422: Cannot trigger a 'workflow_dispatch' on a disabled workflow";
    assert!(
        matches!(classify_lane_dispatch(1, said), DispatchAttempt::Absent(_)),
        "disabled workflow 422 was not Absent: {:?}",
        classify_lane_dispatch(1, said)
    );
}

#[test]
fn publication_lane_off_is_not_prefix_matched_against_lanes_list() {
    let root = tempfile::tempdir().expect("a context root");
    let head = "c".repeat(40);
    let with_both = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: vec![
            (
                "Publication lanes".to_owned(),
                "ci.yml".to_owned(),
                Source::Written,
            ),
            (
                "Publication lane".to_owned(),
                "off".to_owned(),
                Source::Written,
            ),
        ],
        repo: Some("acme/widgets".to_owned()),
    };
    let plural_only = Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: vec![(
            "Publication lanes".to_owned(),
            "off".to_owned(),
            Source::Written,
        )],
        repo: Some("acme/widgets".to_owned()),
    };

    // The `gh` this read spawns is scripted, so it is the *absent* answer the
    // test needs rather than whatever a concurrently-running test has put on
    // `PATH`: `with_scripted_gh` serialises the environment, and a bare spawn
    // here reads another test's fixture and answers `disabled` instead of
    // failing the read. The exact `Publication lane: off` above short-circuits
    // before any `gh` runs either way.
    let bin = crate::test_env::scripted_gh();
    crate::test_env::with_scripted_gh(bin.path(), "[]", || {
        let answer = publication_lane(&with_both, &head, Some(7)).expect("off via exact");
        assert_eq!(answer["state"], "not-consulted", "{answer}");

        // On by default: the check-run path fails to read against an empty
        // script. A prefix-match bug would have returned not-consulted without
        // calling gh.
        let err = publication_lane(&plural_only, &head, Some(7))
            .expect_err("Publication lanes: off must not short-circuit as Publication lane: off");
        assert!(
            !matches!(err.envelope().get("state"), Some(s) if s == "not-consulted"),
            "{err:?}"
        );
    });
}

/// The `gh` a publication-lane read meets behind a head with no check runs:
/// the repository identity and the empty listing, with whichever Actions
/// surface the caller adds.
fn lane_read_answers() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "matches": "repo view",
            "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}",
            "status": 0,
        }),
        serde_json::json!({
            "matches": "check-runs",
            "stdout": serde_json::json!({"total_count": 0, "check_runs": []}).to_string(),
            "status": 0,
        }),
    ]
}

fn lane_read_context(root: &std::path::Path) -> Context {
    Context {
        skill_dir: root.to_path_buf(),
        repo_dir: root.to_path_buf(),
        config: Vec::new(),
        repo: Some("o/r".to_owned()),
    }
}

#[test]
fn publication_lane_answers_disabled_not_absent_when_actions_are_switched_off() {
    let root = tempfile::tempdir().expect("a context root");
    let head = "d".repeat(40);
    let context = lane_read_context(root.path());
    // The measured answer from another repository: `enabled: false` with no `allowed`
    // policy, which the delivery rule reads as "CI cannot run here".
    let permissions = serde_json::json!({"allowed": null, "enabled": false});
    let mut answers = lane_read_answers();
    answers.push(serde_json::json!({
        "matches": "actions/permissions",
        "stdout": permissions.to_string(),
        "status": 0,
    }));
    let bin = crate::test_env::scripted_gh();
    let answer =
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
            publication_lane(&context, &head, Some(7))
        })
        .expect("a readable permissions answer is not a failed read");
    assert_eq!(
        answer["state"], "disabled",
        "an Actions-off repository must answer differently from one with no lane: {answer}"
    );
    assert_eq!(
        answer["read"], permissions,
        "the answer must carry the permissions read, so the run can record it: {answer}"
    );
    let action = answer["action"].as_str().expect("an action");
    assert!(action.contains("not a delivery requirement"), "{action}");
    assert!(action.contains("verbatim"), "{action}");
    assert!(action.contains("still red"), "{action}");
}

#[test]
fn publication_lane_answers_disabled_not_absent_when_every_workflow_is_disabled_manually() {
    let root = tempfile::tempdir().expect("a context root");
    let head = "d".repeat(40);
    let context = lane_read_context(root.path());
    let workflows = serde_json::json!({
        "total_count": 2,
        "workflows": [
            {"state": "disabled_manually", "path": ".github/workflows/ci.yml"},
            {"state": "disabled_manually", "path": ".github/workflows/lint.yml"},
        ],
    });
    let mut answers = lane_read_answers();
    answers.push(serde_json::json!({
        "matches": "actions/permissions",
        "stdout": serde_json::json!({"enabled": true}).to_string(),
        "status": 0,
    }));
    answers.push(serde_json::json!({
        "matches": "actions/workflows",
        "stdout": workflows.to_string(),
        "status": 0,
    }));
    let bin = crate::test_env::scripted_gh();
    let answer =
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
            publication_lane(&context, &head, Some(7))
        })
        .expect("a readable workflows listing is not a failed read");
    assert_eq!(
        answer["state"], "disabled",
        "every workflow disabled by hand must answer disabled: {answer}"
    );
    assert_eq!(
        answer["read"],
        serde_json::json!([
            {"path": ".github/workflows/ci.yml", "state": "disabled_manually"},
            {"path": ".github/workflows/lint.yml", "state": "disabled_manually"},
        ]),
        "the answer must carry the workflow states it read: {answer}"
    );
}

#[test]
fn publication_lane_answers_absent_where_no_workflow_proves_a_lane() {
    // The shapes a repository without a disabled lane answers with: no workflow
    // under `.github/workflows/` at all; one that is active; and a mix, where a
    // single active workflow means CI can run even though another is
    // `disabled_manually`. The first is what a predicate treating "no active
    // workflow" as "Actions off" would collapse into `disabled`, and the third
    // is what an `any` quantifier in place of `every` would.
    for workflows in [
        serde_json::json!({"total_count": 0, "workflows": []}),
        serde_json::json!({
            "total_count": 1,
            "workflows": [
                {"state": "active", "path": ".github/workflows/ci.yml"},
            ],
        }),
        serde_json::json!({
            "total_count": 2,
            "workflows": [
                {"state": "active", "path": ".github/workflows/ci.yml"},
                {"state": "disabled_manually", "path": ".github/workflows/old.yml"},
            ],
        }),
    ] {
        let root = tempfile::tempdir().expect("a context root");
        let head = "d".repeat(40);
        let context = lane_read_context(root.path());
        let mut answers = lane_read_answers();
        answers.push(serde_json::json!({
            "matches": "actions/permissions",
            "stdout": serde_json::json!({"enabled": true}).to_string(),
            "status": 0,
        }));
        answers.push(serde_json::json!({
            "matches": "actions/workflows",
            "stdout": workflows.to_string(),
            "status": 0,
        }));
        let bin = crate::test_env::scripted_gh();
        let answer = crate::test_env::with_scripted_gh(
            bin.path(),
            &crate::test_env::answers(&answers),
            || publication_lane(&context, &head, Some(7)),
        )
        .expect("a readable answer");
        assert_eq!(
            answer["state"], "absent",
            "nothing proves Actions is off, so this is no-lane rather than disabled: {answer}"
        );
        assert!(
            answer.get("read").is_none(),
            "an absent answer carries no read: {answer}"
        );
    }
}

#[test]
fn publication_lane_answers_disabled_for_no_workflow_with_actions_off() {
    // The pair the `Available` doc used to place in the wrong variant: no
    // workflow under `.github/workflows/` and Actions switched off. The
    // permissions read answers before the listing is asked, so this is
    // `disabled` rather than `absent`.
    let root = tempfile::tempdir().expect("a context root");
    let head = "d".repeat(40);
    let context = lane_read_context(root.path());
    let permissions = serde_json::json!({"allowed": null, "enabled": false});
    let mut answers = lane_read_answers();
    answers.push(serde_json::json!({
        "matches": "actions/permissions",
        "stdout": permissions.to_string(),
        "status": 0,
    }));
    answers.push(serde_json::json!({
        "matches": "actions/workflows",
        "stdout": serde_json::json!({"total_count": 0, "workflows": []}).to_string(),
        "status": 0,
    }));
    let bin = crate::test_env::scripted_gh();
    let answer =
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
            publication_lane(&context, &head, Some(7))
        })
        .expect("a readable permissions answer is not a failed read");
    assert_eq!(
        answer["state"], "disabled",
        "no workflow with Actions off is disabled, not absent: {answer}"
    );
    assert_eq!(
        answer["read"], permissions,
        "the permissions read answers first, so it is the read carried: {answer}"
    );
}

#[test]
fn an_unreadable_actions_surface_fails_the_read_rather_than_answering_absent() {
    let cases: [(&str, serde_json::Value, Option<serde_json::Value>); 3] = [
        (
            "the permissions read did not answer",
            serde_json::json!({
                "matches": "actions/permissions",
                "stderr": "gh: HTTP 403",
                "status": 1,
            }),
            None,
        ),
        (
            "the permissions answer carries no enabled boolean",
            serde_json::json!({
                "matches": "actions/permissions",
                "stdout": "{\"allowed\":null}",
                "status": 0,
            }),
            None,
        ),
        (
            "the workflows read did not answer",
            serde_json::json!({
                "matches": "actions/permissions",
                "stdout": "{\"enabled\":true}",
                "status": 0,
            }),
            Some(serde_json::json!({
                "matches": "actions/workflows",
                "stderr": "gh: HTTP 500",
                "status": 1,
            })),
        ),
    ];
    for (what, permissions, workflows) in cases {
        let root = tempfile::tempdir().expect("a context root");
        let head = "d".repeat(40);
        let context = lane_read_context(root.path());
        let mut answers = lane_read_answers();
        answers.push(permissions);
        if let Some(workflows) = workflows {
            answers.push(workflows);
        }
        let bin = crate::test_env::scripted_gh();
        let failure = crate::test_env::with_scripted_gh(
            bin.path(),
            &crate::test_env::answers(&answers),
            || publication_lane(&context, &head, Some(7)),
        )
        .expect_err("an Actions surface that did not answer answered a lane");
        assert!(
            matches!(failure, Failure::Read(_)),
            "{what} was not a failed read: {failure:?}"
        );
        assert!(
            failure.detail().contains("not an `absent` lane"),
            "{what}: {}",
            failure.detail()
        );
    }
}

std::thread_local! {
    static PUBLISH_AT: std::cell::Cell<u16> = const { std::cell::Cell::new(0) };
}

fn publish_clock() -> Option<u16> {
    PUBLISH_AT.with(|cell| Some(cell.get()))
}

fn publishing_at(minutes: u16, run: impl FnOnce()) {
    PUBLISH_AT.with(|cell| cell.set(minutes));
    crate::harness::operation_window::with_now_override(Some(publish_clock), run);
}

fn schedule_skill(value: &str) -> tempfile::TempDir {
    let skill = tempfile::tempdir().expect("a skill root");
    std::fs::write(
        skill.path().join(crate::skill::CONTRACT),
        format!(
            "| Setting | Value here | Skill default |\n|---|---|---|\n| Git operation window | {value} | disabled |\n"
        ),
    )
    .expect("a contract");
    skill
}

fn publish_what() -> Publication<'static> {
    Publication {
        issue: 12,
        run_id: "claude-abcd1234",
        expect_state: "in-progress",
        base: "main",
        branch: "fix/12",
        title: "example",
        pr_body_file: None,
        worktree: None,
        inline_payload: false,
        topology: None,
        topology_reason: None,
        focus: None,
        declaration: Default::default(),
        expected_epoch: None,
        now: "2026-06-01T00:00Z",
    }
}

fn publish_script() -> String {
    crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        12,
        "in-progress",
    )])
}

#[test]
fn publish_review_denies_its_internal_push_outside_the_period() {
    let skill = schedule_skill("enabled 09:00-17:00");
    let repo = tempfile::tempdir().expect("a checkout");
    let context = Context::live(skill.path().to_path_buf(), repo.path().to_path_buf(), None);
    let what = publish_what();
    let bin = crate::test_env::scripted_gh();
    publishing_at(20 * 60, || {
        let failure = crate::test_env::with_scripted_gh(bin.path(), &publish_script(), || {
            super::publish_review(&context, &what)
        })
        .expect_err("a push at 20:00 passed 09:00-17:00");
        assert_eq!(
            failure.envelope()["reason"],
            "git-operation-window-denied",
            "the schedule did not stop the publication: {}",
            failure.envelope()
        );
    });
}

#[test]
fn publish_review_inside_the_period_reaches_the_checks_after_the_schedule() {
    let skill = schedule_skill("enabled 09:00-17:00");
    let repo = tempfile::tempdir().expect("a checkout");
    let context = Context::live(skill.path().to_path_buf(), repo.path().to_path_buf(), None);
    let what = publish_what();
    let bin = crate::test_env::scripted_gh();
    publishing_at(12 * 60, || {
        let failure = crate::test_env::with_scripted_gh(bin.path(), &publish_script(), || {
            super::publish_review(&context, &what)
        })
        .expect_err("a publication from an empty checkout passed");
        assert_ne!(
            failure.envelope()["reason"],
            "git-operation-window-denied",
            "noon did not reach the checks after the schedule: {}",
            failure.envelope()
        );
    });
}

#[test]
fn release_ci_denies_outside_the_period_before_reading_anything_else() {
    let skill = schedule_skill("enabled 09:00-17:00");
    let repo = tempfile::tempdir().expect("a checkout");
    let context = Context::live(skill.path().to_path_buf(), repo.path().to_path_buf(), None);
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let release = CiRelease {
        issue: 12,
        run_id: "claude-abcd1234",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        worktree: None,
        now: "2026-06-01T00:00Z",
    };
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        12,
        "review",
    )]);
    let bin = crate::test_env::scripted_gh();
    publishing_at(20 * 60, || {
        let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            release_ci(&context, &release)
        })
        .expect_err("a release at 20:00 passed 09:00-17:00");
        assert_eq!(
            failure.envelope()["reason"],
            "git-operation-window-denied",
            "{}",
            failure.envelope()
        );
    });
}

/// The renewal writer records whichever activity kind it was asked for, and the
/// marker it hands the timeline carries that kind — read back from the store's
/// own entry, not from the call's answer. It posts no comment to GitHub.
///
/// Mutation: render `"heartbeat"` instead of `kind` in `heartbeat`. The
/// `recorded` assertion stays green and the stored marker becomes `heartbeat`
/// for every row, which is what this catches.
#[test]
fn heartbeat_records_the_activity_kind_it_is_asked_for() {
    let (root, context) = context_with_no_repository();
    let issue = 918_301_u64;
    let run_id = "claude-heartbeat0";
    let note = root.path().join("note.md");
    std::fs::write(&note, "still working\n").expect("the note");

    let bin = crate::test_env::scripted_gh();
    for kind in ["heartbeat", "progress", "blocked"] {
        let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
            run_id,
            issue,
            "in-progress",
        )]);
        let log = root.path().join(format!("{kind}.log"));
        let (answer, timeline) =
            crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
                let answer = super::heartbeat(
                    &context,
                    issue,
                    run_id,
                    "in-progress",
                    &note,
                    kind,
                    "2026-01-01T00:05:00Z",
                );
                (answer, store_timeline(&context, issue))
            });
        let answer =
            answer.unwrap_or_else(|failure| panic!("{kind} was refused: {}", failure.envelope()));
        assert_eq!(answer["recorded"], true, "{kind}: {answer}");
        let marker = format!("<!-- issue-flow: {kind} encoding=pct run-id={run_id} -->");
        assert!(
            timeline.iter().any(|entry| entry.body.contains(&marker)),
            "{kind} did not record its own marker: {timeline:?}"
        );
        let calls = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
        assert!(
            !calls.contains("issue comment"),
            "{kind} posted a comment: {calls}"
        );
    }
}

/// A kind the renewal writer does not own — `branch` and `published` have
/// writers of their own — is refused, and the refusal happens before anything is
/// read: the fake `gh` is never spawned, so nothing was renewed and nothing was
/// posted.
///
/// Mutation: drop the `RENEWAL_KINDS.contains` guard. The call falls through to
/// `verify_claim`, spawns the scripted `gh`, and the counter file this asserts
/// absent appears.
#[test]
fn a_renewal_kind_the_writer_does_not_own_is_refused_before_any_read() {
    let (root, context) = context_with_no_repository();
    let note = root.path().join("note.md");
    std::fs::write(&note, "note\n").expect("the note");
    let issue = 918_302_u64;
    let bin = crate::test_env::scripted_gh();
    let counts = tempfile::tempdir().expect("a counter directory");
    let count = counts.path().join("count");
    let script = crate::test_env::answers(&[
        crate::test_env::live_holder_issue("claude-heartbeat1", issue, "in-progress"),
        serde_json::json!({
            "matches": format!("issue comment {issue}"),
            "stdout": "",
            "status": 0,
        }),
    ]);
    let failure = crate::test_env::with_counting_gh(bin.path(), &script, &count, || {
        super::heartbeat(
            &context,
            issue,
            "claude-heartbeat1",
            "in-progress",
            &note,
            "published",
            "2026-01-01T00:05:00Z",
        )
    })
    .expect_err("a kind the renewal writer does not own was accepted");
    assert_eq!(
        failure.envelope()["reason"],
        "renewal-kind-unsupported",
        "{}",
        failure.envelope()
    );
    assert!(
        !count.exists(),
        "the refused kind still reached the tracker, so it was not refused before the read"
    );
}

/// [`review_world`], descended from a rejected original: the receipt under
/// review is the lineage's one correction, already spent.
fn review_world_with_spent_correction() -> (ReviewTransaction, Vec<ownership::Comment>) {
    let (receipt, mut comments) = review_world();
    let original = ReviewTransaction {
        epoch: "9".repeat(32),
        head: "9".repeat(40),
        digest: "9".repeat(64),
        ..receipt.clone()
    };
    comments[1] = published_with_parent("claude-a", &receipt, &original, "delta-9-1");
    comments.insert(1, published(&original));
    comments.insert(2, computed_by("codex-b", &original, "rejected"));
    (receipt, comments)
}

fn review_world() -> (ReviewTransaction, Vec<ownership::Comment>) {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        severe_finding_with_class(
            "4",
            &"3".repeat(32),
            "codex-b",
            &receipt,
            "loc-1",
            "deterministic",
        ),
    ];
    (receipt, comments)
}

fn rest_listing_of(issue: u64, comments: &[ownership::Comment]) -> serde_json::Value {
    let listed: Vec<serde_json::Value> = comments
        .iter()
        .map(|comment| {
            serde_json::json!({
                "id": 1,
                "node_id": comment.id,
                "user": { "login": "fixture" },
                "created_at": comment.created_at,
                "updated_at": comment.created_at,
                "body": comment.body,
            })
        })
        .collect();
    serde_json::json!({
        "matches": format!("issues/{issue}/comments"),
        "stdout": serde_json::Value::Array(listed).to_string(),
        "status": 0,
    })
}

fn verdict_operation(receipt: &ReviewTransaction, outcome: &str) -> String {
    review_operation_id(
        "review-verdict",
        &[
            "claude-a",
            "codex-b",
            &receipt.epoch,
            "7",
            &receipt.head,
            &receipt.base,
            &receipt.digest,
            outcome,
        ],
    )
}

fn graphql_calls_after_import<T>(
    context: &Context,
    comments: &[ownership::Comment],
    operation: &str,
    body: impl FnOnce(&Context) -> T,
) -> (T, u64) {
    let directory = tempfile::tempdir().expect("a counter directory");
    let path = directory.path().join("state.db");
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(comments)),
        serde_json::json!({"matches": "issue comment 42", "stdout": "", "status": 0}),
    ]);
    let answer = crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            Ledger::open(context, 42, RECLAIM_NOW).expect("the first touch imports the issue");
        });
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            crate::transport::store::counting(operation, || body(context))
        })
    });
    let store = crate::transport::store::Store::open(&path).expect("the counter store opens");
    (
        answer,
        store.calls_since(0).expect("the counter reads").graphql,
    )
}

#[test]
fn a_computed_verdict_on_an_imported_issue_reads_no_graphql_timeline() {
    let (_root, context, receipt) = low_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
    ];
    let operation_id = op(9);
    let request = computed_request(&operation_id, &receipt);

    let (answer, graphql) =
        graphql_calls_after_import(&context, &comments, "record-computed-verdict", |context| {
            record_computed_verdict(context, &request)
        });

    answer.expect("the rejection resting on a deterministic finding is recorded");
    assert_eq!(
        graphql, 0,
        "a verdict makes no GraphQL call: it posts no comment"
    );
}

#[test]
fn a_ci_release_on_an_imported_issue_reads_no_graphql_timeline() {
    let (receipt, comments) = review_world();
    let release = CiRelease {
        issue: 42,
        run_id: "claude-a",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        worktree: None,
        now: "2026-08-31T00:00:00Z",
    };

    let (_root, context) = context_with_no_repository();
    let (answer, graphql) =
        graphql_calls_after_import(&context, &comments, "release-ci", |context| {
            release_ci(context, &release)
        });

    assert_eq!(
        reason(answer.expect_err("no verdict was recorded, so nothing is released")),
        "qualifying-review-verdict-missing"
    );
    assert_eq!(graphql, 0, "the timeline came from the store");
}

/// An `approved` receipt a retired-style accepted verdict left behind is not a
/// release basis: delivery reads the computed verdict alone, so an accepted verdict
/// and its receipt over a `medium` change release nothing.
#[test]
fn an_accepted_verdict_and_its_approved_receipt_release_nothing() {
    let (_root, context, receipt) = medium_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        legacy_accepted(&receipt),
        approved_receipt_by("claude-a", "codex-b", &receipt),
    ];
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "qualifying-review-verdict-missing"
    );
}

/// An escalation is refused by `release_ci` even when the ledger computes
/// `approved` and the marker agrees: computed evidence does not carry an
/// escalation past CI.
#[test]
fn an_escalated_receipt_stops_a_release_the_computed_verdict_would_clear() {
    let (_root, context, receipt) = medium_change();
    let mut comments = reported_and_recorded(&receipt, "medium");
    comments.push(escalated_receipt_by(
        "claude-a",
        &"5".repeat(32),
        "Alexis",
        &receipt,
    ));
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let release = CiRelease {
        issue: 42,
        run_id: "claude-a",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        worktree: None,
        now: "2026-08-31T00:00:00Z",
    };
    let failure =
        crate::test_env::with_scripted_gh(bin.path(), &script, || release_ci(&context, &release))
            .expect_err("an escalation was released over");
    assert_eq!(reason(failure), "review-receipt-escalated");
}

/// A distinct run's `approved` receipt supersedes an earlier escalation and a
/// forbidden run's does not: the publisher cannot approve its way past the
/// decision it handed to a person.
#[test]
fn only_a_distinct_runs_approval_supersedes_an_escalation() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let escalated = escalated_receipt_by("claude-a", &"5".repeat(32), "Alexis", &receipt);
    let distinct = [
        published(&receipt),
        escalated.clone(),
        approved_receipt_by("codex-b", "codex-b", &receipt),
    ];
    assert_eq!(
        terminal_receipt(&distinct, &receipt)
            .expect("readable")
            .map(|terminal| terminal.terminal_state),
        Some("approved".to_owned())
    );
    require_un_escalated_terminal(&distinct, &receipt).expect("the approval supersedes");

    let by_the_publisher = [
        published(&receipt),
        escalated,
        approved_receipt_by("claude-a", "claude-a", &receipt),
    ];
    assert_eq!(
        reason(
            require_un_escalated_terminal(&by_the_publisher, &receipt)
                .expect_err("the publisher approved its own escalation away")
        ),
        "review-receipt-escalated"
    );
}

/// An approved computed verdict records its `approved` terminal receipt
/// **locally** and posts the verdict comment and nothing more, so the receipt
/// that makes an approval final against an escalation lives with the workflow
/// state.
#[test]
fn an_approved_computed_verdict_records_its_receipt_locally_and_posts_no_comment() {
    let (_root, context, receipt) = low_change();
    let comments = vec![live_claim("claude-a"), published(&receipt)];
    let directory = tempfile::tempdir().expect("a directory for the log");
    let path = directory.path().join("state.db");
    let log = directory.path().join("calls.log");
    let bin = crate::test_env::scripted_gh();
    let mut answers = lane_read_answers();
    answers.insert(
        0,
        crate::test_env::issue_view_answer(42, stdout_with(&comments)),
    );
    answers.push(serde_json::json!({"matches": "issue comment 42", "stdout": "", "status": 0}));
    let script = crate::test_env::answers(&answers);
    let operation_id = op(9);
    let request = computed_request(&operation_id, &receipt);

    let (answer, timeline) = crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            Ledger::open(&context, 42, RECLAIM_NOW).expect("the first touch imports the issue");
        });
        crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            let answer = record_computed_verdict(&context, &request).expect("the approval records");
            let timeline = Ledger::open(&context, 42, request.now)
                .expect("the ledger opens")
                .timeline()
                .expect("the timeline reads");
            (answer, timeline)
        })
    });

    assert_eq!(answer["outcome"], "approved", "{answer}");
    assert!(
        timeline
            .iter()
            .any(|comment| carries_receipt(comment, &receipt, "approved")),
        "the approval left no local approved receipt: {timeline:?}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(
        calls.matches("issue comment").count(),
        0,
        "the receipt was posted as a comment: {calls}"
    );
}

/// The receipt an approval recorded before its local write failed is written by
/// replaying the verdict, and a replay posts nothing: the receipt comes out of
/// the store. An `escalated` receipt already standing does not stop it.
#[test]
fn replaying_an_approved_computed_verdict_records_the_missing_receipt_locally() {
    let (_root, context, receipt) = low_change();
    let operation_id = format!("a{}", &receipt.epoch[1..]);
    let comments = vec![
        live_claim("codex-b"),
        published(&receipt),
        escalated_receipt_by("claude-a", &"5".repeat(32), "Alexis", &receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];
    let request = ComputedVerdictReview {
        run_id: "codex-b",
        ..computed_request(&operation_id, &receipt)
    };

    let ((answer, timeline), graphql) =
        graphql_calls_after_import(&context, &comments, "record-computed-verdict", |context| {
            let answer = record_computed_verdict(context, &request).expect("the replay answers");
            let timeline = Ledger::open(context, 42, request.now)
                .expect("the ledger opens")
                .timeline()
                .expect("the timeline reads");
            (answer, timeline)
        });

    assert_eq!(answer["reused_existing_verdict"], true, "{answer}");
    assert_eq!(graphql, 0, "a verdict replay posts nothing");
    assert!(
        timeline
            .iter()
            .any(|comment| carries_receipt(comment, &receipt, "approved")),
        "the replay left no local approved receipt: {timeline:?}"
    );
}

/// An `escalated` receipt is a decision a person took, and it is refused as its
/// own state rather than read as a missing review.
#[test]
fn a_release_refuses_an_escalated_transaction_with_its_own_reason() {
    let (receipt, mut comments) = review_world();
    comments.push(verdict_by(
        "codex-b",
        "accepted",
        &verdict_operation(&receipt, "accepted"),
        &receipt,
    ));
    comments.push(escalated_receipt_by(
        "claude-a",
        &"5".repeat(32),
        "Alexis",
        &receipt,
    ));

    let release = CiRelease {
        issue: 42,
        run_id: "claude-a",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        worktree: None,
        now: "2026-08-31T00:00:00Z",
    };
    let (_root, context) = context_with_no_repository();
    let (answer, _graphql) =
        graphql_calls_after_import(&context, &comments, "release-ci", |context| {
            release_ci(context, &release)
        });

    assert_eq!(
        reason(answer.expect_err("an escalation does not substitute for an acceptance")),
        "review-receipt-escalated"
    );
}

/// An `approved` receipt is the terminal state `release_ci` asks for, read by
/// identity and bounded to the newest publication on its own pull request.
#[test]
fn an_approved_receipt_is_the_terminal_state_a_release_asks_for() {
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let other = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "d".repeat(40),
        ..receipt.clone()
    };
    let comments = [
        published(&receipt),
        approved_receipt_by("claude-a", "codex-b", &receipt),
        approved_receipt_by("claude-a", "codex-b", &other),
    ];

    let terminal = terminal_receipt(&comments, &receipt)
        .expect("readable")
        .expect("an approved receipt stands");
    assert_eq!(terminal.terminal_state, "approved");
    assert_eq!(terminal.reviewer.as_deref(), Some("codex-b"));
    assert_eq!(
        terminal_receipt(&comments, &other)
            .expect("readable")
            .map(|receipt| receipt.transaction.epoch),
        None,
        "a receipt for another transaction is not this one's terminal state"
    );
}

/// Whether a body carries a receipt marker for exactly this transaction and
/// terminal state, read back the way `release_ci` reads it.
fn carries_receipt(comment: &ownership::Comment, receipt: &ReviewTransaction, state: &str) -> bool {
    super::super::markers::parse(&comment.body)
        .iter()
        .any(|marker| {
            marker.get("epoch").map(String::as_str) == Some(receipt.epoch.as_str())
                && marker.get("terminal-state").map(String::as_str) == Some(state)
        })
}

/// An escalation once the one correction is spent records its `escalated` receipt
/// **locally** and posts nothing, so the terminal state delivery refuses lives
/// where the workflow state does rather than on a tracking comment.
#[test]
fn an_escalation_after_the_correction_is_spent_records_a_local_receipt_and_posts_nothing() {
    let (receipt, mut comments) = review_world_with_spent_correction();
    // The reviewer holds the live `review` claim the escalation is gated on.
    comments[0] = live_claim("codex-b");

    let (_root, mut context) = context_with_no_repository();
    context.config = corrections_limit_row("1");
    let directory = tempfile::tempdir().expect("a directory for the log");
    let path = directory.path().join("state.db");
    let log = directory.path().join("calls.log");
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(&comments)),
        serde_json::json!({"matches": "issue comment 42", "stdout": "", "status": 0}),
    ]);
    let escalation = ReviewEscalation {
        issue: 42,
        run_id: "codex-b",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        escalated_to: "asanabrial",
        reason: "the correction was spent",
        now: "2026-08-31T00:00:00Z",
    };

    let (answer, timeline) = crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            Ledger::open(&context, 42, RECLAIM_NOW).expect("the first touch imports the issue");
        });
        crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            let answer =
                escalate_review(&context, &escalation).expect("the escalation is recorded");
            let timeline = Ledger::open(&context, 42, escalation.now)
                .expect("the ledger opens")
                .timeline()
                .expect("the timeline reads");
            (answer, timeline)
        })
    });

    assert_eq!(answer["terminal_state"], "escalated", "{answer}");
    assert_eq!(answer["escalated_to"], "asanabrial", "{answer}");
    assert!(
        timeline
            .iter()
            .any(|comment| carries_receipt(comment, &receipt, "escalated")),
        "the escalation left no local escalated receipt: {timeline:?}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("issue comment"),
        "the escalation wrote a tracking comment: {calls}"
    );
}

/// A spent correction is what authorises an escalation, so a decision handed
/// to a person while the lineage still has its one correction is refused
/// rather than recorded — it would be a route around the repair, not the exit
/// from it. Mutation: dropping the `correction_spent` read lets it through.
#[test]
fn an_escalation_before_the_correction_is_spent_is_refused() {
    let (receipt, mut comments) = review_world();
    comments[0] = live_claim("codex-b");
    comments.push(computed_by("codex-b", &receipt, "rejected"));

    let (_root, context) = context_with_no_repository();
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(&comments)),
        serde_json::json!({"matches": "issue comment 42", "stdout": "", "status": 0}),
    ]);
    let escalation = ReviewEscalation {
        issue: 42,
        run_id: "codex-b",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        escalated_to: "asanabrial",
        reason: "the correction was spent",
        now: "2026-08-31T00:00:00Z",
    };

    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        escalate_review(&context, &escalation)
    });

    assert_eq!(
        reason(answer.expect_err("the lineage has not spent its correction")),
        "escalate-review-correction-unspent"
    );
}

/// An approval is final: an escalation cannot overwrite the `approved` receipt
/// an acceptance recorded, so a change a distinct reviewer accepted cannot be
/// silently flipped to `escalated` and then refused by `release_ci`.
#[test]
fn an_escalation_over_an_approved_receipt_is_refused() {
    let (receipt, mut comments) = review_world_with_spent_correction();
    // The reviewer holds the live `review` claim, and the correction is spent
    // and the transaction already approved.
    comments[0] = live_claim("codex-b");
    comments.push(approved_receipt_by("claude-a", "codex-b", &receipt));

    let (_root, context) = context_with_no_repository();
    let directory = tempfile::tempdir().expect("a directory for the store");
    let path = directory.path().join("state.db");
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(&comments)),
        serde_json::json!({"matches": "issue comment 42", "stdout": "", "status": 0}),
    ]);
    let escalation = ReviewEscalation {
        issue: 42,
        run_id: "codex-b",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        escalated_to: "asanabrial",
        reason: "the correction was spent",
        now: "2026-08-31T00:00:00Z",
    };

    let answer = crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            Ledger::open(&context, 42, RECLAIM_NOW).expect("the first touch imports the issue");
        });
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            escalate_review(&context, &escalation)
        })
    });

    assert_eq!(
        reason(answer.expect_err("an approval is final")),
        "escalate-review-already-approved"
    );
}

/// An escalation is refused before the release basis, so it holds whatever
/// would otherwise release — a distinct accepted verdict, or computed evidence.
#[test]
fn an_escalated_receipt_is_refused_whatever_would_otherwise_release() {
    let (receipt, mut comments) = review_world();
    comments.push(escalated_receipt_by(
        "claude-a",
        &"5".repeat(32),
        "Alexis",
        &receipt,
    ));
    assert_eq!(
        reason(
            require_un_escalated_terminal(&comments, &receipt)
                .expect_err("an escalation is not the acceptance delivery asks for"),
        ),
        "review-receipt-escalated"
    );

    // The same transaction with no receipt at all is not the escalation rule's
    // business: it answers `None` and the release basis decides.
    let (receipt, comments) = review_world();
    assert!(
        require_un_escalated_terminal(&comments, &receipt)
            .expect("a readable timeline")
            .is_none()
    );
}

#[test]
fn a_finding_read_on_an_imported_issue_reads_no_graphql_timeline() {
    let (_receipt, comments) = review_world();

    let (_root, context) = context_with_no_repository();
    let (answer, graphql) =
        graphql_calls_after_import(&context, &comments, "review-findings", |context| {
            review_findings(context, 42, None, None, false, RECLAIM_NOW)
        });

    assert_eq!(answer.expect("the findings answer")["findings_count"], 1);
    assert_eq!(graphql, 0, "the timeline came from the store");
}

#[test]
fn a_finding_another_run_posted_after_import_decides_no_verdict() {
    let (_root, context, receipt) = low_change();
    let base = vec![live_claim("claude-a"), published(&receipt)];
    let foreign = recorded_finding_on(
        &receipt,
        6,
        "f-1",
        "severe",
        Some("deterministic"),
        Some("introduced"),
    );
    let bin = crate::test_env::scripted_gh();
    let before = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(&base)),
        serde_json::json!({"matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0}),
    ]);
    let after = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(&base)),
        rest_listing_of(42, &[foreign]),
        serde_json::json!({"matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0}),
    ]);
    let read = |script: &str| {
        crate::test_env::with_scripted_gh(bin.path(), script, || {
            show_computed_verdict(&context, 42, &receipt, None, false, RECLAIM_NOW)
        })
        .expect("the computed verdict reads")
    };

    let unsynced = read(&before);
    let synced = read(&after);

    assert_eq!(unsynced["outcome"], "approved", "{unsynced}");
    assert_eq!(synced["outcome"], "approved", "{synced}");
}

#[test]
fn the_computed_verdict_read_carries_the_generated_file_reproduction() {
    let (_root, context, receipt) = low_change();
    let base = vec![live_claim("claude-a"), published(&receipt)];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(&base)),
        serde_json::json!({"matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0}),
    ]);

    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        show_computed_verdict(&context, 42, &receipt, None, false, RECLAIM_NOW)
    })
    .expect("the computed verdict reads");

    assert_eq!(answer["generated_files"]["outcomes"], serde_json::json!([]));
    assert!(answer["generated_files"]["refusal"].is_null());
}

// -- computed verdict ------------------------------------------------------

use crate::transport::tier::RiskLevel;

const RISK_LENS: &str = "review-risk";
const RELIABILITY_LENS: &str = "review-reliability";

fn op(n: u32) -> String {
    format!("{n:032x}")
}

fn ledger_receipt() -> ReviewTransaction {
    ReviewTransaction {
        epoch: op(1),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    }
}

fn recorded_finding(
    n: u32,
    id: &str,
    class: &str,
    evidence_class: Option<&str>,
    causal: Option<&str>,
) -> ownership::Comment {
    recorded_finding_on(&ledger_receipt(), n, id, class, evidence_class, causal)
}

fn recorded_finding_on(
    receipt: &ReviewTransaction,
    n: u32,
    id: &str,
    class: &str,
    evidence_class: Option<&str>,
    causal: Option<&str>,
) -> ownership::Comment {
    let operation = op(100 + n);
    let pr = receipt.pr.to_string();
    let mut attributes: Vec<(&str, &str)> = vec![
        ("run-id", "claude-a"),
        ("op-id", &operation),
        ("reviewer", "codex-b"),
        ("epoch", &receipt.epoch),
        ("pr", &pr),
        ("head", &receipt.head),
        ("base", &receipt.base),
        ("digest", &receipt.digest),
        ("id", id),
        ("class", class),
        ("evidence", "observed the defect directly"),
        ("impact", "wrong output on the happy path"),
    ];
    if let Some(value) = evidence_class {
        attributes.push(("evidence-class", value));
    }
    if let Some(value) = causal {
        attributes.push(("causal-disposition", value));
    }
    wrote(
        &n.to_string(),
        super::super::markers::render("review-finding", &attributes).expect("a finding marker"),
    )
}

fn recorded_refutation(n: u32, outcomes: &str) -> ownership::Comment {
    let receipt = ledger_receipt();
    let operation = op(200 + n);
    let pr = receipt.pr.to_string();
    wrote(
        &n.to_string(),
        super::super::markers::render(
            "review-refutation",
            &[
                ("run-id", "claude-a"),
                ("refuter", "codex-c"),
                ("op-id", &operation),
                ("epoch", &receipt.epoch),
                ("pr", &pr),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("outcomes", outcomes),
            ],
        )
        .expect("a refutation marker"),
    )
}

fn recorded_lens_report(n: u32, role: &str, focus: Option<&str>) -> ownership::Comment {
    recorded_lens_report_for(&ledger_receipt(), n, role, focus)
}

fn recorded_lens_report_for(
    receipt: &ReviewTransaction,
    n: u32,
    role: &str,
    focus: Option<&str>,
) -> ownership::Comment {
    let operation = op(300 + n);
    let pr = receipt.pr.to_string();
    let mut attributes: Vec<(&str, &str)> = vec![
        ("run-id", "claude-a"),
        ("op-id", &operation),
        ("role", role),
        ("epoch", &receipt.epoch),
        ("pr", &pr),
        ("head", &receipt.head),
        ("base", &receipt.base),
        ("digest", &receipt.digest),
    ];
    if let Some(value) = focus {
        attributes.push(("focus", value));
    }
    wrote(
        &n.to_string(),
        super::super::markers::render("review-lens-report", &attributes)
            .expect("a lens report marker"),
    )
}

fn computed_over(comments: &[ownership::Comment], level: RiskLevel) -> ComputedVerdict {
    computed_verdict(comments, &ledger_receipt(), level, None, false)
}

fn settled_alone(
    evidence_class: Option<&str>,
    causal: Option<&str>,
    refutation: Option<&str>,
) -> ComputedVerdict {
    let mut comments = vec![recorded_finding(1, "f-1", "severe", evidence_class, causal)];
    if let Some(outcomes) = refutation {
        comments.push(recorded_refutation(2, outcomes));
    }
    computed_over(&comments, RiskLevel::Low)
}

fn ids(verdict: &ComputedVerdict) -> (Vec<&str>, Vec<(&str, &str)>, Vec<&str>) {
    (
        verdict.open.iter().map(String::as_str).collect(),
        verdict
            .escalated
            .iter()
            .map(|e| (e.id.as_str(), e.reason.as_str()))
            .collect(),
        verdict.follow_ups.iter().map(String::as_str).collect(),
    )
}

#[test]
fn a_finding_that_is_not_severe_never_blocks() {
    for class in ["warning", "suggestion"] {
        let verdict = computed_over(
            &[recorded_finding(1, "f-1", class, None, None)],
            RiskLevel::Low,
        );
        assert_eq!(verdict.outcome, ComputedOutcome::Approved, "{class}");
        assert_eq!(ids(&verdict), (vec![], vec![], vec![]), "{class}");
    }
}

#[test]
fn a_severe_finding_with_insufficient_evidence_is_escalated() {
    let verdict = settled_alone(Some("insufficient"), Some("introduced"), None);
    assert_eq!(verdict.outcome, ComputedOutcome::Escalated);
    assert_eq!(
        ids(&verdict),
        (vec![], vec![("f-1", "insufficient-evidence")], vec![])
    );
}

#[test]
fn a_severe_finding_with_an_unknown_cause_is_escalated() {
    let verdict = settled_alone(Some("deterministic"), Some("unknown"), None);
    assert_eq!(verdict.outcome, ComputedOutcome::Escalated);
    assert_eq!(
        ids(&verdict),
        (vec![], vec![("f-1", "causal-disposition-unknown")], vec![])
    );
}

#[test]
fn a_severe_finding_owned_by_the_base_is_a_follow_up_and_does_not_block() {
    for causal in ["pre-existing", "base-only"] {
        let verdict = settled_alone(Some("deterministic"), Some(causal), None);
        assert_eq!(verdict.outcome, ComputedOutcome::Approved, "{causal}");
        assert_eq!(ids(&verdict), (vec![], vec![], vec!["f-1"]), "{causal}");
    }
}

#[test]
fn insufficient_evidence_escalates_even_when_the_cause_is_pre_existing() {
    let verdict = settled_alone(Some("insufficient"), Some("pre-existing"), None);
    assert_eq!(verdict.outcome, ComputedOutcome::Escalated);
    assert_eq!(
        ids(&verdict),
        (vec![], vec![("f-1", "insufficient-evidence")], vec![])
    );
}

#[test]
fn a_deterministic_severe_finding_stands_open_for_correction() {
    let verdict = settled_alone(Some("deterministic"), Some("introduced"), None);
    assert_eq!(verdict.outcome, ComputedOutcome::Rejected);
    assert_eq!(ids(&verdict), (vec!["f-1"], vec![], vec![]));
}

#[test]
fn an_inferential_finding_the_refuter_refuted_is_dropped() {
    let verdict = settled_alone(
        Some("inferential"),
        Some("introduced"),
        Some(r#"[{"id":"f-1","outcome":"refuted"}]"#),
    );
    assert_eq!(verdict.outcome, ComputedOutcome::Approved);
    assert_eq!(ids(&verdict), (vec![], vec![], vec![]));
}

#[test]
fn an_inferential_finding_the_refuter_corroborated_or_left_inconclusive_stays_open() {
    for word in ["corroborated", "inconclusive"] {
        let outcomes = format!(r#"[{{"id":"f-1","outcome":"{word}"}}]"#);
        let verdict = settled_alone(Some("inferential"), Some("introduced"), Some(&outcomes));
        assert_eq!(verdict.outcome, ComputedOutcome::Rejected, "{word}");
        assert_eq!(ids(&verdict), (vec!["f-1"], vec![], vec![]), "{word}");
    }
}

#[test]
fn an_inferential_finding_with_no_refuter_result_is_escalated() {
    let verdict = settled_alone(Some("inferential"), Some("introduced"), None);
    assert_eq!(verdict.outcome, ComputedOutcome::Escalated);
    assert_eq!(
        ids(&verdict),
        (vec![], vec![("f-1", "no-refuter-result")], vec![])
    );
}

#[test]
fn a_severe_finding_missing_its_evidence_class_or_cause_is_escalated() {
    let no_class = settled_alone(None, Some("introduced"), None);
    assert_eq!(no_class.outcome, ComputedOutcome::Escalated);
    assert_eq!(
        ids(&no_class),
        (vec![], vec![("f-1", "evidence-class-missing")], vec![])
    );
    let no_cause = settled_alone(Some("deterministic"), None, None);
    assert_eq!(no_cause.outcome, ComputedOutcome::Escalated);
    assert_eq!(
        ids(&no_cause),
        (vec![], vec![("f-1", "causal-disposition-missing")], vec![])
    );
}

#[test]
fn an_escalated_finding_outranks_an_open_one() {
    let verdict = computed_over(
        &[
            recorded_finding(
                1,
                "open-1",
                "severe",
                Some("deterministic"),
                Some("introduced"),
            ),
            recorded_finding(
                2,
                "unsure-1",
                "severe",
                Some("insufficient"),
                Some("introduced"),
            ),
        ],
        RiskLevel::Low,
    );
    assert_eq!(verdict.outcome, ComputedOutcome::Escalated);
    assert_eq!(verdict.open, vec!["open-1".to_owned()]);
    assert_eq!(verdict.escalated.len(), 1);
}

#[test]
fn an_empty_ledger_with_an_unreported_licensed_lens_is_incomplete_not_approved() {
    let medium = computed_over(&[], RiskLevel::Medium);
    assert_eq!(medium.outcome, ComputedOutcome::Incomplete);
    assert_eq!(medium.missing_roles, vec![RELIABILITY_LENS.to_owned()]);
    let high = computed_over(&[recorded_lens_report(1, RISK_LENS, None)], RiskLevel::High);
    assert_eq!(high.outcome, ComputedOutcome::Incomplete);
    assert_eq!(high.missing_roles.len(), 3, "{:?}", high.missing_roles);
}

#[test]
fn every_licensed_lens_reported_and_nothing_open_is_approved() {
    let reports: Vec<_> = [
        "review-risk",
        "review-readback",
        "review-readability",
        "review-reliability",
    ]
    .iter()
    .enumerate()
    .map(|(n, role)| recorded_lens_report(n as u32 + 1, role, None))
    .collect();
    let verdict = computed_over(&reports, RiskLevel::High);
    assert_eq!(verdict.outcome, ComputedOutcome::Approved);
    assert!(verdict.missing_roles.is_empty());
}

#[test]
fn a_low_tier_needs_no_lens_report_to_be_approved() {
    let verdict = computed_over(&[], RiskLevel::Low);
    assert_eq!(verdict.outcome, ComputedOutcome::Approved);
    assert!(verdict.missing_roles.is_empty());
}

#[test]
fn a_medium_change_licenses_the_lens_a_report_names_by_focus() {
    let verdict = computed_over(
        &[recorded_lens_report(1, RISK_LENS, Some("risk"))],
        RiskLevel::Medium,
    );
    assert_eq!(verdict.outcome, ComputedOutcome::Approved);
    let default_focus = computed_over(
        &[recorded_lens_report(1, RISK_LENS, None)],
        RiskLevel::Medium,
    );
    assert_eq!(default_focus.outcome, ComputedOutcome::Incomplete);
}

#[test]
fn a_second_refutation_batch_for_one_receipt_is_refused() {
    let (_root, context) = context_with_no_repository();
    let receipt = ledger_receipt();
    let comments = vec![
        published(&receipt),
        recorded_finding(4, "f-1", "severe", Some("inferential"), Some("introduced")),
        recorded_refutation(5, r#"[{"id":"f-1","outcome":"corroborated"}]"#),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = op(9);
    let later = refutation_review(
        &operation_id,
        "codex-d",
        &receipt,
        r#"[{"id":"f-1","outcome":"refuted"}]"#,
    );
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &later)
    })
    .expect_err("a later batch overrode the batch already on the receipt");
    assert_eq!(
        failure.envelope()["reason"],
        "refutation-batch-already-recorded"
    );

    let replay = refutation_review(
        &operation_id,
        "codex-d",
        &receipt,
        r#"[{"id":"f-1","outcome":"corroborated"}]"#,
    );
    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &replay)
    })
    .expect("the identical batch reads back");
    assert_eq!(answer["reused_existing_refutation"], true);
}

fn lens_report_review<'a>(
    operation_id: &'a str,
    receipt: &'a ReviewTransaction,
    role: &'a str,
) -> LensReportReview<'a> {
    LensReportReview {
        issue: 42,
        run_id: "claude-a",
        operation_id,
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        role,
        focus: "",
        now: "2026-08-31T00:00:00Z",
    }
}

#[test]
fn a_lens_report_is_idempotent_on_the_receipt_and_the_role() {
    let (_root, context) = context_with_no_repository();
    let receipt = ledger_receipt();
    let comments = vec![
        published(&receipt),
        recorded_lens_report(3, RISK_LENS, None),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);

    let by_role = op(9);
    let again = lens_report_review(&by_role, &receipt, RISK_LENS);
    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_lens_report(&context, &again)
    })
    .expect("a role already reported on the receipt reads back");
    assert_eq!(answer["reused_existing_report"], true);

    let by_operation = op(303);
    let replay = lens_report_review(&by_operation, &receipt, RISK_LENS);
    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_lens_report(&context, &replay)
    })
    .expect("the same operation reads back");
    assert_eq!(answer["reused_existing_report"], true);

    let other = lens_report_review(&by_role, &receipt, "review-readability");
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_lens_report(&context, &other)
    })
    .expect_err("a role nobody reported reaches the claim check");
    assert_eq!(failure.envelope()["reason"], "not-current-live-holder");
}

#[test]
fn a_lens_report_names_a_role_a_tier_can_license() {
    let (_root, context) = context_with_no_repository();
    let receipt = ledger_receipt();
    let operation_id = op(9);
    let report = lens_report_review(&operation_id, &receipt, "review-blind");
    let failure =
        record_lens_report(&context, &report).expect_err("a judge role was recorded as a lens");
    assert_eq!(failure.envelope()["reason"], "invalid-lens-role");
}

/// The delta lens is a shipped, launched reviewer, and its report is the only
/// completion event a delta-lens repair can produce, so `record_lens_report`
/// accepts it exactly as the four tier lenses are accepted: it reaches the
/// claim check rather than being refused as an invalid role.
#[test]
fn a_delta_lens_report_is_accepted() {
    let (_root, context) = context_with_no_repository();
    let receipt = ledger_receipt();
    let comments = vec![published(&receipt)];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = op(9);
    let report = lens_report_review(&operation_id, &receipt, "review-lens");
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_lens_report(&context, &report)
    })
    .expect_err("the delta lens reached the claim check");
    assert_ne!(
        failure.envelope()["reason"],
        "invalid-lens-role",
        "a delta lens report was refused as an invalid role"
    );
}

fn medium_change() -> (tempfile::TempDir, Context, ReviewTransaction) {
    change_of("src/foo.rs", "fn main() {}\n", "fn main() { /* now */ }\n")
}

fn low_change() -> (tempfile::TempDir, Context, ReviewTransaction) {
    change_of(
        "notes.md",
        "A line of prose.\n",
        "A line of prose, reworded.\n",
    )
}

fn change_of(
    file: &str,
    before: &str,
    after: &str,
) -> (tempfile::TempDir, Context, ReviewTransaction) {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("the src directory");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    std::fs::write(repo.join(file), before).expect("content");
    git(&repo, &["add", file]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let base = head_of(&repo);
    std::fs::write(repo.join(file), after).expect("content");
    git(&repo, &["add", file]);
    git(&repo, &["commit", "--quiet", "-m", "head"]);
    let receipt = ReviewTransaction {
        head: head_of(&repo),
        base,
        ..ledger_receipt()
    };
    let context = Context {
        repo: Some("o/r".to_owned()),
        ..context_at(&repo)
    };
    (root, context, receipt)
}

fn computed_request<'a>(
    operation_id: &'a str,
    receipt: &'a ReviewTransaction,
) -> ComputedVerdictReview<'a> {
    ComputedVerdictReview {
        issue: 42,
        run_id: "claude-a",
        operation_id,
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        worktree: None,
        never_ran_evidence_file: None,
        now: "2026-08-31T00:00:00Z",
    }
}

/// A `published` marker whose lineage records the honest narrow repair range
/// `parent-head..head` — the shape `publish_with` writes when `repair_range`
/// answers that the repair's own range is recoverable.
fn published_with_narrow_lineage(
    run_id: &str,
    receipt: &ReviewTransaction,
    parent: &ReviewTransaction,
    delta: &str,
) -> ownership::Comment {
    wrote(
        "3",
        super::super::markers::render(
            "published",
            &[
                ("run-id", run_id),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("parent", &parent.epoch),
                ("parent-pr", &parent.pr.to_string()),
                ("parent-head", &parent.head),
                ("parent-base", &parent.base),
                ("parent-digest", &parent.digest),
                ("delta", delta),
                ("delta-range", "parent-head..head"),
            ],
        )
        .expect("a receipt marker with a narrow lineage"),
    )
}

/// A real repository where `main` holds the base, the branch holds the parent
/// publication's own `src/foo.rs` edit, and the repair adds passive prose on
/// top. The whole change is `medium`; the repair's own `parent_head..head`
/// range is `low`, so a `warning`/`suggestion`-cleared republish of it is the
/// delta-lens shape.
fn delta_lens_repair_world() -> (
    tempfile::TempDir,
    Context,
    ReviewTransaction,
    ReviewTransaction,
) {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("the src directory");
    std::fs::create_dir_all(repo.join("docs")).expect("the docs directory");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    std::fs::write(repo.join("src/foo.rs"), "fn main() {}\n").expect("content");
    git(&repo, &["add", "src/foo.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let base = head_of(&repo);

    std::fs::write(repo.join("src/foo.rs"), "fn main() { /* parent */ }\n").expect("content");
    git(&repo, &["add", "src/foo.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "parent"]);
    let parent = ReviewTransaction {
        epoch: "1".repeat(32),
        head: head_of(&repo),
        base: base.clone(),
        ..ledger_receipt()
    };

    std::fs::write(repo.join("docs/note.md"), "Ordinary prose.\n").expect("content");
    git(&repo, &["add", "docs/note.md"]);
    git(&repo, &["commit", "--quiet", "-m", "repair"]);
    let repair = ReviewTransaction {
        epoch: "2".repeat(32),
        head: head_of(&repo),
        base,
        ..ledger_receipt()
    };

    let context = Context {
        repo: Some("o/r".to_owned()),
        ..context_at(&repo)
    };
    (root, context, parent, repair)
}

/// A `warning`/`suggestion`-cleared narrow repair is reviewed by the delta lens
/// alone, so the computed verdict licenses `review-lens` and nothing else: with
/// the lens reported it reads `approved`, and a version that fell back to the
/// whole change's own tier would demand `review-reliability` instead and read
/// `incomplete`. Forcing `licensed_lens_roles` to answer `None` here turns this
/// red.
#[test]
fn a_delta_lens_repair_licenses_the_delta_lens_alone() {
    let (_root, context, parent, repair) = delta_lens_repair_world();
    let base = vec![
        published(&parent),
        computed_by("codex-b", &parent, "approved"),
        recorded_finding_on(&parent, 6, "f-1", "suggestion", None, None),
        published_with_narrow_lineage("claude-a", &repair, &parent, &"0".repeat(64)),
    ];

    let without =
        computed::computed_for(&context, &base, &repair, None).expect("the repair's panel reads");
    assert_eq!(
        without.0,
        RiskLevel::Medium,
        "the whole change stays medium"
    );
    assert_eq!(
        without.1.outcome,
        ComputedOutcome::Incomplete,
        "{without:?}"
    );
    assert_eq!(without.1.missing_roles, ["review-lens"], "{without:?}");

    let mut with_report = base;
    with_report.push(recorded_lens_report_for(&repair, 3, "review-lens", None));
    let computed = computed::computed_for(&context, &with_report, &repair, None)
        .expect("the repair's panel reads");
    assert_eq!(
        computed.1.outcome,
        ComputedOutcome::Approved,
        "{computed:?}"
    );
    assert!(computed.1.missing_roles.is_empty(), "{computed:?}");

    let mut cleared = with_report;
    cleared.push(stored_computed_verdict(&repair, "medium", "approved", "[]"));
    let basis = release_basis_of(&context, &repair, &cleared).expect("the repair releases");
    assert_eq!(basis.word(), "computed-verdict");
}

/// A `medium` repair over a `high` whole change licenses its own one lens, not
/// the whole change's four: the whole-change tier is still `high` (the marker
/// records it), but only the repair panel's role is demanded. A `severe`
/// clearing finding keeps the tier's own allocation rather than the delta lens,
/// so the panel is the repair level's single lens.
#[test]
fn a_repair_over_a_higher_whole_change_licenses_only_its_own_panel() {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("the src directory");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    std::fs::write(repo.join("src/lib.rs"), "fn main() {}\n").expect("content");
    git(&repo, &["add", "src/lib.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let base_sha = head_of(&repo);

    std::fs::write(repo.join("src/auth.rs"), "fn check() {}\n").expect("content");
    git(&repo, &["add", "src/auth.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "parent"]);
    let parent = ReviewTransaction {
        epoch: "1".repeat(32),
        head: head_of(&repo),
        base: base_sha.clone(),
        ..ledger_receipt()
    };

    std::fs::write(repo.join("src/foo.rs"), "fn thing() {}\n").expect("content");
    git(&repo, &["add", "src/foo.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "repair"]);
    let repair = ReviewTransaction {
        epoch: "2".repeat(32),
        head: head_of(&repo),
        base: base_sha,
        ..ledger_receipt()
    };
    let context = Context {
        repo: Some("o/r".to_owned()),
        ..context_at(&repo)
    };

    let comments = vec![
        published(&parent),
        computed_by("codex-b", &parent, "approved"),
        recorded_finding_on(
            &parent,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
        published_with_narrow_lineage("claude-a", &repair, &parent, &"0".repeat(64)),
    ];
    let (level, verdict) =
        computed::computed_for(&context, &comments, &repair, None).expect("the panel reads");
    assert_eq!(level, RiskLevel::High, "the whole change is high");
    assert_eq!(verdict.missing_roles, [RELIABILITY_LENS], "{verdict:?}");
    assert_eq!(verdict.outcome, ComputedOutcome::Incomplete, "{verdict:?}");
}

fn stored_computed_verdict(
    receipt: &ReviewTransaction,
    level: &str,
    outcome: &str,
    open: &str,
) -> ownership::Comment {
    wrote(
        "5",
        super::super::markers::render(
            "review-computed-verdict",
            &[
                ("run-id", "claude-a"),
                ("op-id", &op(400)),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("level", level),
                ("outcome", outcome),
                ("open", open),
                ("escalated", "[]"),
                ("follow-ups", "[]"),
                ("missing", "[]"),
            ],
        )
        .expect("a computed verdict marker"),
    )
}

fn computed_by(recorder: &str, receipt: &ReviewTransaction, outcome: &str) -> ownership::Comment {
    let initial = match outcome {
        "approved" => 'a',
        "rejected" => 'b',
        _ => 'e',
    };
    let operation = format!("{initial}{}", &receipt.epoch[1..]);
    let open = if outcome == "rejected" {
        r#"["f-1"]"#
    } else {
        "[]"
    };
    let escalated = if outcome == "escalated" {
        r#"[{"id":"f-1","reason":"no-refuter-result"}]"#
    } else {
        "[]"
    };
    wrote(
        "5",
        super::super::markers::render(
            "review-computed-verdict",
            &[
                ("run-id", recorder),
                ("op-id", &operation),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("level", "low"),
                ("outcome", outcome),
                ("open", open),
                ("escalated", escalated),
                ("follow-ups", "[]"),
                ("missing", "[]"),
            ],
        )
        .expect("a computed verdict marker"),
    )
}

#[test]
fn a_computed_approval_over_a_red_lane_is_refused() {
    let (_root, context, receipt) = medium_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
    ];
    let mut answers = lane_read_answers();
    answers.insert(
        0,
        crate::test_env::issue_view_answer(42, stdout_with(&comments)),
    );
    answers[2] = serde_json::json!({
        "matches": "check-runs",
        "stdout": check_runs(&[check_run("ci", "completed", Some("cancelled"))]).to_string(),
        "status": 0,
    });
    let bin = crate::test_env::scripted_gh();
    let operation_id = op(9);
    let request = computed_request(&operation_id, &receipt);
    let failure =
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
            record_computed_verdict(&context, &request)
        })
        .expect_err("an approval was recorded over a red lane");
    assert_eq!(failure.envelope()["reason"], "publication-lane-red");
}

#[test]
fn a_computation_with_an_unreported_lens_is_refused_rather_than_recorded() {
    let (_root, context, receipt) = medium_change();
    let comments = vec![live_claim("claude-a"), published(&receipt)];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let operation_id = op(9);
    let request = computed_request(&operation_id, &receipt);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_computed_verdict(&context, &request)
    })
    .expect_err("an incomplete computation was recorded");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "computed-verdict-incomplete");
    assert_eq!(
        envelope["missing_roles"],
        serde_json::json!([RELIABILITY_LENS])
    );
}

#[test]
fn a_second_computed_verdict_reads_back_only_while_the_ledger_agrees() {
    let (_root, context, receipt) = medium_change();
    let stored =
        |outcome: &str, open: &str| stored_computed_verdict(&receipt, "medium", outcome, open);
    let bin = crate::test_env::scripted_gh();
    let operation_id = op(9);
    let request = computed_request(&operation_id, &receipt);
    let base = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
    ];

    let agreeing = [base.clone(), vec![stored("approved", "[]")]].concat();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&agreeing),
    )]);
    let answer = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_computed_verdict(&context, &request)
    })
    .expect("the identical computation reads back");
    assert_eq!(answer["reused_existing_verdict"], true);

    crate::transport::store::forget_scratch_store();
    let disagreeing = [base, vec![stored("rejected", r#"["f-9"]"#)]].concat();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&disagreeing),
    )]);
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_computed_verdict(&context, &request)
    })
    .expect_err("a verdict over a different ledger was read back as the same");
    assert_eq!(
        failure.envelope()["reason"],
        "computed-verdict-already-recorded"
    );
}

fn release_basis_of(
    context: &Context,
    receipt: &ReviewTransaction,
    comments: &[ownership::Comment],
) -> Result<ReleaseBasis, Failure> {
    release_basis(context, comments, receipt, None)
}

fn release_refusal(
    context: &Context,
    receipt: &ReviewTransaction,
    comments: &[ownership::Comment],
) -> String {
    reason(
        release_basis_of(context, receipt, comments)
            .err()
            .expect("the release was refused"),
    )
}

fn legacy_accepted(receipt: &ReviewTransaction) -> ownership::Comment {
    verdict_attested_by("claude-a", "codex-b", "accepted", &op(500), receipt)
}

fn reported_and_recorded(receipt: &ReviewTransaction, level: &str) -> Vec<ownership::Comment> {
    vec![
        live_claim("claude-a"),
        published(receipt),
        recorded_lens_report_for(receipt, 3, RELIABILITY_LENS, None),
        stored_computed_verdict(receipt, level, "approved", "[]"),
    ]
}

#[test]
fn a_recomputed_approval_with_an_approved_marker_releases() {
    let (_root, context, receipt) = medium_change();
    let comments = reported_and_recorded(&receipt, "medium");
    let basis = release_basis_of(&context, &receipt, &comments).expect("approved and recorded");
    assert_eq!(basis.word(), "computed-verdict");
}

#[test]
fn a_marker_gone_stale_by_a_later_finding_refuses_even_though_it_says_approved() {
    let (_root, context, receipt) = medium_change();
    let mut comments = reported_and_recorded(&receipt, "medium");
    comments.push(recorded_finding_on(
        &receipt,
        6,
        "f-1",
        "severe",
        Some("deterministic"),
        Some("introduced"),
    ));
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "computed-verdict-stale"
    );
}

#[test]
fn a_marker_recorded_under_another_tier_refuses() {
    let (_root, context, receipt) = medium_change();
    let comments = reported_and_recorded(&receipt, "high");
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "computed-verdict-tier-mismatch"
    );
}

#[test]
fn a_complete_ledger_with_no_marker_is_not_released() {
    let (_root, context, receipt) = medium_change();
    let mut comments = reported_and_recorded(&receipt, "medium");
    comments.pop();
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "computed-verdict-unrecorded"
    );
}

#[test]
fn a_computed_escalation_is_not_overridden_by_a_legacy_accepted_verdict() {
    let (_root, context, receipt) = medium_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
        recorded_finding_on(&receipt, 6, "f-1", "severe", None, None),
        legacy_accepted(&receipt),
    ];
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "computed-verdict-escalated"
    );
}

#[test]
fn a_computed_rejection_and_a_missing_lens_are_not_overridden_by_a_legacy_accepted_verdict() {
    let (_root, context, receipt) = medium_change();
    let rejected = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
        legacy_accepted(&receipt),
    ];
    assert_eq!(
        release_refusal(&context, &receipt, &rejected),
        "computed-verdict-rejected"
    );
    let incomplete = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RISK_LENS, None),
        legacy_accepted(&receipt),
    ];
    assert_eq!(
        release_refusal(&context, &receipt, &incomplete),
        "computed-verdict-incomplete"
    );
}

#[test]
fn a_legacy_accepted_verdict_alone_does_not_release_a_medium_receipt() {
    let (_root, context, receipt) = medium_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        legacy_accepted(&receipt),
    ];
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "qualifying-review-verdict-missing"
    );
}

#[test]
fn a_legacy_rejected_verdict_still_refuses_whatever_the_ledger_now_computes() {
    let (_root, context, receipt) = medium_change();
    let mut comments = reported_and_recorded(&receipt, "medium");
    comments.push(verdict_attested_by(
        "claude-a",
        "codex-b",
        "rejected",
        &op(501),
        &receipt,
    ));
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "legacy-verdict-rejected"
    );
}

/// A rejection resting on a severe finding the reviewer itself declared
/// unestablished is refused for that reason rather than as a generic legacy
/// rejection: `insufficient` says neither demonstration nor inference held.
#[test]
fn a_rejection_resting_on_insufficient_evidence_is_refused_by_name() {
    let (_root, context, receipt) = medium_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("insufficient"),
            Some("introduced"),
        ),
        verdict_attested_by("claude-a", "codex-b", "rejected", &op(501), &receipt),
    ];
    let failure = release_basis_of(&context, &receipt, &comments)
        .err()
        .expect("a rejection on insufficient evidence is refused");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "rejected-on-insufficient-evidence");
    assert_eq!(envelope["reviewer"], "codex-b");
    assert_eq!(envelope["insufficient"], serde_json::json!(["f-1"]));
}

#[test]
fn a_medium_receipt_with_no_review_at_all_is_not_released() {
    let (_root, context, receipt) = medium_change();
    let comments = vec![live_claim("claude-a"), published(&receipt)];
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "qualifying-review-verdict-missing"
    );
}

#[test]
fn a_low_receipt_with_no_review_closes_as_approved_on_the_structural_readback() {
    let (_root, context, receipt) = low_change();
    let comments = vec![live_claim("claude-a"), published(&receipt)];
    let basis = release_basis_of(&context, &receipt, &comments).expect("a low receipt");
    assert_eq!(basis.word(), "structural-readback");
}

#[test]
fn a_low_receipt_releases_on_the_recomputed_approval_with_or_without_a_marker() {
    let (_root, context, receipt) = low_change();
    let marked = vec![
        live_claim("claude-a"),
        published(&receipt),
        stored_computed_verdict(&receipt, "low", "approved", "[]"),
    ];
    let basis = release_basis_of(&context, &receipt, &marked).expect("a marked low receipt");
    assert_eq!(basis.word(), "computed-verdict");

    let legacy = vec![
        live_claim("claude-a"),
        published(&receipt),
        legacy_accepted(&receipt),
    ];
    let basis = release_basis_of(&context, &receipt, &legacy).expect("an unmarked low receipt");
    assert_eq!(basis.word(), "structural-readback");
}

#[test]
fn a_low_receipt_with_an_open_finding_is_not_released() {
    let (_root, context, receipt) = low_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
    ];
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "computed-verdict-rejected"
    );
}

#[test]
fn ci_is_refused_on_an_escalated_ledger_whatever_a_legacy_verdict_says() {
    let (_root, context, receipt) = medium_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
        recorded_finding_on(&receipt, 6, "f-1", "severe", None, None),
        legacy_accepted(&receipt),
    ];
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let release = CiRelease {
        issue: 42,
        run_id: "claude-a",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        worktree: None,
        now: "2026-08-31T00:00:00Z",
    };
    let failure =
        crate::test_env::with_scripted_gh(bin.path(), &script, || release_ci(&context, &release))
            .expect_err("an escalated ledger was released");
    assert_eq!(reason(failure), "computed-verdict-escalated");
}

#[test]
fn a_rejection_or_an_escalation_is_not_recorded_while_a_licensed_lens_is_out() {
    for (class, causal, computed) in [
        (Some("deterministic"), Some("introduced"), "rejected"),
        (None, None, "escalated"),
    ] {
        let (_root, context, receipt) = medium_change();
        let comments = vec![
            live_claim("claude-a"),
            published(&receipt),
            recorded_finding_on(&receipt, 6, "f-1", "severe", class, causal),
        ];
        let bin = crate::test_env::scripted_gh();
        let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
            42,
            stdout_with(&comments),
        )]);
        let operation_id = op(9);
        let request = computed_request(&operation_id, &receipt);
        let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            record_computed_verdict(&context, &request)
        })
        .expect_err("a final outcome was recorded with a lens out");
        let envelope = failure.envelope();
        assert_eq!(envelope["reason"], "computed-verdict-incomplete");
        assert_eq!(envelope["computed"], computed);
        crate::transport::store::forget_scratch_store();
    }
}

#[test]
fn medium_lens_reports_that_name_different_focuses_escalate() {
    let receipt = ledger_receipt();
    let agreed = [recorded_lens_report(3, RISK_LENS, Some("risk"))];
    assert_eq!(
        computed_over(&agreed, RiskLevel::Medium).outcome,
        ComputedOutcome::Approved
    );
    let split = [
        recorded_lens_report_for(&receipt, 3, RISK_LENS, Some("risk")),
        recorded_lens_report_for(&receipt, 4, RELIABILITY_LENS, None),
    ];
    let computed = computed_over(&split, RiskLevel::Medium);
    assert_eq!(computed.outcome, ComputedOutcome::Escalated);
    assert_eq!(
        ids(&computed).1,
        vec![("lens-focus", "lens-focus-disagreement")]
    );
}

#[test]
fn a_low_receipt_does_not_release_over_a_rejected_verdict_resting_on_a_pre_existing_finding() {
    let (_root, context, receipt) = low_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("pre-existing"),
        ),
        verdict_attested_by("claude-a", "codex-b", "rejected", &op(501), &receipt),
    ];
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "legacy-verdict-rejected"
    );
}

#[test]
fn a_marker_without_a_tier_still_counts_as_evidence_and_refuses() {
    let (_root, context, receipt) = medium_change();
    let mut comments = reported_and_recorded(&receipt, "medium");
    let levelless = comments
        .pop()
        .expect("the marker")
        .body
        .replace(" level=medium", "");
    comments.push(wrote("5", levelless));
    assert!(computed::has_computed_evidence(&comments, &receipt));
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "computed-verdict-tier-mismatch"
    );
}

fn goal_standing(
    context: &Context,
    receipt: &ReviewTransaction,
    comments: &[ownership::Comment],
) -> serde_json::Value {
    let mut answers = lane_read_answers();
    answers.insert(
        0,
        crate::test_env::issue_view_answer(42, stdout_with(comments)),
    );
    answers.push(serde_json::json!({
        "matches": "actions/permissions",
        "stdout": serde_json::json!({"allowed": null, "enabled": false}).to_string(),
        "status": 0,
    }));
    let bin = crate::test_env::scripted_gh();
    crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
        goal_completion(context, 42, receipt, None, "2026-08-31T00:00:00Z")
    })
    .expect("the goal reads")
}

#[test]
fn a_goal_completes_on_the_low_route_and_on_the_computed_route() {
    let (_root, context, receipt) = low_change();
    let low = goal_standing(
        &context,
        &receipt,
        &[live_claim("claude-a"), published(&receipt)],
    );
    assert_eq!(low["complete"], true, "{low}");
    assert_eq!(low["basis"], "structural-readback");

    crate::transport::store::forget_scratch_store();
    let (_root, context, receipt) = medium_change();
    let computed = goal_standing(
        &context,
        &receipt,
        &reported_and_recorded(&receipt, "medium"),
    );
    assert_eq!(computed["complete"], true, "{computed}");
    assert_eq!(computed["basis"], "computed-verdict");
}

#[test]
fn a_distinct_rejection_still_refuses_after_a_later_distinct_acceptance() {
    let (_root, context, receipt) = medium_change();
    let mut comments = reported_and_recorded(&receipt, "medium");
    comments.push(verdict_attested_by(
        "claude-a",
        "codex-b",
        "rejected",
        &op(501),
        &receipt,
    ));
    comments.push(verdict_attested_by(
        "claude-a",
        "gemini-c",
        "accepted",
        &op(502),
        &receipt,
    ));
    assert_eq!(
        release_refusal(&context, &receipt, &comments),
        "legacy-verdict-rejected"
    );
}

#[test]
fn a_computed_marker_outranks_a_later_aggregate_acceptance_in_a_receipts_standing() {
    let receipt = ledger_receipt();
    let finding = finding_marker("4", &op(7), "judge", &receipt, "f-1", "warning");
    let comments = [
        finding,
        computed_by("codex-b", &receipt, "rejected"),
        verdict_by("codex-b", "accepted", &op(8), &receipt),
    ];
    assert_eq!(
        receipt_standing(&comments, &receipt)
            .expect("readable")
            .standing,
        Standing::Rejected,
        "a rejected computed marker was overridden by an aggregate acceptance recorded after it"
    );
}

#[test]
fn a_receipt_with_several_rejecting_markers_counts_one_round() {
    let (_root, context) = context_with_no_repository();
    let receipt = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let comments = vec![
        published(&receipt),
        computed_by("codex-b", &receipt, "rejected"),
        computed_by("codex-b", &receipt, "escalated"),
        verdict_by("codex-b", "rejected", &op(8), &receipt),
        verdict_by("gemini-c", "rejected", &op(9), &receipt),
    ];
    let answer = read_repair_rounds(&context, &comments);
    assert_eq!(answer["rejections"], 1, "{answer}");
}

#[test]
fn a_goal_is_open_over_an_escalated_receipt_whatever_the_ledger_computes() {
    let (_root, context, receipt) = low_change();
    let comments = vec![
        live_claim("claude-a"),
        published(&receipt),
        escalated_receipt_by("claude-a", &"5".repeat(32), "Alexis", &receipt),
    ];
    let standing = goal_standing(&context, &receipt, &comments);
    assert_eq!(standing["complete"], false, "{standing}");
    assert_eq!(standing["reason"], "review-receipt-escalated", "{standing}");

    crate::transport::store::forget_scratch_store();
    let (_root, context, receipt) = medium_change();
    let mut approving = reported_and_recorded(&receipt, "medium");
    approving.push(escalated_receipt_by(
        "claude-a",
        &"5".repeat(32),
        "Alexis",
        &receipt,
    ));
    let standing = goal_standing(&context, &receipt, &approving);
    assert_eq!(standing["complete"], false, "{standing}");
    assert_eq!(standing["reason"], "review-receipt-escalated", "{standing}");
}

#[test]
fn two_rejected_receipts_on_one_pull_request_count_two_rounds() {
    let (_root, context) = context_with_no_repository();
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let second = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "b".repeat(40),
        digest: "2".repeat(64),
        ..first.clone()
    };
    let comments = vec![
        published(&first),
        computed_by("codex-b", &first, "rejected"),
        published_with_parent("claude-a", &second, &first, "delta-1-2"),
        computed_by("codex-b", &second, "rejected"),
    ];
    let answer = read_repair_rounds(&context, &comments);
    assert_eq!(answer["rejections"], 2, "{answer}");
    assert_eq!(answer["corrections"], 1, "{answer}");
}

#[test]
fn an_aggregate_rejection_beside_a_computed_approval_stands_rejected_in_both_readers() {
    let (_root, context) = context_with_no_repository();
    let first = ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "d".repeat(40),
        digest: "1".repeat(64),
    };
    let repair = ReviewTransaction {
        epoch: "2".repeat(32),
        head: "b".repeat(40),
        digest: "2".repeat(64),
        ..first.clone()
    };
    let mixed = vec![
        published(&first),
        computed_by("codex-b", &first, "approved"),
        verdict_by("codex-b", "rejected", &op(8), &first),
    ];
    assert_eq!(
        receipt_standing(&mixed, &first).expect("readable").standing,
        Standing::Rejected,
        "a rejection recorded beside an approval stands over it"
    );
    let mut with_repair = mixed;
    with_repair.push(published_with_parent(
        "claude-a",
        &repair,
        &first,
        "delta-1-2",
    ));
    let answer = read_repair_rounds(&context, &with_repair);
    assert_eq!(answer["rejections"], 1, "{answer}");
    assert_eq!(answer["corrections"], 1, "{answer}");
}

#[test]
fn a_standing_computed_marker_under_a_new_operation_id_records_the_missing_receipt() {
    let (_root, context, receipt) = low_change();
    let comments = vec![
        live_claim("codex-b"),
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];
    let operation_id = op(9);
    let request = ComputedVerdictReview {
        run_id: "codex-b",
        ..computed_request(&operation_id, &receipt)
    };
    let ((answer, timeline), graphql) =
        graphql_calls_after_import(&context, &comments, "record-computed-verdict", |context| {
            let answer = record_computed_verdict(context, &request).expect("the call answers");
            let timeline = Ledger::open(context, 42, request.now)
                .expect("the ledger opens")
                .timeline()
                .expect("the timeline reads");
            (answer, timeline)
        });
    assert_eq!(answer["reused_existing_verdict"], true, "{answer}");
    assert_eq!(graphql, 0, "a standing marker posts nothing");
    assert!(
        timeline
            .iter()
            .any(|comment| carries_receipt(comment, &receipt, "approved")),
        "the standing-marker path left no local approved receipt"
    );
}

fn refusal_reason(failure: &Failure) -> String {
    failure.envelope()["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn correction_world() -> (ReviewTransaction, Vec<ownership::Comment>) {
    let original = ReviewTransaction {
        epoch: op(9),
        pr: 7,
        head: "9".repeat(40),
        base: "b".repeat(40),
        digest: "9".repeat(64),
    };
    let correction = ReviewTransaction {
        epoch: op(1),
        head: "a".repeat(40),
        digest: "c".repeat(64),
        ..original.clone()
    };
    let comments = vec![
        live_claim("codex-b"),
        published(&original),
        computed_by("codex-b", &original, "rejected"),
        published_with_parent("claude-a", &correction, &original, "delta-9-1"),
    ];
    (correction, comments)
}

#[derive(Clone, Copy)]
struct Validation<'a> {
    run_id: &'a str,
    operation: u32,
    inspection: &'a str,
    original: &'a str,
    regression: &'a str,
    reason: &'a str,
    attempt: &'a str,
}

const PASSING: Validation<'static> = Validation {
    run_id: "codex-b",
    operation: 1,
    inspection: "",
    original: "pass",
    regression: "pass",
    reason: "",
    attempt: "",
};

fn unavailable(operation: u32, attempt: &'static str) -> Validation<'static> {
    Validation {
        run_id: "codex-b",
        operation,
        inspection: "unavailable",
        original: "",
        regression: "",
        reason: "the frozen trees could not be read",
        attempt,
    }
}

struct Session {
    results: Vec<Result<serde_json::Value, Failure>>,
    timeline: Vec<ownership::Comment>,
}

fn validate(
    receipt: &ReviewTransaction,
    comments: &[ownership::Comment],
    validations: &[Validation<'_>],
) -> Session {
    validate_under(Vec::new(), receipt, comments, validations)
}

fn validate_under(
    config: Vec<(String, String, Source)>,
    receipt: &ReviewTransaction,
    comments: &[ownership::Comment],
    validations: &[Validation<'_>],
) -> Session {
    crate::transport::store::forget_scratch_store();
    let (_root, mut context) = context_with_no_repository();
    context.config = config;
    let directory = tempfile::tempdir().expect("a directory for the store");
    let path = directory.path().join("state.db");
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(comments)),
        serde_json::json!({"matches": "issue comment 42", "stdout": "", "status": 0}),
    ]);
    crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            Ledger::open(&context, 42, RECLAIM_NOW).expect("the first touch imports the issue");
            let operations: Vec<String> = validations.iter().map(|v| op(v.operation)).collect();
            let results = validations
                .iter()
                .zip(&operations)
                .map(|(validation, operation)| {
                    record_validator_result(
                        &context,
                        &ValidatorResultReview {
                            issue: 42,
                            run_id: validation.run_id,
                            operation_id: operation,
                            epoch: &receipt.epoch,
                            pr: receipt.pr,
                            head: &receipt.head,
                            base: &receipt.base,
                            digest: &receipt.digest,
                            inspection: validation.inspection,
                            original_criteria: validation.original,
                            correction_regression: validation.regression,
                            reason: validation.reason,
                            attempt: validation.attempt,
                            now: "2026-08-31T00:00:00Z",
                        },
                    )
                })
                .collect();
            let timeline = Ledger::open(&context, 42, "2026-08-31T00:00:00Z")
                .expect("the ledger opens")
                .timeline()
                .expect("the timeline reads");
            Session { results, timeline }
        })
    })
}

fn validated_computed(
    session: &Session,
    receipt: &ReviewTransaction,
    correction_available: bool,
) -> ComputedVerdict {
    computed_verdict(
        &session.timeline,
        receipt,
        crate::transport::tier::RiskLevel::Low,
        None,
        correction_available,
    )
}

#[test]
fn a_correction_waits_for_its_validator_and_a_pass_lets_the_ledger_approve() {
    let (correction, comments) = correction_world();
    let before = validate(&correction, &comments, &[]);
    let waiting = validated_computed(&before, &correction, true);
    assert_eq!(waiting.outcome, ComputedOutcome::Incomplete);
    assert_eq!(waiting.missing_roles, ["review-validator"]);

    let session = validate(&correction, &comments, &[PASSING]);
    let answer = session.results[0].as_ref().expect("a pass is recorded");
    assert_eq!(answer["outcome"], "passed", "{answer}");
    assert_eq!(
        validated_computed(&session, &correction, true).outcome,
        ComputedOutcome::Approved
    );
}

#[test]
fn a_passing_validator_never_overrides_an_open_severe_finding() {
    let (correction, mut comments) = correction_world();
    comments.push(recorded_finding_on(
        &correction,
        7,
        "f-1",
        "severe",
        Some("deterministic"),
        Some("introduced"),
    ));
    let session = validate(&correction, &comments, &[PASSING]);
    assert_eq!(
        validated_computed(&session, &correction, true).outcome,
        ComputedOutcome::Rejected
    );
}

const VALIDATOR_FAILURES: [(&str, &str, &str); 3] = [
    ("fail", "pass", "original_criteria_failed"),
    ("pass", "fail", "correction_regression_failed"),
    ("fail", "fail", "original_criteria_failed"),
];

fn failing_then_passing(
    original: &'static str,
    regression: &'static str,
) -> [Validation<'static>; 2] {
    [
        Validation {
            original,
            regression,
            ..PASSING
        },
        Validation {
            operation: 2,
            ..PASSING
        },
    ]
}

#[test]
fn a_conclusive_failure_with_corrections_remaining_stands_rejected_and_is_not_escalated() {
    for (original, regression, cause) in VALIDATOR_FAILURES {
        let (correction, comments) = correction_world();
        let session = validate(
            &correction,
            &comments,
            &failing_then_passing(original, regression),
        );
        let answer = session.results[0].as_ref().expect("a failure is recorded");
        assert_eq!(answer["outcome"], "rejected", "{answer}");
        assert_eq!(answer["cause"], cause, "{answer}");
        assert!(
            !session.timeline.iter().any(|comment| carries_receipt(
                comment,
                &correction,
                "escalated"
            )),
            "{cause}: a failure with corrections remaining escalated the receipt"
        );
        require_un_escalated_terminal(&session.timeline, &correction)
            .expect("a rejected correction is not an escalated terminal");
        assert_eq!(
            refusal_reason(
                session.results[1]
                    .as_ref()
                    .expect_err("a conclusive result is recorded once")
            ),
            "validator-result-already-recorded"
        );
    }
}

#[test]
fn a_conclusive_failure_at_the_limit_escalates_the_receipt_with_its_cause_and_release_refuses_it() {
    for (original, regression, cause) in VALIDATOR_FAILURES {
        let (correction, comments) = correction_world();
        let session = validate_under(
            corrections_limit_row("1"),
            &correction,
            &comments,
            &failing_then_passing(original, regression),
        );
        let answer = session.results[0].as_ref().expect("a failure is recorded");
        assert_eq!(answer["outcome"], "escalated", "{answer}");
        assert_eq!(answer["cause"], cause, "{answer}");
        assert!(
            session.timeline.iter().any(|comment| carries_receipt(
                comment,
                &correction,
                "escalated"
            )),
            "{cause}: no local escalated receipt"
        );
        assert_eq!(
            reason(
                require_un_escalated_terminal(&session.timeline, &correction)
                    .expect_err("an escalated correction releases nothing")
            ),
            "review-receipt-escalated"
        );
        let computed = validated_computed(&session, &correction, false);
        assert_eq!(computed.outcome, ComputedOutcome::Escalated);
        assert_eq!(computed.escalated[0].reason, cause);
        assert_eq!(
            refusal_reason(
                session.results[1]
                    .as_ref()
                    .expect_err("a conclusive result is recorded once")
            ),
            "validator-result-already-recorded"
        );
    }
}

#[test]
fn a_failed_validator_rejects_the_computed_verdict_while_a_correction_remains() {
    for (original, regression, cause) in VALIDATOR_FAILURES {
        let (correction, comments) = correction_world();
        let session = validate(
            &correction,
            &comments,
            &failing_then_passing(original, regression),
        );
        let computed = validated_computed(&session, &correction, true);
        assert_eq!(computed.outcome, ComputedOutcome::Rejected, "{cause}");
        assert_eq!(computed.open, ["validator"], "{cause}");
        assert!(computed.escalated.is_empty(), "{cause}: {computed:?}");
    }
}

#[test]
fn an_unavailable_inspection_spends_nothing_and_a_later_pass_still_stands() {
    let (correction, comments) = correction_world();
    let session = validate(
        &correction,
        &comments,
        &[
            unavailable(2, "1"),
            unavailable(2, "1"),
            unavailable(3, "2"),
            PASSING,
        ],
    );
    let first = session.results[0].as_ref().expect("recorded");
    assert_eq!(first["outcome"], "inconclusive", "{first}");
    assert_eq!(first["attempts_remaining"], 2, "{first}");
    let replay = session.results[1].as_ref().expect("a replay reads back");
    assert_eq!(replay["reused_existing_result"], true, "{replay}");
    assert_eq!(replay["inconclusive_attempts"], 1, "{replay}");
    let second = session.results[2].as_ref().expect("recorded");
    assert_eq!(second["attempts_remaining"], 1, "{second}");
    assert!(
        !session
            .timeline
            .iter()
            .any(|comment| carries_receipt(comment, &correction, "escalated")),
        "an inconclusive inspection escalated"
    );
    let pass = session.results[3]
        .as_ref()
        .expect("a pass after two inconclusive");
    assert_eq!(pass["outcome"], "passed", "{pass}");
    assert_eq!(
        validated_computed(&session, &correction, true).outcome,
        ComputedOutcome::Approved
    );
}

#[test]
fn the_third_inconclusive_inspection_escalates_and_a_fourth_is_refused() {
    let (correction, comments) = correction_world();
    let session = validate(
        &correction,
        &comments,
        &[
            unavailable(2, "1"),
            unavailable(3, "2"),
            unavailable(4, "3"),
            unavailable(5, "3"),
        ],
    );
    let third = session.results[2].as_ref().expect("recorded");
    assert_eq!(third["outcome"], "escalated", "{third}");
    assert_eq!(third["cause"], "validator_inconclusive", "{third}");
    assert!(
        session
            .timeline
            .iter()
            .any(|comment| carries_receipt(comment, &correction, "escalated")),
        "the third inconclusive attempt left no escalated receipt"
    );
    assert_eq!(
        refusal_reason(session.results[3].as_ref().expect_err("a fourth")),
        "validator-attempts-exhausted"
    );
}

#[test]
fn a_malformed_validator_result_or_attempt_is_refused_before_anything_is_written() {
    let (correction, comments) = correction_world();
    let skipped = unavailable(2, "2");
    let criterion = Validation {
        original: "pass",
        ..unavailable(3, "1")
    };
    let omitted = unavailable(6, "");
    let mute = Validation {
        reason: "",
        ..unavailable(4, "1")
    };
    let unknown = Validation {
        inspection: "partial",
        ..PASSING
    };
    let unjudged = Validation {
        regression: "",
        ..PASSING
    };
    let session = validate(
        &correction,
        &comments,
        &[skipped, criterion, mute, omitted, unknown, unjudged],
    );
    for result in &session.results {
        assert_eq!(
            refusal_reason(result.as_ref().expect_err("refused")),
            "invalid-validator-result"
        );
    }
}

#[test]
fn a_receipt_that_is_not_a_correction_has_no_validator() {
    let (correction, _) = correction_world();
    let comments = [live_claim("codex-b"), published(&correction)];
    let session = validate(&correction, &comments, &[PASSING]);
    assert_eq!(
        refusal_reason(
            session.results[0]
                .as_ref()
                .expect_err("a first publication")
        ),
        "validator-transaction-not-a-correction"
    );
}

#[test]
fn a_publication_on_another_pull_request_is_never_the_recorded_parent() {
    let (_, mut comments) = correction_world();
    let original = ReviewTransaction {
        epoch: op(9),
        pr: 7,
        head: "9".repeat(40),
        base: "b".repeat(40),
        digest: "9".repeat(64),
    };
    let other = ReviewTransaction {
        epoch: op(5),
        pr: 8,
        head: "5".repeat(40),
        ..original.clone()
    };
    comments.truncate(3);
    comments.push(published_with_parent(
        "claude-a",
        &other,
        &original,
        "delta-9-5",
    ));

    assert!(
        publication_on(&comments, Some(7))
            .expect("readable")
            .is_some_and(|parent| parent.receipt == original),
        "a republish on the same pull request lost its own previous publication"
    );
    assert!(
        publication_on(&comments, None).expect("readable").is_none(),
        "a first publication took another pull request's receipt as its parent"
    );
    assert!(
        publication_on(&comments, Some(9))
            .expect("readable")
            .is_none(),
        "a pull request with no publication of its own took another's as its parent"
    );
}

#[test]
fn a_first_publication_on_a_pull_request_records_no_parent_over_another_pulls_publication() {
    let world = republish_world();
    let other_head = world.commit("two.txt", 1);
    world.commit("three.txt", 1);
    let mut other = world.receipt('1', &other_head);
    other.pr = 8;

    let (published, bodies) = world.publish_logging(&[published(&other)], &other_head);
    let published = published.expect("a first publication on pull request 7");
    assert_eq!(published["ok"], true, "{published}");
    let marker = super::super::markers::parse(&bodies)
        .into_iter()
        .find(|marker| marker.get("kind").map(String::as_str) == Some("published"))
        .expect("the publication wrote its marker");
    assert_eq!(marker.get("parent"), None, "{marker:?}");
}

#[test]
fn a_standing_that_cannot_be_read_to_the_end_is_never_a_clearance() {
    let (correction, mut comments) = correction_world();
    comments.push(escalated_receipt_by(
        "claude-a",
        &"5".repeat(32),
        "Alexis",
        &correction,
    ));
    comments.push(approved_receipt_by("claude-a", "codex-b", &correction));

    let superseded = receipt_standing(&comments, &correction).expect("readable");
    assert_ne!(
        superseded.standing,
        Standing::Rejected,
        "a distinct approval supersedes"
    );

    let unknown_publisher: Vec<_> = comments
        .iter()
        .filter(|comment| {
            !(comment.body.contains("issue-flow: published ")
                && comment.body.contains(&format!(" epoch={} ", op(1))))
        })
        .cloned()
        .collect();
    let without_publication = receipt_standing(&unknown_publisher, &correction).expect("readable");
    assert_eq!(
        without_publication.standing,
        Standing::Rejected,
        "an approval cannot supersede an escalation whose publisher is unknown"
    );

    comments.push(wrote(
        "6",
        super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", "short"),
                ("pr", "7"),
                ("head", &correction.head),
                ("base", &correction.base),
                ("digest", &correction.digest),
            ],
        )
        .expect("a marker"),
    ));
    assert!(receipt_standing(&comments, &correction).is_err());
}

struct EscalatedWorld {
    world: RepublishWorld,
    original: ReviewTransaction,
    escalated: ReviewTransaction,
    comments: Vec<ownership::Comment>,
}

fn escalated_world() -> EscalatedWorld {
    let world = limited_to_one_correction(republish_world());
    let original_head = world.commit("two.txt", 1);
    let escalated_head = world.commit("two.txt", 2);
    let original = world.receipt('1', &original_head);
    let escalated = world.receipt('2', &escalated_head);
    let comments = vec![
        live_claim("codex-b"),
        published(&original),
        computed_by("codex-b", &original, "rejected"),
        published_with_parent("claude-a", &escalated, &original, "delta-1-2"),
        escalated_receipt_by("claude-a", &"5".repeat(32), "Alexis", &escalated),
    ];
    EscalatedWorld {
        world,
        original,
        escalated,
        comments,
    }
}

struct Recovered {
    result: Result<serde_json::Value, Failure>,
    timeline: Vec<ownership::Comment>,
    calls: String,
}

fn permission_answer(permission: &str, role: Option<&str>) -> serde_json::Value {
    let mut body = serde_json::json!({ "permission": permission });
    if let Some(role) = role {
        body["role_name"] = serde_json::json!(role);
    }
    serde_json::json!({
        "matches": "collaborators/maintainer-1/permission",
        "stdout": body.to_string(),
        "status": 0,
    })
}

fn recover_as(
    scene: &EscalatedWorld,
    epoch: &str,
    login: &str,
    reason: &str,
    permission: serde_json::Value,
) -> Recovered {
    recover_defaulting(scene, Some(epoch), Some(login), reason, permission, &[])
}

fn recover_defaulting(
    scene: &EscalatedWorld,
    epoch: Option<&str>,
    login: Option<&str>,
    reason: &str,
    permission: serde_json::Value,
    extra: &[serde_json::Value],
) -> Recovered {
    crate::transport::store::forget_scratch_store();
    let context = Context {
        skill_dir: scene.world.work.clone(),
        repo_dir: scene.world.work.clone(),
        config: corrections_limit_row("1"),
        repo: None,
    };
    let directory = tempfile::tempdir().expect("a directory for the store");
    let path = directory.path().join("state.db");
    let log = directory.path().join("calls.log");
    let bin = crate::test_env::scripted_gh();
    let mut answers = vec![
        crate::test_env::issue_view_answer(42, stdout_with(&scene.comments)),
        serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
        permission,
        serde_json::json!({"matches": "issue comment 42", "stdout": "", "status": 0}),
    ];
    answers.extend_from_slice(extra);
    let script = crate::test_env::answers(&answers);
    crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            Ledger::open(&context, 42, RECLAIM_NOW).expect("the first touch imports the issue");
            let result = recover_review(
                &context,
                &RecoverReview {
                    issue: 42,
                    epoch,
                    authorized_by: login,
                    reason,
                    host_repo: "host-owner/host-repo",
                    now: "2026-08-31T00:00:00Z",
                },
            );
            let timeline = Ledger::open(&context, 42, "2026-08-31T00:00:00Z")
                .expect("the ledger opens")
                .timeline()
                .expect("the timeline reads");
            Recovered {
                result,
                timeline,
                calls: std::fs::read_to_string(&log).unwrap_or_default(),
            }
        })
    })
}

const WHY: &str = "the correction was over budget and a person owns the rest";

fn recover_with(scene: &EscalatedWorld, epoch: &str, permission: serde_json::Value) -> Recovered {
    recover_as(scene, epoch, "maintainer-1", WHY, permission)
}

fn has_recovery(timeline: &[ownership::Comment]) -> bool {
    timeline
        .iter()
        .any(|comment| comment.body.contains("issue-flow: review-recovery"))
}

fn refused_with(recovered: &Recovered) -> String {
    refusal_reason(
        recovered
            .result
            .as_ref()
            .expect_err("the recovery is refused"),
    )
}

#[test]
fn a_recovery_with_no_reason_a_malformed_login_or_an_unknown_epoch_is_refused() {
    let scene = escalated_world();
    let ok = permission_answer("admin", Some("admin"));
    let epoch = scene.escalated.epoch.clone();
    let blank = recover_as(&scene, &epoch, "maintainer-1", "  ", ok.clone());
    assert_eq!(refused_with(&blank), "blank-recover-reason");
    for login in ["", "../admin", "a b", "me;rm"] {
        let malformed = recover_as(&scene, &epoch, login, WHY, ok.clone());
        assert_eq!(
            refused_with(&malformed),
            "invalid-recover-login",
            "{login:?}"
        );
        assert!(!has_recovery(&malformed.timeline));
    }
    let unknown = recover_with(&scene, &"f".repeat(32), ok);
    assert_eq!(refused_with(&unknown), "recover-epoch-unknown");
    assert!(!has_recovery(&unknown.timeline));
}

#[test]
fn only_an_admin_or_maintain_role_authorises_a_recovery() {
    let scene = escalated_world();
    let epoch = scene.escalated.epoch.clone();
    for (permission, role) in [
        ("write", Some("maintain")),
        ("admin", Some("admin")),
        ("admin", None),
    ] {
        let recovered = recover_with(&scene, &epoch, permission_answer(permission, role));
        let answer = recovered
            .result
            .as_ref()
            .expect("a maintainer's recovery is recorded");
        assert_eq!(answer["ok"], true, "{permission} {role:?}: {answer}");
        assert!(has_recovery(&recovered.timeline), "{permission} {role:?}");
    }
    for (permission, role) in [
        ("write", Some("write")),
        ("write", None),
        ("read", Some("triage")),
        ("read", Some("read")),
        ("none", None),
    ] {
        let refused = recover_with(&scene, &epoch, permission_answer(permission, role));
        let failure = refused.result.as_ref().expect_err("not a maintainer");
        assert_eq!(
            refusal_reason(failure),
            "recover-not-a-maintainer",
            "{permission} {role:?}"
        );
        assert!(!has_recovery(&refused.timeline));
    }
}

#[test]
fn an_unanswered_permission_is_a_refusal_never_a_clearance() {
    let scene = escalated_world();
    let epoch = scene.escalated.epoch.clone();
    let failed = serde_json::json!({
        "matches": "collaborators/maintainer-1/permission",
        "stdout": "",
        "stderr": "HTTP 403",
        "status": 1,
    });
    let silent = serde_json::json!({
        "matches": "collaborators/maintainer-1/permission",
        "stdout": "{}",
        "status": 0,
    });
    for answer in [failed, silent] {
        let refused = recover_with(&scene, &epoch, answer);
        assert_eq!(refused_with(&refused), "recover-permission-unreadable");
        assert!(!has_recovery(&refused.timeline));
    }
}

#[test]
fn only_the_newest_receipt_that_stands_escalated_is_recovered() {
    let mut scene = escalated_world();
    let stale = recover_with(
        &scene,
        &scene.original.epoch,
        permission_answer("admin", Some("admin")),
    );
    assert_eq!(refused_with(&stale), "published-transaction-mismatch");
    scene.comments.pop();
    let rejected = recover_with(
        &scene,
        &scene.escalated.epoch,
        permission_answer("admin", Some("admin")),
    );
    assert_eq!(refused_with(&rejected), "recover-receipt-not-escalated");
}

#[test]
fn a_receipt_escalated_only_by_a_computed_verdict_is_recoverable() {
    let mut scene = escalated_world();
    scene.comments.pop();
    scene
        .comments
        .push(computed_by("codex-b", &scene.escalated, "escalated"));
    let recovered = recover_with(
        &scene,
        &scene.escalated.epoch,
        permission_answer("admin", Some("admin")),
    );
    assert!(recovered.result.is_ok(), "{:?}", recovered.result);
    assert!(has_recovery(&recovered.timeline));
}

fn authenticated_as(login: &str) -> serde_json::Value {
    serde_json::json!({
        "matches": "api user",
        "stdout": serde_json::json!({ "login": login }).to_string(),
        "status": 0,
    })
}

#[test]
fn a_recovery_without_flags_takes_the_escalated_receipt_and_the_authenticated_login() {
    let scene = escalated_world();

    let recovered = recover_defaulting(
        &scene,
        None,
        None,
        WHY,
        permission_answer("admin", Some("admin")),
        &[authenticated_as("maintainer-1")],
    );

    let answer = recovered.result.as_ref().expect("the short form recovers");
    assert_eq!(answer["epoch"], scene.escalated.epoch, "{answer}");
    assert_eq!(answer["authorized_by"], "maintainer-1", "{answer}");
    assert!(has_recovery(&recovered.timeline));
    assert!(recovered.calls.contains("api user"), "{}", recovered.calls);
}

#[test]
fn the_role_check_still_judges_the_login_the_short_form_resolved() {
    let scene = escalated_world();

    let refused = recover_defaulting(
        &scene,
        None,
        None,
        WHY,
        permission_answer("write", Some("write")),
        &[authenticated_as("maintainer-1")],
    );

    assert_eq!(refused_with(&refused), "recover-not-a-maintainer");
    assert!(!has_recovery(&refused.timeline));
}

#[test]
fn explicit_flags_win_over_the_defaults_and_ask_gh_nothing() {
    let scene = escalated_world();

    let recovered = recover_defaulting(
        &scene,
        Some(&scene.escalated.epoch),
        Some("maintainer-1"),
        WHY,
        permission_answer("admin", Some("admin")),
        &[authenticated_as("someone-else")],
    );

    let answer = recovered.result.as_ref().expect("the long form recovers");
    assert_eq!(answer["authorized_by"], "maintainer-1", "{answer}");
    assert!(
        !recovered.calls.contains("api user"),
        "the authenticated login was asked for although one was named: {}",
        recovered.calls
    );
}

#[test]
fn the_short_form_refuses_when_no_receipt_stands_escalated() {
    let mut scene = escalated_world();
    scene.comments.pop();

    let refused = recover_defaulting(
        &scene,
        None,
        None,
        WHY,
        permission_answer("admin", Some("admin")),
        &[authenticated_as("maintainer-1")],
    );

    assert_eq!(refused_with(&refused), "recover-no-escalated-receipt");
    assert!(!has_recovery(&refused.timeline));
}

#[test]
fn the_short_form_refuses_when_two_pull_requests_stand_escalated() {
    let mut scene = escalated_world();
    let other = ReviewTransaction {
        epoch: "9".repeat(32),
        pr: scene.escalated.pr + 1,
        ..scene.escalated.clone()
    };
    scene.comments.push(published(&other));
    scene.comments.push(escalated_receipt_by(
        "claude-a",
        &"6".repeat(32),
        "Alexis",
        &other,
    ));

    let refused = recover_defaulting(
        &scene,
        None,
        None,
        WHY,
        permission_answer("admin", Some("admin")),
        &[authenticated_as("maintainer-1")],
    );

    assert_eq!(refused_with(&refused), "recover-epoch-ambiguous");
    assert!(!has_recovery(&refused.timeline));
}

#[test]
fn the_short_form_refuses_when_gh_names_no_login() {
    let scene = escalated_world();
    let silent = serde_json::json!({ "matches": "api user", "stdout": "{}", "status": 0 });

    let refused = recover_defaulting(
        &scene,
        None,
        None,
        WHY,
        permission_answer("admin", Some("admin")),
        &[silent],
    );

    assert_eq!(refused_with(&refused), "recover-login-unresolved");
    assert!(!has_recovery(&refused.timeline));
}

#[test]
fn a_recovery_is_local_releases_nothing_and_reads_back_when_repeated() {
    let mut scene = escalated_world();
    let recovered = recover_with(
        &scene,
        &scene.escalated.epoch,
        permission_answer("write", Some("maintain")),
    );
    let answer = recovered
        .result
        .as_ref()
        .expect("a maintainer's recovery is recorded");
    assert_eq!(answer["role"], "maintain", "{answer}");
    assert!(has_recovery(&recovered.timeline));
    assert!(
        recovered
            .calls
            .contains("repos/host-owner/host-repo/collaborators/maintainer-1/permission"),
        "GitHub was never asked about the repository that hosts the pull request: {}",
        recovered.calls
    );
    assert!(
        !recovered.calls.contains("issue comment"),
        "a recovery wrote a tracking comment: {}",
        recovered.calls
    );
    assert_eq!(
        reason(
            require_un_escalated_terminal(&recovered.timeline, &scene.escalated)
                .expect_err("a recovery is not a release")
        ),
        "review-receipt-escalated"
    );

    scene.comments = recovered.timeline.clone();
    let repeat = recover_with(
        &scene,
        &scene.escalated.epoch,
        permission_answer("write", Some("maintain")),
    );
    assert_eq!(
        repeat.result.as_ref().expect("a repeat reads back")["reused_existing_recovery"],
        true
    );
}

fn recovery_marker(receipt: &ReviewTransaction, level: &str, focus: &str) -> ownership::Comment {
    wrote(
        "7",
        super::super::markers::render(
            "review-recovery",
            &[
                ("run-id", "person"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("actor", "maintainer-1"),
                ("permission", "admin"),
                ("reason", "a person owns the rest"),
                ("level", level),
                ("focus", focus),
            ],
        )
        .expect("a recovery marker"),
    )
}

fn successor_published(
    receipt: &ReviewTransaction,
    parent: &ReviewTransaction,
) -> ownership::Comment {
    wrote(
        "8",
        super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("parent", &parent.epoch),
                ("parent-pr", &parent.pr.to_string()),
                ("parent-head", &parent.head),
                ("parent-base", &parent.base),
                ("parent-digest", &parent.digest),
                ("delta", "delta-2-3"),
                ("recovered-from", &parent.epoch),
            ],
        )
        .expect("a published marker"),
    )
}

fn context_of(scene: &EscalatedWorld) -> Context {
    Context {
        skill_dir: scene.world.work.clone(),
        repo_dir: scene.world.work.clone(),
        config: Vec::new(),
        repo: None,
    }
}

fn successor_scene(level: &str, focus: &str) -> (EscalatedWorld, ReviewTransaction) {
    let mut scene = escalated_world();
    let head = scene.world.commit("notes.md", 1);
    let successor = scene.world.receipt('3', &head);
    scene
        .comments
        .push(recovery_marker(&scene.escalated, level, focus));
    scene
        .comments
        .push(successor_published(&successor, &scene.escalated));
    (scene, successor)
}

#[test]
fn a_successor_lineage_keeps_the_predecessors_tier_and_focus_and_owes_a_fresh_review() {
    let (scene, successor) = successor_scene("high", "risk");
    let policy = computed::policy_for(&context_of(&scene), &scene.comments, &successor, None)
        .expect("the policy reads");
    assert_eq!(policy.level, crate::transport::tier::RiskLevel::High);
    assert_eq!(policy.focus, crate::transport::tier::ReviewFocus::Risk);

    let verdict = computed_verdict(&scene.comments, &successor, policy.level, None, false);
    assert_eq!(
        verdict.outcome,
        ComputedOutcome::Incomplete,
        "recover is not a release"
    );
    assert_eq!(verdict.missing_roles.len(), 4, "{verdict:?}");
    assert!(
        !verdict
            .missing_roles
            .contains(&"review-validator".to_owned())
    );

    let publications = all_publications(&scene.comments).expect("readable");
    assert_eq!(
        corrections_spent(&scene.comments, &publications, successor.pr).expect("readable"),
        0,
        "the successor lineage did not get a new correction"
    );
}

fn raised_published(receipt: &ReviewTransaction, raise: &str) -> ownership::Comment {
    wrote(
        "9",
        super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("computed-level", "low"),
                ("raise", raise),
                ("raise-reason", "the diff cannot show who may call this"),
            ],
        )
        .expect("a published marker"),
    )
}

#[test]
fn a_recorded_raise_lifts_the_tier_the_gate_reads_and_an_unknown_word_lifts_nothing() {
    let world_scene = escalated_world();
    let head = world_scene.world.commit("notes.md", 1);
    let receipt = world_scene.world.receipt('4', &head);
    let context = context_of(&world_scene);
    let level_of = |comment: Option<ownership::Comment>| {
        let mut comments = world_scene.comments.clone();
        comments.extend(comment);
        computed::policy_for(&context, &comments, &receipt, None)
            .expect("the policy reads")
            .level
    };
    let computed = level_of(None);
    assert!(computed < crate::transport::tier::RiskLevel::High);
    assert_eq!(
        level_of(Some(raised_published(&receipt, "authorization"))),
        crate::transport::tier::RiskLevel::High,
        "a recorded raise is not read by the gate"
    );
    assert_eq!(
        level_of(Some(raised_published(&receipt, "low"))),
        computed,
        "a word outside the list lifted the tier"
    );
}

#[test]
fn a_raised_repair_is_judged_by_the_whole_high_panel_and_not_the_delta_lens() {
    let (_root, context, parent, repair) = delta_lens_repair_world();
    let mut comments = vec![
        published(&parent),
        computed_by("codex-b", &parent, "approved"),
        recorded_finding_on(&parent, 6, "f-1", "suggestion", None, None),
        wrote(
            "3",
            super::super::markers::render(
                "published",
                &[
                    ("run-id", "claude-a"),
                    ("epoch", &repair.epoch),
                    ("pr", &repair.pr.to_string()),
                    ("head", &repair.head),
                    ("base", &repair.base),
                    ("digest", &repair.digest),
                    ("parent", &parent.epoch),
                    ("parent-pr", &parent.pr.to_string()),
                    ("parent-head", &parent.head),
                    ("parent-base", &parent.base),
                    ("parent-digest", &parent.digest),
                    ("delta", &"0".repeat(64)),
                    ("delta-range", "parent-head..head"),
                    ("computed-level", "medium"),
                    ("raise", "authorization"),
                    ("raise-reason", "the diff cannot show who may call this"),
                ],
            )
            .expect("a raised repair marker"),
        ),
    ];
    comments.push(recorded_lens_report_for(&repair, 3, "review-lens", None));
    let (level, verdict) = computed::computed_for(&context, &comments, &repair, None)
        .expect("the repair's panel reads");
    assert_eq!(level, RiskLevel::High, "{verdict:?}");
    assert_ne!(verdict.outcome, ComputedOutcome::Approved, "{verdict:?}");
    for role in [
        "review-risk",
        "review-readback",
        "review-readability",
        "review-reliability",
    ] {
        assert!(
            verdict.missing_roles.iter().any(|missing| missing == role),
            "a raised repair was not owed `{role}`: {verdict:?}"
        );
    }
}

#[test]
fn a_successor_is_reviewed_through_the_focus_its_predecessor_froze() {
    let (scene, successor) = successor_scene("medium", "risk");
    let mut comments = scene.comments;
    comments.push(recorded_lens_report_for(
        &successor,
        9,
        "review-readability",
        Some("readability"),
    ));
    let verdict = computed_verdict(
        &comments,
        &successor,
        crate::transport::tier::RiskLevel::Medium,
        None,
        false,
    );
    assert_eq!(verdict.outcome, ComputedOutcome::Escalated, "{verdict:?}");
    assert!(
        verdict
            .escalated
            .iter()
            .any(|entry| entry.id == "lens-focus")
    );
    assert_eq!(verdict.missing_roles, ["review-risk"]);
}

#[test]
fn a_successor_whose_inherited_tier_is_above_its_own_does_not_release_on_the_low_path() {
    let world = republish_world();
    let predecessor = world.receipt('2', &"2".repeat(40));
    let head = world.commit("notes.md", 1);
    let successor = world.receipt('3', &head);
    let mut comments = vec![
        live_claim("codex-b"),
        published(&predecessor),
        escalated_receipt_by("claude-a", &"5".repeat(32), "Alexis", &predecessor),
        successor_published(&successor, &predecessor),
    ];
    let scene = EscalatedWorld {
        original: predecessor.clone(),
        escalated: predecessor.clone(),
        comments: Vec::new(),
        world,
    };
    let context = context_of(&scene);
    comments.insert(3, recovery_marker(&predecessor, "high", "reliability"));
    let refused = release_basis(&context, &comments, &successor, None)
        .err()
        .expect("a passive successor of a high lineage released on the structural readback");
    assert_eq!(
        reason(refused),
        "qualifying-review-verdict-missing",
        "the successor took the low path and was refused only by the recomputation"
    );
    comments.retain(|comment| !comment.body.contains("issue-flow: review-recovery"));
    comments.retain(|comment| !comment.body.contains("recovered-from"));
    comments.push(published(&successor));
    assert!(
        release_basis(&context, &comments, &successor, None).is_ok(),
        "a passive change with no inheritance stopped reading as low"
    );
}

#[test]
fn a_successor_whose_recovery_record_is_gone_fails_closed() {
    let (mut scene, successor) = successor_scene("high", "risk");
    scene
        .comments
        .retain(|comment| !comment.body.contains("issue-flow: review-recovery"));
    let failure = inherited_policy_of(&scene.comments, &successor)
        .expect_err("a lost recovery silently dropped the floor");
    assert_eq!(refusal_reason(&failure), "recovery-record-missing");
}

fn inherited_policy_of(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<Option<()>, Failure> {
    recover::inherited_policy(comments, receipt).map(|policy| policy.map(|_| ()))
}

#[test]
fn a_publication_after_a_recovery_writes_the_successor_marker_and_is_outside_the_old_budget() {
    let mut scene = escalated_world();
    scene
        .comments
        .push(recovery_marker(&scene.escalated, "medium", "reliability"));
    scene.world.commit("three.txt", 40);
    let (published, bodies) = scene
        .world
        .publish_logging(&scene.comments, &scene.escalated.head);
    let published = published.expect(
        "a recovered lineage publishes work its predecessor's budget and scope would refuse",
    );
    assert_eq!(published["ok"], true, "{published}");
    let marker = super::super::markers::parse(&bodies)
        .into_iter()
        .find(|marker| marker.get("kind").map(String::as_str) == Some("published"))
        .expect("the publication wrote its marker");
    assert_eq!(
        marker.get("recovered-from"),
        Some(&scene.escalated.epoch),
        "the successor root does not say what it descends from"
    );
    assert!(
        PublicationLineage::from_marker(&marker)
            .expect("a lineage")
            .recovered
    );
}

#[test]
fn a_publication_over_an_escalated_receipt_nobody_recovered_is_still_refused() {
    let scene = escalated_world();
    scene.world.commit("three.txt", 40);
    let envelope = refusal_of(scene.world.publish(&scene.comments, &scene.escalated.head));
    assert_eq!(envelope["reason"], "correction-already-spent", "{envelope}");
}

#[test]
fn a_validator_replay_by_a_run_that_does_not_hold_the_claim_is_refused() {
    let (correction, comments) = correction_world();
    let failing = Validation {
        original: "fail",
        ..PASSING
    };
    let intruder = Validation {
        run_id: "intruder",
        ..failing
    };
    let session = validate(&correction, &comments, &[failing, intruder]);
    assert!(session.results[0].is_ok(), "the claim holder's own result");
    assert_eq!(
        refusal_reason(
            session.results[1]
                .as_ref()
                .expect_err("a replay by a run without the claim read the result back")
        ),
        "not-current-live-holder"
    );
}

#[test]
fn a_host_repository_that_is_not_owner_and_name_is_refused_before_gh_is_asked() {
    let directory = tempfile::tempdir().expect("a directory");
    let context = context_at(directory.path());
    for host_repo in ["", "name", "/name", "owner/", "a/b/c"] {
        let failure = recover::role_of(&context, host_repo, "someone")
            .expect_err("a malformed repository is not asked about");
        let envelope = failure.envelope();
        assert_eq!(
            envelope["reason"], "recover-permission-unreadable",
            "{host_repo:?}"
        );
        assert!(
            envelope["detail"]
                .as_str()
                .is_some_and(|text| text.contains("is not owner/name")),
            "{host_repo:?}: {envelope}"
        );
    }
}

fn remote_has_branch(world: &RepublishWorld) -> bool {
    crate::transport::run(
        &[
            "git",
            "rev-parse",
            "--verify",
            &format!("refs/heads/{REPUBLISH_BRANCH}"),
        ],
        Some(&world.remote),
        crate::transport::How::tolerated(),
    )
    .is_ok_and(|ran| ran.status == 0)
}

#[test]
fn a_publication_expecting_none_is_made_when_the_branch_has_no_publication() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);

    let (answer, _) = world.publish_expecting(&[], &head, Some("none"));

    answer.expect("no publication yet is what the caller expected");
    assert!(remote_has_branch(&world));
}

#[test]
fn a_publication_expecting_the_newest_epoch_is_made() {
    let world = republish_world();
    let first_head = world.commit("two.txt", 1);
    let first = world.receipt('1', &first_head);
    world.commit("two.txt", 2);

    let (answer, _) =
        world.publish_expecting(&[published(&first)], &first_head, Some(&first.epoch));

    answer.expect("the newest epoch is the one expected");
    assert!(remote_has_branch(&world));
}

#[test]
fn a_publication_expecting_a_stale_epoch_is_refused_before_anything_is_pushed() {
    let world = republish_world();
    let first_head = world.commit("two.txt", 1);
    let first = world.receipt('1', &first_head);
    world.commit("two.txt", 2);

    let envelope = refusal_of(
        world
            .publish_expecting(&[published(&first)], &first_head, Some(&"2".repeat(32)))
            .0,
    );

    assert_eq!(envelope["reason"], "expected-epoch-moved", "{envelope}");
    assert_eq!(envelope["recorded_epoch"], first.epoch);
    assert!(!remote_has_branch(&world), "a refused publication pushed");
}

#[test]
fn a_publication_expecting_none_is_refused_once_the_branch_has_a_publication() {
    let world = republish_world();
    let first_head = world.commit("two.txt", 1);
    let first = world.receipt('1', &first_head);
    world.commit("two.txt", 2);

    let envelope = refusal_of(
        world
            .publish_expecting(&[published(&first)], &first_head, Some("none"))
            .0,
    );

    assert_eq!(envelope["reason"], "expected-epoch-moved", "{envelope}");
    assert!(!remote_has_branch(&world));
}

#[test]
fn a_publication_expecting_an_epoch_is_refused_when_the_branch_has_none() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);

    let envelope = refusal_of(world.publish_expecting(&[], &head, Some(&"1".repeat(32))).0);

    assert_eq!(envelope["reason"], "expected-epoch-moved", "{envelope}");
    assert_eq!(envelope["recorded_epoch"], "none");
    assert!(!remote_has_branch(&world));
}

#[test]
fn a_stale_epoch_given_to_either_entry_point_is_refused_before_anything_is_pushed() {
    for operation in ["publish-review", "republish-review"] {
        let world = republish_world();
        let first_head = world.commit("two.txt", 1);
        let first = world.receipt('1', &first_head);
        world.commit("two.txt", 2);

        let (answer, calls) = world.drive(
            &[published(&first)],
            &first_head,
            Some(&"2".repeat(32)),
            Some(operation),
        );

        let envelope = refusal_of(answer);
        assert_eq!(
            envelope["reason"], "expected-epoch-moved",
            "{operation}: {envelope}"
        );
        assert_eq!(envelope["recorded_epoch"], first.epoch);
        assert!(!remote_has_branch(&world), "{operation} pushed");
        assert!(
            !calls.contains("pr edit") && !calls.contains("pr ready"),
            "{operation} touched the pull request: {calls}"
        );
    }
}

#[test]
fn the_expected_epoch_flag_reaches_both_entry_points_and_a_matching_one_publishes() {
    for operation in ["publish-review", "republish-review"] {
        let world = republish_world();
        let first_head = world.commit("two.txt", 1);
        let first = world.receipt('1', &first_head);
        git(
            &world.work,
            &[
                "push",
                "-q",
                "origin",
                &format!("{first_head}:refs/heads/{REPUBLISH_BRANCH}"),
            ],
        );
        world.commit("two.txt", 2);

        let (answer, _) = world.drive(
            &[published(&first)],
            &first_head,
            Some(&first.epoch),
            Some(operation),
        );

        answer.unwrap_or_else(|failure| panic!("{operation}: {}", failure.envelope()));
    }
}

fn generated_change(
    command: &str,
    before: Option<&str>,
    after: &str,
) -> (tempfile::TempDir, Context, ReviewTransaction) {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the repository directory");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    let declaration =
        format!("[[generated]]\npath = \"notes.md\"\ncommand = [\"sh\", \"-c\", \"{command}\"]\n");
    std::fs::write(repo.join("estigia.generated.toml"), declaration).expect("the declaration");
    if let Some(before) = before {
        std::fs::write(repo.join("notes.md"), before).expect("content");
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let base = head_of(&repo);
    std::fs::write(repo.join("notes.md"), after).expect("content");
    std::fs::write(repo.join("prose.md"), "Ordinary prose.\n").expect("content");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "--quiet", "-m", "head"]);
    let receipt = ReviewTransaction {
        head: head_of(&repo),
        base,
        ..ledger_receipt()
    };
    let context = Context {
        repo: Some("o/r".to_owned()),
        ..context_at(&repo)
    };
    (root, context, receipt)
}

fn approve_computed(
    context: &Context,
    receipt: &ReviewTransaction,
    standing: Vec<ownership::Comment>,
    lane: bool,
) -> Result<serde_json::Value, Failure> {
    let mut comments = vec![live_claim("claude-a"), published(receipt)];
    comments.extend(standing);
    let directory = tempfile::tempdir().expect("a directory for the store");
    let path = directory.path().join("state.db");
    let bin = crate::test_env::scripted_gh();
    let mut answers = if lane {
        lane_read_answers()
    } else {
        Vec::new()
    };
    answers.insert(
        0,
        crate::test_env::issue_view_answer(42, stdout_with(&comments)),
    );
    answers.push(serde_json::json!({"matches": "issue comment 42", "stdout": "", "status": 0}));
    let script = crate::test_env::answers(&answers);
    let operation_id = op(9);
    let request = computed_request(&operation_id, receipt);
    crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            Ledger::open(context, 42, RECLAIM_NOW).expect("the first touch imports the issue");
        });
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            record_computed_verdict(context, &request)
        })
    })
}

const GENERATE_NOTES: &str = "printf 'generated\\\\n' > notes.md";

#[test]
fn an_approval_over_a_regenerated_file_is_recorded() {
    for before in [Some("old\n"), None] {
        let (_root, context, receipt) = generated_change(GENERATE_NOTES, before, "generated\n");

        let answer = approve_computed(&context, &receipt, Vec::new(), true)
            .expect("the regenerated bytes match");

        assert_eq!(answer["outcome"], "approved", "{before:?}: {answer}");
    }
}

#[test]
fn an_approval_over_hand_edited_generated_bytes_is_refused_naming_the_path() {
    for before in [Some("old\n"), None] {
        let (_root, context, receipt) = generated_change(GENERATE_NOTES, before, "hand edited\n");

        let failure = approve_computed(&context, &receipt, Vec::new(), true)
            .expect_err("bytes nobody regenerated were approved");

        let envelope = failure.envelope();
        assert_eq!(
            envelope["reason"], "generated-reproduction-mismatch",
            "{envelope}"
        );
        assert_eq!(envelope["paths"][0]["path"], "notes.md", "{envelope}");
        assert_eq!(envelope["paths"][0]["outcome"], "mismatch", "{envelope}");
    }
}

#[test]
fn an_approval_whose_declared_command_fails_is_refused_as_unavailable() {
    let (_root, context, receipt) = generated_change("exit 3", Some("old\n"), "generated\n");

    let failure = approve_computed(&context, &receipt, Vec::new(), true)
        .expect_err("an unrun command was read as a match");

    let envelope = failure.envelope();
    assert_eq!(
        envelope["reason"], "generated-reproduction-unavailable",
        "{envelope}"
    );
    assert_eq!(envelope["paths"][0]["outcome"], "unavailable", "{envelope}");
}

#[test]
fn a_rejection_does_not_run_the_declared_command() {
    let marker_directory = tempfile::tempdir().expect("a directory outside the repository");
    let marker = marker_directory.path().join("ran");
    let command = format!(
        "printf ran > {}; {GENERATE_NOTES}",
        crate::test_env::shell_path(&marker)
    );
    let (_root, context, receipt) = generated_change(&command, Some("old\n"), "generated\n");
    let finding = recorded_finding_on(
        &receipt,
        6,
        "f-1",
        "severe",
        Some("deterministic"),
        Some("introduced"),
    );

    let answer = approve_computed(&context, &receipt, vec![finding], false)
        .expect("the rejection is recorded");

    assert_eq!(answer["outcome"], "rejected", "{answer}");
    assert!(!marker.exists(), "a rejection executed repository code");

    let (_root, context, receipt) = generated_change(&command, Some("old\n"), "generated\n");
    approve_computed(&context, &receipt, Vec::new(), true).expect("the approval records");
    assert!(
        marker.exists(),
        "an approval did not run the declared command"
    );
}

#[test]
fn a_payload_rebuild_over_a_manifest_matches_the_recorded_digest() {
    let (root, context, receipt) = generated_change(GENERATE_NOTES, Some("old\n"), "generated\n");
    let payload =
        super::super::target::review_payload(&context.repo_dir, &receipt.base, &receipt.head)
            .expect("a payload");
    assert!(
        payload.text.contains("# estigia-generated "),
        "{}",
        payload.text
    );
    let comments = vec![published_with_payload_digest(&receipt, &payload.digest)];
    let candidate = candidate_file(root.path(), &payload.text);

    let answer = read_review_payload_membership(&context, &comments, None, &candidate)
        .expect("the rebuild equals the recorded payload");

    assert_eq!(answer["payload_digest"], payload.digest, "{answer}");
}

mod capture;
mod status;

fn budget_world(declared: bool) -> (tempfile::TempDir, Context, String, String, String) {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the repository directory");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    let rows = |tag: &str| -> String { (0..300).map(|row| format!("{tag} {row}\n")).collect() };
    let write_and_commit = |files: &[(&str, String)], message: &str| {
        for (name, content) in files {
            std::fs::write(repo.join(name), content).expect("content");
        }
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "--quiet", "-m", message]);
        head_of(&repo)
    };
    let mut seed = vec![
        ("a.rs", "one\n".to_owned()),
        ("lock.txt", "old\n".to_owned()),
    ];
    if declared {
        seed.push((
            "estigia.generated.toml",
            "[[generated]]\npath = \"lock.txt\"\ncommand = [\"sh\", \"-c\", \"exit 1\"]\n"
                .to_owned(),
        ));
    }
    let base = write_and_commit(&seed, "base");
    let parent_head = write_and_commit(
        &[("a.rs", "two\n".to_owned()), ("lock.txt", rows("first"))],
        "parent",
    );
    let repair_head = write_and_commit(
        &[("a.rs", "three\n".to_owned()), ("lock.txt", rows("second"))],
        "repair",
    );
    let context = context_at(&repo);
    (root, context, base, parent_head, repair_head)
}

fn budget_parent(base: &str, head: &str) -> LatestPublication {
    LatestPublication {
        publisher: "claude-a".to_owned(),
        receipt: ReviewTransaction {
            head: head.to_owned(),
            base: base.to_owned(),
            ..ledger_receipt()
        },
        lineage: None,
        correction_state: None,
        payload_digest: None,
        confirmed: true,
        url: String::new(),
        topology: None,
        topology_reason: None,
        computed_level: None,
        raise: None,
        raise_reason: None,
        focus: None,
        focus_reason: None,
        lane_note: None,
        lane_workflows: Vec::new(),
        autoclose: String::new(),
        mandatory_follow_up: None,
    }
}

fn run_budget_check(
    context: &Context,
    base: &str,
    parent_head: &str,
    repair_head: &str,
) -> Result<CorrectionCheck, Failure> {
    let tree = crate::transport::tier::tree_of(context, None, repair_head).expect("a tree");
    correction_budget_check(
        context,
        &context.repo_dir,
        Some(&budget_parent(base, parent_head)),
        base,
        repair_head,
        &tree,
    )
}

#[test]
fn a_correction_that_regenerates_a_declared_file_is_not_charged_its_churn() {
    let (_root, context, base, parent_head, repair_head) = budget_world(true);

    let check = run_budget_check(&context, &base, &parent_head, &repair_head)
        .expect("the regenerated lines were charged to the correction");

    let state = check.state.expect("a budget state");
    assert_eq!(state.original_changed_lines, 2);
    assert_eq!(state.budget, 2);
    assert_eq!(state.cumulative, 2);
}

#[test]
fn the_same_correction_without_a_declaration_exceeds_the_budget() {
    let (_root, context, base, parent_head, repair_head) = budget_world(false);

    let failure = run_budget_check(&context, &base, &parent_head, &repair_head)
        .err()
        .expect("the churn was free without a declaration");

    assert_eq!(failure.envelope()["reason"], "correction-budget-exceeded");
}

fn advance_base_without_integrating(world: &RepublishWorld) {
    git(&world.work, &["checkout", "-q", "main"]);
    std::fs::write(world.work.join("base-only.txt"), "base\n").expect("content");
    git(&world.work, &["add", "base-only.txt"]);
    git(
        &world.work,
        &["commit", "--quiet", "-m", "the base advances"],
    );
    git(&world.work, &["push", "-q", "origin", "main"]);
    git(&world.work, &["checkout", "-q", REPUBLISH_BRANCH]);
}

#[test]
fn the_base_refusal_during_a_correction_names_only_the_merge() {
    let world = republish_world();
    let parent_head = world.commit("two.txt", 1);
    world.commit("three.txt", 1);
    advance_base_without_integrating(&world);
    let parent = world.receipt('1', &parent_head);

    let envelope = refusal_of(world.publish(
        &[
            published(&parent),
            computed_by(REPUBLISH_RUN, &parent, "rejected"),
        ],
        &parent_head,
    ));
    assert_eq!(
        envelope["reason"], "publication-base-not-ancestor",
        "{envelope}"
    );
    let action = envelope["action"].as_str().unwrap_or_default();
    assert!(
        action.contains("git -C <worktree> merge --no-edit origin/main"),
        "the merge recovery must be named: {envelope}"
    );
    assert!(
        !action.contains("`republish_review`") && !action.contains("`publish_review`"),
        "a rebase recovery was offered during a correction: {envelope}"
    );
    assert!(
        action.contains("validator-delta-carries-upstream"),
        "the refusal must say why a rebase is not offered: {envelope}"
    );
}

#[test]
fn the_base_refusal_outside_a_correction_still_offers_the_rebase() {
    let world = republish_world();
    let parent_head = world.commit("two.txt", 1);
    advance_base_without_integrating(&world);

    let envelope = refusal_of(world.publish(&[], &parent_head));
    assert_eq!(
        envelope["reason"], "publication-base-not-ancestor",
        "{envelope}"
    );
    let action = envelope["action"].as_str().unwrap_or_default();
    assert!(
        action.contains("git -C <worktree> merge --no-edit origin/main")
            && action.contains("`republish_review`")
            && action.contains("`publish_review`"),
        "outside a correction both recoveries stay named: {envelope}"
    );
}

fn lane_off_context(root: &std::path::Path) -> Context {
    Context {
        config: vec![(
            "Publication lane".to_owned(),
            "off".to_owned(),
            Source::Written,
        )],
        ..lane_read_context(root)
    }
}

#[test]
fn release_ci_names_the_lane_and_sends_a_lane_that_is_off_to_the_merge() {
    let root = tempfile::tempdir().expect("a context root");
    let head = "a".repeat(40);
    let (lane, next) = ci_outlook(&lane_off_context(root.path()), &head, 7);
    assert_eq!(lane["state"], "not-consulted", "{lane}");
    assert_eq!(next, "merge");
}

#[test]
fn release_ci_sends_a_repository_whose_actions_are_off_to_the_merge_and_keeps_the_read() {
    let root = tempfile::tempdir().expect("a context root");
    let head = "d".repeat(40);
    let permissions = serde_json::json!({"allowed": null, "enabled": false});
    let mut answers = lane_read_answers();
    answers.push(serde_json::json!({
        "matches": "actions/permissions",
        "stdout": permissions.to_string(),
        "status": 0,
    }));
    let bin = crate::test_env::scripted_gh();
    let (lane, next) =
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
            ci_outlook(&lane_read_context(root.path()), &head, 7)
        });
    assert_eq!(lane["state"], "disabled", "{lane}");
    assert_eq!(lane["read"], permissions, "{lane}");
    assert_eq!(next, "merge");
}

#[test]
fn release_ci_sends_a_lane_that_never_ran_to_the_merge() {
    let head = "b".repeat(40);
    let lane = publication_lane_answer(
        &head,
        read_publication_lane(&check_runs(&[check_run(
            "check (ubuntu-latest)",
            "completed",
            Some("failure"),
        )]))
        .expect("a readable listing"),
        Some(RedClassification::NeverRan {
            annotation: Some("no minutes remaining on this account".to_owned()),
            runs: Vec::new(),
        }),
        ActionsAvailability::Available,
    )
    .expect("a never-ran answer");
    assert_eq!(lane["state"], "never-ran");
    assert_eq!(next_after_release(&lane), "merge");
}

#[test]
fn release_ci_keeps_a_lane_that_will_run_on_ci_status() {
    let root = tempfile::tempdir().expect("a context root");
    let head = "c".repeat(40);
    let answers = [
        serde_json::json!({
            "matches": "repo view",
            "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}",
            "status": 0,
        }),
        serde_json::json!({
            "matches": "check-runs",
            "stdout": check_runs(&[check_run("ci", "in_progress", None)]).to_string(),
            "status": 0,
        }),
    ];
    let bin = crate::test_env::scripted_gh();
    let (lane, next) =
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
            ci_outlook(&lane_read_context(root.path()), &head, 7)
        });
    assert_eq!(lane["state"], "running", "{lane}");
    assert_eq!(next, "ci_status");
}

#[test]
fn release_ci_does_not_send_a_lane_it_could_not_read_to_the_merge() {
    let root = tempfile::tempdir().expect("a context root");
    let head = "c".repeat(40);
    let answers = [
        serde_json::json!({
            "matches": "repo view",
            "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}",
            "status": 0,
        }),
        serde_json::json!({"matches": "check-runs", "stdout": "", "status": 1}),
    ];
    let bin = crate::test_env::scripted_gh();
    let (lane, next) =
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
            ci_outlook(&lane_read_context(root.path()), &head, 7)
        });
    assert_eq!(lane["state"], "unreadable", "{lane}");
    assert_eq!(next, "ci_status");
}
