//! A shell `gh api` call that mutates a review thread, under an autonomous claim.

use super::Action;
use crate::outcome::{Established, NoCommandReason, Observed, Refusal, Resolution};
use crate::transport::window::{ReviewThreadCall, gh_api_review_thread};

/// The stable code of this refusal.
pub const CODE: &str = "review-thread-api-mutation-unverified";

pub(super) fn refusal(action: &Action, observed: &Observed) -> Option<Refusal> {
    let Action::Boundary { command, .. } = action else {
        return None;
    };
    if command != "gh api" {
        return None;
    }
    let Some(Established::Known(line)) = observed.command.as_ref().map(|seen| &seen.against) else {
        return None;
    };
    let calls = super::gh_calls(line, gh_api_review_thread);
    [ReviewThreadCall::Named, ReviewThreadCall::QueryNotOnTheLine]
        .into_iter()
        .find(|call| calls.contains(call))
        .map(refused)
}

/// The refusal for one kind of review thread call.
pub(crate) fn refused(call: ReviewThreadCall) -> Refusal {
    let (message, way_out) = match call {
        ReviewThreadCall::Named => (
            "this API call replies to or resolves a review thread, or writes a review comment, \
             and nothing here checks the receipt, the published head or the fix",
            "the typed `review_thread` tool for review replies and resolutions",
        ),
        ReviewThreadCall::QueryNotOnTheLine => (
            "this GraphQL call takes its query from somewhere the gate cannot read, so what it \
             mutates is unknown and may be a review thread",
            "the typed `review_thread` tool for review replies and resolutions, or the query \
             written on the line (`-f query='...'`) so the gate can read what it mutates",
        ),
    };
    Refusal::not_started(
        CODE,
        message,
        Resolution::no_command(NoCommandReason::OperatorKnowledge, way_out),
    )
}
