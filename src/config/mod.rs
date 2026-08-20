//! The typed configuration model.
//!
//! This is where Rust pays. Every setting is an enum and every consumer is an
//! exhaustive `match`, so adding a `Tracker` does not compile until it has been
//! decided what it does everywhere it is asked.
//!
//! In issue-flow this is an unvalidated markdown table in `SKILL.md` with
//! overrides in `operator.local.md`. Nothing checks that `Worktree location` is
//! a usable path or that `Merge strategy` is one of the three values it names —
//! a typo is discovered by the operation that needed it, halfway through.
//!
//! The table stays: it is what the agent reads, and it has to stay legible.
//! What changes is that reading it produces a [`Config`] or a [`Refusal`], and
//! nothing in between.

use std::path::PathBuf;
use std::time::Duration;

use crate::outcome::{NoCommandReason, Refusal, Resolution};

mod block;
mod settings;

pub use block::{BLOCK_BEGIN, BLOCK_END, CONFIG_FENCE};
pub use settings::{
    AGENT_SETTINGS, Answers, CutBy, CutShort, EVERYWHERE_SETTINGS, MACHINE_SETTINGS,
    OPTIONS_SETTINGS, OPTIONS_SETTINGS_WITHOUT_BOARD, READ_BY_THE_TRANSPORT, SETTINGS, Scope,
    Setting, authority_of, reaches_the_transport, rendered_authority, rows as table_rows,
    rows_split_by_a_separator,
};

/// The name of the ignored local file that overrides the versioned table.
///
/// Renamed from issue-flow's `operator.local.md`: the product is not
/// issue-flow, and a file named after the old one is a file people will look
/// for in the old place. The old name is still *read* when present, so an
/// existing installation keeps working — see [`LEGACY_LOCAL_FILE`].
pub const LOCAL_FILE: &str = "estigia.local.md";

/// The file issue-flow wrote. Read for compatibility, never written.
pub const LEGACY_LOCAL_FILE: &str = "operator.local.md";

/// How long an `ask` authority waits before recording its proposal instead of
/// applying it.
pub const DEFAULT_ASK_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Which tracker holds the issues, and what identifies the project in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tracker {
    /// GitHub Issues, reached through `gh`.
    ///
    /// `repo` is `None` when the repository is to be discovered from the git
    /// remote, which is the normal case; naming it explicitly is for the
    /// checkouts where the remote is not the tracker.
    Github {
        /// The `owner/name` pair, or `None` to discover it from the git remote.
        repo: Option<RepoRef>,
    },
    /// Linear issues.
    Linear,
    /// Trello cards.
    Trello,
}

impl Tracker {
    /// The binding file under `bindings/` that owns this tracker's exact
    /// commands. Every variant must name one, and the seam tests check that the
    /// file it names exists.
    pub fn binding(&self) -> &'static str {
        match self {
            Self::Github { .. } => "bindings/github.md",
            Self::Linear => "bindings/linear.md",
            Self::Trello => "bindings/trello.md",
        }
    }

    /// Whether this tracker's reversible operations have an implementation.
    ///
    /// `None` when the binding declares none, and that is not a gap to be filled
    /// quietly with somebody else's: answering a Linear project with GitHub's
    /// operations would issue `gh` calls to a tracker that is not there, and the
    /// binding's own rule is that *"bindings MUST declare unsupported
    /// capabilities and fail closed."*
    ///
    /// It named a **file** — `scripts/github.py` — until that file stopped being
    /// shipped and the operations began answering in process. What it names now
    /// is where they are answered, because the question every caller asks of it
    /// is *can this tracker be operated at all*, and a path on disk was only ever
    /// this crate's way of saying yes.
    pub fn transport(&self) -> Option<&'static str> {
        match self {
            Self::Github { .. } => Some("estigia"),
            // Both ship a binding the agent reads and nothing that answers.
            // Estigia can install and configure them; it cannot hold their tools.
            Self::Linear | Self::Trello => None,
        }
    }

    /// The repository the operator named, when they named one.
    ///
    /// `owner/name`, for handing to `gh`. Nothing consulted this for a long
    /// time: [`Self::transport`] matches `Github { .. }` and answers with a
    /// script path, so `github acme/issues` and bare `github` drove every code
    /// path identically. The reference was parsed, validated with a refusal
    /// code of its own, written into eleven contracts, offered in the screen's
    /// accepted values — and no decision consulted it, which is the defect
    /// [`crate::config::Setting::Window`]'s note records, reached from a third
    /// direction.
    ///
    /// What it costs is not nothing: an operator whose issues live in a
    /// different repository from the code gets them filed in the checkout's
    /// own, because that is what `gh` infers when nobody tells it otherwise.
    pub fn named_repo(&self) -> Option<String> {
        match self {
            Self::Github { repo: Some(repo) } => Some(repo.to_string()),
            Self::Github { repo: None } | Self::Linear | Self::Trello => None,
        }
    }

    /// The value written back into the table.
    pub fn as_value(&self) -> String {
        match self {
            Self::Github { repo: None } => "github".to_owned(),
            Self::Github { repo: Some(repo) } => format!("github {repo}"),
            Self::Linear => "linear".to_owned(),
            Self::Trello => "trello".to_owned(),
        }
    }
}

/// How a change is planned before any of it is written.
///
/// # Why this is not "the methodology"
///
/// It was, briefly, and that was a modelling error worth recording. `issue-flow`
/// sat in the same enum as `sdd` and `rdd`, so choosing one excluded the others.
/// But the three answer different questions:
///
/// * **issue-flow** — how a *task* moves: roles, states, claims, renewal. It is
///   not a choice here at all; it is the substrate Estigia gates. Take it away
///   and there is no claim to measure a write against.
/// * **sdd** — how a *change is planned*: what exists before code does.
/// * **rdd** — how a change is *authorised*: what a verdict is bound to.
///
/// They compose, and the documents said so in prose while the type said pick
/// one. One end written by hand, the other in code, and nothing crossing them —
/// which is the defect this crate keeps finding, committed by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Planning {
    /// The contract's own path: an analyst writes acceptance criteria on the
    /// issue before implementation, and that is the specification.
    Direct,
    /// Spec-driven: explore, propose, spec, design, tasks — planning phases that
    /// produce artifacts, sitting inside `analysis` and `ready`.
    Sdd {
        /// Whether the artifacts are written under `openspec/` in the
        /// repository, or kept on the issue with the rest of the evidence.
        ///
        /// Off by default, and deliberately: the operator chose the tracker as
        /// the single source of truth, and a spec in a second place is a second
        /// answer to "what were we building". Turning it on is choosing to
        /// answer that question, not forgetting it.
        openspec: bool,
        /// Whether the short form is in force: **spec and tasks only**.
        ///
        /// Full SDD is five phases — explore, propose, spec, design, tasks — and
        /// that is the right shape when nobody yet agrees on what is being
        /// built. On a change where everybody already does, the three middle
        /// artifacts get written to satisfy the process and read by nobody,
        /// which teaches a team that the artifacts are ceremony. A methodology
        /// people route around is worse than a shorter one they follow.
        ///
        /// It is a **separate axis** from `openspec`, not a third value: where
        /// the artifacts live and how many there are are two questions, and
        /// collapsing them would make `sdd lite openspec` unsayable.
        lite: bool,
    },
}

impl Planning {
    /// The document describing this protocol, or `None` when there is nothing
    /// beyond what the contract already says.
    pub fn document(self) -> Option<&'static str> {
        match self {
            Self::Direct => None,
            Self::Sdd { .. } => Some("protocols/sdd.md"),
        }
    }

    /// The value as it is written in the table.
    pub fn as_value(self) -> String {
        match self {
            Self::Direct => "direct".to_owned(),
            Self::Sdd {
                openspec: false,
                lite: false,
            } => "sdd".to_owned(),
            Self::Sdd {
                openspec: true,
                lite: false,
            } => "sdd openspec".to_owned(),
            Self::Sdd {
                openspec: false,
                lite: true,
            } => "sdd lite".to_owned(),
            Self::Sdd {
                openspec: true,
                lite: true,
            } => "sdd lite openspec".to_owned(),
        }
    }

    /// The planning phases this protocol **can** run.
    ///
    /// Can, not will. `protocols/sdd.md` decides engagement per change —
    /// *"Ambiguity, and nothing else"* — so a phase named here may sit out a
    /// change everybody already understands. What the list rules out is a phase
    /// that can never run at all, which is the only thing a configuration can
    /// honestly say in advance.
    ///
    /// Empty under [`Planning::Direct`]: no phase runs, because the acceptance
    /// criteria on the issue *are* the specification and nothing produces an
    /// artifact before them.
    pub fn phases(self) -> &'static [&'static str] {
        match self {
            Self::Direct => &[],
            Self::Sdd { lite: true, .. } => SHORT_FORM_PHASES,
            Self::Sdd { lite: false, .. } => PLANNED_PHASES,
        }
    }

    /// Every planning protocol, checked by the compiler. See [`Judges::all`].
    pub fn all() -> Vec<Self> {
        let every = vec![
            Self::Direct,
            Self::Sdd {
                openspec: false,
                lite: false,
            },
            Self::Sdd {
                openspec: true,
                lite: false,
            },
            Self::Sdd {
                openspec: false,
                lite: true,
            },
            Self::Sdd {
                openspec: true,
                lite: true,
            },
        ];
        for planning in &every {
            match planning {
                Self::Direct | Self::Sdd { .. } => {}
            }
        }
        every
    }
}

/// What a review verdict is bound to.
///
/// Orthogonal to [`Planning`]: how a change was planned says nothing about what
/// authorises it, and the two are chosen separately because they are separately
/// useful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewProtocol {
    /// The contract's own: a verdict names an exact head and base, and every
    /// push invalidates it.
    Standard,
    /// Receipt-driven: the subject is frozen first and the verdict is bound to a
    /// digest over the complete intended target, re-derived at every gate.
    ReceiptDriven,
}

impl ReviewProtocol {
    /// The document describing this protocol, or `None` for the contract's own.
    pub fn document(self) -> Option<&'static str> {
        match self {
            Self::Standard => None,
            Self::ReceiptDriven => Some("protocols/rdd.md"),
        }
    }

    /// The value as it is written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::ReceiptDriven => "receipt-driven",
        }
    }

    /// Every review protocol, checked by the compiler. See [`Judges::all`].
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Standard, Self::ReceiptDriven];
        for protocol in &every {
            match protocol {
                Self::Standard | Self::ReceiptDriven => {}
            }
        }
        every
    }
}

/// How many independent contexts judge a change, and how their verdicts combine.
///
/// An axis of its own rather than part of a methodology: adversarial review is
/// required by the state contract under every one of them, and this only says
/// how many judges look and what their agreement buys.
///
/// [`Authority`] on `Review delegation` answers a different question — *who
/// obtains* the other contexts, this run or separately started ones. This
/// answers *how many* there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judges {
    /// One independent review, which is what the contract already requires.
    Single,
    /// Two judges in parallel, blind to each other, over one frozen target.
    ///
    /// Unanimity is what buys an automated fix. A finding one judge saw is a
    /// suspicion on the record; two judges disagreeing is a question for a
    /// person, with no tie-break — a majority of two is a coin toss wearing a
    /// process.
    TwoBlind,
    /// Five judges in parallel, blind to each other, over one frozen target.
    ///
    /// Three independent confirmations of the same severe finding form quorum.
    /// One or two remain suspicions; ambiguous finding identities never combine.
    FiveBlind,
}

/// What a verdict in this repository has to be backed by.
///
/// The one fact about a repository that only its operator knows, and the one
/// this crate cannot infer: whether a reviewer here establishes a finding by
/// **reading** the change or by **running** something against it.
///
/// It is a row rather than a constant because a constant was the defect. Every
/// role definition Estigia installs used to carry a fixed read-only grant, so a
/// judge could not build, could not run the suite, and could not turn a fix off
/// to see whether the suite went green. In a repository whose stated evidence
/// standard is mutation — this one's is — that judge cannot check the two
/// sentences its own `CLAUDE.md` makes load-bearing, so every panel that mattered
/// was launched under a generic type instead, outside the role and outside every
/// guarantee the contract attaches to it.
///
/// It is not derived from a neighbouring row, and that was considered: `Review
/// protocol` decides what identifies the target, `Blind judges` how many read it,
/// `Planning` what is written before it. None of them encodes what a verdict has
/// to be backed by, and loading a second meaning onto a row that already has one
/// is the shape this crate refuses everywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// A finding is established by reading the target.
    ///
    /// The default, and deliberately the narrower one. A grant that widened on
    /// upgrade would hand every delegated context a shell nobody asked for,
    /// which is the gate-widening this repository's own rules refuse.
    Reading,
    /// A finding is established by running something against the target.
    ///
    /// A reviewer here gets the capability to build, test and mutate — and with
    /// it the precondition that makes that safe, which is a directory nothing
    /// else writes for the duration. The capability without the isolation is
    /// concurrent writers with one tree between them, measured live: a five-judge
    /// panel shared one scratch directory, and one judge's script was overwritten
    /// by another's and then run inside a third judge's checkout, while a fourth
    /// read the implementing run's own notes.
    Measuring,
}

impl Evidence {
    /// The value as it is written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Measuring => "measuring",
        }
    }

    /// Every standard, under an exhaustive match, so a new one cannot be added
    /// without arriving here. Arriving is not being returned — widening the
    /// arm and leaving the list short still compiles. See [`Judges::all`].
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Reading, Self::Measuring];
        for evidence in &every {
            match evidence {
                Self::Reading | Self::Measuring => {}
            }
        }
        every
    }
}

impl Judges {
    /// The document describing this policy, or `None` when there is nothing
    /// beyond what the contract already says.
    pub fn document(self) -> Option<&'static str> {
        match self {
            Self::Single => None,
            Self::TwoBlind | Self::FiveBlind => Some("policies/blind-judges.md"),
        }
    }

    /// The value as it is written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::TwoBlind => "two blind",
            Self::FiveBlind => "five blind",
        }
    }

    /// Every policy, and the `match` below is what keeps it every: adding a
    /// variant stops this compiling until it is listed here. The two protocols
    /// above point at this one because they are built the same way.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Single, Self::TwoBlind, Self::FiveBlind];
        for judges in &every {
            match judges {
                Self::Single | Self::TwoBlind | Self::FiveBlind => {}
            }
        }
        every
    }
}

/// An `owner/name` pair, validated on the way in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    /// The account or organisation the repository belongs to.
    pub owner: String,
    /// The repository's own name.
    pub name: String,
}

impl RepoRef {
    /// Reads an `owner/name` pair, refusing anything else.
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        let mut parts = value.splitn(2, '/');
        let owner = parts.next().unwrap_or_default().trim();
        let name = parts.next().unwrap_or_default().trim();
        let usable = |segment: &str| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        };
        if !usable(owner) || !usable(name) {
            return Err(Refusal::not_started(
                "repo-ref-malformed",
                format!("{value:?} is not an owner/name repository reference"),
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    "the tracker repository, written as owner/name",
                ),
            ));
        }
        Ok(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }
}

impl std::fmt::Display for RepoRef {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}/{}", self.owner, self.name)
    }
}

/// Where isolated checkouts are made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeRoot {
    /// Beside the repository, chosen by Estigia. What `unset` meant.
    Auto,
    /// An absolute directory the operator owns.
    Path(PathBuf),
}

/// Who decides, for one class of decision.
///
/// `Ask` carries its timeout rather than leaving it somewhere else, because the
/// timeout is the part that had to be answered before `Ask` could exist at all:
/// asking collides with reclaiming a dead run, and the whole reclaim mechanism
/// assumes a run moves state on its own.
///
/// The resolution is that `Ask` **proposes and waits**, and on expiry **records
/// the proposed transition as a comment on the issue** instead of applying it.
/// A run that dies leaves a legible record rather than a state nobody wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    /// Propose and wait; on expiry, record the proposal instead of applying it.
    Ask {
        /// How long the proposal waits before it becomes a comment.
        timeout: Duration,
    },
    /// Decide without asking.
    Auto,
}

impl Authority {
    /// Whether this decision may be taken without a person present.
    pub fn is_autonomous(self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// How a reviewed change reaches the base branch.
///
/// One variant on purpose. `Direct` is the only value that exists anywhere in
/// the world today: `Delivery route` appears in issue-flow's configuration
/// table and in no binding, no reference and no execution step — nothing reads
/// it. Modelling a `Fork` that no code honours would be inventing a setting.
///
/// The row is kept because operators have it in their files, and the enum is
/// kept single-variant because the day a second route is real, adding it here
/// stops compiling everywhere it has to be handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryRoute {
    /// A branch in the tracker repository itself.
    Direct,
}

impl DeliveryRoute {
    /// Every route, checked by the compiler. See [`Judges::all`].
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Direct];
        for route in &every {
            match route {
                Self::Direct => {}
            }
        }
        every
    }
}

/// The merge topology delivery must produce, and which the delivered SHA is
/// checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// The delivered SHA's exact parents must be the reviewed base and head.
    MergeCommit,
    /// One commit on the base, whose tree is the reviewed head's.
    Squash,
    /// The reviewed commits replayed onto the base.
    Rebase,
}

impl MergeStrategy {
    /// Every strategy, checked by the compiler. See [`Judges::all`].
    pub fn all() -> Vec<Self> {
        let every = vec![Self::MergeCommit, Self::Squash, Self::Rebase];
        for strategy in &every {
            match strategy {
                Self::MergeCommit | Self::Squash | Self::Rebase => {}
            }
        }
        every
    }
}

/// A language an issue is written in.
///
/// Free text, because the set of languages is not ours to enumerate, but
/// validated: a value with a pipe in it would silently break the table it is
/// written back into.
///
/// Two rows carry one of these, and they are separate because they govern
/// separate prose: the plain-language sentence at the top of an issue exists to
/// be read by somebody who will never read past it, and the body under it is
/// for whoever implements the thing. A repository whose team reads Spanish and
/// whose code and commits are in English wants exactly that pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language(String);

impl Language {
    /// Reads a language name, refusing one that would break the table.
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        let value = value.trim();
        if value.is_empty() || value.contains('|') {
            return Err(Refusal::not_started(
                "language-malformed",
                format!("{value:?} is not usable as a language name"),
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    "the language this row governs, as a plain name such as English",
                ),
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// The name as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A board the workflow state is mirrored onto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRef(String);

impl BoardRef {
    /// Reads a board reference, refusing one that would break the table.
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        let value = value.trim();
        if value.is_empty() || value.contains('|') {
            return Err(Refusal::not_started(
                "board-ref-malformed",
                format!("{value:?} is not usable as a board reference"),
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    "the project board this repository projects onto, or `none`",
                ),
            ));
        }
        // And the shape the transport reads it with, asked of the transport.
        // This refused an empty value and one holding `|` — the character that
        // would break the markdown table — and nothing else, so `estigia config
        // set "Project board" acme/no-numero` answered *"Project board is now
        // acme/no-numero"*, wrote it, and left the mirror off: the reader calls
        // that spec unaddressable and disables itself.
        //
        // `config set`'s one promise is *validating it before anything is
        // written*, and a value that turns a feature off silently is the shape
        // this crate keeps finding — a fact known at the point of decision and
        // not acted on.
        if let Some(fault) = crate::transport::board::board_spec_fault(value) {
            return Err(Refusal::not_started(
                "board-ref-malformed",
                fault,
                Resolution::no_command(
                    NoCommandReason::OperatorKnowledge,
                    "the board as `<owner>/<number>`, taken from its URL, or `none`",
                ),
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// The reference as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Everything `setup` resolves and every operation reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Which tracker holds the issues.
    pub tracker: Tracker,
    /// How a change is planned before any of it is written.
    pub planning: Planning,
    /// What a review verdict is bound to.
    pub review_protocol: ReviewProtocol,
    /// How many independent contexts judge a change before it is delivered.
    pub judges: Judges,
    /// What a verdict here has to be backed by, and so what a reviewer may do.
    pub evidence: Evidence,
    /// The changed lines a pull request aims to stay under.
    ///
    /// Guidance the contract states and nothing enforces — Estigia gates writes,
    /// and a pull request is not one. It is a row rather than a sentence because
    /// as a sentence it lived in two shipped files, where an operator's answer
    /// survives exactly until the next `estigia sync`.
    pub change_size: u32,
    /// Where isolated checkouts are made.
    pub worktree: WorktreeRoot,
    /// The language the plain-language summary sentence is written in.
    pub summary_language: Language,
    /// The language the rest of an issue body is written in.
    pub body_language: Language,
    /// Who authorises delivery.
    pub delivery: Authority,
    /// How a reviewed change reaches the base branch.
    pub route: DeliveryRoute,
    /// Who authorises acquiring an independent review context.
    pub review: Authority,
    /// Who authorises a workflow-state transition.
    pub transitions: Authority,
    /// The merge topology delivery must produce.
    pub merge: MergeStrategy,
    /// The board workflow state is mirrored onto, when there is one.
    pub board: Option<BoardRef>,
    /// Where work integrates: a reviewed branch, or trunk behind a flag.
    pub integration: Integration,
    /// How long a routine write may ride on the previous verification.
    ///
    /// **Only downwards.** This is the first setting that can be changed in one
    /// direction and refused in the other, and the direction is the whole
    /// reason it can exist at all: a shorter window asks the tracker more
    /// often, and a longer one lets a write ride on an answer that is more
    /// stale — which is loosening a guard rail, and a guard rail that can be
    /// loosened is a preference.
    ///
    /// Somebody legitimately wants this shorter: a repository with many
    /// concurrent runs loses races inside two minutes, and paying an extra read
    /// is the cheap half of that trade. Nobody legitimately wants it longer;
    /// what they want is fewer reads, and the honest way to ask for that is not
    /// to widen the window in which the harness is guessing.
    pub window: std::time::Duration,
    /// Which model each delegated role runs on. Empty means "the agent decides".
    pub models: ModelRouting,
    /// Which delegated workers have a definition installed here.
    pub workers: Workers,
    /// Commands this repository treats as irreversible, beyond the built-ins.
    ///
    /// Estigia's own list knows git and GitHub. It cannot know that a repository
    /// ships with `npm publish` or `terraform apply`, and until this existed
    /// those ran as routine writes — able to ride the renewal window, which is
    /// the one thing a boundary must never do.
    ///
    /// Additive only. There is no way to remove a built-in boundary, because a
    /// setting that can make the gate looser is not a guard rail.
    pub boundaries: Vec<String>,
}

impl Default for Config {
    /// The portable defaults — the same ones issue-flow's versioned table ships,
    /// so an operator who installs and configures nothing gets the behaviour
    /// they already had.
    fn default() -> Self {
        Self {
            tracker: Tracker::Github { repo: None },
            // Empty: naming a model for a role nobody delegates to spends an
            // operator's attention on a sub-agent their configuration never
            // creates.
            models: ModelRouting::default(),
            // None. A definition is an instruction carrying a tool allowlist,
            // which the gate already treats as a control surface, so no
            // installation gains one by being upgraded.
            workers: Workers::default(),
            // A reviewed branch. Trunk-based trades the review for a flag, and
            // that trade has to be chosen rather than inherited.
            integration: Integration::Branch,
            // The number the contract has always carried in prose. Kept exactly,
            // so installing this release changes nobody's guidance — the row
            // exists to let an operator move it, not to move it for them.
            change_size: 800,
            window: crate::harness::RENEWAL_WINDOW,
            // The contract Estigia grew out of. An operator who never chooses
            // gets what they already had.
            planning: Planning::Direct,
            review_protocol: ReviewProtocol::Standard,
            // One independent review, which is what the contract has always
            // required. Two is a stronger claim and costs a second context.
            judges: Judges::Single,
            // The narrower answer, so no installation widens a delegated
            // context's capabilities by being upgraded.
            evidence: Evidence::Reading,
            worktree: WorktreeRoot::Auto,
            summary_language: Language("English".to_owned()),
            body_language: Language("English".to_owned()),
            delivery: Authority::Ask {
                timeout: DEFAULT_ASK_TIMEOUT,
            },
            route: DeliveryRoute::Direct,
            review: Authority::Ask {
                timeout: DEFAULT_ASK_TIMEOUT,
            },
            transitions: Authority::Ask {
                timeout: DEFAULT_ASK_TIMEOUT,
            },
            merge: MergeStrategy::MergeCommit,
            board: None,
            boundaries: Vec::new(),
        }
    }
}

impl Config {
    /// Reads a configuration out of a markdown document holding the managed
    /// block, applying any local override document on top.
    ///
    /// Rows absent from both fall back to [`Config::default`]. A row that is
    /// present and unreadable is a refusal — a setting the operator wrote and
    /// Estigia guessed at is worse than one they never wrote.
    pub fn read(versioned: &str, local: Option<&str>) -> Result<Self, Refusal> {
        match Self::read_reporting(versioned, local) {
            (config, None) => Ok(config),
            (_, Some(refusal)) => Err(refusal),
        }
    }

    /// The same read, keeping every row that parses.
    ///
    /// One bad row used to lose every row: [`read`](Self::read) applied each
    /// setting with `?`, so a value it did not recognise discarded the whole
    /// document — including rows beside it that read perfectly.
    ///
    /// That collateral is what made a declared gap look unavoidable. The honesty
    /// contract said an unreadable contract costs the operator's declared
    /// boundaries because "the list that made it a boundary is the thing that
    /// went missing", and that is true only when the boundary row is *itself*
    /// the bad one. Measured: a document holding `| Irreversible commands |
    /// terraform apply |` beside a mistyped `| Renewal window | 30 days |` threw
    /// the boundary away too, and the gate then classified `terraform apply` as
    /// a routine write. The row it needed had parsed.
    ///
    /// Only ever *narrower* than discarding everything: falling back to
    /// [`Config::default`] is the loosest configuration there is — no declared
    /// boundaries and the widest window — and every value a row can carry is a
    /// narrowing of it, because the setters refuse anything else.
    pub fn read_keeping_what_parses(versioned: &str, local: Option<&str>) -> Self {
        Self::read_reporting(versioned, local).0
    }

    /// Applies only rows belonging to `scope` over an existing configuration.
    ///
    /// Scope is checked before parsing the value. A hand-edited document may
    /// contain rows it cannot own; those rows are inert even when their values
    /// are invalid, while a bad value in an owned row still refuses.
    pub(crate) fn read_scope_over(
        base: &Self,
        document: &str,
        scope: Scope,
    ) -> Result<(Self, Vec<Setting>), Refusal> {
        let (config, settings, rejected) = Self::read_scope_reporting(base, document, scope);
        match rejected {
            Some(refusal) => Err(refusal),
            None => Ok((config, settings)),
        }
    }

    /// The scoped read while retaining every owned row that parses.
    pub(crate) fn read_scope_over_keeping_what_parses(
        base: &Self,
        document: &str,
        scope: Scope,
    ) -> (Self, Vec<Setting>) {
        let (config, settings, _) = Self::read_scope_reporting(base, document, scope);
        (config, settings)
    }

    fn read_scope_reporting(
        base: &Self,
        document: &str,
        scope: Scope,
    ) -> (Self, Vec<Setting>, Option<Refusal>) {
        let mut config = base.clone();
        let mut settings = Vec::new();
        let mut rejected = None;
        for (label, value) in settings::rows(document) {
            let Some(setting) = Setting::from_label(&label) else {
                continue;
            };
            if setting.scope() != scope {
                continue;
            }
            if !settings.contains(&setting) {
                settings.push(setting);
            }
            if let Err(refusal) = setting.apply(&mut config, &value) {
                rejected.get_or_insert(refusal);
            }
        }
        (config, settings, rejected)
    }

    /// Both answers at once, so neither is a second copy of the other.
    fn read_reporting(versioned: &str, local: Option<&str>) -> (Self, Option<Refusal>) {
        let mut config = Self::default();
        let mut rejected: Option<Refusal> = None;
        for document in [Some(versioned), local].into_iter().flatten() {
            for (label, value) in settings::rows(document) {
                let Some(setting) = Setting::from_label(&label) else {
                    // An unknown row is the operator's own note, not a defect.
                    // Their file is theirs; Estigia only claims the labels it
                    // published.
                    continue;
                };
                if let Err(refusal) = setting.apply(&mut config, &value) {
                    // The first, because that is the one `read` used to return
                    // and the one an operator is sent to fix.
                    rejected.get_or_insert(refusal);
                }
            }
        }
        (config, rejected)
    }

    /// Renders the table body — the rows between the markers, without them.
    pub fn render_rows(&self) -> String {
        settings::render_rows(self)
    }

    /// The same, for an adapter's own file: only the rows that differ by agent.
    ///
    /// See `settings::render_agent_rows`. The whole table went in here once,
    /// and `doctor` called the result broken — about a file Estigia wrote.
    pub fn render_agent_rows(&self) -> String {
        settings::render_agent_rows(self)
    }

    /// The rows this repository keeps about itself.
    pub fn render_repository_rows(&self) -> String {
        settings::render_repository_rows(self)
    }

    /// The repository rows this file speaks for, and no others.
    ///
    /// See `settings::render_some_repository_rows` for what writing the whole
    /// scope cost.
    pub fn render_some_repository_rows(&self, speaks_for: &[Setting]) -> String {
        settings::render_some_repository_rows(self, speaks_for)
    }

    /// The agent rows this file speaks for, and no others.
    ///
    /// See `settings::render_some_agent_rows` for what writing the whole scope
    /// cost.
    pub fn render_some_agent_rows(&self, speaks_for: &[Setting]) -> String {
        settings::render_some_agent_rows(self, speaks_for)
    }
}

#[cfg(test)]
mod tests;

/// The roles a run can hand work to.
///
/// Exactly the three the configuration already creates: the run itself, the
/// reviewer that `Review delegation` may delegate to, and the judges that
/// `Blind judges` may ask. A fourth name here would be a role nothing spawns —
/// a setting for a sub-agent that does not exist, which is worse than no
/// setting because it reads as configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// The run doing the work.
    Implementer,
    /// The reviewer, when review is delegated.
    Reviewer,
    /// A blind judge, when judges are asked.
    Judge,
}

impl Role {
    /// Its spelling in the table.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Implementer => "implementer",
            Self::Reviewer => "reviewer",
            Self::Judge => "judge",
        }
    }

    /// Every role, under an exhaustive match, so a new one cannot be
    /// added without arriving here. Arriving is not being returned — widening
    /// the arm and leaving the list short still compiles.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Implementer, Self::Reviewer, Self::Judge];
        for role in &every {
            match role {
                Self::Implementer | Self::Reviewer | Self::Judge => {}
            }
        }
        every
    }

    /// The role this word names.
    pub fn parse(word: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|role| role.as_str() == word.trim().to_lowercase())
    }
}

/// Which delegated workers this installation has definitions for.
///
/// # Why this is a row and not a `Model routing` key
///
/// It was a key, for one publication, and two blind judges refused it for the
/// same reason. `implementer` has been an accepted routing key since the row
/// existed, offered as its first example, and documented as inert — *"accepting
/// a name Estigia never spawns costs nothing, because an unread key routes
/// nobody"*. Three shipped Claude presets and three Codex ones set it. So the
/// population with that key stored is every installation that ever chose a
/// profile, and treating its presence as consent would install a definition
/// carrying `Write`, `Edit` and `Bash` into their home **because they
/// upgraded**.
///
/// A rule invented in one change cannot retroactively turn a value written under
/// the opposite rule into an answer. This row can: it did not exist before, so
/// nothing can hold it by accident, and `none` is what every existing contract
/// reads.
///
/// `Model routing` still says what each worker runs on. Two rows, two questions
/// — *does this worker exist here* and *what does it run on* — which is why
/// this is not the duplication this crate refuses elsewhere.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Workers {
    /// The worker that writes and runs the suite.
    pub implementer: bool,
    /// The worker that is repository-read-only.
    pub analyst: bool,
}

impl Workers {
    /// The names, in table order, of the workers this row turns on.
    pub fn names(self) -> Vec<&'static str> {
        let mut named = Vec::new();
        if self.implementer {
            named.push("implementer");
        }
        if self.analyst {
            named.push("analyst");
        }
        named
    }

    /// Whether this row names one particular worker.
    pub fn contains(self, name: &str) -> bool {
        self.names().contains(&name)
    }

    /// The value as it is written in the table.
    pub fn as_value(self) -> String {
        let named = self.names();
        if named.is_empty() {
            return "none".to_owned();
        }
        named.join(" ")
    }

    /// Every spelling the row takes, in the order it offers them.
    pub fn all() -> Vec<Self> {
        vec![
            Self::default(),
            Self {
                implementer: true,
                analyst: false,
            },
            Self {
                implementer: false,
                analyst: true,
            },
            Self {
                implementer: true,
                analyst: true,
            },
        ]
    }

    /// Reads `none`, one name, or both in either order.
    ///
    /// An unordered word set, the shape `Planning`'s tail already uses, so
    /// `analyst implementer` is the same answer as `implementer analyst` rather
    /// than a refusal an operator has to guess their way out of.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty()
            || value.eq_ignore_ascii_case("none")
            || value.eq_ignore_ascii_case("off")
            || value.eq_ignore_ascii_case("unset")
        {
            return Some(Self::default());
        }
        let mut workers = Self::default();
        for word in value.split_whitespace() {
            match word.to_lowercase().as_str() {
                "implementer" => workers.implementer = true,
                "analyst" => workers.analyst = true,
                _ => return None,
            }
        }
        Some(workers)
    }
}

/// How hard a delegated context is asked to think.
///
/// A closed set, and it is the host's closed set rather than one invented here:
/// these are the five words a Claude Code sub-agent definition's `effort:` field
/// takes. Inventing a sixth would render a definition the host reads as nothing,
/// which is the failure this row is being fixed for.
///
/// Effort is deliberately not a number. An operator choosing between `low` and
/// `high` is choosing what the work is worth; a budget would be choosing what it
/// costs, and Estigia cannot see either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Effort {
    /// Mechanical work: a rename, a moved line, a spelling.
    Low,
    /// The host's own default when a definition names no effort.
    Medium,
    /// Work whose shape is still open.
    High,
    /// Above `high`, where the host offers it.
    XHigh,
    /// As much as the host will give.
    Max,
}

impl Effort {
    /// Its spelling, in the row and in the rendered definition alike.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }

    /// Every effort, under an exhaustive match, so a sixth cannot be added
    /// without arriving here.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Low, Self::Medium, Self::High, Self::XHigh, Self::Max];
        for effort in &every {
            match effort {
                Self::Low | Self::Medium | Self::High | Self::XHigh | Self::Max => {}
            }
        }
        every
    }

    /// The effort this word names, or nothing.
    pub fn parse(word: &str) -> Option<Self> {
        let word = word.trim().to_lowercase();
        Self::all()
            .into_iter()
            .find(|effort| effort.as_str() == word)
    }
}

/// One target's model, and how hard it is asked to think.
///
/// The two travel together because they are one decision. An operator sizing a
/// context to a task picks both at once — a rename does not want the model *or*
/// the deliberation a design wants — and splitting them across two rows would be
/// two spellings of that one decision, which is the shape this crate has spent
/// whole rounds removing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// The opaque model ID, never checked against a catalog.
    pub model: String,
    /// The effort, when one was named. `None` renders nothing rather than a
    /// default: a definition that omits the field gets the host's own answer,
    /// and writing that answer down would freeze today's default into a file.
    pub effort: Option<Effort>,
}

impl Route {
    /// Reads `opus`, or `opus/high`, or nothing at all.
    ///
    /// The effort is read from the **right**, and only when the tail is one of
    /// [`Effort`]'s words. A provider-qualified ID carries the same separator —
    /// `anthropic/claude-opus-4` is one model, not `anthropic` at `claude-opus-4`
    /// effort — so reading from the left would eat the provider and leave a model
    /// nobody named.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        let (model, effort) = match value.rsplit_once('/') {
            // An effort with nothing in front of it is refused rather than read
            // as a model called `/high`. The operator named an effort and no
            // model; accepting it would route them to a model ID no catalog has
            // and report the row as set.
            Some((model, tail)) if Effort::parse(tail).is_some() => {
                (model.trim(), Effort::parse(tail))
            }
            _ => (value, None),
        };
        ModelRouting::accepts_model_id(model).then(|| Self {
            model: model.to_owned(),
            effort,
        })
    }

    /// The half of a `key=value` pair that follows the `=`.
    pub fn as_value(&self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for Route {
    /// One spelling, so what is written, what is read back and what is shown in
    /// a message cannot drift apart.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.effort {
            Some(effort) => write!(formatter, "{}/{}", self.model, effort.as_str()),
            None => formatter.write_str(&self.model),
        }
    }
}

/// The states a newly filed issue may be created in.
///
/// Narrower than [`STATES`], and the narrowing is the transport's: `create`
/// carries `choices=["ready", "blocked"]`, because an issue that has just been
/// written is either work somebody may pick up or work waiting on a named
/// condition. Nothing is created already in progress, in review or done.
///
/// Held here because the schema has to publish it and something has to cross it
/// — `every_value_the_transport_constrains_is_one_the_schema_publishes` reads
/// the parser and fails if this list stops being the parser's. It was published
/// as a free string with the two values in prose, which is the shape
/// `Argument::choices` exists to end.
pub const CREATED_STATES: &[&str] = &["ready", "blocked"];

/// The kinds of comment the transport will file.
///
/// `comment` carries `choices=["blocker", "diagnosis", "note"]`. Same story as
/// [`CREATED_STATES`]: named in one sentence of prose and in no machine-readable
/// place, so a client that completes or validates a call had nothing to go on
/// and a wrong guess is argparse refusing the whole thing.
pub const COMMENT_KINDS: &[&str] = &["blocker", "diagnosis", "note"];

/// The workflow states a run can be in.
///
/// Taken from the binding — *"the workflow's states are `analysis`, `ready`,
/// `in-progress`, `review`, `blocked` and `done`"* — and crossed against that
/// sentence by a test, for the same reason [`crate::harness`] crosses its
/// delivery states: a renamed state would make a transition match nothing, and
/// this list is what the transport labels an issue with.
///
/// These were [`ModelRouting`] keys too, until issue #110 measured that nothing
/// read them. Where the issue sits names no context to start, so there was
/// never anything for the family to route to.
pub const STATES: &[&str] = &[
    "analysis",
    "ready",
    "in-progress",
    "review",
    "blocked",
    "done",
];

const APPLY_TARGET: &str = "apply";
const ORCHESTRATE_TARGET: &str = "orchestrate";

/// The SDD planning phases, routable when `Planning` selects them.
///
/// They are not workflow states and not roles: a state says *where the issue
/// is*, a role says *what a sub-agent is*, and these say *what kind of thinking
/// is happening*. Designing and applying are the two that most obviously want
/// different models, which is the whole reason an operator asks for this.
///
/// `apply` is here beside the five because "write the code" is the phase people
/// actually name, even though SDD calls the artifact `tasks`. Both spellings
/// resolve, because refusing the word somebody reaches for teaches them the
/// setting does not work.
pub const SDD_PHASES: &[&str] = &[
    "explore",
    "propose",
    "spec",
    "design",
    "tasks",
    APPLY_TARGET,
    ORCHESTRATE_TARGET,
];

/// The phases whose existence [`Planning`] decides.
///
/// A subset of [`SDD_PHASES`], and the difference is the point. `apply` is left
/// out because writing the code happens under every protocol including
/// `direct` — naming a model for it is never inert. `orchestrate` is left out
/// because it is a property of how the agent delegates, not of how the change
/// was planned.
///
/// What this is *for*: a key named here that the protocol in force does not run
/// is a model an operator chose and nothing will ever consult. That was
/// unsayable while every phase looked alike, so `Model routing` accepted
/// `explore=opus` under `sdd lite` — which runs spec and tasks and nothing
/// else — and reported it set.
pub const PLANNED_PHASES: &[&str] = &["explore", "propose", "spec", "design", "tasks"];

/// The two the short form keeps. See [`Planning::Sdd::lite`].
const SHORT_FORM_PHASES: &[&str] = &["spec", "tasks"];

/// Sub-agent names an orchestration skill is likely to spawn.
///
/// Taken from a published orchestration lifecycle that runs seven of them with
/// isolated context windows. Estigia starts none of them: for five of these six
/// names [`ModelRouting`] is a declaration the agent reads, not a dispatch this
/// binary performs. They are
/// here because an operator running an orchestrator alongside Estigia thinks in
/// **its** vocabulary, and a setting that refuses the word they have in front
/// of them is one they conclude does not work.
///
/// The asymmetry is what makes this safe: accepting a name Estigia never spawns
/// costs nothing, because an unread key routes nobody. Refusing one that
/// somebody's orchestrator does spawn costs them the setting.
///
/// `analyst` is the exception and is deliberately not moved out of this list.
/// It is this contract's own read-only role as well as somebody else's
/// sub-agent name, and naming it here installs [`crate::skill::DELEGATED_AGENTS`]'s
/// read-only definition — which is exactly why that install is gated on the key
/// and tracked as created-outside. A file written unasked under a name another
/// harness answers to would shadow theirs.
pub const ORCHESTRATED_ROLES: &[&str] = &[
    "strategist",
    "analyst",
    "builder",
    "refactorer",
    "validator",
    "auditor",
];

/// Why one model target is visible in an interactive configuration surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTargetKind {
    /// The model reading the run and deciding what to delegate.
    Orchestration,
    /// An active planning phase selected by [`Planning`].
    PlanningPhase,
    /// The universal phase that writes the change.
    Application,
    /// A sub-agent role that an orchestration layer may delegate to.
    DelegatedAgent,
}

/// One model-routing target that is meaningful under the active [`Planning`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelTarget {
    /// The canonical key persisted in [`ModelRouting`].
    pub name: &'static str,
    /// The reason this target is shown.
    pub kind: ModelTargetKind,
}

/// Which model each delegated target should run on.
///
/// # What this does and does not do
///
/// Estigia **does not run the model** — it holds the tools. What it does with
/// this row is narrower and worth stating exactly, because the row's name
/// promises more than any harness could deliver: where a host reads sub-agent
/// definitions, `setup` writes the named model and effort into the ones it
/// installs, and for the two delegated workers the presence of the key is what
/// decides the file exists at all. Every other key is a declaration the agent
/// reads. Nothing here starts a process.
///
/// A route carries a model and, optionally, the effort it runs at — one
/// decision, one place. See [`Route`].
///
/// Empty by default. Naming a model for a role nobody delegates to would spend
/// an operator's attention on a sub-agent their configuration never creates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelRouting {
    /// Target to route; rendered through the canonical target order.
    pub by_role: std::collections::BTreeMap<String, Route>,
}

impl ModelRouting {
    /// Model targets an operator can act on under one planning protocol.
    ///
    /// Persistence accepts a wider vocabulary so changing `Planning` never
    /// makes an existing table unreadable. This narrower projection excludes
    /// inactive phases because presenting an inert route as an ordinary
    /// interactive choice would imply that the current run uses it.
    pub fn visible_targets(planning: Planning) -> Vec<ModelTarget> {
        std::iter::once(ModelTarget {
            name: ORCHESTRATE_TARGET,
            kind: ModelTargetKind::Orchestration,
        })
        .chain(planning.phases().iter().copied().map(|name| ModelTarget {
            name,
            kind: ModelTargetKind::PlanningPhase,
        }))
        .chain(std::iter::once(ModelTarget {
            name: APPLY_TARGET,
            kind: ModelTargetKind::Application,
        }))
        .chain(Role::all().into_iter().map(|role| ModelTarget {
            name: role.as_str(),
            kind: ModelTargetKind::DelegatedAgent,
        }))
        .chain(ORCHESTRATED_ROLES.iter().copied().map(|name| ModelTarget {
            name,
            kind: ModelTargetKind::DelegatedAgent,
        }))
        .collect()
    }

    /// Every target the routing parser accepts, in the order a person chooses it.
    ///
    /// Orchestration first because it is the route most often configured on its
    /// own. The rest comes from the same typed vocabularies the parser uses; a
    /// copied TUI list would eventually offer a target the table rejects or hide
    /// one it accepts.
    pub fn targets() -> Vec<&'static str> {
        std::iter::once(ORCHESTRATE_TARGET)
            .chain(Role::all().into_iter().map(Role::as_str))
            .chain(
                SDD_PHASES
                    .iter()
                    .copied()
                    .filter(|phase| *phase != ORCHESTRATE_TARGET),
            )
            .chain(ORCHESTRATED_ROLES.iter().copied())
            .collect()
    }

    /// The whole route assigned to any canonical target.
    ///
    /// One lookup, because there is one store. The model and the effort were
    /// two questions here and the second one had no answer at all.
    pub fn route(&self, target: &str) -> Option<&Route> {
        self.by_role.get(Self::canonical_target(target)?)
    }

    /// The model assigned to any canonical routing target.
    pub fn for_target(&self, target: &str) -> Option<&str> {
        self.route(target).map(|route| route.model.as_str())
    }

    /// The effort named for a target, when one was.
    pub fn effort_for(&self, target: &str) -> Option<Effort> {
        self.route(target).and_then(|route| route.effort)
    }

    /// Assigns one route -- `opus`, or `opus/high` -- to one canonical target.
    ///
    /// Model IDs are deliberately not validated against a catalog. Catalogs are
    /// advisory host snapshots; persisted custom and future IDs remain valid.
    /// The effort suffix is read by the same [`Route::parse`] the table uses, so
    /// an operator typing into a picker and an operator editing the row are
    /// spelling one thing one way.
    pub fn assign(&mut self, target: &str, model: &str) -> bool {
        let Some(target) = Self::canonical_target(target) else {
            return false;
        };
        let Some(route) = Route::parse(model) else {
            return false;
        };
        self.by_role.insert(target.to_owned(), route);
        true
    }

    /// Whether one opaque model ID fits inside one persisted `key=model` entry.
    ///
    /// Catalog membership is deliberately irrelevant. These four delimiters
    /// are structural: comma starts another assignment, pipe ends the markdown
    /// cell, and either line break ends the table row.
    pub fn accepts_model_id(model: &str) -> bool {
        let model = model.trim();
        !model.is_empty() && !model.contains([',', '|', '\r', '\n'])
    }

    /// Removes one assignment without manufacturing a `target=unset` pair.
    pub fn remove(&mut self, target: &str) -> bool {
        Self::canonical_target(target)
            .and_then(|target| self.by_role.remove(target))
            .is_some()
    }

    /// Removes every assignment.
    pub fn clear(&mut self) {
        self.by_role.clear();
    }

    fn canonical_target(target: &str) -> Option<&'static str> {
        let target = target.trim();
        Self::targets()
            .into_iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(target))
    }

    /// Keys named here for a phase the planning protocol never runs.
    ///
    /// # Why this reports rather than refuses
    ///
    /// The tempting shape is a parse error, and it is the wrong one. `Planning`
    /// and `Model routing` are two rows an operator sets in either order, so
    /// refusing here would mean a table that read yesterday stops reading today
    /// because a *different* row moved — and the way out would be editing a file
    /// the tool now refuses to load. This crate has already paid for that once:
    /// a `Project board` an older build accepted made every `config set` on that
    /// machine refuse, including the one command that could have cleared it.
    ///
    /// So it is answered, not enforced. The screen shows what is inert while the
    /// operator is looking at the row, which is where the information is worth
    /// something and where it costs nothing to be wrong about.
    ///
    /// Sorted by [`PLANNED_PHASES`] rather than by the map, so two calls over one
    /// configuration name the same phases in the same order.
    pub fn inert_phases(&self, planning: Planning) -> Vec<&'static str> {
        let live = planning.phases();
        PLANNED_PHASES
            .iter()
            .filter(|phase| !live.contains(*phase) && self.by_role.contains_key(**phase))
            .copied()
            .collect()
    }

    /// The cell this writes into the table.
    pub fn as_value(&self) -> String {
        if self.by_role.is_empty() {
            return "unset".to_owned();
        }
        // Role order, not insertion order: a table that reorders itself
        // between two writes shows a diff where nothing changed. Roles first,
        // then phases, then the names another orchestrator spawns — a fixed
        // order rather than the map's, so two writes of the same routing
        // produce the same cell and a diff means something changed.
        Role::all()
            .into_iter()
            .map(|role| role.as_str())
            .chain(SDD_PHASES.iter().copied())
            .chain(ORCHESTRATED_ROLES.iter().copied())
            .filter_map(|key| {
                self.by_role
                    .get(key)
                    .map(|route| format!("{key}={}", route.as_value()))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Reads `implementer=opus, design=sonnet/high` — or nothing.
    ///
    /// A word that is not `role=model` is refused rather than skipped. Silently
    /// dropping it would leave a role running on whatever the agent picks while
    /// the table says otherwise, which is the one thing a configuration must
    /// never do.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty()
            || value.eq_ignore_ascii_case("unset")
            || value.eq_ignore_ascii_case("none")
        {
            return Some(Self::default());
        }
        let mut by_role = std::collections::BTreeMap::new();
        for piece in value.split(',') {
            let (key, route) = piece.split_once('=')?;
            // A delegated role, a phase of thinking, or a name somebody else's
            // orchestrator spawns. Not a workflow state: where the issue sits
            // names no context to start, so that family was accepted here for
            // as long as it reached nothing.
            let key = Self::canonical_target(key)?;
            let route = Route::parse(route)?;
            // A role named twice is a contradiction, not a last-one-wins: the
            // operator meant two different things and only one can happen.
            if by_role.insert(key.to_owned(), route).is_some() {
                return None;
            }
        }
        Some(Self { by_role })
    }
}

/// Where work integrates, and what that costs at the gate.
///
/// # Why this changes a decision rather than describing a habit
///
/// Under [`Integration::Branch`] a delivery lands only from a state where a
/// verdict exists — `review` or `done` — because on a branch the review *is*
/// the protection: nothing reaches trunk until somebody answered.
///
/// Trunk-based removes that protection on purpose, and replaces it with a
/// different one: the change reaches trunk early, **switched off**. So the gate
/// asks for the replacement. A delivery from `in-progress` is allowed under
/// [`Integration::Trunk`] **only when a flag is named**, and refused otherwise —
/// which is the same shape as a forced reclaim: the loosening is declarable and
/// the declaration is answerable for.
///
/// Estigia cannot read the code and does not pretend to: naming a flag does not
/// prove the change is behind it. What it does is make the claim **explicit and
/// recorded**, so "we thought it was flagged" stops being something anybody can
/// say afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integration {
    /// A branch, reviewed, then merged. The default, and today's behaviour.
    Branch,
    /// Trunk-based: work lands early, behind a flag.
    Trunk,
}

impl Integration {
    /// Its spelling in the table.
    pub fn as_value(self) -> String {
        match self {
            Self::Branch => "branch".to_owned(),
            Self::Trunk => "trunk".to_owned(),
        }
    }

    /// Every mode, under an exhaustive match, so a new one cannot be
    /// added without arriving here. Arriving is not being returned — widening
    /// the arm and leaving the list short still compiles.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Branch, Self::Trunk];
        for mode in &every {
            match mode {
                Self::Branch | Self::Trunk => {}
            }
        }
        every
    }

    /// Whether a delivery from a state with no verdict may land, given the flag
    /// this run declared.
    ///
    /// Pure and fed the flag, because the interesting case is the one where a
    /// run *says* it is flagged — and a function that went looking for the flag
    /// itself could only find the name, never the switch.
    pub fn admits_unreviewed(self, flag: Option<&str>) -> bool {
        matches!(self, Self::Trunk) && flag.is_some_and(|name| !name.trim().is_empty())
    }
}
