//! The commands that are a person's decision and never an agent's.

use super::Population;
use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// One command only a person may run, and what the gate says when an agent's line does.
pub struct PersonOnly {
    /// The word after `estigia` that names the command.
    pub verb: &'static str,
    /// The target the classifier gives a line that runs it, which is what the deny reads.
    pub target: &'static str,
    /// The stable code of the refusal.
    pub code: &'static str,
    /// What the refusal says happened.
    pub why: &'static str,
    /// What the refusal tells the person to run.
    pub command: &'static str,
}

/// Every command an agent's shell line is refused.
pub const COMMANDS: &[PersonOnly] = &[
    PersonOnly {
        verb: "stand-down",
        target: "estigia stand-down",
        code: "stand-down-is-for-a-person",
        why: "declaring a stand-down lifts every refusal this gate makes, so it is a person's \
          decision and an agent's shell line is not one: the gate refuses it on every host",
        command: "estigia stand-down --reason \"<why>\"",
    },
    PersonOnly {
        verb: "recover",
        target: "estigia recover",
        code: "recover-is-for-a-person",
        why: "recovering an escalated review is a maintainer's decision, and an agent's shell line \
          is not one: the gate refuses it on every host",
        command: "estigia recover <issue> --reason \"<why>\"",
    },
];

/// The record a stand-down is read from, which an agent may not write either.
pub const RECORD: PersonOnly = PersonOnly {
    verb: "stand-down.json",
    target: "~/.estigia/stand-down.json",
    code: "stand-down-is-for-a-person",
    why: "writing the stand-down record is declaring a stand-down, which lifts every refusal \
          this gate makes, so it is a person's decision and an agent's write is not one: the \
          gate refuses it on every host",
    command: "estigia stand-down --reason \"<why>\"",
};

/// The same command when the line carries `--lift` anywhere: refused too, with a way out that lifts.
pub const LIFT: PersonOnly = PersonOnly {
    verb: "stand-down",
    target: "estigia stand-down --lift",
    code: "stand-down-is-for-a-person",
    why: "lifting a stand-down is a person's command as well, and an agent's shell line is not \
          one: the gate refuses a line that runs `estigia stand-down` whatever options it \
          carries, on every host",
    command: "estigia stand-down --lift --reason \"<why>\"",
};

/// The row a line the classifier matched to `command` is refused under: [`LIFT`] when any word
/// of the line is `--lift`, because the gate cannot tell a lift from a declaration that only
/// quotes the flag.
pub fn refined(command: &'static PersonOnly, view: &str) -> &'static PersonOnly {
    let mentions = view
        .split(|c: char| !in_a_word(c))
        .any(|word| word == "--lift");
    if mentions && command.verb == LIFT.verb {
        &LIFT
    } else {
        command
    }
}

const RECORD_FRAGMENT: &str = ".estigia/stand-down.json";

/// Whether a path, as a file tool names it, is the stand-down record.
pub fn is_the_record(path: &str) -> bool {
    [path.to_owned(), dequoted(path)]
        .iter()
        .any(|spelling| matches_the_record(&super::normalise(spelling)))
}

fn dequoted(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\'' | '\\'))
        .collect()
}

/// Whether a normalised path carries the record fragment with a boundary on
/// **both** sides, so a backup such as `stand-down.json.bak` is a different
/// file. The shared [`super::is_anchored_fragment`] anchors a dot-directory on
/// the left only, which reaches the backup.
fn matches_the_record(path: &str) -> bool {
    path.match_indices(RECORD_FRAGMENT).any(|(at, _)| {
        let left = at == 0 || path.as_bytes()[at - 1] == b'/';
        left && ends_the_name(path, at + RECORD_FRAGMENT.len())
    })
}

/// Whether a name ending at `after` is where a path segment ends — at the end, at
/// a `/`, or after a trailing run of dots or spaces. Windows strips a trailing
/// dot or space, so `stand-down.json.` is the same file there and must read as
/// the record; `stand-down.json.bak` is a different one.
fn ends_the_name(path: &str, after: usize) -> bool {
    let stripped = path[after..].trim_start_matches(['.', ' ']);
    stripped.is_empty() || stripped.starts_with('/')
}

/// Whether free text, a shell line or a patch body, names the stand-down record anywhere.
pub fn names_the_record(text: &str) -> bool {
    super::text_names_path(text, &matches_the_record)
        || super::text_names_path(&dequoted(text), &matches_the_record)
}

/// Whether a shell line changes into the control directory and then writes the
/// record under its bare name: `cd ~/.estigia && echo … > stand-down.json`
/// writes the record, but the two halves never join into the one path
/// [`names_the_record`] matches, so the gate would not read it as the record.
/// Only a `cd` whose own target starts at that directory counts — a
/// subdirectory of it included, which refuses a file that is not the record and
/// so fails closed — and not any line that merely names `.estigia` somewhere.
pub fn names_the_record_bare(text: &str) -> bool {
    let writes_the_bare_name = |path: &str| {
        path.match_indices("stand-down.json").any(|(at, _)| {
            let left = at == 0 || path.as_bytes()[at - 1] == b'/';
            left && ends_the_name(path, at + "stand-down.json".len())
        })
    };
    let changes_into_the_control_directory = |text: &str| {
        // Shell metacharacters separate words as whitespace does: `cd ~/.estigia&&echo`
        // and `(cd ~/.estigia` both hold the `cd` the shell runs.
        let mut words = text
            .split(|c: char| {
                c.is_whitespace() || matches!(c, '&' | ';' | '|' | '(' | ')' | '<' | '>')
            })
            .filter(|word| !word.is_empty());
        while let Some(word) = words.next() {
            let changes_directory = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
            if changes_directory != "cd" && changes_directory != "pushd" {
                continue;
            }
            // `cd` takes options before its target (`cd -P ~/.estigia`), and a `cd`
            // with no target of its own (`cd; cd ~/.estigia`) must not swallow the
            // next one.
            let Some(target) = words.by_ref().find(|word| {
                !word.starts_with('-')
                    && !matches!(
                        word.trim_matches(|c: char| !c.is_ascii_alphanumeric()),
                        "cd" | "pushd"
                    )
            }) else {
                return false;
            };
            let target = target
                .trim_matches(|c: char| {
                    !(c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '~' | '_' | '-'))
                })
                // Windows strips a trailing dot or space from a directory name too,
                // so `cd ~/.estigia.` changes into the control directory there.
                .trim_end_matches(['.', ' ']);
            if super::is_anchored_fragment(&format!("{}/", super::normalise(target)), ".estigia/") {
                return true;
            }
        }
        false
    };
    let names = |text: &str| {
        changes_into_the_control_directory(text)
            && (super::text_names_path(text, &writes_the_bare_name)
                || super::text_names_path(&dequoted(text), &writes_the_bare_name))
    };
    names(text) || names(&dequoted(text))
}

/// The population and boundary [`named_in`] enforces.
pub const PERSON_ONLY_POPULATION: Population = Population {
    family: "person-only-shell",
    disposition: super::Disposition::TooTight,
    subject: "named_in",
    text: "the shell spellings of the commands only a person may run, `estigia stand-down`, which lifts every \
           refusal the gate makes, and `estigia recover`, which puts an escalated review back in play. Legitimate population: every command line \
           that runs one, with options allowed between the two words. Boundary: matched on any \
           word whose basename is `estigia` followed by the verb, wherever it stands, on the line \
           and on a dequoted copy with every quote and backslash removed, so neither character \
           hides a word the shell would run, and a line that merely names both, such as a commit \
           message, is refused too; an alias, a wrapper script or a renamed binary that never \
           writes `estigia <verb>` as two words is not matched \u{2014} the same standing every \
           shell rule of this gate has, which `docs/honesty.md` records.",
};

fn in_a_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '\\' | '-')
}

fn runs(words: &[&str], verb: &str) -> bool {
    words.iter().enumerate().any(|(at, word)| {
        super::binary_name(word) == "estigia"
            && words[at + 1..]
                .iter()
                .find(|next| !next.starts_with('-'))
                .is_some_and(|next| *next == verb)
    })
}

/// The command a lowercased line runs, wherever the binary stands and however its words are glued:
/// any word whose basename is `estigia`, then the verb after any `-` options. Read on the line
/// and on a copy with every `"`, `'` and `\` removed, so a quote or backslash inside either word
/// does not split it past the matcher while the shell still runs the command. A false negative is
/// a hole in the gate and a false positive fails closed, so a line that merely names both, such as
/// a commit message, is refused too.
pub fn named_in(view: &str) -> Option<&'static PersonOnly> {
    let dequoted = dequoted(view);
    let plain = words_of(view);
    let folded = words_of(&dequoted);
    COMMANDS
        .iter()
        .find(|command| runs(&plain, command.verb) || runs(&folded, command.verb))
}

fn words_of(text: &str) -> Vec<&str> {
    text.split(|c: char| !in_a_word(c))
        .filter(|word| !word.is_empty())
        .collect()
}

/// The refusal for an action the classifier matched to a person-only command, whoever holds the
/// checkout and whether or not the payload carried a literal command string.
pub fn refusal(action: &super::Action) -> Option<Refusal> {
    let super::Action::Write { target } = action else {
        return None;
    };
    let command = COMMANDS
        .iter()
        .chain([&RECORD, &LIFT])
        .find(|command| command.target == target)?;
    Some(Refusal::not_started(
        command.code,
        command.why.to_owned(),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            format!(
                "tell the person the exact command, and let them run it in their own shell, or \
                 with `!` in Claude Code, which does not pass through the agent's tools: `{}`",
                command.command
            ),
        ),
    ))
}

/// The one place this rule is decided: a deny from the classified action, which no stand-down,
/// claim or other refusal on the line can lift or replace. Every road that decides a call asks it
/// first, before any other decision and before a stand-down is applied.
pub fn deny(action: &super::Action) -> Option<super::Decision> {
    refusal(action).map(|refusal| super::Decision::Deny(Box::new(refusal)))
}

#[cfg(test)]
mod tests;
