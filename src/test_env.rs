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
