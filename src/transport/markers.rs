//! The machine-readable control markers, and the two vocabularies over them.
//!
//! The prose flow adjudicates races and stand-downs by reading comment text, and
//! the skill itself warns that *"parsing prose for the same answer is fragile —
//! any rewording breaks it"*. These markers make the same facts exact. They are
//! HTML comments, so they are invisible in rendered markdown and cannot be
//! reworded by a later run editing the sentence around them.

use std::collections::BTreeMap;

/// A run no longer holds the item, so its earlier claim stops counting.
pub const RELEASE_KINDS: &[&str] = &["standdown", "release", "unassign", "reclaim"];

/// The kinds that prove a run is still alive.
pub const ACTIVITY_KINDS: &[&str] = &["heartbeat", "branch", "published", "progress", "blocked"];

/// The activity kinds the renewal writer may be asked to post.
///
/// `heartbeat`, and the two kinds that say a run is still working or waiting.
/// `branch` and `published` are activity too and are deliberately absent: each
/// has a writer of its own, so this refuses a caller asking the renewal writer
/// to mint one.
pub const RENEWAL_KINDS: &[&str] = &["heartbeat", "progress", "blocked"];

/// Whether the inside of an HTML comment opens a control marker.
///
/// **One predicate.** The prefix was written in three places across two
/// modules: the parser here, the escaper below, and the scan in `ownership`
/// that finds where markers sit in a body. All three answer the same question —
/// *is this comment one of ours* — and the three are the reader, the writer's
/// defence against a body that fakes one, and the reducer that adjudicates from
/// them. A prefix changed in one of the three is a marker one side writes and
/// another does not see.
pub(super) fn opens_a_marker(inside: &str) -> bool {
    inside.trim_start().starts_with(MARKER_PREFIX)
}

/// What every control marker in an issue body opens with.
pub(super) const MARKER_PREFIX: &str = "issue-flow:";

/// The kinds a `comment` may carry.
///
/// **One list, re-exported rather than written again** — the published schema
/// and the enforcement are the same three, so a kind cannot be offered to the
/// agent and refused by the transport, or the reverse.
pub use crate::config::COMMENT_KINDS;

/// The characters percent-encoding leaves alone.
///
/// Taken from the transport's `urllib.parse.quote(value, safe="-._~:/+@")`,
/// plus the alphanumerics `quote` never escapes. Widening or narrowing this by
/// one character changes what a marker says, and a marker is what decides who
/// holds an issue.
fn is_safe(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~:/+@".contains(&byte)
}

/// Appends the percent-encoding of one byte, the same rule [`render`] applies
/// to every attribute value.
fn push_encoded_byte(out: &mut String, byte: u8) {
    if is_safe(byte) {
        out.push(byte as char);
    } else {
        out.push_str(&format!("%{byte:02X}"));
    }
}

/// The percent-encoded length [`render`] would emit for `value` alone, run
/// through [`push_encoded_byte`] rather than a separate cost table so the two
/// can never drift apart.
pub(super) fn encoded_len(value: &str) -> usize {
    let mut probe = String::new();
    for byte in value.bytes() {
        push_encoded_byte(&mut probe, byte);
    }
    probe.len()
}

/// Whether a marker key is one this vocabulary allows.
fn key_is_valid(key: &str) -> bool {
    let mut characters = key.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|c| c.is_ascii_lowercase() || c == '-')
}

/// Whether a kind is one this vocabulary allows.
fn kind_is_valid(kind: &str) -> bool {
    !kind.is_empty() && kind.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

/// Renders one marker.
///
/// Attributes with an empty value are dropped, exactly as the original's
/// `if value` does — an absent run-id must not become `run-id=`, which would
/// read as a marker addressed to a run whose name is the empty string.
///
/// Returns `None` where the transport raises `invalid-marker-attribute`: a kind
/// or a key outside the allowed alphabet. Refusing to render is the safe half —
/// a marker nobody can parse is better than one that parses as something else.
pub fn render(kind: &str, attributes: &[(&str, &str)]) -> Option<String> {
    let kept: Vec<(String, &str)> = attributes
        .iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| (key.replace('_', "-"), *value))
        .collect();
    if !kind_is_valid(kind) || !kept.iter().all(|(key, _)| key_is_valid(key)) {
        return None;
    }

    // `encoding` first, then the caller's own, in the order they were given.
    let mut body = String::from("encoding=pct");
    for (key, value) in &kept {
        body.push(' ');
        body.push_str(key);
        body.push('=');
        for byte in value.bytes() {
            push_encoded_byte(&mut body, byte);
        }
    }
    Some(format!("<!-- issue-flow: {kind} {body} -->"))
}

/// One marker read back out of a comment body.
pub type Marker = BTreeMap<String, String>;

/// Every marker in a body, in the order they appear.
pub fn parse(body: &str) -> Vec<Marker> {
    let mut found = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find("<!--") {
        let after = &rest[at + 4..];
        let Some(close) = after.find("-->") else {
            break;
        };
        let inside = &after[..close];
        rest = &after[close + 3..];

        // A `>` between the opener and the close means the transport reads no
        // marker here at all — its pattern `(?P<attrs>[^>]*?)` cannot cross
        // one — so taking one anyway would be a claim only this side could
        // see. Neither renderer emits one: `>` percent-encodes to `%3E`.
        if inside.contains('>') {
            continue;
        }

        let trimmed = inside.trim_start();
        if !opens_a_marker(trimmed) {
            continue;
        }
        let Some(tail) = trimmed.strip_prefix(MARKER_PREFIX) else {
            continue;
        };
        let mut words = tail.split_whitespace();
        let Some(kind) = words.next() else { continue };
        if !kind_is_valid(kind) {
            continue;
        }
        // Whitespace has to follow the kind, because the transport's own
        // pattern demands it: `(?P<kind>[a-z-]+)\s+(?P<attrs>...)`. Without
        // this, a stand-down only this side can see is a gate only this side
        // stands down.
        let after_kind = tail.trim_start().get(kind.len()..).unwrap_or_default();
        if !after_kind.starts_with(char::is_whitespace) {
            continue;
        }

        let mut attributes = Marker::new();
        let mut encoded = false;
        let mut pairs: Vec<(String, String)> = Vec::new();
        for word in words {
            let Some((key, value)) = word.split_once('=') else {
                continue;
            };
            if key == "encoding" {
                encoded = value == "pct";
            }
            pairs.push((key.to_owned(), value.to_owned()));
        }
        for (key, value) in pairs {
            let value = if encoded && key != "encoding" {
                unquote(&value)
            } else {
                value
            };
            attributes.insert(key, value);
        }
        attributes.insert("kind".to_owned(), kind.to_owned());
        found.push(attributes);
    }
    found
}

/// Reverses percent-encoding, leaving a malformed escape as written.
///
/// `urllib.parse.unquote` does the same: `%zz` is not an escape, and turning it
/// into a replacement character would corrupt a value that is merely odd.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(text) = std::str::from_utf8(&bytes[index + 1..index + 3])
            // Two hex digits, asked for before the parse rather than left to
            // it: `u8::from_str_radix` accepts a leading sign, so `%+4` would
            // decode differently here than in `urllib.parse.unquote` — and a
            // value both sides decode is the run-id, which decides who holds
            // an issue.
            && text.bytes().all(|byte| byte.is_ascii_hexdigit())
            && let Ok(byte) = u8::from_str_radix(text, 16)
        {
            out.push(byte);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Keeps quoted tracker evidence inert.
///
/// A body that quotes a real marker would otherwise *be* that marker once
/// posted: an agent pasting a stand-down it received into a note would issue a
/// stand-down. Neutering the opener is what makes quoting evidence safe. This
/// covers only the opener half; the transport's other defence — pushing prose
/// that still reads as a claim behind a heading — lives in
/// [`super::ownership::claim_prose`].
pub fn escape_control_input(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest.find("<!--") {
        let after = &rest[at + 4..];
        // Two separate reasons to neuter this opener rather than copy it
        // through, and either is enough on its own. The first is quoting: a
        // body that quotes a real marker verbatim would otherwise *be* that
        // marker once posted.
        //
        // The second is a comment that never closes **within this fragment**:
        // `markers::parse` scans forward from any `<!--` to the *first* `-->`
        // wherever it lands, so once this fragment is spliced in front of a
        // real marker, that marker's own close is borrowed, the real marker is
        // consumed and never seen, and a write answers `ok: true` for a verdict
        // nothing can read back. A comment that *does* close within this
        // fragment is not this hazard, which is why an ordinary, self-contained
        // `<!-- note -->` is left exactly as written, below.
        if opens_a_marker(after) || !after.contains("-->") {
            out.push_str(&rest[..at]);
            out.push_str("&lt;!--");
        } else {
            out.push_str(&rest[..at + 4]);
        }
        rest = after;
    }
    out.push_str(rest);
    if super::ownership::claim_prose(&out).is_some() {
        return format!("Quoted evidence:\n\n{out}");
    }
    out
}

#[cfg(test)]
mod tests;
