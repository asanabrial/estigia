// An assertion that panics in a test is the assertion working.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
// A broken doc link is a documented claim that is not true of this crate.
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

pub(crate) mod bounded;
pub mod build_info;
pub mod cli;
pub mod config;
pub mod fence;
pub mod harness;
/// The identity of one issue, as its tracker spells it: a GitHub number or a
/// Linear key. See [`identity::Identity`].
pub mod identity;
pub mod lifecycle;
/// Extracts the inline-code corpus out of Markdown. See
/// [`markdown::markdown_inline_code_spans`].
pub mod markdown;
pub mod outcome;
pub mod paths;
pub mod reviewer;
pub mod setup;
pub mod skill;
/// Strips comments out of Rust source before a test searches it, so a test
/// never recognises or skips a comment by hand. See
/// [`source_text::code_of`].
pub mod source_text;
/// Shared test support: the one lock over the process environment, and the
/// fixtures beside it that need no lock at all.
#[cfg(test)]
pub(crate) mod test_env;
pub mod transport;
pub mod tui;

/// The one spelling that goes in identifiers: never the `estig·IA` form, which
/// stays in the logo and README heading only.
pub const NAME: &str = "estigia";
