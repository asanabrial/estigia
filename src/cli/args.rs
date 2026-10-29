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
//! written rather than the day somebody remembers it — and a `Resolution::run`
//! argument it cannot read at the call site is named rather than skipped, so
//! `doctor`'s one command built from a value is covered by a test of its own

use clap::{Parser, Subcommand};

/// Where an install writes: the machine's home, or one workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ScopeArg {
    /// The machine's home. The default.
    Global,
    /// One workspace, so the install belongs to a checkout.
    Workspace,
}

/// Estigia — workflow authority for coding agents over an issue tracker.
///
/// The version is [`crate::build_info::VERSION`], not `CARGO_PKG_VERSION`
/// alone: two builds of an unreleased crate must answer differently, which is
/// the identity the stale-gate `doctor` row needs if it is to mean anything.
#[derive(Debug, Parser)]
#[command(name = "estigia", version = crate::build_info::VERSION, about, long_about = None)]
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

fn session(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err(
            "a session id cannot be blank \u{2014} omit it and the checkout answers".to_owned(),
        );
    }
    Ok(value.to_owned())
}

fn runtime(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err(
            "a runtime cannot be blank \u{2014} it is the prefix of the id derived".to_owned(),
        );
    }
    Ok(value.to_owned())
}

/// The verbs.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Record this extracted release candidate for the official installers.
    #[command(name = "__record-install", hide = true)]
    RecordInstall,

    /// Run one detached reviewer job: read its manifest, ask the reviewer CLI, write its result.
    #[command(name = "__review-job", hide = true)]
    ReviewJob {
        /// The job directory the launch prepared.
        dir: std::path::PathBuf,
    },

    /// Run one detached check job: read its manifest, run each declared check, write its result.
    #[command(name = "__check-job", hide = true)]
    CheckJob {
        /// The job directory the launch prepared.
        dir: std::path::PathBuf,
    },

    /// Open the setup screen: agents on the left, that agent's settings on the right.
    Tui,

    /// Register Estigia in one named agent.
    #[command(visible_aliases = ["install"])]
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

        /// Permit an unrecorded or source-built binary to change adapter files.
        /// Recorded downgrades and unreadable lifecycle state still refuse.
        #[arg(long)]
        allow_source_build: bool,

        /// Install into the machine's home (the default) or one workspace.
        #[arg(long, value_enum, default_value = "global")]
        scope: ScopeArg,

        /// The workspace a `--scope=workspace` install is rooted at.
        #[arg(long, value_name = "PATH")]
        workspace: Option<std::path::PathBuf>,
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

        /// Bring the machine's home (the default) or one workspace up to date.
        #[arg(long, value_enum, default_value = "global")]
        scope: ScopeArg,

        /// The workspace a `--scope=workspace` sync is rooted at.
        #[arg(long, value_name = "PATH")]
        workspace: Option<std::path::PathBuf>,
    },

    /// Report this executable's recorded lifecycle status without changing it.
    Update,

    /// Say what is installed, where, and whether it is current. With `set`,
    /// record this run's self-reported status line instead.
    Status {
        /// What to do beyond saying what is installed.
        #[command(subcommand)]
        action: Option<StatusAction>,
    },

    /// List the sessions that registered on this machine, with the issue each
    /// holds, and mark the silent and the ended ones. With `send`, put a
    /// message in one of them. The three listing flags belong to the listing;
    /// `send` takes none and refuses them before it writes.
    Sessions {
        /// List every row the registry holds, not only the live ones.
        #[arg(long)]
        all: bool,

        /// Take one page of at most this many rows, live ones unless `--all` is
        /// given, rather than the whole list.
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(1..))]
        limit: Option<u64>,

        /// Continue the page a `next_cursor` came from, exactly as it gave the
        /// token, rather than beginning a new snapshot.
        #[arg(long, value_name = "TOKEN")]
        cursor: Option<String>,

        /// Add a field set to each row of the JSON answer. `status` adds what
        /// the run reported about itself, or the task it is on.
        #[arg(long, value_name = "FIELDS")]
        fields: Option<String>,

        /// What to do beyond listing.
        #[command(subcommand)]
        action: Option<SessionsAction>,
    },

    /// List what the local store recorded across sessions, oldest first.
    ///
    /// Read-only. Transitions, claim and review markers, and messages from this
    /// machine's store, on one UTC time scale, a bounded page at a time. The
    /// last 24 hours unless `--since` says otherwise. Bodies are left out unless
    /// `--bodies` is given. The output names what the feed cannot see.
    Feed {
        /// Only this repository, as `owner/name`; one the store does not hold is refused.
        #[arg(long, value_name = "OWNER/NAME")]
        repo: Option<String>,

        /// Only events of runs on this runtime.
        #[arg(long)]
        runtime: Option<String>,

        /// Only events of this run, or messages to or from it.
        #[arg(long, value_name = "RUN-ID")]
        run: Option<String>,

        /// Only this issue; messages carry no issue and are left out.
        #[arg(long, value_name = "N")]
        issue: Option<u64>,

        /// Only this kind: `transition`, `message`, or a marker kind such as `claim`.
        #[arg(long)]
        kind: Option<String>,

        /// Start of the window: a duration (`90m`, `6h`, `2d`) or a UTC time.
        #[arg(long, value_name = "WHEN")]
        since: Option<String>,

        /// Include the first 200 characters of each entry or message body.
        #[arg(long)]
        bodies: bool,

        /// Take a page of at most this many events.
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(1..))]
        limit: Option<u64>,

        /// Continue the page a `next_cursor` came from, exactly as it gave the token.
        #[arg(long, value_name = "TOKEN")]
        cursor: Option<String>,
    },

    /// Print the native command that resumes a stopped session, where to run
    /// it, and what its claim and worktree look like now.
    ///
    /// `--print` is read-only: nothing is started, released or handed off.
    /// `--exec` starts the command itself, and only when the registry proves
    /// the session's process is gone. The run is a run id or the short name
    /// `estigia sessions` shows.
    Resume {
        /// The run id or short name of a registry row, live or not.
        run: String,

        /// Print the command and the state without starting anything.
        #[arg(long, required_unless_present = "exec", conflicts_with = "exec")]
        print: bool,

        /// Run the command in the run's directory and exit with its code;
        /// refused while the session's process is live or its liveness is unknown.
        #[arg(long)]
        exec: bool,
    },

    /// Say where one issue stands, in one answer.
    ///
    /// Read-only; no claim is needed and nothing is written. The repository and whether the
    /// issue is open, its workflow state and last transitions, who holds it and from which
    /// branch, its pull request and the review verdict with the head and base it is bound to,
    /// the task checklist, `blocked-until` clauses, the holder's last messages and the writes
    /// still queued. A field that could not be read says `unavailable` and why.
    Where {
        /// `owner/name#N`, a bare number, or a tracker key such as `ENG-123`. A bare number held
        /// in several repositories is refused with the candidates.
        #[arg(value_name = "REF")]
        reference: String,
    },

    /// Check that everything a run needs before it swears actually works.
    ///
    /// Read-only. A refusal otherwise arrives one edit at a time with no way to
    /// tell which of five things is wrong, and every failure here names what to
    /// do about it.
    Doctor,

    /// Sweep branches and checkouts no delivery owns.
    ///
    /// No removal until `--confirm`; discovery can refresh remote-tracking refs.
    /// It lists every remote head that is merged or
    /// closed and every registered checkout, keeps any a live claim holds or that
    /// carries uncommitted work or commits the remote does not have, and removes
    /// eligible entries only when the operator confirms. Clean remote-gone orphan
    /// checkouts also qualify when every commit remains on a remote and no known
    /// session or pointer holds them; unreadable eligibility keeps them.
    Prune {
        /// Remove what the sweep lists, rather than only listing it.
        #[arg(long)]
        confirm: bool,

        /// The base branch a merged head is an ancestor of.
        #[arg(long, default_value = "main")]
        base: String,
    },

    /// List the moments a run needed a person, newest first.
    ///
    /// Read-only. One record per moment the store wrote a state a person must
    /// act on: an escalated review, a blocked issue with no clause, a gate
    /// `ask`. A record is never an approval, and not evidence that anyone saw it.
    NeedsPerson {
        /// Show at most this many records.
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u64).range(1..))]
        limit: Option<u64>,
    },

    /// List the uncommitted work in every checkout of this repository.
    ///
    /// Read-only. Reports each registered checkout and each one a run pointer
    /// names: branch, uncommitted files by marker with line counts, commits not
    /// on the upstream, the run that holds it, and whether it is an orphan. A
    /// checkout git cannot be read in is reported unreadable, never omitted.
    Changes,

    /// Admit every open issue the local store has never touched.
    ///
    /// The queues decide from the store, so an open issue it has never admitted
    /// is reported `unadmitted` until an operation imports it. This imports
    /// them all, oldest first, and refuses naming the issue when one's history
    /// diverges rather than storing an answer GitHub disagrees with.
    Store {
        /// What to do with the store.
        #[command(subcommand)]
        action: StoreAction,
    },

    /// Send the GitHub writes queued while the git operation window was closed.
    ///
    /// Sends them oldest first and stops at the first one it cannot read back;
    /// it refuses while the window is closed. Every write to GitHub and every
    /// publication runs the same flush first, so this is for sending without
    /// anything else to do.
    Flush {
        /// Start the flush in the background and return at once.
        #[arg(long, conflicts_with = "status")]
        detach: bool,

        /// Print the queue and send nothing.
        #[arg(long)]
        status: bool,
    },

    /// Read the roles delegated contexts were observed running as, across runs.
    ///
    /// The durable half of the role record: a run that did not produce an entry
    /// can read it after the session that did has ended. Read-only.
    Roles {
        /// Only the entries this run id recorded.
        #[arg(long, value_parser = present)]
        run_id: Option<String>,

        /// Only the entries recorded against this issue.
        #[arg(long, value_parser = crate::identity::Identity::parse)]
        issue: Option<crate::identity::Identity>,

        /// Print machine-readable JSON instead of prose.
        #[arg(long)]
        json: bool,
    },

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

        /// Take Estigia out of the machine's home (the default) or one workspace.
        #[arg(long, value_enum, default_value = "global")]
        scope: ScopeArg,

        /// The workspace a `--scope=workspace` uninstall is rooted at.
        #[arg(long, value_name = "PATH")]
        workspace: Option<std::path::PathBuf>,
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
        /// Which event. One of `pre-tool-use`, `session-start`, `session-end`, `stop`.
        event: String,

        /// Which agent is reading the answer.
        ///
        /// Agents can deny a tool call and none of them spells a decision the
        /// same way. Getting it wrong produces a hook that runs, decides
        /// correctly, and is ignored — which reports success and enforces
        /// nothing. `setup` writes the right one into the command it registers.
        #[arg(long, default_value = "claude-code")]
        dialect: String,

        /// Which agent this hook was registered in.
        ///
        /// Not the same fact as `--dialect`, and the ledger used to record the
        /// dialect under the name `agent`. Estigia's agents share five dialects,
        /// so a call from Codex, OpenCode or Continue was written down as
        /// `claude-code` — and `doctor`'s silence row, whose whole subject is
        /// *which agent sent a call that went through ungated*, said `from
        /// claude-code` and sent the operator to the wrong settings file.
        ///
        /// Optional, because a settings file written by an earlier build does
        /// not send one. Absent, the record says so rather than naming the
        /// dialect and calling it an agent.
        #[arg(long)]
        agent: Option<String>,

        /// Wait for mail addressed to this session instead of answering the event.
        ///
        /// Written by `setup` on Claude Code's `Stop` as an `asyncRewake` hook: the
        /// process outlives the turn and, when mail arrives, exits 2 with the
        /// message on standard error, which wakes the idle session.
        #[arg(long)]
        wait_mail: bool,
    },

    /// Swear to an issue: claim it, and bind this run's writes to that claim.
    ///
    /// Until this runs, Estigia gates nothing, except commits and pushes in a checkout where
    /// the push guard is installed.
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

        /// The workflow state the run pointer records. Defaults to `in-progress`.
        #[arg(long, default_value = "in-progress")]
        state: String,

        /// Move the issue to this state in the same call, after the claim.
        #[arg(long)]
        transition_to: Option<String>,
    },

    /// Hand an issue to another runtime, and record who handed it over.
    ///
    /// Writes a row in this machine's store for that runtime's next session in this repository,
    /// and projects `dev:<runtime>` on the issue with read-back through the outbox. A handoff is
    /// a destination, never a claim: the recipient still claims the issue.
    Handoff {
        /// The issue number.
        issue: String,

        /// The runtime the issue is handed to, such as `opencode`.
        #[arg(long)]
        to: String,

        /// This run's id, as `SessionStart` reported it.
        #[arg(long, env = "ESTIGIA_RUN_ID", value_parser = present)]
        run_id: String,
    },

    /// Recover an escalated review, as a maintainer.
    ///
    /// A person's command: the gate refuses an agent's shell line for it. The named login must
    /// hold the `admin` or `maintain` role on the repository, which GitHub is asked. It releases
    /// nothing: the next publication is a fresh candidate in a successor lineage, reviewed again.
    Recover {
        /// The issue number.
        issue: String,

        /// The epoch of the escalated publication. Defaults to the issue's one escalated receipt.
        #[arg(long)]
        epoch: Option<String>,

        /// The GitHub login of the maintainer authorising the recovery. Defaults to the login
        /// `gh` is authenticated as.
        #[arg(long)]
        authorized_by: Option<String>,

        /// Why the review is being recovered.
        #[arg(long)]
        reason: String,
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

        /// That tool's arguments, as JSON, or `-` to read them from standard
        /// input, which is how an input larger than the argument limit travels.
        #[arg(long, default_value = "{}")]
        input: String,

        /// This run's id.
        ///
        /// Optional here, unlike `claim`: a call may carry a derived identity —
        /// `--session-id` with `--runtime` — or an ambient `ESTIGIA_RUN_ID`.
        /// Without any of them the run is found by asking which oath covers
        /// this checkout — the same question the push guard asks, for the same
        /// reason.
        ///
        /// `ESTIGIA_RUN_ID` supplies it when neither this flag nor
        /// `--session-id` names one: a derived identity supersedes the variable
        /// rather than colliding with it.
        #[arg(long, value_parser = present)]
        run_id: Option<String>,

        /// The host's session id, from which the run id is derived.
        ///
        /// The other half of `--run-id`: a host whose hook event carries the
        /// session id and no run id gives this one, and the derivation stays
        /// here rather than being repeated in the host.
        #[arg(long, value_parser = session)]
        session_id: Option<String>,

        /// The runtime whose run ids `--session-id` derives.
        #[arg(long, value_parser = runtime)]
        runtime: Option<String>,

        /// The root session this call's session was started under.
        ///
        /// For a host whose delegated contexts are separate processes with
        /// session ids of their own: the call is decided as the run derived
        /// from this id and `--runtime` would be, so a child is measured against
        /// its parent's claim and not against a run that holds none. It names no
        /// run of its own and is refused with `--run-id`. Like `--run-id` it is
        /// an assertion the binary cannot check.
        #[arg(long, value_parser = session)]
        parent_session_id: Option<String>,

        /// The caller can put a question to a person and carry back the answer.
        ///
        /// Honoured with `--runtime pi` only. A decision that would be asked is
        /// then answered with exit code 4 and the question on standard error;
        /// without it every such decision stays a refusal, exit 1, so a caller
        /// that never passed it cannot be sent a code it cannot read.
        #[arg(long)]
        can_ask: bool,
    },

    /// Answer whether this session's Estigia run still holds a live claim, on
    /// which issue — and, when it holds none, how many issues it could claim —
    /// or act on the goal through a subcommand.
    ///
    /// Read-only for the bare verb and for `complete`: they write no pointer, no
    /// ledger and no tracker byte, and always exit 0 for a read answer. An
    /// identity that does not name exactly one run is refused, naming the road
    /// that clears it. `progress` and `blocked` are the subcommands that renew
    /// the run's claim; `stop-after` writes the local request that ends the
    /// loop once the claim is released.
    Goal {
        /// The built-in subcommand, when one is given.
        #[command(subcommand)]
        action: Option<GoalAction>,

        /// This run's id.
        ///
        /// One of the two identity roads: the id itself, exactly as
        /// `SessionStart` reported it.
        #[arg(long, global = true, value_parser = present)]
        run_id: Option<String>,

        /// The host's session id, from which the run id is derived.
        ///
        /// The other road: a host that carries the session id and no run id
        /// gives this one with `--runtime`, and the derivation stays here rather
        /// than being repeated in the host.
        #[arg(long, global = true, value_parser = session)]
        session_id: Option<String>,

        /// The runtime whose run ids `--session-id` derives.
        #[arg(long, global = true, value_parser = runtime)]
        runtime: Option<String>,

        /// Also report how many issues this run's own delivery has closed.
        ///
        /// Off by default: the count is enumerated and verified against the
        /// tracker, so it costs reads the plain liveness answer does not.
        #[arg(long)]
        completed: bool,
    },

    /// Give a host with no lifecycle hooks what `SessionStart` and `SessionEnd`
    /// give an agent that has them.
    ///
    /// Not typed by a person: the Pi extension runs it. `start` prints the text
    /// `SessionStart` injects, `end` forgets the run's pointer when the session
    /// is really over.
    Session {
        /// The lifecycle step.
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Relay a sealed reviewer the host launched: materialise its exact prompt,
    /// or admit the raw answer it produced.
    ///
    /// Not typed by a person: the OpenCode review relay plugin runs it, on a
    /// machine with neither reviewer CLI installed. `prompt` writes the exact
    /// bytes the binary built to standard output; `admit` reads the reviewer's
    /// raw answer on standard input and binds it to the job's receipt.
    Review {
        /// The built-in subcommand.
        #[command(subcommand)]
        action: ReviewAction,

        /// This run's id, exactly as `SessionStart` reported it.
        #[arg(long, global = true, value_parser = present)]
        run_id: Option<String>,

        /// The host's session id, from which the run id is derived.
        #[arg(long, global = true, value_parser = session)]
        session_id: Option<String>,

        /// The runtime whose run ids `--session-id` derives.
        #[arg(long, global = true, value_parser = runtime)]
        runtime: Option<String>,
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

/// What `estigia status` does, beyond saying what is installed.
#[derive(Debug, Subcommand)]
pub enum StatusAction {
    /// Set this run's self-reported status: one line, local to the store.
    ///
    /// Never renews a claim, changes a state, or counts as evidence; two writes
    /// within a minute leave one.
    Set {
        /// What this run is doing and what comes next.
        text: String,

        /// The coarse phase.
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(crate::harness::status::PHASES))]
        phase: Option<String>,

        /// This run's id; the ambient `ESTIGIA_RUN_ID` when omitted.
        #[arg(long, value_parser = present)]
        run_id: Option<String>,

        /// This session's id, from which its run id is derived.
        #[arg(long, value_parser = session)]
        session_id: Option<String>,

        /// The runtime whose run ids `--session-id` derives.
        #[arg(long, value_parser = runtime)]
        runtime: Option<String>,
    },
}

/// What `estigia sessions` does beyond listing.
#[derive(Debug, Subcommand)]
pub enum SessionsAction {
    /// Put a message in another session's mailbox. It is delivered at that
    /// session's next hook point and recorded as delivered when it is.
    Send {
        /// The recipient's run id, as `estigia sessions` lists it.
        #[arg(value_parser = present)]
        session: String,

        /// The message.
        #[arg(conflicts_with = "file", required_unless_present = "file")]
        text: Option<String>,

        /// Read the message from this file instead.
        #[arg(long)]
        file: Option<std::path::PathBuf>,

        /// The sending run's id; the ambient `ESTIGIA_RUN_ID` when omitted.
        #[arg(long, value_parser = present)]
        run_id: Option<String>,

        /// The sending session's id, from which its run id is derived.
        #[arg(long, value_parser = session)]
        session_id: Option<String>,

        /// The runtime whose run ids `--session-id` derives.
        #[arg(long, value_parser = runtime)]
        runtime: Option<String>,
    },
}

/// What `estigia session` does.
#[derive(Debug, Subcommand)]
pub enum SessionAction {
    /// Print the context a session begins with: the run id, what it holds, and
    /// what stands in its way. Read-only apart from the daily housekeeping
    /// `SessionStart` does.
    Start {
        /// The host's session id, from which the run id is derived.
        #[arg(long, value_parser = session)]
        session_id: String,

        /// The runtime whose run ids `--session-id` derives.
        #[arg(long, value_parser = runtime)]
        runtime: String,

        /// Why the session began, as the host names it.
        #[arg(long)]
        reason: Option<String>,

        /// The session the host left to begin this one.
        #[arg(long, value_parser = session)]
        previous_session_id: Option<String>,

        /// How this host wakes the session when mail arrives while it is idle:
        /// `poll` (it asks repeatedly), or `stop-waiter` (a process parks at the
        /// end of a turn). Omitted when the adapter records nothing, and the
        /// row then says so rather than guessing.
        #[arg(long, value_parser = ["poll", "stop-waiter"])]
        wake: Option<String>,

        /// The file the host keeps this session in, recorded so `resume` can
        /// name it. Pi resumes from a file and from nothing else.
        #[arg(long, value_parser = present)]
        session_file: Option<String>,
    },
    /// Print the messages other sessions left for this one, and record them as
    /// delivered. Prints nothing when none wait.
    Deliver {
        /// The host's session id, from which the run id is derived.
        #[arg(long, value_parser = session)]
        session_id: String,

        /// The runtime whose run ids `--session-id` derives.
        #[arg(long, value_parser = runtime)]
        runtime: String,

        /// Print the pending messages and the id of the last one, and record nothing.
        #[arg(long, conflicts_with = "through")]
        peek: bool,

        /// Take only the messages up to this id, as `--peek` reported it.
        #[arg(long)]
        through: Option<i64>,
    },
    /// Forget the run's pointer, and only when the session is over. The claim
    /// on the tracker is untouched.
    End {
        /// The host's session id, from which the run id is derived.
        #[arg(long, value_parser = session)]
        session_id: String,

        /// The runtime whose run ids `--session-id` derives.
        #[arg(long, value_parser = runtime)]
        runtime: String,

        /// Why the session ended, as the host names it. A session that was only
        /// replaced (`new`, `resume`, `fork`, `reload`) keeps its pointer.
        #[arg(long)]
        reason: Option<String>,
    },
}

/// What `estigia goal` does, beyond the bare read.
#[derive(Debug, Subcommand)]
pub enum GoalAction {
    /// Record that this run is still working, as timeline activity that renews its claim.
    Progress {
        /// What this run is working on.
        note: String,
    },
    /// Record that this run is waiting, as timeline activity that renews its claim.
    Blocked {
        /// What this run is waiting for.
        reason: String,
    },
    /// Ask the authority whether this goal is complete. Read-only.
    Complete,
    /// Ask the loop to stop once this run's claim is released.
    StopAfter {
        /// Why. Recorded with the request.
        #[arg(long)]
        reason: Option<String>,
        /// Take back a request rather than making one.
        #[arg(long)]
        clear: bool,
    },
    /// Store this session's loop: the objective `/flow-loop` was given, with the
    /// completion cap in its text. The `Stop` hook continues it.
    Start {
        /// The objective, and an optional `--max <n>`, exactly as given.
        #[arg(value_name = "OBJECTIVE", allow_hyphen_values = true)]
        text: String,
    },
}

/// What `estigia store` does.
#[derive(Debug, Subcommand)]
pub enum StoreAction {
    /// Import every open issue the store does not hold.
    Import,
}

/// What `estigia review` does for a relayed reviewer.
#[derive(Debug, Subcommand)]
pub enum ReviewAction {
    /// Materialise the exact prompt of the pending reviewer job on standard output.
    Prompt {
        /// The reserved reviewer name the host is launching.
        #[arg(long)]
        name: String,
    },
    /// Print the model the `reviewer` routing row named, or nothing when it named none.
    Model {
        /// The reserved reviewer name the host is launching.
        #[arg(long)]
        name: String,
    },
    /// Admit the reviewer's raw answer, read from standard input.
    Admit {
        /// The reserved reviewer name the host launched.
        #[arg(long)]
        name: String,
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
        ///
        /// Without it, a save that moves a row answered per agent is refused.
        #[arg(long)]
        agent: Option<String>,
    },
    /// Set one setting, validating it before anything is written.
    ///
    /// A row answered per agent (profile, authorisations, model routing, the
    /// plugins) is refused without `--agent`. The answer names the file written.
    Set {
        /// The setting's label, as it appears in the table.
        setting: String,
        /// The new value.
        value: String,
        /// Which agent's installed contract to write. Required for a row
        /// answered per agent.
        #[arg(long, conflicts_with = "repo")]
        agent: Option<String>,
        /// Write it into this repository's own file, under `.git/`.
        ///
        /// The door the layer needs: reading it has been there since it
        /// existed, and a file nobody can create without a text editor is a
        /// feature nobody has.
        #[arg(long)]
        repo: bool,
        /// Write the agent's shared contract table instead of its own file.
        ///
        /// Only with `--agent`. For an agent that shares a skill root this is
        /// the table every agent without a file of its own reads, so a row
        /// answered per agent is refused there; set it without `--contract`.
        #[arg(long, conflicts_with = "repo")]
        contract: bool,
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
