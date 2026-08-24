//! Pull one named symbol back out of a file, whole.
//!
//! This is the other half of outline mode. On its own, outlining is lossy: the
//! body is gone and the caller has no way back. With extraction, it is only
//! *deferred* — the pack says what exists and where, and whoever reads it asks
//! for the one body they actually need.
//!
//! That changes what compression is allowed to do. Cutting is safe when cutting
//! is reversible, so the outline can be far more aggressive than it could be as
//! a one-way transform.
//!
//! Block ends are found the way each language marks them: by indentation in
//! Python, by brace balance elsewhere. Line-based, like every other pass, for
//! the reasons in ADR-004.

use super::block::{end_by_braces, end_by_indent};
use super::declaration::is_declaration;
use super::lang::Language;

/// One extracted symbol, with the location it came from.
#[derive(Debug, PartialEq, Eq)]
pub struct Snippet {
    pub text: String,
    /// 1-indexed, inclusive, matching what an editor shows.
    pub start_line: usize,
    pub end_line: usize,
    /// True when the block hit [`MAX_SYMBOL_LINES`] before it closed.
    pub truncated: bool,
}

/// The identifier a declaration line declares, if it is one.
///
/// `def ttest_ind(a, b):` → `ttest_ind`. `impl Trait for Type {` → `Trait`,
/// which is imprecise but harmless: the caller asked for a name and gets the
/// block that names it.
fn declared_name(trimmed: &str, lang: Language) -> Option<&str> {
    let prefix = lang
        .signature_prefixes()
        .iter()
        .filter(|p| trimmed.starts_with(**p))
        // Longest prefix first: `pub async fn ` must beat `pub `.
        .max_by_key(|p| p.len())?;

    let rest = trimmed.get(prefix.len()..)?.trim_start();
    let end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    let name = rest.get(..end)?;
    (!name.is_empty()).then_some(name)
}

/// Lines immediately above `index` that belong to the declaration: decorators
/// and doc comments. Returns the first line index to include.
fn attached_start(lines: &[&str], index: usize, lang: Language) -> usize {
    let mut start = index;
    while start > 0 {
        let candidate = lines.get(start - 1).map_or("", |l| l.trim());
        let attached = candidate.starts_with('@')
            || lang.doc_prefixes().iter().any(|p| candidate.starts_with(p))
            || (lang.has_block_comments()
                && (candidate.starts_with("/**")
                    || candidate.starts_with('*')
                    || candidate.starts_with("*/")));
        if !attached {
            break;
        }
        start -= 1;
    }
    start
}

/// Extract the definition of `name` from `source`.
///
/// Returns `None` when no declaration of that name is found. The first match
/// wins: overloads and re-definitions are a known limitation (`BACKLOG.md`).
#[must_use]
pub fn symbol(source: &str, lang: Language, name: &str) -> Option<Snippet> {
    if name.is_empty() || lang.signature_prefixes().is_empty() {
        return None;
    }
    let lines: Vec<&str> = source.lines().collect();

    let index = lines.iter().position(|line| {
        let trimmed = line.trim_start();
        is_declaration(trimmed, lang) && declared_name(trimmed, lang) == Some(name)
    })?;

    let braces = lang.has_block_comments(); // same languages that use `{}` blocks
    let (end, truncated) = if braces {
        end_by_braces(&lines, index)
    } else {
        end_by_indent(&lines, index)
    };

    let start = attached_start(&lines, index, lang);
    let text = lines.get(start..=end)?.join("\n");

    Some(Snippet {
        text,
        start_line: start + 1,
        end_line: end + 1,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::indexing_slicing
    )]
    use super::*;

    const PY: &str = "\
import os


@cache
def alpha(a, b):
    \"\"\"Docstring.\"\"\"
    x = a + b

    return x


def beta():
    return 1


class Gamma:
    def method(self):
        pass
";

    const RS: &str = "\
use std::fmt;

/// Adds two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    if a > b {
        return a;
    }
    a + b
}

pub const LIMIT: u8 = 9;

struct S {
    field: u8,
}
";

    #[test]
    fn extracts_a_python_function_with_its_whole_body() {
        let got = symbol(PY, Language::Python, "alpha").expect("found");
        assert!(got.text.contains("def alpha(a, b):"), "{}", got.text);
        assert!(got.text.contains("return x"), "{}", got.text);
        assert!(!got.text.contains("def beta"), "{}", got.text);
    }

    #[test]
    fn includes_decorators_and_docstrings_above_the_declaration() {
        let got = symbol(PY, Language::Python, "alpha").expect("found");
        assert!(got.text.starts_with("@cache"), "{}", got.text);
        assert!(got.text.contains("\"\"\"Docstring.\"\"\""), "{}", got.text);
    }

    #[test]
    fn a_blank_line_inside_a_body_does_not_end_it() {
        // `alpha` has a blank line before its return.
        let got = symbol(PY, Language::Python, "alpha").expect("found");
        assert!(got.text.contains("return x"), "{}", got.text);
    }

    #[test]
    fn reports_the_line_range_it_came_from() {
        let got = symbol(PY, Language::Python, "beta").expect("found");
        let lines: Vec<&str> = PY.lines().collect();
        assert_eq!(lines[got.start_line - 1].trim(), "def beta():");
        assert_eq!(lines[got.end_line - 1].trim(), "return 1");
    }

    #[test]
    fn extracts_a_python_class() {
        let got = symbol(PY, Language::Python, "Gamma").expect("found");
        assert!(got.text.contains("class Gamma:"), "{}", got.text);
        assert!(got.text.contains("def method(self):"), "{}", got.text);
    }

    #[test]
    fn extracts_a_rust_function_by_brace_balance() {
        let got = symbol(RS, Language::Rust, "add").expect("found");
        assert!(
            got.text.contains("pub fn add(a: i32, b: i32) -> i32 {"),
            "{}",
            got.text
        );
        assert!(got.text.contains("a + b"), "{}", got.text);
        assert!(got.text.trim_end().ends_with('}'), "{}", got.text);
        // The nested `if` block must not end the function early.
        assert_eq!(got.text.matches('{').count(), got.text.matches('}').count());
    }

    #[test]
    fn keeps_the_rust_doc_comment_above_the_function() {
        let got = symbol(RS, Language::Rust, "add").expect("found");
        assert!(
            got.text.starts_with("/// Adds two numbers."),
            "{}",
            got.text
        );
    }

    #[test]
    fn a_declaration_with_no_block_ends_at_its_semicolon() {
        let got = symbol(RS, Language::Rust, "LIMIT").expect("found");
        assert_eq!(got.text.trim(), "pub const LIMIT: u8 = 9;");
    }

    #[test]
    fn extracts_a_struct() {
        let got = symbol(RS, Language::Rust, "S").expect("found");
        assert!(got.text.contains("field: u8"), "{}", got.text);
        assert!(got.text.trim_end().ends_with('}'), "{}", got.text);
    }

    #[test]
    fn returns_none_for_an_unknown_symbol() {
        assert!(symbol(PY, Language::Python, "nope").is_none());
        assert!(symbol(PY, Language::Python, "").is_none());
    }

    #[test]
    fn does_not_match_a_name_that_merely_contains_the_query() {
        // `alph` must not find `alpha`.
        assert!(symbol(PY, Language::Python, "alph").is_none());
    }

    #[test]
    fn does_not_match_a_mention_inside_a_body() {
        let src = "def caller():\n    alpha()\n\ndef alpha():\n    return 1\n";
        let got = symbol(src, Language::Python, "alpha").expect("found");
        assert!(got.text.starts_with("def alpha():"), "{}", got.text);
    }

    #[test]
    fn prefers_the_longest_matching_prefix() {
        let src = "pub async fn fetch(url: &str) -> u8 {\n    0\n}\n";
        let got = symbol(src, Language::Rust, "fetch").expect("found");
        assert!(got.text.starts_with("pub async fn fetch"), "{}", got.text);
    }

    #[test]
    fn languages_without_declaration_syntax_extract_nothing() {
        assert!(symbol("anything at all\n", Language::Other, "anything").is_none());
    }
}
