//! What counts as a declaration, and how far one reaches.
//!
//! Split out of `signatures.rs` so the outline loop reads as a state machine
//! and these predicates can be tested on their own.
//!
//! The parenthesis tracking here is what stops a signature being truncated
//! mid-argument. A half-signature is worse than no signature: it reads as
//! complete, so a caller trusts it and gets the argument list wrong.

use super::lang::Language;

/// Safety valve: a declaration whose parentheses never balance (a stray `(`
/// inside a string) must not swallow the rest of the file.
pub const MAX_CONTINUATION_LINES: usize = 24;

pub fn indent_width(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Net parenthesis depth added by this line.
///
/// Deliberately naive: parentheses inside string literals count. The
/// continuation cap above bounds the damage when that misfires.
pub fn paren_delta(line: &str) -> i32 {
    line.chars().fold(0, |depth, ch| match ch {
        '(' => depth + 1,
        ')' => depth - 1,
        _ => depth,
    })
}

pub fn is_declaration(trimmed: &str, lang: Language) -> bool {
    lang.signature_prefixes()
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

pub fn is_doc(trimmed: &str, lang: Language) -> bool {
    if lang
        .doc_prefixes()
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return true;
    }
    // A continuation `*` is only a doc line where `/** */` blocks exist.
    lang.has_block_comments() && (trimmed.starts_with("/**") || trimmed.starts_with('*'))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::indexing_slicing
    )]
    use super::super::signatures::outline;
    use super::*;

    #[test]
    fn indent_width_counts_leading_whitespace() {
        assert_eq!(indent_width("no indent"), 0);
        assert_eq!(indent_width("    four"), 4);
        assert_eq!(indent_width("\tone tab"), 1);
    }

    #[test]
    fn paren_delta_reports_net_depth() {
        assert_eq!(paren_delta("def f(a, b):"), 0);
        assert_eq!(paren_delta("def f(a,"), 1);
        assert_eq!(paren_delta("    b):"), -1);
    }

    #[test]
    fn declaration_prefixes_need_their_trailing_space() {
        assert!(is_declaration("fn add()", Language::Rust));
        assert!(!is_declaration("fnord()", Language::Rust));
        assert!(is_declaration("def f():", Language::Python));
        assert!(!is_declaration("default = 1", Language::Python));
    }

    #[test]
    fn a_bare_star_is_a_doc_line_only_where_block_comments_exist() {
        // Regression: `*` used to count in every language, so Python docstring
        // bullets survived as orphans with no text around them.
        assert!(is_doc("* Kept.", Language::Rust));
        assert!(!is_doc("* 'propagate': returns nan", Language::Python));
    }

    #[test]
    fn follows_a_multi_line_signature_to_its_closing_paren() {
        // Regression: a truncated signature reads as complete and is worse
        // than none. Found on scipy's ttest_ind.
        let src = "def ttest_ind(a, b, *, axis=0,\n              alternative='two-sided', trim=0):\n    return 1\n";
        let got = outline(src, Language::Python);
        assert!(got.contains("alternative='two-sided'"), "{got}");
        assert!(got.contains("trim=0):"), "{got}");
        assert!(!got.contains("return 1"), "{got}");
    }

    #[test]
    fn every_kept_signature_closes_before_the_next_declaration() {
        // Accumulate across the block: `def f(` is legitimately open on its own
        // line and closes further down. What must never happen is a signature
        // that is still open when the next declaration or the file arrives.
        let src = "def f(\n    a,\n    b,\n):\n    body()\n\ndef g(x):\n    body()\n";
        let got = outline(src, Language::Python);
        let mut depth = 0_i32;
        for line in got.lines() {
            let starts_declaration = line.trim_start().starts_with("def ");
            assert!(
                !(starts_declaration && depth > 0),
                "signature still open at: {line}\n{got}"
            );
            depth += paren_delta(line);
            depth = depth.max(0);
        }
        assert_eq!(depth, 0, "signature left open at end of file:\n{got}");
    }

    #[test]
    fn follows_a_multi_line_rust_signature() {
        let src = "fn long(\n    a: i32,\n    b: i32,\n) -> i32 {\n    a\n}\n";
        let got = outline(src, Language::Rust);
        assert!(got.contains("a: i32"), "{got}");
        assert!(got.contains(") -> i32 {"), "{got}");
        assert!(!got.contains("\n    a\n"), "{got}");
    }

    #[test]
    fn follows_a_multi_line_decorator() {
        let src =
            "@factory(pack_result, default_axis=0,\n         n_samples=2)\ndef f(x):\n    pass\n";
        let got = outline(src, Language::Python);
        assert!(got.contains("n_samples=2)"), "{got}");
    }

    #[test]
    fn an_unbalanced_paren_cannot_swallow_the_file() {
        // A stray `(` inside a string must not absorb everything after it.
        let mut src = String::from("def f(x = \"(\"):\n");
        for _ in 0..200 {
            src.push_str("    body();\n");
        }
        let got = outline(&src, Language::Python);
        assert!(got.lines().count() < MAX_CONTINUATION_LINES + 4, "{got}");
    }
}
