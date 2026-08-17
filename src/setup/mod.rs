//! Registering Estigia in somebody else's agent, and taking it back out.
//!
//! Two tables with the same shape — [`AGENTS`] and [`COMPANIONS`] — and the
//! four invariants copied from Leteo's `src/setup/mod.rs`, which earned them:
//!
//! 1. [`is_configured`] is a query, separate from [`setup`]. The setup screen
//!    that offers twelve agents with twelve empty boxes cannot answer the one
//!    question people arrive with: *where did I install this?*
//! 2. [`uninstall`] is the **exact inverse**, and what it takes out is what
//!    Estigia **created** — see `crate::skill::record` for why those are not
//!    the same set, and what it cost to find out. It takes out what Estigia put in
//!    and nothing else, because these files hold other people's servers, other
//!    tools' hooks and somebody's own notes.
//! 3. Everything Estigia writes into a shared file lives between markers. The
//!    user's file is never rewritten.
//! 4. `--dry-run` reports [`Change`] per file and a count. It is checked against
//!    the real run by a test, so the plan and the act cannot disagree.
//!
//! Leteo keeps no backups and needs none: marked blocks plus an exact inverse
//! leave nothing to restore.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::config::{CONFIG_FENCE, Config, ModelRouting};
use crate::fence::Fence;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::paths;
use crate::skill;

mod companion;
pub(crate) mod plugin;
mod render;
pub mod wiring;
pub use render::NotEditable;

pub(crate) use companion::resolve_on_path;
pub use companion::{COMPANIONS, Companion, CompanionState, find_companion, probe_companion};
pub use render::SERVER_NAME;
use render::{
    Envelope, McpFormat, is_estigia_hook, render_crush_hooks, render_cursor_hooks, render_hooks,
    render_mcp, render_windsurf_hooks, strip_cursor_hooks, strip_hooks, strip_mcp,
    strip_windsurf_hooks,
};

/// What the agent is told, once, in its always-loaded instruction file.
///
/// Deliberately short, and it carries only what fails silently if the agent
/// never reads further: that the authority exists, that a claim is adjudicated
/// rather than asserted, and that a verdict is bound to bytes. Everything else
/// is in the skill, where the tokens are paid for when they are needed.
///
/// Leteo shipped the same protocol twice — injected here *and* as the skill —
/// which cost the tokens twice and let two copies drift. They had drifted. So
/// this names the skill rather than restating it.
pub const DIRECTIVE_TEMPLATE: &str = r#"## Estigia — workflow authority

Issue work in this environment runs under Estigia. Three rules:

1. **A claim is adjudicated, not asserted.** Never infer that you hold an issue
   from a label, an assignee, prose, or a worktree you can see. Run the
   binding's `verify_claim` before the first repository write and at every
   expensive or irreversible boundary; an unreadable control surface permits no
   write.
2. **A verdict is bound to exact bytes.** A review and a green CI run count only
   against the same head and base they were produced on. Every push invalidates
   both — re-publish and ask again rather than carrying a verdict forward.
3. **An unknown result is not clearance.** When a write's outcome cannot be
   read back, say so and stop. Never report the nearest named state instead.

The `{skill}` skill at `{path}` has the rest: the states, the transitions, the
tracker binding, and the delivery topology.

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
/// Nothing is superseded: Estigia has only ever written this pair. The field is
/// still there because the day it stops being true is the day an upgrade would
/// otherwise leave two directives in one file.
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
/// Only roots that have been verified against a real installation are named
/// here. An agent whose skill directory is a guess gets [`Self::Neutral`], which
/// is the convention issue-flow already installs into and which the directive
/// then names by path — an agent that has to be told where the skill is beats
/// a skill written where the agent will not look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillsRoot {
    /// `~/.agents/skills` — the agent-neutral convention.
    Neutral,
    /// `~/.claude/skills`.
    ClaudeCode,
    /// `~/.codex/skills`.
    Codex,
}

/// Which always-loaded instruction file carries the directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstructionFile {
    Neutral,
    ClaudeCode,
    Codex,
    OpenCode,
    GeminiCli,
    Cursor,
    Qwen,
    Windsurf,
    Crush,
    Cline,
    Continue,
}

/// One agent Estigia knows how to register itself in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentAdapter {
    /// The name typed on the command line.
    pub slug: &'static str,
    /// The name shown to a person.
    pub display_name: &'static str,
    skills: SkillsRoot,
    instructions: InstructionFile,
}

const CLAUDE_MODELS: &[&str] = &["fable", "opus", "sonnet", "haiku"];
const CODEX_MODELS: &[&str] = &[
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.3-codex",
    "gpt-5.2-codex",
];

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
    ("explore", "sonnet"),
    ("propose", "opus"),
    ("spec", "sonnet"),
    ("design", "opus"),
    ("tasks", "sonnet"),
    ("apply", "sonnet"),
    ("implementer", "sonnet"),
    ("reviewer", "sonnet"),
    ("judge", "opus"),
];
const CLAUDE_PERFORMANCE: &[(&str, &str)] = &[
    ("orchestrate", "opus"),
    ("explore", "opus"),
    ("propose", "opus"),
    ("spec", "opus"),
    ("design", "opus"),
    ("tasks", "opus"),
    ("apply", "opus"),
    ("implementer", "opus"),
    ("reviewer", "opus"),
    ("judge", "opus"),
];
const CLAUDE_ECONOMY: &[(&str, &str)] = &[
    ("orchestrate", "sonnet"),
    ("explore", "sonnet"),
    ("propose", "sonnet"),
    ("spec", "haiku"),
    ("design", "sonnet"),
    ("tasks", "haiku"),
    ("apply", "sonnet"),
    ("implementer", "sonnet"),
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
    ("orchestrate", "gpt-5.6-sol"),
    ("explore", "gpt-5.6-terra"),
    ("propose", "gpt-5.6-sol"),
    ("spec", "gpt-5.6-terra"),
    ("design", "gpt-5.6-sol"),
    ("tasks", "gpt-5.6-terra"),
    ("apply", "gpt-5.6-terra"),
    ("implementer", "gpt-5.6-terra"),
    ("reviewer", "gpt-5.6-terra"),
    ("judge", "gpt-5.6-sol"),
];
const CODEX_PERFORMANCE: &[(&str, &str)] = &[
    ("orchestrate", "gpt-5.6-sol"),
    ("explore", "gpt-5.6-sol"),
    ("propose", "gpt-5.6-sol"),
    ("spec", "gpt-5.6-sol"),
    ("design", "gpt-5.6-sol"),
    ("tasks", "gpt-5.6-sol"),
    ("apply", "gpt-5.6-sol"),
    ("implementer", "gpt-5.6-sol"),
    ("reviewer", "gpt-5.6-sol"),
    ("judge", "gpt-5.6-sol"),
];
const CODEX_ECONOMY: &[(&str, &str)] = &[
    ("orchestrate", "gpt-5.6-terra"),
    ("explore", "gpt-5.6-terra"),
    ("propose", "gpt-5.6-terra"),
    ("spec", "gpt-5.6-luna"),
    ("design", "gpt-5.6-terra"),
    ("tasks", "gpt-5.6-luna"),
    ("apply", "gpt-5.6-terra"),
    ("implementer", "gpt-5.6-terra"),
    ("reviewer", "gpt-5.6-luna"),
    ("judge", "gpt-5.6-terra"),
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

/// Every agent adapter, in the order `--all` walks them.
pub const AGENTS: &[AgentAdapter] = &[
    // The neutral root first: it is the one that works when nothing else is
    // installed, and the one issue-flow's own installer already writes.
    AgentAdapter {
        slug: "agents",
        display_name: "Agent-neutral (~/.agents)",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::Neutral,
    },
    AgentAdapter {
        slug: "claude-code",
        display_name: "Claude Code",
        skills: SkillsRoot::ClaudeCode,
        instructions: InstructionFile::ClaudeCode,
    },
    AgentAdapter {
        slug: "codex",
        display_name: "Codex",
        skills: SkillsRoot::Codex,
        instructions: InstructionFile::Codex,
    },
    // From here down the instruction file is verified and the skill directory
    // is not, so the skill lands in the neutral root and the directive names
    // the path. Promoting one of these is a one-line change plus a real
    // installation to check it against.
    AgentAdapter {
        slug: "opencode",
        display_name: "OpenCode",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::OpenCode,
    },
    AgentAdapter {
        slug: "gemini-cli",
        display_name: "Gemini CLI",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::GeminiCli,
    },
    AgentAdapter {
        slug: "cursor",
        display_name: "Cursor",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::Cursor,
    },
    AgentAdapter {
        slug: "continue",
        display_name: "Continue",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::Continue,
    },
    AgentAdapter {
        slug: "cline",
        display_name: "Cline",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::Cline,
    },
    AgentAdapter {
        slug: "crush",
        display_name: "Crush",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::Crush,
    },
    AgentAdapter {
        slug: "windsurf",
        display_name: "Windsurf",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::Windsurf,
    },
    AgentAdapter {
        slug: "qwen",
        display_name: "Qwen",
        skills: SkillsRoot::Neutral,
        instructions: InstructionFile::Qwen,
    },
];

/// Whether this adapter installs the skill where the agent finds it on its own.
impl AgentAdapter {
    /// The tail of this adapter's instruction file, as the gate spells paths.
    ///
    /// The gate cannot call `resolve_paths`: it answers one hook invocation
    /// against a string, with no environment to resolve a home directory from.
    /// So it matches a fragment, and a fragment is a second spelling of a path
    /// the installer already decides — the shape that let the contract become
    /// writable once before, when `skill::DIRECTORY` was renamed and a literal
    /// in `CONTROL_SURFACE` was not.
    ///
    /// This lives beside the `match` that resolves the full path, so the two are
    /// read together, and `every_control_file_an_adapter_has_is_one_the_gate_measures`
    /// crosses them: it resolves the real path for every adapter and asks the
    /// gate. A fragment that stops matching what `paths_in` writes fails that
    /// test rather than silently leaving the file unmeasured.
    ///
    /// Lowercase with forward slashes, because `is_control_surface` folds both
    /// before it matches. Two components where the last is generic — `AGENTS.md`
    /// is used by three adapters and `estigia.md` by two — and one where the
    /// directory alone would be too wide.
    pub fn instruction_fragment(&self) -> &'static str {
        match self.instructions {
            // Without the extension, the same trimming `.claude/settings` needed
            // and for the same reason: `contains` never reaches a `.local.`
            // sibling. `~/.claude/CLAUDE.local.md` and `~/.codex/AGENTS.local.md`
            // are read with the same authority as the files beside them and
            // answered `Routine` — the identical shape this change fixed one entry
            // over, applied to the entries it introduced only after a reviewer
            // pointed at them.
            InstructionFile::Neutral => ".agents/agents",
            InstructionFile::ClaudeCode => ".claude/claude",
            InstructionFile::Codex => ".codex/agents",
            InstructionFile::OpenCode => "opencode/agents.md",
            // `%APPDATA%/gemini/` on Windows and `~/.gemini/` elsewhere; the
            // last two components are the same on both.
            InstructionFile::GeminiCli => "gemini/gemini.md",
            InstructionFile::Cursor => ".cursor/estigia-workflow-authority.md",
            InstructionFile::Qwen => ".qwen/qwen.md",
            InstructionFile::Crush => "crush/crush.md",
            InstructionFile::Continue => ".continue/rules/estigia.md",
            InstructionFile::Cline => ".cline/rules/estigia.md",
            InstructionFile::Windsurf => "windsurf/memories/global_rules.md",
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

    /// Whether the agent discovers the skill without being told where it is.
    ///
    /// `false` means the skill lands in the neutral root and the directive
    /// names the path. Callers show this, because "installed" and "installed
    /// where it will be found" are different answers.
    pub fn discovers_skills(&self) -> bool {
        !matches!(self.skills, SkillsRoot::Neutral)
    }
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
    /// The harness is what turns the contract from a request into a gate. It is
    /// installed by default and this is the way out for somebody who wants the
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
    /// Where this agent's sub-agent definitions live, when the gate can read
    /// them back.
    ///
    /// `None` is the interesting value. Cursor, Kiro and the rest keep agent
    /// definitions too, and writing Claude-format definitions there would install a
    /// `tools:` line nothing enforces — a declaration dressed as a boundary,
    /// which is the arrangement this crate criticises wherever it finds it.
    /// So the destination is exactly the set
    /// [`crate::harness::roles::definition_for`] searches, and adding a fourth
    /// means teaching the gate to read it in the same change.
    pub agents_root: Option<PathBuf>,
}

/// Whether Estigia can install lifecycle hooks for this agent.
///
/// Claude Code only, and the reason has narrowed twice while being written
/// down, which is worth recording rather than tidying away.
///
/// It is **not** that the others have no hooks: other harnesses write startup
/// hooks for Codex, OpenCode and Pi. Those notify rather than gate.
///
/// It is **not** that no other agent can deny, either. OpenCode's plugin API
/// has `tool.execute.before`, which blocks a call by throwing. That is a real
/// deny, and it is the obvious next adapter.
///
/// It is that a Claude Code hook is a line in a settings file whose shape has
/// been verified, and an OpenCode plugin is a **TypeScript module** — a
/// different artefact, in a language this crate does not otherwise touch,
/// whose API shape is currently known here only from third-party write-ups.
/// Writing one from those would be installing something that reports success
/// and may enforce nothing, which is the failure this whole project is built
/// against: a wrong path fails while claiming to have worked.
///
/// It also has a documented hole — `tool.execute.before` is reported not to
/// intercept calls made by subagents — so the adapter would ship with a limit
/// worth declaring beside it.
impl AgentAdapter {
    /// Whether `setup` can install a hooks-file gate for this agent.
    pub fn supports_hooks(&self) -> bool {
        self.gate_spec().is_some()
    }

    /// Whether `setup` can gate this agent's tool calls at all, by any
    /// mechanism — a hooks file for three of them, a plugin for OpenCode.
    pub fn can_gate_tools(&self) -> bool {
        // Two agents gate through a file Estigia owns whole rather than through
        // an entry in a settings file: OpenCode's plugin and Cline's hook
        // script. They gate as much as the others do; only the shape differs.
        self.supports_hooks()
            || matches!(
                self.instructions,
                InstructionFile::OpenCode | InstructionFile::Cline
            )
    }
}

/// What a setting is worth for one adapter.
///
/// Not every row means the same thing for every agent, and the difference is
/// not cosmetic: three of the sixteen are enforced by the gate, and an agent
/// Estigia cannot gate gets the row written into its contract and nothing
/// holding it. Saying so is the whole point — a screen that offers `ask` for an
/// agent that will never be asked has taught the operator something false about
/// their own repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applies {
    /// Estigia enforces it for this agent.
    Held,
    /// Written into the contract, and the agent is asked to honour it — but
    /// there is no gate here, so nothing checks that it did.
    Asked(&'static str),
    /// It has no effect for this agent at all.
    /// Nothing produces this today.
    ///
    /// `Renewal window` was the one, until it turned out to be enforced by the
    /// gate rather than obeyed by the agent and moved to the repository. Kept
    /// because the distinction it draws is real and cheap: a row that *asks*
    /// without a gate behind it is still worth setting, and a row that decides
    /// nothing at all is not, and only the second should refuse to open.
    Inert(&'static str),
}

impl Applies {
    /// A word for the row, or nothing when it is simply held.
    pub fn tag(self) -> Option<&'static str> {
        match self {
            Self::Held => None,
            Self::Asked(_) => Some("not held"),
            Self::Inert(_) => Some("no effect"),
        }
    }

    /// Why, in one line.
    pub fn because(self) -> Option<&'static str> {
        match self {
            Self::Held => None,
            Self::Asked(why) | Self::Inert(why) => Some(why),
        }
    }

    /// Whether the screen should let this row be edited.
    ///
    /// Only the inert one is refused. `Asked` is a real answer written into a
    /// real contract, and refusing to let somebody set it would remove the only
    /// way they have to tell a contract-only agent what is expected of it.
    pub fn editable(self) -> bool {
        !matches!(self, Self::Inert(_))
    }
}

impl AgentAdapter {
    /// What this setting is worth for this adapter.
    ///
    /// Derived from [`Self::can_gate_tools`] rather than from a per-adapter
    /// list, because that is the property the answer actually turns on and a
    /// second list would be a second thing to keep in step.
    pub fn applies(&self, setting: crate::config::Setting) -> Applies {
        use crate::config::Setting;
        if setting == Setting::Review {
            return Applies::Asked(
                "Estigia records and releases the review handoff, but this runtime must still \
                 provide a distinct reviewer context",
            );
        }
        if self.can_gate_tools() {
            return Applies::Held;
        }
        match setting {
            // `Renewal window` was here, as the one row that decided nothing
            // for an ungated agent. It is a repository row now — the gate
            // enforces it and cannot tell which agent it is answering — and a
            // row belonging to the repository must not be sealed shut by
            // whichever agent the cursor happens to be on. The same fact is
            // still told, three times, by the rows below.
            //
            // These three name who authorises something. The gate is what
            // stops and asks; without one the contract still says it, and the
            // agent may still honour it, but nothing checks.
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
    /// One SDD planning phase, as a sub-agent definition the host routes to.
    PhaseAgent,
    /// The one static blind-review definition Claude Code routes to.
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
    AfterPhase,
    AfterHooks,
    AtMcp,
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
            // `Kept` is a file nothing happened to. Counting it would make an
            // uninstall over an existing checkout report a dozen changes and
            // then leave the directory exactly as it found it.
            .filter(|action| {
                !matches!(
                    action.change,
                    Change::Unchanged | Change::Kept | Change::Shared | Change::Unrecorded
                )
            })
            .count()
    }
}

#[derive(Debug)]
struct Environment {
    platform: Platform,
    home: PathBuf,
    config_home: Option<PathBuf>,
    app_data: Option<PathBuf>,
}

impl Environment {
    fn resolve(options: &SetupOptions) -> Result<Self> {
        let platform = options.platform.unwrap_or_else(Platform::current);
        let home = match &options.home_dir {
            Some(path) => path.clone(),
            None => paths::home_dir()?,
        };
        paths::require_absolute(&home, "home directory")?;
        // A caller that moved the home meant to move everything under it. Two
        // of the six roots came from `XDG_CONFIG_HOME` and `APPDATA` instead,
        // and on Windows `APPDATA` is always set — so an isolated home left
        // Gemini's instruction file and OpenCode's pointing at the real machine
        // while the skill went to the sandbox. Half a move reads exactly like a
        // whole one until something answers a question about the wrong disk.
        let borrowed = options.home_dir.is_none();
        let inherited = |name: &str| {
            borrowed
                .then(|| env::var_os(name).map(PathBuf::from))
                .flatten()
        };
        Ok(Self {
            platform,
            home,
            // Through `xdg_config_home` rather than beside it. A reviewer
            // measured that this read the variable inline while the gate read it
            // through the public rule, so "one rule" described a shape the code
            // did not have — two implementations that happened to agree, which is
            // the arrangement this crate has already been bitten by twice. They
            // agree by construction now.
            //
            // An explicit override is answered on its own, and never falls
            // through to the variable. Folding the two together read the same on
            // every path the binary can reach and diverged on one a library
            // caller can: `config_home: Some(<relative>)` with the variable set
            // absolute took the **variable** here and `$HOME/.config` before it.
            // A reviewer measured it. A caller that named a config home and named
            // it badly has made a mistake; inheriting the machine's instead is
            // the half-move this whole block exists to refuse.
            config_home: match options.config_home.clone() {
                Some(explicit) => absolute_or_none(Some(explicit)),
                None => borrowed.then(xdg_config_home).flatten(),
            },
            app_data: absolute_or_none(options.app_data.clone().or_else(|| inherited("APPDATA"))),
        })
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
}

fn absolute_or_none(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| path.is_absolute())
}

/// `XDG_CONFIG_HOME`, resolved the one way this crate resolves it.
///
/// Public because the gate needs the same answer, and asking for it is the only
/// way the two roads stay together. `harness::roles::definition_for` reads
/// OpenCode's definition root, and it spelled `~/.config` by hand until a
/// reviewer measured that a relocated variable put the definitions where it never
/// looked — which is `Ok(None)`, which `declared_policy` reads as *the sub-agent
/// may use every tool*. The first fix read the variable itself and so introduced a
/// **third** rule: empty and relative values were taken literally there and folded
/// away here, so the two roads still disagreed on those two inputs. A reviewer
/// measured that too.
///
/// So there is one rule and it lives beside the resolution that uses it: an
/// absolute value or nothing, which is what `Environment::resolve` has always
/// applied through `absolute_or_none`. A relative or empty value is not a config
/// home; it is a caller's mistake, and both roads treat it as absent.
/// The empty-value filter that stood here was dead and a reviewer measured it:
/// an empty `PathBuf` is not absolute, so `absolute_or_none` already answered
/// `None` for it. Two conditions where one decides is how a rule starts to
/// disagree with itself.
pub fn xdg_config_home() -> Option<PathBuf> {
    absolute_or_none(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
}

/// The adapter a slug names, or a refusal listing the ones that exist.
pub fn find_agent(slug: &str) -> Result<&'static AgentAdapter, Refusal> {
    AGENTS
        .iter()
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

/// Where one adapter's files live.
pub fn resolve_paths(adapter: &AgentAdapter, options: &SetupOptions) -> Result<AgentPaths> {
    let environment = Environment::resolve(options)?;
    Ok(paths_in(adapter, &environment))
}

fn paths_in(adapter: &AgentAdapter, environment: &Environment) -> AgentPaths {
    let skill_root = match adapter.skills {
        SkillsRoot::Neutral => environment.home.join(".agents").join("skills"),
        SkillsRoot::ClaudeCode => environment.home.join(".claude").join("skills"),
        SkillsRoot::Codex => environment.home.join(".codex").join("skills"),
    }
    .join(skill::DIRECTORY);

    let instructions = match adapter.instructions {
        InstructionFile::Neutral => environment.home.join(".agents").join("AGENTS.md"),
        InstructionFile::ClaudeCode => environment.home.join(".claude").join("CLAUDE.md"),
        InstructionFile::Codex => environment.home.join(".codex").join("AGENTS.md"),
        InstructionFile::OpenCode => environment.xdg_config().join("opencode").join("AGENTS.md"),
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
        // Global rules, always on, and capped at six thousand characters. The
        // directive is under a thousand, so it fits — but the cap is why it is
        // named here rather than discovered by an operator whose rules file
        // stopped loading.
        // `GlobalContextPaths` defaults to `~/.config/crush/CRUSH.md`, from its
        // own configuration schema. The XDG root is resolved the same way
        // OpenCode's is, and carries the same caveat on Windows.
        InstructionFile::Crush => environment.xdg_config().join("crush").join("CRUSH.md"),
        // `systemMessage.ts` scans `<continueHome>/rules` and applies a rule
        // whenever it is not `invokable` and declares no `globs` and no `regex`
        // — which is to say, a markdown file with no frontmatter is always
        // applied. The directive is exactly that.
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
    };

    // Where this agent's tool-call gate is registered. Three agents can deny;
    // two share the settings envelope and Cursor keeps its own file.
    let hooks = adapter.gate_spec().and_then(|_| {
        Some(match adapter.instructions {
            InstructionFile::ClaudeCode => environment.home.join(".claude").join("settings.json"),
            InstructionFile::Cursor => environment.home.join(".cursor").join("hooks.json"),
            // A sidecar beside the config, not inside it: Codex reads both, and
            // a JSON file is one Estigia can edit without reformatting somebody
            // else's TOML.
            InstructionFile::Codex => environment.home.join(".codex").join("hooks.json"),
            InstructionFile::Qwen => environment.home.join(".qwen").join("settings.json"),
            InstructionFile::Windsurf => environment
                .home
                .join(".codeium")
                .join("windsurf")
                .join("hooks.json"),
            // Hooks, MCP servers and everything else share one file.
            InstructionFile::Crush => environment.xdg_config().join("crush").join("crush.json"),
            // Cline has no settings-file gate: its hook is a script Estigia owns
            // whole, resolved as a plugin below.
            InstructionFile::Cline => environment.home.join(".cline").join("hooks"),
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

    // Verified against a real installation, like the skill roots. An MCP entry
    // written where the agent does not read it reports success and configures
    // nothing — a wrong path is worse than a missing one, because it fails while
    // claiming to have worked.
    let mcp_config = match adapter.instructions {
        InstructionFile::ClaudeCode => Some(environment.home.join(".claude.json")),
        InstructionFile::Codex => Some(environment.home.join(".codex").join("config.toml")),
        InstructionFile::Cursor => Some(environment.home.join(".cursor").join("mcp.json")),
        InstructionFile::Qwen => Some(environment.home.join(".qwen").join("settings.json")),
        // Windsurf's MCP servers live in its own file, and Estigia has not
        // verified where. Named as unknown rather than guessed: an MCP entry in
        // the wrong file is a server that never starts and a `status` line that
        // says it did.
        InstructionFile::Windsurf => None,
        InstructionFile::Crush => Some(environment.xdg_config().join("crush").join("crush.json")),
        // Where Cline reads MCP servers has not been verified, and an entry in
        // the wrong file is a server that never starts under a line saying it
        // did. The gate is what this adapter is for.
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
        // The neutral root is a convention for skills, not an agent with an MCP
        // client of its own.
        InstructionFile::Neutral => None,
    };

    // OpenCode is the only agent here that both loads plugins and has a
    // documented hook that can **deny** a tool call. Claude Code denies through
    // its settings hooks instead; the rest have neither, and writing a plugin
    // an agent does not read would report success and enforce nothing.
    let windows = environment.platform == Platform::Windows;
    let plugin = match adapter.instructions {
        InstructionFile::OpenCode => Some(
            environment
                .xdg_config()
                .join("opencode")
                .join(plugin::DIRECTORY)
                .join(plugin::FILE),
        ),
        // Cline discovers hooks by file name rather than by a settings entry, so
        // its gate is a file Estigia owns whole — the same shape as a plugin,
        // and it travels the same path.
        InstructionFile::Cline => Some(
            environment
                .home
                .join(".cline")
                .join("hooks")
                .join(plugin::cline_hook_file(windows)),
        ),
        _ => None,
    };

    // Exactly the roots `harness::roles::definition_for` searches under the
    // home, and no others — see the field's own note. Claude Code keeps them
    // beside its skills; OpenCode keeps them under the XDG config directory,
    // which was verified against a real installation rather than guessed.
    // Claude Code only, and OpenCode's absence is the correction rather than an
    // oversight. Both keep sub-agent definitions and the gate reads both, so
    // this listed the two — and the payload is written in **Claude Code's**
    // dialect, where `tools:` is a comma-separated line. OpenCode's schema wants
    // an object, so what landed there was not a narrower agent but an invalid
    // configuration: *Expected object | undefined, got "Read, Grep, Glob"*, and
    // the operator's whole `opencode` config stopped loading.
    //
    // Sharing a destination is not sharing a format. Adding OpenCode back means
    // rendering its dialect, and the way to know it is right is its own schema —
    // not this file's assumption that a second reader of the same idea spells it
    // the same way.
    let agents_root = match adapter.slug {
        "claude-code" => Some(environment.home.join(".claude").join("agents")),
        _ => None,
    };

    AgentPaths {
        skill_root,
        instructions,
        hooks,
        mcp_config,
        plugin,
        agents_root,
    }
}

/// One phase definition, with the operator's answers substituted in.
///
/// Two placeholders, and each carries a decision the operator already made
/// somewhere else — which is the whole point of writing these rather than
/// shipping a fixed pair of files.
///
/// `{{MODEL}}` comes from `Model routing`'s phase key. Until now that setting
/// was, in its own words, *"a declaration the agent reads, not a dispatch this
/// binary performs"* — an operator could write `design=opus` and nothing would
/// ever act on it. Written here it becomes the `model:` line of the file the
/// host obeys. `inherit` when nothing was named, because the hosts that read
/// this file spell "whatever the parent is" that way, and an empty value is a
/// frontmatter error rather than a default.
///
/// `{{TOOLS}}` comes from `Planning`'s `openspec` axis. With the artifacts on
/// the issue — the default — no planning phase writes to the repository at all,
/// and the gate holds that. With `openspec` on, the three phases that leave an
/// artifact behind need to write it, so they get `Write` and `Edit` and the two
/// that only think do not. A single fixed list would have to be the wider one,
/// which would hand `explore` a write it never needs.
fn render_phase_agent(template: &str, phase: &str, config: &Config) -> String {
    let model = config.models.for_phase(phase).unwrap_or("inherit");
    let writes_artifacts = matches!(
        config.planning,
        crate::config::Planning::Sdd { openspec: true, .. }
    ) && matches!(phase, "spec" | "design" | "tasks");
    let tools = match (phase, writes_artifacts) {
        // Exploration is the one phase that legitimately reaches outward, and
        // neither of those two ever reaches this gate — see the file's own note.
        ("explore", _) => "Read, Grep, Glob, WebFetch, WebSearch",
        (_, true) => "Read, Grep, Glob, Write, Edit",
        (_, false) => "Read, Grep, Glob",
    };
    template
        .replace("{{MODEL}}", model)
        .replace("{{TOOLS}}", tools)
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

fn reviewer_target(paths: &AgentPaths) -> Option<PathBuf> {
    let name = skill::REVIEW_AGENT.path.strip_prefix("agents/")?;
    Some(paths.agents_root.as_ref()?.join(name))
}

pub(crate) fn reviewer_is_static(existing: &str) -> bool {
    existing == as_the_file_was(Some(existing), skill::REVIEW_AGENT.contents)
}

fn validate_reviewer_definition(paths: &AgentPaths) -> Result<()> {
    let Some(target) = reviewer_target(paths) else {
        return Ok(());
    };
    let Some(root) = paths.agents_root.as_deref() else {
        return Ok(());
    };
    let candidates = crate::harness::roles::reviewer_candidates(root).map_err(|error| {
        reviewer_definition_refusal(
            ReviewerDefinitionCode::Unowned,
            error.path(),
            &format!(
                "cannot be proved not to reserve review-blind: {}",
                error.detail()
            ),
        )
    })?;
    if let Some(other) = candidates.iter().find(|candidate| *candidate != &target) {
        return Err(reviewer_definition_refusal(
            ReviewerDefinitionCode::Unowned,
            other,
            "also reserves review-blind outside Estigia's canonical path",
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
    if !reviewer_is_static(&existing) {
        return Err(reviewer_definition_refusal(
            ReviewerDefinitionCode::Changed,
            &target,
            "was created by Estigia but no longer matches its static definition",
        )
        .into());
    }
    Ok(())
}

/// The directive text for one adapter, naming the path where its skill lands
/// and the file that holds this adapter's own answers.
///
/// The ratchet applied to prose: the directive tells the agent where the skill
/// is, and the path it names is the path [`setup`] wrote to. A directive that
/// names somewhere else is a dead end, so both come from [`AgentPaths`].
///
/// # Why the second sentence exists
///
/// Eight adapters share one contract, so `config set --agent` writes their
/// answers to a file beside it instead. Estigia read that file
/// ([`skill::installed_config_for`]) and nothing the agent reads named it: not
/// the contract's configuration block, which names only `estigia.local.md`, and
/// not this directive, which named only the skill directory. So an operator
/// could set `Planning` for OpenCode alone, be told it was done, and have the
/// agent go on reading the shared row — the override mechanism living in the
/// code and nowhere in the text, which is the exact fault
/// [`skill::configuration_body`] documents itself for avoiding.
///
/// Only for the adapters it applies to. On one with a skill directory of its
/// own there is no second file, and a sentence about a file that will never
/// exist is a sentence that sends a reader looking.
pub fn directive_for(adapter: &AgentAdapter, paths: &AgentPaths) -> String {
    let mine = if adapter.skills == SkillsRoot::Neutral {
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
/// [`directive_is_current`] answers `None` for two situations — a file nothing
/// can read, and a file with no block in it — and `doctor` reports only its
/// `Some(false)`. So a directive that had **drifted** was on the report and one
/// that was **gone** was not, which is the worse of the two: the wrong rules
/// against no rules.
///
/// Measured: replacing `~/.claude/CLAUDE.md` with an operator's own text left
/// `doctor` answering `ok contract claude-code … verified` and no row anywhere
/// about the directive, while `status` on the same machine said *"skill present,
/// directive missing"*. That is the shape this crate's own prose says it has
/// found three times — two commands, one machine, different answers — and this
/// is the fourth.
///
/// `None` here means only what it says: the instruction file could not be read.
pub fn directive_installed(adapter: &AgentAdapter, options: &SetupOptions) -> Option<bool> {
    let paths = resolve_paths(adapter, options).ok()?;
    let existing = fs::read_to_string(&paths.instructions).ok()?;
    Some(DIRECTIVE_FENCE.is_present(&existing))
}

/// Whether the directive this agent loads is the one this binary writes.
///
/// `None` when there is no directive to compare — `--skill-only` leaves the
/// instruction file alone on purpose, and an agent that has one is not a
/// machine with a problem.
///
/// Asked by **doing the write and seeing whether it changes anything**, rather
/// than by rebuilding the expected block here. `upsert` is what `setup` runs,
/// so this cannot drift from it; a second spelling of "what the block should
/// be" is the thing that goes stale while the first one moves.
///
/// It had no check at all. `skill::presence` compares the files under the skill
/// root, and the directive is in the agent's own instruction file — so the one
/// text **every session loads** was unverified. Measured: rewriting the first
/// of the three rules inside Estigia's own fence, from *a claim is adjudicated,
/// not asserted* to *a claim is whatever you say it is*, left `status` saying
/// `configured` and `doctor` saying `ok contract … verified`.
///
/// `sync` already repairs it; only the seeing was missing.
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

/// Whether this agent already has Estigia's directive.
///
/// The setup screen was asked for where nothing can draw it.
///
/// Named separately from [`no_agent_named`] because the way out is different:
/// that one is "say which agent", this one is "there is no screen here, so name
/// the agent on the command line instead".
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
/// when the skill is nobody else's — otherwise it is simply somebody else's.
///
/// Read through `pending` rather than off the disk so that `--dry-run` answers
/// what the real run would: under `--all` the directives come out one at a
/// time, and the plan has to see the same dwindling list the run does.
pub fn skill_shared_with(
    adapter: &AgentAdapter,
    options: &SetupOptions,
    pending: &Pending,
) -> Result<Option<&'static str>> {
    // Named rather than a bare bool so a caller can say who, and so a test that
    // fails names the agent whose skill was about to be taken away.
    let environment = Environment::resolve(options)?;
    let root = paths_in(adapter, &environment).skill_root;
    for other in AGENTS {
        if other.slug == adapter.slug {
            continue;
        }
        let theirs = paths_in(other, &environment);
        if theirs.skill_root != root {
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

/// Whether this agent already carries Estigia's directive.
///
/// Read-only, and quiet about failure: an unreadable or absent instruction file
/// is an agent that is not configured, which is exactly what the caller asked.
/// This is drawn beside a list of choices, not raised as a diagnostic.
pub fn is_configured(adapter: &AgentAdapter, options: &SetupOptions) -> bool {
    let Ok(paths) = resolve_paths(adapter, options) else {
        return false;
    };
    fs::read_to_string(&paths.instructions).is_ok_and(|text| DIRECTIVE_FENCE.is_present(&text))
}

/// What a later step in this run should read.
///
/// Two write steps can land on one file: Qwen keeps its gate and its MCP server
/// in the same `settings.json`. In a real run the second step reads what the
/// first one wrote. Under `--dry-run` nothing was written, so it read the
/// original and planned against a file the run would never produce — the plan
/// said `create` twice where the run does `create` and then `update`.
///
/// `--dry-run` reporting what the real run does is one of the four things setup
/// promises, so the plan carries the memory the disk would have had. `None`
/// records a file this run discarded, which is not the same as one that was
/// never there only until something tries to read it.
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
/// The memory used to live inside one adapter's call, which covered the case it
/// was written for — Qwen keeping its gate and its MCP server in one file — and
/// not the larger one. `setup --all` walks eleven adapters and eight of them
/// share a skill root, so under `--dry-run` each of the eight read the
/// untouched disk and planned to create the same fifteen files again. The plan
/// said 182 where the run does 70.
///
/// That is the fourth thing setup promises, and the one whose being wrong
/// matters most: `--dry-run` exists to be believed before anything happens.
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
    let stopped = |error| SetupFailure::before_mutation(adapter.slug, options.dry_run, error);
    let paths = resolve_paths(adapter, options).map_err(stopped)?;
    let here = std::env::current_dir().ok();
    let layers =
        skill::config_layers_for_install(&paths.skill_root, Some(adapter.slug), here.as_deref())
            .map_err(|error| stopped(error.into()))?;
    let effective = layers
        .effective_over(config)
        .map_err(|error| stopped(error.into()))?;
    setup_adapter_into(adapter, config, &effective, options, pending, true)
}

/// Sets up one adapter from a portable contract and its effective host view.
///
/// A previous adapter may own the shared skill-tree action. Host artifacts are
/// always derived from `effective`, even when `portable` deliberately excludes
/// an agent, local, or repository override.
pub fn setup_adapter_into(
    adapter: &AgentAdapter,
    portable: &Config,
    effective: &Config,
    options: &SetupOptions,
    pending: &mut Pending,
    install_skill: bool,
) -> std::result::Result<SetupResult, SetupFailure> {
    let preview = if options.dry_run {
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
    // This is the one external definition whose name Estigia reserves. Decide
    // ownership and drift before the skill, directive, hooks, or any other
    // setup artifact can move.
    step!(validate_reviewer_definition(&paths));
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
        // Remembered now because it cannot be worked out later: a file holding
        // nothing but the directive block is either one this run made or one
        // the operator kept empty, and afterwards they are the same bytes. The
        // uninstall deletes the first and must leave the second.
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

    if let Some(target) = reviewer_target(&paths) {
        step!(injected_reviewer_definition_create_collision(&target));
        let existing = step!(read_pending(&target, pending));
        // The first preflight protects every earlier setup artifact. This one
        // protects the external definition from appearing while those writes
        // happen; the no-replace create below closes the remaining read/write
        // window rather than trusting this second observation as a lock.
        step!(validate_reviewer_definition(&paths));
        // Ownership lands before the first byte at the external path. A failed
        // create therefore leaves an owned absent target that exact replay can
        // finish, rather than an unowned file setup must refuse next time.
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
        actions.push(write_step!(write_reviewer_definition(
            &paths.skill_root,
            &target,
            existing.as_deref(),
            options.dry_run,
            ownership_added,
        )));
        pending.insert(
            target,
            Some(as_the_file_was(
                existing.as_deref(),
                skill::REVIEW_AGENT.contents,
            )),
        );
    }

    // The planning phases, as sub-agent definitions the host routes to.
    //
    // Written only where the gate can read them back, and only for the phases
    // the protocol in force actually runs: under `direct` no phase runs, so
    // installing five agents that describe themselves as available would be the
    // configuration and the disk disagreeing about what this machine does. Under
    // `sdd lite` the same argument drops `explore`, `propose` and `design`.
    if let Some(agents_root) = paths.agents_root.as_ref() {
        // A phase this install put there and the protocol no longer runs comes
        // back out. Writing without retracting was the first thing this feature
        // got wrong, and it was found by using it: moving `Planning` from `sdd`
        // to `direct` left five agents on disk that the host can still route to,
        // on a machine configured to run none of them. That is the configuration
        // and the disk disagreeing about what this installation does, which is
        // the sentence three lines up promising it would not happen.
        //
        // Only what this install created, for the reason the uninstall gives:
        // another harness ships these names too, and overwriting somebody's
        // file never made it ours to delete.
        for file in skill::PHASE_AGENTS {
            let Some(name) = file.path.strip_prefix("agents/") else {
                continue;
            };
            let phase = name.trim_start_matches("sdd-").trim_end_matches(".md");
            if effective.planning.phases().contains(&phase) {
                continue;
            }
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
        for phase in effective.planning.phases() {
            let Some(file) = skill::PHASE_AGENTS
                .iter()
                .find(|file| file.path == format!("agents/sdd-{phase}.md"))
            else {
                continue;
            };
            let target = agents_root.join(format!("sdd-{phase}.md"));
            let existing = step!(read_pending(&target, pending));
            let desired = render_phase_agent(file.contents, phase, effective);
            actions.push(write_step!(write_file(
                &target,
                existing.as_deref(),
                &desired,
                ActionKind::PhaseAgent,
                options.dry_run,
            )));
            pending.insert(
                target.clone(),
                Some(as_the_file_was(existing.as_deref(), &desired)),
            );
            // Outside the skill tree, so the same rule the directive needs: a
            // definition this run created must come out on the way back, and one
            // that was already there — other harnesses ship these names too — must not.
            if existing.is_none() && !options.dry_run {
                step!(skill::record::note_created_outside(
                    &paths.skill_root,
                    &target
                ));
            }
        }
    }
    boundary!(SetupFailureBoundary::AfterPhase);

    if !options.skip_harness {
        let executable = step!(executable_path());

        if let Some(hooks) = paths.hooks.as_ref()
            && let Some(spec) = adapter.gate_spec()
        {
            let existing = step!(read_pending(hooks, pending));
            let desired = match spec.envelope {
                // Crush's entry is Cursor's plus a matcher, and the renderer
                // already writes one when the spec carries it.
                Envelope::Cursor => step!(render_cursor_hooks(
                    hooks,
                    existing.as_deref(),
                    &executable,
                    adapter.slug,
                    spec,
                )),
                Envelope::Crush => {
                    step!(render_crush_hooks(
                        hooks,
                        existing.as_deref(),
                        &executable,
                        adapter.slug,
                        spec
                    ))
                }
                Envelope::Windsurf => step!(render_windsurf_hooks(
                    hooks,
                    existing.as_deref(),
                    &executable,
                    adapter.slug,
                    spec,
                )),
                Envelope::Settings => {
                    step!(render_hooks(
                        hooks,
                        existing.as_deref(),
                        &executable,
                        adapter.slug,
                        spec
                    ))
                }
            };
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
            // Somebody else's plugin under our name is not something to
            // overwrite, and not something to fail on either: it is reported
            // unchanged, and `status` says the gate is off.
            if existing.is_none() || plugin::is_ours(existing.as_deref()) {
                let desired = match adapter.instructions {
                    InstructionFile::Cline => plugin::cline_hook(
                        &executable,
                        options.platform.unwrap_or_else(Platform::current) == Platform::Windows,
                    ),
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

        boundary!(SetupFailureBoundary::AfterHooks);

        if let Some(config) = paths.mcp_config.as_ref() {
            let existing = step!(read_pending(config, pending));
            let desired = step!(render_mcp(
                config,
                existing.as_deref(),
                &executable,
                adapter.mcp_format(),
            ));
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
    })
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

/// [`uninstall`], remembering what the rest of this run already took out.
///
/// The mirror of [`setup_into`], for the same reason and against the same
/// measurement: `uninstall --all --dry-run` promised seventeen files for each
/// of the eight adapters that share a skill root, and the run takes out three.
pub fn uninstall_from(
    adapter: &AgentAdapter,
    options: &SetupOptions,
    pending: &mut Pending,
) -> Result<SetupResult> {
    let paths = resolve_paths(adapter, options)?;
    // Decided before the skill directory goes, because the record that answers
    // it lives *in* that directory: asked afterwards, whether this instruction
    // file was Estigia's is a question put to a file that is no longer there,
    // and the answer would be "the operator's" for every file every time. The
    // ownership is released only after its deletion succeeds below. Deferring
    // that release keeps a failed reviewer deletion exactly retryable too.
    let directive = match read_pending(&paths.instructions, pending)? {
        Some(existing) => {
            let desired = DIRECTIVE_FENCE.remove(&existing);
            let ours = skill::record::created_outside(&paths.skill_root, &paths.instructions);
            let emptied = is_now_empty(&desired, ActionKind::Directive, &[], ours);
            Some((existing, desired, emptied))
        }
        None => None,
    };

    // The static reviewer is decided and removed before the skill ledger that
    // proves ownership. Changed text is the operator's work: keep it and
    // relinquish the path. Line-ending and final-newline shape are normalized;
    // only a completed deletion permits ownership to be forgotten.
    let mut reviewer_actions = Vec::new();
    if let Some(target) = reviewer_target(&paths)
        && skill::record::created_outside(&paths.skill_root, &target)
    {
        match read_pending(&target, pending)? {
            None => {
                if !options.dry_run {
                    skill::record::forget_outside(&paths.skill_root, &target)?;
                }
            }
            Some(existing) if reviewer_is_static(&existing) => {
                reviewer_actions.push(discard(
                    &target,
                    ActionKind::AgentDefinition,
                    options.dry_run,
                )?);
                pending.insert(target.clone(), None);
                if !options.dry_run {
                    skill::record::forget_outside(&paths.skill_root, &target)?;
                }
            }
            Some(existing) => {
                reviewer_actions.push(SetupAction {
                    kind: ActionKind::AgentDefinition,
                    path: target.clone(),
                    change: Change::Kept,
                });
                pending.insert(target.clone(), Some(existing));
                if !options.dry_run {
                    skill::record::forget_outside(&paths.skill_root, &target)?;
                }
            }
        }
    }

    // The planning phases, taken back before the skill directory goes, for the
    // reason the directive is: the record that says whether this run created
    // them lives *inside* that directory, and asked afterwards it answers "the
    // operator's" for every file.
    //
    // Only the ones this install created. Another harness ships these exact
    // names, so a machine running both has `sdd-spec.md` that Estigia overwrote rather
    // than made — and overwriting somebody's file does not make it ours to
    // delete. That one comes back reported as left behind, which is lossy and
    // said out loud rather than discovered.
    let mut phase_removals: Vec<(PathBuf, Option<String>)> = Vec::new();
    if let Some(agents_root) = paths.agents_root.as_ref() {
        for file in skill::PHASE_AGENTS {
            let Some(name) = file.path.strip_prefix("agents/") else {
                continue;
            };
            let target = agents_root.join(name);
            let Some(existing) = read_pending(&target, pending)? else {
                continue;
            };
            if !skill::record::created_outside(&paths.skill_root, &target) {
                continue;
            }
            if !options.dry_run {
                skill::record::forget_outside(&paths.skill_root, &target)?;
            }
            phase_removals.push((target, Some(existing)));
        }
    }

    // Most agents have no skill directory of their own, so eight of the eleven
    // adapters install into the same neutral root. Taking the skill out with
    // the first of them left the other seven configured and pointing at a
    // directory that was no longer there: uninstalling OpenCode alone made
    // `status` report `configured, skill missing` for eight agents.
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
    // Not one of `FILES` — it is written by a different command, and so it
    // outlived `setup --all --uninstall` and sat in the skill directory of a
    // machine Estigia had been taken off, waiting to configure an install that
    // no longer existed.
    let own = skill::agent_override(&paths.skill_root, adapter.slug);
    if read_pending(&own, pending)?.is_some() {
        actions.push(discard(&own, ActionKind::Skill, options.dry_run)?);
        pending.insert(own, None);
    }

    for (target, _) in phase_removals {
        actions.push(discard(&target, ActionKind::PhaseAgent, options.dry_run)?);
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
        // Remembered whether or not it was written. Under `--dry-run` nothing
        // reaches the disk, and without this the next adapter of an `--all` run
        // reads a directive this run has already taken out — and concludes the
        // skill is still wanted by an agent that is on its way out.
    }

    if let Some(hooks) = paths.hooks.as_ref()
        && let Some(existing) = read_pending(hooks, pending)?
    {
        let desired = match adapter.gate_spec().map(|spec| spec.envelope) {
            Some(Envelope::Cursor | Envelope::Crush) => strip_cursor_hooks(hooks, &existing)?,
            Some(Envelope::Windsurf) => strip_windsurf_hooks(hooks, &existing)?,
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

    if let Some(file) = paths.plugin.as_ref()
        && let Some(existing) = read_optional(file)?
        && plugin::is_ours(Some(&existing))
    {
        actions.push(discard(file, ActionKind::Plugin, options.dry_run)?);
    }

    if let Some(config) = paths.mcp_config.as_ref()
        && let Some(existing) = read_pending(config, pending)?
    {
        let desired = strip_mcp(config, &existing, adapter.mcp_format())?;
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

    Ok(SetupResult {
        agent: adapter.slug,
        dry_run: options.dry_run,
        actions,
        completed: true,
    })
}

/// Whether what is left after Estigia's part comes out holds nothing at all.
///
/// The sharper half of invariant two. Lifting our block out of somebody's
/// `CLAUDE.md` leaves their file, which is right. Lifting it out of a file that
/// **only ever held our block** leaves an empty husk in seven directories with
/// our name on it, which is not an uninstall — it is litter.
///
/// Emptiness is judged after the removal, not by remembering who created the
/// file: a file that holds nothing tells no agent anything, whoever wrote it.
/// For JSON that means an object with no keys, or one whose only keys are the
/// empty containers our own removal left behind.
/// Takes back the keys Estigia wrote around its own entries, once they are gone.
///
/// `version: 1` beside Cursor's hooks is Estigia's, and so is the `hooks`
/// object it created to hold them. [`is_now_empty`] already treats both as
/// scaffolding rather than content — but only to decide whether the whole file
/// goes. A file the operator *also* has keys in survives, and Estigia's
/// scaffolding survived with it:
///
/// ```text
/// before: {"mine": "do not touch"}
/// after:  {"mine": "do not touch", "version": 1, "hooks": {}}
/// ```
///
/// Two keys left in a file Estigia did not create, by the command whose whole
/// job is to be the exact inverse of the one that created them.
///
/// Only when nothing of Estigia's is left: while an entry of its own is still
/// there, the scaffolding is still holding it up.
fn without_scaffolding(desired: &str, scaffolding: &[&str]) -> String {
    // Not skipped when there is no scaffolding: an emptied `hooks` is Estigia's
    // whatever the envelope carries around it, and Crush has no scaffolding and
    // left one behind for exactly that reason.
    // Through the one reader, like the four in `render`. This was the fifth
    // parse site and the only one the byte-order-mark fix did not reach — so on
    // a file carrying one this returned the text untouched, and every piece of
    // scaffolding Estigia had put in somebody's settings **stayed there** after
    // the uninstall that was supposed to be its exact inverse. Silently: the
    // parse failure is not an error here, it is a shrug.
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
    // Only once nothing of Estigia's is left: while an entry of its own is
    // still there, the scaffolding is still holding it up.
    //
    // And only when the scaffolding is *all* that is left. `version` is
    // Cursor's own required field, not something Estigia invented — an operator
    // who already kept hooks of their own had it before Estigia was installed,
    // and removing it unconditionally took it with the uninstall. Their file
    // went from `{"version": 1, "theirs": …}` to `{"theirs": …}`: a hooks
    // document Cursor may refuse to read, broken by a tool leaving.
    //
    // Nothing but scaffolding left means the whole file was Estigia's, which is
    // the case the removal was written for — `{"version": 1, "hooks": {}}`
    // surviving an uninstall as though it were content.
    if !object.contains_key("hooks") && object.keys().all(|key| scaffolding.contains(&key.as_str()))
    {
        for key in scaffolding {
            object.remove(*key);
        }
    }
    // Nothing was Estigia's, so nothing is rewritten. Reserialising regardless
    // reindents a file this run did not change — invariant two, that a file
    // which never mentioned Estigia is *reported unchanged rather than
    // touched*. `strip_hooks` next door keeps it the same way, and the two pipe
    // tests that hold it failed the moment this did not.
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
/// emptied result is ambiguous — a `CLAUDE.md` holding only the block is either
/// one Estigia made or one the operator kept empty, and it used to be deleted
/// either way. The rest are judged on what is left, which for a settings file
/// is sound: Estigia's entries are named, so anything that is not one of them
/// is theirs and keeps the file alive by being there.
///
/// Asking the record rather than choosing a side is the point. Never deleting
/// the file leaves an empty one behind for every agent on a clean uninstall,
/// which `uninstall_leaves_no_file_estigia_created` refuses; always deleting it
/// takes a file that was the operator's. Both are promises this project makes,
/// and only provenance keeps both.
fn is_now_empty(desired: &str, kind: ActionKind, scaffolding: &[&str], ours: bool) -> bool {
    if desired.trim().is_empty() {
        return kind != ActionKind::Directive || ours;
    }
    if kind == ActionKind::Directive {
        return false;
    }
    serde_json::from_str::<Value>(desired).is_ok_and(|root| match root {
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
/// Every renderer here emits `\n` and finishes with one. That is right for a
/// file this crate creates and wrong for one it is only adding a block to: on
/// this crate's own platform the ordinary file is CRLF, and a settings file and
/// an instruction file both came back with **every** ending changed — a
/// whole-file diff in somebody's version control, from a tool that only added a
/// block and took it away again.
///
/// The same promise as the indentation `render` reads off the original and the
/// byte-order mark it keeps: what this crate did not put in the file comes back
/// as it was.
///
/// It is a function rather than four lines inside the writer because **what
/// goes into `pending` has to be what goes onto disk**. Gemini's
/// `settings.json` is both its hooks file and its MCP file, so one run writes
/// it twice and the second pass reads the first pass's text from there. With
/// the shaping inside the writer alone, that pass decided against a file that
/// never existed: eleven of the twelve agent files came back byte for byte and
/// that one lost its endings.
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
    // Against the bytes that would go on disk, not against `desired`. The two
    // are not the same text: [`as_the_file_was`] gives back the line endings and
    // the missing final newline of the file it found, so a file that ends
    // `}` with no newline was compared to one that ends `}\n`, declared
    // `Update`, rewritten — and came out byte for byte identical.
    //
    // Measured: `estigia sync` reported `update C:\Users\alex\.claude.json`
    // on three consecutive runs, and the file's digest never moved. `Update` is
    // this crate's word for *we wrote over what was there*; saying it about a
    // write that changed nothing is the same failure as reporting a state
    // nobody read back, and it rewrites the operator's own settings file on
    // every run to say it.
    //
    // The comparison had its own CRLF rule — `found.replace("\r\n", "\n")` —
    // which is half of what the writer does, kept in a second place. One rule,
    // one place: the shaped text answers both.
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
    }
    Ok(SetupAction {
        kind,
        path: path.to_owned(),
        change,
    })
}

fn write_reviewer_definition(
    skill_root: &Path,
    path: &Path,
    existing: Option<&str>,
    dry_run: bool,
    ownership_added: bool,
) -> Result<SetupAction> {
    if existing.is_some() || dry_run {
        return write_file(
            path,
            existing,
            skill::REVIEW_AGENT.contents,
            ActionKind::AgentDefinition,
            dry_run,
        );
    }

    #[cfg(test)]
    if REVIEWER_DEFINITION_WRITE_FAILURE.with(|injected| injected.replace(false)) {
        anyhow::bail!("injected reviewer-definition write failure");
    }
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("create directory {}", parent.display()))?;
    match paths::create_atomically(path, skill::REVIEW_AGENT.contents) {
        Ok(()) => Ok(SetupAction {
            kind: ActionKind::AgentDefinition,
            path: path.to_owned(),
            change: Change::Create,
        }),
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

/// Estigia's own state, once no agent is left to read it.
///
/// `uninstall --all` took every agent's file out and left `~/.estigia` — the
/// ledger, the run pointers and any stand-down — standing. Measured: after
/// uninstalling all eleven, the **only** thing left under the home was
/// Estigia's own directory. That is the operator's requirement inverted; what
/// they asked for is *everything of the app's, and nothing of mine*.
///
/// And it is not inert on the way back in. Run pointers are what the push guard
/// reads to decide whether a claim covers a checkout; the ledger is what
/// `doctor` reads to decide whether a run may swear at all; and a stand-down is
/// bounded by a clock, not by an installation — so uninstalling and reinstalling
/// inside its window brings the gate back already standing down.
///
/// Only when nothing is left. The state is machine-wide and one `uninstall
/// codex` on a machine that still runs Claude Code must not take the other
/// one's claims with it — the same reason `skill_shared_with` exists.
///
/// Files by name and directories with `remove_dir`, never `remove_dir_all`: if
/// an operator put something of their own in here, it keeps the directory and
/// this leaves it alone. That is the discipline the skill tree already uses.
pub fn forget_state(options: &SetupOptions) -> Vec<PathBuf> {
    if AGENTS.iter().any(|adapter| is_present(adapter, options)) {
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
    remove(crate::harness::session::previous_ledger_path(&runs));
    remove(crate::harness::standdown::path(&runs));
    remove(crate::harness::standdown::legacy_path(&runs));
    // The screen's remembered language. Estigia writes it, it lives in
    // Estigia's own directory, and it means nothing without the screen that
    // reads it — so it goes out with the rest, and the directory can close.
    //
    // It was the one file under `~/.estigia` nothing here named, so an operator
    // who had ever changed the language kept `.estigia/screen` and the
    // directory holding it after taking Estigia off. The test that exists to
    // catch exactly that passed, because its corpus never opened the screen.
    //
    // Through `preference_path` rather than by joining `screen` onto the parent
    // here: one rule, and the file's name belongs to the module that writes it.
    //
    // Resolved the same way `words::remembered` resolves it, which is the whole
    // point of doing it here: an `options.home_dir` of `None` is the ordinary
    // case in a real run — only tests set it — so a removal guarded on `Some`
    // worked in the suite and did nothing on a machine. Measured that way,
    // before this line said `or_else`.
    if let Some(path) = crate::tui::words::preference_path_for(options.home_dir.as_deref()) {
        remove(path);
    }

    if !options.dry_run {
        // Empty only. Anything else in here is not Estigia's to take.
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
        "setup needs to know which agent to configure",
        Resolution::run(format!("estigia setup --all   # or one of: {supported}")),
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

/// Writes an agent override from the same snapshot used to discover ownership.
pub fn write_agent_configuration_from_snapshot(
    snapshot: &AgentConfigurationSnapshot,
    slug: &str,
    config: &Config,
    speaks_for: &[crate::config::Setting],
) -> Result<(), Refusal> {
    let body = match snapshot.document.as_deref() {
        // `upsert` replaces the marked block when it is there and appends it
        // when it is not, so both cases are one call and neither loses what the
        // operator wrote around it.
        Some(existing) => crate::config::CONFIG_FENCE.upsert(
            existing,
            config.render_some_agent_rows(speaks_for).trim_end(),
        ),
        None => fresh_agent_file(slug, config, speaks_for),
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
    let body = match snapshot.document.as_deref() {
        Some(existing) => crate::config::CONFIG_FENCE.upsert(
            existing,
            config.render_some_agent_rows(speaks_for).trim_end(),
        ),
        None => fresh_agent_file(slug, config, speaks_for),
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
    [
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
    .join(
        "
",
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
/// *"Merge strategy is now squash"* over a skill nobody installed.
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

/// A refusal naming what a person has to decide before setup can continue.
pub fn needs_operator_answer(detail: impl Into<String>) -> Refusal {
    Refusal::not_started(
        "setup-needs-an-answer",
        "setup cannot choose this for you",
        Resolution::no_command(NoCommandReason::OperatorKnowledge, detail),
    )
}

#[cfg(test)]
mod tests;

/// Whether Estigia is in this agent at all, by any of the three ways it can be.
///
/// Told, gated, tools — each has its own answer because each is a different
/// question, and this is the union for the callers asking the fourth: *which
/// agents does this machine's Estigia touch*. [`is_configured`] alone reads like
/// that answer and is not it. `setup --skill-only` leaves an agent with the
/// skill installed, the gate registered and the MCP server exposed, and no
/// directive — which `is_configured` calls not configured, correctly for its own
/// question and wrongly for this one. `doctor` filtered its contract checks with
/// it and so reported "no agent is configured, so nothing reads a contract" on a
/// machine whose contract was installed and read, then sent the operator to
/// `setup --all` — the one command that undoes what the flag was for.
pub fn is_present(adapter: &AgentAdapter, options: &SetupOptions) -> bool {
    is_configured(adapter, options) || is_gated(adapter, options) || exposes_tools(adapter, options)
}

/// Whether the registered server names the subcommand that starts one.
///
/// The `tools` row promises *the server the agent reaches Estigia's own
/// operations through*, and it read the `command` and nothing else — so an
/// entry whose `args` no longer say `mcp` reported `running <path>` while the
/// host started a binary that prints its usage and exits `2`. Every operation
/// the agent asks for fails, and the row that exists to say so said the
/// opposite. The same shape the `gate` row had one round earlier, with the
/// matcher in place of the arguments.
///
/// Quiet unless it is sure. A wrapper — `sh -c "estigia mcp"` — puts the
/// subcommand in an argument rather than beside it, so **any** argument
/// mentioning `mcp` is enough. What this reports is an entry that names
/// Estigia's server and nowhere says how to start it.
pub fn tools_start_the_server(adapter: &AgentAdapter, options: &SetupOptions) -> Option<bool> {
    let paths = resolve_paths(adapter, options).ok()?;
    let text = fs::read_to_string(paths.mcp_config?).ok()?;
    if adapter.mcp_format() == McpFormat::CodexToml {
        // The TOML half writes `args = ["mcp"]` under the same table.
        return Some(render::estigia_table_block(&text)?.contains("\"mcp\""));
    }
    let root: Value = serde_json::from_str(&text).ok()?;
    let entry = ["mcpServers", "mcp"]
        .iter()
        .find_map(|key| root.get(key)?.get(SERVER_NAME))?;
    Some(says_mcp(entry.get("args")?))
}

/// Whether an `args` value says how to start the server.
///
/// **Pure and fed**, so the readings can be exercised without a settings file —
/// and because the reader around it was silenced once in a prove-RED and the
/// suite stayed green, which is the shape of a decision nothing tests.
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
/// Read rather than assumed, for the reason [`crate::harness::doctor::gates`]
/// gives about the other half: *installed, looks installed, enforces nothing*.
/// [`exposes_tools`] answers whether an entry exists and nothing about what it
/// points at, so `status` said `tools on` over a server naming a binary that
/// had been moved — every tool call failing, and both commands calling it well.
pub fn tools_command(adapter: &AgentAdapter, options: &SetupOptions) -> Option<PathBuf> {
    let paths = resolve_paths(adapter, options).ok()?;
    let text = fs::read_to_string(paths.mcp_config?).ok()?;
    match adapter.mcp_format() {
        McpFormat::CodexToml => {
            // `[mcp_servers.estigia]` and the `command = "..."` under it, up to
            // the next table. Read by hand because this is the only TOML this
            // crate reads and a parser for one key is a dependency for one key.
            let block = render::estigia_table_block(&text)?;
            block.lines().find_map(|line| {
                let value = line.trim().strip_prefix("command")?.trim_start();
                let value = value.strip_prefix('=')?.trim();
                // Unescaped, not merely unquoted. A TOML basic string writes a
                // Windows path as `C:\\Users\\…`, and stripping the quotes
                // alone reported `running C:\\Users\\…\\estigia.exe` — a path
                // that happens to resolve, because Windows collapses repeated
                // separators, and that no operator would recognise as theirs.
                Some(PathBuf::from(
                    value
                        .trim_matches('"')
                        .replace("\\\\", "\\")
                        .replace("\\\"", "\""),
                ))
            })
        }
        _ => {
            let root: Value = serde_json::from_str(&text).ok()?;
            ["mcpServers", "mcp"]
                .iter()
                .find_map(|key| root.get(key)?.get(SERVER_NAME)?.get("command")?.as_str())
                .map(PathBuf::from)
        }
    }
}

/// Whether the lifecycle gate is registered for this agent.
///
/// A separate query from [`is_configured`], because they answer different
/// questions: one says the agent was told about Estigia, the other says Estigia
/// can stop it. An operator looking at a run that wrote without a claim needs
/// the second answer, and folding it into the first hides exactly that case.
pub fn is_gated(adapter: &AgentAdapter, options: &SetupOptions) -> bool {
    let Ok(paths) = resolve_paths(adapter, options) else {
        return false;
    };
    // An agent gated by a plugin rather than by a settings hook. Two mechanisms,
    // one question — an operator looking at a run that wrote without a claim
    // needs the answer, not the implementation.
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
    let Ok(root) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    // Three envelopes, one question. Codex keeps the events at the top level,
    // the other two wrap them — and an answer that only knows one shape reports
    // no gate on an agent that has one.
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
                        .is_some_and(|command| command.contains("hook pre-tool-use"))
            })
    })
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
        _ => serde_json::from_str::<Value>(&text).is_ok_and(|root| {
            ["mcpServers", "mcp"]
                .iter()
                .any(|key| root.get(key).and_then(|s| s.get(SERVER_NAME)).is_some())
        }),
    }
}

/// The same, for a caller that answers for the whole agent scope.
///
/// The setup screen and `config edit` hand back **every** agent row, because
/// that is what they asked about — the operator saw the whole table and saved
/// it. `config set --agent` is the other case: it asks one question, and
/// writing the rest pinned five rows for every one asked. One entry point each,
/// so the difference is a choice a caller makes rather than a flag it forgets.
pub fn write_agent_configuration_wholly(
    file: &Path,
    slug: &str,
    config: &Config,
) -> Result<(), Refusal> {
    write_agent_configuration(file, slug, config, crate::config::AGENT_SETTINGS)
}
