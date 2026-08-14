//! The machine-readable control markers, and the two vocabularies over them.
//!
//! The prose flow adjudicates races and stand-downs by reading comment text, and
//! the skill itself warns that *"parsing prose for the same answer is fragile —
//! any rewording breaks it"*. These markers make the same facts exact. They are
//! HTML comments, so they are invisible in rendered markdown and cannot be
//! reworded by a later run editing the sentence around them.
//!
//! Ported before any of the five commands that read them — `claim`,
//! `verify-claim`, `heartbeat`, `reclaim`, `unassign` — because all five rest on
//! this and a disagreement here would be a disagreement in all of them at once.

use std::collections::BTreeMap;

/// A run no longer holds the item, so its earlier claim stops counting.
pub const RELEASE_KINDS: &[&str] = &["standdown", "release", "unassign", "reclaim"];

/// The kinds that prove a run is still alive.
pub const ACTIVITY_KINDS: &[&str] = &["heartbeat", "branch", "published"];

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
/// **One list, re-exported rather than written again.** This spelled the three
/// beside `crate::config::COMMENT_KINDS`, which spells the same three in
/// another order — and the two are the published schema and the enforcement:
/// the tool server tells an agent which kinds it may send with `.of(config::
/// COMMENT_KINDS)`, and `comment` refuses with `reserved-comment-kind` from
/// this one.
///
/// So a kind added to either was a kind the agent is offered and the transport
/// rejects, or one the transport takes and the schema hides. They agreed today
/// and nothing made them agree tomorrow — the second pair of this shape found
/// in one sweep, after `STATES`.
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
            if is_safe(byte) {
                body.push(byte as char);
            } else {
                body.push_str(&format!("%{byte:02X}"));
            }
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
        // marker here at all: its pattern is `(?P<attrs>[^>]*?)`, which cannot
        // cross one. This took everything up to the first `-->` and found a
        // marker in it, so `run-id=a>b` was a **claim only this side could
        // see** — the same shape as the `standdown-->` case the note below
        // records, and the same direction: a claim one side honours and the
        // other does not is the two of them disagreeing about who holds an
        // issue.
        //
        // Neither renderer emits one: `>` percent-encodes to `%3E`. A marker
        // carrying a bare `>` is hand-written, and hand-written markers are
        // exactly what the timeline is full of.
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
        // this, `<!-- issue-flow:standdown-->` was a marker here and not there
        // — and the direction that costs is this one, since a stand-down only
        // this side can see is a gate only this side stands down.
        //
        // Neither renderer writes that shape. A grammar is not the set of
        // shapes we happen to emit, which is what the differential says by
        // feeding it a body neither of them would have produced.
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
            // it: `u8::from_str_radix` accepts a leading sign, so `%+4`
            // answered the byte `0x04` here and stayed the three characters it
            // is in `urllib.parse.unquote`. A marker is read back out of
            // comments anybody can write, and the value it carries is the
            // run-id — so the two implementations decoding one differently is
            // the two of them disagreeing about who holds an issue.
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
/// stand-down. Neutering the opener is what makes quoting evidence safe.
///
/// A marker is not the only control shape a body can carry, and this only
/// covered that one. The transport has always had a second half — a body that
/// still reads as a prose claim after escaping is pushed behind a heading, so
/// the `^` the prose reader anchors on no longer reaches it — and the port
/// stopped at the first half. Two things went wrong at once.
///
/// A quoted claim posted verbatim *is* a claim. The marker a caller adds does
/// not save it: the reducer suppresses the prose read only for `claim` and
/// `reclaim`, so a `heartbeat` carrying quoted evidence is read as a claim by
/// whoever the quote names, holding this run's own comment timestamp.
///
/// And the escaped text is hashed. `forced_reclaim_hash` binds the reason a
/// takeover gave, so a reason the two sides escape differently is a digest they
/// compute differently — evidence the transport refuses to recognise as the
/// evidence this crate just published. The differential oracle could not see
/// either: it hashes reasons that are already escaped.
pub fn escape_control_input(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest.find("<!--") {
        let after = &rest[at + 4..];
        if opens_a_marker(after) {
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
