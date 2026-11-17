//! A `gh` read of an issue number this machine allocated while the git window was closed.

use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// The stable code of this answer.
pub const CODE: &str = "provisional-issue-number";

const ISSUE_VERBS: &[&str] = &["view", "comment", "edit", "close", "reopen"];
use super::issue_creation::OPTIONS_WITH_A_VALUE;

/// The provisional number a command line reads or writes through `gh`, and the
/// repository it names with `-R`, when it names one.
pub fn named_provisional(literal: &str) -> Option<(u64, Option<String>)> {
    let lowered = literal.to_ascii_lowercase();
    let words: Vec<&str> = lowered
        .split_whitespace()
        .map(|word| word.trim_matches(|c| matches!(c, ';' | ')' | '(' | '&' | '|' | '"' | '\'')))
        .collect();
    let repo = words
        .windows(2)
        .find(|pair| pair[0] == "-r" || pair[0] == "--repo")
        .map(|pair| pair[1].to_owned())
        .or_else(|| {
            words
                .iter()
                .find_map(|word| word.strip_prefix("--repo="))
                .map(str::to_owned)
        });
    for (at, word) in words.iter().enumerate() {
        if *word != "gh" {
            continue;
        }
        let mut rest = words[at + 1..].iter().copied();
        match skip_options(&mut rest) {
            Some("issue") => {
                if skip_options(&mut rest).is_some_and(|verb| ISSUE_VERBS.contains(&verb))
                    && let Some(number) = skip_options(&mut rest).and_then(provisional_in)
                {
                    return Some((number, repo));
                }
            }
            Some("api") => {
                if let Some(number) = graphql_issue_number(&lowered) {
                    return Some((number, repo));
                }
            }
            _ => {}
        }
    }
    None
}

fn skip_options<'a>(words: &mut impl Iterator<Item = &'a str>) -> Option<&'a str> {
    loop {
        let word = words.next()?;
        if !word.starts_with('-') {
            return Some(word);
        }
        if OPTIONS_WITH_A_VALUE.contains(&word) {
            words.next()?;
        }
    }
}

fn provisional_in(word: &str) -> Option<u64> {
    let digits = word.trim_start_matches('#');
    let digits = digits.rsplit('/').next().unwrap_or(digits);
    digits
        .parse::<u64>()
        .ok()
        .filter(|number| crate::transport::outbox::is_provisional(*number))
}

fn graphql_issue_number(lowered: &str) -> Option<u64> {
    let compact: String = lowered.chars().filter(|c| !c.is_whitespace()).collect();
    let mut from = 0;
    while let Some(at) = compact[from..].find("issue(number:") {
        let start = from + at + "issue(number:".len();
        let digits: String = compact[start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Some(number) = provisional_in(&digits) {
            return Some(number);
        }
        from = start;
    }
    None
}

/// The answer for a shell line that would ask GitHub about a provisional
/// number: what this machine's store holds instead, since GitHub can only say
/// not found. A line naming another repository with `-R` is left to GitHub.
pub fn refusal(literal: Option<&str>) -> Option<Refusal> {
    let (issue, named_repo) = named_provisional(literal?)?;
    let reading = crate::transport::outbox::reading(issue);
    let reading = match reading {
        Ok(reading) => reading,
        Err(failure) => {
            return Some(Refusal::not_started(
                CODE,
                format!(
                    "#{issue} is a provisional number, which GitHub never holds, and this \
                     machine's store could not be read to say what it is: {}",
                    failure.detail()
                ),
                Resolution::no_command(
                    NoCommandReason::WorldAction,
                    "a readable Estigia store (`~/.estigia/state.db`)",
                ),
            ));
        }
    };
    let allocated_in = reading.get("repo").and_then(serde_json::Value::as_str);
    if let (Some(named), Some(allocated_in)) = (named_repo.as_deref(), allocated_in)
        && !named.eq_ignore_ascii_case(allocated_in)
    {
        return None;
    }
    let detail = reading
        .get("detail")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let resolution = match reading.get("real").and_then(serde_json::Value::as_u64) {
        Some(real) => Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            format!("the same command naming #{real}, the number GitHub gave it"),
        ),
        None if reading.get("allocated_here") == Some(&serde_json::Value::Bool(false)) => {
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the real number, from the machine that filed it once its queue is sent",
            )
        }
        None => Resolution::no_command(
            NoCommandReason::WorldAction,
            "the git operation window opening, after which `estigia flush` sends the queue and \
             the issue gets its GitHub number; until then `list_state` with `issue` reads it",
        ),
    };
    let queued = reading
        .get("queued")
        .and_then(serde_json::Value::as_u64)
        .map(|seq| format!(" (queue entry {seq})"))
        .unwrap_or_default();
    Some(Refusal::not_started(
        CODE,
        format!("{detail}{queued}; this answer is the local store's, not GitHub's"),
        resolution,
    ))
}

#[cfg(test)]
mod tests;
