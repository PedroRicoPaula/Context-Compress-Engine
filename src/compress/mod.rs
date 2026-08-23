//! Heuristic compression pipeline.
//!
//! Boundary rule (`docs/ARCHITECTURE.md`): nothing here may reference
//! `crate::mcp::`. This module is a plain library — give it a `&str`, get a
//! [`Report`] back. That is what makes every heuristic testable without a
//! protocol in the way.

pub mod comments;
pub mod guard;
pub mod imports;
pub mod lang;
pub mod signatures;
pub mod whitespace;

use std::path::Path;

pub use guard::GuardError;
pub use lang::Language;

/// Above this many bytes *after* the cheap passes, fall back to outline mode.
/// Roughly 6k tokens — past that, shape beats detail for an agent's context.
pub const OUTLINE_THRESHOLD_BYTES: usize = 24 * 1024;

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

    let mut text = imports::render(&split.imports);
    text.push_str(&body);

    Report {
        language,
        original_bytes,
        compressed_bytes: text.len(),
        text,
        outlined,
    }
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
    Ok(compress_str(&source, Language::from_path(&path), outline_threshold))
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
        assert!(report.text.contains("pub fn add(a: i32, b: i32) -> i32 {"), "{}", report.text);
        assert!(!report.text.contains("internal note"), "{}", report.text);
        assert!(!report.text.contains("adds them"), "{}", report.text);
    }

    #[test]
    fn duplicate_imports_appear_once() {
        let report = compress_str(RUST_SAMPLE, Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert_eq!(report.text.matches("use std::fmt;").count(), 1, "{}", report.text);
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
    fn compressing_a_missing_file_reports_a_guard_error() {
        let root = std::env::temp_dir();
        let error = compress_file("definitely-not-here-9f2a.rs", &root, OUTLINE_THRESHOLD_BYTES)
            .expect_err("missing file must fail");
        assert_eq!(error, GuardError::NotFound);
    }
}
