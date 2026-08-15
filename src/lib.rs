// `unwrap_used` and `expect_used` are denied for the crate proper: a tool people
// run unattended must not answer a bad configuration with a backtrace. Tests are
// the documented exception — an assertion that panics is the assertion working,
// and threading `?` through them buys nothing and costs the reader.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
// A broken doc link is a documented claim about this crate that is not true of
// it, which is the one kind of wrong this crate spends everything else avoiding.
// Nine had accumulated, one of them naming a type no longer here at all. These
// only bind rustdoc, so `cargo doc` is where they are answered.
#![deny(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]

//! Estigia — the river of the oath.
//!
//! Workflow authority for coding agents over an issue tracker: timeline-adjudicated
//! claims, workflow states, exact review binding, and delivery evidence.
//!
//! What it is not, said once so it can be said no with: it is not memory, it is
//! not planning, and it does not manage your agent beyond registering itself in
//! it. A feature that does not fit that sentence does not go in.
//!
//! The binary owns configuration, installation and the tracker transport. The
//! markdown under `skill/` is what the agent *reads*, and it is not compiled —
//! it is embedded verbatim by [`crate::skill`] and written out unchanged.

pub mod cli;
pub mod config;
pub mod fence;
pub mod harness;
pub mod lifecycle;
pub mod outcome;
pub mod paths;
pub mod setup;
pub mod skill;
/// One lock over the process environment, for the tests that set one.
#[cfg(test)]
pub(crate) mod test_env;
pub mod transport;
pub mod tui;

/// The one spelling that goes in identifiers.
///
/// The `estig·IA` reading is a gift the mythology already carried, but it stays
/// in the logo and the README heading, where typographic weight does the work.
/// It never enters a crate name, a binary name, a block marker, or a path: a
/// hyphen there manufactures a compound that does not exist and buys a second
/// name for one program.
pub const NAME: &str = "estigia";
