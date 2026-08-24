//! Heuristic compression pipeline.
//!
//! Boundary rule (`docs/ARCHITECTURE.md`): nothing here may reference
//! `crate::mcp::`. This module is a plain library — give it a `&str`, get a
//! [`Report`] back. That is what makes every heuristic testable without a
//! protocol in the way.

pub mod block;
pub mod comments;
pub mod declaration;
pub mod docstring;
pub mod extract;
pub mod guard;
pub mod imports;
pub mod lang;
pub mod signatures;
pub mod whitespace;

use std::path::Path;

pub use extract::Snippet;
pub use guard::GuardError;
pub use lang::Language;

/// Above this many bytes *after* the cheap passes, fall back to outline mode.
///
/// Measured, not guessed (ADR-010). `bench/threshold.py` swept this over 274
/// real files from three projects: 24 KB left 69.4% of the corpus saved, 8 KB
/// leaves 79.0%, and going below 8 KB buys 3.7 more points while nearly
/// doubling how many files lose their bodies. This is the knee of that curve.
pub const OUTLINE_THRESHOLD_BYTES: usize = 8 * 1024;

/// Result of compressing one source.
#[derive(Debug)]
pub struct Report {
    pub text: String,
    pub language: Language,
    pub original_bytes: usize,
    pub compressed_bytes: usize,
    /// True when bodies were elided because the file was still too large.
    pub outlined: bool,
}

impl Report {
    /// Fraction of the original size retained, in `0.0..=1.0`.
    ///
    /// An empty input is defined as a ratio of 1.0 — nothing was saved because
    /// there was nothing to save.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        if self.original_bytes == 0 {
            return 1.0;
        }
        // Precision loss is irrelevant for a display percentage.
        #[allow(clippy::cast_precision_loss)]
        {
            self.compressed_bytes as f64 / self.original_bytes as f64
        }
    }

    /// Percentage of bytes removed, rounded for display.
    #[must_use]
    pub fn saved_percent(&self) -> f64 {
        ((1.0 - self.ratio()) * 1000.0).round() / 10.0
    }
}

/// Run the heuristic passes over `source`.
///
/// Order matters: comments go first so their content cannot look like an
/// import or a declaration to later passes.
#[must_use]
pub fn compress_str(source: &str, language: Language, outline_threshold: usize) -> Report {
    let original_bytes = source.len();

    let without_comments = comments::strip(source, language);
    let split = imports::split(&without_comments, language);
    let mut body = whitespace::collapse(&split.body);

    let outlined = body.len() > outline_threshold;
    if outlined {
        body = whitespace::collapse(&signatures::outline(&body, language));
    }

    // Module docs and inner attributes must stay above the hoisted imports,
    // or the output is not valid source any more.
    let (preamble, body) = imports::split_preamble(&body);
    let mut text = preamble;
    text.push_str(&imports::render(&split.imports));
    text.push_str(&body);

    Report {
        language,
        original_bytes,
        compressed_bytes: text.len(),
        text,
        outlined,
    }
}

/// Validate `requested` against `root` and pull one symbol out of it whole.
///
/// The counterpart to outline mode: what the outline elides, this restores on
/// demand. Returns `Ok(None)` when the file is readable but holds no such
/// symbol — that is an answer, not a failure.
///
/// # Errors
/// Any [`GuardError`] from the trust boundary; nothing is read until the path
/// passes validation.
pub fn extract_symbol(
    requested: &str,
    root: &Path,
    name: &str,
) -> Result<Option<Snippet>, GuardError> {
    let path = guard::resolve(requested, root)?;
    let source = guard::read_text(&path)?;
    Ok(extract::symbol(&source, Language::from_path(&path), name))
}

/// Validate `requested` against `root`, read it, and compress it.
///
/// # Errors
/// Any [`GuardError`] from the trust boundary; nothing is read until the path
/// passes validation.
pub fn compress_file(
    requested: &str,
    root: &Path,
    outline_threshold: usize,
) -> Result<Report, GuardError> {
    let path = guard::resolve(requested, root)?;
    let source = guard::read_text(&path)?;
    Ok(compress_str(
        &source,
        Language::from_path(&path),
        outline_threshold,
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    const RUST_SAMPLE: &str = "\
// internal note
use std::fmt;


/// Public API.
pub fn add(a: i32, b: i32) -> i32 {
    // adds them
    a + b   
}
use std::fmt;
";

    #[test]
    fn end_to_end_removes_noise_and_keeps_signal() {
        let report = compress_str(RUST_SAMPLE, Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert!(report.text.contains("use std::fmt;"), "{}", report.text);
        assert!(report.text.contains("/// Public API."), "{}", report.text);
        assert!(
            report.text.contains("pub fn add(a: i32, b: i32) -> i32 {"),
            "{}",
            report.text
        );
        assert!(!report.text.contains("internal note"), "{}", report.text);
        assert!(!report.text.contains("adds them"), "{}", report.text);
    }

    #[test]
    fn module_docs_stay_above_the_hoisted_imports() {
        // Regression: hoisting once pushed `//!` below the `use` lines, which
        // makes the output fail to compile.
        let source = "//! Module doc.\nuse std::fmt;\nfn f() {}\n";
        let report = compress_str(source, Language::Rust, OUTLINE_THRESHOLD_BYTES);
        let doc_at = report.text.find("//! Module doc.").expect("doc kept");
        let use_at = report.text.find("use std::fmt;").expect("import kept");
        assert!(
            doc_at < use_at,
            "doc must precede imports:\n{}",
            report.text
        );
    }

    #[test]
    fn duplicate_imports_appear_once() {
        let report = compress_str(RUST_SAMPLE, Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert_eq!(
            report.text.matches("use std::fmt;").count(),
            1,
            "{}",
            report.text
        );
    }

    #[test]
    fn compression_actually_shrinks_the_input() {
        let report = compress_str(RUST_SAMPLE, Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert!(report.compressed_bytes < report.original_bytes);
        assert!(report.ratio() < 1.0);
        assert!(report.saved_percent() > 0.0);
    }

    #[test]
    fn does_not_outline_when_under_the_threshold() {
        let report = compress_str(RUST_SAMPLE, Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert!(!report.outlined);
        assert!(report.text.contains("a + b"), "{}", report.text);
    }

    #[test]
    fn outlines_when_over_the_threshold() {
        // Threshold of 1 byte forces the last rung of the ladder.
        let report = compress_str(RUST_SAMPLE, Language::Rust, 1);
        assert!(report.outlined);
        assert!(report.text.contains("pub fn add"), "{}", report.text);
        assert!(!report.text.contains("a + b"), "{}", report.text);
    }

    #[test]
    fn empty_input_is_handled_without_dividing_by_zero() {
        let report = compress_str("", Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert_eq!(report.original_bytes, 0);
        assert!((report.ratio() - 1.0).abs() < f64::EPSILON);
        assert_eq!(report.text, "");
    }

    #[test]
    fn unknown_languages_still_get_whitespace_compression() {
        let report = compress_str("a\n\n\n\n\nb\n", Language::Other, OUTLINE_THRESHOLD_BYTES);
        assert_eq!(report.text, "a\n\nb\n");
    }

    #[test]
    fn extraction_goes_through_the_same_trust_boundary_as_compression() {
        // The guard is not optional on the second entry point.
        let root = std::env::temp_dir();
        let error = extract_symbol("../../../etc/hosts", &root, "anything")
            .expect_err("traversal must be refused");
        assert!(matches!(
            error,
            GuardError::OutsideRoot | GuardError::NotFound
        ));
    }

    #[test]
    fn compressing_a_missing_file_reports_a_guard_error() {
        let root = std::env::temp_dir();
        let error = compress_file(
            "definitely-not-here-9f2a.rs",
            &root,
            OUTLINE_THRESHOLD_BYTES,
        )
        .expect_err("missing file must fail");
        assert_eq!(error, GuardError::NotFound);
    }
}
