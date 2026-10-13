use super::*;

/// Moving an issue to the state it already holds must leave the label on it.
///
/// Measured on 2026-08-15: `transition --from done --to done` built
/// `gh issue edit 3 --add-label status:done --remove-label status:done`, a
/// self-cancelling edit, and the issue came out of it carrying **no** state
/// label at all. `list_state` could then see it in no partition and every
/// `verify_claim --expect-state` disagreed with it.
///
/// The label is read back out of the stand-in's store rather than off the
/// answer, because the answer is the half that was already lying: the refusal
/// said *nothing was written* over the write that had just happened. A store the
/// call's own edits mutate is the only thing here that can tell a label that
/// survived from a label the fixture was told to repeat.
///
/// The board is measured in the same call, and it is measured because the mirror
/// runs **first**: a `from == to` call that stopped touching the label must not
/// quietly stop mirroring either, and *the board did not move* is not something
/// this test may infer from the label half.
#[test]
fn a_transition_to_the_state_an_issue_already_holds_keeps_its_label() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let labels = trace.path().join("labels.json");
    let log = trace.path().join("calls.log");
    std::fs::write(
        &labels,
        serde_json::json!(["priority:high", "status:done"]).to_string(),
    )
    .expect("the issue's labels are written");

    // The issue's own project, so *the projection moved* is a fact read off the
    // wire rather than the absence of one.
    let items = serde_json::json!({
        "data": { "repository": { "issue": { "projectItems": {
            "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
            "nodes": [{
                "id": "PVTI_1",
                "project": {
                    "id": "PVT_1",
                    "number": 7,
                    "title": "Sprint",
                    "fields": { "nodes": [{
                        "id": "PVTSSF_1",
                        "name": "Status",
                        "options": [{ "id": "opt_done", "name": "Done", "description": "status:done" }],
                    }] },
                },
                "fieldValueByName": { "name": "Done" },
            }],
        } } } }
    })
    .to_string();
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "updateProjectV2ItemFieldValue",
            "stdout": serde_json::json!({
                "data": { "updateProjectV2ItemFieldValue": { "projectV2Item": { "id": "PVTI_1" } } }
            })
            .to_string(),
            "status": 0,
        },
        { "matches": "projectItems", "stdout": items, "status": 0 },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        { "matches": "issue view", "labels": true, "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "transition",
            "arguments": {
                "issue": 12,
                "to": "done",
                "from": "done",
                "run_id": "claude-abcd1234",
            },
        },
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LABELS", &labels)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    // The criterion, and it is the labels rather than the answer: whatever this
    // call decided, the issue still wears the state it was asked to be moved to.
    let held: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(&labels).expect("the label store is readable"),
    )
    .expect("the label store is a list of names");
    assert!(
        held.iter().any(|name| name == "status:done"),
        "a transition to the state the issue already held stripped its state label, leaving \
         {held:?}: {text}"
    );

    let calls = std::fs::read_to_string(&log).expect("the call log is readable");
    assert!(
        !calls.contains("--remove-label status:done"),
        "the state being moved to was sent for removal in the same edit that added it: {calls}"
    );
    assert_eq!(
        response["result"]["isError"], false,
        "asserting the state an issue is already in was refused: {text}"
    );

    // The projection half. The mirror runs before the label edit and answers in
    // its own words; a `from == to` call that stopped writing the label must not
    // have stopped projecting, and *it did not move* may not be inferred from the
    // label assertions above.
    let answer: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| panic!("the transition answer is JSON: {text}"));
    assert_eq!(
        answer["board"]["projects"][0]["set_to"], "Done",
        "the projection did not report moving the project's card to the state asked for: {text}"
    );
    let mirrored = calls
        .find("updateProjectV2ItemFieldValue")
        .expect("the project mutation is on the wire");
    let edited = calls
        .find("issue edit")
        .expect("the label edit is on the wire");
    assert!(
        mirrored < edited,
        "the projection no longer runs before the label edit: {calls}"
    );
}

/// A refusal raised after the label edit landed has to say the edit landed.
///
/// `label-readback-failed` is a `Stop`, and a `Stop` carrying no `world` renders
/// as *nothing was written* with *do not repeat this call* under it. Both
/// sentences are false here: `gh issue edit` has already run, and the call that
/// repairs what it left is `transition --to <state>` with `from` omitted — the
/// one this refusal was telling the caller not to make.
/// `MutationOutcome::Committed` was opened for exactly this, and `transition` was not
/// covered by it.
///
/// Independent of the same-state half by construction: the transition here is a
/// genuine `from != to` move, so the `from == to` fix decides nothing about it.
#[test]
fn a_label_readback_that_fails_after_the_edit_landed_reports_the_write_and_names_the_repair() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let labels = trace.path().join("labels.json");
    let log = trace.path().join("calls.log");
    // Two state labels, and `--from` names only one of them. The edit lands and
    // the read-back still disagrees, which is the shape that reaches the stop
    // with a write behind it.
    std::fs::write(
        &labels,
        serde_json::json!(["status:review", "status:blocked"]).to_string(),
    )
    .expect("the issue's labels are written");

    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        { "matches": "issue view", "labels": true, "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "transition",
            "arguments": {
                "issue": 12,
                "to": "done",
                "from": "review",
                "run_id": "claude-abcd1234",
            },
        },
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LABELS", &labels)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    // The floor: this must still be the refusal it always was, on the same
    // reason word, over an edit that really did land.
    assert_eq!(
        response["result"]["isError"], true,
        "a two-state read-back was answered as a success: {text}"
    );
    assert!(
        text.contains("label-readback-failed"),
        "the refusal changed its reason word, which `tests/honesty.rs` crosses against the \
         documented inventory: {text}"
    );
    let held: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(&labels).expect("the label store is readable"),
    )
    .expect("the label store is a list of names");
    assert_eq!(
        held,
        vec!["status:blocked".to_owned(), "status:done".to_owned()],
        "the fixture did not reach the state this test is about"
    );

    // The sentence a caller reads, not the JSON key on its own.
    assert!(
        text.contains("the write landed; what failed came after it"),
        "a refusal raised after the label edit landed still says the world is untouched: {text}"
    );
    assert!(
        !text.contains("nothing was written"),
        "the refusal reports that nothing was written over an edit that landed: {text}"
    );
    assert!(
        !text.contains("do not repeat this call"),
        "the refusal still forbids the call, and the repair is a call: {text}"
    );
    // And the repair is named. `transition --to <state>` with `from` omitted is
    // what the tool's own contract documents as removing whatever stale state
    // labels are found, and it is what restored the issue.
    assert!(
        text.contains("transition --to done") && text.contains("`from` omitted"),
        "the refusal does not name the call that repairs it: {text}"
    );
}

/// The call the refusal above names has to be the call that clears it.
///
/// `CLAUDE.md`: never name a command in a message unless running it clears the
/// block, because naming a dead end is worse than naming nothing. The refusal
/// next door now names `transition --to <state>` with `from` omitted, so what
/// that call does to a two-state item is no longer somebody else's test.
///
/// It also holds the arm this change did **not** touch: `from` omitted removes
/// every stale state label except the one being added, and leaves every label
/// that is not a state alone.
#[test]
fn the_repair_a_failed_read_back_names_clears_the_item_it_was_named_over() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let labels = trace.path().join("labels.json");
    let log = trace.path().join("calls.log");
    // Exactly what the refusal next door leaves behind, plus a label that is
    // nobody's state: a predicate answering yes to everything would take the
    // operator's triage with it.
    std::fs::write(
        &labels,
        serde_json::json!(["status:blocked", "status:done", "priority:high"]).to_string(),
    )
    .expect("the issue's labels are written");

    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        { "matches": "issue view", "labels": true, "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "transition",
            "arguments": { "issue": 12, "to": "done", "run_id": "claude-abcd1234" },
        },
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LABELS", &labels)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    assert_eq!(
        response["result"]["isError"], false,
        "the call the refusal names does not clear the state it was named over: {text}"
    );
    let held: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(&labels).expect("the label store is readable"),
    )
    .expect("the label store is a list of names");
    assert_eq!(
        held,
        vec!["status:done".to_owned(), "priority:high".to_owned()],
        "the repair left the wrong labels on the issue"
    );
}

/// A finding says what it is, how it was found and what it costs, or it is not
/// recorded at all.
///
/// A classification with no evidence cannot be re-run and one with no impact
/// cannot be weighed, and either is a preference wearing the word `severe`.
/// Since a severe finding is now what authorises a rejection, this is the gate
/// on the thing the gate above reads.
///
/// The same holds one layer further down: a severe finding states **how** its
/// claim is established (`evidence_class`) and **whose defect** it says is on
/// the timeline (`causal_disposition`). Warnings and suggestions state neither,
/// which keeps the cheap observation cheap.
#[test]
fn a_finding_states_its_class_evidence_and_impact_or_it_is_not_recorded() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let publisher = "claude-abcd1234";
    let reviewer = "gemini-c0ffee00";
    let epoch = "e".repeat(32);
    // Real object ids, not arbitrary hex: a severe finding whose disposition
    // names this change as the cause now owes `record_review_finding` a
    // location it checks against `git diff base head`, and a diff over hex
    // that names no commit answers nothing to check it against.
    let (base, head) = commit_a_changed_line(repo, "changed.txt");
    let location = "changed.txt:1";
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let claimed_by = |run: &str| {
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run),
                    ("runtime", "gemini"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        )
    };
    let publication = comment(
        "IC_publication",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    );

    // The publication under review is `epoch`. `names` is what the call claims to
    // judge, and the last three rows are the reason this column exists: the
    // operation validated the receipt\'s **shape** and never asked whether it was
    // the receipt anybody is reviewing. A finding against a superseded epoch, or
    // against one no publication ever had, recorded `ok: true` and sat on the
    // timeline looking like evidence — while two shipped sentences said it was
    // refused. Four reviewers found it and two drove it through this surface.
    let superseded = "1".repeat(32);
    let invented = "2".repeat(32);
    // Who holds the live claim. Every row but the last is the recording run
    // itself; the last is somebody else, which is the one shape that makes
    // `verify_claim` observable here. A reviewer deleted that call outright and
    // the whole suite stayed green — the rule `CLAUDE.md` puts first, on a write
    // that lands on somebody else\'s issue.
    let intruder = "codex-beef0000";
    // `replayed` puts this finding\'s own marker on the timeline **before** the
    // call, so the call is a retry of a write that already landed rather than a
    // first one. It exists for one row, and that row is about *where* the
    // currency check sits rather than whether it is there: a reviewer moved it
    // below the replay branch and the whole suite stayed green, while the
    // comment at the call site claims the placement is deliberate. A retry whose
    // receipt was superseded in the meantime is not a retry of anything still
    // under review, and without this row it answers `ok: true, blocking: true`
    // and names no epoch at all.
    // `bytes` is the head the call names, empty for the published one. It is
    // here for a single row, and that row is the half of the currency check
    // nothing reached: `require_recorded_receipt` compares the **whole** receipt,
    // and every other row that must be refused differs in its epoch, so
    // weakening the comparison to the epoch alone left the suite green. Same
    // epoch, different head is exactly the forgery a whole-receipt comparison
    // exists to stop.
    let other_head = "5".repeat(40);
    // Two fields after `impact`: how what is claimed was established
    // (`evidence_class`) and who owns its cause on the timeline
    // (`causal_disposition`). A severe finding states both; a warning or a
    // suggestion carries none, which is the last row's whole point — pricing
    // the cheap observation out was the defect severity classes were invented
    // to end.
    for (
        label,
        holder,
        names,
        bytes,
        class,
        id,
        evidence,
        impact,
        finding_evidence,
        finding_causality,
        replayed,
        refused,
    ) in [
        // A fourth class nobody defined. The names are the vocabulary the
        // decision tables are written in, so an unknown one is a finding no
        // table can weigh rather than a stricter one.
        (
            "unknown-class",
            reviewer,
            &epoch,
            "",
            "critical",
            "x",
            "e",
            "i",
            "",
            "",
            false,
            true,
        ),
        (
            "no-id",
            reviewer,
            &epoch,
            "",
            "severe",
            "   ",
            "e",
            "i",
            "deterministic",
            "introduced",
            false,
            true,
        ),
        (
            "no-evidence",
            reviewer,
            &epoch,
            "",
            "severe",
            "x",
            "",
            "i",
            "deterministic",
            "introduced",
            false,
            true,
        ),
        (
            "no-impact",
            reviewer,
            &epoch,
            "",
            "severe",
            "x",
            "e",
            "  ",
            "deterministic",
            "introduced",
            false,
            true,
        ),
        // Severe without a stated evidence class or causal disposition. The
        // prose says a classification with no basis cannot be weighed; the
        // refusal here is where that stops being a promise.
        (
            "undeclared-evidence",
            reviewer,
            &epoch,
            "",
            "severe",
            "x",
            "e",
            "i",
            "",
            "introduced",
            false,
            true,
        ),
        (
            "undeclared-disposition",
            reviewer,
            &epoch,
            "",
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "",
            false,
            true,
        ),
        // A word outside either vocabulary. A declaration nobody chose answers
        // nothing, exactly like the unknown class the first row refuses.
        (
            "unknown-evidence-word",
            reviewer,
            &epoch,
            "",
            "severe",
            "x",
            "e",
            "i",
            "narrated",
            "introduced",
            false,
            true,
        ),
        (
            "unknown-disposition-word",
            reviewer,
            &epoch,
            "",
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "blamed",
            false,
            true,
        ),
        (
            "complete",
            reviewer,
            &epoch,
            "",
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "introduced",
            false,
            false,
        ),
        // Well-formed, and not the publication under review.
        (
            "superseded-receipt",
            reviewer,
            &superseded,
            "",
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "introduced",
            false,
            true,
        ),
        // Well-formed, and never published at all.
        (
            "invented-receipt",
            reviewer,
            &invented,
            "",
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "introduced",
            false,
            true,
        ),
        // The retry, over bytes that stopped being under review between the
        // first attempt and this one.
        (
            "replayed-superseded",
            reviewer,
            &superseded,
            "",
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "introduced",
            true,
            true,
        ),
        // The receipt this issue published, named by a head it never had.
        (
            "same-epoch-other-head",
            reviewer,
            &epoch,
            &other_head,
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "introduced",
            false,
            true,
        ),
        // A retry of a write that already landed, over the receipt still under
        // review. It is the replay branch answering, and its `blocking` was
        // pinned by nothing: the round that added the assertion added it to the
        // write path only, so half the declared repair was still green.
        (
            "replayed-complete",
            reviewer,
            &epoch,
            "",
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "introduced",
            true,
            false,
        ),
        // Complete, current, and recorded by a run that does not hold the
        // claim. Nothing else in this table can see `verify_claim`.
        (
            "claim-elsewhere",
            intruder,
            &epoch,
            "",
            "severe",
            "x",
            "e",
            "i",
            "deterministic",
            "introduced",
            false,
            true,
        ),
        // A bounded risk with neither declaration. Asking one to justify itself
        // would price the cheap observation out again — the reason these two
        // fields exist for `severe` alone.
        (
            "warning-carries-no-declaration",
            reviewer,
            &epoch,
            "",
            "warning",
            "y",
            "e",
            "i",
            "",
            "",
            false,
            false,
        ),
    ] {
        let log = trace.path().join(format!("finding-{label}.log"));
        let named_head = if bytes.is_empty() { &head } else { bytes };
        let operation_key = estigia::transport::claim::review_operation_id(
            "review-finding",
            &[
                holder, holder, names, "7", named_head, &base, &digest, id, class,
            ],
        );
        // Absent declarations write no attribute at all rather than an empty
        // one: absence is what the transport reads back, and a stored empty
        // word would make every later reader tell a declaration from its lack.
        let mut finding_fields: Vec<(&str, &str)> = vec![
            ("run-id", holder),
            ("reviewer", holder),
            ("op-id", &operation_key),
            ("epoch", names),
            ("pr", "7"),
            ("head", named_head),
            ("base", &base),
            ("digest", &digest),
            ("id", id),
            ("class", class),
            ("evidence", evidence),
            ("impact", impact),
        ];
        if !finding_evidence.is_empty() {
            finding_fields.push(("evidence-class", finding_evidence));
        }
        if !finding_causality.is_empty() {
            finding_fields.push(("causal-disposition", finding_causality));
            // Owed by every row here that declares a causality: `location`
            // is what `causal_disposition` now has to be checkable against,
            // and this table's rows are about the other declarations, not
            // about the location check itself.
            finding_fields.push(("location", location));
        }
        let written = comment(
            "IC_finding",
            "2026-08-14T10:30:00Z",
            marker("review-finding", &finding_fields),
        );
        let timeline = |after: bool| -> String {
            let mut comments = vec![claimed_by(holder), publication.clone()];
            if after || replayed {
                comments.push(written.clone());
            }
            serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            })
            .to_string()
        };
        let mut script = vec![];
        for before in 1..=3 {
            script.push(serde_json::json!({
                "matches": "issue view", "nth": before,
                "stdout": timeline(false), "status": 0,
            }));
        }
        script.extend([
            serde_json::json!({ "matches": "issue view", "stdout": timeline(true), "status": 0 }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]);
        let answers = serde_json::to_string(&script).expect("the fake tracker script serialises");

        forget_store(home);
        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{reviewer}.json")));
        let mut run = estigia::harness::session::Run::new(reviewer.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        // The undeclared word is omitted rather than sent empty: an optional
        // enum argument that sent "" would be refused for its shape, which is
        // not the refusal these rows exist to meet.
        let mut arguments = serde_json::json!({
            "issue": 12,
            "run_id": reviewer,
            "reviewer": reviewer,
            "epoch": names,
            "pr": 7,
            "head": named_head,
            "base": base,
            "digest": digest,
            "id": id,
            "class": class,
            "evidence": evidence,
            "impact": impact,
        });
        if !finding_evidence.is_empty() {
            arguments["evidence_class"] = finding_evidence.into();
        }
        if !finding_causality.is_empty() {
            arguments["causal_disposition"] = finding_causality.into();
            arguments["location"] = location.into();
        }
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "record_review_finding", "arguments": arguments }
        })
        .to_string();

        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env(
                "ESTIGIA_FAKE_COUNT",
                trace.path().join(format!("finding-{label}-count.json")),
            )
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        // Two doors refuse here and the row does not care which one answered:
        // the tool schema rejects a class outside the enum before the transport
        // is reached, while an empty field is refused by the transport and comes
        // back as a tool error. Asserting on the *shape* of the refusal would
        // pin which door answers, and moving a check between them is not a
        // regression — writing anything to the timeline is.
        let denied = response["error"].is_object() || response["result"]["isError"] == true;
        assert_eq!(
            denied, refused,
            "{label}: the finding was not adjudicated the way this row states: {text} ::              {response}"
        );
        if refused {
            assert!(
                !calls.contains("issue comment"),
                "{label}: a refused finding still wrote to the timeline: {calls}"
            );
        } else if replayed {
            // Nothing new is written — that is what a replay is — and the answer
            // comes from the marker already on the timeline rather than from the
            // arguments. `blocking` is the field an orchestrator acts on, and on
            // this branch it was reported by the code and read by no test.
            let answer: serde_json::Value =
                serde_json::from_str(&text).expect("the finding answer is JSON");
            assert_eq!(
                answer["reused_existing_finding"].as_bool(),
                Some(true),
                "{label}: a retry of a finding already on the timeline wrote a second one: {text}"
            );
            assert_eq!(
                answer["blocking"].as_bool(),
                Some(class == "severe"),
                "{label}: the replay reports a blocking disposition that disagrees with the class \
                 it read back, so a retry after an unreadable answer is told its severe finding is \
                 a note: {text}"
            );
            assert_eq!(
                answer["evidence_class"].as_str(),
                (!finding_evidence.is_empty()).then_some(finding_evidence),
                "{label}: the replay answers with a declaration the timeline does not hold, so \
                 what stands and what a retry reads back disagree: {text}"
            );
            assert_eq!(
                answer["causal_disposition"].as_str(),
                (!finding_causality.is_empty()).then_some(finding_causality),
                "{label}: the replay answers with a causality the timeline does not hold, so \
                 what stands and what a retry reads back disagree: {text}"
            );
        } else {
            assert!(
                local_entries(home)
                    .iter()
                    .any(|body| body.contains("issue-flow: review-finding")),
                "{label}: a complete finding was never recorded: {calls}"
            );
            // The field a caller reads to know whether this finding can block.
            // It was returned by both the write and the replay path and asserted
            // by nothing, so pinning it to `false` left the suite green — the
            // answer would tell an orchestrator its severe finding is a note.
            let answer: serde_json::Value =
                serde_json::from_str(&text).expect("the finding answer is JSON");
            assert_eq!(
                answer["blocking"].as_bool(),
                Some(class == "severe"),
                "{label}: the answer reports a blocking disposition that disagrees with the \
                 class it recorded: {text}"
            );
            assert_eq!(
                answer["evidence_class"].as_str(),
                (!finding_evidence.is_empty()).then_some(finding_evidence),
                "{label}: the recorded declaration does not read back on the write path: {text}"
            );
            assert_eq!(
                answer["causal_disposition"].as_str(),
                (!finding_causality.is_empty()).then_some(finding_causality),
                "{label}: the recorded causality does not read back on the write path: {text}"
            );
        }
    }
}

/// A publication says what it descends from, and says it from the timeline.
///
/// The lineage half. Two rows, and the pair is the whole crossing:
///
/// - **A first publication records no parent at all.** Absence is the answer,
///   not a missing field, and a mechanism that invented one here would make
///   every first publication look like a repair of something.
/// - **A publication over a recorded one records that one.** The parent epoch,
///   head and digest in this fixture are deliberately *not* the ones being
///   published — a run that derived the parent from its own arguments, or that
///   accepted a parent from the caller, answers with the wrong bytes rather than
///   with nothing, which is why the fixture makes all three differ.
///
/// The delta is recomputed here from the two ends the answer itself reports
/// rather than by calling the function that produced it. Asserting
/// `fix_delta_digest(..) == fix_delta_digest(..)` would hold whatever that
/// function did, including returning a constant.
#[test]
fn a_publication_records_the_one_it_repairs_and_a_first_one_records_nothing() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-lineage";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    let sha = |what: &str| -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", what])
            .output()
            .expect("git answers");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    git(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]);
    git(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");
    git(&["checkout", "--quiet", "-b", branch]);
    // A real, resolvable commit: the lineage check runs `git merge-base
    // --is-ancestor` on it, and a fabricated hex string is not one.
    std::fs::write(repo.join("kept.txt"), "midway\n").expect("the parent's own file");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "the parent's own publication",
    ]);
    let parent_head = sha(branch);
    std::fs::write(repo.join("kept.txt"), "changed\n").expect("the changed file");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "the change",
    ]);
    let head = sha(branch);

    let body = trace.path().join("body.md");
    std::fs::write(&body, "Refs #12.\n").expect("the pull request body");

    // The rest of the parent, every part different from what is being
    // published, so a row that answers with its own receipt fails rather than
    // coincides.
    let parent_epoch = "7".repeat(32);
    let parent_digest = "8".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1",
        "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true,
        "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let earlier = serde_json::json!({
        "id": "IC_2",
        "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true,
        "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
             epoch={parent_epoch} -->\n"
        ),
    });

    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": head,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": true,
    });

    // `readable` is the third row and the reason this loop has one. The lineage
    // is read from the timeline with `gh issue view <n> --json comments`, the
    // only read in this operation asking for exactly that shape — so it can be
    // failed on its own without touching the reads before it. The code there
    // argues, at length, that a failed read is not an absent parent. Nothing
    // measured the argument: a reviewer turned that read into a silent `None`
    // and the whole suite stayed green, which is this repository\'s own
    // definition of a fix that is not tested.
    for (label, repairs, readable) in [
        ("first", false, true),
        ("repair", true, true),
        ("unreadable-lineage", true, false),
    ] {
        let mut comments = vec![claimed.clone()];
        if repairs {
            comments.push(earlier.clone());
        }
        let timeline = serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": "status:in-progress"}],
            "comments": comments,
        })
        .to_string();
        let mut script: Vec<serde_json::Value> = vec![];
        if !readable {
            script.push(serde_json::json!({
                "matches": "issues/12/comments", "nth": 2, "stdout": "", "status": 1,
            }));
        }
        let ordinary = serde_json::json!([
            { "matches": "issue view", "stdout": timeline, "status": 0 },
            { "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 },
            { "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 },
            { "matches": "json body", "stdout": serde_json::json!({"body": "names nothing"}).to_string(), "status": 0 },
            { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
            {
                "matches": "api graphql",
                "stdout": serde_json::json!({
                    "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                        "nodes": [],
                        "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                    } } } },
                }).to_string(),
                "status": 0,
            },
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]);
        script.extend(
            ordinary
                .as_array()
                .expect("the ordinary script is an array")
                .iter()
                .cloned(),
        );
        let answers = serde_json::to_string(&script).expect("the fake tracker script serialises");

        forget_store(home);
        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "publish_review", "arguments": {
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "pr_title": "Something",
                "pr_body_file": body.display().to_string(),
                "worktree": repo.display().to_string(),
            }}
        })
        .to_string();

        let log = trace.path().join(format!("lineage-{label}.log"));
        let temp = trace.path().join(format!("temp-{label}"));
        std::fs::create_dir_all(&temp).expect("a temporary directory for the child");
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_LOG", &log)
            .env("ESTIGIA_FAKE_COUNT", temp.join("count.json"))
            .env("TMPDIR", &temp)
            .env("TMP", &temp)
            .env("TEMP", &temp)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if !readable {
            // A read that did not answer is not an answer. Recording this as a
            // first publication would erase a lineage rather than report that it
            // could not be seen — and it would disable both lineage refusals for
            // the round after it, silently, on the one issue where a repair is
            // under review.
            //
            // The **reason** is asserted, not only the refusal. An earlier
            // version of this row checked `isError` alone and passed under the
            // very mutation it exists to catch, because the operation was
            // refusing something else entirely. A row that cannot say why it
            // went red measures nothing.
            assert_eq!(
                response["result"]["isError"], true,
                "{label}: an unreadable timeline was published as a first publication, so a \
                 repair records no parent and both lineage rules stop applying: {text}"
            );
            assert!(
                text.contains("publication lineage"),
                "{label}: the publication refused for some other reason, so this row proves \
                 nothing about the read it is here for: {text}"
            );
            let calls = std::fs::read_to_string(&log).unwrap_or_default();
            assert!(
                !calls.contains("issue comment"),
                "{label}: a publication whose lineage could not be read still posted a receipt, \
                 which is the erased lineage written down as evidence: {calls}"
            );
            continue;
        }
        assert_eq!(
            response["result"]["isError"], false,
            "{label}: the publication did not land: {text}"
        );
        let answer: serde_json::Value =
            serde_json::from_str(&text).expect("the publication answer is JSON");
        let receipt = staged_receipt(home, 12);

        if !repairs {
            // The lineage fields moved off `publish_review`'s own answer to
            // `review_transaction` — see `docs/review-cost.md` — so what a
            // first publication recorded (nothing) is checked on the staged
            // marker, the only place any of it lives now.
            assert!(
                !receipt.contains(" parent=") && !receipt.contains(" delta="),
                "{label}: a first publication staged a lineage in its receipt: {receipt}"
            );
            continue;
        }

        let published_digest = answer["digest"]
            .as_str()
            .expect("the target digest")
            .to_owned();
        // Recomputed from the two ends rather than from the function under test.
        let expected = estigia::transport::ownership::sha256_hex(
            [
                parent_epoch.as_str(),
                parent_head.as_str(),
                parent_digest.as_str(),
                head.as_str(),
                published_digest.as_str(),
            ]
            .join("\n")
            .as_bytes(),
        );
        // The lineage — `parent`, `parent_pr`, `parent_head`, `parent_base`,
        // `parent_digest`, `delta`, `delta_range`, `delta_range_reason` — moved
        // off `publish_review`'s own answer to `review_transaction`; see
        // `docs/review-cost.md`. What a repair actually recorded is checked
        // below, on the staged marker — the durable record `review_transaction`
        // itself reads back, and the only copy that survives this process. The
        // rest of the parent receipt is not decoration: the ledger a re-review
        // reads is matched on the whole receipt, and recording only the epoch
        // left `--parent` satisfiable by one backfilled marker, which is what
        // two reviewers drove. The honest-range half of `repair_range` is
        // exercised here too, which nothing else in this file drives: every
        // other fixture that reaches it takes the `WholeChange` arm.
        for spelled in [
            format!("parent={parent_epoch}"),
            "parent-pr=7".to_owned(),
            format!("parent-head={parent_head}"),
            format!("parent-base={base_sha}"),
            format!("parent-digest={parent_digest}"),
            format!("delta={expected}"),
            "delta-range=parent-head..head".to_owned(),
        ] {
            assert!(
                receipt.contains(&spelled),
                "{label}: the staged receipt does not carry `{spelled}`, so the repair's \
                 lineage will not be what review_transaction reads back: {receipt}"
            );
        }
        assert!(
            !receipt.contains("delta-range-reason="),
            "{label}: an honest narrow range carries no reason for falling back from it: {receipt}"
        );
    }
}

/// Across a repair, a finding says whether it is old work or new, and a new
/// blocker says why the repair is answerable for it.
///
/// The other half of the lineage. The parent ledger stays on the timeline —
/// nothing rewrites it — so what needs adjudicating is the *reference*: an
/// identity that names nothing makes a fresh observation read as settled work,
/// and a severe finding that names nothing at all is how a full-target resweep
/// arrives looking like a defect the repair caused.
///
/// Six rows. Four of them are refusals a mutation would turn green, one is the
/// reassessment the mechanism exists to allow, and one is the deliberate
/// asymmetry: a warning new to a repair owes nobody an explanation, because
/// pricing the cheap observation is the defect this whole change repairs.
#[test]
fn a_repair_finding_names_what_it_continues_or_says_why_it_is_new() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let publisher = "claude-abcd1234";
    let reviewer = "gemini-c0ffee00";
    let parent_epoch = "7".repeat(32);
    let parent_head = "9".repeat(40);
    let parent_digest = "8".repeat(64);
    let epoch = "e".repeat(32);
    // Real object ids, not arbitrary hex: every severe row below declares
    // `causal_disposition: introduced`, which now owes `record_review_finding`
    // a location it checks against `git diff base head`.
    let (base, head) = commit_a_changed_line(repo, "changed.txt");
    let location = "changed.txt:1";
    let digest = "d".repeat(64);
    let delta = "f".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let held = comment(
        "IC_claim",
        "2026-08-14T09:00:00Z",
        marker(
            "claim",
            &[
                ("run-id", reviewer),
                ("runtime", "gemini"),
                ("horizon", "2099-01-01T00:00Z"),
                ("op-id", &"a".repeat(32)),
            ],
        ),
    );
    // The publication that was reviewed, and the one severe finding it drew.
    // Both stay on the timeline: preserving settled work is not an operation
    // here, it is the absence of one.
    let earlier = comment(
        "IC_parent",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &parent_epoch),
                ("pr", "7"),
                ("head", &parent_head),
                ("base", &base),
                ("digest", &parent_digest),
            ],
        ),
    );
    let settled = comment(
        "IC_settled",
        "2026-08-14T09:30:00Z",
        marker(
            "review-finding",
            &[
                ("run-id", reviewer),
                ("reviewer", reviewer),
                ("op-id", &"1".repeat(32)),
                ("epoch", &parent_epoch),
                ("pr", "7"),
                ("head", &parent_head),
                ("base", &base),
                ("digest", &parent_digest),
                ("id", "old-defect"),
                ("class", "severe"),
                ("evidence", "the command that reddened"),
                ("impact", "what it cost"),
            ],
        ),
    );
    // The repair, with the lineage the publication derived. `lineage=false`
    // strips it, which is the only shape in which naming a parent is incoherent
    // rather than merely wrong.
    // A finding this reviewer recorded against the publication under review.
    // It is on the same timeline as the parent ledger and it is **not** part of
    // it: citing it as a parent would let a repair present its own fresh
    // observation as work that was already agreed.
    let current = comment(
        "IC_current",
        "2026-08-14T10:15:00Z",
        marker(
            "review-finding",
            &[
                ("run-id", reviewer),
                ("reviewer", reviewer),
                ("op-id", &"2".repeat(32)),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
                ("id", "this-epoch"),
                ("class", "warning"),
                ("evidence", "noticed while reviewing the repair"),
                ("impact", "small"),
            ],
        ),
    );
    // Recorded after the repair, naming the parent epoch and carrying the
    // repair\'s bytes. Nothing refuses writing it; what must be refused is
    // reading it as the parent ledger.
    let backfilled = comment(
        "IC_backfilled",
        "2026-08-14T10:20:00Z",
        marker(
            "review-finding",
            &[
                ("run-id", reviewer),
                ("reviewer", reviewer),
                ("op-id", &"3".repeat(32)),
                ("epoch", &parent_epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
                ("id", "invented-settled-work"),
                ("class", "severe"),
                ("evidence", "written after the repair was published"),
                ("impact", "none, which is the point"),
            ],
        ),
    );
    // `whole` distinguishes a lineage marker carrying the parent receipt entire
    // from one carrying part of it. `PublicationLineage::from_marker` argues at
    // its own site that a partial lineage must read as no lineage — all of it or
    // none, so one reader cannot see a repair where the next sees a first
    // publication — and nothing measured the argument: a reviewer made the
    // missing `parent-base` fall back to the child\'s and the whole suite stayed
    // green, which would hand both lineage rules a parent receipt that never
    // existed.
    let repair = |lineage: bool, whole: bool| {
        let mut fields: Vec<(&str, &str)> = vec![
            ("run-id", publisher),
            ("epoch", &epoch),
            ("pr", "7"),
            ("head", &head),
            ("base", &base),
            ("digest", &digest),
        ];
        if lineage {
            fields.push(("parent", &parent_epoch));
            fields.push(("parent-pr", "7"));
            fields.push(("parent-head", &parent_head));
            // Exactly one field withheld, and the one a reviewer proved could be
            // filled in from the child. Withholding two would let the second
            // `?` carry the row while the first was quietly defaulted, and the
            // mutation this exists to catch would stay green.
            if whole {
                fields.push(("parent-base", &base));
            }
            fields.push(("parent-digest", &parent_digest));
            fields.push(("delta", &delta));
        }
        comment(
            "IC_publication",
            "2026-08-14T10:00:00Z",
            marker("published", &fields),
        )
    };

    for (label, lineage, whole, id, class, parent, origin, refused, reason) in [
        // The reassessment: an identity the parent ledger holds.
        (
            "reassessed",
            true,
            true,
            "old-defect",
            "severe",
            "old-defect",
            "",
            false,
            "",
        ),
        // An identity it does not. Without this check `parent` is free text and
        // a fresh observation reads as already agreed.
        (
            "invented-parent",
            true,
            true,
            "new-defect",
            "severe",
            "never-found",
            "",
            true,
            "finding-parent-unknown",
        ),
        // A new blocker with nothing said about why the repair owns it.
        (
            "unexplained-blocker",
            true,
            true,
            "new-defect",
            "severe",
            "",
            "",
            true,
            "new-blocker-without-origin",
        ),
        // The same finding, explained.
        (
            "explained-blocker",
            true,
            true,
            "new-defect",
            "severe",
            "",
            "introduced",
            false,
            "",
        ),
        // The asymmetry: a warning new to a repair owes nothing.
        (
            "new-warning",
            true,
            true,
            "a-note",
            "warning",
            "",
            "",
            false,
            "",
        ),
        // The attack, and the reason the lineage carries the parent receipt
        // rather than its epoch. `backfilled` is a finding recorded after the
        // repair, naming the parent **epoch** while carrying the repair\'s own
        // head, base and digest — which is what a reviewer can write at any
        // time, since nothing binds a finding\'s epoch field to a publication.
        // Read by epoch it is a parent-ledger entry; read by receipt it is not
        // one, and citing it must be refused or `--parent` is satisfiable on
        // demand and `new-blocker-without-origin` is bypassed by one extra write.
        (
            "backfilled-parent",
            true,
            true,
            "new-defect",
            "severe",
            "invented-settled-work",
            "",
            true,
            "finding-parent-unknown",
        ),
        // A suggestion, new to the repair, owing nothing — the third class, which
        // no fixture exercised until a reviewer counted them.
        (
            "new-suggestion",
            true,
            true,
            "a-nit",
            "suggestion",
            "",
            "",
            false,
            "",
        ),
        // An identity that exists, on this timeline, against **this** epoch.
        // The parent ledger is the parent epoch and nothing else; reading the
        // whole timeline would make this row pass and let a repair cite its own
        // fresh observation as settled.
        (
            "parent-from-this-epoch",
            true,
            true,
            "new-defect",
            "severe",
            "this-epoch",
            "",
            true,
            "finding-parent-unknown",
        ),
        // The lineage marker carrying four of its five parent fields. Read
        // all-or-none it is no lineage at all, so a severe finding new to
        // this publication owes no origin and lands. Read leniently it is a
        // parent receipt nobody published, and the same finding is refused
        // for want of an origin against an ancestor that does not exist.
        (
            "partial-lineage",
            true,
            false,
            "new-defect",
            "severe",
            "",
            "",
            false,
            "",
        ),
        // A parent named against a publication that descends from nothing.
        (
            "parent-without-lineage",
            false,
            true,
            "new-defect",
            "severe",
            "old-defect",
            "",
            true,
            "finding-parent-without-lineage",
        ),
    ] {
        // These rows are about lineage, not about the declaration gate that
        // the table above proves — so every severe row carries valid
        // declarations and meets the refusal this row names.
        let (finding_evidence, finding_causality) = if class == "severe" {
            ("deterministic", "introduced")
        } else {
            ("", "")
        };
        let log = trace.path().join(format!("repair-{label}.log"));
        let mut fields: Vec<(&str, &str)> = vec![
            ("run-id", reviewer),
            ("reviewer", reviewer),
            ("epoch", &epoch),
            ("pr", "7"),
            ("head", &head),
            ("base", &base),
            ("digest", &digest),
            ("id", id),
            ("class", class),
            ("evidence", "a command that reddens"),
            ("impact", "what it costs"),
        ];
        if !parent.is_empty() {
            fields.push(("parent", parent));
        }
        if !origin.is_empty() {
            fields.push(("origin", origin));
        }
        if !finding_evidence.is_empty() {
            fields.push(("evidence-class", finding_evidence));
        }
        if !finding_causality.is_empty() {
            fields.push(("causal-disposition", finding_causality));
            // Owed by every severe row here: these rows are about lineage,
            // not about the location check, so every one of them carries a
            // location the diff between base and head actually supports.
            fields.push(("location", location));
        }
        let key = estigia::transport::claim::review_operation_id(
            "review-finding",
            &[
                reviewer, reviewer, &epoch, "7", &head, &base, &digest, id, class,
            ],
        );
        fields.insert(2, ("op-id", &key));
        let written = comment(
            "IC_written",
            "2026-08-14T10:30:00Z",
            marker("review-finding", &fields),
        );
        let timeline = |after: bool| -> String {
            let mut comments = vec![
                held.clone(),
                earlier.clone(),
                settled.clone(),
                repair(lineage, whole),
                current.clone(),
                backfilled.clone(),
            ];
            if after {
                comments.push(written.clone());
            }
            serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            })
            .to_string()
        };
        let mut script = vec![];
        for before in 1..=3 {
            script.push(serde_json::json!({
                "matches": "issue view", "nth": before,
                "stdout": timeline(false), "status": 0,
            }));
        }
        script.extend([
            serde_json::json!({ "matches": "issue view", "stdout": timeline(true), "status": 0 }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]);
        let answers = serde_json::to_string(&script).expect("the fake tracker script serialises");

        forget_store(home);
        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{reviewer}.json")));
        let mut run = estigia::harness::session::Run::new(reviewer.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let mut arguments = serde_json::json!({
            "issue": 12,
            "run_id": reviewer,
            "reviewer": reviewer,
            "epoch": epoch,
            "pr": 7,
            "head": head,
            "base": base,
            "digest": digest,
            "id": id,
            "class": class,
            "evidence": "a command that reddens",
            "impact": "what it costs",
        });
        if !parent.is_empty() {
            arguments["parent"] = serde_json::json!(parent);
        }
        if !origin.is_empty() {
            arguments["origin"] = serde_json::json!(origin);
        }
        if !finding_evidence.is_empty() {
            arguments["evidence_class"] = finding_evidence.into();
        }
        if !finding_causality.is_empty() {
            arguments["causal_disposition"] = finding_causality.into();
            arguments["location"] = location.into();
        }
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "record_review_finding", "arguments": arguments }
        })
        .to_string();

        // Where the body this call stages can be read back from. The answer
        // reports the *requested* parent and origin, so asserting on it proves
        // nothing about what the timeline will hold — and deleting both marker
        // fields left the whole suite green, because the readback deliberately
        // does not compare them. The durable ledger is the only copy a
        // re-review ever sees.
        let temp = trace.path().join(format!("repair-{label}-tmp"));
        std::fs::create_dir_all(&temp).expect("a temporary directory for the child");
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env(
                "ESTIGIA_FAKE_COUNT",
                trace.path().join(format!("repair-{label}-count.json")),
            )
            .env("ESTIGIA_FAKE_LOG", &log)
            .env("TMPDIR", &temp)
            .env("TMP", &temp)
            .env("TEMP", &temp)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        assert_eq!(
            response["result"]["isError"], refused,
            "{label}: the finding was not adjudicated the way this row states: {text}"
        );
        if refused {
            assert!(
                text.contains(reason),
                "{label}: refused for something other than `{reason}`, so this row proves \
                 nothing about the rule it is here for: {text}"
            );
            assert!(
                !calls.contains("issue comment"),
                "{label}: a refused finding still wrote to the timeline: {calls}"
            );
        } else {
            assert!(
                local_entries(home)
                    .iter()
                    .any(|body| body.contains("issue-flow: review-finding")),
                "{label}: the finding was allowed and never recorded: {calls}"
            );
            let staged = staged_receipt(home, 12);
            for (name, value) in [("parent", parent), ("origin", origin)] {
                if value.is_empty() {
                    assert!(
                        !staged.contains(&format!("{name}=")),
                        "{label}: the marker carries a {name} this call never supplied: {staged}"
                    );
                } else {
                    assert!(
                        staged.contains(&format!("{name}={value}")),
                        "{label}: the marker does not carry `{name}={value}`, so the ledger a                          re-review reads cannot tell a reassessment from a new observation:                          {staged}"
                    );
                }
            }
        }
    }
}

/// A claim over MCP hard-coded `--runtime`, so a non-Claude run
/// id's own claim used to project `dev:claude` instead of its own name.
///
/// Two calls, the same shape
/// [`a_claim_retry_after_a_landed_write_whose_readback_failed_adopts_the_epoch`]
/// uses: `mint_operation_id` is clock-seeded, so the marker cannot be scripted
/// until the pointer written by the first call names it.
#[test]
fn a_non_claude_run_id_projects_its_own_runtime_not_the_constant() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("calls.log");
    let run_id = "codex-abcd1234";
    let empty = serde_json::json!({
        "state": "OPEN",
        "assignees": [],
        "labels": [{"name": "status:ready"}],
        "comments": [],
    });
    let first = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": empty.to_string(), "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let first_out = tracker_command(home, repo, bin, &first)
        .env("ESTIGIA_FAKE_LOG", &log)
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            "2099-01-01T00:00Z",
        ])
        .output()
        .expect("estigia claim runs");
    assert!(
        !first_out.status.success(),
        "the minimal fixture's static readback was expected to leave the write ambiguous: {}",
        String::from_utf8_lossy(&first_out.stderr)
    );

    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        calls.contains("label create dev:codex"),
        "a non-Claude run id's own claim did not project its own runtime: {calls}"
    );
    assert!(
        !calls.contains("dev:claude"),
        "a non-Claude run id's claim recorded the constant instead of its own runtime: {calls}"
    );

    // The retry: now that the operation id is known, the second `issue view`
    // can carry the marker the first call actually wrote, and the retry
    // completes rather than repeating the ambiguity.
    let pointer = home
        .join(".estigia")
        .join("runs")
        .join(format!("{run_id}.json"));
    let recorded: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer).expect("the pointer is there"))
            .expect("the pointer is JSON");
    let epoch = recorded
        .get("operation_id")
        .and_then(|value| value.as_str())
        .expect("the mint was stored")
        .to_owned();
    let marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "codex"),
            ("horizon", "2099-01-01T00:00Z"),
            ("op-id", &epoch),
        ],
    )
    .expect("a claim marker");
    let held = serde_json::json!({
        "state": "OPEN",
        "assignees": [{"login": "fixture"}],
        "labels": [{"name": "status:ready"}, {"name": "dev:codex"}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-08-17T16:00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed by {run_id}.\n\n{marker}\n"),
        }],
    });
    let second = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": held.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the second script serialises");

    let second_out = tracker_command(home, repo, bin, &second)
        .env("ESTIGIA_FAKE_LOG", &log)
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            "2099-01-01T00:00Z",
        ])
        .output()
        .expect("the retry runs");
    assert!(
        second_out.status.success(),
        "the retry did not adopt the landed claim: {}",
        String::from_utf8_lossy(&second_out.stderr)
    );
}

/// Door 1, end to end: the id a caller writes becomes a
/// repository-level `dev:<runtime>` label — permanent, and the only thing the
/// label's own query reads. `codex-01k4-9f2a` is what the contract's own
/// instruction produces on a host whose session id carries a dash, and the
/// last-dash split reads its runtime as `codex-01k4`, a well-formed component,
/// so the earlier alphabet admitted it and minted `dev:codex-01k4` — invisible to
/// `gh issue list --label dev:codex`. The refusal now happens before the
/// tracker is asked anything, so nothing is created.
///
/// The floor is its sibling above: `codex-abcd1234` reaches
/// `label create dev:codex` through the same road.
#[test]
fn an_id_this_crate_could_not_have_minted_mints_no_label() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("calls.log");
    let nothing = serde_json::to_string(&serde_json::json!([])).expect("an empty script");

    let output = tracker_command(home, repo, bin, &nothing)
        .env("ESTIGIA_FAKE_LOG", &log)
        .args([
            "claim",
            "12",
            "--run-id",
            "codex-01k4-9f2a",
            "--horizon",
            "2099-01-01T00:00Z",
        ])
        .output()
        .expect("estigia claim runs");
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a run id this crate could not have minted was accepted: {said}"
    );
    assert!(
        said.contains("run-id-names-no-run"),
        "the refusal is not the run-id one, so this proves nothing about the shape: {said}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        calls.is_empty(),
        "the tracker was asked something before the id was refused: {calls}"
    );
}
