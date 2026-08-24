//! Python docstring handling for outline mode.
//!
//! A docstring is a *string literal*, not a comment, so the declaration rules
//! in `signatures.rs` never see it — which is how 176 of scipy's docstrings
//! were silently dropped while their bullet lines survived as orphans.
//!
//! What is kept is the summary line, which PEP 257 defines as a complete
//! sentence describing the function. Keeping the whole docstring would undo
//! the compression; keeping none of it removes the only thing that explains
//! what a signature is *for*.
//!
//! The summary is re-emitted as a closed one-line docstring. Emitting an
//! opening `"""` without its closing pair would leave the output as invalid
//! Python — a compressor that emits code that will not parse has failed.

/// Result of feeding the line that closes a docstring.
pub struct Closed {
    /// The one-line summary to emit, if the docstring had one.
    pub rendered: Option<String>,
    /// Whether any content was dropped, so the caller can mark an elision.
    pub elided_any: bool,
}

/// What opening a docstring produced.
pub enum Opening {
    /// `"""One-liner."""` — began and ended on the same line.
    Complete(Closed),
    /// Opened and still running; feed it subsequent lines.
    Ongoing(Docstring),
}

/// A docstring whose body is still streaming past.
pub struct Docstring {
    delimiter: &'static str,
    indent: usize,
    summary: Option<String>,
    elided_any: bool,
}

/// The triple-quote delimiter this line opens a docstring with, if any.
///
/// Accepts the string prefixes Python allows before the quotes (`r`, `b`,
/// `f`, `u`, and their uppercase forms).
fn delimiter_of(trimmed: &str) -> Option<&'static str> {
    let body = trimmed.trim_start_matches(['r', 'R', 'b', 'B', 'u', 'U', 'f', 'F']);
    if body.starts_with("\"\"\"") {
        Some("\"\"\"")
    } else if body.starts_with("'''") {
        Some("'''")
    } else {
        None
    }
}

/// Render `summary` as a closed one-line docstring at `indent` columns.
fn render(indent: usize, summary: &str) -> String {
    // A trailing backslash would escape our own closing quotes.
    let summary = summary.trim().trim_end_matches('\\');
    format!("{:indent$}\"\"\"{summary}\"\"\"", "", indent = indent)
}

impl Docstring {
    /// Try to open a docstring at `line`.
    ///
    /// Returns `None` if the line does not start one.
    #[must_use]
    pub fn open(line: &str) -> Option<Opening> {
        let trimmed = line.trim();
        let delimiter = delimiter_of(trimmed)?;
        let indent = line.len() - line.trim_start().len();
        let after = trimmed.split_once(delimiter).map_or("", |(_, rest)| rest);
        let summary = after.split(delimiter).next().unwrap_or_default().trim();

        if after.contains(delimiter) {
            return Some(Opening::Complete(Closed {
                rendered: (!summary.is_empty()).then(|| render(indent, summary)),
                elided_any: false,
            }));
        }

        Some(Opening::Ongoing(Self {
            delimiter,
            indent,
            summary: (!summary.is_empty()).then(|| summary.to_owned()),
            elided_any: false,
        }))
    }

    /// Feed one line of the docstring body.
    ///
    /// Returns `Some(Closed)` on the line that terminates it, `None` while it
    /// is still open.
    pub fn feed(&mut self, line: &str) -> Option<Closed> {
        let trimmed = line.trim();
        let closes = trimmed.contains(self.delimiter);

        if !trimmed.is_empty() {
            if self.summary.is_none() {
                let text = trimmed.trim_matches(|c| c == '"' || c == '\'').trim();
                if text.is_empty() {
                    self.elided_any = true;
                } else {
                    self.summary = Some(text.to_owned());
                }
            } else {
                self.elided_any = true;
            }
        }

        closes.then(|| Closed {
            rendered: self.summary.as_deref().map(|s| render(self.indent, s)),
            elided_any: self.elided_any,
        })
    }
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

    fn open(line: &str) -> Opening {
        Docstring::open(line).expect("line opens a docstring")
    }

    fn run(lines: &[&str]) -> Closed {
        match open(lines[0]) {
            Opening::Complete(closed) => closed,
            Opening::Ongoing(mut state) => {
                for line in &lines[1..] {
                    if let Some(closed) = state.feed(line) {
                        return closed;
                    }
                }
                panic!("docstring never closed");
            }
        }
    }

    #[test]
    fn recognises_both_quote_styles() {
        assert!(Docstring::open("    \"\"\"Doc.\"\"\"").is_some());
        assert!(Docstring::open("    '''Doc.'''").is_some());
    }

    #[test]
    fn recognises_prefixed_string_literals() {
        assert!(Docstring::open("    r\"\"\"Raw doc.\"\"\"").is_some());
        assert!(Docstring::open("    f'''Formatted.'''").is_some());
    }

    #[test]
    fn ignores_a_line_that_opens_no_docstring() {
        assert!(Docstring::open("    return x").is_none());
        assert!(Docstring::open("    x = \"single quoted\"").is_none());
    }

    #[test]
    fn a_one_line_docstring_closes_immediately() {
        let closed = run(&["    \"\"\"Short and done.\"\"\""]);
        assert_eq!(
            closed.rendered.as_deref(),
            Some("    \"\"\"Short and done.\"\"\"")
        );
        assert!(!closed.elided_any);
    }

    #[test]
    fn keeps_the_summary_from_the_opening_line() {
        let closed = run(&["    \"\"\"Summary here.", "", "    Detail.", "    \"\"\""]);
        assert_eq!(
            closed.rendered.as_deref(),
            Some("    \"\"\"Summary here.\"\"\"")
        );
        assert!(closed.elided_any, "the detail paragraph was dropped");
    }

    #[test]
    fn keeps_the_summary_from_the_line_below_the_opening() {
        let closed = run(&[
            "    \"\"\"",
            "    Compute a thing.",
            "",
            "    Parameters",
            "    \"\"\"",
        ]);
        assert_eq!(
            closed.rendered.as_deref(),
            Some("    \"\"\"Compute a thing.\"\"\"")
        );
    }

    #[test]
    fn output_is_always_a_closed_pair_of_quotes() {
        // An unterminated string would make the whole file invalid Python.
        let closed = run(&["    \"\"\"", "    Summary.", "    More.", "    \"\"\""]);
        let rendered = closed.rendered.expect("summary kept");
        assert_eq!(rendered.matches("\"\"\"").count(), 2, "{rendered}");
    }

    #[test]
    fn a_trailing_backslash_cannot_escape_the_closing_quotes() {
        let closed = run(&[
            "    \"\"\"",
            "    Summary ending in a backslash \\",
            "    \"\"\"",
        ]);
        let rendered = closed.rendered.expect("summary kept");
        assert!(rendered.ends_with("\"\"\""), "{rendered}");
        assert!(!rendered.contains("\\\"\"\""), "{rendered}");
    }

    #[test]
    fn an_empty_docstring_renders_nothing() {
        let closed = run(&["    \"\"\"", "    \"\"\""]);
        assert!(closed.rendered.is_none());
    }

    #[test]
    fn indentation_is_preserved() {
        let closed = run(&["        \"\"\"Nested.\"\"\""]);
        assert_eq!(
            closed.rendered.as_deref(),
            Some("        \"\"\"Nested.\"\"\"")
        );
    }

    #[test]
    fn bullets_after_the_summary_are_dropped() {
        // Regression: these survived as orphan lines with no context.
        let closed = run(&[
            "    \"\"\"",
            "    Summary.",
            "",
            "    * 'propagate': returns nan",
            "    * 'raise': throws",
            "    \"\"\"",
        ]);
        let rendered = closed.rendered.expect("summary kept");
        assert!(!rendered.contains("propagate"), "{rendered}");
        assert!(closed.elided_any);
    }
}
