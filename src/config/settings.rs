//! The settings table, as data.
//!
//! One list, walked by the parser, the renderer, the `setup` screen and the
//! seam tests. The alternative — a `match` in the parser, a `write!` in the
//! renderer and a list of questions in the wizard — is three places that drift,
//! and the drift is silent because a row nobody parses simply has no effect.

use std::path::PathBuf;
use std::time::Duration;

use crate::outcome::{NoCommandReason, Refusal, Resolution};

use super::{
    Authority, Config, DEFAULT_ASK_TIMEOUT, Evidence, GitOperationWindow, IssueRequirement,
    Language, LoopPlugin, Profile, PublicationLane, RepoRef, RepositoryHost, ReviewBudget, Tracker,
    Workflow, WorktreeRoot, delivery,
};
use crate::setup::AgentAdapter;

/// One configurable setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// The whole posture in one word, over the two authorisations that can ask.
    Profile,
    /// Who authorises delivery.
    Delivery,
    /// Who authorises a workflow-state transition.
    Transitions,
    /// Where isolated checkouts are made.
    Worktree,
    /// Which tracker holds the issues.
    Tracker,
    /// Where the code lives and publications are verified.
    RepositoryHost,
    /// Which model each target runs on. A role named here is installed.
    Models,
    /// Where work integrates.
    Integration,
    /// How long a routine write may ride on the previous verification.
    Window,
    /// The daily range commits, pushes, and branch creation may run in.
    GitOperationWindow,
    /// What a verdict here has to be backed by, and so what a reviewer may do.
    Evidence,
    /// Whether accepted verdicts consult the publication lane.
    PublicationLane,
    /// The per-context token figure a review is reported against. Advisory:
    /// nothing here refuses a panel that runs over it.
    ReviewBudget,
    /// How large a change may get before it is split. Guidance, and the
    /// contract says so — not a gate, because Estigia gates writes and a pull
    /// request is not one.
    ChangeSize,
    /// How many corrections one review lineage may spend before a person decides.
    CorrectionsPerLineage,
    /// Commands this repository treats as irreversible.
    Boundaries,
    /// The language the plain-language summary sentence is written in.
    /// issue-flow's long label is kept as an alias, so the value on somebody's
    /// disk goes on meaning what it always meant.
    Summary,
    /// The language the rest of the issue body is written in.
    Body,
    /// What a run holding no claim may write in a checkout that has a tracker.
    IssueRequirement,
    /// Who witnesses a claim in this repository: Estigia, or the team.
    Workflow,
    /// The delivery rows in one word: a preset that fills the five it names.
    DeliveryPreset,
    /// Estigia's workflow states mapped onto the tracker's own state names.
    StateNames,
    /// Whether the run reviews its own change before it is marked ready.
    SelfReview,
    /// Who marks the pull request ready for review.
    ReadyBy,
    /// The CI checks and people whose reviews a pull request waits on, in order.
    ExternalReviewers,
    /// What a correction does to a pull request that is already ready.
    CorrectionsAfterReady,
    /// Who merges the reviewed pull request.
    MergeBy,
    /// Whether OpenCode's optional loop plugin is written.
    LoopPlugin,
    /// Whether OpenCode also gets the generic `/goal` and `/loop` plugin.
    GoalLoopPlugin,
}

/// Every setting, in the order they are written into the table.
///
/// The order is the groups, one after another — see [`Setting::group`] — and
/// within a group the rows are ordered by what they are about rather than by
/// which file answers them. Every per-screen list is this table filtered by a
/// stated predicate, never a second order written by hand.
///
/// Exhaustiveness is checked by a test rather than by the compiler, because a
/// slice cannot be matched — see `every_setting_is_in_the_table`.
pub const SETTINGS: &[Setting] = &[
    // Authority. First because it is applied first: naming a profile writes the
    // two authorisations below it, and an explicit authorisation that follows
    // wins, for the same reason a later row wins over an earlier one anywhere
    // in this table.
    Setting::Profile,
    Setting::Delivery,
    Setting::Transitions,
    // Repository & tracker.
    Setting::Tracker,
    Setting::RepositoryHost,
    Setting::IssueRequirement,
    Setting::Integration,
    Setting::Worktree,
    // Delivery, in the order the steps happen. The preset first, for the
    // reason `Profile` leads Authority: it writes the rows below it, and a row
    // that follows wins.
    Setting::DeliveryPreset,
    Setting::Workflow,
    Setting::StateNames,
    Setting::SelfReview,
    Setting::ReadyBy,
    Setting::ExternalReviewers,
    Setting::CorrectionsAfterReady,
    Setting::MergeBy,
    // Gate limits.
    Setting::Boundaries,
    Setting::Window,
    Setting::GitOperationWindow,
    // Review & change.
    Setting::Evidence,
    Setting::PublicationLane,
    Setting::ReviewBudget,
    Setting::ChangeSize,
    Setting::CorrectionsPerLineage,
    // Agent runtime.
    Setting::Models,
    Setting::LoopPlugin,
    Setting::GoalLoopPlugin,
    // This machine.
    Setting::Summary,
    Setting::Body,
];

/// The heading a setting sits under, so the screen, `config list` and the
/// reference show the same groups in the same order.
///
/// A group is a *theme*, not a scope: `Repository & tracker` holds both the
/// facts every checkout shares and `Unclaimed write policy`, which only a
/// repository's own file answers, because they are one question — where the
/// work lives — and the scope column already says which is which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Who authorises the two things that can ask.
    Authority,
    /// Where the issues live and how work integrates.
    Repository,
    /// Who takes each step from a finished change to its merge.
    Delivery,
    /// What the gate will not let a run do, and how long a decision rides.
    GateLimits,
    /// What a verdict must be backed by, and how large a change may get.
    ReviewAndChange,
    /// Which model each role runs on, and which optional plugins are written.
    AgentRuntime,
    /// The languages this machine's prose is written in.
    Language,
}

impl Group {
    /// Every group, in the order they are shown.
    pub const ALL: &[Group] = &[
        Group::Authority,
        Group::Repository,
        Group::Delivery,
        Group::GateLimits,
        Group::ReviewAndChange,
        Group::AgentRuntime,
        Group::Language,
    ];

    /// The heading, as the English the screen, the listing and the reference
    /// all show. `This machine` rather than `Language` because the group's
    /// scope is the point: the rows do not change because a checkout changed.
    pub fn label(self) -> &'static str {
        match self {
            Self::Authority => "Authority",
            Self::Repository => "Repository & tracker",
            Self::Delivery => "Delivery",
            Self::GateLimits => "Gate limits",
            Self::ReviewAndChange => "Review & change",
            Self::AgentRuntime => "Agent runtime",
            Self::Language => "This machine",
        }
    }
}

/// Who a setting's answer belongs to.
///
/// Every row lands in the same table, so this changes nothing about how a
/// setting is stored — [`crate::skill::installed_config_for`] already layers a
/// shared contract under a per-adapter file. What it changes is who is *asked*:
/// a repository has one tracker whichever agent looks at it, and asking each
/// agent separately implies otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Differs by agent: the answer depends on which one is holding the tools.
    Agent,
    /// A fact about the repository, the same whichever agent asks.
    Everywhere,
    /// What **this repository** requires, answered in its own file and nowhere
    /// else: there is no machine-wide value for it to be tightened from.
    Repository,
    /// A fact about **this machine**, the same whichever repository is open:
    /// a person writes in one language across every checkout they have; a
    /// tracker belongs to one of them.
    Machine,
}

impl Scope {
    /// The one-word name this scope is shown under, in `config list`, the
    /// reference table and the screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Everywhere => "everywhere",
            Self::Repository => "repository",
            Self::Machine => "machine",
        }
    }
}

/// The answers one setting offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answers {
    /// The answers, in the order they are offered.
    pub choices: &'static [&'static str],
    /// Whether anything outside [`Self::choices`] is refused.
    pub closed: bool,
}

impl Answers {
    /// A closed vocabulary: these and nothing else.
    const fn all(choices: &'static [&'static str]) -> Self {
        Self {
            choices,
            closed: true,
        }
    }

    /// Useful answers, with somewhere to type the rest.
    const fn some(choices: &'static [&'static str]) -> Self {
        Self {
            choices,
            closed: false,
        }
    }

    /// Where this value sits in the list, if it is one of them.
    pub fn at(&self, value: &str) -> Option<usize> {
        self.choices.iter().position(|choice| *choice == value)
    }

    /// The answer `delta` places along from this one, wrapping.
    ///
    /// A value that is not in the list — somebody's own model name, a typed
    /// duration — steps onto the **first** choice rather than nowhere: an arrow
    /// key that does nothing on some rows and not others reads as broken, and
    /// there is no sensible neighbour to a value the list has never seen.
    pub fn step(&self, from: &str, delta: isize) -> Option<&'static str> {
        if self.choices.is_empty() {
            return None;
        }
        let count = self.choices.len() as isize;
        let next = match self.at(from) {
            Some(at) => (at as isize + delta).rem_euclid(count),
            None => 0,
        };
        self.choices.get(next as usize).copied()
    }
}

/// The settings that differ by agent, in table order.
///
/// These are the ones whose answer depends on *who is working*, and every one
/// of them is an instruction the **agent** reads and follows — which is what
/// makes a per-agent answer possible at all. A row the gate enforces cannot
/// live here, because the gate does not know which agent it is answering — see
/// [`Setting::Window`] under [`EVERYWHERE_SETTINGS`], which used to.
pub const AGENT_SETTINGS: &[Setting] = &[
    Setting::Profile,
    Setting::Delivery,
    Setting::Transitions,
    Setting::Models,
    Setting::LoopPlugin,
    Setting::GoalLoopPlugin,
];

/// The settings that are the same whichever agent asks, in table order.
///
/// Not every row here is read the same way. Most are read by an agent, out
/// of the prose, or by the transport — but nine rows reach the gate's
/// `GateContext`, the seven here and two of the repository-only rows, and eight
/// of them feed its own allow/deny decision: `Repository host` is carried so
/// MCP publication tools can bind to it rather than so `decide` branches on
/// it, and `Workflow` decides only for the pointer an `assisted` run left.
/// Every write through a repository's own `estigia.local.md` — whichever row
/// it names, including one of those nine — is a `Boundary` and re-reads the
/// tracker rather than riding the renewal window like an ordinary write.
/// [`Setting::decides_what_the_gate_enforces`] names them as an exhaustive
/// `match` a new row here has to answer rather than default past;
/// `docs/configuration.md`'s *Which root the gate decides in* section carries
/// the rest. Deciding a new row's scope is this list; deciding whether the
/// gate itself consults it is that one.
pub const EVERYWHERE_SETTINGS: &[Setting] = &[
    Setting::Tracker,
    Setting::RepositoryHost,
    Setting::Integration,
    Setting::Worktree,
    // The renewal window is here, not beside the agent settings, because it is
    // enforced by the **gate** rather than obeyed by the agent; declared per
    // agent it was a row no decision would ever consult.
    Setting::Boundaries,
    Setting::Window,
    Setting::GitOperationWindow,
    Setting::Evidence,
    Setting::PublicationLane,
    Setting::ReviewBudget,
    Setting::ChangeSize,
    Setting::CorrectionsPerLineage,
];

/// The rows that belong to this machine, in table order.
///
/// Two, and both are languages: what somebody writes in does not change
/// because they opened a different checkout.
pub const MACHINE_SETTINGS: &[Setting] = &[Setting::Summary, Setting::Body];

/// The rows only a repository's own file answers, in table order.
pub const REPOSITORY_SETTINGS: &[Setting] = &[
    Setting::IssueRequirement,
    Setting::DeliveryPreset,
    Setting::Workflow,
    Setting::StateNames,
    Setting::SelfReview,
    Setting::ReadyBy,
    Setting::ExternalReviewers,
    Setting::CorrectionsAfterReady,
    Setting::MergeBy,
];

/// Everything the options page offers, in table order.
///
/// The repository's rows, then this machine's, which is the order they are
/// drawn in because it is the order [`SETTINGS`] declares them: the options
/// page is [`SETTINGS`] filtered to everything that is not the agent's alone,
/// and a second order written here by hand is a second order that drifts.
pub const OPTIONS_SETTINGS: &[Setting] = &[
    Setting::Tracker,
    Setting::RepositoryHost,
    Setting::IssueRequirement,
    Setting::Integration,
    Setting::Worktree,
    Setting::DeliveryPreset,
    Setting::Workflow,
    Setting::StateNames,
    Setting::SelfReview,
    Setting::ReadyBy,
    Setting::ExternalReviewers,
    Setting::CorrectionsAfterReady,
    Setting::MergeBy,
    Setting::Boundaries,
    Setting::Window,
    Setting::GitOperationWindow,
    Setting::Evidence,
    Setting::PublicationLane,
    Setting::ReviewBudget,
    Setting::ChangeSize,
    Setting::CorrectionsPerLineage,
    Setting::Summary,
    Setting::Body,
];

impl Setting {
    /// Whether this agent-scoped row gives `adapter` anything to mean.
    ///
    /// The loop, which OpenCode reads as a plugin and Codex as hook entries — see
    /// [`crate::setup::AgentAdapter::offers_loop`] — and the generic goal and loop
    /// plugin, which OpenCode and Pi read. The row is offered only where it can
    /// act, rather than putting a switch that does nothing on the agents that
    /// read neither; the TUI's per-agent row
    /// builder and its writer both consult this, so the screen and the file it
    /// writes agree on which rows a given adapter owns.
    pub fn applies_to_agent(self, adapter: &AgentAdapter) -> bool {
        match self {
            Self::LoopPlugin => adapter.offers_loop(),
            Self::GoalLoopPlugin => adapter.has_loop_plugin(),
            _ => true,
        }
    }

    /// Whether this row's answer is a directory on this machine.
    ///
    /// One row, today. It is asked rather than matched at the call site so the
    /// screen that offers folders for it does not have to know which row that
    /// is — and so the day a second one arrives, it arrives here.
    pub fn takes_a_directory(self) -> bool {
        matches!(self, Self::Worktree)
    }

    /// Whether a repository's own file may **tighten** this agent row.
    ///
    /// The authorisations stay [`Scope::Agent`], so every match on scope is
    /// unchanged; this is the one predicate that says a repository may still
    /// answer `ask` over an agent's `auto`, and never the reverse.
    pub fn repository_may_tighten(self) -> bool {
        matches!(self, Self::Delivery | Self::Transitions)
    }

    /// The authority this row holds in `config`, for the two that have one.
    pub fn authority_in(self, config: &Config) -> Option<Authority> {
        match self {
            Self::Delivery => Some(config.delivery),
            Self::Transitions => Some(config.transitions),
            _ => None,
        }
    }

    /// Whether `candidate` is no looser than `base` on this row: it asks, or
    /// `base` is already `auto`. An `ask` is never looser than another `ask`,
    /// whatever their durations.
    pub fn tightens(self, base: &Config, candidate: &Config) -> bool {
        !matches!(
            (self.authority_in(base), self.authority_in(candidate)),
            (Some(Authority::Ask { .. }), Some(Authority::Auto))
        )
    }

    /// Whose answer this is. The split is by *what the answer is about*, not
    /// by what happens to be convenient.
    pub fn scope(self) -> Scope {
        match self {
            Self::Profile
            | Self::Delivery
            | Self::Transitions
            | Self::Models
            | Self::LoopPlugin
            | Self::GoalLoopPlugin => Scope::Agent,
            Self::ChangeSize
            | Self::CorrectionsPerLineage
            | Self::Window
            | Self::GitOperationWindow
            | Self::Worktree
            | Self::Tracker
            | Self::RepositoryHost
            | Self::Integration
            // Everywhere rather than per agent, and the suite is what said so:
            // the gate reads this row to render the reserved reviewer's grant,
            // and a row the gate reads cannot have a different answer depending
            // on which agent asked — there is one gate.
            | Self::Evidence
            | Self::PublicationLane
            | Self::ReviewBudget
            | Self::Boundaries => Scope::Everywhere,
            // Written across every checkout somebody has, so they belong to
            // the person rather than to the repository.
            Self::Summary | Self::Body => Scope::Machine,
            Self::IssueRequirement
            | Self::Workflow
            | Self::DeliveryPreset
            | Self::StateNames
            | Self::SelfReview
            | Self::ReadyBy
            | Self::ExternalReviewers
            | Self::CorrectionsAfterReady
            | Self::MergeBy => Scope::Repository,
        }
    }

    /// The heading this row sits under. The split is by *what the row is
    /// about*; every per-screen list is [`SETTINGS`] filtered, so a row's group
    /// decides where it is drawn, listed and documented without a second order
    /// written anywhere.
    ///
    /// Exhaustive rather than wildcarded on purpose: a new row must fail to
    /// compile here until somebody has said which group it belongs to.
    pub fn group(self) -> Group {
        match self {
            Self::Profile | Self::Delivery | Self::Transitions => Group::Authority,
            Self::Tracker
            | Self::RepositoryHost
            | Self::IssueRequirement
            | Self::Integration
            | Self::Worktree => Group::Repository,
            Self::DeliveryPreset
            | Self::Workflow
            | Self::StateNames
            | Self::SelfReview
            | Self::ReadyBy
            | Self::ExternalReviewers
            | Self::CorrectionsAfterReady
            | Self::MergeBy => Group::Delivery,
            Self::Boundaries | Self::Window | Self::GitOperationWindow => Group::GateLimits,
            Self::Evidence
            | Self::PublicationLane
            | Self::ReviewBudget
            | Self::ChangeSize
            | Self::CorrectionsPerLineage => Group::ReviewAndChange,
            Self::Models | Self::LoopPlugin | Self::GoalLoopPlugin => Group::AgentRuntime,
            Self::Summary | Self::Body => Group::Language,
        }
    }

    /// This row's scope as one word, for the surfaces that show it beside the
    /// row rather than in a sentence.
    pub fn scope_label(self) -> &'static str {
        self.scope().label()
    }

    /// Whether this row is one the **gate** reads to decide what it enforces,
    /// rather than one an agent reads and follows.
    ///
    /// A row here reaches [`crate::harness::GateContext`], so a repository
    /// write to it changes what the gate does on the *next* decision, not what
    /// the agent holding the tools does on this one. `harness::GateContext`
    /// carries exactly these as fields built from the contract —
    /// `tracker`, `repository_host`, `boundaries`, `window`, `evidence`,
    /// `integration`, `operation_window`, `issue_requirement` and `workflow`,
    /// the last an `Option` that is `None` when the contract could not be read —
    /// and `tests/honesty.rs`'s
    /// `every_setting_the_gate_enforces_is_one_gate_context_actually_carries`
    /// crosses that source against this predicate. Every other
    /// `Scope::Everywhere` row is read by an agent or by the transport, never
    /// by the gate's own decision.
    ///
    /// This predicate does **not** decide `harness::classify_with`'s answer:
    /// every write to the repository's own `estigia.local.md` is a `Boundary`
    /// now, whichever row it names — see `docs/honesty.md` for why reading a
    /// payload per row could not hold.
    ///
    /// Written as an exhaustive `match` rather than a wildcard arm, on purpose:
    /// a row added later must fail to compile here until somebody has answered
    /// *does the gate act on this*.
    pub fn decides_what_the_gate_enforces(self) -> bool {
        match self {
            Self::Evidence
            | Self::Tracker
            | Self::RepositoryHost
            | Self::Boundaries
            | Self::Window
            | Self::GitOperationWindow
            | Self::Integration
            | Self::IssueRequirement
            | Self::Workflow => true,
            Self::Delivery
            | Self::Transitions
            | Self::Profile
            | Self::Worktree
            | Self::Models
            | Self::PublicationLane
            | Self::ReviewBudget
            | Self::ChangeSize
            | Self::CorrectionsPerLineage
            | Self::Summary
            | Self::Body
            | Self::LoopPlugin
            | Self::GoalLoopPlugin
            | Self::DeliveryPreset
            | Self::StateNames
            | Self::SelfReview
            | Self::ReadyBy
            | Self::ExternalReviewers
            | Self::CorrectionsAfterReady
            | Self::MergeBy => false,
        }
    }

    /// One line saying what this setting decides, for somebody meeting it.
    ///
    /// The table already carries [`Self::accepted`], which says what may be
    /// *typed*. That is a different question from what the row is *for*, and an
    /// operator who does not know the second cannot use the first.
    pub fn about(self) -> &'static str {
        match self {
            Self::Profile => {
                "the two authorisations that can ask, at once: `auto`, `ask`, or a mix an explicit row overrides"
            }
            Self::Delivery => "whether this agent may deliver a reviewed change, or has to ask",
            Self::Transitions => "whether this agent may move a task between states on its own",
            Self::Worktree => "where isolated checkouts are made, when a run needs one",
            Self::Tracker => "where this repository's issues live — the claim is adjudicated there",
            Self::RepositoryHost => {
                "where the code lives and publications are verified — pull requests and tags"
            }
            Self::Models => {
                "which model, and at what effort, each role runs on, for this agent — and which roles this agent has a definition for"
            }
            Self::Integration => "whether work integrates through branches or straight onto trunk",
            Self::Window => "how long a routine write may ride on the last verification",
            Self::GitOperationWindow => {
                "the daily range commits, pushes, and branch creation may run in"
            }
            Self::Evidence => {
                "what a verdict here has to be backed by, and so what a reviewer may do"
            }
            Self::PublicationLane => {
                "whether an accepted verdict consults the publication lane for this head"
            }
            Self::ReviewBudget => {
                "the per-context token figure a review is reported against, and nothing more"
            }
            Self::ChangeSize => "how many changed lines a pull request aims to stay under",
            Self::CorrectionsPerLineage => {
                "how many corrections one review lineage may spend before a person decides"
            }
            Self::IssueRequirement => {
                "whether a run holding no claim may write in this repository: asked each time, \
                 allowed, or refused"
            }
            Self::Workflow => {
                "who witnesses a claim in this repository: Estigia writes the tracker, or the \
                 team coordinates and Estigia does not"
            }
            Self::DeliveryPreset => {
                "the delivery rows below at once: a known flow in one choice, which any row after \
                 it overrides"
            }
            Self::StateNames => {
                "the tracker's own name for each state Estigia moves an issue into, under a \
                 Linear tracker"
            }
            Self::SelfReview => {
                "whether the run reviews its own change in a fresh context before it is marked \
                 ready"
            }
            Self::ReadyBy => "who marks the draft pull request ready for review",
            Self::ExternalReviewers => {
                "the CI checks and people a ready pull request waits on, in order: a stage starts \
                 only once every earlier one is complete"
            }
            Self::CorrectionsAfterReady => {
                "what the run does to a pull request that is already ready when it has a \
                 correction to publish"
            }
            Self::MergeBy => "who merges the reviewed pull request",
            Self::Boundaries => "the commands this repository treats as one-way doors",
            Self::Summary => "the language the summary sentence at the top of an issue is in",
            Self::Body => "the language the rest of an issue body is written in",
            Self::LoopPlugin => {
                "whether this agent gets the loop that continues a session while the run still \
                 holds its issue or another is claimable"
            }
            Self::GoalLoopPlugin => {
                "whether this agent also gets the generic plugin that offers `/goal` and `/loop`, \
                 which consults no tracker and runs no Estigia command"
            }
        }
    }

    /// The answers this setting offers, and whether they are all of them.
    ///
    /// `closed` says whether anything outside the list is refused. Where it is
    /// false the list is a set of useful answers rather than the whole
    /// vocabulary — an absolute path, a model name, a language — and the screen
    /// still has to offer somewhere to type.
    pub fn answers(self) -> Answers {
        // Every entry here must be a value `apply` accepts **and** one that
        // `value_of` writes back the same way, or the screen would show a list
        // whose selected entry is never the one that is set. Held by
        // `every_offered_answer_round_trips`.
        match self {
            // `custom` is accepted and shown when the rows match no profile,
            // but it is not offered: there is nothing to choose about a mix,
            // and picking it would mean nothing.
            Self::Profile => Answers::some(&["balanced", "cautious"]),
            // `ask` alone is the built-in timeout, so the two written out are
            // longer than it — offering `ask 15m` would render back as plain
            // `ask` and never look chosen.
            Self::Delivery | Self::Transitions => {
                Answers::some(&["auto", "ask", "ask 30m", "ask 2h"])
            }
            Self::Worktree => Answers::some(&["unset"]),
            Self::Tracker => Answers::some(&["github"]),
            Self::RepositoryHost => Answers::some(&["github"]),
            Self::Models => Answers::some(&["unset", "by task"]),
            Self::Integration => Answers::all(&["branch", "trunk"]),
            Self::Window => Answers::some(&["default", "1m", "30s"]),
            Self::GitOperationWindow => Answers::some(&[
                "disabled",
                "enabled 09:00-17:00",
                "enabled 22:00-06:00 inverted",
                "enabled mon-fri 08:00-17:00 inverted",
            ]),
            Self::Evidence => Answers::all(&["reading", "measuring"]),
            Self::PublicationLane => Answers::all(&["on", "off"]),
            Self::ReviewBudget => Answers::some(&["none", "150000"]),
            Self::ChangeSize => Answers::some(&["800", "400"]),
            Self::CorrectionsPerLineage => Answers::some(&["3", "1"]),
            Self::IssueRequirement => Answers::all(&["ask", "off", "required"]),
            Self::Workflow => Answers::all(&["autonomous", "assisted"]),
            // `custom` is accepted and not offered, for the reason `Profile`'s is.
            Self::DeliveryPreset => Answers::some(&["autonomous", "assisted", "team"]),
            Self::StateNames => Answers::some(&[
                "none",
                "in-progress=In Progress, review=In Review, done=Done",
            ]),
            Self::SelfReview => Answers::all(&["on", "off"]),
            Self::ReadyBy | Self::MergeBy => Answers::all(&["agent", "person"]),
            Self::ExternalReviewers => {
                Answers::some(&["none", "check:cubic@cubic-dev-ai[bot], review:any"])
            }
            Self::CorrectionsAfterReady => Answers::all(&["refuse", "redraft", "push-ready"]),
            Self::Boundaries => Answers::some(&[
                "none",
                "git push --force",
                "gh release create",
                "gh repo delete",
                "npm publish",
            ]),
            Self::Summary | Self::Body => Answers::some(&["English", "Español"]),
            Self::LoopPlugin | Self::GoalLoopPlugin => Answers::all(&["on", "off"]),
        }
    }

    /// The label written into the first column.
    pub fn label(self) -> &'static str {
        match self {
            Self::Profile => "Authorisation posture",
            Self::Delivery => "Delivery authorisation",
            Self::Transitions => "Transition authorisation",
            Self::Worktree => "Worktree location",
            Self::Tracker => "Tracker",
            Self::RepositoryHost => "Repository host",
            Self::Models => "Model routing",
            Self::Integration => "Integration mode",
            Self::Window => "Renewal window",
            Self::GitOperationWindow => "Git operation window",
            Self::Evidence => "Evidence standard",
            Self::PublicationLane => "Publication lane",
            Self::ReviewBudget => "Review budget",
            Self::ChangeSize => "Change size",
            Self::CorrectionsPerLineage => "Corrections per lineage",
            Self::Boundaries => "Irreversible commands",
            Self::Summary => "Summary language",
            Self::Body => "Issue body language",
            Self::IssueRequirement => "Unclaimed write policy",
            Self::Workflow => "Claim workflow",
            Self::DeliveryPreset => "Delivery preset",
            Self::StateNames => "State names",
            Self::SelfReview => "Self review",
            Self::ReadyBy => "Ready by",
            Self::ExternalReviewers => "External reviewers",
            Self::CorrectionsAfterReady => "Corrections after ready",
            Self::MergeBy => "Merge by",
            Self::LoopPlugin => "Loop plugin",
            Self::GoalLoopPlugin => "Goal loop plugin",
        }
    }

    /// Labels this setting also answers to: issue-flow shipped the long
    /// spelling, which is in operators' files today. Estigia writes the
    /// shorter label and keeps reading the old one, so upgrading does not
    /// quietly reset anyone's language to English.
    ///
    /// Every rename goes through here rather than replacing the old label: a
    /// file somebody already wrote keeps parsing, because
    /// [`Self::from_label`] matches aliases as well as labels.
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            // Read as the new `Issue body language` instead, an old spelling
            // would move the answer onto a row that decides something
            // different.
            Self::Summary => &[
                "Task body language",
                "\"Description for dumb humans\" sentence language",
            ],
            // The names these rows carried before the groups landed. Each is
            // the exact label an installed table writes, so nothing on disk
            // stops meaning what it meant.
            Self::Profile => &["Profile"],
            Self::Integration => &["Integration"],
            Self::Workflow => &["Workflow"],
            Self::IssueRequirement => &["Issue requirement"],
            _ => &[],
        }
    }

    /// The setting a table label names, if any.
    pub fn from_label(label: &str) -> Option<Self> {
        let normalized = normalize_label(label);
        let named = |setting: &Self| {
            std::iter::once(setting.label())
                .chain(setting.aliases().iter().copied())
                .map(normalize_label)
        };
        SETTINGS
            .iter()
            .copied()
            .find(|setting| named(setting).any(|name| name == normalized))
            // The binding looks labels up by **prefix**, not whole word, so a
            // first cell carrying anything after the label is a row the
            // transport acts on and this compared whole words.
            //
            // After the exact match, never before it: a prefix is the looser
            // reading and an exact spelling must never lose to it. No label
            // here is a prefix of another, so nothing else changes hands.
            .or_else(|| {
                SETTINGS.iter().copied().find(|setting| {
                    named(setting).any(|name| {
                        normalized
                            .strip_prefix(&name)
                            .is_some_and(|rest| !rest.is_empty())
                    })
                })
            })
    }

    /// What one of this setting's answers *means*, in one line. `None` for an
    /// answer that is a placeholder rather than a choice — the `unset` on a row
    /// whose real values are typed. Held by
    /// `every_answer_a_setting_offers_says_what_it_means`.
    pub fn means(self, value: &str) -> Option<&'static str> {
        Some(match (self, value) {
            (Self::Profile, "balanced") => {
                "delivery and transitions are `auto`: the run decides without asking"
            }
            (Self::Profile, "cautious") => {
                "delivery and transitions are `ask`: the run proposes and waits, and after the \
                 built-in timeout records the proposal on the issue instead of applying it"
            }
            (Self::Delivery | Self::Transitions, "auto") => "the run does it without asking",
            (Self::Delivery | Self::Transitions, "ask") => {
                "the run proposes and waits; after the built-in timeout it records the proposal \
                 on the issue instead of applying it"
            }
            (Self::Delivery | Self::Transitions, "ask 30m" | "ask 2h") => {
                "the same, with that long to answer before it is recorded rather than applied"
            }
            (Self::Tracker, "github") => "issues live in GitHub, reached through `gh`",
            (Self::Tracker, "linear ENG") => {
                "issues live in Linear, read and only moved through `State names`, under assisted \
                 work; the team key \
                 (`ENG`) names the project"
            }
            (Self::RepositoryHost, "github") => {
                "the code and publications live in GitHub, reached through `gh`"
            }
            (Self::Integration, "branch") => {
                "work integrates on a branch and merges when it is reviewed"
            }
            (Self::Integration, "trunk") => {
                "work integrates on the base branch, behind a flag while it is unfinished"
            }
            (Self::Window, "default") => {
                "the built-in window: a routine write may ride on a recent answer"
            }
            (Self::Window, _) => {
                "that long at most before the tracker is asked again, and never at a boundary"
            }
            (Self::GitOperationWindow, "disabled") => {
                "no schedule: commits, pushes, and branch creation are decided by the other checks"
            }
            (Self::GitOperationWindow, _) => {
                "that daily range, on those weekdays when named, in this machine's local time names \
                 when commits, pushes, and branch creation may run — `inverted` names when they \
                 may not instead"
            }
            (Self::Evidence, "reading") => {
                "a finding here is established by reading the change, so a reviewer needs no shell"
            }
            (Self::Evidence, "measuring") => {
                "a finding here is established by running something, so the unsealed reviewer \
                 gets a shell. The isolated directory that makes a shell safe is the launch's to \
                 provide; Estigia grants the tool and cannot see the directory"
            }
            (Self::PublicationLane, "on") => {
                "accepted verdicts are gated on this head's check runs"
            }
            (Self::PublicationLane, "off") => {
                "accepted verdicts do not read check runs; the answer says the lane was not consulted"
            }
            (Self::IssueRequirement, "ask") => {
                "a write by a run holding no claim is put to the person each time, where the host \
                 can ask, and refused where it cannot"
            }
            (Self::IssueRequirement, "off") => "a run holding no claim may write here freely",
            (Self::IssueRequirement, "required") => {
                "a write by a run holding no claim is refused until an issue is claimed"
            }
            (Self::Workflow, "autonomous") => {
                "Estigia holds the claim and writes the tracker, as it always has"
            }
            (Self::Workflow, "assisted") => {
                "a tool that writes the tracker is refused here, and the loops find no work; the \
                 team decides who takes an issue"
            }
            (Self::DeliveryPreset, "autonomous") => {
                "Estigia claims, reviews its own change, marks it ready and merges it"
            }
            (Self::DeliveryPreset, "assisted") => {
                "the team coordinates and the run stops at a draft pull request; people do the rest"
            }
            (Self::DeliveryPreset, "team") => {
                "the team coordinates; the run reviews its own draft, marks it ready, waits on \
                 the external reviewers and addresses their comments, and a person merges"
            }
            (Self::DeliveryPreset, _) => {
                "the rows below match no preset; each one says what it does"
            }
            (Self::StateNames, "none") => "Estigia moves no state on the tracker's side",
            (Self::StateNames, _) => {
                "the Linear issue is moved to the state named beside each of Estigia's"
            }
            (Self::SelfReview, "on") => {
                "the run reviews its change in a context that did not write it before it is ready"
            }
            (Self::SelfReview, "off") => {
                "no review by the run; the people reviewing are the review"
            }
            (Self::ReadyBy | Self::MergeBy, "agent") => "the run does it",
            (Self::ReadyBy | Self::MergeBy, "person") => {
                "a person does it, and the run waits for it rather than doing it"
            }
            (Self::ExternalReviewers, "none") => "no stage: nothing outside Estigia is waited on",
            (Self::ExternalReviewers, _) => {
                "each stage in turn: a check whose run must complete and whose author's comments \
                 are read, or a person whose review is requested only once the earlier stages are \
                 done"
            }
            (Self::CorrectionsAfterReady, "refuse") => {
                "nothing is published to a ready pull request; a person decides what happens to it"
            }
            (Self::CorrectionsAfterReady, "redraft") => {
                "the pull request goes back to draft before the correction is pushed"
            }
            (Self::CorrectionsAfterReady, "push-ready") => {
                "the correction is pushed to the ready pull request, so its CI and its reviewers \
                 see it at once"
            }
            (Self::ReviewBudget, "none") => "off: no per-context figure is declared",
            (Self::ReviewBudget, _) => {
                "that many tokens is what a single review context is reported against; \
                 nothing here refuses a panel that runs over it"
            }
            (Self::CorrectionsPerLineage, _) => {
                "the corrections a review lineage may publish after a rejection; the one past \
                 that is refused before anything is pushed"
            }
            (Self::ChangeSize, _) => {
                "the changed lines a pull request aims to stay under, and splits past unless the \
                 developer records why"
            }
            (Self::Boundaries, "none") => "no command beyond the built-in one-way doors",
            (Self::Boundaries, "git push --force") => {
                "a forced push rewrites history that others may have fetched, so it needs asking again"
            }
            (Self::Boundaries, "gh release create") => {
                "a published release is public the moment it exists, so it needs asking again"
            }
            (Self::Boundaries, "gh repo delete") => {
                "deleting the repository cannot be taken back from here, so it needs asking again"
            }
            (Self::Boundaries, "npm publish") => {
                "a published package version cannot be reused, so it needs asking again"
            }
            (Self::LoopPlugin, "on") => {
                "Claude Code gets `/flow-loop` and `/flow-status` and a `Stop` hook, OpenCode loads \
                 `flow-loop.js`, Pi loads `estigia-loop.ts` and Codex runs a `Stop` and a \
                 `SessionStart` hook; each continues a session while `estigia goal` says the run \
                 is live or the queue holds claimable work"
            }
            (Self::LoopPlugin, "off") => "no loop plugin or loop hook is written or read",
            (Self::GoalLoopPlugin, "on") => {
                "OpenCode loads `goal-loop.js` and Pi loads `estigia-goal-loop.ts`; each offers \
                 `/goal` and `/loop` and never runs an Estigia command"
            }
            (Self::GoalLoopPlugin, "off") => "no goal and loop plugin is written or read",
            (Self::Summary | Self::Body, _) => "the language that text is written in",
            // A placeholder rather than a choice: the real answers to these are
            // typed, and there is nothing to explain about the word itself.
            (Self::Worktree | Self::Models, _) => return None,
            _ => return None,
        })
    }

    /// The values a person may type, and the only place they learn them.
    ///
    /// Every rejection this module raises names this list, which is the ratchet
    /// applied to configuration: a value the operator cannot discover is a value
    /// they cannot supply. And a value named here that the parser refuses is a
    /// dead end in the one message whose whole job is not to be one — held by
    /// `every_value_a_setting_offers_is_a_value_it_accepts`.
    pub fn accepted(self) -> &'static str {
        match self {
            Self::Profile => "`balanced`, `cautious`, or `custom`",
            Self::Delivery | Self::Transitions => {
                "`auto`, `ask`, or `ask` with a duration such as `ask 30m`"
            }
            Self::Worktree => "`unset`, or an absolute directory",
            Self::Tracker => {
                "`github`, `github <owner>/<name>`, or `linear <team-key>` under assisted work"
            }
            Self::RepositoryHost => "`github` or `github <owner>/<name>`",
            // Every key, not examples: the sentence and the parser have to be
            // one thing. `every_key_the_routing_takes_is_a_key_it_names` reads
            // the lists rather than trusting this sentence, and
            // `a_workflow_state_is_no_longer_a_routing_key` checks the other
            // direction.
            Self::Models => {
                "`unset`, `by task`, or one cell holding two lists separated by `;`. The first is \
                 comma-separated key=model pairs, as in `orchestrate=fable, apply=sonnet`; the \
                 second, after the `;`, is comma-separated key=effort pairs, as in `apply=low`. `by \
                 task` says the shape of the work decides the model and the effort and the launcher \
                 records what it chose, instead of a role's name. A \
                 key is a role (developer, analyst, reviewer, refuter, validator, judge), orchestrate, or apply. A \
                 model ID may use any catalog spelling, or inherit, but no comma, semicolon, pipe, \
                 or line break. An effort is low, medium, high, xhigh or max. Naming a role installs \
                 its definition; reviewer, refuter, validator and judge install nothing"
            }
            Self::Integration => "`branch`, or `trunk`",
            // Only values at or below the built-in. A longer one is refused,
            // and the refusal says so rather than clamping quietly.
            Self::Window => "`default`, or a shorter duration such as `30s` or `1m`",
            Self::GitOperationWindow => {
                "`disabled`, `enabled 09:00-17:00`, `enabled 22:00-06:00 inverted`, or `enabled mon-fri 08:00-17:00 inverted`"
            }
            Self::Evidence => "`reading`, or `measuring`",
            Self::PublicationLane => "`on`, or `off`",
            Self::IssueRequirement => "`ask`, `off`, or `required`",
            Self::Workflow => "`autonomous`, or `assisted`",
            Self::DeliveryPreset => "`autonomous`, `assisted`, `team`, or `custom`",
            Self::StateNames => {
                "`none`, or comma-separated `<state>=<Name>` pairs, the state one of in-progress, \
                 review or done, under a Linear tracker"
            }
            Self::SelfReview => "`on`, or `off`",
            Self::ReadyBy | Self::MergeBy => "`agent`, or `person`",
            Self::ExternalReviewers => {
                "`none`, or comma-separated stages in the order they run: `check:<check name>` or \
                 `check:<check name>@<login>` for a CI check, and `review:<login>`, \
                 `review:<org>/<team>` or `review:any` for a person, any of them ending in \
                 :advisory when its comments need not be addressed, as in `review:any:advisory`"
            }
            Self::CorrectionsAfterReady => "`refuse`, `redraft`, or `push-ready`",
            Self::ReviewBudget => "`none`, or a token count such as `150000`",
            Self::ChangeSize => "a number of lines, such as `800`",
            Self::CorrectionsPerLineage => "a positive whole number, such as `3`",
            Self::Boundaries => "`none`, or commands separated by commas such as `npm publish`",
            Self::Summary | Self::Body => "a plain language name such as `English`",
            Self::LoopPlugin | Self::GoalLoopPlugin => "`on`, or `off`",
        }
    }

    /// Reads one written value into the configuration.
    fn parse_into(self, config: &mut Config, value: &str) -> Result<(), Refusal> {
        // A cell is delimited by `|` and lives on one line, and the block
        // markers delimit the table the row sits in; nothing escapes either,
        // so a value carrying one cannot be stored — written, it becomes extra
        // cells or a second table, and read back it is something else.
        // Refused here rather than after the write: what a table cannot carry
        // is knowable from the argument alone.
        let fence = crate::config::CONFIG_FENCE;
        if let Some(marker) = [fence.begin, fence.end]
            .into_iter()
            .chain(
                fence
                    .superseded
                    .iter()
                    .flat_map(|(open, close)| [*open, *close]),
            )
            .find(|marker| value.contains(marker))
        {
            return Err(Refusal::not_started(
                "config-value-untableable",
                format!(
                    "`{}` cannot hold `{marker}`: it is a marker delimiting the block this row \
                     sits in, and nothing escapes it either",
                    self.label()
                ),
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    format!(
                        "the same value without `{marker}` \u{2014} `{}` accepts {}",
                        self.label(),
                        self.accepted()
                    ),
                ),
            ));
        }
        if let Some(bad) = value
            .chars()
            .find(|c| *c == '|' || *c == '\n' || *c == '\r')
        {
            let what = if bad == '|' { "a `|`" } else { "a line break" };
            return Err(Refusal::not_started(
                "config-value-untableable",
                format!(
                    "`{}` cannot hold {what}: the value is stored as one cell of a \
                     one-line table row, and nothing escapes either",
                    self.label()
                ),
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    format!(
                        "the same value without {what} \u{2014} `{}` accepts {}",
                        self.label(),
                        self.accepted()
                    ),
                ),
            ));
        }
        match self {
            // A count of lines, and nothing else. `0` is refused with the rest:
            // a change size of nothing is a rule that can never be met, and a
            // row whose value forbids every pull request is a gate written by
            // accident in a field the contract calls guidance.
            Self::ChangeSize => {
                config.change_size = match value.trim().parse::<u32>() {
                    Ok(lines) if lines > 0 => lines,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::CorrectionsPerLineage => {
                config.corrections_per_lineage = match value.trim().parse::<u32>() {
                    Ok(corrections) if corrections > 0 => corrections,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::Profile => {
                // `custom` is what the row reads when the two authorisations
                // are a mix; writing it back is a no-op rather than a write of
                // a posture nobody named.
                if lower(value) == "custom" {
                    return Ok(());
                }
                let profile = Profile::parse(value).ok_or_else(|| self.reject(value))?;
                config.set_profile(profile);
            }
            Self::Delivery => config.delivery = parse_authority(self, value)?,
            Self::Transitions => config.transitions = parse_authority(self, value)?,
            Self::Worktree => {
                config.worktree = match lower(value).as_str() {
                    "unset" | "auto" | "" => WorktreeRoot::Auto,
                    _ => {
                        let path = PathBuf::from(value.trim());
                        if !path.is_absolute() {
                            return Err(Refusal::not_started(
                                "worktree-location-not-absolute",
                                format!(
                                    "`Worktree location` is {value:?}, which is not an absolute \
                                     directory"
                                ),
                                Resolution::no_command(
                                    NoCommandReason::OperatorKnowledge,
                                    "an absolute directory for isolated checkouts, or `unset`",
                                ),
                            ));
                        }
                        WorktreeRoot::Path(path)
                    }
                }
            }
            Self::Tracker => {
                let value = value.trim();
                let (head, rest) = match value.split_once(char::is_whitespace) {
                    Some((head, rest)) => (head, rest.trim()),
                    None => (value, ""),
                };
                config.tracker = match lower(head).as_str() {
                    "github" if rest.is_empty() => Tracker::Github { repo: None },
                    "github" => Tracker::Github {
                        repo: Some(RepoRef::parse(rest)?),
                    },
                    "linear" if rest.is_empty() => {
                        return Err(self.reject(value));
                    }
                    "linear" => Tracker::Linear {
                        team: rest.to_owned(),
                    },
                    _ => return Err(self.reject(value)),
                };
                // One place to configure when only Tracker is set.
                // Method call, not `config.repository_host =`, so the honesty
                // crossing that maps GateContext fields through parse_into does
                // not attribute a host write to the Tracker arm.
                config.inherit_host_from_tracker_if_unset();
            }
            Self::RepositoryHost => {
                let value = value.trim();
                let (head, rest) = match value.split_once(char::is_whitespace) {
                    Some((head, rest)) => (head, rest.trim()),
                    None => (value, ""),
                };
                config.repository_host = match lower(head).as_str() {
                    "github" if rest.is_empty() => RepositoryHost::Github { repo: None },
                    "github" => RepositoryHost::Github {
                        repo: Some(RepoRef::parse(rest)?),
                    },
                    _ => return Err(self.reject(value)),
                }
            }
            Self::Models => {
                let (routing, dropped) =
                    crate::config::ModelRouting::parse_keeping_what_parses(value);
                if !dropped.is_empty() {
                    return Err(crate::config::dropped_routes_report(&dropped));
                }
                config.models = routing;
            }
            Self::Integration => {
                config.integration = match lower(value).trim() {
                    "branch" | "branches" => crate::config::Integration::Branch,
                    "trunk" | "trunk-based" | "trunk based" => crate::config::Integration::Trunk,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::Window => {
                let text = lower(value);
                let text = text.trim();
                let asked = if text == "default" || text.is_empty() {
                    crate::harness::RENEWAL_WINDOW
                } else {
                    parse_duration(text).ok_or_else(|| self.reject(value))?
                };
                if asked > crate::harness::RENEWAL_WINDOW || asked.is_zero() {
                    return Err(self.reject(value));
                }
                config.window = asked;
            }
            Self::GitOperationWindow => {
                config.git_operation_window =
                    GitOperationWindow::parse(value).ok_or_else(|| self.reject(value))?;
            }
            Self::Boundaries => {
                config.boundaries = match lower(value).trim() {
                    "none" | "unset" | "" => Vec::new(),
                    listed => {
                        let declared: Vec<String> = listed
                            .split(',')
                            .map(|fragment| {
                                fragment.split_whitespace().collect::<Vec<_>>().join(" ")
                            })
                            .filter(|fragment| !fragment.is_empty())
                            .collect();
                        // An entry that survives trimming to nothing is a comma
                        // somebody left behind, and a silently dropped boundary
                        // is a boundary the operator believes they declared.
                        if declared.is_empty() {
                            return Err(self.reject(value));
                        }
                        declared
                    }
                }
            }
            // No `on`/`off` synonyms here on purpose, unlike the row above. Both
            // answers are a positive statement about this repository, and there
            // is no direction in which one of them is the feature being switched
            // off — `reading` is the narrower grant, not the absent one.
            Self::Evidence => {
                // The two words the row takes, and no synonyms: an undocumented
                // spelling is one an operator cannot discover.
                config.evidence = match lower(value).trim() {
                    "reading" => Evidence::Reading,
                    "measuring" => Evidence::Measuring,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::PublicationLane => {
                config.publication_lane = match lower(value).as_str() {
                    "on" => PublicationLane::On,
                    "off" => PublicationLane::Off,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::IssueRequirement => {
                config.issue_requirement = match lower(value).trim() {
                    "ask" => IssueRequirement::Ask,
                    "off" => IssueRequirement::Off,
                    "required" => IssueRequirement::Required,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::Workflow => {
                config.workflow = match lower(value).trim() {
                    "autonomous" => Workflow::Autonomous,
                    "assisted" => Workflow::Assisted,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::DeliveryPreset => {
                if lower(value) == "custom" {
                    return Ok(());
                }
                let preset =
                    super::DeliveryPreset::parse(value).ok_or_else(|| self.reject(value))?;
                config.set_delivery_preset(preset);
            }
            Self::StateNames => {
                config.state_names =
                    delivery::parse_state_names(value).ok_or_else(|| self.reject(value))?;
            }
            Self::SelfReview => {
                config.self_review =
                    Some(delivery::parse_switch(value).ok_or_else(|| self.reject(value))?);
            }
            Self::ReadyBy => {
                config.ready_by =
                    Some(delivery::parse_party(value).ok_or_else(|| self.reject(value))?);
            }
            Self::ExternalReviewers => {
                config.external_reviewers =
                    delivery::parse_stages(value).ok_or_else(|| self.reject(value))?;
            }
            Self::CorrectionsAfterReady => {
                config.corrections_after_ready =
                    Some(delivery::parse_corrections(value).ok_or_else(|| self.reject(value))?);
            }
            Self::MergeBy => {
                config.merge_by =
                    Some(delivery::parse_party(value).ok_or_else(|| self.reject(value))?);
            }
            Self::GoalLoopPlugin => {
                config.goal_loop_plugin = match lower(value).as_str() {
                    "on" => LoopPlugin::On,
                    "off" => LoopPlugin::Off,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::LoopPlugin => {
                config.loop_plugin = match lower(value).as_str() {
                    "on" => LoopPlugin::On,
                    "off" => LoopPlugin::Off,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::ReviewBudget => {
                config.review_budget = review_budget_of(value).ok_or_else(|| self.reject(value))?;
            }
            Self::Summary => config.summary_language = Language::parse(value)?,
            Self::Body => config.body_language = Language::parse(value)?,
        }
        Ok(())
    }

    /// Reads one written value, or refuses.
    pub fn apply(self, config: &mut Config, value: &str) -> Result<(), Refusal> {
        self.parse_into(config, value)
    }

    /// The value column for this setting, given a configuration.
    pub fn value_of(self, config: &Config) -> String {
        match self {
            Self::Profile => config.profile().map_or_else(
                || "custom".to_owned(),
                |profile| profile.as_value().to_owned(),
            ),
            Self::ChangeSize => config.change_size.to_string(),
            Self::CorrectionsPerLineage => config.corrections_per_lineage.to_string(),
            Self::Delivery => render_authority(config.delivery),
            Self::Transitions => render_authority(config.transitions),
            Self::Worktree => match &config.worktree {
                WorktreeRoot::Auto => "unset".to_owned(),
                WorktreeRoot::Path(path) => path.display().to_string(),
            },
            Self::Tracker => config.tracker.as_value(),
            Self::RepositoryHost => config.repository_host.as_value(),
            Self::Models => config.models.as_value(),
            Self::Integration => config.integration.as_value(),
            Self::Window => {
                if config.window == crate::harness::RENEWAL_WINDOW {
                    "default".to_owned()
                } else {
                    render_duration(config.window)
                }
            }
            Self::GitOperationWindow => config.git_operation_window.as_value(),
            Self::Evidence => config.evidence.as_value().to_owned(),
            Self::PublicationLane => config.publication_lane.as_value().to_owned(),
            Self::IssueRequirement => config.issue_requirement.as_value().to_owned(),
            Self::Workflow => config.workflow.as_value().to_owned(),
            Self::DeliveryPreset => config.delivery_preset().map_or_else(
                || "custom".to_owned(),
                |preset| preset.as_value().to_owned(),
            ),
            Self::StateNames => delivery::render_state_names(&config.state_names),
            Self::SelfReview => if config.self_review() { "on" } else { "off" }.to_owned(),
            Self::ReadyBy => config.ready_by().as_value().to_owned(),
            Self::ExternalReviewers => delivery::render_stages(&config.external_reviewers),
            Self::CorrectionsAfterReady => config.corrections_after_ready().as_value().to_owned(),
            Self::MergeBy => config.merge_by().as_value().to_owned(),
            Self::ReviewBudget => config.review_budget.as_value(),
            Self::Boundaries => {
                if config.boundaries.is_empty() {
                    "none".to_owned()
                } else {
                    config.boundaries.join(", ")
                }
            }
            Self::Summary => config.summary_language.as_str().to_owned(),
            Self::Body => config.body_language.as_str().to_owned(),
            Self::LoopPlugin => config.loop_plugin.as_value().to_owned(),
            Self::GoalLoopPlugin => config.goal_loop_plugin.as_value().to_owned(),
        }
    }

    /// Why this row cannot be chosen under `config`, or `None` when it can.
    pub fn fixed_by(self, config: &Config) -> Option<&'static str> {
        let fixed_under_autonomous = matches!(
            self,
            Self::StateNames
                | Self::SelfReview
                | Self::ReadyBy
                | Self::ExternalReviewers
                | Self::CorrectionsAfterReady
                | Self::MergeBy
        );
        (fixed_under_autonomous && config.workflow == Workflow::Autonomous)
            .then_some(FIXED_UNDER_AUTONOMOUS)
    }

    /// The third column: what the skill does when nothing is configured.
    pub fn default_value(self) -> String {
        self.value_of(&Config::default())
    }

    /// A rejection that names what may be written instead.
    fn reject(self, value: &str) -> Refusal {
        Refusal::not_started(
            "config-value-unrecognised",
            format!(
                "`{}` is {value:?}, which is not one of its values",
                self.label()
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                format!("`{}`: {}", self.label(), self.accepted()),
            ),
        )
    }
}

/// What [`Setting::fixed_by`] answers for a delivery row under `autonomous`.
pub const FIXED_UNDER_AUTONOMOUS: &str = "fixed while `Claim workflow` is `autonomous`: choose \
     `team` or `assisted` in `Delivery preset` to change it";

fn lower(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// The label the retired `Planning` row was written under.
///
/// The setting is gone, but a table written by a build that shipped it still
/// carries the row. Reading it as an unknown row would drop the one report that
/// says the protocol it named is retired, so the read paths recognise the label
/// before [`Setting::from_label`] has a chance to miss it.
pub(crate) const RETIRED_PLANNING_LABEL: &str = "Planning";

/// Whether a written row is the retired `Planning` row.
pub(crate) fn is_retired_planning_label(label: &str) -> bool {
    normalize_label(label) == normalize_label(RETIRED_PLANNING_LABEL)
}

/// Whether a written `Planning` value names the retired SDD protocol.
///
/// `auto` and `per-issue` are included, because they were spellings of `sdd`,
/// not protocols of their own; the tail is read as a set of words rather than a
/// fixed phrase, so `sdd lite openspec` and `sdd openspec lite` both count. A
/// value that names no protocol at all — `direct`, or a word the old setting
/// never took — is not a retirement.
pub(crate) fn planning_is_retired(value: &str) -> bool {
    let value = value.trim();
    let (head, rest) = match value.split_once(char::is_whitespace) {
        Some((head, rest)) => (head, rest.trim()),
        None => (value, ""),
    };
    let understood = lower(rest)
        .split_whitespace()
        .all(|word| matches!(word, "lite" | "short" | "openspec"));
    matches!(
        lower(head).as_str(),
        "sdd" | "spec-driven" | "auto" | "per-issue"
    ) && understood
}

/// What a read did with a `Planning` value naming the retired SDD protocol.
///
/// The row reads as `direct` — the only protocol that remains — rather than
/// being refused: an installed contract carrying it must keep loading, and
/// `setup` rewrites the shared table on its next run. The report says what
/// happened so the rewrite is not silent.
pub(crate) fn retired_planning_report(value: &str) -> Refusal {
    Refusal::not_started(
        "planning-protocol-retired",
        format!(
            "`Planning` is {value:?}; the SDD planning protocol is retired and this reads as \
             `direct`"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "remove the `Planning` row; the setting is retired and reads as `direct`",
        ),
    )
}

/// What a row this build retired is reported as, or nothing when the label is
/// not one of them.
///
/// The setting is gone, and an installed row is never dropped without saying
/// so: the report is what `config set` refuses a new write with, and what the
/// read paths surface by name rather than as an unknown row.
pub(crate) fn retired_setting_report(label: &str, value: &str) -> Option<Refusal> {
    match normalize_label(label).as_str() {
        "repair ceiling" => Some(Refusal::not_started(
            "repair-ceiling-retired",
            format!(
                "`Repair ceiling` is {value:?}; the setting is retired and nothing reads it: a \
                 lineage's corrections are counted by `Corrections per lineage`"
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "remove the `Repair ceiling` row; the row governs nothing",
            ),
        )),
        "delivery route" => Some(Refusal::not_started(
            "delivery-route-retired",
            format!(
                "`Delivery route` is {value:?}; the setting is retired, and a reviewed change \
                 reaches the base branch as a direct branch"
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "remove the `Delivery route` row; delivery is direct and the row configures nothing",
            ),
        )),
        "review delegation" => Some(Refusal::not_started(
            "review-delegation-retired",
            format!(
                "`Review delegation` is {value:?}; the setting is retired, and a run acquires its \
                 reviewers unconditionally — permission, not capability"
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "remove the `Review delegation` row; a run acquires its reviewers without asking",
            ),
        )),
        "merge strategy" => Some(Refusal::not_started(
            "merge-strategy-retired",
            format!(
                "`Merge strategy` is {value:?}; the setting is retired, and delivery merges with a \
                 merge commit"
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "remove the `Merge strategy` row; delivery merges with a merge commit",
            ),
        )),
        "review protocol" => Some(Refusal::not_started(
            "review-protocol-retired",
            format!(
                "`Review protocol` is {value:?}; the setting is retired, and every review is \
                 receipt-driven, following `protocols/rdd.md`"
            ),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "remove the `Review protocol` row; every review is receipt-driven",
            ),
        )),
        _ => None,
    }
}

/// The settings the transport itself reads out of the table.
///
/// Five: the transport calls `context.get` for `worktree location`,
/// `Review budget`, `Git operation window`, `Tracker`
/// (`assist` compares a Linear key's team with the one the row names) and
/// `Model routing` (`launch_review_input` reads the `reviewer` target's model),
/// and for nothing else. `Publication lane` is read via `config_exact` (prefix-safe
/// against `Publication lanes`), not `get`, so it is not in this list. `Review
/// budget` is `review_tier`'s read of the per-context figure its forecast line
/// is stated against; `Review delegation` left this list when its `ask` was
/// retired, `Merge strategy` when the row itself was retired and delivery's
/// merge commit was hard-coded, and `Repair ceiling` when `repair_rounds`
/// stopped reading a setting for the one-correction count. `Git operation
/// window` is the fallback `start_branch` and `publish_review` layer the
/// repository's own schedule over. `review_tier`
/// (`src/transport/tier.rs`) used to read another, `blind judges`, to reduce
/// its panel-size answer — a porting error now reversed: how many judges a change gets is a contract question now,
/// answered in `skill/SKILL.md` and `skill/policies/blind-judges.md`, never
/// derived from a diff. Every other row is read by the agent out of the prose,
/// or by the gate. Held here because the check below is only worth making for
/// rows a second reader actually consults, and crossed by
/// `the_transport_reads_the_settings_this_crate_says_it_does` so the list
/// cannot drift away from the code it describes.
pub const READ_BY_THE_TRANSPORT: &[Setting] = &[
    Setting::Worktree,
    Setting::ReviewBudget,
    Setting::CorrectionsPerLineage,
    Setting::GitOperationWindow,
    Setting::Tracker,
    Setting::Models,
];

/// Whether a row a person typed is one the transport will match.
///
/// The two readers differ, and this crate is the more forgiving: it collapses
/// runs of whitespace and strips the backticks a person writing markdown puts
/// round a name. The answer here is the **stricter** side's, so a crossing can
/// ask what the transport would have seen. Estigia is **not** made stricter to
/// close it: refusing backticks and stray spaces would drop the operator's
/// declared boundaries, trading a report nobody makes for a gate that decides
/// less.
pub fn reaches_the_transport(label: &str, setting: Setting) -> bool {
    let key = label.trim().to_lowercase();
    std::iter::once(setting.label())
        .chain(setting.aliases().iter().copied())
        .any(|name| key.starts_with(&name.to_lowercase()))
}

pub(crate) fn normalize_label(label: &str) -> String {
    label
        .trim()
        .trim_matches('`')
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The one grammar for an authority row: `auto`, `ask`, or `ask <duration>`.
///
/// Public because the handoff marker's own validator reads an `authority` field
/// with it, and this rule had been written three times and the three
/// disagreed. Old markers carry `ask`, so the grammar keeps reading it. Here the split is taken at the first non-digit
/// **character**, which is a boundary by construction.
pub fn authority_of(value: &str) -> Option<Authority> {
    let value = lower(value);
    if value == "auto" {
        return Some(Authority::Auto);
    }
    let rest = value.strip_prefix("ask")?.trim().to_owned();
    if rest.is_empty() {
        return Some(Authority::Ask {
            timeout: DEFAULT_ASK_TIMEOUT,
        });
    }
    parse_duration(&rest).map(|timeout| Authority::Ask { timeout })
}

/// One authority value as this crate spells it, for writing back down.
///
/// A marker records this rather than the operator's own spelling, so that what
/// is written is what [`authority_of`] will read.
pub fn rendered_authority(authority: Authority) -> String {
    render_authority(authority)
}

fn parse_authority(setting: Setting, value: &str) -> Result<Authority, Refusal> {
    authority_of(value).ok_or_else(|| setting.reject(&lower(value)))
}

/// The one grammar for a review-budget row: `none`/`off`/`unset`/empty
/// switches it off, and a positive integer is that many tokens. Private: the
/// transport does not read this row, so `parse_into` is the only caller — see `docs/honesty.md` for
/// what an advisory figure like this one does and does not cover.
fn review_budget_of(value: &str) -> Option<ReviewBudget> {
    match lower(value).trim() {
        "none" | "off" | "unset" | "" => Some(ReviewBudget::Off),
        digits => digits
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
            .map(ReviewBudget::Tokens),
    }
}

/// `30s`, `15m`, `2h`. Small on purpose: a timeout grammar wide enough to be
/// interesting is a timeout grammar people get wrong.
fn parse_duration(value: &str) -> Option<Duration> {
    let (digits, unit) = value.split_at(value.find(|c: char| !c.is_ascii_digit())?);
    let amount: u64 = digits.parse().ok()?;
    let seconds = match unit.trim() {
        "s" => amount,
        "m" => amount.checked_mul(60)?,
        "h" => amount.checked_mul(3600)?,
        _ => return None,
    };
    (seconds > 0).then(|| Duration::from_secs(seconds))
}

fn render_authority(authority: Authority) -> String {
    match authority {
        Authority::Auto => "auto".to_owned(),
        Authority::Ask { timeout } if timeout == DEFAULT_ASK_TIMEOUT => "ask".to_owned(),
        Authority::Ask { timeout } => format!("ask {}", render_duration(timeout)),
    }
}

fn render_duration(timeout: Duration) -> String {
    let seconds = timeout.as_secs();
    if seconds.is_multiple_of(3600) {
        format!("{}h", seconds / 3600)
    } else if seconds.is_multiple_of(60) {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

/// Where a line ends, for the transport that reads this same table.
///
/// One list, because two rules exist over it and they must not drift: which
/// region becomes which rows ([`lines_the_transport_sees`]), and which of those
/// characters can appear *inside* a row and cut it in half
/// ([`rows_split_by_a_separator`]).
const ENDS_A_LINE: [char; 10] = [
    '\n', '\r', '\u{0b}', '\u{0c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// `str::lines` ends a line at a newline and nothing else; the retired Python's
/// `splitlines`, which read the same document, ended one at ten characters, and
/// the difference split a document into different rows per reader. U+001F is
/// absent on purpose: a reader stricter than the transport is the same
/// disagreement facing the other way. Read only inside this module; the export
/// that once served `harness::classify_with`'s per-row branch is gone with that
/// branch — see `docs/honesty.md`.
fn lines_the_transport_sees(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut characters = text.char_indices().peekable();
    while let Some((at, character)) = characters.next() {
        if !ENDS_A_LINE.contains(&character) {
            continue;
        }
        lines.push(&text[start..at]);
        let mut after = at + character.len_utf8();
        // A carriage return followed by a newline is one ending, not two.
        // Splitting it in half would put an empty line between every pair on a
        // file written on Windows — this crate's own platform, and the one the
        // contract is usually edited on.
        if character == '\r' && characters.peek().is_some_and(|(_, next)| *next == '\n') {
            characters.next();
            after += 1;
        }
        start = after;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// The rows whose value carried a `|`, with the label each was written under.
///
/// A `|` separates cells, so a value holding one splits the row a cell early
/// and the value arrives **truncated**. `config set` refuses such a value by
/// name; the other door reads a file Estigia never writes, where nothing
/// refuses anything. A row is cut by a **cell** separator or by a **line**
/// separator, and the two are found differently: the bar makes the row wider
/// than the header it sits under, while the eight line endings in
/// `ENDS_A_LINE` that are not `\n` or `\r` make it *narrower*, so they are
/// found by looking at the line an editor draws. The direction matters:
/// *configuration may only tighten*, and a value read as less than it says
/// always loosens.
pub fn rows_split_by_a_separator(document: &str) -> Vec<CutShort> {
    let block = crate::config::block::CONFIG_FENCE.find(document);
    let region = block.as_ref().map_or(document, |found| found.body.as_str());
    let mut columns: Option<usize> = None;
    let mut split = Vec::new();
    // Split on `\n` and `\r` only: this walks the lines somebody *sees*, so a
    // row cut by one of the other eight arrives whole and can be recognised.
    // `lines_the_transport_sees` would already have cut it, and a rule cannot
    // report a boundary it is standing on the far side of.
    for line in region.split(['\n', '\r']) {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let label = line
            .trim_matches('|')
            .split('|')
            .next()
            .unwrap_or("")
            .trim();
        if let Some(unseen) = line
            .chars()
            .find(|character| ENDS_A_LINE.contains(character))
        {
            // Reported without consulting the width, and before it: this row is
            // several rows by the time anything counts cells, so whatever the
            // count says is about a fragment.
            if Setting::from_label(label).is_some() {
                split.push(CutShort {
                    label: label.to_owned(),
                    by: CutBy::Unseen(unseen),
                });
            }
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        // The header sets the width. Taken from the first row rather than
        // assumed, because a contract carries three columns and an operator's
        // own file usually two — and a rule that guessed would report every row
        // of one of them.
        let Some(columns) = columns else {
            columns = Some(cells.len());
            continue;
        };
        if cells.len() > columns && !label.is_empty() && Setting::from_label(label).is_some() {
            split.push(CutShort {
                label: label.to_owned(),
                by: CutBy::Bar,
            });
        }
    }
    split
}

/// A row read as less than the file says, and what cut it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutShort {
    /// The setting the row was written under.
    pub label: String,
    /// What ended the value early.
    pub by: CutBy,
}

/// What cut a row short.
///
/// The two carry different advice — one is a character an operator typed and
/// can see, the other one they cannot — so the distinction is kept as data and
/// the sentence is written where sentences are written, rather than the fact
/// arriving at `doctor` already phrased.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutBy {
    /// The cell separator itself.
    Bar,
    /// A line ending that shows as nothing, named so it can be searched for.
    Unseen(char),
}

/// The `| label | value | default |` rows of every markdown table in a
/// document, as `(label, value)`.
///
/// Deliberately forgiving about what surrounds it: the operator's local file is
/// theirs, and it may hold prose, other tables and their own notes. Only rows
/// whose label is one Estigia published are acted on.
pub fn rows(document: &str) -> Vec<(String, String)> {
    // Only the marked block, when the document carries one — the transport
    // reads exactly that region and nothing else; the other rows of the
    // shipped contract are prose tables.
    let block = crate::config::block::CONFIG_FENCE.find(document);
    let region = block.as_ref().map_or(document, |found| found.body.as_str());

    lines_the_transport_sees(region)
        .into_iter()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with('|') {
                return None;
            }
            let cells = line
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            if cells.len() < 2 {
                return None;
            }
            // The `|---|---|` separator.
            if cells
                .iter()
                .all(|cell| !cell.is_empty() && cell.chars().all(|c| c == '-' || c == ':'))
            {
                return None;
            }
            // The header, and an empty first cell. The transport drops both.
            let label = cells[0];
            if label.is_empty() || label.eq_ignore_ascii_case("setting") {
                return None;
            }
            Some((label.to_owned(), clean(cells[1])))
        })
        .collect()
}

/// Normalises one cell of the operator table.
///
/// The table is prose as much as it is data, so a cell carries decoration a
/// person needs and a program does not: a backticked token, bold, an
/// explanation after an em dash or in parentheses. This is the transport's
/// `_clean_value`, in Rust, because the two of them read the *same file* and
/// must not read it differently — the transport's normalisation is the older
/// published behaviour, so Estigia is the side that moves.
fn clean(raw: &str) -> String {
    let value = raw.trim();
    // A backticked token is where the machine-usable value always lives.
    if let Some(start) = value.find('`')
        && let Some(length) = value[start + 1..].find('`')
    {
        return value[start + 1..start + 1 + length].trim().to_owned();
    }
    let value = value.replace("**", "");
    let value = value.trim();
    // Drop a trailing explanation: `  — like this` or `  (like this)`.
    let mut cut = None;
    let mut whitespace: Option<usize> = None;
    for (at, character) in value.char_indices() {
        if character.is_whitespace() {
            whitespace = whitespace.or(Some(at));
            continue;
        }
        if let Some(from) = whitespace
            && (character == '(' || (character == '—' && follows_whitespace(value, at, character)))
        {
            cut = Some(from);
            break;
        }
        whitespace = None;
    }
    match cut {
        Some(at) => value[..at].trim().to_owned(),
        None => value.to_owned(),
    }
}

/// Whether whitespace follows the character at `at`.
///
/// An em dash only introduces an explanation when it is a word of its own.
/// `merge—commit` is one value; `squash — because` is a value and a sentence.
fn follows_whitespace(value: &str, at: usize, character: char) -> bool {
    value[at + character.len_utf8()..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
}

/// The table Estigia writes: header, separator, and one row per setting.
///
/// Not a row only a repository's own file answers: the contract has no value for it, and a row
/// nothing reads is a row that reads as a setting.
pub fn render_rows(config: &Config) -> String {
    let held: Vec<Setting> = SETTINGS
        .iter()
        .copied()
        .filter(|setting| setting.scope() != Scope::Repository)
        .collect();
    rows_of(config, &held)
}

/// The table an adapter's **own** file carries: the rows that differ by agent.
///
/// Not the whole table. An adapter's file is layered over the shared contract
/// by [`crate::skill::installed_config_for`], and a repository-wide row in it
/// is read by nobody — the gate resolves those from the contract.
pub fn render_agent_rows(config: &Config) -> String {
    rows_of(config, AGENT_SETTINGS)
}

/// The rows a repository keeps about itself, in table order.
///
/// The mirror of [`render_agent_rows`], through the same renderer: one table
/// shape, so a repository's file and an agent's file cannot drift into two.
pub fn render_repository_rows(config: &Config) -> String {
    rows_of(config, EVERYWHERE_SETTINGS)
}

/// The same, for the rows this file speaks for and no others.
///
/// A repository's file is an **override**: a row that is not in it means *this
/// checkout does not answer for that setting*, and the machine's answer stands.
/// Rendering the whole scope turns every unasked row into an answer — and the
/// values it writes are the **defaults**, because a file that does not exist
/// yet is read as `Config::default()`.
pub fn render_some_repository_rows(config: &Config, speaks_for: &[Setting]) -> String {
    let ordered: Vec<Setting> = SETTINGS
        .iter()
        .copied()
        .filter(|setting| {
            matches!(setting.scope(), Scope::Everywhere | Scope::Repository)
                || setting.repository_may_tighten()
        })
        .filter(|setting| speaks_for.contains(setting))
        .collect();
    rows_of(config, &ordered)
}

/// The same, for an adapter's own file inside a shared skill root.
///
/// Same rule and same reason as the repository's: that file is an override,
/// and a row it does not carry is one this adapter does not answer for.
pub fn render_some_agent_rows(config: &Config, speaks_for: &[Setting]) -> String {
    let ordered: Vec<Setting> = AGENT_SETTINGS
        .iter()
        .copied()
        .filter(|setting| speaks_for.contains(setting))
        .collect();
    rows_of(config, &ordered)
}

/// Header, separator, and one row per setting in the list.
fn rows_of(config: &Config, settings: &[Setting]) -> String {
    let mut out = String::from("| Setting | Value here | Skill default |\n|---|---|---|\n");
    for setting in settings {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            setting.label(),
            setting.value_of(config),
            setting.default_value(),
        ));
    }
    out
}
