//! Extracts the inline-code corpus out of Markdown.
//!
//! [`crate::source_text::code_of`] answers the same question for Rust source;
//! this is its Markdown counterpart, kept in a sibling module so `build.rs`'s
//! own standalone compilation of `source_text.rs` stays narrow — see
//! `docs/honesty-review-tier.md` for the argument.
//!
//! A symbol this crate owns is a claim only where a document backticks it —
//! a prose mention never bound anybody to anything — so a test that crosses
//! `skill/` against the code must read the same corpus a human does.

/// Lines of `markdown` with fenced code blocks (``` or ~~~, one open fence
/// tracked at a time) removed entirely, fence marker lines included.
///
/// Shared by [`markdown_inline_code_spans`] and
/// `transport::tier::is_static_mdx_document`, which both need Markdown text
/// with neither a GFM fence nor whatever a foreign-language block holds —
/// lifted out of the latter rather than reimplemented, so a fence is skipped
/// one way in this crate.
pub(crate) fn markdown_lines_outside_fences(markdown: &str) -> impl Iterator<Item = &str> {
    let mut fence: &str = "";
    markdown.lines().filter_map(move |line| {
        let trimmed = line.strip_prefix('\u{feff}').unwrap_or(line).trim();
        if !fence.is_empty() {
            if trimmed.starts_with(fence) {
                fence = "";
            }
            return None;
        }
        if let Some(marker) = ["```", "~~~"]
            .into_iter()
            .find(|marker| trimmed.starts_with(marker))
        {
            fence = marker;
            return None;
        }
        Some(trimmed)
    })
}

/// One backtick-delimited unit of an unfenced Markdown line.
///
/// A caller wanting the prose keeps only [`Prose`](MarkdownSegment::Prose); a
/// caller wanting the inline-code corpus keeps only
/// [`Span`](MarkdownSegment::Span). Both read the one backtick walk below
/// rather than each matching backtick runs and escapes on its own.
pub(crate) enum MarkdownSegment<'a> {
    Prose(&'a str),
    Span(&'a str),
}

/// Splits `line` into its prose and inline-code-span segments, in order.
///
/// Not strictly alternating: an escaped backtick and an unterminated run each
/// leave two consecutive `Prose` segments, and no caller depends on the
/// alternation.
///
/// Multi-backtick runs open and close a span together (`` ``code`` `` holds a
/// literal backtick), and a backslash-escaped backtick opens or closes
/// nothing — the same two rules `transport::tier`'s Markdown handling already
/// used, read here from one place instead of two.
pub(crate) fn markdown_segments(line: &str) -> Vec<MarkdownSegment<'_>> {
    let mut out = Vec::new();
    let mut rest = line;
    loop {
        let Some(start) = rest.find('`') else {
            out.push(MarkdownSegment::Prose(rest));
            break;
        };
        if escaped_backtick(rest, start) {
            out.push(MarkdownSegment::Prose(&rest[..=start]));
            rest = &rest[start + 1..];
            continue;
        }
        out.push(MarkdownSegment::Prose(&rest[..start]));
        let bytes = rest.as_bytes();
        let mut run = 1;
        while start + run < bytes.len() && bytes[start + run] == b'`' {
            run += 1;
        }
        let marker = &rest[start..start + run];
        let remainder = &rest[start + run..];
        match exact_backtick_run_index(remainder, marker) {
            None => {
                out.push(MarkdownSegment::Prose(&rest[start..]));
                break;
            }
            Some(end) => {
                out.push(MarkdownSegment::Span(&remainder[..end]));
                rest = &remainder[end + run..];
            }
        }
    }
    out
}

fn exact_backtick_run_index(value: &str, marker: &str) -> Option<usize> {
    let mut offset = 0;
    while offset < value.len() {
        let index = offset + value[offset..].find(marker)?;
        let bytes = value.as_bytes();
        let before = index > 0 && bytes[index - 1] == b'`';
        let after = index + marker.len() < value.len() && bytes[index + marker.len()] == b'`';
        if !before && !after && !escaped_backtick(value, index) {
            return Some(index);
        }
        offset = index + 1;
    }
    None
}

fn escaped_backtick(value: &str, index: usize) -> bool {
    let bytes = value.as_bytes();
    let mut backslashes = 0;
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        backslashes += 1;
        cursor -= 1;
    }
    backslashes % 2 == 1
}

/// Every inline code span's content in `markdown`, one entry per span —
/// the corpus a Markdown document commits to, as distinct from prose that
/// only mentions a word.
///
/// Fenced blocks are skipped entirely first, so an example shown inside one
/// never enters the corpus; only a backticked span outside a fence does. A span is
/// matched per line — Markdown code spans do not cross a fenced boundary,
/// and this crate's own tables never carry one across a line break either.
pub fn markdown_inline_code_spans(markdown: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in markdown_lines_outside_fences(markdown) {
        for segment in markdown_segments(line) {
            if let MarkdownSegment::Span(text) = segment {
                found.push(text.to_owned());
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::markdown_inline_code_spans;

    #[test]
    fn a_backticked_symbol_enters_the_corpus_and_a_prose_mention_does_not() {
        let spans =
            markdown_inline_code_spans("Only `real-code` counts, not real-code in prose.\n");
        assert_eq!(spans, vec!["real-code".to_owned()]);
    }

    #[test]
    fn a_symbol_inside_a_fenced_block_never_enters_the_corpus() {
        let spans = markdown_inline_code_spans(
            "Prose with `outside-fence`.\n```\n`inside-fence` is an example, not a claim.\n```\n\
             More `after-fence`.\n",
        );
        assert_eq!(
            spans,
            vec!["outside-fence".to_owned(), "after-fence".to_owned()]
        );
    }

    #[test]
    fn a_tilde_fence_is_skipped_the_same_as_a_backtick_fence() {
        let spans = markdown_inline_code_spans("~~~\n`hidden-code`\n~~~\n`shown-code`\n");
        assert_eq!(spans, vec!["shown-code".to_owned()]);
    }

    #[test]
    fn an_escaped_backtick_does_not_open_or_close_a_span() {
        let spans = markdown_inline_code_spans("A literal \\` then `real-span` then text.\n");
        assert_eq!(spans, vec!["real-span".to_owned()]);
    }

    #[test]
    fn a_double_backtick_span_may_hold_a_literal_backtick() {
        let spans = markdown_inline_code_spans("A span holding one: ``a`b``.\n");
        assert_eq!(spans, vec!["a`b".to_owned()]);
    }
}
