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
    Authority, BoardRef, Config, DEFAULT_ASK_TIMEOUT, DeliveryRoute, Evidence, Judges, Language,
    MergeStrategy, Planning, RepoRef, ReviewProtocol, Tracker, Workers, WorktreeRoot,
};

/// One configurable setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// Who authorises delivery.
    Delivery,
    /// How a reviewed change reaches the base branch.
    Route,
    /// Who authorises acquiring an independent review context.
    Review,
    /// Who authorises a workflow-state transition.
    Transitions,
    /// The merge topology delivery must produce.
    Merge,
    /// Where isolated checkouts are made.
    Worktree,
    /// Which tracker holds the issues.
    Tracker,
    /// How a change is planned before any of it is written.
    Planning,
    /// Which model each delegated role runs on.
    Models,
    /// Which delegated workers have a definition installed here.
    ///
    /// Beside `Model routing` in every ordered list, because the two answer one
    /// question between them: whether a worker exists here, and what it runs on.
    /// They are two rows and not one because a routing key could already be in
    /// somebody's table, written when it meant nothing — see
    /// [`crate::config::Workers`].
    Workers,
    /// Where work integrates.
    Integration,
    /// How long a routine write may ride on the previous verification.
    Window,
    /// What a review verdict is bound to.
    ReviewProtocol,
    /// How many independent contexts judge a change.
    Judges,
    /// What a verdict here has to be backed by, and so what a reviewer may do.
    ///
    /// Beside `Blind judges` in every ordered list, because the two answer one
    /// question between them: how many contexts look, and what a look is worth.
    Evidence,
    /// How large a change may get before it is split.
    ///
    /// Guidance, and the contract says so — not a gate, because Estigia gates
    /// writes and a pull request is not one. It is here because the number was
    /// prose in two shipped files, which put it in the one place an operator
    /// cannot keep an answer: `estigia sync` rewrites both, so a team that
    /// lowered it to 300 got 800 back under a line reading `update`, the same
    /// line a version bump writes.
    ChangeSize,
    /// Commands this repository treats as irreversible.
    Boundaries,
    /// The board workflow state is mirrored onto.
    Board,
    /// The language the plain-language summary sentence is written in.
    ///
    /// issue-flow called this `"Description for dumb humans" sentence language`
    /// and it was exact: the one place any template names a language is that
    /// callout, and the sentence is what it governs. Estigia renamed it `Task
    /// body language`, which claimed the whole issue body and reached one
    /// sentence — a setting widened by renaming it rather than by anything
    /// reading it. Both spellings are [`Setting::aliases`] here, so the value
    /// on somebody's disk goes on meaning what it always meant.
    Summary,
    /// The language the rest of the issue body is written in.
    ///
    /// New, and it needed the template to name it before it could exist: a row
    /// nothing reads is a row `config set` writes, `config list` reads back,
    /// and no decision consults — which is the defect [`Setting::Window`]'s
    /// note records.
    Body,
}

/// Every setting, in the order they are written into the table.
///
/// Exhaustiveness is checked by a test rather than by the compiler, because a
/// slice cannot be matched — see `every_setting_is_in_the_table`.
pub const SETTINGS: &[Setting] = &[
    Setting::Delivery,
    Setting::Route,
    Setting::Review,
    Setting::Transitions,
    Setting::Merge,
    Setting::Worktree,
    Setting::Tracker,
    Setting::Planning,
    Setting::Models,
    Setting::Workers,
    Setting::Integration,
    Setting::Window,
    Setting::ReviewProtocol,
    Setting::Judges,
    Setting::Evidence,
    Setting::ChangeSize,
    Setting::Boundaries,
    Setting::Board,
    Setting::Summary,
    Setting::Body,
];

/// Who a setting's answer belongs to.
///
/// Every row lands in the same table, so this changes nothing about how a
/// setting is stored — [`crate::skill::installed_config_for`] already layers a
/// shared contract under a per-adapter file. What it changes is who is *asked*.
///
/// A table repeated once per agent is one chance per agent to set the tracker
/// again and get it wrong on one of them. This said *sixteen rows* until a
/// reviewer counted; the number has moved twice since and the argument never
/// depended on it. A repository has one tracker
/// whichever agent looks at it, and asking each agent separately implies
/// otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Differs by agent: the answer depends on which one is holding the tools.
    Agent,
    /// A fact about the repository, the same whichever agent asks.
    Everywhere,
    /// A fact about **this machine**, the same whichever repository is open.
    ///
    /// The third answer, and it took a while to need one. Two scopes made
    /// every row that was not about an agent a row about the repository, which
    /// put the language an issue is written in beside the tracker it is filed
    /// on — and those are different questions. A person writes in one language
    /// across every checkout they have; a tracker belongs to one of them.
    Machine,
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
    /// A value that is not in the list — somebody's own board name, a typed
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
/// These are the ones whose answer depends on *who is working*: what that agent
/// may do without asking, how it plans, and what it delegates to.
///
/// Every one of them is an instruction the **agent** reads and follows, which
/// is what makes a per-agent answer possible at all: the agent reads its own
/// file. A row the gate enforces cannot live here, because the gate does not
/// know which agent it is answering — see [`Setting::Window`] under
/// [`EVERYWHERE_SETTINGS`], which used to.
pub const AGENT_SETTINGS: &[Setting] = &[
    Setting::Delivery,
    Setting::Review,
    Setting::Transitions,
    Setting::Planning,
    Setting::Models,
    Setting::Workers,
    Setting::Judges,
];

/// The settings that are the same whichever agent asks, in table order.
pub const EVERYWHERE_SETTINGS: &[Setting] = &[
    Setting::Route,
    Setting::Merge,
    Setting::Worktree,
    Setting::Tracker,
    Setting::Integration,
    // The renewal window is here, and not beside the agent settings, because it
    // is enforced by the **gate** rather than obeyed by the agent — and what
    // reaches the gate is a dialect, a response shape, not an identity. Two
    // adapters register `exit-code` between them and three send no dialect at
    // all. Declared per agent it was a row `config set --agent` would write,
    // `config list --agent` would read back, and no decision would ever
    // consult.
    //
    // In this position because [`SETTINGS`] puts it here, and both lists say
    // they are in table order. Moved to the front, the setup screen offered the
    // repository's rows in one order and the contract wrote them in another.
    Setting::Window,
    Setting::ReviewProtocol,
    Setting::Evidence,
    Setting::ChangeSize,
    Setting::Boundaries,
    Setting::Board,
];

/// The rows that belong to this machine, in table order.
///
/// Two, and both are languages: what somebody writes in does not change
/// because they opened a different checkout. They sit under the repository's
/// rows on the options page, in a section of their own — a page that does not
/// say which of the two a row is has told the operator nothing about where
/// their answer goes.
pub const MACHINE_SETTINGS: &[Setting] = &[Setting::Summary, Setting::Body];

/// Everything the options page offers, in table order.
///
/// The repository's rows and then this machine's, which is the order they are
/// drawn in and the order the table declares them.
pub const OPTIONS_SETTINGS: &[Setting] = &[
    Setting::Route,
    Setting::Merge,
    Setting::Worktree,
    Setting::Tracker,
    Setting::Integration,
    Setting::Window,
    Setting::ReviewProtocol,
    Setting::Evidence,
    Setting::ChangeSize,
    Setting::Boundaries,
    Setting::Board,
    Setting::Summary,
    Setting::Body,
];

/// The same, without the rows the chosen tracker has nothing to do with.
///
/// A board is a GitHub Projects thing. `bindings/linear.md` and
/// `bindings/trello.md` declare no board mirror and the transport asks for one
/// only under GitHub, so on those trackers the row is a question with no
/// answer — and offering it is offering a setting that does nothing, which is
/// the defect this crate has already found three times in its own table.
///
/// A second slice rather than a filter, because `rows` hands back
/// `&'static [Setting]` and it is the **one** list both the cursor and the
/// drawing walk. Two lists that could disagree about how many rows there are is
/// how a highlight ends up two rows below the cursor.
pub const OPTIONS_SETTINGS_WITHOUT_BOARD: &[Setting] = &[
    Setting::Route,
    Setting::Merge,
    Setting::Worktree,
    Setting::Tracker,
    Setting::Integration,
    Setting::Window,
    Setting::ReviewProtocol,
    Setting::Evidence,
    Setting::ChangeSize,
    Setting::Boundaries,
    Setting::Summary,
    Setting::Body,
];

impl Setting {
    /// Whether the chosen tracker gives this row anything to mean.
    ///
    /// Only the board so far, and it is not a judgement about Linear or Trello:
    /// their bindings declare no board mirror and `cfg(config, "project board")`
    /// is read by the GitHub transport alone, so the row is a question those
    /// trackers have no answer to.
    pub fn applies_to(self, tracker: &Tracker) -> bool {
        match self {
            Self::Board => matches!(tracker, Tracker::Github { .. }),
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

    /// Whose answer this is.
    ///
    /// The split is by *what the answer is about*, not by what happens to be
    /// convenient: `Tracker` is where this repository's issues live and does not
    /// become a different place because Codex asked; `Model routing` is the
    /// operator's example of the opposite — Claude Code runs Opus where OpenCode
    /// runs Kimi, in the same checkout, at the same time.
    pub fn scope(self) -> Scope {
        match self {
            Self::Delivery
            | Self::Review
            | Self::Transitions
            | Self::Planning
            | Self::Models
            | Self::Judges
            | Self::Workers => Scope::Agent,
            Self::ChangeSize
            | Self::Window
            | Self::Route
            | Self::Merge
            | Self::Worktree
            | Self::Tracker
            | Self::Integration
            | Self::ReviewProtocol
            // Everywhere rather than per agent, and the suite is what said so:
            // the gate reads this row to render the reserved reviewer's grant,
            // and a row the gate reads cannot have a different answer depending
            // on which agent asked — there is one gate.
            | Self::Evidence
            | Self::Boundaries
            | Self::Board => Scope::Everywhere,
            // Written across every checkout somebody has, so they belong to
            // the person rather than to the repository.
            Self::Summary | Self::Body => Scope::Machine,
        }
    }

    /// One line saying what this setting decides, for somebody meeting it.
    ///
    /// The table already carries [`Self::accepted`], which says what may be
    /// *typed*. That is a different question from what the row is *for*, and an
    /// operator who does not know the second cannot use the first.
    pub fn about(self) -> &'static str {
        match self {
            Self::Delivery => "whether this agent may deliver a reviewed change, or has to ask",
            Self::Route => "how a reviewed change reaches the base branch",
            Self::Review => "whether this agent may fetch its own review, or has to ask",
            Self::Transitions => "whether this agent may move a task between states on its own",
            Self::Merge => "the history the base branch is required to end up with",
            Self::Worktree => "where isolated checkouts are made, when a run needs one",
            Self::Tracker => "where this repository's issues live — the claim is adjudicated there",
            Self::Planning => "how much is written down before any code is",
            Self::Models => {
                "which model, and at what effort, each delegated role, phase and sub-agent runs on, for this agent"
            }
            Self::Integration => "whether work integrates through branches or straight onto trunk",
            Self::Window => "how long a routine write may ride on the last verification",
            Self::ReviewProtocol => "what a review verdict is bound to (RDD lives here)",
            Self::Judges => "how many independent contexts look at a change before it lands",
            Self::Workers => "which delegated workers this agent has a definition for, if any",
            Self::Evidence => {
                "what a verdict here has to be backed by, and so what a reviewer may do"
            }
            Self::ChangeSize => "how many changed lines a pull request aims to stay under",
            Self::Boundaries => "the commands this repository treats as one-way doors",
            Self::Board => "the project board workflow state is mirrored onto",
            Self::Summary => "the language the summary sentence at the top of an issue is in",
            Self::Body => "the language the rest of an issue body is written in",
        }
    }

    /// The answers this setting offers, and whether they are all of them.
    ///
    /// [`Self::accepted`] is prose, written for a refusal message. It is the
    /// wrong shape for a screen: a person reading "`merge commit`, `squash`, or
    /// `rebase`" then has to **type one of them**, exactly, from memory, into a
    /// field that punishes a typo. The same three words as a list can be walked
    /// with an arrow key.
    ///
    /// `closed` says whether anything outside the list is refused. Where it is
    /// false the list is a set of useful answers rather than the whole
    /// vocabulary — an absolute path, a board name, a language — and the screen
    /// still has to offer somewhere to type.
    pub fn answers(self) -> Answers {
        // Every entry here must be a value `apply` accepts **and** one that
        // `value_of` writes back the same way, or the screen would show a list
        // whose selected entry is never the one that is set. Held by
        // `every_offered_answer_round_trips`.
        match self {
            // `ask` alone is the built-in timeout, so the two written out are
            // longer than it — offering `ask 15m` would render back as plain
            // `ask` and never look chosen.
            Self::Delivery | Self::Review | Self::Transitions => {
                Answers::some(&["auto", "ask", "ask 30m", "ask 2h"])
            }
            Self::Route => Answers::all(&["direct"]),
            Self::Merge => Answers::all(&["merge commit", "squash", "rebase"]),
            Self::Worktree => Answers::some(&["unset"]),
            Self::Tracker => Answers::some(&["github", "linear", "trello"]),
            Self::Planning => Answers::all(&[
                "direct",
                "sdd",
                "sdd lite",
                "sdd openspec",
                "sdd lite openspec",
            ]),
            Self::Models => Answers::some(&["unset"]),
            Self::Integration => Answers::all(&["branch", "trunk"]),
            Self::Window => Answers::some(&["default", "1m", "30s"]),
            Self::ReviewProtocol => Answers::all(&["standard", "receipt-driven"]),
            Self::Judges => Answers::all(&["single", "two blind", "five blind"]),
            Self::Workers => {
                Answers::all(&["none", "implementer", "analyst", "implementer analyst"])
            }
            Self::Evidence => Answers::all(&["reading", "measuring"]),
            Self::ChangeSize => Answers::some(&["800", "400"]),
            Self::Boundaries => Answers::some(&["none"]),
            Self::Board => Answers::some(&["none"]),
            Self::Summary | Self::Body => Answers::some(&["English", "Español"]),
        }
    }

    /// The label written into the first column.
    pub fn label(self) -> &'static str {
        match self {
            Self::Delivery => "Delivery authorisation",
            Self::Route => "Delivery route",
            Self::Review => "Review delegation",
            Self::Transitions => "Transition authorisation",
            Self::Merge => "Merge strategy",
            Self::Worktree => "Worktree location",
            Self::Tracker => "Tracker",
            Self::Planning => "Planning",
            Self::Models => "Model routing",
            Self::Integration => "Integration",
            Self::Window => "Renewal window",
            Self::ReviewProtocol => "Review protocol",
            Self::Judges => "Blind judges",
            Self::Workers => "Delegated workers",
            Self::Evidence => "Evidence standard",
            Self::ChangeSize => "Change size",
            Self::Boundaries => "Irreversible commands",
            Self::Board => "Project board",
            Self::Summary => "Summary language",
            Self::Body => "Issue body language",
        }
    }

    /// Labels this setting also answers to.
    ///
    /// issue-flow shipped `"Description for dumb humans" sentence language`,
    /// which is in operators' files today. Estigia writes the shorter label and
    /// keeps reading the old one, so upgrading does not quietly reset anyone's
    /// language to English.
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            // Both older spellings, because the value under either of them
            // governed the summary sentence and nothing else. Read as the new
            // `Issue body language` instead it would move an operator's answer
            // onto a row that decides something different, and leave the row it
            // actually decided sitting at the default.
            Self::Summary => &[
                "Task body language",
                "\"Description for dumb humans\" sentence language",
            ],
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
            // And the way the binding looks one up, which is by **prefix**:
            // `cfg` walks the parsed table and returns the first key that
            // `startswith` the label. So a first cell carrying anything after
            // the label is a row the transport acts on, and this compared whole
            // words and did not.
            //
            // Measured end to end on the installed pair, with
            // `| Project board (mine) | acme/7 |` in the contract: the
            // transport answered `board.enabled=True owner=acme number=7` and
            // `estigia config list` answered `none`. One file, one row, two
            // answers — the transport mirroring every issue to a board the gate
            // reports switched off.
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

    /// What one of this setting's answers *means*, in one line.
    ///
    /// [`Self::answers`] says which words are accepted and [`Self::about`] says
    /// what the row is for, and between them nothing said what choosing one of
    /// the words does. An operator on `Review protocol` was offered `standard`
    /// and `receipt-driven` with a sentence about the row and no sentence about
    /// either answer — so the arrow key that changes what a verdict is bound to
    /// was a guess between two words.
    ///
    /// That is the ratchet this crate applies to every refusal, applied to a
    /// screen: *a value the operator cannot discover is a value they cannot
    /// supply*. Discovering the spelling is not discovering the choice.
    ///
    /// `None` for an answer that is a placeholder rather than a choice — the
    /// `unset` on a row whose real values are typed. Held by
    /// `every_answer_a_setting_offers_says_what_it_means`, which is what stops
    /// this from covering fifteen rows and quietly missing the sixteenth.
    pub fn means(self, value: &str) -> Option<&'static str> {
        Some(match (self, value) {
            (Self::Delivery | Self::Review | Self::Transitions, "auto") => {
                "the run does it without asking"
            }
            (Self::Delivery | Self::Review | Self::Transitions, "ask") => {
                "the run proposes and waits; after the built-in timeout it records the proposal \
                 on the issue instead of applying it"
            }
            (Self::Delivery | Self::Review | Self::Transitions, "ask 30m" | "ask 2h") => {
                "the same, with that long to answer before it is recorded rather than applied"
            }
            (Self::Route, "direct") => "a branch in the tracker's own repository",
            (Self::Merge, "merge commit") => {
                "delivery must produce a commit whose parents are the reviewed base and head"
            }
            (Self::Merge, "squash") => "delivery must produce one commit off the reviewed base",
            (Self::Merge, "rebase") => "delivery must replay the reviewed commits onto the base",
            (Self::Tracker, "github") => "issues live in GitHub, reached through `gh`",
            (Self::Tracker, "linear") => "issues live in Linear; its binding is run by hand",
            (Self::Tracker, "trello") => "cards live in Trello; its binding is run by hand",
            (Self::Planning, "direct") => "acceptance criteria on the issue are the specification",
            // These said "five phases before code", flat, and that is not what
            // the protocol does. `protocols/sdd.md` engages the phases on
            // *ambiguity and nothing else*, and skips the ones an already
            // specified issue has answered — so the screen was teaching an
            // operator that choosing `sdd` buys five artifacts on every change,
            // which is the reading that makes a method feel like ceremony and
            // gets it switched off. Nothing about the behaviour changed here;
            // the sentence describing it did.
            (Self::Planning, "sdd") => {
                "up to five phases, engaged per change when it is ambiguous: \
                 explore, propose, spec, design, tasks"
            }
            (Self::Planning, "sdd lite") => "the short form, same rule: spec and tasks only",
            (Self::Planning, "sdd openspec") => {
                "the same five, with the artifacts written under `openspec/`"
            }
            (Self::Planning, "sdd lite openspec") => "spec and tasks, written under `openspec/`",
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
            (Self::ReviewProtocol, "standard") => {
                "a verdict is a review of the head it was published against"
            }
            (Self::ReviewProtocol, "receipt-driven") => {
                "RDD: a verdict must carry a receipt, the reviewer's own evidence bound to those \
                 bytes"
            }
            // Neither answer is the feature switched off, so neither sentence
            // is written as an absence. What separates them is what a finding
            // has to be produced by, and the capability follows from that rather
            // than the other way round.
            (Self::Evidence, "reading") => {
                "a finding here is established by reading the change, so a reviewer needs no shell"
            }
            (Self::Evidence, "measuring") => {
                "a finding here is established by running something, so a reviewer gets a shell. \
                 The isolated directory that makes a shell safe is the launch's to provide; \
                 Estigia grants the tool and cannot see the directory"
            }
            (Self::Judges, "single") => "one context reviews the change",
            (Self::Judges, "two blind") => {
                "two contexts review it without seeing each other's verdict"
            }
            (Self::Judges, "five blind") => {
                "five independent contexts review it blind; 3-of-5 must confirm the same severe finding"
            }
            (Self::Workers, "none") => "no delegated worker definition is installed here",
            (Self::Workers, "implementer") => {
                "one definition that may write and run the suite in the checkout its launch names"
            }
            (Self::Workers, "analyst") => {
                "one repository-read-only definition, for reading that prepares a write"
            }
            (Self::Workers, "implementer analyst") => "both definitions",
            (Self::ChangeSize, _) => {
                "the changed lines a pull request aims to stay under, and splits past unless the \
                 developer records why"
            }
            (Self::Boundaries, "none") => "no command beyond the built-in one-way doors",
            (Self::Board, "none") => "workflow state is not mirrored onto any board",
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
            Self::Delivery | Self::Review | Self::Transitions => {
                "`auto`, `ask`, or `ask` with a duration such as `ask 30m`"
            }
            Self::Route => "`direct`",
            Self::Merge => "`merge commit`, `squash`, or `rebase`",
            Self::Worktree => "`unset`, or an absolute directory",
            Self::Tracker => "`github`, `github <owner>/<name>`, `linear`, or `trello`",
            Self::Planning => "`direct`, `sdd`, `sdd lite`, `sdd openspec`, or `sdd lite openspec`",
            // Every key, not seven examples of sixteen. The parser takes a
            // delegated role, a phase of thinking, or the name of a sub-agent an
            // orchestration skill spawns — and this named three of the first,
            // two workflow states and two phases, so `orchestrate`, the one an
            // operator asks for first, appeared nowhere at all. It has always
            // worked; nothing said it existed. The states it did name are not
            // keys any more, which is the other half of the same fault: the
            // sentence and the parser have to be one thing.
            //
            // That is this crate's own ratchet turned on its configuration: *a
            // value the operator cannot discover is a value they cannot supply*.
            // Held complete by `every_key_the_routing_takes_is_a_key_it_names`,
            // which reads the three lists rather than trusting this sentence,
            // and by `a_workflow_state_is_no_longer_a_routing_key` in the other
            // direction — a word here the parser refuses is the same lie told
            // the other way round.
            Self::Models => {
                "`unset`, or comma-separated key=model pairs, as in \
                 `orchestrate=fable, design=opus, apply=sonnet/low`. A key is a delegated role \
                 (implementer, reviewer, judge), a phase of thinking (explore, propose, spec, \
                 design, tasks, apply, orchestrate), or a sub-agent (strategist, analyst, \
                 builder, refactorer, validator, auditor). A model ID may use any catalog \
                 spelling but no comma, pipe, or line break, and may carry the effort it runs at \
                 after a slash: low, medium, high, xhigh or max"
            }
            Self::Integration => "`branch`, or `trunk`",
            // Only values at or below the built-in. A longer one is refused,
            // and the refusal says so rather than clamping quietly: an operator
            // who asked for ten minutes and silently got two would believe the
            // gate is looser than it is.
            Self::Window => "`default`, or a shorter duration such as `30s` or `1m`",
            // `rdd` by name, because that **is** the name. `receipt-driven` is
            // what the protocol does; RDD is what people call it and what they
            // go looking for — and the parser has always taken it. A spelling
            // that works and is written nowhere is a feature nobody can find:
            // somebody read this whole screen for it and concluded it was
            // missing.
            Self::ReviewProtocol => "`standard`, or `receipt-driven` (also accepted as `rdd`)",
            Self::Judges => "`single`, `two blind`, or `five blind`",
            Self::Workers => "`none`, `implementer`, `analyst`, or `implementer analyst`",
            Self::Evidence => "`reading`, or `measuring`",
            Self::ChangeSize => "a number of lines, such as `800`",
            Self::Boundaries => "`none`, or commands separated by commas such as `npm publish`",
            Self::Board => "`none`, or a board as `<owner>/<number>`",
            Self::Summary | Self::Body => "a plain language name such as `English`",
        }
    }

    /// Reads one written value into the configuration.
    fn parse_into(self, config: &mut Config, value: &str) -> Result<(), Refusal> {
        // A cell is delimited by `|` and lives on one line. Nothing escapes
        // either, so a value carrying one cannot be stored: written, it becomes
        // extra cells, and read back it is the text before the first pipe.
        //
        // `estigia config set "Irreversible commands" "make deploy | tee log"`
        // wrote it, read back `make deploy`, and refused — correctly, because
        // the read-back caught the mismatch, but with the wrong cause. It named
        // `setting-shadowed-by-local-file` and sent the operator to look for a
        // row in a file that does not exist. `make deploy | tee log` is a
        // plausible command and a plausible thing to declare a one-way door.
        //
        // Refused here rather than after the write: what a table cannot carry
        // is knowable from the argument alone, and this crate settles those
        // before anything on disk is read.
        // The same rule one level out, for the markers that delimit the block
        // the row sits in. Nothing escapes those either: `estigia config set
        // "Summary language" "English <!-- estigia:config:end -->"` was
        // *accepted*, closed the block in the middle of its own table, and the
        // next write appended a second table under a second pair of markers —
        // a contract holding two of every setting, of which the agent reads
        // whichever it reaches first. The read-back reported it the same wrong
        // way the `|` case used to: `setting-shadowed-by-local-file`, naming a
        // file that was not there.
        //
        // The superseded pair as well. It delimits nothing this build writes
        // and everything an installation upgraded from issue-flow still has,
        // which is the case that already cost this crate two tables in one
        // file.
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
            Self::Delivery => config.delivery = parse_authority(self, value)?,
            Self::Review => config.review = parse_authority(self, value)?,
            Self::Transitions => config.transitions = parse_authority(self, value)?,
            Self::Route => {
                config.route = match lower(value).as_str() {
                    "direct" => DeliveryRoute::Direct,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::Merge => {
                config.merge = match lower(value).as_str() {
                    "merge commit" | "merge-commit" | "merge" => MergeStrategy::MergeCommit,
                    "squash" => MergeStrategy::Squash,
                    "rebase" => MergeStrategy::Rebase,
                    _ => return Err(self.reject(value)),
                }
            }
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
                    "linear" if rest.is_empty() => Tracker::Linear,
                    "trello" if rest.is_empty() => Tracker::Trello,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::Planning => {
                let value = value.trim();
                let (head, rest) = match value.split_once(char::is_whitespace) {
                    Some((head, rest)) => (head, rest.trim()),
                    None => (value, ""),
                };
                // The tail is read as a set of words rather than a fixed
                // phrase, so `sdd lite openspec` and `sdd openspec lite` are the
                // same setting. Two independent axes spelled in one cell cannot
                // have an order that matters without somebody getting it wrong.
                let tail = lower(rest);
                let mut words = tail.split_whitespace().peekable();
                let mut lite = false;
                let mut openspec = false;
                let mut understood = true;
                for word in words.by_ref() {
                    match word {
                        "lite" | "short" => lite = true,
                        "openspec" => openspec = true,
                        _ => understood = false,
                    }
                }
                config.planning = match lower(head).as_str() {
                    "direct" | "none" | "off" if tail.trim().is_empty() => Planning::Direct,
                    // `auto` and `per-issue` are accepted as spellings of `sdd`,
                    // not as a protocol of their own. There was briefly a
                    // `Planning::Auto` with four values, and it was a modelling
                    // error: `sdd` already engages per change, on ambiguity and
                    // nothing else, so `auto` asked the operator to choose
                    // between a protocol and the behaviour that protocol already
                    // has. Kept as words because somebody who reaches for `auto`
                    // is asking for exactly what `sdd` does, and refusing the
                    // word they reached for teaches them the setting does not.
                    "sdd" | "spec-driven" | "auto" | "per-issue" if understood => {
                        Planning::Sdd { openspec, lite }
                    }
                    _ => return Err(self.reject(value)),
                }
            }
            Self::Models => {
                config.models = match crate::config::ModelRouting::parse(value) {
                    Some(routing) => routing,
                    None => return Err(self.reject(value)),
                }
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
            Self::ReviewProtocol => {
                config.review_protocol = match lower(value).trim() {
                    "standard" | "direct" => ReviewProtocol::Standard,
                    "receipt-driven" | "rdd" | "receipt driven" => ReviewProtocol::ReceiptDriven,
                    _ => return Err(self.reject(value)),
                }
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
            Self::Workers => {
                config.workers = match Workers::parse(value) {
                    Some(workers) => workers,
                    None => return Err(self.reject(value)),
                };
            }
            Self::Judges => {
                config.judges = match lower(value).trim() {
                    "single" | "one" | "off" => Judges::Single,
                    "two blind" | "two" | "blind" | "on" => Judges::TwoBlind,
                    "five blind" | "five" => Judges::FiveBlind,
                    _ => return Err(self.reject(value)),
                }
            }
            // No `on`/`off` synonyms here on purpose, unlike the row above. Both
            // answers are a positive statement about this repository, and there
            // is no direction in which one of them is the feature being switched
            // off — `reading` is the narrower grant, not the absent one.
            Self::Evidence => {
                // The two words the row takes, and no synonyms. A first draft
                // accepted `read`, `measure` and `mutation`; a reviewer measured
                // that deleting all three left the whole suite green, and none of
                // them appears in `accepts`, in `docs/configuration.md`, or in the
                // picker. An undocumented spelling is one an operator cannot
                // discover and a maintainer cannot know is load-bearing — unlike
                // `rdd`, which the row's own `accepts` names.
                config.evidence = match lower(value).trim() {
                    "reading" => Evidence::Reading,
                    "measuring" => Evidence::Measuring,
                    _ => return Err(self.reject(value)),
                }
            }
            Self::Board => {
                config.board = match lower(value).as_str() {
                    "none" | "unset" | "" => None,
                    _ => Some(BoardRef::parse(value)?),
                }
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
            Self::ChangeSize => config.change_size.to_string(),
            Self::Delivery => render_authority(config.delivery),
            Self::Route => match config.route {
                DeliveryRoute::Direct => "direct".to_owned(),
            },
            Self::Review => render_authority(config.review),
            Self::Transitions => render_authority(config.transitions),
            Self::Merge => match config.merge {
                MergeStrategy::MergeCommit => "merge commit".to_owned(),
                MergeStrategy::Squash => "squash".to_owned(),
                MergeStrategy::Rebase => "rebase".to_owned(),
            },
            Self::Worktree => match &config.worktree {
                WorktreeRoot::Auto => "unset".to_owned(),
                WorktreeRoot::Path(path) => path.display().to_string(),
            },
            Self::Tracker => config.tracker.as_value(),
            Self::Planning => config.planning.as_value(),
            Self::Models => config.models.as_value(),
            Self::Integration => config.integration.as_value(),
            Self::Window => {
                if config.window == crate::harness::RENEWAL_WINDOW {
                    "default".to_owned()
                } else {
                    render_duration(config.window)
                }
            }
            Self::ReviewProtocol => config.review_protocol.as_value().to_owned(),
            Self::Judges => config.judges.as_value().to_owned(),
            Self::Workers => config.workers.as_value(),
            Self::Evidence => config.evidence.as_value().to_owned(),
            Self::Boundaries => {
                if config.boundaries.is_empty() {
                    "none".to_owned()
                } else {
                    config.boundaries.join(", ")
                }
            }
            Self::Board => match &config.board {
                None => "none".to_owned(),
                Some(board) => board.as_str().to_owned(),
            },
            Self::Summary => config.summary_language.as_str().to_owned(),
            Self::Body => config.body_language.as_str().to_owned(),
        }
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

fn lower(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// Labels compare on their words, so a stray backtick or a doubled space in
/// somebody's hand-edited table does not silently drop their setting.
/// The settings the transport itself reads out of the table.
///
/// Three, and only three: the transport calls `context.get` for `project board`,
/// `worktree location` and `Review delegation`, and for nothing else. The third
/// arrived with the review handoff, which stamps the configured authority and one
/// deadline into the request marker — it is read to be *recorded*, never to
/// decide whether to wait. Every other row is read by the agent out of the prose,
/// or by the gate. Held here because the check below is only worth making for
/// rows a second reader actually consults, and crossed by
/// `the_transport_reads_the_settings_this_crate_says_it_does` so the list cannot
/// drift away from the code it describes.
pub const READ_BY_THE_TRANSPORT: &[Setting] = &[Setting::Board, Setting::Worktree, Setting::Review];

/// Whether a row a person typed is one the transport will match.
///
/// The two readers differ, and this crate is the more forgiving: it collapses
/// runs of whitespace and strips the backticks a person writing markdown puts
/// round a name. The answer here is the **stricter** side's, so a crossing can
/// ask what the transport would have seen.
///
/// | First cell | `config list` | the transport |
/// |---|---|---|
/// | `Project board` | `acme/7` | `acme/7` |
/// | `` `Project board` `` | `acme/7` | nothing |
/// | `Project  board` (two spaces) | `acme/7` | nothing |
/// | `Project board (mine)` | `acme/7` | `acme/7` |
///
/// Estigia is **not** made stricter to close it. Backticks and stray spaces
/// reach the gate for every row, including `Irreversible commands`, and refusing
/// them there would drop the operator's declared boundaries — trading a report
/// nobody makes for a gate that decides less.
///
/// An approximation in one place, and a stated one: the retired Python
/// lowercased the whole of Unicode where this lowercases ASCII. Every published
/// label is ASCII, so the two agree on all of them; a first cell that differs
/// only in the case of a non-ASCII letter would be read as reaching the
/// transport when it does not.
pub fn reaches_the_transport(label: &str, setting: Setting) -> bool {
    let key = label.trim().to_lowercase();
    std::iter::once(setting.label())
        .chain(setting.aliases().iter().copied())
        .any(|name| key.starts_with(&name.to_lowercase()))
}

fn normalize_label(label: &str) -> String {
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
/// Public because the transport reads `Review delegation` as well, and this rule
/// had been written three times — here, in the transport's `review_authority`,
/// and again in the handoff marker's own validator. The three already disagreed.
/// `Auto`, `Ask 30m` and `ask 30 m` parse here and were refused by the transport,
/// which mattered because the review handoff is the only exit from the livelock
/// a blocked run has: a mis-cased row left it holding the issue for good. Worse,
/// `ask  30m` passed the transport, was stamped into the marker, and was then
/// refused by the marker's reader — comment posted, readback failed, and every
/// retry answering `review-handoff-operation-conflict` forever.
///
/// Both of those spellings also read the duration by byte offset, so a value
/// ending in a multi-byte character panicked the process rather than being
/// refused. Here the split is taken at the first non-digit **character**, which
/// is a boundary by construction.
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
/// Every character that ends a line for the readers of this table.
///
/// One list, because two rules exist over it and they must not drift: which
/// region becomes which rows ([`lines_the_transport_sees`]), and which of those
/// characters can appear *inside* a row and cut it in half
/// ([`rows_split_by_a_separator`]). A second copy is how the crossing that
/// found this list in the first place came to be needed.
const ENDS_A_LINE: [char; 10] = [
    '\n', '\r', '\u{0b}', '\u{0c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// `str::lines` ends a line at a newline and nothing else. The retired Python's
/// `splitlines`, which read the same document, ended one at ten characters — and
/// while both readers shipped, the difference was not academic. It is kept
/// because the behaviour is the operator's, not the corpus's: the crossings that
/// pinned it are deleted, and a reader that quietly changed here would now be
/// caught by the unit tests below and by nothing else. Measured across a
/// document holding
/// `| Tracker | github |<sep>| Tracker | linear |`:
///
/// | separator | the transport read | this crate read |
/// |---|---|---|
/// | U+000C form feed | `linear` | `github` |
/// | U+2028 line separator | `linear` | `github` |
/// | U+0085 next line | `linear` | `github` |
/// | U+000B, U+001C–U+001E, U+2029 | `linear` | `github` |
/// | U+001F unit separator | `github` | `github` |
///
/// Six of seven: the agent reading one tracker out of a file while the gate
/// enforced another out of the same file, one row apart. That is the failure
/// this crate names in its own contributing rules — *a rule ported to a second
/// language where no crossing could see it* — and the crossing that existed fed
/// **cell values**, so the question of which region becomes which rows was never
/// put to both sides.
///
/// It needs no adversary. `estigia.local.md` is the one file Estigia never
/// writes, so it is typed and pasted by hand, and U+2028 is what a paste out of
/// a browser or a word processor routinely carries.
///
/// U+001F is absent on purpose: Python's own set stops at U+001E, and a reader
/// stricter than the transport is the same disagreement facing the other way.
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
/// A `|` is what separates cells, so a value holding one splits the row a cell
/// early and the value arrives **truncated**. `config set` refuses such a value
/// by name — *the value is stored as one cell of a one-line table row, and
/// nothing escapes either* — and that covers the door Estigia writes through.
/// The other door reads a file Estigia never writes, where nothing refuses
/// anything. Measured on the installed pair, from a hand-written
/// `estigia.local.md`:
///
/// ```text
/// the operator wrote : make deploy | tee log
/// the tool believes  : make deploy
/// ```
///
/// On the row that declares one-way doors. A value read as something the file
/// does not say is the failure this tool exists to refuse, and it was silent.
///
/// A row is cut by a **cell** separator or by a **line** separator, and the two
/// are found differently. The bar makes the row wider than the header it sits
/// under, which is cheap and exact and needs no second parser — the header is
/// the document's own statement of how many columns it has. The eight line
/// endings in `ENDS_A_LINE` that are not `\n` or `\r` make it *narrower*, or
/// no row at all, so width cannot see them; they are found by looking at the
/// line an editor draws and asking whether one is inside it.
///
/// The second was measured on the same row as the first, and it loses the same
/// half:
///
/// ```text
/// the operator wrote : make deploy<U+2028>npm publish
/// the tool believes  : make deploy
/// ```
///
/// `npm publish` stops being a declared one-way door. That direction matters:
/// *configuration may only tighten*, and a value read as less than it says
/// always loosens. `config set` refuses a `|`, a `\n` and a `\r` by name, and
/// U+2028 is exactly the one a paste out of a browser carries — invisible in
/// every editor that does not go looking for it, and read by both this crate
/// and the transport as the end of the row.
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
    // Only the marked block, when the document carries one — the transport reads
    // exactly that region and nothing else.
    //
    // Reading the whole file was reachable and wrong: the shipped `SKILL.md` has
    // 29 table rows and 10 of them are the configuration. The other 19 are prose
    // tables ("Situation | Action", "Load | When"), and they were being offered
    // as settings. Nothing broke, because no prose row's first cell happens to
    // spell a setting label — which is luck, not a property, and the kind that
    // ends the first time somebody writes a table row starting with "Tracker".
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
            // The header, and an empty first cell. The transport drops both, and
            // the difference was visible: Estigia offered `setting = "Value
            // here"` as a configured row. Harmless only for as long as no
            // setting is ever labelled `Setting`.
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
/// The table is prose as much as it is data — the shipped contract itself
/// writes ``| Tracker | `github` | `github` (also `linear`, `trello`) |`` — so a
/// cell carries decoration a person needs and a program does not: a backticked
/// token, bold, an explanation after an em dash or in parentheses.
///
/// This is the transport's `_clean_value`, in Rust, and it is here because the
/// two of them read the *same file* and used to read it differently.
/// `tests/differential.rs` caught it on its first run: the transport read
/// `squash` out of ``| Merge strategy | `squash` — keeps the branch history off
/// the base |`` and Estigia refused the whole contract with
/// `config-value-unrecognised`. Refusing is not where that ends — the gate calls
/// `installed_config(...).unwrap_or_default()`, so it would have gone on
/// enforcing the *defaults* while the transport honoured what the operator
/// actually wrote. One file, two answers, and no error anywhere a person looks.
///
/// The transport's normalisation is the older published behaviour and the one
/// the shipped table teaches, so Estigia is the side that moves.
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
pub fn render_rows(config: &Config) -> String {
    rows_of(config, SETTINGS)
}

/// The table an adapter's **own** file carries: the rows that differ by agent.
///
/// Not the whole table. An adapter's file is layered over the shared contract
/// by [`crate::skill::installed_config_for`], and a repository-wide row in it is
/// read by nobody: the gate resolves those from the contract, because at gate
/// time there is no agent to narrow to. `doctor` says so in as many words —
/// *"N rows in its own file that nothing reads"* — and it was saying it about a
/// file **Estigia had just written**. The scope split was built carefully in
/// `Scope` and thrown away in the last rendering step.
///
/// A row dropped here loses nothing an operator had: it was already reported as
/// the contract's value by every command that reads one. What goes is the lie.
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
///
/// Measured through the binary, one command:
///
/// ```text
/// before   Worktree: C:/trees     Tracker: github acme/web
/// asked    Merge strategy = rebase --repo
/// after    Worktree: unset        Tracker: github
/// ```
///
/// So asking a checkout one question silently answered two more with factory
/// values, and that checkout pointed at a different tracker than the operator
/// had configured. Nothing said so.
pub fn render_some_repository_rows(config: &Config, speaks_for: &[Setting]) -> String {
    let ordered: Vec<Setting> = EVERYWHERE_SETTINGS
        .iter()
        .copied()
        .filter(|setting| speaks_for.contains(setting))
        .collect();
    rows_of(config, &ordered)
}

/// The same, for an adapter's own file inside a shared skill root.
///
/// Same rule and same reason as the repository's: that file is an override, and
/// a row it does not carry is one this adapter does not answer for. Writing the
/// whole scope pinned five rows for every one asked about — measured with two
/// adapters sharing `~/.agents`: after `config set "Blind judges" "two blind"
/// --agent cursor`, moving the machine's `Planning` moved `qwen` and left
/// `cursor` behind, on a row nobody had set for it.
///
/// Less costly than the repository's, and only by luck of where the values come
/// from: this one writes the **effective** answers rather than the defaults, so
/// nothing is lost at the moment of writing. What is lost is later, quietly, and
/// the operator has no reason to look.
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
