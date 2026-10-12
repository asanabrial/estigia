//! What this build of the binary is, so two builds of the same crate version
//! can be told apart. A build without git reports `None` / `false`, the
//! *cannot tell* the stale-gate row must swallow rather than pass.

/// The package version and the commit it was built from, as a person reads it.
/// Always set: `build.rs` embeds it so `env!` is safe here.
pub const VERSION: &str = env!("ESTIGIA_VERSION");

/// The full git commit this build was compiled at, or `None` for a published
/// crate or a build without git.
pub fn commit() -> Option<&'static str> {
    option_env!("ESTIGIA_COMMIT").filter(|value| !value.is_empty())
}

/// Whether the working tree carried uncommitted changes when this was built.
/// `false` without claiming cleanliness when git could not be read.
pub fn dirty() -> bool {
    option_env!("ESTIGIA_DIRTY") == Some("1")
}
