//! The delivery rows: who takes each step between a finished change and its
//! merge, and what a run waits for before going on.
//!
//! `docs/configuration.md`'s *Delivery* group says what each row decides.

use crate::outcome::{NoCommandReason, Refusal, Resolution};

use super::{Config, Setting, Tracker, Workflow};

/// Who performs a step: the run, or a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Party {
    /// The run holding the issue.
    Agent,
    /// Somebody on the team.
    Person,
}

impl Party {
    /// Every answer, in the order the rows offer them.
    pub const ALL: [Self; 2] = [Self::Agent, Self::Person];

    /// The value as written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Person => "person",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "agent" => Some(Self::Agent),
            "person" => Some(Self::Person),
            _ => None,
        }
    }
}

/// What a correction does to a pull request that is already ready for review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corrections {
    /// The run publishes nothing to a ready pull request.
    Refuse,
    /// The pull request goes back to draft before the correction is pushed.
    Redraft,
    /// The correction is pushed to the ready pull request as it stands.
    PushReady,
}

impl Corrections {
    /// Every answer, in the order the row offers them.
    pub const ALL: [Self; 3] = [Self::Refuse, Self::Redraft, Self::PushReady];

    /// The value as written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Refuse => "refuse",
            Self::Redraft => "redraft",
            Self::PushReady => "push-ready",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "refuse" => Some(Self::Refuse),
            "redraft" => Some(Self::Redraft),
            "push-ready" => Some(Self::PushReady),
            _ => None,
        }
    }
}

/// A named combination of the delivery rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryPreset {
    /// Estigia claims, reviews, marks ready and merges.
    Autonomous,
    /// The team coordinates; the run stops at a draft pull request.
    Assisted,
    /// The team coordinates; the run reviews its own draft, marks it ready and
    /// addresses review rounds, and a person merges.
    Team,
}

/// The five rows a preset fills, in table order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetRows {
    /// `Claim workflow`.
    pub workflow: Workflow,
    /// `Self review`.
    pub self_review: bool,
    /// `Ready by`.
    pub ready_by: Party,
    /// `Corrections after ready`.
    pub corrections: Corrections,
    /// `Merge by`.
    pub merge_by: Party,
}

impl DeliveryPreset {
    /// Every preset, in the order the row offers them.
    pub const ALL: [Self; 3] = [Self::Autonomous, Self::Assisted, Self::Team];

    /// The value as written in the table.
    pub fn as_value(self) -> &'static str {
        match self {
            Self::Autonomous => "autonomous",
            Self::Assisted => "assisted",
            Self::Team => "team",
        }
    }

    /// The preset a written value names.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|preset| preset.as_value() == value)
    }

    /// The rows this preset stands for.
    pub fn rows(self) -> PresetRows {
        match self {
            Self::Autonomous => PresetRows {
                workflow: Workflow::Autonomous,
                self_review: true,
                ready_by: Party::Agent,
                corrections: Corrections::Redraft,
                merge_by: Party::Agent,
            },
            Self::Assisted => PresetRows {
                workflow: Workflow::Assisted,
                self_review: false,
                ready_by: Party::Person,
                corrections: Corrections::Refuse,
                merge_by: Party::Person,
            },
            Self::Team => PresetRows {
                workflow: Workflow::Assisted,
                self_review: true,
                ready_by: Party::Agent,
                corrections: Corrections::PushReady,
                merge_by: Party::Person,
            },
        }
    }

    /// The preset `Claim workflow` alone stands for, which the four rows it
    /// does not name follow until one is written.
    pub fn of_workflow(workflow: Workflow) -> Self {
        match workflow {
            Workflow::Autonomous => Self::Autonomous,
            Workflow::Assisted => Self::Assisted,
        }
    }
}

/// What a stage waits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageKind {
    /// A CI check run, named as GitHub shows it, optionally with the login whose
    /// review comments that check leaves.
    Check {
        /// The check run's name.
        name: String,
        /// The account the check comments as, such as `cubic-dev-ai[bot]`.
        author: Option<String>,
    },
    /// A person's or a team's review: a login, `org/team`, or `any` for any
    /// account other than the pull request's author.
    Review {
        /// Who is asked.
        reviewer: String,
    },
}

/// One stage of `External reviewers`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewStage {
    /// What the stage waits on.
    pub kind: StageKind,
    /// Whether its comments must be addressed before the next stage starts.
    pub blocking: bool,
}

impl ReviewStage {
    fn as_value(&self) -> String {
        let body = match &self.kind {
            StageKind::Check {
                name,
                author: Some(author),
            } => format!("check:{name}@{author}"),
            StageKind::Check { name, author: None } => format!("check:{name}"),
            StageKind::Review { reviewer } => format!("review:{reviewer}"),
        };
        if self.blocking {
            body
        } else {
            format!("{body}:advisory")
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (text, blocking) = match text.rsplit_once(':') {
            Some((rest, "advisory")) => (rest, false),
            Some((rest, "blocking")) => (rest, true),
            _ => (text, true),
        };
        let (kind, rest) = text.split_once(':')?;
        let rest = rest.trim();
        let kind = match kind.trim().to_ascii_lowercase().as_str() {
            "check" => {
                let (name, author) = match rest.rsplit_once('@') {
                    Some((name, author)) => (name.trim(), Some(author.trim())),
                    None => (rest, None),
                };
                if name.is_empty() || author.is_some_and(str::is_empty) {
                    return None;
                }
                StageKind::Check {
                    name: name.to_owned(),
                    author: author.map(str::to_owned),
                }
            }
            "review" if !rest.is_empty() && !rest.contains(char::is_whitespace) => {
                StageKind::Review {
                    reviewer: rest.to_owned(),
                }
            }
            _ => return None,
        };
        Some(Self { kind, blocking })
    }
}

/// Reads `External reviewers`: `none`, or comma-separated stages in order.
pub(crate) fn parse_stages(value: &str) -> Option<Vec<ReviewStage>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") || value.is_empty() {
        return Some(Vec::new());
    }
    value.split(',').map(ReviewStage::parse).collect()
}

/// Writes `External reviewers` back down.
pub(crate) fn render_stages(stages: &[ReviewStage]) -> String {
    if stages.is_empty() {
        return "none".to_owned();
    }
    stages
        .iter()
        .map(ReviewStage::as_value)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The workflow states `State names` may map.
pub const MAPPED_STATES: &[&str] = &["in-progress", "review", "done"];

/// Reads `State names`: `none`, or comma-separated `state=Name` pairs.
pub(crate) fn parse_state_names(value: &str) -> Option<Vec<(String, String)>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") || value.is_empty() {
        return Some(Vec::new());
    }
    let mut pairs: Vec<(String, String)> = Vec::new();
    for pair in value.split(',') {
        let (state, name) = pair.split_once('=')?;
        let state = state.trim().to_ascii_lowercase();
        let name = name.trim();
        if !MAPPED_STATES.contains(&state.as_str())
            || name.is_empty()
            || pairs.iter().any(|(seen, _)| *seen == state)
        {
            return None;
        }
        pairs.push((state, name.to_owned()));
    }
    Some(pairs)
}

/// Writes `State names` back down.
pub(crate) fn render_state_names(pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return "none".to_owned();
    }
    pairs
        .iter()
        .map(|(state, name)| format!("{state}={name}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn parse_switch(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

pub(crate) fn parse_party(value: &str) -> Option<Party> {
    Party::parse(value)
}

pub(crate) fn parse_corrections(value: &str) -> Option<Corrections> {
    Corrections::parse(value)
}

impl Config {
    fn following(&self) -> PresetRows {
        DeliveryPreset::of_workflow(self.workflow).rows()
    }

    /// `Self review` in force: the written row, or what `Claim workflow` stands for.
    pub fn self_review(&self) -> bool {
        self.self_review
            .unwrap_or_else(|| self.following().self_review)
    }

    /// `Ready by` in force.
    pub fn ready_by(&self) -> Party {
        self.ready_by.unwrap_or_else(|| self.following().ready_by)
    }

    /// `Corrections after ready` in force.
    pub fn corrections_after_ready(&self) -> Corrections {
        self.corrections_after_ready
            .unwrap_or_else(|| self.following().corrections)
    }

    /// `Merge by` in force.
    pub fn merge_by(&self) -> Party {
        self.merge_by.unwrap_or_else(|| self.following().merge_by)
    }

    fn preset_rows_in_force(&self) -> PresetRows {
        PresetRows {
            workflow: self.workflow,
            self_review: self.self_review(),
            ready_by: self.ready_by(),
            corrections: self.corrections_after_ready(),
            merge_by: self.merge_by(),
        }
    }

    /// The preset the five rows read as, or `None` when they match none.
    pub fn delivery_preset(&self) -> Option<DeliveryPreset> {
        let rows = self.preset_rows_in_force();
        DeliveryPreset::ALL
            .into_iter()
            .find(|preset| preset.rows() == rows)
    }

    /// Writes every row `preset` stands for. `State names` and `External
    /// reviewers` are this repository's own lists and stay as they are.
    pub fn set_delivery_preset(&mut self, preset: DeliveryPreset) {
        let rows = preset.rows();
        self.workflow = rows.workflow;
        self.self_review = Some(rows.self_review);
        self.ready_by = Some(rows.ready_by);
        self.corrections_after_ready = Some(rows.corrections);
        self.merge_by = Some(rows.merge_by);
    }

    /// The first pair of delivery rows that cannot both hold, refused naming
    /// both; `None` when the rows agree.
    pub fn delivery_conflict(&self) -> Option<Refusal> {
        let way_out = |text: &str| {
            Resolution::no_command(NoCommandReason::OperatorKnowledge, text.to_owned())
        };
        if self.workflow == Workflow::Autonomous
            && let Some(row) = self.row_autonomous_fixes()
        {
            return Some(Refusal::not_started(
                "autonomous-fixes-delivery-rows",
                format!(
                    "`Claim workflow` is `autonomous`, which fixes `{}` at the autonomous preset's \
                     value, and this repository's `{}` is `{}`",
                    row.label(),
                    row.label(),
                    row.value_of(self)
                ),
                way_out(
                    "set `Delivery preset` to `team` or `assisted`, or `Claim workflow` to \
                     `assisted`, to choose this row; or set the row back to the autonomous value",
                ),
            ));
        }
        if self.workflow == Workflow::Assisted && self.merge_by() == Party::Agent {
            return Some(Refusal::not_started(
                "assisted-merge-is-the-persons",
                "`Merge by` is `agent` and `Claim workflow` is `assisted`: an assisted run holds \
                 no tracker claim, and delivery is refused without one"
                    .to_owned(),
                way_out("set `Merge by` to `person`, or `Delivery preset` to `autonomous`"),
            ));
        }
        if !self.state_names.is_empty() && !matches!(self.tracker, Tracker::Linear { .. }) {
            return Some(Refusal::not_started(
                "state-names-need-linear",
                format!(
                    "`State names` maps states onto the tracker's own, and this repository's \
                     `Tracker` is `{}`, whose states Estigia already writes as labels",
                    self.tracker.as_value()
                ),
                way_out("set `State names` to `none`, or `Tracker` to `linear <team-key>`"),
            ));
        }
        None
    }

    fn row_autonomous_fixes(&self) -> Option<Setting> {
        let autonomous = DeliveryPreset::Autonomous.rows();
        [
            (
                Setting::SelfReview,
                self.self_review() != autonomous.self_review,
            ),
            (Setting::ReadyBy, self.ready_by() != autonomous.ready_by),
            (
                Setting::CorrectionsAfterReady,
                self.corrections_after_ready() != autonomous.corrections,
            ),
            (Setting::MergeBy, self.merge_by() != autonomous.merge_by),
            (Setting::StateNames, !self.state_names.is_empty()),
            (
                Setting::ExternalReviewers,
                !self.external_reviewers.is_empty(),
            ),
        ]
        .into_iter()
        .find_map(|(row, differs)| differs.then_some(row))
    }
}

#[cfg(test)]
mod tests;
