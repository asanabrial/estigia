//! What the Estigia loop tells a session to do next, for every host that runs one.
//!
//! The OpenCode plugin renders its objective rule and its preamble from here at
//! `setup`, and Codex's `Stop` hook builds the same text at run time, so the two
//! hosts cannot drift apart on what a continuation says.

use serde_json::{Value, json};

/// How a host addresses the `flow` skill from a prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invocation {
    /// A slash command, as OpenCode takes it: `/flow dev`.
    Slash,
    /// A skill mention, as Codex takes it: `$flow dev`.
    Mention,
}

impl Invocation {
    /// The character that starts the skill's name.
    pub fn sigil(self) -> char {
        match self {
            Self::Slash => '/',
            Self::Mention => '$',
        }
    }

    /// What is written before a role word or free text.
    pub fn prefix(self) -> String {
        format!("{}flow ", self.sigil())
    }
}

/// The prompt the loop sends: `dev` when no text was given, the text as it was
/// given when it already names the skill, and the text after the skill otherwise.
pub fn objective(invocation: Invocation, given: &str) -> String {
    if given.is_empty() {
        return format!("{}dev", invocation.prefix());
    }
    if names_the_skill(invocation, given) {
        given.to_owned()
    } else {
        format!("{}{given}", invocation.prefix())
    }
}

fn names_the_skill(invocation: Invocation, given: &str) -> bool {
    let word = given.strip_prefix(invocation.sigil()).unwrap_or(given);
    word.strip_prefix("flow")
        .is_some_and(|rest| rest.chars().next().is_none_or(char::is_whitespace))
}

/// Splits a `--max <n>` (or `--max=<n>`) flag out of the text a slash host was
/// given, returning the text without it and the cap.
///
/// Only a `--max` with a numeric value is removed. A `--max` whose value will
/// not parse — or which ends the text — is left where it was, with its value,
/// rather than silently eating the word that followed: a mistyped cap must not
/// corrupt the objective it was written into.
///
/// The rule is shared: the OpenCode plugin parses the same flag from the same
/// text by its own expression, which likewise removes only `--max` followed by
/// digits, and `/flow-status` reports the cap the same way.
pub fn without_cap(given: &str) -> (String, Option<u64>) {
    let mut cap = None;
    let mut kept: Vec<&str> = Vec::new();
    let mut words = given.split_whitespace().peekable();
    while let Some(word) = words.next() {
        if cap.is_none() && word == "--max" {
            if let Some(value) = words.peek().and_then(|value| (*value).parse::<u64>().ok()) {
                cap = Some(value);
                words.next();
                continue;
            }
        } else if cap.is_none()
            && let Some(value) = word.strip_prefix("--max=")
            && let Ok(value) = value.parse::<u64>()
        {
            cap = Some(value);
            continue;
        }
        kept.push(word);
    }
    (kept.join(" "), cap)
}

const OPENING: &str = "Estigia loop: this session's run id is {run_id}. Hold your issue under \
exactly that run id \u{2014} claim or renew with --run-id {run_id}. Continue the objective below. \
When your issue is done and released, select and claim the next eligible task under the same run id. \
If the operator asks you to stop after the current task, run `estigia goal stop-after --run-id \
{run_id}`; the loop ends once your claim is released.";

const DISCHARGE: &str = " The tracker holds {n} blocked task{s} whose recorded exit condition \
already holds: discharge one by moving it to the state its preserved evidence supports, then \
select and claim it under this run id.";

const ENDS: &str = " Estigia ends the loop when the run holds no live claim and no work is \
claimable, dischargeable, held or pending";

const CAP: &str = ", or when the completion cap is reached";

const CLOSE: &str = "; there is no completion string to write.";

fn filled(template: &str, values: &[(&str, &str)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (key, value)| {
            text.replace(&format!("{{{key}}}"), value)
        })
}

/// The paragraph that tells a continued session which run it is and what ends
/// the loop. `capped` is whether the host can end the loop on a completion cap.
pub fn preamble(run_id: &str, dischargeable: u64, capped: bool) -> String {
    let discharge = if dischargeable > 0 {
        filled(
            DISCHARGE,
            &[
                ("n", &dischargeable.to_string()),
                ("s", if dischargeable == 1 { "" } else { "s" }),
            ],
        )
    } else {
        String::new()
    };
    format!(
        "{}{discharge}{ENDS}{}{CLOSE}",
        filled(OPENING, &[("run_id", run_id)]),
        if capped { CAP } else { "" }
    )
}

/// The whole prompt a `Stop` hook hands back: the preamble, a blank line, and
/// the objective with no operator text.
pub fn continuation(invocation: Invocation, run_id: &str, dischargeable: u64) -> String {
    format!(
        "{}\n\n{}",
        preamble(run_id, dischargeable, false),
        objective(invocation, "")
    )
}

/// The same pieces as data, for the JavaScript plugin to render its own
/// `preamble` from.
pub fn preamble_parts() -> Value {
    json!({
        "opening": OPENING,
        "discharge": DISCHARGE,
        "ends": ENDS,
        "cap": CAP,
        "close": CLOSE,
    })
}

/// The objective's fixed strings for a slash host, for the plugin's `objective`.
pub fn objective_parts() -> Value {
    json!({
        "default": objective(Invocation::Slash, ""),
        "prefix": Invocation::Slash.prefix(),
    })
}

#[cfg(test)]
pub(crate) const SLASH_OBJECTIVES: &[(&str, &str)] = &[
    ("", "/flow dev"),
    ("do the thing", "/flow do the thing"),
    ("only Alicante and Murcia", "/flow only Alicante and Murcia"),
    ("/flow triage x", "/flow triage x"),
    ("flow dev 12", "flow dev 12"),
    ("flow", "flow"),
    ("/flow", "/flow"),
    ("flowchart x", "/flow flowchart x"),
    ("/flowchart x", "/flow /flowchart x"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slash_objective_follows_the_shared_cases() {
        for (given, expected) in SLASH_OBJECTIVES {
            assert_eq!(objective(Invocation::Slash, given), *expected, "{given:?}");
        }
    }

    #[test]
    fn a_mention_host_gets_the_same_rule_with_its_own_sigil() {
        for (given, expected) in [
            ("", "$flow dev"),
            ("do the thing", "$flow do the thing"),
            ("$flow triage x", "$flow triage x"),
            ("flow dev 12", "flow dev 12"),
            ("$flowchart x", "$flow $flowchart x"),
            ("/flow x", "$flow /flow x"),
        ] {
            assert_eq!(objective(Invocation::Mention, given), expected, "{given:?}");
        }
    }

    #[test]
    fn the_preamble_names_the_run_and_counts_what_can_be_discharged() {
        let none = preamble("codex-abc", 0, true);
        assert!(none.starts_with("Estigia loop: this session's run id is codex-abc."));
        assert!(none.contains("--run-id codex-abc"));
        assert!(
            none.contains("estigia goal stop-after --run-id codex-abc"),
            "the stop-after instruction does not carry the run id an identity needs: {none}"
        );
        assert!(!none.contains("blocked task"));
        assert!(none.ends_with(
            "held or pending, or when the completion cap is reached; there is no completion \
             string to write."
        ));

        assert!(preamble("codex-abc", 1, true).contains("holds 1 blocked task whose"));
        assert!(preamble("codex-abc", 3, true).contains("holds 3 blocked tasks whose"));
        assert!(
            preamble("codex-abc", 3, false)
                .ends_with("held or pending; there is no completion string to write.")
        );
    }

    #[test]
    fn the_cap_is_split_out_of_the_text_without_losing_the_objective() {
        for (given, text, cap) in [
            ("", "", None),
            ("do the thing", "do the thing", None),
            ("do the thing --max 2", "do the thing", Some(2)),
            ("--max=3 do the thing", "do the thing", Some(3)),
            ("--max 4", "", Some(4)),
            ("only Alicante and Murcia", "only Alicante and Murcia", None),
            ("do the thing --max abc", "do the thing --max abc", None),
            ("do --max 2 extra", "do extra", Some(2)),
            ("--max", "--max", None),
            ("--max=abc do it", "--max=abc do it", None),
        ] {
            assert_eq!(without_cap(given), (text.to_owned(), cap), "{given:?}");
        }
    }

    #[test]
    fn a_continuation_is_the_preamble_a_blank_line_and_the_bare_objective() {
        let text = continuation(Invocation::Mention, "codex-abc", 2);
        assert_eq!(
            text,
            format!("{}\n\n$flow dev", preamble("codex-abc", 2, false))
        );
    }
}
