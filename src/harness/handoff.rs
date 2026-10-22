use std::path::Path;

use crate::outcome::{NoCommandReason, Refusal, RefusalClass, Resolution};
use crate::transport::store;

/// Records a handoff the transport already projected or queued.
///
/// The caller has written to the tracker by the time this runs, so a store that
/// will not take the row leaves the world changed and this record missing. That
/// is an unknown outcome, not a failure to start.
pub(crate) fn record(
    state_root: &Path,
    repo_dir: &Path,
    issue: u64,
    runtime: &str,
    from_run: &str,
    projection: &str,
    at: &str,
) -> Result<(), Refusal> {
    if issue == 0 || runtime.is_empty() || projection.is_empty() || at.is_empty() {
        return Err(row_unwritten(
            "the transport's answer named no issue, runtime, projection or time",
            issue,
            runtime,
            projection,
        ));
    }
    let opened = store::Store::open(&store::beside(state_root))
        .map_err(|failure| row_unwritten(&failure.detail(), issue, runtime, projection))?;
    opened
        .record_handoff(
            &repo_dir.display().to_string(),
            issue,
            runtime,
            Some(from_run),
            projection,
            at,
        )
        .map_err(|failure| row_unwritten(&failure.detail(), issue, runtime, projection))
}

fn row_unwritten(detail: &str, issue: u64, runtime: &str, projection: &str) -> Refusal {
    Refusal::not_started(
        "handoff-row-unwritten",
        format!(
            "the `dev:{runtime}` projection for #{issue} is {projection}, and the handoff row \
             could not be written to this machine's store: {detail}. The label is on the tracker; \
             this session will not see the handoff at `SessionStart`"
        ),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "read the issue's labels to establish what landed, then repeat the handoff",
        ),
    )
    .classed(RefusalClass::Unknown)
}
