//! Strips `//`, `///`, `//!` and `/* */` comments out of Rust source text
//! before a test searches it as raw bytes, leaving strings and char literals
//! untouched — what makes every `include_str!`-based assertion comment-blind.

/// Strips `//`, `///`, `//!` and `/* */` comments (nested block comments
/// included) out of `source`, leaving string, raw-string and char literals
/// untouched and the line structure unchanged — what a test calls before
/// searching Rust source, so it never mistakes prose for code and never
/// recognises a comment by hand.
pub fn code_of(source: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Code,
        Line,
        Block(usize),
        Str,
        Char,
        Raw(usize),
    }

    let mut state = State::Code;
    let mut out = String::with_capacity(source.len());
    let mut escape = false;
    let mut chars = source.chars().peekable();

    while let Some(c) = chars.next() {
        match state {
            State::Code => match c {
                '/' if chars.peek() == Some(&'/') => {
                    chars.next();
                    state = State::Line;
                }
                '/' if chars.peek() == Some(&'*') => {
                    chars.next();
                    state = State::Block(1);
                }
                '"' => {
                    out.push(c);
                    state = State::Str;
                }
                'r' if matches!(chars.peek(), Some('#') | Some('"')) => {
                    out.push(c);
                    let mut hashes = 0usize;
                    while chars.peek() == Some(&'#') {
                        out.push('#');
                        chars.next();
                        hashes += 1;
                    }
                    if chars.peek() == Some(&'"') {
                        out.push('"');
                        chars.next();
                        state = State::Raw(hashes);
                    }
                }
                '\'' => {
                    let next = chars.peek().copied();
                    let is_char_literal = match next {
                        Some('\\') => true,
                        Some(n) => {
                            !n.is_alphanumeric()
                                && !n.eq_ignore_ascii_case(&'_')
                                && chars.clone().nth(1) == Some('\'')
                        }
                        None => false,
                    };
                    out.push(c);
                    if is_char_literal {
                        state = State::Char;
                    }
                }
                _ => out.push(c),
            },
            State::Line => {
                if c == '\r' || c == '\n' {
                    out.push(c);
                }
                if c == '\n' {
                    state = State::Code;
                }
            }
            State::Block(depth) => {
                if c == '\r' || c == '\n' {
                    out.push(c);
                } else if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    state = if depth == 1 {
                        State::Code
                    } else {
                        State::Block(depth - 1)
                    };
                } else if c == '/' && chars.peek() == Some(&'*') {
                    chars.next();
                    state = State::Block(depth + 1);
                }
            }
            State::Str | State::Char => {
                out.push(c);
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if (state == State::Str && c == '"') || (state == State::Char && c == '\'') {
                    state = State::Code;
                }
            }
            State::Raw(hashes) => {
                out.push(c);
                if c == '"' {
                    let mut seen = 0usize;
                    while chars.peek() == Some(&'#') && seen < hashes {
                        out.push('#');
                        chars.next();
                        seen += 1;
                    }
                    if seen == hashes {
                        state = State::Code;
                    }
                }
            }
        }
    }
    out
}

/// The part of an already-stripped source file that ships, without its inline
/// test module.
///
/// `#[cfg(test)]` on `mod tests;` declares a file a walk skips anyway;
/// `#[cfg(test)]` on `mod tests {` opens one right here. Only the second is a
/// place to stop reading. `source` must be [`code_of`]'s output: a comment can
/// move a boundary taken from raw text.
pub fn shipped_part(source: &str) -> &str {
    let mut at = 0;
    while let Some(found) = source[at..].find("#[cfg(test)]") {
        let start = at + found;
        let rest = &source[start..];
        let inline = rest
            .find("mod tests")
            .is_some_and(|mods| rest[mods..].starts_with("mod tests {"));
        if inline {
            return &source[..start];
        }
        at = start + "#[cfg(test)]".len();
    }
    source
}

#[cfg(test)]
mod tests {
    use super::code_of;

    #[test]
    fn a_trailing_line_comment_is_removed_but_the_code_before_it_stays() {
        let stripped = code_of("let x = 1; // set x\n");
        assert!(stripped.contains("let x = 1;"));
        assert!(!stripped.contains("set x"));
    }

    #[test]
    fn a_doc_comment_is_removed() {
        let stripped = code_of("/// what this does\nfn f() {}\n");
        assert!(!stripped.contains("what this does"));
        assert!(stripped.contains("fn f() {}"));
    }

    #[test]
    fn an_inner_doc_comment_is_removed() {
        let stripped = code_of("//! module-level narrative\nmod m {}\n");
        assert!(!stripped.contains("module-level narrative"));
        assert!(stripped.contains("mod m {}"));
    }

    #[test]
    fn a_nested_block_comment_is_removed_entirely() {
        let stripped = code_of("/* outer /* inner */ still-outer */ code();\n");
        assert!(!stripped.contains("outer"));
        assert!(!stripped.contains("inner"));
        assert!(!stripped.contains("still-outer"));
        assert!(stripped.contains("code();"));
    }

    #[test]
    fn a_slash_slash_inside_an_ordinary_string_literal_survives() {
        let stripped = code_of("let url = \"http://example.com\";\n");
        assert!(stripped.contains("\"http://example.com\""));
    }

    #[test]
    fn a_slash_slash_inside_a_raw_string_survives() {
        let stripped = code_of("let url = r#\"http://example.com\"#;\n");
        assert!(stripped.contains("r#\"http://example.com\"#"));
    }

    #[test]
    fn a_char_literal_holding_a_slash_does_not_open_a_comment() {
        let stripped = code_of("let c = '/'; let after = 1; // trailing\n");
        assert!(stripped.contains("let c = '/'; let after = 1;"));
        assert!(!stripped.contains("trailing"));
    }

    #[test]
    fn a_backslash_char_literal_closes_correctly() {
        let stripped = code_of("let c = '\\\\'; // trailing\n");
        assert!(stripped.contains("let c = '\\\\';"));
        assert!(!stripped.contains("trailing"));
    }

    #[test]
    fn line_count_is_unchanged() {
        let source = "fn f() {\n    // a comment\n    let x = 1; /* inline */ let y = 2;\n}\n";
        let stripped = code_of(source);
        assert_eq!(stripped.lines().count(), source.lines().count());
        assert_eq!(stripped.matches('\n').count(), source.matches('\n').count());
    }

    #[test]
    fn a_multi_hash_raw_string_holding_a_pound_quote_survives() {
        let stripped = code_of("let s = r##\"a \"# quote\"##; // trailing\n");
        assert!(stripped.contains("r##\"a \"# quote\"##;"));
        assert!(!stripped.contains("trailing"));
    }

    #[test]
    fn a_double_quote_inside_a_char_literal_does_not_open_a_string() {
        let stripped = code_of("let c = '\"'; // trailing\n");
        assert!(stripped.contains("let c = '\"';"));
        assert!(!stripped.contains("trailing"));
    }

    #[test]
    fn a_lifetime_adjacent_to_a_char_literal_is_not_mistaken_for_one() {
        let stripped = code_of("fn f<'a>(s: &'a str) { let x = 'x'; } // trailing\n");
        assert!(stripped.contains("fn f<'a>(s: &'a str) { let x = 'x'; }"));
        assert!(!stripped.contains("trailing"));
    }

    #[test]
    fn a_block_comment_marker_inside_a_string_literal_does_not_open_or_close_a_comment() {
        let stripped = code_of("let s = \"/* not a comment */\"; let after = 1;\n");
        assert!(stripped.contains("\"/* not a comment */\""));
        assert!(stripped.contains("let after = 1;"));
    }

    #[test]
    fn a_byte_string_and_a_byte_char_survive() {
        let stripped = code_of("let b = b\"raw // bytes\"; let c = b'\\0'; // trailing\n");
        assert!(stripped.contains("let b = b\"raw // bytes\"; let c = b'\\0';"));
        assert!(!stripped.contains("trailing"));
    }

    #[test]
    fn an_escaped_single_quote_char_literal_closes_correctly() {
        let stripped = code_of("let c = '\\''; // trailing\n");
        assert!(stripped.contains("let c = '\\'';"));
        assert!(!stripped.contains("trailing"));
    }

    #[test]
    fn an_unterminated_block_comment_at_end_of_file_consumes_the_rest_and_does_not_panic() {
        let stripped = code_of("let x = 1;\n/* never closed");
        assert!(stripped.contains("let x = 1;"));
        assert!(!stripped.contains("never closed"));
    }

    #[test]
    fn crlf_line_endings_round_trip_through_a_line_comment() {
        let source = "let x = 1; // set x\r\nlet y = 2;\r\n";
        let stripped = code_of(source);
        assert!(stripped.contains("let x = 1; \r\nlet y = 2;\r\n"));
        assert!(!stripped.contains("set x"));
    }

    #[test]
    fn crlf_line_endings_round_trip_through_a_block_comment() {
        let source = "let x = 1; /* held\r\nover two lines */ let y = 2;\r\n";
        let stripped = code_of(source);
        assert!(!stripped.contains("held"));
        assert!(stripped.contains("let x = 1; "));
        assert!(stripped.contains(" let y = 2;\r\n"));
        assert_eq!(
            stripped.matches("\r\n").count(),
            source.matches("\r\n").count()
        );
    }
}
