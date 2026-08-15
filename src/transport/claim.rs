//! The ownership commands: one read, four checks.
//!
//! **A failed check is a stop; a failed read is nothing.** That distinction is
//! the whole point of this file. Incident I07 records a run that lost a claim
//! race by five seconds, was told so thirty-three seconds later, and then worked
//! another forty-eight minutes — because nothing in its heartbeat loop ever read
//! the timeline again.

use super::{Context, Failure, ownership};

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
/// issue stop itself on its own history.
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
            let marks = super::markers::parse(&comment.body);
            let instructed = marks.iter().any(|mark| {
                let kind = mark.get("kind").map(String::as_str).unwrap_or_default();
                !super::markers::RELEASE_KINDS.contains(&kind)
                    && ownership::is_control_for(mark, run_id)
            });
            // The prose fallback, for comments written before markers existed,
            // and deliberately narrow: a control message must both **name** the
            // run and **instruct** it. A comment that merely mentions a run-id
            // is not an order, and treating one as a stand-down would have a run
            // abandon work nobody asked it to drop.
            instructed
                || (marks.is_empty()
                    && comment.body.contains(run_id)
                    && mentions_standdown(&comment.body))
        })
        .map(|(_, comment)| comment.clone())
}

/// Whether a body instructs somebody to stop, in prose.
///
/// The transport spells this as one bounded alternation and this was a list of
/// `contains`, which disagreed in *both* directions at once.
///
/// It missed `stand<newline>down`, `stand  down` and `standingdown`, all of
/// which `stand(?:ing)?\s*down` covers — a run left deaf to a stand-down the
/// other side had read, which is the expensive direction: this is only consulted
/// when a comment carries no marker and names this run, so it is the last thing
/// standing between a displaced run and its next write.
///
/// And with no word boundaries it fired on `readjudicating`, `back offset` and
/// `you lostness`. That is precisely the phantom the transport documents beside
/// its own claim regex: a run posting a stand-down and stripping its label "on
/// the strength of text written by someone else entirely".
///
/// Public because the differential oracle compared it against `STANDDOWN_PROSE`,
/// the way it compared [`super::ownership::claim_prose`] against `CLAIM_PROSE`.
/// That oracle is deleted and nothing else outside this module calls it, so the
/// visibility now outlives its reason — narrowing it is safe and unhurried.
pub fn mentions_standdown(body: &str) -> bool {
    /// The alternation the transport holds, and whether a word may run on past
    /// the phrase. `adjudicat\w*` is the one that may: its `\w*` is greedy, so
    /// the `\b` behind it is satisfied by construction and only the boundary in
    /// front of it decides anything.
    const PHRASES: &[(&str, bool)] = &[
        ("backing off", false),
        ("back off", false),
        ("reclaiming from", false),
        ("adjudicat", true),
        ("you lost", false),
        ("release the item", false),
    ];
    let lowered = body.to_lowercase();
    stands_down(&lowered)
        || PHRASES
            .iter()
            .any(|(phrase, open_ended)| holds_phrase(&lowered, phrase, *open_ended))
}

/// Whether lowercased `text` holds `stand(?:ing)?\s*down` on word boundaries.
fn stands_down(text: &str) -> bool {
    let mut searched = 0;
    while let Some(at) = text[searched..].find("stand") {
        let start = searched + at;
        searched = start + 1;
        if !opens_a_word(text, start) {
            continue;
        }
        let rest = &text[start + "stand".len()..];
        let rest = rest.strip_prefix("ing").unwrap_or(rest);
        if let Some(after) = rest.trim_start().strip_prefix("down")
            && closes_a_word(after)
        {
            return true;
        }
    }
    false
}

/// Whether lowercased `text` holds `phrase` on word boundaries.
///
/// `open_ended` drops the boundary behind the phrase, for the one alternative
/// that ends in `\w*`.
fn holds_phrase(text: &str, phrase: &str, open_ended: bool) -> bool {
    let mut searched = 0;
    while let Some(at) = text[searched..].find(phrase) {
        let start = searched + at;
        searched = start + 1;
        if opens_a_word(text, start) && (open_ended || closes_a_word(&text[start + phrase.len()..]))
        {
            return true;
        }
    }
    false
}

/// A word character, as `\b` counts them: `\w` is alphanumeric or `_`.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether a match starting at `at` opens on a word boundary.
fn opens_a_word(text: &str, at: usize) -> bool {
    text[..at].chars().next_back().is_none_or(|c| !is_word(c))
}

/// Whether what follows a match closes it on a word boundary.
fn closes_a_word(rest: &str) -> bool {
    rest.chars().next().is_none_or(|c| !is_word(c))
}

/// `verify-claim` — the renewal: prove current live ownership and state.
///
/// The four checks, in order, because each one makes the next meaningful: the
/// issue is still open, it is in exactly the state expected, this run is the
/// current live holder, and nothing since the claim has told it to stop.
///
/// `allow_closed_by_pr` exists for exactly one moment: the renewal immediately
/// before `close`, after this run's own merge. Under `gh issue develop` the
/// merge auto-closes the issue, so a closed issue *there* is the expected
/// outcome of the delivery rather than evidence somebody else took it. Without
/// it the renewal would hard-stop and the run would skip the `transition --to
/// done` that the auto-close makes mandatory — the rule blocking itself. It is
/// deliberately narrow: the issue must have been closed **by the PR that was
/// merged**, and any other closer is still a stop.
pub fn verify_claim(
    context: &Context,
    issue: u64,
    run_id: &str,
    expect_state: &str,
    now: &str,
    allow_closed_by_pr: Option<u64>,
) -> Result<serde_json::Value, Failure> {
    let number = issue.to_string();
    let data = super::gh_json(
        &["issue", "view", &number, "--json", "state,labels,comments"],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh issue view {issue} returned nothing")))?;

    let mut closed_by_own_pr = false;
    let state = text(&data, "state");
    if state != "OPEN" {
        if let Some(pr) = allow_closed_by_pr {
            closed_by_own_pr =
                super::closing::closing_refs(context, issue)?
                    .iter()
                    .any(|reference| {
                        reference.get("number").and_then(serde_json::Value::as_u64) == Some(pr)
                    });
        }
        if !closed_by_own_pr {
            return Err(stop(
                "issue-not-open",
                format!("issue #{issue} is {state} — someone delivered or killed it"),
                "stop; change nothing about the state, it is not yours",
            ));
        }
    }

    // Through the same reader `transition` writes with. This inlined the body
    // of `commands::status_labels`, character for character — so the half of
    // the pair that reads the state and the half that sets it each held their
    // own copy of which label carries it.
    let present = super::commands::status_labels(&data);
    let wanted = format!("status:{expect_state}");
    if present != [wanted.clone()] {
        return Err(stop(
            "unexpected-state",
            format!("expected exactly [{wanted}], found {present:?}"),
            "stop; the item moved without you — leave the new state alone",
        ));
    }

    let comments = comments_of(&data);
    let ownership = holding(&comments, now);
    if ownership.holder.as_deref() != Some(run_id) {
        // Two situations shared one sentence, and the advice belonged to the
        // other one. *Somebody else holds it* is a stop: the work is theirs and
        // writing is not this run's to do. *Nobody holds it* is the ordinary end
        // of a claim's life — the horizon passed and the issue is free — and
        // being told to "release only your own projection and write nothing
        // else" names no way forward when there is one.
        //
        // Measured on the installed binary: a run whose horizon had gone by two
        // hours was refused with `current live holder is none, not
        // claude-abcd1234` and that advice, which is where an agent that worked
        // a little too long lands every time.
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
    let opened = if closed_by_own_pr {
        format!(
            "closed-by-own-pr-{}",
            allow_closed_by_pr.unwrap_or_default()
        )
    } else {
        "issue-open".to_owned()
    };
    Ok(serde_json::json!({
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
    }))
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
/// forty-eight minutes. So the renewal runs first, and **refuses to post** when
/// it says stop — the write never happens rather than happening and being
/// regretted.
pub fn heartbeat(
    context: &Context,
    issue: u64,
    run_id: &str,
    expect_state: &str,
    body_file: &std::path::Path,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let verdict = verify_claim(context, issue, run_id, expect_state, now, None)?;

    let raw = std::fs::read_to_string(body_file).map_err(|_| {
        Failure::Stop(serde_json::json!({ "ok": false, "reason": "heartbeat-body-invalid" }))
    })?;
    // Escaped before the marker is appended, never after: escaping keeps quoted
    // evidence inert, and running it over the marker this call just wrote would
    // neuter its own.
    let marker = super::markers::render("heartbeat", &[("run-id", run_id)]).ok_or_else(|| {
        Failure::Stop(serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }))
    })?;
    let body = format!(
        "{}

{marker}
",
        super::markers::escape_control_input(&raw).trim_end()
    );

    let staged = crate::paths::scratch_file(&format!("heartbeat-{issue}.md"));
    crate::paths::replace_atomically(&staged, &body).map_err(|error| {
        Failure::Write(format!("the heartbeat body could not be staged: {error}"))
    })?;
    let path = staged.display().to_string();
    let answer = super::run(
        &[
            "gh",
            "issue",
            "comment",
            &issue.to_string(),
            "--body-file",
            &path,
        ],
        Some(&context.repo_dir),
        super::How::write(),
    );
    let _ = std::fs::remove_file(&staged);
    answer?;

    Ok(serde_json::json!({
        "ok": true,
        "issue": issue,
        "renewed": verdict.get("checked"),
        "posted": true,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewReceipt {
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

impl ReviewReceipt {
    fn from_marker(marker: &super::markers::Marker) -> Option<Self> {
        let receipt = Self {
            epoch: marker.get("epoch")?.clone(),
            pr: marker.get("pr")?.parse().ok()?,
            head: marker.get("head")?.clone(),
            base: marker.get("base")?.clone(),
            digest: marker.get("digest")?.clone(),
        };
        receipt.is_complete().then_some(receipt)
    }

    fn is_complete(&self) -> bool {
        let exact_hex = |value: &str, width: usize| {
            value.len() == width
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        ownership::is_operation_id(&self.epoch)
            && self.pr > 0
            && exact_hex(&self.head, 40)
            && exact_hex(&self.base, 40)
            && exact_hex(&self.digest, 64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LatestPublication {
    publisher: String,
    receipt: ReviewReceipt,
}

fn latest_publication(comments: &[ownership::Comment]) -> Option<LatestPublication> {
    comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
        .flat_map(|comment| super::markers::parse(&comment.body))
        .filter(|marker| marker.get("kind").map(String::as_str) == Some("published"))
        .filter_map(|marker| {
            let publication = LatestPublication {
                publisher: marker.get("run-id")?.clone(),
                receipt: ReviewReceipt::from_marker(&marker)?,
            };
            (!publication.publisher.is_empty()).then_some(publication)
        })
        .next_back()
}

/// One durable request for another run to review an exact publication receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewHandoff {
    /// The operation that wrote this marker.
    pub operation_id: String,
    /// The run putting the review down.
    pub requester: String,
    /// The exact ownership epoch released after this marker became visible.
    pub target_operation: String,
    /// The immutable publication identity.
    pub receipt: ReviewReceipt,
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
        let receipt = ReviewReceipt::from_marker(marker)?;
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

/// One immutable verdict over a complete publication receipt.
///
/// The attester and the reviewer are separate because the two review routes
/// differ in exactly that: after a handoff the reviewing run claims the issue
/// and records its own verdict, so the two identities coincide and the timeline
/// attributes it. A run that acquired its reviewer directly never releases the
/// claim, so it records its reviewer's outcome itself and the two differ. What
/// Estigia can check is the same in both cases — that the *reviewer* is not the
/// run that published — and no more; see `docs/honesty.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewVerdict {
    /// The operation that wrote this marker.
    pub operation_id: String,
    /// The run that recorded it, holding the live `review` claim.
    pub attester: String,
    /// The context credited with the review. Never the publisher.
    pub reviewer: String,
    /// The immutable publication identity.
    pub receipt: ReviewReceipt,
    /// `accepted` or `rejected`; both resolve a handoff, but only acceptance qualifies delivery.
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
            receipt: ReviewReceipt::from_marker(marker)?,
            outcome,
        })
    }
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

fn handoffs_for(comments: &[ownership::Comment], receipt: &ReviewReceipt) -> Vec<ReviewHandoff> {
    first_protocol_markers(comments, "review-handoff")
        .iter()
        .filter_map(ReviewHandoff::from_marker)
        .filter(|handoff| handoff.receipt == *receipt)
        .collect()
}

fn latest_distinct_review_verdict(
    comments: &[ownership::Comment],
    receipt: &ReviewReceipt,
) -> Option<ReviewVerdict> {
    let publication = latest_publication(comments)?;
    if publication.receipt != *receipt {
        return None;
    }
    // Deliberately not conditioned on a handoff existing. Making the verdict
    // requirement appear only once a handoff is recorded would mean a deleted
    // handoff comment *lowers* the bar from "a distinct reviewer accepted these
    // bytes" to "nothing", and an erased record must never read as clearance.
    // Delivery therefore asks for the same evidence on both review routes.
    let mut forbidden: std::collections::BTreeSet<String> =
        [publication.publisher].into_iter().collect();
    forbidden.extend(
        handoffs_for(comments, receipt)
            .into_iter()
            .map(|handoff| handoff.requester),
    );
    first_protocol_markers(comments, "review-verdict")
        .iter()
        .filter_map(ReviewVerdict::from_marker)
        .rfind(|verdict| {
            verdict.receipt == *receipt && !forbidden.contains(verdict.reviewer.as_str())
        })
}

/// A distinct accepted verdict for this exact latest receipt, if one exists.
pub fn qualifying_review_verdict(
    comments: &[ownership::Comment],
    receipt: &ReviewReceipt,
) -> Option<ReviewVerdict> {
    latest_distinct_review_verdict(comments, receipt)
        .filter(|verdict| verdict.outcome == "accepted")
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
pub fn review_eligibility(comments: &[ownership::Comment], run_id: &str) -> ReviewEligibility {
    let Some(publication) = latest_publication(comments) else {
        return ReviewEligibility::Eligible;
    };
    let handoffs = handoffs_for(comments, &publication.receipt);
    let Some(handoff) = handoffs.last().cloned() else {
        return ReviewEligibility::Eligible;
    };
    if latest_distinct_review_verdict(comments, &publication.receipt).is_some() {
        return ReviewEligibility::Eligible;
    }
    let mut requesters: Vec<String> = handoffs
        .iter()
        .map(|handoff| handoff.requester.clone())
        .collect();
    requesters.sort();
    requesters.dedup();
    if publication.publisher != run_id && !requesters.iter().any(|requester| requester == run_id) {
        return ReviewEligibility::Eligible;
    }
    ReviewEligibility::Excluded {
        publisher: publication.publisher,
        requesters,
        handoff: Box::new(handoff),
    }
}

/// Composes one review protocol comment whose prose came from an agent.
///
/// `comment_with_body` posts what it is handed, and its contract is a body this
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
    } = review_eligibility(comments, run_id)
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

/// Requires the caller's receipt to be the latest complete publication globally.
pub fn recorded_receipt(
    comments: &[ownership::Comment],
    run_id: &str,
    supplied: &ReviewReceipt,
) -> Result<ReviewReceipt, Failure> {
    let latest = latest_publication(comments).ok_or_else(|| {
        stop(
            "published-receipt-missing",
            format!("issue has no complete published receipt for {run_id}"),
            "publish the review target again; incomplete or absent evidence cannot release CI",
        )
    })?;
    if latest.receipt != *supplied || latest.publisher != run_id {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "published-receipt-mismatch",
            "recorded": {
                "run_id": latest.publisher,
                "epoch": latest.receipt.epoch,
                "pr": latest.receipt.pr,
                "head": latest.receipt.head,
                "base": latest.receipt.base,
                "digest": latest.receipt.digest,
            },
            "action": "use the latest complete publication receipt; a republish invalidates every earlier epoch",
        })));
    }
    Ok(latest.receipt)
}

fn require_latest_receipt(
    comments: &[ownership::Comment],
    supplied: &ReviewReceipt,
) -> Result<LatestPublication, Failure> {
    let latest = latest_publication(comments).ok_or_else(|| {
        stop(
            "published-receipt-missing",
            "issue has no complete published receipt".to_owned(),
            "publish the review target again; incomplete or absent evidence cannot be handed off",
        )
    })?;
    if latest.receipt != *supplied {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "published-receipt-mismatch",
            "recorded": {
                "run_id": latest.publisher,
                "epoch": latest.receipt.epoch,
                "pr": latest.receipt.pr,
                "head": latest.receipt.head,
                "base": latest.receipt.base,
                "digest": latest.receipt.digest,
            },
            "action": "use the latest complete publication receipt; a republish invalidates every earlier epoch",
        })));
    }
    Ok(latest)
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

fn review_authority(context: &Context, now: &str) -> Result<(String, String), Failure> {
    let configured = context.get("review delegation").unwrap_or("ask").trim();
    // The configuration's own parser. This was a second spelling of that
    // grammar, stricter than the reader an operator's table goes through, so
    // four values `estigia config` accepts — `Auto`, `AUTO`, `Ask 30m`,
    // `ask 30 m` — refused the one operation that ends a blocked run's wait.
    let parsed = crate::config::authority_of(configured).ok_or_else(|| {
        Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-review-authority", "value": configured,
        }))
    })?;
    let seconds = match parsed {
        crate::config::Authority::Auto => 0,
        crate::config::Authority::Ask { timeout } => timeout.as_secs(),
    };
    // Written back in this crate's spelling rather than the operator's, so what
    // the marker carries is what its own reader will accept. `ask  30m` passed
    // here verbatim and was then refused by that reader, which left the handoff
    // comment posted, its readback failing, and every retry answering
    // `review-handoff-operation-conflict` with the claim never released.
    let authority = crate::config::rendered_authority(parsed);
    let requested = ownership::parse_stamp(now).ok_or_else(|| {
        Failure::Read("the machine clock did not produce a review request timestamp".to_owned())
    })?;
    let seconds = i64::try_from(seconds).map_err(|_| {
        Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-review-authority", "value": configured,
        }))
    })?;
    let deadline = requested.checked_add(seconds).ok_or_else(|| {
        Failure::Read("the review request deadline overflowed the machine clock".to_owned())
    })?;
    let deadline = u64::try_from(deadline).map_err(|_| {
        Failure::Read("the review request deadline predates the system clock".to_owned())
    })?;
    Ok((authority, crate::harness::session::stamp_of(deadline)))
}

fn review_receipt<'a>(
    epoch: &'a str,
    pr: u64,
    head: &'a str,
    base: &'a str,
    digest: &'a str,
) -> Result<ReviewReceipt, Failure> {
    let receipt = ReviewReceipt {
        epoch: epoch.to_owned(),
        pr,
        head: head.to_owned(),
        base: base.to_owned(),
        digest: digest.to_owned(),
    };
    if !receipt.is_complete() {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "incomplete-review-receipt",
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
    let read = || -> Result<serde_json::Value, Failure> {
        super::gh_json(
            &[
                "issue",
                "view",
                &handoff.issue.to_string(),
                "--json",
                "assignees,labels,comments",
            ],
            Some(&context.repo_dir),
        )?
        .ok_or_else(|| Failure::Read(format!("gh issue view {} returned nothing", handoff.issue)))
    };
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
    let data = read()?;
    let comments = comments_of(&data);
    reject_operation_kind_conflict(&comments, &operation_id, &["review-handoff"])?;
    let existing = operation_marker(&comments, &operation_id, "review-handoff", &expected)?;

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
            let current = read()?;
            let comments = comments_of(&current);
            require_latest_receipt(&comments, &receipt)?;
            let ownership = holding(&comments, handoff.now);
            match plan_release(
                &ownership,
                handoff.run_id,
                handoff.runtime,
                false,
                Some(handoff.target_operation),
                &labels_of(&current),
            )? {
                Release::Write { .. } => {}
                Release::Confirm { .. } => unreachable!("the target was supplied"),
            }
            let (authority, deadline) = review_authority(context, handoff.now)?;
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
            super::commands::comment_with_body(context, handoff.issue, &body)?;

            let mut seen = Vec::new();
            for _ in 0..VISIBILITY_ATTEMPTS {
                let observed = read().ok();
                let visible = observed.as_ref().is_some_and(|data| {
                    operation_marker(
                        &comments_of(data),
                        &operation_id,
                        "review-handoff",
                        &expected,
                    )
                    .is_ok_and(|found| found.is_some())
                });
                seen.push(observed);
                if visible {
                    break;
                }
            }
            let Some(marker) = seen.iter().flatten().last().and_then(|data| {
                operation_marker(
                    &comments_of(data),
                    &operation_id,
                    "review-handoff",
                    &expected,
                )
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
    require_latest_receipt(&comments_of(&before_release), &receipt)?;
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
            now: handoff.now,
        },
    )?;

    let after = read().map_err(|failure| {
        Failure::Write(format!(
            "review handoff and release were written, but final state could not be read: {}",
            failure.detail()
        ))
    })?;
    if super::commands::status_labels(&after) != ["status:review"] {
        return Err(Failure::Write(
            "review handoff released ownership but the issue did not remain exactly in review"
                .to_owned(),
        ));
    }
    let after_holding = holding(&comments_of(&after), handoff.now);
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

/// Everything one independent review verdict records.
#[derive(Debug, Clone)]
pub struct VerdictReview<'a> {
    /// The issue whose receipt was reviewed.
    pub issue: u64,
    /// The run recording it, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// The context credited with the review, never the publishing run.
    ///
    /// After a handoff this is the recording run itself. A run that acquired a
    /// reviewer without releasing the claim names that reviewer here instead,
    /// which is what lets the direct route reach delivery at all.
    pub reviewer: &'a str,
    /// Stable across every retry of this verdict.
    pub operation_id: &'a str,
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
    /// `accepted` or `rejected`.
    pub outcome: &'a str,
    /// The moment the live review claim is judged.
    ///
    /// Read from the machine, never from the run being judged. A test may pass
    /// a chosen value; production obtains it from the MCP server's clock.
    pub now: &'a str,
}

/// The one rule a verdict must satisfy, whichever route produced it.
///
/// Written once because it had been written twice, and the two copies had
/// already drifted: the replay path returned a refusal naming neither the
/// publisher nor the requesters that the first-write path names, for the
/// identical condition. A caller hitting a retry got the poorer answer.
fn require_distinct_reviewer(
    comments: &[ownership::Comment],
    receipt: &ReviewReceipt,
    reviewer: &str,
) -> Result<(), Failure> {
    let publication = require_latest_receipt(comments, receipt)?;
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

/// Records an immutable exact-receipt verdict crediting a distinct reviewer.
pub fn record_review_verdict(
    context: &Context,
    verdict: &VerdictReview<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(verdict.operation_id))?;
    if !matches!(verdict.outcome, "accepted" | "rejected") {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-review-verdict", "value": verdict.outcome,
        })));
    }
    let receipt = review_receipt(
        verdict.epoch,
        verdict.pr,
        verdict.head,
        verdict.base,
        verdict.digest,
    )?;
    let read = || -> Result<serde_json::Value, Failure> {
        super::gh_json(
            &[
                "issue",
                "view",
                &verdict.issue.to_string(),
                "--json",
                "labels,comments",
            ],
            Some(&context.repo_dir),
        )?
        .ok_or_else(|| Failure::Read(format!("gh issue view {} returned nothing", verdict.issue)))
    };
    let expected = [
        ("run-id", verdict.run_id),
        ("reviewer", verdict.reviewer),
        ("epoch", verdict.epoch),
        ("pr", &verdict.pr.to_string()),
        ("head", verdict.head),
        ("base", verdict.base),
        ("digest", verdict.digest),
        ("outcome", verdict.outcome),
    ];
    let data = read()?;
    let comments = comments_of(&data);
    reject_operation_kind_conflict(&comments, &operation_id, &["review-verdict"])?;
    if let Some(marker) = operation_marker(&comments, &operation_id, "review-verdict", &expected)? {
        let persisted = ReviewVerdict::from_marker(&marker).ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "review-verdict-operation-conflict",
            }))
        })?;
        require_distinct_reviewer(&comments, &receipt, verdict.reviewer)?;
        let qualifies = qualifying_review_verdict(&comments, &receipt)
            .is_some_and(|qualifying| qualifying.operation_id == persisted.operation_id);
        let resolves = latest_distinct_review_verdict(&comments, &receipt)
            .is_some_and(|recorded| recorded.operation_id == persisted.operation_id);
        return Ok(serde_json::json!({
            "ok": true,
            "issue": verdict.issue,
            "reviewer": persisted.reviewer,
            "attester": persisted.attester,
            "self_attested": persisted.reviewer != persisted.attester,
            "outcome": persisted.outcome,
            "qualifies": qualifies,
            "resolves_handoff": resolves,
            "reused_existing_verdict": true,
        }));
    }

    verify_claim(
        context,
        verdict.issue,
        verdict.run_id,
        "review",
        verdict.now,
        None,
    )?;
    let current = read()?;
    let comments = comments_of(&current);
    require_distinct_reviewer(&comments, &receipt, verdict.reviewer)?;

    let marker = super::markers::render(
        "review-verdict",
        &[
            ("run-id", verdict.run_id),
            ("reviewer", verdict.reviewer),
            ("op-id", &operation_id),
            ("epoch", verdict.epoch),
            ("pr", &verdict.pr.to_string()),
            ("head", verdict.head),
            ("base", verdict.base),
            ("digest", verdict.digest),
            ("outcome", verdict.outcome),
        ],
    )
    .ok_or_else(|| {
        Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
    })?;
    let body = protocol_body(
        &format!(
            "Review verdict `{}` by `{}` for publication epoch `{}`, recorded by `{}`.",
            verdict.outcome, verdict.reviewer, verdict.epoch, verdict.run_id
        ),
        &marker,
    );
    super::commands::comment_with_body(context, verdict.issue, &body)?;

    let mut seen = Vec::new();
    for _ in 0..VISIBILITY_ATTEMPTS {
        let observed = read().ok();
        let visible = observed.as_ref().is_some_and(|data| {
            operation_marker(
                &comments_of(data),
                &operation_id,
                "review-verdict",
                &expected,
            )
            .is_ok_and(|found| found.is_some())
        });
        seen.push(observed);
        if visible {
            break;
        }
    }
    let Some(final_data) = seen.iter().flatten().last() else {
        return Err(Failure::Write(
            "review verdict is not visible; retry the same verdict unchanged".to_owned(),
        ));
    };
    if operation_marker(
        &comments_of(final_data),
        &operation_id,
        "review-verdict",
        &expected,
    )
    .is_ok_and(|found| found.is_none())
    {
        return Err(Failure::Write(
            "review verdict is not visible; retry the same verdict unchanged".to_owned(),
        ));
    }
    let qualifies = qualifying_review_verdict(&comments_of(final_data), &receipt)
        .is_some_and(|qualifying| qualifying.operation_id == operation_id);
    let resolves = latest_distinct_review_verdict(&comments_of(final_data), &receipt)
        .is_some_and(|recorded| recorded.operation_id == operation_id);
    Ok(serde_json::json!({
        "ok": true,
        "issue": verdict.issue,
        "reviewer": verdict.reviewer,
        "attester": verdict.run_id,
        "self_attested": verdict.reviewer != verdict.run_id,
        "outcome": verdict.outcome,
        "qualifies": qualifies,
        "resolves_handoff": resolves,
        "epoch": verdict.epoch,
        "pr": verdict.pr,
        "head": verdict.head,
        "base": verdict.base,
        "digest": verdict.digest,
        "reused_existing_verdict": false,
    }))
}

fn ready_outcome(
    write: Result<(), Failure>,
    read: Result<serde_json::Value, Failure>,
    receipt: &ReviewReceipt,
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
enum Push<'a> {
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

/// `republish-review` — publish a rewritten branch over its own last publication.
///
/// The operation this crate did not have, and the gap was not cosmetic: after a
/// rebase or an amend the ordinary push is refused as a non-fast-forward, so the
/// sequence a run actually performed was *leave Estigia, `git push
/// --force-with-lease` by hand, come back*. The single most destructive git
/// operation was the one step of the delivery path with no claim verification,
/// no timeline re-read and no record — and a run that had lost its claim in the
/// meantime could still force-push, because nothing asked.
///
/// What it adds over [`publish_review`] is the lease and one more renewal. What
/// it deliberately does not add is a plain `--force`: without a lease this is a
/// write that cannot be refused, and an operation that cannot be refused is not
/// a gate.
pub fn republish_review(
    context: &Context,
    what: &Publication<'_>,
) -> Result<serde_json::Value, Failure> {
    let issue = super::gh_json(
        &[
            "issue",
            "view",
            &what.issue.to_string(),
            "--json",
            "comments",
        ],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh issue view {} returned nothing", what.issue)))?;
    // No recorded publication is not *nothing to protect*, it is a caller in the
    // wrong operation. A first publication is `publish-review`, whose refusal on
    // a non-fast-forward is meaningful; forcing over a branch this issue never
    // published means overwriting somebody else's work with no expectation to
    // check it against, which is the plain `--force` this issue rules out
    // wearing a different name.
    let Some(latest) = latest_publication(&comments_of(&issue)) else {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "published-receipt-missing",
            "issue": what.issue,
            "action": "there is no recorded publication on this issue to lease against \u{2014} \
                       publish-review makes the first one, and its refusal on a non-fast-forward \
                       is the check that would be skipped here",
        })));
    };
    // Whose publication it was is not asked, and that is deliberate rather than
    // overlooked. Authority here is the live claim, which `publish_with` verifies
    // twice; a run that reclaimed an abandoned issue is entitled to republish the
    // head the previous holder left, and requiring self-attribution would strand
    // exactly the reclaim the workflow provides for.
    publish_with(
        context,
        what,
        Push::Leased {
            recorded_head: &latest.receipt.head,
        },
    )
}

fn publish_with(
    context: &Context,
    what: &Publication<'_>,
    push: Push<'_>,
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
        now,
    } = *what;
    verify_claim(context, issue, run_id, expect_state, now, None)?;
    let at = worktree.unwrap_or(&context.repo_dir);
    super::run(
        &["git", "fetch", "origin", "--", base],
        Some(at),
        super::How::read(),
    )?;
    let target = super::target::clean_target(context, &format!("origin/{base}"), Some(at))?;

    // The closing-keyword refusal, where it can be true.
    //
    // It used to fire two hundred lines below, after the push and after the pull
    // request was opened, and it is a `Stop` — whose outcome line reads *nothing
    // was written*. It had written a branch and a PR. A run believing that
    // message leaves both orphaned, and the next call fails for an unrelated
    // reason with the operator debugging from a false premise this tool supplied.
    //
    // Every source of a keyword this run introduces is readable before the
    // remote is touched: the commit messages this branch adds, and the body
    // about to be written. So it is read here, above `open_prs` — **above every
    // remote mutation**, not merely above the push. Not above every remote
    // *call*: `verify_claim` and the base fetch precede it, and both are reads.
    // The distinction is the whole claim, so it is stated as the narrower one
    // that is true rather than the wider one that reads better.
    //
    // It sat below `open_prs` first, which was the same defect wearing a
    // shorter distance. On the reused-PR path `ensure_draft` runs `gh pr ready
    // --undo` and `edit_pr` replaces the live title and body, and `pr_body_text`
    // leaves a body naming `#<n>` exactly as written — so a body carrying
    // `Closes #<n>` was published to the pull request, and the refusal that
    // followed still said nothing had been written. That is the field report's
    // own shape: its PR already existed on the retry.
    //
    // The check below stays — it also settles a branch-derived link, and a
    // keyword can arrive from the remote side of a PR this run did not write —
    // but it can no longer be the first thing to notice one.
    let mut wrote_keyword = super::closing::keywords_in_commits(at, base, branch, issue)?;
    if let Some(file) = pr_body_file {
        // Not `if let Ok(..)`. A body this run cannot read is not a body with no
        // keyword in it — the same sentence `keywords_in_commits` is written
        // against, three lines away. Swallowing it here would have refused later
        // instead, after `ensure_draft` may already have run `gh pr ready
        // --undo`, and reported that nothing was written.
        let text = std::fs::read_to_string(file)
            .map_err(|error| Failure::Read(format!("the PR body could not be read: {error}")))?;
        wrote_keyword.extend(super::closing::keywords_naming(&text, issue));
    }
    if !wrote_keyword.is_empty() {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "closing-keyword-live",
            "cause": "closing-keyword",
            "keyword_sources": wrote_keyword,
            "action": "the issue WOULD auto-close on merge, bypassing transition and the mirror \
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
    if let Some(pr) = &reused {
        ensure_draft(context, pr)?;
        if let Some(file) = pr_body_file {
            let raw = std::fs::read_to_string(file).map_err(|error| {
                Failure::Read(format!("the PR body could not be read: {error}"))
            })?;
            edit_pr(context, pr, title, Some(&pr_body_text(&raw, issue)))?;
        } else if !title.is_empty() {
            edit_pr(context, pr, title, None)?;
        }
    } else if pr_body_file.is_none() {
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
        // second read.
        verify_claim(context, issue, run_id, expect_state, now, None)?;
    }
    push_to_origin(at, branch, push)?;

    let answer = published(
        context,
        issue,
        run_id,
        branch,
        base,
        title,
        pr_body_file,
        reused,
        target,
    )
    .map_err(|failure| {
        match failure {
            // `Read` says nothing happened, and something did. The others
            // already carry the right outcome and only want the fact.
            Failure::Read(detail) | Failure::Write(detail) => Failure::Write(format!(
                "{detail} \u{2014} the branch `{branch}` is already pushed to origin, \
                 so this is not a call that changed nothing"
            )),
            other => other,
        }
    });
    // Which route ran, in the answer, because the two are not interchangeable to
    // whoever reads it back: a leased push moved bytes the remote already had,
    // and an incident review that cannot tell the two apart from the record has
    // to reconstruct it from the shell history nobody kept.
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
fn push_to_origin(at: &std::path::Path, branch: &str, push: Push<'_>) -> Result<(), Failure> {
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

fn open_prs(context: &Context, branch: &str) -> Result<Vec<serde_json::Value>, Failure> {
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

fn pr_number(pr: &serde_json::Value) -> u64 {
    pr.get("number")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default()
}

fn view_pr(context: &Context, number: u64) -> Result<serde_json::Value, Failure> {
    super::gh_json(
        &[
            "pr",
            "view",
            &number.to_string(),
            "--json",
            "number,url,headRefOid,baseRefOid,state,isDraft",
        ],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh pr view {number} returned nothing")))
}

fn ensure_draft(context: &Context, pr: &serde_json::Value) -> Result<(), Failure> {
    let number = pr_number(pr);
    if pr.get("isDraft").and_then(serde_json::Value::as_bool) != Some(true) {
        super::run(
            &["gh", "pr", "ready", &number.to_string(), "--undo"],
            Some(&context.repo_dir),
            super::How::write(),
        )?;
    }
    let seen = view_pr(context, number)?;
    if seen.get("isDraft").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "draft-readback-failed", "pr": number,
            "observed": seen,
            "action": "do not push; the reused PR is still ready and would expose the new head to CI",
        })));
    }
    Ok(())
}

/// The half of [`publish_review`] that runs with the branch already pushed.
///
/// Its own function so the one sentence about that is written once, rather
/// than at each of the eight places down here that can fail.
#[expect(
    clippy::too_many_arguments,
    reason = "the arguments are the publication receipt inputs and keeping them explicit avoids a second state object beside Publication"
)]
fn published(
    context: &Context,
    issue: u64,
    run_id: &str,
    branch: &str,
    base: &str,
    title: &str,
    pr_body_file: Option<&std::path::Path>,
    reused: Option<serde_json::Value>,
    target: serde_json::Value,
) -> Result<serde_json::Value, Failure> {
    let (pr, created) = match reused {
        Some(pr) => (pr, false),
        None => {
            let Some(file) = pr_body_file else {
                return Err(Failure::Stop(serde_json::json!({
                    "ok": false,
                    "reason": "missing-pr-body",
                    "action": "pass --pr-body-file for a new PR",
                })));
            };
            let raw = std::fs::read_to_string(file).map_err(|error| {
                Failure::Read(format!("the PR body could not be read: {error}"))
            })?;
            let staged = stage(&pr_body_text(&raw, issue), &format!("pr-{issue}"))?;
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

    let mut seen = Vec::new();
    for _ in 0..PUBLISH_READBACK_ATTEMPTS {
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
    let Some(published) = confirm_published(&seen, local_head, expected_base, true).cloned() else {
        let action = [
            "the remote never reported the head and base that were just pushed. Do NOT",
            "bind review or CI to anything yet: re-read the PR, and if it settled on a",
            "different head, someone else pushed and the delivery target has changed",
        ]
        .join(" ");
        return Err(Failure::Stop(serde_json::json!({
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
            "action": action,
        })));
    };

    // The last two steps, and until the transport was retired their absence was
    // a **disagreement**: the port answered `ok` here and the transport, which
    // was what ran, went on. Retiring the transport promoted this from a half
    // nobody ported into a hole in the only implementation there is.
    //
    // First: will merging this close the issue behind the workflow's back? A
    // closing keyword in the pull request body auto-closes on merge, which skips
    // the transition and the board mirror — the workflow's close is a transition,
    // and GitHub's is not it.
    let verdict = super::commands::assess_autoclose(context, issue, Some(base), Some(branch))?;
    if verdict.get("cause").and_then(serde_json::Value::as_str) == Some("closing-keyword") {
        let mut envelope = serde_json::json!({
            "ok": false,
            "reason": "closing-keyword-live",
            // The local precondition above catches a keyword this run wrote.
            // Reaching here means one arrived from the remote side, by which
            // time the branch is pushed and the pull request is open.
            "world": "committed",
            "pr": number,
            "action": "the issue WILL auto-close on merge, bypassing transition and the mirror \
                       \u{2014} remove the keyword (use `Refs #<n>`) and re-run",
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
    let note = format!(
        "Published draft for review: {url}\n\n- epoch `{epoch}`\n- head `{head}`\n- base `{base_oid}`\n- target `{digest}`\n\nReview is bound to this complete clean target. CI remains blocked while the PR is draft; any republish creates a new epoch.\n\n{}\n",
        // A marker that will not render is a comment that carries no evidence,
        // and posting the prose without it would leave a note nobody can read
        // back as a fact. Refused rather than posted half.
        super::markers::render(
            "published",
            &[
                ("run-id", run_id),
                ("pr", &number.to_string()),
                ("head", head),
                ("base", base_oid),
                ("digest", digest),
                ("epoch", &epoch),
            ],
        )
        .ok_or_else(|| {
            Failure::Write("the published marker could not be rendered".to_owned())
        })?
    );
    super::commands::comment_with_body(context, issue, &note)?;

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
        "autoclose": verdict,
        "next": "transition --to review, obtain the configured blind verdicts, then release_ci with this exact receipt",
    });
    // A branch linked to its issue also auto-closes, and that one is not a
    // defect to refuse — it is the ordinary outcome of `gh issue develop`. What
    // it needs is a person told that the auto-close is not the workflow's close.
    if verdict.get("cause").and_then(serde_json::Value::as_str) == Some("branch-link")
        && let Some(answer) = answer.as_object_mut()
    {
        answer.insert(
            "mandatory_follow_up".to_owned(),
            serde_json::json!(
                "GitHub will auto-close this issue on merge because the branch was linked, not \
                 because of a keyword. Run `transition --to done` after the merge regardless \u{2014} \
                 the auto-close is not the workflow's close and moves neither the label nor the board"
            ),
        );
    }
    Ok(answer)
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

/// The exact publication receipt `release-ci` is allowed to spend.
#[derive(Debug, Clone)]
pub struct CiRelease<'a> {
    /// The issue whose live claim authorises the write.
    pub issue: u64,
    /// The run that published the receipt.
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
    verify_claim(
        context,
        release.issue,
        release.run_id,
        "review",
        release.now,
        None,
    )?;
    let issue = super::gh_json(
        &[
            "issue",
            "view",
            &release.issue.to_string(),
            "--json",
            "comments",
        ],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh issue view {} returned nothing", release.issue)))?;
    let supplied = ReviewReceipt {
        epoch: release.epoch.to_owned(),
        pr: release.pr,
        head: release.head.to_owned(),
        base: release.base.to_owned(),
        digest: release.digest.to_owned(),
    };
    recorded_receipt(&comments_of(&issue), release.run_id, &supplied)?;
    let comments = comments_of(&issue);
    let review_verdict = qualifying_review_verdict(&comments, &supplied).ok_or_else(|| {
        Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "qualifying-review-verdict-missing",
            "receipt": {
                "epoch": release.epoch,
                "pr": release.pr,
                "head": release.head,
                "base": release.base,
                "digest": release.digest,
            },
            "action": "keep the PR draft; a distinct reviewer run must record an accepted verdict for this exact latest receipt",
        }))
    })?;

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
    Ok(serde_json::json!({
        "ok": true,
        "issue": release.issue,
        "epoch": release.epoch,
        "pr": release.pr,
        "head": release.head,
        "base": release.base,
        "digest": release.digest,
        "reviewer": review_verdict.reviewer,
        "draft": false,
        "ci_released": true,
    }))
}

fn require_pr_matches(
    current: &serde_json::Value,
    receipt: &ReviewReceipt,
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
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": if expected_draft { "release-pr-mismatch" } else { "ready-readback-failed" },
            "recorded": {"pr": receipt.pr, "head": receipt.head, "base": receipt.base, "draft": expected_draft},
            "observed": current,
            "action": if expected_draft {
                "do not release CI; the current draft PR is not the exact recorded publication"
            } else {
                "the ready write may have landed differently; re-read the PR before taking any further action"
            },
        })));
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
fn stage(body: &str, name: &str) -> Result<String, Failure> {
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
    kind: &str,
    expected: &[(&str, &str)],
) -> Result<Option<super::markers::Marker>, Failure> {
    let conflict = || {
        Failure::Stop(serde_json::json!({
            "ok": false, "reason": format!("{kind}-operation-conflict"),
        }))
    };
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
            mark.get("kind").map(String::as_str) == Some(kind)
                && mark.get("op-id").map(String::as_str) == Some(operation_id)
        })
        .collect();
    let Some(persisted) = matches.first() else {
        return Ok(None);
    };

    let fields = ownership::OPERATION_FIELDS
        .iter()
        .find(|(name, _)| *name == kind)
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
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": format!("{kind}-metadata-mismatch"),
            "persisted": persisted,
        })));
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
/// a glance. The port had half of one of them: [`converge_projection`] turns a
/// runtime into label edits, and **nothing called it** — while the transport
/// converges the projection after every claim, reclaim and unassign, twelve
/// call sites in all. The assignee had no equivalent here at all.
///
/// Two things follow from getting it wrong, and only the first is cosmetic.
/// A stale `dev:` label names the previous holder to everyone reading the
/// tracker. And the same labels are what [`plan_release`] consults to decide
/// whether a legacy acquisition may be released and whether a successor can be
/// named — so a projection nobody writes is an authorisation nobody can check.
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
    let data = super::gh_json(&["api", "user"], Some(&context.repo_dir))?.unwrap_or_default();
    match data.get("login").and_then(serde_json::Value::as_str) {
        Some(login) if !login.is_empty() => Ok(login.to_owned()),
        _ => Err(Failure::Read("gh api user returned no login".to_owned())),
    }
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

/// Projects the reducer's holder onto the issue, then proves the exact sets
/// landed.
///
/// This is the half the port did not have. After the claim marker is posted and
/// read back, the transport writes the projection — the assignee and the
/// `dev:<runtime>` label — and reads **that** back too, refusing when it does
/// not agree. This side answered `ok` at the point the transport was still
/// working, so an issue could carry a claim nobody could see from the issue
/// page, and a stale `dev:` label from a previous holder could outlive the run
/// that put it there.
///
/// Bounded, and every arm of the bound says something different:
///
/// - **ownership changed before the projection** — the holder moved while this
///   was reading. Nothing was written, so this is a stop and not an ambiguity.
/// - **projection did not read back exactly** — the write landed and the issue
///   does not show it. Ambiguous whenever this run wrote, because the edit may
///   have taken effect after the last read.
/// - **holder-runtime-missing** — the holder's runtime cannot be established, so
///   the assignee is deliberately *not* projected: unknown provenance must not
///   leave a previous holder standing as authoritative.
/// - **ownership-changed-projections-repaired** — the sets are right now, but
///   they are somebody else's; the caller asked about a claim it no longer has.
pub fn converge_ownership_projection(
    context: &Context,
    issue: u64,
    event: Option<ownership::Event>,
    login: Option<&str>,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let login = match login {
        Some(named) if !named.is_empty() => named.to_owned(),
        _ => viewer_login(context)?,
    };
    let read = || -> Result<serde_json::Value, Failure> {
        super::gh_json(
            &[
                "issue",
                "view",
                &issue.to_string(),
                "--json",
                "assignees,labels,comments",
            ],
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
        let mut seen: Vec<Option<serde_json::Value>> = Vec::new();
        for _ in 0..VISIBILITY_ATTEMPTS {
            seen.push(read().ok());
        }
        if seen.iter().all(Option::is_none) {
            return Err(unreadable(
                wrote,
                Failure::Read(format!("gh issue view {issue} never answered")),
            ));
        }
        let settled = wait_for(
            &seen,
            |data| ownership_identity(holding(&comments_of(data), now).event.as_ref()) == expected,
            2,
        );
        let Some(data) = settled else {
            // The holder moved under this read. Take the newest one and try
            // again — the projection belongs to whoever holds it now.
            let latest =
                seen.iter().rev().flatten().next().ok_or_else(|| {
                    unreadable(wrote, Failure::Read("no read answered".to_owned()))
                })?;
            event = holding(&comments_of(latest), now).event;
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
        let mut after: Vec<Option<serde_json::Value>> = Vec::new();
        for _ in 0..VISIBILITY_ATTEMPTS {
            after.push(read().ok());
        }
        let landed = wait_for(
            &after,
            |data| {
                ownership_identity(holding(&comments_of(data), now).event.as_ref()) == expected
                    && projection_state(data) == (wanted_assignees.clone(), wanted_runtimes.clone())
            },
            2,
        );
        let Some(landed) = landed else {
            let Some(latest) = after.iter().rev().flatten().next() else {
                return Err(unreadable(
                    wrote,
                    Failure::Read("ownership projection could not be re-read".to_owned()),
                ));
            };
            let now_held = holding(&comments_of(latest), now).event;
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
                }))
            });
        };
        if unresolved_runtime {
            return Err(Failure::Stop(
                serde_json::json!({"ok": false, "reason": "holder-runtime-missing"}),
            ));
        }
        if changed {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "ownership-changed-projections-repaired",
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
        }))
    })
}

/// What a claim attempt decided, before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    /// This operation already wrote its claim; report it rather than write again.
    AlreadyWritten,
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
/// - **`already-owned-by-different-operation`** — this run already holds it under
///   another operation id. Writing a second acquisition would make one run
///   appear twice and give it two epochs to be released from.
/// - and when neither applies, the horizon still has to be **ahead of now**,
///   because a claim whose deadline has passed is stale the moment it lands.
pub fn may_claim(before: &ownership::Holding, run_id: &str) -> Result<(), Failure> {
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
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "already-owned-by-different-operation",
            "operation_id": mine.operation_id,
        })));
    }
    Ok(())
}

/// How a claim's adjudication ended, once its event is on the timeline.
///
/// Read **after** the write, from the timeline rather than from the write's own
/// answer: a claim is not won by posting, it is won by being the earliest live
/// acquisition once everybody has posted.
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
            "ok": false, "reason": "claim-operation-no-longer-current", "winner": after.holder,
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
pub fn claim(context: &Context, what: &Acquisition<'_>) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(what.operation_id))?;
    require_horizon(what.horizon, None)?;

    let read = |fields: &str| -> Result<serde_json::Value, Failure> {
        super::gh_json(
            &["issue", "view", &what.issue.to_string(), "--json", fields],
            Some(&context.repo_dir),
        )?
        .ok_or_else(|| Failure::Read(format!("gh issue view {} returned nothing", what.issue)))
    };
    const FIELDS: &str = "assignees,labels,comments";

    let data = read(FIELDS)?;
    let comments = comments_of(&data);
    require_review_eligibility(&comments, what.run_id)?;
    reject_operation_kind_conflict(&comments, &operation_id, &["claim", "standdown"])?;
    let expected = [
        ("run-id", what.run_id),
        ("runtime", what.runtime),
        ("horizon", what.horizon),
    ];
    let existing = operation_marker(&comments, &operation_id, "claim", &expected)?;

    if existing.is_none() {
        // Ahead of *now*, not merely well formed: a claim whose deadline has
        // already passed is stale the moment it lands.
        require_horizon(what.horizon, Some(what.now))?;
        may_claim(&holding(&comments, what.now), what.run_id)?;

        super::run(
            &[
                "gh",
                "label",
                "create",
                &format!("dev:{}", what.runtime),
                "--color",
                "bfd4f2",
                "--force",
            ],
            Some(&context.repo_dir),
            super::How::tolerated(),
        )?;

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
            Failure::Stop(serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }))
        })?;
        let body = format!(
            "Claimed by {}, expect to report by {}.

{marker}
",
            what.run_id, what.horizon
        );
        let staged = stage(&body, &format!("claim-{}", what.issue))?;
        let answer = super::run(
            &[
                "gh",
                "issue",
                "comment",
                &what.issue.to_string(),
                "--body-file",
                &staged,
            ],
            Some(&context.repo_dir),
            super::How::write(),
        );
        let _ = std::fs::remove_file(&staged);
        answer?;

        // Wait for it to become visible **without writing again**: the comment
        // has landed, and posting a second one because the tracker had not
        // caught up is how one claim becomes two.
        let mut seen = Vec::new();
        for _ in 0..VISIBILITY_ATTEMPTS {
            let observed = read(FIELDS).ok();
            let visible = observed.as_ref().is_some_and(|data| {
                operation_marker(&comments_of(data), &operation_id, "claim", &expected)
                    .is_ok_and(|found| found.is_some())
            });
            seen.push(observed);
            if visible {
                break;
            }
        }
        if wait_for(
            &seen,
            |data| {
                operation_marker(&comments_of(data), &operation_id, "claim", &expected)
                    .is_ok_and(|found| found.is_some())
            },
            1,
        )
        .is_none()
        {
            // Ambiguous, never a stop: the comment may well have landed and
            // simply not be visible yet.
            return Err(Failure::Write(
                "claim operation is not visible; retry with the same operation ID".to_owned(),
            ));
        }
    }

    let latest = read(FIELDS)?;
    let after = holding(&comments_of(&latest), what.now);
    let mut answer = adjudicate(&after, &operation_id, what.run_id, existing.is_some())?;

    // The projection, and not one step earlier: `adjudicate` is what says this
    // run holds the issue, and projecting for a run that lost the race would
    // put its name on somebody else's work.
    //
    // This call is the half the port did not have. Without it the marker landed
    // and the issue page showed nothing — no assignee, and whatever `dev:` label
    // the previous holder left behind — so this side answered `ok` at the point
    // the transport was still working.
    converge_ownership_projection(context, what.issue, after.event, None, what.now)?;

    answer["issue"] = serde_json::json!(what.issue);
    Ok(answer)
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
/// Markers carry the runtime now; the run-id prefix is what keeps an older one
/// identifiable, and a release has to identify the run it hands the issue to.
/// Whether a holding event belongs to a runtime.
///
/// Markers carry a `runtime` now; the run-id prefix is what keeps a marker
/// written before they did readable. It decides which `dev:` label a **legacy**
/// holder's projection converges on, and a legacy holder is the one case where
/// the runtime cannot be read off the marker at all.
///
/// Public so the differential could reach it; that suite is deleted, so this
/// visibility outlives its reason too. Tracing a whole crossing run showed
/// this predicate was **never executed on either side**: every fixture's holder
/// carries a modern marker, so the inference it exists for was never asked for.
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
/// - **`unassign-metadata-mismatch`** — the runtime does not match what the
///   acquisition recorded. A release has to be *this* acquisition's, not one that
///   merely shares a run-id.
/// - **`target-operation-mismatch`** — the epoch named is not the one that would
///   be released. Between the discovery and the write, ownership moved.
pub fn plan_release(
    ownership: &ownership::Holding,
    run_id: &str,
    runtime: &str,
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

    // A recorded runtime has to be the one releasing. An acquisition that
    // recorded none is legacy — and is held to the labels instead, which was
    // the half of this that went missing. Without it any runtime could release
    // a legacy acquisition, on nothing but its own say-so.
    let mismatched = match mine.runtime.as_deref() {
        Some(recorded) => recorded != runtime,
        None => dev != [runtime],
    };
    if mismatched {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false, "reason": "unassign-metadata-mismatch",
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
    /// Its runtime, which must match what the acquisition recorded.
    pub runtime: &'a str,
    /// The operation id — fresh once, reused on retry.
    pub operation_id: &'a str,
    /// The epoch to end. Absent on the discovery call.
    pub target_operation: Option<&'a str>,
    /// Whether the caller believes somebody else holds it.
    pub held_by_other: bool,
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
    let read = || -> Result<serde_json::Value, Failure> {
        super::gh_json(
            &[
                "issue",
                "view",
                &what.issue.to_string(),
                "--json",
                "assignees,labels,comments",
            ],
            Some(&context.repo_dir),
        )?
        .ok_or_else(|| Failure::Read(format!("gh issue view {} returned nothing", what.issue)))
    };

    let data = read()?;
    let comments = comments_of(&data);
    reject_operation_kind_conflict(&comments, &operation_id, &["unassign"])?;

    let ownership = holding(&comments, what.now);

    // The marker first, as the transport has it. This planned the release
    // before looking, and a plan made after the write has landed says
    // `nothing-to-unassign` — because there is nothing left to release. A run
    // repeating its own unassign after an ambiguous write was told its release
    // had never happened, which is the opposite of what was true.
    let mut sought: Vec<(&str, &str)> = vec![("run-id", what.run_id), ("runtime", what.runtime)];
    if let Some(requested) = what.target_operation {
        sought.push(("target-op", requested));
    }
    if let Some(existing) = operation_marker(&comments, &operation_id, "unassign", &sought)? {
        return resumed_release(&comments, &ownership, &existing, &operation_id, what);
    }

    let plan = plan_release(
        &ownership,
        what.run_id,
        what.runtime,
        what.held_by_other,
        what.target_operation,
        &labels_of(&data),
    )?;
    let Release::Write { target_operation } = plan else {
        let Release::Confirm { target_operation } = plan else {
            unreachable!("the plan is one of two shapes")
        };
        return Ok(serde_json::json!({
            "ok": true,
            "issue": what.issue,
            "write_performed": false,
            "target_operation": target_operation,
            "next": "repeat unassign with --target-operation and the same operation ID",
        }));
    };

    let expected = [
        ("run-id", what.run_id),
        ("runtime", what.runtime),
        ("target-op", target_operation.as_str()),
    ];
    if operation_marker(&comments, &operation_id, "unassign", &expected)?.is_none() {
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
            Failure::Stop(serde_json::json!({ "ok": false, "reason": "invalid-marker-attribute" }))
        })?;
        let body = format!(
            "`{}` releasing exact epoch `{target_operation}`.

{marker}
",
            what.run_id
        );
        let staged = stage(&body, &format!("unassign-{}", what.issue))?;
        let answer = super::run(
            &[
                "gh",
                "issue",
                "comment",
                &what.issue.to_string(),
                "--body-file",
                &staged,
            ],
            Some(&context.repo_dir),
            super::How::write(),
        );
        let _ = std::fs::remove_file(&staged);
        answer?;

        let mut seen = Vec::new();
        for _ in 0..VISIBILITY_ATTEMPTS {
            let observed = read().ok();
            let visible = observed.as_ref().is_some_and(|data| {
                operation_marker(&comments_of(data), &operation_id, "unassign", &expected)
                    .is_ok_and(|found| found.is_some())
            });
            seen.push(observed);
            if visible {
                break;
            }
        }
        if seen.iter().flatten().last().is_none_or(|data| {
            operation_marker(&comments_of(data), &operation_id, "unassign", &expected)
                .is_ok_and(|found| found.is_none())
        }) {
            return Err(Failure::Write(
                "unassign operation is not visible; retry with the same operation ID".to_owned(),
            ));
        }
    }

    // Read back, and **believe the read-back**: a release is not done because
    // the comment posted, it is done because the epoch stopped being
    // authoritative.
    let after = holding(&comments_of(&read()?), what.now);
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
    converge_ownership_projection(context, what.issue, after.event.clone(), None, what.now)?;

    Ok(serde_json::json!({
        "ok": true,
        "issue": what.issue,
        "assignee_kept": after.holder.is_some(),
    }))
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
        return Err(stop(if before.stale.is_empty() {
            "nothing-to-reclaim"
        } else {
            "stale-self-requires-claim"
        }));
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

    let read = || -> Result<serde_json::Value, Failure> {
        super::gh_json(
            &[
                "issue",
                "view",
                &what.issue.to_string(),
                "--json",
                "state,assignees,labels,comments",
            ],
            Some(&context.repo_dir),
        )?
        .ok_or_else(|| Failure::Read(format!("gh issue view {} returned nothing", what.issue)))
    };
    let data = read()?;
    if text(&data, "state") != "OPEN" {
        return Err(Failure::Stop(
            serde_json::json!({ "ok": false, "reason": "issue-not-open" }),
        ));
    }
    let comments = comments_of(&data);
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
            return Ok(serde_json::json!({
                "ok": true,
                "issue": what.issue,
                "write_performed": false,
                "target_operation": target_operation,
                "holder": holder,
                "next": "repeat reclaim with --target-operation and the same operation ID",
            }));
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
        ("runtime", what.runtime),
        ("horizon", what.horizon),
        ("from-op", target_operation.as_str()),
    ];
    // A reclaim marker already under this operation id is a **retry**, not a
    // second takeover, and the transport answers it on a path of its own. This
    // had no such path: it skipped the write, fell into the read-back below,
    // and reported `reused_existing_reclaim: false` — telling a caller nothing
    // was already there when something was.
    if let Some(existing) = operation_marker(&comments, &operation_id, "reclaim", &expected)? {
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
        return Ok(serde_json::json!({
            "ok": true,
            "issue": what.issue,
            "reclaimed_from": holder,
            "run_id": what.run_id,
            "forced": forced,
            "reused_existing_reclaim": true,
        }));
    }

    {
        let body = format!(
            "`{}` reclaiming from `{holder}`.{note}

{marker}
",
            what.run_id
        );
        let staged = stage(&body, &format!("reclaim-{}", what.issue))?;
        let answer = super::run(
            &[
                "gh",
                "issue",
                "comment",
                &what.issue.to_string(),
                "--body-file",
                &staged,
            ],
            Some(&context.repo_dir),
            super::How::write(),
        );
        let _ = std::fs::remove_file(&staged);
        answer?;
    }

    // Read back, and believe the read-back: a takeover is not done because the
    // comment posted, it is done because this operation is the current event.
    //
    // Three different things can be true here and this answered all of them
    // with one `Stop`. A `Stop` says *this did not happen*, and the first of
    // the three is a write whose fate is unknown — the comment may be on the
    // issue and simply not visible yet. Telling a caller nothing happened
    // there is what makes it post a second takeover. [`wait_for`] carries that
    // rule in its own documentation and this was the caller that needed it.
    let mut seen = Vec::new();
    let landed = |data: &serde_json::Value| {
        operation_marker(&comments_of(data), &operation_id, "reclaim", &expected)
            .is_ok_and(|found| found.is_some())
    };
    for _ in 0..VISIBILITY_ATTEMPTS {
        let observed = read().ok();
        let visible = observed.as_ref().is_some_and(landed);
        seen.push(observed);
        if visible {
            break;
        }
    }
    let Some(latest) = wait_for(&seen, landed, 1) else {
        return Err(Failure::Write(
            "the reclaim is not visible after a bounded read-back; do NOT repeat the write, \
             retry with the same operation ID"
                .to_owned(),
        ));
    };

    let after = holding(&comments_of(latest), what.now);
    let event = after.event.clone();
    let forced = won_the_reclaim(after.event, what.run_id)?;

    // A takeover moves the projection as surely as a claim does, and moving it
    // is most of the point: the displaced holder's assignee and `dev:` label
    // stay on the issue until somebody takes them off, and an issue that still
    // names the run it was taken from is an issue nobody can read correctly.
    converge_ownership_projection(context, what.issue, event, None, what.now)?;

    Ok(serde_json::json!({
        "ok": true,
        "issue": what.issue,
        "reclaimed_from": holder,
        "run_id": what.run_id,
        "forced": forced,
        "reused_existing_reclaim": false,
    }))
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
            event.position < control
                && &epoch(event) == target_epoch
                && event.run_id == what.run_id
                && event
                    .runtime
                    .as_deref()
                    .is_none_or(|recorded| recorded == what.runtime)
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
    Ok(serde_json::json!({
        "ok": true,
        "issue": what.issue,
        "assignee_kept": ownership.holder.is_some(),
    }))
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
        })));
    };
    Ok(current.forced)
}

/// Whether a **freshly written** reclaim won, and whether it was forced.
///
/// On run-id, as the transport has it: what this call promised is that this
/// *run* holds the issue afterwards, and it is the run the caller acts as.
///
/// Nothing holding the issue after the comment landed is a **write** failure
/// and not a stop. Something was written, and the caller must re-read rather
/// than conclude the takeover never happened.
fn won_the_reclaim(current: Option<ownership::Event>, run_id: &str) -> Result<bool, Failure> {
    let Some(current) = current else {
        return Err(Failure::Write(
            "the reclaim comment landed but no live holder was established".to_owned(),
        ));
    };
    if current.run_id != run_id {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "lost-reclaim-race",
            "winner": current.run_id,
        })));
    }
    Ok(current.forced)
}

#[cfg(test)]
mod tests;
