//! File extension to language, plus the syntax markers each pass needs.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    /// Unknown or non-code: only the whitespace pass applies.
    Other,
}

impl Language {
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(std::ffi::OsStr::to_str) {
            Some("rs") => Self::Rust,
            Some("py" | "pyi") => Self::Python,
            Some("js" | "jsx" | "mjs" | "cjs") => Self::JavaScript,
            Some("ts" | "tsx") => Self::TypeScript,
            Some("go") => Self::Go,
            _ => Self::Other,
        }
    }

    /// Token that starts a line comment, if the language has one.
    #[must_use]
    pub const fn line_comment(self) -> Option<&'static str> {
        match self {
            Self::Rust | Self::JavaScript | Self::TypeScript | Self::Go => Some("//"),
            Self::Python => Some("#"),
            Self::Other => None,
        }
    }

    /// Comment prefixes that carry semantic value and must survive (ADR-006).
    #[must_use]
    pub const fn doc_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["///", "//!"],
            Self::JavaScript | Self::TypeScript | Self::Go => &["///"],
            Self::Python | Self::Other => &[],
        }
    }

    /// Whether `/* */` block comments exist in this language.
    #[must_use]
    pub const fn has_block_comments(self) -> bool {
        matches!(
            self,
            Self::Rust | Self::JavaScript | Self::TypeScript | Self::Go
        )
    }

    /// Line prefixes that denote an import/include statement.
    #[must_use]
    pub const fn import_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["use ", "pub use ", "extern crate "],
            Self::Python => &["import ", "from "],
            Self::JavaScript | Self::TypeScript => &["import ", "export * from ", "const { "],
            Self::Go => &["import "],
            Self::Other => &[],
        }
    }

    /// Line prefixes that denote a declaration worth keeping in outline mode.
    #[must_use]
    pub const fn signature_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "fn ",
                "pub fn ",
                "async fn ",
                "pub async fn ",
                "struct ",
                "pub struct ",
                "enum ",
                "pub enum ",
                "trait ",
                "pub trait ",
                "impl ",
                "type ",
                "pub type ",
                "const ",
                "pub const ",
                "static ",
                "pub static ",
                "macro_rules! ",
                "mod ",
                "pub mod ",
            ],
            Self::Python => &["def ", "async def ", "class ", "@"],
            Self::JavaScript | Self::TypeScript => &[
                "function ",
                "async function ",
                "class ",
                "export function ",
                "export async function ",
                "export class ",
                "export const ",
                "export default ",
                "interface ",
                "export interface ",
                "type ",
                "export type ",
            ],
            Self::Go => &["func ", "type ", "package "],
            Self::Other => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn detects_languages_by_extension() {
        assert_eq!(
            Language::from_path(Path::new("a/b/main.rs")),
            Language::Rust
        );
        assert_eq!(
            Language::from_path(Path::new("script.py")),
            Language::Python
        );
        assert_eq!(
            Language::from_path(Path::new("app.tsx")),
            Language::TypeScript
        );
        assert_eq!(
            Language::from_path(Path::new("mod.mjs")),
            Language::JavaScript
        );
        assert_eq!(Language::from_path(Path::new("srv.go")), Language::Go);
    }

    #[test]
    fn unknown_and_missing_extensions_fall_back_to_other() {
        assert_eq!(Language::from_path(Path::new("notes.txt")), Language::Other);
        assert_eq!(Language::from_path(Path::new("Makefile")), Language::Other);
        assert_eq!(
            Language::from_path(Path::new(".gitignore")),
            Language::Other
        );
    }

    #[test]
    fn extension_matching_is_case_sensitive_and_does_not_match_substrings() {
        // "rust.rs.bak" must not be treated as Rust.
        assert_eq!(
            Language::from_path(Path::new("rust.rs.bak")),
            Language::Other
        );
    }

    #[test]
    fn other_has_no_syntax_markers_so_only_whitespace_applies() {
        assert!(Language::Other.line_comment().is_none());
        assert!(Language::Other.import_prefixes().is_empty());
        assert!(Language::Other.signature_prefixes().is_empty());
    }

    #[test]
    fn python_has_no_block_comments() {
        assert!(!Language::Python.has_block_comments());
        assert!(Language::Rust.has_block_comments());
    }
}
