use crate::harness::session;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store;

const DEFAULT_LIMIT: u64 = 20;

pub(super) fn unreadable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "needs-person-store-unreadable",
        format!("the moments a person was needed could not be read: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "read the list again; a missing list is not an empty one",
        ),
    )
}

pub(super) fn needs_person(json: bool, limit: Option<u64>) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    let rows =
        match store::open_for_read(&store::beside(&root)).map_err(|f| unreadable(&f.detail()))? {
            Some(held) => held
                .needs_person(limit)
                .map_err(|f| unreadable(&f.detail()))?,
            None => Vec::new(),
        };
    if json {
        super::print_json(&serde_json::json!({ "needs_person": rows, "limit": limit }));
        return Ok(());
    }
    if rows.is_empty() {
        say!("no run has needed a person on this machine");
        return Ok(());
    }
    for row in &rows {
        let issue = row
            .issue
            .map_or(String::new(), |issue| format!(" #{issue}"));
        let run = row.run_id.as_deref().unwrap_or("-");
        say!(
            "{} {}{issue} run {run} at {} ({})",
            row.kind,
            row.repo,
            row.at,
            row.delivery
        );
    }
    say!("A record is not a notification and never an approval; nothing here was seen by anyone.");
    Ok(())
}
