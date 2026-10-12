//! Reading what the tracker answered.
//!
//! This module used to *ask*: it spawned `python <skill>/scripts/github.py`,
//! read its JSON and translated the exit code. Every one of those calls now goes
//! to [`crate::transport::dispatch`] in this process, and what is left here is
//! the half that was always the point — turning an exit code into a refusal
//! somebody can act on.
//!
//! The spawning went with the script. It was dead the moment the last caller
//! moved, and dead is not harmless: `invoke` was public, it began by checking
//! for a file `setup` deliberately no longer installs, and it answered a
//! **complete** installation with *scripts/github.py is not installed* and
//! *run `estigia setup --all`* — the command that had just been run. A message
//! may name a command only when running it clears the block; that one named a
//! dead end, so it is gone rather than documented.
//!
//! # The exit codes are the whole point
//!
//! The transport already draws the distinction the exit-code contract of
//! issue-flow could not express, and it draws it in the one place that matters:
//!
//! > A failed CHECK is a stop; a failed READ is nothing.
//!
//! `0` the check passed. `1` the control surface was read and answered stop.
//! `2` the operator's configuration is wrong, **or the caller's argument was**
//! — two faults on one code, and only the first has a command that fixes it, so
//! [`translate`] takes the table's row, then the transport's own `action`, and
//! names a command only when neither exists. `3` the control surface answered **nothing**. `4` and
//! `5` something may have been written and nobody knows.
//!
//! Translating those into one boolean would throw away the distinction that
//! incident I07 was filed about — a run that lost a claim race by five seconds,
//! was told 33 seconds later, and worked another 48 minutes because nothing
//! read the timeline again. So each one becomes a different [`Refusal`], with a
//! different [`MutationOutcome`] and a different [`Replayability`].

use serde_json::Value;

use super::population::{Disposition, Population};
use crate::outcome::{MutationOutcome, NoCommandReason, Refusal, Replayability, Resolution};

/// Where an [`Answer`] stands: a code that reads as success, a code-1 stop
/// naming its reason, or anything a caller can read as neither.
///
/// Collapsing this into a boolean is what let a healthy pointer and an
/// unreachable tracker share one value in [`super::doctor::stale_run_pointers`]
/// before this type existed — "the tracker answered ok" and "nothing was
/// read" are not the same thing, and callers that need the difference now
/// have a name for each side of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing<'a> {
    /// Code `0`: the check passed.
    Passed,
    /// Code `1` carrying a readable reason: the control surface was read
    /// and answered stop.
    Stopped(&'a str),
    /// Anything else — another code, or a code-1 answer whose body names
    /// no reason: nothing a caller can read as either pass or stop.
    Unread,
}

/// What the transport said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The exit code, kept raw because its meaning is the contract.
    pub code: i32,
    /// The parsed JSON body, when there was one.
    pub body: Option<Value>,
}

impl Answer {
    /// The `reason` field the transport sets on every refusal.
    pub fn reason(&self) -> Option<&str> {
        self.body.as_ref()?.get("reason")?.as_str()
    }

    /// The `reason` the tracker gave, but **only** when it was actually read
    /// and answered stop — `code == 1`. Every other code is a read that did
    /// not land or a write whose fate nobody observed, which `reason()` alone
    /// cannot tell apart from a genuine answer.
    pub fn stopped_reason(&self) -> Option<&str> {
        if self.code != 1 {
            return None;
        }
        self.reason()
    }

    /// Where this answer stands, as three answers rather than two — the same
    /// word [`super::standdown::Standing`] uses, for the same reason: a code
    /// that never landed a read is not the same thing as one that read and
    /// said no. Delegates to [`Self::stopped_reason`] rather than repeating
    /// its `code == 1` rule.
    pub fn standing(&self) -> Standing<'_> {
        if self.code == 0 {
            return Standing::Passed;
        }
        match self.stopped_reason() {
            Some(reason) => Standing::Stopped(reason),
            None => Standing::Unread,
        }
    }

    /// The `detail` field, when present.
    pub fn detail(&self) -> Option<&str> {
        self.body.as_ref()?.get("detail")?.as_str()
    }

    /// Whether this is the answer `verify_claim` gives when an issue closed
    /// behind this run's own delivery rather than somebody else's: its own
    /// `checked[0]` carries the prefix
    /// [`crate::transport::claim::OWN_DELIVERY_CLOSE_PREFIX`] builds. The sha
    /// and epoch it appends are not read here — nothing in the harness needs
    /// them.
    pub fn is_own_delivery_close(&self) -> bool {
        self.body
            .as_ref()
            .and_then(|body| body.get("checked"))
            .and_then(Value::as_array)
            .and_then(|checked| checked.first())
            .and_then(Value::as_str)
            .is_some_and(|first| {
                first.starts_with(crate::transport::claim::OWN_DELIVERY_CLOSE_PREFIX)
            })
    }

    /// The `action` field: the transport's own instruction to the agent.
    pub fn action(&self) -> Option<&str> {
        self.body.as_ref()?.get("action")?.as_str()
    }

    /// Whether the refusal arrived **after** the operation had already
    /// written. A retry resuming an earlier call's marker answers here exactly
    /// as the call that posted it: what the flag reports is that the write had
    /// landed, not which call made it, and not that the marker is still on the
    /// timeline. Absent means what it always meant, so every refusal that does
    /// not set it is unchanged.
    pub fn already_wrote(&self) -> bool {
        self.body
            .as_ref()
            .and_then(|body| body.get("world"))
            .and_then(Value::as_str)
            == Some("committed")
    }
}

/// What a malformed argument should have looked like: the reasons the transport
/// exits `2` for that are the caller's own typo rather than the machine's state.
fn malformed_argument(reason: &str) -> Option<&'static str> {
    match reason {
        "invalid-horizon" => Some(
            "a horizon as an instant this run expects to report by, exactly \
             `YYYY-MM-DDTHH:MMZ` or `YYYY-MM-DDTHH:MM:SSZ`, and later than the claim it renews",
        ),
        "invalid-operation-id" => Some(
            "an operation id of exactly 32 lowercase hexadecimal characters, as the transport \
             issued it",
        ),
        "blank-argument" => Some(
            "a value for the flag the detail names \u{2014} it arrived with nothing in it, and \
             the configuration has no part in that",
        ),
        _ => None,
    }
}

/// What is missing when the GitHub CLI has no login, in one sentence. Held here
/// because two places answer for it and they must not differ. A `no_command`
/// sentence on purpose: `gh auth login` opens a browser, so naming it as *the*
/// command to run would promise something a headless agent cannot do.
pub const UNAUTHENTICATED: &str =
    "an authenticated GitHub CLI \u{2014} run `gh auth login`, which needs a person at a browser";

/// The population and boundary [`translate`] enforces.
pub const TRANSLATE_POPULATION: Population = Population {
    family: "exit-code",
    disposition: Disposition::FailClosed,
    subject: "translate",
    text: "every exit code the transport can return. Legitimate population: `0` through `5`, \
           which is the whole contract its `main()` implements. Boundary: a code outside that \
           range is not a code this mapping has read, and it lands in the `Unknown` arm rather \
           than the stop arm \u{2014} because reporting \"nothing happened\" for a code nobody \
           has seen is the lie the taxonomy exists to prevent. The proof boundary: this shows \
           an unknown code fails towards Unknown, not that the six named ones are mapped to \
           the right outcomes.",
};

/// Turns a transport answer into a refusal, keeping every distinction it drew.
///
/// The mapping is the contract, so it is written once here rather than at each
/// call site. `translate` never invents a resolution: when the transport has
/// named an `action`, that text is what the agent is told, because the transport
/// is the thing that knows which of the four retry shapes applies.
///
/// A refusal whose envelope carries no `detail` is messaged from the envelope's
/// own scalar fields rather than the exit code alone — for **every** code that
/// reaches here without one, not only `2`. `Failure::envelope` always writes a
/// `detail` for `Read`, `Timeout` and `Write`, so `3`, `4` and `5` keep the
/// exit-code sentence; a code-1 `Stop` payload carrying a field beside `ok`,
/// `reason`, `action` and `world` does not.
pub fn translate(answer: &Answer, context: &str) -> Option<Refusal> {
    if answer.code == 0 {
        return None;
    }
    let reason = answer.reason().unwrap_or("transport-refused");
    let detail = answer
        .detail()
        .map(str::to_owned)
        .unwrap_or_else(|| envelope_detail(answer, context));

    // See the `exit-code` population declaration on this function.
    let (outcome, replay, resolution) = match answer.code {
        1 if answer.already_wrote() => (
            MutationOutcome::Committed,
            // Not `ExactReplaySafe`: repeating the identical call mints a fresh
            // epoch over a head somebody else pushed. Read first, then decide.
            Replayability::StatusRequired,
            match answer.action() {
                Some(action) => Resolution::no_command(NoCommandReason::WorldAction, action),
                None => Resolution::no_command(
                    NoCommandReason::WorldAction,
                    "this operation had already written before it refused; re-read the \
                     repository before deciding",
                ),
            },
        ),
        // The control surface was READ, and it answered stop: retrying the
        // identical command repeats the answer.
        1 => (
            MutationOutcome::NotStarted,
            Replayability::NotReplayable,
            match answer.action() {
                Some(action) => Resolution::no_command(NoCommandReason::WorldAction, action),
                None => Resolution::no_command(
                    NoCommandReason::WorldAction,
                    "the tracker's authority changed under this run; re-read the issue",
                ),
            },
        ),
        // The operator's configuration is wrong, **or** an argument was
        // malformed — two different faults sharing one code, and only the first
        // has a command that fixes it. The transport says which of the two it
        // was, so it is asked.
        2 => (
            MutationOutcome::NotStarted,
            Replayability::ExactReplaySafe,
            match (
                answer.reason().and_then(malformed_argument),
                answer.action(),
            ) {
                (Some(wanted), _) | (None, Some(wanted)) => {
                    Resolution::no_command(NoCommandReason::OperatorKnowledge, wanted)
                }
                (None, None) => Resolution::run("estigia config list"),
            },
        ),
        // Nothing was answered. Not a stand-down and never clearance. A `gh`
        // nobody has logged in is not a read worth retrying, and it says which
        // it is in the detail already being carried.
        3 if detail.contains("gh auth login") => (
            MutationOutcome::NotStarted,
            Replayability::ManualActionRequired,
            Resolution::no_command(NoCommandReason::HumanAuthority, UNAUTHENTICATED),
        ),
        3 if reason == "rate-limited" => (
            MutationOutcome::NotStarted,
            Replayability::ManualActionRequired,
            Resolution::no_command(
                NoCommandReason::WorldAction,
                answer.action().unwrap_or(
                    "the GitHub API rate limit has to reset; retrying spends the budget being \
                     waited for",
                ),
            ),
        ),
        3 => (
            MutationOutcome::NotStarted,
            Replayability::ExactReplaySafe,
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "the tracker could not be read; write nothing and retry the read",
            ),
        ),
        // The state is unknown: both end with a caller that must go and look
        // rather than assume.
        _ => (
            MutationOutcome::Unknown,
            Replayability::StatusRequired,
            match answer.action() {
                Some(action) => Resolution::no_command(NoCommandReason::WorldAction, action),
                None => Resolution::no_command(
                    NoCommandReason::WorldAction,
                    "re-read the issue to establish what actually happened",
                ),
            },
        ),
    };

    let refusal = Refusal {
        code: stable_code(reason),
        message: detail,
        outcome,
        replay,
        resolution,
        class: super::refusal_class::class_of_answer(answer.code, reason),
        observed: None,
    };
    let class = refusal.class;
    Some(refusal.classed(class))
}

/// The fields the envelope carries beside the ones the refusal is built from,
/// as text; a value that is a null or an object is dropped rather than named.
fn envelope_detail(answer: &Answer, context: &str) -> String {
    let fields: Vec<String> = answer
        .body
        .as_ref()
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(key, _)| !matches!(key.as_str(), "ok" | "reason" | "action" | "world"))
        .filter_map(|(key, value)| scalar_text(value).map(|text| format!("{key}: {text}")))
        .collect();
    if fields.is_empty() {
        format!("{context}: the transport exited {}", answer.code)
    } else {
        format!("{context}: {}", fields.join(", "))
    }
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Array(items) => {
            let texts: Vec<String> = items.iter().filter_map(scalar_text).collect();
            (texts.len() == items.len()).then(|| texts.join(" | "))
        }
        Value::Null | Value::Object(_) => None,
    }
}

/// The transport's `reason` strings, as `&'static str` codes.
///
/// [`Refusal::code`] is `&'static str` on purpose — a code is matched on, and a
/// code built at runtime is a code nobody can match. The list is the transport's
/// own vocabulary; anything outside it collapses to one honest catch-all rather
/// than leaking a borrowed string.
fn stable_code(reason: &str) -> &'static str {
    TRANSPORT_VOCABULARY
        .iter()
        .copied()
        .find(|known| *known == reason)
        .unwrap_or("transport-refused")
}

include!(concat!(env!("OUT_DIR"), "/transport_vocabulary.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn answer(code: i32, body: Value) -> Answer {
        Answer {
            code,
            body: Some(body),
        }
    }

    #[test]
    fn every_class_is_reported_as_itself_and_never_as_its_neighbour() {
        use crate::outcome::RefusalClass;
        let cases = [
            (3, "read-failed", RefusalClass::Unavailable),
            (3, "rate-limited", RefusalClass::Unavailable),
            (
                1,
                "published-transaction-unparseable",
                RefusalClass::Unavailable,
            ),
            (1, "published-transaction-mismatch", RefusalClass::Stale),
            (1, "claim-operation-conflict", RefusalClass::Stale),
            (1, "reviewer-not-distinct", RefusalClass::Refused),
            (2, "malformed-argument", RefusalClass::Refused),
            (5, "write-failed", RefusalClass::Unknown),
            (5, "ambiguous-write", RefusalClass::Unknown),
        ];
        for (exit, reason, wanted) in cases {
            let refusal = translate(
                &answer(
                    exit,
                    json!({ "ok": false, "reason": reason, "detail": "d", "action": "a" }),
                ),
                "review",
            )
            .expect("a refusal");
            assert_eq!(refusal.class, wanted, "exit {exit} `{reason}`");
        }
    }

    #[test]
    fn a_failed_read_is_never_stale_and_an_unknown_write_is_never_a_named_state() {
        use crate::outcome::RefusalClass;
        let unread = translate(
            &answer(
                3,
                json!({ "ok": false, "reason": "read-failed", "detail": "gh timed out" }),
            ),
            "review",
        )
        .expect("a refusal");
        assert_ne!(unread.class, RefusalClass::Stale);
        assert_ne!(unread.class, RefusalClass::Refused);
        let unknown = translate(
            &answer(
                5,
                json!({ "ok": false, "reason": "ambiguous-write", "detail": "timed out" }),
            ),
            "review",
        )
        .expect("a refusal");
        assert_eq!(unknown.class, RefusalClass::Unknown);
        assert_eq!(unknown.outcome, MutationOutcome::Unknown);
    }

    #[test]
    fn a_stop_that_already_wrote_is_not_reported_as_nothing_written() {
        let committed = translate(
            &answer(
                1,
                json!({
                    "ok": false,
                    "reason": "publication-readback-disagrees",
                    "world": "committed",
                    "action": "re-read the pull request before publishing again",
                }),
            ),
            "publish_review",
        )
        .expect("a refusal");
        assert_eq!(
            committed.outcome,
            MutationOutcome::Committed,
            "a refusal that had already pushed reported {:?}",
            committed.outcome
        );
        assert_eq!(
            committed.outcome.what_happened(),
            "the write landed; what failed came after it"
        );

        // And the ordinary stop, which is every other refusal in the transport.
        // `stale-foreign-requires-reclaim` is one the transport still answers
        // without a world: it is decided before anything is written.
        let untouched = translate(
            &answer(
                1,
                json!({ "ok": false, "reason": "stale-foreign-requires-reclaim" }),
            ),
            "claim",
        )
        .expect("a refusal");
        assert_eq!(untouched.outcome, MutationOutcome::NotStarted);
        assert_eq!(untouched.outcome.what_happened(), "nothing was written");
    }

    #[test]
    fn a_login_nobody_has_done_is_not_answered_with_retry_the_read() {
        let refusal = translate(
            &answer(
                3,
                json!({
                    "ok": false,
                    "reason": "read-failed",
                    "detail": "gh issue view failed (4): To get started with GitHub CLI, please \
                               run:  gh auth login"
                }),
            ),
            "verify",
        )
        .expect("a failed read is a refusal");
        assert_eq!(
            refusal.replay,
            Replayability::ManualActionRequired,
            "an agent is told to repeat a call that cannot succeed"
        );
        assert_eq!(
            refusal.resolution,
            Resolution::no_command(NoCommandReason::HumanAuthority, UNAUTHENTICATED),
            "the answer is not the one `doctor` gives for the same machine"
        );

        // The floor: a read that failed for any other reason keeps the advice
        // that fits it, or this would have turned every unreadable tracker into
        // something only a person can fix.
        let blinked = translate(
            &answer(
                3,
                json!({"ok": false, "reason": "read-failed", "detail": "connection reset"}),
            ),
            "verify",
        )
        .expect("a failed read is a refusal");
        assert_eq!(blinked.replay, Replayability::ExactReplaySafe);
        assert_ne!(blinked.resolution, refusal.resolution);
    }

    #[test]
    fn a_passing_check_is_not_a_refusal() {
        assert!(translate(&answer(0, json!({"ok": true})), "verify").is_none());
    }

    #[test]
    fn a_stop_says_nothing_was_written_and_a_replay_will_not_help() {
        // Incident I07: the run that kept working after being told it lost.
        let refusal = translate(
            &answer(
                1,
                json!({
                    "ok": false,
                    "reason": "not-current-live-holder",
                    "detail": "claude-aaaa lost the race",
                    "action": "stop; the issue is not yours"
                }),
            ),
            "verify",
        )
        .expect("a stop is a refusal");
        assert_eq!(refusal.code, "not-current-live-holder");
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        assert_eq!(refusal.replay, Replayability::NotReplayable);
        assert!(refusal.to_string().contains("the issue is not yours"));
    }

    #[test]
    fn a_read_that_answered_nothing_is_never_clearance_and_never_a_stand_down() {
        // The distinction the module exists for. `3` must not become `1`.
        let refusal = translate(
            &answer(3, json!({"ok": false, "reason": "read-failed"})),
            "verify",
        )
        .expect("a read failure is a refusal");
        assert_eq!(refusal.code, "read-failed");
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        // Retrying the read is exactly the right move, unlike a stop.
        assert_eq!(refusal.replay, Replayability::ExactReplaySafe);
    }

    #[test]
    fn an_ambiguous_write_reports_an_unknown_world() {
        for code in [4, 5] {
            let refusal = translate(
                &answer(
                    code,
                    json!({"ok": false, "reason": "ambiguous-write",
                           "action": "RE-READ the branch, link and refs before retrying"}),
                ),
                "transition",
            )
            .expect("an ambiguous write is a refusal");
            assert_eq!(refusal.outcome, MutationOutcome::Unknown, "exit {code}");
            assert_eq!(refusal.replay, Replayability::StatusRequired);
            assert!(!refusal.outcome.is_clean());
        }
    }

    #[test]
    fn an_exit_code_nobody_has_seen_fails_towards_unknown() {
        // The fail-closed arm of `TRANSLATE_POPULATION`. A code outside the
        // contract must not be reported as "nothing happened".
        let refusal = translate(&answer(99, json!({"ok": false})), "deliver")
            .expect("an unknown code is a refusal");
        assert_eq!(refusal.outcome, MutationOutcome::Unknown);
    }

    #[test]
    fn a_configuration_defect_is_the_one_that_names_a_command() {
        let refusal = translate(
            &answer(2, json!({"ok": false, "reason": "gh-not-found"})),
            "claim",
        )
        .expect("a config defect is a refusal");
        assert!(matches!(refusal.resolution, Resolution::Run { .. }));
    }

    #[test]
    fn an_exit_two_answer_carrying_an_action_is_told_that_action_and_its_cause() {
        let refusal = translate(
            &answer(
                2,
                json!({
                    "ok": false,
                    "reason": "finding-citation-unresolvable",
                    "field": "evidence",
                    "citation": "src/lib.rs:629",
                    "path": "src/lib.rs",
                    "cause": "path-not-in-either-tree",
                    "action": "correct or remove the citation",
                }),
            ),
            "record_review_finding",
        )
        .expect("an unresolvable citation is a refusal");
        assert_eq!(
            refusal.resolution,
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "correct or remove the citation"
            )
        );
        assert_eq!(
            refusal.message,
            "record_review_finding: field: evidence, citation: src/lib.rs:629, path: \
             src/lib.rs, cause: path-not-in-either-tree"
        );
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        assert_eq!(refusal.replay, Replayability::ExactReplaySafe);
    }

    #[test]
    fn an_exit_two_answer_without_an_action_still_carries_what_it_read() {
        let refusal = translate(
            &answer(
                2,
                json!({
                    "ok": false,
                    "reason": "invalid-finding-class",
                    "value": "blocking",
                    "expected": ["severe", "warning", "suggestion"],
                }),
            ),
            "record_review_finding",
        )
        .expect("an unknown class is a refusal");
        assert_eq!(
            refusal.message,
            "record_review_finding: value: blocking, expected: severe | warning | suggestion"
        );
        assert!(matches!(refusal.resolution, Resolution::Run { .. }));
    }

    #[test]
    fn a_transport_detail_is_carried_verbatim_over_the_envelope_fields() {
        let refusal = translate(
            &answer(
                2,
                json!({
                    "ok": false,
                    "reason": "blank-argument",
                    "argument": "--to",
                    "detail": "--to arrived with nothing in it",
                }),
            ),
            "transition",
        )
        .expect("a blank argument is a refusal");
        assert_eq!(refusal.message, "--to arrived with nothing in it");
    }

    #[test]
    fn a_stop_that_names_evidence_and_no_detail_hands_the_lists_to_the_agent() {
        // The shape `review-target-mismatch` really writes (`src/transport/target.rs`):
        // no `detail`, and the three lists its own `action` names. Before the
        // envelope was repeated, the agent was told `expected_target: the
        // transport exited 1` and an `action` naming a field it never received.
        let failure = crate::transport::Failure::Stop(json!({
            "ok": false,
            "reason": "review-target-mismatch",
            "base": "1111111111111111111111111111111111111111",
            "head": "2222222222222222222222222222222222222222",
            "expected_digest": "expected",
            "supplied_digest": "supplied",
            "unreviewed": ["src/lib.rs", "src/main.rs"],
            "not_delivered": ["docs/gone.md"],
            "differing": ["Cargo.toml"],
            "action": "rebuild the review over the complete target",
        }));
        let refusal = translate(
            &Answer {
                code: failure.code(),
                body: Some(failure.envelope()),
            },
            "expected_target",
        )
        .expect("a mismatched target is a refusal");

        // No refusal loses a field it delivers today.
        assert_eq!(refusal.code, "review-target-mismatch");
        assert_eq!(refusal.outcome, MutationOutcome::NotStarted);
        assert_eq!(refusal.replay, Replayability::NotReplayable);
        assert_eq!(
            refusal.resolution,
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "rebuild the review over the complete target"
            )
        );

        // The paths the `action` names reach the agent, both in the message
        // `refusal_result` renders and in the refusal's own text.
        assert!(
            refusal
                .message
                .contains("unreviewed: src/lib.rs | src/main.rs"),
            "the paths the action names never reached the agent: {}",
            refusal.message
        );
        assert!(refusal.message.contains("not_delivered: docs/gone.md"));
        assert!(refusal.message.contains("differing: Cargo.toml"));
        assert!(
            refusal.to_string().contains("src/lib.rs"),
            "the rendered refusal dropped the list: {refusal}"
        );
    }

    #[test]
    fn a_stop_with_nothing_scalar_falls_back_to_the_context_and_the_code() {
        // The last resort `envelope_detail` keeps: a payload whose only keys are
        // the ones `translate` renders itself leaves nothing to repeat, so the
        // sentence is the context and the exit code — and it is asserted here
        // rather than only written.
        let failure = crate::transport::Failure::Stop(json!({
            "ok": false,
            "reason": "stale-foreign-requires-reclaim",
        }));
        let refusal = translate(
            &Answer {
                code: failure.code(),
                body: Some(failure.envelope()),
            },
            "claim",
        )
        .expect("a stop is a refusal");
        assert_eq!(refusal.message, "claim: the transport exited 1");
    }

    #[test]
    fn an_envelope_field_that_is_not_scalar_is_dropped_and_a_scalar_is_repeated() {
        // The honesty sentence this holds: the fallback repeats a scalar-valued
        // field beside the ones the refusal is built from, and drops a null or
        // an object rather than naming it.
        let refusal = translate(
            &answer(
                2,
                json!({
                    "ok": false,
                    "reason": "missing-argument",
                    "operation": "claim",
                    "argument": "--horizon",
                    "value": "soon",
                    "count": 3,
                    "flag": true,
                    "listed": ["a", "b"],
                    "object_field": {"reason": "inner"},
                    "null_field": null,
                }),
            ),
            "claim",
        )
        .expect("a missing argument is a refusal");
        for repeated in [
            "operation: claim",
            "argument: --horizon",
            "value: soon",
            "count: 3",
            "flag: true",
            "listed: a | b",
        ] {
            assert!(
                refusal.message.contains(repeated),
                "the fallback dropped a scalar field: {repeated} is not in {}",
                refusal.message
            );
        }
        assert!(
            !refusal.message.contains("object_field"),
            "an object value was named rather than dropped: {}",
            refusal.message
        );
        assert!(
            !refusal.message.contains("null_field"),
            "a null value was named rather than dropped: {}",
            refusal.message
        );
    }

    #[test]
    fn the_vocabulary_covers_what_the_transport_actually_answers() {
        // The seam this file exists on. A `reason` outside the vocabulary
        // collapses to `transport-refused`, which loses the one thing the code
        // is for — and a hand-maintained list had 24 entries when the transport
        // had 69. The list is generated now, so this checks the generator
        // rather than a copy: it reads the shipped transport directly.
        // Both sides moved to the port together, as this said they would the
        // day it answered for all of it. It reads by the *narrow* rule — the
        // literal `"reason": "..."` pair — while `build.rs` reads by three,
        // adding the refusal constructor and the envelope's own table. That is
        // what keeps this from being a tautology: it asks whether the generator
        // covers at least what the plainest reading finds, and the generator
        // missing one of those is exactly how this seam fails.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("transport");
        let mut transport = String::new();
        let mut stack = vec![root];
        while let Some(at) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&at) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|kind| kind == "rs")
                    && path.file_name().is_some_and(|name| name != "tests.rs")
                {
                    let source = std::fs::read_to_string(&path).unwrap_or_default();
                    let source = crate::source_text::code_of(&source);
                    // Before the inline test module, for the reason `build.rs`
                    // gives: a fixture's reason is not one the product answers.
                    let shipped = source
                        .find("#[cfg(test)]\nmod tests {")
                        .map_or(source.as_str(), |end| &source[..end]);
                    transport.push_str(shipped);
                    transport.push('\n');
                }
            }
        }

        let mut found = 0;
        let mut rest = transport.as_str();
        while let Some(at) = rest.find("\"reason\":") {
            rest = &rest[at + 9..];
            let trimmed = rest.trim_start();
            let Some(body) = trimmed.strip_prefix('"') else {
                continue;
            };
            let Some(end) = body.find('"') else { break };
            let reason = &body[..end];
            if reason.is_empty()
                || !reason
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                continue;
            }
            found += 1;
            assert_eq!(
                stable_code(reason),
                reason,
                "the transport answers `{reason}` and the vocabulary collapses it"
            );
        }
        assert!(
            found > 60,
            "only {found} reasons were read from the transport"
        );
    }

    #[test]
    fn the_vocabulary_holds_the_reasons_the_taxonomy_turns_on() {
        // Spot checks on the ones whose loss would be worst: a lost race and an
        // unproven worktree are precise stops, and reporting either as a
        // generic refusal is how a run keeps working after being told to stop.
        for reason in [
            "lost-claim-race",
            "worktree-ownership-unproven",
            "worktree-owned-by-another-run",
            "publication-readback-disagrees",
            "review-target-mismatch",
            "closing-keyword-live",
        ] {
            assert_eq!(
                stable_code(reason),
                reason,
                "{reason} is not in the vocabulary"
            );
        }
    }

    #[test]
    fn a_refusal_reaching_the_harness_keeps_its_name() {
        // Drives `operation_marker` itself, rather than a hand-typed `reason`
        // string, so this measures the actual defect: a metadata mismatch
        // used to reach this point as `format!("{kind}-metadata-mismatch")`,
        // invisible to `TRANSPORT_VOCABULARY`, and collapsed to
        // `transport-refused` below.
        use crate::transport::claim::{OperationKind, operation_marker};
        use crate::transport::ownership::Comment;

        let operation = "3333333333333333333333333333333c";
        let persisted = Comment {
            id: Some("IC_1".to_owned()),
            created_at: "2026-07-26T01:00:00Z".to_owned(),
            body: crate::transport::markers::render(
                "claim",
                &[
                    ("run-id", "claude-a"),
                    ("runtime", "claude"),
                    ("horizon", "2026-07-26T04:00:00Z"),
                    ("op-id", operation),
                ],
            )
            .expect("a claim marker"),
            viewer_did_author: true,
            includes_created_edit: false,
        };
        let expected = [("run-id", "codex-b")];
        let failure = operation_marker(&[persisted], operation, OperationKind::Claim, &expected)
            .expect_err("codex-b's expectation does not match claude-a's persisted claim");

        let refusal = translate(&answer(failure.code(), failure.envelope()), "claim")
            .expect("still a refusal");
        assert_eq!(refusal.code, "claim-metadata-mismatch");
    }

    #[test]
    fn a_reason_outside_the_transport_vocabulary_does_not_leak_a_borrowed_string() {
        let refusal = translate(
            &answer(1, json!({"ok": false, "reason": "something-new-upstream"})),
            "verify",
        )
        .expect("still a refusal");
        assert_eq!(refusal.code, "transport-refused");
    }

    #[test]
    fn a_body_that_is_not_json_still_produces_a_usable_refusal() {
        // A transport that crashed before printing must not crash the gate.
        let refusal = translate(
            &Answer {
                code: 5,
                body: None,
            },
            "publish-review",
        )
        .expect("still a refusal");
        assert_eq!(refusal.code, "transport-refused");
        assert_eq!(refusal.outcome, MutationOutcome::Unknown);
        assert!(refusal.message.contains("publish-review"));
    }

    #[test]
    fn every_reason_the_port_answers_with_is_a_code_and_not_a_catch_all() {
        let port = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/transport");
        let mut stack = vec![port];
        let mut checked = 0;
        while let Some(directory) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !path.extension().is_some_and(|kind| kind == "rs")
                    || path.file_name().is_some_and(|name| name == "tests.rs")
                {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let text = crate::source_text::code_of(&text);
                // A test module builds whatever envelope it needs, and a
                // fixture's reason is not one the product can answer with.
                let shipped = text
                    .find("#[cfg(test)]")
                    .map_or(text.as_str(), |at| &text[..at]);
                let mut rest = shipped;
                while let Some(at) = rest.find("\"reason\": \"") {
                    rest = &rest[at + "\"reason\": \"".len()..];
                    let Some(end) = rest.find('"') else { break };
                    let reason = &rest[..end];
                    if reason.is_empty()
                        || !reason
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    {
                        continue;
                    }
                    checked += 1;
                    assert_ne!(
                        stable_code(reason),
                        "transport-refused",
                        "the port answers `{reason}` in {} and it reaches a caller as the \
                         catch-all, so the one word saying what went wrong is thrown away",
                        path.display()
                    );
                }
            }
        }
        // The floor: a walk that found nothing would pass every assertion in it.
        assert!(
            checked >= 40,
            "only {checked} reason(s) were found under the port — the walk is broken"
        );
    }

    #[test]
    fn the_generated_vocabulary_excludes_the_fallback() {
        // Replaces a test that scraped `src/transport`'s doc comments for the
        // `/// - **`reason`** — ...` bullets the refusal taxonomies are written
        // in — a test reading comment text, which issue #117 refuses at every
        // level: no test may depend on the text of a Rust comment.
        //
        // `TRANSPORT_VOCABULARY` is generated from code, not written by hand:
        // `build.rs`'s `extract_constructed` walks `src/transport` for every
        // literal reason a refusal is actually constructed with.
        //
        // This used to also assert a floor on `TRANSPORT_VOCABULARY.len()`,
        // duplicating `build.rs`'s own floor over the same generated list.
        // Removed once `build.rs`'s floor rose to sit at or above it; why is
        // in `docs/honesty.md`, not repeated here.
        //
        // This does not assert that every entry round-trips through
        // `stable_code` unchanged: `stable_code` looks an entry up *inside*
        // `TRANSPORT_VOCABULARY` itself and falls back to `"transport-refused"`
        // only when the lookup misses, so for every member of the slice it is
        // being asked to search, that assertion held for any input and any
        // source change and proved nothing about the generator.
        assert!(
            !TRANSPORT_VOCABULARY.contains(&"transport-refused"),
            "the fallback string is in the generated vocabulary itself, which would make \
             `stable_code`'s catch-all indistinguishable from a real constructed reason"
        );
    }
}
