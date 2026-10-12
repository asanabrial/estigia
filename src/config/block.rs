//! Where the configuration table lives inside the installed contract.
//!
//! The mechanics are [`crate::fence`]; this file only says which markers.

use crate::fence::Fence;

/// Opening marker of Estigia's configuration block.
pub const BLOCK_BEGIN: &str = "<!-- estigia:config:start -->";
/// Closing marker. See [`BLOCK_BEGIN`].
pub const BLOCK_END: &str = "<!-- estigia:config:end -->";

/// The configuration block, and the issue-flow block it replaces, so an
/// upgrade cannot leave two configuration tables in one `SKILL.md`.
pub const CONFIG_FENCE: Fence = Fence {
    begin: BLOCK_BEGIN,
    end: BLOCK_END,
    superseded: &[(
        "<!-- issue-flow:config:start -->",
        "<!-- issue-flow:config:end -->",
    )],
};
