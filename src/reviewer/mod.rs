//! Launching a reviewer as a child process of the binary.
//!
//! An [`Adapter`] hands one [`Invocation`] to an installed agent CLI that has
//! no tools, no session and no user settings, and returns what it said as raw
//! bytes or a coded [`Refusal`]. Nothing here is a verdict: the bytes are the
//! reviewer's answer and a refusal is never read as a pass or as a skip.
//!
//! What goes in and what comes out of a launch is here too: [`prompt`] builds
//! the bytes a reviewer is handed, [`result`] reads what it answered, and
//! [`inspect`] is the read-only look at a correction a validator is allowed.
//!
//! [`launch`] runs one of them from prompt input to parsed answer, and
//! [`launch_recorded`] hands that answer to a recorder. The lens roles, the
//! refuter and the validator are started by `launch_review_input` as a
//! detached [`job`] and recorded by `collect_review_input`. The
//! transport also reaches [`inspect`], through `inspect_candidate`.

mod adapters;
pub mod inspect;
pub mod job;
mod launch;
pub(crate) mod process_tree;
pub mod prompt;
mod relay;
pub mod result;
mod role;
mod run;
mod scratch;

#[cfg(test)]
mod tests;

pub(crate) use adapters::DEFAULT_DEADLINE;
pub use adapters::{ClaudeCode, Codex};
pub use launch::{Recording, launch, launch_recorded, settle};
pub use relay::Relay;
pub use role::Role;

use crate::config::ModelRouting;
use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// The identity of the review one invocation belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    /// The review epoch the invocation was issued in.
    pub epoch: String,
    /// The pull request the epoch was published for.
    pub pr: u64,
    /// The head commit under review.
    pub head: String,
    /// The base the head is measured against.
    pub base: String,
    /// The digest of the complete-target manifest the publication binds, the
    /// one a verdict is later recorded against.
    pub digest: String,
    /// The SHA-256 of the diff the reviewer is handed, as published. It is the
    /// bytes the prompt must carry, not the manifest [`Self::digest`] names.
    pub payload_digest: String,
}

/// One reviewer run: who is asked, about what, and with which model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The reviewer role being run.
    pub role: String,
    /// The review this run belongs to.
    pub receipt: Receipt,
    /// The prompt, passed on standard input byte for byte.
    pub prompt: Vec<u8>,
    /// The model to ask for; `None` leaves the CLI's own choice.
    pub model: Option<String>,
}

/// Something that can run an [`Invocation`] and return the reviewer's raw
/// answer, or refuse with a [`ReviewerCode`]. It never returns a partial or
/// empty answer in place of a failure.
pub trait Adapter {
    /// Runs the invocation to completion.
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal>;

    /// Whether this adapter cannot run in process and the host must relay it.
    ///
    /// `true` only for [`Relay`]: a reviewer with no CLI installed is run by the
    /// host through the review relay plugin, and this process writes the job and
    /// admits the answer rather than spawning a child.
    fn relays(&self) -> bool {
        false
    }
}

/// The stable codes a reviewer launch refuses with. None of them is a pass and
/// none is a skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewerCode {
    /// The chosen reviewer CLI could not be found: not on `PATH`, or its path no
    /// longer exists.
    CliMissing,
    /// The CLI exists and did not run, or could not be observed: a spawn error
    /// other than not-found, containment, a scratch directory, a failed read of
    /// its output, a prompt that could not be delivered, or a descendant that
    /// held its output open past the cleanup bound.
    LaunchFailed,
    /// The CLI ran and exited non-zero or by signal.
    ExitedNonzero,
    /// The CLI exited zero with an empty answer or no last message.
    NoAnswer,
    /// The CLI exited zero but its result object says it is an error.
    ErrorResult,
    /// The raw output passed the cap.
    OutputTooLarge,
    /// The CLI had not finished by the deadline.
    Deadline,
    /// The CLI refused for quota, credit, rate or authentication reasons.
    Unavailable,
    /// The reviewer answered, and not in the shape its role's contract names.
    ResultMalformed,
    /// The diff offered for a prompt is not the one the receipt published.
    PayloadMismatch,
    /// What would be handed to a reviewer is past the size cap.
    InputTooLarge,
    /// A receipt's head or parent does not name the commit it claims, or does
    /// not exist in this repository.
    InspectTransactionUnresolved,
    /// An inspection named a path the correction did not change.
    InspectOutsideCorrection,
    /// The path is one the correction changed and has no copy on the side
    /// asked for: an added file's parent, a deleted file's candidate.
    InspectAbsentSide,
    /// The checkout could not be read for an inspection.
    InspectUnreadable,
}

impl ReviewerCode {
    /// The kebab-case code a caller matches on.
    pub const fn code(self) -> &'static str {
        match self {
            Self::CliMissing => "reviewer-cli-missing",
            Self::LaunchFailed => "reviewer-launch-failed",
            Self::ExitedNonzero => "reviewer-exited-nonzero",
            Self::NoAnswer => "reviewer-no-answer",
            Self::ErrorResult => "reviewer-error-result",
            Self::OutputTooLarge => "reviewer-output-too-large",
            Self::Deadline => "reviewer-deadline",
            Self::Unavailable => "reviewer-unavailable",
            Self::ResultMalformed => "reviewer-result-malformed",
            Self::PayloadMismatch => "reviewer-payload-mismatch",
            Self::InputTooLarge => "reviewer-input-too-large",
            Self::InspectTransactionUnresolved => "reviewer-inspect-transaction-unresolved",
            Self::InspectOutsideCorrection => "reviewer-inspect-outside-correction",
            Self::InspectAbsentSide => "reviewer-inspect-absent-side",
            Self::InspectUnreadable => "reviewer-inspect-unreadable",
        }
    }

    fn resolution(self) -> (NoCommandReason, &'static str) {
        match self {
            Self::CliMissing => (
                NoCommandReason::OperatorKnowledge,
                "install the `claude` or the `codex` CLI so it resolves on PATH; this review is \
                 not skipped and is not counted until a reviewer has answered",
            ),
            Self::LaunchFailed => (
                NoCommandReason::OperatorKnowledge,
                "the error above names what to fix before the reviewer can run; this review is \
                 not skipped and is not counted until a reviewer has answered",
            ),
            Self::ExitedNonzero => (
                NoCommandReason::OperatorKnowledge,
                "read the reviewer's own message above and fix what it names; this review is not \
                 skipped and is not counted until a reviewer has answered",
            ),
            Self::NoAnswer => (
                NoCommandReason::WorldAction,
                "the reviewer finished without saying anything; run the review again, and do not \
                 count this one",
            ),
            Self::ErrorResult => (
                NoCommandReason::OperatorKnowledge,
                "the reviewer CLI reported an error result: read it above and fix what it names; \
                 this review is not skipped and is not counted until a reviewer has answered",
            ),
            Self::OutputTooLarge => (
                NoCommandReason::OperatorKnowledge,
                "the reviewer answered with more than the raw output cap; a smaller candidate is \
                 the only way to a smaller answer, and this review is not counted",
            ),
            Self::Deadline => (
                NoCommandReason::WorldAction,
                "the reviewer did not finish in time and its process tree was killed; run the \
                 review again when the agent service is responsive, and do not count this one",
            ),
            Self::Unavailable => (
                NoCommandReason::WorldAction,
                "restore the reviewer CLI's quota, credit or login, then run the review again; \
                 an unavailable reviewer is never read as a pass and never as a skip",
            ),
            Self::ResultMalformed => (
                NoCommandReason::WorldAction,
                "the reviewer did not answer in its role's contract; run the review again, and do \
                 not count this one or read any part of it",
            ),
            Self::PayloadMismatch => (
                NoCommandReason::OperatorKnowledge,
                "the diff must be the exact bytes the publication bound; rebuild it from the \
                 receipt's base and head rather than editing it",
            ),
            Self::InputTooLarge => (
                NoCommandReason::OperatorKnowledge,
                "a smaller candidate is the only way to a smaller prompt or a smaller inspection; \
                 it is never truncated, and this review is not counted",
            ),
            Self::InspectTransactionUnresolved => (
                NoCommandReason::OperatorKnowledge,
                "the receipt must name two full commit ids this checkout holds; fetch them or \
                 name the receipt this correction was published with",
            ),
            Self::InspectOutsideCorrection => (
                NoCommandReason::OperatorKnowledge,
                "an inspection reads only the paths the correction changed over its parent; name \
                 one of them",
            ),
            Self::InspectAbsentSide => (
                NoCommandReason::OperatorKnowledge,
                "read the other side of this path, or its patch; an added file has no parent copy \
                 and a deleted one has no candidate copy",
            ),
            Self::InspectUnreadable => (
                NoCommandReason::WorldAction,
                "the checkout could not be read; restore it and inspect again, and treat the \
                 candidate as uninspected rather than as having passed",
            ),
        }
    }

    pub(crate) fn refusal(self, message: impl Into<String>) -> Refusal {
        let (reason, detail) = self.resolution();
        Refusal::not_started(self.code(), message, Resolution::no_command(reason, detail))
    }
}

/// The adapter for the first installed reviewer CLI: `claude`, then `codex`.
/// Refuses with [`ReviewerCode::CliMissing`] when neither resolves on `PATH`.
///
/// The detached runner's own selection: it runs in a child with no host, so it
/// never takes the relay. The launch path is [`select_reviewer`].
pub fn select_adapter() -> Result<Box<dyn Adapter>, Refusal> {
    select_adapter_with(crate::setup::resolve_on_path, false, false)
}

/// The reviewer the launch path uses for the run `run_id`: on an OpenCode run
/// whose relay plugin is installed, the relay; otherwise the first installed CLI,
/// or the relay when neither is installed and a host's review relay (OpenCode's
/// plugin, Pi's extension) is.
///
/// A run id whose runtime cannot be read keeps the CLI-first order. Where no CLI
/// and no relay exist, delivery blocks with a coded `unavailable` that names the
/// CLI to install — never a skip and never a silent absence.
pub fn select_reviewer(run_id: &str) -> Result<Box<dyn Adapter>, Refusal> {
    let relay = crate::setup::review_relay_installed();
    let relay_first = runs_on_opencode(run_id) && crate::setup::opencode_review_relay_installed();
    match select_adapter_with(crate::setup::resolve_on_path, relay, relay_first) {
        Ok(adapter) => Ok(adapter),
        Err(_) => Err(ReviewerCode::Unavailable.refusal(
            "no `claude` or `codex` CLI is installed and no host review relay (OpenCode or Pi) is \
             installed either, so this review cannot run; install the `claude` or the `codex` CLI \
             so it resolves on PATH",
        )),
    }
}

pub(crate) fn runs_on_opencode(run_id: &str) -> bool {
    use crate::harness::session::{RunIdShape, run_id_shape};
    matches!(
        run_id_shape(run_id),
        RunIdShape::Named { runtime } | RunIdShape::Nameless { runtime } if runtime == "opencode"
    )
}

fn select_adapter_with(
    resolve: impl Fn(&str) -> Option<std::path::PathBuf>,
    relay: bool,
    relay_first: bool,
) -> Result<Box<dyn Adapter>, Refusal> {
    if relay_first {
        return Ok(Box::new(Relay::new()));
    }
    if let Some(program) = resolve("claude") {
        return Ok(Box::new(ClaudeCode::new(program)));
    }
    if let Some(program) = resolve("codex") {
        return Ok(Box::new(Codex::new(program)));
    }
    if relay {
        return Ok(Box::new(Relay::new()));
    }
    Err(ReviewerCode::CliMissing.refusal(
        "neither `claude` nor `codex` was found on an absolute PATH entry, so no reviewer can run",
    ))
}

/// The model the `reviewer` routing target names, with `inherit` and no
/// assignment both meaning "pass no model flag".
pub fn reviewer_model(routing: &ModelRouting) -> Option<String> {
    routing
        .for_target("reviewer")
        .filter(|model| *model != "inherit")
        .map(str::to_owned)
}

/// The model `role` launches on: its own routing target when it has one and
/// the `reviewer` target otherwise.
pub fn model_for_role(routing: &ModelRouting, role: Role) -> Option<String> {
    let own = match role {
        Role::Refuter => routing.for_target("refuter"),
        Role::Validator => routing.for_target("validator"),
        _ => None,
    };
    match own {
        Some("inherit") => None,
        Some(model) => Some(model.to_owned()),
        None => reviewer_model(routing),
    }
}
