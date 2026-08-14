//! The command line, as a type.
//!
//! Every flag a rejection is allowed to name has to exist here. That is the
//! first half of the ratchet, and `clap`'s derive is what makes it checkable:
//! a `--flag` in a message and no field for it is caught by the real parser.
//!
//! Two tests do it, because one was not enough.
//! `every_command_a_rejection_names_parses` builds refusals by their own
//! constructors, which is the deeper check — it proves the *reachable* ones.
//! Its population is a list somebody has to remember to add to, and it covered
//! fourteen commands where the source held twenty-two; the ones it missed were
//! `doctor`'s, which are `Health::Broken` rather than `Refusal` and so cannot
//! be in a list of refusals at all.
//! `every_command_the_source_suggests_parses_and_not_only_the_listed_ones`
//! reads them out of the source instead, so a new one is checked the day it is
//! written rather than the day somebody remembers it.

use clap::{Parser, Subcommand};

/// Estigia — workflow authority for coding agents over an issue tracker.
#[derive(Debug, Parser)]
#[command(name = "estigia", version, about, long_about = None)]
pub struct Cli {
    /// The verb.
    #[command(subcommand)]
    pub command: Command,

    /// Print machine-readable JSON instead of prose.
    #[arg(long, global = true)]
    pub json: bool,
}

/// A run id with something in it.
///
/// `--run-id ""` satisfied `clap`, which checks that a flag is **present**
/// rather than that it says anything, and went all the way to the tracker. What
/// it would have recorded is a claim under an empty id — precisely "a claim
/// the gate will never match", which is the sentence this flag's own
/// documentation gives as the reason it is asked for rather than guessed.
///
/// Here rather than in the verb, because a value parser runs before anything is
/// read: the rule the rest of this crate states as *everything knowable from
/// the command line is settled before anything on disk is read*. An empty run
/// id used to be settled by a network round trip, and answered with whatever
/// the tracker happened to say.
fn present(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("a run id cannot be blank \u{2014} `SessionStart` reports one".to_owned());
    }
    Ok(value.to_owned())
}

/// The verbs.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Record this extracted release candidate for the official installers.
    #[command(name = "__record-install", hide = true)]
    RecordInstall,

    /// Open the setup screen, or register Estigia in one named agent.
    //  because that is the word people reach for — Leteo has one, and an
    // operator who has just installed a binary types the name of the screen,
    // not the name of the verb. A subcommand that does not exist teaches them
    // the screen does not either.
    #[command(visible_aliases = ["install", "tui"])]
    Setup {
        /// Which agent. Omit with `--all`, or with `--companion`.
        agent: Option<String>,

        /// Every agent Estigia knows.
        #[arg(long, conflicts_with = "agent")]
        all: bool,

        /// Report what would change and write nothing.
        #[arg(long)]
        dry_run: bool,

        /// Take Estigia back out: everything setup created, and nothing it
        /// merely wrote over.
        #[arg(long)]
        uninstall: bool,

        /// Install the skill without touching the instruction file.
        #[arg(long)]
        skill_only: bool,

        /// Detect a companion and say what to do about it. Installs nothing.
        #[arg(long, value_name = "NAME")]
        companion: Option<String>,

        /// Open the setup screen: agents on the left, that agent's settings on the right.
        ///
        /// Reads answers from standard input, so it is the one path here that
        /// needs a person. It ends by printing the commands that reproduce the
        /// same result without it.
        #[arg(long, conflicts_with_all = ["all", "uninstall", "companion"])]
        interactive: bool,

        /// Permit an unrecorded or source-built binary to change adapter files.
        /// Recorded downgrades and unreadable lifecycle state still refuse.
        #[arg(long)]
        allow_source_build: bool,
    },

    /// Bring an installed skill up to this binary's copy, keeping the
    /// configuration that is already there.
    Sync {
        /// Which agent. Defaults to every configured one.
        agent: Option<String>,

        /// Every configured agent, stated explicitly.
        #[arg(long, conflicts_with = "agent")]
        all: bool,

        /// Report what would change and write nothing.
        #[arg(long)]
        dry_run: bool,

        /// Permit an unrecorded or source-built binary to change adapter files.
        /// Recorded downgrades and unreadable lifecycle state still refuse.
        #[arg(long)]
        allow_source_build: bool,
    },

    /// Report this executable's recorded lifecycle status without changing it.
    Update,

    /// Say what is installed, where, and whether it is current.
    Status,

    /// Check that everything a run needs before it swears actually works.
    ///
    /// Read-only. A refusal otherwise arrives one edit at a time with no way to
    /// tell which of five things is wrong, and every failure here names what to
    /// do about it.
    Doctor,

    /// Take Estigia out of one agent, or all of them.
    Uninstall {
        /// Which agent. Omit with `--all`.
        agent: Option<String>,

        /// Every agent Estigia knows.
        #[arg(long, conflicts_with = "agent")]
        all: bool,

        /// Report what would change and write nothing.
        #[arg(long)]
        dry_run: bool,
    },

    /// Serve the workflow operations as MCP tools over standard input/output.
    ///
    /// Not typed by a person: `estigia setup` registers this in the agent's MCP
    /// configuration, and the agent starts it.
    Mcp,

    /// Run one agent lifecycle event. Reads the payload on standard input.
    ///
    /// Not typed by a person: `estigia setup` writes these into the agent's
    /// settings, and the agent invokes them.
    Hook {
        /// Which event. One of `pre-tool-use`, `session-start`, `session-end`.
        event: String,

        /// Which agent is reading the answer.
        ///
        /// Three agents can deny a tool call and none of them spells a decision
        /// the same way. Getting it wrong produces a hook that runs, decides
        /// correctly, and is ignored — which reports success and enforces
        /// nothing. `setup` writes the right one into the command it registers.
        #[arg(long, default_value = "claude-code")]
        dialect: String,

        /// Which agent this hook was registered in.
        ///
        /// Not the same fact as `--dialect`, and the ledger used to record the
        /// dialect under the name `agent`. Eleven agents share five dialects, so
        /// a call from Codex, OpenCode or Continue was written down as
        /// `claude-code` — and `doctor`'s silence row, whose whole subject is
        /// *which agent sent a call that went through ungated*, said `from
        /// claude-code` and sent the operator to the wrong settings file.
        ///
        /// Optional, because a settings file written by an earlier build does
        /// not send one. Absent, the record says so rather than naming the
        /// dialect and calling it an agent.
        #[arg(long)]
        agent: Option<String>,
    },

    /// Swear to an issue: claim it, and bind this run's writes to that claim.
    ///
    /// Until this runs, Estigia gates nothing — the oath binds once sworn.
    Claim {
        /// The issue number.
        issue: String,

        /// This run's id, as `SessionStart` reported it.
        ///
        /// Required rather than guessed: a claim recorded under the wrong run-id
        /// is a claim the gate will never match, and a silently wrong run-id is
        /// worse than being asked for it.
        #[arg(long, env = "ESTIGIA_RUN_ID", value_parser = present)]
        run_id: String,

        /// When this run expects to report, as `2026-07-31T23:00Z`.
        #[arg(long)]
        horizon: String,

        /// The workflow state to verify against. Defaults to `in-progress`.
        #[arg(long, default_value = "in-progress")]
        state: String,
    },

    /// Put an issue down. Releases the tracker projection and forgets the run.
    Release {
        /// This run's id.
        #[arg(long, env = "ESTIGIA_RUN_ID", value_parser = present)]
        run_id: String,
    },

    /// Ask the gate what it would decide, without an agent in the way.
    ///
    /// The harness made legible: same code path as the hook, printed instead of
    /// returned. For working out why a write was refused.
    Gate {
        /// The tool name, as the agent would send it.
        tool: String,

        /// That tool's arguments, as JSON.
        #[arg(long, default_value = "{}")]
        input: String,

        /// This run's id.
        ///
        /// Optional here, unlike `claim`: an agent whose plugin API hands out a
        /// project directory and no session identity has none to give. Without
        /// it the run is found by asking which oath covers this checkout — the
        /// same question the push guard asks, for the same reason.
        #[arg(long, env = "ESTIGIA_RUN_ID", value_parser = present)]
        run_id: Option<String>,
    },

    /// Install the push guard into this repository.
    ///
    /// The one gate no agent can route around: a `pre-push` hook refuses a push
    /// that no live claim authorises, whoever typed it. `git push --no-verify`
    /// bypasses it, which is a guard rail working as one.
    Guard {
        /// Take it back out. Only removes a hook Estigia wrote.
        #[arg(long)]
        uninstall: bool,

        /// Report what would change and write nothing.
        #[arg(long)]
        dry_run: bool,
    },

    /// Stand the gate down for a bounded, recorded window
    StandDown {
        /// Why. Required — a stand-down with nothing to answer for is a switch.
        #[arg(long)]
        reason: String,
        /// How long, in minutes. Capped, so it cannot become a switch.
        #[arg(long, default_value_t = 30)]
        minutes: u64,
        /// Lift it now rather than waiting for it to expire.
        #[arg(long)]
        lift: bool,
    },
    /// Read and write the operator configuration.
    Config {
        /// What to do with the configuration.
        #[command(subcommand)]
        action: ConfigAction,
    },
}

/// What `estigia config` does.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print every setting, its value, and what it accepts.
    List {
        /// Which agent's installed contract to read. Defaults to the first
        /// configured one.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Review and change every setting on one screen.
    Edit {
        /// Which agent's installed contract to read and write.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Set one setting, validating it before anything is written.
    Set {
        /// The setting's label, as it appears in the table.
        setting: String,
        /// The new value.
        value: String,
        /// Which agent's installed contract to write.
        #[arg(long, conflicts_with = "repo")]
        agent: Option<String>,
        /// Write it into this repository's own file, under `.git/`.
        ///
        /// The door the layer needs: reading it has been there since it
        /// existed, and a file nobody can create without a text editor is a
        /// feature nobody has.
        #[arg(long)]
        repo: bool,
    },
    /// List the repositories that answer for themselves.
    ///
    /// Every checkout `config set --repo` has written to, so the ones that
    /// carry their own answers can be found again from anywhere.
    Repos,
    /// Take away a repository's own answers, leaving the contract underneath.
    ///
    /// Not the same as setting them back: the rows go, and whatever the
    /// installed contract says takes over again — which is what the repository
    /// answered with before it ever answered for itself.
    Forget {
        /// Which checkout. Defaults to the one this is run in.
        #[arg(long)]
        repo: Option<String>,
    },
}
