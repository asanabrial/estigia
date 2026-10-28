//! `review-tier` — the risk tier one change derives, from evidence alone.
//!
//! A change is classified by **what it touches**, so a five-thousand-line
//! documentation change and a five-line authorization edit are not separated
//! by their line counts. What the tier reads is a boundary rather than a size,
//! and three boundaries participate, each stated where it applies rather than
//! as an exception here: the changed-path count at zero, the summed candidate
//! blob bytes against `SCAN_BYTE_BUDGET`, and the per-path authored line count
//! at zero.
//!
//! The `0`/`1`/`4` **lens count** is [`RiskLevel::lenses`]: `low` answers `0`
//! lens questions, `medium` answers `1`, chosen by a focus, and `high` answers
//! four. Which role fills each slot is [`RiskLevel::lens_roles`]. Ordinary
//! review licenses lenses only; judges exist solely as the opt-in Judgment
//! Day, which this binding allocates nothing for. Everything else — the
//! closed signal set, the hot-path tokens, the passive-extension and path
//! exclusions, the byte-bounded content proof, the configuration exclusion,
//! and the fail-closed cross-check between a tier and the reasons that
//! explain it — is read from a tree-to-tree diff.
//!
//! Three more producers are ported below: `isServiceTokenReviewPath`
//! (adjacent `service`/`token` path tokens), `isShellReviewPath` (`.sh`,
//! `.bash`, `.zsh`, or a `.yml`/`.yaml` under `.github/workflows/`), and
//! `executableModeChanged` (an execute bit flip between the two modes). The
//! first two are gated the way the source project gates them,
//! `isSemanticRiskEligible` (`risk.go:805-819`): inspectable (nonzero authored
//! lines, not binary, not mode-only), no `docs`/`testdata`/`fixture`/
//! `fixtures`/`__fixtures__` path segment, no `readme`-prefixed basename, and
//! either a semantic source extension or a configuration path. The mode-flip
//! producer carries no such gate in the source project and none is added
//! here — it fires on any path, which is what lets a chmod inside `docs/`
//! still read as evidence.
//!
//! The content scan (`processBoundaryRiskReasons` and the dangerous-sink
//! catalogue) lives in `content_scan`: a process-spawn construct, an
//! interpreter directive on a file's first line, or one of ten
//! dangerous-sink families in the lines a change adds reads `high`.
//!
//! Not ported: `isNativeReviewAuthorityPath`, their own six-path
//! self-reference. It is not a narrower port — it is not ported at all,
//! measured rather than assumed: substituting this repository's own gate
//! files answered `high` on every one of three real deliveries measured
//! against it, all through `src/transport/claim.rs`, a 7,000-plus-line
//! module touched in most of the repository's own recent history, which
//! made the classifier a constant function. This repository's review
//! authority is not separable by filename the way the source project's is;
//! The gap is named in
//! `docs/honesty.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::harness::population::{Disposition, Population};

use super::{
    Context, Failure, How, Output, pinned_diff_args, run, run_with_env, run_with_isolated_env,
};

/// A byte budget on every blob this classifier reads to prove a candidate
/// passive, so neither passive-content proof nor its absence can be turned
/// into an unbounded repository read. Matches the source project's own
/// `processBoundaryScanByteLimit` (`risk.go:25`).
const SCAN_BYTE_BUDGET: u64 = 8 << 20;

const REASON_HOT_PATH: &str = "hot_path";
const REASON_SERVICE_TOKEN: &str = "service_token";
const REASON_SHELL_SOURCE: &str = "shell_source";
const REASON_EXECUTABLE_MODE: &str = "executable_mode";
const REASON_NON_EXECUTABLE_ONLY: &str = "non_executable_only";
const REASON_CONFIGURATION_CHANGE: &str = "configuration_change";
const REASON_EXECUTABLE_CHANGE: &str = "executable_change";
const REASON_EMPTY_CONTENT: &str = "empty_content";
/// A range with no changed paths at all — `base == head`, or any pair whose
/// diff touched nothing. Distinct from [`REASON_NON_EXECUTABLE_ONLY`], which
/// names a change that *was* measured and found to touch nothing but
/// non-executable, passive-shaped paths: that reading is vacuously true over
/// an empty stat set too, and conflating the two let `classify_risk` and
/// [`risk_level_from_reasons`] agree on `Low` for a range nothing was ever
/// read from.
const REASON_NOTHING_MEASURED: &str = "nothing_measured";

/// The extensions `isSemanticRiskEligible` and `stripSemanticPathExtension`
/// treat as source, ported unchanged from `semanticSourceExtensions`
/// (`risk.go:27-31`).
const SEMANTIC_SOURCE_EXTENSIONS: &[&str] = &[
    ".c", ".cc", ".cpp", ".cs", ".go", ".h", ".hpp", ".java", ".js", ".jsx", ".kt", ".kts", ".php",
    ".py", ".rb", ".rs", ".sh", ".bash", ".zsh", ".ts", ".tsx",
];

/// Ported unchanged from `hasExcludedRiskSegment`'s switch (`risk.go:845-852`).
const EXCLUDED_RISK_SEGMENTS: &[&str] =
    &["docs", "testdata", "fixture", "fixtures", "__fixtures__"];

/// The four roles `High` licenses: the source project's risk, resilience,
/// readability and reliability, with resilience filled by `review-readback`.
pub(crate) const HIGH_LENS_ROLES: [&str; 4] = [
    crate::harness::roles::RESERVED_RISK,
    crate::harness::roles::RESERVED_READBACK,
    crate::harness::roles::RESERVED_READABILITY,
    crate::harness::roles::RESERVED_RELIABILITY,
];

/// The lens a `Medium` change is reviewed through, the source project's
/// `--focus`: `reliability` unless the caller names another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReviewFocus {
    /// `review-risk`.
    Risk,
    /// `review-readback`, which fills the source project's resilience slot.
    Resilience,
    /// `review-readability`.
    Readability,
    /// `review-reliability`.
    #[default]
    Reliability,
}

/// The spellings [`ReviewFocus::parse`] accepts, in the order the source
/// project lists them.
pub const REVIEW_FOCUSES: &[&str] = &["risk", "resilience", "readability", "reliability"];

impl ReviewFocus {
    /// Reads a caller's `focus` argument; an absent one is the default and an
    /// unknown one is a read failure naming the four, never a silent default.
    pub fn parse(focus: Option<&str>) -> Result<Self, Failure> {
        match focus.map(str::trim) {
            None => Ok(Self::default()),
            Some("risk") => Ok(Self::Risk),
            Some("resilience") => Ok(Self::Resilience),
            Some("readability") => Ok(Self::Readability),
            Some("reliability") => Ok(Self::Reliability),
            Some(other) => Err(Failure::Read(format!(
                "focus must be one of {REVIEW_FOCUSES:?}, not {other:?}"
            ))),
        }
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Risk => "risk",
            Self::Resilience => "resilience",
            Self::Readability => "readability",
            Self::Reliability => "reliability",
        }
    }

    fn role(self) -> &'static str {
        match self {
            Self::Risk => crate::harness::roles::RESERVED_RISK,
            Self::Resilience => crate::harness::roles::RESERVED_READBACK,
            Self::Readability => crate::harness::roles::RESERVED_READABILITY,
            Self::Reliability => crate::harness::roles::RESERVED_RELIABILITY,
        }
    }
}

/// The risk items a caller may declare to raise a tier, each naming a
/// consequence the diff signals cannot see. Any one raises the tier to `high`.
pub const RISK_ITEMS: &[&str] = &[
    "authorization",
    "migration",
    "input",
    "concurrency",
    "irreversibility",
    "contract",
];

/// What a caller declares beside a tier request. A `raise` can only move the
/// computed tier up; the reasons are free text the answer carries back.
#[derive(Debug, Clone, Copy, Default)]
pub struct Declaration<'a> {
    /// One of [`RISK_ITEMS`].
    pub raise: Option<&'a str>,
    /// Why the item applies; required with `raise`.
    pub raise_reason: Option<&'a str>,
    /// Why the `focus` was chosen; requires a `focus`.
    pub focus_reason: Option<&'a str>,
}

pub(crate) struct Resolved {
    focus: ReviewFocus,
    focus_named: bool,
    raise: Option<(&'static str, String)>,
    focus_reason: Option<String>,
}

impl Resolved {
    pub(crate) fn marker_fields(&self, computed: Option<RiskLevel>) -> Vec<(&'static str, String)> {
        let mut fields = Vec::new();
        if let (Some((item, reason)), Some(computed)) = (&self.raise, computed) {
            fields.push(("computed-level", computed.as_str().to_owned()));
            fields.push(("raise", (*item).to_owned()));
            fields.push(("raise-reason", reason.clone()));
        }
        if self.focus_named {
            fields.push(("focus", self.focus.word().to_owned()));
        }
        if let Some(reason) = &self.focus_reason {
            fields.push(("focus-reason", reason.clone()));
        }
        fields
    }

    pub(crate) fn raises(&self) -> bool {
        self.raise.is_some()
    }
}

fn reason_of(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

impl Declaration<'_> {
    pub(crate) fn resolve(&self, focus: Option<&str>) -> Result<Resolved, Failure> {
        let raise = match self.raise.map(str::trim) {
            None => {
                if self.raise_reason.is_some() {
                    return Err(Failure::ConfigDefect(
                        serde_json::json!({"ok": false, "reason": "risk-raise-reason-without-raise", "action": "name the risk item the reason explains with `raise`, or omit `raise_reason`", "expected": RISK_ITEMS}),
                    ));
                }
                None
            }
            Some(word) => {
                if ["low", "medium", "none"].contains(&word) {
                    return Err(Failure::ConfigDefect(
                        serde_json::json!({"ok": false, "reason": "risk-raise-lowers", "action": "a raise only moves a tier up; omit it to keep the computed tier", "value": word}),
                    ));
                }
                let Some(item) = RISK_ITEMS.iter().find(|item| **item == word) else {
                    return Err(Failure::ConfigDefect(
                        serde_json::json!({"ok": false, "reason": "invalid-risk-raise", "action": "name one of the risk items listed, or omit the raise", "value": word, "expected": RISK_ITEMS}),
                    ));
                };
                let Some(reason) = reason_of(self.raise_reason) else {
                    return Err(Failure::ConfigDefect(
                        serde_json::json!({"ok": false, "reason": "risk-raise-reason-missing", "action": "say in one sentence why the item applies with `raise_reason`", "value": word}),
                    ));
                };
                Some((*item, reason.to_owned()))
            }
        };
        let focus_reason = match self.focus_reason {
            None => None,
            Some(_) if focus.is_none() => {
                return Err(Failure::ConfigDefect(
                    serde_json::json!({"ok": false, "reason": "focus-reason-without-focus", "action": "name the `focus` the reason explains, or omit `focus_reason`"}),
                ));
            }
            Some(text) => match reason_of(Some(text)) {
                Some(reason) => Some(reason.to_owned()),
                None => {
                    return Err(Failure::ConfigDefect(
                        serde_json::json!({"ok": false, "reason": "blank-focus-reason", "action": "say why this focus was chosen, or omit `focus_reason`"}),
                    ));
                }
            },
        };
        Ok(Resolved {
            focus: ReviewFocus::parse(focus)?,
            focus_named: focus.is_some(),
            raise,
            focus_reason,
        })
    }
}

/// The three tiers, closed by construction — no fourth arm exists to reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    /// At least one path changed, every authored byte is proven passive prose,
    /// and nothing configures anything. A range that changed no path answers
    /// `Medium`, not this: over zero bytes the passive proof is vacuous.
    Low,
    /// Neither `Low` nor `High`: the ordinary case, one consolidated review —
    /// and a range nothing was read from, which is not the same as one read
    /// and found harmless.
    Medium,
    /// A hot-path token, an adjacent `service`/`token` path pair, a shell-source
    /// extension, an execute-bit flip, a process-spawn construct or a
    /// dangerous sink was touched.
    High,
}

impl RiskLevel {
    /// The number of lens questions this tier licenses — a fixed allocation
    /// per tier, not an audit of which lens could find a subject in the
    /// diff; more than one sealed lens can plausibly have a subject in an
    /// ordinary code-touching change, and this count does not measure that.
    /// `Medium` answers `1`, the lens its [`ReviewFocus`] names; `High`
    /// answers the source project's four. Takes no `Context` and cannot fail
    /// the way the rest of this module's reads can, because a lens count is
    /// derived entirely from the tier and never from a repository's own
    /// configuration.
    pub fn lenses(self) -> u8 {
        match self {
            RiskLevel::Low => 0,
            RiskLevel::Medium => 1,
            RiskLevel::High => HIGH_LENS_ROLES.len() as u8,
        }
    }

    /// The reserved reviewer roles this tier licenses, so the answer names
    /// what to launch and not only how many. `Medium` answers the one role
    /// `focus` selects; `High` answers all four and ignores `focus`; `Low`
    /// answers none.
    pub fn lens_roles(self, focus: ReviewFocus) -> Vec<&'static str> {
        match self {
            RiskLevel::Low => Vec::new(),
            RiskLevel::Medium => vec![focus.role()],
            RiskLevel::High => HIGH_LENS_ROLES.to_vec(),
        }
    }

    /// The English word [`RiskLevel::lenses`] answers for this tier, so a
    /// rendered sentence states the count once. Panics on a count `lenses`
    /// does not answer today — only `0`, `1` and `4`.
    pub fn lenses_spelled(self) -> &'static str {
        match self.lenses() {
            0 => "zero",
            1 => "one",
            4 => "four",
            other => {
                panic!("RiskLevel::lenses() answered {other}, which this has no word for")
            }
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    }

    pub(crate) fn parse(word: &str) -> Option<Self> {
        match word {
            "low" => Some(RiskLevel::Low),
            "medium" => Some(RiskLevel::Medium),
            "high" => Some(RiskLevel::High),
            _ => None,
        }
    }
}

/// The tier `base..head` derives, read from the answer [`review_tier`] gives
/// so a caller that needs only the level cannot classify differently.
pub(crate) fn tier_level(
    context: &Context,
    base: &str,
    head: &str,
    worktree: Option<&Path>,
) -> Result<RiskLevel, Failure> {
    let answer = review_tier(context, base, head, worktree, None)?;
    answer
        .get("level")
        .and_then(serde_json::Value::as_str)
        .and_then(RiskLevel::parse)
        .ok_or_else(|| Failure::Read("review_tier answered no readable level".to_owned()))
}

/// One piece of evidence behind a tier.
///
/// `signal` is set on the seven producers whose evidence is named — `auth`,
/// `update`, `security` or `payments` on `hot_path`, `auth` on
/// `service_token`, `shell_process` on `shell_source`, `process_boundary` and
/// `process_scan_limit`, a dangerous-sink family name on `dangerous_sink`,
/// `permissions` on `executable_mode` — and left unset on the five
/// fallback/exclusion codes, which name what was true of the change rather than a named signal:
/// `non_executable_only`, `configuration_change`, `executable_change`,
/// `empty_content` and `nothing_measured`. [`review_tier`] reads whether any
/// reason carries one, not which, to cross-check the tier a caller derived
/// independently against the tier this module computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskReason {
    /// One of `hot_path`, `service_token`, `shell_source`,
    /// `executable_mode`, `process_boundary`, `process_scan_limit`,
    /// `dangerous_sink`, `non_executable_only`, `configuration_change`,
    /// `executable_change`, `empty_content` or `nothing_measured`.
    pub code: &'static str,
    /// Set on `hot_path`, `service_token`, `shell_source`, `executable_mode`,
    /// `process_boundary`, `process_scan_limit` and `dangerous_sink`; unset on
    /// every other code.
    pub signal: Option<&'static str>,
    /// The path this reason is about, when it names one.
    pub path: Option<String>,
}

fn reason_json(reason: &RiskReason) -> serde_json::Value {
    let mut value = serde_json::json!({"code": reason.code});
    if let Some(signal) = reason.signal {
        value["signal"] = serde_json::json!(signal);
    }
    if let Some(path) = &reason.path {
        value["path"] = serde_json::json!(path);
    }
    value
}

/// Whether [`diff_stats`] reads the caller's ambient Git configuration or a
/// view immune to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffIsolation {
    /// The ordinary worktree and its ambient config. What `review_tier`
    /// reads, and whose `low` answer lets `release_ci` release with no
    /// recorded verdict, so a configuration that moves it moves a clearance.
    Ambient,
    /// A fresh, empty, bare `GIT_DIR`, reached through
    /// [`run_with_isolated_env`]'s replaced environment rather than
    /// [`run_with_env`]'s layered one. `docs/honesty.md` has what each of
    /// the two closes and what neither does.
    Isolated,
}

/// Derives the tier for `base..head` and answers it with the lens count, the
/// authored line count, and the reasons that explain the tier.
///
/// Reads two trees, never the worktree: both ends are resolved to commits and
/// every git call below is `diff <base> <head>`, so the answer is reproducible
/// from the same two object ids on any checkout that has them.
///
/// The whole change, always — a repair round reads a narrower one through
/// [`review_tier_for_repair`] instead of this one, when that narrower range
/// is honestly recoverable. This function's contract does not change to
/// serve that: it still answers `base..head` exactly as it always has.
pub fn review_tier(
    context: &Context,
    base: &str,
    head: &str,
    worktree: Option<&Path>,
    focus: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    review_tier_declared(
        context,
        base,
        head,
        worktree,
        focus,
        &Declaration::default(),
    )
}

/// [`review_tier`] with the caller's [`Declaration`]: a raise that can only
/// move the tier up, and the reason the focus was chosen.
pub fn review_tier_declared(
    context: &Context,
    base: &str,
    head: &str,
    worktree: Option<&Path>,
    focus: Option<&str>,
    declaration: &Declaration<'_>,
) -> Result<serde_json::Value, Failure> {
    let resolved = declaration.resolve(focus)?;
    let at = worktree.unwrap_or(&context.repo_dir);
    let base_oid = resolve_commit(at, base)?;
    let head_oid = resolve_commit(at, head)?;
    let stats = diff_stats(at, &base_oid, &head_oid, DiffIsolation::Ambient)?;
    let authority = base_oid.clone();
    let answer = review_tier_answer(
        at,
        &authority,
        base_oid,
        head_oid,
        stats,
        RangeAnswer::Plain,
        &resolved,
    )?;
    Ok(with_forecast(answer, context))
}

fn review_budget_of(context: &Context) -> crate::config::ReviewBudget {
    let mut config = crate::config::Config::default();
    match context.get("review budget") {
        Some(value)
            if crate::config::Setting::ReviewBudget
                .apply(&mut config, value)
                .is_ok() =>
        {
            config.review_budget
        }
        _ => crate::config::ReviewBudget::default(),
    }
}

fn forecast_line(answer: &serde_json::Value, budget: crate::config::ReviewBudget) -> String {
    let level = answer["level"].as_str().unwrap_or("unknown");
    let roles: Vec<&str> = answer["lens_roles"]
        .as_array()
        .map(|roles| roles.iter().filter_map(|role| role.as_str()).collect())
        .unwrap_or_default();
    let lenses = match roles.len() {
        0 => "no lenses".to_owned(),
        1 => format!("1 lens ({})", roles[0]),
        count => format!("{count} lenses ({})", roles.join(", ")),
    };
    let spend = match budget {
        crate::config::ReviewBudget::Off => {
            "no Review budget is declared, so no spend is forecast".to_owned()
        }
        crate::config::ReviewBudget::Tokens(per_context) => format!(
            "about {} tokens against a Review budget of {per_context} per context",
            u64::from(per_context) * (roles.len() as u64)
        ),
    };
    format!("{level}: {lenses}, {spend}")
}

fn with_forecast(mut answer: serde_json::Value, context: &Context) -> serde_json::Value {
    let line = forecast_line(&answer, review_budget_of(context));
    answer["forecast"] = serde_json::json!(line);
    answer
}

/// What range [`review_tier_answer`] measured, and — for
/// [`review_tier_for_repair`] alone — whether that was the repair's own
/// narrow one or a fallback to the whole change, and why.
///
/// Shared past this module by [`repair_range`], the one place that decides
/// it: a publication recording which range a re-review should read
/// (`src/transport/claim.rs`) asks this same question and must never carry a
/// second answer to it.
pub(crate) enum RangeAnswer {
    /// [`review_tier`]'s own contract: no narrower range exists to have
    /// fallen back from, so the answer carries no `range` field at all,
    /// unchanged from before either range-aware operation existed.
    Plain,
    /// [`review_tier_for_repair`] measured `parent_head..head` and it was
    /// honest: `parent_head` is an ancestor of `head` and the range touched
    /// at least one path.
    Repair,
    /// [`review_tier_for_repair`] fell back to `base..head` instead, and why.
    WholeChange(WholeChangeReason),
}

/// Why [`review_tier_for_repair`] fell back to the whole `base..head` range
/// instead of answering over the repair's own `parent_head..head`.
pub(crate) enum WholeChangeReason {
    /// `parent_head` is not an ancestor of `head`: history was rewritten
    /// under this repair, most often a rebase onto an advanced base, and
    /// there is no repair-only range left to recover from what the timeline
    /// records.
    HistoryRewritten,
    /// `parent_head` is still an ancestor of `head`, but `parent_head..head`
    /// touched no path at all — an identical-bytes republish, which the
    /// contract itself names as a real input. A range with no changed paths
    /// carries no evidence about the repair's own edits, so there is no
    /// repair-scoped tier to derive from it.
    EmptyRepairRange,
    /// `parent_head` is still an ancestor of `head`, but the base advanced:
    /// the fork point `merge_base(base, head)` is no longer inside
    /// `parent_head`'s history. The range carries the advance, which is the
    /// base's and not the repair's, and a base that moved is read whole.
    BaseAdvanced,
    /// `parent_head` is still an ancestor of `head` and the fork point is
    /// still inside its history — the base did not move — but
    /// `parent_head..head` contains a merge commit, most often a side branch
    /// merged back. The range carries that merge, which is not the repair's
    /// own edits, so the whole change is read.
    MergedRange,
}

impl WholeChangeReason {
    /// Every reason, so one recorded as a string can be read back through [`Self::as_str`] without
    /// a second list of the strings to drift from the first.
    const ALL: [WholeChangeReason; 4] = [
        WholeChangeReason::HistoryRewritten,
        WholeChangeReason::EmptyRepairRange,
        WholeChangeReason::BaseAdvanced,
        WholeChangeReason::MergedRange,
    ];

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            WholeChangeReason::HistoryRewritten => "history-rewritten",
            WholeChangeReason::EmptyRepairRange => "empty-repair-range",
            WholeChangeReason::BaseAdvanced => "base-advanced",
            WholeChangeReason::MergedRange => "merged-range",
        }
    }

    /// The reason a recorded `delta-range-reason` names, when it names one of these.
    pub(crate) fn parse(reason: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|known| known.as_str() == reason)
    }

    /// Whether the range carries edits that are not the repair's own — the base's advance, a
    /// rewritten history, or a merge. An empty range is still the repair's own, just empty.
    pub(crate) fn carries_upstream(&self) -> bool {
        matches!(
            self,
            WholeChangeReason::HistoryRewritten
                | WholeChangeReason::BaseAdvanced
                | WholeChangeReason::MergedRange
        )
    }
}

/// What [`repair_range`] can answer, and all it can answer.
///
/// Narrower than [`RangeAnswer`] deliberately: [`RangeAnswer::Plain`] is
/// [`review_tier`]'s own case, and a repair has by definition something to
/// have narrowed from. A type that cannot spell that arm leaves no
/// impossible case for either caller to assert against.
pub(crate) enum RepairRange {
    /// `parent_head` is an ancestor of `head` and the range touched at
    /// least one path, so `parent_head..head` is honest.
    Narrow,
    /// It is not honest, and why.
    WholeChange(WholeChangeReason),
}

/// The one place that decides whether `parent_head..head` is an honest range
/// to read for a repair, given a checkout and the three ends involved.
///
/// Ancestry answers whether history was rewritten, for the reason
/// [`review_tier_for_repair`]'s own doc comment argues at length: there is no
/// pair of object ids, frozen or not, that reads `parent_head..head` without
/// also reading a rebase. [`review_tier_for_repair`] calls this rather than
/// keeping its own copy of the decision, and so does
/// [`crate::transport::claim::publish_with`], which records the answer on the
/// publication marker rather than computing the diff or restricting what a
/// re-reviewer reads — `docs/honesty.md` names what that does and does not
/// buy. A base advance keeps the ancestry and is told apart by the fork point:
/// `merge_base(base, head)` outside `parent_head`'s history is
/// [`WholeChangeReason::BaseAdvanced`]. A merge commit in the range whose fork
/// point is still inside `parent_head`'s history is a different shape — the
/// base did not move, a side branch was merged back — and is
/// [`WholeChangeReason::MergedRange`], which also catches a caller passing a
/// base older than the one the head merged. Both are read whole.
pub(crate) fn repair_range(
    at: &Path,
    base: &str,
    parent_head: &str,
    head: &str,
) -> Result<RepairRange, Failure> {
    // Resolved and discarded: matching `review_tier_for_repair`'s own
    // unconditional resolution of all three ends, so a caller that never
    // separately validates `base` still gets a failed read on one that does
    // not resolve, regardless of which branch below the ancestry check takes.
    resolve_commit(at, base)?;
    let parent_head_oid = resolve_commit(at, parent_head)?;
    let head_oid = resolve_commit(at, head)?;

    if !super::target::is_ancestor(at, &parent_head_oid, &head_oid)? {
        return Ok(RepairRange::WholeChange(
            WholeChangeReason::HistoryRewritten,
        ));
    }

    let fork_point = super::target::merge_base(at, base, &head_oid)?;
    if !super::target::is_ancestor(at, &fork_point, &parent_head_oid)? {
        return Ok(RepairRange::WholeChange(WholeChangeReason::BaseAdvanced));
    }

    let merged_in_range = !run(
        &[
            "git",
            "rev-list",
            "--merges",
            "--max-count=1",
            &format!("{parent_head_oid}..{head_oid}"),
        ],
        Some(at),
        How::read(),
    )?
    .stdout
    .trim()
    .is_empty();
    if merged_in_range {
        return Ok(RepairRange::WholeChange(WholeChangeReason::MergedRange));
    }

    let repair_stats = diff_stats(at, &parent_head_oid, &head_oid, DiffIsolation::Ambient)?;
    if repair_stats.is_empty() {
        return Ok(RepairRange::WholeChange(
            WholeChangeReason::EmptyRepairRange,
        ));
    }

    Ok(RepairRange::Narrow)
}

/// Derives the tier for a repair round's own range, honestly.
///
/// A frozen candidate tree was never a defence against base movement:
/// `tree_of(head)` is the tree *of a commit*, `git diff tree(A) tree(B)` is
/// the same diff as `git diff A B`, and after a rebase the repair head's own
/// content already carries whatever the base's advance folded into it. There
/// is no pair of object ids, frozen or not, that reads `parent_head..head`
/// without also reading the rebase — the question this answers is not "which
/// ids to diff" but "is that range honest at all", and the only thing that
/// answers it is ancestry.
///
/// When `parent_head` is still an ancestor of `head`, nothing rewrote
/// history under this repair, and the range is read as `parent_head..head`
/// — except when that range touches no path at all: `git merge-base
/// --is-ancestor A A` exits `0`, so an identical-bytes republish (a real
/// input the contract itself names) makes `parent_head == head` an ancestor
/// of itself, and diffing a commit against itself proves nothing. Rather
/// than special-case that one input, the general rule holds for any empty
/// repair range: a diff with no changed paths says nothing about the repair's
/// own edits, so this falls back to the whole `base..head` rather than
/// narrowing to a range that measured none of them. When `parent_head` is *not* an ancestor of `head`, the base
/// moved and the branch was rewritten onto it — branch protection here
/// forces exactly that whenever `main` advances during review — and there is
/// no sound repair-only range left to recover from what the timeline
/// records, so this falls back to the whole range for that reason instead.
/// Either way the answer names which range it actually measured and why,
/// rather than a boolean claiming a base movement nobody tested: an amend
/// with no base movement is not a rewritten history, a base advanced and
/// merged in leaves `parent_head` an ancestor while the range still carries
/// the advance and is answered `base-advanced`, and a side branch merged back
/// with the base unmoved is answered `merged-range`; all three are read whole.
///
/// In the fallback case the answer's `"base"` field carries the resolved
/// `base` argument, as everywhere else; in the narrow case it carries
/// `parent_head` instead, since that is the range actually measured — there
/// is no separate field for it.
///
/// `clearing_finding` is the class of the `review_finding` that cleared the
/// approved-republish gate on the parent receipt — `severe`, `warning` or
/// `suggestion`, [`super::claim::FINDING_CLASSES`]'s own vocabulary; anything
/// else is a read failure naming the allowed values. When it names a
/// `warning` or a `suggestion` and the range this repair measured is the
/// honest narrow one, the answer additionally carries `panel: "lens"`,
/// `lenses: 1` — the delta lens alone reviews a repair of a
/// non-blocking finding, in place of `level`'s own allocation — unless the
/// repair's own level is `high`, where the hot-path shape keeps the tier's
/// own panel. `level` keeps its ordinary meaning either way; only the panel
/// size changes. A `severe` clearing finding, a `high` repair level, and the
/// whole-change fallback (history rewritten, a base advance, a merged range or
/// an empty repair range), all
/// keep the tier's own allocation — see
/// `skill/policies/blind-judges.md`'s repair-loop section for why.
pub fn review_tier_for_repair(
    context: &Context,
    base: &str,
    parent_head: &str,
    head: &str,
    worktree: Option<&Path>,
    clearing_finding: Option<&str>,
    focus: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    review_tier_for_repair_declared(
        context,
        base,
        parent_head,
        head,
        worktree,
        clearing_finding,
        focus,
        &Declaration::default(),
    )
}

/// [`review_tier_for_repair`] with the caller's [`Declaration`]. A raise to
/// `high` keeps the tier's own panel, as a `high` repair level always does.
#[allow(clippy::too_many_arguments)]
pub fn review_tier_for_repair_declared(
    context: &Context,
    base: &str,
    parent_head: &str,
    head: &str,
    worktree: Option<&Path>,
    clearing_finding: Option<&str>,
    focus: Option<&str>,
    declaration: &Declaration<'_>,
) -> Result<serde_json::Value, Failure> {
    let resolved = declaration.resolve(focus)?;
    if let Some(class) = clearing_finding
        && !super::claim::FINDING_CLASSES.contains(&class)
    {
        return Err(Failure::Read(format!(
            "clearing_finding must be one of {:?}, not {class:?}",
            super::claim::FINDING_CLASSES
        )));
    }
    let at = worktree.unwrap_or(&context.repo_dir);
    let base_oid = resolve_commit(at, base)?;
    let parent_head_oid = resolve_commit(at, parent_head)?;
    let head_oid = resolve_commit(at, head)?;

    let answer = match repair_range(at, base, parent_head, head)? {
        RepairRange::WholeChange(reason) => {
            let stats = diff_stats(at, &base_oid, &head_oid, DiffIsolation::Ambient)?;
            let answer = RangeAnswer::WholeChange(reason);
            let authority = base_oid.clone();
            review_tier_answer(at, &authority, base_oid, head_oid, stats, answer, &resolved)?
        }
        RepairRange::Narrow => {
            let repair_stats = diff_stats(at, &parent_head_oid, &head_oid, DiffIsolation::Ambient)?;
            let mut answer = review_tier_answer(
                at,
                &base_oid,
                parent_head_oid,
                head_oid,
                repair_stats,
                RangeAnswer::Repair,
                &resolved,
            )?;
            if let Some(class) = clearing_finding
                && class != "severe"
                && answer.get("level").and_then(|level| level.as_str()) != Some("high")
                && let Some(object) = answer.as_object_mut()
            {
                object.insert("panel".to_owned(), serde_json::json!("lens"));
                object.insert("lenses".to_owned(), serde_json::json!(1));
                object.insert(
                    "lens_roles".to_owned(),
                    serde_json::json!([crate::harness::roles::RESERVED_LENS]),
                );
            }
            answer
        }
    };
    Ok(with_forecast(answer, context))
}

/// The population and boundary `explained_matches_level` enforces.
pub const EXPLAINED_MATCHES_LEVEL_POPULATION: Population = Population {
    family: "explained-matches-level",
    disposition: Disposition::FailClosed,
    subject: "explained_matches_level",
    text: "every tier answer whose reasons do not license its level: a `classify_risk` result \
           that `risk_level_from_reasons` would not derive from the very reasons published \
           beside it. Boundary: a disagreement is treated as the stop arm, a `Failure::Read`, \
           rather than as clearance to publish a level nothing would explain. No input reaching \
           this cross-check through `review_tier_answer` is known to exist — `classify_risk` and \
           `risk_level_from_reasons` are both reached from the same \
           `derive_snapshot_risk_reasons` output there (the content scan's `process_boundary`, \
           `process_scan_limit` and `dangerous_sink` reasons all carry a signal, so they are \
           `high` on both sides), `classify_risk` additionally reading the \
           `stats` and the two flags `review_tier_answer` derives beside them, and \
           `tests::the_two_derivations_agree_on_a_spread_of_diff_stat_shapes` pins that agreement \
           over a spread of shapes rather than asserting it holds from inspection alone. The \
           guard exists for a future edit that breaks that agreement, not for one known today.",
};

/// A tier whose own reasons imply a different one could never be shown
/// honestly to whatever reads [`review_tier_answer`]'s answer to decide
/// anything from it. Takes `level` and `reasons` as plain parameters, rather
/// than deriving both internally, so a test can hand it a pair
/// [`review_tier_answer`]'s own computation is not known to ever produce.
fn explained_matches_level(level: RiskLevel, reasons: &[RiskReason]) -> Result<(), Failure> {
    let explained = risk_level_from_reasons(reasons);
    if explained != level {
        return Err(Failure::Read(format!(
            "review tier {level:?} is not explained by its reasons ({explained:?})"
        )));
    }
    Ok(())
}

/// The classification shared by [`review_tier`] and
/// [`review_tier_for_repair`], over an already-resolved pair of object ids
/// and the diff stats already read between them.
fn review_tier_answer(
    at: &Path,
    authority: &str,
    base_oid: String,
    head_oid: String,
    mut stats: Vec<DiffStat>,
    range: RangeAnswer,
    declared: &Resolved,
) -> Result<serde_json::Value, Failure> {
    let generated = super::generated::manifested_paths(at, authority, &base_oid, &head_oid)?;
    stats.retain(|stat| !generated.contains(&stat.path));
    let active_content = active_passive_content_paths(at, &stats)?;
    let content_reasons = content_scan::content_scan_reasons(at, &base_oid, &head_oid, &stats)?;
    let reasons = derive_snapshot_risk_reasons(&stats, &active_content, content_reasons);
    let (only_passive, touches_configuration) = passive_and_configuration(&stats, &active_content);
    let level = classify_risk(&stats, &reasons, only_passive, touches_configuration);
    explained_matches_level(level, &reasons)?;
    let computed_level = level;
    let level = match declared.raise {
        Some(_) => computed_level.max(RiskLevel::High),
        None => computed_level,
    };

    let mut answer = serde_json::json!({
        "ok": true,
        "base": base_oid,
        "head": head_oid,
        "level": level.as_str(),
        "lenses": level.lenses(),
        "lens_roles": level.lens_roles(declared.focus),
        "changed_lines": count_changed_lines(&stats),
        "generated_paths": generated,
        "reasons": reasons.iter().map(reason_json).collect::<Vec<_>>(),
    });
    if let Some((item, reason)) = &declared.raise {
        answer["computed_level"] = serde_json::json!(computed_level.as_str());
        answer["raise"] = serde_json::json!(item);
        answer["raise_reason"] = serde_json::json!(reason);
    }
    if let Some(reason) = &declared.focus_reason {
        answer["focus_reason"] = serde_json::json!(reason);
    }
    match range {
        RangeAnswer::Plain => {}
        RangeAnswer::Repair => {
            answer["range"] = serde_json::json!("repair");
        }
        RangeAnswer::WholeChange(reason) => {
            answer["range"] = serde_json::json!("whole-change");
            answer["whole_change_reason"] = serde_json::json!(reason.as_str());
        }
    }
    Ok(answer)
}

fn resolve_commit(at: &Path, reference: &str) -> Result<String, Failure> {
    let refspec = format!("{reference}^{{commit}}");
    let output = run(
        &["git", "rev-parse", "--verify", &refspec],
        Some(at),
        How::read(),
    )?;
    let oid = output.stdout.trim().to_owned();
    if oid.len() != 40 || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Failure::Read(format!(
            "git did not resolve {reference:?} to a commit id: {oid:?}"
        )));
    }
    Ok(oid)
}

fn passive_and_configuration(
    stats: &[DiffStat],
    active_content: &BTreeSet<String>,
) -> (bool, bool) {
    let only_passive = stats.iter().all(|stat| {
        !active_content.contains(&stat.path) && is_passive_content_candidate_stat(stat)
    });
    let touches_configuration = stats
        .iter()
        .any(|stat| is_configuration_review_path(&stat.path));
    (only_passive, touches_configuration)
}

fn classify_risk(
    stats: &[DiffStat],
    reasons: &[RiskReason],
    only_passive: bool,
    touches_configuration: bool,
) -> RiskLevel {
    let has_signal = reasons.iter().any(|reason| reason.signal.is_some());
    if has_signal || touches_hot_path(stats) {
        return RiskLevel::High;
    }
    // An empty stat set makes `only_passive` vacuously true — nothing was
    // measured, so nothing was proven passive either — checked explicitly
    // here because `only_passive` is `true` over an empty slice, so without
    // this the next branch returns `Low` for a range nothing was read from.
    if stats.is_empty() {
        return RiskLevel::Medium;
    }
    if only_passive && !touches_configuration {
        return RiskLevel::Low;
    }
    RiskLevel::Medium
}

/// Mirrors the tier a caller derives from the published reasons alone
/// (`riskLevelFromReasons`, `risk.go:185-195`).
fn risk_level_from_reasons(reasons: &[RiskReason]) -> RiskLevel {
    if reasons.iter().any(|reason| reason.signal.is_some()) {
        return RiskLevel::High;
    }
    if reasons.len() == 1 && reasons[0].code == REASON_NON_EXECUTABLE_ONLY {
        return RiskLevel::Low;
    }
    RiskLevel::Medium
}

// ---------------------------------------------------------------------------
// Diff stats: two git calls, two trees.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DiffStat {
    path: String,
    additions: u64,
    deletions: u64,
    binary: bool,
    mode_only: bool,
    old_mode: String,
    new_mode: String,
    old_object: String,
    new_object: String,
}

struct RawMode {
    old_mode: String,
    new_mode: String,
    old_object: String,
    new_object: String,
}

/// `--numstat -z` for additions, deletions and binary; `--raw -z` for the old
/// and new modes and blob ids, mirroring the source project's own two calls
/// (`snapshot.go:471`, `:506`). The two listings are cross-checked against
/// each other's path set — this port has no third, independent manifest to
/// check either one against, unlike the snapshot layer they are read from
/// there.
fn diff_stats(
    at: &Path,
    base: &str,
    head: &str,
    isolation: DiffIsolation,
) -> Result<Vec<DiffStat>, Failure> {
    let (_guard, envs) = match isolation {
        DiffIsolation::Ambient => (None, Vec::new()),
        DiffIsolation::Isolated => {
            let (guard, envs) = super::isolated_diff_env(at)?;
            (Some(guard), envs)
        }
    };
    let envs: Vec<(&str, &str)> = envs
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let run_diff = |diff_args: &[&str]| -> Result<Output, Failure> {
        match isolation {
            DiffIsolation::Ambient => run_with_env(diff_args, Some(at), How::tolerated(), &envs),
            DiffIsolation::Isolated => {
                run_with_isolated_env(diff_args, Some(at), How::tolerated(), &envs)
            }
        }
    };

    let numstat = run_diff(&pinned_diff_args(&[
        "--numstat",
        "-z",
        "--no-renames",
        "--ignore-submodules=none",
        base,
        head,
        "--",
    ]))?;
    if numstat.status != 0 && numstat.status != 1 {
        return Err(Failure::Read(format!(
            "git diff --numstat failed ({status}): {detail}",
            status = numstat.status,
            detail = numstat.stderr.trim()
        )));
    }
    if numstat.stdout_replaced {
        return Err(Failure::Read(
            "git diff --numstat produced bytes that are not UTF-8".to_owned(),
        ));
    }

    let raw = run_diff(&pinned_diff_args(&[
        "--raw",
        "-z",
        "--no-renames",
        "--ignore-submodules=none",
        base,
        head,
        "--",
    ]))?;
    if raw.status != 0 && raw.status != 1 {
        return Err(Failure::Read(format!(
            "git diff --raw failed ({status}): {detail}",
            status = raw.status,
            detail = raw.stderr.trim()
        )));
    }
    if raw.stdout_replaced {
        return Err(Failure::Read(
            "git diff --raw produced bytes that are not UTF-8".to_owned(),
        ));
    }

    let mut stats_by_path: BTreeMap<String, DiffStat> = BTreeMap::new();
    for record in numstat.stdout.split('\0') {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(Failure::Read(format!(
                "unexpected diff stat record {record:?}"
            )));
        };
        let logical = normalize_logical_path(path).map_err(Failure::Read)?;
        if stats_by_path.contains_key(&logical) {
            return Err(Failure::Read(format!(
                "duplicate diff stat path {logical:?}"
            )));
        }
        let (binary, additions, deletions) = if added == "-" && deleted == "-" {
            (true, 0, 0)
        } else {
            let additions = added
                .parse::<u64>()
                .map_err(|_| Failure::Read(format!("parse additions for {logical:?}")))?;
            let deletions = deleted
                .parse::<u64>()
                .map_err(|_| Failure::Read(format!("parse deletions for {logical:?}")))?;
            (false, additions, deletions)
        };
        stats_by_path.insert(
            logical.clone(),
            DiffStat {
                path: logical,
                additions,
                deletions,
                binary,
                mode_only: false,
                old_mode: String::new(),
                new_mode: String::new(),
                old_object: String::new(),
                new_object: String::new(),
            },
        );
    }

    let raw_records: Vec<&str> = raw.stdout.split('\0').collect();
    let mut modes_by_path: BTreeMap<String, RawMode> = BTreeMap::new();
    let mut index = 0;
    while index < raw_records.len() {
        let header = raw_records[index];
        if header.is_empty() {
            index += 1;
            continue;
        }
        let fields: Vec<&str> = header.split_whitespace().collect();
        if fields.len() != 5
            || fields[0].len() != 7
            || !fields[0].starts_with(':')
            || index + 1 >= raw_records.len()
            || raw_records[index + 1].is_empty()
        {
            return Err(Failure::Read(format!(
                "unexpected raw diff record {header:?}"
            )));
        }
        if !matches!(fields[4], "A" | "D" | "M" | "T") {
            return Err(Failure::Read(format!(
                "unexpected raw diff status {:?}",
                fields[4]
            )));
        }
        let old_mode = fields[0][1..].to_owned();
        let new_mode = fields[1].to_owned();
        if !valid_raw_git_mode(&old_mode) || !valid_raw_git_mode(&new_mode) {
            return Err(Failure::Read(format!(
                "unexpected raw diff modes {old_mode:?} and {new_mode:?}"
            )));
        }
        let old_object = fields[2].to_owned();
        let new_object = fields[3].to_owned();
        index += 1;
        let logical = normalize_logical_path(raw_records[index]).map_err(Failure::Read)?;
        if modes_by_path.contains_key(&logical) {
            return Err(Failure::Read(format!(
                "duplicate raw diff path {logical:?}"
            )));
        }
        modes_by_path.insert(
            logical,
            RawMode {
                old_mode,
                new_mode,
                old_object,
                new_object,
            },
        );
        index += 1;
    }

    if stats_by_path.len() != modes_by_path.len() {
        return Err(Failure::Read(
            "git diff numstat and raw disagree on the changed paths".to_owned(),
        ));
    }
    let mut stats = Vec::with_capacity(stats_by_path.len());
    for (path, mut stat) in stats_by_path {
        let Some(modes) = modes_by_path.remove(&path) else {
            return Err(Failure::Read(format!(
                "path {path:?} is missing from the raw diff"
            )));
        };
        stat.mode_only = modes.old_object == modes.new_object && modes.old_mode != modes.new_mode;
        stat.old_mode = modes.old_mode;
        stat.new_mode = modes.new_mode;
        stat.old_object = modes.old_object;
        stat.new_object = modes.new_object;
        stats.push(stat);
    }
    Ok(stats)
}

fn valid_raw_git_mode(mode: &str) -> bool {
    mode.len() == 6 && mode.bytes().all(|byte| (b'0'..=b'7').contains(&byte))
}

fn count_changed_lines(stats: &[DiffStat]) -> u64 {
    stats
        .iter()
        .filter(|stat| !stat.binary && !stat.mode_only)
        .map(|stat| stat.additions + stat.deletions)
        .sum()
}

// ---------------------------------------------------------------------------
// The correction budget: two tree-to-tree primitives and the pure formula.
// ---------------------------------------------------------------------------

const MAX_CORRECTION_CHANGED_LINES: u64 = 200;

pub(crate) fn correction_budget(original_changed_lines: u64) -> u64 {
    MAX_CORRECTION_CHANGED_LINES.min(original_changed_lines / 2 + original_changed_lines % 2)
}

pub(crate) fn floor_two(original_changed_lines: u64) -> u64 {
    let budget = correction_budget(original_changed_lines);
    if original_changed_lines > 0 && budget < 2 {
        2
    } else {
        budget
    }
}

pub(crate) fn tree_of(
    context: &Context,
    worktree: Option<&Path>,
    rev: &str,
) -> Result<String, Failure> {
    tree_at(worktree.unwrap_or(&context.repo_dir), rev)
}

pub(crate) fn tree_at(at: &Path, rev: &str) -> Result<String, Failure> {
    let oid = run(
        &["git", "rev-parse", "--verify", &format!("{rev}^{{tree}}")],
        Some(at),
        How::read(),
    )?
    .stdout
    .trim()
    .to_owned();
    if !super::target::is_object_id(&oid) {
        return Err(Failure::Read(format!(
            "git did not resolve {rev:?} to a tree object id: {oid:?}"
        )));
    }
    Ok(oid)
}

pub(crate) fn tree_pair_changed_lines(
    context: &Context,
    worktree: Option<&Path>,
    base_tree: &str,
    candidate_tree: &str,
    excluded: &[String],
) -> Result<u64, Failure> {
    let at = worktree.unwrap_or(&context.repo_dir);
    let mut stats = diff_stats(at, base_tree, candidate_tree, DiffIsolation::Isolated)?;
    stats.retain(|stat| !excluded.contains(&stat.path));
    Ok(count_changed_lines(&stats))
}

pub(crate) fn tree_pair_changed_paths(
    context: &Context,
    worktree: Option<&Path>,
    base_tree: &str,
    candidate_tree: &str,
) -> Result<Vec<String>, Failure> {
    tree_pair_changed_paths_at(
        worktree.unwrap_or(&context.repo_dir),
        base_tree,
        candidate_tree,
    )
}

pub(crate) fn tree_pair_changed_paths_at(
    at: &Path,
    base_tree: &str,
    candidate_tree: &str,
) -> Result<Vec<String>, Failure> {
    Ok(
        diff_stats(at, base_tree, candidate_tree, DiffIsolation::Isolated)?
            .into_iter()
            .map(|stat| stat.path)
            .collect(),
    )
}

/// The `new` side of every rename `base_tree..candidate_tree` contains,
/// normalised the way [`diff_stats`] normalises a changed path so the two can
/// be compared as strings. [`diff_stats`] reads with `--no-renames`, so a
/// rename is otherwise two independent paths; the integration admits a rename
/// destination beside the conflicted paths because a merge that moved a file
/// leaves the author a destination to adjust.
pub(crate) fn tree_pair_rename_destinations(
    context: &Context,
    worktree: Option<&Path>,
    base_tree: &str,
    candidate_tree: &str,
) -> Result<Vec<String>, Failure> {
    let at = worktree.unwrap_or(&context.repo_dir);
    let (_guard, envs) = super::isolated_diff_env(at)?;
    let envs: Vec<(&str, &str)> = envs
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let output = super::run_with_isolated_env(
        &pinned_diff_args(&[
            "--name-status",
            "-M",
            "-z",
            "--ignore-submodules=none",
            base_tree,
            candidate_tree,
            "--",
        ]),
        Some(at),
        How::read(),
        &envs,
    )?;
    if output.status != 0 && output.status != 1 {
        return Err(Failure::Read(format!(
            "git diff --name-status failed ({status}): {detail}",
            status = output.status,
            detail = output.stderr.trim()
        )));
    }
    let records: Vec<&str> = output.stdout.split('\0').collect();
    let mut destinations = Vec::new();
    let mut index = 0;
    while index < records.len() {
        let status = records[index];
        if status.is_empty() {
            index += 1;
            continue;
        }
        let (paths, width) = if status.starts_with('R') || status.starts_with('C') {
            (&records[index + 1..], 3)
        } else {
            (&records[index + 1..], 2)
        };
        if paths.len() < width - 1 {
            return Err(Failure::Read(format!(
                "unexpected diff name-status record {status:?}"
            )));
        }
        if width == 3 {
            let destination = normalize_logical_path(paths[1]).map_err(Failure::Read)?;
            destinations.push(destination);
        }
        index += width;
    }
    destinations.sort();
    destinations.dedup();
    Ok(destinations)
}

// ---------------------------------------------------------------------------
// Paths: normalisation, extensions, exclusions.
// ---------------------------------------------------------------------------

/// Rejects anything a diff record's path field cannot be trusted to mean:
/// empty, a `NUL`, a backslash (git always spells `/`, so one here is not a
/// separator), a leading `/`, or a value a lexical clean would not return
/// unchanged — which also catches `.`, `..` and any `../` a clean pass would
/// otherwise leave untouched. Ported from `normalizeLogicalPath`
/// (`risk.go:1177-1186`).
pub(crate) fn normalize_logical_path(value: &str) -> Result<String, String> {
    if value.is_empty() || value.contains('\0') || value.contains('\\') || value.starts_with('/') {
        return Err(format!("invalid logical path {value:?}"));
    }
    let cleaned = go_path_clean(value);
    if cleaned == "." || cleaned == ".." || cleaned.starts_with("../") || cleaned != value {
        return Err(format!("logical path is not canonical: {value:?}"));
    }
    Ok(cleaned)
}

/// A `/`-only lexical clean, matching Go's `path.Clean` rather than a
/// filesystem path type: collapses `.` segments and duplicate slashes, and
/// pops a `..` against the previous segment where one exists.
fn go_path_clean(value: &str) -> String {
    if value.is_empty() {
        return ".".to_owned();
    }
    let rooted = value.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for segment in value.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                if out.last().is_some_and(|last| *last != "..") {
                    out.pop();
                } else if !rooted {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    let joined = out.join("/");
    if rooted {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_owned()
    } else {
        joined
    }
}

/// The final path component: the whole string when it carries no `/`.
fn go_base(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, base)| base)
}

/// The suffix from the final `.` in the final path component, matching Go's
/// `path.Ext` — including a component that is *only* a dot-prefixed name,
/// such as `.env`, which is its own extension by this rule.
fn go_ext(path: &str) -> &str {
    let base = go_base(path);
    match base.rfind('.') {
        Some(at) => &base[at..],
        None => "",
    }
}

fn ascii_lower(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

fn split_lower_alnum(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|token| !token.is_empty())
}

/// Nominates the extensions whose content can still be passive. Membership is
/// only a nomination — mode and bytes decide the tier. Ported unchanged from
/// `isPassiveDocumentExtension` (`risk.go:901-912`).
fn is_passive_document_extension(logical_path: &str) -> bool {
    match go_ext(logical_path).to_lowercase().as_str() {
        ".md" => !is_operational_markdown_path(logical_path),
        ".mdx" | ".rst" | ".adoc" | ".png" | ".jpg" | ".jpeg" | ".gif" => true,
        _ => false,
    }
}

/// Ported unchanged from `isOperationalMarkdownPath` (`risk.go:914-944`).
fn is_operational_markdown_path(logical_path: &str) -> bool {
    let lower = ascii_lower(logical_path);
    let segments: Vec<&str> = lower.split('/').collect();
    if matches!(
        go_base(&lower),
        "agents.md"
            | "claude.md"
            | "gemini.md"
            | "kimi.md"
            | "skill.md"
            | "copilot-instructions.md"
    ) {
        return true;
    }
    for (index, segment) in segments.iter().enumerate() {
        if matches!(
            *segment,
            ".agent"
                | ".agents"
                | ".claude"
                | ".codex"
                | ".cursor"
                | ".opencode"
                | "openspec"
                | "runtime"
        ) {
            return true;
        }
        if index == 1
            && segments[0] == "internal"
            && matches!(*segment, "runtime" | "assets" | "templates")
        {
            return true;
        }
        let name = segment.strip_suffix(go_ext(segment)).unwrap_or(segment);
        for token in split_lower_alnum(name) {
            if matches!(
                token,
                "agent"
                    | "agents"
                    | "skill"
                    | "skills"
                    | "prompt"
                    | "prompts"
                    | "instruction"
                    | "instructions"
                    | "orchestrator"
                    | "orchestrators"
                    | "workflow"
                    | "workflows"
            ) {
                return true;
            }
        }
    }
    false
}

/// Nominates a path for the zero tier. Operational, agent-facing and
/// configuration material is excluded because those documents are consumed as
/// instructions rather than read as prose. Ported unchanged from
/// `isPassiveContentCandidatePath` (`risk.go:949-978`).
fn is_passive_content_candidate_path(logical_path: &str) -> bool {
    if !is_passive_document_extension(logical_path) || is_configuration_review_path(logical_path) {
        return false;
    }
    if logical_path.to_lowercase().starts_with("internal/assets/") {
        return false;
    }
    let base = go_base(logical_path).to_lowercase();
    if matches!(
        base.as_str(),
        "agents.md"
            | "claude.md"
            | "gemini.md"
            | "kimi.md"
            | "soul.md"
            | "tools.md"
            | "skill.md"
            | "copilot-instructions.md"
    ) {
        return false;
    }
    let ext = go_ext(&base).to_lowercase();
    let stem = base.strip_suffix(&ext).unwrap_or(&base);
    for token in stem
        .split(['-', '_', '.', ' '])
        .filter(|token| !token.is_empty())
    {
        if matches!(
            token,
            "agent"
                | "agents"
                | "command"
                | "commands"
                | "prompt"
                | "prompts"
                | "rule"
                | "rules"
                | "runtime"
                | "runtimes"
                | "skill"
                | "skills"
                | "workflow"
                | "workflows"
        ) {
            return false;
        }
    }
    for segment in logical_path.to_lowercase().split('/') {
        if matches!(
            segment,
            ".github"
                | ".claude"
                | ".codex"
                | ".cursor"
                | ".gemini"
                | ".kiro"
                | ".kilo"
                | ".opencode"
                | ".windsurf"
                | "agent"
                | "agents"
                | "command"
                | "commands"
                | "prompt"
                | "prompts"
                | "rule"
                | "rules"
                | "runtime"
                | "runtimes"
                | "skill"
                | "skills"
                | "workflow"
                | "workflows"
        ) {
            return false;
        }
    }
    true
}

/// Ported unchanged from `isRegularNonExecutableGitMode` (`risk.go:892-899`):
/// any mode this does not recognise is not passive, which is what fails a
/// mode this classifier cannot parse closed rather than open.
fn is_regular_non_executable_git_mode(mode: &str) -> bool {
    matches!(mode, "" | "000000" | "100644")
}

/// Adds the mode half of the zero-tier nomination. Ported unchanged from
/// `isPassiveContentCandidateStat` (`risk.go:983-988`).
fn is_passive_content_candidate_stat(stat: &DiffStat) -> bool {
    if stat.binary || stat.mode_only || !is_passive_content_candidate_path(&stat.path) {
        return false;
    }
    is_regular_non_executable_git_mode(&stat.old_mode)
        && is_regular_non_executable_git_mode(&stat.new_mode)
}

/// Ported unchanged from `isConfigurationReviewPath` (`risk.go:1097-1112`).
fn is_configuration_review_path(logical_path: &str) -> bool {
    let base = go_base(logical_path).to_lowercase();
    if base == ".env" || base.starts_with(".env.") {
        return true;
    }
    if matches!(
        base.as_str(),
        "go.mod"
            | "go.sum"
            | "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "dockerfile"
            | "makefile"
    ) {
        return true;
    }
    matches!(
        go_ext(logical_path).to_lowercase().as_str(),
        ".json" | ".yaml" | ".yml" | ".toml" | ".ini" | ".env"
    )
}

// ---------------------------------------------------------------------------
// Name-derived signals: service-token and shell-source paths, and the
// eligibility gate both are read behind.
// ---------------------------------------------------------------------------

/// Ported unchanged from `isInspectableStat` (`risk.go:839-843`), dropping the
/// generated-golden half this port never carries. The gate is the nonzero
/// authored line count: a path whose `additions + deletions` is zero, or that
/// is binary or mode-only, is not inspectable and carries no name-derived
/// signal — so a zero-byte `.github/workflows/*.yml` never reaches
/// [`is_semantic_risk_eligible`]'s shell-source producer.
fn is_inspectable_stat(stat: &DiffStat) -> bool {
    stat.additions + stat.deletions != 0 && !stat.binary && !stat.mode_only
}

/// Ported unchanged from `hasExcludedRiskSegment` (`risk.go:845-852`).
fn has_excluded_risk_segment(logical_path: &str) -> bool {
    logical_path
        .split('/')
        .any(|segment| EXCLUDED_RISK_SEGMENTS.contains(&ascii_lower(segment).as_str()))
}

/// Gates the two name-derived producers below: a path is weak evidence on its
/// own, so documentation, fixtures and a `readme`-shaped basename are kept
/// out — a file named `service-token` must never escalate on its name alone.
/// Ported unchanged from `isSemanticRiskEligible` (`risk.go:805-819`).
fn is_semantic_risk_eligible(stat: &DiffStat) -> bool {
    if !is_inspectable_stat(stat) || has_excluded_risk_segment(&stat.path) {
        return false;
    }
    if ascii_lower(go_base(&stat.path)).starts_with("readme") {
        return false;
    }
    let extension = ascii_lower(go_ext(&stat.path));
    if SEMANTIC_SOURCE_EXTENSIONS.contains(&extension.as_str()) {
        return true;
    }
    is_configuration_review_path(&stat.path)
}

/// Ported unchanged from `stripSemanticPathExtension` (`risk.go:857-863`).
fn strip_semantic_path_extension(segment: &str) -> &str {
    let extension = ascii_lower(go_ext(segment));
    if SEMANTIC_SOURCE_EXTENSIONS.contains(&extension.as_str())
        || is_configuration_review_path(segment)
    {
        segment.strip_suffix(go_ext(segment)).unwrap_or(segment)
    } else {
        segment
    }
}

/// Ported unchanged from `hasAdjacentServiceTokenTokens` (`risk.go:865-873`).
fn has_adjacent_service_token_tokens(segment: &str) -> bool {
    let lower = ascii_lower(segment);
    let tokens: Vec<&str> = lower
        .split(['-', '_'])
        .filter(|token| !token.is_empty())
        .collect();
    tokens
        .windows(2)
        .any(|pair| pair[0] == "service" && pair[1] == "token")
}

/// Ported unchanged from `isServiceTokenReviewPath` (`risk.go:693-700`).
fn is_service_token_review_path(logical_path: &str) -> bool {
    logical_path
        .split('/')
        .any(|segment| has_adjacent_service_token_tokens(strip_semantic_path_extension(segment)))
}

/// Ported unchanged from `isShellReviewPath` (`risk.go:702-711`).
fn is_shell_review_path(logical_path: &str) -> bool {
    match ascii_lower(go_ext(logical_path)).as_str() {
        ".sh" | ".bash" | ".zsh" => true,
        ".yml" | ".yaml" => logical_path.starts_with(".github/workflows/"),
        _ => false,
    }
}

/// The one mode-derived producer that carries no eligibility gate at all —
/// unlike its two name-derived neighbours above, a chmod inside `docs/` or
/// `testdata/` still reads as evidence. Ported unchanged from
/// `executableModeChanged` (`risk.go:713-720`), including the detail that
/// only a **literal empty string** reads as an absent side; git's own raw
/// diff never emits one — it emits `000000` — so this never treats a real
/// added-or-deleted file as absent, and a brand-new file created already
/// executable escalates the same as a chmod on an existing one.
fn executable_mode_changed(old_mode: &str, new_mode: &str) -> bool {
    if old_mode.is_empty() || new_mode.is_empty() {
        return false;
    }
    let (Ok(old_value), Ok(new_value)) = (
        u32::from_str_radix(old_mode, 8),
        u32::from_str_radix(new_mode, 8),
    ) else {
        return false;
    };
    (old_value & 0o111) != (new_value & 0o111)
}

// ---------------------------------------------------------------------------
// Hot path: the closed signal set and the five tokens.
// ---------------------------------------------------------------------------

/// Ported unchanged from `hotPathRiskSignals`'s token loop (`risk.go:1145-1163`).
/// Not ported: `isNativeReviewAuthorityPath`, the source project's own
/// six-path self-reference. This repository's review authority is not
/// separable by filename the way theirs is — `docs/honesty.md` names the
/// measurement that found this and the change that would let it
/// be ported.
fn hot_path_risk_signals(logical_path: &str) -> Vec<&'static str> {
    let mut signals = Vec::new();
    let lower = logical_path.to_lowercase();
    for token in lower
        .split(['/', '\\', '.', '-', '_'])
        .filter(|token| !token.is_empty())
    {
        match token {
            "auth" => signals.push("auth"),
            "update" => signals.push("update"),
            "security" => signals.push("security"),
            // A webhook handler owns signature verification, credential
            // handling and authorization boundaries, so it is security
            // evidence — the same reading `risk.go:1155-1158` gives it.
            "webhook" => signals.push("security"),
            "payments" => signals.push("payments"),
            _ => {}
        }
    }
    signals.sort_unstable();
    signals.dedup();
    signals
}

fn touches_hot_path(stats: &[DiffStat]) -> bool {
    stats
        .iter()
        .any(|stat| !hot_path_risk_signals(&stat.path).is_empty())
}

// ---------------------------------------------------------------------------
// Reasons.
// ---------------------------------------------------------------------------

fn is_empty_content_stat(stat: &DiffStat) -> bool {
    !stat.binary && !stat.mode_only && stat.additions + stat.deletions == 0
}

/// Ported from `fallbackRiskReason` (`risk.go:722-738`), first match over the
/// stats in path order. An empty slice answers [`REASON_NOTHING_MEASURED`]
/// rather than falling through the loop to [`REASON_NON_EXECUTABLE_ONLY`],
/// which an absent loop reaches vacuously — saying a change was measured and
/// found passive, of one that was never measured at all. That branch is a
/// divergence from the port rather than a translation of it: the Go original
/// has none.
fn fallback_risk_reason(stats: &[DiffStat], active_content: &BTreeSet<String>) -> RiskReason {
    if stats.is_empty() {
        return RiskReason {
            code: REASON_NOTHING_MEASURED,
            signal: None,
            path: None,
        };
    }
    for stat in stats {
        if is_configuration_review_path(&stat.path) {
            return RiskReason {
                code: REASON_CONFIGURATION_CHANGE,
                signal: None,
                path: Some(stat.path.clone()),
            };
        }
        let contradicted = active_content.contains(&stat.path);
        if contradicted || !is_passive_content_candidate_stat(stat) {
            let code = if is_empty_content_stat(stat) {
                REASON_EMPTY_CONTENT
            } else {
                REASON_EXECUTABLE_CHANGE
            };
            return RiskReason {
                code,
                signal: None,
                path: Some(stat.path.clone()),
            };
        }
    }
    RiskReason {
        code: REASON_NON_EXECUTABLE_ONLY,
        signal: None,
        path: None,
    }
}

/// Ported from `deriveSnapshotRiskReasons` (`risk.go:664-691`): the hot path,
/// the two name-derived producers behind [`is_semantic_risk_eligible`], and
/// the ungated mode-flip producer, and the reasons the content scan
/// ([`content_scan`]) already read from the frozen bytes.
fn derive_snapshot_risk_reasons(
    stats: &[DiffStat],
    active_content: &BTreeSet<String>,
    content_reasons: Vec<RiskReason>,
) -> Vec<RiskReason> {
    let mut candidates = content_reasons;
    for stat in stats {
        if is_semantic_risk_eligible(stat) {
            if is_service_token_review_path(&stat.path) {
                candidates.push(RiskReason {
                    code: REASON_SERVICE_TOKEN,
                    signal: Some("auth"),
                    path: Some(stat.path.clone()),
                });
            }
            if is_shell_review_path(&stat.path) {
                candidates.push(RiskReason {
                    code: REASON_SHELL_SOURCE,
                    signal: Some("shell_process"),
                    path: Some(stat.path.clone()),
                });
            }
        }
        if executable_mode_changed(&stat.old_mode, &stat.new_mode) {
            candidates.push(RiskReason {
                code: REASON_EXECUTABLE_MODE,
                signal: Some("permissions"),
                path: Some(stat.path.clone()),
            });
        }
        for signal in hot_path_risk_signals(&stat.path) {
            candidates.push(RiskReason {
                code: REASON_HOT_PATH,
                signal: Some(signal),
                path: Some(stat.path.clone()),
            });
        }
    }
    if candidates.is_empty() {
        candidates.push(fallback_risk_reason(stats, active_content));
    }
    canonical_risk_reasons(candidates)
}

/// Sorts and dedups on `(code, signal)`, matching `canonicalRiskReasons`
/// (`risk.go:753-781`) — the path is a tiebreak for which duplicate survives,
/// never part of the identity a duplicate is judged against.
fn canonical_risk_reasons(mut reasons: Vec<RiskReason>) -> Vec<RiskReason> {
    reasons.sort_by(|left, right| {
        left.code
            .cmp(right.code)
            .then_with(|| left.signal.unwrap_or("").cmp(right.signal.unwrap_or("")))
            .then_with(|| {
                left.path
                    .as_deref()
                    .unwrap_or("")
                    .cmp(right.path.as_deref().unwrap_or(""))
            })
    });
    let mut seen = BTreeSet::new();
    let mut canonical = Vec::new();
    for reason in reasons {
        if seen.insert((reason.code, reason.signal.unwrap_or(""))) {
            canonical.push(reason);
        }
    }
    canonical
}

// ---------------------------------------------------------------------------
// Passive content proof: byte-bounded, tree-to-tree.
// ---------------------------------------------------------------------------

/// Reads the frozen bytes behind every passive candidate and returns the
/// paths whose content contradicts the passive shape their extension
/// advertises. Sizes are probed before any blob is read so classification
/// stays bounded — an over-budget candidate set is never proven passive and
/// fails closed into a higher tier. Ported from `activePassiveContentPaths`
/// (`risk.go:314-397`): one `cat-file --batch-check` sizes every candidate and
/// one `cat-file --batch` reads them, as the source project's batched call does.
fn active_passive_content_paths(
    at: &Path,
    stats: &[DiffStat],
) -> Result<BTreeSet<String>, Failure> {
    let candidates: Vec<&DiffStat> = stats
        .iter()
        .filter(|stat| is_passive_content_candidate_stat(stat))
        .collect();
    if candidates.is_empty() {
        return Ok(BTreeSet::new());
    }

    let mut oids: Vec<&str> = Vec::new();
    for stat in &candidates {
        for (mode, object) in [
            (&stat.old_mode, &stat.old_object),
            (&stat.new_mode, &stat.new_object),
        ] {
            if !mode.is_empty() && mode != "000000" {
                oids.push(object);
            }
        }
    }
    oids.sort_unstable();
    oids.dedup();

    let total: u64 = blob_sizes(at, &oids)?.iter().sum();
    if total > SCAN_BYTE_BUDGET {
        return Ok(candidates.iter().map(|stat| stat.path.clone()).collect());
    }

    let contents: BTreeMap<&str, String> = oids
        .iter()
        .copied()
        .zip(blob_contents(at, &oids)?)
        .collect();

    let mut active = BTreeSet::new();
    for stat in &candidates {
        for (mode, object) in [
            (&stat.old_mode, &stat.old_object),
            (&stat.new_mode, &stat.new_object),
        ] {
            if mode.is_empty() || mode == "000000" {
                continue;
            }
            let Some(text) = contents.get(object.as_str()) else {
                return Err(Failure::Read(format!(
                    "read passive candidate {:?}: blob {object} was not read back",
                    stat.path
                )));
            };
            if !is_passive_document_content(&stat.path, text) {
                active.insert(stat.path.clone());
                break;
            }
        }
    }
    Ok(active)
}

pub(super) fn blob_sizes(at: &Path, oids: &[&str]) -> Result<Vec<u64>, Failure> {
    let answer = cat_file_batch(at, "--batch-check", oids)?;
    let mut rest = answer.as_slice();
    oids.iter()
        .map(|oid| batch_header(&mut rest, oid))
        .collect()
}

pub(super) fn blob_contents(at: &Path, oids: &[&str]) -> Result<Vec<String>, Failure> {
    let answer = cat_file_batch(at, "--batch", oids)?;
    let mut rest = answer.as_slice();
    oids.iter()
        .map(|oid| {
            let size = usize::try_from(batch_header(&mut rest, oid)?)
                .map_err(|_| Failure::Read(format!("blob {oid} is too large to read")))?;
            if rest.get(size) != Some(&b'\n') {
                return Err(Failure::Read(format!(
                    "git cat-file --batch answered blob {oid} short"
                )));
            }
            let content = String::from_utf8_lossy(&rest[..size]).into_owned();
            rest = &rest[size + 1..];
            Ok(content)
        })
        .collect()
}

fn cat_file_batch(at: &Path, mode: &str, oids: &[&str]) -> Result<Vec<u8>, Failure> {
    if oids.is_empty() {
        return Ok(Vec::new());
    }
    let input: String = oids.iter().map(|oid| format!("{oid}\n")).collect();
    super::run_with_stdin(
        &["git", "cat-file", mode],
        Some(at),
        How::read(),
        input.as_bytes(),
    )
}

fn batch_header(rest: &mut &[u8], oid: &str) -> Result<u64, Failure> {
    let Some(end) = rest.iter().position(|byte| *byte == b'\n') else {
        return Err(Failure::Read(format!(
            "git cat-file answered nothing for {oid}"
        )));
    };
    let line = String::from_utf8_lossy(&rest[..end]).into_owned();
    *rest = &rest[end + 1..];
    let fields: Vec<&str> = line.split(' ').collect();
    match fields.as_slice() {
        [_, _, size] => size
            .parse::<u64>()
            .map_err(|_| Failure::Read(format!("git cat-file answered {line:?} for {oid}"))),
        _ => Err(Failure::Read(format!(
            "git cat-file could not read {oid}: {line:?}"
        ))),
    }
}

pub(super) fn blob_content(at: &Path, oid: &str) -> Result<String, Failure> {
    let output = run(&["git", "cat-file", "-p", oid], Some(at), How::read())?;
    Ok(output.stdout)
}

/// Proves a document inert from its own bytes: an embedded `NUL`, bytes this
/// transport could not decode as UTF-8, an interpreter directive, or (for
/// `.mdx`) runtime syntax all withdraw the nomination extension made. Ported
/// from `isPassiveDocumentContent` (`risk.go:448-459`), with one deliberate
/// divergence from `utf8.Valid`: `run()` hands back a `String` already
/// decoded with `String::from_utf8_lossy`, so the original bytes are gone by
/// the time this runs and `Output::stdout_replaced` cannot be read here at
/// all — and it would under-detect on its own regardless: a length-
/// preserving invalid subpart (three raw bytes replaced by one three-byte
/// `U+FFFD`) leaves it `false`. This checks the decoded text for `U+FFFD`
/// directly instead, which catches every invalid subpart regardless of its
/// length delta and is a strict superset of what `stdout_replaced` would
/// have caught, so this function takes no such parameter. The consequence:
/// a document whose bytes are valid UTF-8 and legitimately **contain**
/// `U+FFFD` is no longer provably passive either — `medium`, never `low` —
/// because this function cannot tell that content apart from lossy-decoded
/// damage once both have arrived as the same character. That is
/// fail-closed, not a bug, and it is a real divergence from `utf8.Valid`,
/// named in `docs/honesty.md`.
fn is_passive_document_content(logical_path: &str, content: &str) -> bool {
    if content.contains('\0') || content.contains(char::REPLACEMENT_CHARACTER) {
        return false;
    }
    if has_interpreter_directive(content) {
        return false;
    }
    if go_ext(logical_path).eq_ignore_ascii_case(".mdx") {
        return is_static_mdx_document(content);
    }
    true
}

/// Ported unchanged from `hasInterpreterDirective` (`risk.go:463-465`).
fn has_interpreter_directive(content: &str) -> bool {
    content
        .strip_prefix('\u{feff}')
        .unwrap_or(content)
        .starts_with("#!")
}

/// Ported from `isStaticMDXDocument` (`risk.go:990-1017`): every line outside
/// a fenced block must carry no JavaScript import/export, brace or tag once
/// its own code spans and leading block comments are stripped.
///
/// The fence walk itself lives in
/// [`crate::markdown::markdown_lines_outside_fences`] — the corpus
/// extractor `crate::markdown::markdown_inline_code_spans` needs the exact
/// same fence handling, and a second copy is how the two would drift apart.
fn is_static_mdx_document(content: &str) -> bool {
    for trimmed in crate::markdown::markdown_lines_outside_fences(content) {
        let prose = trim_leading_js_block_comments(&strip_markdown_code_spans(trimmed));
        if starts_js_keyword(&prose, "import")
            || starts_js_keyword(&prose, "export")
            || prose.contains(['{', '}'])
            || prose.contains('<')
        {
            return false;
        }
    }
    true
}

fn trim_leading_js_block_comments(value: &str) -> String {
    let mut value = value;
    while let Some(rest) = value.strip_prefix("/*") {
        let Some(end) = rest.find("*/") else { break };
        value = rest[end + 2..].trim();
    }
    value.to_owned()
}

fn starts_js_keyword(value: &str, keyword: &str) -> bool {
    let Some(rest) = value.strip_prefix(keyword) else {
        return false;
    };
    match rest.chars().next() {
        None => true,
        Some(next) => !(next.is_ascii_alphanumeric() || next == '_' || next == '$'),
    }
}

/// Keeps only the prose of `value`, backtick-delimited code spans removed —
/// the polarity `crate::markdown::markdown_inline_code_spans` does not
/// want, since that one keeps the spans and drops the prose. Both read the
/// same backtick walk, [`crate::markdown::markdown_segments`], rather than
/// each matching backtick runs and escapes on its own.
fn strip_markdown_code_spans(value: &str) -> String {
    let mut visible = String::new();
    for segment in crate::markdown::markdown_segments(value) {
        if let crate::markdown::MarkdownSegment::Prose(text) = segment {
            visible.push_str(text);
        }
    }
    visible.trim().to_owned()
}

mod content_scan;

#[cfg(test)]
mod tests;
