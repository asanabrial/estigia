//! The reviewer adapter a host drives when no reviewer CLI is installed.
//!
//! A relay cannot run a reviewer in this process: the host runs it, OpenCode through
//! its own sub-agent tool and Pi in process through its model registry, and the
//! review relay hands the binary the exact prompt and the reviewer's raw answer. [`Relay::review`] therefore refuses, because a
//! caller that reaches it asked this process to run a reviewer it cannot run —
//! and that refusal is the coded `unavailable` naming the CLI to install.

use super::{Adapter, Invocation, ReviewerCode};
use crate::outcome::Refusal;

/// The relay: no reviewer CLI is installed, so the host runs the reviewer and
/// the binary builds the prompt and admits the answer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Relay;

impl Relay {
    /// The relay adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Adapter for Relay {
    fn review(&self, _invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        Err(ReviewerCode::Unavailable.refusal(
            "no `claude` or `codex` CLI is installed, so this process cannot run the reviewer \
             itself; the host relays it through the OpenCode review plugin or the Pi review \
             extension, or install the `claude` or the `codex` CLI so it resolves on PATH",
        ))
    }

    fn relays(&self) -> bool {
        true
    }
}
