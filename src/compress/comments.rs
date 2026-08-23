//! Comment stripping that keeps documentation (ADR-006).
//!
//! Inline comments mostly restate the line below them; doc comments carry
//! intent an agent cannot re-derive from a signature. So: `//` and `#` go,
//! `///`, `//!` and `/** */` stay.
//!
//! The scanner is quote-aware, so a `//` inside a string literal (a URL, a
//! path) is not mistaken for a comment. It is still line-based — see ADR-004
//! for why tree-sitter is deferred, and `docs/BACKLOG.md` for the known gaps.

use super::lang::Language;

/// What a quote-aware scan of one line found first.
enum Found {
    LineComment(usize),
    BlockOpen(usize),
    Nothing,
}

/// Scan `line` for the first comment marker that is *not* inside a string.
fn scan(line: &str, marker: &str, blocks: bool, quote_chars: &[char]) -> Found {
    let mut in_string: Option<char> = None;
    let mut escaped = false;

    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(quote) = in_string {
            if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_string = None;
            }
            continue;
        }

        // Outside a string literal: a marker here is a real comment.
        if quote_chars.contains(&ch) {
            in_string = Some(ch);
            continue;
        }
        let rest = line.get(index..).unwrap_or_default();
        if blocks && rest.starts_with("/*") {
            return Found::BlockOpen(index);
        }
        if rest.starts_with(marker) {
            return Found::LineComment(index);
        }
    }
    Found::Nothing
}

/// Quote characters that open a string literal in this language.
///
/// ponytail: Rust omits `'` on purpose — lifetimes (`&'a str`) are far more
/// common than char literals containing a quote, and treating `'a` as an open
/// string would swallow the rest of the line. Cost: `'"'` confuses the scanner.
/// Upgrade path is tree-sitter (ADR-004), not more special cases here.
const fn quote_chars(lang: Language) -> &'static [char] {
    match lang {
        Language::Rust => &['"'],
        Language::JavaScript | Language::TypeScript => &['"', '\'', '`'],
        Language::Python | Language::Go | Language::Other => &['"', '\''],
    }
}

fn is_doc_line(trimmed: &str, lang: Language) -> bool {
    lang.doc_prefixes()
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

/// Remove non-documentation comments from `source`.
///
/// Returns the source unchanged for languages with no known comment syntax.
#[must_use]
pub fn strip(source: &str, lang: Language) -> String {
    let Some(marker) = lang.line_comment() else {
        return source.to_owned();
    };
    let quotes = quote_chars(lang);
    let blocks = lang.has_block_comments();

    let mut out = String::with_capacity(source.len());
    let mut in_block = false;
    let mut block_is_doc = false;

    for (number, line) in source.lines().enumerate() {
        // A shebang is executable metadata, not a comment.
        if number == 0 && line.starts_with("#!") {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if in_block {
            if block_is_doc {
                out.push_str(line);
                out.push('\n');
            }
            if line.contains("*/") {
                in_block = false;
                block_is_doc = false;
            }
            continue;
        }

        let trimmed = line.trim_start();

        // Doc comments survive verbatim, including their indentation.
        if is_doc_line(trimmed, lang) {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        match scan(line, marker, blocks, quotes) {
            Found::Nothing => {
                out.push_str(line);
                out.push('\n');
            }
            Found::LineComment(index) => {
                let (code, _) = line.split_at(index);
                let code = code.trim_end();
                if !code.is_empty() {
                    out.push_str(code);
                    out.push('\n');
                }
                // A comment-only line is dropped entirely.
            }
            Found::BlockOpen(index) => {
                let (code, comment) = line.split_at(index);
                let is_doc = comment.starts_with("/**");
                let closes_here = comment.get(2..).unwrap_or_default().contains("*/");

                if is_doc {
                    out.push_str(line);
                    out.push('\n');
                } else {
                    let code = code.trim_end();
                    if !code.is_empty() {
                        out.push_str(code);
                        out.push('\n');
                    }
                }
                if !closes_here {
                    in_block = true;
                    block_is_doc = is_doc;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn drops_a_comment_only_line() {
        assert_eq!(
            strip("// noise\nlet x = 1;\n", Language::Rust),
            "let x = 1;\n"
        );
    }

    #[test]
    fn drops_a_trailing_comment_but_keeps_the_code() {
        assert_eq!(strip("let x = 1; // why\n", Language::Rust), "let x = 1;\n");
    }

    #[test]
    fn keeps_rust_doc_comments() {
        let src = "/// Adds two numbers.\n//! Module doc.\nfn add() {}\n";
        assert_eq!(strip(src, Language::Rust), src);
    }

    #[test]
    fn keeps_a_doc_block_comment_and_drops_a_plain_one() {
        let doc = "/**\n * Kept.\n */\nfn f() {}\n";
        assert_eq!(strip(doc, Language::Rust), doc);
        assert_eq!(
            strip("/*\n plain\n*/\nfn f() {}\n", Language::Rust),
            "fn f() {}\n"
        );
    }

    #[test]
    fn does_not_strip_a_double_slash_inside_a_string() {
        // The bug this scanner exists to prevent.
        let src = "let url = \"https://example.com/x\";\n";
        assert_eq!(strip(src, Language::Rust), src);
    }

    #[test]
    fn strips_a_comment_that_follows_a_string_containing_slashes() {
        let src = "let url = \"https://example.com\"; // drop me\n";
        assert_eq!(
            strip(src, Language::Rust),
            "let url = \"https://example.com\";\n"
        );
    }

    #[test]
    fn respects_escaped_quotes_when_tracking_strings() {
        let src = "let s = \"a\\\"b\"; // gone\n";
        assert_eq!(strip(src, Language::Rust), "let s = \"a\\\"b\";\n");
    }

    #[test]
    fn a_lifetime_does_not_break_comment_stripping() {
        // Regression guard for the `'` exclusion documented in quote_chars.
        let src = "fn f<'a>(x: &'a str) {} // gone\n";
        assert_eq!(strip(src, Language::Rust), "fn f<'a>(x: &'a str) {}\n");
    }

    #[test]
    fn python_hash_comments_go_but_docstrings_stay() {
        let src = "def f():\n    \"\"\"Doc stays.\"\"\"\n    x = 1  # gone\n# gone too\n";
        let expected = "def f():\n    \"\"\"Doc stays.\"\"\"\n    x = 1\n";
        assert_eq!(strip(src, Language::Python), expected);
    }

    #[test]
    fn python_shebang_survives_on_the_first_line_only() {
        let src = "#!/usr/bin/env python3\nx = 1\n#!not a shebang\n";
        assert_eq!(
            strip(src, Language::Python),
            "#!/usr/bin/env python3\nx = 1\n"
        );
    }

    #[test]
    fn a_hash_inside_a_python_string_is_not_a_comment() {
        let src = "color = \"#ff0000\"\n";
        assert_eq!(strip(src, Language::Python), src);
    }

    #[test]
    fn unknown_languages_are_returned_untouched() {
        let src = "# this might be a markdown heading\n";
        assert_eq!(strip(src, Language::Other), src);
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert_eq!(strip("", Language::Rust), "");
    }

    #[test]
    fn multibyte_content_is_not_corrupted() {
        let src = "let s = \"héllo 世界\"; // gone\n";
        assert_eq!(strip(src, Language::Rust), "let s = \"héllo 世界\";\n");
    }

    #[test]
    #[ignore = "known V1 limit: raw strings need a real lexer (ADR-004)"]
    fn raw_strings_spanning_the_marker_are_a_known_gap() {
        let src = "let s = r#\"a // not a comment\"#;\n";
        assert_eq!(strip(src, Language::Rust), src);
    }
}
