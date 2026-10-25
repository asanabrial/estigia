//! The typed configuration model.
//!
//! This is where Rust pays. Every setting is an enum and every consumer is an
//! exhaustive `match`, so adding a `Tracker` does not compile until it has been
//! decided what it does everywhere it is asked.
//!
//! The markdown table stays — it is what the agent reads — but reading it
//! produces a [`Config`] or a [`Refusal`], and nothing in between.

use std::path::PathBuf;
use std::time::Duration;

use crate::harness::population::{Disposition, Population};
use crate::outcome::{NoCommandReason, Refusal, Resolution};

mod block;
mod delivery;
mod settings;

pub use block::{BLOCK_BEGIN, BLOCK_END, CONFIG_FENCE};
pub use delivery::{
    Corrections, DeliveryPreset, MAPPED_STATES, Party, PresetRows, ReviewStage, StageKind,
};
pub(crate) use delivery::{parse_stages, render_stages};
pub(crate) use settings::retired_setting_report;
pub use settings::{
    AGENT_SETTINGS, Answers, CutBy, CutShort, EVERYWHERE_SETTINGS, FIXED_UNDER_AUTONOMOUS, Group,
    MACHINE_SETTINGS, OPTIONS_SETTINGS, READ_BY_THE_TRANSPORT, REPOSITORY_SETTINGS, SETTINGS,
    Scope, Setting, authority_of, reaches_the_transport, rendered_authority, rows as table_rows,
    rows_split_by_a_separator,
};

/// The name of the ignored local file that overrides the versioned table.
/// Renamed from issue-flow's `operator.local.md`; the old name is still read
/// when present — see [`LEGACY_LOCAL_FILE`].
pub const LOCAL_FILE: &str = "estigia.local.md";

/// The file issue-flow wrote. Read for compatibility, never written.
pub const LEGACY_LOCAL_FILE: &str = "operator.local.md";

/// How long an `ask` authority waits before recording its proposal instead of
/// applying it.
pub const DEFAULT_ASK_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// The eight branches on [`Tracker::transport`] that answer its absent arm.
///
/// [`Tracker::Linear`] is that second variant: its *tracking*
/// operations are not implemented, so `transport()` answers `None` and every
/// arm below is reached when the tracker is Linear, except `cli::inert_tracker`,
/// which asks `Tracker::adjudicates_unclaimed_writes` and so excludes a
/// read-only tracker only under `Workflow: assisted`, where a run does swear
/// against it. The two refusals that need only that a run *can* hold a claim,
/// `harness::unclaimed::applies` and `harness::issue_requirement::in_force`,
/// ask [`Tracker::adjudicates_unclaimed_writes`] instead and so hold under
/// Linear only with `Workflow: assisted`. They are the seam, and this declaration binds to `Tracker`'s own variants — so adding
/// one moves the fingerprint and `tests/guards.rs` sends a reader back to each
/// site, which is how each was walked through when Linear landed.
pub const TRACKER_TRANSPORT_ABSENT_POPULATION: Population = Population {
    family: "tracker-transport-absent",
    disposition: Disposition::TooTight,
    subject: "Tracker",
    text: "the eight branches on `Tracker::transport()` answering `None`: \
           `cli::doctor`'s \"has no executable transport\" sentence and \
           `cli::inert_tracker`, `harness::doctor::stale_run_pointers`'s \
           `Skipped` row, `harness::mcp::run_tool`'s tracker arm, \
           `harness::decide_observing`'s `NoTracker` aside, \
           `harness::tracker_answer_for_pointer`, `harness::queue_read`, \
           and `harness::completed_work`. Each \
           is reached when `Tracker` is `Linear`, except `cli::inert_tracker`, \
           which asks `Tracker::adjudicates_unclaimed_writes` and so excludes \
           a read-only tracker only under `Workflow: assisted`, where a run does \
           swear against it; \
           `harness::unclaimed::applies` and `harness::issue_requirement::in_force` \
           ask `Tracker::adjudicates_unclaimed_writes` instead, which a further \
           variant must also decide; a \
           further variant must be walked through every one. Boundary: a call site added before \
           a further variant lands is not caught, which is the escape this \
           population names rather than hides.",
};

/// Which tracker holds the issues, and what identifies the project in it.
///
/// Distinct from [`RepositoryHost`]: the tracker adjudicates claims and
/// workflow state; the host publishes pull requests and tags. GitHub is both
/// today. Linear and Trello were removed as configuration claims with no
/// executable; re-adding them is a tracker transport, not a host one.
/// [`Tracker::Linear`] returned as a
/// **read-only** source admitted only under `Workflow: assisted`: its
/// [`Tracker::transport`] answers `None`, because Estigia writes nothing to
/// Linear and its tracking operations have no implementation. [`Tracker`] stays
/// an enum so typed answers remain validated (`estigia config set Tracker linear`
/// still fails without a team key, not silently coerces), and
/// [`Tracker::transport`] is consulted per tracking operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tracker {
    /// GitHub Issues, reached through `gh`. `repo` is `None` to discover it
    /// from the git remote.
    Github {
        /// The `owner/name` pair, or `None` to discover it from the git remote.
        repo: Option<RepoRef>,
    },
    /// Linear, read-only, under `Workflow: assisted`. `team` is the team key the
    /// issue identifiers carry (the `ENG` of `ENG-123`), recorded so the source
    /// is named in configuration and refused under autonomous work.
    Linear {
        /// The team key, e.g. `ENG`.
        team: String,
    },
}

impl Tracker {
    /// The binding file under `bindings/` that owns this tracker's exact
    /// commands. Every variant must name one, and the seam tests check that the
    /// file it names exists.
    pub fn binding(&self) -> &'static str {
        match self {
            Self::Github { .. } => "bindings/github.md",
            Self::Linear { .. } => "bindings/linear.md",
        }
    }

    /// Whether this tracker's **tracking** operations have an implementation.
    ///
    /// `None` for [`Tracker::Linear`]: it is read-only and admitted only under
    /// assisted work, so no claim, state, comment or label is ever written
    /// through it. The read the assisted path needs is
    /// [`crate::transport::linear`], not a tracking operation.
    pub fn transport(&self) -> Option<&'static str> {
        match self {
            Self::Github { .. } => Some("estigia"),
            Self::Linear { .. } => None,
        }
    }

    /// Whether this tracker is read-only and so admits only assisted work.
    ///
    /// The one predicate the three refusals read, so the reason
    /// is named in one place rather than three.
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::Linear { .. })
    }

    /// Whether a run can come to hold a claim against this tracker, which is what
    /// makes refusing an unsworn write a lock somebody can open.
    ///
    /// True for GitHub, which swears through its transport, and for a read-only
    /// tracker only when `workflow` is assisted, the one workflow that holds a
    /// local claim against it. Under any other workflow nothing could clear the
    /// refusal, so it stands down.
    pub fn adjudicates_unclaimed_writes(&self, workflow: Option<Workflow>) -> bool {
        self.transport().is_some() || (self.is_read_only() && workflow == Some(Workflow::Assisted))
    }

    /// The Linear team key, when this is the Linear tracker.
    pub fn team(&self) -> Option<&str> {
        match self {
            Self::Github { .. } => None,
            Self::Linear { team } => Some(team),
        }
    }

    /// The repository the operator named, when they named one.
    ///
    /// When it is `None`, an operator whose issues live in a different
    /// repository from the code gets them filed in the checkout's own, because
    /// that is what `gh` infers. Linear names no repository.
    pub fn named_repo(&self) -> Option<String> {
        match self {
            Self::Github { repo: Some(repo) } => Some(repo.to_string()),
            Self::Github { repo: None } => None,
            Self::Linear { .. } => None,
        }
    }

    /// The value written back into the table.
    pub fn as_value(&self) -> String {
        match self {
            Self::Github { repo: None } => "github".to_owned(),
            Self::Github { repo: Some(repo) } => format!("github {repo}"),
            Self::Linear { team } => format!("linear {team}"),
        }
    }
}

/// Where the code lives and publications are verified — pull requests, tags,
/// and the receipt-bound review half.
///
/// Separate from [`Tracker`] so a future Linear (or other) tracker can hold
/// issues while GitHub remains the host. Only GitHub is implemented;
/// the enum shape matches [`Tracker`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryHost {
    /// GitHub, reached through `gh`. `repo` is `None` to discover it from the
    /// git remote — the same spelling as [`Tracker::Github`].
    Github {
        /// The `owner/name` pair, or `None` to discover it from the git remote.
        repo: Option<RepoRef>,
    },
}

impl RepositoryHost {
    /// Whether this host's **publication** operations have an implementation.
    pub fn transport(&self) -> Option<&'static str> {
        match self {
            Self::Github { .. } => Some("estigia"),
        }
    }

    /// The repository the operator named on the host, when they named one.
    pub fn named_repo(&self) -> Option<String> {
        match self {
            Self::Github { repo: Some(repo) } => Some(repo.to_string()),
            Self::Github { repo: None } => None,
        }
    }

    /// The value written back into the table.
    pub fn as_value(&self) -> String {
        match self {
            Self::Github { repo: None } => "github".to_owned(),
            Self::Github { repo: Some(repo) } => format!("github {repo}"),
        }
    }

    /// A host matching this tracker when the tracker *is* a host, else GitHub
    /// with no named repo — so an installation that only sets `Tracker` keeps
    /// one place to configure.
    pub fn from_tracker(tracker: &Tracker) -> Self {
        match tracker {
            Tracker::Github { repo } => Self::Github { repo: repo.clone() },
            // Linear names no repository, so the host stays GitHub with nothing
            // pinned — the assisted path publishes its draft pull request there.
            Tracker::Linear { .. } => Self::Github { repo: None },
        }
    }

    /// Whether this is still the auto-default host (no pin).
    pub fn is_default_github(&self) -> bool {
        matches!(self, Self::Github { repo: None })
    }
}

/// What a verdict in this repository has to be backed by.
///
/// A row rather than a constant because a fixed read-only grant left a judge
/// unable to build or run the suite in a repository whose evidence standard is
/// mutation, so every panel that mattered was launched outside the role and
/// outside every guarantee the contract attaches to it. No neighbouring row
/// encodes this; it is not derived from one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// A finding is established by reading the target. The default, and
    /// deliberately the narrower one: a grant that widened on upgrade would
    /// hand every delegated context a shell nobody asked for.
    Reading,
    /// A finding is established by running something against the target. The
    /// capability comes with the precondition that makes it safe — a directory
    /// nothing else writes for the duration.
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
    /// arm and leaving the list short still compiles.
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

/// Whether `record_computed_verdict` consults the publication lane for an approval.
///
/// A repository whose CI is disabled or unbillable still had no way to say
/// the lane gate is not in force — only to supply never-ran evidence per verdict.
/// `Off` skips the check-run read and records that the lane was not consulted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationLane {
    /// The default: accepted verdicts are gated on the head's check runs.
    On,
    /// Do not read check runs for an accepted verdict; say so in the answer.
    Off,
}

impl PublicationLane {
    /// The value as written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }

    /// Every answer, under an exhaustive match.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::On, Self::Off];
        for lane in &every {
            match lane {
                Self::On | Self::Off => {}
            }
        }
        every
    }
}

/// What a run holding no claim may do to a checkout that has a tracker.
///
/// Repository-only: it is read from the repository's own file and nowhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueRequirement {
    /// The default: the host is asked before an unclaimed write, except inside an isolated
    /// checkout that a run pointer's `worktree` covers.
    Ask,
    /// Unclaimed writes are allowed.
    Off,
    /// Unclaimed writes are refused.
    Required,
}

impl IssueRequirement {
    /// The value as written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Off => "off",
            Self::Required => "required",
        }
    }

    /// Every answer, under an exhaustive match.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Ask, Self::Off, Self::Required];
        for requirement in &every {
            match requirement {
                Self::Ask | Self::Off | Self::Required => {}
            }
        }
        every
    }
}

/// Who witnesses a claim in this repository.
///
/// Repository-only: it is read from the repository's own file and nowhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workflow {
    /// The default: Estigia holds the claim and writes the tracker.
    Autonomous,
    /// The team coordinates, and Estigia writes nothing to the tracker.
    Assisted,
}

impl Workflow {
    /// The value as written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Autonomous => "autonomous",
            Self::Assisted => "assisted",
        }
    }

    /// Every answer, under an exhaustive match.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Autonomous, Self::Assisted];
        for workflow in &every {
            match workflow {
                Self::Autonomous | Self::Assisted => {}
            }
        }
        every
    }
}

/// Whether `setup` writes one of the optional loops: the Estigia loop, as
/// OpenCode's plugin or Codex's `Stop` and `SessionStart` hook entries, or
/// the generic goal and loop plugin OpenCode and Pi read, each under its own row.
///
/// The Estigia loop row is offered where
/// [`crate::setup::AgentAdapter::offers_loop`] holds and the generic one where
/// [`crate::setup::AgentAdapter::has_loop_plugin`] does — see
/// [`Setting::applies_to_agent`]. `Off` is the default: a continuation nobody
/// asked for is a behaviour change to every install this crate touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopPlugin {
    /// The default: no loop plugin is written.
    Off,
    /// The loop is written: the plugin beside OpenCode's gate, or Codex's entries.
    On,
}

impl LoopPlugin {
    /// The value as written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }

    /// Every answer, under an exhaustive match.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::On, Self::Off];
        for plugin in &every {
            match plugin {
                Self::On | Self::Off => {}
            }
        }
        every
    }
}

/// Whether a reviewer holds any tool at all, orthogonal to [`Evidence`].
///
/// Not a [`Setting`]: no `Config` field constructs [`Self::Sealed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seal {
    /// One tool grant, `WebSearch` — no filesystem read, but reaching the
    /// public internet. See `docs/honesty.md` for what that leaves unenforced.
    Sealed,
    /// The grant [`Evidence`] decides. Not the only reachable value: six of
    /// seven `RESERVED_REVIEWERS` rows construct [`Self::Sealed`] instead.
    Unsealed,
}

impl Seal {
    /// The value as it would be written in the table, mirroring
    /// [`Evidence::as_value`].
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Sealed => "sealed",
            Self::Unsealed => "unsealed",
        }
    }

    /// Every value, under an exhaustive match for the same reason
    /// [`Evidence::all`] states.
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Sealed, Self::Unsealed];
        for seal in &every {
            match seal {
                Self::Sealed | Self::Unsealed => {}
            }
        }
        every
    }
}

/// The per-context token figure a review is reported against.
///
/// Advisory: nothing here refuses a panel that runs over it, records a comparison, or reads `record_spend`'s
/// own `tokens` field to check one. `docs/honesty.md` says what the number
/// behind the default can and cannot count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewBudget {
    /// No declared figure.
    Off,
    /// That many tokens, per review context.
    Tokens(u32),
}

impl Default for ReviewBudget {
    fn default() -> Self {
        Self::Tokens(150_000)
    }
}

impl ReviewBudget {
    /// The value as it is written in the table.
    pub fn as_value(self) -> String {
        match self {
            Self::Off => "none".to_owned(),
            Self::Tokens(n) => n.to_string(),
        }
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
    /// The repository a GitHub remote URL names, or `None` for any other host or shape.
    pub fn from_github_remote(url: &str) -> Option<Self> {
        let url = url.trim();
        let path = [
            "https://github.com/",
            "git@github.com:",
            "ssh://git@github.com/",
        ]
        .iter()
        .find_map(|prefix| url.strip_prefix(prefix))?;
        let path = path.trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        if path.matches('/').count() != 1 {
            return None;
        }
        Self::parse(path).ok()
    }

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
/// `Ask` **proposes and waits**, and on expiry **records the proposed
/// transition as a comment on the issue** instead of applying it, so a run
/// that dies leaves a legible record rather than a state nobody wrote.
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

/// One whole posture, in a word, over the two authorisations that can ask.
///
/// Derived rather than stored: naming a profile writes the rows it means, so
/// there is no profile field that an explicit `Delivery authorisation` or
/// `Transition authorisation` row could lose to. That
/// is what "an explicit row beats the profile" is — there is nowhere else for
/// the profile to live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Delivery and transitions `auto`: the run decides without asking.
    Balanced,
    /// Delivery and transitions `ask`: the run proposes and waits. Review
    /// delegation is not asked about, because its `ask` is retired.
    Cautious,
}

impl Profile {
    /// Its spelling in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Cautious => "cautious",
        }
    }

    /// Every profile, under an exhaustive match, so a third cannot be added
    /// without arriving here. See [`Evidence::all`].
    pub fn all() -> Vec<Self> {
        let every = vec![Self::Balanced, Self::Cautious];
        for profile in &every {
            match profile {
                Self::Balanced | Self::Cautious => {}
            }
        }
        every
    }

    /// The profile this word names, or nothing.
    pub fn parse(word: &str) -> Option<Self> {
        let word = word.trim().to_ascii_lowercase();
        Self::all()
            .into_iter()
            .find(|profile| profile.as_value() == word)
    }

    /// The authority every authorisation takes under this profile.
    pub fn authority(self) -> Authority {
        match self {
            Self::Balanced => Authority::Auto,
            Self::Cautious => Authority::Ask {
                timeout: DEFAULT_ASK_TIMEOUT,
            },
        }
    }
}

/// A language an issue is written in.
///
/// Free text, but validated: a value with a pipe in it would silently break
/// the table it is written back into. Two rows carry one of these because they
/// govern separate prose — the one-line summary and the body.
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

/// When a run may commit, push, or create a branch in this repository.
///
/// Disabled by default, which preserves today's unrestricted behaviour. Enabled
/// carries one daily range in this machine's local civil time, read as the
/// half-open interval `[start, end)`: the start minute is inside, the end
/// minute is outside. A start later than the end crosses midnight, and the
/// part after midnight belongs to the day the range started on. Equal
/// endpoints are invalid. `days` restricts the range to a set of weekdays;
/// without one every day is meant. `invert` permits the exact complement over
/// the whole week instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitOperationWindow {
    /// No schedule: protected operations are decided by the other checks alone.
    #[default]
    Disabled,
    /// Protected operations are permitted only inside the range, or only
    /// outside it when `invert` is set. Minutes since midnight, `[start, end)`.
    Enabled {
        /// The first permitted minute, inclusive.
        start: u16,
        /// The first minute past the range, exclusive.
        end: u16,
        /// The weekdays the range applies to, one bit per day with Monday as
        /// bit zero; zero means no day set was written and so every day.
        days: u8,
        /// Whether the range names the denied period instead of the permitted one.
        invert: bool,
    },
    /// A row was written and it does not parse. Denies every protected
    /// operation: an unknown schedule is not clearance.
    Invalid,
    /// The repository a write is aimed at has no checkout on this machine whose
    /// rows can be read. Denies every protected operation: a schedule nobody
    /// can read is not clearance either.
    Unreadable,
}

const DAY_NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
const EVERY_DAY: u8 = 0b111_1111;

fn parse_day(text: &str) -> Option<usize> {
    DAY_NAMES
        .iter()
        .position(|name| text.eq_ignore_ascii_case(name))
}

fn parse_days(text: &str) -> Option<u8> {
    let mut days = 0u8;
    for item in text.split(',') {
        let (first, last) = match item.split_once('-') {
            Some((first, last)) => (parse_day(first)?, parse_day(last)?),
            None => {
                let day = parse_day(item)?;
                (day, day)
            }
        };
        let mut day = first;
        loop {
            let bit = 1u8 << day;
            if days & bit != 0 {
                return None;
            }
            days |= bit;
            if day == last {
                break;
            }
            day = (day + 1) % 7;
        }
    }
    Some(days)
}

fn render_days(days: u8) -> String {
    let mut parts = Vec::new();
    let mut day = 0;
    while day < 7 {
        if days & (1 << day) == 0 {
            day += 1;
            continue;
        }
        let first = day;
        while day + 1 < 7 && days & (1 << (day + 1)) != 0 {
            day += 1;
        }
        parts.push(if day - first >= 2 {
            format!("{}-{}", DAY_NAMES[first], DAY_NAMES[day])
        } else if day > first {
            format!("{},{}", DAY_NAMES[first], DAY_NAMES[day])
        } else {
            DAY_NAMES[first].to_owned()
        });
        day += 1;
    }
    parts.join(",")
}

impl GitOperationWindow {
    /// Reads one written value. `None` is not disabled, it is malformed.
    pub fn parse(value: &str) -> Option<Self> {
        let text = value.trim();
        if text.is_empty()
            || text.eq_ignore_ascii_case("disabled")
            || text.eq_ignore_ascii_case("off")
            || text.eq_ignore_ascii_case("none")
            || text.eq_ignore_ascii_case("unset")
        {
            return Some(Self::Disabled);
        }
        let head = text.split_whitespace().next()?;
        if !head.eq_ignore_ascii_case("enabled") {
            return None;
        }
        let rest = text[head.len()..].trim_start();
        if rest.is_empty() {
            return None;
        }
        let (range, invert) = match rest.rsplit_once(char::is_whitespace) {
            Some((range, tail)) if tail.eq_ignore_ascii_case("inverted") => (range, true),
            _ => (rest, false),
        };
        let first = range.split_whitespace().next()?;
        let (days, range) = if first.starts_with(|c: char| c.is_ascii_alphabetic()) {
            (parse_days(first)?, range[first.len()..].trim_start())
        } else {
            (0, range)
        };
        let (start_text, end_text) = range.split_once('-')?;
        let start = parse_civil_minutes(start_text.trim())?;
        let end = parse_civil_minutes(end_text.trim())?;
        (start != end).then_some(Self::Enabled {
            start,
            end,
            days,
            invert,
        })
    }

    /// The value as it is written in the table.
    pub fn as_value(self) -> String {
        match self {
            Self::Disabled => "disabled".to_owned(),
            Self::Enabled {
                start,
                end,
                days,
                invert,
            } => {
                let days = if days == 0 {
                    String::new()
                } else {
                    format!("{} ", render_days(days))
                };
                let rendered = format!(
                    "enabled {days}{:02}:{:02}-{:02}:{:02}",
                    start / 60,
                    start % 60,
                    end / 60,
                    end % 60
                );
                if invert {
                    format!("{rendered} inverted")
                } else {
                    rendered
                }
            }
            Self::Invalid => "invalid".to_owned(),
            Self::Unreadable => "unreadable".to_owned(),
        }
    }

    /// The day set as written, or nothing when every day is meant.
    pub fn days_value(self) -> Option<String> {
        match self {
            Self::Enabled { days, .. } if days != 0 => Some(render_days(days)),
            _ => None,
        }
    }

    /// Whether this names an enabled schedule.
    pub fn enabled(self) -> bool {
        matches!(self, Self::Enabled { .. })
    }

    /// Whether a protected operation may run at this minute since Monday
    /// midnight, in the machine's local time.
    pub fn permits(self, week_minute: u16) -> bool {
        match self {
            Self::Disabled => true,
            Self::Invalid => false,
            Self::Unreadable => false,
            Self::Enabled {
                start,
                end,
                days,
                invert,
            } => {
                let days = if days == 0 { EVERY_DAY } else { days };
                let day = (week_minute / 1440 % 7) as u8;
                let minutes = week_minute % 1440;
                let on = |day: u8| days & (1 << day) != 0;
                let inside = if start < end {
                    on(day) && minutes >= start && minutes < end
                } else {
                    (on(day) && minutes >= start) || (on((day + 6) % 7) && minutes < end)
                };
                inside != invert
            }
        }
    }
}

/// Reads one `H:MM` or `HH:MM` civil time as minutes since midnight.
pub fn parse_civil_minutes(text: &str) -> Option<u16> {
    let (hours, minutes) = text.split_once(':')?;
    let hours: u16 = hours.parse().ok()?;
    let minutes: u16 = if minutes.len() == 2 {
        minutes.parse().ok()?
    } else {
        return None;
    };
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

/// Everything `setup` resolves and every operation reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Which tracker holds the issues.
    pub tracker: Tracker,
    /// Where publications and the repository host live. Defaults to GitHub
    /// matching [`Self::tracker`] when that tracker is GitHub.
    pub repository_host: RepositoryHost,
    /// What a verdict here has to be backed by, and so what a reviewer may do.
    pub evidence: Evidence,
    /// Whether accepted verdicts consult the publication lane.
    pub publication_lane: PublicationLane,
    /// What a run holding no claim may write in a checkout that has a tracker.
    pub issue_requirement: IssueRequirement,
    /// Who witnesses a claim in this repository.
    pub workflow: Workflow,
    /// `State names`: Estigia's states mapped onto the tracker's own.
    pub state_names: Vec<(String, String)>,
    /// `Self review` as written; [`Config::self_review`] is the value in force.
    pub self_review: Option<bool>,
    /// `Ready by` as written; [`Config::ready_by`] is the value in force.
    pub ready_by: Option<Party>,
    /// `External reviewers`, in the order the stages run.
    pub external_reviewers: Vec<ReviewStage>,
    /// `Corrections after ready` as written; [`Config::corrections_after_ready`]
    /// is the value in force.
    pub corrections_after_ready: Option<Corrections>,
    /// `Merge by` as written; [`Config::merge_by`] is the value in force.
    pub merge_by: Option<Party>,
    /// Whether OpenCode's optional loop plugin is written.
    pub loop_plugin: LoopPlugin,
    /// Whether OpenCode's generic `/goal` and `/loop` plugin is written.
    pub goal_loop_plugin: LoopPlugin,
    /// The per-context token figure a review is reported against. Advisory:
    /// nothing here refuses a panel that exceeds it.
    pub review_budget: ReviewBudget,
    /// The changed lines a pull request aims to stay under. Guidance the
    /// contract states and nothing enforces; a row rather than a sentence so
    /// an operator's answer survives the next `estigia sync`.
    pub change_size: u32,
    /// How many corrections one review lineage may spend. Enforced at
    /// publication, and what `escalate_review` waits on.
    pub corrections_per_lineage: u32,
    /// Where isolated checkouts are made.
    pub worktree: WorktreeRoot,
    /// The language the plain-language summary sentence is written in.
    pub summary_language: Language,
    /// The language the rest of an issue body is written in.
    pub body_language: Language,
    /// Who authorises delivery.
    pub delivery: Authority,
    /// Who authorises a workflow-state transition.
    pub transitions: Authority,
    /// Where work integrates: a reviewed branch, or trunk behind a flag.
    pub integration: Integration,
    /// How long a routine write may ride on the previous verification.
    ///
    /// **Only downwards.** A shorter window asks the tracker more often; a
    /// longer one lets a write ride on a more stale answer, which is loosening
    /// a guard rail — and a guard rail that can be loosened is a preference.
    pub window: std::time::Duration,
    /// The daily range commits, pushes, and branch creation may run in.
    /// Disabled by default, so installing this release changes nobody's gate.
    pub git_operation_window: GitOperationWindow,
    /// Which model each target runs on. A role named here is installed; empty
    /// means no role is, and the agent decides every model.
    pub models: ModelRouting,
    /// Commands this repository treats as irreversible, beyond the built-ins.
    /// Estigia cannot know that a repository ships with `npm publish` or
    /// `terraform apply`, and those must not ride the renewal window.
    /// Additive only: there is no way to remove a built-in boundary, because a
    /// setting that can make the gate looser is not a guard rail.
    pub boundaries: Vec<String>,
}

impl Config {
    /// When only `Tracker` is set, the host inherits its pin.
    /// A later `Repository host` row overrides; call this after applying Tracker.
    pub fn inherit_host_from_tracker_if_unset(&mut self) {
        if self.repository_host.is_default_github() {
            self.repository_host = RepositoryHost::from_tracker(&self.tracker);
        }
    }

    /// The posture this configuration reads as, or `None` when the two
    /// authorisations do not agree.
    ///
    /// `ask` here is the plain built-in timeout and nothing longer: it has to
    /// render back as the word `ask` the row offers, or `cautious` would answer
    /// for a value the operator cannot see. `ask 30m` is a mix, and reads
    /// `custom`.
    pub fn profile(&self) -> Option<Profile> {
        let plain_ask = |authority: Authority| matches!(authority, Authority::Ask { timeout } if timeout == DEFAULT_ASK_TIMEOUT);
        if self.delivery == Authority::Auto && self.transitions == Authority::Auto {
            return Some(Profile::Balanced);
        }
        if plain_ask(self.delivery) && plain_ask(self.transitions) {
            return Some(Profile::Cautious);
        }
        None
    }

    /// Writes every row `profile` means. The model half is not one of them:
    /// `Model routing` stays exactly as the operator left it.
    pub fn set_profile(&mut self, profile: Profile) {
        let authority = profile.authority();
        self.delivery = authority;
        self.transitions = authority;
    }
}

impl Default for Config {
    /// The portable defaults, and they are the balanced
    /// posture: `Delivery authorisation` and `Transition authorisation` `auto`. The model half is separate —
    /// `Model routing` stays unset, and `setup` renders the reviewed `balanced`
    /// preset into the definitions where the adapter has one.
    fn default() -> Self {
        Self {
            tracker: Tracker::Github { repo: None },
            repository_host: RepositoryHost::Github { repo: None },
            // Empty: no role is named, so no definition that carries `Write`,
            // `Edit` and `Bash` is installed by being upgraded. The balanced
            // posture renders the reviewed preset's *phases* into the phase
            // definitions; it never names a delegated role, because naming one
            // is what installs it.
            models: ModelRouting::default(),
            // A reviewed branch. Trunk-based trades the review for a flag, and
            // that trade has to be chosen rather than inherited.
            integration: Integration::Branch,
            // The number the contract has always carried in prose. Kept exactly,
            // so installing this release changes nobody's guidance — the row
            // exists to let an operator move it, not to move it for them.
            publication_lane: PublicationLane::On,
            issue_requirement: IssueRequirement::Ask,
            workflow: Workflow::Autonomous,
            state_names: Vec::new(),
            self_review: None,
            ready_by: None,
            external_reviewers: Vec::new(),
            corrections_after_ready: None,
            merge_by: None,
            // No loop plugin: it ships behind a flag and
            // stays behind the default.
            loop_plugin: LoopPlugin::Off,
            goal_loop_plugin: LoopPlugin::Off,
            // 150,000 tokens per context. `docs/honesty.md` says where the
            // figure comes from and what it does not count.
            review_budget: ReviewBudget::Tokens(150_000),
            change_size: 800,
            corrections_per_lineage: 3,
            window: crate::harness::RENEWAL_WINDOW,
            // The narrower answer, so no installation widens a delegated
            // context's capabilities by being upgraded.
            evidence: Evidence::Reading,
            worktree: WorktreeRoot::Auto,
            summary_language: Language("English".to_owned()),
            body_language: Language("English".to_owned()),
            // The balanced posture. An installation that carries
            // its own rows in the contract keeps them; one that is missing a
            // row takes the new value on the next `setup`.
            delivery: Authority::Auto,
            transitions: Authority::Auto,
            boundaries: Vec::new(),
            git_operation_window: GitOperationWindow::Disabled,
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
        match Self::read_reporting(versioned, local, false) {
            (config, None) => Ok(config),
            (_, Some(refusal)) => Err(refusal),
        }
    }

    /// The same read, keeping every row that parses.
    ///
    /// Only ever *narrower* than discarding everything: falling back to
    /// [`Config::default`] is the loosest configuration there is — no declared
    /// boundaries and the widest window — and every value a row can carry is a
    /// narrowing of it, because the setters refuse anything else.
    pub fn read_keeping_what_parses(versioned: &str, local: Option<&str>) -> Self {
        Self::read_reporting(versioned, local, true).0
    }

    /// The same tolerant read, with the row it could not take.
    pub(crate) fn read_keeping_what_parses_reporting(
        versioned: &str,
        local: Option<&str>,
    ) -> (Self, Option<Refusal>) {
        let (config, refused, folded, retired) = Self::read_channels(versioned, local, true);
        (config, refused.or(folded).or(retired))
    }

    /// A strict read that folds a retired `Delegated workers` row rather than
    /// refusing it, with every other unreadable row still a refusal.
    ///
    /// The fold is lossless — each name the row held becomes an `inherit` in the
    /// routing — so it is not the *present and unreadable* case a read-only
    /// surface refuses: it is a row whose meaning this build defines. Making
    /// that fold fatal refused `setup` over a contract it was the only command
    /// able to rewrite, and then refused every later run identically because it
    /// wrote nothing — the documented upgrade could not clear a state it was the
    /// only command expected to clear. A row a person typed and this build
    /// cannot read is still refused here, unchanged.
    pub(crate) fn read_folding_retired_workers(
        versioned: &str,
        local: Option<&str>,
    ) -> Result<(Self, Option<Refusal>), Refusal> {
        let (config, refused, folded, retired) = Self::read_channels(versioned, local, false);
        match refused {
            Some(refusal) => Err(refusal),
            None => Ok((config, folded.or(retired))),
        }
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
        let (config, settings, rejected) = Self::read_scope_reporting(base, document, scope, false);
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
        let (config, settings, _) = Self::read_scope_reporting(base, document, scope, true);
        (config, settings)
    }

    /// The scoped tolerant read, with the row it could not take.
    pub(crate) fn read_scope_over_keeping_what_parses_reporting(
        base: &Self,
        document: &str,
        scope: Scope,
    ) -> (Self, Vec<Setting>, Option<Refusal>) {
        let (config, settings, refused, folded, retired) =
            Self::read_scope_channels(base, document, scope, true);
        (config, settings, refused.or(folded).or(retired))
    }

    /// Lays the authorisation rows a repository's file may tighten over `config`.
    ///
    /// A row is honoured unless it is `auto` over a `base` that says `ask`; the
    /// row it cannot honour is left out of the returned settings and `config`
    /// keeps `base`'s answer. `base` is what the layers below this one read.
    pub(crate) fn tighten_over(
        base: &Self,
        config: &mut Self,
        document: &str,
        tolerant: bool,
    ) -> (Vec<Setting>, Option<Refusal>) {
        let mut owned: Vec<Setting> = Vec::new();
        let mut refused = None;
        for (label, value) in settings::rows(document) {
            let Some(setting) = Setting::from_label(&label) else {
                continue;
            };
            if !setting.repository_may_tighten() && setting.scope() != Scope::Repository {
                continue;
            }
            let mut probe = base.clone();
            if let Err(refusal) = setting.apply(&mut probe, &value) {
                if !tolerant {
                    refused.get_or_insert(refusal);
                }
                continue;
            }
            if setting.tightens(base, &probe) {
                let _ = setting.apply(config, &value);
                if !owned.contains(&setting) {
                    owned.push(setting);
                }
            } else {
                let _ = setting.apply(config, &setting.value_of(base));
                owned.retain(|held| held != &setting);
            }
        }
        (owned, refused)
    }

    /// The scoped read that folds a retired `Delegated workers` row rather than
    /// refusing it, every other unreadable owned row unchanged.
    pub(crate) fn read_scope_over_folding_retired_workers(
        base: &Self,
        document: &str,
        scope: Scope,
    ) -> Result<(Self, Vec<Setting>, Option<Refusal>), Refusal> {
        let (config, settings, refused, folded, retired) =
            Self::read_scope_channels(base, document, scope, false);
        match refused {
            Some(refusal) => Err(refusal),
            None => Ok((config, settings, folded.or(retired))),
        }
    }

    fn read_scope_reporting(
        base: &Self,
        document: &str,
        scope: Scope,
        tolerant: bool,
    ) -> (Self, Vec<Setting>, Option<Refusal>) {
        let (config, settings, refused, folded, _retired) =
            Self::read_scope_channels(base, document, scope, tolerant);
        (config, settings, refused.or(folded))
    }

    /// The scoped read with its three complaints kept apart: a row this build
    /// cannot parse, a retired row it folded, and a `Planning` value naming the
    /// retired SDD protocol.
    fn read_scope_channels(
        base: &Self,
        document: &str,
        scope: Scope,
        tolerant: bool,
    ) -> (
        Self,
        Vec<Setting>,
        Option<Refusal>,
        Option<Refusal>,
        Option<Refusal>,
    ) {
        let mut config = base.clone();
        let mut settings = Vec::new();
        let mut refused = None;
        let mut folded = None;
        let mut retired = None;
        let mut legacy = Vec::new();
        for (label, value) in settings::rows(document) {
            if Self::is_legacy_workers_label(&label) {
                if scope == Scope::Agent {
                    legacy.push(value);
                }
                continue;
            }
            if let Some(report) = settings::retired_setting_report(&label, &value) {
                retired.get_or_insert(report);
                continue;
            }
            if settings::is_retired_planning_label(&label) {
                if scope == Scope::Agent && settings::planning_is_retired(&value) {
                    retired.get_or_insert_with(|| settings::retired_planning_report(&value));
                }
                continue;
            }
            let Some(setting) = Setting::from_label(&label) else {
                continue;
            };
            if setting.scope() != scope {
                continue;
            }
            if !settings.contains(&setting) {
                settings.push(setting);
            }
            if let Some(refusal) = Self::apply_row(&mut config, setting, &value, tolerant) {
                refused.get_or_insert(refusal);
            }
        }
        // After every routing row, so a `Model routing` row written below the
        // retired one cannot wipe a fold it never saw; a role the routing
        // already names is left to it.
        for value in legacy {
            if let Some(refusal) = Self::apply_legacy_workers(&mut config, &value) {
                folded.get_or_insert(refusal);
            }
        }
        (config, settings, refused, folded, retired)
    }

    /// Applies one row, keeping what parses when the caller allows it.
    ///
    /// `Model routing` is the one row a tolerant read applies partly: a key
    /// this build no longer takes is dropped on its own rather than costing the
    /// whole cell, and the refusal returned names it. Every other row is
    /// all-or-nothing, as the strict read has always been.
    fn apply_row(
        config: &mut Self,
        setting: Setting,
        value: &str,
        tolerant: bool,
    ) -> Option<Refusal> {
        if tolerant && setting == Setting::Models {
            let (routing, dropped) = ModelRouting::parse_keeping_what_parses(value);
            config.models = routing;
            return (!dropped.is_empty()).then(|| dropped_routes_report(&dropped));
        }
        setting.apply(config, value).err()
    }

    fn is_legacy_workers_label(label: &str) -> bool {
        settings::normalize_label(label) == settings::normalize_label(LEGACY_WORKERS_LABEL)
    }

    /// Folds a retired `Delegated workers` row into the routing, and says so.
    ///
    /// `developer analyst` was two booleans saying which definitions exist; a
    /// role's presence in the routing says that now, so each name becomes
    /// `inherit` unless the routing already names it. The row is what its
    /// absence leaves behind, and it is never dropped without this report.
    fn apply_legacy_workers(config: &mut Self, value: &str) -> Option<Refusal> {
        let Some(named) = legacy_workers_named(value) else {
            return Some(legacy_workers_report(value, &[]));
        };
        let mut migrated = Vec::new();
        for name in named {
            if !config.models.installs(name) {
                config.models.assign(name, "inherit");
                migrated.push(name);
            }
        }
        (!migrated.is_empty()).then(|| legacy_workers_report(value, &migrated))
    }

    /// Both answers at once, so neither is a second copy of the other.
    ///
    /// The retired-protocol report is deliberately left out: this is the read a
    /// strict caller uses, and a `Planning` value naming the retired SDD
    /// protocol loads as `direct` rather than stopping the table. The reporting
    /// reads carry the report.
    fn read_reporting(
        versioned: &str,
        local: Option<&str>,
        tolerant: bool,
    ) -> (Self, Option<Refusal>) {
        let (config, refused, folded, _retired) = Self::read_channels(versioned, local, tolerant);
        (config, refused.or(folded))
    }

    /// The read with its three complaints kept apart: a row this build cannot
    /// parse, a retired row it folded, and a `Planning` value naming the
    /// retired SDD protocol.
    ///
    /// The three are returned side by side, and the reporting readers merge
    /// them with the row complaint first. A reader that must not confuse them
    /// takes the channels apart, as [`Self::read_folding_retired_workers`] does;
    /// one that only reports gets the row an operator is sent to fix.
    fn read_channels(
        versioned: &str,
        local: Option<&str>,
        tolerant: bool,
    ) -> (Self, Option<Refusal>, Option<Refusal>, Option<Refusal>) {
        let mut config = Self::default();
        let mut refused: Option<Refusal> = None;
        let mut folded: Option<Refusal> = None;
        let mut retired: Option<Refusal> = None;
        let mut legacy = Vec::new();
        for document in [Some(versioned), local].into_iter().flatten() {
            for (label, value) in settings::rows(document) {
                if Self::is_legacy_workers_label(&label) {
                    legacy.push(value);
                    continue;
                }
                if let Some(report) = settings::retired_setting_report(&label, &value) {
                    retired.get_or_insert(report);
                    continue;
                }
                if settings::is_retired_planning_label(&label) {
                    if settings::planning_is_retired(&value) {
                        retired.get_or_insert_with(|| settings::retired_planning_report(&value));
                    }
                    continue;
                }
                let Some(setting) = Setting::from_label(&label) else {
                    // An unknown row is the operator's own note, not a defect.
                    // Their file is theirs; Estigia only claims the labels it
                    // published.
                    continue;
                };
                if setting.scope() == Scope::Repository {
                    continue;
                }
                if let Some(refusal) = Self::apply_row(&mut config, setting, &value, tolerant) {
                    // The first, because that is the one `read` used to return
                    // and the one an operator is sent to fix.
                    refused.get_or_insert(refusal);
                }
            }
        }
        // After every routing row, in either document, so a `Model routing` row
        // written below a retired one cannot wipe a fold it never saw.
        for value in legacy {
            if let Some(refusal) = Self::apply_legacy_workers(&mut config, &value) {
                folded.get_or_insert(refusal);
            }
        }
        (config, refused, folded, retired)
    }

    /// Renders the table body — the rows between the markers, without them.
    pub fn render_rows(&self) -> String {
        settings::render_rows(self)
    }

    /// The same, for an adapter's own file: only the rows that differ by agent.
    pub fn render_agent_rows(&self) -> String {
        settings::render_agent_rows(self)
    }

    /// The rows this repository keeps about itself.
    pub fn render_repository_rows(&self) -> String {
        settings::render_repository_rows(self)
    }

    /// The repository rows this file speaks for, and no others.
    pub fn render_some_repository_rows(&self, speaks_for: &[Setting]) -> String {
        settings::render_some_repository_rows(self, speaks_for)
    }

    /// The agent rows this file speaks for, and no others.
    pub fn render_some_agent_rows(&self, speaks_for: &[Setting]) -> String {
        settings::render_some_agent_rows(self, speaks_for)
    }
}

#[cfg(test)]
mod tests;

/// The roles a run can route a model to.
///
/// Six: the two Estigia installs a definition for — the run that writes and
/// runs the suite, and the read-only analyst — and the four the review panel is
/// made of, a reviewer when review is delegated, the refuter and the validator
/// that fall back to it, and the judge when Judgment Day is asked for. Present
/// in the routing is what installs the first two; the rest have no definition
/// to install, and [`Self::installs`] is the word that separates them rather
/// than a second list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// The worker that writes and runs the suite.
    Developer,
    /// The worker that is repository-read-only.
    Analyst,
    /// The reviewer, when review is delegated.
    Reviewer,
    /// The refuter, which launches on the reviewer's model when it has none.
    Refuter,
    /// The validator, which launches on the reviewer's model when it has none.
    Validator,
    /// A blind judge, when judges are asked.
    Judge,
}

impl Role {
    /// Its spelling in the table.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Developer => "developer",
            Self::Analyst => "analyst",
            Self::Reviewer => "reviewer",
            Self::Refuter => "refuter",
            Self::Validator => "validator",
            Self::Judge => "judge",
        }
    }

    /// Every role, under an exhaustive match, so a new one cannot be
    /// added without arriving here. Arriving is not being returned — widening
    /// the arm and leaving the list short still compiles.
    pub fn all() -> Vec<Self> {
        let every = vec![
            Self::Developer,
            Self::Analyst,
            Self::Reviewer,
            Self::Refuter,
            Self::Validator,
            Self::Judge,
        ];
        for role in &every {
            match role {
                Self::Developer
                | Self::Analyst
                | Self::Reviewer
                | Self::Refuter
                | Self::Validator
                | Self::Judge => {}
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

    /// Whether Estigia installs a definition for this role.
    ///
    /// Naming the role in the routing is what installs it, so this is the
    /// predicate the install and retract pass reads. The reviewer and the
    /// judge are roles a review's contexts are launched under, and nothing
    /// Estigia writes is a definition of them, so a routing key for one is a
    /// declaration and never a file.
    pub fn installs(self) -> bool {
        matches!(self, Self::Developer | Self::Analyst)
    }
}

/// The label of the row whose job the roles section took over.
///
/// `Delegated workers` named which workers this installation had definitions
/// for. A role's presence in the routing names it now, so the row is gone — but
/// a table that still carries it is not ignored: the read folds each name it
/// holds into the routing as `inherit` and says so, because a definition
/// disappearing without a word is exactly the migration this refuses.
pub(crate) const LEGACY_WORKERS_LABEL: &str = "Delegated workers";

/// The workers a legacy `Delegated workers` value named.
///
/// `none`, one name, or both in either order, the same word set the row always
/// took. `None` for a value the row never accepted, so the caller reports it
/// rather than guessing at it.
pub(crate) fn legacy_workers_named(value: &str) -> Option<Vec<&'static str>> {
    let value = value.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("off")
        || value.eq_ignore_ascii_case("unset")
    {
        return Some(Vec::new());
    }
    let mut named = Vec::new();
    for word in value.split_whitespace() {
        let role = Role::parse(word)?;
        if !role.installs() {
            return None;
        }
        if !named.contains(&role.as_str()) {
            named.push(role.as_str());
        }
    }
    Some(named)
}

/// How hard a delegated context is asked to think.
///
/// A closed set, and it is the host's closed set rather than one invented here:
/// these are the words a Claude Code sub-agent definition's `effort:` field
/// takes. Deliberately not a number — an operator choosing between `low` and
/// `high` is choosing what the work is worth, not what it costs.
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

/// One target's model, and the effort it is asked to think at.
///
/// Two decisions, separately settable and separately shown: a target may name
/// a model and no effort, an effort and no model, both, or neither. A route
/// with no model is one the parent's model is inherited by; a route with no
/// effort renders no `effort:` line, because an absent field is the host's own
/// answer and writing that answer down would freeze today's default into a
/// file. `model` is `None` for a target that names an effort and no model —
/// which is itself a way of naming the target — and a route that is entirely
/// empty is no route at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Route {
    /// The opaque model ID, never checked against a catalog.
    pub model: Option<String>,
    /// The effort, when one was named.
    pub effort: Option<Effort>,
}

impl Route {
    /// Reads one model value — a model ID, `inherit`, or nothing.
    ///
    /// Never an effort: the effort is its own list and its own parser, which is
    /// what retires the slash that used to make one value carry two decisions.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if retired_effort_suffix(value).is_some() {
            return None;
        }
        ModelRouting::accepts_model_id(value).then(|| Self {
            model: Some(value.to_owned()),
            effort: None,
        })
    }

    /// The model value as it is written in the model list, or nothing.
    pub fn model_value(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Whether this route names nothing at all.
    pub fn is_empty(&self) -> bool {
        self.model.is_none() && self.effort.is_none()
    }

    /// How the whole route reads in a display, effort included.
    ///
    /// Display only: the persisted cell keeps the model and the effort in two
    /// separate lists, and this is the one line the screen shows for a row.
    pub fn as_value(&self) -> String {
        match (&self.model, self.effort) {
            (Some(model), Some(effort)) => format!("{model} \u{b7} {}", effort.as_str()),
            (Some(model), None) => model.clone(),
            (None, Some(effort)) => format!("inherit \u{b7} {}", effort.as_str()),
            (None, None) => "inherit".to_owned(),
        }
    }
}

impl std::fmt::Display for Route {
    /// One spelling, so what is written, what is read back and what is shown in
    /// a message cannot drift apart.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.as_value())
    }
}

/// The states a newly filed issue may be created in.
///
/// Narrower than [`STATES`], and the narrowing is the transport's: `create`
/// carries `choices=["ready", "blocked"]`, because an issue that has just been
/// written is either work somebody may pick up or work waiting on a named
/// condition. The test
/// `every_value_the_transport_constrains_is_one_the_schema_publishes` reads
/// this parser and fails if this list stops being the parser's.
pub const CREATED_STATES: &[&str] = &["ready", "blocked"];

/// The kinds of comment the transport will file: `comment` carries
/// `choices=["blocker", "diagnosis", "note"]`, the same schema publishing this
/// parser's list as [`CREATED_STATES`].
pub const COMMENT_KINDS: &[&str] = &["blocker", "diagnosis", "note"];

/// The workflow states a run can be in.
///
/// Taken from the binding — *"the workflow's states are `analysis`, `ready`,
/// `in-progress`, `review`, `blocked` and `done`"* — and crossed against that
/// sentence by a test: a renamed state would make a transition match nothing,
/// and this list is what the transport labels an issue with.
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

/// The row's third value, beside `unset` and the `key=model` pairs.
///
/// Not `auto`: that word already means *permission* in the three authorisation
/// rows, and a second meaning in this table is one an operator would read wrong.
pub const BY_TASK: &str = "by task";

/// The two sections one row's targets are presented in.
///
/// They answer different questions — *what kind of thinking is happening* and
/// *which context is doing it* — and neither lists the other's targets. A
/// role's presence in the routing is what installs its definition; nothing in
/// the phases section installs anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSection {
    /// Orchestration and the universal `apply`.
    Phases,
    /// The roles a run can hand work to.
    Roles,
}

/// Why one model target is visible in an interactive configuration surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTargetKind {
    /// The model reading the run and deciding what to delegate.
    Orchestration,
    /// The universal phase that writes the change.
    Application,
    /// A role a run can hand work to.
    DelegatedAgent,
}

impl ModelTargetKind {
    /// Which section this target is shown in.
    pub fn section(self) -> ModelSection {
        match self {
            Self::Orchestration | Self::Application => ModelSection::Phases,
            Self::DelegatedAgent => ModelSection::Roles,
        }
    }
}

/// One model-routing target an operator can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelTarget {
    /// The canonical key persisted in [`ModelRouting`].
    pub name: &'static str,
    /// The reason this target is shown.
    pub kind: ModelTargetKind,
}

/// One entry a read of the routing could not take, and what it became.
///
/// A key a build no longer accepts is dropped rather than rewritten to
/// `unset`, and this is the report that says so: `replacement` names the key
/// that took its place, where one did — `implementer` is `developer` now — and
/// is `None` for a name that was only ever somebody else's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedRoute {
    /// The key as it was written.
    pub key: String,
    /// The key that replaced it, when a rename is what removed it.
    pub replacement: Option<&'static str>,
}

/// The key `implementer` became when the worker took the role's name.
fn replacement_for_retired_key(key: &str) -> Option<&'static str> {
    key.trim()
        .eq_ignore_ascii_case("implementer")
        .then_some("developer")
}

/// A value still wearing the retired `model/effort` suffix, if it is.
///
/// The two decisions are separate lists now. A model ID whose last `/` segment
/// is an effort word is the old spelling, refused rather than read as an opaque
/// ID, so a stored `apply=sonnet/low` is reported instead of silently becoming
/// a model named `sonnet/low`. A provider-qualified ID such as
/// `anthropic/claude-opus-4` ends in none of the five words and is untouched.
fn retired_effort_suffix(value: &str) -> Option<&str> {
    let (_, tail) = value.rsplit_once('/')?;
    Effort::parse(tail).map(|_| tail)
}

/// What a read did with entries this build no longer accepts.
///
/// A refusal so a report and a strict failure are one sentence: the tolerant
/// read keeps the cell and hands this back, the strict read fails on it, and
/// the writer prints it. Never "rewritten to `unset` in silence", which is the
/// shape this replaced.
pub(crate) fn dropped_routes_report(dropped: &[DroppedRoute]) -> Refusal {
    let named = dropped
        .iter()
        .map(|entry| match entry.replacement {
            Some(replacement) => format!("`{}` (now `{replacement}`)", entry.key),
            None => format!("`{}`", entry.key),
        })
        .collect::<Vec<_>>()
        .join(", ");
    Refusal::not_started(
        "model-routing-key-dropped",
        format!(
            "`Model routing` no longer accepts {named}; it takes {}",
            ModelRouting::targets()
                .iter()
                .map(|key| format!("`{key}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Resolution::run(
            "estigia config set \"Model routing\" \"<the entries you keep, in the current spelling>\" --agent <slug>",
        ),
    )
}

/// What a read did with a retired `Delegated workers` row.
fn legacy_workers_report(value: &str, migrated: &[&str]) -> Refusal {
    let message = if migrated.is_empty() {
        format!(
            "`{LEGACY_WORKERS_LABEL}` is {value:?}, which is not a value it ever took; the row is \
             gone and a role's presence in `Model routing` installs it now"
        )
    } else {
        format!(
            "`{LEGACY_WORKERS_LABEL}` is {value:?}; {} now names its definitions, so the row is \
             gone",
            migrated
                .iter()
                .map(|name| format!("`{name}` in `Model routing`"))
                .collect::<Vec<_>>()
                .join(" and ")
        )
    };
    Refusal::not_started(
        "delegated-workers-row-retired",
        message,
        Resolution::run(
            "estigia config set \"Model routing\" \"<the entries you keep>\" --agent <slug>",
        ),
    )
}

/// Which model each target should run on, and at what effort.
///
/// Estigia does not run the model that does the work — it holds the tools. The one launch it makes
/// itself is a review's: `launch_review_input` starts a reviewer CLI on the model the `reviewer`
/// key names. Where a host reads
/// sub-agent definitions, `setup` writes the named model and effort into the
/// roles it installs; a role's presence here is what installs it, and naming a
/// role that is not installed reaches nothing. Every other key except `reviewer` is a
/// declaration the agent reads.
///
/// Model and effort are two decisions and so two lists in the one cell: models
/// first, then, after a `;`, the efforts. A target may appear in either or
/// both. Empty by default.
///
/// A third value, [`BY_TASK`], says the shape of the work decides the model and
/// the effort and the launcher records what it chose, rather than a role's name
/// doing so. It names no route and is mutually exclusive with the pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelRouting {
    /// Target to route; rendered through the canonical target order.
    pub by_role: std::collections::BTreeMap<String, Route>,
    /// Whether the shape of the work decides the model and the effort, rather
    /// than a role's name. `by task` is the row's third value; it names no
    /// route, and a cell that sets it and names pairs beside it is refused.
    pub by_task: bool,
}

impl ModelRouting {
    /// Model targets an interactive surface offers.
    ///
    /// Orchestration and `apply` are universal, and the roles are the four a
    /// run can hand work to. The retired planning phases are absent: nothing
    /// installs a definition for one any more, and offering an inert route as
    /// an ordinary interactive choice would imply that the current run uses it.
    pub fn visible_targets() -> Vec<ModelTarget> {
        std::iter::once(ModelTarget {
            name: ORCHESTRATE_TARGET,
            kind: ModelTargetKind::Orchestration,
        })
        .chain(std::iter::once(ModelTarget {
            name: APPLY_TARGET,
            kind: ModelTargetKind::Application,
        }))
        .chain(Role::all().into_iter().map(|role| ModelTarget {
            name: role.as_str(),
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
            .chain(std::iter::once(APPLY_TARGET))
            .collect()
    }

    /// The keys in the fixed order the cell is written in.
    fn canonical_order() -> Vec<&'static str> {
        Role::all()
            .into_iter()
            .map(Role::as_str)
            .chain([APPLY_TARGET, ORCHESTRATE_TARGET])
            .collect()
    }

    /// The whole route assigned to any canonical target.
    pub fn route(&self, target: &str) -> Option<&Route> {
        self.by_role.get(Self::canonical_target(target)?)
    }

    /// Whether a role is named here — which is what installs its definition.
    pub fn installs(&self, target: &str) -> bool {
        self.route(target).is_some()
    }

    /// The model assigned to any canonical routing target.
    pub fn for_target(&self, target: &str) -> Option<&str> {
        self.route(target).and_then(Route::model_value)
    }

    /// The effort named for a target, when one was.
    pub fn effort_for(&self, target: &str) -> Option<Effort> {
        self.route(target).and_then(|route| route.effort)
    }

    /// Assigns one model — `opus`, `inherit`, or another opaque ID — to a target.
    ///
    /// The effort already named for the target is left alone: the two are
    /// separate decisions, so setting a model must not clear an effort and
    /// setting an effort must not clear a model.
    pub fn assign(&mut self, target: &str, model: &str) -> bool {
        let Some(target) = Self::canonical_target(target) else {
            return false;
        };
        let Some(route) = Route::parse(model) else {
            return false;
        };
        self.by_role.entry(target.to_owned()).or_default().model = route.model;
        self.by_task = false;
        true
    }

    /// Names, or clears, the effort for one target.
    ///
    /// An effort is its own decision and lands in its own list; naming one for
    /// a target that has no model is naming the target, so it is a route like
    /// any other and installs the role it names.
    pub fn assign_effort(&mut self, target: &str, effort: Option<Effort>) -> bool {
        let Some(target) = Self::canonical_target(target) else {
            return false;
        };
        let entry = self.by_role.entry(target.to_owned()).or_default();
        match effort {
            Some(effort) => entry.effort = Some(effort),
            None => entry.effort = None,
        }
        if entry.is_empty() {
            self.by_role.remove(target);
        }
        self.by_task = false;
        true
    }

    /// Whether one opaque model ID fits inside one persisted `key=model` entry.
    ///
    /// Catalog membership is deliberately irrelevant. These delimiters are
    /// structural: comma starts another assignment, semicolon starts the effort
    /// list, pipe ends the markdown cell, and either line break ends the table
    /// row.
    pub fn accepts_model_id(model: &str) -> bool {
        let model = model.trim();
        !model.is_empty() && !model.contains([',', ';', '|', '\r', '\n'])
    }

    /// Removes one target's whole route without manufacturing `target=unset`.
    pub fn remove(&mut self, target: &str) -> bool {
        Self::canonical_target(target)
            .and_then(|target| self.by_role.remove(target))
            .is_some()
    }

    /// Removes every assignment.
    pub fn clear(&mut self) {
        self.by_role.clear();
        self.by_task = false;
    }

    fn canonical_target(target: &str) -> Option<&'static str> {
        let target = target.trim();
        Self::targets()
            .into_iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(target))
    }

    /// The cell this writes into the table.
    pub fn as_value(&self) -> String {
        if self.by_task {
            return BY_TASK.to_owned();
        }
        if self.by_role.is_empty() {
            return "unset".to_owned();
        }
        // Fixed order, not the map's: two writes of the same routing produce
        // the same cell, so a diff means something changed.
        let models = Self::canonical_order()
            .into_iter()
            .filter_map(|key| {
                self.by_role
                    .get(key)
                    .and_then(Route::model_value)
                    .map(|model| format!("{key}={model}"))
            })
            .collect::<Vec<_>>()
            .join(", ");
        let efforts = Self::canonical_order()
            .into_iter()
            .filter_map(|key| {
                self.by_role
                    .get(key)
                    .and_then(|route| route.effort)
                    .map(|effort| format!("{key}={}", effort.as_str()))
            })
            .collect::<Vec<_>>()
            .join(", ");
        if efforts.is_empty() {
            return models;
        }
        format!("{models}; {efforts}")
    }

    /// Reads the whole cell — `developer=opus, apply=sonnet; apply=low`.
    ///
    /// A key or model this build does not accept is refused rather than
    /// skipped. Silently dropping it would leave a role running on whatever
    /// the agent picks while the table says otherwise, which is the one thing a
    /// configuration must never do. [`Self::parse_keeping_what_parses`] is the
    /// same read for a writer, which keeps every entry it can and names the
    /// ones it had to drop.
    pub fn parse(value: &str) -> Option<Self> {
        let (routing, dropped) = Self::parse_keeping_what_parses(value);
        dropped.is_empty().then_some(routing)
    }

    /// The same read, keeping every entry that parses and naming the rest.
    ///
    /// One entry that is wrong — a retired key, a model with a delimiter in it,
    /// a target named twice — drops that entry alone. The cell is not lost
    /// whole and is never rewritten to `unset` in silence: the caller reports
    /// what `dropped` names.
    pub fn parse_keeping_what_parses(value: &str) -> (Self, Vec<DroppedRoute>) {
        let value = value.trim();
        if value.is_empty()
            || value.eq_ignore_ascii_case("unset")
            || value.eq_ignore_ascii_case("none")
        {
            return (Self::default(), Vec::new());
        }
        if value.eq_ignore_ascii_case(BY_TASK) {
            return (
                Self {
                    by_task: true,
                    ..Self::default()
                },
                Vec::new(),
            );
        }
        let (models, efforts) = match value.split_once(';') {
            Some((models, efforts)) => (models, Some(efforts)),
            None => (value, None),
        };
        let mut routing = Self::default();
        let mut dropped = Vec::new();
        for piece in models.split(',') {
            let Some((key, model)) = piece.split_once('=') else {
                if !piece.trim().is_empty() {
                    dropped.push(Self::dropped(piece));
                }
                continue;
            };
            let Some(target) = Self::canonical_target(key) else {
                dropped.push(Self::dropped(key));
                continue;
            };
            let Some(route) = Route::parse(model) else {
                dropped.push(Self::dropped(key));
                continue;
            };
            if routing.by_role.contains_key(target) {
                dropped.push(Self::dropped(key));
                continue;
            }
            routing.by_role.insert(target.to_owned(), route);
        }
        if let Some(efforts) = efforts {
            for piece in efforts.split(',') {
                let Some((key, word)) = piece.split_once('=') else {
                    if !piece.trim().is_empty() {
                        dropped.push(Self::dropped(piece));
                    }
                    continue;
                };
                let Some(target) = Self::canonical_target(key) else {
                    dropped.push(Self::dropped(key));
                    continue;
                };
                let Some(effort) = Effort::parse(word) else {
                    dropped.push(Self::dropped(key));
                    continue;
                };
                if routing
                    .by_role
                    .get(target)
                    .is_some_and(|route| route.effort.is_some())
                {
                    dropped.push(Self::dropped(key));
                    continue;
                }
                routing.by_role.entry(target.to_owned()).or_default().effort = Some(effort);
            }
        }
        (routing, dropped)
    }

    fn dropped(key: &str) -> DroppedRoute {
        let key = key.trim().to_owned();
        let replacement = replacement_for_retired_key(&key);
        DroppedRoute { key, replacement }
    }
}

/// Where work integrates, and what that costs at the gate.
///
/// Under [`Integration::Branch`] a delivery lands only from a state where a
/// verdict exists — `review` or `done` — because on a branch the review *is*
/// the protection. Trunk-based replaces it on purpose: a delivery from
/// `in-progress` is allowed **only when a flag is named**, the same shape as a
/// forced reclaim — the loosening is declarable and the declaration is
/// answerable for. Naming a flag does not prove the change is behind it; it
/// makes the claim explicit and recorded.
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
    pub fn admits_unreviewed(self, flag: Option<&str>) -> bool {
        matches!(self, Self::Trunk) && flag.is_some_and(|name| !name.trim().is_empty())
    }
}
