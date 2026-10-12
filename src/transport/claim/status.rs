//! `review_status`: the one next step of a review, derived from the local store.

use serde_json::{Value, json};

use std::path::Path;

use super::capture::GRACE_SECONDS;
use super::computed::{self, ComputedOutcome};
use super::validator::VALIDATOR_ROLE;
use super::{
    NO_EPOCH, ReceiptStanding, ReviewTransaction, Standing, all_publications,
    corrections_per_lineage, corrections_spent, receipt_standing, recover,
    refuse_unconfirmed_publication, refutations_for, release_basis, view_pr,
};
use crate::config::RepoRef;
use crate::harness::session::now_seconds;
use crate::reviewer::job::{JobStatus, job_status};
use crate::transport::store::{Job, Ledger, Repo, store_path};
use crate::transport::{Context, Failure, How, run};

const LAUNCH_REVIEW_INPUT: &str = "launch_review_input";
const COLLECT_REVIEW_INPUT: &str = "collect_review_input";
const STALE_VERDICT: [&str; 2] = ["computed-verdict-stale", "computed-verdict-tier-mismatch"];
const UNRECORDED: [&str; 5] = [
    "computed-verdict-unrecorded",
    "computed-verdict-incomplete",
    "computed-verdict-escalated",
    "computed-verdict-rejected",
    "qualifying-review-verdict-missing",
];
const STALE_WITHOUT_CAUSE: &str = "the recorded approval no longer matches the ledger or the tier, \
    and no finding was recorded against it: publishing over an approval needs such a finding or an \
    advanced base, and the recorded verdict cannot be recorded over. A person decides: when the \
    change that made it stale established a defect the verdict did not weigh, record it against \
    this receipt with `record_review_finding` and publish again; otherwise close the pull request \
    and publish the change afresh";
const NO_REFUTER_RESULT: &str = "no-refuter-result";
const FRESH_APPROVED_MOVED: &str = "the pull request's head or base is not the one the newest \
    publication recorded, and this receipt stands approved with no finding recorded after its \
    verdict. Publishing again is admitted only when the base advanced past the one this receipt \
    recorded; otherwise it is refused `approved-republish-without-cause`. Read the pull request: \
    record the defect the verdict did not weigh against this receipt with `record_review_finding` \
    and publish again, which admits the publication; when the move is the base's own advance, \
    `publish_review` integrates it";
const DEFAULT_MOVED: &str = "the pull request's head or base is not the one the newest publication \
    recorded: read the pull request, then publish it again deliberately with `publish_review`";

/// What the review of one issue does next, answered from the local store, the pull request's own
/// head, base, draft and merge state, and the publication lane. Reads no comment and no label.
///
/// The answer names the receipt it was read against, and every command it names carries that
/// binding (`epoch`/`pr`/`head`/`base`/`digest`, or `expected_epoch` for a publication), so a
/// publication landing afterwards is refused as stale by the command, never executed.
pub fn review_status(context: &Context, issue: u64) -> Result<Value, Failure> {
    let comments = Ledger::read_timeline_at(&store_path()?, &repo_of(context)?, issue)?;
    let publications = all_publications(&comments)?;
    let Some(latest) = publications.last() else {
        return Ok(answer(
            issue,
            None,
            execute(
                "publish_review",
                "no-publication",
                json!({"issue": issue, "expected_epoch": NO_EPOCH}),
            ),
        ));
    };
    let receipt = &latest.receipt;
    let pull = view_pr(context, receipt.pr)?;
    let standing = receipt_standing(&comments, receipt)?;
    let escalation = standing.escalated.then(|| {
        if recover::recovered(&comments, receipt) {
            republish(issue, receipt, "recovered")
        } else {
            with_why(
                halt(
                    "escalated",
                    &format!(
                        "a person runs `estigia recover {issue} --reason \"<why>\"`; an agent does \
                         not. The epoch and the login default to this escalated receipt and the \
                         authenticated `gh` user, and `--epoch {}` names the receipt when the \
                         issue holds more than one",
                        receipt.epoch
                    ),
                ),
                &computed::escalation_story(&comments, receipt)
                    .unwrap_or_else(|| ESCALATED_WITHOUT_CAUSE.to_owned()),
            )
        }
    });
    let next = match pull.get("state").and_then(Value::as_str) {
        Some("MERGED") => halt("delivered", &delivered_continuation()),
        Some("CLOSED") => match escalation {
            Some(held) if held["kind"] == "stop" => held,
            _ => execute(
                "publish_review",
                "pull-request-closed",
                json!({"issue": issue, "expected_epoch": NO_EPOCH}),
            ),
        },
        Some("OPEN") => {
            refuse_unconfirmed_publication(latest)?;
            if let Some(held) = escalation {
                held
            } else {
                if !on_receipt(&pull, receipt) {
                    moved_off_receipt(context, issue, receipt, &comments, &standing)?
                } else {
                    let bound = binding(issue, receipt);
                    match standing.standing {
                        Standing::Approved => {
                            if let Some(next) =
                                refused_approval(context, issue, receipt, &comments, &standing)?
                            {
                                next
                            } else if pull.get("isDraft").and_then(Value::as_bool) == Some(false) {
                                halt("released", &released_continuation(receipt))
                            } else {
                                execute("release_ci", "approved", bound)
                            }
                        }
                        Standing::Rejected => {
                            let publications = all_publications(&comments)?;
                            if corrections_spent(&comments, &publications, receipt.pr)?
                                >= corrections_per_lineage(context)
                            {
                                with_why(
                                    execute("escalate_review", "correction-spent", bound),
                                    CORRECTION_SPENT_WHY,
                                )
                            } else {
                                let due = republish(issue, receipt, "correction-due");
                                match computed::validator_rejection_story(&comments, receipt) {
                                    Some(story) => with_why(due, &story),
                                    None => due,
                                }
                            }
                        }
                        Standing::Unjudged => unjudged(context, &comments, receipt, bound)?,
                    }
                }
            }
        }
        _ => {
            return Err(Failure::Read(format!(
                "gh pr view {} answered no state",
                receipt.pr
            )));
        }
    };
    Ok(answer(issue, Some(receipt), next))
}

fn delivered_continuation() -> String {
    format!(
        "the pull request is merged; the review is delivered and nothing is left to review. The \
         run holding this receipt completes the delivery: {}",
        crate::transport::closing::FOLLOW_UP,
    )
}

fn released_continuation(receipt: &ReviewTransaction) -> String {
    format!(
        "CI is released for this receipt. When CI is green on this exact head and base, the run \
         holding this receipt merges it with `gh pr merge {pr} --merge --match-head-commit {head}` \
         if `Delivery authorisation` authorises this run, and otherwise asks first, naming the \
         reviewed head {head} and base {base}",
        pr = receipt.pr,
        head = receipt.head,
        base = receipt.base,
    )
}

fn on_receipt(pull: &Value, receipt: &ReviewTransaction) -> bool {
    let current = |key: &str| pull.get(key).and_then(Value::as_str);
    current("headRefOid") == Some(receipt.head.as_str())
        && current("baseRefOid") == Some(receipt.base.as_str())
}

/// The issue's newest publication, only when it is still the live one: confirmed, approved and
/// not escalated, on an open pull request whose head and base are the receipt's. Anything else
/// answers `None`, never a receipt that has moved.
pub(crate) fn current_review_transaction(
    context: &Context,
    issue: u64,
) -> Result<Option<ReviewTransaction>, Failure> {
    let comments = Ledger::read_timeline_at(&store_path()?, &repo_of(context)?, issue)?;
    let Some(latest) = all_publications(&comments)?.pop() else {
        return Ok(None);
    };
    if !latest.confirmed {
        return Ok(None);
    }
    let standing = receipt_standing(&comments, &latest.receipt)?;
    if standing.escalated || standing.standing != Standing::Approved {
        return Ok(None);
    }
    let pull = view_pr(context, latest.receipt.pr)?;
    let open = pull.get("state").and_then(Value::as_str) == Some("OPEN");
    Ok((open && on_receipt(&pull, &latest.receipt)).then_some(latest.receipt))
}

fn republish(issue: u64, receipt: &ReviewTransaction, reason: &str) -> Value {
    execute(
        "publish_review",
        reason,
        json!({"issue": issue, "expected_epoch": receipt.epoch}),
    )
}

fn refused_approval(
    context: &Context,
    issue: u64,
    receipt: &ReviewTransaction,
    comments: &[crate::transport::ownership::Comment],
    standing: &ReceiptStanding,
) -> Result<Option<Value>, Failure> {
    let Err(refusal) = release_basis(context, comments, receipt, None) else {
        return Ok(None);
    };
    let Failure::Stop(found) = refusal else {
        return Err(refusal);
    };
    let reason = found["reason"].as_str().unwrap_or_default();
    if !STALE_VERDICT.contains(&reason) {
        if !UNRECORDED.contains(&reason) {
            return Err(Failure::Stop(found));
        }
        return Ok(Some(unjudged(
            context,
            comments,
            receipt,
            binding(issue, receipt),
        )?));
    }
    if standing.answered {
        return Ok(Some(republish(issue, receipt, "verdict-stale")));
    }
    Ok(Some(halt("verdict-stale", STALE_WITHOUT_CAUSE)))
}

pub fn repo_of(context: &Context) -> Result<Repo, Failure> {
    let named = (context.repo.as_deref().map(RepoRef::parse).transpose())
        .map_err(|_| Failure::Read("the configured repository is not owner/name".to_owned()))?;
    let found = match named {
        Some(named) => Some(named),
        None => {
            let url = run(
                &["git", "remote", "get-url", "origin"],
                Some(&context.repo_dir),
                How::tolerated(),
            )?;
            (url.status == 0)
                .then(|| RepoRef::from_github_remote(&url.stdout))
                .flatten()
        }
    };
    found
        .map(|found| Repo {
            host: "github.com".to_owned(),
            owner: found.owner,
            name: found.name,
        })
        .ok_or_else(|| {
            Failure::Read(
                "the repository could not be named without GitHub: no repository is configured \
                 and `origin` is not a github.com remote"
                    .to_owned(),
            )
        })
}

fn unjudged(
    context: &Context,
    comments: &[crate::transport::ownership::Comment],
    receipt: &ReviewTransaction,
    bound: Value,
) -> Result<Value, Failure> {
    let (_, computed) = computed::computed_for(context, comments, receipt, None)?;
    if !computed.missing_roles.is_empty() {
        let lens = computed
            .missing_roles
            .iter()
            .any(|role| role != VALIDATOR_ROLE);
        let inputs = annotated(
            context,
            issue_of(&bound),
            receipt,
            computed
                .missing_roles
                .iter()
                .map(|role| (role.as_str(), None)),
        )?;
        let reason = if lens {
            "lens-report-missing"
        } else {
            "validator-owed"
        };
        return Ok(collect(reason, bound, inputs));
    }
    if computed.outcome == ComputedOutcome::Escalated
        && refutations_for(comments, receipt).is_empty()
        && computed
            .escalated
            .iter()
            .all(|found| found.reason == NO_REFUTER_RESULT)
    {
        let standing = super::first_severe_findings(comments, receipt);
        let findings = super::inferential_population(&standing);
        let inputs = annotated(
            context,
            issue_of(&bound),
            receipt,
            std::iter::once(("refutation-batch", Some(json!(findings)))),
        )?;
        return Ok(collect("refutation-missing", bound, inputs));
    }
    if computed.outcome == ComputedOutcome::Approved {
        match super::require_cleared_publication_lane(
            context,
            receipt.pr,
            &receipt.head,
            &receipt.base,
            None,
        ) {
            Ok(_) => {}
            Err(Failure::Stop(refusal))
                if refusal
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_some_and(|reason| reason.starts_with("publication-lane-")) =>
            {
                let reason = refusal["reason"].as_str().unwrap_or_default().to_owned();
                let action = refusal["action"].as_str().unwrap_or_default().to_owned();
                return Ok(halt(&reason, &action));
            }
            Err(other) => return Err(other),
        }
    }
    Ok(execute(
        "record_computed_verdict",
        "verdict-computable",
        bound,
    ))
}

fn issue_of(bound: &Value) -> u64 {
    bound["issue"].as_u64().unwrap_or_default()
}

fn annotated<'a>(
    context: &Context,
    issue: u64,
    receipt: &ReviewTransaction,
    inputs: impl Iterator<Item = (&'a str, Option<Value>)>,
) -> Result<Vec<Value>, Failure> {
    let jobs = Ledger::read_jobs_at(&store_path()?, &repo_of(context)?, issue, &receipt.epoch)?;
    let clock = now_seconds().ok_or_else(|| Failure::Read("the clock is unreadable".to_owned()))?;
    inputs
        .map(|(name, findings)| {
            let held = jobs.iter().find(|job| {
                job.name == name && matches!(job.state.as_str(), "running" | "admitting")
            });
            let mut input = match held {
                None => json!({"name": name, "capture": LAUNCH_REVIEW_INPUT}),
                Some(job) if collectable(job, clock)? => {
                    json!({"name": name, "capture": COLLECT_REVIEW_INPUT, "job": job.id})
                }
                Some(job) => json!({
                    "name": name,
                    "capture": null,
                    "running": {"job": job.id, "deadline_at": job.deadline_at},
                }),
            };
            if let Some(findings) = findings {
                input["findings"] = findings;
            }
            Ok(input)
        })
        .collect()
}

fn collectable(job: &Job, clock: u64) -> Result<bool, Failure> {
    Ok(match job_status(Path::new(&job.dir))? {
        JobStatus::Running => clock >= job.deadline_at + GRACE_SECONDS,
        JobStatus::Finished(_) | JobStatus::Lost => true,
    })
}

fn moved_off_receipt(
    context: &Context,
    issue: u64,
    receipt: &ReviewTransaction,
    comments: &[crate::transport::ownership::Comment],
    standing: &ReceiptStanding,
) -> Result<Value, Failure> {
    if standing.standing == Standing::Rejected {
        let publications = all_publications(comments)?;
        if corrections_spent(comments, &publications, receipt.pr)?
            >= corrections_per_lineage(context)
        {
            return Ok(with_why(
                halt(
                    "pull-request-moved",
                    "the pull request's head or base is not the one the newest publication \
                     recorded, and this lineage has spent all its corrections: publishing it again \
                     is refused `correction-already-spent`. Hand the decision to a named person \
                     with `escalate_review`, or publish the remaining work as a new change on a \
                     new pull request",
                ),
                CORRECTION_SPENT_WHY,
            ));
        }
    }
    if standing.standing == Standing::Approved && !standing.answered {
        if let Some(next) = refused_approval(context, issue, receipt, comments, standing)? {
            return Ok(next);
        }
        return Ok(halt("pull-request-moved", FRESH_APPROVED_MOVED));
    }
    Ok(halt("pull-request-moved", DEFAULT_MOVED))
}

fn binding(issue: u64, receipt: &ReviewTransaction) -> Value {
    json!({
        "issue": issue,
        "epoch": receipt.epoch,
        "pr": receipt.pr,
        "head": receipt.head,
        "base": receipt.base,
        "digest": receipt.digest,
    })
}

fn execute(operation: &str, reason: &str, arguments: Value) -> Value {
    json!({
        "kind": "execute",
        "reason_code": reason,
        "operation": operation,
        "arguments": arguments,
    })
}

fn collect(reason: &str, arguments: Value, inputs: Vec<Value>) -> Value {
    let waiting = inputs
        .iter()
        .map(|input| input["running"]["deadline_at"].as_u64())
        .collect::<Option<Vec<u64>>>()
        .and_then(|deadlines| deadlines.into_iter().min());
    let mut next = json!({
        "kind": "collect",
        "reason_code": reason,
        "operation": null,
        "arguments": arguments,
        "inputs": inputs,
    });
    if let Some(deadline) = waiting {
        next["ask_after"] = deadline.into();
    }
    next
}

const CORRECTION_SPENT_WHY: &str = "this lineage spent all its corrections and its last \
    publication still stands rejected, so no further correction is allowed; a person decides \
    whether to accept the change as it stands or have the remaining work published as a new change";

const ESCALATED_WITHOUT_CAUSE: &str = "the review stands escalated and the timeline records no \
    cause for it; a person decides whether to accept the change as it stands or have it replaced \
    by a new change";

fn with_why(mut next: Value, why: &str) -> Value {
    next["why"] = Value::String(why.to_owned());
    next
}

fn halt(reason: &str, continuation: &str) -> Value {
    json!({
        "kind": "stop",
        "reason_code": reason,
        "operation": null,
        "continuation": continuation,
    })
}

fn answer(issue: u64, receipt: Option<&ReviewTransaction>, next: Value) -> Value {
    let horizon = if next["kind"] == "stop" {
        "terminal"
    } else {
        "partial"
    };
    json!({
        "ok": true,
        "issue": issue,
        "receipt": receipt.map(|receipt| json!({
            "epoch": receipt.epoch,
            "pr": receipt.pr,
            "head": receipt.head,
            "base": receipt.base,
            "digest": receipt.digest,
        })),
        "next": next,
        "forecast": {"horizon": horizon},
    })
}
