//! What happened to the world, and what may be done about it.
//!
//! Issue-flow's exit-code contract (0/1/2/3/4/5) collapses cases that are
//! distinguishable. A `claim` that expired mid-write, a `transition` whose label
//! landed and whose project projection failed, and a `publish-review` whose push
//! landed but whose readback never converged all leave with `5` — "something
//! went wrong", which is the same answer for "nothing was written" and "something
//! was written and I do not know what".
//!
//! Two axes, kept apart on purpose, because a caller asks two different
//! questions: *did it land?* and *may I run it again?* Collapsing them is what
//! produced the `5`.
//!
//! > When reality presents a state outside the taxonomy, the system reports the
//! > nearest named state, which is a lie told with confidence.
//!
//! So the taxonomy carries [`MutationOutcome::Unknown`] as a first-class answer
//! rather than a rounding error, and every rejection carries a
//! [`Resolution`] — the ratchet: a message may name a command only when running
//! it discharges the block, and when there is no such command it must say which
//! kind of thing is missing instead.

use std::fmt;

use serde::Serialize;

/// Whether the write reached the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationOutcome {
    /// Nothing was sent. The world is exactly as it was.
    NotStarted,
    /// Something was sent and the answer never arrived. The world may or may
    /// not have changed, and this process cannot tell which.
    Unknown,
    /// The write landed and was read back.
    Committed,
}

impl MutationOutcome {
    /// Whether the caller may act as though nothing happened.
    ///
    /// Only [`NotStarted`](Self::NotStarted) earns that. `Unknown` does not, and
    /// the whole point of the enum is that it cannot be quietly folded in.
    pub fn is_clean(self) -> bool {
        matches!(self, Self::NotStarted)
    }
}

/// Whether running the same command again is safe, and if not, what is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Replayability {
    /// Re-running would duplicate or corrupt. Do not.
    NotReplayable,
    /// The operation is idempotent against this tracker; the identical
    /// invocation may be repeated.
    ExactReplaySafe,
    /// Read the current state first, then decide. The retry depends on what is
    /// found there.
    StatusRequired,
    /// No command settles this. A person has to act.
    ManualActionRequired,
}

/// Why no command can honestly be named, from a closed vocabulary.
///
/// The ratchet's escape hatch, and it is deliberately narrow: three reasons, no
/// free text. A rejection that reaches for a fourth is a rejection that has a
/// resolution and has not written it down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoCommandReason {
    /// The operator knows something the program cannot discover — which repo,
    /// which board, which of two equally valid intentions.
    OperatorKnowledge,
    /// The world has to change first: a network, a credential, a service that
    /// is down, a release that is not published yet.
    WorldAction,
    /// A person holds the authority. Approval, a merge nobody delegated, a
    /// decision that is theirs.
    HumanAuthority,
}

impl fmt::Display for NoCommandReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::OperatorKnowledge => "operator-knowledge",
            Self::WorldAction => "world-action",
            Self::HumanAuthority => "human-authority",
        };
        formatter.write_str(text)
    }
}

/// The way out of a rejection.
///
/// > A message may name a command only when running it discharges the block.
/// > Naming a dead end is worse than naming nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    /// An invocation that exists in the real dispatch and, run as written,
    /// clears this rejection.
    Run {
        /// The exact command line, ready to paste.
        command: String,
    },
    /// No command can exist here, and this says which kind of gap it is.
    NoCommand {
        /// Which kind of gap it is, from the closed vocabulary.
        reason: NoCommandReason,
        /// What the operator, the world, or the person has to supply.
        detail: String,
    },
}

impl Resolution {
    /// A resolution that names a runnable invocation.
    pub fn run(command: impl Into<String>) -> Self {
        Self::Run {
            command: command.into(),
        }
    }

    /// A resolution that names why no invocation exists.
    pub fn no_command(reason: NoCommandReason, detail: impl Into<String>) -> Self {
        Self::NoCommand {
            reason,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Run { command } => write!(formatter, "run: {command}"),
            Self::NoCommand { reason, detail } => write!(formatter, "[{reason}] {detail}"),
        }
    }
}

/// A value the gate either established or could not, where "could not" is a
/// shape rather than an absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Established<T> {
    /// The gate read a value.
    Known(T),
    /// The gate could not, and this says why.
    Unestablished {
        /// The reason nothing could be read.
        unestablished: &'static str,
    },
}

/// What the gate read out of one shell command, whether or not it matched
/// anything — the reading behind a boundary refusal, so a wrong stated cause
/// can be checked against what was actually parsed rather than trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandObserved {
    /// The fragment the classifier matched, e.g. `"gh pr merge"`.
    pub matched: Established<String>,
    /// The normalised line it matched against.
    pub against: Established<String>,
    /// The raw single command string from the payload.
    pub literal: Established<String>,
    /// Which list `matched` came from, when it came from the two the crate
    /// distinguishes: `"irreversible"` for the built-in list, or
    /// `"operator-declared"` for a boundary the operator added — the one
    /// distinction `classify_observing`'s `IRREVERSIBLE`-and-`extra` chain
    /// draws that matters to a reader deciding whether the boundary is this
    /// crate's own judgement or the operator's. `None` for every other arm
    /// that sets `matched` — the disarm list, `gh api`, `REPOSITORY_SHELL`
    /// and `writes_a_file` all match against a list too, but none of them
    /// has a second source to distinguish from a first, so there is nothing
    /// this field would tell a reader that `matched` does not already say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary_source: Option<&'static str>,
    /// The byte that kept a `gh pr merge` command from carrying a pull
    /// request identity, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disqualifying_byte: Option<String>,
    /// The built-in irreversible fragments on the line that share a tier with
    /// another such fragment, when one tier holds two or more. Empty when no
    /// tier is overfull, so a line carrying exactly one fragment reads as it
    /// always did.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub same_tier_fragments: Vec<String>,
    /// The paths a write tool names in a patch body, read only when no argument
    /// field names a path — the one place a write tool's target is not a value
    /// of its own. Empty for every shell call.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub write_targets: Vec<String>,
}

/// Which stated candidate a resolved checkout actually came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckoutSource {
    /// The payload's own `cwd`.
    PayloadCwd,
    /// A Bash call's `workdir`.
    PayloadWorkdir,
    /// A write tool's target, narrowed to the directory it lands in.
    WriteTarget,
    /// This process's own launch directory, asked only when nothing else
    /// named a covered checkout.
    LaunchDirectory,
}

/// What the gate read when it resolved which checkout a call belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckoutObserved {
    /// The directory the resolution landed on, in the spelling the gate used
    /// rather than a normalised one — reporting anything else would be
    /// reporting a value the decision was not made on. A reader matching it
    /// against a path of their own should place both first: a launch directory
    /// is a bare `current_dir()`, which answers `/private/var/…` for a
    /// `/var/…` on macOS and the 8.3 `RUNNER~1` form on some Windows hosts.
    pub adjudicated: String,
    /// Which candidate it came from.
    pub source: CheckoutSource,
    /// Why a stated candidate was discarded — every reason established, not
    /// only the last one. A payload can name a `cwd` **and** a `workdir`,
    /// and each can be discarded for its own reason; collapsing that to one
    /// slot silently drops whichever was not written to it last.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dropped: Vec<&'static str>,
}

/// What the gate parsed, beside the rule it applied to what it parsed —
/// present only on a refusal the gate itself produced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Observed {
    /// What the gate read out of the shell command, on a `Bash`-shaped call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<CommandObserved>,
    /// What the gate read while resolving which checkout the call belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<CheckoutObserved>,
    /// The local and remote refs of each line a `git push` names on the
    /// pre-push hook's stdin. Empty whenever the caller had none to give, which
    /// a push is not allowed to read as "no ref is outside the claim"; a remote
    /// the caller did not know is the empty string.
    #[serde(skip)]
    pub push_refs: Vec<(String, String)>,
}

impl Observed {
    /// Whether neither half was ever set — a reading that established
    /// nothing, distinct from one that did and simply had no command or
    /// checkout worth naming. A caller attaching this is attaching nothing,
    /// and should say so by not attaching it at all.
    pub fn is_empty(&self) -> bool {
        self.command.is_none() && self.checkout.is_none()
    }
}

/// Why a refusal happened, in the one word that says what the caller does next.
///
/// Four values, because four different next moves exist and a caller that cannot
/// tell them apart reads an outage as a verdict: read again, re-read, stop, or
/// go and find out what the world holds. The rules for placing a code are in
/// `docs/refusal-classes.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefusalClass {
    /// The control surface could not be read. Nothing was decided; the answer
    /// is not a state and never clearance.
    Unavailable,
    /// The surface was read and what the run was acting on has moved: a
    /// receipt, a head or an authority. Re-read before deciding anything.
    Stale,
    /// The surface was read and the rule says no. Repeating the command
    /// repeats the answer.
    Refused,
    /// A write may or may not have landed. Not a named state of any kind: look
    /// before acting.
    Unknown,
}

impl RefusalClass {
    /// The kebab-case word `--json` carries.
    pub fn word(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Stale => "stale",
            Self::Refused => "refused",
            Self::Unknown => "unknown",
        }
    }

    /// What an agent is told the class means, shared by every text surface.
    pub fn meaning(self) -> &'static str {
        match self {
            Self::Unavailable => "the control surface could not be read; this is not a state",
            Self::Stale => "what this run acted on has moved; re-read before deciding",
            Self::Refused => "the rule says no; repeating the command repeats the answer",
            Self::Unknown => "the outcome cannot be established; look before acting",
        }
    }
}

/// A refusal that carries everything the caller needs to act on it.
///
/// Six fields are the whole contract: a typed code to match on, prose for a
/// person, what happened to the world, whether running the same command again
/// is safe, what to do, and which class of answer this is. None of them is optional, which is the mechanism —
/// a rejection cannot be constructed without deciding whether a command
/// exists. `observed` is additional and never part of that contract: `None`
/// is a valid answer, and stands for a refusal built without the gate's own
/// reading behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    /// A stable kebab-case code. Matched on by callers and by tests; never
    /// reworded to improve the prose.
    pub code: &'static str,
    /// What went wrong, in a sentence, for a person.
    pub message: String,
    /// Whether the write reached the world.
    pub outcome: MutationOutcome,
    /// Whether running the same command again is safe.
    pub replay: Replayability,
    /// The way out.
    pub resolution: Resolution,
    /// Whether the surface could not be read, what was acted on moved, the rule
    /// said no, or the outcome is unknown.
    pub class: RefusalClass,
    /// What the gate parsed on its way to this refusal, when it produced one.
    ///
    /// Boxed rather than inline: `Observed` carries the raw command line and
    /// several other strings, and inlining it here pushed every `Result<T,
    /// Refusal>` in the crate — most of which never carry one — past
    /// clippy's `result_large_err` floor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<Box<Observed>>,
}

impl Refusal {
    /// A refusal that never touched the world — the common case for a
    /// configuration or argument defect. The class is the one
    /// `harness::refusal_class` holds for `code`, and `unavailable` for a code it
    /// does not name: an unnamed refusal is never reported as the rule saying no.
    pub fn not_started(
        code: &'static str,
        message: impl Into<String>,
        resolution: Resolution,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            outcome: MutationOutcome::NotStarted,
            replay: Replayability::ExactReplaySafe,
            resolution,
            class: crate::harness::refusal_class::class_of_code(code),
            observed: None,
        }
    }

    /// A refusal because what the run acted on has moved; nothing was written.
    pub fn stale(code: &'static str, message: impl Into<String>, resolution: Resolution) -> Self {
        Self::not_started(code, message, resolution).classed(RefusalClass::Stale)
    }

    /// A refusal because the control surface could not be read; nothing was
    /// written and nothing was decided.
    pub fn unavailable(
        code: &'static str,
        message: impl Into<String>,
        resolution: Resolution,
    ) -> Self {
        Self::not_started(code, message, resolution).classed(RefusalClass::Unavailable)
    }

    /// Sets the class explicitly.
    ///
    /// `unknown` and an unknown outcome are one fact, so choosing the class
    /// sets the outcome and the replay advice that go with it; a refusal cannot
    /// say "nothing was written" under `Class: unknown`.
    pub fn classed(self, class: RefusalClass) -> Self {
        if class == RefusalClass::Unknown {
            return Self {
                class,
                outcome: MutationOutcome::Unknown,
                replay: Replayability::StatusRequired,
                ..self
            };
        }
        Self { class, ..self }
    }

    /// The text an agent is shown on the hook and tool-server surfaces: code,
    /// class, outcome, replay and resolution, once for both.
    ///
    /// `noun` is what a retry would repeat, as [`Replayability::advice`] takes it.
    pub fn agent_text(&self, noun: &str) -> String {
        format!(
            "{}\n({})\n\nClass: {} \u{2014} {}\nWhat happened to the world: {}\nReplay: {}\n\n{}",
            self.message,
            self.code,
            self.class.word(),
            self.class.meaning(),
            self.outcome.what_happened(),
            self.replay.advice(noun),
            self.resolution,
        )
    }

    /// Attaches what the gate parsed on its way to this refusal.
    pub fn observing(self, observed: Observed) -> Self {
        Self {
            observed: Some(Box::new(observed)),
            ..self
        }
    }

    /// What happened to the world, in a sentence, when it is worth a line.
    ///
    /// Silent for [`MutationOutcome::NotStarted`]: a refusal already implies
    /// nothing happened, and a line saying so on every one of them is a line
    /// people stop reading. The words are [`MutationOutcome::what_happened`]'s
    /// — one mapping across every surface that shows the outcome.
    fn what_happened(&self) -> Option<&'static str> {
        match self.outcome {
            MutationOutcome::NotStarted => None,
            outcome => Some(outcome.what_happened()),
        }
    }

    /// Whether running it again is safe, when the answer is not "yes".
    ///
    /// The words are [`Replayability::advice`]'s — the same mapping an agent
    /// reads, not a second copy of it.
    fn about_retrying(&self) -> Option<String> {
        match self.replay {
            // The one difference kept, and it is presentation rather than
            // vocabulary: a line saying a command is safe to repeat, printed on
            // every refusal that is, is a line people stop reading.
            Replayability::ExactReplaySafe => None,
            replay => Some(replay.advice("command")),
        }
    }
}

/// The two axes, then the way out.
impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.message, self.code)?;
        // Both halves are the shared vocabulary's words, borrowed rather than
        // spelled again here.
        if let Some(line) = self.what_happened() {
            write!(formatter, "\n  {line}")?;
        }
        if let Some(line) = self.about_retrying() {
            write!(formatter, "\n  {line}")?;
        }
        write!(formatter, "\n  {}", self.resolution)
    }
}

impl std::error::Error for Refusal {}

/// The exit codes the binary uses.
///
/// Deliberately few, and none of them means "something went wrong". The
/// distinctions live in [`Refusal`], where they can be read; the exit code only
/// has to tell a shell whether to continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    /// The command did what it said.
    Success = 0,
    /// The command refused. Whether anything had already been written is
    /// [`Refusal::outcome`]'s to say, not this code's.
    Refused = 1,
    /// The command was interrupted and the world may have changed. This is the
    /// code that must never be reused for anything else.
    Indeterminate = 2,
    /// The invocation could not be read: nothing was attempted, and **nothing
    /// was decided**.
    ///
    /// Apart from [`Self::Refused`] because the difference is the whole reason
    /// the hooks read a status at all: `Refused` is a decision — the world said
    /// no — and every script this crate writes propagates it. A usage error is
    /// not a decision, and propagating it blocks a push for a reason the person
    /// typing it cannot act on.
    ///
    /// `3` rather than a fourth word, because the readers already handle it:
    /// the hook script and the OpenCode plugin treat anything outside `0`,
    /// `1` and `2` as *it did not answer*, say so, and let the write through.
    /// The Pi extension blocks on it instead.
    Unreadable = 3,
    /// `estigia gate` put the decision to a person, for a caller that said with
    /// `--can-ask` that it can carry the question and the answer.
    ///
    /// Never answered to a caller that did not say so: a reader that knows
    /// only `0`, `1` and `2` would take it for a gate that did not answer and
    /// let the write through.
    Asked = 4,
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(code: ExitCode) -> Self {
        Self::from(code as u8)
    }
}

impl MutationOutcome {
    /// What an agent is told happened to the world.
    ///
    /// **One mapping** for every surface that shows a refusal — hook, tool
    /// server, or the message a person reads — because a word changed in one is
    /// two surfaces describing one outcome differently, to a reader who reads
    /// both.
    pub fn what_happened(self) -> &'static str {
        match self {
            Self::NotStarted => "nothing was written",
            Self::Unknown => "a write may have landed and cannot be confirmed",
            Self::Committed => "the write landed; what failed came after it",
        }
    }
}

impl Replayability {
    /// What an agent is told about doing it again.
    ///
    /// `noun` is the one axis on which the two surfaces legitimately differ: a
    /// hook refuses a *command* and the tool server refuses a *call*, and they
    /// said so — the only difference between the two copies, and the reason
    /// this takes a parameter rather than being a constant.
    pub fn advice(self, noun: &str) -> String {
        match self {
            Self::NotReplayable => format!("do not repeat this {noun}"),
            Self::ExactReplaySafe => format!("the same {noun} may be repeated"),
            Self::StatusRequired => "read the current state first".to_owned(),
            Self::ManualActionRequired => "a person has to act".to_owned(),
        }
    }
}

/// The exit code a refusal deserves, derived rather than chosen at each site.
pub fn exit_code_for(refusal: &Refusal) -> ExitCode {
    match refusal.outcome {
        MutationOutcome::NotStarted => ExitCode::Refused,
        MutationOutcome::Unknown => ExitCode::Indeterminate,
        // A committed write is not a refusal; if one is ever built this way the
        // caller is better served by the indeterminate code than by a zero.
        MutationOutcome::Committed => ExitCode::Indeterminate,
    }
}

#[cfg(test)]
mod tests {

    /// One outcome, one sentence, whichever surface asks. Crossed rather than
    /// pinned to a string: what matters is that the two cannot drift again.
    #[test]
    fn every_outcome_says_the_same_thing_to_a_person_as_to_an_agent() {
        for outcome in [
            super::MutationOutcome::NotStarted,
            super::MutationOutcome::Unknown,
            super::MutationOutcome::Committed,
        ] {
            let refusal = super::Refusal {
                code: "a-code",
                message: "a message".into(),
                outcome,
                replay: super::Replayability::StatusRequired,
                resolution: super::Resolution::run("estigia status"),
                class: super::RefusalClass::Refused,
                observed: None,
            };
            match refusal.what_happened() {
                // The one difference that is a display choice rather than a
                // vocabulary: a refusal already implies nothing happened.
                None => assert_eq!(
                    outcome,
                    super::MutationOutcome::NotStarted,
                    "an outcome worth a line was not given one"
                ),
                Some(said) => assert_eq!(
                    said,
                    outcome.what_happened(),
                    "the sentence a person reads for {outcome:?} is not the one an agent reads"
                ),
            }
        }

        // The floor: the shared mapping distinguishes the three, or the
        // assertion above would hold against one sentence for everything.
        let said: std::collections::BTreeSet<&str> = [
            super::MutationOutcome::NotStarted,
            super::MutationOutcome::Unknown,
            super::MutationOutcome::Committed,
        ]
        .iter()
        .map(|outcome| outcome.what_happened())
        .collect();
        assert_eq!(said.len(), 3, "two outcomes are described the same way");

        // And the sentence for a write that may or may not have landed does not
        // claim a request went anywhere: `setup` uses it for a file.
        assert!(
            !super::MutationOutcome::Unknown
                .what_happened()
                .contains("sent"),
            "the unknown outcome is described as a message that was sent, which a local write is not"
        );

        // The other axis, which had the same defect: both surfaces answered
        // *may I run this again?* with two different kinds of fact.
        for replay in [
            super::Replayability::NotReplayable,
            super::Replayability::ExactReplaySafe,
            super::Replayability::StatusRequired,
            super::Replayability::ManualActionRequired,
        ] {
            let refusal = super::Refusal {
                code: "a-code",
                message: "a message".into(),
                outcome: super::MutationOutcome::Unknown,
                replay,
                resolution: super::Resolution::run("estigia status"),
                class: super::RefusalClass::Refused,
                observed: None,
            };
            match refusal.about_retrying() {
                None => assert_eq!(
                    replay,
                    super::Replayability::ExactReplaySafe,
                    "a retry answer worth a line was not given one"
                ),
                Some(said) => assert_eq!(
                    said,
                    replay.advice("command"),
                    "the retry advice a person reads for {replay:?} is not the one an agent reads"
                ),
            }
        }

        // The same floor, on the second axis: four states, four answers.
        let advised: std::collections::BTreeSet<String> = [
            super::Replayability::NotReplayable,
            super::Replayability::ExactReplaySafe,
            super::Replayability::StatusRequired,
            super::Replayability::ManualActionRequired,
        ]
        .iter()
        .map(|replay| replay.advice("command"))
        .collect();
        assert_eq!(
            advised.len(),
            4,
            "two retry answers are worded the same way"
        );
    }

    use super::*;

    #[test]
    fn unknown_is_not_clean() {
        // The whole reason the enum exists: "I do not know" must not be
        // reachable from the same branch as "nothing happened".
        assert!(MutationOutcome::NotStarted.is_clean());
        assert!(!MutationOutcome::Unknown.is_clean());
        assert!(!MutationOutcome::Committed.is_clean());
    }

    #[test]
    fn the_class_is_in_the_json_and_in_the_text_an_agent_reads() {
        for (class, word) in [
            (RefusalClass::Unavailable, "unavailable"),
            (RefusalClass::Stale, "stale"),
            (RefusalClass::Refused, "refused"),
            (RefusalClass::Unknown, "unknown"),
        ] {
            let refusal =
                Refusal::not_started("a-code", "a message", Resolution::run("estigia status"))
                    .classed(class);
            let json = serde_json::to_value(&refusal).expect("serialises");
            assert_eq!(json["class"], word);
            let text = refusal.agent_text("call");
            assert!(text.contains(&format!("Class: {word} ")), "{text}");
        }
    }

    #[test]
    fn an_unknown_outcome_never_exits_zero() {
        let refusal = Refusal {
            code: "readback-did-not-converge",
            message: "the push landed and the readback never agreed".into(),
            outcome: MutationOutcome::Unknown,
            replay: Replayability::StatusRequired,
            resolution: Resolution::run("estigia status --issue 12"),
            class: RefusalClass::Unknown,
            observed: None,
        };
        assert_eq!(exit_code_for(&refusal), ExitCode::Indeterminate);
    }

    #[test]
    fn a_refusal_renders_its_resolution() {
        let refusal = Refusal::not_started(
            "tracker-not-configured",
            "no tracker is configured for this repository",
            Resolution::run("estigia setup"),
        );
        let rendered = refusal.to_string();
        assert!(rendered.contains("tracker-not-configured"));
        assert!(rendered.contains("run: estigia setup"));
    }

    #[test]
    fn a_missing_resolution_names_which_kind_of_gap_it_is() {
        let refusal = Refusal::not_started(
            "profile-custom",
            "the three authorisations match no profile",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "which posture this repository wants",
            ),
        );
        assert!(refusal.to_string().contains("[operator-knowledge]"));
    }
}

#[cfg(test)]
mod message_shape {
    /// No message reaches a person with a run of spaces in it.
    ///
    /// A line continuation written as `\` instead of `\` is an escaped
    /// backslash, not a continuation, and `cargo fmt` then folds the source
    /// indentation into the string.
    #[test]
    fn no_message_carries_the_source_indentation() {
        // Built rather than written: a literal run of spaces in this file would
        // be found by the guard itself.
        let run = " ".repeat(6);

        for (path, source) in sources() {
            // The translation table is not messages. It carries the help page,
            // which is a hanging-indent column layout on purpose — the same
            // shape this guard exists to call a defect, and here it is the
            // point. Exempted by name rather than by a heuristic, because a
            // heuristic wide enough to let this through would let a folded
            // refusal through with it.
            if path == "tui/words.rs" {
                continue;
            }
            for (number, line) in source.lines().enumerate() {
                let trimmed = line.trim_start();
                for content in string_literals(trimmed) {
                    // Deliberate column padding sits at the edge of a string; a
                    // folded continuation has words on both sides of the gap.
                    // Maximal runs, not fixed-width ones: `match_indices` does
                    // not overlap, so against a wide gap a fixed-width match
                    // landed *inside* it and the defect went through.
                    let gaps = content.match_indices(&run).map(|(at, _)| {
                        let start = content[..at]
                            .rfind(|character: char| character != ' ')
                            .map_or(0, |at| at + 1);
                        let end = at
                            + content[at..]
                                .find(|character: char| character != ' ')
                                .unwrap_or(content.len() - at);
                        (start, end)
                    });
                    let folded = gaps.into_iter().any(|(start, end)| {
                        let before = content[..start].chars().next_back();
                        let after = content[end..].chars().next();
                        // Prose on both sides is a folded continuation. A gap
                        // that leads into a `{}` placeholder is a column being
                        // padded, and flagging that would make the guard
                        // demand its own false positives be "fixed".
                        before.is_some_and(char::is_alphanumeric)
                            && after.is_some_and(char::is_alphabetic)
                    });
                    assert!(
                        !folded,
                        "{path}:{} folds source indentation into a message:
{trimmed}",
                        number + 1
                    );
                }
            }
        }
    }

    /// The contents of every double-quoted literal on one line.
    fn string_literals(line: &str) -> Vec<&str> {
        let mut found = Vec::new();
        let bytes = line.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'"' {
                index += 1;
                continue;
            }
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b'"' {
                end += if bytes[end] == b'\\' { 2 } else { 1 };
            }
            if end <= bytes.len()
                && line.is_char_boundary(start)
                && line.is_char_boundary(end.min(bytes.len()))
            {
                found.push(&line[start..end.min(bytes.len())]);
            }
            index = end + 1;
        }
        found
    }

    /// Every Rust source in the crate, found rather than listed — a list beside
    /// the code is a list that stops matching it.
    ///
    /// The ratchet is enforced on **resolutions** — the line under a refusal —
    /// and a command named in the message itself makes the same promise to the
    /// same reader, so it is read out of the source too.
    #[test]
    fn a_command_named_in_prose_is_a_command_that_runs() {
        let mut named: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (source, text) in sources() {
            for line in text.lines() {
                let mut rest = line;
                while let Some(at) = rest.find("`estigia ") {
                    rest = &rest[at + 1..];
                    let Some(end) = rest.find('`') else { break };
                    let command = rest[..end].trim();
                    if command
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == ' ' || c == '-')
                    {
                        named.insert(format!("{command}\u{1}{source}"));
                    }
                }
            }
        }

        let mut checked = 0;
        for entry in &named {
            let (command, source) = entry.split_once('\u{1}').expect("the pair");
            let argv: Vec<&str> = command.split_whitespace().collect();
            checked += 1;
            if let Err(error) = <crate::cli::Cli as clap::Parser>::try_parse_from(&argv) {
                use clap::error::ErrorKind;
                // What this asks is that the **verb exists**, not that the
                // sentence is a complete invocation. Prose names a command the
                // way English does — *"until `estigia claim` has been run"* —
                // and demanding its arguments there would forbid the ordinary
                // way to refer to a verb. A missing argument means clap knew
                // the verb; an unknown subcommand or flag means it did not, and
                // that is the dead end the ratchet forbids.
                assert!(
                    !matches!(
                        error.kind(),
                        ErrorKind::InvalidSubcommand
                            | ErrorKind::UnknownArgument
                            | ErrorKind::NoEquals
                    ),
                    "`{command}`, named in {source}, is not a command this binary has: {error}"
                );
            }
        }
        // The floor: a scan that found nothing would pass in silence.
        assert!(
            checked >= 10,
            "only {checked} commands were read out of the source, so this scanned almost nothing"
        );
    }

    fn sources() -> Vec<(String, String)> {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            let entries = std::fs::read_dir(&directory).unwrap_or_else(|error| {
                panic!("{} must be readable: {error}", directory.display())
            });
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_some_and(|kind| kind == "rs") {
                    let name = path
                        .strip_prefix(&root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/");
                    let text = std::fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("{} must be readable: {error}", name));
                    found.push((name, crate::source_text::code_of(&text)));
                }
            }
        }
        found.sort();
        assert!(
            found.len() > 20,
            "the crate has {} sources; the walk is not reading the tree",
            found.len()
        );
        found
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;

    fn refusal(outcome: MutationOutcome, replay: Replayability) -> Refusal {
        Refusal {
            code: "a-code",
            message: "a sentence".to_owned(),
            outcome,
            replay,
            resolution: Resolution::run("estigia status"),
            class: RefusalClass::Refused,
            observed: None,
        }
    }

    #[test]
    fn a_refusal_whose_write_landed_does_not_read_like_one_that_wrote_nothing() {
        // This module's header: both axes were carried and serialised into
        // `--json`, and neither was ever printed — a person got the collapsed
        // answer this taxonomy replaced.
        let nothing = format!(
            "{}",
            refusal(MutationOutcome::NotStarted, Replayability::ExactReplaySafe)
        );
        let landed = format!(
            "{}",
            refusal(MutationOutcome::Committed, Replayability::NotReplayable)
        );
        let unknown = format!(
            "{}",
            refusal(MutationOutcome::Unknown, Replayability::StatusRequired)
        );
        assert_ne!(nothing, landed);
        assert_ne!(nothing, unknown);
        assert_ne!(landed, unknown);

        // And each says the thing that changes what to do next.
        assert!(landed.contains("the write landed"), "{landed}");
        // `Replayability::advice`'s words, since this surface stopped keeping
        // its own copy of them.
        assert!(landed.contains("do not repeat"), "{landed}");
        // The words are `MutationOutcome::what_happened`'s, for the same
        // reason.
        assert!(unknown.contains("may have landed"), "{unknown}");
        assert!(unknown.contains("read the current state"), "{unknown}");

        // Nothing having happened is what a refusal already implies, so it
        // earns no line.
        assert_eq!(nothing.lines().count(), 2, "{nothing}");
        assert!(nothing.contains("a sentence (a-code)"));

        // The way out is still last, whatever came before it.
        for said in [&nothing, &landed, &unknown] {
            assert!(
                said.lines()
                    .next_back()
                    .is_some_and(|last| last.contains("estigia status")),
                "the resolution is not the last thing said: {said}"
            );
        }
    }

    #[test]
    fn observed_is_absent_rather_than_null_when_a_refusal_carries_none() {
        // A refusal built through `not_started` never touches the gate's own
        // reading, and most refusals in this crate are exactly that — a
        // configuration or argument defect nothing was ever parsed against.
        let refusal =
            Refusal::not_started("a-code", "a sentence", Resolution::run("estigia status"));
        let value = serde_json::to_value(&refusal).expect("a refusal serialises");
        assert!(
            value.get("observed").is_none(),
            "a refusal with no observed reading serialised a null rather than omitting the key: \
             {value}"
        );
    }

    #[test]
    fn attaching_observed_does_not_change_what_a_person_reads() {
        let plain = refusal(MutationOutcome::NotStarted, Replayability::ExactReplaySafe);
        let carrying = plain.clone().observing(Observed {
            command: Some(CommandObserved {
                matched: Established::Known("gh pr merge".to_owned()),
                against: Established::Known("gh pr merge 1".to_owned()),
                literal: Established::Known("gh pr merge 1".to_owned()),
                boundary_source: Some("irreversible"),
                disqualifying_byte: None,
                same_tier_fragments: Vec::new(),
                write_targets: Vec::new(),
            }),
            checkout: Some(CheckoutObserved {
                adjudicated: "/a/checkout".to_owned(),
                source: CheckoutSource::LaunchDirectory,
                dropped: Vec::new(),
            }),
            push_refs: Vec::new(),
        });
        assert_eq!(
            plain.to_string(),
            carrying.to_string(),
            "the human sentence changed when an observed reading was attached"
        );
    }
}
