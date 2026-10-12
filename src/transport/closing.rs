//! Will this issue auto-close on merge, and if so, from what?
//!
//! Two distinct causes with one symptom, and the remedy differs completely:
//!
//! - a **closing keyword** in a PR body or a commit message — avoidable, and a
//!   hard stop, because the text is the author's to fix;
//! - a **branch link** created by `gh issue develop` — GitHub turns the
//!   Development-sidebar link into a closing reference the moment a PR opens from
//!   that branch, and empties `linkedBranches` in the same move. No keyword is
//!   involved and no edit removes it.
//!
//! The second is not a defect in the run: it is what the recommended linking
//! command does. Making it a hard stop would make the recommended path
//! permanently un-shippable, and **a gate that always fires is a gate that gets
//! ignored**. So it is reported loudly with the follow-up it mandates, which
//! [`FOLLOW_UP`] states.

use super::{Context, Failure};

/// What a branch-linked auto-close obligates a run to do after the merge.
pub const FOLLOW_UP: &str = "Run `transition --to done` after the merge \u{2014} the auto-close is \
     not the workflow's close and moves neither the label nor the board \u{2014} and then \
     `release`, which is last and not optional: it puts the issue down on the tracker and drops \
     the local pointer this run wrote, and the delivering run is the only context that can drop \
     it without asking the tracker whose it was.";

const QUERY: &str = r#"
query($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    issue(number: $number) {
      closedByPullRequestsReferences(first: 100, after: $cursor) {
        nodes { number state headRefName baseRefName }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
"#;

/// The page cap. Bounds a hostile or unstable connection; reaching it while
/// GitHub still advertises another page is a **failed read**, not partial
/// evidence.
const MAX_PAGES: usize = 100;

/// Whether a JSON value is one the transport would read as **true**.
///
/// Python's truthiness, for the one place this port has to reproduce it: an
/// empty list, an empty object, a null and an empty string are all false there,
/// and `is_some()` calls every one of them an error.
fn carries_something(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(flag) => *flag,
        serde_json::Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        serde_json::Value::String(text) => !text.is_empty(),
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(map) => !map.is_empty(),
    }
}

/// The keywords GitHub treats as closing an issue.
const KEYWORDS: &[&str] = &[
    "closes", "closed", "close", "fixes", "fixed", "fix", "resolves", "resolved", "resolve",
];

/// Who this repository is, for the GraphQL query.
pub(super) fn repo_identity(context: &Context) -> Result<(String, String), Failure> {
    let data = super::gh_json(
        &["repo", "view", "--json", "owner,name"],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read("gh repo view returned nothing".to_owned()))?;
    identity_of(&data)
}

/// The owner and name in a `gh repo view` answer, or a read that did not answer.
///
/// **Its own function so it can be measured**, the way `is_object_id` is.
///
/// The rule: present and named, or the read did not answer. A `gh repo view`
/// returning JSON without the fields is not a repository owned by nobody; it is
/// a response nobody can act on — and this set authorises post-merge claim
/// renewal, so four callers act on it.
pub(super) fn identity_of(data: &serde_json::Value) -> Result<(String, String), Failure> {
    let (Some(owner), Some(name)) = (
        data.get("owner")
            .and_then(|owner| owner.get("login"))
            .and_then(serde_json::Value::as_str)
            .filter(|login| !login.trim().is_empty()),
        data.get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.trim().is_empty()),
    ) else {
        return Err(Failure::Read(
            "gh repo view answered without naming the repository's owner and name".to_owned(),
        ));
    };
    Ok((owner.to_owned(), name.to_owned()))
}

/// The complete closing-PR connection, or a rejected read.
///
/// This set authorises post-merge claim renewal, so a truncated or malformed
/// response **cannot** be treated as an empty one. Every shape check below is
/// there because "no closing PRs" and "the answer did not arrive" have opposite
/// consequences and the same appearance.
pub fn closing_refs(context: &Context, issue: u64) -> Result<Vec<serde_json::Value>, Failure> {
    let (owner, name) = repo_identity(context)?;
    let number = issue.to_string();
    let mut by_number: std::collections::BTreeMap<u64, serde_json::Value> = Default::default();
    let mut seen_cursors: std::collections::BTreeSet<String> = Default::default();
    let mut cursor: Option<String> = None;

    for page in 0..MAX_PAGES {
        let held = cursor.clone().unwrap_or_default();
        let query = format!("query={QUERY}");
        let mut arguments: Vec<&str> = vec![
            "api", "graphql", "-f", &query, "-f", "OWNER", "-f", "NAME", "-F", "NUMBER",
        ];
        let owner_argument = format!("owner={owner}");
        let name_argument = format!("name={name}");
        let number_argument = format!("number={number}");
        let cursor_argument = format!("cursor={held}");
        arguments[5] = &owner_argument;
        arguments[7] = &name_argument;
        arguments[9] = &number_argument;
        if cursor.is_some() {
            arguments.push("-f");
            arguments.push(&cursor_argument);
        }
        let data = super::gh_json(&arguments, Some(&context.repo_dir))?;

        let Some(data) = data.filter(|value| value.is_object()) else {
            return Err(Failure::Read(
                "closing-reference GraphQL response is partial or malformed".to_owned(),
            ));
        };
        // Present and **empty** is not an error: a response carrying
        // `"errors": []`, which GraphQL does return, is a clean read.
        if data.get("errors").is_some_and(carries_something) {
            return Err(Failure::Read(
                "closing-reference GraphQL response is partial or malformed".to_owned(),
            ));
        }
        let connection = data
            .get("data")
            .and_then(|data| data.get("repository"))
            .filter(|value| value.is_object())
            .and_then(|repository| repository.get("issue"))
            .and_then(|issue| issue.get("closedByPullRequestsReferences"))
            .filter(|value| value.is_object());
        let Some(connection) = connection else {
            return Err(Failure::Read(
                "closing-reference response omitted the issue connection".to_owned(),
            ));
        };
        let (nodes, page_info) = super::connection_page(connection, "closing-reference response")?;
        let Some(has_next) = page_info
            .get("hasNextPage")
            .and_then(serde_json::Value::as_bool)
        else {
            return Err(Failure::Read(
                "closing-reference page metadata has no boolean hasNextPage".to_owned(),
            ));
        };

        for reference in nodes {
            let malformed = !reference.is_object()
                || !reference
                    .get("number")
                    .is_some_and(serde_json::Value::is_u64)
                || !reference
                    .get("state")
                    .is_some_and(serde_json::Value::is_string);
            if malformed {
                return Err(Failure::Read(
                    "closing-reference response contains a malformed PR node".to_owned(),
                ));
            }
            let key = reference
                .get("number")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default();
            match by_number.get(&key) {
                // A duplicate that disagrees with itself is not a duplicate; it
                // is two answers, and neither can be believed.
                Some(previous) if previous != reference => {
                    return Err(Failure::Read(
                        "closing-reference response contains conflicting duplicate PR nodes"
                            .to_owned(),
                    ));
                }
                Some(_) => {}
                None => {
                    by_number.insert(key, reference.clone());
                }
            }
        }

        if !has_next {
            return Ok(by_number.into_values().collect());
        }
        let next = page_info
            .get("endCursor")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if next.is_empty() || !seen_cursors.insert(next.clone()) {
            return Err(Failure::Read(
                "closing-reference pagination did not provide a fresh continuation cursor"
                    .to_owned(),
            ));
        }
        cursor = Some(next);
        if page + 1 == MAX_PAGES {
            return Err(Failure::Read(
                "closing-reference pagination exceeded its safety bound".to_owned(),
            ));
        }
    }
    Err(Failure::Read(
        "closing-reference pagination did not terminate".to_owned(),
    ))
}

/// Every closing keyword in `text`, paired with the issue number it names.
///
/// Hand-scanned rather than matched by a pattern, because the shape is small and
/// the two spellings GitHub accepts — `#12` and the full issue URL — read more
/// clearly listed than escaped. The caller decides which issues it cares about.
fn closing_references(text: &str) -> Vec<(String, u64)> {
    // Matched case-insensitively **on the original text**, never on a lowercased
    // copy. Lowercasing can change a string's byte length, and every offset here
    // is used to slice what the caller is shown — so the two must be the same
    // string. Walking one byte at a time also lands inside an em dash the moment
    // a body carries prose, which is most of them.
    let mut found = Vec::new();
    let mut at = 0;

    while at < text.len() {
        if !text.is_char_boundary(at) {
            at += 1;
            continue;
        }
        let Some(keyword) = KEYWORDS.iter().find(|keyword| {
            // `get` rather than a slice: the end of a keyword-length window can
            // land inside a multi-byte character, and indexing there panics.
            text.get(at..at + keyword.len())
                .is_some_and(|window| window.eq_ignore_ascii_case(keyword))
                // By character and not by byte: a byte-wise test reads the
                // continuation byte of a letter like `á` as a non-word byte and
                // calls the position a word break.
                && text[..at].chars().next_back().is_none_or(|c| !is_word(c))
                && text[at + keyword.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !is_word(c))
        }) else {
            at += 1;
            continue;
        };
        let start = at;
        let mut cursor = at + keyword.len();
        cursor += count_while(&text[cursor..], char::is_whitespace);
        if text[cursor..].starts_with(':') {
            cursor += 1;
            cursor += count_while(&text[cursor..], char::is_whitespace);
        }
        let rest = &text[cursor..];
        let after = reference_prefix(rest).map(|length| cursor + length);
        let Some(digits_at) = after else {
            at = start + keyword.len();
            continue;
        };
        let length = count_while(&text[digits_at..], |c| c.is_ascii_digit());
        if length == 0 {
            at = start + keyword.len();
            continue;
        }
        let Some(number) = text[digits_at..digits_at + length].parse::<u64>().ok() else {
            at = start + keyword.len();
            continue;
        };
        // Reported as written, because a person has to find it in their own prose.
        found.push((text[start..digits_at + length].to_owned(), number));
        at = digits_at + length;
    }
    found
}

/// Every closing keyword in `text` that names `issue`, as written.
pub fn keywords_naming(text: &str, issue: u64) -> Vec<String> {
    closing_references(text)
        .into_iter()
        .filter(|(_, named)| *named == issue)
        .map(|(written, _)| written)
        .collect()
}

/// Every closing keyword in `text`, whatever issue it names, as written.
pub fn keywords_any(text: &str) -> Vec<String> {
    closing_references(text)
        .into_iter()
        .map(|(written, _)| written)
        .collect()
}

/// Every closing keyword in `text` that names an issue other than `issue`.
pub fn keywords_elsewhere(text: &str, issue: u64) -> Vec<String> {
    closing_references(text)
        .into_iter()
        .filter(|(_, named)| *named != issue)
        .map(|(written, _)| written)
        .collect()
}

/// The length of whatever precedes the digits of an issue reference.
///
/// GitHub names `#10`, the full URL, `GH-10` and `octo-org/octo-repo#100`, and
/// all are taken — a spelling it does not know is a merge that closes an issue
/// while the check reports the branch clear. The cross-repository form is taken
/// whatever repository it names, because this function is handed a number and
/// not a repository: a false positive there is one warning read by a person,
/// while the other direction costs the transition.
fn reference_prefix(text: &str) -> Option<usize> {
    if text.starts_with('#') {
        return Some(1);
    }
    // `GH-10`, which GitHub documents beside `#10`.
    if text
        .get(..3)
        .is_some_and(|head| head.eq_ignore_ascii_case("gh-"))
    {
        return Some(3);
    }
    if let Some(length) = url_issue_prefix(text) {
        return Some(length);
    }
    // `<owner>/<name>#`, the cross-repository shorthand.
    let owner = count_while(text, |c| c.is_alphanumeric() || ".-_".contains(c));
    if owner == 0 || !text[owner..].starts_with('/') {
        return None;
    }
    let after_owner = owner + 1;
    let name = count_while(&text[after_owner..], |c| {
        c.is_alphanumeric() || ".-_".contains(c)
    });
    if name == 0 || !text[after_owner + name..].starts_with('#') {
        return None;
    }
    Some(after_owner + name + 1)
}

/// The length of a `https://github.com/<owner>/<name>/issues/` prefix.
fn url_issue_prefix(text: &str) -> Option<usize> {
    const HOST: &str = "https://github.com/";
    if !text
        .get(..HOST.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(HOST))
    {
        return None;
    }
    let mut length = HOST.len();
    for _ in 0..2 {
        let segment = count_while(&text[length..], |c| {
            c.is_alphanumeric() || ".-_".contains(c)
        });
        if segment == 0 || !text[length + segment..].starts_with('/') {
            return None;
        }
        length += segment + 1;
    }
    text.get(length..length + 7)
        .is_some_and(|word| word.eq_ignore_ascii_case("issues/"))
        .then_some(length + 7)
}

fn count_while(text: &str, predicate: impl Fn(char) -> bool) -> usize {
    text.chars()
        .take_while(|c| predicate(*c))
        .map(char::len_utf8)
        .sum()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests;

/// The commit messages `origin/<base>..<branch>` adds, read where they live.
///
/// One place because it was two, and the two disagreed about what a `git log`
/// that did not answer means: an unread source is not *no keyword found*, so
/// this refuses rather than tolerating the failure.
///
/// The checkout is the caller's to name — `publish_review` reads the isolated
/// one, `assess_autoclose` the repository root. That difference is recorded in
/// `docs/honesty.md`, not settled here.
pub(super) fn commit_log(
    at: &std::path::Path,
    base: &str,
    branch: &str,
) -> Result<String, Failure> {
    let range = format!("origin/{base}..{branch}");
    super::run(
        &["git", "log", &range, "--format=%B"],
        Some(at),
        super::How::read(),
    )
    .map(|log| log.stdout)
}

/// The closing keywords the commits this branch adds name, read where it lives.
pub fn keywords_in_commits(
    at: &std::path::Path,
    base: &str,
    branch: &str,
    issue: u64,
) -> Result<Vec<String>, Failure> {
    Ok(keywords_naming(&commit_log(at, base, branch)?, issue))
}

/// Every closing keyword the commits this branch adds, whatever issue it names.
pub fn any_keywords_in_commits(
    at: &std::path::Path,
    base: &str,
    branch: &str,
) -> Result<Vec<String>, Failure> {
    Ok(keywords_any(&commit_log(at, base, branch)?))
}
