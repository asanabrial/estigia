//! What happened to the world, and what may be done about it.
//!
//! Issue-flow's exit-code contract (0/1/2/3/4/5) collapses cases that are
//! distinguishable. A `claim` that expired mid-write, a `transition` whose label
//! landed and whose board mirror failed, and a `publish-review` whose push
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

/// A refusal that carries everything the caller needs to act on it.
///
/// The four fields are the whole contract: a typed code to match on, prose for
/// a person, what happened to the world, and what to do. None of them is
/// optional, which is the mechanism — a rejection cannot be constructed without
/// deciding whether a command exists.
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
}

impl Refusal {
    /// A refusal that never touched the world — the common case for a
    /// configuration or argument defect.
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
        }
    }

    /// What happened to the world, in a sentence, when it is worth a line.
    ///
    /// Silent for [`MutationOutcome::NotStarted`]: nothing happening is what a
    /// refusal already implies, and a line saying so on every one of them is a
    /// line people stop reading. The other two are the ones that change what to
    /// do next, and they were reaching nobody.
    ///
    /// The **words** are [`MutationOutcome::what_happened`]'s, not this
    /// function's. That mapping's own documentation says *one mapping*, having
    /// been consolidated across the two surfaces an agent reads — and this, the
    /// surface a **person** reads, was a third copy saying something different
    /// about the same outcome.
    ///
    /// Different, and for half its callers false. It answered `Unknown` with
    /// *"something was sent and no answer came back"*, which is written for a
    /// tracker call; `setup` wraps a failed local install in the same outcome,
    /// so an operator whose `~/.claude` was an ordinary **file** was told a
    /// request had gone out and might have landed, about a directory their own
    /// disk had refused to create. Two states forced through the binary said it
    /// in those words. What is shared says *a write may have landed and cannot
    /// be confirmed*, which is true of a `gh` call and of a file alike.
    fn what_happened(&self) -> Option<&'static str> {
        match self.outcome {
            MutationOutcome::NotStarted => None,
            outcome => Some(outcome.what_happened()),
        }
    }

    /// Whether running it again is safe, when the answer is not "yes".
    ///
    /// The **words** are [`Replayability::advice`]'s, for the reason the line
    /// above this one gives about the other axis: the same four states were
    /// spelled twice, once for a person and once for an agent, and one of the
    /// four did not merely differ in wording. `ManualActionRequired` told a
    /// person *no command settles this* — which is about Estigia's vocabulary —
    /// and an agent *a person has to act*, which is about who has to do
    /// something next. Two surfaces answering *may I run this again?* with two
    /// different kinds of fact.
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
///
/// This module exists because issue-flow's exit codes "collapse cases that are
/// distinguishable... the same answer for 'nothing was written' and 'something
/// was written and I do not know what'" — and its own header says why they are
/// kept apart: "a caller asks two different questions: *did it land?* and *may I
/// run it again?*"
///
/// Both were carried, both were serialised into `--json`, and neither was ever
/// printed. So the person at the terminal — the one reader who cannot ask a
/// follow-up question — got exactly the collapsed answer the taxonomy was built
/// to replace, while a program got both.
impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.message, self.code)?;
        // Both halves are the shared vocabulary's words, so one is borrowed and
        // the other owned. Written one after the other rather than made to
        // agree on a type, which is where a second copy would go back.
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
    /// The command refused, and nothing was written.
    Refused = 1,
    /// The command was interrupted and the world may have changed. This is the
    /// code that must never be reused for anything else.
    Indeterminate = 2,
    /// The invocation could not be read: nothing was attempted, and **nothing
    /// was decided**.
    ///
    /// Apart from [`Self::Refused`] because the difference is the whole reason
    /// the hooks read a status at all. `Refused` is a decision — the world
    /// said no — and every script this crate writes propagates it, which is
    /// what blocks a push. A usage error is not a decision, and propagating it
    /// blocks a push for a reason the person typing it cannot act on.
    ///
    /// Measured: a `pre-push` hook left from a build whose `hook` took one more
    /// flag exited `1`, and `git push` came back `error: unexpected argument
    /// --from-a-newer-build` with the push refused. That is the failure
    /// `guard::script` exists to prevent, arriving through the one code it
    /// trusts.
    ///
    /// `3` rather than a fourth word, because both readers already handle it:
    /// the hook script and the OpenCode plugin treat anything outside `0`,
    /// `1` and `2` as *it did not answer*, say so, and let the write through.
    Unreadable = 3,
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(code: ExitCode) -> Self {
        Self::from(code as u8)
    }
}

impl MutationOutcome {
    /// What an agent is told happened to the world.
    ///
    /// **One mapping.** This was written out at both surfaces that show a
    /// refusal to an agent — the hook's denial and the tool server's error —
    /// and the second one's own comment says they carry *"the same four things,
    /// because an agent reading either one needs the same four things"*, while
    /// holding a second copy of what those things say. A word changed in one is
    /// two surfaces describing one outcome differently, to the same reader.
    ///
    /// Three surfaces now: `Refusal::what_happened`, the one a person reads,
    /// was the copy left out of that consolidation. It is folded in here, and
    /// the half-sentence it carried that this one did not — *what failed came
    /// after it* — is kept, because it is true wherever `Committed` appears: a
    /// refusal carrying it is by definition one whose failure arrived after the
    /// write.
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

    /// One outcome, one sentence, whichever surface asks.
    ///
    /// Three surfaces show what happened to the world: the hook's denial and
    /// the tool server's error, which an agent reads, and the refusal a person
    /// reads on standard error. The first two were consolidated onto
    /// `MutationOutcome::what_happened` and the third was left holding its own
    /// copy — which said, for `Unknown`, *something was sent and no answer came
    /// back*. `setup` reports a failed local install with that outcome, so an
    /// operator whose `~/.claude` was an ordinary file was told a request had
    /// gone out about a directory their own disk had refused to create.
    ///
    /// Crossed rather than pinned to a string: what matters is that the two
    /// cannot drift again, not which words win.
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

        // The other axis, which had the same defect and one worse case. Both
        // surfaces answer *may I run this again?*, and for
        // `ManualActionRequired` they answered with two different kinds of
        // fact: a person was told *no command settles this*, about Estigia's
        // vocabulary, and an agent *a person has to act*, about who moves next.
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
    fn an_unknown_outcome_never_exits_zero() {
        let refusal = Refusal {
            code: "readback-did-not-converge",
            message: "the push landed and the readback never agreed".into(),
            outcome: MutationOutcome::Unknown,
            replay: Replayability::StatusRequired,
            resolution: Resolution::run("estigia status --issue 12"),
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
            "board-unknown",
            "two project boards match and neither is recorded",
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "which board this repository projects onto",
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
    /// indentation into the string. It compiles, the tests pass, and the agent
    /// reads `for it,                      and its operations` — which is the
    /// kind of defect that only ever shows up in the one place it matters.
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
                if trimmed.starts_with("//") {
                    continue;
                }
                for content in string_literals(trimmed) {
                    // Deliberate column padding sits at the edge of a string;
                    // a folded continuation has words on both sides of the gap.
                    // Only the second is a defect, and telling them apart is
                    // what keeps this guard from being switched off.
                    // Maximal runs, not fixed-width ones. `match_indices` does
                    // not overlap, so against a fourteen-space gap the second
                    // match landed *inside* it: the character before was a
                    // space, the test failed, and the defect went through. The
                    // guard caught gaps of six to eleven spaces and nothing
                    // wider — which is to say it missed the badly folded lines
                    // most reliably.
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

    /// Every Rust source in the crate, found rather than listed.
    ///
    /// This was twelve `include_str!` entries, and `skill.rs` was not one of
    /// them — so a message folded into the contract the agent reads went
    /// straight through the guard whose whole job is that. A list of the files a
    /// rule applies to is the rule quietly applying to fewer things than it
    /// says, which is the shape this crate keeps finding.
    ///
    /// Read at run time rather than embedded, because `include_str!` needs a
    /// literal and a literal is the list all over again.
    /// A command named in prose is a command that runs.
    ///
    /// The ratchet is enforced on **resolutions** — the line under a refusal — and
    /// a command named in the message itself makes the same promise to the same
    /// reader. `estigia config set --repo` was written into one of those a few
    /// rounds ago; nothing checked it, and the check that would have was looking
    /// somewhere else.
    ///
    /// Read out of the source rather than listed here, for the reason the
    /// resolution inventory gives about its own list: a list beside the code is a
    /// list that stops matching it.
    #[test]
    fn a_command_named_in_prose_is_a_command_that_runs() {
        let mut named: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (source, text) in sources() {
            for line in text.lines() {
                // Comments are prose about the code, not prose the operator reads.
                if line.trim_start().starts_with("//") {
                    continue;
                }
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
        // The floor: a scan that found nothing would pass in silence, and it did
        // find nothing on the first attempt — the pattern was looking for the wrong
        // quoting.
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
                    found.push((name, text));
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
        }
    }

    #[test]
    fn a_refusal_whose_write_landed_does_not_read_like_one_that_wrote_nothing() {
        // This module's header: issue-flow's exit codes gave "the same answer
        // for 'nothing was written' and 'something was written and I do not
        // know what'", and the two axes are kept apart because "a caller asks
        // two different questions: *did it land?* and *may I run it again?*"
        //
        // Both axes were carried on every refusal and both were serialised into
        // `--json`. Neither was ever printed. So a person at a terminal — the
        // one reader who cannot ask a follow-up question — was handed exactly
        // the collapsed answer this taxonomy replaced, and would retry a
        // command whose write had already landed.
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
        // its own copy of them. What is pinned is the same: do not repeat it.
        assert!(landed.contains("do not repeat"), "{landed}");
        // The words are `MutationOutcome::what_happened`'s since the surface a
        // person reads stopped keeping its own copy of them. What this pins is
        // unchanged: the line says the write may have landed.
        assert!(unknown.contains("may have landed"), "{unknown}");
        assert!(unknown.contains("read the current state"), "{unknown}");

        // Nothing having happened is what a refusal already implies, so it earns
        // no line: a sentence printed on every refusal is one people stop
        // reading, and then the two that matter go with it.
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
}
