//! Reading one version's entry out of a markdown changelog.
//!
//! Read-only by design. It neither creates the tag nor the Release, because both
//! are irreversible and stay with the agent. What it removes is the step that
//! gets skipped or improvised: every version this workflow ships is supposed to
//! have a changelog entry, and `--generate-notes` quietly substitutes a list of
//! commit subjects for it — which reads like documentation without being any.
//!
//! Hand-written rather than a regex, and not to avoid a dependency: the two
//! rules that make this correct are easier to *read* as code than as a pattern,
//! and both were learned from a live failure.

/// A located entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The heading line, whole. Cutting it at the version would drop the date
    /// and description the entry's own author wrote there — precisely the
    /// context a release note needs.
    pub heading: String,
    /// Everything under it, trimmed.
    pub body: String,
}

/// Why an entry could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// No heading opens with this version.
    Missing,
    /// More than one does.
    ///
    /// Not resolved by picking the first. Whichever is chosen becomes permanent
    /// in an immutable tag, and the topmost is not reliably the real one — a
    /// superseded draft above a genuine entry is exactly the shape that goes
    /// wrong.
    Ambiguous(Vec<String>),
}

/// Extracts one version's entry.
///
/// Format-tolerant, because changelog conventions vary and this skill must not
/// impose one: any heading level, `v` optional, `[1.2.3]` bracketed or bare, an
/// optional `Version`/`Release` word, and anything may *follow* on the heading
/// line — a date, a severity, a description.
///
/// **The version must open the heading, not merely appear in it.** That anchor
/// is the whole correctness of this function. Caught live against a real
/// changelog: an entry headed `### 2026-07-25 — (sin bump de versión) —
/// (… sigue en v6.9.8)` — an entry whose entire point is that 6.9.8 did *not*
/// ship in it — was matched for 6.9.8 ahead of the genuine `### v6.9.8` heading
/// further down. The tag would have carried notes describing a different change
/// and explicitly disclaiming the version it was named after.
///
/// The character after the number must not be alphanumeric, `.`, `-` or `+`.
/// Rejecting only another digit let a query for `6.9.8` match `### v6.9.8-rc1`,
/// because `-` is neither a digit nor a dot — so a release candidate sitting
/// above the real entry would win. And `1.2.3` must not match `1.2.30`, which a
/// naive substring search does.
pub fn section(text: &str, version: &str) -> Result<Section, Trouble> {
    let wanted = version.trim_start_matches(['v', 'V']);
    let mut found: Vec<(usize, usize)> = Vec::new();

    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if let Some(level) = heading_level(line, wanted) {
            found.push((offset, level));
        }
        offset += line.len();
    }

    if found.is_empty() {
        return Err(Trouble::Missing);
    }
    if found.len() > 1 {
        return Err(Trouble::Ambiguous(
            found
                .iter()
                .map(|(at, _)| {
                    text[*at..]
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_owned()
                })
                .collect(),
        ));
    }

    let (start, level) = found[0];
    let line_end = text[start..].find('\n').map_or(text.len(), |at| start + at);
    let heading = text[start..line_end].trim().to_owned();

    // The section ends at the next heading of the **same or shallower** level,
    // so sub-headings inside an entry stay with it.
    let rest = &text[line_end..];
    let mut end = text.len();
    let mut at = line_end;
    for line in rest.split_inclusive('\n') {
        if at > line_end && is_heading_at_most(line, level) {
            end = at;
            break;
        }
        at += line.len();
    }
    let body = text[line_end..end].trim().to_owned();
    Ok(Section { heading, body })
}

/// The heading level, when this line opens with `version`.
fn heading_level(line: &str, wanted: &str) -> Option<usize> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    let rest = rest
        .strip_prefix([' ', '\t'])?
        .trim_start_matches([' ', '\t']);

    // An optional `Version`/`Release` word, then an optional bracket and `v`.
    let rest = strip_word(rest, "version")
        .or_else(|| strip_word(rest, "release"))
        .unwrap_or(rest);
    let rest = rest.strip_prefix('[').unwrap_or(rest);
    let rest = rest.strip_prefix(['v', 'V']).unwrap_or(rest);

    // Case-insensitively, because the transport compiles this whole pattern
    // with `IGNORECASE` and this compared the version byte for byte. A version
    // with letters in it — every pre-release has them — was then read two ways.
    //
    // Both answers it produced were wrong, and the second is the expensive one.
    // `1.0.0-rc1` against a heading written `v1.0.0-RC1` was reported as
    // `no-changelog-entry`, telling somebody to write an entry that is already
    // there. And where a changelog holds *both* spellings, the transport calls
    // it ambiguous and refuses, while this picked the first silently — which is
    // the one outcome this function's own contract says it must never have,
    // because the tag it feeds is immutable.
    let after = strip_prefix_ignoring_case(rest, wanted)?;
    let after = after.strip_prefix(']').unwrap_or(after);
    match after.chars().next() {
        None => Some(hashes),
        // `[0-9A-Za-z.+\-]`, which is ASCII where `is_alphanumeric` is not.
        Some(next) if next.is_ascii_alphanumeric() || ".-+".contains(next) => None,
        Some(_) => Some(hashes),
    }
}

/// `text` with `prefix` taken off the front, ignoring ASCII case.
fn strip_prefix_ignoring_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// Strips a leading word and the whitespace after it, case-insensitively.
fn strip_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let head = text.get(..word.len())?;
    if !head.eq_ignore_ascii_case(word) {
        return None;
    }
    let rest = &text[word.len()..];
    rest.starts_with([' ', '\t'])
        .then(|| rest.trim_start_matches([' ', '\t']))
}

/// Whether this line is a heading of at most `level` hashes.
fn is_heading_at_most(line: &str, level: usize) -> bool {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=level).contains(&hashes) && line[hashes..].starts_with([' ', '\t'])
}

#[cfg(test)]
mod tests;
