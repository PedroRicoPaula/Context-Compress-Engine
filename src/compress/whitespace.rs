//! Whitespace reduction: trailing spaces and runs of blank lines.
//!
//! Cheap, language-agnostic, and safe for every file type — this is the only
//! pass that also runs on `Language::Other`.

/// Consecutive blank lines collapse to at most this many.
const MAX_CONSECUTIVE_BLANKS: usize = 1;

/// Trim trailing whitespace and collapse blank-line runs.
///
/// Leading indentation is preserved: it carries block structure in Python and
/// readability everywhere else.
#[must_use]
pub fn collapse(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut blank_run = 0_usize;

    for line in source.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run > MAX_CONSECUTIVE_BLANKS {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(trimmed);
        out.push('\n');
    }

    // Leading and trailing blank lines carry nothing.
    let trimmed = out.trim_matches('\n');
    if trimmed.is_empty() {
        String::new()
    } else {
        let mut result = String::with_capacity(trimmed.len() + 1);
        result.push_str(trimmed);
        result.push('\n');
        result
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn collapses_runs_of_blank_lines_to_one() {
        assert_eq!(collapse("a\n\n\n\n\nb\n"), "a\n\nb\n");
    }

    #[test]
    fn keeps_a_single_blank_line_as_a_separator() {
        assert_eq!(collapse("a\n\nb\n"), "a\n\nb\n");
    }

    #[test]
    fn strips_trailing_whitespace_but_keeps_indentation() {
        assert_eq!(collapse("    indented   \n\tt\t\n"), "    indented\n\tt\n");
    }

    #[test]
    fn drops_leading_and_trailing_blank_lines() {
        assert_eq!(collapse("\n\n\ncode\n\n\n"), "code\n");
    }

    #[test]
    fn whitespace_only_lines_count_as_blank() {
        assert_eq!(collapse("a\n   \n\t\n\nb\n"), "a\n\nb\n");
    }

    #[test]
    fn empty_and_whitespace_only_input_yield_empty_output() {
        assert_eq!(collapse(""), "");
        assert_eq!(collapse("\n\n\n"), "");
        assert_eq!(collapse("   \n\t  \n"), "");
    }

    #[test]
    fn output_always_ends_with_exactly_one_newline() {
        for input in ["a", "a\n", "a\n\n\n"] {
            let got = collapse(input);
            assert!(got.ends_with('\n'), "{input:?} -> {got:?}");
            assert!(!got.ends_with("\n\n"), "{input:?} -> {got:?}");
        }
    }

    #[test]
    fn crlf_line_endings_are_normalised() {
        // \r is trailing whitespace, so it goes with the trim_end.
        assert_eq!(collapse("a\r\nb\r\n"), "a\nb\n");
    }

    #[test]
    fn multibyte_characters_survive_intact() {
        // Guards against byte-offset slicing: these are 2-4 byte chars.
        assert_eq!(collapse("héllo → 世界  \n"), "héllo → 世界\n");
    }

    #[test]
    fn is_idempotent() {
        let once = collapse("a\n\n\n\nb   \n\n");
        assert_eq!(collapse(&once), once);
    }
}
