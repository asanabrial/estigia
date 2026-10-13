//! Registering Estigia in somebody else's agent, and taking it back out.
//!
//! Four invariants:
//!
//! 1. [`is_configured`] is a query, separate from [`setup`].
//! 2. [`uninstall`] is the **exact inverse**: it takes out what Estigia put in
//!    and nothing else — see `crate::skill::record` for why that is not the
//!    same set as what Estigia created.
//! 3. Everything Estigia writes into a shared file lives between markers; the
//!    user's file is never rewritten.
//! 4. `--dry-run` reports [`Change`] per file and a count, checked against the
//!    real run by a test.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::config::{CONFIG_FENCE, Config, Evidence, LoopPlugin, ModelRouting, Route, Seal};
use crate::fence::Fence;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::paths;
use crate::skill;

mod capabilities;
mod codex_trust;
mod companion;
pub(crate) mod plugin;
mod render;
pub mod wiring;
pub use capabilities::{Capabilities, Gate, capabilities_for_slug};
pub use codex_trust::Skipped;
pub use render::NotEditable;

pub(crate) use companion::resolve_on_path;
pub use companion::{COMPANIONS, Companion, CompanionState, find_companion, probe_companion};
pub use render::SERVER_NAME;
use render::{
    Envelope, McpFormat, is_estigia_hook, render_cursor_hooks, render_hooks, render_mcp,
    runs_estigia_hook, strip_cursor_hooks, strip_hooks, strip_mcp,
};

/// What the agent is told, once, in its always-loaded instruction file.
///
/// Deliberately short, and it names the skill rather than restating it: the
/// protocol lives in one place, and everything else is in the skill, where the
/// tokens are paid for when they are needed.
pub const DIRECTIVE_TEMPLATE: &str = r#"## Estigia — workflow authority

Issue work in this environment runs under Estigia. Three rules:

1. **A claim is adjudicated, not asserted.** Never infer that you hold an issue
   from a label, an assignee, prose, or a worktree you can see. The gate
   re-verifies the claim on the writes it observes and at every expensive or
   irreversible boundary; an unreadable control surface permits no write. Run
   the binding's `verify_claim` when you want the answer yourself.
2. **A verdict is bound to exact bytes.** A review and a green CI run count only
   against the same head and base they were produced on. Every push invalidates
   both — re-publish and ask again rather than carrying a verdict forward.
3. **An unknown result is not clearance.** When a write's outcome cannot be
   read back, say so and stop. Never report the nearest named state instead.

The `{skill}` skill at `{path}` has the rest: the states, the transitions, the
tracker binding, and the delivery topology.

Acquiring a context this work needs — a review panel, a reader, a worker — is
decided by the operator table and not by asking again. Where a row authorises
it, launch it and report what it returned. This grants nothing else: what a
launched context may write is still the launching run's claim.

Written by `estigia setup`. Anything **inside** these two markers is replaced whole on the next run
and taken away by the uninstall, so an addition made here is lost without being reported — the same
rule the operator table states about its own block. Your own instructions go outside them, where
they are kept."#;

/// The markers that fence the directive inside an agent's instruction file.
pub const DIRECTIVE_BEGIN: &str =
    "<!-- BEGIN ESTIGIA WORKFLOW AUTHORITY - managed by estigia setup -->";
/// Closing marker. See [`DIRECTIVE_BEGIN`].
pub const DIRECTIVE_END: &str = "<!-- END ESTIGIA WORKFLOW AUTHORITY -->";

/// The directive block inside an agent's always-loaded instruction file.
///
/// Nothing is superseded: Estigia has only ever written this pair, and the day
/// that stops being true is the day an upgrade would otherwise leave two
/// directives in one file.
pub const DIRECTIVE_FENCE: Fence = Fence {
    begin: DIRECTIVE_BEGIN,
    end: DIRECTIVE_END,
    superseded: &[],
};

/// The operating system, as far as path resolution is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Windows, where roaming application data lives under `%APPDATA%`.
    Windows,
    /// macOS, where it lives under `~/Library/Application Support`.
    MacOs,
    /// Everything else, which follows the XDG base directory specification.
    Unix,
}

impl Platform {
    /// The platform this binary was built for.
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }
}

/// Where an agent looks for skills.
///
/// Only roots read out of that agent's own shipping source are named here, and
/// not a page describing the path — a page is a lower bar. An agent whose skill
/// directory is a guess gets [`Self::Neutral`], and the directive names the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillsRoot {
    /// `~/.agents/skills` — the agent-neutral convention.
    Neutral,
    /// `~/.claude/skills`, or `skills` under `$CLAUDE_CONFIG_DIR` when that is set.
    ClaudeCode,
    /// `~/.codex/skills`.
    Codex,
    /// `~/.dsh/skills` — DeepSeek Harness's own user skill root.
    ///
    /// Verified from the harness's own `skill-filesystem` provider. It resolves
    /// the root under `$DSH_HOME`, falling back to `~/.dsh`; Estigia reads that
    /// variable and contains it the way `XDG_CONFIG_HOME` is contained.
    DshHarness,
}

/// Which always-loaded instruction file carries the directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstructionFile {
    Neutral,
    ClaudeCode,
    Codex,
    OpenCode,
    Pi,
    GeminiCli,
    Cursor,
    Qwen,
    Windsurf,
    Crush,
    Cline,
    Continue,
    ZCode,
    DeepSeekHarness,
}

/// One agent Estigia knows how to register itself in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentAdapter {
    /// The name typed on the command line.
    pub slug: &'static str,
    /// The name shown to a person.
    pub display_name: &'static str,
    /// The runtime's own configuration surface, and the key into
    /// [`capabilities::canonical`].
    instructions: InstructionFile,
    /// What this adapter declares its runtime carries. Equal to the canonical
    /// table, checked at compile time for [`AGENTS`] and refused at
    /// construction by `AgentAdapter::checked`.
    capabilities: Capabilities,
}

const CLAUDE_MODELS: &[&str] = &["fable", "opus", "sonnet", "haiku"];
const CODEX_MODELS: &[&str] = &["gpt-6-astra", "gpt-6.1-sol", "gpt-6-luna", "gpt-5.5"];

/// A reviewed model-routing preset for one adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelProfile {
    /// Stable name shown in the configuration UI.
    pub name: &'static str,
    assignments: &'static [(&'static str, &'static str)],
}

impl ModelProfile {
    /// Expands this preset into the same canonical route custom editing uses.
    pub fn routing(self) -> Option<ModelRouting> {
        let mut routing = ModelRouting::default();
        self.assignments
            .iter()
            .all(|(target, model)| routing.assign(target, model))
            .then_some(routing)
    }
}

const CLAUDE_BALANCED: &[(&str, &str)] = &[
    ("orchestrate", "opus"),
    ("apply", "sonnet"),
    ("developer", "sonnet"),
    ("analyst", "opus"),
    ("reviewer", "sonnet"),
    ("judge", "opus"),
];
const CLAUDE_PERFORMANCE: &[(&str, &str)] = &[
    ("orchestrate", "opus"),
    ("apply", "opus"),
    ("developer", "opus"),
    ("analyst", "opus"),
    ("reviewer", "opus"),
    ("judge", "opus"),
];
const CLAUDE_ECONOMY: &[(&str, &str)] = &[
    ("orchestrate", "sonnet"),
    ("apply", "sonnet"),
    ("developer", "sonnet"),
    ("analyst", "sonnet"),
    ("reviewer", "haiku"),
    ("judge", "sonnet"),
];
const CLAUDE_PROFILES: &[ModelProfile] = &[
    ModelProfile {
        name: "balanced",
        assignments: CLAUDE_BALANCED,
    },
    ModelProfile {
        name: "performance",
        assignments: CLAUDE_PERFORMANCE,
    },
    ModelProfile {
        name: "economy",
        assignments: CLAUDE_ECONOMY,
    },
];

const CODEX_BALANCED: &[(&str, &str)] = &[
    ("orchestrate", "gpt-6-astra"),
    ("apply", "gpt-6.1-sol"),
    ("developer", "gpt-6.1-sol"),
    ("analyst", "gpt-6-astra"),
    ("reviewer", "gpt-6.1-sol"),
    ("judge", "gpt-6-astra"),
];
const CODEX_PERFORMANCE: &[(&str, &str)] = &[
    ("orchestrate", "gpt-6-astra"),
    ("apply", "gpt-6-astra"),
    ("developer", "gpt-6-astra"),
    ("analyst", "gpt-6-astra"),
    ("reviewer", "gpt-6-astra"),
    ("judge", "gpt-6-astra"),
];
const CODEX_ECONOMY: &[(&str, &str)] = &[
    ("orchestrate", "gpt-6.1-sol"),
    ("apply", "gpt-6.1-sol"),
    ("developer", "gpt-6.1-sol"),
    ("analyst", "gpt-6.1-sol"),
    ("reviewer", "gpt-6-luna"),
    ("judge", "gpt-6.1-sol"),
];
const CODEX_PROFILES: &[ModelProfile] = &[
    ModelProfile {
        name: "balanced",
        assignments: CODEX_BALANCED,
    },
    ModelProfile {
        name: "performance",
        assignments: CODEX_PERFORMANCE,
    },
    ModelProfile {
        name: "economy",
        assignments: CODEX_ECONOMY,
    },
];

/// Where one adapter's advisory model suggestions come from.
///
/// This is discovery metadata, never validation. Estigia does not run models,
/// and a listed ID is not proof that the host will accept or use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCatalogSource {
    /// A reviewed list maintained with this adapter.
    Curated(&'static [&'static str]),
    /// The configured providers reported by the host's `opencode models` CLI.
    OpenCode,
    /// No verified catalog exists; custom IDs remain available.
    None,
}

/// How many adapters take the agent-neutral skills root.
///
/// Derived, because documents and tests carried the number by hand and it
/// disagreed with the table; a hand-spelled count agrees with the table only
/// until somebody adds a row.
pub fn adapters_on_the_neutral_root() -> usize {
    AGENTS
        .iter()
        .filter(|adapter| matches!(adapter.capabilities.skills, SkillsRoot::Neutral))
        .count()
}

/// The supported agent adapters: the ones `setup` installs into.
///
/// Each entry's `capabilities` is that adapter's own declaration; the canonical
/// table is `capabilities::canonical`, and a declaration that disagrees with it
/// fails the build.
pub const AGENTS: &[AgentAdapter] = &[
    // The neutral root first: it is the one that works when nothing else is
    // installed.
    AgentAdapter {
        slug: "agents",
        display_name: "Agent-neutral (~/.agents)",
        instructions: InstructionFile::Neutral,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::None,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
    AgentAdapter {
        slug: "claude-code",
        display_name: "Claude Code",
        instructions: InstructionFile::ClaudeCode,
        capabilities: Capabilities {
            skills: SkillsRoot::ClaudeCode,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: true,
            loop_plugin: false,
            offers_loop: true,
            review_relay: false,
        },
    },
    AgentAdapter {
        slug: "codex",
        display_name: "Codex",
        instructions: InstructionFile::Codex,
        capabilities: Capabilities {
            skills: SkillsRoot::Codex,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: true,
            review_relay: false,
        },
    },
    // Most of what follows has a verified instruction file and an unverified
    // skill directory, so the skill lands in the neutral root and the directive
    // names the path. `deepseek-harness` at the end of the table is the
    // exception and says why on its own entry.
    AgentAdapter {
        slug: "opencode",
        display_name: "OpenCode",
        instructions: InstructionFile::OpenCode,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::OwnedFile,
            tool_server: true,
            contract: true,
            agent_definitions: true,
            loop_plugin: true,
            offers_loop: true,
            review_relay: true,
        },
    },
    AgentAdapter {
        slug: "pi",
        display_name: "Pi",
        instructions: InstructionFile::Pi,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::OwnedFile,
            tool_server: true,
            contract: true,
            agent_definitions: true,
            loop_plugin: true,
            offers_loop: true,
            review_relay: true,
        },
    },
    AgentAdapter {
        slug: "cursor",
        display_name: "Cursor",
        instructions: InstructionFile::Cursor,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
    // Verified from ZCode's own shipped configuration reference: the user
    // instruction file is `~/.zcode/AGENTS.md`, loaded into every session. Its
    // skills roots are read on a page rather than out of its own shipping
    // source, so this adapter sits in the tier below.
    AgentAdapter {
        slug: "zcode",
        display_name: "ZCode",
        instructions: InstructionFile::ZCode,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::None,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
    // Verified from DeepSeek Harness's own `skill-filesystem` and
    // `agent-instructions` providers: the skill lands in `~/.dsh/skills` and the
    // directive in `~/.dsh/AGENTS.md`, both under `$DSH_HOME` when it is set and
    // contained, and both read from the harness's own shipping source, so this
    // adapter discovers its own skills.
    AgentAdapter {
        slug: "deepseek-harness",
        display_name: "DeepSeek Harness",
        instructions: InstructionFile::DeepSeekHarness,
        capabilities: Capabilities {
            skills: SkillsRoot::DshHarness,
            gate: Gate::None,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
];

/// Agent adapters Estigia no longer installs into, but must still take itself
/// out of.
///
/// Each entry keeps the paths and strip functions it had as a supported adapter,
/// so `uninstall` reaches an install an earlier build wrote — its directive
/// block, its MCP entry and its gate hook. `setup` refuses a slug named here
/// with `agent-retired`, and the gate keeps reading a retired host's instruction
/// file and hook dialect, because a machine that installed one still has them.
pub const RETIRED_AGENTS: &[AgentAdapter] = &[
    AgentAdapter {
        slug: "gemini-cli",
        display_name: "Gemini CLI",
        instructions: InstructionFile::GeminiCli,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
    AgentAdapter {
        slug: "continue",
        display_name: "Continue",
        instructions: InstructionFile::Continue,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
    AgentAdapter {
        slug: "cline",
        display_name: "Cline",
        instructions: InstructionFile::Cline,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::OwnedFile,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
    AgentAdapter {
        slug: "crush",
        display_name: "Crush",
        instructions: InstructionFile::Crush,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
    AgentAdapter {
        slug: "windsurf",
        display_name: "Windsurf",
        instructions: InstructionFile::Windsurf,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: false,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
    AgentAdapter {
        slug: "qwen",
        display_name: "Qwen",
        instructions: InstructionFile::Qwen,
        capabilities: Capabilities {
            skills: SkillsRoot::Neutral,
            gate: Gate::Settings,
            tool_server: true,
            contract: true,
            agent_definitions: false,
            loop_plugin: false,
            offers_loop: false,
            review_relay: false,
        },
    },
];

/// Every adapter, supported or retired.
pub fn every_adapter() -> impl Iterator<Item = &'static AgentAdapter> {
    AGENTS.iter().chain(RETIRED_AGENTS.iter())
}

impl AgentAdapter {
    /// Whether this adapter is retired: `uninstall` reaches it, `setup` refuses it.
    pub fn retired(&self) -> bool {
        RETIRED_AGENTS
            .iter()
            .any(|retired| retired.slug == self.slug)
    }
}

/// Whether this adapter installs the skill where the agent finds it on its own.
impl AgentAdapter {
    /// The tail of this adapter's instruction file, as the gate spells paths.
    ///
    /// The gate cannot call `resolve_paths`: it answers one hook invocation
    /// against a string, with no environment to resolve a home directory from,
    /// so it matches a fragment of a path the installer decides. This lives
    /// beside the `match` that resolves the full path, and
    /// `every_control_file_an_adapter_has_is_one_the_gate_measures` crosses them.
    ///
    /// Lowercase with forward slashes, because `is_control_surface` folds both.
    pub fn instruction_fragment(&self) -> &'static str {
        match self.instructions {
            // Without the extension: `contains` never reaches a `.local.`
            // sibling, and `~/.claude/CLAUDE.local.md` and `~/.codex/AGENTS.local.md`
            // are read with the same authority as the files beside them.
            InstructionFile::Neutral => ".agents/agents",
            InstructionFile::ClaudeCode => ".claude/claude",
            InstructionFile::Codex => ".codex/agents",
            InstructionFile::OpenCode => "opencode/agents.md",
            InstructionFile::Pi => ".pi/agent/agents",
            // `%APPDATA%/gemini/` on Windows and `~/.gemini/` elsewhere; the
            // last two components are the same on both.
            InstructionFile::GeminiCli => "gemini/gemini.md",
            InstructionFile::Cursor => ".cursor/estigia-workflow-authority.md",
            InstructionFile::Qwen => ".qwen/qwen.md",
            InstructionFile::Crush => "crush/crush.md",
            InstructionFile::Continue => ".continue/rules/estigia.md",
            InstructionFile::Cline => ".cline/rules/estigia.md",
            InstructionFile::Windsurf => "windsurf/memories/global_rules.md",
            // `~/.zcode/AGENTS.md` and `~/.dsh/AGENTS.md`, trimmed of the
            // extension for the reason at the head of this match.
            InstructionFile::ZCode => ".zcode/agents",
            InstructionFile::DeepSeekHarness => ".dsh/agents",
        }
    }

    /// The skills tree this adapter's host discovers, as the gate spells paths.
    ///
    /// The tree around the contract is a control surface because it holds more
    /// than Estigia: a sibling skill directory is read with the same authority
    /// as the contract. Derived from the same declared skills root that decides
    /// where the installer writes, so the two cannot disagree without failing
    /// the suite. A second hand-written `SkillsRoot` mapping beside `paths_in`'s
    /// rather than a truncation, so a new adapter reusing a `SkillsRoot`
    /// inherits its answer without a compiler question.
    pub fn skills_root_fragment(&self) -> &'static str {
        match self.capabilities.skills {
            SkillsRoot::Neutral => ".agents/skills/",
            SkillsRoot::ClaudeCode => ".claude/skills/",
            SkillsRoot::Codex => ".codex/skills/",
            SkillsRoot::DshHarness => ".dsh/skills/",
        }
    }

    /// Reviewed presets this adapter can expand without probing the host.
    pub fn model_profiles(&self) -> &'static [ModelProfile] {
        match self.instructions {
            InstructionFile::ClaudeCode => CLAUDE_PROFILES,
            InstructionFile::Codex => CODEX_PROFILES,
            _ => &[],
        }
    }

    /// The provenance of this host's advisory model suggestions.
    ///
    /// Derived from the adapter's private host identity rather than from a TUI
    /// slug map, so adding or renaming an adapter cannot silently borrow another
    /// host's namespace.
    pub fn model_catalog(&self) -> ModelCatalogSource {
        match self.instructions {
            InstructionFile::ClaudeCode => ModelCatalogSource::Curated(CLAUDE_MODELS),
            InstructionFile::Codex => ModelCatalogSource::Curated(CODEX_MODELS),
            InstructionFile::OpenCode => ModelCatalogSource::OpenCode,
            _ => ModelCatalogSource::None,
        }
    }

    /// The hosts whose curated catalog lists `model` when this adapter's own does not.
    ///
    /// Advisory only: a model ID stays opaque, so nothing refuses on this answer.
    pub fn foreign_model_hosts(&self, model: &str) -> Vec<&'static str> {
        if let ModelCatalogSource::Curated(own) = self.model_catalog()
            && own.contains(&model)
        {
            return Vec::new();
        }
        let mut hosts: Vec<&'static str> = Vec::new();
        for other in AGENTS.iter().filter(|other| other.slug != self.slug) {
            if let ModelCatalogSource::Curated(listed) = other.model_catalog()
                && listed.contains(&model)
                && !hosts.contains(&other.display_name)
            {
                hosts.push(other.display_name);
            }
        }
        hosts
    }

    /// Every distinct model a routing names that belongs to another host, with those hosts.
    pub fn foreign_models_in(&self, routing: &ModelRouting) -> Vec<(String, Vec<&'static str>)> {
        let mut found: Vec<(String, Vec<&'static str>)> = Vec::new();
        for model in routing.by_role.values().filter_map(Route::model_value) {
            if found.iter().any(|(seen, _)| seen == model) {
                continue;
            }
            let hosts = self.foreign_model_hosts(model);
            if !hosts.is_empty() {
                found.push((model.to_owned(), hosts));
            }
        }
        found
    }

    /// Whether the agent discovers the skill without being told where it is.
    ///
    /// `false` means the skill lands in the neutral root and the directive
    /// names the path. Callers show this, because "installed" and "installed
    /// where it will be found" are different answers.
    pub fn discovers_skills(&self) -> bool {
        !matches!(self.capabilities.skills, SkillsRoot::Neutral)
    }

    /// The skills root this adapter declares, as the canonical table states it.
    pub fn skills(&self) -> SkillsRoot {
        self.capabilities.skills
    }

    /// What this adapter declares its runtime carries.
    ///
    /// Equal to the canonical table for the runtime, checked at compile time
    /// for every entry of [`AGENTS`] and refused at construction by
    /// `AgentAdapter::checked`.
    pub fn capabilities(&self) -> Capabilities {
        self.capabilities
    }
}

/// Where a `setup` install writes.
///
/// Distinct from [`crate::config::Scope`], which is about which *settings* an
/// answer belongs to. This is about where an install's files land: the machine
/// once, or one workspace each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstallScope {
    /// The machine-wide roots under the resolved home. The default.
    #[default]
    Global,
    /// The project-level roots under one workspace, so the install belongs to a
    /// checkout rather than to the machine.
    Workspace,
}

/// Everything a caller may point somewhere else, for tests and for operators
/// whose profile is not where the platform thinks it is.
#[derive(Debug, Clone, Default)]
pub struct SetupOptions {
    /// Report what would change without writing anything.
    pub dry_run: bool,
    /// Skip the instruction-file directive and install only the skill.
    pub skip_directive: bool,
    /// Skip the lifecycle hooks that make the workflow authority mechanical.
    ///
    /// Installed by default; this is the way out for somebody who wants the
    /// skill without a process on the critical path of every edit.
    pub skip_harness: bool,
    /// Override the detected platform.
    pub platform: Option<Platform>,
    /// Override the home directory.
    pub home_dir: Option<PathBuf>,
    /// Override `$XDG_CONFIG_HOME`.
    pub config_home: Option<PathBuf>,
    /// Override `%APPDATA%`.
    pub app_data: Option<PathBuf>,
    /// Override `$CLAUDE_CONFIG_DIR`.
    pub claude_config_dir: Option<PathBuf>,
    /// Override `$PI_CODING_AGENT_DIR`.
    pub pi_agent_dir: Option<PathBuf>,
    /// Where this install writes: the machine's home, or one workspace.
    pub scope: InstallScope,
    /// The workspace a [`InstallScope::Workspace`] install is rooted at.
    ///
    /// Required by that scope and ignored by [`InstallScope::Global`]: a caller
    /// that names the workspace scope and no workspace is refused rather than
    /// silently writing to the machine's home.
    pub workspace: Option<PathBuf>,
}

/// The files one adapter owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPaths {
    /// The directory the skill tree is written into.
    pub skill_root: PathBuf,
    /// The always-loaded instruction file the directive is fenced inside.
    pub instructions: PathBuf,
    /// The settings file holding lifecycle hooks, when the agent has one.
    pub hooks: Option<PathBuf>,
    /// The MCP configuration file, when the agent reads one.
    pub mcp_config: Option<PathBuf>,
    /// The plugin file that gates tool calls, when the agent loads one.
    pub plugin: Option<PathBuf>,
    /// The optional loop plugin for OpenCode and Pi, whether or not it is installed.
    ///
    /// Named for those two always, so `uninstall` can find a file an earlier
    /// install wrote; whether it is **written** is the `Loop plugin` setting's
    /// answer, not this path being `Some`.
    pub loop_plugin: Option<PathBuf>,
    /// The directory this agent reads custom slash commands from, when it reads
    /// one. `None` for every adapter but Claude Code.
    ///
    /// Named whether or not the `Loop plugin` setting is on, so uninstall can
    /// find the commands an earlier install wrote; the setting decides whether
    /// they are written.
    pub commands: Option<PathBuf>,
    /// The directory Pi reads prompt templates from. `None` for every adapter
    /// but Pi.
    ///
    /// Named for Pi always, so uninstall can find the templates an earlier
    /// install wrote.
    pub prompts: Option<PathBuf>,
    /// Where this agent's sub-agent definitions live, when the gate can read
    /// them back.
    ///
    /// `None` is the interesting value: every destination here is one of the
    /// roots [`crate::harness::roles::definition_for`] searches, and adding a
    /// fourth means teaching the gate to read it in the same change.
    pub agents_root: Option<PathBuf>,
    /// One line per inherited root this resolution noted — because it pointed
    /// outside the resolved home, or because it was not an absolute
    /// path. `APPDATA`, `XDG_CONFIG_HOME` and `DSH_HOME` are noted
    /// for the first reason alone — a relative value among them is dropped
    /// before the note pass — and `CLAUDE_CONFIG_DIR` is exempt from the
    /// containment check and is noted for the second alone. An empty vector here
    /// does not say the machine was clean; it says nothing was noted *for this
    /// adapter*. See `docs/honesty.md`'s entry on it. A caller that hits
    /// one is expected to report it, not to fail on it.
    pub notes: Vec<String>,
}

/// Whether Estigia can install lifecycle hooks for this agent.
///
/// Claude Code only — not for lack of hooks elsewhere, but because its hook's
/// shape is the one verified against a published reference. An adapter built on
/// third-party write-ups would report success and may enforce nothing, and it
/// would ship with the documented subagent hole beside it.
impl AgentAdapter {
    /// Whether `setup` can install a hooks-file gate for this agent.
    pub fn supports_hooks(&self) -> bool {
        matches!(self.capabilities.gate, Gate::Settings)
    }

    /// Whether `setup` can gate this agent's tool calls at all, by any
    /// mechanism — a hooks file, or a plugin or script Estigia owns whole.
    pub fn can_gate_tools(&self) -> bool {
        !matches!(self.capabilities.gate, Gate::None)
    }

    /// Whether this adapter reads a loop plugin Estigia can write.
    pub fn has_loop_plugin(&self) -> bool {
        self.capabilities.loop_plugin
    }

    /// Whether the `Loop plugin` setting means anything for this adapter:
    /// OpenCode and Pi read a plugin or extension Estigia writes, Codex runs the
    /// `Stop` and `SessionStart` entries Estigia registers in its hooks file.
    pub fn offers_loop(&self) -> bool {
        self.capabilities.offers_loop
    }
}

impl AgentAdapter {
    /// The directory, joined onto this adapter's own configuration base — see
    /// `agent_definitions_base`, which is Claude Code's root, OpenCode's
    /// `$XDG_CONFIG_HOME/opencode` or Pi's agent directory — where it reads a sub-agent definition the
    /// host routes to on its own. One of the roots
    /// [`crate::harness::roles::definition_for`] searches, and no others, and
    /// `None` when it has none at all.
    ///
    /// Matched on the **instruction enum** rather than `adapter.slug`, so an
    /// added variant fails to compile until somebody writes `None` on purpose —
    /// a twelfth adapter once got no root silently with the suite green. Claude
    /// Code keeps them beside its skills, under its own configuration root
    /// (`environment.claude_config()`, `$CLAUDE_CONFIG_DIR` when set and
    /// `~/.claude` otherwise) rather than `$HOME` directly. OpenCode keeps its
    /// own under `$XDG_CONFIG_HOME/opencode`, which is why the base directory is
    /// per adapter (`agent_definitions_base`) and not the Claude one. Reusing
    /// Claude Code's renderer here is refused, and OpenCode has one of its
    /// own for the two delegated workers,
    /// while the reserved reviewers stay Claude-only.
    ///
    /// `const` so the canonical table's `agent_definitions` column is crossed
    /// against this `match` at compile time, not merely declared beside it.
    pub const fn agent_definitions_root(&self) -> Option<&'static str> {
        match self.instructions {
            InstructionFile::ClaudeCode | InstructionFile::OpenCode | InstructionFile::Pi => {
                Some("agents")
            }
            InstructionFile::Neutral
            | InstructionFile::Codex
            | InstructionFile::GeminiCli
            | InstructionFile::Cursor
            | InstructionFile::Qwen
            | InstructionFile::Crush
            | InstructionFile::Continue
            | InstructionFile::Cline
            | InstructionFile::Windsurf
            | InstructionFile::ZCode
            | InstructionFile::DeepSeekHarness => None,
        }
    }

    /// Whether an installed agent definition can reach this adapter's host at
    /// all.
    pub fn has_agent_definitions(&self) -> bool {
        self.capabilities.agent_definitions
    }
}

/// What a setting is worth for one adapter.
///
/// Most rows are enforced by the gate, and an agent Estigia cannot gate gets
/// the row written into its contract and nothing holding it. Saying so is the
/// whole point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
    /// Estigia enforces it for this agent.
    Held,
    /// Written into the contract, and the agent is asked to honour it — but
    /// there is no gate here, so nothing checks that it did.
    Asked(&'static str),
}

impl Applies {
    /// A word for the row, or nothing when it is simply held.
    pub fn tag(self) -> Option<&'static str> {
        match self {
            Self::Held => None,
            Self::Asked(_) => Some("not held"),
        }
    }

    /// Why, in one line.
    pub fn because(self) -> Option<&'static str> {
        match self {
            Self::Held => None,
            Self::Asked(why) => Some(why),
        }
    }
}

impl AgentAdapter {
    /// What this setting is worth for this adapter.
    ///
    /// One property decides it: [`Self::can_gate_tools`], for the rows the gate
    /// enforces. A row that decided nothing here used to follow from
    /// [`Self::has_agent_definitions`], and that row is gone —
    /// `Delegated workers` moved its effect into `Model routing`, which also
    /// declares models and so reaches an adapter with no definitions root.
    pub fn applies(&self, setting: crate::config::Setting) -> Applies {
        use crate::config::Setting;
        if self.can_gate_tools() {
            return Applies::Held;
        }
        match setting {
            // These three name who authorises something. The gate is what stops
            // and asks; without one the contract still says it, and the agent
            // may still honour it, but nothing checks.
            Setting::Delivery | Setting::Transitions | Setting::Boundaries => Applies::Asked(
                "Estigia does not gate this agent's tool calls — the contract asks, and the \
                     pre-push guard still holds the push",
            ),
            _ => Applies::Held,
        }
    }
}

/// Which of an adapter's obligations an action concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// A file of the skill tree.
    Skill,
    /// The fenced directive in the agent's instruction file.
    Directive,
    /// The lifecycle hooks that make the workflow authority mechanical.
    Hooks,
    /// The MCP server registration, through which the agent calls operations.
    McpServer,
    /// The plugin that gates tool calls, for an agent that reads one.
    Plugin,
    /// The optional loop plugin, written while the `Loop plugin` setting is on.
    LoopPlugin,
    /// The OpenCode review relay plugin, written on every OpenCode install.
    ReviewPlugin,
    /// One of the loop's slash commands, for a runtime that reads them.
    Command,
    /// One of the prompt templates written for Pi on every install.
    PromptTemplate,
    /// The generic goal and loop plugin, written while the `Goal loop plugin` setting is on.
    GoalLoopPlugin,
    /// A retired SDD phase definition, taken back out where this install wrote it.
    PhaseAgent,
    /// One delegated worker definition, installed because the row named it.
    DelegatedAgent,
    /// A reserved reviewer definition Claude Code routes to, as a template —
    /// one per reserved reviewer name.
    AgentDefinition,
    /// One adapter's configuration inside a shared skill root.
    AgentConfiguration,
    /// The rows stored with one repository.
    RepositoryConfiguration,
}

/// What one file needed.
pub use crate::skill::Change;

/// One file, and what happened or would happen to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupAction {
    /// Which obligation this file belongs to.
    pub kind: ActionKind,
    /// The absolute path.
    pub path: PathBuf,
    /// What it needed.
    pub change: Change,
}

/// The outcome of setting up or uninstalling one agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetupResult {
    /// The adapter slug.
    pub agent: &'static str,
    /// Whether anything was actually written.
    pub dry_run: bool,
    /// One entry per file the adapter owns.
    pub actions: Vec<SetupAction>,
    /// Whether the adapter's complete lifecycle reached the successful return.
    #[serde(skip)]
    pub completed: bool,
    /// `AgentPaths::notes`, carried onto the result so the report an operator
    /// reads is the one that names an environment substitution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// A setup that stopped together with every action it had already proved.
#[derive(Debug)]
pub struct SetupFailure {
    /// The underlying renderer, reader, or writer failure.
    pub error: anyhow::Error,
    /// Actions proved before the failure; the adapter is not lifecycle-complete.
    pub result: SetupResult,
    /// A write was sent but its result was not proved.
    pub write_attempted: bool,
    /// Which setup phase produced the failure.
    pub phase: SetupFailurePhase,
}

/// Whether setup was only reading/planning or had entered mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupFailurePhase {
    /// Validation performed internally before a real invocation mutates.
    Prevalidation,
    /// A caller-requested dry-run preview.
    Preview,
    /// The real mutation pass.
    Mutation,
}

impl SetupFailure {
    /// Builds evidence for a failure reached before setup could mutate.
    pub(crate) fn before_mutation(
        agent: &'static str,
        dry_run: bool,
        error: anyhow::Error,
    ) -> Self {
        Self {
            error,
            result: SetupResult {
                agent,
                dry_run,
                actions: Vec::new(),
                completed: false,
                notes: Vec::new(),
            },
            write_attempted: false,
            phase: if dry_run {
                SetupFailurePhase::Preview
            } else {
                SetupFailurePhase::Prevalidation
            },
        }
    }
}

impl std::fmt::Display for SetupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for SetupFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SetupFailureBoundary {
    BeforeSkill,
    AfterSkill,
    AfterDirective,
    AfterDefinitions,
    AfterHooks,
    AtMcp,
}

/// Whether the internal dry-run preview of a real setup is skipped (debug
/// builds only).
///
/// Two roads, both compiled out of `--release`:
///
/// - **Spawned test binaries** (`tests/pipe.rs`) set
///   `ESTIGIA_TEST_SKIP_PREVIEW=1` on every `Command`; the first read caches
///   that answer for the child's whole life.
/// - **In-process lib tests** never set the variable (a process-wide env flip
///   would race every concurrent reader). They start with the skip already on
///   (`cfg!(test)` below), and a fixture that must exercise the preview arms
///   [`with_preview`] instead, which stores a thread-local.
///
/// The preview only enriches failure evidence: on success its result is
/// discarded. Skipping it halves every real `setup()` the suite performs
/// without changing what a green run asserts.
#[cfg(debug_assertions)]
fn preview_skipped() -> bool {
    use std::sync::atomic::Ordering;
    if FORCE_PREVIEW.with(|flag| flag.get()) {
        return false;
    }
    if PREVIEW_SKIP_FOR_TESTS.load(Ordering::SeqCst) {
        return true;
    }
    static FROM_ENV: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("ESTIGIA_TEST_SKIP_PREVIEW").is_some());
    *FROM_ENV
}

#[cfg(debug_assertions)]
thread_local! {
    static FORCE_PREVIEW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(debug_assertions)]
static PREVIEW_SKIP_FOR_TESTS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(cfg!(test));

/// Runs `body` with the debug preview skip cleared on this thread, so the
/// internal dry-run preview of a real setup executes.
///
/// Test binaries start with the skip already on; without this, a fixture that
/// asserts prevalidation classification or proven planned actions would never
/// execute the branch it means to cover. Production never calls this; a release
/// build does not contain it.
#[cfg(test)]
pub(crate) fn with_preview<T>(body: impl FnOnce() -> T) -> T {
    #[cfg(debug_assertions)]
    {
        let was = FORCE_PREVIEW.with(|flag| flag.replace(true));
        let answer = body();
        FORCE_PREVIEW.with(|flag| flag.set(was));
        answer
    }
    #[cfg(not(debug_assertions))]
    {
        body()
    }
}

#[cfg(test)]
std::thread_local! {
    static SETUP_FAILURE: std::cell::Cell<Option<(&'static str, SetupFailureBoundary)>> = const { std::cell::Cell::new(None) };
    static SETUP_PREVALIDATION_FAILURE: std::cell::Cell<Option<(&'static str, SetupFailureBoundary)>> = const { std::cell::Cell::new(None) };
    static REVIEWER_DEFINITION_REMOVAL_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static REVIEWER_DEFINITION_WRITE_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static REVIEWER_DEFINITION_CREATE_COLLISION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(crate) const INJECTED_REVIEWER_COLLISION: &str = "operator-created reviewer\n";

#[cfg(test)]
pub(crate) fn inject_setup_failure(slug: &'static str, boundary: SetupFailureBoundary) {
    SETUP_FAILURE.with(|injected| injected.set(Some((slug, boundary))));
}

#[cfg(test)]
pub(crate) fn inject_setup_prevalidation_failure(
    slug: &'static str,
    boundary: SetupFailureBoundary,
) {
    SETUP_PREVALIDATION_FAILURE.with(|injected| injected.set(Some((slug, boundary))));
}

#[cfg(test)]
pub(crate) fn inject_reviewer_definition_removal_failure() {
    REVIEWER_DEFINITION_REMOVAL_FAILURE.with(|injected| injected.set(true));
}

#[cfg(test)]
pub(crate) fn inject_reviewer_definition_write_failure() {
    REVIEWER_DEFINITION_WRITE_FAILURE.with(|injected| injected.set(true));
}

#[cfg(test)]
pub(crate) fn inject_reviewer_definition_create_collision() {
    REVIEWER_DEFINITION_CREATE_COLLISION.with(|injected| injected.set(true));
}

#[cfg(test)]
fn injected_reviewer_definition_create_collision(path: &Path) -> Result<()> {
    if REVIEWER_DEFINITION_CREATE_COLLISION.with(|injected| injected.replace(false)) {
        let parent = path
            .parent()
            .with_context(|| format!("{} has no parent directory", path.display()))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("create directory {}", parent.display()))?;
        fs::write(path, INJECTED_REVIEWER_COLLISION)
            .with_context(|| format!("inject collision at {}", path.display()))?;
    }
    Ok(())
}

#[cfg(not(test))]
fn injected_reviewer_definition_create_collision(_: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
fn injected_setup_failure(adapter: &AgentAdapter, boundary: SetupFailureBoundary) -> Result<()> {
    SETUP_FAILURE.with(|injected| {
        if injected.get() == Some((adapter.slug, boundary)) {
            injected.set(None);
            anyhow::bail!("injected setup failure at {boundary:?}")
        }
        Ok(())
    })
}

#[cfg(test)]
fn injected_setup_prevalidation_failure(
    adapter: &AgentAdapter,
    boundary: SetupFailureBoundary,
) -> Result<()> {
    SETUP_PREVALIDATION_FAILURE.with(|injected| {
        if injected.get() == Some((adapter.slug, boundary)) {
            injected.set(None);
            anyhow::bail!("injected setup prevalidation failure at {boundary:?}")
        }
        Ok(())
    })
}

#[cfg(not(test))]
fn injected_setup_failure(_: &AgentAdapter, _: SetupFailureBoundary) -> Result<()> {
    Ok(())
}

#[cfg(not(test))]
fn injected_setup_prevalidation_failure(_: &AgentAdapter, _: SetupFailureBoundary) -> Result<()> {
    Ok(())
}

impl SetupResult {
    /// How many files were, or would be, touched.
    pub fn changed_files(&self) -> usize {
        self.actions
            .iter()
            // `Kept` is a file nothing happened to; counting it would make an
            // uninstall report changes it did not make.
            .filter(|action| {
                !matches!(
                    action.change,
                    Change::Unchanged | Change::Kept | Change::Shared | Change::Unrecorded
                )
            })
            .count()
    }
}

/// Which inherited machine variable a dropped root came from.
///
/// An enum rather than the `&str` tag it replaces, so the two matches over it —
/// `name` and `fallback` — are exhaustive: a new inherited variable fails to
/// compile rather than surviving unnoticed in one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InheritedVariable {
    AppData,
    XdgConfigHome,
    ClaudeConfigDir,
    PiAgentDir,
    DshHome,
}

impl InheritedVariable {
    /// The name a rendered note calls this variable by — unchanged by this
    /// being an enum internally rather than the string it replaces.
    fn name(self) -> &'static str {
        match self {
            InheritedVariable::AppData => "APPDATA",
            InheritedVariable::XdgConfigHome => "XDG_CONFIG_HOME",
            InheritedVariable::ClaudeConfigDir => "CLAUDE_CONFIG_DIR",
            InheritedVariable::PiAgentDir => "PI_CODING_AGENT_DIR",
            InheritedVariable::DshHome => "DSH_HOME",
        }
    }

    /// The home-relative fallback this run used once the inherited value was
    /// dropped — one call to the method that decides it, not a second copy of
    /// the fallback formula.
    fn fallback(self, environment: &Environment) -> PathBuf {
        match self {
            InheritedVariable::AppData => environment.roaming(),
            InheritedVariable::XdgConfigHome => environment.xdg_config(),
            InheritedVariable::ClaudeConfigDir => environment.claude_config(),
            InheritedVariable::PiAgentDir => environment.pi_agent(),
            InheritedVariable::DshHome => environment.dsh_home_dir(),
        }
    }
}

#[derive(Debug)]
struct Environment {
    platform: Platform,
    home: PathBuf,
    config_home: Option<PathBuf>,
    app_data: Option<PathBuf>,
    claude_config_dir: Option<PathBuf>,
    pi_agent_dir: Option<PathBuf>,
    dsh_home: Option<PathBuf>,
    /// One entry per inherited root this run declined to use, tagged with the
    /// variable it came from — see [`Environment::resolve`]. Kept tagged rather
    /// than flattened to `String` because `paths_in` decides which adapters a
    /// substitution actually concerns, per adapter.
    notes: Vec<(InheritedVariable, String)>,
}

impl Environment {
    fn resolve(options: &SetupOptions) -> Result<Self> {
        let platform = options.platform.unwrap_or_else(Platform::current);
        let home = match &options.home_dir {
            Some(path) => path.clone(),
            None => paths::home_dir()?,
        };
        paths::require_absolute(&home, "home directory")?;
        // A caller that moved the home meant to move everything under it, and
        // on Windows `APPDATA` is always set — so an isolated home left some
        // roots pointing at the real machine. Half a move reads exactly like a
        // whole one until something answers a question about the wrong disk.
        let borrowed = options.home_dir.is_none();
        let inherited = |name: &str| {
            borrowed
                .then(|| env::var_os(name).map(PathBuf::from))
                .flatten()
        };
        // Through `xdg_config_home`, not beside it: two implementations that
        // happened to agree is the arrangement this crate has been bitten by
        // twice. An explicit override is answered on its own and never falls
        // through to the variable — `config_home: Some(<relative>)` with the
        // variable set absolute once took the **variable**, the half-move this
        // block exists to refuse.
        let config_home_inherited = options.config_home.is_none();
        let config_home = match options.config_home.clone() {
            Some(explicit) => absolute_or_none(Some(explicit)),
            None => borrowed.then(xdg_config_home).flatten(),
        };
        let app_data_inherited = options.app_data.is_none();
        let app_data = absolute_or_none(if options.app_data.is_some() {
            options.app_data.clone()
        } else {
            inherited("APPDATA")
        });
        // A root read from the environment is dropped when it is not absolute —
        // a relative one names a path under the working directory rather than a
        // configuration root — and that drop is *noted*, the honesty the
        // out-of-home case below already owes. An explicit override is the
        // caller's own value and stays silent, as it always was.
        let mut relative = Vec::new();
        let claude_config_dir = if options.claude_config_dir.is_some() {
            absolute_or_none(options.claude_config_dir.clone())
        } else {
            absolute_or_noted(
                inherited("CLAUDE_CONFIG_DIR"),
                InheritedVariable::ClaudeConfigDir,
                &mut relative,
            )
        };
        let pi_agent_dir = if options.pi_agent_dir.is_some() {
            absolute_or_none(options.pi_agent_dir.clone())
        } else {
            absolute_or_noted(
                inherited("PI_CODING_AGENT_DIR")
                    .filter(|value| !value.as_os_str().is_empty())
                    .map(|value| expand_tilde(value, &home)),
                InheritedVariable::PiAgentDir,
                &mut relative,
            )
        };
        let dsh_home = absolute_or_none(inherited("DSH_HOME"));

        // An inherited value is only ever contained-checked against
        // a `home` this same call read from the environment — never against a
        // `home` the caller supplied deliberately. `paths::covers`, not a
        // hand-rolled `starts_with`: it case-folds on Windows, where `%APPDATA%`
        // and `%USERPROFILE%` routinely differ in case. `paths::placed` is not
        // this question — its `None` means *inside*, fail-closed for the gate,
        // which here would silently keep exactly the value this exists to drop.
        let mut escaped = Vec::new();
        let config_home = if config_home_inherited {
            contained_or_noted(
                &home,
                config_home,
                InheritedVariable::XdgConfigHome,
                &mut escaped,
            )
        } else {
            config_home
        };
        let app_data = if app_data_inherited {
            contained_or_noted(&home, app_data, InheritedVariable::AppData, &mut escaped)
        } else {
            app_data
        };
        let dsh_home =
            contained_or_noted(&home, dsh_home, InheritedVariable::DshHome, &mut escaped);

        let mut environment = Self {
            platform,
            home,
            config_home,
            app_data,
            claude_config_dir,
            pi_agent_dir,
            dsh_home,
            notes: Vec::new(),
        };
        // Built from the fallback methods rather than restating the formula
        // here.
        environment.notes = escaped
            .into_iter()
            .map(|(variable, ignored): (InheritedVariable, PathBuf)| {
                let used = variable.fallback(&environment);
                let message = format!(
                    "{} points to {} outside the resolved home; using {} instead",
                    variable.name(),
                    ignored.display(),
                    used.display()
                );
                (variable, message)
            })
            .chain(relative.into_iter().map(|(variable, ignored)| {
                let used = variable.fallback(&environment);
                let message = format!(
                    "{} is not an absolute path ({}); using {} instead",
                    variable.name(),
                    ignored.display(),
                    used.display()
                );
                (variable, message)
            }))
            .collect();
        Ok(environment)
    }

    fn xdg_config(&self) -> PathBuf {
        self.config_home
            .clone()
            .unwrap_or_else(|| self.home.join(".config"))
    }

    fn roaming(&self) -> PathBuf {
        self.app_data
            .clone()
            .unwrap_or_else(|| self.home.join("AppData").join("Roaming"))
    }

    fn claude_config(&self) -> PathBuf {
        self.claude_config_dir
            .clone()
            .unwrap_or_else(|| self.home.join(".claude"))
    }

    fn pi_agent(&self) -> PathBuf {
        self.pi_agent_dir
            .clone()
            .unwrap_or_else(|| self.home.join(".pi").join("agent"))
    }

    fn dsh_home_dir(&self) -> PathBuf {
        self.dsh_home
            .clone()
            .unwrap_or_else(|| self.home.join(".dsh"))
    }

    // Claude Code keeps `.claude.json` inside `$CLAUDE_CONFIG_DIR` when the
    // variable is set, and beside the home otherwise. The two branches differ in
    // parent — the relocated directory, or `$HOME` rather than
    // `claude_config()`'s answer — which is why this is not a `.join` onto that
    // method.
    fn claude_mcp_config(&self) -> PathBuf {
        match &self.claude_config_dir {
            Some(directory) => directory.join(".claude.json"),
            None => self.home.join(".claude.json"),
        }
    }
}

/// An inherited root, kept only when it resolves under `home`.
///
/// An isolated checkout whose `APPDATA` still pointed at the
/// operator's real profile rewrote their real configuration. The under-home
/// fallback already covers *absent*; this covers *present and pointing
/// somewhere else*. `escaped` collects what was dropped rather than discarding
/// it, so the caller can report — never refuse — the substitution.
fn contained_or_noted(
    home: &Path,
    candidate: Option<PathBuf>,
    variable: InheritedVariable,
    escaped: &mut Vec<(InheritedVariable, PathBuf)>,
) -> Option<PathBuf> {
    match candidate {
        Some(candidate) if crate::paths::covers(home, &candidate) => Some(candidate),
        Some(candidate) => {
            escaped.push((variable, candidate));
            None
        }
        None => None,
    }
}

fn absolute_or_none(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| path.is_absolute())
}

/// The same rule as [`absolute_or_none`], for a root read from the environment,
/// reporting what it dropped. A relative value names a path under the working
/// directory rather than a configuration root, so it is still treated as absent
/// — and the drop is recorded rather than left silent.
fn absolute_or_noted(
    path: Option<PathBuf>,
    variable: InheritedVariable,
    relative: &mut Vec<(InheritedVariable, PathBuf)>,
) -> Option<PathBuf> {
    match path {
        Some(path) if path.is_absolute() => Some(path),
        Some(path) => {
            relative.push((variable, path));
            None
        }
        None => None,
    }
}

/// `XDG_CONFIG_HOME`, resolved the one way this crate resolves it.
///
/// Public because the gate needs the same answer: one rule, absolute or
/// nothing, and a relative or empty value is a caller's mistake treated as
/// absent. The *installer* applies a second rule on top that
/// the gate does not — `Environment::resolve` keeps this answer only when it
/// resolves under the same call's home — and that stays safe because
/// `definition_for` pushes `~/.config/opencode/agents` **and** this answer.
/// Widen the gate to a *single* root and that stops being true.
pub fn xdg_config_home() -> Option<PathBuf> {
    absolute_or_none(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
}

/// `CLAUDE_CONFIG_DIR`, resolved the same one way [`xdg_config_home`] resolves
/// its own variable.
///
/// Public for the same reason: the gate needs this answer too.
/// `definition_for` pushes the root this names **beside** the default, never
/// instead of it, the same asymmetry as for the OpenCode root
/// beside it. `authorize_review_blind_launch` reads it differently, on
/// purpose: when this is set, it is the *only* tree the canonical reviewer is
/// proved against, because the default `~/.claude/agents` belongs to
/// whichever profile the operator is not running under.
pub fn claude_config_dir() -> Option<PathBuf> {
    absolute_or_none(std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from))
}

/// `PI_CODING_AGENT_DIR`, resolved the way Pi reads it: a leading `~` is the
/// home, and anything not absolute after that is treated as absent.
///
/// Public for the gate, which needs the same answer to recognise a relocated
/// agent directory as control surface. Unlike the roots `Environment::resolve`
/// contains under the home, this one is followed wherever it points, because
/// a launcher that relocates Pi puts the directory outside the home on purpose.
pub fn pi_agent_dir() -> Option<PathBuf> {
    let value = std::env::var_os("PI_CODING_AGENT_DIR").filter(|value| !value.is_empty())?;
    absolute_or_none(Some(expand_tilde(
        PathBuf::from(value),
        &paths::home_dir().ok()?,
    )))
}

fn expand_tilde(path: PathBuf, home: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => home.join(rest),
        Err(_) => path,
    }
}

/// The adapter a slug names, or a refusal listing the ones that exist.
///
/// Searches retired adapters too, so `uninstall` and `doctor` reach an install
/// an earlier build wrote. An install path wants [`find_installable_agent`].
pub fn find_agent(slug: &str) -> Result<&'static AgentAdapter, Refusal> {
    every_adapter()
        .find(|adapter| adapter.slug == slug)
        .ok_or_else(|| {
            let supported = AGENTS
                .iter()
                .map(|adapter| adapter.slug)
                .collect::<Vec<_>>()
                .join(", ");
            Refusal::not_started(
                "agent-unknown",
                format!("{slug:?} is not an agent Estigia knows"),
                Resolution::run(format!("estigia setup <{supported}>")),
            )
        })
}

/// The adapter a slug names for installation: [`find_agent`], refusing a retired
/// one.
///
/// A retired adapter keeps its entry so `uninstall` and `doctor` can still reach
/// it; this is the door that stops an install into a host Estigia has dropped.
pub fn find_installable_agent(slug: &str) -> Result<&'static AgentAdapter, Refusal> {
    let adapter = find_agent(slug)?;
    if adapter.retired() {
        return Err(retired_agent(adapter));
    }
    Ok(adapter)
}

/// The refusal every install path answers a retired adapter with.
pub fn retired_agent(adapter: &AgentAdapter) -> Refusal {
    Refusal::not_started(
        "agent-retired",
        format!(
            "{} is retired; Estigia no longer installs into it",
            adapter.slug
        ),
        Resolution::run(format!("estigia uninstall {}", adapter.slug)),
    )
}

/// The files a `--scope=workspace` install writes for one adapter.
///
/// `None` when the adapter's project-level destinations have not been verified
/// against the runtime's own documentation. A path the runtime does not read
/// reports success and configures nothing, so an unverified adapter is refused
/// rather than guessed at. The verified destinations, and where each was read
/// from, are in `docs/what-it-writes.md`.
fn workspace_paths(adapter: &AgentAdapter, workspace: &Path) -> Option<AgentPaths> {
    let skill_at = |base: PathBuf| base.join(skill::DIRECTORY);
    let agents_at = |base: PathBuf| {
        adapter
            .agent_definitions_root()
            .map(|directory| base.join(directory))
    };
    let built = match adapter.instructions {
        InstructionFile::Neutral => AgentPaths {
            skill_root: skill_at(workspace.join(".agents").join("skills")),
            instructions: workspace.join("AGENTS.md"),
            hooks: None,
            mcp_config: None,
            plugin: None,
            loop_plugin: None,
            commands: None,
            prompts: None,
            agents_root: None,
            notes: Vec::new(),
        },
        InstructionFile::ClaudeCode => AgentPaths {
            skill_root: skill_at(workspace.join(".claude").join("skills")),
            instructions: workspace.join("CLAUDE.md"),
            hooks: Some(workspace.join(".claude").join("settings.json")),
            mcp_config: Some(workspace.join(".mcp.json")),
            plugin: None,
            loop_plugin: None,
            commands: Some(workspace.join(".claude").join("commands")),
            prompts: None,
            agents_root: agents_at(workspace.join(".claude")),
            notes: Vec::new(),
        },
        InstructionFile::Codex => AgentPaths {
            skill_root: skill_at(workspace.join(".agents").join("skills")),
            instructions: workspace.join("AGENTS.md"),
            hooks: Some(workspace.join(".codex").join("hooks.json")),
            mcp_config: Some(workspace.join(".codex").join("config.toml")),
            plugin: None,
            loop_plugin: None,
            commands: None,
            prompts: None,
            agents_root: None,
            notes: Vec::new(),
        },
        InstructionFile::OpenCode => AgentPaths {
            // `SkillsRoot::Neutral`: the shared root, as for the agent-neutral
            // row, so the one `AGENTS.md` four runtimes read names a skill
            // directory they all keep.
            skill_root: skill_at(workspace.join(".agents").join("skills")),
            instructions: workspace.join("AGENTS.md"),
            hooks: None,
            mcp_config: Some(workspace.join("opencode.json")),
            plugin: Some(
                workspace
                    .join(".opencode")
                    .join(plugin::DIRECTORY)
                    .join(plugin::FILE),
            ),
            loop_plugin: Some(
                workspace
                    .join(".opencode")
                    .join(plugin::DIRECTORY)
                    .join(plugin::LOOP_FILE),
            ),
            commands: None,
            prompts: None,
            agents_root: agents_at(workspace.join(".opencode")),
            notes: Vec::new(),
        },
        InstructionFile::Cursor => AgentPaths {
            skill_root: skill_at(workspace.join(".agents").join("skills")),
            instructions: workspace.join("AGENTS.md"),
            hooks: Some(workspace.join(".cursor").join("hooks.json")),
            mcp_config: Some(workspace.join(".cursor").join("mcp.json")),
            plugin: None,
            loop_plugin: None,
            commands: None,
            prompts: None,
            agents_root: None,
            notes: Vec::new(),
        },
        // ZCode and DeepSeek Harness are adapted globally, but where each reads
        // a project-level file has not been verified. The retired adapters are
        // not installed into at all. Both are refused rather than guessed.
        InstructionFile::ZCode
        | InstructionFile::DeepSeekHarness
        | InstructionFile::Pi
        | InstructionFile::GeminiCli
        | InstructionFile::Qwen
        | InstructionFile::Windsurf
        | InstructionFile::Crush
        | InstructionFile::Continue
        | InstructionFile::Cline => return None,
    };
    Some(built)
}

/// The workspace a `--scope=workspace` install is rooted at, validated.
///
/// `None` under the global scope. A missing or relative workspace is refused
/// here, so no caller has to remember the rule.
fn workspace_of(options: &SetupOptions) -> Result<Option<PathBuf>> {
    if options.scope != InstallScope::Workspace {
        return Ok(None);
    }
    let workspace = options.workspace.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "--scope=workspace needs a workspace to install into; name one with --workspace"
        )
    })?;
    paths::require_absolute(&workspace, "workspace")?;
    Ok(Some(workspace))
}

/// Where one adapter's files live, under the chosen scope.
///
/// `global` resolves the machine's roots; `workspace` maps the adapter to its
/// project-level destinations under `options.workspace`, refusing an adapter
/// whose destinations have not been verified.
pub fn resolve_paths(adapter: &AgentAdapter, options: &SetupOptions) -> Result<AgentPaths> {
    match workspace_of(options)? {
        Some(workspace) => workspace_paths(adapter, &workspace).ok_or_else(|| {
            anyhow::anyhow!(
                "{} has no verified project-level install destination; --scope=workspace \
                 refuses it rather than writing a path the agent may not read",
                adapter.slug
            )
        }),
        None => {
            let environment = Environment::resolve(options)?;
            Ok(paths_in(adapter, &environment))
        }
    }
}

/// One line per inherited root this run declined to use, with no adapter
/// attached.
///
/// A per-adapter resolution keeps a note only for the adapter whose own
/// resolved paths sit under the dropped variable's fallback, so an adapter the
/// substitution moved out of `is_present`'s reach carries no note at all — and
/// `sync` with no adapter named never reports it. The note text is
/// adapter-independent, so a run-level caller takes them here.
pub fn substitution_notes(options: &SetupOptions) -> Vec<String> {
    if options.scope == InstallScope::Workspace {
        return Vec::new();
    }
    Environment::resolve(options)
        .map(|environment| {
            environment
                .notes
                .into_iter()
                .map(|(_, message)| message)
                .collect()
        })
        .unwrap_or_default()
}

/// The adapter's own configuration directory the agent-definitions root sits
/// under, or `None` for an adapter with no root.
///
/// Per adapter because the roots do not share a base: Claude Code keeps its
/// agents under `$CLAUDE_CONFIG_DIR`, OpenCode under `$XDG_CONFIG_HOME`'s
/// `opencode` directory, Pi under the agent directory its other files
/// already use (`$PI_CODING_AGENT_DIR`, else `~/.pi/agent`). Exhaustive, so a fourth root fails to compile until its
/// base is named here rather than landing under `~/.claude` at run time.
fn agent_definitions_base(adapter: &AgentAdapter, environment: &Environment) -> Option<PathBuf> {
    match adapter.instructions {
        InstructionFile::ClaudeCode => Some(environment.claude_config()),
        InstructionFile::OpenCode => Some(environment.xdg_config().join("opencode")),
        InstructionFile::Pi => Some(environment.pi_agent()),
        InstructionFile::Neutral
        | InstructionFile::Codex
        | InstructionFile::GeminiCli
        | InstructionFile::Cursor
        | InstructionFile::Qwen
        | InstructionFile::Crush
        | InstructionFile::Continue
        | InstructionFile::Cline
        | InstructionFile::Windsurf
        | InstructionFile::ZCode
        | InstructionFile::DeepSeekHarness => None,
    }
}

/// Where OpenCode's optional loop plugin lives, whether or not it is installed.
///
/// Named whether or not the `Loop plugin` setting is on, so an uninstall can
/// find the file it has to take back; the setting decides whether it is
/// written, not where it is.
fn loop_plugin_at(adapter: &AgentAdapter, environment: &Environment) -> Option<PathBuf> {
    match adapter.instructions {
        InstructionFile::OpenCode => Some(
            environment
                .xdg_config()
                .join("opencode")
                .join(plugin::DIRECTORY)
                .join(plugin::LOOP_FILE),
        ),
        InstructionFile::Pi => Some(
            environment
                .pi_agent()
                .join(plugin::PI_DIRECTORY)
                .join(plugin::PI_LOOP_FILE),
        ),
        _ => None,
    }
}

/// Where the generic goal and loop plugin lives beside the loop plugin, and the
/// source that goes there, for an adapter that reads one.
fn goal_loop_at(
    adapter: &AgentAdapter,
    loop_file: Option<&Path>,
) -> Option<(PathBuf, &'static str)> {
    let loop_file = loop_file.filter(|_| adapter.has_loop_plugin())?;
    let (name, source) = match adapter.instructions {
        InstructionFile::Pi => (plugin::PI_GOAL_LOOP_FILE, plugin::PI_GOAL_LOOP_SOURCE),
        _ => (plugin::GOAL_LOOP_FILE, plugin::GOAL_LOOP_SOURCE),
    };
    Some((loop_file.with_file_name(name), source))
}

fn paths_in(adapter: &AgentAdapter, environment: &Environment) -> AgentPaths {
    let skill_root = match adapter.capabilities.skills {
        SkillsRoot::Neutral => environment.home.join(".agents").join("skills"),
        SkillsRoot::ClaudeCode => environment.claude_config().join("skills"),
        SkillsRoot::Codex => environment.home.join(".codex").join("skills"),
        SkillsRoot::DshHarness => environment.dsh_home_dir().join("skills"),
    }
    .join(skill::DIRECTORY);

    let instructions = match adapter.instructions {
        InstructionFile::Neutral => environment.home.join(".agents").join("AGENTS.md"),
        InstructionFile::ClaudeCode => environment.claude_config().join("CLAUDE.md"),
        InstructionFile::Codex => environment.home.join(".codex").join("AGENTS.md"),
        InstructionFile::OpenCode => environment.xdg_config().join("opencode").join("AGENTS.md"),
        InstructionFile::Pi => environment.pi_agent().join("AGENTS.md"),
        // Gemini keeps its settings under %APPDATA% on Windows and ~/.gemini
        // everywhere else, and the instruction file sits beside them.
        InstructionFile::GeminiCli => match environment.platform {
            Platform::Windows => environment.roaming().join("gemini").join("GEMINI.md"),
            Platform::MacOs | Platform::Unix => environment.home.join(".gemini").join("GEMINI.md"),
        },
        InstructionFile::Cursor => environment
            .home
            .join(".cursor")
            .join("estigia-workflow-authority.md"),
        InstructionFile::Qwen => environment.home.join(".qwen").join("QWEN.md"),
        // Global rules, always on, and capped at six thousand characters — the
        // cap is why the path is named here rather than discovered.
        InstructionFile::Crush => environment.xdg_config().join("crush").join("CRUSH.md"),
        // `systemMessage.ts` scans `<continueHome>/rules` and applies any rule
        // that is not `invokable` and declares no globs — a markdown file with
        // no frontmatter is always applied, and the directive is exactly that.
        InstructionFile::Continue => environment
            .home
            .join(".continue")
            .join("rules")
            .join("estigia.md"),
        // Its rules directory, loaded for every task.
        InstructionFile::Cline => environment
            .home
            .join(".cline")
            .join("rules")
            .join("estigia.md"),
        InstructionFile::Windsurf => environment
            .home
            .join(".codeium")
            .join("windsurf")
            .join("memories")
            .join("global_rules.md"),
        // Verified against ZCode's own configuration reference: the user-scope
        // instruction file, loaded before any workspace `AGENTS.md`.
        InstructionFile::ZCode => environment.home.join(".zcode").join("AGENTS.md"),
        // DeepSeek Harness's user-global instruction file, under `$DSH_HOME`
        // when the harness's variable is set and contained.
        InstructionFile::DeepSeekHarness => environment.dsh_home_dir().join("AGENTS.md"),
    };

    // Where this agent's tool-call gate is registered. The file differs per
    // adapter.
    let hooks = adapter.gate_spec().and_then(|_| {
        Some(match adapter.instructions {
            InstructionFile::ClaudeCode => environment.claude_config().join("settings.json"),
            InstructionFile::Cursor => environment.home.join(".cursor").join("hooks.json"),
            // A sidecar beside the config, not inside it: a JSON file is one
            // Estigia can edit without reformatting somebody else's TOML.
            InstructionFile::Codex => environment.home.join(".codex").join("hooks.json"),
            InstructionFile::Qwen => environment.home.join(".qwen").join("settings.json"),
            InstructionFile::Windsurf => environment
                .home
                .join(".codeium")
                .join("windsurf")
                .join("hooks.json"),
            // Hooks, MCP servers and everything else share one file.
            InstructionFile::Crush => environment.xdg_config().join("crush").join("crush.json"),
            // Cline is deliberately absent: it has no settings-file gate. Its
            // hook is a script Estigia owns whole, resolved as a plugin below,
            // and the `gate_spec` guard above never reaches this match for it.
            InstructionFile::Continue => environment.home.join(".continue").join("settings.json"),
            InstructionFile::GeminiCli => match environment.platform {
                Platform::Windows => environment.roaming().join("gemini").join("settings.json"),
                Platform::MacOs | Platform::Unix => {
                    environment.home.join(".gemini").join("settings.json")
                }
            },
            _ => return None,
        })
    });

    // Read out of each agent's own source or reference, the way the skill roots
    // are: an MCP entry written where the agent does not read it reports success
    // and configures nothing.
    let mcp_config = match adapter.instructions {
        InstructionFile::ClaudeCode => Some(environment.claude_mcp_config()),
        InstructionFile::Codex => Some(environment.home.join(".codex").join("config.toml")),
        InstructionFile::Cursor => Some(environment.home.join(".cursor").join("mcp.json")),
        InstructionFile::Qwen => Some(environment.home.join(".qwen").join("settings.json")),
        // Where Windsurf's MCP servers live has not been verified. Named as
        // unknown rather than guessed.
        InstructionFile::Windsurf => None,
        InstructionFile::Crush => Some(environment.xdg_config().join("crush").join("crush.json")),
        // Where Cline reads MCP servers has not been verified; named unknown
        // rather than guessed. The gate is what this adapter is for.
        InstructionFile::Cline => None,
        InstructionFile::Continue => None,
        InstructionFile::GeminiCli => Some(match environment.platform {
            Platform::Windows => environment.roaming().join("gemini").join("settings.json"),
            Platform::MacOs | Platform::Unix => {
                environment.home.join(".gemini").join("settings.json")
            }
        }),
        InstructionFile::OpenCode => Some(
            environment
                .xdg_config()
                .join("opencode")
                .join("opencode.json"),
        ),
        InstructionFile::Pi => Some(environment.pi_agent().join("mcp.json")),
        // The neutral root is a convention for skills, not an agent with an MCP
        // client of its own.
        InstructionFile::Neutral => None,
        // Verified against a running client rather than read on a page — see
        // `docs/honesty.md`.
        InstructionFile::ZCode => Some(
            environment
                .home
                .join(".zcode")
                .join("cli")
                .join("config.json"),
        ),
        // DeepSeek Harness's server registration site has not been crossed
        // against a real installation; named unknown rather than guessed.
        InstructionFile::DeepSeekHarness => None,
    };

    // OpenCode is the only agent here that both loads plugins and has a
    // documented hook that can **deny** a tool call.
    let windows = environment.platform == Platform::Windows;
    let plugin = match adapter.instructions {
        InstructionFile::OpenCode => Some(
            environment
                .xdg_config()
                .join("opencode")
                .join(plugin::DIRECTORY)
                .join(plugin::FILE),
        ),
        InstructionFile::Pi => Some(
            environment
                .pi_agent()
                .join(plugin::PI_DIRECTORY)
                .join(plugin::PI_FILE),
        ),
        // Cline discovers hooks by file name rather than a settings entry, so
        // its gate is a file Estigia owns whole — the same shape as a plugin.
        InstructionFile::Cline => Some(
            environment
                .home
                .join(".cline")
                .join("hooks")
                .join(plugin::cline_hook_file(windows)),
        ),
        _ => None,
    };

    // Where the loop plugin would live, for the adapter that reads one, whether
    // or not the `Loop plugin` setting writes it. `None` everywhere else.
    let loop_plugin = loop_plugin_at(adapter, environment);

    // Where Claude Code reads the loop's slash commands, whether or not the
    // `Loop plugin` setting writes them.
    let commands = (adapter.instructions == InstructionFile::ClaudeCode)
        .then(|| environment.claude_config().join("commands"));
    let prompts = (adapter.instructions == InstructionFile::Pi)
        .then(|| environment.pi_agent().join(plugin::PI_PROMPT_DIRECTORY));

    // See the field's own note, and `AgentAdapter::agent_definitions_root` for
    // which adapters have one and why. The base is per adapter because Claude
    // Code's and OpenCode's roots do not share one.
    let agents_root = adapter
        .agent_definitions_root()
        .zip(agent_definitions_base(adapter, environment))
        .map(|(directory, base)| base.join(directory));
    // Built with an empty `notes` first and destructured back out immediately
    // below, so a field added to `AgentPaths` makes the destructure fail to
    // compile until somebody names it here — the omission has to be deliberate.
    let built = AgentPaths {
        skill_root,
        instructions,
        hooks,
        mcp_config,
        plugin,
        loop_plugin,
        commands,
        prompts,
        agents_root,
        notes: Vec::new(),
    };

    // Which of this adapter's own roots the substituted variable actually
    // decided, rather than every note the environment carries. Checked
    // structurally — does any resolved path here sit under the fallback — so
    // an adapter added later is covered without a second list to keep in step.
    let AgentPaths {
        skill_root,
        instructions,
        hooks,
        mcp_config,
        plugin,
        loop_plugin,
        commands,
        prompts,
        agents_root,
        notes: _,
    } = &built;
    let resolved: [Option<&Path>; 9] = [
        Some(skill_root.as_path()),
        Some(instructions.as_path()),
        hooks.as_deref(),
        mcp_config.as_deref(),
        plugin.as_deref(),
        loop_plugin.as_deref(),
        commands.as_deref(),
        prompts.as_deref(),
        agents_root.as_deref(),
    ];
    let touches = |base: PathBuf| {
        resolved
            .iter()
            .flatten()
            .any(|path| path.starts_with(&base))
    };
    let notes = environment
        .notes
        .iter()
        .filter(|(variable, _)| touches(variable.fallback(environment)))
        .map(|(_, message)| message.clone())
        .collect();

    AgentPaths { notes, ..built }
}

/// Writes the `effort:` line, or takes the placeholder line out entirely.
///
/// Taking it out is the whole reason this is not a `replace`: the absent field
/// is the only way to say nothing. Line-preserving rather than line-splitting,
/// so a CRLF checkout comes back with its endings.
fn render_effort(rendered: &str, effort: Option<crate::config::Effort>) -> String {
    match effort {
        Some(effort) => rendered.replace("{{EFFORT}}", &format!("effort: {}", effort.as_str())),
        None => rendered
            .split_inclusive('\n')
            .filter(|line| line.trim() != "{{EFFORT}}")
            .collect(),
    }
}

/// Renders one delegated worker definition.
///
/// The grant is written literally in each file because it belongs to the role
/// rather than to a setting. A developer that cannot run the suite has not
/// been made safer, and an analyst handed a shell to make one row simpler is the
/// widening this pair exists to avoid. `Evidence standard` decides the
/// reviewer's grant and only the reviewer's — that row is owned elsewhere.
pub fn render_delegated_agent(template: &str, target: &str, config: &Config) -> String {
    let model = config.models.for_target(target).unwrap_or("inherit");
    render_effort(
        &template.replace("{{MODEL}}", model),
        config.models.effort_for(target),
    )
}

/// Renders one delegated worker definition in OpenCode's own dialect.
///
/// The body is the template's, unchanged; only the frontmatter is translated,
/// because OpenCode routes a sub-agent by the file's name and reads a
/// `permission:` map where the Claude dialect writes a comma-separated `tools:`
/// line. `description` is carried verbatim and `mode: subagent` says the worker
/// is launched rather than selected in the Tab rotation. `model:` is written
/// only when `Model routing` names one — OpenCode inherits the parent's model
/// otherwise, and Claude's literal `inherit` is not a value it reads. There is
/// no `effort:` line, because OpenCode has no effort field: an effort named for
/// a worker reaches no file here, which `docs/honesty.md` records rather than
/// mapping onto a field the host does not have.
///
/// `edit` and `bash` are derived from the same `tools:` line the Claude
/// rendering carries — `edit` is allowed iff that list names any write tool,
/// `bash` iff it names `Bash` — so the two dialects agree on those two. The
/// fetch and delegate tools are literal denials here, which is what every
/// shipped template's Claude list also says; a template that allowed one would
/// diverge.
///
/// A template whose first line is not the frontmatter fence is returned
/// unchanged, so a malformed template would reach OpenCode's root in
/// Claude-dialect bytes. Every shipped template opens with the fence.
pub fn render_opencode_delegated_agent(template: &str, target: &str, config: &Config) -> String {
    let policy = crate::harness::roles::declared_policy(template);
    let allows = |tool: &str| {
        matches!(
            policy.as_ref().map(|policy| policy.verdict(tool)),
            Some(crate::harness::roles::Verdict::Allow)
        )
    };
    let editable = ["Write", "Edit", "MultiEdit", "NotebookEdit", "Update"]
        .iter()
        .any(|tool| allows(tool));
    let verdict = |allowed: bool| if allowed { "allow" } else { "deny" };

    let Some((frontmatter, body)) = split_frontmatter(template) else {
        return template.to_owned();
    };
    let description = description_block(frontmatter).unwrap_or_default();
    let model = config
        .models
        .for_target(target)
        .filter(|model| *model != "inherit")
        .map(|model| format!("model: {model}\n"))
        .unwrap_or_default();

    format!(
        "---\n{description}\nmode: subagent\n{model}permission:\n  edit: {}\n  bash: {}\n  \
         webfetch: deny\n  websearch: deny\n  task: deny\n---{body}",
        verdict(editable),
        verdict(allows("Bash")),
    )
}

/// Renders one delegated worker definition in the dialect Pi's example
/// subagent extension reads.
///
/// The frontmatter is `name`, `description`, `tools` and `model`, and the body
/// is the template's, unchanged. `tools` is the template's allowlist mapped onto
/// Pi's built-in tool names (`Read` to `read`, `Grep` to `grep`, `Glob` to
/// `find` and `ls`, `Write`, `Edit`, `Bash` to their lowercase twins), derived
/// from the same declared policy the other dialects use. `model:` is written
/// only when `Model routing` names one, and there is no effort line: the
/// example reads no such field. A template whose first line is not the
/// frontmatter fence is returned unchanged.
pub fn render_pi_delegated_agent(template: &str, target: &str, config: &Config) -> String {
    let policy = crate::harness::roles::declared_policy(template);
    let allows = |tool: &str| {
        matches!(
            policy.as_ref().map(|policy| policy.verdict(tool)),
            Some(crate::harness::roles::Verdict::Allow)
        )
    };
    let tools: Vec<&str> = [
        ("Read", &["read"][..]),
        ("Grep", &["grep"][..]),
        ("Glob", &["find", "ls"][..]),
        ("Write", &["write"][..]),
        ("Edit", &["edit"][..]),
        ("Bash", &["bash"][..]),
    ]
    .into_iter()
    .filter(|(tool, _)| allows(tool))
    .flat_map(|(_, names)| names.iter().copied())
    .collect();

    let Some((frontmatter, body)) = split_frontmatter(template) else {
        return template.to_owned();
    };
    let description = description_block(frontmatter).unwrap_or_default();
    let model = config
        .models
        .for_target(target)
        .filter(|model| *model != "inherit")
        .map(|model| format!("model: {model}\n"))
        .unwrap_or_default();

    format!(
        "---\nname: {target}\n{description}\n{model}tools: {}\n---{body}",
        tools.join(", ")
    )
}

/// The frontmatter and the bytes after its closing `---`.
///
/// A leading BOM and leading whitespace are skipped before the opening `---`
/// is looked for. The body is returned whole, leading newlines included, so a
/// rendering that replaces only the frontmatter leaves the definition's prose
/// byte-identical. `None` when no opening `---` follows, or when it has no
/// closing `\n---`.
fn split_frontmatter(definition: &str) -> Option<(&str, &str)> {
    let rest = definition.trim_start_matches('\u{feff}').trim_start();
    rest.strip_prefix("---")?.split_once("\n---")
}

/// The `description:` block of a frontmatter fence, verbatim.
///
/// It starts at the first line beginning `description:` and runs through every
/// following line that is empty or starts with a space or tab — YAML folded
/// blocks continue through blank lines — stopping at the first non-empty line
/// at column zero. `None` when no line declares a description.
fn description_block(frontmatter: &str) -> Option<String> {
    let mut lines = frontmatter
        .lines()
        .skip_while(|line| !line.starts_with("description:"));
    let first = lines.next()?;
    let block: Vec<&str> = std::iter::once(first)
        .chain(lines.take_while(|line| line.is_empty() || line.starts_with([' ', '\t'])))
        .collect();
    Some(block.join("\n"))
}

/// Renders one delegated worker definition in the adapter's own dialect.
///
/// `None` when the adapter has no agent-definitions root, which is the same
/// answer [`AgentAdapter::has_agent_definitions`] gives — a test crosses the
/// two rather than trusting them to stay level.
fn render_delegated_definition(
    adapter: &AgentAdapter,
    template: &str,
    target: &str,
    config: &Config,
) -> Option<String> {
    match adapter.instructions {
        InstructionFile::ClaudeCode => Some(render_delegated_agent(template, target, config)),
        InstructionFile::OpenCode => {
            Some(render_opencode_delegated_agent(template, target, config))
        }
        InstructionFile::Pi => Some(render_pi_delegated_agent(template, target, config)),
        InstructionFile::Neutral
        | InstructionFile::Codex
        | InstructionFile::GeminiCli
        | InstructionFile::Cursor
        | InstructionFile::Qwen
        | InstructionFile::Crush
        | InstructionFile::Continue
        | InstructionFile::Cline
        | InstructionFile::Windsurf
        | InstructionFile::ZCode
        | InstructionFile::DeepSeekHarness => None,
    }
}

#[derive(Clone, Copy)]
enum ReviewerDefinitionCode {
    Unowned,
    Changed,
}

impl ReviewerDefinitionCode {
    fn code(self) -> &'static str {
        match self {
            Self::Unowned => "reviewer-definition-unowned",
            Self::Changed => "reviewer-definition-changed",
        }
    }
}

fn reviewer_definition_refusal(code: ReviewerDefinitionCode, path: &Path, reason: &str) -> Refusal {
    Refusal::not_started(
        code.code(),
        format!("{} {reason}", path.display()),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            format!("move {} aside before setup", path.display()),
        ),
    )
}

/// Refuse a delegated worker whose definition somebody else authored.
///
/// `analyst` is a name another harness's orchestrator answers to, so the
/// operator most likely to turn this worker on is the one whose `analyst.md`
/// already exists — and because ownership is recorded only for files Estigia
/// created, overwriting it would be gone in both directions, silently.
fn delegated_definition_refusal(worker: &str, path: &Path) -> Refusal {
    Refusal::not_started(
        "delegated-definition-unowned",
        format!(
            "{} already exists and Estigia has no record of creating it",
            path.display()
        ),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            format!(
                "move {} aside to let Estigia author it — that always clears this — or, on a \
                 machine Estigia is already installed on, run `estigia config set \"Model \
                 routing\" --agent <slug>` with a value that does not name {worker} to leave the file alone",
                path.display()
            ),
        ),
    )
}

fn validate_delegated_definitions(paths: &AgentPaths, effective: &Config) -> Result<()> {
    let Some(agents_root) = paths.agents_root.as_deref() else {
        return Ok(());
    };
    for (worker, file) in skill::DELEGATED_AGENTS {
        if !effective.models.installs(worker) {
            continue;
        }
        let Some(name) = file.path.strip_prefix("agents/") else {
            continue;
        };
        let target = agents_root.join(name);
        if read_optional(&target)?.is_none()
            || skill::record::created_outside(&paths.skill_root, &target)
        {
            continue;
        }
        return Err(delegated_definition_refusal(worker, &target).into());
    }
    Ok(())
}

fn reviewer_target(paths: &AgentPaths, name: &str) -> Option<PathBuf> {
    Some(paths.agents_root.as_ref()?.join(format!("{name}.md")))
}

/// The reviewer definition with this repository's evidence standard filled
/// in, and installed under `name`.
///
/// The grant is derived rather than fixed, for the reason
/// [`crate::config::Evidence`] gives. The discipline paragraph moves with the
/// grant: a capability handed over without the rule that bounds it is worse
/// than not handing it over. [`Seal`] is orthogonal and decided first: a
/// sealed reviewer's grant does not depend on `evidence` at all. The
/// frontmatter `name:` line is rewritten to `name` rather than derived from
/// `seal`, so the caller — which already knows which reserved name it is
/// rendering for — decides, and a rendering is installable under its own
/// reserved name rather than shadowing another one. The needle that
/// substitution searches for is read back out of `template`'s own rendered
/// frontmatter rather than assumed to be the judge's placeholder, so a
/// template whose own `name:` line already equals `name` (every reserved
/// template's, on the row it installs under by default) is rewritten
/// correctly rather than skipped by accident.
pub fn render_reviewer_agent(template: &str, evidence: Evidence, seal: Seal, name: &str) -> String {
    let (tools, paragraph) = match seal {
        Seal::Sealed => (
            "WebSearch",
            "You hold one tool — a web search — so you cannot read a file, search the repository,\n\
             or run a shell. Do not use the search to look for the change under review: your\n\
             verdict rests on the payload you were handed, not on anything you find. Do NOT edit\n\
             files, change tracker state, record the aggregate verdict, or repair a finding.\n\
             Return findings to the orchestrator with severity, precise evidence, and a stable\n\
             identity based on the affected behavior and location.",
        ),
        Seal::Unsealed => match evidence {
            // Reproduced **exactly** as every build before wrote it — same wrapping,
            // same sentence order. `reviewer_is_static` compares bytes, so a
            // rendering that merely says the same thing differently makes every
            // installed copy foreign; `the_default_rendering_is_the_bytes_already_installed`
            // pins it.
            Evidence::Reading => (
                "Read, Grep, Glob",
                "Remain read-only. Do NOT edit files, run a shell, change tracker state, record the aggregate verdict,\n\
                 or repair a finding. Return findings to the orchestrator with severity, precise evidence, and a stable\n\
                 identity based on the affected behavior and location.",
            ),
            // A shell, because a finding established by running something cannot be
            // established without running it — and the isolation rule in the same
            // breath: the grant does not stop a judge rewriting the target; the
            // directory it is confined to does.
            Evidence::Measuring => (
                "Read, Grep, Glob, Bash",
                "A finding here is established by running something, so you may build, test and mutate\n\
                 — inside the directory this launch hands you and nowhere else. That directory\n\
                 is yours alone for the duration: not a sibling judge's, not the orchestrator's,\n\
                 not another run's, and outside the checkout under claim. If you find yourself\n\
                 inside one, stop and say so rather than measuring there: a write inside it is\n\
                 judged against the claim of the run that launched you, and allowed.\n\
                 Restore every mutation and confirm the tree is clean before you report.\n\
     If it is already dirty when you arrive, stop and say so rather than\n\
                 restoring it — whatever made it dirty was somebody's measurement, and repairing\n\
                 it destroys theirs. Still do NOT change tracker state, record the aggregate\n\
                 verdict, or repair a finding. Return findings to the orchestrator with severity,\n\
                 precise evidence, and a stable identity based on the affected behavior and\n\
                 location.",
            ),
        },
    };
    let discipline = format!("{paragraph}\n\n{}", crate::skill::VERDICT_BASIS_REQUIREMENT);
    let rendered = template
        .replace("{{TOOLS}}", tools)
        .replace("{{DISCIPLINE}}", &discipline);
    let Some(placeholder) = declared_reviewer_name(&rendered) else {
        return rendered;
    };
    rendered.replacen(
        &format!("\nname: {placeholder}\n"),
        &format!("\nname: {name}\n"),
        1,
    )
}

/// The name a rendered template's own frontmatter declares.
///
/// Every reserved template carries its own placeholder in its own bytes — the
/// judge's say `name: review-blind`, the lens's says `name: review-lens` —
/// rather than one hardcoded constant every template is assumed to share.
/// `None` when a template has no `name:` line at all, which
/// [`render_reviewer_agent`] reads as nothing to rewrite.
fn declared_reviewer_name(rendered: &str) -> Option<&str> {
    let frontmatter = crate::harness::roles::frontmatter(rendered)?;
    let (_, after) = frontmatter.split_once("\nname: ")?;
    after.split_once('\n').map(|(name, _)| name)
}

pub(crate) const REVIEWER_TEMPLATE_BEFORE_CONTRACT: &str =
    include_str!("reviewer_before_contract.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_SEVERITY: &str =
    include_str!("reviewer_before_severity.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_NEVER_SEVERE: &str =
    include_str!("reviewer_before_never_severe.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_LOCATION: &str =
    include_str!("reviewer_before_location.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_PANEL_SIZE: &str =
    include_str!("reviewer_before_panel_size.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_REVIEW_CONTRACT: &str =
    include_str!("reviewer_before_review_contract.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_PORT_REPAIR: &str =
    include_str!("reviewer_before_port_repair.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_LENS_PASS: &str =
    include_str!("reviewer_before_lens_pass.md");

pub(crate) const LENS_TEMPLATE_BEFORE_TWO_WORDS: &str = include_str!("lens_before_two_words.md");

pub(crate) const LENS_RISK_BEFORE_TWO_WORDS: &str = include_str!("lens_risk_before_two_words.md");

pub(crate) const LENS_READABILITY_BEFORE_TWO_WORDS: &str =
    include_str!("lens_readability_before_two_words.md");

pub(crate) const LENS_RELIABILITY_BEFORE_TWO_WORDS: &str =
    include_str!("lens_reliability_before_two_words.md");

pub(crate) const LENS_READBACK_BEFORE_TWO_WORDS: &str =
    include_str!("lens_readback_before_two_words.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_TIER_SCALED_PANEL: &str =
    include_str!("reviewer_before_tier_scaled_panel.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_JUDGMENT_DAY: &str =
    include_str!("reviewer_before_judgment_day.md");

pub(crate) const LENS_TEMPLATE_BEFORE_JUDGMENT_DAY: &str =
    include_str!("lens_before_judgment_day.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_JUDGMENT_DAY_CONTRACT: &str =
    include_str!("reviewer_before_judgment_day_contract.md");

pub(crate) const LENS_TEMPLATE_BEFORE_DELTA_ONLY_DESCRIPTION: &str =
    include_str!("lens_before_delta_only_description.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_NO_LOW_JUDGE: &str =
    include_str!("reviewer_before_no_low_judge.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_ORIGIN_FIELD: &str =
    include_str!("reviewer_before_origin_field.md");

pub(crate) const REVIEWER_TEMPLATE_BEFORE_IMPACT_SCOPE: &str =
    include_str!("reviewer_before_impact_scope.md");

pub(crate) const LENS_TEMPLATE_BEFORE_ORIGIN_FIELD: &str =
    include_str!("lens_before_origin_field.md");

pub(crate) const LENS_RISK_BEFORE_ORIGIN_FIELD: &str =
    include_str!("lens_risk_before_origin_field.md");

pub(crate) const LENS_READABILITY_BEFORE_ORIGIN_FIELD: &str =
    include_str!("lens_readability_before_origin_field.md");

pub(crate) const LENS_RELIABILITY_BEFORE_ORIGIN_FIELD: &str =
    include_str!("lens_reliability_before_origin_field.md");

pub(crate) const LENS_READBACK_BEFORE_ORIGIN_FIELD: &str =
    include_str!("lens_readback_before_origin_field.md");

pub(crate) const LENS_RISK_BEFORE_MANDATE: &str = include_str!("lens_risk_before_mandate.md");

pub(crate) const LENS_READABILITY_BEFORE_MANDATE: &str =
    include_str!("lens_readability_before_mandate.md");

pub(crate) const LENS_RELIABILITY_BEFORE_MANDATE: &str =
    include_str!("lens_reliability_before_mandate.md");

pub(crate) const LENS_READBACK_BEFORE_MANDATE: &str =
    include_str!("lens_readback_before_mandate.md");

pub(crate) const LENS_TEMPLATE_BEFORE_EMPTY_FINDINGS: &str =
    include_str!("lens_before_empty_findings.md");

pub(crate) const LENS_RISK_BEFORE_EMPTY_FINDINGS: &str =
    include_str!("lens_risk_before_empty_findings.md");

pub(crate) const LENS_READABILITY_BEFORE_EMPTY_FINDINGS: &str =
    include_str!("lens_readability_before_empty_findings.md");

pub(crate) const LENS_RELIABILITY_BEFORE_EMPTY_FINDINGS: &str =
    include_str!("lens_reliability_before_empty_findings.md");

pub(crate) const LENS_READBACK_BEFORE_EMPTY_FINDINGS: &str =
    include_str!("lens_readback_before_empty_findings.md");

fn reviewer_matches(existing: &str, rendered: &str) -> bool {
    existing == rendered || existing == as_the_file_was(Some(existing), rendered)
}

/// Every current rendering [`reviewer_is_static`] can recognise, built once.
///
/// Enumerating [`Evidence::all`] × [`RESERVED_REVIEWERS`] re-rendered every
/// template on every ownership check; the inputs are const tables, so the
/// bodies are a pure function of the binary.
static STATIC_REVIEWER_RENDERINGS: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| {
        crate::harness::roles::RESERVED_REVIEWERS
            .iter()
            .flat_map(|&(name, seal, template)| {
                Evidence::all()
                    .into_iter()
                    .map(move |evidence| render_reviewer_agent(template, evidence, seal, name))
            })
            .collect()
    });

pub(crate) fn reviewer_is_static_for_seal(existing: &str, seal: Seal, name: &str) -> bool {
    let Some(template) = crate::harness::roles::reserved_reviewer_template(name) else {
        return false;
    };
    Evidence::all().into_iter().any(|evidence| {
        reviewer_matches(
            existing,
            &render_reviewer_agent(template, evidence, seal, name),
        )
    })
}

/// Whether the installed reviewer is one this binary's renderer can produce.
///
/// One of the spellings Estigia can produce now, enumerated over
/// [`Evidence::all`] × [`crate::harness::roles::RESERVED_REVIEWERS`] — so a
/// third standard, or a name added to that list, is covered without a second
/// enumeration to keep in step. Launch uses [`reviewer_is_static_for_seal`]
/// instead, bound to the canonical path's own name; this name-agnostic form
/// remains for the setup-time ownership decision, which is not asking which
/// reserved name a path is.
pub(crate) fn reviewer_is_static(existing: &str) -> bool {
    STATIC_REVIEWER_RENDERINGS
        .iter()
        .any(|rendered| reviewer_matches(existing, rendered))
}

/// Every superseded template `historical_reviewer_renderings` derives a
/// frozen body from, oldest last. The current asset is not here: its
/// rendered forms are exactly what [`reviewer_is_static`] already covers, and
/// only its pre-basis body is a historical one.
///
/// Lens templates belong here too. `reviewer_is_static` renders whichever
/// template a reserved name carries, so a lens whose own body changed is
/// recognised only while it matches the current one; without a frozen
/// predecessor `validate_reviewer_definition` refuses an installed copy as
/// matching no rendering of its definition.
///
/// The enumeration pairs every template here with every reserved name, so it
/// admits `(template, name)` combinations no release ever wrote — a superseded
/// judge body under a lens name. That is deliberate: the ledger is a strict
/// superset for ownership, not a provenance record, and every extra member is
/// accepted only at a reserved path.
const REVIEWER_TEMPLATES_SUPERSEDED: &[&str] = &[
    LENS_TEMPLATE_BEFORE_EMPTY_FINDINGS,
    LENS_RISK_BEFORE_EMPTY_FINDINGS,
    LENS_READABILITY_BEFORE_EMPTY_FINDINGS,
    LENS_RELIABILITY_BEFORE_EMPTY_FINDINGS,
    LENS_READBACK_BEFORE_EMPTY_FINDINGS,
    REVIEWER_TEMPLATE_BEFORE_JUDGMENT_DAY_CONTRACT,
    LENS_TEMPLATE_BEFORE_DELTA_ONLY_DESCRIPTION,
    REVIEWER_TEMPLATE_BEFORE_JUDGMENT_DAY,
    LENS_TEMPLATE_BEFORE_JUDGMENT_DAY,
    LENS_RISK_BEFORE_MANDATE,
    LENS_READABILITY_BEFORE_MANDATE,
    LENS_RELIABILITY_BEFORE_MANDATE,
    LENS_READBACK_BEFORE_MANDATE,
    REVIEWER_TEMPLATE_BEFORE_NO_LOW_JUDGE,
    REVIEWER_TEMPLATE_BEFORE_IMPACT_SCOPE,
    REVIEWER_TEMPLATE_BEFORE_ORIGIN_FIELD,
    LENS_TEMPLATE_BEFORE_ORIGIN_FIELD,
    LENS_RISK_BEFORE_ORIGIN_FIELD,
    LENS_READABILITY_BEFORE_ORIGIN_FIELD,
    LENS_RELIABILITY_BEFORE_ORIGIN_FIELD,
    LENS_READBACK_BEFORE_ORIGIN_FIELD,
    REVIEWER_TEMPLATE_BEFORE_TIER_SCALED_PANEL,
    LENS_TEMPLATE_BEFORE_TWO_WORDS,
    LENS_RISK_BEFORE_TWO_WORDS,
    LENS_READABILITY_BEFORE_TWO_WORDS,
    LENS_RELIABILITY_BEFORE_TWO_WORDS,
    LENS_READBACK_BEFORE_TWO_WORDS,
    REVIEWER_TEMPLATE_BEFORE_LENS_PASS,
    REVIEWER_TEMPLATE_BEFORE_PORT_REPAIR,
    REVIEWER_TEMPLATE_BEFORE_REVIEW_CONTRACT,
    REVIEWER_TEMPLATE_BEFORE_PANEL_SIZE,
    REVIEWER_TEMPLATE_BEFORE_LOCATION,
    REVIEWER_TEMPLATE_BEFORE_NEVER_SEVERE,
    REVIEWER_TEMPLATE_BEFORE_SEVERITY,
    REVIEWER_TEMPLATE_BEFORE_CONTRACT,
];

/// Every body a previous binary could have installed under a reserved
/// reviewer name, so an install still carrying one upgrades instead of being
/// refused as foreign.
///
/// Each template in [`REVIEWER_TEMPLATES_SUPERSEDED`], rendered across every
/// `(name, seal)` pair [`crate::harness::roles::RESERVED_REVIEWERS`] carries
/// and both forms `evidence` produces — with and without
/// `VERDICT_BASIS_REQUIREMENT` — so a sealed body from a superseded template
/// is covered the same way an unsealed one already was. Before this, every
/// entry rendered under `Seal::Unsealed` and `RESERVED_REVIEWER` alone, which
/// left `review-blind-sealed.md` from any earlier binary unrecognisable; a
/// uniform enumeration over both forms and every reserved name is a strict
/// superset of that narrower set.
/// [`historical_reviewer_renderings`], built once per process and
/// deduplicated. The inputs are `include_str!` bodies and const tables, so
/// the result is a pure function of the binary and cannot go stale.
static HISTORICAL_REVIEWER_RENDERINGS: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| {
        let mut bodies = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut push = |body: String| {
            if seen.insert(body.clone()) {
                bodies.push(body);
            }
        };
        if let Some(body) = without_basis(
            skill::REVIEW_AGENT.contents,
            Evidence::Reading,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        ) {
            push(body);
        }
        for template in REVIEWER_TEMPLATES_SUPERSEDED {
            for &(name, seal, _) in crate::harness::roles::RESERVED_REVIEWERS {
                for evidence in Evidence::all() {
                    push(render_reviewer_agent(template, evidence, seal, name));
                    if let Some(body) = without_basis(template, evidence, seal, name) {
                        push(body);
                    }
                }
            }
        }
        bodies
    });

fn historical_reviewer_renderings() -> &'static [String] {
    &HISTORICAL_REVIEWER_RENDERINGS
}

/// The same bodies, keyed by the reserved name each one's frontmatter declares.
///
/// `validate_reviewer_definition` asks about one name; scanning every
/// rendering for that answer was the dominant cost of an upgrade loop over
/// the whole ledger. The map is built once beside the flat list so the two
/// cannot drift.
static HISTORICAL_REVIEWER_RENDERINGS_BY_NAME: std::sync::LazyLock<
    std::collections::HashMap<String, Vec<usize>>,
> = std::sync::LazyLock::new(|| {
    let mut by_name: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (index, body) in HISTORICAL_REVIEWER_RENDERINGS.iter().enumerate() {
        if let Some(name) = declared_reviewer_name(body) {
            by_name.entry(name.to_owned()).or_default().push(index);
        }
    }
    by_name
});

fn without_basis(template: &str, evidence: Evidence, seal: Seal, name: &str) -> Option<String> {
    let with_basis = render_reviewer_agent(template, evidence, seal, name);
    let needle = format!("\n\n{}", crate::skill::VERDICT_BASIS_REQUIREMENT);
    let (before, after) = with_basis.split_once(&needle)?;
    Some(format!("{before}{after}"))
}

fn reviewer_is_historical(existing: &str) -> bool {
    let bodies = historical_reviewer_renderings();
    let Some(name) = declared_reviewer_name(existing) else {
        return bodies
            .iter()
            .any(|rendered| reviewer_matches(existing, rendered));
    };
    match HISTORICAL_REVIEWER_RENDERINGS_BY_NAME.get(name) {
        Some(indices) => indices
            .iter()
            .any(|&index| reviewer_matches(existing, &bodies[index])),
        None => false,
    }
}

/// Owned bytes this binary wrote, or that a previous one did, or that this
/// build's own write left at `path` under `root`.
///
/// One place for the whole rule, read by both doors that ask it: the setup
/// preflight (`validate_reviewer_definition`) and `uninstall_from`'s choice
/// between deleting a reviewer definition and keeping it as the operator's
/// edit. A digest arm added at one door and not the other would let them
/// disagree — a body setup accepts as ours-older and uninstall then keeps as
/// somebody's work, orphaned at the reserved path for the next install to
/// meet as unowned.
///
/// The digest arm answers what neither the static renderer nor the frozen
/// ledger can: a body an `--allow-source-build` install wrote from a
/// template that moved again before it was ever published, which joins no
/// ledger because nothing publishes an in-flight round — the two "no frozen
/// body joins the historical ledger" notes in
/// `the_default_rendering_is_the_bytes_already_installed` say so. Absent is
/// not read as a match: a
/// record written before digests existed, or one that never wrote at this
/// exact path, falls straight through to the static and historical checks
/// unchanged. Launch still uses [`reviewer_is_static_for_seal`], bound to the
/// canonical path's own name and seal — a body accepted here for its digest
/// alone is not a current rendering, and launch never treats it as one.
pub(crate) fn reviewer_is_ours(root: &Path, path: &Path, existing: &str) -> bool {
    reviewer_is_static(existing)
        || reviewer_is_historical(existing)
        || skill::record::written_outside(root, path).as_deref()
            == Some(skill::record::digest_of(existing).as_str())
}

/// Takes one reviewer definition back, if this install created it. Changed
/// text is the operator's work: it is kept and its path relinquished. A path
/// with no ownership record is not touched.
fn retire_reviewer(
    paths: &AgentPaths,
    pending: &mut Pending,
    name: &str,
    dry_run: bool,
) -> Result<Option<SetupAction>> {
    let Some(target) = reviewer_target(paths, name) else {
        return Ok(None);
    };
    if !skill::record::created_outside(&paths.skill_root, &target) {
        return Ok(None);
    }
    let action = match read_pending(&target, pending)? {
        None => None,
        Some(existing) if reviewer_is_ours(&paths.skill_root, &target, &existing) => {
            let action = discard(&target, ActionKind::AgentDefinition, dry_run)?;
            pending.insert(target.clone(), None);
            Some(action)
        }
        Some(existing) => {
            pending.insert(target.clone(), Some(existing));
            Some(SetupAction {
                kind: ActionKind::AgentDefinition,
                path: target.clone(),
                change: Change::Kept,
            })
        }
    };
    if !dry_run {
        skill::record::forget_outside(&paths.skill_root, &target)?;
    }
    Ok(action)
}

fn validate_reviewer_definition(paths: &AgentPaths, name: &str) -> Result<()> {
    let Some(target) = reviewer_target(paths, name) else {
        return Ok(());
    };
    let Some(root) = paths.agents_root.as_deref() else {
        return Ok(());
    };
    let candidates = crate::harness::roles::reviewer_candidates(root, name).map_err(|error| {
        reviewer_definition_refusal(
            ReviewerDefinitionCode::Unowned,
            error.path(),
            &format!("cannot be proved not to reserve {name}: {}", error.detail()),
        )
    })?;
    if let Some(other) = candidates.iter().find(|candidate| *candidate != &target) {
        return Err(reviewer_definition_refusal(
            ReviewerDefinitionCode::Unowned,
            other,
            &format!("also reserves {name} outside Estigia's canonical path"),
        )
        .into());
    }
    let Some(existing) = read_optional(&target)? else {
        return Ok(());
    };
    if !skill::record::created_outside(&paths.skill_root, &target) {
        return Err(reviewer_definition_refusal(
            ReviewerDefinitionCode::Unowned,
            &target,
            "already exists and Estigia has no record of creating it",
        )
        .into());
    }
    if !reviewer_is_ours(&paths.skill_root, &target, &existing) {
        // Edited since install, or of unknown provenance: the record holds a
        // digest for this path exactly so the two read differently. A digest
        // that is there and disagrees proves an edit — nothing else moves
        // installed bytes. No digest proves nothing either way: a record that
        // predates digests cannot tell an older build from an edit, and the
        // refusal says unknown rather than picking one.
        let edited = skill::record::written_outside(&paths.skill_root, &target)
            .is_some_and(|last| last != skill::record::digest_of(&existing));
        if edited {
            return Err(reviewer_definition_refusal(
                ReviewerDefinitionCode::Changed,
                &target,
                "was edited since Estigia installed it",
            )
            .into());
        }
        return Err(reviewer_definition_refusal(
            ReviewerDefinitionCode::Changed,
            &target,
            "was created by Estigia but matches no current or previous rendering of its definition",
        )
        .into());
    }
    Ok(())
}

/// The directive text for one adapter, naming the path where its skill lands
/// and the file that holds this adapter's own answers.
///
/// The ratchet applied to prose: the path the directive names is the path
/// [`setup`] wrote to, both from [`AgentPaths`]. The second sentence exists
/// because most adapters share one contract, so `config set --agent` writes
/// their answers to a file beside it — and nothing else the agent reads names
/// it. Only for the adapters it applies to.
pub fn directive_for(adapter: &AgentAdapter, paths: &AgentPaths) -> String {
    let mine = if !adapter.discovers_skills() {
        format!(
            "\n\nThat skill directory is shared, so your own answers are kept apart from it, in \
             `{}`. When that file is present, its rows override the contract's table for you \
             alone; `{}` beside it still overrides both.",
            skill::agent_override(&paths.skill_root, adapter.slug)
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
            crate::config::LOCAL_FILE,
        )
    } else {
        String::new()
    };
    format!(
        "{}{mine}",
        DIRECTIVE_TEMPLATE
            .replace("{skill}", skill::DIRECTORY)
            .replace("{path}", &paths.skill_root.display().to_string())
    )
}

/// Whether the directive block is in the agent's instruction file at all.
///
/// `None` here means only what it says: the instruction file could not be read.
pub fn directive_installed(adapter: &AgentAdapter, options: &SetupOptions) -> Option<bool> {
    let paths = resolve_paths(adapter, options).ok()?;
    let existing = fs::read_to_string(&paths.instructions).ok()?;
    Some(DIRECTIVE_FENCE.is_present(&existing))
}

/// Whether the directive this agent loads is the one this binary writes.
///
/// `None` when there is no directive to compare. Asked by **doing the write
/// and seeing whether it changes anything**, via the same `upsert` `setup`
/// runs — so this cannot drift from it.
pub fn directive_is_current(adapter: &AgentAdapter, options: &SetupOptions) -> Option<bool> {
    let paths = resolve_paths(adapter, options).ok()?;
    let existing = fs::read_to_string(&paths.instructions).ok()?;
    if !DIRECTIVE_FENCE.is_present(&existing) {
        return None;
    }
    let desired = DIRECTIVE_FENCE.upsert(&existing, &directive_for(adapter, &paths));
    // Line endings normalised on both sides, because `upsert` normalises and a
    // CRLF instruction file would otherwise read as drifted on every machine
    // that has one.
    Some(desired == existing.replace("\r\n", "\n"))
}

/// The setup screen was asked for where nothing can draw it. Named separately
/// from [`no_agent_named`] because the way out is different.
pub fn no_terminal() -> Refusal {
    Refusal::not_started(
        "no-terminal-for-the-screen",
        "the setup screen needs a terminal, and standard input is not one",
        Resolution::run("estigia setup --all   # or estigia setup <agent>"),
    )
}

/// A **configured** adapter that keeps its skill in the same directory as this
/// one, if there is one.
///
/// Two questions turn on this. Uninstalling: `None` means this adapter is the
/// last one out, which is when the skill can go. Reporting: an agent that is
/// not configured but whose root holds a skill is a half-finished install only
/// when the skill is nobody else's. Read through `pending` rather than off the
/// disk so that `--dry-run` answers what the real run would.
pub fn skill_shared_with(
    adapter: &AgentAdapter,
    options: &SetupOptions,
    pending: &Pending,
) -> Result<Option<&'static str>> {
    // A workspace's record is the only thing that says which adapters have an
    // install; a record that is absent or unreadable is an unknown, and an
    // unknown keeps the skill exactly as it keeps the directive — otherwise a
    // removal leaves a directive naming a directory it has just deleted.
    // A workspace.s record is the only thing that says which adapters have an
    // install; a record that is absent or unreadable is an unknown, and an
    // unknown keeps the skill exactly as it keeps the directive — otherwise a
    // removal leaves a directive naming a directory it has just deleted.
    if options.scope == InstallScope::Workspace && workspace_installed_adapters(options).is_none() {
        return Ok(Some(adapter.display_name));
    }
    // Named rather than a bare bool so a caller can say who.
    let mine = resolve_paths(adapter, options)?;
    let root = mine.skill_root;
    for other in every_adapter() {
        if other.slug == adapter.slug {
            continue;
        }
        // An adapter with no verified workspace destination is not an adapter
        // this workspace install shares a skill with.
        let Ok(theirs) = resolve_paths(other, options) else {
            continue;
        };
        if theirs.skill_root != root {
            continue;
        }
        if options.scope == InstallScope::Workspace {
            // Reached only with a record that can be read: who has an install
            // is what it says, not what a shared instruction file suggests.
            let Some(recorded) = workspace_installed_adapters(options) else {
                return Ok(Some(other.display_name));
            };
            if recorded.iter().any(|slug| slug == other.slug) {
                return Ok(Some(other.display_name));
            }
            continue;
        }
        if read_pending(&theirs.instructions, pending)?
            .is_some_and(|text| DIRECTIVE_FENCE.is_present(&text))
        {
            return Ok(Some(other.display_name));
        }
    }
    Ok(None)
}

/// What a read of an agent's instruction file found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Installed {
    Configured,
    Absent,
    Unknown(String),
}

/// Whether this agent carries Estigia's directive, with "could not look" kept
/// apart from "looked and it is not there".
pub(crate) fn read_configured(adapter: &AgentAdapter, options: &SetupOptions) -> Installed {
    if options.scope == InstallScope::Workspace {
        return installed_from(
            workspace_installed_adapters(options)
                .is_some_and(|recorded| recorded.iter().any(|slug| slug == adapter.slug)),
        );
    }
    let paths = match resolve_paths(adapter, options) {
        Ok(paths) => paths,
        Err(error) => return Installed::Unknown(format!("{error:#}")),
    };
    match fs::read_to_string(&paths.instructions) {
        Ok(text) => installed_from(DIRECTIVE_FENCE.is_present(&text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Installed::Absent,
        Err(error) => Installed::Unknown(format!("{}: {error}", paths.instructions.display())),
    }
}

fn installed_from(present: bool) -> Installed {
    if present {
        Installed::Configured
    } else {
        Installed::Absent
    }
}

/// Whether this agent already carries Estigia's directive.
///
/// Read-only, and quiet about failure: an unreadable or absent instruction file
/// is an agent that is not configured, which is exactly what the caller asked.
/// A caller that must tell the two apart reads `read_configured`.
pub fn is_configured(adapter: &AgentAdapter, options: &SetupOptions) -> bool {
    read_configured(adapter, options) == Installed::Configured
}

/// What a later step in this run should read.
///
/// Two write steps can land on one file, and under `--dry-run` nothing was
/// written — so the plan carries the memory the disk would have had. `--dry-run`
/// reporting what the real run does is one of the things setup promises. `None`
/// records a file this run discarded.
fn read_pending(
    path: &Path,
    pending: &BTreeMap<PathBuf, Option<String>>,
) -> Result<Option<String>> {
    match pending.get(path) {
        Some(remembered) => Ok(remembered.clone()),
        None => read_optional(path),
    }
}

/// Writes the skill and the directive for one agent.
pub fn setup(
    adapter: &AgentAdapter,
    config: &Config,
    options: &SetupOptions,
) -> Result<SetupResult> {
    setup_into(adapter, config, options, &mut Pending::new())
}

/// What earlier steps of this run have already written, or would have.
pub use crate::skill::Pending;

/// [`setup`], remembering what the rest of this run already did.
///
/// Without the memory, `--all --dry-run` planned to create the same shared
/// skill files once per adapter: the plan said 182 where the run does 70 — and
/// `--dry-run` exists to be believed before anything happens.
pub fn setup_into(
    adapter: &AgentAdapter,
    config: &Config,
    options: &SetupOptions,
    pending: &mut Pending,
) -> Result<SetupResult> {
    setup_into_evidenced(adapter, config, options, pending).map_err(|failure| failure.error)
}

/// [`setup_into`], retaining partial action and write-attempt evidence on error.
pub(crate) fn setup_into_evidenced(
    adapter: &AgentAdapter,
    config: &Config,
    options: &SetupOptions,
    pending: &mut Pending,
) -> std::result::Result<SetupResult, SetupFailure> {
    refuse_retired(adapter, options)?;
    // A workspace adapter with no verified destination is refused here rather
    // than recorded and left: a record naming an install that cannot land pins
    // machine state behind a name nothing can take back.
    resolve_paths(adapter, options)
        .map_err(|refusal| SetupFailure::before_mutation(adapter.slug, options.dry_run, refusal))?;
    // Before the adapter's own files: a record naming the checkout only once
    // they have landed cannot be written if that write fails, and an install a
    // later uninstall cannot see is one nothing takes back. Nothing but the
    // checkout directory itself is created before the record.
    note_workspace_install(options, adapter.slug)
        .map_err(|error| SetupFailure::before_mutation(adapter.slug, options.dry_run, error))?;
    root_recorded_once_written(
        options,
        install_into_evidenced(adapter, config, options, pending),
    )
}

fn refuse_retired(
    adapter: &AgentAdapter,
    options: &SetupOptions,
) -> std::result::Result<(), SetupFailure> {
    if adapter.retired() {
        return Err(SetupFailure::before_mutation(
            adapter.slug,
            options.dry_run,
            retired_agent(adapter).into(),
        ));
    }
    Ok(())
}

fn install_into_evidenced(
    adapter: &AgentAdapter,
    config: &Config,
    options: &SetupOptions,
    pending: &mut Pending,
) -> std::result::Result<SetupResult, SetupFailure> {
    let stopped = |error| SetupFailure::before_mutation(adapter.slug, options.dry_run, error);
    let paths = resolve_paths(adapter, options).map_err(stopped)?;
    // From here on, a failure carries `paths.notes` — the same substitution
    // report `resolve_paths` itself would hand a caller that stopped there.
    let stopped_after_resolution = |error| {
        let mut failure = SetupFailure::before_mutation(adapter.slug, options.dry_run, error);
        failure.result.notes = paths.notes.clone();
        failure
    };
    let here = std::env::current_dir().ok();
    let (layers, folded) =
        skill::config_layers_for_install(&paths.skill_root, Some(adapter.slug), here.as_deref())
            .map_err(|error| stopped_after_resolution(error.into()))?;
    let effective = layers
        .effective_over(config)
        .map_err(|error| stopped_after_resolution(error.into()))?;
    let mut outcome = install_adapter_into(adapter, config, &effective, options, pending, true);
    if let Some(report) = folded {
        match &mut outcome {
            Ok(result) => result.notes.push(report.message),
            Err(failure) => failure.result.notes.push(report.message),
        }
    }
    outcome
}

/// Sets up one adapter from a portable contract and its effective host view.
///
/// A previous adapter may own the shared skill-tree action. Host artifacts are
/// always derived from `effective`, even when `portable` deliberately excludes
/// an agent, local, or repository override.
///
/// Refuses a retired adapter before writing anything: every install path
/// reaches the host through here.
pub fn setup_adapter_into(
    adapter: &AgentAdapter,
    portable: &Config,
    effective: &Config,
    options: &SetupOptions,
    pending: &mut Pending,
    install_skill: bool,
) -> std::result::Result<SetupResult, SetupFailure> {
    refuse_retired(adapter, options)?;
    // Refused before it is recorded, for the reason `setup_into_evidenced`
    // states, and recorded before its files, for the same one.
    resolve_paths(adapter, options)
        .map_err(|refusal| SetupFailure::before_mutation(adapter.slug, options.dry_run, refusal))?;
    note_workspace_install(options, adapter.slug)
        .map_err(|error| SetupFailure::before_mutation(adapter.slug, options.dry_run, error))?;
    root_recorded_once_written(
        options,
        install_adapter_into(
            adapter,
            portable,
            effective,
            options,
            pending,
            install_skill,
        ),
    )
}

fn install_adapter_into(
    adapter: &AgentAdapter,
    portable: &Config,
    effective: &Config,
    options: &SetupOptions,
    pending: &mut Pending,
    install_skill: bool,
) -> std::result::Result<SetupResult, SetupFailure> {
    #[cfg(debug_assertions)]
    let skip_internal_preview = preview_skipped();
    #[cfg(not(debug_assertions))]
    let skip_internal_preview = false;
    let preview = if options.dry_run || skip_internal_preview {
        None
    } else {
        let preview = SetupOptions {
            dry_run: true,
            ..options.clone()
        };
        let mut preview_pending = pending.clone();
        let preview_result = setup_into_with_skill(
            adapter,
            portable,
            effective,
            &preview,
            &mut preview_pending,
            install_skill,
            false,
        )
        .map_err(|mut failure| {
            // This is validation for a real invocation, not the dry-run the
            // validator used internally. No planned action is real evidence.
            failure.result.dry_run = false;
            failure.result.actions.clear();
            failure.write_attempted = false;
            failure
        })?;
        Some((preview_result, preview_pending))
    };
    let result = setup_into_with_skill(
        adapter,
        portable,
        effective,
        options,
        pending,
        install_skill,
        true,
    );
    match (result, preview) {
        (Err(mut failure), Some((planned, expected))) => {
            for action in planned.actions {
                if failure
                    .result
                    .actions
                    .iter()
                    .any(|proven| proven.kind == action.kind && proven.path == action.path)
                    || matches!(
                        action.change,
                        Change::Unchanged | Change::Kept | Change::Shared | Change::Unrecorded
                    )
                {
                    continue;
                }
                let proved = match expected.get(&action.path) {
                    Some(Some(text)) => {
                        fs::read_to_string(&action.path).is_ok_and(|written| written == *text)
                    }
                    Some(None) => !action.path.exists(),
                    None => false,
                };
                if proved {
                    failure.result.actions.push(action);
                }
            }
            Err(failure)
        }
        (result, _) => result,
    }
}

fn setup_into_with_skill(
    adapter: &AgentAdapter,
    portable: &Config,
    effective: &Config,
    options: &SetupOptions,
    pending: &mut Pending,
    install_skill: bool,
    inject_failures: bool,
) -> std::result::Result<SetupResult, SetupFailure> {
    let phase = if !inject_failures {
        SetupFailurePhase::Prevalidation
    } else if options.dry_run {
        SetupFailurePhase::Preview
    } else {
        SetupFailurePhase::Mutation
    };
    let actions = Vec::new();
    // Empty until `resolve_paths` answers below; every failure ahead of that
    // point has nothing to report.
    let mut notes: Vec<String> = Vec::new();
    macro_rules! step {
        ($operation:expr) => {
            match $operation {
                Ok(value) => value,
                Err(error) => {
                    return Err(SetupFailure {
                        error: error.into(),
                        result: SetupResult {
                            agent: adapter.slug,
                            dry_run: options.dry_run,
                            actions,
                            completed: false,
                            notes: notes.clone(),
                        },
                        write_attempted: false,
                        phase,
                    });
                }
            }
        };
    }
    macro_rules! write_step {
        ($operation:expr) => {
            match $operation {
                Ok(value) => value,
                Err(error) => {
                    return Err(SetupFailure {
                        error: error.into(),
                        result: SetupResult {
                            agent: adapter.slug,
                            dry_run: options.dry_run,
                            actions,
                            completed: false,
                            notes: notes.clone(),
                        },
                        write_attempted: !options.dry_run,
                        phase,
                    });
                }
            }
        };
    }
    macro_rules! boundary {
        ($boundary:expr) => {
            if inject_failures {
                step!(injected_setup_failure(adapter, $boundary));
            } else {
                step!(injected_setup_prevalidation_failure(adapter, $boundary));
            }
        };
    }

    let paths = step!(resolve_paths(adapter, options));
    notes = paths.notes.clone();
    // These are the external definitions whose names Estigia reserves. Decide
    // ownership and drift before the skill, directive, hooks, or any other
    // setup artifact can move. Reviewers are Claude Code's dialect; an adapter
    // with an agent-definitions root that is not Claude Code has none to
    // validate.
    if adapter.instructions == InstructionFile::ClaudeCode {
        for (reviewer_name, _, _) in crate::harness::roles::RESERVED_REVIEWERS {
            if !crate::harness::roles::binary_launched_reviewer(reviewer_name) {
                step!(validate_reviewer_definition(&paths, reviewer_name));
            }
        }
    }
    step!(validate_delegated_definitions(&paths, effective));
    boundary!(SetupFailureBoundary::BeforeSkill);
    let mut actions = if install_skill {
        write_step!(skill::install_into(
            &paths.skill_root,
            portable,
            options.dry_run,
            pending
        ))
        .actions
        .into_iter()
        .map(|action| SetupAction {
            kind: ActionKind::Skill,
            path: action.path,
            change: action.change,
        })
        .collect::<Vec<_>>()
    } else {
        vec![SetupAction {
            kind: ActionKind::Skill,
            path: paths.skill_root.join(skill::CONTRACT),
            change: Change::Shared,
        }]
    };
    boundary!(SetupFailureBoundary::AfterSkill);

    if !options.skip_directive {
        let existing = step!(read_pending(&paths.instructions, pending));
        let desired = DIRECTIVE_FENCE.upsert(
            existing.as_deref().unwrap_or(""),
            &directive_for(adapter, &paths),
        );
        actions.push(write_step!(write_file(
            &paths.instructions,
            existing.as_deref(),
            &desired,
            ActionKind::Directive,
            options.dry_run,
        )));
        // Remembered now because it cannot be worked out later: afterwards, a
        // file holding nothing but the directive block is the same bytes whether
        // this run made it or the operator kept it empty.
        if existing.is_none() && !options.dry_run {
            step!(skill::record::note_created_outside(
                &paths.skill_root,
                &paths.instructions
            ));
        }
        pending.insert(
            paths.instructions.clone(),
            Some(as_the_file_was(existing.as_deref(), &desired)),
        );
    }
    boundary!(SetupFailureBoundary::AfterDirective);

    if adapter.instructions == InstructionFile::ClaudeCode {
        for (reviewer_name, seal, template) in crate::harness::roles::RESERVED_REVIEWERS
            .iter()
            .copied()
            .filter(|(name, _, _)| !crate::harness::roles::binary_launched_reviewer(name))
        {
            let Some(target) = reviewer_target(&paths, reviewer_name) else {
                continue;
            };
            step!(injected_reviewer_definition_create_collision(&target));
            let existing = step!(read_pending(&target, pending));
            // The first preflight protects every earlier setup artifact; this one
            // protects the external definition from appearing while those writes
            // happen. The no-replace create below closes the remaining window
            // rather than trusting this second observation as a lock.
            step!(validate_reviewer_definition(&paths, reviewer_name));
            // Ownership lands before the first byte at the external path, so a
            // failed create leaves an owned absent target exact replay can finish.
            let mut ownership_added = false;
            if existing.is_none() && !skill::record::created_outside(&paths.skill_root, &target) {
                let record_path = skill::record::path(&paths.skill_root);
                let record_change = if skill::record::exists(&paths.skill_root) {
                    Change::Update
                } else {
                    Change::Create
                };
                if !options.dry_run {
                    step!(skill::record::note_created_outside(
                        &paths.skill_root,
                        &target
                    ));
                    ownership_added = true;
                }
                // Fresh skill installation already names this path. Keep its
                // `Create`; an upgrade gains one `Update`, never a duplicate.
                if let Some(action) = actions.iter_mut().find(|action| action.path == record_path) {
                    debug_assert_eq!(action.kind, ActionKind::Skill);
                    if action.change != Change::Create {
                        action.change = record_change;
                    }
                } else {
                    actions.push(SetupAction {
                        kind: ActionKind::Skill,
                        path: record_path,
                        change: record_change,
                    });
                }
            }
            // One rendering, handed to the write and to the record of what that
            // write will leave behind — the copy is gone rather than guarded, so
            // the two cannot disagree.
            let desired = render_reviewer_agent(template, effective.evidence, seal, reviewer_name);
            actions.push(write_step!(write_reviewer_definition(
                &paths.skill_root,
                &target,
                existing.as_deref(),
                options.dry_run,
                ownership_added,
                &desired,
            )));
            pending.insert(target, Some(as_the_file_was(existing.as_deref(), &desired)));
        }
        for reviewer_name in crate::harness::roles::BINARY_LAUNCHED_REVIEWERS {
            if let Some(action) = write_step!(retire_reviewer(
                &paths,
                pending,
                reviewer_name,
                options.dry_run
            )) {
                actions.push(action);
            }
        }
    }

    // The retired SDD phase definitions: written by a build that shipped the
    // protocol, and taken back out now that no phase is ever installed. Only
    // what this install created — another harness ships these names too, and
    // overwriting somebody's file never made it ours to delete.
    if adapter.instructions == InstructionFile::ClaudeCode
        && let Some(agents_root) = paths.agents_root.as_ref()
    {
        for name in skill::RETIRED_PHASE_AGENTS {
            let target = agents_root.join(name);
            if step!(read_pending(&target, pending)).is_none()
                || !skill::record::created_outside(&paths.skill_root, &target)
            {
                continue;
            }
            actions.push(write_step!(discard(
                &target,
                ActionKind::PhaseAgent,
                options.dry_run
            )));
            pending.insert(target.clone(), None);
            if !options.dry_run {
                step!(skill::record::forget_outside(&paths.skill_root, &target));
            }
        }
    }

    // The delegated roles, whose switch is the routing.
    // Named or not named, and nothing in between: writing these whenever an
    // agents root exists would put a definition carrying `Write`, `Edit` and
    // `Bash` into somebody's home because they upgraded, and an agent
    // definition is authority. Naming the role in `Model routing` is the
    // consent. Written for every adapter with a root, each in its own dialect.
    if let Some(agents_root) = paths.agents_root.as_ref() {
        for (target_key, file) in skill::DELEGATED_AGENTS {
            let Some(name) = file.path.strip_prefix("agents/") else {
                continue;
            };
            let target = agents_root.join(name);
            let existing = step!(read_pending(&target, pending));
            if !effective.models.installs(target_key) {
                if existing.is_none() || !skill::record::created_outside(&paths.skill_root, &target)
                {
                    continue;
                }
                actions.push(write_step!(discard(
                    &target,
                    ActionKind::DelegatedAgent,
                    options.dry_run
                )));
                pending.insert(target.clone(), None);
                if !options.dry_run {
                    step!(skill::record::forget_outside(&paths.skill_root, &target));
                }
                continue;
            }
            // Asked again, immediately before the write. The preflight decided
            // ownership before anything was written; a file appearing at this
            // path in between would be overwritten with no ownership recorded,
            // and `analyst` is a name another harness's orchestrator answers to.
            //
            // Not a lock and not claimed as one: it closes the window it sees.
            if existing.is_some() && !skill::record::created_outside(&paths.skill_root, &target) {
                step!(Err::<(), _>(delegated_definition_refusal(
                    target_key, &target
                )));
            }
            let Some(desired) =
                render_delegated_definition(adapter, file.contents, target_key, effective)
            else {
                continue;
            };
            actions.push(write_step!(write_file(
                &target,
                existing.as_deref(),
                &desired,
                ActionKind::DelegatedAgent,
                options.dry_run,
            )));
            pending.insert(
                target.clone(),
                Some(as_the_file_was(existing.as_deref(), &desired)),
            );
            if existing.is_none() && !options.dry_run {
                step!(skill::record::note_created_outside(
                    &paths.skill_root,
                    &target
                ));
            }
        }
    }

    // The install half of retirement: a definition that left the install lists
    // comes off on the way in, on the delegated loop's own ownership gate.
    if let Some(agents_root) = paths.agents_root.as_ref() {
        for name in skill::RETIRED_AGENTS {
            let target = agents_root.join(name);
            if step!(read_pending(&target, pending)).is_none()
                || !skill::record::created_outside(&paths.skill_root, &target)
            {
                continue;
            }
            actions.push(write_step!(discard(
                &target,
                ActionKind::DelegatedAgent,
                options.dry_run
            )));
            pending.insert(target.clone(), None);
            if !options.dry_run {
                step!(skill::record::forget_outside(&paths.skill_root, &target));
            }
        }
    }
    boundary!(SetupFailureBoundary::AfterDefinitions);

    if !options.skip_harness {
        let executable = step!(executable_path());
        let mut codex_trust: Option<(Vec<codex_trust::Entry>, Vec<String>)> = None;

        if let Some(hooks) = paths.hooks.as_ref()
            && let Some(spec) = adapter.gate_spec()
        {
            let existing = step!(read_pending(hooks, pending));
            let desired = match spec.envelope {
                Envelope::Cursor => step!(render_cursor_hooks(
                    hooks,
                    existing.as_deref(),
                    &executable,
                    adapter.slug,
                    spec,
                )),
                Envelope::Crush | Envelope::Windsurf => {
                    step!(Err::<String, _>(retired_agent(adapter)))
                }
                Envelope::Settings => {
                    step!(render_hooks(
                        hooks,
                        existing.as_deref(),
                        &executable,
                        adapter.slug,
                        spec,
                        (adapter.instructions == InstructionFile::Codex
                            || adapter.instructions == InstructionFile::ClaudeCode)
                            && effective.loop_plugin == LoopPlugin::On,
                    ))
                }
            };
            if adapter.instructions == InstructionFile::Codex {
                if let Some(config) = paths.mcp_config.as_ref() {
                    let held = step!(read_pending(config, pending));
                    step!(codex_trust::unmergeable(
                        config,
                        held.as_deref().unwrap_or("")
                    ));
                }
                codex_trust = Some((
                    codex_trust::owned(hooks, &desired),
                    existing
                        .as_deref()
                        .map(|text| {
                            codex_trust::owned(hooks, text)
                                .into_iter()
                                .map(|entry| entry.key)
                                .collect()
                        })
                        .unwrap_or_default(),
                ));
            }
            actions.push(write_step!(write_file(
                hooks,
                existing.as_deref(),
                &desired,
                ActionKind::Hooks,
                options.dry_run,
            )));
            pending.insert(
                hooks.clone(),
                Some(as_the_file_was(existing.as_deref(), &desired)),
            );
        }

        if let Some(file) = paths.plugin.as_ref() {
            let existing = step!(read_optional(file));
            // Somebody else's plugin under our name is reported unchanged, and
            // `status` says the gate is off.
            if existing.is_none() || plugin::is_ours(existing.as_deref()) {
                let desired = match adapter.instructions {
                    InstructionFile::Cline => plugin::cline_hook(
                        &executable,
                        options.platform.unwrap_or_else(Platform::current) == Platform::Windows,
                    ),
                    InstructionFile::Pi => plugin::pi_source(&executable),
                    _ => plugin::source(&executable),
                };
                actions.push(write_step!(write_file(
                    file,
                    existing.as_deref(),
                    &desired,
                    ActionKind::Plugin,
                    options.dry_run,
                )));
                pending.insert(
                    file.clone(),
                    Some(as_the_file_was(existing.as_deref(), &desired)),
                );
            }
        }

        // The review relay plugin: written for OpenCode and Pi on every install, like
        // the gate plugin and unlike the loop plugin. A host with no reviewer CLI
        // needs it to run a sealed reviewer at all, and a host with one never
        // reaches it, so there is no setting to gate it on. A file Estigia did
        // not write is left alone.
        if let Some(file) = review_plugin_path(adapter, &paths) {
            let existing = step!(read_optional(&file));
            if existing.is_none() || plugin::review_relay_is_ours(existing.as_deref()) {
                let desired = match adapter.instructions {
                    InstructionFile::Pi => plugin::pi_review_relay_source(&executable),
                    _ => plugin::review_relay_source(&executable),
                };
                actions.push(write_step!(write_file(
                    &file,
                    existing.as_deref(),
                    &desired,
                    ActionKind::ReviewPlugin,
                    options.dry_run,
                )));
                pending.insert(file, Some(as_the_file_was(existing.as_deref(), &desired)));
            }
        }

        // The loop plugin follows the `Loop plugin` setting: `on` writes it,
        // `off` takes back one Estigia wrote, and a file Estigia did not write
        // is left alone either way. The path only says where it lands.
        if let Some(file) = paths.loop_plugin.as_ref() {
            let existing = step!(read_optional(file));
            if effective.loop_plugin == LoopPlugin::On {
                if existing.is_none() || plugin::loop_is_ours(existing.as_deref()) {
                    let desired = match adapter.instructions {
                        InstructionFile::Pi => plugin::pi_loop_source(&executable),
                        _ => plugin::loop_source(&executable),
                    };
                    actions.push(write_step!(write_file(
                        file,
                        existing.as_deref(),
                        &desired,
                        ActionKind::LoopPlugin,
                        options.dry_run,
                    )));
                    pending.insert(
                        file.clone(),
                        Some(as_the_file_was(existing.as_deref(), &desired)),
                    );
                }
            } else if plugin::loop_is_ours(existing.as_deref()) {
                actions.push(step!(discard(
                    file,
                    ActionKind::LoopPlugin,
                    options.dry_run
                )));
            }

            let legacy = file.with_file_name(plugin::LEGACY_LOOP_FILE);
            let legacy_text = step!(read_optional(&legacy));
            if adapter.instructions == InstructionFile::OpenCode
                && plugin::loop_is_ours(legacy_text.as_deref())
            {
                actions.push(step!(discard(
                    &legacy,
                    ActionKind::LoopPlugin,
                    options.dry_run
                )));
            }
        }

        // The loop's slash commands follow the same `Loop plugin` setting, for
        // the adapter that reads them: `on` writes each, `off` takes back one
        // Estigia wrote, and a file Estigia did not write is left alone.
        if let Some(directory) = paths.commands.as_ref() {
            for (name, source) in plugin::LOOP_COMMANDS {
                let file = directory.join(name);
                let existing = step!(read_optional(&file));
                if effective.loop_plugin == LoopPlugin::On {
                    if existing.is_none() || plugin::command_is_ours(existing.as_deref()) {
                        actions.push(write_step!(write_file(
                            &file,
                            existing.as_deref(),
                            source,
                            ActionKind::Command,
                            options.dry_run,
                        )));
                        pending.insert(file, Some(as_the_file_was(existing.as_deref(), source)));
                    }
                } else if plugin::command_is_ours(existing.as_deref()) {
                    actions.push(step!(discard(&file, ActionKind::Command, options.dry_run)));
                }
            }
        }

        if let Some(directory) = paths.prompts.as_ref() {
            for (name, source) in plugin::PI_PROMPTS {
                let file = directory.join(name);
                let existing = step!(read_optional(&file));
                if existing.is_none() || plugin::prompt_is_ours(existing.as_deref()) {
                    actions.push(write_step!(write_file(
                        &file,
                        existing.as_deref(),
                        source,
                        ActionKind::PromptTemplate,
                        options.dry_run,
                    )));
                    pending.insert(file, Some(as_the_file_was(existing.as_deref(), source)));
                }
            }
        }

        if let Some((file, source)) = goal_loop_at(adapter, paths.loop_plugin.as_deref()) {
            let existing = step!(read_optional(&file));
            if effective.goal_loop_plugin == LoopPlugin::On {
                if existing.is_none() || plugin::goal_loop_is_ours(existing.as_deref()) {
                    actions.push(write_step!(write_file(
                        &file,
                        existing.as_deref(),
                        source,
                        ActionKind::GoalLoopPlugin,
                        options.dry_run,
                    )));
                    pending.insert(file, Some(as_the_file_was(existing.as_deref(), source)));
                }
            } else if plugin::goal_loop_is_ours(existing.as_deref()) {
                actions.push(step!(discard(
                    &file,
                    ActionKind::GoalLoopPlugin,
                    options.dry_run
                )));
            }
        }

        boundary!(SetupFailureBoundary::AfterHooks);

        if let Some(config) = paths.mcp_config.as_ref() {
            let existing = step!(read_pending(config, pending));
            let trusted = match codex_trust.as_ref() {
                Some((entries, stale)) => {
                    let text = existing.as_deref().unwrap_or("");
                    Some(step!(codex_trust::with_trust(config, text, stale, entries)))
                }
                None => None,
            };
            let desired = step!(render_mcp(
                config,
                trusted.as_deref().or(existing.as_deref()),
                &executable,
                adapter.mcp_format(),
            ));
            // The guard covers the MCP edit in the same file, not only the trust.
            if let Some((entries, _)) = codex_trust.as_ref() {
                step!(codex_trust::verify(config, &desired, entries));
            }
            if inject_failures
                && injected_setup_failure(adapter, SetupFailureBoundary::AtMcp).is_err()
            {
                return Err(SetupFailure {
                    error: anyhow::anyhow!("injected setup failure at AtMcp"),
                    result: SetupResult {
                        agent: adapter.slug,
                        dry_run: options.dry_run,
                        actions,
                        completed: false,
                        notes,
                    },
                    write_attempted: !options.dry_run,
                    phase,
                });
            }
            actions.push(write_step!(write_file(
                config,
                existing.as_deref(),
                &desired,
                ActionKind::McpServer,
                options.dry_run,
            )));
            pending.insert(
                config.clone(),
                Some(as_the_file_was(existing.as_deref(), &desired)),
            );
        }
    }

    Ok(SetupResult {
        agent: adapter.slug,
        dry_run: options.dry_run,
        actions,
        completed: true,
        notes,
    })
}

/// The definitions in `files` that this install created and that uninstall must
/// take back, resolving each one's target under `agents_root` and forgetting the
/// ownership in the same pass.
///
/// Shared by the Claude-only phases and the delegated workers so the ownership
/// rule has one body: only a file `created_outside` proves is Estigia's comes
/// out, and a file somebody else authored is left where it is.
fn relinquish_definitions<'a>(
    paths: &AgentPaths,
    pending: &mut Pending,
    agents_root: &Path,
    names: impl Iterator<Item = &'a str>,
    kind: ActionKind,
    dry_run: bool,
) -> Result<Vec<(PathBuf, ActionKind)>> {
    let mut removals = Vec::new();
    for name in names {
        let target = agents_root.join(name);
        if read_pending(&target, pending)?.is_none()
            || !skill::record::created_outside(&paths.skill_root, &target)
        {
            continue;
        }
        if !dry_run {
            skill::record::forget_outside(&paths.skill_root, &target)?;
        }
        removals.push((target, kind));
    }
    Ok(removals)
}

/// Removes Estigia from one agent, leaving everything else alone.
///
/// Invariant two. The instruction file keeps every other tool's block and the
/// operator's own prose; the skill directory keeps any file Estigia did not
/// write. A file that never mentioned Estigia is reported unchanged rather
/// than touched, and one that does not exist is not created just to say Estigia
/// is not in it.
pub fn uninstall(adapter: &AgentAdapter, options: &SetupOptions) -> Result<SetupResult> {
    uninstall_from(adapter, options, &mut Pending::new())
}

/// [`uninstall`], remembering what the rest of this run already took out: the
/// mirror of [`setup_into`], against the same measurement.
pub fn uninstall_from(
    adapter: &AgentAdapter,
    options: &SetupOptions,
    pending: &mut Pending,
) -> Result<SetupResult> {
    // A workspace install never wrote anything for an adapter with no
    // project-level destination — ZCode, DeepSeek Harness, and the retired
    // adapters — so a workspace uninstall has nothing to take back. Refusing
    // here would make `uninstall --all --scope=workspace` name eight adapters
    // that could not have been installed into the workspace at all.
    if let Some(workspace) = workspace_of(options)?
        && workspace_paths(adapter, &workspace).is_none()
    {
        return Ok(SetupResult {
            agent: adapter.slug,
            dry_run: options.dry_run,
            actions: Vec::new(),
            completed: true,
            notes: Vec::new(),
        });
    }

    // An unreadable record is an unknown, and an unknown is not clearance:
    // this refuses before anything is removed, rather than half-reversing a
    // checkout and failing at the record write on the way out.
    if options.scope == InstallScope::Workspace {
        // An unresolvable record is an unknown, and an unknown is not
        // clearance: this refuses before anything is removed, rather than
        // half-reversing a checkout and failing at the record write on the way
        // out.
        let Some(entry) = workspace_entry(options) else {
            return Err(anyhow::anyhow!(
                "resolving where this checkout's install is recorded"
            ));
        };
        read_workspace_adapters(&entry)?;
    }

    // And an adapter the record does not name was never installed here: the
    // shared instruction file and skill belong to the adapters it does name, so
    // this removal would otherwise strip a sibling's install.
    if options.scope == InstallScope::Workspace
        && let Some(recorded) = workspace_installed_adapters(options)
        && !recorded.iter().any(|slug| slug == adapter.slug)
    {
        return Ok(SetupResult {
            agent: adapter.slug,
            dry_run: options.dry_run,
            actions: Vec::new(),
            completed: true,
            notes: Vec::new(),
        });
    }

    let paths = resolve_paths(adapter, options)?;

    // Before the first removal, so an unparseable `config.toml` refuses with
    // nothing yet gone, and only when there is trust to remove.
    if adapter.instructions == InstructionFile::Codex
        && let Some(config) = paths.mcp_config.as_ref()
        && let Some(existing) = read_pending(config, pending)?
    {
        let has_trust = match paths.hooks.as_ref() {
            Some(hooks) => match read_pending(hooks, pending)? {
                Some(hooks_text) => !codex_trust::trusted_keys(hooks, &hooks_text).is_empty(),
                None => false,
            },
            None => false,
        };
        if has_trust {
            codex_trust::ensure_parseable(config, &existing)?;
        }
    }

    // Decided before the skill directory goes, because the record that answers
    // it lives *in* that directory. The ownership is released only after its
    // deletion succeeds below, keeping a failed reviewer deletion retryable.
    //
    // In a workspace the instruction file is shared, so a sibling the record
    // still names reads the same directive: it goes with the last adapter, not
    // the first.
    let directive = if shared_instructions_survive(adapter, options, &paths) {
        None
    } else {
        match read_pending(&paths.instructions, pending)? {
            Some(existing) => {
                let desired = DIRECTIVE_FENCE.remove(&existing);
                let ours = skill::record::created_outside(&paths.skill_root, &paths.instructions);
                let emptied = is_now_empty(&desired, ActionKind::Directive, &[], ours);
                Some((existing, desired, emptied))
            }
            None => None,
        }
    };

    // The reviewers are decided and removed before the skill ledger that
    // proves ownership. Changed text is the operator's work: keep it and
    // relinquish the path. Reviewers are Claude Code's dialect, so an adapter
    // with a root that is not Claude Code never received one.
    let mut reviewer_actions = Vec::new();
    if adapter.instructions == InstructionFile::ClaudeCode {
        for (reviewer_name, _, _) in crate::harness::roles::RESERVED_REVIEWERS {
            reviewer_actions.extend(retire_reviewer(
                &paths,
                pending,
                reviewer_name,
                options.dry_run,
            )?);
        }
    }

    // The phase and delegated definitions, taken back before the skill
    // directory goes, for the reason the directive is: the ownership record
    // lives *inside* that directory. Only the ones this install created —
    // overwriting somebody's file does not make it ours to delete. Phases are
    // Claude Code's dialect; the delegated workers are installed per dialect
    // and come out by the same ownership rule, `analyst` in particular being a
    // name somebody else's orchestrator answers to.
    let mut phase_removals: Vec<(PathBuf, ActionKind)> = Vec::new();
    if adapter.instructions == InstructionFile::ClaudeCode
        && let Some(agents_root) = paths.agents_root.as_ref()
    {
        phase_removals.extend(relinquish_definitions(
            &paths,
            pending,
            agents_root,
            skill::RETIRED_PHASE_AGENTS.iter().copied(),
            ActionKind::PhaseAgent,
            options.dry_run,
        )?);
    }
    if let Some(agents_root) = paths.agents_root.as_ref() {
        phase_removals.extend(relinquish_definitions(
            &paths,
            pending,
            agents_root,
            skill::DELEGATED_AGENTS
                .iter()
                .filter_map(|(_, file)| file.path.strip_prefix("agents/")),
            ActionKind::DelegatedAgent,
            options.dry_run,
        )?);
    }
    // And the definitions this crate shipped once and does not ship now: a name
    // that left the install list is reached by no walk of it.
    if let Some(agents_root) = paths.agents_root.as_ref() {
        phase_removals.extend(relinquish_definitions(
            &paths,
            pending,
            agents_root,
            skill::RETIRED_AGENTS.iter().copied(),
            ActionKind::DelegatedAgent,
            options.dry_run,
        )?);
    }

    // Most agents share the neutral skill root, so the skill only goes when
    // the last adapter on that root does.
    let removed = if skill_shared_with(adapter, options, pending)?.is_some() {
        skill::keep_in(&paths.skill_root)?
    } else {
        skill::uninstall_from(&paths.skill_root, options.dry_run, pending)?
    };
    let mut actions = reviewer_actions;
    actions.extend(removed.actions.into_iter().map(|action| SetupAction {
        kind: ActionKind::Skill,
        path: action.path,
        change: action.change,
    }));

    // This adapter's own answers, if `config set --agent` ever wrote them.
    // Not one of `FILES` — written by a different command, so it outlived an
    // uninstall that took everything else.
    let own = skill::agent_override(&paths.skill_root, adapter.slug);
    if read_pending(&own, pending)?.is_some() {
        actions.push(discard(&own, ActionKind::Skill, options.dry_run)?);
        pending.insert(own, None);
    }

    for (target, kind) in phase_removals {
        actions.push(discard(&target, kind, options.dry_run)?);
        pending.insert(target, None);
    }

    if let Some((existing, desired, emptied)) = directive {
        actions.push(if emptied {
            let action = discard(&paths.instructions, ActionKind::Directive, options.dry_run)?;
            if !options.dry_run {
                skill::record::forget_outside(&paths.skill_root, &paths.instructions)?;
            }
            action
        } else {
            write_file(
                &paths.instructions,
                Some(&existing),
                &desired,
                ActionKind::Directive,
                options.dry_run,
            )?
        });
        pending.insert(
            paths.instructions.clone(),
            if emptied { None } else { Some(desired) },
        );
        // Remembered whether or not it was written: under `--dry-run` nothing
        // reaches the disk, and the next adapter of an `--all` run must not
        // read a directive this run has already taken out.
    }

    let mut trusted_keys: Vec<String> = Vec::new();
    if let Some(hooks) = paths.hooks.as_ref()
        && let Some(existing) = read_pending(hooks, pending)?
    {
        if adapter.instructions == InstructionFile::Codex {
            trusted_keys = codex_trust::trusted_keys(hooks, &existing);
        }
        let desired = match adapter.gate_spec().map(|spec| spec.envelope) {
            Some(Envelope::Cursor | Envelope::Crush | Envelope::Windsurf) => {
                strip_cursor_hooks(hooks, &existing)?
            }
            _ => strip_hooks(hooks, &existing)?,
        };
        let scaffolding = adapter
            .gate_spec()
            .map_or(&[][..], |spec| spec.envelope.scaffolding());
        let desired = without_scaffolding(&desired, scaffolding);
        actions.push(
            if is_now_empty(&desired, ActionKind::Hooks, scaffolding, true) {
                pending.insert(hooks.clone(), None);
                discard(hooks, ActionKind::Hooks, options.dry_run)?
            } else {
                pending.insert(
                    hooks.clone(),
                    Some(as_the_file_was(Some(&existing), &desired)),
                );
                write_file(
                    hooks,
                    Some(&existing),
                    &desired,
                    ActionKind::Hooks,
                    options.dry_run,
                )?
            },
        );
    }

    // A plain uninstall takes everything Estigia wrote, gate plugin included.
    if let Some(file) = paths.plugin.as_ref()
        && let Some(existing) = read_optional(file)?
        && plugin::is_ours(Some(&existing))
    {
        actions.push(discard(file, ActionKind::Plugin, options.dry_run)?);
    }

    // And the review relay plugin: a file left behind that shells out to an
    // uninstalled binary would be a dangling file, not an uninstall. A file
    // Estigia did not write is neither removed nor reported.
    if let Some(file) = review_plugin_path(adapter, &paths)
        && let Some(existing) = read_optional(&file)?
        && plugin::review_relay_is_ours(Some(&existing))
    {
        actions.push(discard(&file, ActionKind::ReviewPlugin, options.dry_run)?);
    }

    // The loop plugin too, whether or not this install was the one that asked
    // for it: leaving a plugin that shells out to an uninstalled binary is a
    // dangling file, not an uninstall. A file Estigia did not write is neither
    // removed nor reported.
    //
    // Read through `paths`, never a freshly resolved environment: the loop
    // plugin is scope-derived like every other destination, and recomputing it
    // here would leave a workspace copy behind and take the machine's instead.
    if let Some(file) = paths.loop_plugin.as_ref()
        && let Some(existing) = read_optional(file)?
        && plugin::loop_is_ours(Some(&existing))
    {
        actions.push(discard(file, ActionKind::LoopPlugin, options.dry_run)?);
    }

    if let Some(file) = paths
        .loop_plugin
        .as_ref()
        .filter(|_| adapter.instructions == InstructionFile::OpenCode)
        .map(|file| file.with_file_name(plugin::LEGACY_LOOP_FILE))
        && let Some(existing) = read_optional(&file)?
        && plugin::loop_is_ours(Some(&existing))
    {
        actions.push(discard(&file, ActionKind::LoopPlugin, options.dry_run)?);
    }

    // The loop's slash commands too, whether or not this install asked for
    // them: a command left behind would call a binary an uninstall removed.
    if let Some(directory) = paths.commands.as_ref() {
        for (name, _) in plugin::LOOP_COMMANDS {
            let file = directory.join(name);
            if let Some(existing) = read_optional(&file)?
                && plugin::command_is_ours(Some(&existing))
            {
                actions.push(discard(&file, ActionKind::Command, options.dry_run)?);
            }
        }
    }

    if let Some(directory) = paths.prompts.as_ref() {
        for (name, _) in plugin::PI_PROMPTS {
            let file = directory.join(name);
            if let Some(existing) = read_optional(&file)?
                && plugin::prompt_is_ours(Some(&existing))
            {
                actions.push(discard(&file, ActionKind::PromptTemplate, options.dry_run)?);
            }
        }
    }

    if let Some((file, _)) = goal_loop_at(adapter, paths.loop_plugin.as_deref())
        && let Some(existing) = read_optional(&file)?
        && plugin::goal_loop_is_ours(Some(&existing))
    {
        actions.push(discard(&file, ActionKind::GoalLoopPlugin, options.dry_run)?);
    }

    if let Some(config) = paths.mcp_config.as_ref()
        && let Some(existing) = read_pending(config, pending)?
    {
        let untrusted = codex_trust::without_trust(config, &existing, &trusted_keys)?;
        let desired = strip_mcp(config, &untrusted, adapter.mcp_format())?;
        // An unchanged result is the operator's own empty containers, not a husk
        // this uninstall left: only a file this step changed may be discarded.
        if desired != existing {
            actions.push(
                if is_now_empty(&desired, ActionKind::McpServer, &[], true) {
                    discard(config, ActionKind::McpServer, options.dry_run)?
                } else {
                    write_file(
                        config,
                        Some(&existing),
                        &desired,
                        ActionKind::McpServer,
                        options.dry_run,
                    )?
                },
            );
        }
    }

    forget_workspace_install(options, adapter.slug)?;
    forget_global_install_root(options)?;

    Ok(SetupResult {
        agent: adapter.slug,
        dry_run: options.dry_run,
        actions,
        completed: true,
        // Shares `resolve_paths` with `setup_into_with_skill`, so a
        // substitution is reported on the way out exactly as on the way in.
        notes: paths.notes.clone(),
    })
}

/// Whether what is left after Estigia's part comes out holds nothing at all.
///
/// The sharper half of invariant two: lifting our block out of a file that
/// **only ever held our block** must not leave an empty husk behind. Emptiness
/// is judged after the removal, not by remembering who created the file.
/// Takes back the keys Estigia wrote around its own entries, once they are gone.
///
/// Only when nothing of Estigia's is left: while an entry of its own is still
/// there, the scaffolding is still holding it up. And only when the
/// scaffolding is *all* that is left — `version` is Cursor's own required
/// field, and an operator who kept hooks of their own had it before Estigia
/// was installed.
fn without_scaffolding(desired: &str, scaffolding: &[&str]) -> String {
    // Not skipped when there is no scaffolding: an emptied `hooks` is Estigia's
    // whatever the envelope carries around it.
    // Through the one reader, like the four in `render`; the parse failure is
    // not an error here, it is a shrug.
    let Ok(mut root) = serde_json::from_str::<Value>(render::without_mark(desired)) else {
        return desired.to_owned();
    };
    let before = root.clone();
    let Some(object) = root.as_object_mut() else {
        return desired.to_owned();
    };
    // An emptied `hooks` says nothing and was not there before.
    if object
        .get("hooks")
        .and_then(Value::as_object)
        .is_some_and(serde_json::Map::is_empty)
    {
        object.remove("hooks");
    }
    // Only once nothing of Estigia's is left, and only when the scaffolding is
    // *all* that is left — otherwise an operator's own keys are in there too.
    if !object.contains_key("hooks") && object.keys().all(|key| scaffolding.contains(&key.as_str()))
    {
        for key in scaffolding {
            object.remove(*key);
        }
    }
    // Nothing was Estigia's, so nothing is rewritten — invariant two.
    if root == before {
        return desired.to_owned();
    }
    // Their indentation, not this crate's taste.
    render::as_written_public(Some(desired), &root).unwrap_or_else(|_| desired.to_owned())
}

/// Whether what is left is Estigia's to take the file away with.
///
/// `ours` answers the question the disk cannot: did this install create the
/// file. It decides the directive alone, because that is the only file where an
/// emptied result is ambiguous. Asking the record rather than choosing a side
/// is the point — never deleting leaves an empty file on a clean uninstall;
/// always deleting takes a file that was the operator's.
fn is_now_empty(desired: &str, kind: ActionKind, scaffolding: &[&str], ours: bool) -> bool {
    if desired.trim().is_empty() {
        return kind != ActionKind::Directive || ours;
    }
    if kind == ActionKind::Directive {
        return false;
    }
    render::parse_settings(desired).is_some_and(|root| match root {
        Value::Object(map) => map.iter().all(|(key, value)| match value {
            _ if scaffolding.contains(&key.as_str()) => true,
            Value::Object(inner) => inner.is_empty(),
            Value::Array(inner) => inner.is_empty(),
            Value::Null => true,
            _ => false,
        }),
        _ => false,
    })
}

/// Removes a file Estigia's part was all of, and the directory if that emptied.
///
/// The parent is only removed when it comes out empty, so a directory holding
/// anybody else's file survives.
fn discard(path: &Path, kind: ActionKind, dry_run: bool) -> Result<SetupAction> {
    if !dry_run {
        #[cfg(test)]
        if kind == ActionKind::AgentDefinition
            && REVIEWER_DEFINITION_REMOVAL_FAILURE.with(|injected| injected.replace(false))
        {
            anyhow::bail!("injected reviewer-definition removal failure");
        }
        fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
    Ok(SetupAction {
        kind,
        path: path.to_owned(),
        change: Change::Remove,
    })
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

/// The rendered text, given back the shape the file already had.
///
/// Every renderer here emits `\n` and finishes with one — right for a file this
/// crate creates, wrong for one it is only adding a block to. The same promise
/// as the indentation and the byte-order mark: what this crate did not put in
/// the file comes back as it was.
///
/// A function rather than four lines inside the writer because **what goes
/// into `pending` has to be what goes onto disk** — one run writes Gemini's
/// `settings.json` twice, and the second pass reads the first pass's text.
fn as_the_file_was(existing: Option<&str>, desired: &str) -> String {
    let mut desired = match existing {
        Some(found) if found.contains("\r\n") => desired.replace('\n', "\r\n"),
        _ => desired.to_owned(),
    };
    // And the last one, when the file did not end with one.
    if existing.is_some_and(|found| !found.ends_with('\n')) {
        while desired.ends_with('\n') || desired.ends_with('\r') {
            desired.pop();
        }
    }
    desired
}

fn write_file(
    path: &Path,
    existing: Option<&str>,
    desired: &str,
    kind: ActionKind,
    dry_run: bool,
) -> Result<SetupAction> {
    // Against the bytes that would go on disk, not against `desired`:
    // [`as_the_file_was`] gives back the line endings and final newline of the
    // file it found, so a byte-for-byte identical rewrite is `Unchanged`
    // rather than an `Update` that rewrites the operator's settings on every
    // run.
    let shaped = existing.map(|found| as_the_file_was(Some(found), desired));
    let change = match (existing, shaped.as_deref()) {
        (None, _) => Change::Create,
        (Some(found), Some(shaped)) if found == shaped => Change::Unchanged,
        _ => Change::Update,
    };
    if !dry_run && change != Change::Unchanged {
        #[cfg(test)]
        if kind == ActionKind::AgentDefinition
            && REVIEWER_DEFINITION_WRITE_FAILURE.with(|injected| injected.replace(false))
        {
            anyhow::bail!("injected reviewer-definition write failure");
        }
        let parent = path
            .parent()
            .with_context(|| format!("{} has no parent directory", path.display()))?;
        fs::create_dir_all(parent)
            .with_context(|| format!("create directory {}", parent.display()))?;
        let desired = shaped.unwrap_or_else(|| desired.to_owned());
        paths::replace_atomically(path, &desired)
            .with_context(|| format!("write {}", path.display()))?;
        // Re-derived from disk, not reported from the plan above: the write
        // just verified what landed, so read it rather than restating what was
        // decided. A `Create` that found the file already holding the bytes is
        // still a `Create` — the file did not exist when this call looked —
        // but an `Update` that landed byte-identical is `Unchanged`.
        let landed =
            fs::read_to_string(path).with_context(|| format!("read back {}", path.display()))?;
        let change = match existing {
            None => Change::Create,
            Some(found) if found == landed => Change::Unchanged,
            Some(_) => Change::Update,
        };
        return Ok(SetupAction {
            kind,
            path: path.to_owned(),
            change,
        });
    }
    Ok(SetupAction {
        kind,
        path: path.to_owned(),
        change,
    })
}

/// Records the digest of a reviewer definition on every real write, including
/// one that changed nothing — the ledger question is about the *last* thing
/// this crate put there, and a record that stopped at the first install would
/// call every upgrade an operator's own work. The skill root's own
/// `note_written` call reasons the same way, in `src/skill.rs`: "a record that
/// stopped at the install would call every file an upgrade touched somebody's
/// work."
fn record_reviewer_write(skill_root: &Path, path: &Path, landed: &str) -> Result<()> {
    skill::record::note_written_outside(skill_root, path, skill::record::digest_of(landed))
        .with_context(|| {
            format!(
                "record the reviewer definition written at {}",
                path.display()
            )
        })
}

fn write_reviewer_definition(
    skill_root: &Path,
    path: &Path,
    existing: Option<&str>,
    dry_run: bool,
    ownership_added: bool,
    desired: &str,
) -> Result<SetupAction> {
    if existing.is_some() || dry_run {
        let action = write_file(
            path,
            existing,
            desired,
            ActionKind::AgentDefinition,
            dry_run,
        )?;
        // What the write actually left on disk, not the pre-shape `desired`:
        // `write_file` reshapes to the existing file's line endings and final
        // newline before it lands, and the digest this crate reads back later
        // is compared against bytes read straight off disk.
        if !dry_run {
            record_reviewer_write(skill_root, path, &as_the_file_was(existing, desired))?;
        }
        return Ok(action);
    }

    #[cfg(test)]
    if REVIEWER_DEFINITION_WRITE_FAILURE.with(|injected| injected.replace(false)) {
        anyhow::bail!("injected reviewer-definition write failure");
    }
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("create directory {}", parent.display()))?;
    match paths::create_atomically(path, desired) {
        Ok(()) => {
            record_reviewer_write(skill_root, path, desired)?;
            Ok(SetupAction {
                kind: ActionKind::AgentDefinition,
                path: path.to_owned(),
                change: Change::Create,
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if ownership_added {
                skill::record::forget_outside(skill_root, path).with_context(|| {
                    format!(
                        "forget ownership after {} appeared during reviewer creation",
                        path.display()
                    )
                })?;
            }
            Err(reviewer_definition_refusal(
                ReviewerDefinitionCode::Unowned,
                path,
                "appeared after preflight and was preserved rather than replaced",
            )
            .into())
        }
        Err(error) => Err(error).with_context(|| format!("create {}", path.display())),
    }
}

/// Characters no quoting makes safe in every shell a hook command may meet.
///
/// Each one measured rather than reasoned about, by handing `sh` the very
/// string a hook carries:
///
/// ```text
/// "C:\Users\a$b\estigia.exe"      -> C:\Users\a\estigia.exe    the path, truncated
/// "C:\Users\O'Brien\estigia.exe"  -> unchanged                 so `'` is not here
/// ```
///
/// `$` and a backtick are command substitution and `${…}` is expansion, all of
/// which still happen inside double quotes. `"` closes the quoting. `%` is
/// `cmd`'s own expansion, and `cmd` is a shell Estigia cannot rule out: single
/// quoting would have handled the first three and `cmd //c` answers *el nombre
/// de archivo … no son correctos* to a single-quoted command, so there is no
/// quoting that covers both — which leaves refusing the character.
const UNQUOTABLE: &[char] = &['$', '`', '"', '%'];

/// Whether a shell would read this path back as itself.
///
/// **Pure and fed**, because the path that matters is `current_exe()` and the
/// paths worth checking are the ones this machine does not have.
pub fn quotable(path: &Path) -> Result<(), Refusal> {
    let shown = path.display().to_string();
    if let Some(bad) = shown.chars().find(|c| UNQUOTABLE.contains(c)) {
        return Err(Refusal::not_started(
            "executable-path-not-quotable",
            format!(
                "{shown} holds a `{bad}`, which a shell reads out of the path itself \u{2014} a \
                 hook built around it runs something else and the gate never fires"
            ),
            // Spelled out rather than joined from the list: collecting them
            // gives ``$`"%``, where the backtick closes the quoting around the
            // others and an operator reads a line of punctuation.
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "Estigia installed somewhere without `$`, a backtick, `\"` or `%` in the path",
            ),
        ));
    }
    // A UNC prefix, for the same reason one character along: `sh` reads the
    // pair as one escaped backslash, so `\\server\share\estigia.exe` arrives as
    // `\server\share\estigia.exe` and there is nothing there. Measured the same
    // way as the list above.
    if shown.starts_with("\\\\") {
        return Err(Refusal::not_started(
            "executable-path-not-quotable",
            format!(
                "{shown} is a UNC path, and a shell reads `\\\\` as one backslash \u{2014} the \
                 hook would look for the gate at a path that is not there"
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "Estigia installed on a local path, or the share mapped to a drive letter",
            ),
        ));
    }
    Ok(())
}

/// The executable to write into hook commands.
fn executable_path() -> Result<PathBuf> {
    let executable = env::current_exe().context("resolve the Estigia executable")?;
    // Canonicalization resolves symlinks; on Windows it also returns a `\\?\`
    // verbatim path that shells refuse to execute.
    let executable =
        paths::remove_windows_verbatim_prefix(executable.canonicalize().unwrap_or(executable));
    // Refused here, before anything is written: a hook installed around a path
    // a shell rereads is a gate that is registered, reported installed, and
    // pointing somewhere else — the one state this crate exists to refuse. It
    // is knowable from the path alone, which is where this crate settles it.
    quotable(&executable)?;
    Ok(executable)
}

/// Where a workspace install records itself: one file per checkout under
/// `<home>/.estigia/workspaces`.
fn workspace_registry(options: &SetupOptions) -> Result<PathBuf> {
    // `state_root` is `<home>/.estigia/runs`; the record sits beside it, not
    // inside the run-pointer directory.
    crate::harness::session::state_root(options.home_dir.as_deref())
        .map(|runs| runs.with_file_name("workspaces"))
        .map_err(|refusal| anyhow::anyhow!("{refusal:?}"))
}

/// The record's name, keyed by the checkout's canonical path so a trailing
/// separator names the same record.
fn workspace_entry(options: &SetupOptions) -> Option<PathBuf> {
    let workspace = options.workspace.as_ref()?;
    let canonical = fs::canonicalize(workspace).unwrap_or_else(|_| workspace.clone());
    let name = crate::skill::record::digest_of(&canonical.to_string_lossy());
    workspace_registry(options)
        .ok()
        .map(|directory| directory.join(format!("{name}.json")))
}

/// The adapters a record names. `Ok(None)` for no record; `Err` for a record
/// that exists and cannot be read or parsed.
fn read_workspace_adapters(entry: &Path) -> Result<Option<Vec<String>>> {
    let text = match fs::read_to_string(entry) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("parsing the workspace install record {}", entry.display()))?;
    let list = value
        .get("adapters")
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("reading the adapters of {}", entry.display()))?;
    let mut adapters = Vec::with_capacity(list.len());
    for slug in list {
        // A record this build can read but not vouch for is an unknown, not an
        // install that names nothing.
        let slug = slug
            .as_str()
            .with_context(|| format!("reading the adapters of {}", entry.display()))?;
        adapters.push(slug.to_owned());
    }
    Ok(Some(adapters))
}

/// The adapters this checkout's record names, or `None` for a record that is
/// absent or unreadable — an unknown, which every reader treats as one.
fn workspace_installed_adapters(options: &SetupOptions) -> Option<Vec<String>> {
    let entry = workspace_entry(options)?;
    read_workspace_adapters(&entry).ok().flatten()
}

/// Whether a recorded sibling reads the same instruction file, so the directive
/// in it is still its pointer. An unreadable record keeps the file.
fn shared_instructions_survive(
    adapter: &AgentAdapter,
    options: &SetupOptions,
    mine: &AgentPaths,
) -> bool {
    if options.scope != InstallScope::Workspace {
        return false;
    }
    let Some(recorded) = workspace_installed_adapters(options) else {
        return true;
    };
    recorded.iter().any(|slug| {
        slug != adapter.slug
            && find_agent(slug)
                .ok()
                .and_then(|other| resolve_paths(other, options).ok())
                .is_some_and(|theirs| theirs.instructions == mine.instructions)
    })
}

/// Writes the record for `options.workspace`: the checkout, and the adapters
/// installed in it.
fn write_workspace_record(options: &SetupOptions, adapters: &[String]) -> Result<()> {
    let directory = workspace_registry(options)
        .context("resolving where a workspace install records itself")?;
    let entry = workspace_entry(options).context("naming a workspace install's record")?;
    fs::create_dir_all(&directory)?;
    // The same path the record's name is keyed on: `any_workspace_install`
    // decides liveness from this field, so the spelling the operator typed —
    // through a symlinked parent, say — must not be what it resolves.
    let canonical = options
        .workspace
        .as_ref()
        .map(|workspace| fs::canonicalize(workspace).unwrap_or_else(|_| workspace.clone()));
    let body = serde_json::json!({
        "workspace": canonical.as_ref().map(|path| path.to_string_lossy().into_owned()),
        "adapters": adapters,
    });
    // A reader refuses a record it cannot parse, so the write must not be what
    // truncates one: the bytes land beside it and are renamed in.
    let staged = entry.with_extension("json.new");
    fs::write(&staged, serde_json::to_vec(&body)?)?;
    match fs::rename(&staged, &entry) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&staged);
            Err(error.into())
        }
    }
}

/// Records that `options.workspace` carries an install of `slug`, on the machine.
fn note_workspace_install(options: &SetupOptions, slug: &str) -> Result<()> {
    if options.scope != InstallScope::Workspace || options.dry_run {
        return Ok(());
    }
    // The record's name is the checkout's canonical path, and nothing has been
    // written yet, so a checkout that does not exist has to exist before the
    // name is taken or the way back computes a different one.
    if let Some(workspace) = options.workspace.as_ref() {
        fs::create_dir_all(workspace)
            .with_context(|| format!("creating the workspace {}", workspace.display()))?;
    }
    // A workspace install that cannot record itself is one a machine-wide
    // uninstall cannot see: a failed install, not a silent one.
    let Some(entry) = workspace_entry(options) else {
        return Err(anyhow::anyhow!("naming a workspace install's record"));
    };
    let mut adapters = read_workspace_adapters(&entry)?.unwrap_or_default();
    if !adapters.iter().any(|known| known == slug) {
        adapters.push(slug.to_owned());
        adapters.sort();
    }
    write_workspace_record(options, &adapters)
}

/// Drops `slug` from the record, and the record with the last adapter.
fn forget_workspace_install(options: &SetupOptions, slug: &str) -> Result<()> {
    if options.scope != InstallScope::Workspace || options.dry_run {
        return Ok(());
    }
    let Some(entry) = workspace_entry(options) else {
        return Err(anyhow::anyhow!("naming a workspace install's record"));
    };
    let Some(mut adapters) = read_workspace_adapters(&entry)? else {
        return Ok(());
    };
    if !adapters.iter().any(|known| known == slug) {
        return Ok(());
    }
    adapters.retain(|known| known != slug);
    if !adapters.is_empty() {
        return write_workspace_record(options, &adapters);
    }
    match fs::remove_file(&entry) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// Whether any workspace install is recorded on this machine: a record whose
/// checkout is present and that names an adapter. A registry that cannot be read
/// is an unknown, and an unknown is not clearance.
fn any_workspace_install(options: &SetupOptions) -> bool {
    let registry = match workspace_registry(options) {
        Ok(registry) => registry,
        // An unresolvable registry is an unknown, and an unknown is not
        // clearance.
        Err(_) => return true,
    };
    let records = match fs::read_dir(&registry) {
        Ok(records) => records,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        Err(_) => return true,
    };
    for record in records {
        let Ok(record) = record else {
            return true;
        };
        // Not every entry is a record: Finder writes `.DS_Store`, an editor a
        // swap file. A stray file is not an install.
        if record.path().extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(record.path()) else {
            return true;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            return true;
        };
        let Some(workspace) = value.get("workspace").and_then(serde_json::Value::as_str) else {
            return true;
        };
        // Genuinely absent is not the same as unreadable: only `NotFound` says
        // the checkout is gone.
        match fs::metadata(workspace) {
            Ok(metadata) if metadata.is_dir() => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(_) | Err(_) => return true,
        }
        // A record whose shape this build does not know is an unknown, and an
        // unknown is not clearance; one that names an empty list is a record
        // whose install has gone, and the checkout directory outlived it.
        let Some(adapters) = value.get("adapters").and_then(serde_json::Value::as_array) else {
            return true;
        };
        if !adapters.is_empty() {
            return true;
        }
    }
    false
}

/// Where a global install records the configuration roots it was made under:
/// one file per distinct set of roots under `<home>/.estigia/roots`.
fn install_root_registry(options: &SetupOptions) -> Result<PathBuf> {
    crate::harness::session::state_root(options.home_dir.as_deref())
        .map(|runs| runs.with_file_name("roots"))
        .map_err(|refusal| anyhow::anyhow!("{refusal:?}"))
}

/// The record for the roots `options` resolves, and its body.
fn install_root_entry(options: &SetupOptions) -> Result<(PathBuf, Vec<u8>)> {
    let environment = Environment::resolve(options)?;
    let body = serde_json::to_vec(&serde_json::json!({
        "claude_config_dir": environment.claude_config().to_string_lossy(),
        "pi_agent_dir": environment.pi_agent().to_string_lossy(),
        "config_home": environment.xdg_config().to_string_lossy(),
        "app_data": environment.roaming().to_string_lossy(),
    }))?;
    let name = crate::skill::record::digest_of(&String::from_utf8_lossy(&body));
    Ok((
        install_root_registry(options)?.join(format!("{name}.json")),
        body,
    ))
}

/// Records the roots once an install has begun to write, so a prevalidation
/// refusal leaves nothing behind and a partial write is still found.
fn root_recorded_once_written(
    options: &SetupOptions,
    outcome: std::result::Result<SetupResult, SetupFailure>,
) -> std::result::Result<SetupResult, SetupFailure> {
    let wrote = match &outcome {
        Ok(_) => !options.dry_run,
        Err(failure) => failure.phase == SetupFailurePhase::Mutation,
    };
    if !wrote {
        return outcome;
    }
    match note_global_install_root(options) {
        Ok(()) => outcome,
        Err(error) => {
            let result = match outcome {
                Ok(result) => result,
                Err(failure) => failure.result,
            };
            Err(SetupFailure {
                error,
                result,
                write_attempted: true,
                phase: SetupFailurePhase::Mutation,
            })
        }
    }
}

/// Records the roots a global install resolves, so an uninstall under another
/// profile can still find this one.
fn note_global_install_root(options: &SetupOptions) -> Result<()> {
    if options.scope != InstallScope::Global || options.dry_run {
        return Ok(());
    }
    let (entry, body) = install_root_entry(options)?;
    if entry.is_file() {
        return Ok(());
    }
    if let Some(directory) = entry.parent() {
        fs::create_dir_all(directory)?;
    }
    let staged = entry.with_extension(format!(
        "{}.{:?}.new",
        std::process::id(),
        std::thread::current().id()
    ));
    fs::write(&staged, body)?;
    fs::rename(&staged, &entry).inspect_err(|_| {
        let _ = fs::remove_file(&staged);
    })?;
    Ok(())
}

/// Drops the record of these roots once no adapter is left under them.
fn forget_global_install_root(options: &SetupOptions) -> Result<()> {
    if options.scope != InstallScope::Global || options.dry_run {
        return Ok(());
    }
    if every_adapter().any(|adapter| is_present(adapter, options)) {
        return Ok(());
    }
    let (entry, _) = install_root_entry(options)?;
    match fs::remove_file(&entry) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    if let Some(directory) = entry.parent() {
        let _ = fs::remove_dir(directory);
    }
    Ok(())
}

/// The root records on this machine, for a caller that reports the ones a run
/// created or dropped.
pub fn install_root_records(options: &SetupOptions) -> Vec<PathBuf> {
    let Ok(registry) = install_root_registry(options) else {
        return Vec::new();
    };
    let mut records: Vec<PathBuf> = fs::read_dir(registry)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "json"))
        .collect();
    records.sort();
    records
}

/// What the recorded roots say about the shared state: whether one still holds
/// an install, and which records could not be read.
fn recorded_install_roots(options: &SetupOptions) -> (bool, Vec<PathBuf>) {
    let Ok(registry) = install_root_registry(options) else {
        return (true, Vec::new());
    };
    let records = match fs::read_dir(&registry) {
        Ok(records) => records,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (false, Vec::new()),
        Err(_) => return (true, vec![registry]),
    };
    let mut installed = false;
    let mut unreadable = Vec::new();
    for record in records {
        let Ok(record) = record else {
            installed = true;
            continue;
        };
        let path = record.path();
        if path.extension().and_then(std::ffi::OsStr::to_str) != Some("json") {
            continue;
        }
        let root = |value: &serde_json::Value, key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
        };
        let parsed = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| {
                Some(SetupOptions {
                    scope: InstallScope::Global,
                    workspace: None,
                    claude_config_dir: Some(root(&value, "claude_config_dir")?),
                    pi_agent_dir: value
                        .get("pi_agent_dir")
                        .and_then(serde_json::Value::as_str)
                        .map(PathBuf::from),
                    config_home: Some(root(&value, "config_home")?),
                    app_data: Some(root(&value, "app_data")?),
                    ..options.clone()
                })
            });
        match parsed {
            Some(recorded) => {
                installed |= every_adapter().any(|adapter| is_present(adapter, &recorded));
            }
            None => {
                installed = true;
                unreadable.push(path);
            }
        }
    }
    unreadable.sort();
    (installed, unreadable)
}

/// The root records that could not be read, each one a reason
/// [`forget_state`] kept the state.
pub fn unreadable_install_roots(options: &SetupOptions) -> Vec<PathBuf> {
    recorded_install_roots(options).1
}

/// Estigia's own state, once no agent is left to read it.
///
/// Only when nothing is left: the state is machine-wide, and one `uninstall
/// codex` on a machine that still runs Claude Code must not take the other
/// one's claims with it. Files by name and directories with `remove_dir`,
/// never `remove_dir_all`: if an operator put something of their own in here,
/// it keeps the directory.
pub fn forget_state(options: &SetupOptions) -> Vec<PathBuf> {
    // The state is machine-wide, so an install in **either** scope keeps it: a
    // global uninstall cannot see the checkouts, and consults their record.
    let machine = SetupOptions {
        scope: InstallScope::Global,
        workspace: None,
        ..options.clone()
    };
    if every_adapter().any(|adapter| is_present(adapter, options))
        || every_adapter().any(|adapter| is_present(adapter, &machine))
        || any_workspace_install(&machine)
        || recorded_install_roots(options).0
    {
        return Vec::new();
    }
    let Ok(runs) = crate::harness::session::state_root(options.home_dir.as_deref()) else {
        return Vec::new();
    };
    let mut taken = Vec::new();
    let mut remove = |path: PathBuf| {
        if !path.is_file() {
            return;
        }
        if options.dry_run || fs::remove_file(&path).is_ok() {
            taken.push(path);
        }
    };

    // Every run pointer, by the extension they are written with.
    if let Ok(entries) = fs::read_dir(&runs) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|kind| kind == "json") {
                remove(path);
            }
        }
    }
    remove(crate::harness::session::ledger_path(&runs));
    remove(crate::harness::session::roles_path(&runs));
    // The store holds the decision history and the workflow state that has no
    // GitHub object; removing it is the operator's recorded requirement for the
    // last agent, not a rebuild.
    remove(crate::transport::store::beside(&runs));
    remove(crate::transport::store::beside(&runs).with_extension("db-wal"));
    remove(crate::transport::store::beside(&runs).with_extension("db-shm"));
    // The store's snapshots, by name; the directory goes below only if empty.
    let snapshots = crate::transport::store::snapshots_dir(&crate::transport::store::beside(&runs));
    if let Ok(entries) = fs::read_dir(&snapshots) {
        for entry in entries.flatten() {
            remove(entry.path());
        }
    }
    remove(crate::harness::standdown::path(&runs));
    remove(crate::harness::standdown::legacy_path(&runs));
    for record in install_root_records(options) {
        remove(record);
    }
    // Every stop-after request, by the extension they are written with, under
    // its own directory beside `runs/`.
    if let Ok(entries) = fs::read_dir(crate::harness::session::stop_after_dir(&runs)) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|kind| kind == "json") {
                remove(path);
            }
        }
    }
    // The screen's remembered language: Estigia writes it, it lives in
    // Estigia's own directory, and it means nothing without the screen.
    // Through `preference_path`, not by joining a name here, and resolved the
    // same way `words::remembered` resolves it — `home_dir` is `None` in a
    // real run, so a `Some`-guarded removal worked in the suite and did
    // nothing on a machine.
    if let Some(path) = crate::tui::words::preference_path_for(options.home_dir.as_deref()) {
        remove(path);
    }

    if !options.dry_run {
        // Empty only. Anything else in here is not Estigia's to take.
        let _ = fs::remove_dir(&snapshots);
        if let Ok(registry) = install_root_registry(options) {
            let _ = fs::remove_dir(registry);
        }
        let _ = fs::remove_dir(crate::harness::session::stop_after_dir(&runs));
        let _ = fs::remove_dir(&runs);
        if let Some(parent) = runs.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
    taken
}

/// A refusal for a caller that named no agent and no `--all`.
pub fn no_agent_named() -> Refusal {
    let supported = AGENTS
        .iter()
        .map(|adapter| adapter.slug)
        .collect::<Vec<_>>()
        .join(", ");
    Refusal::not_started(
        "agent-not-named",
        "setup needs to know which agent to configure; the screen is `estigia tui`",
        Resolution::run(format!(
            "estigia setup --all   # or estigia tui / one of: {supported}"
        )),
    )
}

/// Writes one adapter's own table, creating the file if it is not there yet.
///
/// Unlike [`rewrite_configuration`] this may create: the contract is installed
/// by `setup` and its absence means nothing is installed, but an adapter's own
/// file exists only once somebody sets something for that adapter. Refusing
/// because it is missing would make the first `config set --agent` impossible.
///
/// It carries a header saying what it is and which adapter reads it. A bare
/// table in a shared directory is a file whose owner nobody can work out, and
/// this one sits beside the operator's own.
pub fn write_agent_configuration(
    file: &Path,
    slug: &str,
    config: &Config,
    speaks_for: &[crate::config::Setting],
) -> Result<(), Refusal> {
    let snapshot = agent_configuration_snapshot(file)?;
    write_agent_configuration_from_snapshot(&snapshot, slug, config, speaks_for)
}

/// One fail-closed read of an agent override document.
#[derive(Debug, Clone)]
pub struct AgentConfigurationSnapshot {
    path: PathBuf,
    document: Option<String>,
}

impl AgentConfigurationSnapshot {
    /// Binds a path to text an earlier layer read, without reopening the file.
    pub(crate) fn from_document(path: PathBuf, document: Option<String>) -> Self {
        Self { path, document }
    }

    /// The exact text read, or `None` only when the file did not exist.
    pub fn document(&self) -> Option<&str> {
        self.document.as_deref()
    }

    /// Agent-scoped rows explicitly owned by this exact document.
    pub fn settings(&self) -> Vec<crate::config::Setting> {
        self.document
            .as_deref()
            .map(crate::config::table_rows)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(label, _)| crate::config::Setting::from_label(&label))
            .filter(|setting| setting.scope() == crate::config::Scope::Agent)
            .fold(Vec::new(), |mut settings, setting| {
                if !settings.contains(&setting) {
                    settings.push(setting);
                }
                settings
            })
    }
}

/// Reads an agent override, treating only `NotFound` as an empty snapshot.
pub fn agent_configuration_snapshot(file: &Path) -> Result<AgentConfigurationSnapshot, Refusal> {
    let document = read_optional(file).map_err(|error| {
        Refusal::not_started(
            "config-local-unreadable",
            format!("could not read {}: {error}", file.display()),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "that file readable, or moved aside if those overrides are no longer wanted",
            ),
        )
    })?;
    Ok(AgentConfigurationSnapshot {
        path: file.to_owned(),
        document,
    })
}

/// The settings table's own marked block, refreshed for an OpenCode override
/// that already exists — plus, only for `opencode` and `pi`, the review section
/// `fresh_agent_file` writes on a first install. Both go through
/// [`crate::fence::Fence::upsert`] rather than one; `CONFIG_FENCE` owns only
/// the settings table, and an upgrade that touched only that fence left the
/// review section exactly as whichever binary first wrote it, on every host
/// that installed one before the panel size it named stopped being one
/// `review_tier` states.
fn upgraded_agent_body(
    existing: &str,
    slug: &str,
    config: &Config,
    speaks_for: &[crate::config::Setting],
) -> String {
    // `upsert` replaces the marked block when it is there and appends it
    // when it is not, so both cases are one call and neither loses what the
    // operator wrote around it.
    let updated = crate::config::CONFIG_FENCE.upsert(
        existing,
        config.render_some_agent_rows(speaks_for).trim_end(),
    );
    let Some((fence, section)) = host_review_section(slug) else {
        return updated;
    };
    // A file an earlier binary wrote carries this heading with nothing
    // marking its span, so `OPENCODE_REVIEW_FENCE.find` cannot locate it and
    // upserting over it would append a second "Review on this host" section
    // rather than replace the first. Strip the unmarked one, by the one
    // anchor stable across both this version's own section and every
    // earlier one — the heading itself — before the fenced upsert runs.
    const STALE_HEADING: &str = "## Review on this host";
    let base =
        if slug == "opencode" && updated.contains(STALE_HEADING) && !fence.is_present(&updated) {
            let cut = updated.find(STALE_HEADING).unwrap_or(updated.len());
            let section_end = updated[cut..]
                .find("\n## ")
                .map_or(updated.len(), |offset| cut + offset + 1);
            let before = updated[..cut].trim_end();
            let after = updated[section_end..].trim_start();
            if after.is_empty() {
                before.to_owned()
            } else {
                format!("{before}\n\n{after}")
            }
        } else {
            updated
        };
    fence.upsert(&base, &section)
}

/// The rows of `speaks_for` that give `slug`'s adapter anything to mean.
///
/// The one place a row that only some adapters act on is kept out of a file
/// somebody else's screen never offered it for: every writer of an agent
/// override goes through here, so `config edit --agent qwen` cannot persist a
/// `Loop plugin` row the qwen screen does not draw.
fn speaks_for_this_agent(
    slug: &str,
    speaks_for: &[crate::config::Setting],
) -> Result<Vec<crate::config::Setting>, Refusal> {
    let adapter = find_agent(slug)?;
    Ok(speaks_for
        .iter()
        .copied()
        .filter(|setting| setting.applies_to_agent(adapter))
        .collect())
}

/// Writes an agent override from the same snapshot used to discover ownership.
pub fn write_agent_configuration_from_snapshot(
    snapshot: &AgentConfigurationSnapshot,
    slug: &str,
    config: &Config,
    speaks_for: &[crate::config::Setting],
) -> Result<(), Refusal> {
    let speaks_for = speaks_for_this_agent(slug, speaks_for)?;
    let body = match snapshot.document.as_deref() {
        Some(existing) => upgraded_agent_body(existing, slug, config, &speaks_for),
        None => fresh_agent_file(slug, config, &speaks_for),
    };
    crate::paths::replace_atomically(&snapshot.path, &body).map_err(|error| {
        Refusal::not_started(
            "agent-configuration-unwritable",
            format!("could not write {}: {error}", snapshot.path.display()),
            Resolution::run("estigia doctor"),
        )
    })
}

/// Plans or writes one adapter override through the batch's pending manifest.
pub fn write_agent_configuration_into(
    file: &Path,
    slug: &str,
    config: &Config,
    speaks_for: &[crate::config::Setting],
    dry_run: bool,
    pending: &mut Pending,
) -> Result<SetupAction, Refusal> {
    let snapshot = match pending.get(file) {
        Some(document) => {
            AgentConfigurationSnapshot::from_document(file.to_owned(), document.clone())
        }
        None => agent_configuration_snapshot(file)?,
    };
    write_agent_configuration_snapshot_into(&snapshot, slug, config, speaks_for, dry_run, pending)
}

/// Plans or writes from the same snapshot used to discover owned rows.
pub fn write_agent_configuration_snapshot_into(
    snapshot: &AgentConfigurationSnapshot,
    slug: &str,
    config: &Config,
    speaks_for: &[crate::config::Setting],
    dry_run: bool,
    pending: &mut Pending,
) -> Result<SetupAction, Refusal> {
    let speaks_for = speaks_for_this_agent(slug, speaks_for)?;
    let body = match snapshot.document.as_deref() {
        Some(existing) => upgraded_agent_body(existing, slug, config, &speaks_for),
        None => fresh_agent_file(slug, config, &speaks_for),
    };
    let action = write_file(
        &snapshot.path,
        snapshot.document.as_deref(),
        &body,
        ActionKind::AgentConfiguration,
        dry_run,
    )
    .map_err(|error| {
        Refusal::not_started(
            "agent-configuration-unwritable",
            format!("could not write {}: {error}", snapshot.path.display()),
            Resolution::run("estigia doctor"),
        )
    })?;
    pending.insert(
        snapshot.path.clone(),
        Some(as_the_file_was(snapshot.document.as_deref(), &body)),
    );
    Ok(action)
}

/// Writes what a repository says about itself, where it says it.
///
/// The mirror of [`write_agent_configuration`], and it keeps the same two
/// rules: `upsert` so whatever the operator wrote around the block survives,
/// and an atomic replace so nothing reads half a file.
///
/// A missing file may be created only by a caller explicitly establishing that
/// repository layer, such as `config set --repo`. Ordinary setup calls this
/// only after finding an existing layer, so it does not migrate contract rows.
/// Any other read failure refuses before the existing bytes can be replaced.
pub fn write_repository_configuration(
    file: &Path,
    config: &Config,
    speaks_for: &[crate::config::Setting],
) -> Result<(), Refusal> {
    let existing = read_optional(file).map_err(|error| {
        Refusal::not_started(
            "config-local-unreadable",
            format!("could not read {}: {error}", file.display()),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "that file readable, or moved aside if those repository overrides are no longer wanted",
            ),
        )
    })?;
    let body = crate::config::CONFIG_FENCE.upsert(
        existing.as_deref().unwrap_or_default(),
        config.render_some_repository_rows(speaks_for).trim_end(),
    );
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    crate::paths::replace_atomically(file, &body).map_err(|error| {
        Refusal::not_started(
            "repository-configuration-unwritable",
            format!("could not write {}: {error}", file.display()),
            Resolution::run("estigia doctor"),
        )
    })
}

/// Plans or writes repository rows through the same manifest as setup.
pub fn write_repository_configuration_into(
    file: &Path,
    config: &Config,
    speaks_for: &[crate::config::Setting],
    dry_run: bool,
    pending: &mut Pending,
) -> Result<SetupAction, Refusal> {
    let existing = read_pending(file, pending).map_err(|error| {
        Refusal::not_started(
            "config-local-unreadable",
            format!("could not read {}: {error}", file.display()),
            Resolution::run("estigia doctor"),
        )
    })?;
    let body = crate::config::CONFIG_FENCE.upsert(
        existing.as_deref().unwrap_or_default(),
        config.render_some_repository_rows(speaks_for).trim_end(),
    );
    let action = write_file(
        file,
        existing.as_deref(),
        &body,
        ActionKind::RepositoryConfiguration,
        dry_run,
    )
    .map_err(|error| {
        Refusal::not_started(
            "repository-configuration-unwritable",
            format!("could not write {}: {error}", file.display()),
            Resolution::run("estigia doctor"),
        )
    })?;
    pending.insert(
        file.to_owned(),
        Some(as_the_file_was(existing.as_deref(), &body)),
    );
    Ok(action)
}

/// The whole file, for an adapter that had none.
fn fresh_agent_file(slug: &str, config: &Config, speaks_for: &[crate::config::Setting]) -> String {
    let mut body = [
        format!("# Estigia configuration for `{slug}`"),
        String::new(),
        [
            "This adapter shares a skill directory with others, so its settings live here",
            "rather than in the contract. Rows here override the contract's table for this",
            "adapter only. `estigia.local.md`, if you have one, still overrides both.",
        ]
        .join(" "),
        String::new(),
        block(config, speaks_for),
    ]
    .join("\n");
    // OpenCode and Pi have no reserved review-blind type. The next clone reads
    // this file, not a machine-local note, so the ordinary path cannot stay "handoff".
    if let Some((fence, section)) = host_review_section(slug) {
        body.push('\n');
        body.push_str(&fence.upsert("", &section));
    }
    body
}

/// The span `upgraded_agent_body` refreshes on every write, fresh install and
/// upgrade alike — unlike `CONFIG_FENCE`, which only ever owns the settings
/// table.
const OPENCODE_REVIEW_FENCE: crate::fence::Fence = crate::fence::Fence {
    begin: "<!-- estigia:opencode-review:start -->",
    end: "<!-- estigia:opencode-review:end -->",
    superseded: &[],
};

const PI_REVIEW_FENCE: crate::fence::Fence = crate::fence::Fence {
    begin: "<!-- estigia:pi-review:start -->",
    end: "<!-- estigia:pi-review:end -->",
    superseded: &[],
};

/// The fence and review section an adapter's own file carries, for the hosts
/// that relay a sealed reviewer instead of launching a `review-blind` type.
fn host_review_section(slug: &str) -> Option<(crate::fence::Fence, String)> {
    match slug {
        "opencode" => Some((OPENCODE_REVIEW_FENCE, opencode_review_section())),
        "pi" => Some((PI_REVIEW_FENCE, pi_review_section())),
        _ => None,
    }
}

/// The panel-launch instructions this host reads in place of the reserved
/// `review-blind` type it does not have — restated in the contract's own
/// current terms (`skill/references/runtime-notes.md`, `skill/policies/blind-judges.md`)
/// rather than in a wording of its own that a later change
/// to those terms would leave behind. The binary, not a host `Task`, launches
/// the sealed lenses, so their counts are rendered from
/// `RiskLevel::lenses()` rather than typed. `pub` so a crossing
/// test can read the rendered surface directly.
pub fn opencode_review_section() -> String {
    review_section(
        "launch the reviewer with your own sub-agent tool naming the\n\
         reserved reviewer, and the OpenCode review relay plugin supplies the binary's prompt and\n\
         hands the answer back for admission, so the answer is never yours to read. With the plugin\n\
         installed, `launch_review_input` answers `relay: true` here even when a CLI is installed.",
    )
}

/// The same instructions for Pi, whose relay is a tool the extension registers
/// rather than a sub-agent tool it intercepts. `pub` for the same reason as
/// [`opencode_review_section`].
pub fn pi_review_section() -> String {
    review_section(&format!(
        "call the `{}` tool with the slot's name as `name`, and the Pi review relay\n\
         extension supplies the binary's prompt, runs the reviewer in process on the model the\n\
         `reviewer` routing row names (the session model when it is `inherit`), and hands the\n\
         answer back for admission, so the answer is never yours to read. A refusal the tool\n\
         returns is the outcome: ask `review_status` again.",
        plugin::PI_REVIEW_TOOL
    ))
}

fn review_section(relay: &str) -> String {
    use crate::transport::tier::RiskLevel;
    let high_lenses = RiskLevel::High.lenses_spelled();
    let medium_lenses = RiskLevel::Medium.lenses_spelled();
    let low_lenses = RiskLevel::Low.lenses_spelled();
    format!(
        "## Review on this host\n\n\
         `review-blind` is a Claude launch type. It is not a missing capability here.\n\
         The binary launches the panel. After `publish_review` and a publication lane that is green, never-ran, absent, disabled, or has\n\
         no lane at all \u{2014} not red, not unreadable \u{2014} call `launch_review_input` and then `collect_review_input` for each input\n\
         `review_status` offers (the sealed lenses, {medium_lenses} at `medium`, {high_lenses} at\n\
         `high`, {low_lenses} at `low`, the roles named by `review_tier`'s `lens_roles`; the refuter when a\n\
         standing `inferential` finding owes one; and for a correction, the validator). An input whose\n\
         `capture` is `null` has its reviewer still running: call nothing for it and ask `review_status`\n\
         again. Pass `names` to `launch_review_input` or `collect_review_input` to advance every offered input\n\
         in one call; each slot keeps its own claim check and a running slot is reported rather than\n\
         stopping the batch. Launch no `Task` for\n\
         any of them yourself: the binary starts the first installed `claude` or `codex` CLI with no\n\
         tools, which must be on your `PATH`. It records a lens's findings and then its report, the\n\
         refuter's batch, or the validator's result, itself; those records have no tool of their own,\n\
         so only `collect_review_input` writes them and a hand call cannot reach one. A\n\
         missing, out-of-quota or logged-out CLI refuses the capture and records nothing, and the\n\
         input stays offered. On a machine with neither CLI, `launch_review_input` answers `relay:\n\
         true` and starts no CLI: {relay} That is a\n\
         launch, not a handoff: `handoff_review` remains only for a host that can neither start a\n\
         CLI nor relay. When `review_status` offers none, call\n\
         `record_computed_verdict`, then `release_ci`. A receipt that stands escalated stays so until a\n\
         person runs `estigia recover`; tell them the command and never run it yourself.\n\
         Call `handoff_review` only if that launch failed, never as the first move and never a\n\
         smaller or serialized panel. A host that can call the tool and calls `handoff_review`\n\
         without launching first is a defect against this sentence.\n\
         The binary starts each reviewer in an empty scratch directory, and `claude` with\n\
         `--setting-sources \"\"`, so its starting context is the review contract, the receipt, the criteria and the\n\
         payload it was handed, and not the tracker binding or this host\'s AGENTS.md.\n\n{}",
        crate::skill::VERDICT_BASIS_REQUIREMENT
    )
}

/// The marked block of an adapter's own file, alone.
///
/// The rows that differ by agent, and only those — the same table the update
/// path writes. Two paths that build one file have to build the same file, or
/// which one ran last decides what an operator sees.
fn block(config: &Config, speaks_for: &[crate::config::Setting]) -> String {
    format!(
        "{}
{}
{}
",
        crate::config::BLOCK_BEGIN,
        config.render_some_agent_rows(speaks_for).trim_end(),
        crate::config::BLOCK_END
    )
}

/// Rewrites just the configuration block of an already-installed contract.
///
/// The cheap path for `estigia config set`: the contract on disk may be a
/// newer skill than this binary embeds, and replacing the whole tree to change
/// one table would silently downgrade it.
///
/// A contract that is not there is a refusal, not an `Unchanged`. Reporting
/// "already correct" for a file that does not exist is the same class of lie
/// the outcome taxonomy exists to prevent — the caller would print
/// *"Change size is now 400"* over a skill nobody installed.
pub fn rewrite_configuration(contract: &Path, config: &Config) -> Result<Change, Refusal> {
    let existing = match read_optional(contract) {
        Ok(Some(existing)) => existing,
        Ok(None) => {
            return Err(Refusal::not_started(
                "skill-not-installed",
                format!("{} does not exist", contract.display()),
                Resolution::run("estigia setup --all"),
            ));
        }
        Err(error) => return Err(unreadable_contract(contract, &error)),
    };
    // The same body `setup` writes, not a second one built here. This used to
    // be its own shorter `format!`, so `estigia config set` replaced the block
    // with a version missing the paragraph that names `estigia.local.md` — the
    // override mechanism left in the code and deleted from the text, which is
    // the exact failure `skill::configuration_body` is documented against.
    let body = crate::skill::configuration_body(config);
    let desired = CONFIG_FENCE.upsert(&existing, &body);
    let action = write_file(
        contract,
        Some(&existing),
        &desired,
        ActionKind::Skill,
        false,
    )
    .map_err(|error| unreadable_contract(contract, &error))?;
    Ok(action.change)
}

/// A refusal for a contract that exists and cannot be read or written.
///
/// `world-action` rather than a command: a permission bit, a full disk or a
/// file somebody else holds open is not something Estigia can name an
/// invocation for, and inventing one would be naming a dead end.
fn unreadable_contract(contract: &Path, error: &anyhow::Error) -> Refusal {
    Refusal::not_started(
        "contract-not-writable",
        format!("{}: {error}", contract.display()),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "read and write permission on the installed contract",
        ),
    )
}

#[cfg(test)]
pub(crate) mod tests;

/// Whether Estigia is in this agent at all, by any of the three ways it can be:
/// told, gated, tools. [`is_configured`] alone reads like that answer and is
/// not it.
pub fn is_present(adapter: &AgentAdapter, options: &SetupOptions) -> bool {
    is_configured(adapter, options) || is_gated(adapter, options) || exposes_tools(adapter, options)
}

/// The review relay plugin's path for an adapter that loads plugins, derived from
/// the gate plugin's own directory so both scopes resolve it the same way.
///
/// `None` for every adapter that declares no `review_relay` capability: a host
/// reads a relay only from the plugin directory its gate plugin already lives in.
pub fn review_plugin_path(adapter: &AgentAdapter, paths: &AgentPaths) -> Option<PathBuf> {
    let name = match adapter.instructions {
        InstructionFile::OpenCode => plugin::REVIEW_FILE,
        InstructionFile::Pi => plugin::PI_REVIEW_FILE,
        _ => return None,
    };
    paths.plugin.as_ref().map(|file| file.with_file_name(name))
}

/// Whether a review relay is installed on this machine, for any host that has one.
///
/// Read by the launch path: with it, a machine with neither reviewer CLI takes
/// the relay; without it, the same machine is refused with the coded
/// `unavailable` naming the CLI to install. Which host takes the relay ahead of an
/// installed CLI is [`crate::reviewer::select_reviewer`]'s rule.
pub fn review_relay_installed() -> bool {
    relay_installed_where(|_| true)
}

/// Whether OpenCode's review relay plugin is installed and ours.
pub fn opencode_review_relay_installed() -> bool {
    relay_installed_where(|adapter| matches!(adapter.instructions, InstructionFile::OpenCode))
}

fn relay_installed_where(host: impl Fn(&AgentAdapter) -> bool) -> bool {
    AGENTS
        .iter()
        .filter(|adapter| adapter.capabilities().review_relay && host(adapter))
        .any(|adapter| {
            let Ok(paths) = resolve_paths(adapter, &SetupOptions::default()) else {
                return false;
            };
            review_plugin_path(adapter, &paths)
                .and_then(|file| std::fs::read_to_string(file).ok())
                .is_some_and(|text| plugin::review_relay_is_ours(Some(&text)))
        })
}

/// Where this dialect's entry sits: `mcpServers.<name>` or `mcp.<name>` for
/// every dialect but ZCode, which nests a `servers` object under `mcp` first.
fn mcp_entry(root: &Value, format: McpFormat) -> Option<&Value> {
    if format == McpFormat::ZCode {
        return root.get("mcp")?.get("servers")?.get(SERVER_NAME);
    }
    ["mcpServers", "mcp"]
        .iter()
        .find_map(|key| root.get(key)?.get(SERVER_NAME))
}

/// Whether the registered server names the subcommand that starts one.
///
/// Quiet unless it is sure: a wrapper — `sh -c "estigia mcp"` — puts the
/// subcommand in an argument rather than beside it, so **any** argument
/// mentioning `mcp` is enough.
pub fn tools_start_the_server(adapter: &AgentAdapter, options: &SetupOptions) -> Option<bool> {
    let paths = resolve_paths(adapter, options).ok()?;
    let text = fs::read_to_string(paths.mcp_config?).ok()?;
    if adapter.mcp_format() == McpFormat::CodexToml {
        // The TOML half writes `args = ["mcp"]` under the same table.
        return Some(render::estigia_table_block(&text)?.contains("\"mcp\""));
    }
    let root = render::parse_settings(&text)?;
    let entry = mcp_entry(&root, adapter.mcp_format())?;
    Some(says_mcp(entry.get("args")?))
}

/// Whether an `args` value says how to start the server.
///
/// **Pure and fed**, so the readings can be exercised without a settings file.
pub fn says_mcp(args: &Value) -> bool {
    let mut said = Vec::new();
    collect_words(args, &mut said);
    said.iter().any(|word| word.contains("mcp"))
}

fn collect_words(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::String(text) => into.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_words(item, into);
            }
        }
        _ => {}
    }
}

/// The program this agent's tool server entry names, if it has one.
///
/// Read rather than assumed: *installed, looks installed, enforces nothing*.
pub fn tools_command(adapter: &AgentAdapter, options: &SetupOptions) -> Option<PathBuf> {
    let paths = resolve_paths(adapter, options).ok()?;
    let text = fs::read_to_string(paths.mcp_config?).ok()?;
    match adapter.mcp_format() {
        McpFormat::CodexToml => {
            // `[mcp_servers.estigia]` and the `command = "..."` under it, up to
            // the next table. Read by hand: this answers a `status` question and
            // never writes.
            let block = render::estigia_table_block(&text)?;
            block.lines().find_map(|line| {
                let value = line.trim().strip_prefix("command")?.trim_start();
                let value = value.strip_prefix('=')?.trim();
                // Unescaped, not merely unquoted: a TOML basic string writes a
                // Windows path as `C:\\Users\\…`, which no operator would
                // recognise as theirs.
                Some(PathBuf::from(
                    value
                        .trim_matches('"')
                        .replace("\\\\", "\\")
                        .replace("\\\"", "\""),
                ))
            })
        }
        format => {
            let root = render::parse_settings(&text)?;
            let command = mcp_entry(&root, format)?.get("command")?;
            // OpenCode joins the executable and its arguments into one array;
            // every other JSON format writes the executable alone.
            let named = match command {
                Value::String(text) => text.as_str(),
                Value::Array(items) => items.first()?.as_str()?,
                _ => return None,
            };
            Some(PathBuf::from(named))
        }
    }
}

/// Whether the lifecycle gate is registered for this agent.
///
/// A separate query from [`is_configured`]: one says the agent was told about
/// Estigia, the other says Estigia can stop it.
pub fn is_gated(adapter: &AgentAdapter, options: &SetupOptions) -> bool {
    let Ok(paths) = resolve_paths(adapter, options) else {
        return false;
    };
    // An agent gated by a plugin rather than by a settings hook: two
    // mechanisms, one question.
    if let Some(file) = paths.plugin
        && plugin::is_ours(fs::read_to_string(&file).ok().as_deref())
    {
        return true;
    }
    let Some(hooks) = paths.hooks else {
        return false;
    };
    let Ok(text) = fs::read_to_string(&hooks) else {
        return false;
    };
    let Some(root) = render::parse_settings(&text) else {
        return false;
    };
    // Three envelopes, one question: Codex keeps the events at the top level,
    // the other two wrap them.
    let events = root
        .get("hooks")
        .and_then(Value::as_object)
        .or_else(|| root.as_object());
    events.is_some_and(|events| {
        events
            .values()
            .filter_map(Value::as_array)
            .flatten()
            .any(|entry| {
                is_estigia_hook(entry)
                    || entry
                        .get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|command| {
                            runs_estigia_hook(
                                command,
                                Some(crate::harness::hook::Event::PreToolUse),
                            )
                        })
            })
    })
}

/// Whether Codex would skip the gate this agent has registered, and why.
///
/// Read from the two files Codex reads, not from what setup last wrote:
/// `Some` is a gate that is registered and will not run.
pub fn skipped_by_agent(adapter: &AgentAdapter, options: &SetupOptions) -> Option<Skipped> {
    if adapter.instructions != InstructionFile::Codex {
        return None;
    }
    let paths = resolve_paths(adapter, options).ok()?;
    let hooks = paths.hooks?;
    let config = paths.mcp_config?;
    let entries = codex_trust::owned(&hooks, &fs::read_to_string(&hooks).ok()?);
    let text = match fs::read_to_string(&config) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return (!entries.is_empty()).then(|| Skipped {
                detail: format!(
                    "whether Codex trusts the gate cannot be told: {} could not be read ({error})",
                    config.display()
                ),
                cleared_by_setup: false,
                advice: "make the file readable".to_owned(),
            });
        }
    };
    codex_trust::skipped(&hooks, &config, &entries, &text)
}

/// Whether the workflow tools are registered for this agent.
pub fn exposes_tools(adapter: &AgentAdapter, options: &SetupOptions) -> bool {
    let Ok(paths) = resolve_paths(adapter, options) else {
        return false;
    };
    let Some(config) = paths.mcp_config else {
        return false;
    };
    let Ok(text) = fs::read_to_string(&config) else {
        return false;
    };
    match adapter.mcp_format() {
        McpFormat::CodexToml => text.lines().any(render::opens_estigia_table),
        format => {
            render::parse_settings(&text).is_some_and(|root| mcp_entry(&root, format).is_some())
        }
    }
}

/// The same, for a caller that answers for the whole agent scope.
///
/// The setup screen and `config edit` hand back **every** agent row;
/// `config set --agent` asks one question, and writing the rest would pin five
/// rows for every one asked. A row that
/// [`crate::config::Setting::applies_to_agent`] is false for is kept out by the
/// writer itself, for the same reason the screen does not draw it — a value
/// that means nothing is not written down.
pub fn write_agent_configuration_wholly(
    file: &Path,
    slug: &str,
    config: &Config,
) -> Result<(), Refusal> {
    write_agent_configuration(file, slug, config, crate::config::AGENT_SETTINGS)
}
