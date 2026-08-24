//! Where a block of code ends.
//!
//! Two rules, because languages use two conventions: brace balance where `{}`
//! delimits, indentation in Python. Split out of `extract.rs` so finding a
//! symbol and bounding it stay separately testable.
//!
//! Both are bounded and both report when they hit their bound. A body that
//! silently stops mid-function is the failure mode ADR-008 exists to prevent:
//! partial output that looks complete is worse than none.

use super::declaration::indent_width;

/// A symbol that runs past this many lines is almost certainly a parsing
/// failure rather than a real definition. Bounded like `MAX_CONTINUATION_LINES`.
pub const MAX_SYMBOL_LINES: usize = 2_000;

/// Net brace depth added by a line.
pub fn brace_delta(line: &str) -> i32 {
    line.chars().fold(0, |depth, ch| match ch {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

/// Last line of a brace-delimited block starting at `index`.
pub fn end_by_braces(lines: &[&str], index: usize) -> (usize, bool) {
    let mut depth = 0_i32;
    let mut opened = false;

    for (offset, line) in lines.iter().enumerate().skip(index) {
        depth += brace_delta(line);
        if depth > 0 {
            opened = true;
        }
        // A declaration with no body at all (`pub const X: u8 = 1;`) ends here.
        if opened && depth <= 0 {
            return (offset, false);
        }
        if offset - index >= MAX_SYMBOL_LINES {
            return (offset, true);
        }
        if !opened && line.trim_end().ends_with(';') {
            return (offset, false);
        }
    }
    // Ran out of file with the block still open: report it rather than hand
    // back a body that silently stops mid-function (ADR-008).
    (lines.len().saturating_sub(1), depth > 0)
}

/// Last line of an indentation-delimited block starting at `index`.
pub fn end_by_indent(lines: &[&str], index: usize) -> (usize, bool) {
    let base = lines.get(index).map_or(0, |l| indent_width(l));
    let mut last = index;

    for (offset, line) in lines.iter().enumerate().skip(index + 1) {
        if line.trim().is_empty() {
            continue; // blank lines inside a body do not end it
        }
        if indent_width(line) <= base {
            return (last, false);
        }
        last = offset;
        if offset - index >= MAX_SYMBOL_LINES {
            return (last, true);
        }
    }
    (last, false)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::indexing_slicing
    )]
    use super::super::extract::symbol;
    use super::super::lang::Language;
    use super::*;

    #[test]
    fn brace_delta_reports_net_depth() {
        assert_eq!(brace_delta("fn f() {"), 1);
        assert_eq!(brace_delta("}"), -1);
        assert_eq!(brace_delta("if x { y() }"), 0);
    }

    #[test]
    fn a_nested_block_does_not_end_the_outer_one() {
        let lines = vec![
            "fn f() {",
            "    if x {",
            "        y();",
            "    }",
            "    z();",
            "}",
            "after",
        ];
        assert_eq!(end_by_braces(&lines, 0), (5, false));
    }

    #[test]
    fn indentation_ends_a_block_at_the_first_line_back_at_base() {
        let lines = vec!["def f():", "    a()", "    b()", "def g():"];
        assert_eq!(end_by_indent(&lines, 0), (2, false));
    }

    #[test]
    fn a_trailing_blank_line_is_not_part_of_the_block() {
        let lines = vec!["def f():", "    a()", "", "def g():"];
        assert_eq!(end_by_indent(&lines, 0), (1, false));
    }

    #[test]
    fn a_runaway_block_is_bounded_and_flagged() {
        let mut src = String::from("def huge():\n");
        for _ in 0..MAX_SYMBOL_LINES + 50 {
            src.push_str("    work()\n");
        }
        let got = symbol(&src, Language::Python, "huge").expect("found");
        assert!(got.truncated, "should report truncation");
        assert!(got.text.lines().count() <= MAX_SYMBOL_LINES + 2);
    }

    #[test]
    fn an_unclosed_rust_block_is_flagged_rather_than_silently_short() {
        let got = symbol("fn broken() {\n    work();\n", Language::Rust, "broken").expect("found");
        assert!(got.truncated, "unbalanced braces must be reported");
    }
}
