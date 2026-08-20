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
        plan_release(&held, "claude-a", "claude", false, None, &[]).expect("discovery"),
        Release::Confirm {
            target_operation: "op1".to_owned()
        }
    );
    // The second binds it.
    assert_eq!(
        plan_release(&held, "claude-a", "claude", false, Some("op1"), &[]).expect("the write"),
        Release::Write {
            target_operation: "op1".to_owned()
        }
    );
    // Naming a different epoch is refused: ownership moved in between.
    assert_eq!(
        reason(
            plan_release(&held, "claude-a", "claude", false, Some("op2"), &[])
                .expect_err("the epoch moved")
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
        reason(plan_release(&empty, "claude-a", "claude", false, None, &[]).expect_err("nothing")),
        "nothing-to-unassign"
    );

    // "Somebody else holds it" when nobody else does.
    assert_eq!(
        reason(
            plan_release(&held, "claude-a", "claude", true, None, &[])
                .expect_err("the timeline says otherwise")
        ),
        "held-by-other-without-other-holder"
    );

    // A runtime that is not the one recorded.
    assert_eq!(
        reason(
            plan_release(&held, "claude-a", "codex", false, None, &[])
                .expect_err("another runtime")
        ),
        "unassign-metadata-mismatch"
    );

    // An acquisition that recorded no runtime is legacy, and is held to the
    // **labels** instead. Nothing was holding it to anything: this took the
    // recorded runtime as the whole test, so any runtime at all could release a
    // legacy acquisition on nothing but its own say-so.
    let mut legacy = event("claude-a", 0, None);
    legacy.runtime = None;
    let old = ownership::Holding {
        holder: Some("claude-a".to_owned()),
        event: Some(legacy.clone()),
        live: vec![legacy],
        stale: Vec::new(),
    };
    assert!(
        plan_release(
            &old,
            "claude-a",
            "codex",
            false,
            None,
            &[String::from("dev:codex")]
        )
        .is_ok(),
        "the projection named this runtime and it was still refused"
    );
    for projection in [
        Vec::new(),
        vec![String::from("dev:claude")],
        vec![String::from("dev:codex"), String::from("dev:claude")],
    ] {
        assert_eq!(
            reason(
                plan_release(&old, "claude-a", "codex", false, None, &projection)
                    .expect_err("the projection does not say codex")
            ),
            "unassign-metadata-mismatch",
            "released a legacy acquisition against {projection:?}"
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
        plan_release(
            &held,
            "claude-a",
            "claude",
            false,
            None,
            &[String::from("dev:codex")]
        )
        .is_ok(),
        "a successor the labels do name was refused"
    );
    for projection in [
        Vec::new(),
        vec![String::from("dev:claude")],
        vec![String::from("dev:codex"), String::from("dev:codex")],
    ] {
        assert_eq!(
            reason(
                plan_release(&held, "claude-a", "claude", false, None, &projection)
                    .expect_err("the successor cannot be named")
            ),
            "holder-runtime-missing",
            "handed the issue to a holder nobody can name, against {projection:?}"
        );
    }
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
    // not a takeover.
    assert_eq!(
        reason(
            plan_takeover(
                &holding(Vec::new(), vec![event("claude-a", 0, None)]),
                "claude-a",
                true,
                None
            )
            .unwrap_err()
        ),
        "stale-self-requires-claim"
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

#[test]
fn a_written_reclaim_that_nothing_holds_is_a_write_failure_and_never_a_stop() {
    // One `Stop` used to answer all of these. A `Stop` says *this did not
    // happen*, and that is the one thing not known here: the comment is on the
    // issue. Told nothing happened, a caller writes the takeover again.
    assert!(matches!(
        won_the_reclaim(None, "claude-a"),
        Err(Failure::Write(_))
    ));

    let lost = won_the_reclaim(Some(takeover("claude-b", "9999", false)), "claude-a")
        .expect_err("another run holds it");
    assert_eq!(
        lost.envelope().get("winner").and_then(|w| w.as_str()),
        Some("claude-b")
    );
    assert_eq!(reason(lost), "lost-reclaim-race");

    // Won, and the answer carries whether it was forced rather than what the
    // caller asked for.
    assert!(
        won_the_reclaim(Some(takeover("claude-a", "0123", true)), "claude-a").expect("won"),
        "a forced takeover came back unforced"
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
    let existing = operation_marker(&comments, operation, "unassign", &[("run-id", "claude-a")])
        .expect("a lookup")
        .expect("this run's own unassign");

    // Planning again is where the old answer came from, and it is the wrong
    // question: the epoch is gone precisely because this call worked.
    assert_eq!(
        reason(
            plan_release(&ownership, "claude-a", "claude", false, Some(target), &[])
                .expect_err("nothing is left to release")
        ),
        "nothing-to-unassign"
    );

    let answer = resumed_release(
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

fn published_by(run_id: &str, receipt: &ReviewReceipt) -> ownership::Comment {
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

fn published(receipt: &ReviewReceipt) -> ownership::Comment {
    published_by("claude-a", receipt)
}

fn handoff_by(
    run_id: &str,
    target: &str,
    operation: &str,
    receipt: &ReviewReceipt,
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
    receipt: &ReviewReceipt,
) -> ownership::Comment {
    verdict_attested_by(run_id, run_id, outcome, operation, receipt)
}

/// The direct route, where the claim holder records the reviewer it acquired.
fn verdict_attested_by(
    attester: &str,
    reviewer: &str,
    outcome: &str,
    operation: &str,
    receipt: &ReviewReceipt,
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

#[test]
fn an_unresolved_handoff_excludes_its_publisher_and_requesters_but_not_another_run() {
    let receipt = ReviewReceipt {
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
        } = review_eligibility(&comments, excluded)
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
        review_eligibility(&comments, "gemini-c"),
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
            review_eligibility(&split, "claude-a"),
            ReviewEligibility::Excluded { .. }
        ),
        "the publishing run was offered work whose review is unresolved"
    );
    assert!(
        matches!(
            review_eligibility(&split, "codex-b"),
            ReviewEligibility::Excluded { .. }
        ),
        "the requesting run was offered the handoff it asked somebody else to take"
    );
    assert_eq!(
        review_eligibility(&split, "gemini-c"),
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
    let receipt = ReviewReceipt {
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
            ReviewReceipt::from_marker(&marker("published", &overrides)).is_none(),
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
        latest_publication(&anonymous).is_none(),
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

/// `SKILL.md` promises the requesting run cannot *select or reclaim* an
/// unresolved handoff. Selection was refused inside `claim` and reclaim was
/// not, so a requester whose replacement went stale could take the item back
/// and become, once again, the only holder forbidden to produce its verdict —
/// the exact livelock the handoff exists to end. One guard, consulted by both
/// routes, is what keeps the two from disagreeing again.
#[test]
fn no_acquisition_route_returns_an_unresolved_handoff_to_its_requester() {
    let receipt = ReviewReceipt {
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

    let source = include_str!("../claim.rs");
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

/// `comment_with_body` posts what it is handed, and its contract is *a body
/// this crate composed*. The blocker, discharger and run-id in a review
/// protocol comment are agent text, so a body that quoted a marker would **be**
/// that marker once posted: the publishing run could carry a forged
/// `review-verdict` inside its own handoff comment, satisfy the handoff it is
/// forbidden to satisfy, and clear `release_ci` with evidence it wrote itself.
/// That is the liveness fix turning into an integrity hole, which is the one
/// trade this issue refuses.
#[test]
fn agent_text_in_a_protocol_comment_cannot_forge_a_second_marker() {
    let receipt = ReviewReceipt {
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
    let source = include_str!("../claim.rs");
    for entry in [
        "\npub fn handoff_review(",
        "\npub fn record_review_verdict(",
    ] {
        let body = source.split_once(entry).expect("the writer exists").1;
        let body = body.split_once("\npub fn ").map_or(body, |(body, _)| body);
        let composed = body
            .find("protocol_body(")
            .unwrap_or_else(|| panic!("{entry} composes its comment without escaping agent text"));
        let posted = body
            .find("comment_with_body(")
            .unwrap_or_else(|| panic!("{entry} no longer posts a comment"));
        assert!(composed < posted, "{entry} posts before it escapes");
    }
}

#[test]
fn a_distinct_exact_receipt_verdict_resolves_but_only_acceptance_qualifies_delivery() {
    let receipt = ReviewReceipt {
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

    for (reviewer, outcome) in [("claude-a", "accepted"), ("codex-b", "approved")] {
        let mut comments = base.clone();
        comments.push(verdict_by(
            reviewer,
            outcome,
            "33333333333333333333333333333333",
            &receipt,
        ));
        assert!(
            matches!(
                review_eligibility(&comments, "claude-a"),
                ReviewEligibility::Excluded { .. }
            ),
            "{reviewer}'s {outcome} marker incorrectly resolved the handoff"
        );
    }

    let mut rejected = base.clone();
    rejected.push(verdict_by(
        "codex-b",
        "rejected",
        "33333333333333333333333333333333",
        &receipt,
    ));
    assert_eq!(
        review_eligibility(&rejected, "claude-a"),
        ReviewEligibility::Eligible,
        "a rejected review must let the author resume to fix the receipt"
    );
    assert!(qualifying_review_verdict(&rejected, &receipt).is_none());

    // The read side keeps its own copy of the requester rule, and it is not
    // redundant with the one the writer enforces: a verdict crediting a
    // requester can predate the handoff that made them one, and the writer
    // never saw it.
    //
    // The requester here is deliberately **not** the publisher. With the two the
    // same run, the publisher half refuses on its own and this asserts nothing —
    // which is what an earlier version of this block did, while its comment
    // claimed a measurement the fixture could not make.
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
        qualifying_review_verdict(&requester_only, &receipt).is_none(),
        "a verdict crediting the run that asked for the review qualified delivery"
    );
    assert!(
        matches!(
            review_eligibility(&requester_only, "codex-b"),
            ReviewEligibility::Excluded { .. }
        ),
        "crediting the requester resolved its own handoff"
    );

    let mut accepted = base;
    accepted.push(verdict_by(
        "codex-b",
        "accepted",
        "33333333333333333333333333333333",
        &receipt,
    ));
    assert_eq!(
        review_eligibility(&accepted, "claude-a"),
        ReviewEligibility::Eligible
    );
    assert_eq!(
        qualifying_review_verdict(&accepted, &receipt)
            .expect("a qualifying verdict")
            .reviewer,
        "codex-b"
    );

    accepted.push(verdict_by(
        "codex-b",
        "rejected",
        "44444444444444444444444444444444",
        &receipt,
    ));
    assert!(qualifying_review_verdict(&accepted, &receipt).is_none());
    assert_eq!(
        review_eligibility(&accepted, "claude-a"),
        ReviewEligibility::Eligible,
        "the later rejection resolves the transfer but withdraws delivery qualification"
    );
}

#[test]
fn stale_edited_untrusted_and_prose_verdicts_never_resolve_a_handoff() {
    let receipt = ReviewReceipt {
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
                review_eligibility(&comments, "claude-a"),
                ReviewEligibility::Excluded { .. }
            ),
            "non-qualifying evidence released the publisher"
        );
    }

    let mut edited_handoff = handoff;
    edited_handoff.includes_created_edit = true;
    assert_eq!(
        review_eligibility(&[published(&receipt), edited_handoff], "claude-a"),
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
    let receipt = ReviewReceipt {
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
            "review-handoff",
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
            operation_marker(&[first, conflict], operation, "review-handoff", &expected,)
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
    let root = tempfile::tempdir().expect("a context root");
    let context = |value: &str| super::super::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: vec![("Review delegation".to_owned(), value.to_owned())],
        repo: None,
    };
    let receipt = ReviewReceipt {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };

    // Every spelling the configuration reader accepts is one the transport
    // accepts, and what it writes down is one the marker reader reads back.
    for spelling in [
        "auto", "Auto", "AUTO", "ask", "Ask", "ask 30m", "Ask 30m", "ask 30 m", "ask  30m",
        "ask 2h", "ask 45s",
    ] {
        assert!(
            crate::config::authority_of(spelling).is_some(),
            "{spelling} is not a value the configuration accepts; fix the fixture, not the rule"
        );
        let (recorded, _deadline) = review_authority(&context(spelling), "2026-07-26T04:00:00Z")
            .unwrap_or_else(|failure| {
                panic!(
                    "{spelling} was refused by the transport: {:?}",
                    failure.envelope()
                )
            });
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
        assert!(
            review_authority(&context(broken), "2026-07-26T04:00:00Z").is_err(),
            "{broken} was accepted by the transport"
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
            review_eligibility(&comments, "codex-b"),
            ReviewEligibility::Eligible,
            "{broken} was read as a durable handoff"
        );
    }
}

#[test]
fn review_authority_records_one_deadline_without_turning_auto_into_a_capability() {
    let root = tempfile::tempdir().expect("a context root");
    let context = |value: &str| super::super::Context {
        skill_dir: root.path().to_path_buf(),
        repo_dir: root.path().to_path_buf(),
        config: vec![("Review delegation".to_owned(), value.to_owned())],
        repo: None,
    };
    assert_eq!(
        review_authority(&context("ask 30m"), "2026-07-26T04:00:00Z").expect("a timed request"),
        ("ask 30m".to_owned(), "2026-07-26T04:30:00Z".to_owned())
    );
    assert_eq!(
        review_authority(&context("auto"), "2026-07-26T04:00:00Z")
            .expect("auto records immediately"),
        ("auto".to_owned(), "2026-07-26T04:00:00Z".to_owned())
    );
}

#[test]
fn the_compound_handoff_records_before_release_and_checks_review_afterwards() {
    let source = include_str!("../claim.rs");
    let body = source
        .split_once("pub fn handoff_review(")
        .expect("the compound operation exists")
        .1
        .split_once("pub struct VerdictReview")
        .expect("the compound operation ends")
        .0;
    let verify = body.find("verify_claim(").expect("renews first");
    let receipt = body
        .find("require_latest_receipt")
        .expect("checks the receipt");
    let record = body.find("comment_with_body").expect("records the handoff");
    let receipt_retry = body
        .rfind("require_latest_receipt")
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
fn ci_release_spends_the_receipt_only_after_a_qualifying_verdict() {
    let source = include_str!("../claim.rs");
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
        .find("qualifying_review_verdict")
        .expect("checks the distinct verdict");
    let ready = body.find("ready_write").expect("marks the PR ready");
    assert!(receipt < verdict && verdict < ready);
}

#[test]
fn a_release_requires_the_latest_complete_publication_receipt() {
    let first = ReviewReceipt {
        epoch: "11111111111111111111111111111111".to_owned(),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    };
    let second = ReviewReceipt {
        epoch: "22222222222222222222222222222222".to_owned(),
        head: "d".repeat(40),
        ..first.clone()
    };
    let comments = [published(&first), published(&second)];

    assert_eq!(
        recorded_receipt(&comments, "claude-a", &second).expect("the latest receipt"),
        second
    );
    assert_eq!(
        reason(recorded_receipt(&comments, "claude-a", &first).expect_err("an old epoch")),
        "published-receipt-mismatch"
    );

    let foreign = ReviewReceipt {
        epoch: "33333333333333333333333333333333".to_owned(),
        head: "e".repeat(40),
        ..second.clone()
    };
    let cross_run = [published(&second), published_by("codex-b", &foreign)];
    assert_eq!(
        reason(
            recorded_receipt(&cross_run, "claude-a", &second)
                .expect_err("another run republished later")
        ),
        "published-receipt-mismatch"
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
    assert_eq!(
        reason(recorded_receipt(&[incomplete], "claude-b", &first).expect_err("incomplete")),
        "published-receipt-missing"
    );
}

#[test]
fn an_ambiguous_ready_write_is_settled_only_by_exact_readback() {
    let receipt = ReviewReceipt {
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
    let receipt = ReviewReceipt {
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
    let source = include_str!("../claim.rs");
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
fn the_ordinary_publication_has_no_way_to_force() {
    // The last acceptance criterion, and the one that decays silently: a boolean
    // added to `Publication` later, or a `Push` chosen from an argument, would
    // reintroduce exactly the implicit force the operation was split in two to
    // avoid. `publish_review` names its variant as a literal and takes nothing
    // that could name the other.
    let source = include_str!("../claim.rs");
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
    assert_eq!(latest_publication(&[]), None);

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
            .expect("a complete marker is a receipt")
            .receipt
            .head,
        head,
        "the lease would be taken against something other than the recorded head"
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
/// Spelled out to the count because the sentence before it said *"every
/// reachable combination"* while covering five of six and naming an unreachable
/// one, in the hunk rewritten to make that claim precise. A claim about coverage
/// is a claim, and this change has been corrected for making one loosely more
/// than once.
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
    assert_eq!(classify_lane_dispatch(0, ""), LaneDispatch::Started);

    for said in [
        "HTTP 403: Resource not accessible by personal access token",
        "gh: HTTP 401: Bad credentials",
    ] {
        assert!(
            matches!(classify_lane_dispatch(1, said), LaneDispatch::Forbidden(_)),
            "a permission refusal was not read as one: {said}"
        );
    }
    for said in [
        "could not find any workflows named ci.yml",
        "gh: HTTP 404: Not Found",
        "gh: HTTP 422: Workflow does not have 'workflow_dispatch' trigger",
    ] {
        assert!(
            matches!(classify_lane_dispatch(1, said), LaneDispatch::Absent(_)),
            "a repository with no dispatchable lane would have been refused a publication: {said}"
        );
    }
    assert!(
        matches!(
            classify_lane_dispatch(1, "dial tcp: lookup api.github.com: no such host"),
            LaneDispatch::Unknown(_)
        ),
        "a call that did not answer was read as an answer"
    );
}

/// The receipt comment says what this publication did about CI, and says it per
/// lane.
///
/// **The test that did not exist**, and its absence is why the receipt spent a
/// whole change contradicting the answer returned beside it. The dispatch sits
/// immediately above the call that writes this comment, so `publish_review`
/// answered `"publication_lane": "started"` — with a note saying an accepted
/// verdict waits on that run — while posting a comment that said *"CI remains
/// blocked while the PR is draft"*. Two sentences from one call, disagreeing,
/// on issue #30's own timeline twice. The four contract sentences that had to
/// move were found by grepping `.md` files; this fifth one is emitted at run
/// time, so no grep over documents could reach it and nothing here read it.
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
fn the_publication_receipt_says_what_it_did_about_ci_and_says_it_per_lane() {
    let marker = "<!-- issue-flow: published run-id=claude-abcd1234 -->";
    let note = |lane: &LaneDispatch| {
        publication_note(
            "https://github.com/o/r/pull/7",
            &"e".repeat(32),
            &"b".repeat(40),
            &"c".repeat(40),
            &"d".repeat(64),
            lane,
            marker,
        )
    };

    for (label, lane) in [
        ("started", LaneDispatch::Started),
        ("absent", LaneDispatch::Absent("HTTP 404".to_owned())),
        ("unknown", LaneDispatch::Unknown("no such host".to_owned())),
    ] {
        let text = note(&lane);
        for line in [
            &format!("- epoch `{}`", "e".repeat(32)),
            &format!("- head `{}`", "b".repeat(40)),
            &format!("- base `{}`", "c".repeat(40)),
            &format!("- target `{}`", "d".repeat(64)),
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
        assert!(
            text.contains("ci.yml"),
            "the {label} receipt names no publication lane at all: {text}"
        );
    }

    // What each state says about a verdict, which is the thing the receipt is
    // read for. `started` is the only one that may promise a run.
    assert!(
        note(&LaneDispatch::Started).contains(
            "was started against this head as this receipt was recorded, and an accepted verdict \
             cannot be recorded until it is green"
        ),
        "a started lane does not tell the reviewer a verdict waits on it: {}",
        note(&LaneDispatch::Started)
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
