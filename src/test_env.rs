//! One lock over the process environment, for the tests that need to set one.
//!
//! A process has one environment, and `cargo` runs a crate's unit tests as one
//! multi-threaded binary. Two helpers with a `Mutex` each are therefore two
//! locks excluding nothing from one another — and that is what stood here: a
//! copy of this helper lived in `harness::roles::tests` and another in
//! `setup::tests`, each justified by a comment saying *"these are two test
//! binaries and neither can call the other's"*.
//!
//! That comment was false, and measurably so: `cargo test --lib config_home
//! -- --list` names one binary. A reviewer reproduced the consequence — five
//! failures in eighty filtered runs at `--test-threads=4`, none at
//! `--test-threads=1`, with assertions reading *"a borrowed home takes the
//! variable"* and *"an override of `relative/config` fell through to the
//! variable"*. Exactly environment cross-talk, and it bites the filtered run a
//! developer types rather than a full pass, where 970 other tests spread the
//! window out.
//!
//! The same false premise was carrying both `// SAFETY:` notes on the
//! `set_var` calls. One lock, in one place, is what makes those notes true.

use std::path::Path;
use std::sync::{Mutex, PoisonError};

/// The one lock. Private, so no caller can take it and forget to give it back.
static ENVIRONMENT: Mutex<()> = Mutex::new(());

/// Run `body` with `XDG_CONFIG_HOME` set, and put back what was there.
///
/// An empty value stands for *absent*, which is what `absolute_or_none` already
/// makes of it, so a caller wanting "unset" passes an empty path rather than
/// reaching for a second helper.
pub(crate) fn with_config_home<T>(value: &Path, body: impl FnOnce() -> T) -> T {
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);

    let before = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: `ENVIRONMENT` is the only lock over this process's environment and
    // is held for the whole body, so this is the only thread reading or writing
    // the variable for the duration, and the previous value is restored before
    // the guard is released.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", value);
    }
    let answer = body();
    unsafe {
        match before {
            Some(previous) => std::env::set_var("XDG_CONFIG_HOME", previous),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    drop(guard);
    answer
}

/// Run `body` with `directory` ahead of everything already on `PATH`, and put
/// both `PATH` and `ESTIGIA_FAKE_ANSWERS` back afterwards.
///
/// The transport's own doc comment names the mechanism this reuses: "a test
/// that replaces `gh` and `git` on `PATH` replaces the whole world this code
/// can see." `directory` is expected to hold a real executable named `gh` —
/// `tests/pipe.rs` builds one from the `fake_process` example for exactly this
/// reason, and a script will not do: on Windows, resolving a bare `gh` finds
/// `gh.exe` and never a `gh.cmd`.
///
/// `ESTIGIA_FAKE_ANSWERS` is the second half — `fake_process` reads it to
/// decide what to answer — and it is set and restored under the same lock as
/// `PATH`, for the reason the one lock in this module exists at all: two
/// tests racing to spawn `gh` would each see the other's script.
pub(crate) fn with_scripted_gh<T>(directory: &Path, answers: &str, body: impl FnOnce() -> T) -> T {
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);

    let path_before = std::env::var_os("PATH");
    let mut joined = std::ffi::OsString::from(directory);
    if let Some(existing) = &path_before {
        joined.push(if cfg!(windows) { ";" } else { ":" });
        joined.push(existing);
    }
    let answers_before = std::env::var_os("ESTIGIA_FAKE_ANSWERS");
    // SAFETY: `ENVIRONMENT` is the only lock over this process's environment,
    // held for the whole body, so this is the only thread reading or writing
    // either variable for the duration — and both are restored before the
    // guard is released, the same discipline `with_config_home` documents.
    unsafe {
        std::env::set_var("PATH", &joined);
        std::env::set_var("ESTIGIA_FAKE_ANSWERS", answers);
    }
    let answer = body();
    unsafe {
        match path_before {
            Some(previous) => std::env::set_var("PATH", previous),
            None => std::env::remove_var("PATH"),
        }
        match answers_before {
            Some(previous) => std::env::set_var("ESTIGIA_FAKE_ANSWERS", previous),
            None => std::env::remove_var("ESTIGIA_FAKE_ANSWERS"),
        }
    }
    drop(guard);
    answer
}

/// A directory holding a real `gh` that answers from `ESTIGIA_FAKE_ANSWERS`.
///
/// Built from the `fake_process` example the same way `tests/pipe.rs`'s own
/// `tracker_rig` finds it: this test binary sits under `deps/`, one level
/// below the profile directory the example's own `examples/` sits beside. A
/// real executable, not a script — `examples/fake_process.rs`'s own module
/// doc says why a script answers nothing on Windows, where resolving a bare
/// `gh` finds `gh.exe` and never a `gh.cmd`.
///
/// Shared rather than written once per test module: `harness::guard::tests`
/// and `harness::doctor`'s own module both need one `gh` to answer a
/// `verify-claim` differently per issue, and a second copy of "where does the
/// example live" is exactly the copy this repository keeps finding.
pub(crate) fn scripted_gh() -> tempfile::TempDir {
    let here = std::env::current_exe().expect("this test binary knows where it is");
    let built = here
        .parent()
        .and_then(Path::parent)
        .expect("the test binary is under a profile directory")
        .join("examples")
        .join(if cfg!(windows) {
            "fake_process.exe"
        } else {
            "fake_process"
        });
    assert!(
        built.is_file(),
        "the process fixture is not built, so this test would measure nothing: run `cargo \
         build --examples` ({})",
        built.display()
    );
    let bin = tempfile::tempdir().expect("a directory for the fake gh");
    std::fs::copy(
        &built,
        bin.path().join(if cfg!(windows) { "gh.exe" } else { "gh" }),
    )
    .expect("the fake gh is copied onto the path");
    bin
}

/// A scripted answer matching `gh issue view <issue> --json …`, so two
/// holders naming different issues can be answered differently by one `gh`.
pub(crate) fn issue_view_answer(issue: u64, stdout: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "matches": format!("view {issue} --json"),
        "stdout": stdout.to_string(),
        "status": 0,
    })
}

/// What `gh issue view` answers for an issue the run's own claim never
/// touched: closed, no comments, nothing an own-delivery close could find —
/// which is exactly what `verify_claim` reads as `issue-not-open`.
pub(crate) fn closed_issue(issue: u64) -> serde_json::Value {
    issue_view_answer(
        issue,
        serde_json::json!({ "state": "CLOSED", "labels": [], "comments": [] }),
    )
}

/// What `gh issue view` answers for an issue that is open and does not carry
/// the label `verify_claim` expects — read as `unexpected-state`, a real
/// answer and not the one reconciliation drops a holder on.
pub(crate) fn open_but_unmatched_issue(issue: u64) -> serde_json::Value {
    issue_view_answer(
        issue,
        serde_json::json!({ "state": "OPEN", "labels": [], "comments": [] }),
    )
}

/// What `gh issue view` answers for an issue that is open, whose label
/// matches the default expected state (`status:in-progress`) so the check
/// ahead of ownership passes, and whose comments carry no acquisition for
/// anybody at all — `verify_claim`'s `holding` reduces that to "current live
/// holder is none", the `not-current-live-holder` reason a pointer projecting
/// a claim nobody holds answers with.
pub(crate) fn not_current_live_holder_issue(issue: u64) -> serde_json::Value {
    issue_view_answer(
        issue,
        serde_json::json!({
            "state": "OPEN",
            "labels": [{ "name": "status:in-progress" }],
            "comments": [],
        }),
    )
}

/// What `gh issue view` answers for an issue this exact run genuinely,
/// currently holds — the shape `verify_claim` answers `ok` to, and the one
/// that lets `gate` reach `Decision::Allow`. The same marker shape
/// `tests/pipe.rs`'s own `issue_answer` uses, so a horizon `2099-01-01T00:00Z`
/// ahead of any clock this suite runs on.
pub(crate) fn live_holder_issue(run_id: &str, issue: u64, state: &str) -> serde_json::Value {
    let marker = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude horizon=2099-01-01T00:00Z \
         op-id={} -->",
        "a".repeat(32)
    );
    issue_view_answer(
        issue,
        serde_json::json!({
            "state": "OPEN",
            "labels": [{ "name": format!("status:{state}") }],
            "comments": [{
                "id": "IC_1",
                "createdAt": "2026-01-01T00:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": format!("Claimed by {run_id}.\n\n{marker}\n"),
            }],
        }),
    )
}

/// A `gh` that cannot answer at all, for whatever `issue view` it is asked.
pub(crate) fn unreachable_tracker_answer() -> serde_json::Value {
    serde_json::json!({
        "matches": "issue view",
        "stderr": "gh: could not resolve to a repository",
        "status": 1,
    })
}

/// `ESTIGIA_FAKE_ANSWERS`, from the entries above.
pub(crate) fn answers(entries: &[serde_json::Value]) -> String {
    serde_json::Value::Array(entries.to_vec()).to_string()
}
