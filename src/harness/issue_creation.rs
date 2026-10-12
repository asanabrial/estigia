//! Filing an issue from a shell line, which skips the analyst template the `create` tool checks.

use super::Population;
use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// The stable code of this refusal.
pub const CODE: &str = "issue-creation-outside-flow";

/// The population and boundary [`names_issue_creation`] enforces.
pub const ISSUE_CREATION_POPULATION: Population = Population {
    family: "issue-creation-shell",
    disposition: super::Disposition::TooTight,
    subject: "names_issue_creation",
    text: "the shell spellings that file an issue without going through the `create` tool: \
           `gh issue create` and its alias `gh issue new`, with options allowed between the \
           words, and `gh api` posting to an `/issues` path (a query string is ignored) or a \
           GraphQL `createIssue` mutation. Legitimate population: every issue filed from a command \
           line that names `gh`. Boundary: matched on substrings of the command line, so a line that \
           merely mentions the command is refused too, `gh` after a wrapper such as `sudo` is \
           matched, and an alias or a script that never spells `gh` on the line is not; \
           `create` is the one place the analyst template is checked.",
};

const OPTIONS_WITH_A_VALUE: &[&str] = &["-r", "--repo", "--hostname"];

/// Whether a lowercased, option-stripped command line files an issue.
pub fn names_issue_creation(normalized: &str) -> bool {
    super::contains_whole_command(normalized, "gh issue create")
        || super::contains_whole_command(normalized, "gh issue new")
        || gh_reaches_issue_creation(normalized)
        || api_files_an_issue(normalized)
}

fn api_files_an_issue(normalized: &str) -> bool {
    let posts = super::mutates_through_the_api(normalized);
    if !posts {
        return false;
    }
    let reads = normalized.contains("-x get") || normalized.contains("--method get");
    let issues_path = normalized.split_whitespace().any(|word| {
        let path = word.trim_matches(|c| c == '"' || c == '\'');
        [path, path.split('?').next().unwrap_or_default()]
            .iter()
            .any(|candidate| candidate.trim_end_matches('/').ends_with("/issues"))
    });
    !reads && (issues_path || normalized.contains("createissue"))
}

fn gh_reaches_issue_creation(normalized: &str) -> bool {
    let mut from = 0;
    while let Some(at) = normalized[from..].find("gh ") {
        let start = from + at;
        from = start + 1;
        let before = normalized[..start].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '/')) {
            continue;
        }
        let mut words = normalized[start + 3..]
            .split_whitespace()
            .map(|word| word.trim_end_matches([';', ')', '&', '|', '"', '\'']));
        if skip_options(&mut words) == Some("issue")
            && skip_options(&mut words).is_some_and(|w| w == "create" || w == "new")
        {
            return true;
        }
    }
    false
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

/// The refusal for a shell line that files an issue, whoever holds the checkout.
pub fn refusal(literal: Option<&str>) -> Option<Refusal> {
    let lowered = literal?.to_ascii_lowercase();
    let normalized = super::without_leading_options(&lowered);
    names_issue_creation(&normalized).then(|| {
        Refusal::not_started(
            CODE,
            "this command files an issue without the analyst template: the body is never checked \
             for the fixed first heading, the NOTE alert or the heading order"
                .to_owned(),
            Resolution::no_command(
                NoCommandReason::OperatorKnowledge,
                "the issue body filled from the analyst issue template and filed with the \
                 `create` tool, which refuses a body that does not follow it before anything \
                 is filed",
            ),
        )
    })
}

#[cfg(test)]
mod tests;
