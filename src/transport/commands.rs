//! The transport's operations, ported one at a time.
//!
//! Each one was held to the Python's answer over the same input by
//! `tests/differential.rs`, and a command landed here only once that crossing
//! passed for it. The port is complete, and both the Python and the corpus are
//! deleted — so nothing gates an edit to this file against a second
//! implementation now. Only the unit tests beside each command do.

use super::{Context, Failure};

/// The operations this transport performs itself.
///
/// A hand-written list in the Python, and hand-written here for the same reason
/// it is hand-written there: it is a *claim about the contract*, not a
/// reflection of the dispatch table. What it must never do is drift from it.
/// `tests/differential.rs` is what used to stop that, and it is deleted; a name
/// added here that the dispatch does not answer is now caught by nothing.
///
/// The **other** direction is now crossed, by
/// `dispatch::tests::every_operation_a_tool_offers_is_claimed_or_declared_unscripted`.
/// It is the direction that bit: `republish-review` was dispatched, exposed as a
/// tool and left out of this list, so `estigia config` reported eighteen
/// scripted operations for a transport that answered nineteen — the drift this
/// comment predicts, on the first operation added since it was written.
pub(super) const SCRIPTED: &[&str] = &[
    "ensure-states",
    "create",
    "list-state",
    "claim",
    "reclaim",
    "verify-claim",
    "transition",
    "comment",
    "heartbeat",
    "start-branch",
    "publish-review",
    "republish-review",
    "handoff-review",
    "review-finding",
    "review-verdict",
    "release-ci",
    "unassign",
    "changelog-notes",
    "check-closing-keywords",
    "audit-board",
    // Read-only, and performed here all the same. Found by the crossing named
    // above the moment it was written, which is the point: this list had drifted
    // from the dispatch by three operations, not one, and `estigia config` was
    // under-reporting two of them for longer than the operation that prompted
    // the check. Both have rows in the GitHub binding's operations table, so the
    // contract already claimed them and only this answer did not.
    "expected-target",
    "base-movement",
];

/// What the transport deliberately does **not** do, and why.
///
/// The irreversible remote writes and the two judgements. This is the same line
/// the harness draws with `DELIVERS`: landing work needs a verdict, and a
/// verdict is not something a script produces.
pub(super) const NOT_SCRIPTED: &[(&str, &str)] = &[
    (
        "merge",
        "irreversible remote write — the agent runs it after verifying SHAs",
    ),
    (
        "publish_version",
        "irreversible remote write — annotated tags are never moved",
    ),
    (
        "close",
        "kept with merge/publish_version so delivery stays one prose-owned sequence",
    ),
    (
        "review_status/ci_status",
        "verdict and CI interpretation stay with the agent",
    ),
];

/// `config` — the resolved operator configuration.
///
/// The first command ported, and chosen first because it touches neither `gh`
/// nor `git`: it is the whole envelope with none of the world, so it proves the
/// shape before anything has to be mocked.
pub fn config(context: &Context) -> Result<serde_json::Value, Failure> {
    let board = super::board::Board::parse(
        context.get("project board").unwrap_or_default(),
        context,
        true,
    );
    let mut table = serde_json::Map::new();
    for (key, value) in &context.config {
        table.insert(key.to_lowercase(), serde_json::Value::String(value.clone()));
    }

    Ok(serde_json::json!({
        "ok": true,
        "skill_dir": context.skill_dir.display().to_string(),
        "config": table,
        "board": {
            "enabled": board.enabled,
            "owner": board.owner,
            "number": board.number,
        },
        "scripted_operations": SCRIPTED,
        "not_scripted": NOT_SCRIPTED
            .iter()
            .map(|(name, why)| ((*name).to_owned(), serde_json::Value::String((*why).to_owned())))
            .collect::<serde_json::Map<_, _>>(),
    }))
}

/// The workflow's six states, in the order the transport declares them.
///
/// **One list, re-exported rather than written again.** This spelled all six
/// out beside `crate::config::STATES`, which spells the same six — two places
/// holding one rule, agreeing today and with nothing making them agree
/// tomorrow. A state renamed in the configuration would go on creating the old
/// label here, so `ensure-states` would make six labels and `transition` would
/// move to a seventh that no `status:` label exists for.
///
/// It is the shape this crate's contributing note puts first — *prefer removing
/// a copy to adding a check* — and no check was watching this pair either.
///
/// Re-exported and not merely referenced, because the transport's callers ask
/// for it by this path and the schemas the tool server publishes come from
/// `config::STATES`: one name for one list, whichever door a reader arrives at.
pub use crate::config::STATES;

/// `ensure-states` — create the six status labels, idempotently.
///
/// Every call is `check=False` in the original and [`super::How::tolerated`] here, and
/// that is the whole design rather than laziness: `--force` makes creation
/// idempotent, so a label that already exists reports non-zero and means
/// *success*. Treating that as a failure would make the one command whose job is
/// to be safe to re-run the one command nobody can re-run.
///
/// Its reason for existing is ordering: `gh` refuses to attach a label that does
/// not exist, and the refusal arrives at `gh issue create` — the analyst's last
/// step, after all the analysis is done.
pub fn ensure_states(context: &Context) -> Result<serde_json::Value, Failure> {
    let ensured: Vec<String> = STATES
        .iter()
        .map(|state| format!("status:{state}"))
        .collect();
    for name in &ensured {
        super::run(
            &[
                "gh", "label", "create", name, "--color", "ededed", "--force",
            ],
            Some(&context.repo_dir),
            super::How::tolerated(),
        )?;
    }
    Ok(serde_json::json!({ "ok": true, "ensured": ensured }))
}

/// `comment` — post a markdown body from a file.
///
/// Two refusals before anything is written, and both are about the marker rather
/// than the prose:
///
/// - a kind outside `note`/`blocker`/`diagnosis` is **reserved**. The control
///   vocabulary lives in the same namespace, so letting a caller pass `standdown`
///   here would let an ordinary comment issue a stand-down.
/// - a run-id without a kind, or a kind without a run-id, is **incomplete**. A
///   marker addressed to nobody, or one from nobody, is a fact half-recorded, and
///   the timeline is what adjudicates ownership.
///
/// The body is escaped before the marker is appended, never after: escaping is
/// what keeps quoted evidence inert, and running it over the marker this command
/// just wrote would neuter its own.
pub fn comment(
    context: &Context,
    issue: u64,
    body_file: &std::path::Path,
    run_id: Option<&str>,
    kind: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    if let Some(kind) = kind
        && !super::markers::COMMENT_KINDS.contains(&kind)
    {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "reserved-comment-kind" }),
        ));
    }
    if run_id.is_some_and(|id| !id.is_empty()) != kind.is_some_and(|k| !k.is_empty()) {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "comment-marker-incomplete" }),
        ));
    }

    let raw = std::fs::read_to_string(body_file).map_err(|_| {
        Failure::Stop(serde_json::json!({ "ok": false, "reason": "comment-body-invalid" }))
    })?;
    let kind = kind.unwrap_or("note");
    let marker = super::markers::render(kind, &[("run-id", run_id.unwrap_or_default())])
        .ok_or_else(|| {
            Failure::Stop(serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }))
        })?;
    let body = format!(
        "{}\n\n{marker}\n",
        super::markers::escape_control_input(&raw).trim_end()
    );

    // Every markdown body goes through a file, with no exception for "this one
    // is short" — that inconsistency is what let evidence-bearing comments
    // arrive silently damaged.
    // Written atomically, because `gh` is a *reader of this file* and
    // `fs::write` truncates before it fills: a reader arriving inside that
    // window sees half a comment, and half a comment posted is worse than none.
    // The crate's own guard catches this, and it caught this.
    let path = crate::paths::scratch_file(&format!("{issue}-{kind}.md"));
    crate::paths::replace_atomically(&path, &body).map_err(|error| {
        Failure::Write(format!("the comment body could not be staged: {error}"))
    })?;
    let staged = path.display().to_string();
    let answer = super::run(
        &[
            "gh",
            "issue",
            "comment",
            &issue.to_string(),
            "--body-file",
            &staged,
        ],
        Some(&context.repo_dir),
        super::How::write(),
    );
    // Removed on both paths: the `gh` call between write and unlink raises on any
    // transient failure, and a plain unlink after it is simply skipped there.
    // One leaked file per failed comment is small, and small is what accumulates
    // silently for months.
    let _ = std::fs::remove_file(&path);
    answer?;

    Ok(serde_json::json!({ "ok": true, "issue": issue, "kind": kind }))
}

/// `list-state` — the unclaimed items in one state, partitioned by domain.
///
/// The ordering rule is the whole point, and it is a rule about **not** doing
/// something. Each partition is sorted oldest first, because age is the
/// workflow's tie-break and the only ordering this transport is entitled to
/// apply. Ranking by priority *inside* a partition needs that domain's own scale
/// contract, which this code does not know and must not invent — and ranking
/// *across* partitions is forbidden outright, because two scales have no common
/// unit. So every label goes back raw beside the age order and the agent applies
/// the contract it loaded.
pub fn list_state(
    context: &Context,
    state: &str,
    run_id: &str,
    limit: u32,
) -> Result<serde_json::Value, Failure> {
    let label = format!("status:{state}");
    // Kept before the shadowing below turns it into text: the answer has to
    // say whether this ceiling is what ended the list.
    let ceiling = u64::from(limit);
    let limit = limit.to_string();
    let data = super::gh_json(
        &[
            "issue",
            "list",
            "--label",
            &label,
            "--search",
            "no:assignee",
            "--json",
            "number,title,labels,createdAt",
            "--limit",
            &limit,
        ],
        Some(&context.repo_dir),
    )?;
    let items = data
        .as_ref()
        .and_then(serde_json::Value::as_array)
        .cloned()
        .ok_or_else(|| Failure::Read(format!("gh issue list returned no list for {label}")))?;

    let mut eligible = Vec::new();
    let mut excluded = Vec::new();
    for item in &items {
        if state != "review" {
            eligible.push(item.clone());
            continue;
        }
        let number = item
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                Failure::Read("a review queue candidate had no issue number".to_owned())
            })?;
        // The queue list does not carry comments. Each candidate timeline is a
        // required input to requester exclusion, so one unreadable candidate
        // makes the queue unreadable rather than silently eligible.
        let issue = issue_view(context, number, "comments")?;
        let comments = queue_comments(&issue, number)?;
        match super::claim::review_eligibility(&comments, run_id) {
            super::claim::ReviewEligibility::Eligible => eligible.push(item.clone()),
            super::claim::ReviewEligibility::Excluded {
                publisher,
                requesters,
                handoff,
            } => {
                let handoff = *handoff;
                excluded.push(serde_json::json!({
                    "number": number,
                    "reason": "review-handoff-requester-excluded",
                    "publisher": publisher,
                    "requesters": requesters,
                    "receipt": {
                        "epoch": handoff.receipt.epoch,
                        "pr": handoff.receipt.pr,
                        "head": handoff.receipt.head,
                        "base": handoff.receipt.base,
                        "digest": handoff.receipt.digest,
                    },
                    "blocker": handoff.blocker,
                    "discharger": handoff.discharger,
                    "requested_at": handoff.requested_at,
                    "deadline": handoff.deadline,
                }))
            }
        }
    }

    let mut partitions: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for item in &eligible {
        let names: Vec<String> = item
            .get("labels")
            .and_then(serde_json::Value::as_array)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label.get("name").and_then(serde_json::Value::as_str))
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let domain = names
            .iter()
            .find_map(|name| name.strip_prefix("domain:"))
            .unwrap_or("unassigned")
            .to_owned();
        let entry = serde_json::json!({
            "number": item.get("number"),
            "title": item.get("title"),
            "createdAt": item.get("createdAt"),
            "labels": names,
        });
        if let Some(bucket) = partitions
            .entry(domain)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
        {
            bucket.push(entry);
        }
    }
    for bucket in partitions.values_mut() {
        if let Some(list) = bucket.as_array_mut() {
            list.sort_by(|left, right| {
                let at = |value: &serde_json::Value| {
                    value
                        .get("createdAt")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned()
                };
                at(left).cmp(&at(right))
            });
        }
    }

    Ok(serde_json::json!({
        "ok": true,
        "state": state,
        "requester": run_id,
        "count": eligible.len(),
        "excluded_count": excluded.len(),
        "excluded": excluded,
        // Whether this answer *is* the queue, or the ceiling cut it off. The
        // fetched count and the limit are both right here. `count` may be lower
        // after requester exclusion, so this deliberately uses the pre-filter
        // list rather than making a full queue look complete.
        //
        // The tool's `limit` argument already says it in prose: *"an answer
        // holding exactly that many may be a longer queue read to its limit"*.
        // That is read once, at `tools/list`, and the answer is read every
        // time. A partial answer taken for the state is the failure this crate
        // is named for, and the sentence warning about it lived somewhere other
        // than the thing being read.
        //
        // `>=` and not `==`: a ceiling of nought asks for nothing and gets
        // nothing, and an empty answer to a question that could not return
        // anything is exactly the case that must not read as an empty queue.
        "at_limit": items.len() as u64 >= ceiling,
        "partitions": partitions,
        "note": "ordering inside a partition needs the domain's scale contract — apply it yourself",
    }))
}

pub(super) fn queue_comments(
    issue: &serde_json::Value,
    number: u64,
) -> Result<Vec<super::ownership::Comment>, Failure> {
    issue
        .get("comments")
        .and_then(serde_json::Value::as_array)
        .map(|comments| comments.iter().map(super::claim::comment_of).collect())
        .ok_or_else(|| {
            Failure::Read(format!(
                "gh issue view {number} returned no comment timeline for review eligibility"
            ))
        })
}

/// One issue's fields, or a read failure.
///
/// `gh` answering nothing is not an empty issue: it is a read that did not
/// happen, and the difference decides whether a caller may write.
fn issue_view(context: &Context, issue: u64, fields: &str) -> Result<serde_json::Value, Failure> {
    let number = issue.to_string();
    let data = super::gh_json(
        &["issue", "view", &number, "--json", fields],
        Some(&context.repo_dir),
    )?;
    data.ok_or_else(|| Failure::Read(format!("gh issue view {issue} returned nothing")))
}

/// Whether a label name is the one that carries the workflow state.
///
/// **One predicate.** The prefix was written in three places: this rule, its
/// twin over a list of bare names below, and a third copy inlined in
/// `claim::verify_claim` — which is the half of the pair that *reads* the state
/// while `transition` is the half that *writes* it. Two halves of one decision,
/// each with its own idea of which label carries it, is the shape this crate
/// keeps paying for.
pub(super) fn is_status_label(name: &str) -> bool {
    name.starts_with("status:")
}

fn board_home(context: &Context) -> Result<String, Failure> {
    let (owner, name) = super::closing::repo_identity(context)?;
    Ok(format!("{owner}/{name}"))
}

/// The `status:*` labels an issue carries, in the order `gh` reported them.
pub(super) fn status_labels(data: &serde_json::Value) -> Vec<String> {
    data.get("labels")
        .and_then(serde_json::Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.get("name").and_then(serde_json::Value::as_str))
                .filter(|name| is_status_label(name))
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Creates a label before anything tries to attach it.
///
/// Tolerated on purpose: `--force` is idempotent, so a label that already exists
/// reports non-zero and means success.
pub(super) fn ensure_label(context: &Context, name: &str, colour: &str) -> Result<(), Failure> {
    super::run(
        &["gh", "label", "create", name, "--color", colour, "--force"],
        Some(&context.repo_dir),
        super::How::tolerated(),
    )
    .map(|_| ())
}

/// `create` — file an issue carrying every marker the finding supplies.
///
/// Labels are created **before** they are attached, all of them, because `gh`
/// refuses to attach one that does not exist and the refusal arrives at
/// `gh issue create` — the analyst's last step, after all the analysis is done.
///
/// The URL is parsed defensively and the failure is a *write* failure. `gh`
/// printing a URL is not a contract, and an unguarded index here would crash out
/// of the JSON envelope entirely **after the issue was already filed** — leaving
/// a caller who retries to file a duplicate.
#[allow(clippy::too_many_arguments)]
pub fn create(
    context: &Context,
    identity: &str,
    title: &str,
    body_file: &std::path::Path,
    state: &str,
    priority: Option<&str>,
    domain: Option<&str>,
    runtime: Option<&str>,
    run_id: Option<&str>,
    use_cache: bool,
) -> Result<serde_json::Value, Failure> {
    for status in STATES {
        ensure_label(context, &format!("status:{status}"), "ededed")?;
    }
    let mut labels = vec![format!("status:{state}")];
    if let Some(priority) = priority.filter(|value| !value.is_empty()) {
        ensure_label(context, priority, "d93f0b")?;
        labels.push(priority.to_owned());
    }
    if let Some(domain) = domain.filter(|value| !value.is_empty()) {
        let label = format!("domain:{domain}");
        ensure_label(context, &label, "0e8a16")?;
        labels.push(label);
    }
    if let Some(runtime) = runtime.filter(|value| !value.is_empty()) {
        let label = format!("analyst:{runtime}");
        ensure_label(context, &label, "c5def5")?;
        labels.push(label);
    }

    let mut body = std::fs::read_to_string(body_file)
        .map_err(|error| Failure::Read(format!("the body file could not be read: {error}")))?;
    if let Some(run_id) = run_id.filter(|value| !value.is_empty()) {
        let marker =
            super::markers::render("analysis", &[("run-id", run_id)]).ok_or_else(|| {
                Failure::Stop(
                    serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }),
                )
            })?;
        body = format!(
            "{}

{marker}
",
            body.trim_end()
        );
    }

    let staged = crate::paths::scratch_file(&format!("create-{state}.md"));
    crate::paths::replace_atomically(&staged, &body)
        .map_err(|error| Failure::Write(format!("the body could not be staged: {error}")))?;
    let staged_path = staged.display().to_string();
    let full_title = format!("{identity}: {title}");
    let mut arguments = vec![
        "gh",
        "issue",
        "create",
        "--title",
        &full_title,
        "--body-file",
        &staged_path,
    ];
    for label in &labels {
        arguments.push("--label");
        arguments.push(label);
    }
    let answer = super::run(&arguments, Some(&context.repo_dir), super::How::write());
    let _ = std::fs::remove_file(&staged);
    let printed = answer?.stdout.trim().to_owned();

    let number = printed
        .rsplit("/issues/")
        .next()
        .and_then(|tail| {
            let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
            digits.parse::<u64>().ok().filter(|_| {
                tail.trim_end()
                    .chars()
                    .all(|c| c.is_ascii_digit() || c.is_whitespace())
            })
        })
        .ok_or_else(|| {
            Failure::Write(
                [
                    "the issue may have been created, but its URL could not be parsed from",
                    &format!(
                        "`gh issue create` output: {:?}",
                        printed.chars().take(200).collect::<String>()
                    ),
                    "— re-read before retrying, a retry would file a duplicate",
                ]
                .join(" "),
            )
        })?;
    let url = printed.lines().next_back().unwrap_or_default().to_owned();

    // The case everyone forgets: a fresh issue reaches the board with an empty
    // Status and no transition ever follows to correct it.
    let mut board = super::board::Board::parse(
        context.get("project board").unwrap_or_default(),
        context,
        use_cache,
    );
    let home = if board.enabled {
        board_home(context).unwrap_or_default()
    } else {
        String::new()
    };
    let mirror = board.set_status(number, state, &home);

    Ok(serde_json::json!({
        "ok": true,
        "issue": number,
        "url": url,
        "labels": labels,
        "board": mirror,
    }))
}

/// `transition` — mirror the board, swap the label, read **both** back.
///
/// The order is the design. The fragile, easily-skipped half (the mirror) runs
/// before anything can short-circuit it; the reliable one-call label edit
/// follows. And the read-back covers the board because that is the half with no
/// other feedback loop — a wrong label is caught by the very next `list-state`,
/// a column nobody looks at stays wrong forever.
///
/// The label edit is a **write**. `gh issue edit` with both `--add-label` and
/// `--remove-label` can apply partially, so reporting its failure as a read
/// would tell the caller "nothing happened, retry" about an issue that may
/// already be carrying two states.
///
/// That is true of the *read-back* as well, and it is the half this operation
/// got wrong for longer: everything below the edit is refused as a write that
/// landed, because it did. Two rules follow from it — the removal is never the
/// label being added, and a read-back that disagrees says so on the `world`
/// rather than leaving a caller to read *nothing was written*.
pub fn transition(
    context: &Context,
    issue: u64,
    to: &str,
    from: Option<&str>,
    use_cache: bool,
) -> Result<serde_json::Value, Failure> {
    let spec = context.get("project board").unwrap_or_default().to_owned();
    let mut board = super::board::Board::parse(&spec, context, use_cache);
    // Read once, before anything writes, and hand the same answer to the
    // writer, the read-back and the repair. Asked again lower down it was a `?`
    // *below* the label edit, so a `gh repo view` that failed there reported
    // "nothing was written" over an edit that had landed.
    let home = if board.enabled {
        board_home(context).unwrap_or_default()
    } else {
        String::new()
    };
    let mirror = board.set_status(issue, to, &home);

    let number = issue.to_string();
    let add = format!("status:{to}");
    let mut arguments: Vec<String> = vec![
        "gh".to_owned(),
        "issue".to_owned(),
        "edit".to_owned(),
        number,
        "--add-label".to_owned(),
        add.clone(),
    ];
    match from.filter(|value| !value.is_empty()) {
        Some(from) => {
            let leaving = format!("status:{from}");
            // The removal, unless it is the label being added.
            //
            // `--add-label status:done --remove-label status:done` is a
            // self-cancelling edit and `gh` settles it by dropping the label:
            // measured on issue #3, which came out of exactly that call wearing
            // no state at all, invisible to `list_state` in every partition and
            // disagreeing with every `verify_claim --expect-state`. The
            // read-back then found nothing and the refusal said *nothing was
            // written*.
            //
            // Naming the state being entered as the state being left is not
            // exotic — it is what a run sends to assert the state it believes an
            // issue is already in, the same reflex `verify_claim` exists for.
            // The add is kept, so the call still ends with the issue in `to`
            // whether or not it started there; only the window with no state is
            // gone.
            if leaving != add {
                arguments.push("--remove-label".to_owned());
                arguments.push(leaving);
            }
        }
        None => {
            // Without `--from`, remove whatever stale state labels are there.
            // An item wearing two states poisons every query touching either.
            for stale in status_labels(&issue_view(context, issue, "labels")?) {
                if stale != add {
                    arguments.push("--remove-label".to_owned());
                    arguments.push(stale);
                }
            }
        }
    }
    let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
    // With what the mirror did, when this fails. The board is moved **first** —
    // the module's header says why: the fragile half has to run before anything
    // can short-circuit it — so a label edit that fails leaves the caller in the
    // one state this file calls "the exact inverse of this module's promise":
    // the board ahead of the authoritative store.
    //
    // The fact is in hand at the point of decision and the `?` dropped it. What
    // the caller was told is `gh issue edit failed` and *"re-read to establish
    // what actually happened"* — and they re-read the labels, find the old one,
    // and conclude nothing happened. The board is not in that reading.
    //
    // The mirror's own words rather than a guess: it says whether it moved
    // anything, was skipped, or was never attempted, and those need different
    // answers.
    if let Err(failure) = super::run(&borrowed, Some(&context.repo_dir), super::How::write()) {
        let Failure::Write(detail) = failure else {
            return Err(failure);
        };
        return Err(Failure::Write(format!(
            "{detail} — the board mirror ran first and answered {mirror}, so read it as well as \
             the labels before deciding what happened"
        )));
    }

    let after = status_labels(&issue_view(context, issue, "labels")?);
    let mut result = serde_json::json!({
        "ok": true,
        "issue": issue,
        "to": to,
        "labels_after": after,
        "board": mirror,
    });

    if after != [add] {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "label-readback-failed",
            // The edit above ran and reported success, so whatever the labels
            // are now, they are what this call left. Without this key a `Stop`
            // renders as *nothing was written* with *do not repeat this call*
            // under it, over an issue whose labels this very call has just
            // changed — the shape issue #1 opened `MutationOutcome::Committed`
            // for, in `publish_review`, and the one `transition` was never
            // covered by. A caller who believes both sentences stops, and the
            // state stays broken.
            "world": "committed",
            "detail": format!("expected exactly [status:{to}], found {after:?}"),
            // Naming the repair rather than forbidding it. Omitting `from`
            // removes whatever stale state labels are found — documented on the
            // arm above, and what actually restored issue #3 — so the call this
            // refusal used to warn against is the call that clears it.
            "action": format!(
                "fix on the spot \u{2014} a two-state item poisons every query touching either \
                 state. Repeat this transition as `transition --to {to}` with `from` omitted, \
                 which removes whatever stale state labels are found"
            ),
        })));
    }

    if board.enabled {
        let column = board.read_status(issue, &home);
        let expected = board
            .meta()
            .and_then(|meta| super::board::Board::column_for(&meta, to));
        result["board_column_after"] = column
            .clone()
            .map_or(serde_json::Value::Null, serde_json::Value::String);
        match (&column, &expected) {
            (None, _) => {
                result["board_note"] = serde_json::json!(format!(
                    "issue #{issue} is not on the board (or not visible) — labels carry the truth"
                ));
            }
            (Some(column), Some(expected)) if *column != expected.name => {
                // Before believing the mirror failed, distrust the **cache**. The
                // mutation addresses the column by stable id, so renaming a
                // column leaves the board correct while the cached option *name*
                // goes stale — and comparing a live name against a stale one
                // reports a failure that never happened, blocking every
                // transition on this issue for as long as the cache lives.
                let mut fresh = super::board::Board::parse(&spec, context, false);
                let fresh_expected = fresh
                    .meta()
                    .and_then(|meta| super::board::Board::column_for(&meta, to));
                if fresh_expected
                    .as_ref()
                    .is_some_and(|option| *column == option.name)
                {
                    result["board_note"] = serde_json::json!(
                        [
                            "the cached column name was stale (the board column was renamed);",
                            "the mirror itself had landed correctly and the cache is now",
                            "refreshed",
                        ]
                        .join(" ")
                    );
                } else {
                    let retried = fresh.set_status(issue, to, &home);
                    let recheck = fresh.read_status(issue, &home);
                    result["board_repair"] = serde_json::json!({
                        "retried": retried,
                        "column_now": recheck,
                    });
                    if let Some(option) = &fresh_expected
                        && recheck.as_deref() != Some(option.name.as_str())
                    {
                        return Err(Failure::Stop(serde_json::json!({
                            "ok": false,
                            "reason": "board-readback-failed",
                            "detail": format!(
                                "board says {recheck:?}, labels say status:{to}"
                            ),
                            "action": "the mirror is not landing — investigate before continuing",
                        })));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(result)
}

/// The `status:*` labels in a list of label names.
fn status_names(labels: &serde_json::Value) -> Vec<String> {
    labels
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|name| is_status_label(name))
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// `audit-board` — compare every card's column against its own `status:*` label.
///
/// The mirror only fires on a transition somebody makes; it does not repair
/// drift a previous run left behind. This is one pass over data a single query
/// already returns, which is why the binding says a run that finds its own drift
/// should check for the others while the response is in hand.
///
/// **Zero cards is not a clean board.** An empty read and a clean board produce
/// the same empty drift list, and reporting that as a pass is the exact failure
/// this command exists to remove: a step that silently did nothing while looking
/// like it succeeded. It is not a failed read either — a brand-new project
/// genuinely has no cards, and telling the caller to retry forever is its own
/// wrong answer. The honest verdict is **inconclusive**: the audit ran and could
/// not conclude anything.
pub fn audit_board(
    context: &Context,
    fix: bool,
    use_cache: bool,
) -> Result<serde_json::Value, Failure> {
    let spec = context.get("project board").unwrap_or_default().to_owned();
    let mut board = super::board::Board::parse(&spec, context, use_cache);
    if !board.enabled {
        return Ok(serde_json::json!({
            "ok": true, "audited": false, "skipped": "no board configured",
        }));
    }
    let Some(meta) = board.meta() else {
        return Ok(serde_json::json!({
            "ok": true, "audited": false, "skipped": board.skip_reason,
        }));
    };

    let cards = board.all_cards();
    if cards.is_empty() {
        let action = [
            "this is NOT a clean board. Either the project is genuinely empty (fine,",
            "nothing to audit) or the query was misdirected — wrong owner type,",
            "missing `project` scope, wrong board number. Confirm which before",
            "treating the board as verified",
        ]
        .join(" ");
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "board-empty-inconclusive",
            // **Which board.** It said *"the board returned zero cards"* while
            // the action beside it asked the operator to confirm the owner, the
            // scope and the number — the three things this sentence could have
            // named and did not. An operator with more than one board reads a
            // refusal that fits all of them.
            //
            // The identity is in hand two lines up: `board.meta()` resolved it,
            // which is what made the query possible at all.
            "detail": format!(
                "{}/{} returned zero cards",
                board.owner.as_deref().unwrap_or("?"),
                board
                    .number
                    .map(|number| number.to_string())
                    .unwrap_or_else(|| "?".to_owned()),
            ),
            "board": format!(
                "{}/{}",
                board.owner.as_deref().unwrap_or_default(),
                board.number.unwrap_or_default()
            ),
            "action": action,
        })));
    }

    let mut drift = Vec::new();
    let mut missing_column = Vec::new();
    let mut unread_labels = Vec::new();
    let mut foreign = Vec::new();
    let home = board_home(context)?;
    for card in &cards {
        if !super::board::card_is_ours(card, &home) {
            let mut entry = card.clone();
            entry["problem"] = serde_json::json!("card belongs to another repository");
            foreign.push(entry);
            continue;
        }
        // An unfinished read is not a verdict. `labels(first: N)` is a window
        // and the state is a label, so a card carrying more labels than arrived
        // can have its `status:` outside it — which read as "no status label"
        // and was reported as drift on a correctly labelled issue, one `--fix`
        // could never clear because the repair only acts on exactly one label.
        // The other direction hides a second status label and calls real drift
        // clean.
        if card.get("labels_complete") == Some(&serde_json::Value::Bool(false)) {
            let mut entry = card.clone();
            entry["problem"] = serde_json::json!("more labels than this read returned");
            unread_labels.push(entry);
            continue;
        }
        let labels = status_names(card.get("labels").unwrap_or(&serde_json::Value::Null));
        if labels.len() != 1 {
            let mut entry = card.clone();
            entry["problem"] =
                serde_json::json!(format!("expected one status label, found {labels:?}"));
            drift.push(entry);
            continue;
        }
        let state = labels[0].split_once(':').map_or("", |(_, rest)| rest);
        let expected = super::board::Board::column_for(&meta, state);
        let column = card.get("column").and_then(serde_json::Value::as_str);
        match (column, &expected) {
            (None, _) => {
                let mut entry = card.clone();
                entry["expected"] = serde_json::json!(
                    expected
                        .as_ref()
                        .map_or(state, |option| option.name.as_str())
                );
                missing_column.push(entry);
            }
            (Some(column), Some(option)) if column != option.name => {
                let mut entry = card.clone();
                entry["expected"] = serde_json::json!(option.name);
                entry["problem"] = serde_json::json!("column disagrees with label");
                drift.push(entry);
            }
            _ => {}
        }
    }

    let mut repaired = Vec::new();
    if fix {
        for card in drift.iter().chain(&missing_column) {
            let labels = status_names(card.get("labels").unwrap_or(&serde_json::Value::Null));
            if labels.len() == 1
                && let Some(issue) = card.get("issue").and_then(serde_json::Value::as_u64)
            {
                let state = labels[0].split_once(':').map_or("", |(_, rest)| rest);
                let mut entry = board.set_status(issue, state, &home);
                entry["issue"] = serde_json::json!(issue);
                repaired.push(entry);
            }
        }
    }

    Ok(serde_json::json!({
        "ok": true,
        "audited": true,
        "cards": cards.len(),
        "drift": drift,
        "missing_column": missing_column,
        "unread_labels": unread_labels,
        "foreign": foreign,
        "repaired": repaired,
    }))
}

/// `changelog-notes` — the entry a tag should carry, read from the changelog.
///
/// **Fails closed.** A version bump with no entry is not a tagging problem to
/// work around: the entry is part of what "delivered" means, and inventing notes
/// at tag time is how a changelog becomes a thing nobody trusts.
///
/// A relative `--file` resolves against the repository, **never** against the
/// process working directory. The old order asked whether the path existed
/// first, which answers against the ambient directory — and in a monorepo every
/// worktree shares the same relative layout, so `docs/x/CHANGELOG.md` exists in
/// both and the ambient one won. The two differ exactly when it matters: when
/// the branch added or corrected the entry the tag is about. Reading the wrong
/// branch's changelog into an immutable tag is silent and unfixable.
pub fn changelog_notes(
    context: &Context,
    file: &std::path::Path,
    version: &str,
    include_heading: bool,
    out: Option<&std::path::Path>,
) -> Result<serde_json::Value, Failure> {
    let resolved = if file.is_absolute() {
        file.to_path_buf()
    } else {
        context.repo_dir.join(file)
    };
    let shown = resolved.display().to_string();
    let Ok(text) = std::fs::read_to_string(&resolved) else {
        let action = [
            "a relative --file is resolved against --repo-dir; pass an absolute",
            "path, or point --repo-dir at the checkout that holds the changelog",
        ]
        .join(" ");
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "changelog-not-found", "file": shown, "action": action,
        })));
    };

    let found = super::changelog::section(&text, version).map_err(|trouble| match trouble {
        super::changelog::Trouble::Missing => {
            let action = [
                "this version has no entry. Write it BEFORE tagging — a tag is immutable",
                "and the entry is part of the delivery, not a formality after it",
            ]
            .join(" ");
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "no-changelog-entry",
                "version": version, "file": shown.clone(), "action": action,
            }))
        }
        super::changelog::Trouble::Ambiguous(headings) => {
            let action = [
                "more than one heading claims this version — resolve the changelog",
                "before tagging; picking one silently would make the wrong choice permanent",
            ]
            .join(" ");
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "ambiguous-changelog-entry",
                "version": version, "headings": headings, "action": action,
            }))
        }
    })?;

    if found.body.is_empty() {
        let action = [
            "the heading exists but says nothing under it — a tag message of one",
            "title line is not release notes",
        ]
        .join(" ");
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "empty-changelog-entry",
            "version": version, "heading": found.heading, "action": action,
        })));
    }

    let notes = if include_heading {
        format!(
            "{}

{}",
            found.heading, found.body
        )
    } else {
        found.body.clone()
    };
    let written = match out {
        Some(path) => {
            crate::paths::replace_atomically(
                path,
                &format!(
                    "{notes}
"
                ),
            )
            .map_err(|error| Failure::Write(format!("the notes could not be written: {error}")))?;
            Some(
                std::fs::canonicalize(path)
                    .map(crate::paths::remove_windows_verbatim_prefix)
                    .unwrap_or_else(|_| path.to_path_buf())
                    .display()
                    .to_string(),
            )
        }
        None => None,
    };

    let next = [
        "git tag -a <tag> -F <notes-file> <merge-sha>, and `gh release create",
        "--notes-file <notes-file>` where the component publishes Releases — never",
        "--generate-notes, which replaces what a human wrote with a list of commit subjects",
    ]
    .join(" ");
    Ok(serde_json::json!({
        "ok": true,
        "version": version,
        "file": shown,
        "heading": found.heading,
        "lines": found.body.lines().count(),
        "notes_file": written,
        "notes": if written.is_some() { serde_json::Value::Null } else { serde_json::json!(notes) },
        "next": next,
    }))
}

/// Will this issue auto-close on merge, and from what?
///
/// This predicts the **pending** merge. Merged historical closers prove past
/// deliveries, not a current risk; unrelated open PRs are equally irrelevant
/// once the caller names a branch.
/// Posts one comment whose body this crate composed.
///
/// Staged to a file and handed to `gh --body-file`, like every other body this
/// transport writes: a body on the command line is a body a shell may rewrite,
/// which is the corruption `bindings/github.md` documents. Atomically, because
/// `gh` is a reader of that file and half a comment posted is worse than none.
pub(super) fn comment_with_body(context: &Context, issue: u64, body: &str) -> Result<(), Failure> {
    let path = crate::paths::scratch_file(&format!("{issue}-published.md"));
    crate::paths::replace_atomically(&path, body).map_err(|error| {
        Failure::Write(format!("the comment body could not be staged: {error}"))
    })?;
    super::run(
        &[
            "gh",
            "issue",
            "comment",
            &issue.to_string(),
            "--body-file",
            &path.display().to_string(),
        ],
        Some(&context.repo_dir),
        super::How::write(),
    )?;
    Ok(())
}

pub(super) fn assess_autoclose(
    context: &Context,
    issue: u64,
    base: Option<&str>,
    branch: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    let text_of = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let refs: Vec<serde_json::Value> = super::closing::closing_refs(context, issue)?
        .into_iter()
        .filter(|reference| text_of(reference, "state") == "OPEN")
        .filter(|reference| branch.is_none_or(|branch| text_of(reference, "headRefName") == branch))
        .filter(|reference| base.is_none_or(|base| text_of(reference, "baseRefName") == base))
        .collect();
    if refs.is_empty() {
        return Ok(serde_json::json!({
            "will_autoclose": false, "cause": serde_json::Value::Null, "linked_prs": [],
        }));
    }

    // A non-empty connection proves the issue WILL auto-close, but not WHY, and
    // the remedy differs completely by cause: text an author wrote can be
    // edited, a link GitHub derived from the branch cannot.
    let mut hits = Vec::new();
    for reference in &refs {
        let number = reference
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default()
            .to_string();
        // A body nobody could read is not a body with no closing keyword. It
        // does not change *whether* the issue auto-closes — `closing_refs`
        // settles that above — but it changes **why**, and the comment three
        // lines up says the remedy differs completely by cause: text an author
        // wrote can be edited, a link GitHub derived from the branch cannot. A
        // silent read pointed at the one that cannot be undone.
        let body = super::gh_json(
            &["pr", "view", &number, "--json", "body"],
            Some(&context.repo_dir),
        )?
        .ok_or_else(|| {
            Failure::Read(format!(
                "gh pr view {number} returned nothing, so why this issue auto-closes is unknown"
            ))
        })?;
        for text in super::closing::keywords_naming(&text_of(&body, "body"), issue) {
            hits.push(serde_json::json!({ "where": format!("pr#{number} body"), "text": text }));
        }
    }
    if let (Some(base), Some(branch)) = (base, branch) {
        for text in super::closing::keywords_in_commits(&context.repo_dir, base, branch, issue)? {
            hits.push(serde_json::json!({ "where": "commit message", "text": text }));
        }
    }

    Ok(serde_json::json!({
        "will_autoclose": true,
        "cause": if hits.is_empty() { "branch-link" } else { "closing-keyword" },
        "keyword_sources": hits,
        "linked_prs": refs,
    }))
}

/// `check-closing-keywords` — run again before merging.
///
/// Auto-close bypasses `transition`: no state move, no mirror, labels frozen
/// wherever they were. Reading one's own prose is how a keyword slips through,
/// so this checks it mechanically — and the branch's commit messages can
/// introduce one after the body is already clean, which is why it is run twice.
///
/// A **keyword** is a hard stop, because the text is the author's to fix. A
/// **branch link** is not, because it is what the recommended linking command
/// does — and a gate that always fires is a gate that gets ignored. It is
/// reported with the follow-up it mandates instead.
pub fn check_closing_keywords(
    context: &Context,
    issue: u64,
    base: Option<&str>,
    branch: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    let verdict = assess_autoclose(context, issue, base, branch)?;
    let cause = verdict.get("cause").and_then(serde_json::Value::as_str);

    if cause == Some("closing-keyword") {
        let action = [
            "remove the keyword — use `Refs #<n>` — then re-run; the value can lag",
            "a few seconds",
        ]
        .join(" ");
        let mut payload = serde_json::json!({ "ok": false, "reason": "closing-keyword-live" });
        merge_into(&mut payload, &verdict);
        payload["action"] = serde_json::json!(action);
        return Err(Failure::Stop(payload));
    }

    let mut result = serde_json::json!({ "ok": true, "issue": issue });
    merge_into(&mut result, &verdict);
    if cause == Some("branch-link") {
        let warning = [
            "this issue WILL auto-close on merge because `gh issue develop` linked the branch,",
            "not because of any keyword — no edit removes it",
        ]
        .join(" ");
        let follow_up = [
            "after merge, run `transition --to done` anyway: GitHub's auto-close is not the",
            "workflow's close, so without it the label and the board freeze where they are",
        ]
        .join(" ");
        result["warning"] = serde_json::json!(warning);
        result["mandatory_follow_up"] = serde_json::json!(follow_up);
    }
    Ok(result)
}

/// Copies one object's keys onto another, the way the Python's `**verdict` does.
fn merge_into(target: &mut serde_json::Value, source: &serde_json::Value) {
    let (Some(target), Some(source)) = (target.as_object_mut(), source.as_object()) else {
        return;
    };
    for (key, value) in source {
        target.insert(key.clone(), value.clone());
    }
}

/// Did the base move in a way this delivery must integrate before review?
///
/// Pure, and fed answers rather than fetching them, because the states it
/// separates are races no test can reproduce by calling git. The distinction the
/// operator policy turns on:
///
/// - the base did not move — nothing to decide;
/// - it moved, but every path it touched is disjoint from the delivery target
///   **and** a read-only merge reports no conflict — *compatible*, so the branch
///   is left alone. Merging the base routinely is what the policy forbids: it
///   rewrites the review target for no reason and invalidates evidence already
///   bound to the old head;
/// - it moved and touched paths this delivery also touches, or the merge
///   conflicts — *material*, integrated before final review.
///
/// `conflicted = None` means the merge could not be established, and that is
/// **never** "compatible". Textual disjointness is not semantic compatibility
/// either: this reports evidence and says so, and the judgement stays with the
/// caller.
pub fn classify_base_movement(
    recorded: &str,
    current: &str,
    moved: &std::collections::BTreeSet<String>,
    target: &std::collections::BTreeSet<String>,
    conflicted: Option<bool>,
) -> serde_json::Value {
    if recorded == current {
        return serde_json::json!({ "movement": "none", "integrate": false });
    }
    let overlap: Vec<&String> = moved.intersection(target).collect();
    match conflicted {
        None => serde_json::json!({
            "movement": "unknown", "integrate": true, "overlap": overlap,
            "detail": "the merge result could not be established",
        }),
        Some(true) => serde_json::json!({
            "movement": "conflicting", "integrate": true, "overlap": overlap,
        }),
        Some(false) if !overlap.is_empty() => serde_json::json!({
            "movement": "overlapping", "integrate": true, "overlap": overlap,
        }),
        Some(false) => {
            let detail = [
                "disjoint paths and a conflict-free merge; semantic impact is the caller's",
                "judgement, and this is evidence rather than a verdict",
            ]
            .join(" ");
            serde_json::json!({
                "movement": "compatible", "integrate": false, "overlap": [], "detail": detail,
            })
        }
    }
}

/// `base-movement` — evidence about later base movement. **Never a verdict.**
///
/// `git merge-tree --write-tree` performs the merge without touching the index,
/// the worktree or any branch, and its exit status reports conflicts. It is not
/// side-effect free, and saying so plainly matters because an adversarial
/// reviewer is repository read-only: it writes the merged tree into the object
/// store, and this command fetches first, which updates remote-tracking refs.
/// Neither moves a branch, the index or a file — "read-only" here means *does
/// not modify the delivery*, not *writes nothing at all*.
///
/// `--write-tree` needs git 2.38. Below that the merge cannot be established and
/// the answer is `unknown`, which integrates rather than assuming compatibility.
pub fn base_movement(
    context: &Context,
    base: &str,
    recorded: &str,
    worktree: Option<&std::path::Path>,
) -> Result<serde_json::Value, Failure> {
    let where_ = worktree.unwrap_or(&context.repo_dir);
    super::run(
        &["git", "fetch", "origin"],
        Some(where_),
        super::How::read(),
    )?;

    let reference = format!("origin/{base}^{{commit}}");
    let current = super::run(
        &["git", "rev-parse", "--verify", &reference],
        Some(where_),
        super::How::read(),
    )?
    .stdout
    .trim()
    .to_owned();
    let head = super::run(
        &["git", "rev-parse", "--verify", "HEAD^{commit}"],
        Some(where_),
        super::How::read(),
    )?
    .stdout
    .trim()
    .to_owned();

    let changed = |from: &str, to: &str| -> Result<std::collections::BTreeSet<String>, Failure> {
        let range = format!("{from}..{to}");
        let out = super::run(
            &["git", "diff", "--name-only", "-z", &range],
            Some(where_),
            super::How::read(),
        )?;
        // A path is bytes, and `git` hands them back the way the filesystem
        // holds them. Decoding with replacement is right for a message and
        // wrong here: the answer this command returns *is* a list of paths, and
        // a replaced byte makes one of them a path that is not the path —
        // reported as evidence a reviewer weighs, with nothing saying it was
        // rewritten on the way out.
        //
        // Refused rather than guessed, the same way `manifest_trouble` refuses
        // an unreadable tree entry instead of naming it something. The original
        // does not replace here at all: it reads this call binary and decodes
        // with `surrogateescape`, which keeps the bytes and then cannot be
        // serialised — `json.dumps` emits a lone surrogate and the harness that
        // reads it back gets *invalid JSON*. Two implementations, two different
        // wrong answers, and the crossing had never posed the case.
        if out.stdout_replaced {
            return Err(Failure::Read(format!(
                "{range} lists a path that is not UTF-8, so it cannot be reported \
                 without rewriting it; base movement is not classified here"
            )));
        }
        Ok(out
            .stdout
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    };

    let moved = if recorded != current {
        changed(recorded, &current)?
    } else {
        Default::default()
    };
    let target = changed(recorded, &head)?;

    let mut conflicted = None;
    if recorded != current {
        let merge = super::run(
            &[
                "git",
                "merge-tree",
                "--write-tree",
                "--no-messages",
                &head,
                &current,
            ],
            Some(where_),
            super::How::tolerated(),
        )?;
        // Only `0` and `1` are answers. Any other status leaves this unknown —
        // the merge did not answer, which is not "compatible".
        conflicted = match merge.status {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
    }

    let verdict = classify_base_movement(recorded, &current, &moved, &target, conflicted);
    let semantic = [
        "not decided here — disjoint paths and a clean merge are evidence, not",
        "proof that the change still means what it did",
    ]
    .join(" ");
    let mut result = serde_json::json!({
        "ok": true,
        "recorded_base": recorded,
        "current_base": current,
        "head": head,
        "moved_paths": moved,
        "target_paths": target,
        "conflict": conflicted,
    });
    merge_into(&mut result, &verdict);
    result["semantic_impact"] = serde_json::json!(semantic);
    Ok(result)
}

/// Reads `git status --porcelain=v1 -z` into `(code, path)` pairs.
///
/// A rename reports the destination first and the **source** in the next field.
/// The source is a deletion from the delivered tree and must not be left
/// standing — a target that keeps both ships a path twice under two names.
pub fn read_status(fields: &[&str]) -> Result<Vec<(String, String)>, Failure> {
    let mut status = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let entry = fields[index];
        if entry.len() < 4 {
            return Err(Failure::Read(format!(
                "unreadable git status entry {entry:?}"
            )));
        }
        let (code, rest) = entry.split_at(2);
        let path = rest[1..].to_owned();
        if code.starts_with(['R', 'C']) {
            index += 1;
            let Some(source) = fields.get(index) else {
                return Err(Failure::Read(
                    "git status reported a rename with no source path".to_owned(),
                ));
            };
            status.push(("D ".to_owned(), (*source).to_owned()));
        }
        status.push((code.to_owned(), path));
        index += 1;
    }
    Ok(status)
}

/// Turns a manifest complaint into the transport's own refusal.
pub fn manifest_trouble(trouble: super::manifest::Trouble) -> Failure {
    Failure::Read(match trouble {
        super::manifest::Trouble::Unreadable(what) => format!("unreadable tree entry {what:?}"),
        super::manifest::Trouble::Duplicate(path) => format!("the tree listed {path:?} twice"),
        super::manifest::Trouble::Empty => {
            "the tree listed no entries; an empty answer is not a delivery target".to_owned()
        }
    })
}
