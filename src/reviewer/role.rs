use crate::harness::roles::{
    RESERVED_LENS, RESERVED_READABILITY, RESERVED_READBACK, RESERVED_REFUTER, RESERVED_RELIABILITY,
    RESERVED_RISK, RESERVED_VALIDATOR,
};

/// A reviewer role the binary can hand a prompt to: the five sealed lenses, the
/// refuter and the targeted validator. The two judges are not here: their
/// template carries a placeholder for the evidence standard that only the
/// launching lifecycle renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// `review-risk`.
    Risk,
    /// `review-readback`.
    Readback,
    /// `review-readability`.
    Readability,
    /// `review-reliability`.
    Reliability,
    /// `review-lens`, the delta lens.
    Lens,
    /// `review-refuter`.
    Refuter,
    /// `review-validator`.
    Validator,
}

/// The shape of the answer a role gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// A list of findings.
    Findings,
    /// One outcome per candidate finding.
    Outcomes,
    /// An inspection and its two criteria.
    Inspection,
}

impl Role {
    /// Every role, in the order the lens panel is launched, then the refuter and
    /// the validator.
    pub const ALL: [Role; 7] = [
        Role::Risk,
        Role::Readback,
        Role::Readability,
        Role::Reliability,
        Role::Lens,
        Role::Refuter,
        Role::Validator,
    ];

    /// The reserved reviewer name, the one `Invocation::role` carries.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Risk => RESERVED_RISK,
            Self::Readback => RESERVED_READBACK,
            Self::Readability => RESERVED_READABILITY,
            Self::Reliability => RESERVED_RELIABILITY,
            Self::Lens => RESERVED_LENS,
            Self::Refuter => RESERVED_REFUTER,
            Self::Validator => RESERVED_VALIDATOR,
        }
    }

    /// The role a reserved reviewer name stands for, if the binary can run it.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.name() == name)
    }

    /// What this role answers with.
    pub const fn answer(self) -> Answer {
        match self {
            Self::Risk | Self::Readback | Self::Readability | Self::Reliability | Self::Lens => {
                Answer::Findings
            }
            Self::Refuter => Answer::Outcomes,
            Self::Validator => Answer::Inspection,
        }
    }

    /// Whether this role is one of the five lenses, which answer with findings.
    pub const fn is_lens(self) -> bool {
        matches!(self.answer(), Answer::Findings)
    }

    pub(super) const fn template(self) -> &'static str {
        match self {
            Self::Risk => crate::skill::REVIEW_RISK.contents,
            Self::Readback => crate::skill::REVIEW_READBACK.contents,
            Self::Readability => crate::skill::REVIEW_READABILITY.contents,
            Self::Reliability => crate::skill::REVIEW_RELIABILITY.contents,
            Self::Lens => crate::skill::REVIEW_LENS.contents,
            Self::Refuter => crate::skill::REVIEW_REFUTER.contents,
            Self::Validator => crate::skill::REVIEW_VALIDATOR.contents,
        }
    }
}
