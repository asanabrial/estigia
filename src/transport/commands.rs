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
/// A hand-written list, and hand-written for a reason: it is a *claim about the
/// contract*, not a reflection of the dispatch table. The other direction is
/// crossed by
/// `dispatch::tests::every_operation_a_tool_offers_is_claimed_or_declared_unscripted`.
pub const SCRIPTED: &[&str] = &[
    "ensure-states",
    "flush",
    "create",
    "list-state",
    "list-identities",
    "claim",
    "reclaim",
    "verify-claim",
    "transition",
    "comment",
    "blocked-until",
    "task",
    "tasks",
    "heartbeat",
    "start-branch",
    "judge-workspaces",
    "assist",
    "draft-pr",
    "ready-pr",
    "review-stages",
    "review-thread",
    "publish-review",
    "republish-review",
    "handoff",
    "handoff-review",
    "review-finding",
    "review-verdict",
    "review-refutation",
    "review-disposition",
    "review-lens-report",
    "launch-review-input",
    "collect-review-input",
    "review-validator-result",
    "review-computed-verdict",
    "computed-verdict",
    "escalate-review",
    "spend",
    "spend-summary",
    "release-ci",
    "run-checks",
    "unassign",
    "changelog-notes",
    "check-closing-keywords",
    // Read-only, and performed here all the same: both have rows in the GitHub
    // binding's operations table.
    "expected-target",
    "base-movement",
    "publication-lane",
    "repair-rounds",
    "review-status",
    "review-findings",
    "review-transaction",
    "review-payload-membership",
    "inspect-candidate",
    "review-tier",
    "review-tier-for-repair",
    "review-read",
    "target",
];

/// What the transport deliberately does **not** do, and why.
///
/// The irreversible remote writes and the two judgements. This is the same line
/// the harness draws with `DELIVERS`: landing work needs a verdict, and a
/// verdict is not something a script produces.
pub(super) const NOT_SCRIPTED: &[(&str, &str)] = &[
    (
        "publish_version",
        "irreversible remote write — annotated tags are never moved",
    ),
    (
        "close",
        "kept with publish_version so a manual delivery stays one prose-owned sequence; `deliver` runs it in the same call as the merge",
    ),
    (
        "review_status/ci_status",
        "verdict and CI interpretation stay with the agent",
    ),
];

/// `config` — the resolved operator configuration.
pub fn config(context: &Context) -> Result<serde_json::Value, Failure> {
    let mut table = serde_json::Map::new();
    for (key, value, _) in &context.config {
        table.insert(key.to_lowercase(), serde_json::Value::String(value.clone()));
    }

    Ok(serde_json::json!({
        "ok": true,
        "skill_dir": context.skill_dir.display().to_string(),
        "config": table,
        "scripted_operations": SCRIPTED,
        "not_scripted": NOT_SCRIPTED
            .iter()
            .map(|(name, why)| ((*name).to_owned(), serde_json::Value::String((*why).to_owned())))
            .collect::<serde_json::Map<_, _>>(),
    }))
}

/// The workflow's six states, in the order the transport declares them.
///
/// **One list, re-exported rather than written again** — and not merely
/// referenced, because the transport's callers ask for it by this path and the
/// schemas the tool server publishes come from `config::STATES`: one name for
/// one list, whichever door a reader arrives at.
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
///
/// While the git operation window is closed it writes nothing and answers
/// `deferred`, because the `Reconverge` a queued `create`, `claim` or `transition`
/// already schedules creates the labels at the flush.
pub fn ensure_states(context: &Context) -> Result<serde_json::Value, Failure> {
    let ensured: Vec<String> = STATES
        .iter()
        .map(|state| format!("status:{state}"))
        .collect();
    if matches!(
        super::window::state(context),
        super::window::Mode::Closed(_)
    ) {
        return Ok(serde_json::json!({
            "ok": true,
            "deferred": true,
            "ensured": ensured,
            "window": "closed",
        }));
    }
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

/// `comment` — post a person's prose from a file, with no marker.
///
/// Two refusals before anything is written, and both are about the vocabulary
/// rather than the prose:
///
/// - a kind outside `note`/`blocker`/`diagnosis` is **reserved**, so an ordinary
///   comment cannot name a control kind.
/// - a run-id without a kind, or a kind without a run-id, is **incomplete**.
///
/// The body reaches GitHub as the prose alone, escaped so quoted evidence stays
/// inert for an older timeline that still carries markers.
pub fn comment(
    context: &Context,
    issue: u64,
    body_file: &std::path::Path,
    run_id: Option<&str>,
    kind: Option<&str>,
    now: &str,
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
    let body = format!(
        "{}\n",
        super::markers::escape_control_input(&raw).trim_end()
    );

    if super::outbox::is_closed(context) {
        let queued = super::outbox::queue(
            context,
            issue,
            super::store::OutboxKind::Comment,
            &serde_json::json!({ "kind": kind, "body": body }),
            now,
        )?;
        return Ok(
            serde_json::json!({ "ok": true, "issue": issue, "kind": kind, "queued": queued }),
        );
    }

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
    let _ = std::fs::remove_file(&path);
    answer?;

    Ok(serde_json::json!({ "ok": true, "issue": issue, "kind": kind }))
}

/// One queue item as every partitioned answer carries it.
fn queue_entry(item: &serde_json::Value) -> serde_json::Value {
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
    let mut entry = serde_json::json!({
        "number": item.get("number"),
        "title": item.get("title"),
        "createdAt": item.get("createdAt"),
        "labels": names,
    });
    if item.get("provisional") == Some(&serde_json::Value::Bool(true)) {
        entry["provisional"] = serde_json::Value::Bool(true);
    }
    entry
}

/// The issues created while the window was closed and not yet filed, shaped
/// like the open-list items `list-state` partitions and marked `provisional`.
///
/// GitHub's list cannot hold them, and the store already gives each its state,
/// so without this a run could not see the work it filed itself. Their title
/// and labels come from the queued create, the only place they are kept.
fn provisional_items(
    store: Option<&super::store::Store>,
    repo: &super::store::Repo,
) -> Result<Vec<serde_json::Value>, Failure> {
    let Some(store) = store.filter(|store| store.unsent_outbox().is_ok_and(|waiting| waiting > 0))
    else {
        return Ok(Vec::new());
    };
    Ok(store
        .outbox(repo)?
        .into_iter()
        .filter(|row| {
            row.kind == super::store::OutboxKind::Create
                && row.state != super::store::OutboxState::Sent
                && super::outbox::is_provisional(row.issue)
        })
        .map(|row| {
            let labels: Vec<serde_json::Value> = row
                .payload
                .get("labels")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(|name| serde_json::json!({ "name": name }))
                .collect();
            serde_json::json!({
                "number": row.issue,
                "title": row.payload.get("title").cloned().unwrap_or(serde_json::Value::Null),
                "labels": labels,
                "createdAt": row.enqueued_at,
                "provisional": true,
            })
        })
        .collect())
}

/// The partition a queue entry belongs in: its `domain:` label, or
/// `unassigned` when it carries none.
fn entry_domain(entry: &serde_json::Value) -> String {
    entry
        .get("labels")
        .and_then(serde_json::Value::as_array)
        .and_then(|labels| {
            labels.iter().find_map(|label| {
                label
                    .as_str()
                    .and_then(|name| name.strip_prefix("domain:"))
                    .map(ToOwned::to_owned)
            })
        })
        .unwrap_or_else(|| "unassigned".to_owned())
}

/// Entries partitioned by domain, oldest first in each bucket. Age is the only
/// ordering the transport applies; ranking inside a bucket is the caller's job.
fn partition_entries(
    entries: Vec<serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut partitions: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for entry in &entries {
        let domain = entry_domain(entry);
        if let Some(bucket) = partitions
            .entry(domain)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut()
        {
            bucket.push(entry.clone());
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
    partitions
}

fn declared_scale(domain: &str) -> Option<&'static str> {
    if domain == "general" {
        Some("priority")
    } else {
        None
    }
}

fn carries_scale_value(labels: &[String], scale: &str) -> bool {
    let prefix = format!("{scale}:");
    labels
        .iter()
        .any(|name| name.starts_with(&prefix) && name.len() > prefix.len())
}

fn priority_scale_checked(priority: &str, domain: Option<&str>) -> bool {
    let Some((scale, value)) = priority.split_once(':') else {
        return false;
    };
    !value.is_empty()
        && domain
            .and_then(declared_scale)
            .is_some_and(|declared| declared == scale)
}

fn unchecked_priorities(
    partitions: &serde_json::Map<String, serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut unchecked = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (domain, bucket) in partitions {
        let Some(scale) = declared_scale(domain) else {
            continue;
        };
        let Some(items) = bucket.as_array() else {
            continue;
        };
        for item in items {
            let labels: Vec<String> = item
                .get("labels")
                .and_then(serde_json::Value::as_array)
                .map(|labels| {
                    labels
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            if carries_scale_value(&labels, scale) {
                continue;
            }
            let number = item
                .get("number")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if !seen.insert(format!("{domain}:{}", number)) {
                continue;
            }
            unchecked.push(serde_json::json!({
                "number": number,
                "partition": domain,
                "labels": labels,
            }));
        }
    }
    unchecked
}

/// Every open issue object the repository carries, oldest first, with pull
/// requests dropped.
///
/// Read from the REST issues endpoint, not `gh issue list`: gh routes every
/// `--label` filter through GitHub search, which lags a write by seconds. No
/// label filters this read, because the local store decides a state and a label
/// never does. The request asks the endpoint for
/// `sort=created&direction=asc`, because the endpoint answers newest-first by
/// default and the workflow selects on age: a cut must keep the **oldest** open
/// items, not the newest. A payload that is not pages of issues is a failed
/// read.
fn open_issue_objects(context: &Context) -> Result<Vec<serde_json::Value>, Failure> {
    let path = "repos/{owner}/{repo}/issues?state=open&per_page=100&sort=created&direction=asc";
    let data = super::gh_json(
        &["api", "--paginate", "--slurp", path],
        Some(&context.repo_dir),
    )?;
    let unreadable =
        || Failure::Read("the issues endpoint returned no pages for the open issues".to_owned());
    let pages = data
        .as_ref()
        .and_then(serde_json::Value::as_array)
        .ok_or_else(unreadable)?;
    let mut issues = Vec::new();
    for page in pages {
        for issue in page.as_array().ok_or_else(unreadable)? {
            if issue.get("pull_request").is_some() {
                continue;
            }
            issues.push(issue.clone());
        }
    }
    Ok(issues)
}

/// Every open issue the repository carries, oldest first, shaped like the
/// `gh issue list --json` items `list-state` partitions.
///
/// Beside the items comes how many open issues there were before the cut.
fn open_items(context: &Context, limit: usize) -> Result<(Vec<serde_json::Value>, u64), Failure> {
    let open = open_issue_objects(context)?;
    let mut items = Vec::new();
    for issue in &open {
        let field = |key: &str| issue.get(key).cloned().unwrap_or(serde_json::Value::Null);
        let labels: Vec<serde_json::Value> = issue
            .get("labels")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .map(|entry| match entry {
                serde_json::Value::String(name) => serde_json::json!({ "name": name }),
                other => serde_json::json!({ "name": other.get("name").cloned() }),
            })
            .collect();
        items.push(serde_json::json!({
            "number": field("number"),
            "title": field("title"),
            "labels": labels,
            "createdAt": field("created_at"),
        }));
    }
    let open = open.len() as u64;
    items.truncate(limit);
    Ok((items, open))
}

/// Every open issue number the repository carries, oldest first, with pull
/// requests dropped and no cut: what `Store::import` loops over.
pub(crate) fn open_issue_numbers(context: &Context) -> Result<Vec<u64>, Failure> {
    let mut numbers = Vec::new();
    for issue in open_issue_objects(context)? {
        let number = issue
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Failure::Read("an open issue had no issue number".to_owned()))?;
        numbers.push(number);
    }
    Ok(numbers)
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
///
/// The state and the holder come from the local store, never from a `status:`
/// label or a GitHub assignee. The open-issue list is GitHub's and the human-owned
/// domain and priority labels go back raw; the state each issue is in is the
/// store's, and an open issue the store has never admitted is reported under
/// `unadmitted` rather than placed in a state a label claims.
///
/// While the git operation window held writes back, an issue created in that
/// time is not on GitHub's list; it is appended, marked `provisional`, so a run
/// sees what it filed itself. Nothing else about the answer changes.
///
/// A truncated answer (`at_limit`) holds the **oldest** matching items, so the
/// oldest-first rule above survives the cut and the absolute oldest item is in
/// the window. What a cut does not preserve is ranking: the highest-priority
/// item may sit outside the window, and `at_limit` is the only surface that
/// says so.
pub fn list_state(
    context: &Context,
    state: &str,
    run_id: &str,
    limit: u32,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let ceiling = u64::from(limit);
    let (items, _open) = open_items(context, limit as usize)?;
    let repo = super::claim::repo_of(context)?;
    let store = super::store::open_for_read(&super::store::store_path()?)?;
    let rows = match &store {
        Some(store) => store.issue_rows(&repo)?,
        None => Vec::new(),
    };
    let states: std::collections::BTreeMap<u64, Option<String>> = rows
        .iter()
        .map(|row| (row.number, row.state.clone()))
        .collect();
    let provisional = provisional_items(store.as_ref(), &repo)?;
    let candidates: Vec<serde_json::Value> = items.iter().cloned().chain(provisional).collect();

    let mut eligible = Vec::new();
    let mut excluded = Vec::new();
    let mut unadmitted = Vec::new();
    let mut open = 0u64;
    for item in &candidates {
        let number = item
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Failure::Read("an open issue had no issue number".to_owned()))?;
        let Some(stored) = states.get(&number) else {
            unadmitted.push(serde_json::json!({
                "number": number,
                "title": item.get("title").cloned().unwrap_or(serde_json::Value::Null),
            }));
            continue;
        };
        if stored.as_deref() != Some(state) {
            continue;
        }
        open += 1;
        let comments = match &store {
            Some(store) => store.timeline(&repo, number)?,
            None => Vec::new(),
        };
        if super::claim::holding(&comments, now).holder.is_some() {
            continue;
        }
        if state != "review" {
            eligible.push(item.clone());
            continue;
        }
        // The queue list does not carry comments. Each candidate timeline is a
        // required input to requester exclusion, so one unreadable candidate
        // makes the queue unreadable rather than silently eligible.
        match super::claim::review_eligibility(&comments, run_id)? {
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

    let mut entries = Vec::new();
    let mut dischargeable = 0u64;
    let mut pending = 0u64;
    let mut unconditioned = 0u64;
    for item in &eligible {
        let mut entry = queue_entry(item);
        if state == "blocked" {
            let number = item
                .get("number")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| {
                    Failure::Read("a blocked queue candidate had no issue number".to_owned())
                })?;
            let conditions = blocked_conditions(store.as_ref(), &repo, number, &states);
            let holds = !conditions.is_empty()
                && conditions.iter().all(|condition| {
                    condition.get("holds") == Some(&serde_json::Value::Bool(true))
                });
            let settled_by_a_person = conditions.is_empty()
                || conditions
                    .iter()
                    .any(|condition| condition.get("unparsed").is_some());
            if holds {
                dischargeable += 1;
            } else if settled_by_a_person {
                unconditioned += 1;
            } else {
                pending += 1;
            }
            entry["conditions"] = serde_json::Value::Array(conditions);
        }
        entries.push(entry);
    }

    let partitions = partition_entries(entries);
    let unchecked = unchecked_priorities(&partitions);

    let mut answer = serde_json::json!({
        "ok": true,
        "state": state,
        "requester": run_id,
        "count": eligible.len(),
        "excluded_count": excluded.len(),
        "excluded": excluded,
        // Whether this answer *is* the queue, or the ceiling cut it off. `>=`
        // and not `==`: a ceiling of nought asks for nothing and gets nothing,
        // and an empty answer must not read as an empty queue. `count` may be
        // lower after requester exclusion, so this deliberately uses the
        // pre-filter list.
        "at_limit": items.len() as u64 >= ceiling,
        "partitions": partitions,
        "note": "ordering inside a partition needs the domain's scale contract — apply it yourself",
    });
    // Additive only: an answer with nothing unrankable is byte-identical to the
    // one from before this key existed.
    if !unchecked.is_empty() {
        answer["unchecked"] = serde_json::Value::Array(unchecked);
    }
    // Same rule: only a blocked queue has conditions to discharge, so every
    // other state's answer keeps the shape it had.
    if state == "blocked" {
        answer["dischargeable"] = serde_json::Value::from(dischargeable);
        answer["pending"] = serde_json::Value::from(pending);
        answer["unconditioned"] = serde_json::Value::from(unconditioned);
    }
    // The states whose issues are held by somebody: `open` says how many there
    // are whoever holds them, which the unassigned `count` cannot.
    if state == "review" || state == "in-progress" {
        answer["open"] = serde_json::Value::from(open);
    }
    // An open issue the store has never admitted is named, not silently dropped
    // and not placed in whatever state its label claims.
    if !unadmitted.is_empty() {
        answer["unadmitted"] = serde_json::Value::Array(unadmitted);
    }
    Ok(answer)
}

/// The identity one body recorded in its `analysis` marker, or `None`.
///
/// The empty attribute is dropped rather than reported: `markers::render`
/// never writes one, and a hand-edited `identity=` is the absent answer, not an
/// identity spelled as nothing — a caller must not read it as a key.
///
/// The first marker that carries one answers, not the first marker: a body
/// filed before the template stopped carrying its own marker holds an
/// identity-less one ahead of the one `create` composed.
pub(super) fn recorded_identity(body: &str) -> Option<String> {
    super::markers::parse(body)
        .into_iter()
        .filter(|marker| marker.get("kind").map(String::as_str) == Some("analysis"))
        .find_map(|marker| {
            marker
                .get("identity")
                .cloned()
                .filter(|identity| !identity.is_empty())
        })
}

/// `list-identities` — the dedup key each open issue recorded.
///
/// The key comes from the local store's `issues.identity`, which `create` and
/// the lazy import recorded. GitHub's open list gives the numbers and nothing
/// else, so no per-issue read is paid: `list_state` is read every loop iteration
/// and the identity matters only at the moment a finding is filed.
///
/// The answer is `identities`, each `{number, identity}`, with `identity` the
/// string the store recorded or `null` when it recorded none — including an open
/// issue the store has never admitted. `null` and never `""`: an issue with no
/// key is not an issue whose key is empty. The title is never read, because the
/// identity left it.
pub fn list_identities(context: &Context, limit: u32) -> Result<serde_json::Value, Failure> {
    let ceiling = u64::from(limit);
    let (items, open) = open_items(context, limit as usize)?;
    let repo = super::claim::repo_of(context)?;
    let store = super::store::open_for_read(&super::store::store_path()?)?;
    let rows = match &store {
        Some(store) => store.issue_rows(&repo)?,
        None => Vec::new(),
    };
    let identities: std::collections::BTreeMap<u64, Option<String>> = rows
        .into_iter()
        .map(|row| (row.number, row.identity))
        .collect();
    let provisional = provisional_items(store.as_ref(), &repo)?;
    let mut answer = Vec::new();
    for item in items.iter().chain(&provisional) {
        let number = item
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| Failure::Read("an open issue had no issue number".to_owned()))?;
        let identity = identities
            .get(&number)
            .cloned()
            .flatten()
            .filter(|identity| !identity.is_empty())
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
        let mut entry = serde_json::json!({
            "number": number,
            "identity": identity,
        });
        if item.get("provisional").is_some() {
            entry["provisional"] = serde_json::Value::Bool(true);
        }
        answer.push(entry);
    }
    Ok(serde_json::json!({
        "ok": true,
        "count": answer.len(),
        "open": open,
        "at_limit": items.len() as u64 >= ceiling,
        "identities": answer,
        "note": "`identity` is the key the issue recorded when it was filed, or null when it recorded none",
    }))
}

/// The `blocked-until` marker bodies one body recorded, rendered as markers so
/// the store can answer them back. A malformed one is kept rather than dropped:
/// the reader reports it as unevaluated, never as a clause that holds.
pub(super) fn recorded_blocked_markers(body: &str) -> Vec<String> {
    super::markers::parse(body)
        .into_iter()
        .filter(|marker| marker.get("kind").map(String::as_str) == Some("blocked-until"))
        .filter_map(|marker| {
            let attributes: Vec<(&str, &str)> = marker
                .iter()
                .filter(|(key, _)| key.as_str() != "kind")
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            super::markers::render("blocked-until", &attributes)
        })
        .collect()
}

/// The exit conditions one blocked item recorded, each already evaluated.
///
/// A clause holds when the issue it names carries the state it named, read from
/// the local store. An item that recorded none answers with an empty list, and an
/// empty list is not a discharged condition — it is a wait this read does not
/// judge, which is what a human adjudication is.
///
/// **A clause this read cannot settle is reported as one that does not hold, and
/// never dropped.** A clause that is not well formed, or that names an issue the
/// store has never admitted, must make its item *not* dischargeable — an
/// unevaluated condition is not a satisfied one — while the queue's own count
/// stays a number. The malformed clause carries `unparsed: true` and the
/// unreadable one `unread: true`, so both stay distinguishable from an item that
/// recorded no clause at all.
fn blocked_conditions(
    store: Option<&super::store::Store>,
    repo: &super::store::Repo,
    number: u64,
    states: &std::collections::BTreeMap<u64, Option<String>>,
) -> Vec<serde_json::Value> {
    let Some(store) = store else {
        return Vec::new();
    };
    let markers = match store.blocked_clauses(repo, number) {
        Ok(markers) => markers,
        Err(_) => return vec![unevaluated("unread")],
    };
    markers
        .iter()
        .map(|body| {
            let marker = super::markers::parse(body)
                .into_iter()
                .find(|marker| marker.get("kind").map(String::as_str) == Some("blocked-until"));
            let target = marker
                .as_ref()
                .and_then(|marker| marker.get("issue"))
                .and_then(|value| value.parse::<u64>().ok());
            let state = marker
                .as_ref()
                .and_then(|marker| marker.get("state"))
                .cloned()
                .filter(|state| !state.is_empty());
            match (target, state) {
                (Some(target), Some(state)) => match states.get(&target) {
                    Some(stored) => serde_json::json!({
                        "issue": target,
                        "state": state,
                        "holds": stored.as_deref() == Some(state.as_str()),
                    }),
                    None => serde_json::json!({
                        "issue": target,
                        "state": state,
                        "holds": false,
                        "unread": true,
                    }),
                },
                _ => unevaluated("unparsed"),
            }
        })
        .collect()
}

/// One clause that cannot be reported as holding, named by why.
fn unevaluated(why: &str) -> serde_json::Value {
    serde_json::json!({
        "issue": serde_json::Value::Null,
        "state": serde_json::Value::Null,
        "holds": false,
        why: true,
    })
}

/// One issue's fields, or a read failure.
///
/// `gh` answering nothing is not an empty issue: it is a read that did not
/// happen, and the difference decides whether a caller may write.
pub(super) fn issue_view(
    context: &Context,
    issue: u64,
    fields: &str,
) -> Result<serde_json::Value, Failure> {
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
/// `claim::verify_claim` — the half of the pair that *reads* the state while
/// `transition` is the half that *writes* it.
pub(super) fn is_status_label(name: &str) -> bool {
    name.starts_with("status:")
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

/// A finding identity means nothing without the receipt it was recorded
/// against, so `--findings` without a usable `--epoch` is refused.
///
/// A blank value is no value, the module's own empty-filter convention: an
/// empty `--findings` files an issue with no finding identity rather than being
/// refused, and an empty `--epoch` is no receipt. Refused here rather than only
/// at the marker so `dispatch`'s `create` can ask it before `commands::create`
/// writes a label.
pub(super) fn findings_require_an_epoch(
    epoch: Option<&str>,
    findings: Option<&str>,
) -> Result<(), Failure> {
    let findings = findings.filter(|value| !value.is_empty());
    let epoch = epoch.filter(|value| !value.is_empty());
    if findings.is_some() && epoch.is_none() {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "finding-without-transaction",
            "argument": "--findings",
            "detail": "--findings names a receipt; --epoch must be supplied and non-empty",
        })));
    }
    Ok(())
}

/// The `review` marker a finding-filed issue is recorded with, or `None` when
/// the call names no epoch. It is kept in the local store beside the issue's
/// other recorded markers and never rendered into the filed body.
pub(super) fn review_marker(
    run_id: Option<&str>,
    identity: &str,
    epoch: Option<&str>,
    findings: Option<&str>,
) -> Result<Option<String>, Failure> {
    findings_require_an_epoch(epoch, findings)?;
    let Some(epoch) = epoch.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let mut attributes = vec![
        ("run-id", run_id.unwrap_or_default()),
        ("identity", identity),
        ("epoch", epoch),
    ];
    if let Some(findings) = findings.filter(|value| !value.is_empty()) {
        attributes.push(("findings", findings));
    }
    super::markers::render("review", &attributes)
        .map(Some)
        .ok_or_else(|| {
            Failure::Stop(serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }))
        })
}

fn invalid_clause(detail: String) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "invalid-blocked-clause",
        "detail": detail,
        "action": "call `blocked_until` again with `until_issue` and `until_state` naming an \
                   issue and a workflow state, or with `clear`; for `create`, spell \
                   `blocked_until` as `<issue>=<state>` joined by `+` and file it `blocked`",
    }))
}

fn clause_marker(target: u64, state: &str) -> Result<String, Failure> {
    if target == 0 {
        return Err(invalid_clause("`0` is not an issue number".to_owned()));
    }
    if !STATES.contains(&state) {
        return Err(invalid_clause(format!(
            "`{state}` is not a workflow state the local store can hold"
        )));
    }
    super::markers::render(
        "blocked-until",
        &[("issue", &target.to_string()), ("state", state)],
    )
    .ok_or_else(|| invalid_clause("the clause could not be rendered".to_owned()))
}

fn creation_clauses(spec: Option<&str>, state: &str) -> Result<Vec<String>, Failure> {
    let Some(spec) = spec.filter(|spec| !spec.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    if state != "blocked" {
        return Err(invalid_clause(format!(
            "clauses belong to a blocked issue, and this one is filed `{state}`"
        )));
    }
    spec.split('+')
        .map(|pair| {
            let (issue, until) = pair.split_once('=').ok_or_else(|| {
                invalid_clause(format!("`{pair}` is not spelled `<issue>=<state>`"))
            })?;
            let target = issue
                .trim()
                .parse::<u64>()
                .map_err(|_| invalid_clause(format!("`{issue}` is not an issue number")))?;
            clause_marker(target, until.trim())
        })
        .collect()
}

/// `blocked-until` — record or clear the clauses that discharge a blocked issue.
///
/// Local to the store: nothing is written to the tracker. A clause holds when
/// the store puts issue `until_issue` in state `until_state`, and the queue read
/// counts it exactly as it counts a clause imported from a body. No claim is
/// required, because the analyst files a blocked issue without holding one; the
/// issue must be one the store holds in `blocked`, since a clause on any other
/// state would never be read.
pub fn blocked_until(
    context: &Context,
    issue: u64,
    until_issue: Option<u64>,
    until_state: Option<&str>,
    clear: bool,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let clause = match (clear, until_issue, until_state) {
        (true, None, None) => None,
        (false, Some(target), Some(state)) => Some(clause_marker(target, state)?),
        (true, _, _) => {
            return Err(invalid_clause(
                "`clear` removes the recorded clauses and takes no clause of its own".to_owned(),
            ));
        }
        (false, _, _) => {
            return Err(invalid_clause(
                "a clause needs both `until_issue` and `until_state`, or `clear`".to_owned(),
            ));
        }
    };
    let repo = super::claim::repo_of(context)?;
    let mut store = super::store::Store::open(&super::store::store_path()?)?;
    let state = store.issue_state(&repo, issue)?;
    if state.as_deref() != Some("blocked") {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "issue-not-blocked",
            "issue": issue,
            "state": state,
            "action": "a clause is read only for an issue the store holds in `blocked`; file or \
                       transition it there first",
        })));
    }
    let mut answer = serde_json::json!({ "ok": true, "issue": issue });
    match clause {
        Some(clause) => {
            answer["recorded"] = store
                .record_blocked_clause(&repo, issue, &clause, now)?
                .into()
        }
        None => answer["cleared"] = store.clear_blocked_clauses(&repo, issue)?.into(),
    }
    answer["clauses"] = store.blocked_clauses(&repo, issue)?.len().into();
    Ok(answer)
}

fn invalid_task(detail: String) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "invalid-task",
        "detail": detail,
        "action": "call `task` again with an id, a route, a status, a commit and an evidence note \
                   in the shapes its schema names",
    }))
}

/// One `task` call: the row to write and the claim to write it under.
pub struct TaskCall<'a> {
    /// The claimed issue.
    pub issue: u64,
    /// The run that holds it.
    pub run_id: &'a str,
    /// The state the run believes the issue is in.
    pub expect_state: &'a str,
    /// The task's stable id.
    pub id: &'a str,
    /// What the task is and its acceptance.
    pub text: Option<&'a str>,
    /// `inline` or `delegated`.
    pub route: Option<&'a str>,
    /// `open` or `done`.
    pub status: Option<&'a str>,
    /// The work-unit commit identity.
    pub commit: Option<&'a str>,
    /// Where the work is recorded when it is not a commit.
    pub evidence: Option<&'a str>,
    /// Answer with the changed row alone, without the whole checklist.
    pub compact: bool,
    /// The caller's clock.
    pub now: &'a str,
}

fn task_json(row: &super::store::TaskRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "text": row.text,
        "route": row.route,
        "done": row.done,
        "commit": row.commit,
        "evidence": row.evidence,
    })
}

/// `task` — record or update one row of the claimed issue's task checklist.
///
/// The checklist lives in the local store, never on the tracker: a write is
/// refused unless the run holds the issue, and a revision is the same id with new
/// fields. A new row needs its text and route; `done` needs a commit identity, or
/// an `evidence` note when the work is not a commit. The answer carries the
/// changed row beside the whole checklist unless [`TaskCall::compact`] asks for
/// the row alone.
pub fn task(context: &Context, call: &TaskCall<'_>) -> Result<serde_json::Value, Failure> {
    let well_formed_id = !call.id.is_empty()
        && call.id.len() <= 40
        && call
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !well_formed_id {
        return Err(invalid_task(format!(
            "`{}` is not a task id: 1 to 40 of letters, digits, `.`, `_` or `-`",
            call.id
        )));
    }
    if let Some(route) = call.route
        && !["inline", "delegated"].contains(&route)
    {
        return Err(invalid_task(format!(
            "`{route}` is not `inline` or `delegated`"
        )));
    }
    if let Some(text) = call.text
        && (text.trim().is_empty() || text.len() > 4000)
    {
        return Err(invalid_task(
            "`text` is blank or longer than 4000 bytes".to_owned(),
        ));
    }
    let done = match call.status {
        None => None,
        Some("open") => Some(false),
        Some("done") => Some(true),
        Some(other) => {
            return Err(invalid_task(format!("`{other}` is not `open` or `done`")));
        }
    };
    if let Some(commit) = call.commit
        && !((7..=64).contains(&commit.len())
            && commit.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')))
    {
        return Err(invalid_task(format!(
            "`{commit}` is not a commit identity: 7 to 64 lowercase hex digits"
        )));
    }
    if let Some(evidence) = call.evidence
        && (evidence.trim().is_empty() || evidence.len() > 4000)
    {
        return Err(invalid_task(
            "`evidence` is blank or longer than 4000 bytes".to_owned(),
        ));
    }
    super::claim::verify_claim(
        context,
        call.issue,
        call.run_id,
        call.expect_state,
        call.now,
        None,
    )?;
    let repo = super::claim::repo_of(context)?;
    let mut store = super::store::Store::open(&super::store::store_path()?)?;
    let change = super::store::TaskChange {
        id: call.id,
        text: call.text.map(str::trim),
        route: call.route,
        done,
        commit: call.commit,
        evidence: call.evidence.map(str::trim),
    };
    let row = store.change_task(&repo, call.issue, &change, call.now)?;
    if call.compact {
        return Ok(serde_json::json!({
            "ok": true,
            "issue": call.issue,
            "task": task_json(&row),
        }));
    }
    let tasks: Vec<_> = store
        .tasks(&repo, call.issue)?
        .iter()
        .map(task_json)
        .collect();
    Ok(serde_json::json!({
        "ok": true,
        "issue": call.issue,
        "task": task_json(&row),
        "tasks": tasks,
    }))
}

/// `tasks` — the task checklist the store holds for one issue, in the order the
/// tasks were first recorded. Read-only and needing no claim: a run resuming an
/// issue reads it before it holds anything.
///
/// The repository comes from the shared scope resolver, so a number the checkout has never
/// seen but the store or a live run places elsewhere reads that repository's
/// checklist; several candidates are refused `ambiguous-issue`. A store that does
/// not exist, or does not hold the repository, answers `unavailable` and no count:
/// an empty checklist is a read, and nothing was read.
pub fn tasks(context: &Context, issue: u64) -> Result<serde_json::Value, Failure> {
    let path = super::store::store_path()?;
    let runs = path.with_file_name("runs");
    let scope = super::scope::resolve(
        context,
        &crate::identity::Reference::Local(crate::identity::Identity::Number(issue)),
        &super::scope::Places {
            store_file: &path,
            state_root: Some(&runs),
        },
        super::scope::Prefer::Checkout,
    )?;
    let named = super::scope::spelled(&scope.repo);
    let held = match super::store::open_for_read(&path)? {
        None => Err("the local store does not exist yet".to_owned()),
        Some(store) if store.holds_repo(&scope.repo)? => Ok(store.tasks(&scope.repo, issue)?),
        Some(_) => Err(format!("the local store holds no repository {named}")),
    };
    Ok(match held {
        Ok(rows) => {
            let tasks: Vec<_> = rows.iter().map(task_json).collect();
            serde_json::json!({
                "ok": true,
                "issue": issue,
                "repo": named,
                "scope": scope.basis.word(),
                "count": tasks.len(),
                "tasks": tasks,
            })
        }
        Err(why) => serde_json::json!({
            "ok": true,
            "issue": issue,
            "repo": named,
            "scope": scope.basis.word(),
            "unavailable": why,
            "tasks": [],
        }),
    })
}

/// `create` — file an issue, recording its identity and `blocked-until` clauses
/// in the local store rather than in the body.
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
    epoch: Option<&str>,
    findings: Option<&str>,
    blocked_until: Option<&str>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let body = std::fs::read_to_string(body_file)
        .map_err(|error| Failure::Read(format!("the body file could not be read: {error}")))?;
    super::template::check(&body)?;
    let clauses = creation_clauses(blocked_until, state)?;
    let review = review_marker(run_id, identity, epoch, findings)?;
    let mut labels = vec![format!("status:{state}")];
    let mut ensure: Vec<(String, &str)> = STATES
        .iter()
        .map(|status| (format!("status:{status}"), "ededed"))
        .collect();
    if let Some(priority) = priority.filter(|value| !value.is_empty()) {
        ensure.push((priority.to_owned(), "d93f0b"));
        labels.push(priority.to_owned());
    }
    if let Some(domain) = domain.filter(|value| !value.is_empty()) {
        let label = format!("domain:{domain}");
        ensure.push((label.clone(), "0e8a16"));
        labels.push(label);
    }
    if let Some(runtime) = runtime.filter(|value| !value.is_empty()) {
        let label = format!("analyst:{runtime}");
        ensure.push((label.clone(), "c5def5"));
        labels.push(label);
    }

    let closed = super::outbox::is_closed(context);
    if !closed {
        for (name, colour) in &ensure {
            ensure_label(context, name, colour)?;
        }
    }

    let repo = super::claim::repo_of(context)?;
    let mut store = super::store::Store::open(&super::store::store_path()?)?;
    if closed {
        return create_offline(
            &mut store,
            &repo,
            &OfflineCreate {
                identity,
                title,
                body: &body,
                state,
                labels: &labels,
                ensure: &ensure,
                clauses: &clauses,
                review: review.as_deref(),
                priority_checked: priority.filter(|value| !value.is_empty()).map(|priority| {
                    priority_scale_checked(priority, domain.filter(|value| !value.is_empty()))
                }),
                now,
            },
        );
    }
    let pending = pending_create_key(&repo, identity, title);
    let (number, url) = file_or_adopt(
        context, &mut store, &repo, None, &pending, title, &body, state, &labels, false,
    )?;

    // The case everyone forgets: a fresh issue reaches its projects with an
    // empty Status and no transition ever follows to correct it.
    let mirror = super::board::set_status(context, number, state);

    {
        let labels_payload: Vec<serde_json::Value> = labels
            .iter()
            .map(|label| serde_json::json!({ "name": label }))
            .collect();
        let payload = serde_json::json!({
            "body": body,
            "labels": labels_payload,
            "comments": [],
        });
        let recorded = super::store::Recorded {
            identity: Some(identity).filter(|identity| !identity.is_empty()),
            clauses: &clauses,
            review: review.as_deref(),
        };
        store
            .import_recorded(&repo, number, &payload, now, &recorded)
            .map_err(|failure| {
                Failure::Write(format!(
                    "issue #{number} was filed but the local store did not record it: {}; \
                     calling `create` again with the same identity and title adopts issue \
                     #{number} instead of filing a duplicate",
                    failure.detail()
                ))
            })?;
        let _ = store.set_meta(&pending, "");
    }

    let mut answer = serde_json::json!({
        "ok": true,
        "issue": number,
        "url": url,
        "labels": labels,
        "board": mirror,
    });
    // Filed regardless; the queue read, not the filing, is what reports drift.
    if let Some(priority) = priority.filter(|value| !value.is_empty()) {
        answer["priority_checked"] = serde_json::Value::Bool(priority_scale_checked(
            priority,
            domain.filter(|value| !value.is_empty()),
        ));
    }
    Ok(answer)
}

pub(super) fn pending_create_key(repo: &super::store::Repo, identity: &str, title: &str) -> String {
    format!(
        "pending-create {}/{}/{} {}",
        repo.host,
        repo.owner,
        repo.name,
        Some(identity)
            .filter(|identity| !identity.is_empty())
            .unwrap_or(title)
    )
}

/// Files the issue, or adopts the one an earlier attempt filed under exactly
/// this title: `retrying` says an earlier attempt may have reached GitHub even
/// when the store kept no mark of it. Only an issue the authenticated user
/// created at or after `since` and the store does not already hold is adopted.
#[allow(clippy::too_many_arguments)]
pub(super) fn file_or_adopt(
    context: &Context,
    store: &mut super::store::Store,
    repo: &super::store::Repo,
    since: Option<&str>,
    pending: &str,
    title: &str,
    body: &str,
    state: &str,
    labels: &[String],
    retrying: bool,
) -> Result<(u64, String), Failure> {
    let earlier = retrying || store.meta(pending)?.is_some_and(|held| !held.is_empty());
    let adopted = if earlier {
        filed_under_title(context, store, repo, title, since)?
    } else {
        None
    };
    match adopted {
        Some(found) => Ok(found),
        None => {
            store.set_meta(pending, title)?;
            file_issue(context, title, body, state, labels)
        }
    }
}

struct OfflineCreate<'a> {
    identity: &'a str,
    title: &'a str,
    body: &'a str,
    state: &'a str,
    labels: &'a [String],
    ensure: &'a [(String, &'a str)],
    clauses: &'a [String],
    review: Option<&'a str>,
    priority_checked: Option<bool>,
    now: &'a str,
}

/// The `create` of a closed window: the issue is recorded under a provisional
/// number, and the filing waits in the queue for the number GitHub will issue.
fn create_offline(
    store: &mut super::store::Store,
    repo: &super::store::Repo,
    what: &OfflineCreate<'_>,
) -> Result<serde_json::Value, Failure> {
    let provisional = store.allocate_provisional(repo)?;
    let labels_payload: Vec<serde_json::Value> = what
        .labels
        .iter()
        .map(|label| serde_json::json!({ "name": label }))
        .collect();
    let recorded = super::store::Recorded {
        identity: Some(what.identity).filter(|identity| !identity.is_empty()),
        clauses: what.clauses,
        review: what.review,
    };
    store.import_recorded(
        repo,
        provisional,
        &serde_json::json!({
            "body": what.body,
            "labels": labels_payload,
            "comments": [],
        }),
        what.now,
        &recorded,
    )?;
    let ensure: Vec<serde_json::Value> = what
        .ensure
        .iter()
        .map(|(name, colour)| serde_json::json!([name, colour]))
        .collect();
    let queued = super::outbox::queue_in(
        store,
        repo,
        provisional,
        super::store::OutboxKind::Create,
        &serde_json::json!({
            "title": what.title,
            "body": what.body,
            "state": what.state,
            "labels": what.labels,
            "ensure": ensure,
            "identity": what.identity,
        }),
        what.now,
    )?;
    super::outbox::queue_in(
        store,
        repo,
        provisional,
        super::store::OutboxKind::Reconverge,
        &serde_json::json!({}),
        what.now,
    )?;
    let mut answer = serde_json::json!({
        "ok": true,
        "issue": provisional,
        "provisional": true,
        "url": null,
        "labels": what.labels,
        "board": "deferred",
        "queued": queued,
    });
    if let Some(checked) = what.priority_checked {
        answer["priority_checked"] = serde_json::Value::Bool(checked);
    }
    Ok(answer)
}

fn file_issue(
    context: &Context,
    title: &str,
    body: &str,
    state: &str,
    labels: &[String],
) -> Result<(u64, String), Failure> {
    static STAGED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let staged = crate::paths::scratch_file(&format!(
        "create-{state}-{}.md",
        STAGED.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    crate::paths::replace_atomically(&staged, body)
        .map_err(|error| Failure::Write(format!("the body could not be staged: {error}")))?;
    let staged_path = staged.display().to_string();
    let mut arguments = vec![
        "gh",
        "issue",
        "create",
        "--title",
        title,
        "--body-file",
        &staged_path,
    ];
    for label in labels {
        arguments.push("--label");
        arguments.push(label);
    }
    let answer = super::run(
        &arguments,
        Some(&context.repo_dir),
        super::outbox::flush::bounded_write(),
    );
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
    Ok((number, url))
}

const ADOPTION_MARGIN_SECONDS: i64 = 300;

fn epoch_seconds(stamp: &str) -> Option<i64> {
    let digits = |from: usize, to: usize| stamp.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (digits(0, 4)?, digits(5, 7)?, digits(8, 10)?);
    let (hour, minute, second) = (digits(11, 13)?, digits(14, 16)?, digits(17, 19)?);
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn title_phrase_query(title: &str) -> String {
    format!("\"{}\" in:title", title.replace('"', " "))
}

/// The one issue the authenticated user filed under exactly `title` at or
/// after `since`, which a `create` whose store admission failed left behind.
/// An issue the store already holds is not a candidate, an unreadable answer
/// is a failure and not an empty list, and two candidates are not guessed
/// between.
fn filed_under_title(
    context: &Context,
    store: &super::store::Store,
    repo: &super::store::Repo,
    title: &str,
    since: Option<&str>,
) -> Result<Option<(u64, String)>, Failure> {
    let floor = since
        .map(|stamp| {
            epoch_seconds(stamp)
                .map(|seconds| seconds - ADOPTION_MARGIN_SECONDS)
                .ok_or_else(|| Failure::Read(format!("the row time {stamp:?} could not be read")))
        })
        .transpose()?;
    let listed = super::gh_json(
        &[
            "issue",
            "list",
            "--state",
            "all",
            "--author",
            "@me",
            "--search",
            &title_phrase_query(title),
            "--json",
            "number,title,url,createdAt",
            "--limit",
            "30",
        ],
        Some(&context.repo_dir),
    )?;
    let items = listed
        .as_ref()
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            Failure::Read(format!(
                "the title search for {title:?} did not answer a list of issues"
            ))
        })?;
    let mut matches: Vec<(u64, String)> = Vec::new();
    for item in items {
        if item.get("title").and_then(serde_json::Value::as_str) != Some(title) {
            continue;
        }
        let (Some(number), Some(url)) = (
            item.get("number").and_then(serde_json::Value::as_u64),
            item.get("url").and_then(serde_json::Value::as_str),
        ) else {
            continue;
        };
        if let Some(floor) = floor {
            let created = item
                .get("createdAt")
                .and_then(serde_json::Value::as_str)
                .and_then(epoch_seconds);
            if !created.is_some_and(|created| created >= floor) {
                continue;
            }
        }
        if store.holds_issue(repo, number)? {
            continue;
        }
        matches.push((number, url.to_owned()));
    }
    match matches.as_slice() {
        [] => Ok(None),
        [only] => Ok(Some(only.clone())),
        several => Err(Failure::Write(format!(
            "an earlier `create` filed an issue its store did not record, and {} issues carry \
             the title {title:?} ({}); none was adopted, so a duplicate was not filed",
            several.len(),
            several
                .iter()
                .map(|(number, _)| format!("#{number}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
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
    now: &str,
) -> Result<serde_json::Value, Failure> {
    require_state(to)?;
    transition_on(
        context,
        &super::store::Ledger::open(context, issue, now)?,
        issue,
        to,
        from,
        now,
    )
}

/// Refuses a state the local store cannot hold, before anything is read.
pub(crate) fn require_state(to: &str) -> Result<(), Failure> {
    if STATES.contains(&to) {
        return Ok(());
    }
    Err(Failure::Read(format!(
        "`{to}` is not a workflow state the local store can hold, so nothing was written"
    )))
}

/// [`transition`] against a ledger this call already opened.
pub(crate) fn transition_on(
    context: &Context,
    ledger: &super::store::Ledger,
    issue: u64,
    to: &str,
    from: Option<&str>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let before = ledger.state().ok().flatten();
    let delivery = if to == "done" {
        super::claim::verified_delivery(context, &ledger.timeline()?)?
    } else {
        None
    };
    if super::outbox::is_closed(context) {
        ledger.set_state(to, now)?;
        let queued = super::outbox::queue(
            context,
            issue,
            super::store::OutboxKind::Reconverge,
            &serde_json::json!({}),
            now,
        )?;
        let mut deferred = serde_json::json!({
            "ok": true,
            "issue": issue,
            "to": to,
            "projection": "deferred",
            "queued": queued,
        });
        if let Some(delivery) = delivery {
            deferred["delivery"] = delivery;
        }
        super::store::report_unrecorded(
            &mut deferred,
            need_person_for_blocked(ledger, to, before.as_deref(), now),
        );
        return Ok(deferred);
    }
    let mut result = project_state(context, ledger, issue, to, from, now)?;
    super::store::report_unrecorded(
        &mut result,
        need_person_for_blocked(ledger, to, before.as_deref(), now),
    );
    if let Some(delivery) = delivery {
        result["delivery"] = delivery;
    }
    Ok(result)
}

fn need_person_for_blocked(
    ledger: &super::store::Ledger,
    to: &str,
    before: Option<&str>,
    now: &str,
) -> Result<bool, super::Failure> {
    if to == "blocked" && before != Some("blocked") && !ledger.has_blocked_clause() {
        return ledger.needs_person("blocked", None, now);
    }
    Ok(false)
}

/// The projection half of [`transition`]: the board column, the `status:`
/// label, and the read-back of both, with the store recording the state
/// between the label edit and its read-back.
pub(super) fn project_state(
    context: &Context,
    ledger: &super::store::Ledger,
    issue: u64,
    to: &str,
    from: Option<&str>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let mirror = super::board::set_status(context, issue, to);

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
            // The removal, unless it is the label being added:
            // `--add-label status:done --remove-label status:done` is a
            // self-cancelling edit and `gh` settles it by dropping the label.
            // The add is kept, so the call still ends with the issue in `to`
            // whether or not it started there.
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
    // With what the mirror did, when this fails: the board is moved **first**,
    // so a label edit that fails leaves the board ahead of the authoritative
    // store — the caller must read it as well as the labels before deciding
    // what happened. The mirror's own words rather than a guess, since moved,
    // skipped and never attempted need different answers.
    if let Err(failure) = super::run(&borrowed, Some(&context.repo_dir), super::How::write()) {
        let Failure::Write(detail) = failure else {
            return Err(failure);
        };
        return Err(Failure::Write(format!(
            "{detail} — the project projection ran first and answered {mirror}, so read it as well \
             as the labels before deciding what happened"
        )));
    }

    ledger.set_state(to, now).map_err(|failure| {
        Failure::Write(format!(
            "the labels moved but the local store did not record the state: {}",
            failure.detail()
        ))
    })?;

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
            // are now, they are what this call left — this must not read as
            // *nothing was written*.
            "world": "committed",
            "detail": format!("expected exactly [status:{to}], found {after:?}"),
            // Naming the repair rather than forbidding it: omitting `from`
            // removes whatever stale state labels are found, and that is what
            // clears this.
            "action": format!(
                "fix on the spot \u{2014} a two-state item poisons every query touching either \
                 state. Repeat this transition as `transition --to {to}` with `from` omitted, \
                 which removes whatever stale state labels are found"
            ),
        })));
    }

    // The read-back covers the projects because that is the half with no other
    // feedback loop: a wrong label is caught by the next `list-state`, a column
    // nobody looks at stays wrong forever.
    let reading = super::board::read_status(context, issue, to);
    result["board_column_after"] = reading.clone();
    if !agreed(&reading) {
        // An empty Status is possible and the mutation addresses the column by
        // its stable id, so distrust a single read before believing the
        // projection failed: project again and re-read.
        let retried = super::board::set_status(context, issue, to);
        let recheck = super::board::read_status(context, issue, to);
        result["board_repair"] = serde_json::json!({
            "retried": retried,
            "column_now": recheck,
        });
        if !agreed(&recheck) {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "board-readback-failed",
                "detail": format!("a project's column disagrees with status:{to}: {recheck}"),
                "action": "the projection is not landing — investigate before continuing",
            })));
        }
    }
    Ok(result)
}

/// Whether every project in a `read_status` answer carries the state asked for.
///
/// A project with no matching column reports `agrees: true` because the mirror
/// had nothing to write there; only a project that was given a target and shows
/// another one is a disagreement.
fn agreed(reading: &serde_json::Value) -> bool {
    reading
        .get("projects")
        .and_then(serde_json::Value::as_array)
        .is_none_or(|projects| {
            projects.iter().all(|project| {
                project.get("agrees").and_then(serde_json::Value::as_bool) != Some(false)
            })
        })
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

/// Records one protocol entry this crate composed in the local store. Nothing
/// is posted: the review family's markers are the store's alone, and GitHub
/// keeps only the prose a person wrote.
pub(super) fn record_entry(
    context: &Context,
    issue: u64,
    body: &str,
    now: &str,
) -> Result<(), Failure> {
    super::store::Ledger::open(context, issue, now)?.append(body, now)
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
    let mut elsewhere = Vec::new();
    for reference in &refs {
        let number = reference
            .get("number")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default()
            .to_string();
        // A body nobody could read is not a body with no closing keyword: it
        // does not change *whether* the issue auto-closes, but it changes
        // **why**, and the remedy differs completely by cause.
        let body = super::gh_json(
            &["pr", "view", &number, "--json", "body"],
            Some(&context.repo_dir),
        )?
        .ok_or_else(|| {
            Failure::Read(format!(
                "gh pr view {number} returned nothing, so why this issue auto-closes is unknown"
            ))
        })?;
        let body = text_of(&body, "body");
        for text in super::closing::keywords_naming(&body, issue) {
            hits.push(serde_json::json!({ "where": format!("pr#{number} body"), "text": text }));
        }
        for text in super::closing::keywords_elsewhere(&body, issue) {
            elsewhere
                .push(serde_json::json!({ "where": format!("pr#{number} body"), "text": text }));
        }
    }
    if let (Some(base), Some(branch)) = (base, branch) {
        let log = super::closing::commit_log(&context.repo_dir, base, branch)?;
        for text in super::closing::keywords_naming(&log, issue) {
            hits.push(serde_json::json!({ "where": "commit message", "text": text }));
        }
        for text in super::closing::keywords_elsewhere(&log, issue) {
            elsewhere.push(serde_json::json!({ "where": "commit message", "text": text }));
        }
    }

    Ok(serde_json::json!({
        "will_autoclose": true,
        "cause": if hits.is_empty() { "branch-link" } else { "closing-keyword" },
        "keyword_sources": hits,
        "other_issue_keywords": elsewhere,
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

    let elsewhere = verdict
        .get("other_issue_keywords")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|keywords| !keywords.is_empty());
    if elsewhere {
        let action = [
            "remove the keyword — use `Refs #<n>` — then re-run; it would auto-close another",
            "issue on merge and skip that issue's transition and mirror",
        ]
        .join(" ");
        let mut payload = serde_json::json!({
            "ok": false,
            "reason": "closing-keyword-live",
            "cause": "closing-keyword-elsewhere",
        });
        merge_into(&mut payload, &verdict);
        payload["cause"] = serde_json::json!("closing-keyword-elsewhere");
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
        result["warning"] = serde_json::json!(warning);
        result["mandatory_follow_up"] = serde_json::json!(super::closing::FOLLOW_UP);
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
///   **and** a read-only merge reports no conflict — *compatible*: the movement
///   itself needs nothing, which is the `"integrate": false` this emits.
///   Merging the base routinely is what the policy forbids: it rewrites the
///   review target for no reason and invalidates evidence already bound to the
///   old head;
/// - it moved and touched paths this delivery also touches, or the merge
///   conflicts — *material*, integrated before final review.
///
/// `"integrate": false` is the movement's own answer. It is **not reachable as
/// advice under `required_status_checks.strict: true`**, the protection this
/// repository's delivery assumes and which nothing here reads back — an off
/// setting is a boundary `docs/honesty.md` states rather than a state this
/// function can see. Under it, GitHub requires the topic branch to be up to
/// date with the base before merging: every movement but `none` forces an
/// update, the update is a push that mints a new epoch, and the contract spends
/// that forced integration by the movement — a repair-shaped republish over the
/// parent receipt for `compatible`, a whole-change re-review for every other
/// value (`skill/protocols/rdd.md`, "When the base moves").
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
        // wrong here: the answer *is* a list of paths, and a replaced byte
        // makes one of them a path that is not the path. Refused rather than
        // guessed, the same way `manifest_trouble` refuses an unreadable tree
        // entry instead of naming it something.
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
        super::manifest::Trouble::Unhashable(path) => format!(
            "{path:?} is changed but could not be hashed, so the target would silently omit it"
        ),
        super::manifest::Trouble::Empty => {
            "the tree listed no entries; an empty answer is not a delivery target".to_owned()
        }
    })
}

#[cfg(test)]
mod tests;
