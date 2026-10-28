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
    let run_id = session::run_id(runtime, session_id);
    let seen = opened.session_seen(
        &SessionSeen {
            run_id: &run_id,
            runtime,
            session_id,
            repo_dir: repo_dir.to_str(),
            wake,
            pane: ambient_pane().as_deref(),
        },
        now,
    );
    if seen.is_ok() {
        let pid = super::liveness::agent_pid();
        let host = pid.and_then(|_| super::liveness::host_name());
        let _ = opened.session_process(&run_id, pid.filter(|_| host.is_some()), host.as_deref());
    }
}

/// The host pane the process registering the session was started in.
///
/// `HERDR_PANE_ID` is the identifier herdr exports to every process in a pane;
/// it was read off a live `opencode` process inside herdr 0.9.3 and 0.9.5. A
/// process started outside a pane carries none, and the row then says nothing
/// rather than naming a pane nobody can reach.
pub(crate) fn ambient_pane() -> Option<String> {
    std::env::var("HERDR_PANE_ID")
        .ok()
        .map(|pane| pane.trim().to_owned())
        .filter(|pane| !pane.is_empty())
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_registered_session_records_the_agent_process_and_this_machine() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        record_start(directory.path(), "claude", "sess-1", directory.path(), None);

        let rows = store::Store::open(&store::beside(directory.path()))
            .expect("the store opens")
            .sessions()
            .expect("the sessions read");

        assert_eq!(rows.len(), 1);
        let pid = super::super::liveness::agent_pid();
        assert_eq!(rows[0].host_pid, pid);
        assert_eq!(
            rows[0].host_name,
            pid.and_then(|_| super::super::liveness::host_name())
        );
    }
}
