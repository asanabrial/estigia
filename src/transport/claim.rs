//! The ownership commands: one read, four checks.
//!
//! **A failed check is a stop; a failed read is nothing.** That distinction is
//! the whole point of this file. Incident I07 records a run that lost a claim
//! race by five seconds, was told so thirty-three seconds later, and then worked
//! another forty-eight minutes — because nothing in its heartbeat loop ever read
//! the timeline again.

use std::collections::HashMap;

use super::store::Ledger;
use super::{Context, Failure, ownership};
use crate::harness::population::{Disposition, Population};
use crate::outcome::Refusal;

mod capture;
mod close;
mod computed;
mod recover;
mod status;
mod validator;

pub use capture::opencode_relay_launch;
pub use capture::{
    CaptureInput, collect_review_input, launch_review_input, relay_admit, relay_model, relay_prompt,
};

/// A transport failure as the refusal a caller shows, through the tracker's own
/// translation so the code, class and resolution are the ones every other door
/// carries.
pub(crate) fn failure_refusal(failure: &Failure) -> Refusal {
    capture::refusal_of(failure)
}
pub(crate) use close::verified_delivery;
pub use recover::{RecoverReview, host_repo_of, recover_review};
pub use status::review_status;
pub(crate) use status::{current_review_transaction, repo_of};
pub use validator::{VALIDATOR_ATTEMPTS, ValidatorResultReview, record_validator_result};

pub use computed::{
    COMPUTED_OUTCOMES, ComputedOutcome, ComputedVerdict, ComputedVerdictReview, Escalation,
    LensReportReview, computed_verdict, record_computed_verdict, record_lens_report,
    show_computed_verdict,
};

/// Reads one comment of a tracker payload into the shape the reducer expects.
///
/// Public because it is the *only* place allowed to answer "what did the
/// tracker say about this comment": the two trust flags both default to the
/// untrusted value, and a second decoder that spelled either default the other
/// way would hand the reducer a comment the tracker never sent. The oracle in
/// `tests/differential.rs` read its corpora through here for exactly that
/// reason — it once hand-rolled its own and defaulted `includesCreatedEdit` to
/// `false`, so a corpus omitting the field gave Rust a *trusted* comment and
/// the transport an untrusted one, and the two sides were then compared on
/// inputs that were not the same.
pub fn comment_of(comment: &serde_json::Value) -> ownership::Comment {
    ownership::Comment {
        id: comment
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        created_at: text(comment, "createdAt"),
        body: text(comment, "body"),
        // Both default to the **untrusted** value. A field the tracker did not
        // send is not a field that said yes.
        viewer_did_author: comment
            .get("viewerDidAuthor")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        includes_created_edit: comment
            .get("includesCreatedEdit")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
    }
}

/// The issue's timeline from the local store, after the sync that brings in a control
/// instruction a person or another runtime commented.
pub(crate) fn timeline(
    context: &Context,
    issue: u64,
    now: &str,
) -> Result<Vec<ownership::Comment>, Failure> {
    Ledger::open(context, issue, now)?.timeline()
}

/// Reads the comments of an issue into the shape the reducer expects.
pub fn comments_of(data: &serde_json::Value) -> Vec<ownership::Comment> {
    data.get("comments")
        .and_then(serde_json::Value::as_array)
        .map(|comments| comments.iter().map(comment_of).collect())
        .unwrap_or_default()
}

/// Who currently holds the issue, over an already-read timeline.
///
/// Threads the reducer's own pieces together in the order they depend on each
/// other: the acquisitions, then which takeovers stand, then the releases those
/// takeovers justify, then the fold.
pub fn holding(comments: &[ownership::Comment], now: &str) -> ownership::Holding {
    let events = ownership::ownership_events(comments);
    let valid = ownership::valid_reclaim_positions(&events, comments);
    let legacy = ownership::legacy_release_positions(comments, &events, &valid);
    let epochs = ownership::epoch_release_positions(comments, &events, &valid);
    // Authoritative, not every event. A takeover that did not prove itself is
    // on the issue and decides nothing — `authoritative_events` has said so
    // since it was written, and the fold was handed the unfiltered list
    // anyway. So a `reclaim` naming a `from-op` that is nowhere on the
    // timeline made its author the holder here while the transport reported
    // nobody holding, and a timeline whose target scrolled out of the fetched
    // window looks exactly like one that was invented.
    let authoritative = ownership::authoritative_events(comments);
    ownership::reduce_ownership(&authoritative, &legacy, &epochs, comments, now)
}

/// The control message that stops this run, if the timeline carries one.
///
/// Only what comes **after** the claim counts: a stand-down written before a run
/// acquired is about a previous holder, and reading it would have every resumed
/// issue stop itself on its own history. Only a control-kind marker instructs; a
/// markerless comment is prose and decides nothing.
pub fn control_after(
    comments: &[ownership::Comment],
    run_id: &str,
    watermark: usize,
) -> Option<ownership::Comment> {
    comments
        .iter()
        .enumerate()
        .filter(|(position, _)| *position > watermark)
        .filter(|(_, comment)| comment.viewer_did_author && !comment.includes_created_edit)
        .find(|(_, comment)| {
            super::markers::parse(&comment.body).iter().any(|mark| {
                let kind = mark.get("kind").map(String::as_str).unwrap_or_default();
                !super::markers::RELEASE_KINDS.contains(&kind)
                    && ownership::is_control_for(mark, run_id)
            })
        })
        .map(|(_, comment)| comment.clone())
}

/// The prefix `verify_claim` writes into `checked[0]` when the issue closed
/// behind the review's registered delivery, and the one
/// [`Answer::is_own_delivery_close`](crate::harness::tracker::Answer::is_own_delivery_close)
/// reads back. The delimiter is part of it: the sha and epoch that follow are
/// not, so a reader that matched the bare token would accept a value this never
/// wrote.
///
/// One spelling, shared by the builder and the reader, so neither can drift
/// from the other; [`OWN_DELIVERY_CLOSE_SPELLING_POPULATION`] says what still
/// escapes that and how. It is also the literal
/// `tests/honesty.rs::code_exists_as_a_format_prefix` crosses `skill/`'s
/// mention of the token against, so it stays written out here rather than
/// assembled.
pub const OWN_DELIVERY_CLOSE_PREFIX: &str = "closed-by-own-delivery:";

/// The sites that spell [`OWN_DELIVERY_CLOSE_PREFIX`].
pub const OWN_DELIVERY_CLOSE_SPELLING_POPULATION: Population = Population {
    family: "own-delivery-close-spelling",
    disposition: Disposition::TooTight,
    subject: "OWN_DELIVERY_CLOSE_PREFIX",
    text: "every site that reads or writes the `closed-by-own-delivery` token: the builder at \
           `verify_claim`'s `checked[0]`, and the reader in \
           `harness::tracker::Answer::is_own_delivery_close`. The subject is the one const they \
           share, so editing the token moves this fingerprint and `tests/guards.rs` reopens it. \
           Boundary: a second spelling introduced anywhere else does not move it — that is the \
           escape this population names rather than hides — and the sha and epoch the builder \
           appends after the token are not read here, so a change to them is outside it.",
};

/// `verify-claim` — the renewal: prove current live ownership and state.
///
/// The four checks, in order, because each one makes the next meaningful: the
/// issue is still open, it is in exactly the state expected, this run is the
/// current live holder, and nothing since the claim has told it to stop.
///
/// `allow_closed_by_pr` exists for exactly one moment: the renewal immediately
/// before `close`, after the delivery merge. Under `gh issue develop` the merge
/// auto-closes the issue, so a closed issue *there* is the expected outcome of
/// the delivery rather than evidence somebody else took it. Without it the
/// renewal would hard-stop and the run would skip the `transition --to done`
/// that the auto-close makes mandatory — the rule blocking itself. It is
/// deliberately narrow: the issue must have been closed **by the PR that was
/// merged**, and any other closer is still a stop. Whose run published that PR
/// is not part of it: the live claim is the authority, so a successor that
/// inherited the review renews against the delivery it holds.
pub fn verify_claim(
    context: &Context,
    issue: u64,
    run_id: &str,
    expect_state: &str,
    now: &str,
    allow_closed_by_pr: Option<u64>,
) -> Result<serde_json::Value, Failure> {
    let ledger = Ledger::open(context, issue, now)?;
    verify_held(
        context,
        &ledger,
        issue,
        run_id,
        expect_state,
        now,
        allow_closed_by_pr,
    )
}

/// [`verify_claim`] over a ledger the caller already opened, so a write that
/// follows the renewal records into the same store without asking GitHub twice
/// whose issue it is.
pub(crate) fn verify_held(
    context: &Context,
    ledger: &Ledger,
    issue: u64,
    run_id: &str,
    expect_state: &str,
    now: &str,
    allow_closed_by_pr: Option<u64>,
) -> Result<serde_json::Value, Failure> {
    let comments = ledger.timeline()?;
    let mut own_close = None;
    if !ledger.is_open() {
        let state = "CLOSED";
        own_close = close::delivery_close(context, issue, &comments, None, allow_closed_by_pr)?;
        if own_close.is_none() {
            // A branch-link auto-close can land on a merge the receipt does not
            // match; when the closer is still the review's registered pull
            // request, name the continuation.
            if close::closing_publication(context, issue, &comments)?.is_some()
                && holding(&comments, now).holder.as_deref() == Some(run_id)
            {
                let action = format!(
                    "the branch link closed it when the reviewed pull request merged, and the \
                     work is still yours: `gh issue reopen {issue}`, then `transition` back into \
                     the state the work is in and renew before writing"
                );
                return Err(stop(
                    "issue-not-open",
                    format!(
                        "issue #{issue} is {state} — closed by the review's registered pull request"
                    ),
                    &action,
                ));
            }
            return Err(stop(
                "issue-not-open",
                format!("issue #{issue} is {state} — someone delivered or killed it"),
                "stop; change nothing about the state, it is not yours",
            ));
        }
    }

    let present: Vec<String> = ledger
        .state()?
        .map(|state| format!("status:{state}"))
        .into_iter()
        .collect();
    let wanted = format!("status:{expect_state}");
    if present != [wanted.clone()] {
        return Err(stop(
            "unexpected-state",
            format!("expected exactly [{wanted}], found {present:?}"),
            "stop; the item moved without you — leave the new state alone",
        ));
    }

    let ownership = holding(&comments, now);
    if ownership.holder.as_deref() != Some(run_id) {
        // Two situations shared one sentence, and the advice belonged to the
        // other one. *Somebody else holds it* is a stop: the work is theirs and
        // writing is not this run's to do. *Nobody holds it* is the ordinary end
        // of a claim's life — the horizon passed and the issue is free — and
        // being told to "release only your own projection and write nothing
        // else" names no way forward when there is one.
        //
        // Naming the operation is allowed here because running it discharges
        // the block, which is the crate's one rule about naming one — and it is
        // measured rather than assumed: `may_claim` accepts a run whose *own*
        // claim went stale, asserted by
        // `claim::tests::a_claim_is_won_on_the_timeline_and_not_by_posting`'s
        // neighbour. It does not promise success: claiming is an adjudication,
        // and a run that lost it is told so by `claim` itself.
        //
        // Worded without asserting whose claim lapsed. A caller that never
        // claimed at all reaches this same branch, and telling it that *its*
        // claim expired would be a second false sentence in place of the first.
        return Err(match ownership.holder.as_deref() {
            Some(holder) => stop(
                "not-current-live-holder",
                format!("current live holder is {holder}, not {run_id}"),
                "stop; release only your own projection and write nothing else",
            ),
            None => stop(
                "not-current-live-holder",
                format!("current live holder is none, not {run_id}"),
                "nobody holds it: claim it again before writing — an unheld issue is claimable, \
                 and this run's own lapsed claim does not block that",
            ),
        });
    }
    // A holder always carries the event that established it, but the reducer's
    // two fields are separate values and this must not assume they agree.
    let Some(held) = ownership.event.as_ref() else {
        return Err(stop(
            "not-current-live-holder",
            format!("ownership names {run_id} with no acquisition behind it"),
            "stop; release only your own projection and write nothing else",
        ));
    };

    if control_after(&comments, run_id, held.position).is_some() {
        return Err(stop(
            "control-message",
            format!("a control message after your claim names {run_id}"),
            "stop; acknowledge once, drop your dev:<runtime> label, write nothing else",
        ));
    }

    // Report what was actually checked. Saying "issue-open" about an issue that
    // is closed — even legitimately, by this run's own merge — is the kind of
    // small untruth that later gets quoted as evidence.
    let opened = match &own_close {
        Some(found) => format!(
            "{OWN_DELIVERY_CLOSE_PREFIX}{}:{}",
            found.delivering_sha, found.epoch
        ),
        None => "issue-open".to_owned(),
    };
    let mut answer = serde_json::json!({
        "ok": true,
        "issue": issue,
        "run_id": run_id,
        "state": expect_state,
        "claim_watermark": (!held.created_at.is_empty()).then(|| held.created_at.clone()),
        "checked": [
            opened,
            "single-expected-state".to_owned(),
            "current-live-holder".to_owned(),
            "no-control-message".to_owned(),
        ],
    });
    if let Some(found) = own_close {
        answer["delivering_sha"] = serde_json::json!(found.delivering_sha);
        answer["epoch"] = serde_json::json!(found.epoch);
        answer["pr"] = serde_json::json!(found.pr);
    }
    Ok(answer)
}

fn stop(reason: &str, detail: String, action: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false, "reason": reason, "detail": detail, "action": action,
    }))
}

fn text(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// `heartbeat` — renewal first, progress second. **Never the reverse.**
///
/// A heartbeat that only writes is deaf to the one channel that can revoke the
/// claim, and that is not hypothetical: it is how a stand-down sat unread for
/// forty-eight minutes. So the renewal runs first, and **refuses to record** when
/// it says stop — the write never happens rather than happening and being
/// regretted.
///
/// The activity kind is the caller's, one writer for three words: `heartbeat`,
/// `progress` and `blocked`. A kind outside [`super::markers::RENEWAL_KINDS`] is
/// refused before anything is read or recorded, because `branch` and `published`
/// have writers of their own and a caller asking this one to mint one is asking
/// for a second writer of the same marker.
pub fn heartbeat(
    context: &Context,
    issue: u64,
    run_id: &str,
    expect_state: &str,
    body_file: &std::path::Path,
    kind: &str,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    if !super::markers::RENEWAL_KINDS.contains(&kind) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "renewal-kind-unsupported"
        })));
    }
    let ledger = Ledger::open(context, issue, now)?;
    let verdict = verify_held(context, &ledger, issue, run_id, expect_state, now, None)?;

    let raw = std::fs::read_to_string(body_file).map_err(|_| {
        Failure::Stop(serde_json::json!({ "ok": false, "reason": "heartbeat-body-invalid" }))
    })?;
    // Escaped before the marker is appended, never after: escaping keeps quoted
    // evidence inert, and running it over the marker this call just wrote would
    // neuter its own.
    let marker = super::markers::render(kind, &[("run-id", run_id)]).ok_or_else(|| {
        Failure::Stop(serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }))
    })?;
    let body = format!(
        "{}

{marker}
",
        super::markers::escape_control_input(&raw).trim_end()
    );

    ledger.append(&body, now)?;

    Ok(serde_json::json!({
        "ok": true,
        "issue": issue,
        "renewed": verdict.get("checked"),
        "recorded": true,
    }))
}

/// The PR body, with the safe reference prepended when the file forgot it.
///
/// Written down rather than improvised per pull request, because knowing the
/// rule is demonstrably not enough: a closing keyword typed by hand is how an
/// issue auto-closes without ever passing through `transition`, leaving the
/// label and the board frozen wherever they were.
///
/// `Refs #<n>` is a plain reference and deliberately **not** a closing keyword.
pub fn pr_body_text(body: &str, issue: u64) -> String {
    if body.contains(&format!("#{issue}")) {
        return body.to_owned();
    }
    format!(
        "Refs #{issue} — a plain reference, deliberately NOT a closing keyword.

{body}"
    )
}

/// How many times a publication is re-read before it is disbelieved.
///
/// Named after the transport's own constant, and it was not: this was
/// `READBACK_ATTEMPTS`, which is the transport's name for a **different** number
/// — its `READBACK_ATTEMPTS` is 7 and lives here as [`VISIBILITY_ATTEMPTS`],
/// while the 10 below is its `PUBLISH_READBACK_ATTEMPTS`. Two constants with
/// their names crossed over is a trap for whoever ports the next function that
/// uses one: they reach for the familiar name and get the other bound. Renamed
/// after walking into it.
pub const PUBLISH_READBACK_ATTEMPTS: usize = 10;

/// Base seconds between publication read-back attempts.
///
/// The first attempt reads immediately; before attempt `n` (2 through 10)
/// the loop waits [`PUBLISH_READBACK_RETRY_BASE_SECS`] times `n - 1` seconds
/// — 2, 4, …, 18 — so ten attempts cover about ninety seconds, the order of
/// the two lags measured (45 and 120 seconds) rather than the
/// order of a round trip. The spacing has its ceiling with it: a remote that
/// never settles still refuses after the tenth read instead of hanging.
pub const PUBLISH_READBACK_RETRY_BASE_SECS: u64 = 2;

/// Seconds waited before read-back attempt `upcoming` (1-based).
///
/// Pure so the schedule is pinned without sleeping: the loop sleeps through
/// a seam the tests replace, and this function is what they assert on. Never
/// called with 1 — the first attempt reads immediately.
pub(super) fn readback_delay_secs(upcoming: usize) -> u64 {
    PUBLISH_READBACK_RETRY_BASE_SECS * upcoming.saturating_sub(1) as u64
}

/// Fallback workflow name when the repository's workflow list cannot be read.
///
/// Discovery is the ordinary path: every active `.github/workflows/*` file is a
/// candidate, minus the operator's exclude list. This name is reached only when
/// that listing fails, so a transient API error is not a silent `Absent` and a
/// fixture that never scripts the list still exercises the dispatch path. It is
/// not a claim that every repository's lane is called this — `docs/honesty.md`
/// records what remains unprovable after discovery.
const PUBLICATION_LANE_FALLBACK: &str = "ci.yml";

/// How many check runs one page of the check-run listing may hold.
///
/// The listing is asked for in one page because a **partial** answer here is the
/// failure this crate is named for: thirty green lanes and a red thirty-first
/// read as thirty green ones. `judge_publication_lane` refuses when
/// `total_count` outruns what arrived rather than deciding on the page it got,
/// so a repository with more than this many check runs on one head fails the
/// read instead of being cleared by it.
const LANE_PAGE: usize = 100;

/// One attempt to start a single workflow file.
///
/// Four values rather than a `Result`, because three of them are *not* failures
/// of this operation and one is. A repository with no dispatchable lane and a
/// caller whose token may not dispatch look identical to a `?`, and they need
/// opposite outcomes: the first is the ordinary state of every consumer that has
/// not adopted the lane, and the second is a permission the operator can grant
/// in one command.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DispatchAttempt {
    /// The request was accepted. Check runs appear on the pushed head.
    Started,
    /// There is no dispatchable lane by that name here. Reported, never refused.
    Absent(String),
    /// The token may not start workflows. Refused, before a receipt exists.
    Forbidden(String),
    /// The call did not answer at all. Reported: an unknown dispatch cannot
    /// clear anything, because `record_computed_verdict` re-reads the world.
    Unknown(String),
}

/// What starting the publication lane set answered for one epoch.
///
/// Zero or more workflow files may be candidates; this is the aggregate over
/// every attempt. `Started` always carries at least one basename that actually
/// accepted the dispatch — the receipt and the answer both name that set, so a
/// narrowed gate cannot pass for a wide one in silence.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LaneDispatch {
    /// At least one workflow accepted the dispatch. Names are workflow file
    /// basenames (`ci.yml`), the same spelling `gh workflow run` takes.
    Started(Vec<String>),
    /// No candidate was dispatchable. Reported, never refused.
    Absent(String),
    /// The token may not start workflows. Refused before a receipt exists.
    Forbidden { workflow: String, detail: String },
    /// No candidate answered clearly and none started. Reported.
    Unknown(String),
}

/// Reads one `gh workflow run` outcome without guessing.
///
/// **Pure, so the classification can be measured** — posing a 403 through a
/// scripted `gh` proves the wiring and not the rule, and the rule is where the
/// damage is.
///
/// Only `401` and `403` are read as a permission refusal, and `404` deliberately
/// is not: GitHub answers `404` both for a workflow that is not there and for
/// one the caller may not see, and this operation has already pushed to the
/// repository by the time it asks — so the ambiguous code is sent to the arm
/// that lets a consumer keep working. Refusing on `404` would turn every
/// repository without an Estigia-shaped lane into a repository that cannot
/// publish.
fn classify_lane_dispatch(status: i32, stderr: &str) -> DispatchAttempt {
    if status == 0 {
        return DispatchAttempt::Started;
    }
    let said = stderr.trim().to_owned();
    if said.contains("HTTP 403") || said.contains("HTTP 401") {
        return DispatchAttempt::Forbidden(said);
    }
    // The three ways a repository says *there is no lane here to start*: no
    // workflow of that name, a workflow the listing does not show, and a
    // workflow that exists without the trigger. `gh` prints the first itself and
    // relays the other two from the API.
    if said.contains("could not find any workflows")
        || said.contains("HTTP 404")
        || said.contains("HTTP 422")
    {
        return DispatchAttempt::Absent(said);
    }
    DispatchAttempt::Unknown(said)
}

/// Workflow basenames named in a free-form config cell.
fn lane_name_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            std::path::Path::new(part)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(part)
                .to_owned()
        })
        .collect()
}

/// One operator-table cell by **exact** label, case-insensitive.
///
/// [`Context::get`] is a first-wins **prefix** match. That is wrong for a pair
/// of labels where one is a prefix of the other — `Publication lanes` against
/// `Publication lanes exclude` — because the shorter key then binds the longer
/// row and an exclude-only table collapses discovery to nothing. Exact match is
/// the only shape that lets both rows coexist without ordering tricks.
fn config_exact<'a>(context: &'a Context, label: &str) -> Option<&'a str> {
    context
        .config
        .iter()
        .find(|(key, _, _)| key.eq_ignore_ascii_case(label))
        .map(|(_, value, _)| value.as_str())
}

/// Operator exclude list for publication-lane discovery.
///
/// Read from the `Publication lanes exclude` row when present. Unmodelled by
/// the settings table on purpose: a free-form list of workflow basenames, the
/// same shape as irreversible commands, and a missing row means exclude nothing.
fn publication_lane_excludes(context: &Context) -> Vec<String> {
    config_exact(context, "Publication lanes exclude")
        .map(lane_name_list)
        .unwrap_or_default()
}

/// Whether this basename is on the exclude list.
fn lane_is_excluded(name: &str, exclude: &[String]) -> bool {
    exclude.iter().any(|entry| entry.eq_ignore_ascii_case(name))
}

/// Every workflow file under `.github/workflows/`, with the state GitHub
/// reports for it.
///
/// One reader for one listing: the dispatch candidate list and the
/// disabled-workflow read both come from it, so the `state` spelling cannot
/// drift between them. Dynamic app workflows (paths outside
/// `.github/workflows/`) and nested paths are dropped here so they are never
/// asked.
fn workflow_states(
    context: &Context,
    owner: &str,
    name: &str,
) -> Result<Vec<(String, String)>, String> {
    let path = format!("repos/{owner}/{name}/actions/workflows?per_page=100");
    let data = super::gh_json(&["api", &path], Some(&context.repo_dir))
        .map_err(|failure| failure.detail())?
        .ok_or_else(|| "the workflows listing returned nothing".to_owned())?;
    let Some(workflows) = data.get("workflows").and_then(serde_json::Value::as_array) else {
        return Err("the workflows listing carries no workflows array".to_owned());
    };
    let mut found: Vec<(String, String)> = Vec::new();
    for workflow in workflows {
        let state = workflow
            .get("state")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let path = workflow
            .get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let Some(relative) = path.strip_prefix(".github/workflows/") else {
            continue;
        };
        if relative.is_empty() || relative.contains('/') {
            continue;
        }
        found.push((relative.to_owned(), state.to_owned()));
    }
    found.sort();
    found.dedup();
    Ok(found)
}

/// Active workflow file basenames under `.github/workflows/`.
///
/// Does **not** parse consumer YAML for `workflow_dispatch`: a candidate without
/// the trigger answers `HTTP 422` on dispatch and is classified `Absent` for
/// that file alone. Non-active states are dropped here so they are never asked.
fn list_active_workflow_files(context: &Context) -> Result<Vec<String>, String> {
    let (owner, name) =
        super::closing::repo_identity(context).map_err(|failure| failure.detail())?;
    let mut names: Vec<String> = workflow_states(context, &owner, &name)?
        .into_iter()
        .filter(|(_, state)| state == "active")
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

/// What the Actions surfaces answer about whether CI can run at all.
enum ActionsAvailability {
    /// `actions/permissions` reported `enabled: false`, or every workflow under
    /// `.github/workflows/` is `disabled_manually`. `read` is what answered,
    /// carried verbatim so the run can record it on the issue.
    CannotRun { read: serde_json::Value },
    /// Actions can run, or nothing proves it cannot. A repository with no
    /// workflow under `.github/workflows/` **and Actions enabled** is here
    /// rather than in `CannotRun`: it has no lane, which is a different answer.
    /// With Actions off the permissions read answers `CannotRun` first.
    Available,
}

/// Reads whether CI can run in this repository at all.
///
/// Read only where the check-run listing came back empty: whenever check runs
/// arrived they are the answer about the bytes, and this cannot make a red or
/// running head green. Two surfaces, in order —
/// `repos/<owner>/<name>/actions/permissions` answers `enabled: false` for a
/// repository whose Actions are switched off, and the workflow listing answers
/// `disabled_manually` when every file under `.github/workflows/` carries it.
/// Either read failing is a failed read, never `Available`: an answer that did
/// not arrive is not a repository where Actions is on.
fn actions_availability(
    context: &Context,
    owner: &str,
    name: &str,
) -> Result<ActionsAvailability, Failure> {
    let permissions_path = format!("repos/{owner}/{name}/actions/permissions");
    let permissions = super::gh_json(&["api", &permissions_path], Some(&context.repo_dir))
        .map_err(|failure| actions_unreadable(&permissions_path, &failure.detail()))?
        .ok_or_else(|| actions_unreadable(&permissions_path, "the response was empty"))?;
    let Some(enabled) = permissions
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
    else {
        return Err(actions_unreadable(
            &permissions_path,
            "the response carries no `enabled` boolean",
        ));
    };
    if !enabled {
        return Ok(ActionsAvailability::CannotRun { read: permissions });
    }
    let workflows_path = format!("repos/{owner}/{name}/actions/workflows?per_page=100");
    let states = workflow_states(context, owner, name)
        .map_err(|what| actions_unreadable(&workflows_path, &what))?;
    if states.is_empty() || states.iter().any(|(_, state)| state != "disabled_manually") {
        return Ok(ActionsAvailability::Available);
    }
    let read = serde_json::Value::Array(
        states
            .into_iter()
            .map(|(path, state)| {
                serde_json::json!({ "path": format!(".github/workflows/{path}"), "state": state })
            })
            .collect(),
    );
    Ok(ActionsAvailability::CannotRun { read })
}

/// The one sentence an Actions surface that did not answer produces.
///
/// Distinct from [`lane_unreadable`]: that one is about a head's check runs,
/// this one about whether the repository can run CI at all, and its action
/// names the permission rather than a second attempt, because the two causes
/// it separates answer differently.
fn actions_unreadable(path: &str, what: &str) -> Failure {
    Failure::Read(format!(
        "whether Actions can run for this repository could not be read ({what}); this is a failed \
         read and not an `absent` lane, so nothing about these bytes has been cleared \u{2014} \
         `gh api {path}` says which failure it was: a transient one answers on a second attempt, \
         and a token without `Actions: read` answers the same every time and needs the permission \
         rather than another attempt"
    ))
}

/// The workflow basenames this publication will try to start.
///
/// Resolution order:
///
/// 1. `Publication lanes` set to an explicit comma list — that list, minus the
///    exclude row. Opt-in width for a repository that does not want discovery.
/// 2. `Publication lanes` set to `none` / `off` — empty, so the epoch has no
///    lane and verdicts stay ungated, deliberately.
/// 3. Otherwise discover every active `.github/workflows/*` file, minus the
///    exclude row.
/// 4. When discovery itself fails, fall back to [`PUBLICATION_LANE_FALLBACK`]
///    (still honouring the exclude row) so a list outage is not a silent
///    absence and fixtures that never script the listing still reach dispatch.
fn publication_lane_candidates(context: &Context) -> Vec<String> {
    let exclude = publication_lane_excludes(context);
    // Exact label, never [`Context::get`]: see [`config_exact`].
    if let Some(raw) = config_exact(context, "Publication lanes") {
        let trimmed = raw.trim();
        let lower = trimmed.to_ascii_lowercase();
        if matches!(lower.as_str(), "none" | "off") {
            return Vec::new();
        }
        if !matches!(lower.as_str(), "" | "auto" | "default" | "discover") {
            return lane_name_list(trimmed)
                .into_iter()
                .filter(|name| !lane_is_excluded(name, &exclude))
                .collect();
        }
    }
    match list_active_workflow_files(context) {
        Ok(names) => names
            .into_iter()
            .filter(|name| !lane_is_excluded(name, &exclude))
            .collect(),
        Err(_) => {
            if lane_is_excluded(PUBLICATION_LANE_FALLBACK, &exclude) {
                Vec::new()
            } else {
                vec![PUBLICATION_LANE_FALLBACK.to_owned()]
            }
        }
    }
}

/// Starts one workflow file against a branch that has just been pushed.
fn dispatch_one_lane(context: &Context, workflow: &str, branch: &str) -> DispatchAttempt {
    match super::run(
        &["gh", "workflow", "run", workflow, "--ref", branch],
        Some(&context.repo_dir),
        // A non-zero status is an **answer** here, not a failure: which answer
        // it is decides between refusing this publication and letting a
        // repository without a lane carry on.
        super::How::tolerated(),
    ) {
        Ok(output) => classify_lane_dispatch(output.status, &output.stderr),
        // `run` answers `Read` when the process cannot spawn, whatever `How` it
        // was given. Nothing was learned, and nothing learned is not a
        // repository that has no lane.
        Err(failure) => DispatchAttempt::Unknown(failure.detail()),
    }
}

/// Starts every publication-lane candidate against a branch that has just been
/// pushed.
///
/// Against the **branch**, because `workflow_dispatch` takes a ref and not a
/// commit. The window that opens is real and small: somebody pushing over the
/// branch between this call and the push above starts a lane on their bytes
/// rather than on these. It degrades to *this head has no check runs*, which
/// [`judge_publication_lane`] lets through — the old ordering, not a false
/// clearance — and `docs/honesty.md` says so.
///
/// Once per **candidate** per publication epoch, never per push. A candidate
/// without `workflow_dispatch` answers `Absent` for that file and does not stop
/// the others. Any one `Forbidden` refuses the whole publication before a
/// receipt exists — a token that may not dispatch must not leave an epoch
/// judges will bind to.
fn dispatch_publication_lane(context: &Context, branch: &str) -> LaneDispatch {
    let candidates = publication_lane_candidates(context);
    if candidates.is_empty() {
        return LaneDispatch::Absent(
            "no publication lane candidates after discovery and excludes".to_owned(),
        );
    }
    let mut started = Vec::new();
    let mut last_absent = String::new();
    let mut last_unknown = String::new();
    for workflow in candidates {
        match dispatch_one_lane(context, &workflow, branch) {
            DispatchAttempt::Started => started.push(workflow),
            DispatchAttempt::Forbidden(detail) => {
                return LaneDispatch::Forbidden { workflow, detail };
            }
            DispatchAttempt::Absent(said) => last_absent = said,
            DispatchAttempt::Unknown(said) => last_unknown = said,
        }
    }
    if !started.is_empty() {
        return LaneDispatch::Started(started);
    }
    if !last_unknown.is_empty() {
        return LaneDispatch::Unknown(last_unknown);
    }
    if last_absent.is_empty() {
        LaneDispatch::Absent("no candidate accepted the dispatch".to_owned())
    } else {
        LaneDispatch::Absent(last_absent)
    }
}

/// Workflow basenames rendered for a receipt or answer note.
fn format_lane_workflows(workflows: &[String]) -> String {
    workflows
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The publication lane's answer for one exact head, or a read that did not
/// answer.
///
/// **Pure and fed**, for the same reason [`confirm_published`] is: the thing
/// worth measuring is the decision over four states, not the transport under it.
///
/// The state that decides safety is the first one. **No check runs at all is a
/// pass**, unchanged from before this existed. A repository with no dispatchable
/// lane has no check runs on any head, and refusing there would stop every such
/// repository from recording a verdict at all — breaking the consumers this is
/// supposed to protect. The exposure this change closes is closed only for
/// repositories that answer.
///
/// The other three refuse, and `in_progress` is not `success`: a lane that has
/// not finished has not said the bytes are sound, and banking a verdict against
/// it is exactly the trade this change exists to stop.
///
/// `Red` carries **every** completed, non-green check run on the head, not
/// only the first. A head can carry more than one — `LANE_PAGE`'s own doc
/// comment exists because one head can carry many — and a reader that decided
/// anything from a single one of them, with the rest unexamined, would answer
/// differently depending on the order GitHub happened to list them in. `reds`
/// is never built empty: this variant exists only when that vector has at
/// least one entry. `unfinished` names the first run on the head that has not
/// completed, checked over every run and not only the reds, so a head
/// carrying one never-ran red plus a still-queued check run is never cleared
/// on the reds alone.
enum LaneReading {
    Absent,
    Green,
    Red {
        reds: Vec<RedCheckRun>,
        unfinished: Option<UnfinishedCheckRun>,
    },
    Running {
        name: String,
        status: String,
        run_url: String,
    },
}

/// One completed check run whose conclusion is not green.
#[derive(Debug, Clone)]
struct RedCheckRun {
    name: String,
    conclusion: String,
    run_url: String,
}

/// One check run on the head that has not reached `completed`, found beside
/// a `Red` reading rather than instead of it: a never-ran
/// classification banked while a sibling check run was still `queued` or
/// `in_progress` is exactly the freeze `require_cleared_publication_lane`
/// exists to refuse.
struct UnfinishedCheckRun {
    name: String,
    status: String,
    run_url: String,
}

fn read_publication_lane(data: &serde_json::Value) -> Result<LaneReading, String> {
    let Some(runs) = data.get("check_runs").and_then(serde_json::Value::as_array) else {
        return Err("the response carries no check_runs list".to_owned());
    };
    let Some(total) = data.get("total_count").and_then(serde_json::Value::as_u64) else {
        return Err("the response carries no total_count".to_owned());
    };
    if total as usize != runs.len() {
        return Err(format!(
            "{total} check runs exist and {} arrived, so this is one page of several",
            runs.len()
        ));
    }
    if runs.is_empty() {
        return Ok(LaneReading::Absent);
    }

    let text = |run: &serde_json::Value, key: &str| {
        run.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    // Every completed, non-green run, gathered rather than returned on the
    // first: a head carrying two reds — one that never ran a step and one
    // that genuinely did — must not be judged on whichever one GitHub lists
    // first. `classify_reds` is what tells them apart; this only collects them.
    let reds: Vec<RedCheckRun> = runs
        .iter()
        .filter_map(|run| {
            let (status, conclusion) = (text(run, "status"), text(run, "conclusion"));
            if status != "completed"
                || matches!(conclusion.as_str(), "success" | "neutral" | "skipped")
            {
                return None;
            }
            Some(RedCheckRun {
                name: text(run, "name"),
                conclusion: if conclusion.is_empty() {
                    "none reported".to_owned()
                } else {
                    conclusion
                },
                run_url: text(run, "html_url"),
            })
        })
        .collect();
    // Read over every run on the head, reds included, before `Red` is ever
    // returned: a never-ran classification decided from `reds` alone cannot
    // see a sibling check run still `queued` or `in_progress`.
    let first_unfinished = runs.iter().find_map(|run| {
        let status = text(run, "status");
        if status == "completed" {
            return None;
        }
        Some((text(run, "name"), status, text(run, "html_url")))
    });
    if !reds.is_empty() {
        let unfinished = first_unfinished.map(|(name, status, run_url)| UnfinishedCheckRun {
            name,
            status,
            run_url,
        });
        return Ok(LaneReading::Red { reds, unfinished });
    }
    if let Some((name, status, run_url)) = first_unfinished {
        return Ok(LaneReading::Running {
            name,
            status,
            run_url,
        });
    }
    Ok(LaneReading::Green)
}

/// Where a red run's failed-step log places the failure, against the diff the
/// receipt names.
struct LaneFailureLocation {
    failing_paths: Vec<String>,
    changed_paths: Vec<String>,
    outside_diff: bool,
}

/// The refusal one red check run answers with, whichever of possibly several
/// on the head it is.
///
/// **The one place this JSON is built**, so `judge_publication_lane`'s
/// immediate, unclassified refusal and `require_cleared_publication_lane`'s
/// classified one — which may name a *different* run than the first one
/// GitHub listed, once `classify_reds` has looked at all of them — read as
/// the identical shape either way. The classified one alone may carry
/// `location`: an immediate refusal has read neither the run's log nor the
/// diff, and for a red lane its caller replaces it with the classified one.
fn red_refusal(head: &str, red: &RedCheckRun, location: Option<&LaneFailureLocation>) -> Failure {
    let watched = workflow_run_id(&red.run_url);
    let read_it = match &watched {
        Some(id) => format!("read it with `gh run view {id} --log-failed`"),
        None => format!("read it at {}", red.run_url),
    };
    let placed = match location {
        Some(location) if location.outside_diff => format!(
            " the failing log names {} \u{2014} none of them in this change's diff, so the failure \
             is not a defect in these bytes",
            location.failing_paths.join(", ")
        ),
        Some(location) => format!(
            " the failing log names {} \u{2014} at least one is in this change's diff",
            location.failing_paths.join(", ")
        ),
        None => String::new(),
    };
    let mut body = serde_json::json!({
        "ok": false,
        "reason": "publication-lane-red",
        "head": head,
        "lane": red.name,
        "conclusion": red.conclusion,
        "run_url": red.run_url,
        "action": format!(
            "the publication lane for these exact bytes is red, so a verdict recorded against \
             them would be discarded the moment CI answers \u{2014} {read_it}, fix it, then \
             `republish_review`, which starts a new epoch and a new lane.{placed}"
        ),
    });
    if let Some(location) = location {
        body["failing_paths"] = serde_json::json!(location.failing_paths);
        body["changed_paths"] = serde_json::json!(location.changed_paths);
        body["outside_diff"] = serde_json::json!(location.outside_diff);
    }
    Failure::Stop(body)
}

/// Reads where a red run's failed-step log says it failed, and whether that is
/// inside the diff under review.
///
/// The location comes from the **log**, not from the check run's annotations.
/// GitHub annotates a step failure at the workflow directory — a real failure
/// on this repository's own `ci.yml` answers `path: ".github"`, `message:
/// "Process completed with exit code 101."` — so an annotation names the
/// workflow, never the line a compiler or a test named, and comparing it to a
/// diff of source paths answers "outside" for every failure. That inversion is
/// the one thing this must not do. The failed step's log does carry the
/// location: `--> src/foo.rs:12:5` from a compiler, `panicked at
/// src/foo.rs:12:5` from a test.
///
/// `None` when either half is unknown — the log names no source path, or the
/// diff cannot be read — because an answer computed from half the evidence
/// would read as certainty. The refusal then carries no location, which is
/// exactly what it did before this existed.
fn lane_failure_location(
    context: &Context,
    red: &RedCheckRun,
    base: &str,
    head: &str,
) -> Option<LaneFailureLocation> {
    let failing_paths = failed_log_paths(context, red);
    if failing_paths.is_empty() {
        return None;
    }
    let changed_paths = changed_paths_between(context, base, head)?;
    let outside_diff = failing_paths
        .iter()
        .all(|path| !changed_paths.iter().any(|changed| changed == path));
    Some(LaneFailureLocation {
        failing_paths,
        changed_paths,
        outside_diff,
    })
}

/// Every source path a red run's failed-step log names, deduplicated and in the
/// order the log first names each.
///
/// Best-effort and empty on any failure, because the caller treats an empty
/// list as "nothing was read" rather than as "nothing failed".
fn failed_log_paths(context: &Context, red: &RedCheckRun) -> Vec<String> {
    let Some(id) = workflow_run_id(&red.run_url) else {
        return Vec::new();
    };
    let Ok(out) = super::run(
        &["gh", "run", "view", &id, "--log-failed"],
        Some(&context.repo_dir),
        super::How::read(),
    ) else {
        return Vec::new();
    };
    if out.stdout_replaced {
        return Vec::new();
    }
    source_paths_in(&out.stdout)
}

/// The source paths a log names, in the order they first appear.
///
/// Two shapes carry one: a compiler's `--> path:line[:column]`, and a test
/// runner's `panicked at path:line[:column]`. A token is kept only when its
/// extension is one this repository's failures name, so the log's own prose
/// cannot be read as a file.
fn source_paths_in(log: &str) -> Vec<String> {
    const SOURCE_EXTENSIONS: &[&str] = &["rs", "toml", "md", "yml", "yaml", "js", "ts", "json"];
    let mut paths: Vec<String> = Vec::new();
    for line in log.lines() {
        for anchor in ["--> ", "panicked at "] {
            let Some((_, rest)) = line.split_once(anchor) else {
                continue;
            };
            let Some(token) = rest.split_whitespace().next() else {
                continue;
            };
            let Some(path) = source_path_of(token.trim_end_matches(':')) else {
                continue;
            };
            let named = SOURCE_EXTENSIONS
                .iter()
                .any(|extension| path.ends_with(&format!(".{extension}")));
            if named && !paths.iter().any(|seen| seen == &path) {
                paths.push(path);
            }
        }
    }
    paths
}

/// The path in a `path:line[:column]` token, when the token has that shape.
///
/// The separators are normalised to `/`: a Windows lane's `rustc` and `std`
/// write `src\foo.rs`, while `git diff --name-only` always answers `src/foo.rs`,
/// so a path returned verbatim would never equal a changed one and every Windows
/// failure would read as outside the diff. `parse_finding_location` normalises
/// the same shape for the same comparison.
fn source_path_of(token: &str) -> Option<String> {
    let mut parts = token.rsplitn(3, ':');
    let last = parts.next()?;
    let middle = parts.next()?;
    if !last.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let path = match parts.next() {
        Some(head) if middle.bytes().all(|byte| byte.is_ascii_digit()) => head,
        Some(_) => return None,
        None => middle,
    };
    (!path.is_empty() && !path.chars().any(char::is_whitespace)).then(|| path.replace('\\', "/"))
}

/// The paths a receipt's `base..head` range changes, read in the checkout.
///
/// `None` when the range does not resolve. This is the transport's ordinary
/// position on a read that told us nothing, and it must not fall back to an
/// empty diff: an empty diff would make every failure look outside the change.
fn changed_paths_between(context: &Context, base: &str, head: &str) -> Option<Vec<String>> {
    let range = format!("{base}..{head}");
    let out = super::run(
        &["git", "diff", "--name-only", "-z", &range],
        Some(&context.repo_dir),
        super::How::read(),
    )
    .ok()?;
    if out.stdout_replaced {
        return None;
    }
    Some(
        out.stdout
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// The refusal an unfinished check run answers with, whichever caller reached
/// it — the immediate `Running` reading, or a never-ran classification whose
/// head still carries a sibling run that has not completed.
///
/// **The one place this JSON is built**, so both callers read as the
/// identical shape.
fn unfinished_refusal(pr: u64, head: &str, name: &str, status: &str, run_url: &str) -> Failure {
    let watch = match workflow_run_id(run_url) {
        Some(id) => format!("gh run watch {id} --exit-status"),
        None => format!("gh pr checks {pr} --watch --fail-fast"),
    };
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "publication-lane-unfinished",
        "head": head,
        "lane": name,
        "status": status,
        "run_url": run_url,
        "action": format!(
            "the publication lane for these exact bytes has not answered yet, and an \
             unfinished lane is not a green one \u{2014} run `{watch}`, then record this same \
             verdict again"
        ),
    }))
}

fn judge_publication_lane(
    data: &serde_json::Value,
    pr: u64,
    head: &str,
    path: &str,
) -> Result<(), Failure> {
    match read_publication_lane(data) {
        Err(what) => Err(lane_unreadable(head, path, &what)),
        Ok(LaneReading::Green | LaneReading::Absent) => Ok(()),
        // Reported on the first red in listing order — an immediate signal,
        // not a classification. Whether that is also the run a caller should
        // actually be pointed at, when the head carries more than one, is
        // `require_cleared_publication_lane`'s job to settle before this is
        // ever trusted for a never-ran clearance; see `classify_reds`, which
        // rebuilds this refusal naming whichever run it finds to have run.
        Ok(LaneReading::Red { reds, .. }) => {
            let Some(red) = reds.first() else {
                unreachable!("LaneReading::Red is never built with no runs");
            };
            Err(red_refusal(head, red, None))
        }
        Ok(LaneReading::Running {
            name,
            status,
            run_url,
        }) => Err(unfinished_refusal(pr, head, &name, &status, &run_url)),
    }
}

/// The workflow run id inside a check run's own URL.
///
/// The check-run payload names the check suite and never the workflow run, and
/// the run is what `gh run watch` and `gh run view` take. The URL is
/// `.../actions/runs/<id>/job/<id>`, so the id is read from there or the refusal
/// falls back to a command that needs no id. Returning `None` rather than
/// guessing is the point: a refusal naming `gh run watch` with the wrong number
/// is a dead end, and this repository calls that worse than naming nothing.
fn workflow_run_id(url: &str) -> Option<String> {
    let id = url.split("/actions/runs/").nth(1)?.split('/').next()?;
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_digit())).then(|| id.to_owned())
}

/// What clearing the publication lane rested on.
///
/// `Ordinary` covers green and absent, exactly as before this existed:
/// nothing extra is written about either. `NeverRan` is new, and it is what
/// `record_computed_verdict` writes into the verdict marker on the same
/// footing as a finding's `evidence` — stored, read back, never re-run. A job
/// whose containers never started (a billing failure on the account running
/// Actions, most often) is not evidence of a defect the way a job that ran and
/// failed is, and `gh run view --log-failed` answers nothing for it because
/// there is nothing it ran to log.
enum LaneClearance {
    /// Green, or no check runs at all.
    Ordinary,
    /// Every distinct workflow run behind the head's red check runs reported
    /// no steps on any job, and the caller's own local-verification evidence
    /// stood in for all of them. Verbatim, trimmed, never inspected for
    /// content — the same honest limit `basis` states for itself.
    NeverRan { evidence: String },
    /// `Publication lane: off` — check runs were not read.
    NotConsulted,
}

/// Whether a workflow run's jobs all report `steps: []`.
///
/// **Pure and fed**, so the shape a fixture can pose is a jobs listing, not a
/// process. `None` when the listing itself gives no answer — no `jobs` key, an
/// empty array, or a page that does not carry every job the run has — because
/// a read that told us nothing, or told us only part of the answer, is not
/// evidence either way, and defaulting it to `false` would make an
/// *unreadable or partial* jobs listing look exactly like one this crate
/// measured and found to have run. The caller reading `None` as "did not
/// prove never-ran" is what keeps that case on the fail-closed side, the
/// ordinary red refusal, rather than the permissive one.
///
/// The partial-page check mirrors [`read_publication_lane`]'s own: a run with
/// more jobs than one page holds answered `false` here before this existed,
/// because `.all()` only ever sees the page that arrived — the identical
/// shape of failure `read_publication_lane`'s own doc comment calls "the
/// failure this crate is named for", reached from its sibling rather than
/// closed by it.
///
/// The predicate is structural on purpose — see the decision
/// record: an annotation's wording is prose a third party can reword without
/// notice, so nothing here reads it as anything but evidence to display.
fn jobs_report_never_ran(data: &serde_json::Value) -> Option<bool> {
    let jobs = data.get("jobs")?.as_array()?;
    let total = data
        .get("total_count")
        .and_then(serde_json::Value::as_u64)?;
    if total as usize != jobs.len() {
        return None;
    }
    if jobs.is_empty() {
        return None;
    }
    Some(jobs.iter().all(|job| {
        job.get("steps")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|steps| steps.is_empty())
    }))
}

/// The check run inside a listing whose own `html_url` is `run_url`.
///
/// `read_publication_lane` already found this exact run once and kept only
/// three of its fields; refinding it here by the value it returned is cheaper
/// and more honest than widening what it carries forward for a reader that
/// needs it only when the lane turns out red.
fn find_check_run<'a>(data: &'a serde_json::Value, run_url: &str) -> Option<&'a serde_json::Value> {
    data.get("check_runs")?
        .as_array()?
        .iter()
        .find(|run| run.get("html_url").and_then(serde_json::Value::as_str) == Some(run_url))
}

/// The first annotation message a check run's own annotations listing carries.
///
/// Evidence for the refusal's prose, never the predicate: [`classify_reds`]
/// decides never-ran on `steps: []` alone and reads this only to tell an
/// operator, in GitHub's own words, why nothing ran.
fn first_annotation_message(data: &serde_json::Value) -> Option<String> {
    data.as_array()?.iter().find_map(|annotation| {
        annotation
            .get("message")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
    })
}

/// The refusal a never-ran lane answers with when no evidence was supplied.
///
/// Distinct from `publication-lane-red`, and its action names what was
/// observed rather than what is broken — and names it as an observation:
/// `steps: []` is what GitHub's jobs API reported, not proof the containers
/// never started, which is inference this repository is careful to keep out
/// of a branch and out of its own prose. `runs` names every distinct workflow
/// run behind the head's reds that answered this way, so an operator told to
/// confirm it is handed the runs to look at rather than a bare instruction —
/// the gap a reviewer found in the shape [`red_refusal`] already avoided.
fn never_ran_evidence_required(
    head: &str,
    runs: &[RedCheckRun],
    annotation: &Option<String>,
) -> Failure {
    let named: Vec<String> = runs
        .iter()
        .map(|red| format!("`{}` ({})", red.name, red.run_url))
        .collect();
    let heard = match annotation {
        Some(message) => format!(" GitHub's own annotation says: {message:?}."),
        None => String::new(),
    };
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "publication-lane-never-ran",
        "head": head,
        "runs": runs.iter().map(|red| serde_json::json!({
            "lane": red.name,
            "conclusion": red.conclusion,
            "run_url": red.run_url,
        })).collect::<Vec<_>>(),
        "action": format!(
            "every job on {} reports no steps at all \u{2014} that is what GitHub's jobs listing \
             recorded, and there is no log to read because nothing ran. Commonly a billing or \
             spending-limit failure on the account running Actions kept it from starting.{heard} \
             Estigia cannot verify the cause from here: confirm it yourself \u{2014} the account's \
             Actions billing status, or an equivalent local check \u{2014} then record this same \
             verdict again with --never-ran-evidence-file naming a file that says what you found. \
             That argument changes nothing for a run that genuinely executed a step; it is read \
             only when every job's steps are empty",
            named.join(", "),
        ),
    }))
}

/// Whether a workflow run behind one red check run never executed a single
/// step, decided from its jobs alone — and GitHub's own annotation, when
/// there is one.
///
/// `None` when the jobs read does not prove never-ran — an ordinary red run,
/// or a jobs read that answered nothing or only part of the answer — which is
/// the fail-closed side: a read that told us nothing is not evidence either
/// way, so it is never treated as clearance. `Some(annotation)` when every
/// job's `steps` came back empty; `annotation` is read only now, as a
/// best-effort call that leaves it `None` on its own failure rather than
/// falling back to "not proven" over a run the jobs listing already cleared
/// of having run.
///
/// Called once per **distinct** workflow run by [`classify_reds`], which is
/// the only caller and the only place this or the annotation read happens —
/// a green, absent or running lane never reaches either, and neither does a
/// red run [`classify_reds`] excludes before ever asking (a `cancelled`
/// conclusion).
fn never_ran_signal(
    context: &Context,
    data: &serde_json::Value,
    run_url: &str,
) -> Option<Option<String>> {
    let run_id = workflow_run_id(run_url)?;
    let (owner, repo) = super::closing::repo_identity(context).ok()?;
    let jobs_path = format!("repos/{owner}/{repo}/actions/runs/{run_id}/jobs?per_page={LANE_PAGE}");
    let never_ran = super::gh_json(&["api", &jobs_path], Some(&context.repo_dir))
        .ok()
        .flatten()
        .and_then(|jobs| jobs_report_never_ran(&jobs))
        .unwrap_or(false);
    if !never_ran {
        return None;
    }
    let annotation = find_check_run(data, run_url)
        .and_then(|run| run.get("id").and_then(serde_json::Value::as_u64))
        .and_then(|id| {
            let annotations_path = format!("repos/{owner}/{repo}/check-runs/{id}/annotations");
            super::gh_json(&["api", &annotations_path], Some(&context.repo_dir))
                .ok()
                .flatten()
        })
        .and_then(|list| first_annotation_message(&list));
    Some(annotation)
}

/// What every check run behind a `Red` reading comes to, once each distinct
/// workflow run behind it has been asked.
enum RedClassification {
    /// At least one distinct workflow run genuinely executed a step. Named,
    /// so a refusal always points at the run that ran rather than whichever
    /// one the check-run listing happened to list first.
    Ran(RedCheckRun),
    /// Every distinct workflow run reported no steps on any job. `runs` is
    /// every one of them, in case a caller's evidence has to answer for more
    /// than one.
    NeverRan {
        annotation: Option<String>,
        runs: Vec<RedCheckRun>,
    },
}

/// Classifies every check run a `Red` reading carries, not only the first —
/// **the one place this decision is made**, read by both
/// [`require_cleared_publication_lane`] and the advisory [`publication_lane`]
/// so the two cannot disagree about one head.
///
/// Before this existed, a head carrying two failing check runs — one whose
/// run never executed a step and one that genuinely ran and failed — was
/// accepted or refused depending on which one GitHub happened to list first,
/// because only the first-listed run's jobs were ever read. This asks about
/// every one of them and only answers `NeverRan` when none of them ran.
///
/// **Distinct workflow runs only.** Several check runs can belong to one
/// workflow run — a matrix build posts one check run per job against a
/// single `gh run view` id — so this groups by the id [`workflow_run_id`]
/// extracts and reads each run's jobs once, never once per check run sharing
/// it.
///
/// A `cancelled` conclusion is excluded before any network read, never
/// eligible for never-ran clearance. Cancelling a run before any step starts
/// reports the identical `steps: []` shape a billing failure does, and it is
/// the one way the run being judged could reach for this branch itself
/// rather than a platform failure reaching it for them. Excluding it here
/// closes that path outright rather than merely disclosing it; `docs/honesty.md`
/// says why this is the one conclusion singled out and not a wider guess.
fn classify_reds(
    context: &Context,
    data: &serde_json::Value,
    reds: &[RedCheckRun],
) -> RedClassification {
    debug_assert!(
        !reds.is_empty(),
        "classify_reds is never called with an empty reds slice"
    );
    // Read before the dedupe below collapses `reds` to one entry per run id:
    // a `cancelled` conclusion on any entry excludes its run, whether or not
    // that entry is the one the dedupe would have kept.
    for red in reds {
        if red.conclusion == "cancelled" {
            return RedClassification::Ran(red.clone());
        }
    }
    // One entry per distinct workflow run id — a matrix's several check runs
    // collapse to the one run behind all of them, read once. The first check
    // run naming an id is the one kept for reporting if that run turns out to
    // have executed; which of several sharing an id is arbitrary and none of
    // them decides the branch.
    let mut distinct: std::collections::BTreeMap<String, RedCheckRun> =
        std::collections::BTreeMap::new();
    for red in reds {
        // A red check run whose URL carries no readable run id can never be
        // proven never-ran, and treating an unclassifiable run as satisfied
        // would reopen the ordering bug this function exists to close, under
        // a different name.
        let Some(run_id) = workflow_run_id(&red.run_url) else {
            return RedClassification::Ran(red.clone());
        };
        distinct.entry(run_id).or_insert_with(|| red.clone());
    }
    let mut annotation: Option<String> = None;
    for red in distinct.values() {
        match never_ran_signal(context, data, &red.run_url) {
            Some(found) => {
                if annotation.is_none() {
                    annotation = found;
                }
            }
            None => return RedClassification::Ran(red.clone()),
        }
    }
    RedClassification::NeverRan {
        annotation,
        runs: distinct.into_values().collect(),
    }
}

/// Refuses a verdict whose head the publication lane has not cleared.
///
/// The half that makes this change mechanical rather than a memory rule.
/// Dispatching a lane at publication time and asking somebody to look at it
/// before launching judges is a rule that depends on whoever writes the prompt,
/// which is a failure this repository has concluded against. This is the enforcement
/// point, and there is exactly one: `record_computed_verdict` is the only writer
/// of a verdict marker, and `release_ci` reads what it wrote rather than
/// deciding again.
///
/// The evidence gets **the binding the verdict has** for free. Check runs attach
/// to a SHA by construction, so the receipt's own head is the key and nothing
/// new has to be recorded, compared or invalidated. In particular
/// [`ReviewTransaction`] is untouched: it is compared by equality in
/// [`recorded_receipt`], and a run id added to it would change what a republish
/// invalidates.
///
/// A red lane is not answered here directly any more: every check run it
/// carries is handed to [`classify_reds`], which either finds one that ran —
/// naming it, whether or not that is the run `judge_publication_lane` reported
/// first — or clears the way for the caller's own evidence to stand in for
/// all of them. Green, absent, running and unreadable are answered exactly as
/// they were — the extra reads only ever follow a lane `judge_publication_lane`
/// has already called red.
fn publication_lane_is_off(context: &Context) -> bool {
    // Exact label: `get` is prefix-first and collides with `Publication lanes`
    // / `Publication lanes exclude` (same trap `config_exact` already exists for).
    config_exact(context, "Publication lane")
        .map(str::trim)
        .map(|value| value.eq_ignore_ascii_case("off"))
        .unwrap_or(false)
}

fn require_cleared_publication_lane(
    context: &Context,
    pr: u64,
    head: &str,
    base: &str,
    never_ran_evidence_file: Option<&std::path::Path>,
) -> Result<LaneClearance, Failure> {
    // A configured off gate skips the check-run read entirely.
    // Cancelled conclusions are only classified after a listing is read, so
    // they cannot clear under off either — there is no listing.
    if publication_lane_is_off(context) {
        return Ok(LaneClearance::NotConsulted);
    }
    let (owner, name) = super::closing::repo_identity(context)?;
    let path = format!("repos/{owner}/{name}/commits/{head}/check-runs?per_page={LANE_PAGE}");
    // The transport's own read failure, re-said in this operation's words. Left
    // as it came, it reads `gh api ... failed (1)` and nothing tells the caller
    // that the thing which did not answer was the lane, or that a lane which did
    // not answer clears nothing. Both frames are `Failure::Read`, so the outcome
    // channel is unchanged: this replaces a sentence, not a decision.
    let data = super::gh_json(&["api", &path], Some(&context.repo_dir))
        .map_err(|failure| lane_unreadable(head, &path, &failure.detail()))?
        .ok_or_else(|| lane_unreadable(head, &path, "the response was empty"))?;
    let Err(refusal) = judge_publication_lane(&data, pr, head, &path) else {
        return Ok(LaneClearance::Ordinary);
    };
    let Failure::Stop(envelope) = &refusal else {
        return Err(refusal);
    };
    if envelope.get("reason").and_then(serde_json::Value::as_str) != Some("publication-lane-red") {
        return Err(refusal);
    }
    // The same listing above, read again rather than carried out of
    // `judge_publication_lane`: `data` never left this function, so this costs
    // a match and nothing else. It can only answer `Red` here — `Green` and
    // `Absent` already returned `Ok` above, and a listing `read_publication_lane`
    // cannot parse would already have answered `Err` before `judge_publication_lane`
    // could name a reason at all.
    let Ok(LaneReading::Red { reds, unfinished }) = read_publication_lane(&data) else {
        return Err(refusal);
    };
    match classify_reds(context, &data, &reds) {
        RedClassification::Ran(red) => {
            let location = lane_failure_location(context, &red, base, head);
            Err(red_refusal(head, &red, location.as_ref()))
        }
        RedClassification::NeverRan { annotation, runs } => {
            // Checked before the evidence-file requirement: sending a caller
            // to produce evidence for a lane that has not finished names a
            // clearance nothing can reach yet.
            if let Some(unfinished) = unfinished {
                return Err(unfinished_refusal(
                    pr,
                    head,
                    &unfinished.name,
                    &unfinished.status,
                    &unfinished.run_url,
                ));
            }
            let Some(file) = never_ran_evidence_file else {
                return Err(never_ran_evidence_required(head, &runs, &annotation));
            };
            let raw = std::fs::read_to_string(file).map_err(|error| {
                Failure::Stop(serde_json::json!({
                    "ok": false,
                    "reason": "never-ran-evidence-invalid",
                    "detail": format!("the evidence file could not be read: {error}"),
                }))
            })?;
            let evidence = raw.trim().to_owned();
            if says_nothing(&evidence) {
                return Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false,
                    "reason": "blank-never-ran-evidence",
                    "detail": "a supplied --never-ran-evidence-file must say something: name what \
                               was checked, or state plainly that nothing was verified — an empty \
                               file, whitespace, or filler such as `n/a` says neither. Omitting \
                               the argument entirely is refused instead as \
                               publication-lane-never-ran",
                })));
            }
            Ok(LaneClearance::NeverRan { evidence })
        }
    }
}

/// The publication lane for one head, as an answer a run can read before launching judges.
///
/// Distinguishes green, red, never-ran, running, absent and disabled. The first
/// five are read from the head's check-run listing; `disabled` is the answer
/// when that listing is empty and the repository's own Actions surfaces prove
/// CI cannot run at all. A check-run listing that does not answer, and an
/// Actions surface that does not answer, are failed reads rather than named
/// states. Nothing here refuses a panel launch: a launch is not a write the
/// gate sees.
///
/// Red is classified the same way `require_cleared_publication_lane`
/// classifies it — through the shared `classify_reds` — so this answer and
/// `record_computed_verdict`'s refusal can never disagree about the same head,
/// on a head with one failing check run or several. The extra reads stay off
/// every path that does not need them: only a `Red` reading goes on to fetch
/// jobs, exactly as `require_cleared_publication_lane` does, and only an
/// `Absent` one asks the Actions surfaces.
pub fn publication_lane(
    context: &Context,
    head: &str,
    pr: Option<u64>,
) -> Result<serde_json::Value, Failure> {
    let _ = pr;
    if head.len() != 40
        || !head
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Failure::Read(format!(
            "publication lane head is not a full lowercase object id: {head:?}"
        )));
    }
    if publication_lane_is_off(context) {
        return Ok(serde_json::json!({
            "ok": true,
            "head": head,
            "state": "not-consulted",
            "action": "Publication lane is off for this repository, so check runs were not read; \
                       an accepted verdict is not gated on a lane",
        }));
    }
    let (owner, name) = super::closing::repo_identity(context)?;
    let path = format!("repos/{owner}/{name}/commits/{head}/check-runs?per_page={LANE_PAGE}");
    let data = super::gh_json(&["api", &path], Some(&context.repo_dir))
        .map_err(|failure| lane_unreadable(head, &path, &failure.detail()))?
        .ok_or_else(|| lane_unreadable(head, &path, "the response was empty"))?;
    let reading =
        read_publication_lane(&data).map_err(|what| lane_unreadable(head, &path, &what))?;
    let actions = if matches!(reading, LaneReading::Absent) {
        actions_availability(context, &owner, &name)?
    } else {
        ActionsAvailability::Available
    };
    let classification = if let LaneReading::Red { reds, .. } = &reading {
        Some(classify_reds(context, &data, reds))
    } else {
        None
    };
    publication_lane_answer(head, reading, classification, actions)
}

/// `classification` is computed by the caller — `Some` exactly when `reading`
/// is `LaneReading::Red`, `None` otherwise — so this stays a pure formatter
/// over an already-settled decision rather than one more place that decision
/// could be made differently. `actions` is `CannotRun` exactly when the caller
/// asked the Actions surfaces and they answered that CI cannot run, which it
/// only asks for on an `Absent` reading.
fn publication_lane_answer(
    head: &str,
    reading: LaneReading,
    classification: Option<RedClassification>,
    actions: ActionsAvailability,
) -> Result<serde_json::Value, Failure> {
    let (state, extra) = match reading {
        LaneReading::Absent => match actions {
            ActionsAvailability::CannotRun { read } => (
                "disabled",
                serde_json::json!({
                    "read": read,
                    "action": "Actions cannot run for this repository, so CI is not a delivery \
                               requirement here: the merge rests on the accepted verdict for this \
                               exact receipt and on the local suite over these bytes \u{2014} \
                               record the `read` above verbatim on the issue before merging. A \
                               run that executed and failed, or was cancelled, is still red",
                }),
            ),
            ActionsAvailability::Available => (
                "absent",
                serde_json::json!({
                    "action": "this head has no check runs, so Estigia cannot prove a lane exists; \
                               proceed, as an accepted verdict would",
                }),
            ),
        },
        LaneReading::Green => (
            "green",
            serde_json::json!({
                "action": "launch the panel; an accepted verdict can be recorded against this head",
            }),
        ),
        LaneReading::Red { unfinished, .. } => match (classification, unfinished) {
            // Agrees with the gate: `require_cleared_publication_lane` refuses
            // `publication-lane-unfinished` for this exact shape, before it
            // ever reaches the evidence-file requirement `never-ran` implies.
            (Some(RedClassification::NeverRan { .. }), Some(unfinished)) => (
                "running",
                serde_json::json!({
                    "name": unfinished.name,
                    "status": unfinished.status,
                    "run_url": unfinished.run_url,
                    "action": "wait, or launch knowing the verdict may not be recordable until the \
                               lane finishes. An unfinished lane is not a green one",
                }),
            ),
            (Some(RedClassification::NeverRan { annotation, runs }), None) => (
                "never-ran",
                serde_json::json!({
                    "runs": runs.iter().map(|red| serde_json::json!({
                        "lane": red.name,
                        "run_url": red.run_url,
                    })).collect::<Vec<_>>(),
                    "annotation": annotation,
                    "action": "every job on every failing run reports no steps at all \u{2014} \
                               that is what GitHub recorded; commonly a billing or spending-limit \
                               failure on the account running Actions kept it from starting rather \
                               than a defect. Launch: record_computed_verdict can still approve this \
                               head, gated on --never-ran-evidence-file rather than on green",
                }),
            ),
            (Some(RedClassification::Ran(red)), _) => (
                "red",
                serde_json::json!({
                    "name": red.name,
                    "conclusion": red.conclusion,
                    "run_url": red.run_url,
                    "action": "do not launch: an accepted verdict cannot be recorded against a red \
                               head. Fix it and republish_review. Nothing refuses a panel launched \
                               anyway; a launch is not a write the gate sees",
                }),
            ),
            (None, _) => unreachable!("a Red reading is always classified by its caller"),
        },
        LaneReading::Running {
            name,
            status,
            run_url,
        } => (
            "running",
            serde_json::json!({
                "name": name,
                "status": status,
                "run_url": run_url,
                "action": "wait, or launch knowing the verdict may not be recordable until the lane \
                           finishes. An unfinished lane is not a green one",
            }),
        ),
    };
    let mut answer = serde_json::json!({
        "ok": true,
        "head": head,
        "state": state,
    });
    if let (Some(answer), Some(extra)) = (answer.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            answer.insert(key.clone(), value.clone());
        }
    }
    Ok(answer)
}

/// The one sentence a lane that did not answer produces.
///
/// One place, because the two callers say the same thing about the same failure
/// and a copy is where the softer of the two would eventually be written. The
/// wording is load-bearing: a read that failed and a lane that went red are
/// different answers with opposite consequences, and reporting the first as the
/// second sends somebody to fix a test that never ran.
///
/// It does **not** say "retry", which is what it said for one round. Not every
/// cause of this refusal is transient: a token that can push, comment and
/// dispatch but lacks `Checks: read` fails this read identically every time, and
/// a message telling that caller to try again is a dead end named as a way out —
/// the one thing `CLAUDE.md` says a refusal must never do. So the command names
/// the read that tells the two apart, and the sentence names the fix for the
/// half that repeating cannot reach.
fn lane_unreadable(head: &str, path: &str, what: &str) -> Failure {
    Failure::Read(format!(
        "the publication lane for `{head}` could not be read ({what}); this is a failed read and \
         not a green lane, so nothing about these bytes has been cleared \u{2014} `gh api {path}` \
         says which failure it was: a transient one answers on a second attempt, and a token \
         without `Checks: read` answers the same every time and needs the permission rather than \
         another attempt"
    ))
}

/// Whether the remote settled on the head and base that were just pushed.
///
/// **Pure and fed**: the observations come in as a list rather than being
/// fetched, because the thing worth testing is the *decision* — how many
/// disagreeing reads are tolerated, and what an unreadable one counts as — not
/// the sleeping between them.
///
/// A read that is not an object id is **not** a disagreement: it is a read that
/// did not answer, and it is skipped rather than counted against the remote.
/// Reporting "the remote settled elsewhere" for a response that never arrived
/// would send a run to investigate a push nobody made.
pub fn confirm_published<'a>(
    observations: &'a [serde_json::Value],
    expected_head: &str,
    expected_base: &str,
    expected_draft: bool,
) -> Option<&'a serde_json::Value> {
    observations
        .iter()
        .take(PUBLISH_READBACK_ATTEMPTS)
        .find(|seen| {
            let head = seen
                .get("headRefOid")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let readable = head.len() >= 7 && head.chars().all(|c| c.is_ascii_hexdigit());
            readable
                && head == expected_head
                && seen.get("baseRefOid").and_then(serde_json::Value::as_str) == Some(expected_base)
                && seen.get("isDraft").and_then(serde_json::Value::as_bool) == Some(expected_draft)
        })
}

/// The immutable identity one publication epoch records.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReviewTransaction {
    /// The publication epoch. Every republish creates another.
    pub epoch: String,
    /// The pull request number.
    pub pr: u64,
    /// The published head commit.
    pub head: String,
    /// The base commit GitHub reported.
    pub base: String,
    /// The complete clean target's manifest digest.
    pub digest: String,
}

/// A marker whose own field set is present but does not parse is refused by
/// name rather than read as an absent one — `CLAUDE.md`, "What not to do":
/// reporting the nearest named state instead of the one that was not read back.
/// `field` is the first field that is missing, unparseable or the wrong shape,
/// so the refusal points at the comment a person has to read.
///
/// Two functions rather than one taking the reason: the reason must stay a
/// literal for `build.rs` to see it, and a literal reaching a shared helper as a
/// parameter is invisible to that walk.
fn published_transaction_unparseable(epoch: &str, field: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "published-transaction-unparseable",
        "epoch": epoch,
        "field": field,
        "action": marker_unparseable_action(epoch, field),
    }))
}

fn correction_state_unparseable(epoch: &str, field: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "correction-state-unparseable",
        "epoch": epoch,
        "field": field,
        "action": marker_unparseable_action(epoch, field),
    }))
}

fn marker_unparseable_action(epoch: &str, field: &str) -> String {
    format!(
        "the published marker {epoch:?} carries a {field:?} field that does not read back as the \
         value it recorded, so what it wrote cannot be trusted \u{2014} read that comment and \
         repair or remove it; nothing has been pushed and no pull request was touched"
    )
}

/// Whether `value` is exactly `width` lowercase hexadecimal digits.
fn exact_hex(value: &str, width: usize) -> bool {
    value.len() == width
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

impl ReviewTransaction {
    /// The merge topology that proves a commit delivered this receipt: the
    /// reviewed head is the merge's second parent and the receipt's base is an
    /// ancestor of its first. The one rule the closed-issue renewal and the
    /// harness's post-merge delivery steps both read.
    pub(crate) fn delivered_by_merge(
        &self,
        second_parent: &str,
        base_is_ancestor_of_first_parent: bool,
    ) -> bool {
        second_parent == self.head && base_is_ancestor_of_first_parent
    }

    /// Three outcomes, not two: `Ok(None)` when the marker carries none of the
    /// receipt's own fields, `Ok(Some)` when all are present and well formed,
    /// and `Err` when any is present but does not parse or does not have the
    /// shape [`Self::is_complete`] requires. A `published` marker always carries
    /// all five, so [`all_publications`] refuses the `Err` rather than dropping
    /// the publication from the lineage in silence.
    fn read(marker: &super::markers::Marker) -> Result<Option<Self>, Failure> {
        const FIELDS: [&str; 5] = ["epoch", "pr", "head", "base", "digest"];
        if FIELDS.iter().all(|field| marker.get(*field).is_none()) {
            return Ok(None);
        }
        let epoch = marker.get("epoch").cloned().unwrap_or_default();
        let unparseable = |field: &str| published_transaction_unparseable(&epoch, field);
        let receipt = Self {
            epoch: marker
                .get("epoch")
                .cloned()
                .ok_or_else(|| unparseable("epoch"))?,
            pr: marker
                .get("pr")
                .ok_or_else(|| unparseable("pr"))?
                .parse()
                .map_err(|_| unparseable("pr"))?,
            head: marker
                .get("head")
                .cloned()
                .ok_or_else(|| unparseable("head"))?,
            base: marker
                .get("base")
                .cloned()
                .ok_or_else(|| unparseable("base"))?,
            digest: marker
                .get("digest")
                .cloned()
                .ok_or_else(|| unparseable("digest"))?,
        };
        if !receipt.is_complete() {
            // Name the first field whose shape is wrong, so the refusal points
            // at the part of the comment a person has to read.
            let field = if !ownership::is_operation_id(&receipt.epoch) {
                "epoch"
            } else if receipt.pr == 0 {
                "pr"
            } else if !exact_hex(&receipt.head, 40) {
                "head"
            } else if !exact_hex(&receipt.base, 40) {
                "base"
            } else {
                "digest"
            };
            return Err(unparseable(field));
        }
        Ok(Some(receipt))
    }

    /// The two-outcome reader the receipt-shaped ledgers — findings, handoffs,
    /// verdicts, refutations, dispositions and spend — still use. Their markers
    /// are not `published`, and an unparseable receipt on one of them is out of
    /// this change's scope; [`all_publications`] reads through [`Self::read`]
    /// instead, so a `published` marker is never dropped in silence.
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        Self::read(marker).ok().flatten()
    }

    pub(crate) fn is_complete(&self) -> bool {
        ownership::is_operation_id(&self.epoch)
            && self.pr > 0
            && exact_hex(&self.head, 40)
            && exact_hex(&self.base, 40)
            && exact_hex(&self.digest, 64)
    }
}

/// What a reviewer found, as its own durable record.
///
/// One marker per finding rather than a ledger serialised into the verdict, and
/// the reason is that the transport already knows how to read markers: each one
/// is separately attributable, separately provable-unedited by
/// `first_operation_markers`, and separately addressable by the lineage that has
/// to reference it after a repair. A blob inside one field of the verdict
/// inherits none of that and adds a parser nothing else here needs.
///
/// The verdict then *references* this ledger rather than containing it, which is
/// what makes the rule enforceable at all: a `rejected` outcome can be refused
/// when no `severe` finding stands against the exact receipt it names.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReviewFinding {
    /// The operation that wrote this marker.
    pub operation_id: String,
    /// The run that recorded it.
    pub attester: String,
    /// The context credited with finding it.
    pub reviewer: String,
    /// The publication this finding judges.
    pub receipt: ReviewTransaction,
    /// Stable within a lineage, so a repair can say which finding it answers and
    /// two judges reporting the same defect can be counted as agreeing rather
    /// than as two defects. Chosen by the reviewer from the affected behaviour
    /// and location, never from the run or the wording — the same rule the
    /// analyst contract already states for issue identity.
    pub id: String,
    /// Exactly one of `severe`, `warning` or `suggestion`.
    pub class: String,
    /// What was observed, concretely enough for somebody else to re-run.
    pub evidence: String,
    /// What it costs if left alone.
    pub impact: String,
    /// The parent ledger's finding this one continues, when it continues one.
    ///
    /// Present means *this was already found against the bytes this repair
    /// descends from and is being reassessed*; absent means it is new to this
    /// epoch. Nothing else in the protocol can tell those two apart, and telling
    /// them apart is what stops a re-review rediscovering settled work.
    pub parent: Option<String>,
    /// Why a **new** severe finding is being raised against a repair.
    ///
    /// `introduced` or `exposed`. Required of a severe finding that names no
    /// parent in a publication that has one, and meaningless anywhere else. What
    /// is adjudicated is that the declaration is present and is one of those two
    /// words; whether it is true is a reviewer's claim, like every other field
    /// here.
    pub origin: Option<String>,
    /// How the claim behind this finding says it was established.
    ///
    /// Required of a `severe` finding, whose absence [`record_review_finding`]
    /// refuses: a classification with no stated basis cannot be re-run, and
    /// that is exactly how a preference ends up wearing the word `severe`.
    pub evidence_class: Option<String>,
    /// Who owns this finding's cause on the timeline.
    ///
    /// Required of a `severe` finding alongside [`ReviewFinding::evidence_class`]:
    /// a pre-existing or base-only defect is not answered by this change's
    /// repair no matter how real it is. Both fields are outside the operation
    /// key like `basis` on the verdict, so a retry reads back what stands
    /// rather than minting a second event over different words.
    pub causal_disposition: Option<String>,
    /// Where in the diff this finding lives, when it names one.
    ///
    /// Required of a `severe` finding whose [`Self::causal_disposition`] is
    /// `introduced` or `worsened`, checked against `base..head` whenever it
    /// is given at all, and otherwise the reviewer's own claim like every
    /// other field here. Outside the operation key for the same reason
    /// [`Self::causal_disposition`] is.
    pub location: Option<String>,
    /// The shipped lens this finding came from, when it names one, rather
    /// than a judge. Outside the operation key for the same reason
    /// [`Self::evidence_class`] is: a retry naming the same lens differently
    /// spelled reads back what stands rather than minting a second event.
    pub lens: Option<String>,
}

/// The three classifications a finding may carry.
///
/// `severe` is the only one that can produce a rejection, and the definition is
/// deliberately narrow: a reproducible correctness, security, reliability or
/// contract defect with concrete user or delivery impact. `warning` is a bounded
/// risk with no demonstrated incorrectness. `suggestion` covers optional
/// improvement, cosmetic and style wording, and reviewer preference.
///
/// Anything unclassifiable defaults to `suggestion` by the recorder's own
/// refusal rather than by a silent fallback: an unrecognised word is rejected,
/// because a classification nobody chose is exactly how a preference becomes a
/// blocker.
pub const FINDING_CLASSES: &[&str] = &["severe", "warning", "suggestion"];

/// The origin words a repair finding may state for a defect that is new to it.
///
/// `introduced` means the repair created it; `exposed` means the repair made
/// reachable something that was already wrong. The vocabulary does not cover a
/// severe defect that was always present, always reachable, and simply missed
/// by an earlier round; that gap is in `docs/honesty.md`. Both stated words
/// are the reviewer's claim and neither is checkable here.
pub const FINDING_ORIGINS: &[&str] = &["introduced", "exposed"];

/// The evidence words a severe finding states for how its claim is established.
///
/// `deterministic` means the behaviour was demonstrated; `inferential` means it
/// follows from what was read; `insufficient` states honestly that neither held
/// and leaves the finding a claim like every other field here. A severe finding
/// that says none of them cannot be re-run, which is why [`record_review_finding`]
/// refuses the class rather than trusting the reviewer's contract to carry it
/// alone.
pub const FINDING_EVIDENCE_CLASSES: &[&str] = &["deterministic", "inferential", "insufficient"];

/// The causal words a severe finding states for who owns its defect on the
/// timeline.
///
/// `introduced`, `worsened`, `pre-existing`, `base-only` or `unknown`. A
/// pre-existing or base-only disposition is recorded, not acted on: what
/// blocks this change is a defect *this* change answers, not everything the
/// review happened to read. Nothing here checks the word against the truth,
/// for the same reason nothing checks `origin`.
pub const FINDING_CAUSAL_DISPOSITIONS: &[&str] = &[
    "introduced",
    "worsened",
    "pre-existing",
    "base-only",
    "unknown",
];

/// What `says_nothing` catches, and what it deliberately does not.
pub const SAYS_NOTHING_POPULATION: Population = Population {
    family: "says-nothing",
    disposition: Disposition::TooTight,
    subject: "SAYS_NOTHING_WORDS",
    text: "every supplied proof — a finding's `evidence` or `impact`, a disposition's or an \
           escalation's `reason`, or the `--never-ran-evidence-file` that stands in for a lane that never \
           ran — whose words carry no information: filler standing in for the \
           declaration the field asks for. Legitimate population: every spelling of \"I am \
           not going to say anything here\", including the empty or whitespace-only case \
           this same function checks first, which is part of this subject rather than a \
           separate check beside it. Boundary: the fixed word list below, matched on the \
           trimmed, lowercased whole value. Filler is unbounded — a caller can invent a \
           spelling this list has never seen — so the list is a floor a real caller can show \
           needs widening, not a definitive fix, and it carries no minimum length: a short \
           honest basis such as \"nothing was executed\" is the exact answer the contract \
           asks for and must stay accepted, so refusing on length would catch it along with \
           the filler this population is for. The other direction is real too: `none`, \
           `same` and `unchanged` can be a caller's genuine `impact`, and `nothing` can be a \
           genuine `basis` — both are refused as false positives anyway, because `Disposition` \
           has no variant for both directions at once and the refusal is the design.",
};

pub(crate) const SAYS_NOTHING_WORDS: &[&str] = &[
    "n/a",
    "na",
    "tbd",
    "todo",
    "none",
    "nil",
    "null",
    "pass",
    "ok",
    "-",
    "--",
    ".",
    "placeholder",
    "see above",
    "as above",
    "same",
    "unchanged",
    "nothing",
];

/// Whether `text`, trimmed and lowercased, is a proof that carries no
/// information: empty, whitespace-only, or one of a fixed list of filler that
/// wears the shape of an answer without giving one. See
/// [`SAYS_NOTHING_POPULATION`] for the population this is declared against.
pub(crate) fn says_nothing(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.is_empty() || SAYS_NOTHING_WORDS.contains(&trimmed.to_lowercase().as_str())
}

impl ReviewFinding {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        let operation_id = marker.get("op-id")?.clone();
        let attester = marker.get("run-id")?.clone();
        let reviewer = marker.get("reviewer")?.clone();
        let id = marker.get("id")?.clone();
        let class = marker.get("class")?.clone();
        let evidence = marker.get("evidence")?.clone();
        let impact = marker.get("impact")?.clone();
        if !ownership::is_operation_id(&operation_id)
            || attester.is_empty()
            || reviewer.is_empty()
            || id.is_empty()
            || evidence.is_empty()
            || impact.is_empty()
            || !FINDING_CLASSES.contains(&class.as_str())
        {
            return None;
        }
        Some(Self {
            operation_id,
            attester,
            reviewer,
            receipt: ReviewTransaction::from_marker(marker)?,
            id,
            class,
            evidence,
            impact,
            parent: marker
                .get("parent")
                .filter(|value| !value.is_empty())
                .cloned(),
            origin: marker
                .get("origin")
                .filter(|value| !value.is_empty())
                .cloned(),
            evidence_class: marker
                .get("evidence-class")
                .filter(|value| FINDING_EVIDENCE_CLASSES.contains(&value.as_str()))
                .cloned(),
            causal_disposition: marker
                .get("causal-disposition")
                .filter(|value| FINDING_CAUSAL_DISPOSITIONS.contains(&value.as_str()))
                .cloned(),
            location: marker
                .get("location")
                .filter(|value| !value.is_empty())
                .cloned(),
            lens: marker
                .get("lens")
                .filter(|value| crate::harness::roles::reserved_lens(value))
                .cloned(),
        })
    }

    /// Whether this finding is the kind that can block a delivery.
    pub fn is_severe(&self) -> bool {
        self.class == "severe"
    }
}

/// The three words a refutation batch may record for one candidate.
///
/// `corroborated` and `inconclusive` leave a rejection exactly as permitted as
/// an unrefuted finding does; only `refuted` changes
/// the computed verdict, by dropping the finding from the ledger. A word outside this vocabulary is
/// refused rather than defaulted, the same reason [`FINDING_CLASSES`] is
/// closed.
pub const REFUTATION_OUTCOMES: &[&str] = &["corroborated", "refuted", "inconclusive"];

/// One entry of a refutation batch: the finding it adjudicates and the word it
/// answers.
///
/// Rendered as a JSON array on the `outcomes` marker attribute, which
/// [`markers::render`](super::markers::render) percent-encodes whole —
/// `serde_json` escapes any delimiter a finding's own `id` might carry, so an
/// id containing a comma or a colon cannot be misread as a second entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RefutationOutcome {
    /// The finding this outcome adjudicates — [`ReviewFinding::id`].
    pub id: String,
    /// One of [`REFUTATION_OUTCOMES`].
    pub outcome: String,
}

/// One refutation batch, recorded once against one exact review transaction.
///
/// A refuter is sealed and holds no tools that could write this itself
/// (`docs/honesty.md`), so this is always recorded by the run that holds the
/// live `review` claim, on the refuter's behalf — the same shape a lens
/// finding is recorded in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRefutation {
    /// The operation that wrote this marker.
    pub operation_id: String,
    /// The run that recorded it.
    pub attester: String,
    /// The distinct context credited with the refutation. Never the
    /// publisher, and never a run that requested review of this receipt.
    pub refuter: String,
    /// The publication this batch adjudicates.
    pub receipt: ReviewTransaction,
    /// One entry per blocking candidate the batch was handed.
    pub outcomes: Vec<RefutationOutcome>,
}

impl ReviewRefutation {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        let operation_id = marker.get("op-id")?.clone();
        let attester = marker.get("run-id")?.clone();
        let refuter = marker.get("refuter")?.clone();
        let outcomes: Vec<RefutationOutcome> =
            serde_json::from_str(marker.get("outcomes")?).ok()?;
        if !ownership::is_operation_id(&operation_id) || attester.is_empty() || refuter.is_empty() {
            return None;
        }
        if outcomes.iter().any(|entry| {
            entry.id.trim().is_empty() || !REFUTATION_OUTCOMES.contains(&entry.outcome.as_str())
        }) {
            return None;
        }
        Some(Self {
            operation_id,
            attester,
            refuter,
            receipt: ReviewTransaction::from_marker(marker)?,
            outcomes,
        })
    }
}

/// The three words a disposition batch may record for one finding.
///
/// `acted-on` means the finding was repaired in a later publication, `filed`
/// means it became its own tracker issue — whose number the entry carries —
/// and `declined` means it was judged not worth acting on, and the entry's
/// reason says why. A word outside this vocabulary is refused rather than
/// defaulted, the same reason [`FINDING_CLASSES`] is closed.
pub const DISPOSITION_WORDS: &[&str] = &["acted-on", "filed", "declined"];

/// One entry of a disposition batch: the finding the author answered and what
/// they did about it.
///
/// Rendered as a JSON array on the `dispositions` marker attribute, which
/// [`markers::render`](super::markers::render) percent-encodes whole — the
/// same reason [`RefutationOutcome`] is, so a finding's own `id` cannot be
/// misread as a second entry by carrying the JSON's delimiters.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DispositionEntry {
    /// The finding this entry answers — [`ReviewFinding::id`].
    pub id: String,
    /// One of [`DISPOSITION_WORDS`].
    pub disposition: String,
    /// Why this answer, in the author's own words. Required of every entry:
    /// the whole point of the record is that the decision is legible.
    pub reason: String,
    /// The issue a `filed` finding became, required exactly when the word is
    /// `filed`. A declaration like every other field here — Estigia stores the
    /// number and never confirms the issue exists or carries the finding;
    /// `docs/honesty.md` says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filed_as: Option<u64>,
}

/// One disposition batch, recorded once per receipt and finding population.
///
/// The author's own answer, and deliberately **not** subject to the
/// `require_distinct_reviewer` rule its neighbours carry: a finding's
/// disposition says what the run that answers for the change did about it, so
/// distinctness would make the operation unrecordable by the only context that
/// can record it. The record is auditable rather than independent, and
/// `docs/honesty.md` names exactly that.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReviewDisposition {
    /// The operation that wrote this marker.
    pub operation_id: String,
    /// The run that recorded it, holding the live `review` claim.
    pub attester: String,
    /// The publication this batch answers findings against.
    pub receipt: ReviewTransaction,
    /// One entry per non-blocking finding the batch answers.
    pub dispositions: Vec<DispositionEntry>,
}

impl ReviewDisposition {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        let operation_id = marker.get("op-id")?.clone();
        let attester = marker.get("run-id")?.clone();
        let dispositions: Vec<DispositionEntry> =
            serde_json::from_str(marker.get("dispositions")?).ok()?;
        if !ownership::is_operation_id(&operation_id) || attester.is_empty() {
            return None;
        }
        if dispositions.iter().any(|entry| {
            entry.id.trim().is_empty()
                || !DISPOSITION_WORDS.contains(&entry.disposition.as_str())
                || entry.reason.trim().is_empty()
                || (entry.disposition == "filed") != entry.filed_as.is_some_and(|issue| issue > 0)
        }) {
            return None;
        }
        Some(Self {
            operation_id,
            attester,
            receipt: ReviewTransaction::from_marker(marker)?,
            dispositions,
        })
    }
}

/// The identity a disposition batch's operation key is minted from: the
/// finding ids it answers, sorted and deduplicated.
///
/// The ids rather than the words, and both halves matter. A retry of one
/// batch — the same findings answered again — names the same operation and
/// reads back the first record, so a dropped answer is never a second event.
/// A batch covering a **different** population is a different operation, which
/// is what lets a run answer one finding now and another later without a
/// collision. Nothing owes a disposition, so this is never a completeness
/// check: [`record_disposition`] answers whichever findings it is handed and
/// refuses only the malformed ones.
///
/// A batch that will not parse answers its own text. The transport refuses it
/// unread either way, and the fallback keeps the key deterministic so a retry
/// of the same malformed bytes is the same refusal rather than a new event.
pub fn disposition_answers(batch: &str) -> String {
    let Ok(entries) = serde_json::from_str::<Vec<DispositionEntry>>(batch) else {
        return batch.to_owned();
    };
    let mut ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.join("\n")
}

/// The phases this repository actually runs a context in, and therefore the
/// only ones [`record_spend`] accepts.
///
/// A closed list for the same reason [`FINDING_CLASSES`] is closed: an open
/// vocabulary cannot be totalled by phase, and a phase nobody runs is a slot
/// [`spend_summary`] would report zero for forever. `review` covers every
/// shape `Review delegation` can select — a single independent reviewer, or a
/// two- or five-judge blind panel — because the phase names *who is spending*,
/// not how many contexts that is; `contexts` on the record already carries the
/// headcount. `analyst` covers an audit run the same way, since both read the
/// repository under the same reserved role. `orchestrator` is the coordinating
/// run itself, whose own consumption is never attributable to one publication
/// receipt as cleanly as the other three — `docs/honesty.md` says what that
/// costs.
pub const SPEND_PHASES: &[&str] = &["review", "developer", "analyst", "orchestrator"];

/// One recorded measurement of what a phase cost against one publication
/// receipt.
///
/// `input` and `tokens` are kept apart on purpose and neither is derived from
/// the other: `input` is what the phase was handed — brief plus payload, in
/// bytes — and `tokens` is what it consumed. A number computed by dividing one
/// by the other would compare an input this side can measure directly against
/// a total the compared project excludes "by design", which is exactly the
/// comparison that turned out to be wrong. `conditions` is free text naming
/// what this measurement varied, so a second run under different conditions
/// can be told apart from a repeat of this one rather than averaged into it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpendRecord {
    /// The operation that wrote this marker.
    pub operation_id: String,
    /// The run that recorded it.
    pub attester: String,
    /// The publication this measurement is attributed to.
    pub receipt: ReviewTransaction,
    /// The phase word the marker recorded. [`record_spend`] writes only a word
    /// from [`SPEND_PHASES`], but a marker is a historical fact: one written
    /// before a phase was renamed still reads under the word it carried.
    pub phase: String,
    /// What was varied for this measurement, declared by the caller and never
    /// checked — the axis a later comparison has to hold constant or state it
    /// changed.
    pub conditions: String,
    /// Bytes the phase was handed: brief plus payload.
    pub input: u64,
    /// Tokens the phase consumed.
    pub tokens: u64,
    /// Wall-clock seconds the phase ran.
    pub seconds: u64,
    /// How many agents produced this figure — a two-judge panel is one record
    /// with `contexts: 2`.
    pub contexts: u64,
    /// What produced it, when the caller said.
    pub note: Option<String>,
}

impl SpendRecord {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        let operation_id = marker.get("op-id")?.clone();
        let attester = marker.get("run-id")?.clone();
        let phase = marker.get("phase")?.clone();
        let conditions = marker.get("conditions")?.clone();
        let input: u64 = marker.get("input")?.parse().ok()?;
        let tokens: u64 = marker.get("tokens")?.parse().ok()?;
        let seconds: u64 = marker.get("seconds")?.parse().ok()?;
        let contexts: u64 = marker.get("contexts")?.parse().ok()?;
        if !ownership::is_operation_id(&operation_id)
            || attester.is_empty()
            || conditions.is_empty()
            || phase.is_empty()
            || input == 0
            || tokens == 0
            || seconds == 0
            || contexts == 0
        {
            return None;
        }
        Some(Self {
            operation_id,
            attester,
            receipt: ReviewTransaction::from_marker(marker)?,
            phase,
            conditions,
            input,
            tokens,
            seconds,
            contexts,
            note: marker
                .get("note")
                .filter(|value| !value.is_empty())
                .cloned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LatestPublication {
    publisher: String,
    receipt: ReviewTransaction,
    lineage: Option<PublicationLineage>,
    correction_state: Option<CorrectionState>,
    payload_digest: Option<String>,
    /// False when the push landed and the PR readback never confirmed the head
    /// and base. Still a lease target for [`republish_review`]; never a review
    /// or CI target — see [`recorded_receipt`].
    confirmed: bool,
    /// The pull request's URL, as `published` recorded it. Empty on a marker
    /// written before this field existed — `review_transaction` is the only
    /// reader, and it has nothing else to fall back to.
    url: String,
    /// The declared execution topology and, when `inline` needed one, its
    /// stated reason. Absent on a marker written before this field existed and
    /// on any publication that declared none.
    topology: Option<String>,
    topology_reason: Option<String>,
    /// The raise-only risk declaration, the tier it was computed against and
    /// the focus with its reason, as the marker recorded them. All absent on a
    /// publication that declared none.
    computed_level: Option<String>,
    raise: Option<String>,
    raise_reason: Option<String>,
    focus: Option<String>,
    focus_reason: Option<String>,
    /// What starting the publication lane answered: `started`, `absent`,
    /// `unknown`, in the reviewer-facing prose `published` wrote, and which
    /// workflows started. Absent lane state means the marker predates this
    /// field, which is the same "no evidence recorded" the field's absence
    /// means everywhere else on this struct.
    lane_note: Option<String>,
    lane_workflows: Vec<String>,
    /// The autoclose verdict `published` assessed, recorded whole rather than
    /// field by field — see [`published`]'s `autoclose-json` marker attribute.
    /// A marker written before this field existed, or one whose recorded text
    /// does not parse, answers `will_autoclose: null` with
    /// `autoclose_assessed: false` — unknown, never a false "no".
    autoclose: String,
    /// The mandatory follow-up sentence, present exactly when this
    /// publication's autoclose cause was `branch-link`.
    mandatory_follow_up: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CorrectionState {
    genesis_tree: String,
    /// `\x1f`-separated on the marker.
    genesis_paths: Vec<String>,
    original_changed_lines: u64,
    budget: u64,
    /// The running total every accepted repair on this lineage has added,
    /// including the one that just published.
    cumulative: u64,
}

impl CorrectionState {
    /// Three outcomes, matching [`ReviewTransaction::read`]: `Ok(None)` when the
    /// marker carries none of the budget's own fields — every marker written
    /// before they existed, which [`bootstrap_correction_state`] is the migration
    /// path for — `Ok(Some)` when all are present and parse, and `Err` naming the
    /// epoch and the first field that is present but unparseable. Reading a
    /// half-written budget as an absent one would reset the cumulative to zero in
    /// silence, which is exactly the fail-open this reader exists to stop.
    fn read(marker: &super::markers::Marker) -> Result<Option<Self>, Failure> {
        const FIELDS: [&str; 5] = [
            "budget-genesis-tree",
            "budget-genesis-paths",
            "budget-original-lines",
            "budget",
            "budget-cumulative",
        ];
        if FIELDS.iter().all(|field| marker.get(*field).is_none()) {
            return Ok(None);
        }
        let epoch = marker.get("epoch").cloned().unwrap_or_default();
        let unparseable = |field: &str| correction_state_unparseable(&epoch, field);
        Ok(Some(Self {
            genesis_tree: marker
                .get("budget-genesis-tree")
                .cloned()
                .ok_or_else(|| unparseable("budget-genesis-tree"))?,
            genesis_paths: marker
                .get("budget-genesis-paths")
                .ok_or_else(|| unparseable("budget-genesis-paths"))?
                .split('\u{1f}')
                .map(str::to_owned)
                .collect(),
            original_changed_lines: marker
                .get("budget-original-lines")
                .ok_or_else(|| unparseable("budget-original-lines"))?
                .parse()
                .map_err(|_| unparseable("budget-original-lines"))?,
            budget: marker
                .get("budget")
                .ok_or_else(|| unparseable("budget"))?
                .parse()
                .map_err(|_| unparseable("budget"))?,
            cumulative: marker
                .get("budget-cumulative")
                .ok_or_else(|| unparseable("budget-cumulative"))?
                .parse()
                .map_err(|_| unparseable("budget-cumulative"))?,
        }))
    }
}

const CORRECTION_TEST_DIRECTORIES: &[&str] = &["test", "tests", "__tests__", "spec", "specs"];

fn correction_path_dir(path: &str) -> &str {
    path.rfind('/').map_or(".", |index| &path[..index])
}

fn correction_path_base(path: &str) -> &str {
    path.rfind('/').map_or(path, |index| &path[index + 1..])
}

/// `_test.rs` mirrors Rust's own build-excluded `_test.go`-style convention;
/// the `tests/` clause additionally admits top-level integration binaries
/// such as `tests/pipe.rs`, which carry no such suffix.
fn correction_test_file_name(path: &str) -> bool {
    let Some(stem) = correction_path_base(path).strip_suffix(".rs") else {
        return false;
    };
    (stem.len() > "_test".len() && stem.ends_with("_test"))
        || path.split('/').any(|segment| segment == "tests")
}

fn correction_companion_directory(added_dir: &str, genesis_dir: &str) -> bool {
    if added_dir == genesis_dir {
        return true;
    }
    if !CORRECTION_TEST_DIRECTORIES.contains(
        &correction_path_base(added_dir)
            .to_ascii_lowercase()
            .as_str(),
    ) {
        return false;
    }
    let parent = correction_path_dir(added_dir);
    parent == genesis_dir || parent == correction_path_dir(genesis_dir)
}

fn correction_added_path_admissible(added: &str, genesis: &[String]) -> bool {
    if !correction_test_file_name(added) {
        return false;
    }
    let directory = correction_path_dir(added);
    genesis
        .iter()
        .any(|reviewed| correction_companion_directory(directory, correction_path_dir(reviewed)))
}

fn admit_correction_scope<'a>(paths: &'a [String], genesis: &[String]) -> Result<(), &'a str> {
    for path in paths {
        if genesis.iter().any(|reviewed| reviewed == path) {
            continue;
        }
        if !correction_added_path_admissible(path, genesis) {
            return Err(path);
        }
    }
    Ok(())
}

fn bootstrap_correction_state(
    context: &Context,
    at: &std::path::Path,
    base: &str,
    head: &str,
) -> Result<CorrectionState, Failure> {
    let fork_point = super::target::merge_base(at, base, head)?;
    let genesis_tree = super::tier::tree_of(context, Some(at), &fork_point)?;
    let candidate_tree = super::tier::tree_of(context, Some(at), head)?;
    let genesis_paths =
        super::tier::tree_pair_changed_paths(context, Some(at), &genesis_tree, &candidate_tree)?;
    let generated = super::generated::manifested_paths(at, base, &fork_point, head)?;
    let original_changed_lines = super::tier::tree_pair_changed_lines(
        context,
        Some(at),
        &genesis_tree,
        &candidate_tree,
        &generated,
    )?;
    Ok(CorrectionState {
        budget: super::tier::floor_two(original_changed_lines),
        genesis_tree,
        genesis_paths,
        original_changed_lines,
        cumulative: 0,
    })
}

struct CorrectionCheck {
    state: Option<CorrectionState>,
    refrozen: bool,
    integrated_parent: bool,
}

/// The tree a round is measured from when its base advanced past the parent's
/// and was integrated: what git itself would merge `parent_head` and `new_base`
/// into, and the paths that merge conflicted on. A conflicted merge still
/// writes a tree, with the markers in the files, so the author's resolution is
/// measured as authored lines. Run in the same isolated git directory and
/// replaced environment as the budget's diffs, with the conflict style pinned,
/// so a repository's own merge configuration, attributes or drivers cannot
/// change what is charged; the tree is written to the real object database,
/// where those diffs read it.
fn integrated_parent_tree(
    at: &std::path::Path,
    parent_head: &str,
    new_base: &str,
) -> Result<(String, Vec<String>), Failure> {
    let (_guard, envs) = super::isolated_diff_env(at)?;
    let envs: Vec<(&str, &str)> = envs
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let output = super::run_with_isolated_env(
        &[
            "git",
            "-c",
            "merge.conflictStyle=merge",
            "-c",
            "core.quotePath=false",
            "merge-tree",
            "--write-tree",
            "--no-messages",
            parent_head,
            new_base,
        ],
        Some(at),
        super::How::tolerated(),
        &envs,
    )?;
    let tree = validated_merge_tree(output.status, &output.stdout, &output.stderr)?;
    let conflicted = merge_tree_conflicted_paths(&output.stdout)?;
    Ok((tree, conflicted))
}

/// Whether the base this publication carries was merged into the head and git merges the parent
/// head and that base without a conflict, so the correction's own diff can be read from the
/// merged tree. Any question git cannot answer reads as not integrated, which escalates.
fn base_integrated_cleanly(
    at: &std::path::Path,
    parent: &ReviewTransaction,
    base: &str,
    head: &str,
) -> bool {
    base_advanced_past(at, &parent.base, base).unwrap_or(false)
        && super::target::is_ancestor(at, base, head).unwrap_or(false)
        && integrated_parent_tree(at, &parent.head, base)
            .is_ok_and(|(_, conflicted)| conflicted.is_empty())
}

/// The object-id half of [`integrated_parent_tree`]'s validation, pure over
/// the three fields of the `git merge-tree` run so it can be driven directly.
fn validated_merge_tree(status: i32, stdout: &str, stderr: &str) -> Result<String, Failure> {
    let tree = stdout.lines().next().unwrap_or_default().trim();
    if !matches!(status, 0 | 1) || !super::target::is_object_id(tree) {
        return Err(Failure::Read(format!(
            "the base advanced past the parent publication's and was integrated, so this round \
             can only be measured against the tree git merges the parent head and the new base \
             into, and `git merge-tree --write-tree` (git 2.38 or newer) did not produce one \
             (status {status}): {detail}. Nothing has been pushed and the correction budget was \
             not reset",
            detail = stderr.trim(),
        )));
    }
    Ok(tree.to_owned())
}

/// The conflicted paths [`integrated_parent_tree`]'s output lists, one line
/// per path per stage after the tree line, formatted `<mode> <object>
/// <stage>\t<path>`. `--no-messages` suppresses the informational lines, not
/// these.
fn merge_tree_conflicted_paths(stdout: &str) -> Result<Vec<String>, Failure> {
    let mut lines = stdout.lines();
    lines.next();
    let mut conflicted = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((_, path)) = line.split_once('\t') else {
            return Err(Failure::Read(format!(
                "unexpected merge-tree conflict record {line:?}"
            )));
        };
        conflicted.push(super::tier::normalize_logical_path(path).map_err(Failure::Read)?);
    }
    conflicted.sort();
    conflicted.dedup();
    Ok(conflicted)
}

/// Percent-encoded byte ceiling on `budget-genesis-paths`, the marker field
/// built from the original candidate's own manifest rather than from anything
/// the caller passed. Overflow is a refusal, never a
/// truncation — the same shape [`super::target::REVIEW_PAYLOAD_CEILING`] and
/// [`FINDING_TEXT_CEILING`] already hold, applied here because a manifest
/// too large to record must be caught before anything remote moves, not
/// discovered after the branch is pushed and the pull request opened.
pub const GENESIS_MANIFEST_CEILING: usize = 32 * 1024;

/// The length [`GENESIS_MANIFEST_CEILING`] bounds: the `\u{1f}`-joined
/// `budget-genesis-paths` value exactly as [`super::markers::render`] would
/// percent-encode it, not the raw joined byte count — every separator, and
/// every other byte outside the marker's safe set, costs three characters
/// once encoded rather than one.
fn genesis_manifest_encoded_len(genesis_paths: &[String]) -> usize {
    super::markers::encoded_len(&genesis_paths.join("\u{1f}"))
}

/// `parent`'s own frozen tree pair is recomputed here rather than read off
/// its marker, so a marker written before this field existed still works.
/// Only [`CorrectionState`] is read back rather than recomputed.
///
/// A base that advanced past the parent's and was integrated resets nothing:
/// the round is measured from [`integrated_parent_tree`], and the paths that
/// merge conflicted on and the advance's rename destinations are admitted
/// beside the genesis paths. Only a fork point outside the parent's history
/// with no such advance starts a fresh genesis.
///
/// **Pull-request-scoped, not issue-scoped — carried by `parent` itself.**
/// The caller selects the latest publication on this pull request, never the
/// issue's; docs/honesty.md has why the distinction is load-bearing.
fn correction_budget_check(
    context: &Context,
    at: &std::path::Path,
    parent: Option<&LatestPublication>,
    this_base: &str,
    this_head: &str,
    this_candidate_tree: &str,
) -> Result<CorrectionCheck, Failure> {
    let Some(parent) = parent else {
        return Ok(CorrectionCheck {
            state: None,
            refrozen: false,
            integrated_parent: false,
        });
    };

    let fork_point = super::target::merge_base(at, this_base, this_head)?;
    let integrated = !super::target::is_ancestor(at, &fork_point, &parent.receipt.head)?;
    let advanced = base_advanced_past(at, &parent.receipt.base, this_base)?;
    if integrated && !advanced {
        return Ok(CorrectionCheck {
            state: Some(bootstrap_correction_state(
                context, at, this_base, this_head,
            )?),
            refrozen: true,
            integrated_parent: false,
        });
    }
    let integrated_parent = integrated && advanced;

    let CorrectionState {
        genesis_tree,
        genesis_paths,
        original_changed_lines,
        budget,
        cumulative: cumulative_before,
    } = match &parent.correction_state {
        Some(state) => state.clone(),
        None => {
            // The parent is the original, unbudgeted publication, so genesis
            // bootstraps from *its* ends, not this repair's.
            let mut bootstrap = bootstrap_correction_state(
                context,
                at,
                &parent.receipt.base,
                &parent.receipt.head,
            )?;
            bootstrap.cumulative = 0;
            bootstrap
        }
    };

    let (parent_candidate_tree, integrated_paths) = if integrated_parent {
        let rename_destinations = super::tier::tree_pair_rename_destinations(
            context,
            Some(at),
            &super::tier::tree_of(context, Some(at), &parent.receipt.base)?,
            &super::tier::tree_of(context, Some(at), this_base)?,
        )?;
        let (tree, conflicted) = integrated_parent_tree(at, &parent.receipt.head, this_base)?;
        let mut integrated_paths = conflicted;
        integrated_paths.extend(rename_destinations);
        integrated_paths.sort();
        integrated_paths.dedup();
        (tree, integrated_paths)
    } else {
        (
            super::tier::tree_of(context, Some(at), &parent.receipt.head)?,
            Vec::new(),
        )
    };
    let admitted_paths: Vec<String> = genesis_paths
        .iter()
        .chain(&integrated_paths)
        .cloned()
        .collect();
    let changed_paths = super::tier::tree_pair_changed_paths(
        context,
        Some(at),
        &parent_candidate_tree,
        this_candidate_tree,
    )?;
    if let Err(offending) = admit_correction_scope(&changed_paths, &admitted_paths) {
        let action = if integrated_parent {
            format!(
                "{offending:?} changed outside the paths this integrated round admits and is not \
                 an admissible companion test \u{2014} this round measures from the tree git \
                 merges the parent head and the new base into and admits, beside the reviewed \
                 candidate's own paths, the paths that merge conflicted on and the advance's \
                 rename destinations. Nothing has been pushed and no pull request was touched. \
                 Keep the correction inside the admitted paths, or add only a companion test \
                 alongside one of them (a `_test.rs` file, or anything under a `tests/`-shaped \
                 directory next to it); anything else is a new change, not a correction, and \
                 belongs on a new pull request, which starts its own lineage and its own budget"
            )
        } else {
            format!(
                "{offending:?} changed outside the reviewed candidate's own paths and is not an \
                 admissible companion test \u{2014} nothing has been pushed and no pull request \
                 was touched. Keep the correction inside the paths that were reviewed, or add \
                 only a companion test alongside one of them (a `_test.rs` file, or anything \
                 under a `tests/`-shaped directory next to it); anything else is a new change, \
                 not a correction, and belongs on a new pull request, which starts its own \
                 lineage and its own budget"
            )
        };
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "correction-outside-reviewed-scope",
            "path": offending,
            "genesis_paths": genesis_paths,
            "integrated_paths": integrated_paths,
            "action": action,
        })));
    }

    let generated =
        super::generated::manifested_paths(at, this_base, &parent_candidate_tree, this_head)?;
    let actual = super::tier::tree_pair_changed_lines(
        context,
        Some(at),
        &parent_candidate_tree,
        this_candidate_tree,
        &generated,
    )?;
    let cumulative_after = cumulative_before + actual;
    if cumulative_after > budget {
        let measured = if integrated_parent {
            "measured tree-to-tree against the tree git merges the parent head and the new base into"
        } else {
            "measured tree-to-tree against the parent publication's own candidate"
        };
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "correction-budget-exceeded",
            "original_changed_lines": original_changed_lines,
            "budget": budget,
            "correction_changed_lines": actual,
            "cumulative_correction_lines": cumulative_after,
            "action": format!(
                "this repair's own correction ({actual} changed lines, {measured}) brings the \
                 cumulative total to {cumulative_after} against a budget of {budget} frozen from \
                 the original candidate's own {original_changed_lines} changed lines \u{2014} \
                 nothing has been pushed and no pull request was touched. The remaining work is \
                 larger than this candidate's repair allowance; publish it as a new change on a \
                 new pull request, which starts its own lineage and its own budget"
            ),
        })));
    }

    Ok(CorrectionCheck {
        state: Some(CorrectionState {
            genesis_tree,
            genesis_paths,
            original_changed_lines,
            budget,
            cumulative: cumulative_after,
        }),
        refrozen: false,
        integrated_parent,
    })
}

/// What a repair publication descends from.
///
/// Derived from the timeline at publish time and never passed in, so a caller
/// cannot name a parent it prefers and a repair cannot be recorded as a first
/// publication. Absent on a first publication, which is the only shape that has
/// nothing to descend from.
///
/// **The whole parent receipt, not its epoch.** The first version of this
/// carried the epoch and the head, and the reader that used it matched findings
/// on the epoch alone — which is a looser identity than it looks, because
/// `publication_epoch` mixes a clock into its hash and a finding's epoch field
/// is whatever the finding says it is. Two reviewers demonstrated the cost: a
/// `review-finding` backfilled after the repair, carrying the parent epoch and
/// the repair's own bytes, satisfied the parent-existence rule and let a new
/// severe finding through with no origin. Carrying the receipt lets the parent
/// ledger be read by the same receipt-exact reader every other ledger here uses, and deletes the looser
/// reader rather than guarding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationLineage {
    /// The complete receipt of the publication this one repairs.
    pub parent: ReviewTransaction,
    /// One immutable name for this exact repair. See [`fix_delta_digest`].
    pub delta: String,
    /// `REPAIR_RANGE_LABEL` when the transport's own `repair_range` decision
    /// answered that the narrow range is honest: `parent_head` still an
    /// ancestor of `head`, and that range touching at least one path.
    /// Ancestry alone is not the criterion — an identical-bytes republish
    /// satisfies it and still has no repair-only range to read.
    /// `None` when it is not, in which case [`Self::delta_range_reason`]
    /// carries why. Absent on any marker written before this field existed,
    /// and on a first publication.
    pub delta_range: Option<String>,
    /// The transport's own `WholeChangeReason` spelling when
    /// [`Self::delta_range`] is absent because the narrow range was not
    /// honest — never a spelling of the same state coined separately here.
    /// `None` when [`Self::delta_range`] is present, and on any marker
    /// written before this field existed.
    pub delta_range_reason: Option<String>,
    /// Whether the base this publication carries was integrated cleanly: the base advanced past
    /// the parent's, the head descends from it, and `git merge-tree` merges the parent head and
    /// that base without a conflict. Then the correction's own diff is the one from the merged
    /// tree to the head, which the validator reads where [`Self::delta_range_reason`] says the
    /// narrow range carries upstream work. Recorded as `delta-parent=integrated`.
    pub integrated_parent: bool,
    /// Whether this publication is the root of a successor lineage: it descends
    /// from a receipt a maintainer recovered, and is a fresh candidate rather
    /// than a correction of it.
    pub recovered: bool,
}

/// The value [`PublicationLineage::delta_range`] carries when the transport's
/// own `repair_range` decision answers that the narrow range is honest — the
/// same range every shipped surface names, spelled once so recording it and
/// stating it in prose can never drift into two different strings.
const REPAIR_RANGE_LABEL: &str = "parent-head..head";

impl PublicationLineage {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        // All of it or none. A marker carrying part of a lineage is a marker
        // whose lineage cannot be acted on, and treating it as a partial answer
        // would let a repair look like a first publication in one reader and a
        // repair in the next.
        let parent = ReviewTransaction {
            epoch: marker.get("parent")?.clone(),
            pr: marker.get("parent-pr")?.parse().ok()?,
            head: marker.get("parent-head")?.clone(),
            base: marker.get("parent-base")?.clone(),
            digest: marker.get("parent-digest")?.clone(),
        };
        let delta = marker.get("delta")?.clone();
        // Tolerated absent, unlike every field above: every marker written
        // before this pair existed carries neither, and must keep parsing as
        // the lineage it always was rather than losing it to a shape check
        // written after the fact.
        let delta_range = marker.get("delta-range").cloned();
        let delta_range_reason = marker.get("delta-range-reason").cloned();
        let integrated_parent =
            marker.get("delta-parent").map(String::as_str) == Some("integrated");
        // The same completeness the live receipt is held to. A parent of the
        // wrong shape is one no finding could ever match, so admitting it would
        // reopen by the back door what the shape check closes at the front.
        (parent.is_complete() && !delta.is_empty()).then_some(Self {
            parent,
            delta,
            delta_range,
            delta_range_reason,
            integrated_parent,
            recovered: marker.get("recovered-from").is_some(),
        })
    }
}

/// One immutable name for the transition between two publications.
///
/// The parent whole, and the child by the two fields a repair moves. The parent
/// contributes its epoch, which `publication_epoch` mints from that receipt
/// entire; the child contributes its head and its target digest, not its base,
/// pull request or epoch. A reviewer measured that the omitted fields are not
/// independently reachable — a judge drove two different bases through the
/// built binary to a byte-identical digest, because `manifest_digest` hashes
/// the tree at `HEAD` and never the base — and nothing reads
/// this value for a decision, so the bound is legibility rather than a gate.
/// It names *which*
/// repair this is; it is not an enumeration of the changed paths and nothing
/// here claims it is. The delta a reviewer reads is `parent_head..head` when
/// the transport's own `repair_range` decision answers that it is honest —
/// [`PublicationLineage`] records which, in [`PublicationLineage::delta_range`] and
/// [`PublicationLineage::delta_range_reason`] — and this digest records the ends
/// regardless of which case applied. `docs/honesty.md` carries what that does and
/// does not buy.
pub fn fix_delta_digest(parent: &ReviewTransaction, head: &str, digest: &str) -> String {
    super::ownership::sha256_hex(
        [
            parent.epoch.as_str(),
            parent.head.as_str(),
            parent.digest.as_str(),
            head,
            digest,
        ]
        .join(
            "
",
        )
        .as_bytes(),
    )
}

/// Every `published` marker on the timeline, in the order it was written.
///
/// The same filter chain [`latest_publication`] uses, kept once: a publication
/// is not an operation-id-bearing kind (a republish is not a retry of one, it
/// is a new epoch), so there is nothing to dedupe here — every trusted marker
/// counts.
///
/// Fallible because a `published` marker whose receipt or budget fields are
/// present but malformed is refused by name here rather than dropped: a corrupt
/// publication and no publication must not be the same input to `latest`,
/// `repair_rounds` or a budget. A marker carrying none of those fields is a
/// pre-schema marker and is still admitted as the lineage it always was.
fn all_publications(comments: &[ownership::Comment]) -> Result<Vec<LatestPublication>, Failure> {
    let mut publications = Vec::new();
    for comment in comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
    {
        for marker in super::markers::parse(&comment.body) {
            if marker.get("kind").map(String::as_str) != Some("published") {
                continue;
            }
            let Some(publisher) = marker.get("run-id").filter(|run| !run.is_empty()).cloned()
            else {
                continue;
            };
            let Some(receipt) = ReviewTransaction::read(&marker)? else {
                return Err(published_transaction_unparseable(
                    &marker.get("epoch").cloned().unwrap_or_default(),
                    "epoch",
                ));
            };
            publications.push(LatestPublication {
                publisher,
                receipt,
                lineage: PublicationLineage::from_marker(&marker),
                correction_state: CorrectionState::read(&marker)?,
                // Tolerated absent, like `delta-range` above: a marker written
                // before this field existed carries none, and one that does not
                // carry it simply cannot be crossed against a candidate text.
                payload_digest: marker.get("payload-digest").cloned(),
                // Absent means confirmed: every marker written before this field
                // existed was posted only after readback agreed. Only an explicit
                // `unconfirmed=true` is a lease-only receipt.
                confirmed: marker.get("unconfirmed").map(String::as_str) != Some("true"),
                url: marker.get("url").cloned().unwrap_or_default(),
                topology: marker.get("topology").cloned(),
                topology_reason: marker.get("topology-reason").cloned(),
                computed_level: marker.get("computed-level").cloned(),
                raise: marker.get("raise").cloned(),
                raise_reason: marker.get("raise-reason").cloned(),
                focus: marker.get("focus").cloned(),
                focus_reason: marker.get("focus-reason").cloned(),
                lane_note: marker.get("lane-note").cloned(),
                lane_workflows: marker
                    .get("lanes")
                    .map(|joined| joined.split(',').map(str::to_owned).collect())
                    .unwrap_or_default(),
                autoclose: marker.get("autoclose-json").cloned().unwrap_or_default(),
                mandatory_follow_up: marker.get("mandatory-follow-up").cloned(),
            });
        }
    }
    Ok(publications)
}

/// The newest publication on `pr`, when there is one: what a publication on
/// that pull request records as its parent, and nothing from another pull
/// request ever is.
fn publication_on(
    comments: &[ownership::Comment],
    pr: Option<u64>,
) -> Result<Option<LatestPublication>, Failure> {
    Ok(match pr {
        Some(pr) => all_publications(comments)?
            .into_iter()
            .rfind(|publication| publication.receipt.pr == pr),
        None => None,
    })
}

fn latest_publication(
    comments: &[ownership::Comment],
) -> Result<Option<LatestPublication>, Failure> {
    Ok(all_publications(comments)?.into_iter().next_back())
}

pub(super) fn same_latest_publication(
    left: &[ownership::Comment],
    right: &[ownership::Comment],
) -> bool {
    latest_publication(left) == latest_publication(right)
}

/// The newest publication on one pull request, if the issue recorded one.
///
/// One issue may carry several pull requests — a chained delivery publishes
/// each slice under the issue — so the epoch a receipt must match is the newest
/// one **for its own pull request**, not the newest on the issue. A republish of
/// one pull request still invalidates that pull request's earlier epochs, while
/// a later slice's publication leaves an earlier slice's receipt usable. See
/// [`require_recorded_receipt`].
fn latest_publication_for_pr(
    comments: &[ownership::Comment],
    pr: u64,
) -> Result<Option<LatestPublication>, Failure> {
    let publications = all_publications(comments)?;
    Ok(newest_publication_for_pr(&publications, pr).cloned())
}

/// The newest publication on one pull request, if any — the predicate
/// [`require_recorded_receipt`] and [`latest_publication_for_pr`] both read.
fn newest_publication_for_pr(
    publications: &[LatestPublication],
    pr: u64,
) -> Option<&LatestPublication> {
    publications
        .iter()
        .rev()
        .find(|publication| publication.receipt.pr == pr)
}

/// One durable request for another run to review an exact review transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewHandoff {
    /// The operation that wrote this marker.
    pub operation_id: String,
    /// The run putting the review down.
    pub requester: String,
    /// The exact ownership epoch released after this marker became visible.
    pub target_operation: String,
    /// The immutable publication identity.
    pub receipt: ReviewTransaction,
    /// The configured permission under which review was requested.
    pub authority: String,
    /// When the request was recorded.
    pub requested_at: String,
    /// The one deadline recorded for the request. Elapsing it changes nothing.
    pub deadline: String,
    /// The exact condition preventing this run from completing review.
    pub blocker: String,
    /// Who or what can discharge that condition.
    pub discharger: String,
}

impl ReviewHandoff {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        let operation_id = marker.get("op-id")?.clone();
        let target_operation = marker.get("target-op")?.clone();
        let requester = marker.get("run-id")?.clone();
        let receipt = ReviewTransaction::from_marker(marker)?;
        let authority = marker.get("authority")?.clone();
        let requested_at = marker.get("requested-at")?.clone();
        let deadline = marker.get("deadline")?.clone();
        let blocker = marker
            .get("blocker")
            .filter(|value| !value.is_empty())?
            .clone();
        let discharger = marker
            .get("discharger")
            .filter(|value| !value.is_empty())?
            .clone();
        // The configuration's own grammar, not a second copy of it. This was a
        // third spelling that disagreed with the other two, and it read the
        // duration's unit by byte offset — a marker whose authority ended in a
        // multi-byte character panicked whoever parsed the timeline, which is
        // `claim`, `reclaim` and every candidate of the review queue.
        let valid_authority = crate::config::authority_of(&authority).is_some();
        let valid_target = ownership::is_operation_id(&target_operation)
            || target_operation
                .strip_prefix("legacy-")
                .is_some_and(|identity| !identity.is_empty());
        if !(ownership::is_operation_id(&operation_id)
            && valid_target
            && valid_authority
            && ownership::parse_stamp(&requested_at).is_some()
            && ownership::parse_stamp(&deadline).is_some_and(|deadline| {
                ownership::parse_stamp(&requested_at).is_some_and(|requested| deadline >= requested)
            }))
        {
            return None;
        }
        Some(Self {
            operation_id,
            requester,
            target_operation,
            receipt,
            authority,
            requested_at,
            deadline,
            blocker,
            discharger,
        })
        .filter(|handoff| !handoff.requester.is_empty())
    }
}

/// A retired aggregate verdict, read from the marker an earlier release wrote.
///
/// Nothing records one any more: [`retired_review_verdict`] refuses it. The
/// marker is still read where a round already spent or a handoff already
/// resolved has to stay counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewVerdict {
    /// The operation that wrote this marker.
    pub operation_id: String,
    /// The run that recorded it.
    pub attester: String,
    /// The context credited with the review.
    pub reviewer: String,
    /// The immutable publication identity.
    pub receipt: ReviewTransaction,
    /// `accepted` or `rejected`.
    pub outcome: String,
}

impl ReviewVerdict {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        let operation_id = marker.get("op-id")?.clone();
        let attester = marker.get("run-id")?.clone();
        let reviewer = marker.get("reviewer")?.clone();
        let outcome = marker.get("outcome")?.clone();
        if !ownership::is_operation_id(&operation_id)
            || attester.is_empty()
            || reviewer.is_empty()
            || !matches!(outcome.as_str(), "accepted" | "rejected")
        {
            return None;
        }
        Some(Self {
            operation_id,
            attester,
            reviewer,
            receipt: ReviewTransaction::from_marker(marker)?,
            outcome,
        })
    }
}

/// A terminal receipt: the state one review transaction reached.
///
/// A review transaction is opened by `publish_review` and closed by a receipt:
/// `approved` on the computed verdict that ends the review, or `escalated` when
/// the decision passes to a named person. A rejected verdict closes the round
/// without a receipt. An approval is final: it supersedes an `escalated` receipt
/// and `escalate_review` refuses to overwrite one, so the terminal state is the
/// `approved` receipt once an approval is recorded.
///
/// A receipt is a **local record**: Estigia appends it to the store and never
/// posts it to GitHub, so nothing reads it from a GitHub comment body. Both
/// states are read from the store timeline the same way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewReceipt {
    /// The run that recorded it, holding the live `review` claim.
    pub attester: String,
    /// The immutable review-transaction identity it closes.
    pub transaction: ReviewTransaction,
    /// `approved` or `escalated`.
    pub terminal_state: String,
    /// The distinct reviewer credited with an `approved` receipt.
    pub reviewer: Option<String>,
    /// The person the decision passed to on an `escalated` receipt.
    pub escalated_to: Option<String>,
    /// Why the decision was escalated; `escalated` only.
    pub reason: Option<String>,
}

impl ReviewReceipt {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        let attester = marker
            .get("run-id")
            .filter(|value| !value.is_empty())?
            .clone();
        let terminal_state = marker.get("terminal-state")?.clone();
        if !matches!(terminal_state.as_str(), "approved" | "escalated") {
            return None;
        }
        let reviewer = marker
            .get("reviewer")
            .filter(|value| !value.is_empty())
            .cloned();
        let escalated_to = marker
            .get("escalated-to")
            .filter(|value| !value.is_empty())
            .cloned();
        let reason = marker
            .get("reason")
            .filter(|value| !value.trim().is_empty())
            .cloned();
        if terminal_state == "approved" && reviewer.is_none() {
            return None;
        }
        if terminal_state == "escalated" && escalated_to.is_none() {
            return None;
        }
        Some(Self {
            attester,
            transaction: ReviewTransaction::from_marker(marker)?,
            terminal_state,
            reviewer,
            escalated_to,
            reason,
        })
    }
}

/// Every receipt standing against one exact review transaction, in order.
fn receipts_for(
    comments: &[ownership::Comment],
    transaction: &ReviewTransaction,
) -> Vec<ReviewReceipt> {
    comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .flat_map(|comment| super::markers::parse(&comment.body))
        .filter(|marker| marker.get("kind").map(String::as_str) == Some("review-receipt"))
        .filter_map(|marker| ReviewReceipt::from_marker(&marker))
        .filter(|receipt| receipt.transaction == *transaction)
        .collect()
}

/// The terminal receipt standing for this transaction, when one does.
///
/// Bounded exactly like [`latest_distinct_review_verdict`]: the transaction must
/// be the newest publication on its own pull request, and an `approved` receipt
/// credited to a run the review forbade — its publisher, or a requester of a
/// handoff over it — is not one. An `escalated` receipt credits nobody, so it
/// stands whatever run recorded it.
fn terminal_receipt(
    comments: &[ownership::Comment],
    transaction: &ReviewTransaction,
) -> Result<Option<ReviewReceipt>, Failure> {
    let Some(publication) = latest_publication_for_pr(comments, transaction.pr)? else {
        return Ok(None);
    };
    if publication.receipt != *transaction {
        return Ok(None);
    }
    Ok(standing_terminal(
        comments,
        transaction,
        Some(publication.publisher),
    ))
}

fn standing_terminal(
    comments: &[ownership::Comment],
    transaction: &ReviewTransaction,
    publisher: Option<String>,
) -> Option<ReviewReceipt> {
    let known = publisher.is_some();
    let mut forbidden: std::collections::BTreeSet<String> = publisher.into_iter().collect();
    forbidden.extend(
        handoffs_for(comments, transaction)
            .into_iter()
            .map(|handoff| handoff.requester),
    );
    receipts_for(comments, transaction)
        .into_iter()
        .rfind(|receipt| {
            receipt.terminal_state != "approved"
                || receipt
                    .reviewer
                    .as_deref()
                    .is_some_and(|reviewer| known && !forbidden.contains(reviewer))
        })
}

/// The terminal receipt of a transaction `release_ci` is about to release, with
/// an `escalated` one refused here.
///
/// Read before the release basis so the rule holds whatever would otherwise
/// release: an escalation is a decision a person took, not the acceptance
/// delivery asks for, and computed evidence must not carry it past CI.
fn require_un_escalated_terminal(
    comments: &[ownership::Comment],
    transaction: &ReviewTransaction,
) -> Result<Option<ReviewReceipt>, Failure> {
    let terminal = terminal_receipt(comments, transaction)?;
    if let Some(terminal) = &terminal
        && terminal.terminal_state != "approved"
    {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "review-receipt-escalated",
            "escalated_to": terminal.escalated_to,
            "receipt": {
                "epoch": transaction.epoch,
                "pr": transaction.pr,
                "head": transaction.head,
                "base": transaction.base,
                "digest": transaction.digest,
            },
            "action": "an escalation does not substitute for an approved computed verdict; the \
                       named person decides, and only an approved computed verdict releases CI",
        })));
    }
    Ok(terminal)
}

fn first_protocol_markers(
    comments: &[ownership::Comment],
    kind: &str,
) -> Vec<super::markers::Marker> {
    let mut markers: Vec<(usize, usize, super::markers::Marker)> =
        ownership::first_operation_markers(comments)
            .into_values()
            .filter(|(_, _, marker, unedited)| {
                *unedited && marker.get("kind").map(String::as_str) == Some(kind)
            })
            .map(|(position, index, marker, _)| (position, index, marker))
            .collect();
    markers.sort_by_key(|(position, index, _)| (*position, *index));
    markers.into_iter().map(|(_, _, marker)| marker).collect()
}

/// Every finding standing against one exact publication.
///
/// Bound to the receipt, so a republish leaves the parent ledger readable and
/// does not carry it forward: the findings of one epoch judge that epoch's bytes
/// and nothing else. What a repair inherits is decided by the lineage, which
/// names its parent, rather than by a ledger that quietly followed the work.
fn findings_for(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Vec<ReviewFinding> {
    first_protocol_markers(comments, "review-finding")
        .iter()
        .filter_map(ReviewFinding::from_marker)
        .filter(|finding| finding.receipt == *receipt)
        .collect()
}

/// Corrections one lineage may spend: the `Corrections per lineage` row, or its
/// default when the row is absent or does not parse. v4's
/// `MaxCompactCorrectionAttempts` is 1; this build's default is not.
pub(crate) fn corrections_per_lineage(context: &Context) -> usize {
    let mut config = crate::config::Config::default();
    if let Some(value) = context.get("corrections per lineage") {
        let _ = crate::config::Setting::CorrectionsPerLineage.apply(&mut config, value);
    }
    config.corrections_per_lineage as usize
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Approved,
    Rejected,
    Unjudged,
}

impl Standing {
    fn word(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Unjudged => "unjudged",
        }
    }
}

struct ReceiptStanding {
    standing: Standing,
    answered: bool,
    escalated: bool,
    /// The class of the newest finding recorded against the receipt, when that
    /// finding is what answered its verdict — the finding that licensed a
    /// repair. `None` when no finding answered it.
    clearing_finding: Option<String>,
}

/// A `rejected` aggregate verdict and a local `escalated` receipt stand over any later marker.
/// `answered` is a finding recorded against the receipt after its verdict.
fn receipt_standing(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<ReceiptStanding, Failure> {
    let mut computed_verdict: Option<(usize, computed::ComputedOutcome)> = None;
    let mut aggregate_verdict: Option<(usize, bool)> = None;
    let mut aggregate_rejected = false;
    let mut finding_at: Option<usize> = None;
    let mut finding_class: Option<String> = None;
    for (position, comment) in comments.iter().enumerate() {
        if !comment.viewer_did_author || comment.includes_created_edit {
            continue;
        }
        for marker in super::markers::parse(&comment.body) {
            match marker.get("kind").map(String::as_str) {
                Some("review-computed-verdict") => {
                    if let Some(recorded) = computed::ReviewComputedVerdict::from_marker(&marker)
                        && recorded.receipt == *receipt
                    {
                        computed_verdict = Some((position, recorded.verdict.outcome));
                    }
                }
                Some("review-verdict") => {
                    if let Some(recorded) = ReviewVerdict::from_marker(&marker)
                        && recorded.receipt == *receipt
                    {
                        aggregate_rejected |= recorded.outcome == "rejected";
                        aggregate_verdict = Some((position, recorded.outcome == "accepted"));
                    }
                }
                Some("review-finding") => {
                    if let Some(recorded) = ReviewFinding::from_marker(&marker)
                        && recorded.receipt == *receipt
                    {
                        finding_at = Some(position);
                        finding_class = Some(recorded.class);
                    }
                }
                _ => {}
            }
        }
    }
    let publisher = all_publications(comments)?
        .into_iter()
        .find(|found| found.receipt == *receipt)
        .map(|found| found.publisher);
    let escalated = standing_terminal(comments, receipt, publisher)
        .is_some_and(|terminal| terminal.terminal_state == "escalated");
    let (verdict_at, standing) = match (computed_verdict, aggregate_verdict) {
        _ if aggregate_rejected || escalated => (None, Standing::Rejected),
        (Some((at, computed::ComputedOutcome::Approved)), _) => (Some(at), Standing::Approved),
        (
            Some((at, computed::ComputedOutcome::Rejected | computed::ComputedOutcome::Escalated)),
            _,
        ) => (Some(at), Standing::Rejected),
        (Some(_), _) => (None, Standing::Unjudged),
        (None, Some((at, true))) => (Some(at), Standing::Approved),
        (None, _) => (None, Standing::Unjudged),
    };
    let computed_escalated = matches!(
        computed_verdict,
        Some((_, computed::ComputedOutcome::Escalated))
    );
    let answered =
        matches!((verdict_at, finding_at), (Some(verdict), Some(finding)) if finding > verdict);
    Ok(ReceiptStanding {
        standing,
        escalated: escalated || computed_escalated,
        answered,
        clearing_finding: if answered { finding_class } else { None },
    })
}

/// A first publication, or one over an approved parent, begins a new lineage and zeroes the count;
/// one over an unjudged parent spends nothing.
fn corrections_spent(
    comments: &[ownership::Comment],
    publications: &[LatestPublication],
    pr: u64,
) -> Result<usize, Failure> {
    let mut spent = 0;
    for publication in publications
        .iter()
        .filter(|publication| publication.receipt.pr == pr)
    {
        spent = match &publication.lineage {
            Some(lineage) if lineage.recovered => 0,
            Some(lineage) if lineage.parent.pr == pr => {
                match receipt_standing(comments, &lineage.parent)?.standing {
                    Standing::Rejected => spent + 1,
                    Standing::Unjudged => spent,
                    Standing::Approved => 0,
                }
            }
            _ => 0,
        };
    }
    Ok(spent)
}

/// Whether the lineage `receipt` belongs to may still spend a correction, counted up to and
/// including `receipt` itself.
fn corrections_remain(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<bool, Failure> {
    let mut publications = all_publications(comments)?;
    if let Some(at) = publications
        .iter()
        .position(|publication| publication.receipt == *receipt)
    {
        publications.truncate(at + 1);
    }
    Ok(corrections_spent(comments, &publications, receipt.pr)? < corrections_per_lineage(context))
}

/// A base that merely differs, reset backwards or swapped for another line of history, earns nothing.
fn base_advanced_past(
    at: &std::path::Path,
    parent_base: &str,
    payload_base: &str,
) -> Result<bool, Failure> {
    Ok(parent_base != payload_base && super::target::is_ancestor(at, parent_base, payload_base)?)
}

/// Every spend record on this issue, of any phase and against any receipt —
/// including one this issue has since republished over.
///
/// Not receipt-filtered the way [`findings_for`] is: a spend record states a
/// historical fact about a round that happened, not a judgement about the
/// round currently under review, so a republish must not make an earlier
/// round's cost unreadable. [`record_spend`] carries the same divergence from
/// [`record_review_finding`]'s shape.
fn spend_records(comments: &[ownership::Comment]) -> Vec<SpendRecord> {
    first_protocol_markers(comments, "spend")
        .iter()
        .filter_map(SpendRecord::from_marker)
        .collect()
}

/// Whether this issue's timeline ever published exactly this receipt, at any
/// epoch — not only the latest one.
///
/// [`record_spend`] asks this instead of [`require_recorded_receipt`] for the
/// same reason [`spend_records`] is not receipt-filtered: a spend record
/// judges nothing about the live review, so it is not refused merely for
/// naming a receipt a later publication superseded. It still has to name a
/// receipt that was real, so a fabricated or mistyped epoch settles nothing.
fn known_publication(comments: &[ownership::Comment], receipt: &ReviewTransaction) -> bool {
    // Reads every `published` marker directly, the same way `latest_publication`
    // does and not through `first_protocol_markers`: a `published` marker
    // carries no `op-id`, so `ownership::first_operation_markers`'s op-id-keyed
    // map — what `first_protocol_markers` walks — never holds one at all.
    comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .flat_map(|comment| super::markers::parse(&comment.body))
        .filter(|marker| marker.get("kind").map(String::as_str) == Some("published"))
        .filter_map(|marker| ReviewTransaction::from_marker(&marker))
        .any(|published| published == *receipt)
}

/// Every refutation batch recorded against this exact publication.
fn refutations_for(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Vec<ReviewRefutation> {
    first_protocol_markers(comments, "review-refutation")
        .iter()
        .filter_map(ReviewRefutation::from_marker)
        .filter(|refutation| refutation.receipt == *receipt)
        .collect()
}

/// Every identity one receipt recorded, in timeline order of first appearance,
/// each paired with whether **any** recording of it is `warning` or
/// `suggestion`.
///
/// One identity can be recorded more than once: the `review-finding` operation
/// key carries both `id` and `class`, so a second judge recording the same
/// identity writes a second marker rather than editing the first. The one rule
/// [`record_disposition`] reads from here is which identity a batch may answer:
/// one is dispositionable when at least one of its recordings is non-severe; a
/// `severe` recording beside it does not withdraw it, because the severe
/// recording is adjudicated by the verdict and the non-severe one is what a
/// disposition answers.
fn disposition_population(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Vec<(String, bool)> {
    let mut population: Vec<(String, bool)> = Vec::new();
    for finding in findings_for(comments, receipt) {
        let non_severe = !finding.is_severe();
        match population.iter_mut().find(|(id, _)| *id == finding.id) {
            Some((_, owed)) => *owed |= non_severe,
            None => population.push((finding.id, non_severe)),
        }
    }
    population
}

/// The outcome each finding id was last recorded with, across every
/// refutation batch standing against this receipt.
///
/// A later batch's entry for one id overrides an earlier batch's, the same
/// "what stands, not the pool" reading — though [`record_refutation`] refuses a second batch
/// for a receipt, so the ordinary case is exactly one.
fn refutation_outcomes(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> std::collections::BTreeMap<String, String> {
    let mut outcomes: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for refutation in refutations_for(comments, receipt) {
        for entry in refutation.outcomes {
            outcomes.insert(entry.id, entry.outcome);
        }
    }
    outcomes
}

/// The first severe recording of each finding id, which is the one a
/// refutation batch and the computed verdict both classify it by.
/// The ids a refutation batch must cover: every standing inferential severe finding.
fn inferential_population(
    by_id: &std::collections::BTreeMap<String, ReviewFinding>,
) -> std::collections::BTreeSet<&String> {
    by_id
        .values()
        .filter(|finding| finding.evidence_class.as_deref() == Some("inferential"))
        .map(|finding| &finding.id)
        .collect()
}

fn first_severe_findings(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> std::collections::BTreeMap<String, ReviewFinding> {
    let mut by_id: std::collections::BTreeMap<String, ReviewFinding> =
        std::collections::BTreeMap::new();
    for finding in findings_for(comments, receipt)
        .into_iter()
        .filter(ReviewFinding::is_severe)
    {
        by_id.entry(finding.id.clone()).or_insert(finding);
    }
    by_id
}

fn handoffs_for(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Vec<ReviewHandoff> {
    first_protocol_markers(comments, "review-handoff")
        .iter()
        .filter_map(ReviewHandoff::from_marker)
        .filter(|handoff| handoff.receipt == *receipt)
        .collect()
}

/// The runs that may not review a receipt — the publisher and every requester
/// of a handoff — or `None` when the receipt is not the newest publication on
/// its own pull request.
///
/// Deliberately not conditioned on a handoff existing. Making the review
/// requirement appear only once a handoff is recorded would mean a deleted
/// handoff comment *lowers* the bar from "a distinct reviewer judged these
/// bytes" to "nothing", and an erased record must never read as clearance.
fn forbidden_reviewers(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<Option<std::collections::BTreeSet<String>>, Failure> {
    let Some(publication) = latest_publication_for_pr(comments, receipt.pr)? else {
        return Ok(None);
    };
    if publication.receipt != *receipt {
        return Ok(None);
    }
    let mut forbidden: std::collections::BTreeSet<String> =
        [publication.publisher].into_iter().collect();
    forbidden.extend(
        handoffs_for(comments, receipt)
            .into_iter()
            .map(|handoff| handoff.requester),
    );
    Ok(Some(forbidden))
}

/// Every aggregate `review-verdict` over this exact receipt that credits a
/// distinct reviewer, oldest first.
fn distinct_review_verdicts(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<Vec<ReviewVerdict>, Failure> {
    let Some(forbidden) = forbidden_reviewers(comments, receipt)? else {
        return Ok(Vec::new());
    };
    Ok(first_protocol_markers(comments, "review-verdict")
        .iter()
        .filter_map(ReviewVerdict::from_marker)
        .filter(|verdict| {
            verdict.receipt == *receipt && !forbidden.contains(verdict.reviewer.as_str())
        })
        .collect())
}

/// The newest of [`distinct_review_verdicts`], if one was ever recorded.
fn latest_distinct_review_verdict(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<Option<ReviewVerdict>, Failure> {
    Ok(distinct_review_verdicts(comments, receipt)?.pop())
}

/// Whether a distinct run has recorded its judgement of this exact receipt — a
/// computed verdict of any outcome, or a retired verdict that already did — so
/// the handoff asking for it is resolved.
fn handoff_resolved(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<bool, Failure> {
    let Some(forbidden) = forbidden_reviewers(comments, receipt)? else {
        return Ok(false);
    };
    Ok(latest_distinct_review_verdict(comments, receipt)?.is_some()
        || computed::recorders_for(comments, receipt)
            .iter()
            .any(|recorder| !forbidden.contains(recorder)))
}

enum ReleaseBasis {
    Computed,
    StructuralReadback,
}

impl ReleaseBasis {
    fn word(&self) -> &'static str {
        match self {
            Self::Computed => "computed-verdict",
            Self::StructuralReadback => "structural-readback",
        }
    }
}

/// What clears one exact receipt for release: the verdict recomputed over the
/// live ledger, and nothing else.
///
/// A receipt carrying a computed marker or a lens report is cleared by
/// [`computed::require_computed_clearance`]: `medium` and `high` need a
/// recorded marker that agrees with the recomputation and reads `approved`.
/// A `low` receipt licenses no lens, so the same recomputation clears it with
/// or without a marker — the structural readback. An unreadable tier is a
/// refusal, never a clearance.
///
/// A legacy `review-verdict` marker is history. An `accepted` one clears
/// nothing. A `rejected` one from a distinct reviewer still refuses at every
/// tier, because the marker binds those exact bytes and an old record must never
/// read as less than it said; a republish is a new receipt and starts clean.
fn release_basis(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
    worktree: Option<&std::path::Path>,
) -> Result<ReleaseBasis, Failure> {
    if let Some(rejecting) = distinct_review_verdicts(comments, receipt)?
        .iter()
        .find(|verdict| verdict.outcome == "rejected")
    {
        let insufficient: Vec<String> = findings_for(comments, receipt)
            .into_iter()
            .filter(|finding| finding.reviewer == rejecting.reviewer)
            .filter(ReviewFinding::is_severe)
            .filter(|finding| finding.evidence_class.as_deref() == Some("insufficient"))
            .map(|finding| finding.id)
            .collect();
        if !insufficient.is_empty() {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "rejected-on-insufficient-evidence",
                "reviewer": &rejecting.reviewer,
                "receipt": {
                    "epoch": receipt.epoch,
                    "pr": receipt.pr,
                    "head": receipt.head,
                    "base": receipt.base,
                    "digest": receipt.digest,
                },
                "insufficient": insufficient,
                "action": "keep the PR draft; a `severe` finding that declares `insufficient` \
                           evidence states that it was not established, so a `rejected` verdict \
                           may not rest on it \u{2014} record that observation as a `warning` or a \
                           `suggestion`, informational and never a rejection, or establish it and \
                           declare `deterministic` or `inferential`, then publish and review again",
            })));
        }
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "legacy-verdict-rejected",
            "receipt": {
                "epoch": receipt.epoch,
                "pr": receipt.pr,
                "head": receipt.head,
                "base": receipt.base,
                "digest": receipt.digest,
            },
            "action": "keep the PR draft; a distinct reviewer rejected these exact bytes under \
                       the retired verdict; correct the change and publish again",
        })));
    }
    if computed::has_computed_evidence(comments, receipt) {
        computed::require_computed_clearance(context, comments, receipt, worktree)?;
        return Ok(ReleaseBasis::Computed);
    }
    let low = computed::policy_for(context, comments, receipt, worktree)
        .is_ok_and(|policy| policy.level == super::tier::RiskLevel::Low);
    if low {
        computed::require_computed_clearance(context, comments, receipt, worktree)?;
        return Ok(ReleaseBasis::StructuralReadback);
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "qualifying-review-verdict-missing",
        "receipt": {
            "epoch": receipt.epoch,
            "pr": receipt.pr,
            "head": receipt.head,
            "base": receipt.base,
            "digest": receipt.digest,
        },
        "action": "keep the PR draft; call `review_status`, `launch_review_input` and then `collect_review_input` for every \
                   lens slot it offers, then `record_computed_verdict`, for this exact latest \
                   receipt",
    })))
}

/// Whether one run may select or directly claim an unassigned review item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewEligibility {
    /// No unresolved handoff excludes this run.
    Eligible,
    /// This run published or requested the unresolved review.
    Excluded {
        /// The run that published the latest receipt.
        publisher: String,
        /// Every run that requested another reviewer for this receipt.
        requesters: Vec<String>,
        /// The newest durable handoff, for clear queue metadata.
        handoff: Box<ReviewHandoff>,
    },
}

/// Reduces publication, handoff and verdict markers into requester eligibility.
///
/// Read per pull request: an issue may carry several, and each one's latest
/// receipt answers its own handoff, so a later slice's publication neither
/// hides an earlier slice's unresolved handoff nor resolves it.
pub fn review_eligibility(
    comments: &[ownership::Comment],
    run_id: &str,
) -> Result<ReviewEligibility, Failure> {
    let publications = all_publications(comments)?;
    let mut seen = std::collections::BTreeSet::new();
    for publication in publications.iter().rev() {
        if !seen.insert(publication.receipt.pr) {
            continue;
        }
        let handoffs = handoffs_for(comments, &publication.receipt);
        let Some(handoff) = handoffs.last().cloned() else {
            continue;
        };
        if handoff_resolved(comments, &publication.receipt)? {
            continue;
        }
        let mut requesters: Vec<String> = handoffs
            .iter()
            .map(|handoff| handoff.requester.clone())
            .collect();
        requesters.sort();
        requesters.dedup();
        if publication.publisher != run_id
            && !requesters.iter().any(|requester| requester == run_id)
        {
            continue;
        }
        return Ok(ReviewEligibility::Excluded {
            publisher: publication.publisher.clone(),
            requesters,
            handoff: Box::new(handoff),
        });
    }
    Ok(ReviewEligibility::Eligible)
}

/// Composes one review protocol comment whose prose came from an agent.
///
/// `record_entry` posts what it is handed, and its contract is a body this
/// crate composed. A blocker, a discharger and a run-id are caller text, so a
/// body quoting a marker would **be** that marker once posted — the publishing
/// run could carry a forged `review-verdict` inside its own handoff comment and
/// clear the gate it is the one run forbidden to clear. `comment` and
/// `heartbeat` already escape for this reason; these two writers were the first
/// to route agent text through the unescaped door.
fn protocol_body(prose: &str, marker: &str) -> String {
    format!(
        "{}\n\n{marker}\n",
        super::markers::escape_control_input(prose).trim_end()
    )
}

/// Refuses the runs an unresolved handoff excludes, whatever route they take.
///
/// Both acquisitions end here rather than each carrying its own copy of the
/// refusal. `claim` checked this and `reclaim` did not, so a requester whose
/// replacement went stale could take the item back and be, again, the only
/// holder forbidden to review it — while `SKILL.md` described a single rule
/// covering both. A rule held in two places is one that disagrees with itself.
pub fn require_review_eligibility(
    comments: &[ownership::Comment],
    run_id: &str,
) -> Result<(), Failure> {
    let ReviewEligibility::Excluded {
        publisher,
        requesters,
        handoff,
    } = review_eligibility(comments, run_id)?
    else {
        return Ok(());
    };
    let handoff = *handoff;
    Err(Failure::Stop(serde_json::json!({
        "ok": false,
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
        "deadline": handoff.deadline,
        "action": "another run must claim and review this exact receipt; elapsed time does not satisfy the handoff",
    })))
}

/// Requires the caller's receipt to be the newest complete publication on its
/// own pull request, from whichever run supplies it.
///
/// One issue may carry several pull requests — a chained delivery publishes
/// each slice under the issue — so the epoch a receipt must match is the newest
/// one for that receipt's pull request, not the newest on the issue. A
/// republish of one pull request still invalidates its earlier epochs, while a
/// later slice's publication leaves an earlier slice's receipt usable.
///
/// The publisher recorded on the `published` marker is never compared, so a
/// run that did not publish the receipt may still obtain it. That widens who
/// may make the call and not which bytes reach CI: `release_ci`'s other four
/// checks — a live review claim, the computed verdict recomputed for this
/// exact receipt, the current draft PR identity, and a re-derived
/// coherent clean target — are unchanged. `docs/honesty.md` records what the
/// restore does not prove.
pub fn recorded_receipt(
    comments: &[ownership::Comment],
    supplied: &ReviewTransaction,
) -> Result<ReviewTransaction, Failure> {
    require_recorded_receipt(comments, supplied, "cannot release CI").map(|latest| latest.receipt)
}

fn require_recorded_receipt(
    comments: &[ownership::Comment],
    supplied: &ReviewTransaction,
    blocked: &str,
) -> Result<LatestPublication, Failure> {
    let publications = all_publications(comments)?;
    let Some(any) = publications.last() else {
        return Err(stop(
            "published-transaction-missing",
            "issue has no complete published receipt".to_owned(),
            &format!("publish the review target again; incomplete or absent evidence {blocked}"),
        ));
    };
    let Some(latest) = newest_publication_for_pr(&publications, supplied.pr) else {
        return Err(published_transaction_mismatch(any, supplied));
    };
    refuse_unconfirmed_publication(latest)?;
    if latest.receipt != *supplied {
        return Err(published_transaction_mismatch(latest, supplied));
    }
    Ok(latest.clone())
}

/// The refusal for a supplied receipt that is not the newest publication on its
/// own pull request. The `recorded` fields name the newest publication the
/// lookup could find — for this pull request when it has one, and the issue's
/// newest otherwise — so a caller sees what the timeline holds.
fn published_transaction_mismatch(
    recorded: &LatestPublication,
    supplied: &ReviewTransaction,
) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "published-transaction-mismatch",
        "recorded": {
            "run_id": recorded.publisher,
            "epoch": recorded.receipt.epoch,
            "pr": recorded.receipt.pr,
            "head": recorded.receipt.head,
            "base": recorded.receipt.base,
            "digest": recorded.receipt.digest,
        },
        "supplied": {
            "epoch": supplied.epoch,
            "pr": supplied.pr,
            "head": supplied.head,
            "base": supplied.base,
            "digest": supplied.digest,
        },
        "action": "use the newest complete review transaction for this pull request, \
                   publishing it first when the timeline holds none; a republish of one pull \
                   request invalidates its own earlier epochs, and a second pull request on \
                   the issue keeps its own",
    }))
}

/// What `expected_epoch` is spelled when the branch's pull request has no
/// publication yet.
pub const NO_EPOCH: &str = "none";

fn expected_epoch_moved(expected: &str, newest: Option<&str>, pr: Option<u64>) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "expected-epoch-moved",
        "expected_epoch": expected,
        "recorded_epoch": newest.unwrap_or(NO_EPOCH),
        "pr": pr,
        "action": "the newest publication on this branch's pull request is not the epoch the \
                   answer you acted on named, so that answer is stale: read the state again \
                   and decide from it. Nothing has been pushed and no pull request was opened",
    }))
}

/// An unconfirmed publication is a lease target, never a review or CI target.
fn refuse_unconfirmed_publication(latest: &LatestPublication) -> Result<(), Failure> {
    if latest.confirmed {
        return Ok(());
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "publication-unconfirmed",
        "recorded": {
            "run_id": latest.publisher,
            "epoch": latest.receipt.epoch,
            "pr": latest.receipt.pr,
            "head": latest.receipt.head,
            "base": latest.receipt.base,
            "digest": latest.receipt.digest,
        },
        "action": "the latest publication's head was pushed but never confirmed by pull-request \
                   readback, so review and CI cannot bind to it. Run republish_review after \
                   correcting the cause (rebase onto the current base when the head is no longer \
                   a descendant of it); it leases against that head and records a confirmed \
                   receipt. Do not hand force-push",
    })))
}

/// Whether one run's goal is complete, adjudicated against delivery evidence.
///
/// Read-only, like [`publication_lane`] and [`review_findings`]: no claim, no
/// `run_id`, no write the gate sees. The answer rests on three facts, in order —
/// this receipt is the newest complete publication on its own pull request,
/// neither an escalation nor `release_basis` refuses it, and its lane is not red. `release_basis` is the
/// rule `release_ci` applies: the computed verdict recomputed over the live
/// ledger. An `escalated` terminal receipt is checked first, as `release_ci`
/// checks it, and answers the goal open. Nothing here re-reads the work or the merge; `docs/honesty.md` says
/// what that costs.
///
/// A red lane is an answer, not an unreadable one: the bytes were delivered and
/// CI answered against them, so `lane-red` is the standing. A lane that did not
/// answer is the read failure it is everywhere else, and the caller fails closed.
pub fn goal_completion(
    context: &Context,
    issue: u64,
    receipt: &ReviewTransaction,
    worktree: Option<&std::path::Path>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let comments = timeline(context, issue, now)?;

    // The latest publication is not this receipt: earlier bytes, or a republish
    // this run does not hold. That is an open goal, never an unreadable one —
    // the timeline answered, and its answer is that these are not the bytes.
    if recorded_receipt(&comments, receipt).is_err() {
        return Ok(serde_json::json!({
            "ok": true,
            "complete": false,
            "standing": "open",
            "reason": "the latest publication is not this run's recorded receipt",
        }));
    }
    let cleared = require_un_escalated_terminal(&comments, receipt)
        .and_then(|_| release_basis(context, &comments, receipt, worktree));
    let basis = match cleared {
        Ok(basis) => basis,
        Err(Failure::Stop(envelope)) => {
            let reason = match envelope.get("reason").and_then(serde_json::Value::as_str) {
                Some("qualifying-review-verdict-missing") | None => {
                    "no review clears the newest receipt on its own pull request".to_owned()
                }
                Some(named) => named.to_owned(),
            };
            let mut answer = serde_json::json!({"ok": true, "complete": false, "standing": "open"});
            answer["reason"] = serde_json::json!(reason);
            return Ok(answer);
        }
        Err(other) => return Err(other),
    };
    let lane = publication_lane(context, &receipt.head, Some(receipt.pr))?;
    let state = lane
        .get("state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match state {
        "green" | "never-ran" | "absent" | "disabled" | "not-consulted" => {}
        "red" => {
            return Ok(serde_json::json!({
                "ok": true,
                "complete": false,
                "standing": "lane-red",
                "reason": "the publication lane is red",
            }));
        }
        "running" => {
            return Ok(serde_json::json!({
                "ok": true,
                "complete": false,
                "standing": "lane-running",
                "reason": "the publication lane has not finished",
            }));
        }
        // A state this verb does not know is not one it may clear on: an
        // unknown result is not clearance, so it fails closed as a failed read.
        _ => {
            return Err(Failure::Read(format!(
                "the publication lane answered a state this verb does not clear: {state:?}"
            )));
        }
    }
    Ok(serde_json::json!({
        "ok": true,
        "complete": true,
        "standing": "cleared",
        "basis": basis.word(),
        "head": receipt.head,
        "lane": state,
    }))
}

/// The most `done` candidates one completion count reads.
const COMPLETED_CANDIDATES: u64 = 100;

/// How many issues this run's own delivery has closed, read from the local
/// store.
///
/// Candidates are the store's `done` issues, and each is adjudicated by
/// `delivery_close` narrowed to this run's own publication, the same proof
/// `verify-claim` uses, over the store's own
/// timeline; only the merge proof reads GitHub, which is GitHub's own object.
/// `None` when a read did not land: a count a fault emptied is not zero, and the
/// loop it answers stops on the fault rather than reading it as none done. A
/// delivery whose topology this build cannot read is a floor — not counted, and
/// the count stays a definite answer.
pub fn completed_count(context: &Context, run_id: &str) -> Result<Option<u64>, Failure> {
    let repo = repo_of(context)?;
    let Some(store) = super::store::open_for_read(&super::store::store_path()?)? else {
        return Ok(Some(0));
    };
    let rows = match store.issue_rows(&repo) {
        Ok(rows) => rows,
        Err(_) => return Ok(None),
    };
    let mut completed: u64 = 0;
    let mut candidates: u64 = 0;
    for row in rows
        .iter()
        .filter(|row| row.state.as_deref() == Some("done"))
    {
        candidates += 1;
        if candidates > COMPLETED_CANDIDATES {
            break;
        }
        let comments = match store.timeline(&repo, row.number) {
            Ok(comments) => comments,
            Err(_) => return Ok(None),
        };
        match close::delivery_close(context, row.number, &comments, Some(run_id), None) {
            Ok(Some(_)) => completed += 1,
            Ok(None) => {}
            Err(Failure::Stop(_)) => {}
            Err(_) => return Ok(None),
        }
    }
    Ok(Some(completed))
}

/// The recorded publication for one epoch — everything `publish_review` and
/// `republish_review` no longer carry in their own answer, plus the identity
/// fields, exactly as the `published` marker recorded them. See
/// `docs/review-cost.md` for what moved here and why.
///
/// Read-only, like [`review_findings`] — no claim required, and no `run_id`,
/// because a read is not a write the gate sees. `--epoch` selects among the
/// epochs this issue's own timeline recorded, through `all_publications`,
/// never through [`recorded_receipt`]: this tool answers a repair's own
/// receipt after a later republish has landed, which is exactly when
/// `recorded_receipt` demanding the newest publication on the receipt's own
/// pull request would refuse.
/// Unlike [`review_findings`] it has no fallback to a latest publication's
/// recorded lineage parent — an epoch that exists only in that parent field
/// has no marker of its own to answer from, so naming it is refused with
/// every selectable epoch listed. Omitting `--epoch` answers the latest
/// publication.
pub fn review_transaction(
    context: &Context,
    issue: u64,
    epoch: Option<&str>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    if let Some(value) = epoch {
        require_operation_id(Some(value))?;
    }
    let comments = timeline(context, issue, now)?;

    let latest = latest_publication(&comments)?.ok_or_else(|| {
        stop(
            "published-transaction-missing",
            "issue has no complete published receipt".to_owned(),
            "publish the review target first; there is nothing to answer a receipt for",
        )
    })?;
    let publications = all_publications(&comments)?;

    let (publication, selected) = match epoch {
        None => (latest, "latest"),
        Some(value) => match publications
            .into_iter()
            .find(|publication| publication.receipt.epoch == value)
        {
            Some(publication) => (publication, "epoch"),
            None => {
                let recorded: Vec<String> = all_publications(&comments)?
                    .into_iter()
                    .map(|publication| publication.receipt.epoch)
                    .collect();
                return Err(Failure::Stop(serde_json::json!({
                    "ok": false,
                    "reason": "publication-epoch-not-found",
                    "recorded": recorded,
                    "action": "name an epoch this issue's timeline actually published, or omit \
                               --epoch to read the latest \u{2014} every epoch this issue's \
                               timeline recorded is listed above",
                })));
            }
        },
    };

    let (autoclose, autoclose_assessed): (serde_json::Value, bool) =
        match serde_json::from_str(&publication.autoclose) {
            Ok(assessed) => (assessed, true),
            Err(_) => (
                serde_json::json!({ "will_autoclose": null, "cause": null, "linked_prs": [] }),
                false,
            ),
        };
    let lane_workflow_count = publication.lane_workflows.len();

    Ok(serde_json::json!({
        "ok": true,
        "issue": issue,
        "selected": selected,
        "epoch": publication.receipt.epoch,
        "pr": publication.receipt.pr,
        "head": publication.receipt.head,
        "base": publication.receipt.base,
        "digest": publication.receipt.digest,
        "confirmed": publication.confirmed,
        "url": publication.url,
        "topology": publication.topology,
        "topology_reason": publication.topology_reason,
        "computed_level": publication.computed_level,
        "raise": publication.raise,
        "raise_reason": publication.raise_reason,
        "focus": publication.focus,
        "focus_reason": publication.focus_reason,
        "parent": publication.lineage.as_ref().map(|lineage| &lineage.parent.epoch),
        "parent_pr": publication.lineage.as_ref().map(|lineage| lineage.parent.pr),
        "parent_head": publication.lineage.as_ref().map(|lineage| &lineage.parent.head),
        "parent_base": publication.lineage.as_ref().map(|lineage| &lineage.parent.base),
        "parent_digest": publication.lineage.as_ref().map(|lineage| &lineage.parent.digest),
        "delta": publication.lineage.as_ref().map(|lineage| &lineage.delta),
        "delta_range": publication.lineage.as_ref().and_then(|lineage| lineage.delta_range.as_ref()),
        "delta_range_reason": publication.lineage.as_ref().and_then(|lineage| lineage.delta_range_reason.as_ref()),
        "publication_lane_note": publication.lane_note,
        "publication_lane_workflows": publication.lane_workflows,
        "publication_lane_count": lane_workflow_count,
        "autoclose": autoclose,
        "autoclose_assessed": autoclose_assessed,
        "mandatory_follow_up": publication.mandatory_follow_up,
    }))
}

/// A stable operation identity for one immutable review protocol event.
pub fn review_operation_id(kind: &str, fields: &[&str]) -> String {
    let mut identity = format!("{}:{kind}", kind.len());
    for field in fields {
        identity.push('|');
        identity.push_str(&field.len().to_string());
        identity.push(':');
        identity.push_str(field);
    }
    ownership::sha256_hex(identity.as_bytes())
        .chars()
        .take(32)
        .collect()
}

/// The operation identity of one finding: the receipt, the run and reviewer recording it,
/// and the finding's own `id` and `class`. The MCP server mints the same key for
/// `record_review_finding`, so recording a finding by hand and by capture reads back one event.
pub fn finding_operation_id(
    run_id: &str,
    reviewer: &str,
    receipt: &ReviewTransaction,
    id: &str,
    class: &str,
) -> String {
    review_operation_id(
        "review-finding",
        &[
            run_id,
            reviewer,
            &receipt.epoch,
            &receipt.pr.to_string(),
            &receipt.head,
            &receipt.base,
            &receipt.digest,
            id,
            class,
        ],
    )
}

/// The operation identity of one lens report: the receipt, the run and the lens role.
pub fn lens_report_operation_id(run_id: &str, role: &str, receipt: &ReviewTransaction) -> String {
    review_operation_id(
        "review-lens-report",
        &[
            run_id,
            role,
            &receipt.epoch,
            &receipt.pr.to_string(),
            &receipt.head,
            &receipt.base,
            &receipt.digest,
        ],
    )
}

fn review_authority(now: &str) -> Result<(String, String), Failure> {
    let requested = ownership::parse_stamp(now).ok_or_else(|| {
        Failure::Read("the machine clock did not produce a review request timestamp".to_owned())
    })?;
    let deadline = u64::try_from(requested).map_err(|_| {
        Failure::Read("the review request deadline predates the system clock".to_owned())
    })?;
    Ok((
        crate::config::rendered_authority(crate::config::Authority::Auto),
        crate::harness::session::stamp_of(deadline),
    ))
}

fn review_receipt<'a>(
    epoch: &'a str,
    pr: u64,
    head: &'a str,
    base: &'a str,
    digest: &'a str,
) -> Result<ReviewTransaction, Failure> {
    let receipt = ReviewTransaction {
        epoch: epoch.to_owned(),
        pr,
        head: head.to_owned(),
        base: base.to_owned(),
        digest: digest.to_owned(),
    };
    if !receipt.is_complete() {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "incomplete-review-transaction",
        })));
    }
    Ok(receipt)
}

/// Everything the compound review handoff binds before ownership is released.
#[derive(Debug, Clone)]
pub struct HandoffReview<'a> {
    /// The issue remaining in `review`.
    pub issue: u64,
    /// The current holder requesting another reviewer.
    pub run_id: &'a str,
    /// The holder's runtime projection.
    pub runtime: &'a str,
    /// Stable across every retry of this handoff.
    pub operation_id: &'a str,
    /// The exact ownership epoch this call may release.
    pub target_operation: &'a str,
    /// The publication epoch.
    pub epoch: &'a str,
    /// The pull request number.
    pub pr: u64,
    /// The full published head SHA.
    pub head: &'a str,
    /// The full published base SHA.
    pub base: &'a str,
    /// The complete-target digest.
    pub digest: &'a str,
    /// The precise condition this run cannot discharge itself.
    pub blocker: &'a str,
    /// Who or what can discharge the blocker.
    pub discharger: &'a str,
    /// The moment the timeline is judged and the request is recorded.
    ///
    /// Read from the machine, never from the run being judged. A test may pass
    /// a chosen value; production obtains it from the MCP server's clock.
    pub now: &'a str,
}

/// Records one exact review request, then releases only its named ownership epoch.
pub fn handoff_review(
    context: &Context,
    handoff: &HandoffReview<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(handoff.operation_id))?;
    let receipt = review_receipt(
        handoff.epoch,
        handoff.pr,
        handoff.head,
        handoff.base,
        handoff.digest,
    )?;
    let read = || timeline(context, handoff.issue, handoff.now);
    let labels = || super::commands::issue_view(context, handoff.issue, "labels");
    let expected = [
        ("run-id", handoff.run_id),
        ("target-op", handoff.target_operation),
        ("epoch", handoff.epoch),
        ("pr", &handoff.pr.to_string()),
        ("head", handoff.head),
        ("base", handoff.base),
        ("digest", handoff.digest),
        ("blocker", handoff.blocker),
        ("discharger", handoff.discharger),
    ];
    let comments = read()?;
    reject_operation_kind_conflict(&comments, &operation_id, &["review-handoff"])?;
    let existing = operation_marker(
        &comments,
        &operation_id,
        OperationKind::ReviewHandoff,
        &expected,
    )?;

    let recorded = match existing {
        Some(marker) => ReviewHandoff::from_marker(&marker).ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "review-handoff-operation-conflict",
            }))
        })?,
        None => {
            // The marker changes what the queue and claim gate decide, so the
            // live review claim is re-read immediately before that first write.
            verify_claim(
                context,
                handoff.issue,
                handoff.run_id,
                "review",
                handoff.now,
                None,
            )?;
            let comments = read()?;
            require_recorded_receipt(&comments, &receipt, "cannot be handed off")?;
            let ownership = holding(&comments, handoff.now);
            match plan_release(
                &ownership,
                handoff.run_id,
                false,
                Some(handoff.target_operation),
                &labels_of(&labels()?),
            )? {
                Release::Write { .. } => {}
                Release::Confirm { .. } => unreachable!("the target was supplied"),
            }
            let (authority, deadline) = review_authority(handoff.now)?;
            let marker = super::markers::render(
                "review-handoff",
                &[
                    ("run-id", handoff.run_id),
                    ("target-op", handoff.target_operation),
                    ("op-id", &operation_id),
                    ("epoch", handoff.epoch),
                    ("pr", &handoff.pr.to_string()),
                    ("head", handoff.head),
                    ("base", handoff.base),
                    ("digest", handoff.digest),
                    ("authority", &authority),
                    ("requested-at", handoff.now),
                    ("deadline", &deadline),
                    ("blocker", handoff.blocker),
                    ("discharger", handoff.discharger),
                ],
            )
            .ok_or_else(|| {
                Failure::Stop(
                    serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}),
                )
            })?;
            let body = protocol_body(
                &format!(
                    "Review handoff requested by `{}` for publication epoch `{}`.\n\nBlocker: {}\n\nDischarger: {}",
                    handoff.run_id, handoff.epoch, handoff.blocker, handoff.discharger
                ),
                &marker,
            );
            super::commands::record_entry(context, handoff.issue, &body, handoff.now)?;

            let mut seen = Vec::new();
            for _ in 0..VISIBILITY_ATTEMPTS {
                let observed = read().ok();
                let visible = observed.as_ref().is_some_and(|data| {
                    operation_marker(data, &operation_id, OperationKind::ReviewHandoff, &expected)
                        .is_ok_and(|found| found.is_some())
                });
                seen.push(observed);
                if visible {
                    break;
                }
            }
            let Some(marker) = seen.iter().flatten().last().and_then(|data| {
                operation_marker(data, &operation_id, OperationKind::ReviewHandoff, &expected)
                    .ok()
                    .flatten()
            }) else {
                return Err(Failure::Write(
                    "review handoff is not visible; retry the same request unchanged".to_owned(),
                ));
            };
            ReviewHandoff::from_marker(&marker).ok_or_else(|| {
                Failure::Write("the visible review handoff is incomplete".to_owned())
            })?
        }
    };

    // A persisted handoff can be retried after an ambiguous marker write. Recheck
    // its subject here so that a republish cannot turn that retry into release of
    // an epoch whose review request is no longer current.
    let before_release = read()?;
    require_recorded_receipt(&before_release, &receipt, "cannot be handed off")?;
    let release_id = review_operation_id("review-handoff-release", &[&operation_id]);
    unassign(
        context,
        &Departure {
            issue: handoff.issue,
            run_id: handoff.run_id,
            runtime: handoff.runtime,
            operation_id: &release_id,
            target_operation: Some(handoff.target_operation),
            held_by_other: false,
            worktree: None,
            now: handoff.now,
        },
    )?;

    let after = read().map_err(|failure| {
        Failure::Write(format!(
            "review handoff and release were written, but final state could not be read: {}",
            failure.detail()
        ))
    })?;
    let after_labels = labels().map_err(|failure| {
        Failure::Write(format!(
            "review handoff and release were written, but final state could not be read: {}",
            failure.detail()
        ))
    })?;
    if super::commands::status_labels(&after_labels) != ["status:review"] {
        return Err(Failure::Write(
            "review handoff released ownership but the issue did not remain exactly in review"
                .to_owned(),
        ));
    }
    let after_holding = holding(&after, handoff.now);
    let target_still_authoritative =
        after_holding
            .live
            .iter()
            .chain(&after_holding.stale)
            .any(|event| {
                ownership::ownership_epoch(
                    event.operation_id.as_deref(),
                    event.comment.id.as_deref(),
                    &event.comment.created_at,
                    &event.comment.body,
                ) == handoff.target_operation
            });
    if target_still_authoritative {
        return Err(Failure::Write(
            "review handoff is visible but its released ownership epoch remains authoritative"
                .to_owned(),
        ));
    }

    Ok(serde_json::json!({
        "ok": true,
        "issue": handoff.issue,
        "state": "review",
        "requester": recorded.requester,
        "target_operation": recorded.target_operation,
        "epoch": recorded.receipt.epoch,
        "pr": recorded.receipt.pr,
        "head": recorded.receipt.head,
        "base": recorded.receipt.base,
        "digest": recorded.receipt.digest,
        "authority": recorded.authority,
        "requested_at": recorded.requested_at,
        "deadline": recorded.deadline,
        "blocker": recorded.blocker,
        "discharger": recorded.discharger,
        "holder": after_holding.holder,
    }))
}

/// What one reviewer found, and what it is worth.
pub struct FindingReview<'a> {
    /// The issue whose receipt was reviewed.
    pub issue: u64,
    /// The run recording it, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// The context credited with finding it.
    pub reviewer: &'a str,
    /// Stable across every retry of this finding.
    pub operation_id: &'a str,
    /// The publication epoch this finding judges.
    pub epoch: &'a str,
    /// The pull request number.
    pub pr: u64,
    /// The full published head SHA.
    pub head: &'a str,
    /// The full published base SHA.
    pub base: &'a str,
    /// The complete-target digest.
    pub digest: &'a str,
    /// The finding's identity within this lineage.
    pub id: &'a str,
    /// `severe`, `warning` or `suggestion`.
    pub class: &'a str,
    /// What was observed, concretely enough to re-run.
    pub evidence: &'a str,
    /// What it costs if left alone.
    pub impact: &'a str,
    /// The parent ledger's finding this one continues, or empty when it is new
    /// to this epoch.
    pub parent: &'a str,
    /// `introduced` or `exposed` for a new severe finding against a repair, or
    /// empty. See [`ReviewFinding::origin`].
    pub origin: &'a str,
    /// How the claim is established, required of a severe finding. Empty on any
    /// other class means the declaration was not made and is not owed.
    pub evidence_class: &'a str,
    /// Who owns the defect, required of a severe finding. Same emptiness rule
    /// as [`FindingReview::evidence_class`].
    pub causal_disposition: &'a str,
    /// Where in the diff this finding lives: `path:line` or
    /// `path:start-end`, the path repository-relative.
    ///
    /// Required exactly where [`FindingReview::causal_disposition`] makes a
    /// claim the diff can settle — `severe` with `introduced` or `worsened`.
    /// A malformed value is refused whenever one is given at all, under any
    /// class or disposition; whether it falls inside a changed hunk of the
    /// named path is checked only where a location is owed, so a
    /// `pre-existing` finding may still volunteer one outside the change.
    /// Empty everywhere else means the declaration was not made and is not
    /// owed, the same rule [`FindingReview::evidence_class`] states for
    /// itself.
    pub location: &'a str,
    /// The shipped lens this finding came from, when it names one, rather than
    /// a judge. Validated against [`crate::harness::roles::reserved_lens`] and
    /// never minted from caller text. Empty means this finding is a judge's.
    ///
    /// A lens finding owes a [`Self::location`] whose path `base..head`
    /// changes — a lens is sealed to the diff, so a finding naming anything
    /// outside it is a finding the lens could not have seen. Checked on the
    /// path alone, never the line, **except** one that is also `severe` with
    /// [`Self::causal_disposition`] `introduced` or `worsened`, which owes the
    /// same line containment any severe causal finding owes — not a
    /// lens-specific exception, the one rule applied to both.
    pub lens: &'a str,
    /// The moment the live review claim is judged.
    ///
    /// Read from the machine, never from the run being judged. A test may pass
    /// a chosen value; production obtains it from the MCP server's clock.
    pub now: &'a str,
}

/// What one phase cost against one review transaction.
pub struct SpendReview<'a> {
    /// The issue this measurement is recorded against.
    pub issue: u64,
    /// The run recording it, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// Stable across every retry of this measurement.
    pub operation_id: &'a str,
    /// One of [`SPEND_PHASES`].
    pub phase: &'a str,
    /// The publication epoch this measurement is attributed to. Unlike a
    /// finding's, this need not be the issue's current receipt — see
    /// [`record_spend`].
    pub epoch: &'a str,
    /// The pull request number.
    pub pr: u64,
    /// The full published head SHA.
    pub head: &'a str,
    /// The full published base SHA.
    pub base: &'a str,
    /// The complete-target digest.
    pub digest: &'a str,
    /// What was varied for this measurement. Part of this record's identity,
    /// not its body: two records against the same receipt and phase naming
    /// different conditions are two measurements, never a replay of one
    /// another.
    pub conditions: &'a str,
    /// Bytes the phase was handed: brief plus payload. Kept apart from
    /// [`Self::tokens`] and never divided against it — see `docs/honesty.md`.
    pub input: u64,
    /// Tokens the phase consumed.
    pub tokens: u64,
    /// Wall-clock seconds the phase ran.
    pub seconds: u64,
    /// How many agents produced this figure.
    pub contexts: u64,
    /// What produced it, when the caller says. Empty means undeclared.
    pub note: &'a str,
    /// The moment the live review claim is judged.
    ///
    /// Read from the machine, never from the run being judged. A test may pass
    /// a chosen value; production obtains it from the MCP server's clock.
    pub now: &'a str,
}

/// A finding's location, parsed: a repository-relative path and the new-side
/// line or line range it names.
///
/// `path` owns its bytes rather than borrowing the input, because a
/// backslash-separated path is normalised to the forward slashes `git`
/// itself reports — the platform this repository is developed on pastes a
/// path the other way, and a borrow could not carry the rewritten text.
pub(crate) struct FindingLocation {
    path: String,
    start: u32,
    end: u32,
}

/// Whether a path reads as repository-relative rather than absolute or
/// escaping the repository — `/etc/passwd:1` and `../secrets:1` are both
/// refused here rather than handed to `git diff` to answer for.
///
/// A colon anywhere in the path is refused, not only a drive letter at byte
/// index 1: a repository-relative path in this repository never contains
/// one, and the two shapes that do are both dangerous to let through. A
/// leading colon is a `git` pathspec magic signature — `:!x` selects every
/// *other* path, so a location built from it would settle against the whole
/// diff rather than refuse. An embedded one is what `rg -n`/`clippy`
/// emit — `path:line:column` — and splitting on the last colon alone would
/// read the line number as a fragment of the path instead of refusing it.
fn looks_repository_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains(':')
        && !path.split(['/', '\\']).any(|part| part == "..")
}

/// Parses `path:line` or `path:start-end`, the only two shapes a finding's
/// location may take.
///
/// `None` for anything else: no colon, a path that is not repository-relative,
/// a line that is not a whole number, a range whose end precedes its start, or
/// a line of `0` — a diff has no line zero to point at. Leading and trailing
/// whitespace is trimmed once, before either half is read, so it cannot
/// survive into the path or turn a well-formed line into a malformed one.
/// The path's separators are normalised to `/` after that: a backslash reads
/// as a `git` pathspec escape rather than a directory separator, so the
/// literal path this returns must already match what `git diff` reports.
pub(crate) fn parse_finding_location(location: &str) -> Option<FindingLocation> {
    let location = location.trim();
    let (path, lines) = location.rsplit_once(':')?;
    if !looks_repository_relative(path) {
        return None;
    }
    // `u32::from_str` accepts a leading `+`, which `path:+5` should not: a
    // diff has no signed lines to point at.
    let digits = |text: &str| -> Option<u32> {
        (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    let (start, end) = match lines.split_once('-') {
        Some((start, end)) => (digits(start)?, digits(end)?),
        None => {
            let line = digits(lines)?;
            (line, line)
        }
    };
    if start == 0 || end < start {
        return None;
    }
    Some(FindingLocation {
        path: path.replace('\\', "/"),
        start,
        end,
    })
}

/// Renders a parsed location's line half back the way it names one: `"42"` for
/// a single line, `"40-45"` for a range — so a refusal names the same line the
/// caller wrote rather than a range that always shows two numbers.
fn finding_location_field(start: u32, end: u32) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

/// Whether `git diff --unified=0 <base> <head> -- <path>` mentions `path` at
/// all, and the new-side line ranges it records there — the `+c,d` half of
/// every `@@ -a,b +c,d @@` header belonging to that path.
///
/// The two answers are kept apart because an empty range vector used to mean
/// both "the diff never mentions this path" and "the diff touched this path
/// with nothing on the new side" — a pure deletion, a deleted file, a binary
/// change, a mode-only change. Both refused a location alike, and the second
/// case has no honest way through: `introduced`/`worsened` demand a
/// location, and a deletion-only path can offer no line a hunk ever added.
/// `touched` distinguishes them so the caller can accept the second and keep
/// refusing the first.
///
/// The pathspec is `:(literal)<path>`, not `<path>` — `--` stops option
/// parsing but does not disable pathspec magic, so a caller-supplied path
/// starting with `:` would otherwise be read as a magic signature rather
/// than a filename. That alone does not close every gap a pathspec opens: a
/// bare directory and a glob both still match more than the one file this
/// check means to ask about even under `:(literal)`, because prefix and
/// wildcard matching are not the magic that flag disables. What closes both
/// is the `diff --git a/<path> b/<path>` header line every matched file
/// carries — `parsed.path` never has a rename to disagree about, since a
/// single-path pathspec suppresses rename detection before it can run — so
/// only hunks under a header naming exactly `path` are kept.
///
/// Kept apart from [`record_review_finding`] so a test can drive it against
/// a real repository without any tracker at all — the same reason
/// [`never_ran_signal`] is its own function rather than inlined into
/// [`classify_reds`].
fn changed_new_side_lines(
    context: &Context,
    base: &str,
    head: &str,
    path: &str,
) -> Result<(bool, Vec<std::ops::RangeInclusive<u32>>), Failure> {
    let pathspec = format!(":(literal){path}");
    // `--inter-hunk-context=0` on the command line, not folded into
    // `pinned_diff_args`: `target.rs` shares that helper and wants git's
    // ordinary hunk fusing for the payload it renders for a human to read.
    // `--no-renames` makes the single-path-pathspec invariant the doc comment
    // above states hold by construction rather than by git's evaluation
    // order alone — see `docs/honesty.md`.
    let output = super::run(
        &super::pinned_diff_args(&[
            "--unified=0",
            "--no-renames",
            "--inter-hunk-context=0",
            base,
            head,
            "--",
            &pathspec,
        ]),
        Some(&context.repo_dir),
        super::How::read(),
    )
    .map_err(|failure| match failure {
        Failure::Read(detail) => Failure::Read(format!(
            "could not read the diff between {base} and {head} for {path:?}: {detail}"
        )),
        other => other,
    })?;
    let header = format!("diff --git a/{path} b/{path}");
    let mut touched = false;
    let mut in_target = false;
    let mut ranges = Vec::new();
    for line in output.stdout.lines() {
        if line.starts_with("diff --git ") {
            in_target = line == header;
            touched |= in_target;
            continue;
        }
        if in_target && let Some(range) = new_side_range(line) {
            ranges.push(range);
        }
    }
    Ok((touched, ranges))
}

/// One `@@ -a,b +c,d @@` hunk header's new-side range, or `None` for any other
/// line of the diff — every hunk's context and `+`/`-` lines, and the
/// `diff --git`/`index` preamble, all fail this the same way.
///
/// `None` too for a pure deletion (`+c,0`): it adds no new-side line, so there
/// is nothing here for a location to fall inside.
fn new_side_range(line: &str) -> Option<std::ops::RangeInclusive<u32>> {
    let rest = line.strip_prefix("@@ -")?;
    let (_old, rest) = rest.split_once(" +")?;
    let (new, _tail) = rest.split_once(" @@")?;
    let (start, count) = match new.split_once(',') {
        Some((start, count)) => (start.parse::<u32>().ok()?, count.parse::<u32>().ok()?),
        None => (new.parse::<u32>().ok()?, 1),
    };
    if count == 0 {
        return None;
    }
    Some(start..=(start + count - 1))
}

/// The first line in `claimed` that no range in `ranges` covers — what a
/// containment refusal names to fix, rather than saying the whole span is
/// outside the change.
fn first_unchanged_line(
    claimed: &std::ops::RangeInclusive<u32>,
    ranges: &[std::ops::RangeInclusive<u32>],
) -> u32 {
    let mut cursor = *claimed.start();
    for range in ranges {
        if cursor > *claimed.end() {
            break;
        }
        if *range.end() < cursor {
            continue;
        }
        if *range.start() > cursor {
            return cursor;
        }
        cursor = range.end() + 1;
    }
    cursor
}

/// One token [`citation_tokens`] can produce from a finding's free-text
/// field: a maximal unquoted run, or the span between two occurrences of the
/// same quote character. Ports `artifactReferenceTokens`
/// (`artifact_admission.go:654-695`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct CitationToken {
    value: String,
    quoted: bool,
}

/// Splits `text` into [`CitationToken`]s in one forward pass. Rejects
/// nothing — it only decides where one token ends and the next begins.
/// Whitespace closes the pending token; `'`, `"` or `` ` `` closes it and
/// opens a quoted span that ends at the next occurrence of the *same*
/// character, not a matched-pair grammar, so an unbalanced quote in ordinary
/// prose can capture everything up to its next twin —
/// `docs/honesty-review-tier.md` names this gap. A quote with no closer is
/// discarded, and the run after it begins a fresh token rather than reopening
/// the one the quote just closed.
fn citation_tokens(text: &str) -> Vec<CitationToken> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut pending = String::new();
    let mut index = 0;
    while index < chars.len() {
        let current = chars[index];
        if current.is_whitespace() {
            if !pending.is_empty() {
                tokens.push(CitationToken {
                    value: std::mem::take(&mut pending),
                    quoted: false,
                });
            }
            index += 1;
            continue;
        }
        if current == '\'' || current == '"' || current == '`' {
            if !pending.is_empty() {
                tokens.push(CitationToken {
                    value: std::mem::take(&mut pending),
                    quoted: false,
                });
            }
            if let Some(offset) = chars[index + 1..].iter().position(|byte| *byte == current) {
                let span: String = chars[index + 1..index + 1 + offset].iter().collect();
                if !span.is_empty() {
                    tokens.push(CitationToken {
                        value: span,
                        quoted: true,
                    });
                }
                index = index + 1 + offset + 1;
            } else {
                index += 1;
            }
            continue;
        }
        pending.push(current);
        index += 1;
    }
    if !pending.is_empty() {
        tokens.push(CitationToken {
            value: pending,
            quoted: false,
        });
    }
    tokens
}

/// What one [`CitationToken`] resolves to under `artifactRepositoryPathReference`
/// (`artifact_admission.go:618-652`). The third outcome the caller must keep
/// distinct from both of these — not a reference at all — is
/// [`citation_reference`] answering `None`, and is never refused.
enum CitationReference {
    /// Looks like a path citation and fails the normalisation oracle:
    /// escapes the repository via a `..` component, starts with `/` or `\`,
    /// or contains a `:` anywhere — everything [`looks_repository_relative`]
    /// refuses — or normalises to a path ending in `/`. An embedded,
    /// non-leading backslash is rewritten to `/` rather than refused for being
    /// one; a [`CitationReference::Candidate`] carries the rewritten path.
    Malformed { path: String },
    /// A repository-relative path worth looking up, with the line or range
    /// the citation named.
    Candidate { path: String, end: u32 },
}

/// Whether `path` contains a `.` immediately followed by an ASCII letter —
/// the narrow half of the `pathLike` gate below that tells `foo.rs` and
/// `honesty.md` apart from `1.2` and `10.30`.
fn citation_dot_then_letter(path: &str) -> bool {
    path.char_indices().any(|(at, character)| {
        character == '.'
            && path[at + character.len_utf8()..]
                .chars()
                .next()
                .is_some_and(|next| next.is_ascii_alphabetic())
    })
}

/// Classifies one token as [`CitationReference::Candidate`],
/// [`CitationReference::Malformed`], or not a reference at all (`None`).
///
/// Four deliberate divergences from `artifactRepositoryPathReference`
/// (`artifact_admission.go:618-652`), all named in
/// `docs/honesty-review-tier.md`: the line half accepts `N-M` ranges as well
/// as a bare `N`, where theirs accepts only a bare digit run and silently
/// drops every range; the `pathLike` gate below is a disjunction — a quoted
/// token, a path containing `/`, or a `.` followed by an ASCII letter — so
/// only a slashless, unquoted token is held to the narrower dot clause, where
/// theirs accepts any `.` at all and so reads `1.2:3` as the path `1.2`; an
/// embedded, non-leading backslash is rewritten to `/` rather than refused
/// for being one, matching this repository's own `parse_finding_location`;
/// and a path ending in a raw `/` answers `None` rather than being routed
/// through the origin's `normalizeLogicalPath`, whose `path.Clean(value) !=
/// value` makes `docs/` non-canonical and rejects the whole result.
/// `docs/honesty-review-tier.md` says why the backslash reading is stated as
/// a reading of the origin's code rather than a checked fact.
fn citation_reference(token: &CitationToken) -> Option<CitationReference> {
    let trimmed;
    let raw = if token.quoted {
        token.value.as_str()
    } else {
        trimmed = token
            .value
            .trim_start_matches(['(', '[', '{', '<'])
            .trim_end_matches([')', ']', '}', '>', '.', ',', ';', '!', '?']);
        trimmed
    };
    let (path_part, line_part) = raw.rsplit_once(':')?;
    if path_part.is_empty() || line_part.is_empty() {
        return None;
    }
    let digits = |text: &str| -> Option<u32> {
        (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| text.parse().ok())
            .flatten()
    };
    let (start, end) = match line_part.split_once('-') {
        Some((left, right)) => (digits(left)?, digits(right)?),
        None => {
            let line = digits(line_part)?;
            (line, line)
        }
    };
    if start == 0 || end < start {
        return None;
    }
    if path_part.contains("://") {
        return None;
    }
    if path_part.ends_with('/') {
        return None;
    }
    let path_like = token.quoted || path_part.contains('/') || citation_dot_then_letter(path_part);
    if !path_like {
        return None;
    }
    let normalized_path = path_part.replace('\\', "/");
    if normalized_path.ends_with('/') || !looks_repository_relative(path_part) {
        return Some(CitationReference::Malformed {
            path: path_part.to_owned(),
        });
    }
    Some(CitationReference::Candidate {
        path: normalized_path,
        end,
    })
}

/// The first token of `text` that [`check_finding_citations`] refuses as
/// `malformed-path` before any tree is read.
pub(crate) fn malformed_citation(text: &str) -> Option<String> {
    citation_tokens(text).into_iter().find_map(|token| {
        matches!(
            citation_reference(&token),
            Some(CitationReference::Malformed { .. })
        )
        .then_some(token.value)
    })
}

/// One `git ls-tree` record's kind, restricted to what
/// [`resolve_citation_path`] treats as satisfying a citation.
enum CitationRecordKind {
    Directory,
    LinedBlob { oid: String },
    NoLineCheck,
}

/// Parses one non-empty `git ls-tree -z --full-tree` record for `path`,
/// strictly: measured on this checkout as `<mode> <type> <oid>\t<path>`, and
/// any deviation — a missing tab, a tail that is not `path` byte for byte, a
/// header of other than three fields, an object id that is not hex — is a
/// [`Failure::Read`], never read as absence. A surprising answer from `git`
/// must not be mistaken for a path that simply is not there.
fn parse_ls_tree_record(record: &str, path: &str) -> Result<CitationRecordKind, Failure> {
    let (header, tail) = record.split_once('\t').ok_or_else(|| {
        Failure::Read(format!(
            "git ls-tree answered a record with no tab for {path:?}: {record:?}"
        ))
    })?;
    if tail != path {
        return Err(Failure::Read(format!(
            "git ls-tree answered {tail:?} for the requested path {path:?}"
        )));
    }
    let fields: Vec<&str> = header.split(' ').collect();
    let (mode, kind, object) = match fields.as_slice() {
        [mode, kind, object] => (*mode, *kind, *object),
        _ => {
            return Err(Failure::Read(format!(
                "git ls-tree answered a header of {} field(s), not 3, for {path:?}: {header:?}",
                fields.len()
            )));
        }
    };
    if object.is_empty() || !object.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Failure::Read(format!(
            "git ls-tree answered an object id that is not hex for {path:?}: {object:?}"
        )));
    }
    match (mode, kind) {
        ("040000", "tree") => Ok(CitationRecordKind::Directory),
        ("100644", "blob") | ("100755", "blob") => Ok(CitationRecordKind::LinedBlob {
            oid: object.to_owned(),
        }),
        ("120000", "blob") | ("160000", "commit") => Ok(CitationRecordKind::NoLineCheck),
        _ => Err(Failure::Read(format!(
            "git ls-tree answered an unexpected {mode} {kind} for {path:?}"
        ))),
    }
}

/// `git ls-tree -z --full-tree <sha> -- :(literal)<path>`, parsed. `None` for
/// an empty answer — `path` does not exist in `sha`'s tree. More than one
/// record is an error, not a fallback: a literal pathspec answers with
/// exactly one record for a plain path, but with every entry underneath a
/// path ending in `/`, which names a directory rather than a file. Nothing
/// should route such a path here — [`citation_reference`] treats a raw
/// trailing slash as not a reference at all and refuses as
/// [`CitationReference::Malformed`] a path that only ends in one once
/// backslashes are normalised — so this arm is a fail-closed backstop, not
/// an expected shape.
fn citation_ls_tree(
    context: &Context,
    sha: &str,
    path: &str,
) -> Result<Option<CitationRecordKind>, Failure> {
    let pathspec = format!(":(literal){path}");
    let output = super::run(
        &["git", "ls-tree", "-z", "--full-tree", sha, "--", &pathspec],
        Some(&context.repo_dir),
        super::How::read(),
    )?;
    let records: Vec<&str> = output
        .stdout
        .split('\0')
        .filter(|record| !record.is_empty())
        .collect();
    match records.as_slice() {
        [] => Ok(None),
        [record] => parse_ls_tree_record(record, path).map(Some),
        other => Err(Failure::Read(format!(
            "git ls-tree answered {} records for {path:?}, expected at most one",
            other.len()
        ))),
    }
}

/// What one citation's path resolved to, walking both `base` and `head`
/// rather than stopping at the first to answer, in precedence order: a
/// `100644`/`100755` blob in either tree contributes a line count first, and
/// the citation is checked against the larger of the two — a path present at
/// base and absent at head is still accepted, and so is one whose line
/// exists only past base's shorter blob, at head. Only if neither tree has a
/// blob does a `120000` symlink or `160000` submodule commit in either tree
/// answer with no line count, since neither carries lines to check. A tree
/// entry — `040000` — never satisfies a citation on its own; a directory
/// answered by either tree, with no tree also answering a blob, symlink or
/// submodule, is what makes `CitationResolution::Directory`.
#[derive(Clone)]
enum CitationResolution {
    Found { line_count: Option<u64> },
    Directory,
    Absent,
}

/// Ports `frozenRepositoryPathLookup.contains` (`artifact_admission.go:547-578`),
/// with one divergence named in `docs/honesty-review-tier.md`: theirs validates
/// the line syntactically and never opens the blob, where a `100644`/`100755`
/// match
/// here is read with [`tier::blob_content`] and its line count carried back
/// so the caller can refuse a citation past end of file. A `120000` symlink
/// or `160000` submodule commit satisfies existence alone and carries no line
/// count — `docs/honesty-review-tier.md` says why.
fn resolve_citation_path(
    context: &Context,
    base: &str,
    head: &str,
    path: &str,
) -> Result<CitationResolution, Failure> {
    let mut saw_directory = false;
    let mut saw_no_line_check = false;
    let mut largest_line_count: Option<u64> = None;
    for sha in [base, head] {
        match citation_ls_tree(context, sha, path)? {
            Some(CitationRecordKind::Directory) => saw_directory = true,
            Some(CitationRecordKind::NoLineCheck) => saw_no_line_check = true,
            Some(CitationRecordKind::LinedBlob { oid }) => {
                let content = super::tier::blob_content(&context.repo_dir, &oid)?;
                let line_count = content.lines().count() as u64;
                largest_line_count = Some(match largest_line_count {
                    Some(largest) => largest.max(line_count),
                    None => line_count,
                });
            }
            None => {}
        }
    }
    Ok(if let Some(line_count) = largest_line_count {
        CitationResolution::Found {
            line_count: Some(line_count),
        }
    } else if saw_no_line_check {
        CitationResolution::Found { line_count: None }
    } else if saw_directory {
        CitationResolution::Directory
    } else {
        CitationResolution::Absent
    })
}

/// Resolves `path` through `resolve`, keeping the answer in `cache` so a
/// finding citing the same path more than once spawns one lookup rather than
/// one per citation. Never caches an `Err` — an unreadable answer is asked
/// again next time, not remembered as though it had settled something.
fn cached_citation_resolution(
    cache: &mut HashMap<String, CitationResolution>,
    path: &str,
    resolve: impl FnOnce() -> Result<CitationResolution, Failure>,
) -> Result<CitationResolution, Failure> {
    if let Some(hit) = cache.get(path) {
        return Ok(hit.clone());
    }
    let resolution = resolve()?;
    cache.insert(path.to_owned(), resolution.clone());
    Ok(resolution)
}

/// Scans `evidence`, then `impact`, for every citation either field's prose
/// makes, and refuses the finding at the first one that cannot be resolved
/// against `base..head`: a finding's free-text fields were
/// written to the ledger verbatim and read by nobody, so a citation naming a
/// path that never existed recorded exactly as cleanly as one that did.
fn check_finding_citations(
    context: &Context,
    base: &str,
    head: &str,
    evidence: &str,
    impact: &str,
) -> Result<(), Failure> {
    let mut cache: HashMap<String, CitationResolution> = HashMap::new();
    for (field, text) in [("evidence", evidence), ("impact", impact)] {
        for token in citation_tokens(text) {
            let Some(reference) = citation_reference(&token) else {
                continue;
            };
            let (path, cause): (String, &str) = match reference {
                CitationReference::Malformed { path } => (path, "malformed-path"),
                CitationReference::Candidate { path, end } => {
                    let resolution = cached_citation_resolution(&mut cache, &path, || {
                        resolve_citation_path(context, base, head, &path)
                    })?;
                    let cause = match resolution {
                        CitationResolution::Absent => "path-not-in-either-tree",
                        CitationResolution::Directory => "path-is-not-a-file",
                        CitationResolution::Found {
                            line_count: Some(count),
                        } if u64::from(end) > count => "line-past-end-of-file",
                        CitationResolution::Found { .. } => continue,
                    };
                    (path, cause)
                }
            };
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "finding-citation-unresolvable",
                "field": field,
                "citation": token.value,
                "path": path,
                "cause": cause,
                "action": "correct or remove the citation \u{2014} name a path base..head \
                           actually has, at a line inside it, or drop the reference from the \
                           prose",
            })));
        }
    }
    Ok(())
}

/// The checks `record_review_finding` makes from the finding's own text, before it reads
/// anything: the vocabularies, the words every finding owes and the declarations a
/// severe one owes. Answers the validated operation id.
fn check_finding_shape(finding: &FindingReview<'_>) -> Result<String, Failure> {
    let operation_id = require_operation_id(Some(finding.operation_id))?;
    if !FINDING_CLASSES.contains(&finding.class) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-finding-class", "value": finding.class,
            "expected": FINDING_CLASSES,
        })));
    }
    if !finding.origin.is_empty() && !FINDING_ORIGINS.contains(&finding.origin) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-finding-origin", "value": finding.origin,
            "expected": FINDING_ORIGINS,
        })));
    }
    if !finding.evidence_class.is_empty()
        && !FINDING_EVIDENCE_CLASSES.contains(&finding.evidence_class)
    {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-finding-evidence-class", "value": finding.evidence_class,
            "expected": FINDING_EVIDENCE_CLASSES,
        })));
    }
    if !finding.causal_disposition.is_empty()
        && !FINDING_CAUSAL_DISPOSITIONS.contains(&finding.causal_disposition)
    {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-finding-causal-disposition",
            "value": finding.causal_disposition,
            "expected": FINDING_CAUSAL_DISPOSITIONS,
        })));
    }
    // `--lens` names one of the five shipped sealed lenses or nothing; it is
    // never minted from caller text.
    if !finding.lens.is_empty() && !crate::harness::roles::reserved_lens(finding.lens) {
        let expected: Vec<&str> = crate::harness::roles::RESERVED_REVIEWERS
            .iter()
            .filter(|&&(name, _, _)| crate::harness::roles::reserved_lens(name))
            .map(|&(name, _, _)| name)
            .collect();
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-finding-lens", "value": finding.lens,
            "expected": expected,
        })));
    }
    // Every field a later reader needs in order to act on this without asking
    // the author. A finding with no evidence cannot be re-run and a finding with
    // no impact cannot be weighed, and either one is a preference wearing a
    // classification — including one that names a word instead of an answer,
    // which `says_nothing` catches the same way it catches an empty `evidence`
    // or `impact`. `id` is not asked to say anything beyond present: it is an
    // identifier, not a declaration.
    for (name, value) in [
        ("id", finding.id),
        ("evidence", finding.evidence),
        ("impact", finding.impact),
    ] {
        let missing = if matches!(name, "evidence" | "impact") {
            says_nothing(value)
        } else {
            value.trim().is_empty()
        };
        if missing {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "incomplete-review-finding", "missing": name,
            })));
        }
    }
    // And the severe class owes two declarations the others do not: how its
    // claim is established and who owns its cause. A rejection rests on this
    // class alone, and a severe finding that says neither is a preference
    // wearing a classification by another route.
    if finding.class == "severe" {
        for (name, value) in [
            ("evidence_class", finding.evidence_class),
            ("causal_disposition", finding.causal_disposition),
        ] {
            if value.trim().is_empty() {
                return Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false, "reason": "incomplete-review-finding", "missing": name,
                })));
            }
        }
    }
    Ok(operation_id)
}

/// The checks `record_review_finding` makes against the change and the lineage: where the
/// finding lives, what its prose cites, and the parent or origin it owes. Reads the
/// checkout and `comments`, writes nothing.
fn check_finding_against_change(
    context: &Context,
    finding: &FindingReview<'_>,
    receipt: &ReviewTransaction,
    comments: &[ownership::Comment],
) -> Result<(), Failure> {
    // `introduced` and `worsened` are the two words that claim the defect was
    // not there, or not this bad, before this change's bytes — the one claim
    // `base..head` can actually settle. Every other disposition names
    // something the diff has no opinion on, so a location stays optional
    // there.
    let location_owed = finding.class == "severe"
        && matches!(finding.causal_disposition, "introduced" | "worsened");
    // A lens is sealed to one diff, so every finding it produces owes a
    // location naming a path that diff actually changed — under any class or
    // causal_disposition, unlike `location_owed` above.
    let lens_owed = !finding.lens.is_empty();
    // Trimmed once, here, and used for presence, parsing and the write below —
    // so a whitespace-only value reads as absent everywhere rather than as
    // "no declaration" to the presence check and "something" to the marker.
    let location = finding.location.trim();
    if location.is_empty() {
        if location_owed {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "finding-location-required",
                "value": finding.location,
                "action": "name where this defect lives \u{2014} `path:line` or \
                           `path:start-end`, the path repository-relative \u{2014} before \
                           recording a severe finding whose causal_disposition names this \
                           change as the cause",
            })));
        }
        if lens_owed {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "lens-finding-location-required",
                "value": finding.location,
                "lens": finding.lens,
                "action": "name where this defect lives \u{2014} `path:line` or \
                           `path:start-end`, the path repository-relative \u{2014} before \
                           recording a finding from a sealed lens, whose whole world is the \
                           diff it was sealed to",
            })));
        }
    // A malformed location is malformed under every disposition, so that half
    // is checked whenever one is given. Whether it falls inside the change is
    // not: outside the diff is the defect for `introduced` and exactly where a
    // `pre-existing` one belongs. Refusing both would punish a reviewer for
    // volunteering the location that proves the defect predates these bytes.
    } else {
        let Some(parsed) = parse_finding_location(location) else {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false,
                "reason": "finding-location-malformed",
                "value": finding.location,
                "action": "state it as `path:line` or `path:start-end`, with a \
                           repository-relative path",
            })));
        };
        if location_owed {
            let (touched, ranges) =
                changed_new_side_lines(context, &receipt.base, &receipt.head, &parsed.path)?;
            let claimed = parsed.start..=parsed.end;
            // A path the diff mentions but that carries no new-side hunk — a
            // pure deletion, a deleted file, a binary or mode-only change —
            // settles by having been touched at all: there is no hunk range
            // for any line to fall inside, and refusing it anyway would leave
            // a deletion-only defect with no honest way to be recorded as
            // `introduced`. A path the diff never mentions settles nothing
            // either way.
            //
            // Containment, not overlap: the whole claimed span must lie
            // inside one hunk's range, not merely share a line with it.
            let settled = if ranges.is_empty() {
                touched
            } else {
                ranges
                    .iter()
                    .any(|range| range.start() <= claimed.start() && claimed.end() <= range.end())
            };
            if !settled {
                let unchanged = first_unchanged_line(&claimed, &ranges);
                return Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false,
                    "reason": "finding-location-outside-the-change",
                    "value": finding.location,
                    "path": parsed.path,
                    "line": finding_location_field(parsed.start, parsed.end),
                    "first_unchanged_line": unchanged,
                    "action": format!(
                        "correct the location so it names a path base..head actually changed \
                         and a line inside one changed hunk of it \u{2014} line {unchanged} in \
                         {} is not \u{2014} or record this finding's causal_disposition as \
                         pre-existing if the defect is not this change's",
                        parsed.path,
                    ),
                })));
            }
        // A lens finding not already covered by the stricter check above: the
        // path only, per their check (c) at `artifact_admission.go:397-399` —
        // the line is not checked here, so a lens may anchor anywhere in a
        // changed file.
        } else if lens_owed {
            let (touched, _ranges) =
                changed_new_side_lines(context, &receipt.base, &receipt.head, &parsed.path)?;
            if !touched {
                return Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false,
                    "reason": "lens-finding-location-outside-the-diff",
                    "value": finding.location,
                    "path": parsed.path,
                    "lens": finding.lens,
                    "action": "name a path base..head actually changed \u{2014} a lens finding \
                               may anchor any line of a changed file, but not a path the diff \
                               never touched",
                })));
            }
        }
    }

    // Below the replay return for the reason that return states for itself, and
    // above the lineage rules so an unresolvable citation is named before a blocker is.
    check_finding_citations(
        context,
        &receipt.base,
        &receipt.head,
        finding.evidence,
        finding.impact,
    )?;

    // What this publication descends from, and therefore which of the two
    // lineage rules apply. Read off the timeline; a caller does not get to say.
    //
    // No epoch filter any more, and its removal is the point: the receipt was
    // checked against the live publication for its own pull request above, so
    // this *is* that publication. The filter it replaces was what made both
    // rules disengage silently on any other epoch.
    let ancestry = latest_publication_for_pr(comments, receipt.pr)?
        .and_then(|publication| publication.lineage);

    // A finding that says it continues an earlier one must name one that exists.
    // Otherwise `parent` is free text that makes a fresh observation look
    // settled, and *settled* is exactly the reading a re-review acts on.
    if !finding.parent.is_empty() {
        let Some(lineage) = &ancestry else {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "finding-parent-without-lineage",
                "parent": finding.parent,
                "action": "this publication descends from nothing, so there is no earlier ledger \
                           for that identity to have come from",
            })));
        };
        // The parent ledger by the parent **receipt**, not by its epoch. An
        // epoch is not a function of the receipt — `publication_epoch` mixes a
        // clock into its hash — and a finding's epoch field is whatever the
        // finding says it is, so matching on it alone made `--parent` satisfiable
        // by backfilling one marker that named the parent epoch and carried the
        // repair's own bytes. Two reviewers drove exactly that.
        let known: Vec<String> = findings_for(comments, &lineage.parent)
            .into_iter()
            .map(|earlier| earlier.id)
            .collect();
        if !known.iter().any(|id| id == finding.parent) {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "finding-parent-unknown",
                "parent": finding.parent,
                "parent_epoch": lineage.parent.epoch,
                "recorded": known,
                "action": "name a finding recorded against the parent receipt \u{2014} the \
                           identities it holds are listed above \u{2014} or record this as new \
                           to this publication and state its origin",
            })));
        }
    // And the other half: a **new** blocker against a repair has to say why the
    // repair is answerable for it. Without this, a full-target resweep that
    // rediscovers or rephrases a settled observation arrives indistinguishable
    // from a defect the repair caused, and a reviewer reading the second cannot
    // tell it from the first.
    //
    // Severe only. A warning or a suggestion new to a repair costs nothing, and
    // asking it to justify itself would price the cheap observation out again —
    // the defect the other half of this change exists to fix.
    } else if finding.class == "severe"
        && ancestry.is_some()
        && !FINDING_ORIGINS.contains(&finding.origin)
    {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "new-blocker-without-origin",
            "expected": FINDING_ORIGINS,
            "action": "name the finding this reassesses, or state whether the repair introduced \
                       this defect or exposed one that was already there",
        })));
    }
    Ok(())
}

/// The checks `record_review_finding` makes from the finding's own text, the recorded receipt,
/// the change and the lineage, in that order and over a timeline the caller already read, so a
/// batch can be refused whole before any finding is recorded. It is not the whole of the recorder:
/// the operation-id kind conflict, the replay branch and `verify_claim` are not part of it and
/// still happen only at the write. Writes nothing and renews nothing.
pub(crate) fn precheck_review_finding(
    context: &Context,
    finding: &FindingReview<'_>,
    comments: &[ownership::Comment],
) -> Result<(), Failure> {
    check_finding_shape(finding)?;
    let receipt = review_receipt(
        finding.epoch,
        finding.pr,
        finding.head,
        finding.base,
        finding.digest,
    )?;
    require_recorded_receipt(comments, &receipt, "cannot be recorded as a finding")?;
    check_finding_against_change(context, finding, &receipt, comments)
}

/// Records one finding against an exact review transaction.
///
/// Separate from the verdict on purpose. A verdict is one decision about a
/// publication; a finding is one observation about it, and a review makes several
/// of different weights. Folding them together is what produced the defect this
/// operation exists to end — the observation and the decision reduced to a single
/// bit, so a suggestion cost a rejection.
///
/// Idempotent on `operation_id` in the same shape as every other durable write
/// here: a replay reads back the marker it already wrote rather than adding a
/// second one, so a retry after an unreadable answer cannot double-count a
/// finding into a quorum.
pub fn record_review_finding(
    context: &Context,
    finding: &FindingReview<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = check_finding_shape(finding)?;
    // The shape first: `review_receipt` refuses an epoch, head, base or digest
    // that could not name anything. Whether it names the publication actually
    // under review is asked below, once the timeline has been read.
    let receipt = review_receipt(
        finding.epoch,
        finding.pr,
        finding.head,
        finding.base,
        finding.digest,
    )?;
    let read = || timeline(context, finding.issue, finding.now);
    let pr_field = finding.pr.to_string();
    let expected = [
        ("run-id", finding.run_id),
        ("reviewer", finding.reviewer),
        ("epoch", finding.epoch),
        ("pr", &pr_field),
        ("head", finding.head),
        ("base", finding.base),
        ("digest", finding.digest),
        ("id", finding.id),
        ("class", finding.class),
    ];
    let comments = read()?;
    reject_operation_kind_conflict(&comments, &operation_id, &["review-finding"])?;
    // The receipt has to be the one under review, and this operation did not
    // ask. Three costs, each one why the check is here, for a finding recorded
    // against a superseded epoch — or against an epoch no publication ever had:
    //
    // - The ledger a verdict reads is receipt-bound, so those findings counted
    //   for nothing while reading as though they counted.
    // - `ancestry` below resolves through the live publication, so against any
    //   other epoch **both** lineage rules silently disengaged.
    // - One mistyped hex character recorded a finding that satisfied nothing,
    //   and the verdict's refusal then named `record_review_finding` as the way
    //   out — a command that had already succeeded and would succeed again
    //   without clearing the block. `CLAUDE.md`: naming a dead end is worse than
    //   naming nothing.
    //
    // On the replay branch too, which is where `record_computed_verdict` puts its
    // own copy of this. A retry whose receipt has been superseded since is not a
    // retry of anything that is still under review.
    //
    // Ahead of the location check below rather than after it, on purpose: the
    // diff that check reads comes from `base..head` on this same receipt, and a
    // caller naming a stale or simply wrong one would otherwise be told to fix
    // the *location* when the real defect is the epoch it named.
    require_recorded_receipt(&comments, &receipt, "cannot be recorded as a finding")?;
    // Ahead of the location block below, on purpose: `location` and
    // `causal_disposition` are outside the operation key, like `basis` on the
    // verdict, so a retry naming a corrected-but-still-outside location for an
    // operation already on the timeline reads back the body that stands
    // instead of being refused over wording that no longer matters. A replay
    // whose checkout cannot read `base..head` at all — the case
    // `skill/bindings/github.md` states this guarantee for — must reach this
    // branch before the diff read the location block performs ever runs.
    if let Some(marker) = operation_marker(
        &comments,
        &operation_id,
        OperationKind::ReviewFinding,
        &expected,
    )? {
        let persisted = ReviewFinding::from_marker(&marker).ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "review-finding-operation-conflict",
            }))
        })?;
        let mut answer = serde_json::json!({
            "ok": true,
            "issue": finding.issue,
            "reviewer": persisted.reviewer,
            "id": persisted.id,
            "class": persisted.class,
            "blocking": persisted.is_severe(),
            "reused_existing_finding": true,
        });
        // What stands, not what was asked for. A replay reports the marker
        // already on the timeline, and a caller retrying with a different
        // parent or origin needs to see which one it actually has. The key is
        // omitted rather than `null` when the persisted finding carries none —
        // see `docs/review-cost.md`.
        if let Some(object) = answer.as_object_mut() {
            for (key, value) in [
                ("parent", &persisted.parent),
                ("origin", &persisted.origin),
                ("evidence_class", &persisted.evidence_class),
                ("causal_disposition", &persisted.causal_disposition),
                ("location", &persisted.location),
                ("lens", &persisted.lens),
            ] {
                if let Some(value) = value {
                    object.insert(key.to_owned(), serde_json::json!(value));
                }
            }
        }
        return Ok(answer);
    }

    check_finding_against_change(context, finding, &receipt, &comments)?;

    let location = finding.location.trim();

    verify_claim(
        context,
        finding.issue,
        finding.run_id,
        "review",
        finding.now,
        None,
    )?;

    let mut written: Vec<(&str, &str)> = vec![
        ("run-id", finding.run_id),
        ("reviewer", finding.reviewer),
        ("op-id", &operation_id),
        ("epoch", finding.epoch),
        ("pr", &pr_field),
        ("head", finding.head),
        ("base", finding.base),
        ("digest", finding.digest),
        ("id", finding.id),
        ("class", finding.class),
        ("evidence", finding.evidence),
        ("impact", finding.impact),
    ];
    if !finding.parent.is_empty() {
        written.push(("parent", finding.parent));
    }
    if !finding.origin.is_empty() {
        written.push(("origin", finding.origin));
    }
    // Outside the operation key, like `basis` on the verdict: a retry that
    // words its declaration differently names the same event and reads back
    // what stands rather than minting a second finding over it.
    if !finding.evidence_class.is_empty() {
        written.push(("evidence-class", finding.evidence_class));
    }
    if !finding.causal_disposition.is_empty() {
        written.push(("causal-disposition", finding.causal_disposition));
    }
    if !location.is_empty() {
        written.push(("location", location));
    }
    if !finding.lens.is_empty() {
        written.push(("lens", finding.lens));
    }
    let marker = super::markers::render("review-finding", &written).ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    let body = protocol_body(
        &format!(
            "Review finding `{}` classified `{}` by `{}` for publication epoch `{}`.",
            finding.id, finding.class, finding.reviewer, finding.epoch
        ),
        &marker,
    );
    super::commands::record_entry(context, finding.issue, &body, finding.now)?;

    let mut seen = Vec::new();
    for _ in 0..VISIBILITY_ATTEMPTS {
        let observed = read().ok();
        let visible = observed.as_ref().is_some_and(|data| {
            operation_marker(data, &operation_id, OperationKind::ReviewFinding, &expected)
                .ok()
                .flatten()
                .is_some()
        });
        if visible {
            let mut answer = serde_json::json!({
                "ok": true,
                "issue": finding.issue,
                "reviewer": finding.reviewer,
                "id": finding.id,
                "class": finding.class,
                "blocking": finding.class == "severe",
                "reused_existing_finding": false,
            });
            // Omitted rather than `null` when absent — see the replay branch
            // above and `docs/review-cost.md`.
            if let Some(object) = answer.as_object_mut() {
                for (key, value) in [
                    ("parent", finding.parent),
                    ("origin", finding.origin),
                    ("evidence_class", finding.evidence_class),
                    ("causal_disposition", finding.causal_disposition),
                    ("location", location),
                    ("lens", finding.lens),
                ] {
                    if !value.is_empty() {
                        object.insert(key.to_owned(), serde_json::json!(value));
                    }
                }
            }
            return Ok(answer);
        }
        seen.push(observed);
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false, "reason": "review-finding-readback-failed", "world": "committed",
    })))
}

/// Everything one refutation batch records against one exact publication
/// receipt.
pub struct Refutation<'a> {
    /// The issue whose receipt is being adjudicated.
    pub issue: u64,
    /// The run recording it, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// The distinct context credited with the refutation.
    pub refuter: &'a str,
    /// Stable across every retry of this batch.
    pub operation_id: &'a str,
    /// The publication epoch this batch adjudicates.
    pub epoch: &'a str,
    /// The pull request number.
    pub pr: u64,
    /// The full published head SHA.
    pub head: &'a str,
    /// The full published base SHA.
    pub base: &'a str,
    /// The complete-target manifest digest.
    pub digest: &'a str,
    /// The whole batch, one entry per blocking candidate, as a JSON array of
    /// `{"id": ..., "outcome": ...}` objects — see [`RefutationOutcome`].
    pub outcomes: &'a str,
    /// The moment the live review claim is judged.
    ///
    /// Read from the machine, never from the run being judged. A test may pass
    /// a chosen value; production obtains it from the MCP server's clock.
    pub now: &'a str,
}

/// Records one refutation batch: `corroborated`, `refuted` or `inconclusive`
/// for every severe finding this receipt's own inferential population owes
/// one, in a single write.
///
/// A `deterministic` severe finding already qualifies a rejection with no
/// confirmation owed, so it skips this pass entirely — the batch must neither
/// carry one nor be refused for omitting it. Completeness is asked only of
/// the `inferential` population: naming an id this receipt never recorded a
/// severe finding under, or naming a `deterministic` one, is refused before
/// anything is written, the same way an incomplete batch is.
pub fn record_refutation(
    context: &Context,
    refutation: &Refutation<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(refutation.operation_id))?;
    let outcomes: Vec<RefutationOutcome> =
        serde_json::from_str(refutation.outcomes).map_err(|_| {
            Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "invalid-refutation-batch", "value": refutation.outcomes,
                "action": "supply a JSON array of {\"id\": <finding id>, \"outcome\": \
                           <corroborated|refuted|inconclusive>} objects",
            }))
        })?;
    for entry in &outcomes {
        if entry.id.trim().is_empty() {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "invalid-refutation-batch", "value": refutation.outcomes,
                "action": "every entry names the finding id it adjudicates",
            })));
        }
        if !REFUTATION_OUTCOMES.contains(&entry.outcome.as_str()) {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "invalid-refutation-outcome",
                "id": entry.id, "value": entry.outcome, "expected": REFUTATION_OUTCOMES,
            })));
        }
    }
    {
        let mut seen = std::collections::BTreeSet::new();
        for entry in &outcomes {
            if !seen.insert(entry.id.as_str()) {
                return Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false, "reason": "refutation-batch-duplicate-finding", "id": entry.id,
                })));
            }
        }
    }

    let receipt = review_receipt(
        refutation.epoch,
        refutation.pr,
        refutation.head,
        refutation.base,
        refutation.digest,
    )?;
    let read = || timeline(context, refutation.issue, refutation.now);
    let pr_field = refutation.pr.to_string();
    let expected = [
        ("run-id", refutation.run_id),
        ("refuter", refutation.refuter),
        ("epoch", refutation.epoch),
        ("pr", &pr_field),
        ("head", refutation.head),
        ("base", refutation.base),
        ("digest", refutation.digest),
    ];
    let comments = read()?;
    reject_operation_kind_conflict(&comments, &operation_id, &["review-refutation"])?;
    if let Some(marker) = operation_marker(
        &comments,
        &operation_id,
        OperationKind::ReviewRefutation,
        &expected,
    )? {
        let persisted = ReviewRefutation::from_marker(&marker).ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "review-refutation-operation-conflict",
            }))
        })?;
        return Ok(serde_json::json!({
            "ok": true,
            "issue": refutation.issue,
            "refuter": persisted.refuter,
            "outcomes": persisted.outcomes,
            "outcomes_count": persisted.outcomes.len(),
            "reused_existing_refutation": true,
        }));
    }

    // Establishes both the receipt currency `findings_for` below relies on and
    // the distinctness rule in one call: the publishing run, and every run that
    // asked somebody else to review, cannot be credited with adjudicating their
    // own receipt's findings.
    //
    // Ahead of `verify_claim`, like `record_review_finding`'s own content
    // checks: a caller's batch is refused for what it says before it is asked
    // to prove it may write at all.
    require_distinct_reviewer(&comments, &receipt, refutation.refuter)?;

    if let Some(standing) = refutations_for(&comments, &receipt).first() {
        let sorted = |entries: &[RefutationOutcome]| {
            let mut entries = entries.to_vec();
            entries.sort_by(|a, b| a.id.cmp(&b.id));
            entries
        };
        if sorted(&standing.outcomes) == sorted(&outcomes) {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": refutation.issue,
                "refuter": standing.refuter,
                "outcomes": standing.outcomes,
                "outcomes_count": standing.outcomes.len(),
                "reused_existing_refutation": true,
            }));
        }
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "refutation-batch-already-recorded",
            "refuter": standing.refuter,
            "outcomes": standing.outcomes,
            "action": "the batch already on this receipt stands and no later batch overrides it; \
                       a corrected answer belongs to a new publication recorded with \
                       `republish_review`, against its own receipt",
        })));
    }

    let by_id = first_severe_findings(&comments, &receipt);
    let inferential = inferential_population(&by_id);

    let mut covered: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in &outcomes {
        let Some(finding) = by_id.get(&entry.id) else {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "refutation-unknown-finding",
                "id": entry.id,
                "recorded": by_id.keys().collect::<Vec<_>>(),
                "action": "name a severe finding this receipt actually recorded, listed above",
            })));
        };
        if finding.evidence_class.as_deref() == Some("deterministic") {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "refutation-includes-deterministic-finding",
                "id": entry.id,
                "action": "a deterministic severe finding already qualifies a rejection with no \
                           confirmation owed; it skips this pass and must not be named in the batch",
            })));
        }
        covered.insert(entry.id.clone());
    }
    let missing: Vec<&String> = inferential
        .into_iter()
        .filter(|id| !covered.contains(id.as_str()))
        .collect();
    if !missing.is_empty() {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "refutation-batch-incomplete",
            "missing": missing,
            "action": "name every standing inferential severe finding against this receipt \
                       exactly once — the identities still missing are listed above",
        })));
    }

    verify_claim(
        context,
        refutation.issue,
        refutation.run_id,
        "review",
        refutation.now,
        None,
    )?;

    let rendered_outcomes = serde_json::to_string(&outcomes).map_err(|_| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    let written: Vec<(&str, &str)> = vec![
        ("run-id", refutation.run_id),
        ("refuter", refutation.refuter),
        ("op-id", &operation_id),
        ("epoch", refutation.epoch),
        ("pr", &pr_field),
        ("head", refutation.head),
        ("base", refutation.base),
        ("digest", refutation.digest),
        ("outcomes", &rendered_outcomes),
    ];
    let marker = super::markers::render("review-refutation", &written).ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    let body = protocol_body(
        &format!(
            "Review refutation recorded by `{}` for publication epoch `{}`, covering {} \
             finding(s).",
            refutation.refuter,
            refutation.epoch,
            outcomes.len()
        ),
        &marker,
    );
    super::commands::record_entry(context, refutation.issue, &body, refutation.now)?;

    let mut seen = Vec::new();
    for _ in 0..VISIBILITY_ATTEMPTS {
        let observed = read().ok();
        let visible = observed.as_ref().is_some_and(|data| {
            operation_marker(
                data,
                &operation_id,
                OperationKind::ReviewRefutation,
                &expected,
            )
            .ok()
            .flatten()
            .is_some()
        });
        if visible {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": refutation.issue,
                "refuter": refutation.refuter,
                "outcomes": outcomes,
                "outcomes_count": outcomes.len(),
                "reused_existing_refutation": false,
            }));
        }
        seen.push(observed);
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false, "reason": "review-refutation-readback-failed", "world": "committed",
    })))
}

/// Everything one disposition batch records against one exact publication
/// receipt.
pub struct DispositionReview<'a> {
    /// The issue whose receipt is being answered.
    pub issue: u64,
    /// The run recording it, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// Stable across every retry of this batch.
    pub operation_id: &'a str,
    /// The publication epoch this batch answers findings against.
    pub epoch: &'a str,
    /// The pull request number.
    pub pr: u64,
    /// The full published head SHA.
    pub head: &'a str,
    /// The full published base SHA.
    pub base: &'a str,
    /// The complete-target manifest digest.
    pub digest: &'a str,
    /// The whole batch, one entry per identity the run chooses to answer, as a
    /// JSON array of `{"id": ..., "disposition": ..., "reason": ...,
    /// "filed_as"?: ...}` objects — see [`DispositionEntry`].
    pub dispositions: &'a str,
    /// The moment the live review claim is judged.
    ///
    /// Read from the machine, never from the run being judged. A test may pass
    /// a chosen value; production obtains it from the MCP server's clock.
    pub now: &'a str,
}

/// Records an optional disposition batch: `acted-on`, `filed` or `declined`
/// for any of the identities on the receipt that carry at least one `warning`
/// or `suggestion` recording, in a single write.
///
/// A non-severe finding is informational and blocks nothing, so **nothing owes
/// this record**: a run that records answers whichever findings it chooses and
/// may leave the rest unrecorded, and a finding left unrecorded is delivered
/// as informational by its class alone. What the call buys is a durable
/// account of the findings the run did act on or file. No distinct context is
/// required, because the run answering for the change is the only context that
/// can say what it did about a finding.
///
/// An identity whose recordings are all `severe` is adjudicated by the verdict
/// and never by a disposition: naming one is refused, and so is an id this
/// receipt recorded no finding under, a duplicate entry, a word outside the
/// three, a blank or filler reason, and a `filed` entry that names no issue. A
/// receipt this issue's timeline never published is refused on the same terms
/// [`record_spend`] refuses one — the record survives a later publication
/// because a finding acted on is only known to be so after the repair lands.
pub fn record_disposition(
    context: &Context,
    disposition: &DispositionReview<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(disposition.operation_id))?;
    let entries: Vec<DispositionEntry> =
        serde_json::from_str(disposition.dispositions).map_err(|_| {
            Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "invalid-disposition-batch",
                "value": disposition.dispositions,
                "action": "supply a JSON array of {\"id\": <finding id>, \"disposition\": \
                           <acted-on|filed|declined>, \"reason\": <text>} objects, with \
                           \"filed_as\": <issue number> on every `filed` entry",
            }))
        })?;
    for entry in &entries {
        if entry.id.trim().is_empty() {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "invalid-disposition-batch",
                "value": disposition.dispositions,
                "action": "every entry names the finding id it answers",
            })));
        }
        if !DISPOSITION_WORDS.contains(&entry.disposition.as_str()) {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "invalid-disposition-word",
                "id": entry.id, "value": entry.disposition, "expected": DISPOSITION_WORDS,
            })));
        }
        if says_nothing(&entry.reason) {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "blank-disposition-reason", "id": entry.id,
                "detail": "every disposition states why: what the repair was, the issue the \
                           finding became, or the judgement declining it. An empty, \
                           whitespace-only, or filler reason says none of them",
            })));
        }
        match (entry.disposition.as_str(), entry.filed_as) {
            ("filed", Some(issue)) if issue > 0 => {}
            ("filed", _) => {
                return Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false, "reason": "disposition-filed-issue-missing", "id": entry.id,
                    "action": "a `filed` entry carries \"filed_as\": <issue number>, the tracker \
                               issue the finding became",
                })));
            }
            (_, Some(_)) => {
                return Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false, "reason": "disposition-unexpected-filed-issue", "id": entry.id,
                    "action": "`filed_as` names the issue a `filed` finding became and belongs on \
                               no other word",
                })));
            }
            (_, None) => {}
        }
    }
    {
        let mut seen = std::collections::BTreeSet::new();
        for entry in &entries {
            if !seen.insert(entry.id.as_str()) {
                return Err(Failure::ConfigDefect(serde_json::json!({
                    "ok": false, "reason": "disposition-batch-duplicate-finding", "id": entry.id,
                })));
            }
        }
    }

    let receipt = review_receipt(
        disposition.epoch,
        disposition.pr,
        disposition.head,
        disposition.base,
        disposition.digest,
    )?;
    let read = || timeline(context, disposition.issue, disposition.now);
    let pr_field = disposition.pr.to_string();
    let expected = [
        ("run-id", disposition.run_id),
        ("epoch", disposition.epoch),
        ("pr", pr_field.as_str()),
        ("head", disposition.head),
        ("base", disposition.base),
        ("digest", disposition.digest),
    ];
    let comments = read()?;
    reject_operation_kind_conflict(&comments, &operation_id, &["review-disposition"])?;
    if let Some(marker) = operation_marker(
        &comments,
        &operation_id,
        OperationKind::ReviewDisposition,
        &expected,
    )? {
        let persisted = ReviewDisposition::from_marker(&marker).ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "review-disposition-operation-conflict",
            }))
        })?;
        return Ok(serde_json::json!({
            "ok": true,
            "issue": disposition.issue,
            "attester": persisted.attester,
            "dispositions": persisted.dispositions,
            "dispositions_count": persisted.dispositions.len(),
            "reused_existing_disposition": true,
        }));
    }

    // A historical receipt stays answerable — the same divergence
    // `record_spend` carries — because `acted-on` is only known after the
    // repair's later publication has landed, which is exactly when
    // `require_recorded_receipt` would refuse the finding's own answer. Nothing
    // reads this record as a gate, so answering an older receipt only widens
    // what may be recorded.
    if !known_publication(&comments, &receipt) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "disposition-transaction-unknown",
            "action": "name a receipt this issue's timeline actually published; a fabricated \
                       or mistyped epoch answers nothing",
        })));
    }

    let population = disposition_population(&comments, &receipt);
    let recorded: std::collections::BTreeMap<String, bool> = population.iter().cloned().collect();

    for entry in &entries {
        let Some(owed) = recorded.get(&entry.id) else {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "disposition-unknown-finding",
                "id": entry.id,
                "recorded": recorded.keys().collect::<Vec<_>>(),
                "action": "name a finding this receipt actually recorded, listed above — \
                           `review_findings` answers what stands",
            })));
        };
        if !owed {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "disposition-includes-severe-finding",
                "id": entry.id,
                "action": "a finding recorded only as severe is adjudicated by the verdict and \
                           never by a disposition; answer it by rejecting or accepting, and \
                           disposition the warnings and suggestions",
            })));
        }
    }

    verify_claim(
        context,
        disposition.issue,
        disposition.run_id,
        "review",
        disposition.now,
        None,
    )?;

    let rendered = serde_json::to_string(&entries).map_err(|_| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    let written: Vec<(&str, &str)> = vec![
        ("run-id", disposition.run_id),
        ("op-id", &operation_id),
        ("epoch", disposition.epoch),
        ("pr", &pr_field),
        ("head", disposition.head),
        ("base", disposition.base),
        ("digest", disposition.digest),
        ("dispositions", &rendered),
    ];
    let marker = super::markers::render("review-disposition", &written).ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    let body = protocol_body(
        &format!(
            "Review dispositions recorded by `{}` for publication epoch `{}`, answering {} \
             non-blocking finding(s).",
            disposition.run_id,
            disposition.epoch,
            entries.len()
        ),
        &marker,
    );
    super::commands::record_entry(context, disposition.issue, &body, disposition.now)?;

    let mut seen = Vec::new();
    for _ in 0..VISIBILITY_ATTEMPTS {
        let observed = read().ok();
        let visible = observed.as_ref().is_some_and(|data| {
            operation_marker(
                data,
                &operation_id,
                OperationKind::ReviewDisposition,
                &expected,
            )
            .is_ok_and(|found| found.is_some())
        });
        if visible {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": disposition.issue,
                "attester": disposition.run_id,
                "dispositions": entries,
                "dispositions_count": entries.len(),
                "reused_existing_disposition": false,
            }));
        }
        seen.push(observed);
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false, "reason": "review-disposition-readback-failed", "world": "committed",
    })))
}

/// Records one measurement of what a phase cost against a publication
/// receipt.
///
/// Follows [`record_review_finding`]'s shape — receipt validation, marker
/// composition, idempotent replay and readback — with two deliberate
/// divergences, both named where they diverge below:
///
/// - the receipt need not be the issue's *current* one, checked with
///   `known_publication` rather than `require_recorded_receipt`;
/// - `conditions` joins the receipt and the phase as this record's identity,
///   because two measurements that vary only the conditions are two records,
///   not a replay of one.
///
/// Nothing here refuses on the *number*: a caller-supplied `input`, `tokens`,
/// `seconds` or `contexts` is recorded, not verified. `docs/honesty.md` says
/// what that costs.
pub fn record_spend(
    context: &Context,
    spend: &SpendReview<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(spend.operation_id))?;
    if !SPEND_PHASES.contains(&spend.phase) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-spend-phase", "value": spend.phase,
            "expected": SPEND_PHASES,
        })));
    }
    let conditions = spend.conditions.trim();
    if conditions.is_empty() {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "blank-spend-conditions",
            "action": "name what this measurement varied \u{2014} \"sealed\", \"checkout \
                       budgeted\", \"payload only\" \u{2014} so a later measurement can be \
                       told apart from a repeat of this one rather than averaged into it",
        })));
    }
    // Recorded, not verified — like every other field a caller declares here.
    // What is checked is only that the four counts say *something*: a zero
    // is never a measurement, it is the shape of a field nobody filled in.
    for (name, value) in [
        ("input", spend.input),
        ("tokens", spend.tokens),
        ("seconds", spend.seconds),
        ("contexts", spend.contexts),
    ] {
        if value == 0 {
            return Err(Failure::ConfigDefect(serde_json::json!({
                "ok": false, "reason": "non-positive-spend-value", "field": name,
            })));
        }
    }
    let receipt = review_receipt(spend.epoch, spend.pr, spend.head, spend.base, spend.digest)?;
    let read = || timeline(context, spend.issue, spend.now);
    let pr_field = spend.pr.to_string();
    let input_field = spend.input.to_string();
    let tokens_field = spend.tokens.to_string();
    let seconds_field = spend.seconds.to_string();
    let contexts_field = spend.contexts.to_string();
    let expected = [
        ("run-id", spend.run_id),
        ("phase", spend.phase),
        ("epoch", spend.epoch),
        ("pr", pr_field.as_str()),
        ("head", spend.head),
        ("base", spend.base),
        ("digest", spend.digest),
        ("conditions", conditions),
    ];
    let comments = read()?;
    reject_operation_kind_conflict(&comments, &operation_id, &["spend"])?;
    // Diverges from `record_review_finding`'s `require_recorded_receipt`: a
    // spend record states a historical fact about a round that happened, not
    // a judgement about the round currently under review, so it must still
    // remain recordable once a later publication supersedes the receipt it
    // names. It still has to name a receipt this timeline actually published.
    if !known_publication(&comments, &receipt) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "spend-transaction-unknown",
            "action": "name a receipt this issue's timeline actually published; a fabricated \
                       or mistyped epoch settles nothing",
        })));
    }
    if let Some(marker) =
        operation_marker(&comments, &operation_id, OperationKind::Spend, &expected)?
    {
        let persisted = SpendRecord::from_marker(&marker).ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "spend-operation-conflict",
            }))
        })?;
        return Ok(serde_json::json!({
            "ok": true,
            "issue": spend.issue,
            "phase": persisted.phase,
            "conditions": persisted.conditions,
            "input": persisted.input,
            "tokens": persisted.tokens,
            "seconds": persisted.seconds,
            "contexts": persisted.contexts,
            "note": persisted.note,
            "reused_existing_spend": true,
        }));
    }

    verify_claim(
        context,
        spend.issue,
        spend.run_id,
        "review",
        spend.now,
        None,
    )?;

    let mut written: Vec<(&str, &str)> = vec![
        ("run-id", spend.run_id),
        ("op-id", &operation_id),
        ("phase", spend.phase),
        ("epoch", spend.epoch),
        ("pr", pr_field.as_str()),
        ("head", spend.head),
        ("base", spend.base),
        ("digest", spend.digest),
        ("conditions", conditions),
        ("input", input_field.as_str()),
        ("tokens", tokens_field.as_str()),
        ("seconds", seconds_field.as_str()),
        ("contexts", contexts_field.as_str()),
    ];
    let note = spend.note.trim();
    if !note.is_empty() {
        written.push(("note", note));
    }
    let marker = super::markers::render("spend", &written).ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    let body = protocol_body(
        &format!(
            "Spend recorded for phase `{}` of publication epoch `{}`.",
            spend.phase, spend.epoch
        ),
        &marker,
    );
    super::commands::record_entry(context, spend.issue, &body, spend.now)?;

    let mut seen = Vec::new();
    for _ in 0..VISIBILITY_ATTEMPTS {
        let observed = read().ok();
        let visible = observed.as_ref().is_some_and(|data| {
            operation_marker(data, &operation_id, OperationKind::Spend, &expected)
                .ok()
                .flatten()
                .is_some()
        });
        if visible {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": spend.issue,
                "phase": spend.phase,
                "conditions": conditions,
                "input": spend.input,
                "tokens": spend.tokens,
                "seconds": spend.seconds,
                "contexts": spend.contexts,
                "note": (!note.is_empty()).then_some(note),
                "reused_existing_spend": false,
            }));
        }
        seen.push(observed);
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false, "reason": "spend-readback-failed", "world": "committed",
    })))
}

/// What one issue's recorded spend answers: every measurement of the named
/// phase, side by side with its own conditions and receipt, so the same layer
/// measured twice can be compared. **Read-only**, like [`publication_lane`]
/// and [`super::target::expected_target`] — it takes no live claim, because a read is
/// not a write the gate sees.
///
/// `issue_totals` is present for spotting rounds — an issue with four
/// publications and three rejections is what it exists to make visible —
/// but it is not the comparison and nothing here divides one figure by
/// another to make it look like one. The comparison is `records`: entries
/// that share a phase and differ in `conditions`.
pub fn spend_summary(
    context: &Context,
    issue: u64,
    phase: &str,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    if !SPEND_PHASES.contains(&phase) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-spend-phase", "value": phase,
            "expected": SPEND_PHASES,
        })));
    }
    let comments = timeline(context, issue, now)?;
    let all = spend_records(&comments);

    let record_json = |record: &SpendRecord| {
        serde_json::json!({
            "epoch": record.receipt.epoch,
            "pr": record.receipt.pr,
            "head": record.receipt.head,
            "base": record.receipt.base,
            "digest": record.receipt.digest,
            "run_id": record.attester,
            "conditions": record.conditions,
            "input": record.input,
            "tokens": record.tokens,
            "seconds": record.seconds,
            "contexts": record.contexts,
            "note": record.note,
        })
    };
    let records: Vec<serde_json::Value> = all
        .iter()
        .filter(|record| record.phase == phase)
        .map(record_json)
        .collect();

    #[derive(Default, Clone)]
    struct Totals {
        input: u64,
        tokens: u64,
        seconds: u64,
        contexts: u64,
        records: u64,
    }
    impl Totals {
        fn add(&mut self, record: &SpendRecord) {
            self.input += record.input;
            self.tokens += record.tokens;
            self.seconds += record.seconds;
            self.contexts += record.contexts;
            self.records += 1;
        }
        fn json(&self) -> serde_json::Value {
            serde_json::json!({
                "input": self.input,
                "tokens": self.tokens,
                "seconds": self.seconds,
                "contexts": self.contexts,
                "records": self.records,
            })
        }
    }

    let mut grand = Totals::default();
    let mut by_phase: std::collections::BTreeMap<&str, Totals> = std::collections::BTreeMap::new();
    // A `Vec`, not a map: epochs are kept in the order this issue's timeline
    // first mentions them, which is publication order — a map would answer
    // the same rounds in a different order on every read.
    let mut by_epoch: Vec<(ReviewTransaction, Totals)> = Vec::new();
    for record in &all {
        grand.add(record);
        by_phase
            .entry(record.phase.as_str())
            .or_default()
            .add(record);
        match by_epoch
            .iter_mut()
            .find(|(receipt, _)| *receipt == record.receipt)
        {
            Some((_, totals)) => totals.add(record),
            None => {
                let mut totals = Totals::default();
                totals.add(record);
                by_epoch.push((record.receipt.clone(), totals));
            }
        }
    }

    Ok(serde_json::json!({
        "ok": true,
        "issue": issue,
        "phase": phase,
        "records": records,
        "record_count": records.len(),
        "issue_totals": {
            "input": grand.input,
            "tokens": grand.tokens,
            "seconds": grand.seconds,
            "contexts": grand.contexts,
            "records": grand.records,
            "by_phase": by_phase
                .into_iter()
                .map(|(name, totals)| (name.to_owned(), totals.json()))
                .collect::<serde_json::Map<String, serde_json::Value>>(),
            "by_epoch": by_epoch.into_iter().map(|(receipt, totals)| {
                let mut entry = totals.json();
                entry["epoch"] = serde_json::Value::String(receipt.epoch);
                entry["pr"] = serde_json::json!(receipt.pr);
                entry["head"] = serde_json::Value::String(receipt.head);
                entry["base"] = serde_json::Value::String(receipt.base);
                entry["digest"] = serde_json::Value::String(receipt.digest);
                entry
            }).collect::<Vec<_>>(),
        },
    }))
}

/// `repair_rounds` — whether the change under review may still be corrected.
///
/// **Read-only**, like [`publication_lane`] and
/// [`super::target::expected_target`] — no claim required, because a read is
/// not a write the gate sees. Answers where the lineage stands rather than
/// deciding anything: the refusal is `publish_review`'s.
///
/// Everything is scoped to **one pull request** — the one the issue's latest
/// publication names, which is the change under review. An issue outlives its
/// changes, so counting across the whole issue charges new work with a
/// delivered change's corrections. With no publication at all there is no
/// change, and the answer is zero throughout.
///
/// - **`rejections`** — publications on that pull request whose receipt stands
///   `rejected` or `escalated`, by `receipt_standing`.
/// - **`corrections`** — corrections the current lineage has spent, by
///   `corrections_spent`: publications over a parent that stands rejected or
///   escalated, since the lineage began.
/// - **`correction_limit`** — `Corrections per lineage`, by `corrections_per_lineage`.
/// - **`correction_spent`** — `corrections` has reached the limit. A second
///   correction is refused at publication, and `escalate_review` is open.
/// - **`correction_available`** — the newest publication stands rejected or
///   escalated and the limit is not reached: a correction may be published now.
pub fn repair_rounds(
    context: &Context,
    issue: u64,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let comments = timeline(context, issue, now)?;
    let publications = all_publications(&comments)?;
    let (rejections, corrections, newest) = match publications.last() {
        Some(latest) => {
            let pr = latest.receipt.pr;
            let mut rejected = 0;
            for publication in publications
                .iter()
                .filter(|publication| publication.receipt.pr == pr)
            {
                if receipt_standing(&comments, &publication.receipt)?.standing == Standing::Rejected
                {
                    rejected += 1;
                }
            }
            (
                rejected,
                corrections_spent(&comments, &publications, pr)?,
                receipt_standing(&comments, &latest.receipt)?.standing,
            )
        }
        None => (0, 0, Standing::Unjudged),
    };
    let limit = corrections_per_lineage(context);
    Ok(serde_json::json!({
        "ok": true,
        "issue": issue,
        "rejections": rejections,
        "corrections": corrections,
        "correction_limit": limit,
        "correction_spent": corrections >= limit,
        "correction_available": newest == Standing::Rejected && corrections < limit,
        "latest_verdict": newest.word(),
    }))
}

/// Byte ceiling on the summed `evidence` and `impact` text [`review_findings`]
/// answers under `--with-text`. Overflow is a refusal, never a truncation —
/// the same shape [`super::target::REVIEW_PAYLOAD_CEILING`] already holds for
/// a review payload, applied here because a ledger is divisible where a diff
/// is not: the caller can ask for one finding at a time with `--id`.
pub const FINDING_TEXT_CEILING: usize = 32 * 1024;

/// Every finding a reviewer recorded against one exact review transaction,
/// for a refuter adjudicating findings it was handed rather than performing
/// this repository's own re-review.
///
/// **Read-only**, like [`publication_lane`] and [`repair_rounds`] — no claim
/// required, because a read is not a write the gate sees, and no `run_id`
/// for the same reason.
///
/// `--epoch` resolves to a **full receipt read off the timeline**, never to
/// caller-supplied bytes: a publication this issue's timeline actually
/// recorded, or — failing that — the latest publication's own recorded
/// lineage parent, so a parent whose own `published` comment was deleted is
/// still answerable. Omitting `--epoch` answers the latest publication.
///
/// Findings are matched by the resolved receipt, never by epoch: a finding's
/// own `epoch` field is whatever the finding says it is — the reader this
/// walks already carries the warning for why matching on it alone is unsafe,
/// and selecting the *publication* by epoch before matching by receipt is the
/// safe direction.
///
/// Structured fields only by default; `with_text` adds `evidence` and
/// `impact` verbatim, checked against [`FINDING_TEXT_CEILING`] first —
/// overflow refuses rather than truncates, naming `--id` as the way to read
/// one finding at a time.
pub fn review_findings(
    context: &Context,
    issue: u64,
    epoch: Option<&str>,
    id: Option<&str>,
    with_text: bool,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    if let Some(value) = epoch {
        require_operation_id(Some(value))?;
    }
    let comments = timeline(context, issue, now)?;

    let latest = latest_publication(&comments)?.ok_or_else(|| {
        stop(
            "published-transaction-missing",
            "issue has no complete published receipt".to_owned(),
            "publish the review target first; there is nothing to answer findings against",
        )
    })?;
    let publications = all_publications(&comments)?;

    let (resolved, selected) = match epoch {
        None => (latest.receipt, "latest"),
        Some(value) => {
            if let Some(publication) = publications
                .iter()
                .find(|publication| publication.receipt.epoch == value)
            {
                (publication.receipt.clone(), "epoch")
            } else if let Some(parent) = latest
                .lineage
                .as_ref()
                .filter(|lineage| lineage.parent.epoch == value)
                .map(|lineage| lineage.parent.clone())
            {
                (parent, "epoch")
            } else {
                let mut recorded: Vec<String> = publications
                    .iter()
                    .map(|publication| publication.receipt.epoch.clone())
                    .collect();
                if let Some(lineage) = latest.lineage.as_ref()
                    && !recorded.contains(&lineage.parent.epoch)
                {
                    recorded.push(lineage.parent.epoch.clone());
                }
                return Err(Failure::Stop(serde_json::json!({
                    "ok": false,
                    "reason": "findings-transaction-unknown",
                    "recorded": recorded,
                    "action": "name an epoch this issue's timeline actually published, or the \
                               latest publication's own recorded parent \u{2014} both are \
                               listed above",
                })));
            }
        }
    };

    let parent_epoch = publications
        .iter()
        .find(|publication| publication.receipt == resolved)
        .and_then(|publication| publication.lineage.as_ref())
        .map(|lineage| lineage.parent.epoch.clone());

    let mut findings = findings_for(&comments, &resolved);
    if let Some(value) = id {
        let recorded: Vec<String> = findings.iter().map(|finding| finding.id.clone()).collect();
        findings.retain(|finding| finding.id == value);
        if findings.is_empty() {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "finding-id-unknown",
                "id": value,
                "recorded": recorded,
                "action": "name one of the identities recorded against this receipt, listed \
                           above, or omit --id to read every finding on it",
            })));
        }
    }

    let text_bytes: usize = findings
        .iter()
        .map(|finding| finding.evidence.len() + finding.impact.len())
        .sum();
    if with_text && text_bytes > FINDING_TEXT_CEILING {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "finding-text-too-large",
            "bytes": text_bytes,
            "ceiling": FINDING_TEXT_CEILING,
            "findings": findings.iter().map(|finding| finding.id.clone()).collect::<Vec<_>>(),
            "action": "read the same receipt without --with-text, then read one finding at a \
                       time with --id from that answer",
        })));
    }

    let finding_json = |finding: &ReviewFinding| {
        let mut entry = serde_json::json!({
            "operation_id": finding.operation_id,
            "attester": finding.attester,
            "reviewer": finding.reviewer,
            "receipt": {
                "epoch": finding.receipt.epoch,
                "pr": finding.receipt.pr,
                "head": finding.receipt.head,
                "base": finding.receipt.base,
                "digest": finding.receipt.digest,
            },
            "id": finding.id,
            "class": finding.class,
            "parent": finding.parent,
            "origin": finding.origin,
            "evidence_class": finding.evidence_class,
            "causal_disposition": finding.causal_disposition,
            "location": finding.location,
            "lens": finding.lens,
            "blocking": finding.is_severe(),
            "evidence_bytes": finding.evidence.len(),
            "impact_bytes": finding.impact.len(),
        });
        if with_text {
            entry["evidence"] = serde_json::Value::String(finding.evidence.clone());
            entry["impact"] = serde_json::Value::String(finding.impact.clone());
        }
        entry
    };

    Ok(serde_json::json!({
        "ok": true,
        "issue": issue,
        "receipt": {
            "epoch": resolved.epoch,
            "pr": resolved.pr,
            "head": resolved.head,
            "base": resolved.base,
            "digest": resolved.digest,
        },
        "selected": selected,
        "parent_epoch": parent_epoch,
        "findings": findings.iter().map(finding_json).collect::<Vec<_>>(),
        "findings_count": findings.len(),
        "text_bytes": text_bytes,
        "text_included": with_text,
    }))
}

/// The read-only look at the correction one publication made over its parent,
/// for a validator that is handed no checkout of its own.
///
/// Read-only, like [`review_transaction`] — no claim and no `run_id`. The
/// epoch resolves through `all_publications`, and the two commits read between
/// are the publication's own `head` and its recorded lineage parent's `head`,
/// never anything the caller supplies. A first publication or a recovered
/// successor refuses `inspect-parent-missing`: there is no correction to look
/// at. What [`crate::reviewer::inspect::inspect`] refuses with keeps its code,
/// and a code this does not name is `reviewer-inspect-unreadable`.
pub fn inspect_candidate(
    context: &Context,
    issue: u64,
    request: &InspectRequest<'_>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    use crate::reviewer::inspect::{Inspect, InspectReceipt, Side, inspect};

    require_operation_id(Some(request.epoch))?;
    let malformed = |detail: String| {
        Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "malformed-argument", "detail": detail,
        }))
    };
    let side = match request.side {
        None => None,
        Some("parent") => Some(Side::Parent),
        Some("candidate") => Some(Side::Candidate),
        Some(other) => {
            return Err(malformed(format!(
                "side must be `parent` or `candidate`, not {other:?}"
            )));
        }
    };
    let path = request.path.map(str::to_owned);
    let what = match (request.request, path, side) {
        ("name-status", None, None) => Inspect::NameStatus,
        ("numstat", None, None) => Inspect::Numstat,
        ("stat", None, None) => Inspect::Stat,
        ("patch", path, None) => Inspect::Patch { path },
        ("object", Some(path), Some(side)) => Inspect::Object { path, side },
        ("object", _, _) => {
            return Err(malformed(
                "an `object` request names both a `path` and a `side`".to_owned(),
            ));
        }
        ("name-status" | "numstat" | "stat" | "patch", _, _) => {
            return Err(malformed(format!(
                "a `{}` request takes {}",
                request.request,
                if request.request == "patch" {
                    "an optional `path` and no `side`"
                } else {
                    "neither `path` nor `side`"
                }
            )));
        }
        (other, _, _) => {
            return Err(malformed(format!(
                "request must be one of name-status, numstat, stat, patch or object, not {other:?}"
            )));
        }
    };

    let comments = timeline(context, issue, now)?;
    let publications = all_publications(&comments)?;
    let Some(publication) = publications
        .iter()
        .find(|publication| publication.receipt.epoch == request.epoch)
    else {
        let recorded: Vec<String> = publications
            .iter()
            .map(|publication| publication.receipt.epoch.clone())
            .collect();
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "publication-epoch-not-found",
            "recorded": recorded,
            "action": "name an epoch this issue's timeline actually published \u{2014} every \
                       epoch it recorded is listed above",
        })));
    };
    let Some(lineage) = publication.lineage.as_ref() else {
        return Err(stop(
            "inspect-parent-missing",
            format!(
                "publication {} has no recorded parent, so there is no correction to inspect",
                publication.receipt.epoch
            ),
            "inspect a publication that was published as a repair of an earlier one; a first \
             publication is reviewed from its payload, not inspected",
        ));
    };

    if lineage.recovered {
        return Err(stop(
            "inspect-parent-missing",
            format!(
                "publication {} is a recovered successor, a fresh candidate and not a correction \
                 of its recorded parent",
                publication.receipt.epoch
            ),
            "review a recovered successor from its payload; only a repair is inspected against \
             its parent",
        ));
    }

    let at = request.worktree.unwrap_or(&context.repo_dir);
    let parent_side = match validator::delta_reading(lineage) {
        validator::DeltaReading::Integrated => {
            validator::delta_parent(at, lineage, &publication.receipt.base)?
        }
        _ => lineage.parent.head.clone(),
    };
    let receipt = InspectReceipt {
        parent_head: &parent_side,
        head: &publication.receipt.head,
    };
    let text = inspect(at, &receipt, &what).map_err(inspection_refusal)?;

    Ok(serde_json::json!({
        "ok": true,
        "issue": issue,
        "epoch": publication.receipt.epoch,
        "request": request.request,
        "path": request.path,
        "side": request.side,
        "text": text,
    }))
}

fn inspection_refusal(refusal: crate::outcome::Refusal) -> Failure {
    let action = match &refusal.resolution {
        crate::outcome::Resolution::Run { command } => command.clone(),
        crate::outcome::Resolution::NoCommand { detail, .. } => detail.clone(),
    };
    let detail = refusal.message;
    match refusal.code {
        "reviewer-inspect-transaction-unresolved" => {
            stop("reviewer-inspect-transaction-unresolved", detail, &action)
        }
        "reviewer-inspect-outside-correction" => {
            stop("reviewer-inspect-outside-correction", detail, &action)
        }
        "reviewer-inspect-absent-side" => stop("reviewer-inspect-absent-side", detail, &action),
        "reviewer-input-too-large" => stop("reviewer-input-too-large", detail, &action),
        _ => stop("reviewer-inspect-unreadable", detail, &action),
    }
}

/// What [`inspect_candidate`] is asked, as the caller spelled it.
#[derive(Debug, Clone, Copy)]
pub struct InspectRequest<'a> {
    /// The publication epoch whose correction is inspected.
    pub epoch: &'a str,
    /// One of `name-status`, `numstat`, `stat`, `patch` or `object`.
    pub request: &'a str,
    /// A path the correction changed.
    pub path: Option<&'a str>,
    /// `parent` or `candidate`, for an `object` request.
    pub side: Option<&'a str>,
    /// The checkout to read in; the repository directory when `None`.
    pub worktree: Option<&'a std::path::Path>,
}

/// Whether a candidate reviewer-facing text is a genuine subset of the
/// payload one exact review transaction named — never whether it is the
/// whole of it. Withholding whole files to fit a sealed reviewer's limit is
/// legitimate and must stay legitimate; the question this answers is whether
/// every line the candidate kept is real, not whether every line was kept.
///
/// Read-only, like [`review_findings`] — no claim required, and no `run_id`,
/// because a read is not a write the gate sees. The receipt resolves the same
/// way [`review_findings`]'s does, through `all_publications`, never
/// through [`recorded_receipt`]: a copy checked after a republish has landed
/// is the ordinary case this exists for, and `recorded_receipt` demanding the
/// newest publication on the receipt's own pull request would refuse exactly then.
///
/// **In this order, and the order is the design.**
/// [`super::target::review_payload`] recomputes the payload from the resolved
/// receipt's `base` and `head`, through the same function `publish_review`
/// called rather than a second implementation of the diff. That recomputation
/// is reconciled against the marker's own recorded `payload-digest` before
/// anything about the candidate is asked. The two can disagree without the
/// candidate being wrong: both ends are object ids, so only a different
/// git version can still move it. So a mismatch
/// answers `payload-rebuild-mismatch` and never reports a line: naming one
/// would be answering the one question this cannot actually settle. A
/// receipt resolved with no recorded `payload-digest` at all —
/// written before the field existed, or reached only through an inherited
/// lineage parent with no marker of its own — answers
/// `payload-digest-unavailable` for the same reason: there is nothing
/// recorded to reconcile against.
///
/// Only once the digests agree is the candidate tested, and only for
/// membership: every non-empty line of `candidate` must be a line somewhere
/// in the payload. The first one that is not answers `payload-line-not-found`,
/// naming its 1-based position in `candidate`; `omitted` counts the payload's
/// own distinct non-empty lines that never appear in `candidate`, so a caller
/// can state its own subset honestly. Membership checks presence only — never
/// order, multiplicity or which file a line came from — so a candidate
/// assembled entirely from real lines can still misrepresent the change and
/// answer `ok: true`.
pub fn review_payload_membership(
    context: &Context,
    issue: u64,
    epoch: Option<&str>,
    candidate: &std::path::Path,
    worktree: Option<&std::path::Path>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    if let Some(value) = epoch {
        require_operation_id(Some(value))?;
    }
    let comments = timeline(context, issue, now)?;

    let latest = latest_publication(&comments)?.ok_or_else(|| {
        stop(
            "published-transaction-missing",
            "issue has no complete published receipt".to_owned(),
            "publish the review target first; there is nothing to cross a candidate text \
             against",
        )
    })?;
    let publications = all_publications(&comments)?;

    let (resolved, payload_digest, selected) = match epoch {
        None => (latest.receipt, latest.payload_digest, "latest"),
        Some(value) => {
            if let Some(publication) = publications
                .iter()
                .find(|publication| publication.receipt.epoch == value)
            {
                (
                    publication.receipt.clone(),
                    publication.payload_digest.clone(),
                    "epoch",
                )
            } else if let Some(parent) = latest
                .lineage
                .as_ref()
                .filter(|lineage| lineage.parent.epoch == value)
                .map(|lineage| lineage.parent.clone())
            {
                (parent, None, "epoch")
            } else {
                let mut recorded: Vec<String> = publications
                    .iter()
                    .map(|publication| publication.receipt.epoch.clone())
                    .collect();
                if let Some(lineage) = latest.lineage.as_ref()
                    && !recorded.contains(&lineage.parent.epoch)
                {
                    recorded.push(lineage.parent.epoch.clone());
                }
                return Err(Failure::Stop(serde_json::json!({
                    "ok": false,
                    "reason": "payload-membership-transaction-unknown",
                    "recorded": recorded,
                    "action": "name an epoch this issue's timeline actually published, or the \
                               latest publication's own recorded parent \u{2014} both are \
                               listed above",
                })));
            }
        }
    };

    let receipt_json = serde_json::json!({
        "epoch": resolved.epoch,
        "pr": resolved.pr,
        "head": resolved.head,
        "base": resolved.base,
        "digest": resolved.digest,
    });

    let Some(recorded_payload_digest) = payload_digest else {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "payload-digest-unavailable",
            "issue": issue,
            "receipt": receipt_json,
            "selected": selected,
            "action": "this receipt's marker carries no recorded `payload-digest` \u{2014} \
                       written before the field existed, or resolved only through an inherited \
                       lineage parent with no marker of its own \u{2014} so there is nothing \
                       recorded to cross the candidate text against",
        })));
    };

    let at = worktree.unwrap_or(&context.repo_dir);
    let payload = super::target::review_payload(at, &resolved.base, &resolved.head)?;

    if payload.digest != recorded_payload_digest {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "payload-rebuild-mismatch",
            "issue": issue,
            "receipt": receipt_json,
            "selected": selected,
            "recorded_payload_digest": recorded_payload_digest,
            "rebuilt_payload_digest": payload.digest,
            "action": "the payload this receipt names could not be rebuilt here \u{2014} the \
                       rebuild runs isolated and both ends are object ids this checkout \
                       already resolved, so only a different git version can still disagree \
                       without the candidate being wrong. This reports no line, and there is \
                       nothing further this operation can determine",
        })));
    }

    let candidate_text = std::fs::read_to_string(candidate)
        .map_err(|error| Failure::Read(format!("the candidate text could not be read: {error}")))?;

    let payload_lines: std::collections::BTreeSet<&str> = payload
        .text
        .lines()
        .filter(|line| !line.is_empty())
        .collect();

    let mut offense: Option<(usize, &str)> = None;
    for (index, line) in candidate_text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        if !payload_lines.contains(line) {
            offense = Some((index + 1, line));
            break;
        }
    }
    if let Some((line, text)) = offense {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "payload-line-not-found",
            "issue": issue,
            "receipt": receipt_json,
            "selected": selected,
            "line": line,
            "text": text,
            "action": "this line of the candidate text is not a line of the recomputed payload \
                       \u{2014} the candidate was not drawn entirely from what this receipt \
                       published, or was altered after being drawn from it",
        })));
    }

    let candidate_lines: std::collections::BTreeSet<&str> = candidate_text
        .lines()
        .filter(|line| !line.is_empty())
        .collect();
    let omitted = payload_lines
        .iter()
        .filter(|line| !candidate_lines.contains(*line))
        .count();

    Ok(serde_json::json!({
        "ok": true,
        "issue": issue,
        "receipt": receipt_json,
        "selected": selected,
        "payload_digest": payload.digest,
        "payload_lines": payload_lines.len(),
        "candidate_lines": candidate_lines.len(),
        "omitted": omitted,
    }))
}

/// The receipt currency and distinctness a refutation must satisfy.
///
/// The publishing run, and every run that asked somebody else to review, cannot
/// be credited with adjudicating their own receipt's findings.
fn require_distinct_reviewer(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
    reviewer: &str,
) -> Result<(), Failure> {
    let publication = require_recorded_receipt(
        comments,
        receipt,
        "cannot be credited with a verdict or a refutation",
    )?;
    let requesters: Vec<String> = handoffs_for(comments, receipt)
        .into_iter()
        .map(|handoff| handoff.requester)
        .collect();
    if publication.publisher == reviewer || requesters.iter().any(|run| run == reviewer) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "reviewer-not-distinct",
            "publisher": publication.publisher,
            "requesters": requesters,
            "action": "the publishing run, and every run that asked somebody else to review, cannot be credited with reviewing their own receipt",
        })));
    }
    Ok(())
}

/// The `approved` receipt marker for one computed verdict.
///
/// The marker is a **local record**: it is appended to the store and never
/// posted to GitHub, so a receipt lives where the workflow state does. See
/// `docs/honesty.md` for what a local record does and does not prove.
fn approved_receipt_marker(
    run_id: &str,
    reviewer: &str,
    transaction: &ReviewTransaction,
) -> Result<String, Failure> {
    let pr_field = transaction.pr.to_string();
    super::markers::render(
        "review-receipt",
        &[
            ("run-id", run_id),
            ("epoch", &transaction.epoch),
            ("pr", &pr_field),
            ("head", &transaction.head),
            ("base", &transaction.base),
            ("digest", &transaction.digest),
            ("terminal-state", "approved"),
            ("reviewer", reviewer),
        ],
    )
    .ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })
}

/// Records an `approved` receipt locally, and nothing on GitHub.
///
/// The approval is what the receipt closes, so this is written after the
/// verdict is confirmed. A missing receipt is recoverable: replaying the
/// verdict writes it, and nothing but an escalation depends on it, so a
/// receipt with no verdict releases nothing.
fn record_approved_receipt(
    context: &Context,
    issue: u64,
    run_id: &str,
    reviewer: &str,
    transaction: &ReviewTransaction,
    now: &str,
) -> Result<(), Failure> {
    let marker = approved_receipt_marker(run_id, reviewer, transaction)?;
    let body = format!("{marker}\n");
    Ledger::open(context, issue, now)?.append(&body, now)
}

/// A local receipt write that failed after the verdict it closes was already
/// recorded and confirmed.
///
/// The tracker write landed, so the world changed and the failure is reported as
/// one rather than as a read: the answer names the replay that records the
/// missing receipt, because a caller told only about the store step would take
/// the verdict itself for failed.
fn receipt_write_failed(failure: Failure) -> Failure {
    Failure::Write(format!(
        "the computed verdict is recorded and confirmed, but its `approved` receipt could not be \
         written to the local store: {}; replay the same operation id to record the receipt",
        failure.detail()
    ))
}

/// Whether an `approved` receipt already stands for this transaction.
fn approved_receipt_stands(
    comments: &[ownership::Comment],
    transaction: &ReviewTransaction,
) -> bool {
    receipts_for(comments, transaction)
        .iter()
        .any(|standing| standing.terminal_state == "approved")
}

/// The refusal every attempt to record an aggregate verdict receives.
///
/// The verdict is computed from the receipt's ledger and recorded by
/// [`computed::record_computed_verdict`]; judges record findings only. Old
/// `review-verdict` markers still parse and are read where a round already
/// spent has to stay counted. The receipt the caller named is echoed so the
/// replacement call can be made from this answer, and `ignored` names the
/// arguments of the retired call that decided nothing.
pub fn retired_review_verdict(receipt: Option<&ReviewTransaction>, ignored: &[&str]) -> Failure {
    let mut envelope = serde_json::json!({
        "ok": false,
        "reason": "review-verdict-retired",
        "ignored": ignored,
        "action": "judges record findings only; call `launch_review_input` and then `collect_review_input` for every lens slot \
                   `review_status` offers, then `record_computed_verdict` for the exact receipt",
    });
    if let Some(receipt) = receipt {
        envelope["receipt"] = serde_json::json!({
            "epoch": receipt.epoch,
            "pr": receipt.pr,
            "head": receipt.head,
            "base": receipt.base,
            "digest": receipt.digest,
        });
    }
    Failure::Stop(envelope)
}

/// The refusal every hand call to a record only the capture writes receives.
///
/// A lens report, a refutation batch and a validator result are written by
/// [`collect_review_input`] after the binary ran the reviewer; a hand call would record work
/// nobody did. `operation` is the transport operation the caller used, echoed so the answer names
/// what was refused.
pub fn recorded_by_capture_only(operation: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "recorded-by-capture-only",
        "operation": operation,
        "action": "the binary launches this reviewer and records its result itself; call \
                   `review_status` for the slot it offers, then `launch_review_input` and then `collect_review_input` with that \
                   slot's name",
    }))
}

/// Everything an escalation binds before it records that the decision passed to
/// a person.
#[derive(Debug, Clone)]
pub struct ReviewEscalation<'a> {
    /// The issue under review.
    pub issue: u64,
    /// The run holding the live `review` claim.
    pub run_id: &'a str,
    /// The review transaction being escalated.
    pub epoch: &'a str,
    /// The pull request.
    pub pr: u64,
    /// The transaction's published head.
    pub head: &'a str,
    /// The transaction's recorded base.
    pub base: &'a str,
    /// The transaction's complete-target digest.
    pub digest: &'a str,
    /// The person who takes the decision.
    pub escalated_to: &'a str,
    /// Why the decision passed to them.
    pub reason: &'a str,
    /// The moment the timeline is judged against.
    pub now: &'a str,
}

/// Records that a review transaction's decision passed to a named person.
///
/// The escalation is the exit a spent correction forces: a change whose one
/// correction is used stays in `review` and a person decides, and this
/// operation is what puts that decision on the record instead of leaving it in
/// prose. It does
/// **not** release CI — an `escalated` receipt is refused by `release_ci`
/// exactly as an escalation cannot substitute for an approved computed verdict.
///
/// An approval is final: a transaction that already carries an `approved`
/// receipt is refused, because an escalation cannot overwrite the approval
/// that is the only basis which releases CI, and a shadowed approval would
/// make `release_ci` refuse the change.
///
/// The `escalated` receipt is a **local record**: it is appended to the store
/// and never posted to GitHub, so this operation writes no tracking comment.
pub fn escalate_review(
    context: &Context,
    escalation: &ReviewEscalation<'_>,
) -> Result<serde_json::Value, Failure> {
    if escalation.escalated_to.trim().is_empty() {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-escalate-target",
            "action": "name the person who takes the decision; an empty target is nobody",
        })));
    }
    if says_nothing(escalation.reason) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "blank-escalate-reason",
            "action": "state the condition the decision is handed over with — the correction \
                       that was spent, or the finding the ledger could not settle",
        })));
    }
    let transaction = review_receipt(
        escalation.epoch,
        escalation.pr,
        escalation.head,
        escalation.base,
        escalation.digest,
    )?;
    let ledger = Ledger::open(context, escalation.issue, escalation.now)?;
    let comments = ledger.timeline()?;
    verify_claim(
        context,
        escalation.issue,
        escalation.run_id,
        "review",
        escalation.now,
        None,
    )?;
    // A terminal receipt is final: a transaction reaches one terminal state, and
    // an escalation cannot overwrite the `approved` one an acceptance recorded.
    // The standing state is read by transaction identity, so a receipt that no
    // longer bounds the newest publication still governs its own transaction.
    if let Some(standing) = receipts_for(&comments, &transaction).into_iter().last() {
        if standing.terminal_state == "escalated" {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": escalation.issue,
                "epoch": standing.transaction.epoch,
                "pr": standing.transaction.pr,
                "terminal_state": standing.terminal_state,
                "escalated_to": standing.escalated_to,
                "reason_bytes": standing.reason.as_deref().map(str::len).unwrap_or(0),
                "reused_existing_escalation": true,
            }));
        }
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "escalate-review-already-approved",
            "terminal_state": standing.terminal_state,
            "action": "the transaction already reached `approved`; an approval is final, and an \
                       escalation cannot substitute for the approved computed verdict that is \
                       the only basis which releases CI",
        })));
    }
    require_recorded_receipt(&comments, &transaction, "cannot escalate")?;
    // A spent correction is what authorises the escalation, so it is read rather
    // than assumed: a decision handed to a person before the corrections are
    // used is a route around the repair, not the exit from it.
    let rounds = repair_rounds(context, escalation.issue, escalation.now)?;
    if rounds
        .get("correction_spent")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "escalate-review-correction-unspent",
            "rounds": rounds,
            "action": "publish the next correction, or record the computed verdict; an escalation \
                       records a decision the spent correction forced, and this lineage has not \
                       spent it",
        })));
    }
    record_escalated_receipt(
        &ledger,
        escalation.run_id,
        &transaction,
        escalation.escalated_to,
        escalation.reason,
        escalation.now,
    )?;
    Ok(serde_json::json!({
        "ok": true,
        "issue": escalation.issue,
        "epoch": escalation.epoch,
        "pr": escalation.pr,
        "terminal_state": "escalated",
        "escalated_to": escalation.escalated_to,
        "reason_bytes": escalation.reason.len(),
        "reused_existing_escalation": false,
    }))
}

fn record_escalated_receipt(
    ledger: &Ledger,
    run_id: &str,
    transaction: &ReviewTransaction,
    escalated_to: &str,
    reason: &str,
    now: &str,
) -> Result<(), Failure> {
    let pr_field = transaction.pr.to_string();
    let marker = super::markers::render(
        "review-receipt",
        &[
            ("run-id", run_id),
            ("epoch", &transaction.epoch),
            ("pr", &pr_field),
            ("head", &transaction.head),
            ("base", &transaction.base),
            ("digest", &transaction.digest),
            ("terminal-state", "escalated"),
            ("escalated-to", escalated_to),
            ("reason", reason),
        ],
    )
    .ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    ledger.append(&format!("{marker}\n"), now)
}

fn ready_outcome(
    write: Result<(), Failure>,
    read: Result<serde_json::Value, Failure>,
    receipt: &ReviewTransaction,
) -> Result<(), Failure> {
    match read {
        Ok(ready) if require_pr_matches(&ready, receipt, false).is_ok() => Ok(()),
        Ok(ready) => {
            if let Err(failure) = write {
                return Err(match failure {
                    Failure::Read(detail) => Failure::Read(format!(
                        "{detail}; readback proved the PR did not become the exact ready target: {ready}"
                    )),
                    other => Failure::Write(format!(
                        "{}; readback did not show the exact PR ready: {ready}",
                        other.detail()
                    )),
                });
            }
            require_pr_matches(&ready, receipt, false)
        }
        Err(read_failure) => {
            let detail = match write {
                Ok(()) => format!(
                    "gh pr ready returned success, but its exact ready state could not be read back: {}",
                    read_failure.detail()
                ),
                Err(Failure::Read(write_detail)) => {
                    return Err(Failure::Read(format!(
                        "{write_detail}; the follow-up read also failed: {}",
                        read_failure.detail()
                    )));
                }
                Err(write_failure) => format!(
                    "{}; the exact ready outcome could not be read back: {}",
                    write_failure.detail(),
                    read_failure.detail()
                ),
            };
            Err(Failure::Write(detail))
        }
    }
}

/// `publish-review` — establish draft isolation, then push and publish a receipt.
///
/// A resumed issue **reuses its existing open PR**; creating a duplicate is not
/// recovery. And a reused PR keeps the title and body it was created with unless
/// they are replaced — so a resumed delivery whose scope changed would advertise
/// the old description over the new head, and the reviewer would read one change
/// and approve another.
///
/// It does **not** transition. `transition --to review` comes after, so `review`
/// points at a head the reviewer and CI can actually judge.
#[derive(Debug, Clone)]
pub struct Publication<'a> {
    /// The issue being delivered.
    pub issue: u64,
    /// The run that holds it.
    pub run_id: &'a str,
    /// The state the renewal expects to find.
    pub expect_state: &'a str,
    /// The base branch.
    pub base: &'a str,
    /// The branch carrying the work.
    pub branch: &'a str,
    /// The title to apply, on a new PR or a reused one.
    pub title: &'a str,
    /// The body file. **Required for a new PR**, optional for a reused one.
    pub pr_body_file: Option<&'a std::path::Path>,
    /// The isolated checkout to push from, when this run was given one.
    ///
    /// The original takes it — `worktree = Path(args.worktree) if args.worktree
    /// else cwd`, and the push runs there — and the port had no field for it,
    /// so the dispatcher parsed `--worktree` into a map nobody read and pushed
    /// from the base checkout every time. The tool declares the argument to the
    /// agent, which makes it a value asked for and discarded.
    ///
    /// It happens to reach the same refs when the checkout is a git worktree,
    /// which is what `start-branch` makes: measured, a `git push -u origin --
    /// fix/12` from the base carried the worktree's commit exactly, because a
    /// worktree shares the object store and the refs. That is why nothing broke
    /// and why nothing noticed. It stops being true the moment the configured
    /// location is a separate clone, and *the argument the caller sent decides
    /// nothing* is the defect either way.
    pub worktree: Option<&'a std::path::Path>,
    /// Whether the answer carries the payload's full text. `payload_bytes` and
    /// `payload_digest` are always present; this decides only the text itself,
    /// which is what can make an answer too large for the MCP surface to
    /// return. Measured on this repository, the threshold sits inside the
    /// ordinary range rather than at the large end: answers carrying 73,272
    /// and 145,756 payload bytes were refused, one carrying 36,288 was not
    pub inline_payload: bool,
    /// The execution topology this run declares for this delivery — free text
    /// against `SKILL.md`'s Direct Work Delegation table (`inline`,
    /// `developer:3`, `analyst`, …). **Recorded, not bound**: it carries
    /// none of the four properties [`ReviewTransaction`] holds — most of all the
    /// fourth, since Estigia cannot observe how an agent wrote code — so it
    /// follows the `payload-digest` precedent (written where the marker is
    /// rendered, read back by nothing) rather than becoming a fifth required
    /// field there.
    pub topology: Option<&'a str>,
    /// Why this delivery declared the topology it did. Recorded beside
    /// `topology`; given without a topology it is refused. Nothing reads it back to decide.
    pub topology_reason: Option<&'a str>,
    /// The lens focus this delivery asks for; recorded with `declaration`, and
    /// refused when it is not one of [`crate::transport::tier::REVIEW_FOCUSES`].
    pub focus: Option<&'a str>,
    /// The raise and the reasons declared beside the focus, validated by the
    /// same rule `review_tier` applies and recorded on the marker.
    pub declaration: super::tier::Declaration<'a>,
    /// The epoch the caller believes is the newest publication on this branch's
    /// pull request, or `none` for a branch with no publication yet. `None`
    /// asks nothing. Any other newest publication refuses the call before
    /// anything is pushed.
    pub expected_epoch: Option<&'a str>,
    /// The moment the timeline is judged against.
    ///
    /// **Read from the machine, never from the run being judged.** The binding
    /// takes it from `utc_now_stamp()` for a reason that is not tidiness: this
    /// value decides whether a claim is still live, so a run that supplies it
    /// decides whether its own claim has expired. The port accepts it as an
    /// argument because a test has to be able to stand at a chosen moment; that
    /// is the only caller allowed to choose one.
    pub now: &'a str,
}

/// How the branch reaches the remote.
///
/// The **only** difference between `publish-review` and `republish-review`, and
/// it is an enum rather than a boolean argument for the reason the second entry
/// point exists at all: a run cannot arrive at the destroying push by leaving a
/// flag at its default, mistyping one, or copying a call from a neighbouring
/// site. `publish_review` constructs [`Push::FastForward`] and has no way to
/// name the other variant.
#[derive(Debug, Clone, Copy)]
pub(super) enum Push<'a> {
    /// The ordinary publication. The remote refuses anything that is not a
    /// fast-forward, and that refusal is the whole safety of this path.
    FastForward,
    /// The republication of a branch whose history was rewritten — a rebase
    /// onto a base that moved, or an amended commit.
    ///
    /// `recorded_head` is the head the **latest `published` marker on this
    /// issue's timeline** carries, and the lease is taken against exactly that.
    /// Not the bare `--force-with-lease`, whose implicit expectation is the
    /// remote-tracking ref: any fetch — including the one this operation itself
    /// runs against the base — can refresh that ref, and a lease against a
    /// value the run has only just learned protects nothing. The recorded head
    /// is a value a human reviewed and a receipt bound, which is the only
    /// expectation worth leasing against.
    Leased { recorded_head: &'a str },
}

/// See [`Publication`].
///
/// This is the fast-forward entry point, and the last of the issue's acceptance
/// criteria is that it stay one: it never force-pushes, under any argument.
/// [`republish_review`] is the other one.
pub fn publish_review(
    context: &Context,
    what: &Publication<'_>,
) -> Result<serde_json::Value, Failure> {
    publish_with(context, what, Push::FastForward)
}

/// Whether a publication of `branch` would be a correction: a recorded
/// receipt of this issue stands on the branch's open pull request and has not
/// been recovered into a fresh lineage.
fn publishes_a_correction(
    context: &Context,
    branch: &str,
    issue: u64,
    now: &str,
) -> Result<bool, Failure> {
    let existing = open_prs(context, branch)?;
    let [pr] = existing.as_slice() else {
        return Ok(false);
    };
    let comments = timeline(context, issue, now)?;
    let Some(here) = publication_on(&comments, Some(pr_number(pr)))? else {
        return Ok(false);
    };
    Ok(!recover::recovered(&comments, &here.receipt))
}

/// `republish-review` — publish a rewritten branch over its own last publication.
///
/// A hand-run `git push --force-with-lease` after a rebase verifies the claim
/// but lacks the rest: no lease against the head a receipt recorded, so the
/// push overwrites whatever the remote held; no record tying the new bytes to
/// that receipt; and no coverage at all for a push typed into a terminal the
/// gate does not watch, which is what the pre-push hook exists for and what
/// `estigia doctor` reports when it is not installed.
///
/// What it adds over [`publish_review`] is the lease and one more renewal. What
/// it deliberately does not add is a plain `--force`: without a lease this is a
/// write that cannot be refused, and an operation that cannot be refused is not
/// a gate.
pub fn republish_review(
    context: &Context,
    what: &Publication<'_>,
) -> Result<serde_json::Value, Failure> {
    // No recorded publication is not *nothing to protect*, it is a caller in the
    // wrong operation. A first publication is `publish-review`, whose refusal on
    // a non-fast-forward is meaningful; forcing over a branch this issue never
    // published means overwriting somebody else's work with no expectation to
    // check it against, which is the plain `--force` this issue rules out
    // wearing a different name.
    // The lease is against the head recorded for **this branch's** pull request,
    // not the issue's newest: a chained delivery publishes several pull requests
    // under one issue, and leasing slice 1's branch against slice 2's head would
    // refuse a republish the contract permits.
    let comments = timeline(context, what.issue, what.now)?;
    let latest = match open_prs(context, what.branch)?.first().map(pr_number) {
        Some(pr) => latest_publication_for_pr(&comments, pr)?,
        None => latest_publication(&comments)?,
    };
    let Some(latest) = latest else {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "published-transaction-missing",
            "issue": what.issue,
            "action": "there is no recorded publication on this issue to lease against \u{2014} \
                       publish-review makes the first one, and its refusal on a non-fast-forward \
                       is the check that would be skipped here",
        })));
    };
    // Which *run* published it is not asked, and that is deliberate rather than
    // overlooked. Authority here is the live claim, which `publish_with` verifies
    // twice; a run that reclaimed an abandoned issue is entitled to republish the
    // head the previous holder left, and requiring self-attribution would strand
    // exactly the reclaim the workflow provides for.
    //
    // Which *account* published it is asked, one level down and not by choice:
    // `latest_publication` keeps only comments `viewer_did_author` vouches for,
    // because a marker this identity did not write is a marker anyone could have
    // forged. So a reclaim across two GitHub accounts finds no receipt and is
    // refused below — fail-closed, and a real limit rather than the unqualified
    // *whose publication it was is not asked* this comment used to claim.
    // `docs/honesty.md` carries it.
    publish_with(
        context,
        what,
        Push::Leased {
            recorded_head: &latest.receipt.head,
        },
    )
}

/// Read the pull request back until it carries the pushed head and base.
///
/// Attempts are spaced through `sleep`: the first reads immediately, each
/// later one waits its scheduled delay, so ten attempts cover the window
/// [`PUBLISH_READBACK_RETRY_BASE_SECS`] states rather than the time ten
/// round trips take. Returns every observation for the refusal to report.
fn read_back_pr(
    context: &Context,
    number: u64,
    local_head: &str,
    expected_base: &str,
    sleep: &mut dyn FnMut(std::time::Duration),
) -> Result<Vec<serde_json::Value>, Failure> {
    let mut seen = Vec::new();
    for upcoming in 1..=PUBLISH_READBACK_ATTEMPTS {
        if upcoming > 1 {
            sleep(std::time::Duration::from_secs(readback_delay_secs(
                upcoming,
            )));
        }
        if let Some(value) = super::gh_json(
            &[
                "pr",
                "view",
                &number.to_string(),
                "--json",
                "number,url,headRefOid,baseRefOid,state,isDraft",
            ],
            Some(&context.repo_dir),
        )? {
            seen.push(value);
        }
        if confirm_published(&seen, local_head, expected_base, true).is_some() {
            break;
        }
    }
    Ok(seen)
}

/// Which of the two worlds a failed read-back stands in.
///
/// Read from the branch ref, never from the pull request object: when the ref
/// already carries the pushed head the remote has not caught up; when it
/// carries another the head moved. A ref that cannot be read answers neither,
/// and the refusal says it cannot tell rather than picking one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadbackWorld {
    /// The branch ref carries the pushed head; only the pull request lags.
    Lag,
    /// The branch ref carries another head; somebody else pushed.
    Moved,
    /// The ref did not answer; lag cannot be told from a moved head.
    Unknown,
}

/// Classify a failed read-back from the branch ref head beside the pushed one.
fn classify_readback(branch_ref_head: Option<&str>, local_head: &str) -> ReadbackWorld {
    match branch_ref_head {
        Some(head) if head == local_head => ReadbackWorld::Lag,
        Some(_) => ReadbackWorld::Moved,
        None => ReadbackWorld::Unknown,
    }
}

/// The world-specific lead of the read-back refusal: what happened, and
/// whether to wait or to investigate. Pure so all three worlds are pinned
/// without a remote; the recovery guidance is composed after it, unchanged.
fn readback_world_lead(world: ReadbackWorld, local_head: &str, number: u64) -> String {
    match world {
        ReadbackWorld::Lag => format!(
            "The branch ref already carries `{local_head}` while pull request {number} does \
             not — the remote has not caught up with the push, so wait for it to settle and \
             run republish_review against the unconfirmed head rather than investigating a \
             push nobody made."
        ),
        ReadbackWorld::Moved => format!(
            "The branch ref no longer carries `{local_head}` — the head moved while \
             publishing, so somebody else may have pushed."
        ),
        ReadbackWorld::Unknown => {
            "The branch ref could not be read, so lag cannot be told from a moved head — \
             treat this as a moved head until a fresh read of the ref says otherwise."
                .to_string()
        }
    }
}

/// The head the remote branch ref carries, or a failed read.
///
/// Only ever read on the read-back failure path, where one extra call is the
/// difference between waiting and investigating. Any failure — identity,
/// transport, or a ref naming no head — is a read the caller reports as
/// unknown rather than a second refusal stacked behind the first.
fn remote_branch_head(context: &Context, branch: &str) -> Result<String, Failure> {
    let (owner, name) = super::closing::repo_identity(context)?;
    super::gh_json(
        &[
            "api",
            &format!("repos/{owner}/{name}/git/refs/heads/{branch}"),
        ],
        Some(&context.repo_dir),
    )?
    .and_then(|data| {
        data.get("object")?
            .get("sha")?
            .as_str()
            .filter(|sha| !sha.is_empty())
            .map(str::to_owned)
    })
    .ok_or_else(|| Failure::Read(format!("the branch ref for {branch} did not answer")))
}

/// The declared execution topology, trimmed and checked at the boundary.
///
/// One decision for all three surfaces: the trimmed values are what the
/// marker records and the answer echoes, so they
/// cannot diverge by construction. A blank declaration is refused rather than
/// recorded, the way `--basis` is — a caller that sent nothing and a caller
/// that sent emptiness said different things. A reason with no topology is
/// incomplete on its own and is refused rather than kept where only some
/// surfaces would show it. Case is preserved as typed: with no consumer
/// reading the field back, folding it would lose information for nothing.
fn normalize_topology<'a>(
    topology: Option<&'a str>,
    topology_reason: Option<&'a str>,
) -> Result<(Option<&'a str>, Option<&'a str>), Failure> {
    if topology.is_some_and(says_nothing) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "blank-topology",
            "action": "declare how the change was written, or omit --topology entirely: an \
                       empty, whitespace-only, or filler value says nothing and is recorded nowhere",
        })));
    }
    if topology_reason.is_some_and(says_nothing) {
        // The reason's own refusal, not the inline threshold's: it fires for a
        // blank reason on any declared topology, so it names that value rather
        // than assuming `inline`.
        let action = match topology.map(str::trim) {
            Some(value) if value.eq_ignore_ascii_case("inline") => format!(
                "the declared topology `{value}` carries a blank --topology-reason: say why it \
                 needed no delegation, or omit --topology-reason \
                 entirely"
            ),
            Some(value) => format!(
                "the declared topology `{value}` carries a blank --topology-reason: say why it \
                 was declared, or omit --topology-reason entirely"
            ),
            None => "the topology reason is blank and no topology was declared: declare \
                     --topology alongside a reason, or omit --topology-reason entirely"
                .to_owned(),
        };
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "blank-topology-reason",
            "action": action,
        })));
    }
    if topology.is_none() && topology_reason.is_some() {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false,
            "reason": "topology-reason-without-topology",
            "action": "a reason explains a declaration: declare --topology alongside it, or omit \
                       --topology-reason",
        })));
    }
    Ok((topology.map(str::trim), topology_reason.map(str::trim)))
}

/// GitHub caps one comment body at 65,536 characters. A published marker and
/// its prose go into one such comment, so a caller's free text that renders past
/// this cap would produce a comment GitHub refuses — after the branch is already
/// pushed.
const COMMENT_BODY_CEILING: usize = 65_536;

/// An upper bound, in characters, on every part of a published comment that is
/// neither the caller's own text, the frozen genesis manifest, nor the
/// publication lane: the prose template and each generated marker field at its
/// widest. **Derived by enumerating the fields [`published`] writes**, not
/// measured from one comment — the 1,454 this replaced was a sample taken before
/// the url, lane, autoclose and budget fields existed, so the format outgrew it
/// and the bound stopped bounding. `docs/honesty.md` records the enumeration.
const PUBLICATION_FIXED_OVERHEAD: usize = 8_192;

/// The most, in **bytes**, of a `gh` stderr the `lane-note` may carry. A
/// dispatch's detail is free-form transport text with no ceiling of its own, so
/// the comment can only be bounded once the detail is clamped here;
/// [`LANE_SENTENCE_CEILING`] reserves this clamp, percent-encoded, plus the
/// fixed sentences around it.
const LANE_DETAIL_CEILING: usize = 1_024;

/// The fixed sentences the publication lane renders around its workflow names,
/// and the clamped `gh` detail an `Absent` or `Unknown` dispatch embeds in its
/// `lane-note` — the sentences plus [`LANE_DETAIL_CEILING`] **bytes**, measured
/// through the marker's own encoding, which at worst triples them.
const LANE_SENTENCE_CEILING: usize = 4_096;

/// The most the publication lane can add to the published comment: its workflow
/// names, rendered three times (the marker's `lanes`, the marker's `lane-note`,
/// and the prose CI clause), plus [`LANE_SENTENCE_CEILING`]. The names are
/// measured through [`super::markers::encoded_len`] because the two marker
/// renderings percent-encode them — backticks, commas and spaces are outside the
/// marker's safe set — so the raw length under-reserves. Read from the same
/// candidates [`dispatch_publication_lane`] dispatches, because no constant
/// bounds a repository's workflow names.
fn lane_comment_bytes(context: &Context) -> usize {
    let names = format_lane_workflows(&publication_lane_candidates(context));
    super::markers::encoded_len(&names)
        .saturating_mul(3)
        .saturating_add(LANE_SENTENCE_CEILING)
}

/// The bytes the caller's own fields — `run-id`, `topology`,
/// `topology-reason` and the declared risk and focus — add to the published comment. None is bounded by a
/// constant: `run-id` is **not** bounded by the run-pointer filename, since
/// `session::store` is best-effort. The marker half renders the marker and
/// subtracts the empty marker's own width; the prose half is [`topology_line`].
fn caller_comment_bytes(
    run_id: &str,
    topology: Option<&str>,
    reason: Option<&str>,
    declared: &[(&str, String)],
) -> usize {
    let mut fields: Vec<(&str, &str)> = vec![("run-id", run_id)];
    fields.extend(declared.iter().map(|(name, value)| (*name, value.as_str())));
    if let Some(value) = topology {
        fields.push(("topology", value));
    }
    if let Some(value) = reason {
        fields.push(("topology-reason", value));
    }
    let empty = super::markers::render("published", &[]).map_or(0, |marker| marker.len());
    let with = super::markers::render("published", &fields).map_or(0, |marker| marker.len());
    with.saturating_sub(empty) + topology_line(topology, reason).len()
}

/// The `- topology ...` line [`publication_note`] renders, extracted so the
/// pre-push ceiling measures the same string that will be posted rather than a
/// second copy of the format.
fn topology_line(topology: Option<&str>, reason: Option<&str>) -> String {
    match topology {
        Some(value) => {
            let value = super::markers::escape_control_input(value);
            match reason {
                Some(reason) if !reason.trim().is_empty() => {
                    let reason = super::markers::escape_control_input(reason);
                    format!("\n- topology `{value}` \u{2014} {reason}")
                }
                _ => format!("\n- topology `{value}`"),
            }
        }
        None => String::new(),
    }
}

/// Debug builds only: skip the publication read-back wait when the test
/// fixture asks for it. A `--release` build compiles the read out entirely,
/// so the production schedule is unchanged. A shorter wait can only produce
/// more refusals, never an acceptance — the loop still runs every attempt.
#[cfg(debug_assertions)]
fn readback_wait_is_disabled() -> bool {
    static DISABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("ESTIGIA_TEST_READBACK_NO_WAIT").is_some());
    *DISABLED
}

/// The repository's schedule for an operation that publishes or writes to
/// GitHub.
///
/// A publication always pushes and a CI release always marks a pull request
/// ready, so outside the permitted period each is always denied.
pub(super) fn publish_schedule(context: &Context) -> Result<(), Failure> {
    match super::window::state(context) {
        super::window::Mode::Open => Ok(()),
        super::window::Mode::Closed(window) => Err(super::branch::schedule_denial(&window)),
    }
}

fn publish_with(
    context: &Context,
    what: &Publication<'_>,
    push: Push<'_>,
) -> Result<serde_json::Value, Failure> {
    #[cfg(debug_assertions)]
    if readback_wait_is_disabled() {
        return publish_with_sleep(context, what, push, &mut |_| {});
    }
    publish_with_sleep(context, what, push, &mut |delay| std::thread::sleep(delay))
}

/// [`publish_with`] with the read-back wait injected.
///
/// Production passes the real sleep; tests pass a recorder or a no-op, so the
/// suite pins the schedule without paying the window production covers. Same
/// body otherwise — a behaviour difference here would be the fixture proving
/// what it was told to.
fn publish_with_sleep(
    context: &Context,
    what: &Publication<'_>,
    push: Push<'_>,
    sleep: &mut dyn FnMut(std::time::Duration),
) -> Result<serde_json::Value, Failure> {
    let Publication {
        issue,
        run_id,
        expect_state,
        base,
        branch,
        title,
        pr_body_file,
        worktree,
        inline_payload,
        topology,
        topology_reason,
        focus,
        declaration,
        expected_epoch,
        now,
    } = *what;
    verify_claim(context, issue, run_id, expect_state, now, None)?;
    // Before the first remote call that is not a read: the internal push this
    // publication ends in is an ordinary push, so outside the permitted period
    // it is denied here rather than at the ref it would move.
    publish_schedule(context)?;
    // Trimmed once, here, so the predicate below, the marker, the note and
    // the answer cannot disagree: every one of them reads these bindings.
    let (topology, topology_reason) = normalize_topology(topology, topology_reason)?;
    // The caller's own text is the only part of the comment that no constant
    // bounds, so it is measured before the first remote write. The budget also
    // reserves the manifest's ceiling and the lane's measured width, or no single
    // bound bounds the comment. The enumeration is in `docs/honesty.md`.
    let declared = declaration.resolve(focus)?;
    let caller_bytes = caller_comment_bytes(
        run_id,
        topology,
        topology_reason,
        &declared.marker_fields(Some(super::tier::RiskLevel::Medium)),
    );
    let lane_bytes = lane_comment_bytes(context);
    let budget = COMMENT_BODY_CEILING
        .saturating_sub(PUBLICATION_FIXED_OVERHEAD)
        .saturating_sub(GENESIS_MANIFEST_CEILING)
        .saturating_sub(lane_bytes);
    if caller_bytes > budget {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "publication-comment-too-large",
            "bytes": caller_bytes,
            "ceiling": budget,
            "comment_ceiling": COMMENT_BODY_CEILING,
            "fixed_overhead": PUBLICATION_FIXED_OVERHEAD,
            "manifest_ceiling": GENESIS_MANIFEST_CEILING,
            "lane_overhead": lane_bytes,
            "action": format!(
                "--run-id, --topology, --topology-reason and the risk and focus declarations are rendered into the one published \
                 comment and together would leave no room under GitHub's \
                 {COMMENT_BODY_CEILING}-character cap once the rest of the comment is counted. \
                 Shorten them; nothing has been pushed and no pull request was opened"
            ),
        })));
    }
    let at = worktree.unwrap_or(&context.repo_dir);
    super::run(
        &["git", "fetch", "origin", "--", base],
        Some(at),
        super::How::read(),
    )?;
    let target = super::target::clean_target(context, &format!("origin/{base}"), Some(at))?;
    let payload_head = target
        .get("head")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let payload_base = target
        .get("base")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let target_digest = target
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    // Above `review_payload` on purpose: moving it below would materialise the
    // corrupted diff and measure its size against `review-payload-too-large`
    // before refusing it.
    if !super::target::is_ancestor(at, payload_base, payload_head)? {
        let merge_base = super::target::merge_base(at, payload_base, payload_head)?;
        let correcting = publishes_a_correction(context, branch, issue, now)?;
        let integrate = format!(
            "{base} advanced past what {branch} was rebased onto, so a two-dot diff would \
             carry everything {base} gained since {merge_base} inverted into this change's \
             own deletions. Recover by integrating {base}'s current tip into {branch} from \
             `in-progress`, as exactly one bare command: \
             `git -C <worktree> merge --no-edit origin/{base}` \u{2014} the gate admits no \
             pipe, redirect or second command on that line, and the re-review reads the \
             whole change, because a moved base is read whole"
        );
        let action = if correcting {
            format!(
                "{integrate}. Do not rebase: this is a correction over a recorded receipt, a \
                 rebase rewrites that receipt's head, and the targeted validator then refuses \
                 `validator-delta-carries-upstream` and the correction escalates to a person. \
                 Nothing has been pushed and no pull request was opened"
            )
        } else {
            format!(
                "{integrate} \u{2014} or by rebasing {branch} onto {base}'s current tip and \
                 publishing again: `republish_review` when this issue already has a recorded \
                 publication \u{2014} the ordinary push is refused as a non-fast-forward after \
                 a rebase \u{2014} or `publish_review` when it does not. Nothing has been \
                 pushed and no pull request was opened"
            )
        };
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "publication-base-not-ancestor",
            "base": payload_base,
            "head": payload_head,
            "merge_base": merge_base,
            "action": action,
        })));
    }

    let payload = super::target::review_payload(at, payload_base, payload_head)?;

    // The closing-keyword refusal, where it can be true. A `Stop` here reads
    // *nothing was written*, and a run believing that leaves the branch and PR
    // it had already written orphaned — so every source of a keyword this run
    // introduces is readable before the remote is touched: the commit messages
    // this branch adds, and the body about to be written. Read here, above
    // `open_prs` — **above every remote mutation**, not merely above the push.
    // Not above every remote *call*: `verify_claim` and the base fetch precede
    // it, and both are reads.
    //
    // The check below stays — it also settles a branch-derived link, and a
    // keyword can arrive from the remote side of a PR this run did not write —
    // but it can no longer be the first thing to notice one.
    let mut wrote_keyword = super::closing::any_keywords_in_commits(at, base, branch)?;
    // Read **once**, here, and reused below. Not `if let Ok(..)` either: a body
    // this run cannot read is not a body with no keyword in it — the same
    // sentence `keywords_in_commits` is written against, three lines away.
    //
    // The reused-PR branch used to read the same file a second time, below
    // `ensure_draft`. Two reads of one file is the copy this repository says to
    // remove rather than guard, and the copy was in the worse place: the second
    // read's failure landed *after* `gh pr ready --undo` may have run, so a file
    // that became unreadable in between refused with `nothing was written` over
    // a pull request that had already been un-readied. Hoisting the first read
    // above `open_prs` closed that window for the keyword scan and left the
    // second read sitting inside it.
    let body_text = match pr_body_file {
        Some(file) => {
            let text = std::fs::read_to_string(file).map_err(|error| {
                Failure::Read(format!("the PR body could not be read: {error}"))
            })?;
            wrote_keyword.extend(super::closing::keywords_any(&text));
            Some(text)
        }
        None => None,
    };
    if !wrote_keyword.is_empty() {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "closing-keyword-live",
            "cause": "closing-keyword",
            "keyword_sources": wrote_keyword,
            "action": "an issue WOULD auto-close on merge, bypassing transition and the mirror \
                       \u{2014} remove the keyword (use `Refs #<n>`) and re-run; nothing has been \
                       pushed and no pull request was opened",
        })));
    }

    // Discover and, when needed, draft a reused PR before the push. A ready PR
    // would otherwise emit synchronize and expose the new head before the
    // review barrier was restored.
    let existing = open_prs(context, branch)?;
    if existing.len() > 1 {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "ambiguous-open-prs", "prs": existing,
            "action": "more than one open PR for this branch — resolve by hand",
        })));
    }
    let reused = existing.first().cloned();

    // Read from the timeline here, before `ensure_draft` may rewrite the pull
    // request: a failed read here is a failed read, not an absent parent.
    let comments = timeline(context, issue, now).map_err(|failure| match failure {
        Failure::Read(message) => Failure::Read(format!(
            "{message} \u{2014} reading the publication lineage"
        )),
        Failure::Write(message) => Failure::Write(format!(
            "{message} \u{2014} reading the publication lineage"
        )),
        other => other,
    })?;
    let reused_pr = reused.as_ref().map(pr_number);
    let parent_here = publication_on(&comments, reused_pr)?;
    if let Some(expected) = expected_epoch {
        let newest = parent_here.as_ref().map(|here| here.receipt.epoch.as_str());
        if newest.unwrap_or(NO_EPOCH) != expected {
            return Err(expected_epoch_moved(expected, newest, reused_pr));
        }
    }
    let parent = parent_here.clone();
    let successor = parent_here
        .as_ref()
        .is_some_and(|here| recover::recovered(&comments, &here.receipt));
    let lineage = parent
        .as_ref()
        .map(|parent| -> Result<PublicationLineage, Failure> {
            let (delta_range, delta_range_reason) = match super::tier::repair_range(
                at,
                payload_base,
                &parent.receipt.head,
                payload_head,
            )? {
                super::tier::RepairRange::Narrow => (Some(REPAIR_RANGE_LABEL.to_owned()), None),
                super::tier::RepairRange::WholeChange(reason) => {
                    (None, Some(reason.as_str().to_owned()))
                }
            };
            let integrated_parent = delta_range_reason
                .as_deref()
                .and_then(super::tier::WholeChangeReason::parse)
                .is_some_and(|reason| {
                    matches!(
                        reason,
                        super::tier::WholeChangeReason::BaseAdvanced
                            | super::tier::WholeChangeReason::MergedRange
                    )
                })
                && base_integrated_cleanly(at, &parent.receipt, payload_base, payload_head);
            Ok(PublicationLineage {
                delta: fix_delta_digest(&parent.receipt, payload_head, target_digest),
                parent: parent.receipt.clone(),
                delta_range,
                delta_range_reason,
                integrated_parent,
                recovered: successor,
            })
        })
        .transpose()?;

    let parent_judged = parent_here
        .as_ref()
        .map(|parent| receipt_standing(&comments, &parent.receipt))
        .transpose()?;
    if let (Some(parent), Some(verdict), false) = (&parent_here, &parent_judged, successor) {
        match verdict.standing {
            Standing::Rejected => {
                let spent =
                    corrections_spent(&comments, &all_publications(&comments)?, parent.receipt.pr)?;
                let limit = corrections_per_lineage(context);
                if spent >= limit {
                    return Err(Failure::Stop(serde_json::json!({
                        "ok": false,
                        "reason": "correction-already-spent",
                        "issue": issue,
                        "pr": parent.receipt.pr,
                        "epoch": parent.receipt.epoch,
                        "corrections": spent,
                        "correction_limit": limit,
                        "action": "this lineage already spent all the corrections it is allowed \
                                   and its last publication still stands rejected or escalated, \
                                   so another correction is refused. Nothing has been pushed and no pull \
                                   request was opened. Hand the decision to a named person with \
                                   `escalate_review`, or publish the remaining work as a new \
                                   change on a new pull request, which starts its own lineage \
                                   and its own corrections",
                    })));
                }
            }
            Standing::Approved
                if !verdict.answered
                    && !base_advanced_past(at, &parent.receipt.base, payload_base)? =>
            {
                return Err(Failure::Stop(serde_json::json!({
                    "ok": false,
                    "reason": "approved-republish-without-cause",
                    "issue": issue,
                    "pr": parent.receipt.pr,
                    "epoch": parent.receipt.epoch,
                    "head": parent.receipt.head,
                    "verdict": Standing::Approved.word(),
                    "action": "the panel approved this pull request's last publication, no \
                               finding has been recorded against that exact receipt since, and \
                               the base has not advanced past the one it recorded. A republish \
                               over an approved receipt spends no correction but stays inside its \
                               pull request's correction budget and scope, and nothing gives \
                               this one a reason to be. Record a \
                               review_finding against that receipt naming the defect the \
                               verdict did not weigh, or merge the base's advance in. Nothing \
                               has been pushed and no pull request was opened",
                })));
            }
            Standing::Approved | Standing::Unjudged => {}
        }
    }

    let this_pr = reused.as_ref().map(pr_number);
    // Selects the latest **confirmed** publication on THIS pull request, unlike
    // `latest` above (docs/honesty.md has the defect this replaced). An
    // unconfirmed marker is a lease target only — using it as the budget parent
    // would bootstrap a fresh genesis with cumulative zero and drop the chain
    // a prior confirmed repair froze.
    let correction_parent = match this_pr {
        Some(pr) if !successor => all_publications(&comments)?
            .into_iter()
            .rev()
            .find(|publication| publication.receipt.pr == pr && publication.confirmed),
        _ => None,
    };

    let this_candidate_tree = super::tier::tree_of(context, Some(at), payload_head)?;
    let budget = correction_budget_check(
        context,
        at,
        correction_parent.as_ref(),
        payload_base,
        payload_head,
        &this_candidate_tree,
    )?;
    if let Some(state) = &budget.state {
        let encoded_bytes = genesis_manifest_encoded_len(&state.genesis_paths);
        if encoded_bytes > GENESIS_MANIFEST_CEILING {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "genesis-manifest-too-large",
                "bytes": encoded_bytes,
                "ceiling": GENESIS_MANIFEST_CEILING,
                "paths": state.genesis_paths.len(),
                "action": "the original candidate's changed-path manifest is too large to record \
                           on the published marker \u{2014} nothing has been pushed and no pull \
                           request was touched. Publish this change as a new change on a new pull \
                           request, which starts its own lineage: a first publication writes no \
                           manifest at all",
            })));
        }
    }

    // Whether the pull request has been **written to** by the time a later
    // refusal has to describe the world. Set where a write actually happens, and
    // not by asking whether one was reused.
    //
    // It was an `ensure_draft` that answered *did I un-ready it*, and that bool
    // could not change any outcome: two reviewers each turned it to a constant,
    // both ways, and the whole suite stayed green. The reason is structural —
    // `--pr-title` goes through `Flags::need`, which refuses a blank value, so
    // one of the two `edit_pr` arms always fires on this path and the flag is
    // already true whatever `ensure_draft` did. A flag whose value decides
    // nothing is the shape this crate refuses everywhere else. What replaced it
    // is not a flag at all: [`PullRequestWrites`] records *which* writes
    // happened, because the sentence a later refusal produces has to name them
    // and one boolean cannot. `ensure_draft`'s answer is back for that reason
    // and no other — it decides nothing here, and it is the only thing that
    // knows whether a ready pull request was un-readied.
    let mut wrote = PullRequestWrites::default();
    if let Some(pr) = &reused {
        wrote.undrafted = ensure_draft(context, pr)?;
        if let Some(raw) = &body_text {
            edit_pr(context, pr, title, Some(&pr_body_text(raw, issue)))
                .map_err(|failure| after_rewriting_the_pr(failure, wrote))?;
            wrote.edited = Some(Edited::TitleAndBody);
        } else if !title.is_empty() {
            // Title only, and the report says so. Whether anything can reach
            // this arm is [`Edited::Title`]'s to answer, and it is answered
            // there: restating it here is how this file came to assert both
            // *reachable from the CLI* and *the claim that a CLI route reaches
            // it was wrong*, 127 lines apart, in the commit that corrected the
            // fact.
            edit_pr(context, pr, title, None)
                .map_err(|failure| after_rewriting_the_pr(failure, wrote))?;
            wrote.edited = Some(Edited::Title);
        }
    } else if body_text.is_none() {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "missing-pr-body",
            "action": "pass --pr-body-file for a new PR",
        })));
    }

    if matches!(push, Push::Leased { .. }) {
        // The renewal the issue asks for, *immediately* before the push and not
        // merely somewhere above it. The verification at the top of this body is
        // separated from here by a fetch, a target derivation, a keyword scan
        // over every commit the branch adds, a pull request listing and — on the
        // reused path — a `gh pr ready --undo` and a body edit. Every one of
        // those is a network round trip, and a claim can be reclaimed across any
        // of them. For the fast-forward push that gap costs a refused push; for
        // this one it costs history, which is why only this route pays for the
        // second read. And its refusal has to say which world it is in:
        // `ensure_draft` and `edit_pr` may already have rewritten the pull
        // request, so a fresh one — where none did — keeps the original refusal.
        verify_claim(context, issue, run_id, expect_state, now, None)
            .map_err(|failure| after_rewriting_the_pr(failure, wrote))?;
    }
    // And the lease's own refusal, which is the answer this whole operation
    // exists to produce. It was the one left raw: the *rarer* path — the claim
    // moving — got a sentence naming the rewritten pull request, while the
    // designed outcome came back as a bare write failure that said nothing about
    // it. On the outcome this exists to produce — a lease git refuses — the
    // branch is provably untouched, so the pull request is the only thing left to
    // report. Not on every failure this can return: a transport error or a hook
    // rejecting after the ref moved leaves the branch's state unknown, and the
    // answer stays a `Write`, which the harness renders as *the write may have
    // landed*. The narrower sentence is the true one.
    push_to_origin(at, branch, push).map_err(|failure| after_rewriting_the_pr(failure, wrote))?;

    // The publication lane, started **once** and here.
    //
    // Here rather than anywhere else, and the two neighbours are why. It cannot
    // be above the push, because a dispatch names a ref and the bytes it must
    // measure are the ones the push just put there. It must not be below
    // `published`, because `published` is what writes the receipt: a receipt
    // recorded beside a lane nobody could start is an epoch that judges will be
    // bound to and CI will never answer for, which is the state this whole
    // change exists to remove. So the one refusal this can produce lands in the
    // narrowest committed world available — the branch is pushed, no epoch
    // exists, and re-running the same call after the permission is granted
    // reuses the pull request and publishes cleanly.
    //
    // Once per **publication epoch** and never per push: this body runs once per
    // `publish_review` or `republish_review` call and each such call creates one
    // epoch. Dispatching per push is the cardinality `CHANGELOG.md` records the
    // draft barrier refusing, and it is not what this is.
    let lane = dispatch_publication_lane(context, branch);
    if let LaneDispatch::Forbidden { workflow, detail } = &lane {
        return Err(after_rewriting_the_pr(
            Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "publication-lane-forbidden",
                // The push has happened. `Stop` alone reads as *nothing was
                // written*, and that sentence here would send somebody looking
                // for a branch that is already on the remote.
                "world": "committed",
                "branch": branch,
                "workflow": workflow,
                "detail": detail,
                "action": "this token may not start workflows, so the publication lane cannot run \
                           and no review transaction has been recorded. Grant it that permission \
                           \u{2014} `gh auth refresh -s workflow` for a `gh auth login` token, or \
                           add `actions: write` to the token in `GH_TOKEN` \u{2014} then run this \
                           same call again: the branch is already pushed, so it reuses the pull \
                           request and publishes",
            })),
            wrote,
        ));
    }

    let computed = if declared.raises() {
        Some(super::tier::tier_level(
            context,
            payload_base,
            payload_head,
            Some(at),
        )?)
    } else {
        None
    };
    let declared_fields = declared.marker_fields(computed);
    let answer = published(
        context,
        issue,
        run_id,
        branch,
        base,
        title,
        body_text.as_deref(),
        reused,
        target,
        &lane,
        &payload,
        inline_payload,
        topology,
        topology_reason,
        &declared_fields,
        lineage,
        this_candidate_tree,
        budget,
        now,
        sleep,
    )
    .map_err(|failure| {
        match failure {
            // `Read` says nothing happened, and something did. The others
            // already carry the right outcome and only want the fact.
            Failure::Read(detail) | Failure::RateLimited(detail) | Failure::Write(detail) => {
                Failure::Write(format!(
                    "{detail} \u{2014} the branch `{branch}` is already pushed to origin, \
                 so this is not a call that changed nothing"
                ))
            }
            other => other,
        }
    });
    // What the publication lane answered, in the call's own report.
    //
    // Reported and not refused for every value but `Forbidden`, which has
    // already returned above. A repository with no dispatchable lane, and a
    // dispatch whose outcome could not be read, both leave this epoch with no
    // check runs on its head — and `judge_publication_lane` lets a head with no
    // check runs through, so the run gets the ordering it had before this
    // existed rather than a refusal it cannot clear. Saying so in the answer is
    // the difference between that and losing the protection silently.
    let answer = answer.map(|mut answer| {
        // The detail — the note, and which workflows started — moved to
        // `review_transaction`; the write answer keeps only the state word.
        // See `docs/review-cost.md`.
        let (state, _note, _workflows) = lane_report(&lane);
        if let Some(answer) = answer.as_object_mut() {
            answer.insert("publication_lane".to_owned(), serde_json::json!(state));
        }
        answer
    });
    // Which route ran, in **the answer this call returns**, because the two are
    // not interchangeable to the run reading it back: a leased push moved bytes
    // the remote already had.
    //
    // Not in the record, and the difference is worth stating rather than
    // implying. The `published` marker this writes to the issue is byte-identical
    // to an ordinary publication's, so a later incident review reading the
    // timeline still cannot tell a republish from a publish. Adding a key to
    // that marker does not, on its own, cost a consumer — `markers::parse` is
    // tolerant and only `ReviewTransaction::from_marker`'s five named fields are
    // read strictly, and this file's own `topology`/`topology-reason` fields
    // in [`published`] are proof: recorded, unbound, and nothing that read
    // this marker before them had to change. So this gap is scope, not cost,
    // and is named here and in `docs/honesty.md` rather than closed as a side
    // effect of an unrelated change.
    let Push::Leased { recorded_head } = push else {
        return answer;
    };
    answer.map(|mut answer| {
        if let Some(answer) = answer.as_object_mut() {
            answer.insert("republished".to_owned(), serde_json::json!(true));
            answer.insert(
                "leased_against".to_owned(),
                serde_json::json!(recorded_head),
            );
        }
        answer
    })
}

/// What this run has already written to the reused pull request.
///
/// The writes named apart rather than one flag: one flag set by `edit_pr` alone
/// named a draft conversion that did not happen, and an operator told to put
/// back a draft conversion runs `gh pr ready`, which exposes the branch to CI —
/// the exact outcome `ensure_draft` exists to prevent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PullRequestWrites {
    /// `gh pr ready --undo` ran: it was ready, and is now a draft.
    undrafted: bool,
    /// What `edit_pr` replaced, when it ran at all.
    edited: Option<Edited>,
}

/// Which fields one `edit_pr` call replaced.
///
/// Body implies title: the one call takes both, and the arm that omits a body
/// still sends a title. So there is no *body only*, and offering one would be a
/// state this crate cannot produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edited {
    /// The title-only arm — **not reachable through anything that ships**, and
    /// kept anyway: no shipped caller leaves `body_text` `None` beside a reused
    /// pull request, but [`Publication`] takes an `Option` body and the branch
    /// below is real: an arm that cannot be reached still decides what a
    /// refusal *says* if anything ever reaches it, and collapsing it into
    /// [`Edited::TitleAndBody`] would name a body replacement that did not
    /// happen — the exact defect this type was introduced to remove. Nothing
    /// measures the difference, because nothing can drive the arm.
    Title,
    TitleAndBody,
}

impl PullRequestWrites {
    /// The writes, named exactly, or `None` when there are none.
    ///
    /// Each fragment completes *"the pull request …"*, and they have to do so in
    /// the **same grammatical shape** or the joined sentence is broken. The
    /// draft clause was a bare past participle and the edit clause a finite verb
    /// phrase, sharing a `was` supplied by the caller — so the ordinary case, a
    /// pull request that was already draft and only edited, produced *"the pull
    /// request was had its title and body replaced"*. That is the sentence an
    /// operator reads at the one moment they most need to understand what
    /// happened, and two tests pinned it with `contains`.
    fn describe(self) -> Option<String> {
        let mut done: Vec<&str> = Vec::new();
        if self.undrafted {
            done.push("was converted back to draft");
        }
        match self.edited {
            Some(Edited::TitleAndBody) => done.push("had its title and body replaced"),
            Some(Edited::Title) => done.push("had its title replaced"),
            None => {}
        }
        (!done.is_empty()).then(|| done.join(" and "))
    }
}

/// A refusal that arrives once the reused pull request has been written to.
///
/// The outcome channel, given the fact it did not have. `Stop` means *nothing
/// was written*, and after any of the writes above that is false. A run that
/// believes the refusal leaves somebody else's pull request altered and is told
/// to write nothing else, so nobody puts it back.
///
/// `Read` and `Write` are folded into `Write` for the same reason the readback
/// failures below it are: a read that failed is not a call that changed nothing
/// when a change has already gone out.
fn after_rewriting_the_pr(failure: Failure, wrote: PullRequestWrites) -> Failure {
    let Some(done) = wrote.describe() else {
        return failure;
    };
    match failure {
        Failure::Stop(mut envelope) => {
            if let Some(envelope) = envelope.as_object_mut() {
                envelope.insert("world".to_owned(), serde_json::json!("committed"));
                // **Appended, never replaced**: replacing the action destroys
                // the only sentence that names a way forward, and some of the
                // `Stop`s reaching here — the pre-push renewal, and
                // `publication-lane-forbidden` — carry actions of their own.
                let said = envelope
                    .get("action")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let wrote = format!(
                    "the pull request {done} before this refusal, so that much did happen and \
                     somebody has to decide whether to put it back"
                );
                envelope.insert(
                    "action".to_owned(),
                    serde_json::json!(if said.is_empty() {
                        wrote
                    } else {
                        format!("{said}; {wrote}")
                    }),
                );
            }
            Failure::Stop(envelope)
        }
        // No `already` before the clause: the fragments carry their own verb, so
        // *"the pull request already was converted back to draft"* is what that
        // word produces once the un-ready can reach this arm on its own.
        Failure::Read(detail) | Failure::RateLimited(detail) | Failure::Write(detail) => {
            Failure::Write(format!(
                "{detail} \u{2014} the pull request {done}, so this is not a call that changed nothing"
            ))
        }
        other => other,
    }
}

/// The `--force-with-lease` argument, spelled once.
///
/// Its own function because the spelling is the safety. `--force-with-lease`
/// **with no value** leases against the remote-tracking ref, which this very
/// operation refreshes when it fetches the base, and `--force-with-lease=<ref>`
/// with no `:<expect>` does the same for that one ref. Only the three-part form
/// names the commit the remote must currently be at, and only that form refuses
/// when somebody else pushed. A future edit that drops the `:{expected}` half
/// would still compile, still push, and still look like a lease.
fn lease_for(branch: &str, expected: &str) -> String {
    format!("--force-with-lease={branch}:{expected}")
}

/// The push, and the only place either route reaches the remote.
///
/// Separated from [`publish_with`] so a test can drive both routes against a
/// real bare repository. What that buys is the acceptance criterion nothing else
/// could check: *a test proves the lease refuses when the remote has moved
/// since*. Asserting the argument string would only prove this crate can format
/// one — git decides whether a lease refuses, and a lease git silently accepts
/// as a plain force would pass every assertion made about the text.
pub(super) fn push_to_origin(
    at: &std::path::Path,
    branch: &str,
    push: Push<'_>,
) -> Result<(), Failure> {
    match push {
        Push::FastForward => super::run(
            &["git", "push", "-u", "origin", "--", branch],
            Some(at),
            super::How::write(),
        ),
        Push::Leased { recorded_head } => {
            let lease = lease_for(branch, recorded_head);
            super::run(
                &["git", "push", &lease, "-u", "origin", "--", branch],
                Some(at),
                super::How::write(),
            )
        }
    }
    .map(|_| ())
}

pub(super) fn open_prs(context: &Context, branch: &str) -> Result<Vec<serde_json::Value>, Failure> {
    let fields = "number,url,headRefOid,baseRefOid,isDraft";
    super::gh_json(
        &["pr", "list", "--head", branch, "--state", "open", "--json", fields],
        Some(&context.repo_dir),
    )?
    .and_then(|value| value.as_array().cloned())
    .ok_or_else(|| Failure::Read(format!(
        "gh pr list returned no list for {branch}, so whether this branch already has an open pull request is unknown"
    )))
}

pub(super) fn pr_number(pr: &serde_json::Value) -> u64 {
    pr.get("number")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default()
}

pub(crate) fn view_pr(context: &Context, number: u64) -> Result<serde_json::Value, Failure> {
    super::gh_json(
        &[
            "pr",
            "view",
            &number.to_string(),
            "--json",
            "number,url,headRefOid,baseRefOid,state,isDraft,mergeable,mergeStateStatus",
        ],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh pr view {number} returned nothing")))
}

/// **Its own refusals carry its own write.**
///
/// A pull request that was already draft is confirmed and left alone, and both
/// refusals below can honestly say the world is untouched. One that was ready
/// has been un-readied by the time either is reached, so neither may read as
/// *nothing was written*. Reported here rather than returned to the caller,
/// because the caller cannot see which of these two exits was taken and
/// `isDraft` is read from the remote, not from the arguments.
///
/// Reported here rather than returned to the caller, because the caller cannot
/// see which of these two exits was taken and `isDraft` is read from the remote,
/// not from the arguments.
fn ensure_draft(context: &Context, pr: &serde_json::Value) -> Result<bool, Failure> {
    let number = pr_number(pr);
    let mut wrote = false;
    if pr.get("isDraft").and_then(serde_json::Value::as_bool) != Some(true) {
        super::run(
            &["gh", "pr", "ready", &number.to_string(), "--undo"],
            Some(&context.repo_dir),
            super::How::write(),
        )?;
        wrote = true;
    }
    let seen = view_pr(context, number).map_err(|failure| un_readied(failure, wrote, number))?;
    if seen.get("isDraft").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err(un_readied(
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "draft-readback-failed", "pr": number,
                "observed": seen,
                "action": "do not push; the reused PR is still ready and would expose the new head to CI",
            })),
            wrote,
            number,
        ));
    }
    Ok(wrote)
}

/// A refusal from inside [`ensure_draft`], once `gh pr ready --undo` has run.
///
/// **It adds the world and keeps the caller's action**, because the action is
/// the only sentence naming the hazard: *"do not push; the reused PR is still
/// ready and would expose the new head to CI"*. A refusal that destroys its own
/// warning is worse than one that says less.
///
/// The **`Read`/`Write` exit** words the write, and as an *attempt*: `--undo`
/// returned zero and the state could not be read afterwards.
/// [`PullRequestWrites::describe`] names writes that landed, and saying "was
/// converted back to draft" here would report a state this code did not confirm
/// — or, on the stop below, one it read back as false.
///
/// The **`Stop` exit** words nothing and adds only the world, deliberately: the
/// refusal's whole point is that the un-ready did not take, so it asserts
/// nothing else. The world says a write landed, and the action says do not push.
fn un_readied(failure: Failure, wrote: bool, pr: u64) -> Failure {
    if !wrote {
        return failure;
    }
    match failure {
        Failure::Stop(mut envelope) => {
            if let Some(envelope) = envelope.as_object_mut() {
                envelope.insert("world".to_owned(), serde_json::json!("committed"));
            }
            Failure::Stop(envelope)
        }
        Failure::Read(detail) | Failure::RateLimited(detail) | Failure::Write(detail) => {
            Failure::Write(format!(
                "{detail} \u{2014} `gh pr ready --undo` had already run against pull request #{pr}, so \
             this is not a call that changed nothing"
            ))
        }
        other => other,
    }
}

/// The `gh` detail a `lane-note` may carry, clamped to [`LANE_DETAIL_CEILING`]
/// **bytes** on a character boundary. A dispatch's stderr has no ceiling of its
/// own, so the published comment is bounded only once the detail is bounded
/// here first. The clamp counts bytes, not characters, because the marker
/// percent-encodes byte by byte — a character count would let four-byte scalars
/// expand threefold past the reservation.
fn clamp_lane_detail(said: &str) -> String {
    if said.len() <= LANE_DETAIL_CEILING {
        return said.to_owned();
    }
    let mut end = LANE_DETAIL_CEILING;
    while !said.is_char_boundary(end) {
        end -= 1;
    }
    said[..end].to_owned()
}

/// The lane state word, the reviewer-facing note, and which workflows
/// started, read off one [`LaneDispatch`].
///
/// Shared by the write answer's `publication_lane` field and the `published`
/// marker's `lane-note`/`lanes` attributes, so the two can never name a
/// different note for the same dispatch outcome — the exact drift a second,
/// hand-copied match arm would risk.
fn lane_report(lane: &LaneDispatch) -> (&'static str, String, Vec<String>) {
    match lane {
        LaneDispatch::Started(workflows) => {
            let named = format_lane_workflows(workflows);
            let note = if workflows.len() == 1 {
                format!(
                    "one run of {named} was dispatched against this head; an accepted verdict \
                     cannot be recorded until it is green"
                )
            } else {
                format!(
                    "{} runs ({named}) were dispatched against this head; an accepted verdict \
                     cannot be recorded until every one is green",
                    workflows.len()
                )
            };
            ("started", note, workflows.clone())
        }
        LaneDispatch::Absent(said) => (
            "absent",
            format!(
                "this repository has no dispatchable publication lane, so this epoch has no \
                 publication lane and verdicts are not gated on one ({})",
                clamp_lane_detail(said)
            ),
            Vec::new(),
        ),
        LaneDispatch::Unknown(said) => (
            "unknown",
            format!(
                "starting the publication lane set did not answer, so whether this epoch has a \
                 lane is unknown; read the checks on this head before obtaining verdicts ({})",
                clamp_lane_detail(said)
            ),
            Vec::new(),
        ),
        // Returned before `published` is ever called, and matched rather than
        // absorbed so that adding a value to the enum is a compile error here
        // instead of a silent omission from the report.
        LaneDispatch::Forbidden { detail, .. } => ("forbidden", detail.clone(), Vec::new()),
    }
}

/// The marker's detail fields — the URL (which the write answer still carries
/// as well), the lane's detail, the autoclose verdict and its follow-up — all
/// answered by `review_transaction` rather than by `publish_review`'s own
/// answer. Separate from [`published`] so a test can pin every write: the
/// claim that nothing was removed, only relocated, depends on them.
fn marker_detail_fields(
    url: &str,
    lane_note: &str,
    lane_workflows: &[String],
    verdict: &serde_json::Value,
) -> Vec<(&'static str, String)> {
    let mut fields: Vec<(&'static str, String)> = vec![("url", url.to_owned())];
    if !lane_workflows.is_empty() {
        fields.push(("lanes", lane_workflows.join(",")));
        fields.push(("lane-count", lane_workflows.len().to_string()));
    }
    fields.push(("lane-note", lane_note.to_owned()));
    fields.push(("autoclose-json", verdict.to_string()));
    let follow_up =
        if verdict.get("cause").and_then(serde_json::Value::as_str) == Some("branch-link") {
            format!(
                "GitHub will auto-close this issue on merge because the branch was linked, not \
                 because of a keyword. After the merge, `verify_claim` answers \
                 `closed-by-own-delivery` naming the delivering SHA and this receipt, not \
                 `issue-not-open`. {}",
                super::closing::FOLLOW_UP,
            )
        } else {
            String::new()
        };
    fields.push(("mandatory-follow-up", follow_up));
    fields
}

/// The half of [`publish_review`] that runs with the branch already pushed.
///
/// Its own function so the one sentence about that is written once, rather
/// than at each of the eight places down here that can fail.
#[expect(
    clippy::too_many_arguments,
    reason = "the arguments are the review transaction inputs and keeping them explicit avoids a second state object beside Publication"
)]
fn published(
    context: &Context,
    issue: u64,
    run_id: &str,
    branch: &str,
    base: &str,
    title: &str,
    // The body's **bytes**, not its path. Reading the file again here is what
    // made *"read once"* false: the closing-keyword scan validated one read and
    // the pull request was created from a second, so a body that gained
    // `Closes #<n>` in between passed the scan and was published anyway — and
    // that keyword auto-closes the issue on merge, which is the bypass the scan
    // exists to refuse. One read, carried down.
    body_text: Option<&str>,
    reused: Option<serde_json::Value>,
    target: serde_json::Value,
    // What starting the publication lane answered, because the receipt this
    // writes has to say it. The dispatch happens immediately above this call,
    // so a receipt that describes CI without it describes the world as it was
    // one line earlier — see [`publication_note`].
    lane: &LaneDispatch,
    payload: &super::target::ReviewPayload,
    // Whether the answer below carries `payload` whole. `payload_bytes` and
    // `payload_digest` are unconditional; this is the one field a caller has
    // to ask for, and the reason is size — see `Publication::inline_payload`.
    inline_payload: bool,
    // The declared execution topology and, when `inline` needed one, its
    // stated reason — see `Publication::topology`. Recorded on the marker and
    // answered back by `review_transaction`, not by this call's own answer;
    // the refusal over it already ran, in `publish_with`.
    topology: Option<&str>,
    topology_reason: Option<&str>,
    // The validated risk and focus declaration as marker fields, with the
    // computed tier a raise was made against; see [`tier::Resolved::marker_fields`].
    declared: &[(&'static str, String)],
    lineage: Option<PublicationLineage>,
    candidate_tree: String,
    budget: CorrectionCheck,
    now: &str,
    sleep: &mut dyn FnMut(std::time::Duration),
) -> Result<serde_json::Value, Failure> {
    let (pr, created) = match reused {
        Some(pr) => (pr, false),
        None => {
            let Some(raw) = body_text else {
                return Err(Failure::Stop(serde_json::json!({
                    "ok": false,
                    "reason": "missing-pr-body",
                    "action": "pass --pr-body-file for a new PR",
                })));
            };
            let staged = stage(&pr_body_text(raw, issue), &format!("pr-{issue}"))?;
            let answer = super::run(
                &[
                    "gh",
                    "pr",
                    "create",
                    "--base",
                    base,
                    "--head",
                    branch,
                    "--title",
                    title,
                    "--draft",
                    "--body-file",
                    &staged,
                ],
                Some(&context.repo_dir),
                super::How::write(),
            );
            let _ = std::fs::remove_file(&staged);
            answer?;
            let fresh = open_prs(context, branch)?;
            let Some(pr) = fresh.first().cloned() else {
                return Err(Failure::Read(
                    "the PR was created but is not visible yet".to_owned(),
                ));
            };
            (pr, true)
        }
    };
    let number = pr_number(&pr);

    // Everything reported from here on comes from the **read-back**, never from
    // the write path or the list that preceded it. `pr` is now an identifier.
    let local_head = target
        .get("head")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let expected_base = target
        .get("base")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    let seen = read_back_pr(context, number, local_head, expected_base, sleep)?;
    let Some(published) = confirm_published(&seen, local_head, expected_base, true).cloned() else {
        // The push landed; the PR may exist; the timeline would otherwise hold
        // nothing naming that head. Recording an unconfirmed publication is what
        // keeps `republish_review`'s lease satisfiable without a hand force-push.
        // Review and CI still refuse it — `confirmed` is false.
        let digest = target
            .get("digest")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let recovery = record_unconfirmed_publication(
            context,
            &UnconfirmedPublication {
                issue,
                run_id,
                pr: number,
                head: local_head,
                base: expected_base,
                digest,
                payload,
                now,
            },
        );
        let branch_ref = remote_branch_head(context, branch).ok();
        let world = classify_readback(branch_ref.as_deref(), local_head);
        let branch_ref_matches_head = branch_ref.as_ref().map(|sha| sha == local_head);
        let lead = readback_world_lead(world, local_head, number);
        let action = match &recovery {
            Ok(epoch) => format!(
                "{lead} The push landed at `{local_head}` and an unconfirmed publication was recorded \
                 (epoch `{epoch}`, pr {number}). The remote is ahead of any earlier confirmed \
                 receipt. Do NOT hand force-push and do NOT bind review or CI to this epoch: run \
                 republish_review after correcting the cause (rebase onto the current base when \
                 the head is no longer a descendant of it); it leases against that head and \
                 records a confirmed receipt. If the PR settled on a different head, someone \
                 else pushed and the lease will refuse rather than overwrite them"
            ),
            Err(failure) => format!(
                "the remote never reported the head and base that were just pushed, and \
                 recording an unconfirmed publication also failed ({}). The branch is on \
                 the remote with no lease target on the timeline. Re-read the PR; recovering \
                 without a hand force-push may not be possible until a marker can be written",
                failure.detail()
            ),
        };
        let mut envelope = serde_json::json!({
            "ok": false,
            "reason": "publication-readback-disagrees",
            // After the push and after the pull request: `nothing was written`
            // would be false, and this is the channel that says so.
            "world": "committed",
            "pr": number,
            "expected_head": local_head,
            "expected_base": expected_base,
            "expected_draft": true,
            "observed": seen.last(),
            "branch_ref": branch_ref,
            "branch_ref_matches_head": branch_ref_matches_head,
            "action": action,
        });
        if let Ok(epoch) = &recovery
            && let Some(envelope) = envelope.as_object_mut()
        {
            envelope.insert("unconfirmed_epoch".to_owned(), serde_json::json!(epoch));
            envelope.insert("unconfirmed_head".to_owned(), serde_json::json!(local_head));
        }
        return Err(Failure::Stop(envelope));
    };

    // The last two steps, and until the transport was retired their absence was
    // a **disagreement**: the port answered `ok` here and the transport, which
    // was what ran, went on. Retiring the transport promoted this from a half
    // nobody ported into a hole in the only implementation there is.
    //
    // First: will merging this close the issue behind the workflow's back? A
    // closing keyword in the pull request body auto-closes on merge, which skips
    // the transition and its projection — the workflow's close is a transition,
    // and GitHub's is not it.
    let verdict = super::commands::assess_autoclose(context, issue, Some(base), Some(branch))?;
    let names_this_issue =
        verdict.get("cause").and_then(serde_json::Value::as_str) == Some("closing-keyword");
    let names_another = verdict
        .get("other_issue_keywords")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|keywords| !keywords.is_empty());
    if names_this_issue || names_another {
        let (action, cause) = if names_this_issue {
            (
                "the issue WILL auto-close on merge, bypassing transition and the mirror \
                 \u{2014} remove the keyword (use `Refs #<n>`) and re-run",
                "closing-keyword",
            )
        } else {
            (
                "a closing keyword in the pull request will auto-close ANOTHER issue on merge, \
                 bypassing that issue's transition and mirror \u{2014} remove the keyword \
                 (use `Refs #<n>`) and re-run",
                "closing-keyword-elsewhere",
            )
        };
        let mut envelope = serde_json::json!({
            "ok": false,
            "reason": "closing-keyword-live",
            // Set here, not merged from the verdict: its cause is `branch-link`
            // for this case, and the remedy is read from the cause.
            "cause": cause,
            // The local precondition above catches a keyword this run wrote.
            // Reaching here means one arrived from the remote side, by which
            // time the branch is pushed and the pull request is open.
            "world": "committed",
            "pr": number,
            "action": action,
        });
        if let (Some(envelope), Some(verdict)) = (envelope.as_object_mut(), verdict.as_object()) {
            for (key, value) in verdict {
                envelope.entry(key.clone()).or_insert(value.clone());
            }
        }
        return Err(Failure::Stop(envelope));
    }

    // Second: the `published` marker. It is the delivery evidence — the exact
    // head and base a review and a CI run are bound to — and without it the pull
    // request exists and the timeline holds nothing that ties a verdict to those
    // bytes, which is the whole of what this product is for.
    let head = published
        .get("headRefOid")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let base_oid = published
        .get("baseRefOid")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let url = published
        .get("url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let digest = target
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let epoch = publication_epoch(run_id, number, head, base_oid, digest);
    let pr_number = number.to_string();
    let parent_pr = lineage
        .as_ref()
        .map(|lineage| lineage.parent.pr.to_string())
        .unwrap_or_default();
    let mut fields: Vec<(&str, &str)> = vec![
        ("run-id", run_id),
        ("pr", &pr_number),
        ("head", head),
        ("base", base_oid),
        ("digest", digest),
        ("payload-digest", &payload.digest),
        ("epoch", &epoch),
    ];
    // The lane's detail, the URL and the autoclose verdict, recorded here
    // because `publish_review`'s answer keeps only the lane's state word and
    // `review_transaction` answers the rest from this marker.
    let (_lane_state, lane_note, lane_workflows) = lane_report(lane);
    let detail = marker_detail_fields(url, &lane_note, &lane_workflows, &verdict);
    let detail_refs: Vec<(&str, &str)> = detail
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();
    fields.extend(detail_refs);
    if let Some(lineage) = &lineage {
        // The parent receipt whole, so the ledger it names can be read the way
        // every other ledger here is read — by the exact receipt. Recording only
        // the epoch made the parent a name rather than an identity, and a name
        // is what a backfilled marker can claim.
        fields.push(("parent", &lineage.parent.epoch));
        fields.push(("parent-pr", &parent_pr));
        fields.push(("parent-head", &lineage.parent.head));
        fields.push(("parent-base", &lineage.parent.base));
        fields.push(("parent-digest", &lineage.parent.digest));
        fields.push(("delta", &lineage.delta));
        if let Some(range) = &lineage.delta_range {
            fields.push(("delta-range", range));
        }
        if let Some(reason) = &lineage.delta_range_reason {
            fields.push(("delta-range-reason", reason));
        }
        if lineage.integrated_parent {
            fields.push(("delta-parent", "integrated"));
        }
        if lineage.recovered {
            fields.push(("recovered-from", &lineage.parent.epoch));
        }
    }
    fields.push(("budget-candidate-tree", &candidate_tree));
    let genesis_paths_joined;
    let original_lines_field;
    let budget_field;
    let cumulative_field;
    if let Some(state) = &budget.state {
        genesis_paths_joined = state.genesis_paths.join("\u{1f}");
        original_lines_field = state.original_changed_lines.to_string();
        budget_field = state.budget.to_string();
        cumulative_field = state.cumulative.to_string();
        fields.push(("budget-genesis-tree", &state.genesis_tree));
        fields.push(("budget-genesis-paths", &genesis_paths_joined));
        fields.push(("budget-original-lines", &original_lines_field));
        fields.push(("budget", &budget_field));
        fields.push(("budget-cumulative", &cumulative_field));
        if budget.refrozen {
            fields.push(("budget-refrozen", "base-advanced"));
        }
        if budget.integrated_parent {
            fields.push(("budget-parent", "integrated"));
        }
    }
    // Recorded on the same terms as `payload-digest` above: a caller's value,
    // written to the marker and read back by `review_transaction`, never by
    // `publish_review`'s own answer. `markers::render` drops an empty value,
    // so an absent `topology` and an empty one are indistinguishable here,
    // same as every other optional field on this marker.
    if let Some(value) = topology {
        fields.push(("topology", value));
    }
    if let Some(value) = topology_reason {
        fields.push(("topology-reason", value));
    }
    fields.extend(declared.iter().map(|(name, value)| (*name, value.as_str())));
    let note = publication_note(
        url,
        &epoch,
        head,
        base_oid,
        digest,
        &payload.digest,
        topology,
        topology_reason,
        lane,
        // A marker that will not render is a comment that carries no evidence,
        // and posting the prose without it would leave a note nobody can read
        // back as a fact. Refused rather than posted half.
        &super::markers::render("published", &fields).ok_or_else(|| {
            Failure::Write("the published marker could not be rendered".to_owned())
        })?,
    );
    super::commands::record_entry(context, issue, &note, now)?;

    // Two `next` sentences, not one edited by a flag check downstream. The
    // omitted-payload one must not tell a judge to be handed bytes this answer
    // did not carry, and must not offer a second publication as the way to
    // get them: `publication_epoch` mixes in the clock, so that mints a new
    // epoch and voids every verdict recorded against this one.
    //
    // Both are pinned whole by `tests/pipe.rs`, not by forbidden substrings.
    // Two panels rejected two attempts at the latter: a guard spelled around
    // the wording it policed, then a token blocklist that `publication` and
    // `--inline-payload` both walk past. A `contains` cannot decide whether a
    // sentence offers a route, so editing either string reddens the suite and
    // the judgement happens at the edit.
    let next = if inline_payload {
        "transition --to review, hand each judge the `payload` from this answer rather than a \
         checkout to search, record each lens report and the computed verdict, then release_ci with \
         this exact receipt"
    } else {
        "transition --to review, hand each judge a checkout of base/head to search, record each \
         lens report and the computed verdict, then release_ci with this exact receipt"
    };
    // The topology declaration, the lineage, the autoclose verdict and its
    // mandatory follow-up all moved to `review_transaction` — see
    // `docs/review-cost.md` for the measured shrink and `review_transaction`
    // for where each field now lives; the marker above already recorded every
    // one of them. Only identity fields, `next`, and the optional `payload`
    // stay on this answer.
    let mut answer = serde_json::json!({
        "ok": true,
        "issue": issue,
        "pr": number,
        "url": pr.get("url"),
        "created": created,
        "draft": true,
        "epoch": epoch,
        "head": published.get("headRefOid"),
        "base": published.get("baseRefOid"),
        "digest": digest,
        "payload_digest": payload.digest,
        "payload_bytes": payload.text.len(),
        "next": next,
    });
    if inline_payload {
        // Whole or absent, never truncated: a partial diff handed to a judge
        // is worse than none.
        answer["payload"] = serde_json::json!(payload.text);
    }
    Ok(answer)
}

/// The review transaction's prose, for one epoch and the lane it got.
///
/// **Pure, and separate from [`published`], because nothing read this text.**
/// It is the sentence every blind reviewer and every resuming run sees, and it
/// has to agree with the answer returned beside it — a reviewer told CI is
/// blocked reads a `publication-lane-unfinished` refusal as a malfunction
/// rather than as the documented gate. `src/transport/claim/tests.rs` reads it.
///
/// The two lanes are said apart, because conflating them is what let the old
/// sentence read as true. The **publication** lane is the `workflow_dispatch`
/// this operation just started against the pushed head, and it is what an
/// accepted verdict waits on. The ordinary **pull-request-event** lane is the
/// one that still does not run until the pull request is marked ready. Saying
/// only the second left a reviewer nothing to look at.
///
/// A function of the lane rather than one sentence, because three of the four
/// states reach here and they are not the same fact. Claiming a run was started
/// for a repository that has no dispatchable lane would be the same defect one
/// state along — a receipt disagreeing with the answer returned beside it.
#[expect(
    clippy::too_many_arguments,
    reason = "the receipt identity lines stay explicit so a dropped field is a compile error rather than a silent omission from the comment a reviewer reads"
)]
fn publication_note(
    url: &str,
    epoch: &str,
    head: &str,
    base: &str,
    digest: &str,
    payload_digest: &str,
    topology: Option<&str>,
    topology_reason: Option<&str>,
    lane: &LaneDispatch,
    marker: &str,
) -> String {
    let ci = match lane {
        LaneDispatch::Started(workflows) => {
            let named = format_lane_workflows(workflows);
            if workflows.len() == 1 {
                format!(
                    "One run of {named} was started against this head as this receipt was \
                     recorded, and an accepted verdict cannot be recorded until it is green."
                )
            } else {
                format!(
                    "{} runs ({named}) were started against this head as this receipt was \
                     recorded, and an accepted verdict cannot be recorded until every one is \
                     green.",
                    workflows.len()
                )
            }
        }
        LaneDispatch::Absent(_) => {
            "This repository has no dispatchable publication lane, so this epoch has \
             no publication lane and verdicts are not gated on one."
                .to_owned()
        }
        LaneDispatch::Unknown(_) => {
            "Starting the publication lane set against this head did not answer, so whether \
             this epoch has a publication lane is unknown \u{2014} read the checks on this head \
             before obtaining verdicts."
                .to_owned()
        }
        // Returned before `published` is ever called, and matched rather than
        // absorbed for the same reason the report arm above it is: adding a
        // value to the enum should be a compile error here, not a state that
        // silently inherits somebody else's sentence.
        LaneDispatch::Forbidden { workflow, .. } => format!(
            "The publication lane `{workflow}` could not be started for this \
             head, so nothing will answer for these bytes."
        ),
    };
    // Both fields are caller-supplied free text, escaped for the reason
    // `never_ran_clause` (`:3956`) escapes evidence: unescaped, either could
    // quote a real marker and become one once posted. The one format lives in
    // [`topology_line`] so the pre-push ceiling measures the same string.
    let topology_line = topology_line(topology, topology_reason);
    format!(
        "Published draft for review: {url}\n\n- epoch `{epoch}`\n- head `{head}`\n- base `{base}`\n- target `{digest}`\n- payload `{payload_digest}`{topology_line}\n\nReview is bound to this complete clean target. The payload is the change a judge is handed. {ci} The pull request stays draft, so the ordinary pull-request-event lane still waits for it to be marked ready; any republish creates a new epoch.\n\n{marker}\n"
    )
}

fn publication_epoch(run_id: &str, pr: u64, head: &str, base: &str, digest: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    super::ownership::sha256_hex(
        format!("{run_id}\0{pr}\0{head}\0{base}\0{digest}\0{now}").as_bytes(),
    )
    .chars()
    .take(32)
    .collect()
}

/// A publication recorded after the push landed and before readback confirmed it.
///
/// The head is what this run pushed, not what the PR view reported — the PR view
/// is what failed. That head is what [`republish_review`] leases against so the
/// remote can be rewritten without a hand force-push. Review and CI refuse it
/// until a later confirmed publication supersedes it.
/// What a push left on the remote when pull-request readback never confirmed it.
struct UnconfirmedPublication<'a> {
    issue: u64,
    run_id: &'a str,
    pr: u64,
    head: &'a str,
    base: &'a str,
    digest: &'a str,
    payload: &'a super::target::ReviewPayload,
    now: &'a str,
}

fn record_unconfirmed_publication(
    context: &Context,
    what: &UnconfirmedPublication<'_>,
) -> Result<String, Failure> {
    let epoch = publication_epoch(what.run_id, what.pr, what.head, what.base, what.digest);
    let issue = what.issue;
    let run_id = what.run_id;
    let pr = what.pr;
    let head = what.head;
    let base = what.base;
    let digest = what.digest;
    let payload = what.payload;
    let pr_number = pr.to_string();
    let fields: Vec<(&str, &str)> = vec![
        ("run-id", run_id),
        ("pr", &pr_number),
        ("head", head),
        ("base", base),
        ("digest", digest),
        ("payload-digest", &payload.digest),
        ("epoch", &epoch),
        ("unconfirmed", "true"),
    ];
    let marker = super::markers::render("published", &fields).ok_or_else(|| {
        Failure::Write("the unconfirmed published marker could not be rendered".to_owned())
    })?;
    let note = format!(
        "Unconfirmed publication after a push whose pull-request readback disagreed.\n\n\
         - epoch `{epoch}`\n\
         - head `{head}`\n\
         - base `{base}`\n\
         - target `{digest}`\n\
         - payload `{payload_digest}`\n\n\
         The branch is on the remote at this head. Review and CI must not bind to this epoch. \
         Run `republish_review` after correcting the cause; it leases against this head and \
         records a confirmed receipt. Do not hand force-push.\n\n\
         {marker}\n",
        payload_digest = payload.digest,
    );
    super::commands::record_entry(context, issue, &note, what.now)?;
    Ok(epoch)
}

/// The exact review transaction `release-ci` is allowed to spend.
#[derive(Debug, Clone)]
pub struct CiRelease<'a> {
    /// The issue whose live claim authorises the write.
    pub issue: u64,
    /// The run holding the live `review` claim this write is measured
    /// against. It need not be the run that published the receipt.
    pub run_id: &'a str,
    /// The publication epoch.
    pub epoch: &'a str,
    /// The pull request number.
    pub pr: u64,
    /// The recorded published head.
    pub head: &'a str,
    /// The recorded published base.
    pub base: &'a str,
    /// The recorded complete-target digest.
    pub digest: &'a str,
    /// The checkout whose clean target is re-derived.
    pub worktree: Option<&'a std::path::Path>,
    /// The moment the timeline is judged against.
    ///
    /// Read from the machine, never from the run being judged. A test may pass
    /// a chosen value; production obtains it from the MCP server's clock.
    pub now: &'a str,
}

/// Releases a frozen draft target to CI after replaying every receipt field.
pub fn release_ci(
    context: &Context,
    release: &CiRelease<'_>,
) -> Result<serde_json::Value, Failure> {
    let ledger = Ledger::open(context, release.issue, release.now)?;
    verify_held(
        context,
        &ledger,
        release.issue,
        release.run_id,
        "review",
        release.now,
        None,
    )?;
    publish_schedule(context)?;
    let comments = ledger.timeline()?;
    let supplied = ReviewTransaction {
        epoch: release.epoch.to_owned(),
        pr: release.pr,
        head: release.head.to_owned(),
        base: release.base.to_owned(),
        digest: release.digest.to_owned(),
    };
    recorded_receipt(&comments, &supplied)?;
    // An `escalated` receipt is a decision somebody took; it is refused whatever
    // basis would otherwise release, so computed evidence cannot carry it past CI
    // and it can never read as a missing review.
    require_un_escalated_terminal(&comments, &supplied)?;
    let basis = release_basis(context, &comments, &supplied, release.worktree)?;

    // A `warning` or a `suggestion` on this receipt is informational: it never
    // opened a correction and nothing about delivery waits on it. A run may
    // have recorded a `review_disposition` for a finding it acted on or filed,
    // and that record is read where it is asked for rather than here.
    let current = view_pr(context, release.pr)?;
    require_pr_matches(&current, &supplied, true)?;
    let target = super::target::clean_target(context, release.base, release.worktree)?;
    if target.get("head").and_then(serde_json::Value::as_str) != Some(release.head)
        || target.get("base").and_then(serde_json::Value::as_str) != Some(release.base)
        || target.get("digest").and_then(serde_json::Value::as_str) != Some(release.digest)
    {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "release-target-mismatch",
            "recorded": {"head": release.head, "base": release.base, "digest": release.digest},
            "derived": target,
            "action": "do not release CI; republish the current clean target and obtain fresh review evidence",
        })));
    }

    let ready_write = super::run(
        &["gh", "pr", "ready", &release.pr.to_string()],
        Some(&context.repo_dir),
        super::How::write(),
    )
    .map(|_| ());
    ready_outcome(ready_write, view_pr(context, release.pr), &supplied)?;
    let (lane, next) = ci_outlook(context, release.head, release.pr);
    Ok(serde_json::json!({
        "ok": true,
        "issue": release.issue,
        "epoch": release.epoch,
        "pr": release.pr,
        "head": release.head,
        "base": release.base,
        "digest": release.digest,
        "basis": basis.word(),
        "draft": false,
        "ci_released": next == "ci_status",
        "lane": lane,
        "next": next,
    }))
}

/// What a run does once `release_ci` has marked the pull request ready: the
/// publication lane read for the head, and the step that follows it. `merge`
/// only where the lane says no CI can run; `ci_status` for every other
/// reading, an unreadable one included.
fn ci_outlook(context: &Context, head: &str, pr: u64) -> (serde_json::Value, &'static str) {
    match publication_lane(context, head, Some(pr)) {
        Ok(lane) => {
            let next = next_after_release(&lane);
            (lane, next)
        }
        Err(failure) => (
            serde_json::json!({
                "ok": false,
                "state": "unreadable",
                "action": "the publication lane could not be read, so whether CI will run is \
                           unknown; treat it as running and use ci_status",
                "detail": failure.envelope(),
            }),
            "ci_status",
        ),
    }
}

fn next_after_release(lane: &serde_json::Value) -> &'static str {
    match lane.get("state").and_then(serde_json::Value::as_str) {
        Some("not-consulted" | "disabled" | "never-ran") => "merge",
        _ => "ci_status",
    }
}

fn require_pr_matches(
    current: &serde_json::Value,
    receipt: &ReviewTransaction,
    expected_draft: bool,
) -> Result<(), Failure> {
    let matches = current.get("number").and_then(serde_json::Value::as_u64) == Some(receipt.pr)
        && current
            .get("headRefOid")
            .and_then(serde_json::Value::as_str)
            == Some(receipt.head.as_str())
        && current
            .get("baseRefOid")
            .and_then(serde_json::Value::as_str)
            == Some(receipt.base.as_str())
        && current.get("isDraft").and_then(serde_json::Value::as_bool) == Some(expected_draft);
    if !matches {
        let recorded = serde_json::json!(
            {"pr": receipt.pr, "head": receipt.head, "base": receipt.base, "draft": expected_draft}
        );
        return Err(Failure::Stop(if expected_draft {
            serde_json::json!({
                "ok": false,
                "reason": "release-pr-mismatch",
                "recorded": recorded,
                "observed": current,
                "action": "do not release CI; the current draft PR is not the exact recorded \
                           publication",
            })
        } else {
            serde_json::json!({
                "ok": false,
                "reason": "ready-readback-failed",
                "recorded": recorded,
                "observed": current,
                "action": "the ready write may have landed differently; re-read the PR before \
                           taking any further action",
            })
        }));
    }
    Ok(())
}

/// Replaces a reused PR's title, and its body when one was supplied.
fn edit_pr(
    context: &Context,
    pr: &serde_json::Value,
    title: &str,
    body: Option<&str>,
) -> Result<(), Failure> {
    let number = pr
        .get("number")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default()
        .to_string();
    match body {
        Some(body) => {
            let staged = stage(body, &format!("pr-edit-{number}"))?;
            let answer = super::run(
                &[
                    "gh",
                    "pr",
                    "edit",
                    &number,
                    "--title",
                    title,
                    "--body-file",
                    &staged,
                ],
                Some(&context.repo_dir),
                super::How::write(),
            );
            let _ = std::fs::remove_file(&staged);
            answer?;
        }
        None => {
            super::run(
                &["gh", "pr", "edit", &number, "--title", title],
                Some(&context.repo_dir),
                super::How::write(),
            )?;
        }
    }
    Ok(())
}

/// Writes a body to a temporary file, atomically because `gh` reads it.
pub(super) fn stage(body: &str, name: &str) -> Result<String, Failure> {
    let path = crate::paths::scratch_file(&format!("{name}.md"));
    crate::paths::replace_atomically(&path, body)
        .map_err(|error| Failure::Write(format!("the body could not be staged: {error}")))?;
    Ok(path.display().to_string())
}

/// The operation id an ownership write must carry, or a refusal.
///
/// Exit code `2` rather than `1`, because a malformed argument is the caller's
/// own defect and not the tracker changing its mind. Reading one as the other
/// tells a run "somebody took your claim" about a typo.
pub fn require_operation_id(value: Option<&str>) -> Result<String, Failure> {
    let value = value.unwrap_or_default();
    if !ownership::is_operation_id(value) {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "invalid-operation-id" }),
        ));
    }
    Ok(value.to_owned())
}

/// The event kinds `operation_marker` resolves an id against.
///
/// Narrower than `ownership::OPERATION_FIELDS`: `standdown` carries no
/// `expected` shape for a caller to compare against and never reaches this
/// function, so it has no variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    /// A run taking or holding an item.
    Claim,
    /// A run taking an item over from a stale or foreign holder.
    Reclaim,
    /// A run releasing an item back to the pool.
    Unassign,
    /// A review finding recorded against a receipt.
    ReviewFinding,
    /// A request to hand a review to another run.
    ReviewHandoff,
    /// A refutation batch recorded against a receipt.
    ReviewRefutation,
    /// A disposition batch recorded against a receipt.
    ReviewDisposition,
    /// A budget spend recorded against a receipt.
    Spend,
    /// A lens's completion recorded against a receipt.
    ReviewLensReport,
    /// A verdict computed from the ledger of one receipt.
    ReviewComputedVerdict,
}

impl OperationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Claim => "claim",
            Self::Reclaim => "reclaim",
            Self::Unassign => "unassign",
            Self::ReviewFinding => "review-finding",
            Self::ReviewHandoff => "review-handoff",
            Self::ReviewRefutation => "review-refutation",
            Self::ReviewDisposition => "review-disposition",
            Self::Spend => "spend",
            Self::ReviewLensReport => "review-lens-report",
            Self::ReviewComputedVerdict => "review-computed-verdict",
        }
    }

    /// The envelope `operation_marker` refuses two copies of one operation
    /// with, when they disagree about a field the kind declares.
    ///
    /// The reason is a literal in every arm, on purpose: `build.rs` extracts
    /// each reason value beside its `reason` key from the transport's source
    /// text, so a literal here is a reason the generated vocabulary already
    /// sees, and the match being exhaustive is what stops a new variant from
    /// compiling without one.
    fn operation_conflict(self) -> serde_json::Value {
        match self {
            Self::Claim => serde_json::json!({ "ok": false, "reason": "claim-operation-conflict" }),
            Self::Reclaim => {
                serde_json::json!({ "ok": false, "reason": "reclaim-operation-conflict" })
            }
            Self::Unassign => {
                serde_json::json!({ "ok": false, "reason": "unassign-operation-conflict" })
            }
            Self::ReviewFinding => {
                serde_json::json!({ "ok": false, "reason": "review-finding-operation-conflict" })
            }
            Self::ReviewHandoff => {
                serde_json::json!({ "ok": false, "reason": "review-handoff-operation-conflict" })
            }
            Self::ReviewRefutation => {
                serde_json::json!({ "ok": false, "reason": "review-refutation-operation-conflict" })
            }
            Self::ReviewDisposition => {
                serde_json::json!({ "ok": false, "reason": "review-disposition-operation-conflict" })
            }
            Self::Spend => serde_json::json!({ "ok": false, "reason": "spend-operation-conflict" }),
            Self::ReviewLensReport => {
                serde_json::json!({
                    "ok": false, "reason": "review-lens-report-operation-conflict",
                    "action": "this operation id already records a different lens event; \
                               record the report under the id that call minted",
                })
            }
            Self::ReviewComputedVerdict => {
                serde_json::json!({
                    "ok": false, "reason": "review-computed-verdict-operation-conflict",
                    "action": "this operation id already records a different verdict event; \
                               record the verdict under the id that call minted",
                })
            }
        }
    }

    /// The envelope `operation_marker` refuses a caller's expectation with,
    /// when it differs from what was persisted. See `operation_conflict` for
    /// why the reason is a literal in every arm.
    fn metadata_mismatch(self, persisted: &super::markers::Marker) -> serde_json::Value {
        match self {
            Self::Claim => {
                serde_json::json!({ "ok": false, "reason": "claim-metadata-mismatch", "persisted": persisted })
            }
            Self::Reclaim => {
                serde_json::json!({ "ok": false, "reason": "reclaim-metadata-mismatch", "persisted": persisted })
            }
            Self::Unassign => {
                serde_json::json!({ "ok": false, "reason": "unassign-metadata-mismatch", "persisted": persisted })
            }
            Self::ReviewFinding => {
                serde_json::json!({ "ok": false, "reason": "review-finding-metadata-mismatch", "persisted": persisted })
            }
            Self::ReviewHandoff => {
                serde_json::json!({ "ok": false, "reason": "review-handoff-metadata-mismatch", "persisted": persisted })
            }
            Self::ReviewRefutation => {
                serde_json::json!({ "ok": false, "reason": "review-refutation-metadata-mismatch", "persisted": persisted })
            }
            Self::ReviewDisposition => {
                serde_json::json!({ "ok": false, "reason": "review-disposition-metadata-mismatch", "persisted": persisted })
            }
            Self::Spend => {
                serde_json::json!({ "ok": false, "reason": "spend-metadata-mismatch", "persisted": persisted })
            }
            Self::ReviewLensReport => {
                serde_json::json!({
                    "ok": false, "reason": "review-lens-report-metadata-mismatch",
                    "persisted": persisted,
                    "action": "repeat the run, role and receipt the persisted marker above records",
                })
            }
            Self::ReviewComputedVerdict => {
                serde_json::json!({
                    "ok": false, "reason": "review-computed-verdict-metadata-mismatch",
                    "persisted": persisted,
                    "action": "repeat the run and receipt the persisted marker above records",
                })
            }
        }
    }
}

/// The marker an operation already wrote, if it wrote one.
///
/// This is what makes a retry safe: an operation id names **one event**, so a
/// second attempt finds its own earlier marker and reports it instead of writing
/// again. Two claim comments for one claim is exactly what it prevents.
///
/// It resolves identical transport copies and refuses everything else:
///
/// - a **conflict** when two copies of one operation disagree about any field
///   the kind declares. Repairing metadata after the fact would let a retry mean
///   something the first attempt did not.
/// - a **mismatch** when what was persisted differs from what this caller
///   expected. Rebinding an operation to new values is how one id comes to name
///   two different events.
/// - and a conflict when the first marker sits in an **edited** comment, whose
///   contents cannot be trusted to be what was written.
pub fn operation_marker(
    comments: &[ownership::Comment],
    operation_id: &str,
    kind: OperationKind,
    expected: &[(&str, &str)],
) -> Result<Option<super::markers::Marker>, Failure> {
    let conflict = || Failure::Stop(kind.operation_conflict());
    if let Some((_, _, _, unedited)) =
        ownership::first_operation_markers(comments).get(operation_id)
        && !unedited
    {
        return Err(conflict());
    }

    let matches: Vec<super::markers::Marker> = comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .flat_map(|comment| super::markers::parse(&comment.body))
        .filter(|mark| {
            mark.get("kind").map(String::as_str) == Some(kind.as_str())
                && mark.get("op-id").map(String::as_str) == Some(operation_id)
        })
        .collect();
    let Some(persisted) = matches.first() else {
        return Ok(None);
    };

    let fields = ownership::OPERATION_FIELDS
        .iter()
        .find(|(name, _)| *name == kind.as_str())
        .map(|(_, fields)| *fields)
        .unwrap_or_default();
    let shape = |mark: &super::markers::Marker| -> Vec<Option<String>> {
        fields
            .iter()
            .map(|field| mark.get(*field).cloned())
            .collect()
    };
    if matches.iter().any(|mark| shape(mark) != shape(persisted)) {
        return Err(conflict());
    }
    if expected
        .iter()
        .any(|(key, value)| persisted.get(*key).map(String::as_str) != Some(*value))
    {
        return Err(Failure::Stop(kind.metadata_mismatch(persisted)));
    }
    Ok(Some(persisted.clone()))
}

/// Refuses an operation id that has been used for a kind it does not belong to.
///
/// One id names one event, and an event has one kind. Letting a `claim`'s id
/// later carry a `standdown` would make the timeline's own identity ambiguous:
/// the same operation would appear to both take and release the item.
pub fn reject_operation_kind_conflict(
    comments: &[ownership::Comment],
    operation_id: &str,
    allowed: &[&str],
) -> Result<(), Failure> {
    let conflict = |kinds: Option<Vec<String>>| {
        let mut payload =
            serde_json::json!({ "ok": false, "reason": "operation-id-kind-conflict" });
        if let Some(kinds) = kinds {
            payload["kinds"] = serde_json::json!(kinds);
        }
        Failure::Stop(payload)
    };
    if let Some((_, _, _, unedited)) =
        ownership::first_operation_markers(comments).get(operation_id)
        && !unedited
    {
        return Err(conflict(None));
    }
    let mut kinds: Vec<String> = comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .flat_map(|comment| super::markers::parse(&comment.body))
        .filter(|mark| mark.get("op-id").map(String::as_str) == Some(operation_id))
        .filter_map(|mark| mark.get("kind").cloned())
        .collect();
    kinds.sort();
    kinds.dedup();
    if kinds.iter().any(|kind| !allowed.contains(&kind.as_str())) {
        return Err(conflict(Some(kinds)));
    }
    Ok(())
}

/// The horizon a claim must declare, or a refusal.
///
/// Shape **and** value: it has to be the exact stamp form, it has to parse, and
/// when a floor is given it has to be **strictly after** it. A horizon equal to
/// now is already spent, and one behind it was never granted — a claim that
/// declares either is asking for a deadline it has already missed.
///
/// Exit code `2`, like an operation id: a malformed argument is the caller's own
/// defect, not the tracker changing its mind.
pub fn require_horizon(value: &str, after: Option<&str>) -> Result<String, Failure> {
    let stamp = ownership::parse_stamp(value);
    let floor = after.and_then(ownership::parse_stamp);
    let well_formed = ownership::is_horizon(value) && stamp.is_some();
    let ahead = match (stamp, floor) {
        (Some(stamp), Some(floor)) => stamp > floor,
        _ => true,
    };
    if !well_formed || !ahead {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "invalid-horizon" }),
        ));
    }
    Ok(value.to_owned())
}

/// How many times a read is repeated while waiting for a write to become visible.
pub const VISIBILITY_ATTEMPTS: usize = 7;

/// Bounds an eventual-consistency read **without repeating the write**.
///
/// That is the whole reason it exists as a wait rather than a retry: the comment
/// has already been posted, and posting it again because the tracker had not
/// caught up yet is how one claim becomes two.
///
/// **Pure and fed**: the observations come in as a list, so what is testable is
/// the decision — how many agreeing reads are needed, what an unreadable one
/// resets, and which failure an exhausted wait raises — rather than the sleeping.
///
/// Exhausting it after a write is an **ambiguous write**, never a stop: the
/// write may well have landed and simply not be visible, and telling a caller
/// "nothing happened" about that is the lie the taxonomy exists to prevent.
pub fn wait_for(
    observations: &[Option<serde_json::Value>],
    predicate: impl Fn(&serde_json::Value) -> bool,
    consecutive: usize,
) -> Option<&serde_json::Value> {
    let mut matches = 0;
    for seen in observations.iter().take(VISIBILITY_ATTEMPTS) {
        match seen {
            Some(data) if predicate(data) => {
                matches += 1;
                if matches >= consecutive.max(1) {
                    return Some(data);
                }
            }
            // A read that did not answer resets the run of agreements. Two
            // agreeing reads either side of a failure are not two in a row, and
            // treating them as such is how a flapping tracker looks settled.
            _ => matches = 0,
        }
    }
    None
}

/// What an issue should show while a run holds it.
///
/// The `dev:<runtime>` label and the assignee, which are what a person reads at
/// a glance. A stale `dev:` label names the previous holder to everyone reading
/// the tracker, and the same labels are what [`plan_release`] consults to decide
/// whether a successor can be named — so a projection nobody writes is an
/// authorisation nobody can check.
///
/// **Unresolved is not empty.** A holder whose runtime cannot be established
/// leaves `unresolved_runtime` set and every expected set empty: unknown legacy
/// provenance must not leave the *previous* holder projected as authoritative,
/// and it must not quietly project the new one either.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Projection {
    /// Who the issue should be assigned to.
    pub assignees: std::collections::BTreeSet<String>,
    /// The `dev:<runtime>` labels it should carry — one, or none.
    pub runtimes: std::collections::BTreeSet<String>,
    /// A holder whose runtime nothing could establish.
    pub unresolved_runtime: bool,
}

/// The projection a holder calls for, given what the issue already carries.
///
/// An acquisition that recorded no runtime is legacy, and the only thing left
/// that can name it is the `dev:` labels already there — but only when they
/// name **exactly one** runtime this holder could be using. Two candidates is
/// not a majority, it is a question, and the answer is that nothing is
/// projected until somebody says.
pub fn projection_for(
    event: Option<&ownership::Event>,
    labels: &[String],
    login: Option<&str>,
) -> Projection {
    let Some(event) = event else {
        return Projection::default();
    };
    let runtime = match event.runtime.as_deref() {
        Some(recorded) => Some(recorded.to_owned()),
        None => {
            let mut legacy: Vec<&str> = labels
                .iter()
                .filter_map(|name| name.strip_prefix("dev:"))
                .filter(|name| holder_uses_runtime(event, name))
                .collect();
            legacy.sort_unstable();
            legacy.dedup();
            match legacy.as_slice() {
                [only] => Some((*only).to_owned()),
                _ => None,
            }
        }
    };
    let Some(runtime) = runtime else {
        return Projection {
            unresolved_runtime: true,
            ..Projection::default()
        };
    };
    Projection {
        assignees: login.map(ToOwned::to_owned).into_iter().collect(),
        runtimes: [format!("dev:{runtime}")].into_iter().collect(),
        unresolved_runtime: false,
    }
}

/// The assignees and `dev:` labels an issue payload currently carries.
pub fn projection_state(
    data: &serde_json::Value,
) -> (
    std::collections::BTreeSet<String>,
    std::collections::BTreeSet<String>,
) {
    let assignees = data
        .get("assignees")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("login").and_then(serde_json::Value::as_str))
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let runtimes = labels_of(data)
        .into_iter()
        .filter(|name| name.starts_with("dev:"))
        .collect();
    (assignees, runtimes)
}

/// The label projection that has to agree with the holder.
///
/// Exactly one `dev:<runtime>` label, belonging to the run that actually holds
/// the issue. Returns the labels to add and to remove, so the caller owns the
/// writes and this stays checkable.
///
/// A projection that disagrees with the timeline is worse than none: the labels
/// are what a person reads at a glance, so a stale `dev:` label tells everyone
/// the issue belongs to a run that put it down.
pub fn converge_projection(
    present: &[String],
    holder_runtime: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let projected: Vec<&String> = present
        .iter()
        .filter(|name| name.starts_with("dev:"))
        .collect();
    let wanted = holder_runtime.map(|runtime| format!("dev:{runtime}"));

    let remove: Vec<String> = projected
        .iter()
        .filter(|name| wanted.as_ref() != Some(**name))
        .map(|name| (*name).clone())
        .collect();
    let add: Vec<String> = wanted
        .filter(|name| !present.contains(name))
        .into_iter()
        .collect();
    (add, remove)
}

/// The login `gh` is authenticated as.
fn viewer_login(context: &Context) -> Result<String, Failure> {
    super::store::viewer_login(context)
}

/// Refuses a claim whose issue carries a `dev:` label or assignee the store
/// cannot explain.
///
/// Two machines with separate stores can each hold one issue and neither is
/// told: the store never reads GitHub's projection before claiming, so it cannot
/// notice a projection another store left behind. This is that read. The
/// override the design names is `reclaim`, which does not run this.
///
/// A `dev:<runtime>` label is explained by an ownership event on the store's
/// timeline, live or stale, or by a legacy current holder whose run id names the
/// runtime. An assignee is explained only by the authenticated login.
fn require_explainable_projection(
    context: &Context,
    ledger: &Ledger,
    what: &Acquisition<'_>,
) -> Result<(), Failure> {
    let data = super::gh_json(
        &[
            "issue",
            "view",
            &what.issue.to_string(),
            "--json",
            "assignees,labels",
        ],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh issue view {} returned nothing", what.issue)))?;
    let (assignees, runtimes) = projection_state(&data);
    let timeline = ledger.timeline()?;
    let events = ownership::ownership_events(&timeline);
    let holder = holding(&timeline, what.now).event;
    let explained = |runtime: &str| {
        events
            .iter()
            .any(|event| event.runtime.as_deref() == Some(runtime))
            || holder
                .as_ref()
                .is_some_and(|event| event.runtime.is_none() && holder_uses_runtime(event, runtime))
    };
    let mut labels: Vec<String> = Vec::new();
    for label in runtimes {
        let Some(runtime) = label.strip_prefix("dev:") else {
            continue;
        };
        if !explained(runtime) && !ledger.handoff_explains(&context.repo_dir, runtime)? {
            labels.push(label);
        }
    }
    let viewer = if assignees.is_empty() {
        None
    } else {
        Some(viewer_login(context)?)
    };
    let assignees: Vec<String> = assignees
        .into_iter()
        .filter(|login| viewer.as_deref() != Some(login.as_str()))
        .collect();
    if labels.is_empty() && assignees.is_empty() {
        return Ok(());
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "foreign-projection",
        "labels": labels,
        "assignees": assignees,
        "action": "the store cannot explain this projection; an explicit `reclaim` takes it over",
    })))
}

/// The fields that make one ownership event the same event as another.
///
/// Compared as a whole rather than by run-id: a run that claims, releases and
/// claims again is the same run and a **different** holding, and projecting the
/// first one's sets over the second would be right about who and wrong about
/// what.
fn ownership_identity(event: Option<&ownership::Event>) -> serde_json::Value {
    match event {
        None => serde_json::Value::Null,
        Some(event) => serde_json::json!([
            event.created_at,
            event.position,
            event.run_id,
            event.kind,
            event.runtime,
            event.horizon,
            event.from,
            event.operation_id,
            event.forced,
        ]),
    }
}

fn read_until_agreed(
    read: &impl Fn() -> Result<serde_json::Value, Failure>,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> (Vec<Option<serde_json::Value>>, Option<Failure>) {
    let mut seen = Vec::new();
    for _ in 0..VISIBILITY_ATTEMPTS {
        match read() {
            Ok(data) => seen.push(Some(data)),
            Err(limited @ Failure::RateLimited(_)) => return (seen, Some(limited)),
            Err(_) => seen.push(None),
        }
        if wait_for(&seen, &predicate, 2).is_some() {
            break;
        }
    }
    (seen, None)
}

/// Projects the reducer's holder onto the issue, then proves the exact sets
/// landed.
///
/// After the claim marker is posted and read back, the projection — the
/// assignee and the `dev:<runtime>` label — is written and read back too,
/// refusing when it does not agree.
///
/// Bounded, and every arm of the bound says something different:
///
/// - **ownership changed before the projection** — the holder moved while this
///   was reading. The projection wrote nothing, so this is a stop and not an
///   ambiguity — though the caller's control marker is already on the timeline,
///   so it still declares `world: committed`.
/// - **projection did not read back exactly** — the write landed and the issue
///   does not show it. Ambiguous whenever this run wrote, because the edit may
///   have taken effect after the last read.
/// - **holder-runtime-missing** — the holder's runtime cannot be established, so
///   the assignee is deliberately *not* projected: unknown provenance must not
///   leave a previous holder standing as authoritative.
/// - **ownership-changed-projections-repaired** — the sets are right now, but
///   they are somebody else's; the caller asked about a claim it no longer has.
///
/// Every stop here carries `world: committed`: this function runs only after
/// the caller's control marker has been posted and read back, so *nothing was
/// written* would be about the projection rather than about the call.
pub fn converge_ownership_projection(
    context: &Context,
    issue: u64,
    event: Option<ownership::Event>,
    login: Option<&str>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    converge_ownership_projection_from(context, issue, event, login, now, Judge::Payload)
}

/// Where the holder a projection is judged against comes from.
pub(crate) enum Judge<'a> {
    /// The comments in each `gh issue view` payload, as before the store.
    Payload,
    /// The local store, which a read of GitHub cannot move.
    Store(&'a Ledger),
}

fn judged(
    judge: &Judge<'_>,
    data: &serde_json::Value,
    now: &str,
) -> Result<ownership::Holding, Failure> {
    Ok(match judge {
        Judge::Payload => holding(&comments_of(data), now),
        Judge::Store(ledger) => holding(&ledger.timeline()?, now),
    })
}

/// [`converge_ownership_projection`], told where the holder is judged from.
///
/// Only the holder comes from `judge`: the assignee and the `dev:` label are
/// projections, so they are read from GitHub whichever it is.
pub(crate) fn converge_ownership_projection_from(
    context: &Context,
    issue: u64,
    event: Option<ownership::Event>,
    login: Option<&str>,
    now: &str,
    judge: Judge<'_>,
) -> Result<serde_json::Value, Failure> {
    let fields = match judge {
        Judge::Payload => "assignees,labels,comments",
        Judge::Store(_) => "assignees,labels",
    };
    let login = match login {
        Some(named) if !named.is_empty() => named.to_owned(),
        _ => viewer_login(context)?,
    };
    let read = || -> Result<serde_json::Value, Failure> {
        super::gh_json(
            &["issue", "view", &issue.to_string(), "--json", fields],
            Some(&context.repo_dir),
        )?
        .ok_or_else(|| Failure::Read(format!("gh issue view {issue} returned nothing")))
    };
    // What this run has already done, carried across attempts. `wrote` is what
    // turns a later failed read from "nothing happened" into "nobody can say" —
    // the distinction the whole taxonomy exists for.
    let (mut changed, mut wrote) = (false, false);
    let mut event = event;

    for _ in 0..VISIBILITY_ATTEMPTS {
        let expected = ownership_identity(event.as_ref());
        let unreadable = |wrote: bool, failure: Failure| -> Failure {
            if wrote {
                Failure::Write("ownership projection repair could not be read back".to_owned())
            } else {
                failure
            }
        };

        // Two agreeing reads, because one is a snapshot and the tracker is
        // eventually consistent.
        let agrees = |data: &serde_json::Value| {
            judged(&judge, data, now)
                .is_ok_and(|held| ownership_identity(held.event.as_ref()) == expected)
        };
        let (seen, limited) = read_until_agreed(&read, agrees);
        if let Some(limit) = limited
            && wait_for(&seen, agrees, 2).is_none()
        {
            return Err(unreadable(wrote, limit));
        }
        if seen.iter().all(Option::is_none) {
            return Err(unreadable(
                wrote,
                Failure::Read(format!("gh issue view {issue} never answered")),
            ));
        }
        let settled = wait_for(&seen, agrees, 2);
        let Some(data) = settled else {
            // The holder moved under this read. Take the newest one and try
            // again — the projection belongs to whoever holds it now.
            let latest =
                seen.iter().rev().flatten().next().ok_or_else(|| {
                    unreadable(wrote, Failure::Read("no read answered".to_owned()))
                })?;
            event = judged(&judge, latest, now)?.event;
            changed = true;
            continue;
        };

        // The runtime, and the one case where it cannot be established. A legacy
        // holder carries none, so the `dev:` labels present are the only
        // evidence — and exactly one of them pointing at this holder is
        // evidence, while two are a guess.
        let mut runtime = event.as_ref().and_then(|event| event.runtime.clone());
        if let Some(held) = event.as_ref()
            && runtime.is_none()
        {
            let legacy: Vec<String> = labels_of(data)
                .iter()
                .filter_map(|name| name.strip_prefix("dev:").map(ToOwned::to_owned))
                .filter(|candidate| holder_uses_runtime(held, candidate))
                .collect();
            runtime = match legacy.as_slice() {
                [only] => Some(only.clone()),
                _ => None,
            };
        }
        let unresolved_runtime = event.is_some() && runtime.is_none();
        // Unknown provenance must not leave the previous holder projected as
        // authoritative, so the assignee is withheld rather than guessed.
        let wanted_assignees: std::collections::BTreeSet<String> =
            match (event.as_ref(), runtime.as_ref()) {
                (Some(_), Some(_)) => [login.clone()].into_iter().collect(),
                _ => Default::default(),
            };
        let wanted_runtimes: std::collections::BTreeSet<String> = runtime
            .as_ref()
            .map(|runtime| format!("dev:{runtime}"))
            .into_iter()
            .collect();
        let (assignees, runtimes) = projection_state(data);

        let mut edit: Vec<String> = ["gh", "issue", "edit", &issue.to_string()]
            .iter()
            .map(|part| (*part).to_owned())
            .collect();
        for gone in assignees.difference(&wanted_assignees) {
            edit.push("--remove-assignee".to_owned());
            edit.push(gone.clone());
        }
        if wanted_assignees.difference(&assignees).next().is_some() {
            edit.push("--add-assignee".to_owned());
            edit.push("@me".to_owned());
        }
        for gone in runtimes.difference(&wanted_runtimes) {
            edit.push("--remove-label".to_owned());
            edit.push(gone.clone());
        }
        for wanted in wanted_runtimes.difference(&runtimes) {
            super::commands::ensure_label(context, wanted, "bfd4f2")?;
            edit.push("--add-label".to_owned());
            edit.push(wanted.clone());
        }
        let edits = edit.len() > 4;
        if edits {
            let borrowed: Vec<&str> = edit.iter().map(String::as_str).collect();
            super::run(&borrowed, Some(&context.repo_dir), super::How::write())?;
            wrote = true;
        }

        // The proof. Both halves, because the sets landing is not the same fact
        // as the holder still being the holder they were projected for.
        let landed_as_wanted = |data: &serde_json::Value| {
            agrees(data)
                && projection_state(data) == (wanted_assignees.clone(), wanted_runtimes.clone())
        };
        let (after, limited) = read_until_agreed(&read, landed_as_wanted);
        if let Some(limit) = limited
            && wait_for(&after, landed_as_wanted, 2).is_none()
        {
            return Err(unreadable(wrote, limit));
        }
        let landed = wait_for(&after, landed_as_wanted, 2);
        let Some(landed) = landed else {
            let Some(latest) = after.iter().rev().flatten().next() else {
                return Err(unreadable(
                    wrote,
                    Failure::Read("ownership projection could not be re-read".to_owned()),
                ));
            };
            let now_held = judged(&judge, latest, now)?.event;
            if ownership_identity(now_held.as_ref()) != expected {
                event = now_held;
                changed = true;
                continue;
            }
            // Three different sentences, and the order is the transport's. An
            // attempt that **wrote** is ambiguous about its own write, and says
            // so in its own words; one that wrote on an *earlier* pass and not
            // this one is a repair that never settled; one that never wrote at
            // all is a plain stop, because nothing is in doubt.
            //
            // Collapsing the first two was this port's first mistake here: it
            // answered *did not stabilize* where the transport answers *did not
            // read back exactly*, which is the same verdict told about the wrong
            // write.
            return Err(if edits {
                Failure::Write("ownership projection write did not read back exactly".to_owned())
            } else if wrote {
                Failure::Write("ownership projection repair did not stabilize".to_owned())
            } else {
                Failure::Stop(serde_json::json!({
                    "ok": false,
                    "reason": "ownership projection write did not read back exactly",
                    "world": "committed",
                }))
            });
        };
        if unresolved_runtime {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false, "reason": "holder-runtime-missing", "world": "committed",
            })));
        }
        if changed {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "ownership-changed-projections-repaired",
                "world": "committed",
            })));
        }
        return Ok(landed.clone());
    }
    Err(if wrote {
        Failure::Write("ownership kept changing during bounded projection repair".to_owned())
    } else {
        Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "ownership-kept-changing-before-projection",
            "world": "committed",
        }))
    })
}

/// What a claim attempt decided, before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    /// This run already holds the issue. Adopt that epoch rather than write again.
    AlreadyWritten {
        /// The operation id on the live marker.
        operation_id: String,
    },
    /// Nothing stands in the way: write the claim.
    Write,
}

/// Whether a fresh claim may be written against the ownership that precedes it.
///
/// **Pure and fed**, so the three refusals can be checked without a tracker.
/// Each one is a different way of saying "not like this":
///
/// - **`stale-foreign-requires-reclaim`** — nobody holds it, but somebody else's
///   expired claim is on the timeline. Claiming over it would erase a takeover
///   that should have been declared; `reclaim` is the operation that says so out
///   loud.
/// - **`already-owned-by-different-operation`** — this run already holds it, but
///   the live marker has no adoptable operation id. A second acquisition would
///   make one run appear twice. A live marker that *does* name an epoch is not
///   this refusal: it is [`Attempt::AlreadyWritten`].
/// - and when neither applies, the horizon still has to be **ahead of now**,
///   because a claim whose deadline has passed is stale the moment it lands.
pub fn may_claim(before: &ownership::Holding, run_id: &str) -> Result<Attempt, Failure> {
    let foreign_stale: Vec<String> = {
        let mut names: Vec<String> = before
            .stale
            .iter()
            .filter(|event| event.run_id != run_id)
            .map(|event| event.run_id.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    };
    if before.holder.is_none() && !foreign_stale.is_empty() {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "stale-foreign-requires-reclaim", "holders": foreign_stale,
        })));
    }
    if let Some(mine) = before.live.iter().find(|event| event.run_id == run_id) {
        // A claim that finds this same run already holding this issue is a
        // renewal whose key was lost, not a conflict. The write landed; the
        // pointer recorded nothing; the retry minted a fresh key. Measured
        // 2026-08-17 during a GitHub outage, five times in one session.
        // Adopting writes no second comment and lets Swear complete the pointer.
        if let Some(operation_id) = mine
            .operation_id
            .clone()
            .filter(|id| ownership::is_operation_id(id))
        {
            return Ok(Attempt::AlreadyWritten { operation_id });
        }
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "already-owned-by-different-operation",
            "operation_id": mine.operation_id,
        })));
    }
    Ok(Attempt::Write)
}

/// How a claim's adjudication ended, once its event is on the timeline.
///
/// Read **after** the write, from the timeline rather than from the write's own
/// answer: a claim is not won by posting, it is won by being the earliest live
/// acquisition once everybody has posted.
///
/// The refusals that follow the write — `lost-claim-race` and
/// `claim-operation-no-longer-current` — carry `world: committed`: the write
/// had already landed, read visible by the visibility loop or found by this
/// retry's first read. `claim-operation-no-longer-current` is reached when that
/// marker left the timeline before this read, so the flag reports the landing,
/// not the marker's survival.
pub fn adjudicate(
    after: &ownership::Holding,
    operation_id: &str,
    run_id: &str,
    reused: bool,
) -> Result<serde_json::Value, Failure> {
    let active = after
        .live
        .iter()
        .chain(&after.stale)
        .find(|event| event.operation_id.as_deref() == Some(operation_id));
    let Some(active) = active else {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "claim-operation-no-longer-current",
            "winner": after.holder, "world": "committed",
        })));
    };
    // A resumed operation whose event has since expired is not a claim to carry
    // on with: the deadline it declared has passed, whoever wrote it.
    if reused
        && after
            .stale
            .iter()
            .any(|event| event.position == active.position)
    {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "claim-operation-expired",
            "operation_id": operation_id,
            "claimed_at": active.created_at,
        })));
    }
    if after.event.is_none() {
        return Err(Failure::Write(
            "claim became stale before adjudication; its event was written".to_owned(),
        ));
    }
    if after.holder.as_deref() != Some(run_id) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "lost-claim-race", "winner": after.holder,
            "world": "committed",
        })));
    }
    Ok(serde_json::json!({
        "ok": true,
        "run_id": run_id,
        "reused_existing_claim": reused,
        "claimed_at": active.created_at,
        "horizon": active.horizon,
        "next": "transition to in-progress before any repository write",
    }))
}

/// Everything one acquisition needs to say about itself.
#[derive(Debug, Clone)]
pub struct Acquisition<'a> {
    /// The issue being claimed.
    pub issue: u64,
    /// The run claiming it.
    pub run_id: &'a str,
    /// The runtime it will project as `dev:<runtime>`.
    pub runtime: &'a str,
    /// The deadline it declares.
    pub horizon: &'a str,
    /// The operation id — **fresh once, reused on every retry**.
    pub operation_id: &'a str,
    /// The moment the timeline is judged against.
    ///
    /// **Read from the machine, never from the run being judged.** The binding
    /// takes it from `utc_now_stamp()` for a reason that is not tidiness: this
    /// value decides whether a claim is still live, so a run that supplies it
    /// decides whether its own claim has expired. The port accepts it as an
    /// argument because a test has to be able to stand at a chosen moment; that
    /// is the only caller allowed to choose one.
    pub now: &'a str,
}

/// Whether the operation id resuming this call already names a **dead**
/// acquisition of `run_id`'s own — one the timeline no longer holds live,
/// whichever call wrote it.
///
/// The whole timeline is read rather than the epoch [`ownership::Holding`]
/// reduces to: a later foreign `reclaim` drops this run's earlier acquisition
/// out of `live` and `stale` both, and the id it left behind is dead all the
/// same. Read over [`ownership::ownership_events`], not the authoritative
/// subset, so it sees every acquisition marker the kind and metadata checks
/// below would scan. Does not read `event.kind`, because `claim` and `reclaim`
/// share one operation-id space.
fn is_dead_self_operation(
    acquisitions: &[ownership::Event],
    live: &[ownership::Event],
    run_id: &str,
    operation_id: &str,
) -> bool {
    acquisitions
        .iter()
        .filter(|event| event.run_id == run_id)
        .filter(|event| event.operation_id.as_deref() == Some(operation_id))
        .any(|event| !live.iter().any(|held| held.position == event.position))
}

/// `claim` — write or resume one operation-scoped acquisition.
///
/// The operation id is what makes a retry safe: it names **one event**, so a
/// second attempt finds its own earlier marker and resumes rather than writing a
/// second claim. That is why the id must be fresh once and reused thereafter,
/// and why using one id for two kinds is refused outright.
///
/// The adjudication is read from the **timeline**, never from the write's own
/// answer. A claim is not won by posting; it is won by being the earliest live
/// acquisition once everybody has posted.
///
/// - **`claim-operation-expired`** — the id this call would resume already
///   names a **dead** acquisition of this run's own, whichever call wrote it:
///   one the timeline no longer holds live, whether it lapsed or a later
///   foreign `reclaim` closed the epoch it sat in. Not an operation to resume;
///   a fresh `claim` under a new id is what the caller wants, and this stops
///   before the checks below would answer something narrower for the same
///   fact — a wrong kind, or a metadata mismatch.
pub fn claim(context: &Context, what: &Acquisition<'_>) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(what.operation_id))?;
    require_horizon(what.horizon, None)?;

    let ledger = Ledger::open(context, what.issue, what.now)?;
    if !super::outbox::is_provisional(what.issue) {
        require_explainable_projection(context, &ledger, what)?;
    }
    let decided = ledger.transact(|tx| {
        let comments = tx.timeline()?;
        require_review_eligibility(&comments, what.run_id)?;

        // Bound once and read twice below, so both decisions — is the id
        // resuming this call dead, and does a fresh one have anywhere to write —
        // answer from the one snapshot the comment above `is_dead_self_operation`
        // claims they share.
        let before = holding(&comments, what.now);

        // An id that already names this same run's own acquisition, dead, is not
        // an operation to resume — whichever call wrote it, and whether it lapsed
        // or a later foreign `reclaim` closed the epoch it sat in. Left to the
        // checks below, that shows up as a wrong kind when the dead event was a
        // `reclaim`, or as a metadata mismatch when the caller sensibly asks for
        // a horizon that has not already passed — two different refusals for one
        // fact, neither naming it. `adjudicate` already has the name for it, for
        // the same event found stale after a write; this is the same check, moved
        // ahead of the two that would otherwise answer first and hide it. Only a
        // dead acquisition of this run's own reaches here — a live one is the
        // retry the checks below exist to resume.
        if is_dead_self_operation(
            &ownership::ownership_events(&comments),
            &before.live,
            what.run_id,
            &operation_id,
        ) {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "claim-operation-expired",
                "operation_id": operation_id,
            })));
        }

        let mut operation_id = operation_id.clone();
        reject_operation_kind_conflict(&comments, &operation_id, &["claim", "standdown"])?;
        let expected = [("run-id", what.run_id), ("horizon", what.horizon)];
        let existing = operation_marker(&comments, &operation_id, OperationKind::Claim, &expected)?;

        let mut reused = existing.is_some();
        let mut wrote = false;
        if existing.is_none() {
            match may_claim(&before, what.run_id)? {
                Attempt::AlreadyWritten {
                    operation_id: adopted,
                } => {
                    operation_id = adopted;
                    reused = true;
                    reject_operation_kind_conflict(
                        &comments,
                        &operation_id,
                        &["claim", "standdown"],
                    )?;
                    let adopted_expected = [("run-id", what.run_id)];
                    if operation_marker(
                        &comments,
                        &operation_id,
                        OperationKind::Claim,
                        &adopted_expected,
                    )?
                    .is_none()
                    {
                        return Err(Failure::Write(
                            "claim operation is not visible; retry with the same operation ID"
                                .to_owned(),
                        ));
                    }
                }
                Attempt::Write => {
                    // Ahead of *now*, not merely well formed: a claim whose deadline
                    // has already passed is stale the moment it lands.
                    require_horizon(what.horizon, Some(what.now))?;
                    let marker = super::markers::render(
                        "claim",
                        &[
                            ("run-id", what.run_id),
                            ("runtime", what.runtime),
                            ("horizon", what.horizon),
                            ("op-id", &operation_id),
                        ],
                    )
                    .ok_or_else(|| {
                        Failure::Stop(
                            serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }),
                        )
                    })?;
                    let body = format!("{marker}\n");
                    tx.append(&body, what.now)?;
                    wrote = true;
                }
            }
        }

        let after = holding(&tx.timeline()?, what.now);
        let adjudication = adjudicate(&after, &operation_id, what.run_id, reused);
        if wrote && is_lost_claim_race(&adjudication) {
            let Err(failure) = adjudication else {
                unreachable!("just matched a lost-claim-race error")
            };
            return Err(rolled_back(failure));
        }
        Ok(Decided {
            wrote,
            event: after.event,
            adjudication,
        })
    })?;

    let closed = super::outbox::is_closed(context);
    if !closed && (decided.wrote || decided.adjudication.is_ok()) {
        super::commands::ensure_label(context, &format!("dev:{}", what.runtime), "bfd4f2")
            .map_err(|failure| {
                Failure::Write(format!(
                    "the claim is recorded in the local store but its label could not be \
                     made: {}",
                    failure.detail()
                ))
            })?;
    }
    let (adjudication, event) = (decided.adjudication, decided.event);
    let mut answer = adjudication?;

    // The projection, and not one step earlier: `adjudicate` is what says this
    // run holds the issue, and projecting for a run that lost the race would
    // put its name on somebody else's work.
    //
    // This call is the half the port did not have. Without it the marker landed
    // and the issue page showed nothing — no assignee, and whatever `dev:` label
    // the previous holder left behind — so this side answered `ok` at the point
    // the transport was still working.
    let deferred = project_ownership(context, what.issue, event, what.now, &ledger)?;

    answer["issue"] = serde_json::json!(what.issue);
    mark_deferred(&mut answer, deferred);
    if !closed && let Ok(Some(current)) = current_review_transaction(context, what.issue) {
        answer["review_transaction"] = serde_json::json!({
            "epoch": current.epoch,
            "pr": current.pr,
            "head": current.head,
            "base": current.base,
            "digest": current.digest,
        });
    }
    Ok(answer)
}

/// Moves the issue's assignee and `dev:` label to the holder the store names,
/// or, while the git operation window is closed, queues that move and answers
/// the place it took in the queue.
fn project_ownership(
    context: &Context,
    issue: u64,
    event: Option<ownership::Event>,
    now: &str,
    ledger: &Ledger,
) -> Result<Option<u64>, Failure> {
    if super::outbox::is_closed(context) {
        return super::outbox::queue(
            context,
            issue,
            super::store::OutboxKind::Reconverge,
            &serde_json::json!({ "ownership": true }),
            now,
        )
        .map(Some);
    }
    converge_ownership_projection_from(context, issue, event, None, now, Judge::Store(ledger))?;
    Ok(None)
}

/// Says in an answer that its projection was queued rather than made.
fn mark_deferred(answer: &mut serde_json::Value, queued: Option<u64>) {
    if let Some(queued) = queued {
        answer["projection"] = serde_json::json!("deferred");
        answer["queued"] = serde_json::json!(queued);
    }
}

struct Decided {
    wrote: bool,
    event: Option<ownership::Event>,
    adjudication: Result<serde_json::Value, Failure>,
}

/// Whether this adjudication is a fresh claim that wrote its marker and still
/// lost: the one outcome `claim` rolls back rather than commit.
fn is_lost_claim_race(adjudication: &Result<serde_json::Value, Failure>) -> bool {
    matches!(
        adjudication,
        Err(Failure::Stop(body))
            if body.get("reason").and_then(serde_json::Value::as_str) == Some("lost-claim-race")
    )
}

/// A lost claim's refusal once its marker is rolled back: the world did not
/// change, so it drops the `world: committed` a landed write would carry.
fn rolled_back(failure: Failure) -> Failure {
    match failure {
        Failure::Stop(mut body) => {
            if let Some(object) = body.as_object_mut() {
                object.remove("world");
            }
            Failure::Stop(body)
        }
        other => other,
    }
}

/// The label names on an issue payload, in the order the tracker gave them.
fn labels_of(data: &serde_json::Value) -> Vec<String> {
    data.get("labels")
        .and_then(serde_json::Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(|label| label.get("name").and_then(serde_json::Value::as_str))
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Whether an event's holder runs on `runtime`.
///
/// Whether a holding event belongs to a runtime.
///
/// Markers carry a `runtime` now; the run-id prefix is what keeps a marker
/// written before they did readable. It decides which `dev:` label a **legacy**
/// holder's projection converges on, and a legacy holder is the one case where
/// the runtime cannot be read off the marker at all.
///
/// Public: the harness's tests reach it.
pub fn holder_uses_runtime(event: &ownership::Event, runtime: &str) -> bool {
    event.runtime.as_deref() == Some(runtime) || event.run_id.starts_with(&format!("{runtime}-"))
}

/// What a release should do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Release {
    /// Nothing has been written. The caller is told **which** epoch it would
    /// release and asked to repeat the call naming it.
    ///
    /// Two phases on purpose: discovery is read-only, and the write then binds
    /// one exact acquisition. A release that just said "whatever I hold" would
    /// end an epoch acquired *after* the discovery — including one somebody else
    /// handed over in between.
    Confirm {
        /// The epoch this release would end.
        target_operation: String,
    },
    /// Release exactly this epoch.
    Write {
        /// The epoch to end, named by the caller and confirmed here.
        target_operation: String,
    },
}

/// Decides what a release does, before anything is written.
///
/// **Pure and fed.** Four refusals, and each names a different mistake:
///
/// - **`nothing-to-unassign`** — this run holds nothing. Releasing would be a
///   write with no subject.
/// - **`held-by-other-without-other-holder`** — the caller said somebody else
///   holds it and nobody else does. Believing that would release the item on the
///   strength of a claim the timeline contradicts.
/// - **`holder-runtime-missing`** — the successor left in place declared no
///   runtime, and the `dev:` labels on the issue do not resolve to exactly one
///   either. Releasing here would leave a holder nobody can project a label for.
/// - **`target-operation-mismatch`** — the epoch named is not the one that would
///   be released. Between the discovery and the write, ownership moved.
pub fn plan_release(
    ownership: &ownership::Holding,
    run_id: &str,
    held_by_other: bool,
    requested_target: Option<&str>,
    projected: &[String],
) -> Result<Release, Failure> {
    let mine = ownership
        .live
        .iter()
        .chain(&ownership.stale)
        .find(|event| event.run_id == run_id);
    let Some(mine) = mine else {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "nothing-to-unassign" }),
        ));
    };

    let other_live = ownership.live.iter().any(|event| event.run_id != run_id);
    if held_by_other && !other_live {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "held-by-other-without-other-holder",
        })));
    }

    // Handing the issue on: whoever is left has to be nameable. A successor
    // that declared no runtime is only identifiable through the `dev:` labels
    // already on the issue, so unless those resolve to exactly one runtime,
    // releasing here would leave a holder nobody can project a label for.
    let dev: Vec<&str> = projected
        .iter()
        .filter_map(|name| name.strip_prefix("dev:"))
        .collect();
    let successor = ownership.live.iter().find(|event| event.run_id != run_id);
    if Some(mine) == ownership.event.as_ref()
        && let Some(successor) = successor
        && successor.runtime.is_none()
        && dev
            .iter()
            .filter(|name| holder_uses_runtime(successor, name))
            .count()
            != 1
    {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "holder-runtime-missing",
        })));
    }

    let target = ownership::ownership_epoch(
        mine.operation_id.as_deref(),
        mine.comment.id.as_deref(),
        &mine.comment.created_at,
        &mine.comment.body,
    );
    match requested_target {
        None => Ok(Release::Confirm {
            target_operation: target,
        }),
        Some(requested) if requested != target => Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "target-operation-mismatch",
            "expected": target,
            "provided": requested,
        }))),
        Some(_) => Ok(Release::Write {
            target_operation: target,
        }),
    }
}

/// Everything one release needs to say about itself.
#[derive(Debug, Clone)]
pub struct Departure<'a> {
    /// The issue being released.
    pub issue: u64,
    /// The run releasing it.
    pub run_id: &'a str,
    /// Its runtime. Attribution written into the marker, never compared: the
    /// run id is the identity a release has to match.
    pub runtime: &'a str,
    /// The operation id — fresh once, reused on retry.
    pub operation_id: &'a str,
    /// The epoch to end. Absent on the discovery call.
    pub target_operation: Option<&'a str>,
    /// Whether the caller believes somebody else holds it.
    pub held_by_other: bool,
    /// The isolated checkout this run created, when the harness knows it — and
    /// it is the harness that fills this from the run pointer, never a caller:
    /// a `release` that removed a directory it was merely handed would be the
    /// unsafe direction of exactly the operation that has to be safe.
    pub worktree: Option<&'a std::path::Path>,
    /// The moment the timeline is judged against.
    ///
    /// **Read from the machine, never from the run being judged.** The binding
    /// takes it from `utc_now_stamp()` for a reason that is not tidiness: this
    /// value decides whether a claim is still live, so a run that supplies it
    /// decides whether its own claim has expired. The port accepts it as an
    /// argument because a test has to be able to stand at a chosen moment; that
    /// is the only caller allowed to choose one.
    pub now: &'a str,
}

/// `unassign` — release the item without changing its state.
///
/// Discovery is read-only and the write binds **one exact acquisition epoch**.
/// A release that ended "whatever I hold" would end one obtained after the
/// discovery — including one somebody else handed over in between.
///
/// The state is deliberately untouched: putting an item down is not the same as
/// moving it backwards, and deciding where it should go instead is somebody
/// else's call.
pub fn unassign(context: &Context, what: &Departure<'_>) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(what.operation_id))?;
    let ledger = Ledger::open(context, what.issue, what.now)?;
    // The `dev:` labels are a projection, read only to name a successor whose
    // marker declared no runtime. The ownership they are read beside is not.
    let projected = if super::outbox::is_provisional(what.issue) {
        Vec::new()
    } else {
        labels_of(
            &super::gh_json(
                &[
                    "issue",
                    "view",
                    &what.issue.to_string(),
                    "--json",
                    "assignees,labels",
                ],
                Some(&context.repo_dir),
            )?
            .ok_or_else(|| {
                Failure::Read(format!("gh issue view {} returned nothing", what.issue))
            })?,
        )
    };

    let decided = ledger.transact(|tx| {
        let comments = tx.timeline()?;
        reject_operation_kind_conflict(&comments, &operation_id, &["unassign"])?;

        let ownership = holding(&comments, what.now);

        // The marker first, as the transport has it. This planned the release
        // before looking, and a plan made after the write has landed says
        // `nothing-to-unassign` — because there is nothing left to release. A run
        // repeating its own unassign after an ambiguous write was told its
        // release had never happened, which is the opposite of what was true.
        let mut sought: Vec<(&str, &str)> = vec![("run-id", what.run_id)];
        if let Some(requested) = what.target_operation {
            sought.push(("target-op", requested));
        }
        if let Some(existing) =
            operation_marker(&comments, &operation_id, OperationKind::Unassign, &sought)?
        {
            return Ok(Released::Resumed {
                comments,
                ownership,
                existing,
            });
        }

        let plan = plan_release(
            &ownership,
            what.run_id,
            what.held_by_other,
            what.target_operation,
            &projected,
        )?;
        let Release::Write { target_operation } = plan else {
            let Release::Confirm { target_operation } = plan else {
                unreachable!("the plan is one of two shapes")
            };
            return Ok(Released::Discovered { target_operation });
        };

        let expected = [
            ("run-id", what.run_id),
            ("target-op", target_operation.as_str()),
        ];
        if operation_marker(&comments, &operation_id, OperationKind::Unassign, &expected)?.is_none()
        {
            let marker = super::markers::render(
                "unassign",
                &[
                    ("run-id", what.run_id),
                    ("runtime", what.runtime),
                    ("op-id", &operation_id),
                    ("target-op", &target_operation),
                ],
            )
            .ok_or_else(|| {
                Failure::Stop(
                    serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }),
                )
            })?;
            tx.append(&format!("{marker}\n"), what.now)?;
        }
        let comments = tx.timeline()?;
        Ok(Released::Written {
            target_operation,
            after: holding(&comments, what.now),
            comments,
        })
    })?;

    let (target_operation, after, comments) = match decided {
        Released::Resumed {
            comments,
            ownership,
            existing,
        } => {
            return resumed_release(
                context,
                &comments,
                &ownership,
                &existing,
                &operation_id,
                what,
            );
        }
        Released::Discovered { target_operation } => {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": what.issue,
                "write_performed": false,
                "target_operation": target_operation,
                "next": "repeat unassign with --target-operation and the same operation ID",
            }));
        }
        Released::Written {
            target_operation,
            after,
            comments,
        } => (target_operation, after, comments),
    };

    // Read back, and **believe the read-back**: a release is not done because
    // the comment posted, it is done because the epoch stopped being
    // authoritative.
    let still_authoritative = after.live.iter().chain(&after.stale).any(|event| {
        ownership::ownership_epoch(
            event.operation_id.as_deref(),
            event.comment.id.as_deref(),
            &event.comment.created_at,
            &event.comment.body,
        ) == target_operation
    });
    if still_authoritative {
        return Err(Failure::Write(
            "unassign is visible but its target epoch remains authoritative".to_owned(),
        ));
    }
    // The caller said somebody else holds it. If after the release nobody does —
    // or this run still does — then what it believed was already false, and it
    // has just written on a premise the timeline never supported.
    let nobody_else = after.holder.is_none() || after.holder.as_deref() == Some(what.run_id);
    if what.held_by_other && nobody_else {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "held-by-other-holder-disappeared",
        })));
    }

    // The third caller, and the one that says this is a mechanism rather than a
    // step two commands happen to share. A release moves the projection off the
    // run that let go — or onto whoever the timeline says holds it now, which is
    // why the reducer's event is what it is told rather than "nobody".
    let deferred = project_ownership(context, what.issue, after.event.clone(), what.now, &ledger)?;

    let mut answer = release_answer(context, what, after.holder.is_some(), &comments);
    mark_deferred(&mut answer, deferred);
    Ok(answer)
}

enum Released {
    Resumed {
        comments: Vec<ownership::Comment>,
        ownership: ownership::Holding,
        existing: super::markers::Marker,
    },
    Discovered {
        target_operation: String,
    },
    Written {
        target_operation: String,
        after: ownership::Holding,
        comments: Vec<ownership::Comment>,
    },
}

/// Every branch this run recorded starting, in the order the issue names them.
///
/// Read from the `branch` markers `start_branch` posts rather than from the
/// pull request's own head, because the two are not always the same branch: a
/// branch delivered through another, or pushed under a name other than the pull
/// request's, is one `delete_branch_on_merge` never touches.
fn recorded_branches(comments: &[ownership::Comment], run_id: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for comment in comments {
        for marker in super::markers::parse(&comment.body) {
            if marker.get("kind").map(String::as_str) != Some("branch")
                || marker.get("run-id").map(String::as_str) != Some(run_id)
            {
                continue;
            }
            if let Some(branch) = marker.get("branch")
                && !branch.is_empty()
                && !found.contains(branch)
            {
                found.push(branch.clone());
            }
        }
    }
    found
}

/// The local half of a release: put down the checkout this run created, once
/// the tracker has confirmed the delivery it is released from.
///
/// Never a refusal. The release has already landed when this runs, so a
/// checkout that cannot be put down is kept and named in the answer rather than
/// turned into a failed release — and the named reason is what the next reader
/// needs, because the pointer it could have been found through is about to be
/// forgotten.
fn put_down_checkout(
    context: &Context,
    worktree: &std::path::Path,
    issue: u64,
    run_id: &str,
    comments: &[ownership::Comment],
) -> serde_json::Value {
    match close::delivery_close(context, issue, comments, None, None) {
        Ok(Some(own)) => {
            // The remote heads first: the deletion pushes from this checkout,
            // and `retire_checkout` removes it. A release run from inside its
            // own worktree would otherwise be pushing from a directory that no
            // longer exists.
            let branches = recorded_branches(comments, run_id);
            let remote = (!branches.is_empty()).then(|| {
                super::branch::retire_remote_branches(context, &branches)
                    .iter()
                    .map(super::branch::RemoteRetired::to_json)
                    .collect::<Vec<_>>()
            });
            let mut answer = super::branch::retire_checkout(context, worktree, &own.head).to_json();
            if let Some(remote) = remote {
                answer["remote_branches"] = serde_json::Value::Array(remote);
            }
            answer
        }
        Ok(None) => serde_json::json!({
            "removed": false,
            "outcome": "kept",
            "worktree": worktree.display().to_string(),
            "detail": "the issue's close is not the registered delivery's merge, so the checkout is \
                       preserved",
        }),
        Err(failure) => serde_json::json!({
            "removed": false,
            "outcome": "kept",
            "worktree": worktree.display().to_string(),
            "detail": format!("the delivery could not be read back: {}", failure.detail()),
        }),
    }
}

/// The answer both release paths return, so a retry names its checkout too.
///
/// [`unassign`] reaches this after its own write; [`resumed_release`] reaches it
/// on a retry, where the marker is already on the issue. The checkout half used
/// to run only on the first path, so a release that posted its marker and then
/// failed before cleanup left a delivered checkout with no record and no report
/// — the pointer is forgotten on the way out either way.
fn release_answer(
    context: &Context,
    what: &Departure<'_>,
    assignee_kept: bool,
    comments: &[ownership::Comment],
) -> serde_json::Value {
    let mut answer = serde_json::json!({
        "ok": true,
        "issue": what.issue,
        "assignee_kept": assignee_kept,
    });
    if let Some(worktree) = what.worktree {
        answer["checkout"] =
            put_down_checkout(context, worktree, what.issue, what.run_id, comments);
    }
    answer
}

/// What a takeover should do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Takeover {
    /// Nothing written. The caller is told **which** epoch it would displace and
    /// asked to repeat the call naming it. Discovery is read-only for the same
    /// reason a release's is: between looking and writing, ownership can move.
    Confirm {
        /// The epoch this takeover would displace.
        target_operation: String,
        /// Who currently holds it.
        holder: String,
    },
    /// Displace exactly this epoch, held by this run.
    Write {
        /// The epoch to displace.
        target_operation: String,
        /// The run being displaced.
        holder: String,
    },
}

/// Decides what a takeover does, before anything is written.
///
/// **Pure and fed.** Five refusals, and the distinctions between them are the
/// point — each one sends the caller somewhere different:
///
/// - **`nothing-to-reclaim`** — the timeline holds no acquisition at all. There
///   is nothing to take.
/// - **`stale-self-requires-claim`** — the only expired acquisition is this run's
///   own. Taking over from yourself is not a takeover; it is a fresh `claim`.
/// - **`already-yours`** — the live holder is this run. Same answer, said before
///   any epoch is bound.
/// - **`holder-not-stale`** — the holder is alive and this is not forced. Taking
///   a live holder quietly is the one thing a reclaim must never do; saying
///   `--force` is what makes it declarable, and evidence is what makes it
///   answerable for.
/// - **`target-operation-mismatch`** — the epoch named is not the one that would
///   be displaced. Ownership moved between the discovery and the write.
pub fn plan_takeover(
    before: &ownership::Holding,
    run_id: &str,
    forced: bool,
    requested_target: Option<&str>,
) -> Result<Takeover, Failure> {
    let stop = |reason: &str| Failure::Stop(serde_json::json!({ "ok": false, "reason": reason }));

    let target = before.event.clone().or_else(|| {
        before
            .stale
            .iter()
            .find(|event| event.run_id != run_id)
            .cloned()
    });
    let Some(target) = target else {
        if before.stale.is_empty() {
            return Err(stop("nothing-to-reclaim"));
        }
        // The bare reason named a rule and not a way through it: `claim` is
        // where that rule points. Through the module's own `stop`, not the
        // closure of the same name bound just above — this refusal is the one
        // here that needs the other two arguments.
        return Err(self::stop(
            "stale-self-requires-claim",
            "the only expired acquisition on this issue is this run's own".to_owned(),
            "claim it again \u{2014} a lapsed claim of your own does not block that; \
             displacing yourself is not a takeover, which is why `reclaim` is not the call \
             for it",
        ));
    };
    if target.run_id == run_id {
        return Err(stop("already-yours"));
    }
    let target_is_stale = before
        .stale
        .iter()
        .any(|event| event.position == target.position);
    if !target_is_stale && !forced {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "holder-not-stale", "holder": target.run_id,
        })));
    }

    let epoch = ownership::ownership_epoch(
        target.operation_id.as_deref(),
        target.comment.id.as_deref(),
        &target.comment.created_at,
        &target.comment.body,
    );
    match requested_target {
        None => Ok(Takeover::Confirm {
            target_operation: epoch,
            holder: target.run_id,
        }),
        Some(requested) if requested != epoch => Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "target-operation-mismatch",
            "expected": epoch,
            "provided": requested,
        }))),
        Some(_) => Ok(Takeover::Write {
            target_operation: epoch,
            holder: target.run_id,
        }),
    }
}

/// Everything one takeover needs to say about itself.
#[derive(Debug, Clone)]
pub struct Reclaim<'a> {
    /// The issue being taken over.
    pub issue: u64,
    /// The run taking it.
    pub run_id: &'a str,
    /// Its runtime.
    pub runtime: &'a str,
    /// The deadline it declares.
    pub horizon: &'a str,
    /// The operation id — fresh once, reused on retry.
    pub operation_id: &'a str,
    /// The epoch to displace. Absent on the discovery call.
    pub target_operation: Option<&'a str>,
    /// Whether this takes a holder that is **not** stale.
    pub force: bool,
    /// The evidence file, required when forcing.
    pub reason_file: Option<&'a std::path::Path>,
    /// The moment the timeline is judged against.
    ///
    /// **Read from the machine, never from the run being judged.** The binding
    /// takes it from `utc_now_stamp()` for a reason that is not tidiness: this
    /// value decides whether a claim is still live, so a run that supplies it
    /// decides whether its own claim has expired. The port accepts it as an
    /// argument because a test has to be able to stand at a chosen moment; that
    /// is the only caller allowed to choose one.
    pub now: &'a str,
}

/// `reclaim` — take over work whose holder never came back.
///
/// Discovery is read-only; the write binds the takeover **and its evidence** to
/// one exact ownership epoch. Evidence that is not bound is transferable, and
/// transferable evidence justifies any takeover rather than this one.
///
/// A reason without `--force` is refused outright: supplying evidence for a
/// takeover that is not declaring itself forced means one of the two is a
/// mistake, and guessing which would either lose the evidence or force a
/// takeover nobody asked to force.
pub fn reclaim(context: &Context, what: &Reclaim<'_>) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(what.operation_id))?;
    require_horizon(what.horizon, None)?;
    if what.reason_file.is_some() && !what.force {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "force-required-for-reason" }),
        ));
    }

    let ledger = Ledger::open(context, what.issue, what.now)?;
    if !ledger.is_open() {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "issue-not-open" }),
        ));
    }

    let decided = ledger.transact(|tx| {
        let comments = tx.timeline()?;
        require_review_eligibility(&comments, what.run_id)?;
        reject_operation_kind_conflict(&comments, &operation_id, &["reclaim"])?;
        require_horizon(what.horizon, Some(what.now))?;
        let before = holding(&comments, what.now);
        let plan = plan_takeover(&before, what.run_id, what.force, what.target_operation)?;
        let (target_operation, holder) = match plan {
            Takeover::Confirm {
                target_operation,
                holder,
            } => {
                return Ok(Reclaimed::Discovered {
                    target_operation,
                    holder,
                });
            }
            Takeover::Write {
                target_operation,
                holder,
            } => (target_operation, holder),
        };

        // The evidence, and the digest that binds it to *this* takeover.
        let mut evidence_hash = None;
        let mut note = String::new();
        if what.force {
            let Some(file) = what.reason_file else {
                return Err(Failure::Stop(
                    serde_json::json!({ "ok": false, "reason": "force-reason-required" }),
                ));
            };
            let raw = std::fs::read_to_string(file).map_err(|_| {
                Failure::Stop(serde_json::json!({ "ok": false, "reason": "force-reason-invalid" }))
            })?;
            let reason = super::markers::escape_control_input(&raw).trim().to_owned();
            if reason.is_empty() {
                return Err(Failure::Stop(
                    serde_json::json!({ "ok": false, "reason": "force-reason-required" }),
                ));
            }
            evidence_hash = Some(ownership::forced_reclaim_hash(
                Some(&operation_id),
                &ownership::sha256_hex(reason.as_bytes()),
                what.run_id,
                Some(what.runtime),
                Some(what.horizon),
                Some(holder.as_str()),
                Some(&target_operation),
            ));
            note = format!(
                "

{}

{reason}",
                ownership::FORCED_EVIDENCE_HEADING
            );
        }

        let mut attributes: Vec<(&str, &str)> = vec![
            ("run-id", what.run_id),
            ("runtime", what.runtime),
            ("horizon", what.horizon),
            ("op-id", &operation_id),
            ("from", &holder),
            ("from-op", &target_operation),
        ];
        if what.force {
            attributes.push(("forced", "true"));
        }
        if let Some(hash) = &evidence_hash {
            attributes.push(("evidence-hash", hash));
        }
        let marker = super::markers::render("reclaim", &attributes).ok_or_else(|| {
            Failure::Stop(serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }))
        })?;

        let expected = [
            ("run-id", what.run_id),
            ("horizon", what.horizon),
            ("from-op", target_operation.as_str()),
        ];
        // A reclaim marker already under this operation id is a **retry**, not a
        // second takeover, and the transport answers it on a path of its own. This
        // had no such path: it skipped the write, fell into the read-back below,
        // and reported `reused_existing_reclaim: false` — telling a caller nothing
        // was already there when something was.
        if let Some(existing) =
            operation_marker(&comments, &operation_id, OperationKind::Reclaim, &expected)?
        {
            // The evidence a forced takeover gave is part of what the operation id
            // identifies. This computed the hash only to write it, and never once
            // compared it to what is already on the issue — so a retry handed a
            // *different* reason file found the old marker, wrote nothing, and
            // returned success, while the justification recorded against the
            // takeover stayed the one nobody meant to give.
            let forced = resumed_reclaim(
                &existing,
                evidence_hash.as_deref(),
                holding(&comments, what.now).event,
                &operation_id,
            )?;
            return Ok(Reclaimed::Resumed { holder, forced });
        }

        let body = format!(
            "{}

{marker}
",
            note.trim_start()
        );
        tx.append(&body, what.now)?;

        Ok(Reclaimed::Written { holder })
    })?;

    let holder = match decided {
        Reclaimed::Discovered {
            target_operation,
            holder,
        } => {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": what.issue,
                "write_performed": false,
                "target_operation": target_operation,
                "holder": holder,
                "next": "repeat claim with --takeover --target-operation and the same operation ID",
            }));
        }
        Reclaimed::Resumed { holder, forced } => {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": what.issue,
                "reclaimed_from": holder,
                "run_id": what.run_id,
                "forced": forced,
                "reused_existing_reclaim": true,
            }));
        }
        Reclaimed::Written { holder } => holder,
    };
    let comments = ledger.timeline()?;
    let after = holding(&comments, what.now);
    let event = after.event.clone();
    let forced = won_the_reclaim(after.event, what.run_id, &comments, &operation_id)?;

    // A takeover moves the projection as surely as a claim does, and moving it
    // is most of the point: the displaced holder's assignee and `dev:` label
    // stay on the issue until somebody takes them off, and an issue that still
    // names the run it was taken from is an issue nobody can read correctly.
    let deferred = project_ownership(context, what.issue, event, what.now, &ledger)?;

    let mut answer = serde_json::json!({
        "ok": true,
        "issue": what.issue,
        "reclaimed_from": holder,
        "run_id": what.run_id,
        "forced": forced,
        "reused_existing_reclaim": false,
    });
    mark_deferred(&mut answer, deferred);
    Ok(answer)
}

enum Reclaimed {
    Discovered {
        target_operation: String,
        holder: String,
    },
    Resumed {
        holder: String,
        forced: bool,
    },
    Written {
        holder: String,
    },
}

/// What a **retried** unassign means: the comment is already on the issue.
///
/// Every branch here was missing. The port planned the release again instead,
/// which answers a different question — one whose answer changes the moment the
/// write it is retrying takes effect.
///
/// The target is checked against the timeline **before** this operation's own
/// control marker, because that is the only place an epoch this run may end can
/// be. A marker naming anything else is not a release anybody can honour, and
/// `invalid-unassign-target` says so rather than proceeding on a name.
fn resumed_release(
    context: &Context,
    comments: &[ownership::Comment],
    ownership: &ownership::Holding,
    existing: &super::markers::Marker,
    operation_id: &str,
    what: &Departure<'_>,
) -> Result<serde_json::Value, Failure> {
    let invalid = || {
        Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "invalid-unassign-target",
            "action": "re-read ownership and use a fresh operation ID",
        }))
    };
    let Some(target_epoch) = existing.get("target-op") else {
        return Err(invalid());
    };
    let Some((control, ..)) = ownership::first_operation_markers(comments).remove(operation_id)
    else {
        return Err(invalid());
    };
    let epoch = |event: &ownership::Event| {
        ownership::ownership_epoch(
            event.operation_id.as_deref(),
            event.comment.id.as_deref(),
            &event.comment.created_at,
            &event.comment.body,
        )
    };
    let names_one = ownership::authoritative_events(comments)
        .iter()
        .any(|event| {
            event.position < control && &epoch(event) == target_epoch && event.run_id == what.run_id
        });
    if !names_one {
        return Err(invalid());
    }

    // Visible and still authoritative is a **write** failure: something was
    // written and the timeline has not settled on it, which is not the same as
    // the release not having happened.
    if ownership
        .live
        .iter()
        .chain(&ownership.stale)
        .any(|event| &epoch(event) == target_epoch)
    {
        return Err(Failure::Write(
            "unassign is visible but its target epoch remains authoritative".to_owned(),
        ));
    }
    if what.held_by_other
        && ownership
            .holder
            .as_deref()
            .is_none_or(|holder| holder == what.run_id)
    {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "held-by-other-holder-disappeared",
        })));
    }
    Ok(release_answer(
        context,
        what,
        ownership.holder.is_some(),
        comments,
    ))
}

/// Whether a **retried** reclaim still stands, and whether it was forced.
///
/// Pure and fed: the marker already on the issue, the evidence this call gave,
/// and the live event go in, so the decision is testable without a tracker.
///
/// The evidence comparison is the part that had no equivalent at all. An
/// operation id identifies a takeover *and the justification it gave*, so a
/// retry carrying a different reason file is not the same operation — and
/// answering it with success would leave the issue recording a justification
/// nobody meant to give while telling the caller theirs was accepted.
fn resumed_reclaim(
    existing: &super::markers::Marker,
    supplied: Option<&str>,
    current: Option<ownership::Event>,
    operation_id: &str,
) -> Result<bool, Failure> {
    if let Some(hash) = supplied
        && existing.get("evidence-hash").map(String::as_str) != Some(hash)
    {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "reclaim-metadata-mismatch" }),
        ));
    }
    let winner = current.as_ref().map(|event| event.run_id.clone());
    let Some(current) = current.filter(|event| event.operation_id.as_deref() == Some(operation_id))
    else {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "reclaim-operation-no-longer-current",
            // Named, because "you lost" and "you lost to *them*" are different
            // amounts of help.
            "winner": winner,
            "world": "committed",
        })));
    };
    Ok(current.forced)
}

/// The refusal a fold-discarded takeover answers with.
///
/// `winner` is present only when the timeline still shows a holder: the
/// no-holder branch has none to name, and the displaced run's id there would
/// name a run the timeline does not show holding anything.
fn reclaim_discarded(discard: ownership::ReclaimDiscard, winner: Option<&str>) -> Failure {
    let mut envelope = serde_json::json!({
        "ok": false,
        "reason": "reclaim-discarded",
        "condition": discard.as_str(),
        "world": "committed",
        "action": "the reclaim comment landed and the ownership fold discarded it; \
                   re-read the issue's timeline, then correct what `condition` names \
                   before writing another reclaim",
    });
    if let Some(winner) = winner {
        envelope["winner"] = serde_json::json!(winner);
    }
    Failure::Stop(envelope)
}

/// Whether a **freshly written** reclaim won, and whether it was forced.
///
/// On run-id, as the transport has it: what this call promised is that this
/// *run* holds the issue afterwards, and it is the run the caller acts as.
///
/// A holder that is not this run is two different answers. Either the fold
/// discarded this run's reclaim — [`ownership::discarded_reclaim`] names the
/// condition — or the reclaim counted and somebody else still holds the issue,
/// which is the only thing `lost-reclaim-race` says.
///
/// No live holder is those same two answers. A fold-discarded reclaim that
/// displaced nobody leaves no holder exactly when its target was stale and the
/// re-read no longer carries it, so the condition is consulted on this branch
/// too; only a timeline that shows no reclaim of this operation at all is the
/// write whose fate is unknown.
fn won_the_reclaim(
    current: Option<ownership::Event>,
    run_id: &str,
    comments: &[ownership::Comment],
    operation_id: &str,
) -> Result<bool, Failure> {
    let Some(current) = current else {
        if let Some(discard) = ownership::discarded_reclaim(comments, operation_id) {
            return Err(reclaim_discarded(discard, None));
        }
        return Err(Failure::Write(
            "the reclaim comment landed but no live holder was established".to_owned(),
        ));
    };
    if current.run_id != run_id {
        if let Some(discard) = ownership::discarded_reclaim(comments, operation_id) {
            return Err(reclaim_discarded(discard, Some(&current.run_id)));
        }
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "lost-reclaim-race",
            "winner": current.run_id,
            "world": "committed",
        })));
    }
    Ok(current.forced)
}

#[cfg(test)]
mod tests;
