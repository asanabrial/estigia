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
//! [`translate`] asks the transport which it was rather than sending everybody
//! to read the same table. `3` the control surface answered **nothing**. `4` and
//! `5` something may have been written and nobody knows.
//!
//! Translating those into one boolean would throw away the distinction that
//! incident I07 was filed about — a run that lost a claim race by five seconds,
//! was told 33 seconds later, and worked another 48 minutes because nothing
//! read the timeline again. So each one becomes a different [`Refusal`], with a
//! different [`MutationOutcome`] and a different [`Replayability`].

use serde_json::Value;

use crate::outcome::{MutationOutcome, NoCommandReason, Refusal, Replayability, Resolution};

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

    /// The `detail` field, when present.
    pub fn detail(&self) -> Option<&str> {
        self.body.as_ref()?.get("detail")?.as_str()
    }

    /// The `action` field: the transport's own instruction to the agent.
    pub fn action(&self) -> Option<&str> {
        self.body.as_ref()?.get("action")?.as_str()
    }

    /// Whether the refusal arrived **after** this call had already written.
    ///
    /// The outcome below is otherwise derived from the exit code alone, and a
    /// stop is code `1` whether it refused before touching anything or after
    /// pushing a branch and opening a pull request. `publish_review` did the
    /// second and reported *nothing was written*: a run believing it left an
    /// orphan branch and an orphan PR, and the next call failed for a reason
    /// unrelated to the real one.
    ///
    /// The taxonomy already had the word — `MutationOutcome::Committed` says
    /// *the write landed; what failed came after it* — and no way for the
    /// transport to reach it. This is that channel. Absent means what it always
    /// meant, so every refusal that does not set it is unchanged.
    pub fn already_wrote(&self) -> bool {
        self.body
            .as_ref()
            .and_then(|body| body.get("world"))
            .and_then(Value::as_str)
            == Some("committed")
    }
}

/// What a malformed argument should have looked like.
///
/// The two reasons the transport exits `2` for that are the caller's own typo
/// rather than the machine's state — see `Failure::code`, which routes exactly
/// these two away from `1` so a run does not read a mistyped horizon as
/// "somebody took your claim". Naming the shape is what discharges them, and it
/// is a shape this build already enforces rather than a guess about one.
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
        // A flag that arrived and carried nothing. It fell to the fallback
        // below and sent the caller to `estigia config list`, which lists
        // settings and has nothing to say about an argument the caller sent
        // blank — a dead end, which is the one thing the ratchet forbids.
        "blank-argument" => Some(
            "a value for the flag the detail names \u{2014} it arrived with nothing in it, and \
             the configuration has no part in that",
        ),
        // Every other reason on this code is the configuration, which has a
        // command that fixes it.
        _ => None,
    }
}

/// What is missing when the GitHub CLI has no login, in one sentence.
///
/// Held here because two places answer for it and they must not differ:
/// `doctor`'s `gh` row, which an operator reads when they ask why nothing
/// works, and the refusal a run is handed when a tracker read fails for that
/// reason. It is a `no_command` sentence rather than an invocation on purpose —
/// `gh auth login` opens a browser, so naming it as *the* command to run would
/// promise something a headless agent cannot do.
pub const UNAUTHENTICATED: &str =
    "an authenticated GitHub CLI \u{2014} run `gh auth login`, which needs a person at a browser";

/// Turns a transport answer into a refusal, keeping every distinction it drew.
///
/// The mapping is the contract, so it is written once here rather than at each
/// call site. `translate` never invents a resolution: when the transport has
/// named an `action`, that text is what the agent is told, because the transport
/// is the thing that knows which of the four retry shapes applies.
///
/// guard:population exit-code fail-closed: every exit code the transport can
/// return. Legitimate population: `0` through `5`, which is the whole contract
/// its `main()` implements. Boundary: a code outside that range is not a code
/// this mapping has read, and it lands in the `Unknown` arm rather than the
/// stop arm — because reporting "nothing happened" for a code nobody has seen
/// is the lie the taxonomy exists to prevent. The proof boundary: this shows
/// an unknown code fails towards Unknown, not that the six named ones are
/// mapped to the right outcomes.
///
/// `1` is read twice, and the population is unchanged by that: it is still one
/// code, decided further by whether the transport declared it had already
/// written. A stop that pushed a branch before refusing is the same decision
/// about a different world, and reporting it as `nothing was written` was the
/// same lie by a different route.
pub fn translate(answer: &Answer, context: &str) -> Option<Refusal> {
    if answer.code == 0 {
        return None;
    }
    let reason = answer.reason().unwrap_or("transport-refused");
    let detail = answer
        .detail()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{context}: the transport exited {}", answer.code));

    // See the `exit-code` population declaration on this function.
    let (outcome, replay, resolution) = match answer.code {
        // The control surface was READ, and it answered stop. Authority changed
        // under this run: retrying the identical command repeats the answer.
        // A stop that already wrote. Same decision, different world: the
        // refusal is still the tracker's answer, but reporting it as if nothing
        // had happened is the one thing this taxonomy exists to refuse.
        1 if answer.already_wrote() => (
            MutationOutcome::Committed,
            // Not `ExactReplaySafe`. `publication-readback-disagrees` reaches
            // here saying *do not bind review or CI to anything yet: re-read the
            // pull request*, and repeating the identical call mints a fresh
            // epoch over a head somebody else pushed. Read first, then decide.
            Replayability::StatusRequired,
            match answer.action() {
                Some(action) => Resolution::no_command(NoCommandReason::WorldAction, action),
                None => Resolution::no_command(
                    NoCommandReason::WorldAction,
                    "this call wrote before it refused; re-read the repository before deciding",
                ),
            },
        ),
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
        // has a command that fixes it.
        //
        // Both used to be told `estigia config list`. Running it discharges the
        // configuration case and does nothing whatever for a mistyped horizon,
        // which is the ratchet's one rule about naming a command: a message may
        // name one only when running it discharges the block. Measured on the
        // real binary — `claim 1 --horizon "not a date"` answered
        // `invalid-horizon` and then sent the operator to read a table that has
        // no horizon in it.
        //
        // The transport already says which of the two it was, so it is asked.
        2 => (
            MutationOutcome::NotStarted,
            Replayability::ExactReplaySafe,
            match answer.reason().and_then(malformed_argument) {
                Some(wanted) => Resolution::no_command(NoCommandReason::OperatorKnowledge, wanted),
                None => Resolution::run("estigia config list"),
            },
        ),
        // Nothing was answered. Not a stand-down and never clearance.
        //
        // Two failures share this code and only one of them is worth retrying.
        // A network that blinked is; a `gh` nobody has logged in is not, and it
        // is the one every fresh machine has. Measured on the real binary and
        // through the tool server alike: both answered *the same call may be
        // repeated* and *write nothing and retry the read*, over a read that
        // cannot succeed until a person opens a browser. An agent told that
        // retries until something else stops it.
        //
        // `gh` says which it is, in the detail already being carried — *please
        // run: gh auth login*. The answer is `doctor`'s, shared rather than
        // written again: one machine, one sentence about what is missing.
        3 if detail.contains("gh auth login") => (
            MutationOutcome::NotStarted,
            Replayability::ManualActionRequired,
            Resolution::no_command(NoCommandReason::HumanAuthority, UNAUTHENTICATED),
        ),
        3 => (
            MutationOutcome::NotStarted,
            Replayability::ExactReplaySafe,
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "the tracker could not be read; write nothing and retry the read",
            ),
        ),
        // The state is unknown. `4` is a defect in the transport and `5` is a
        // write whose fate nobody observed; both end with a caller that must go
        // and look rather than assume.
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

    Some(Refusal {
        code: stable_code(reason),
        message: detail,
        outcome,
        replay,
        resolution,
    })
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

    /// A stop that already wrote does not report that nothing was written.
    ///
    /// The outcome was derived from the exit code alone, and a stop is `1`
    /// whether it refused before touching anything or after pushing a branch and
    /// opening a pull request. `publish_review` did the second and said *nothing
    /// was written*: the run that believed it left both orphaned, and the next
    /// call failed for a reason unrelated to the real one — the operator
    /// debugging it starting from a false premise this tool supplied.
    ///
    /// `MutationOutcome::Committed` already existed and the transport had no way
    /// to reach it. A refusal that does not claim to have written is unchanged,
    /// which is the other half of this and the reason the default stays.
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
        let untouched = translate(
            &answer(1, json!({ "ok": false, "reason": "lost-claim-race" })),
            "claim",
        )
        .expect("a refusal");
        assert_eq!(untouched.outcome, MutationOutcome::NotStarted);
        assert_eq!(untouched.outcome.what_happened(), "nothing was written");
    }

    /// A `gh` nobody has logged in is not a read worth retrying.
    ///
    /// Measured on the real binary and through the tool server: both answered
    /// *the same call may be repeated* and *write nothing and retry the read*,
    /// over a read that cannot succeed until a person opens a browser. An agent
    /// told that retries until something else stops it — the dead end the
    /// ratchet exists to forbid, reached through the advice rather than through
    /// a named command.
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
        // guard:population exit-code — the fail-closed arm. A code outside the contract
        // must not be reported as "nothing happened".
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

    /// Every reason the **port** can answer with survives `stable_code`.
    ///
    /// The vocabulary is generated at build time and was read off
    /// `skill/scripts/github.py` alone. There are two transports now, and the
    /// port answers with six reasons the original never had — it does its own
    /// argument handling where the Python had `argparse` — so
    /// `missing-argument`, `unknown-operation`, `malformed-argument` and three
    /// more all collapsed to `transport-refused`.
    ///
    /// That is the defect `build.rs`'s own header describes: *"a code that is
    /// not in its list collapses to `transport-refused` — which is the exact
    /// defect the whole outcome taxonomy exists to prevent"*. It had been fixed
    /// for the transport and reopened by the port.
    ///
    /// Measured through the server: `start_branch` refused with
    /// `{"reason":"missing-argument","argument":"--repo-name"}` in the envelope
    /// and `(transport-refused)` on the screen — which is how a dead tool went
    /// unnoticed, because the one word that said what was wrong never arrived.
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

    /// A refusal the transport's own prose declares is one the harness can name.
    ///
    /// The other guard in this file reads the code, by the same rule `build.rs`
    /// reads it by, so the two can be wrong together — and were. `cmd_reclaim`
    /// writes its refusal as `stop(if before.stale.is_empty() {
    /// "nothing-to-reclaim" } else { "stale-self-requires-claim" }, …)`, and a
    /// generator that expected a literal against the bracket found neither.
    /// Both reached a run as `transport-refused` while `holder-not-stale` — a
    /// plain literal three lines away — came through intact. Two of the five
    /// refusals whose documentation opens by saying *the distinctions between
    /// them are the point*, collapsed into the one word that has no point.
    ///
    /// This reads the **prose** instead: the bulleted list where the author
    /// declares the taxonomy, three lines above the code that answers it. It is
    /// a different source, written by a person for a person, and it cannot
    /// share a parsing mistake with the generator.
    #[test]
    fn a_refusal_the_transport_documents_is_one_the_harness_can_tell_apart() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("transport");
        let mut declared: Vec<String> = Vec::new();
        let mut stack = vec![root];
        while let Some(at) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&at) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|kind| kind != "rs") {
                    continue;
                }
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for line in source.lines() {
                    // `/// - **`some-reason`** — what it means`, which is how
                    // every refusal taxonomy in this crate is written down.
                    let Some(rest) = line.trim_start().strip_prefix("/// - **`") else {
                        continue;
                    };
                    let Some(end) = rest.find("`**") else {
                        continue;
                    };
                    let word = &rest[..end];
                    if word.contains('-')
                        && word
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    {
                        declared.push(word.to_owned());
                    }
                }
            }
        }
        declared.sort();
        declared.dedup();

        // The floor: a reader that stopped matching agrees with an empty list
        // and never fails, which is the shape of guard this crate refuses.
        assert!(
            declared.len() >= 8,
            "only {} documented refusals were found, so this compared almost nothing: {declared:?}",
            declared.len()
        );

        let collapsed: Vec<&String> = declared
            .iter()
            .filter(|reason| stable_code(reason) != reason.as_str())
            .collect();
        assert!(
            collapsed.is_empty(),
            "the transport documents these refusals as telling a caller different things, and the \
             harness answers `transport-refused` for every one of them: {collapsed:?}"
        );
    }
}
