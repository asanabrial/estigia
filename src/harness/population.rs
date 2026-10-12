//! The vocabulary a guard's declared population is written in.
//!
//! A [`Population`] const states, for one guard, what it is supposed to catch,
//! how it is wrong on purpose, and which item holds the rule. This file holds
//! the type and nothing else: each const sits beside the code it describes, in
//! the module that owns the rule.
//! `tests/guards.rs` walks `src/` for them, resolves each `subject` to the item
//! it names, and fingerprints the two together — so this is a vocabulary a
//! person reads and a compiler checks, rather than a comment a test scrapes out
//! of a doc attribute.
//!
//! Every one of those consts is `pub` and none is called from anywhere: in a
//! library target that is not dead code, it is a published statement of what
//! each guard covers. `cargo doc --no-deps` puts it in front of a reader the
//! way the doc comment it replaced never was, since a doc comment on a
//! `pub(crate)` or private item does not publish at all.

/// The three ways a population can be wrong, and nothing else.
///
/// A closed vocabulary for the same reason the rejection reasons are one: a
/// disposition invented for one guard is a disposition nobody can compare
/// against another. An enum makes that a compile error instead of a string a
/// test has to catch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// The guard catches less than its stated population: a real escape
    /// exists and is named rather than hidden.
    TooTight,
    /// The guard catches more than its stated population: a false positive
    /// costs a read or a refusal, but nothing slips through.
    TooLoose,
    /// The guard treats an input outside its stated population as the stop
    /// arm rather than as clearance.
    FailClosed,
}

impl Disposition {
    /// The kebab-case spelling used in messages and in the fingerprint input.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TooTight => "too-tight",
            Self::TooLoose => "too-loose",
            Self::FailClosed => "fail-closed",
        }
    }
}

/// One guard's declared population: what it is supposed to catch, how it is
/// wrong on purpose, and the item in the same file as the declaration that
/// holds the rule.
///
/// `tests/guards.rs` fingerprints `text` together with `subject`'s own
/// tokens, so changing either the population's boundary or the code beneath
/// it reopens the claim — the mechanism this type carries is described there,
/// not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Population {
    /// A short kebab-case name for the guard this declares.
    pub family: &'static str,
    /// How this population is deliberately wrong.
    pub disposition: Disposition,
    /// The item in the same file as the declaration that holds the rule,
    /// named without its kind keyword (`WRITE_TOOLS`, not `const
    /// WRITE_TOOLS`).
    pub subject: &'static str,
    /// The population and its boundary, as written.
    pub text: &'static str,
}

/// The type ascription that marks a `Population` declaration in source text.
///
/// Two counters — `tests/guards.rs`'s `population_text_occurrences` and
/// `tests/honesty.rs`'s declared-population count — used to spell this
/// independently, once as `": Population ="` and once as `": Population =
/// Population {"`. The second missed a declaration `rustfmt` breaks across
/// lines, so one more `Population` could exist while the shorter spelling's
/// count still matched the documented one. One constant, used by both counters over
/// the whole stripped file text rather than per line, closes that gap.
///
/// Built with `concat!` rather than written as one literal: both counters walk
/// every `.rs` file in `src/`, this one included, and a literal spelling this
/// exact ascription would be counted as an occurrence of itself the moment the
/// walk reached this file — a false declaration nothing declared. Split across
/// three pieces, the raw source never holds the contiguous substring the
/// counters search for, and `concat!` still resolves to the one value both of
/// them match against.
pub const DECLARATION_ASCRIPTION: &str = concat!(":", " Population", " =");
