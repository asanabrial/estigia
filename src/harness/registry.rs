use std::path::Path;

use super::session;
use crate::transport::store::{self, SessionSeen};

pub(crate) fn record_start(
    state_root: &Path,
    runtime: &str,
    session_id: &str,
    repo_dir: &Path,
    wake: Option<&str>,
) {
    let (Some(now), Ok(opened)) = (
        session::now_seconds(),
        store::Store::open(&store::beside(state_root)),
    ) else {
        return;
    };
    let _ = opened.session_seen(
        &SessionSeen {
            run_id: &session::run_id(runtime, session_id),
            runtime,
            session_id,
            repo_dir: repo_dir.to_str(),
            wake,
        },
        now,
    );
}

/// Records how a host that is already running can be woken, without touching
/// the rest of the row: the Stop waiter learns it is parking, not starting.
pub(crate) fn record_wake(state_root: &Path, run_id: &str, wake: &str) {
    let path = store::beside(state_root);
    let Ok(Some(_)) = store::open_for_read(&path) else {
        return;
    };
    if let Ok(opened) = store::Store::open(&path) {
        let _ = opened.session_wake(run_id, wake);
    }
}

pub(crate) fn record_end(state_root: &Path, runtime: &str, session_id: &str, reason: Option<&str>) {
    let path = store::beside(state_root);
    let (Some(now), Ok(Some(_))) = (session::now_seconds(), store::open_for_read(&path)) else {
        return;
    };
    if let Ok(opened) = store::Store::open(&path) {
        let _ = opened.session_ended(&session::run_id(runtime, session_id), reason, now);
    }
}
