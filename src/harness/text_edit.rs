//! A GitHub call that only replaces the title or body of an issue or a pull request.

/// Whether this shell line is one plain command that edits a title, a body or both, and nothing
/// else.
///
/// `gh issue edit` and `gh pr edit` carrying only their title and body flags, and `gh api` with an
/// explicit `PATCH` of one issue or pull request whose fields are only `title` and `body`. A line
/// that cannot be read without running a shell is not one: an operator, an expansion or a second
/// command answers `false`.
pub(crate) fn is_text_edit(literal: &str) -> bool {
    let Some(words) = plain_words(literal) else {
        return false;
    };
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["gh", kind @ ("issue" | "pr"), "edit", rest @ ..] => edits_text(kind, rest),
        ["gh", "api", rest @ ..] => patches_text(rest),
        _ => false,
    }
}

const TITLE_AND_BODY: &[&str] = &["--title", "-t", "--body", "-b", "--body-file", "-F"];
const REPOSITORY: &[&str] = &["--repo", "-R"];

fn edits_text(kind: &str, rest: &[&str]) -> bool {
    let mut edited = false;
    let mut target = None;
    let mut words = rest.iter().copied();
    while let Some(word) = words.next() {
        if !word.starts_with('-') {
            if target.replace(word).is_some() || !names_one(kind, word) {
                return false;
            }
            continue;
        }
        let (flag, attached) = match word.split_once('=') {
            Some((flag, _)) if flag.starts_with("--") => (flag, true),
            _ => (word, false),
        };
        if !TITLE_AND_BODY.contains(&flag) && !REPOSITORY.contains(&flag) {
            return false;
        }
        edited |= TITLE_AND_BODY.contains(&flag);
        if !attached && words.next().is_none() {
            return false;
        }
    }
    edited && (kind == "pr" || target.is_some())
}

fn names_one(kind: &str, word: &str) -> bool {
    let Some(path) = word.strip_prefix("https://github.com/") else {
        return is_number(word.strip_prefix('#').unwrap_or(word));
    };
    let named = if kind == "pr" { "pull" } else { "issues" };
    matches!(
        path.split('/').collect::<Vec<_>>().as_slice(),
        [_, _, segment, number] if *segment == named && is_number(number)
    )
}

fn is_number(word: &str) -> bool {
    !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_digit())
}

fn patches_text(rest: &[&str]) -> bool {
    let mut endpoint = None;
    let mut patch = false;
    let mut edited = false;
    let mut words = rest.iter().copied();
    while let Some(word) = words.next() {
        match word {
            "-X" | "--method" => {
                if words.next() != Some("PATCH") {
                    return false;
                }
                patch = true;
            }
            "-XPATCH" | "--method=PATCH" => patch = true,
            "-f" | "-F" | "--field" | "--raw-field" => {
                let field = words.next().and_then(|field| field.split_once('='));
                if !matches!(field, Some(("title" | "body", _))) {
                    return false;
                }
                edited = true;
            }
            "--silent" => {}
            "-q" | "--jq" => {
                words.next();
            }
            _ if word.starts_with('-') => return false,
            _ => {
                if endpoint.replace(word).is_some() {
                    return false;
                }
            }
        }
    }
    patch && edited && endpoint.is_some_and(addresses_one)
}

fn addresses_one(endpoint: &str) -> bool {
    let path = endpoint.strip_prefix('/').unwrap_or(endpoint);
    matches!(
        path.split('/').collect::<Vec<_>>().as_slice(),
        ["repos", owner, repo, "issues" | "pulls", number]
            if !owner.is_empty() && !repo.is_empty() && is_number(number)
    )
}

fn plain_words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = line.trim().chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                let held = word.get_or_insert_default();
                loop {
                    match chars.next()? {
                        '\'' => break,
                        inner => held.push(inner),
                    }
                }
            }
            '"' => {
                let held = word.get_or_insert_default();
                loop {
                    match chars.next()? {
                        '"' => break,
                        '$' | '`' | '\\' | '!' => return None,
                        inner => held.push(inner),
                    }
                }
            }
            ' ' | '\t' => words.extend(word.take()),
            _ if c.is_alphanumeric() || "-_./:@=,+%".contains(c) => {
                word.get_or_insert_default().push(c);
            }
            _ => return None,
        }
    }
    words.extend(word);
    Some(words)
}

#[cfg(test)]
pub(crate) mod tests;
