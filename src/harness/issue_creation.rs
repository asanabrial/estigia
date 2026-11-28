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
           line that names `gh`. Boundary: `gh issue create` and `gh issue new` are matched on \
           substrings of the command line, so a line that merely mentions them is refused too; the \
           `gh api` spellings are read per command from the endpoint of the call that writes, a \
           query string ignored and a trailing slash trimmed, so a directory or an `--input` file \
           named `issues` is not an endpoint and a `-X GET` read in another command exempts \
           nothing; `gh` after a wrapper such as `sudo` is matched, and an alias or a script that \
           never spells `gh` on the line is not; `create` is the one place the analyst template is \
           checked.",
};

/// The `gh` options that take a value, skipped when reading the words after `gh`.
pub(crate) const OPTIONS_WITH_A_VALUE: &[&str] = super::GH_OPTIONS_WITH_A_VALUE;

/// Whether a lowercased, option-stripped command line files an issue.
pub fn names_issue_creation(normalized: &str) -> bool {
    super::names_gh_command(normalized, &["issue", "create"])
        || super::names_gh_command(normalized, &["issue", "new"])
        || api_files_an_issue(normalized)
}

fn api_files_an_issue(normalized: &str) -> bool {
    !super::gh_calls(normalized, |call| {
        crate::transport::window::gh_api_files_an_issue(call).then_some(())
    })
    .is_empty()
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
                 is filed; for another repository, the same call with `repo` set to its \
                 `owner/name`",
            ),
        )
    })
}

#[cfg(test)]
mod tests;
