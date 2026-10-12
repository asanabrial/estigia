//! The canonical per-runtime capability table.
//!
//! Every adapter declares what its runtime carries, and this table is the
//! canonical answer that declaration is checked against. The check runs at
//! compile time over [`AGENTS`](super::AGENTS) — a row whose declaration
//! disagrees with the table fails the build — and at construction through
//! `AgentAdapter::checked`, which a caller reaching the table by name uses.

use super::{AgentAdapter, InstructionFile, SkillsRoot};

/// How a runtime can refuse a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// A hook the runtime reads out of its own settings file.
    Settings,
    /// A file Estigia owns whole — OpenCode's plugin, Pi's extension, Cline's hook script.
    OwnedFile,
    /// No gate mechanism Estigia has verified for this runtime.
    None,
}

/// What one runtime carries, as the canonical table states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Where the runtime discovers skills, or [`SkillsRoot::Neutral`] when the
    /// skill lands in the shared root and the directive names the path.
    pub skills: SkillsRoot,
    /// The runtime's tool-call gate, if it has one.
    pub gate: Gate,
    /// Whether Estigia registers its MCP server for this runtime.
    pub tool_server: bool,
    /// Whether the always-loaded directive is written for this runtime.
    pub contract: bool,
    /// Whether the runtime has an agent-definitions root.
    pub agent_definitions: bool,
    /// Whether the runtime reads a loop plugin Estigia writes.
    pub loop_plugin: bool,
    /// Whether the `Loop plugin` setting means anything for this runtime.
    pub offers_loop: bool,
    /// Whether the runtime has a review relay, the transport that carries a reviewer's result
    /// in by hand. Wording about one reaches only a runtime that has it.
    pub review_relay: bool,
}

impl Capabilities {
    /// What a runtime absent from the table carries: nothing at all.
    pub const NOTHING: Self = Self {
        skills: SkillsRoot::Neutral,
        gate: Gate::None,
        tool_server: false,
        contract: false,
        agent_definitions: false,
        loop_plugin: false,
        offers_loop: false,
        review_relay: false,
    };
}

/// The canonical table: what each runtime carries.
///
/// Total over [`InstructionFile`] by construction — the `match` has no `_` arm,
/// so a runtime added to the enum fails to compile until its row is written
/// here rather than falling through to a default nobody chose.
pub(super) const fn canonical(instructions: InstructionFile) -> Capabilities {
    match instructions {
        InstructionFile::Neutral => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::None,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::ClaudeCode => Capabilities {
            skills: SkillsRoot::ClaudeCode,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: true,
            loop_plugin: false,
            offers_loop: true,
            review_relay: false,
        },
        InstructionFile::Codex => Capabilities {
            skills: SkillsRoot::Codex,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: true,
            review_relay: false,
        },
        InstructionFile::OpenCode => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::OwnedFile,
            tool_server: true,
            contract: true,
            agent_definitions: true,
            loop_plugin: true,
            offers_loop: true,
            review_relay: true,
        },
        InstructionFile::Pi => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::OwnedFile,
            tool_server: true,
            contract: true,
            agent_definitions: true,
            loop_plugin: true,
            offers_loop: true,
            review_relay: true,
        },
        InstructionFile::GeminiCli => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::Cursor => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::Qwen => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::Continue => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::Cline => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::OwnedFile,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::Crush => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::Windsurf => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::ZCode => Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::None,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
        InstructionFile::DeepSeekHarness => Capabilities {
            skills: SkillsRoot::DshHarness,
            gate: Gate::None,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    }
}

/// The canonical capabilities of the runtime an adapter slug names, or
/// [`Capabilities::NOTHING`] when no adapter answers to it.
///
/// The lookup is fail-closed: a runtime nobody has decided about carries
/// nothing, rather than inheriting a default.
pub fn capabilities_for_slug(slug: &str) -> Capabilities {
    super::every_adapter()
        .find(|adapter| adapter.slug == slug)
        .map(|adapter| adapter.capabilities)
        .unwrap_or(Capabilities::NOTHING)
}

const fn same_skills(left: SkillsRoot, right: SkillsRoot) -> bool {
    matches!(
        (left, right),
        (SkillsRoot::Neutral, SkillsRoot::Neutral)
            | (SkillsRoot::ClaudeCode, SkillsRoot::ClaudeCode)
            | (SkillsRoot::Codex, SkillsRoot::Codex)
            | (SkillsRoot::DshHarness, SkillsRoot::DshHarness)
    )
}

const fn same_gate(left: Gate, right: Gate) -> bool {
    matches!(
        (left, right),
        (Gate::Settings, Gate::Settings)
            | (Gate::OwnedFile, Gate::OwnedFile)
            | (Gate::None, Gate::None)
    )
}

const fn same_capabilities(left: Capabilities, right: Capabilities) -> bool {
    same_skills(left.skills, right.skills)
        && same_gate(left.gate, right.gate)
        && left.tool_server == right.tool_server
        && left.contract == right.contract
        && left.agent_definitions == right.agent_definitions
        && left.loop_plugin == right.loop_plugin
        && left.offers_loop == right.offers_loop
        && left.review_relay == right.review_relay
}

const fn gate_has_spec(gate: Gate, has_spec: bool) -> bool {
    matches!(gate, Gate::Settings) == has_spec
}

/// Whether an adapter's declaration agrees with the table and with the files
/// it resolves.
///
/// `const` so the check runs over the whole registry at compile time: a
/// declaration that disagrees is a build failure, not a run-time surprise.
const fn capabilities_agree(adapter: &AgentAdapter) -> bool {
    let table = canonical(adapter.instructions);
    same_capabilities(adapter.capabilities, table)
        && gate_has_spec(adapter.capabilities.gate, adapter.gate_spec().is_some())
        && adapter.capabilities.agent_definitions == adapter.agent_definitions_root().is_some()
}

const _: () = {
    let mut index = 0;
    while index < super::AGENTS.len() {
        assert!(
            capabilities_agree(&super::AGENTS[index]),
            "an adapter's declaration disagrees with capabilities::canonical"
        );
        index += 1;
    }
    // Retired adapters keep their declarations checked too: the gate still reads
    // their control surfaces, so a declaration that drifted would gate the wrong
    // paths while the suite stayed green.
    let mut index = 0;
    while index < super::RETIRED_AGENTS.len() {
        assert!(
            capabilities_agree(&super::RETIRED_AGENTS[index]),
            "a retired adapter's declaration disagrees with capabilities::canonical"
        );
        index += 1;
    }
};

/// A declaration that disagrees with the table, named by both sides.
#[cfg(test)]
#[derive(Debug)]
pub(super) struct CapabilityMismatch {
    /// The runtime the adapter names.
    pub(super) instructions: InstructionFile,
    /// What the adapter declared.
    pub(super) declared: Capabilities,
    /// What the table says the runtime carries.
    pub(super) canonical: Capabilities,
}

#[cfg(test)]
impl std::fmt::Display for CapabilityMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "adapter declares {:?} for {:?}, the table says {:?}",
            self.declared, self.instructions, self.canonical
        )
    }
}

#[cfg(test)]
impl AgentAdapter {
    /// Builds an adapter only when its declaration agrees with the table.
    pub(super) fn checked(
        slug: &'static str,
        display_name: &'static str,
        instructions: InstructionFile,
        capabilities: Capabilities,
    ) -> Result<Self, CapabilityMismatch> {
        let adapter = Self {
            slug,
            display_name,
            instructions,
            capabilities,
        };
        if capabilities_agree(&adapter) {
            Ok(adapter)
        } else {
            Err(CapabilityMismatch {
                instructions,
                declared: capabilities,
                canonical: canonical(instructions),
            })
        }
    }
}
