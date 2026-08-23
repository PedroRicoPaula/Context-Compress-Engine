//! Import hoisting: pull import lines out, dedupe, and group them at the top.
//!
//! Imports are high-signal (they tell an agent what the file depends on) but
//! scattered and often duplicated across a long file. Collecting them costs
//! nothing and makes the dependency surface readable in one glance.

use super::lang::Language;

/// Source split into its import lines and everything else.
pub struct Split {
    pub imports: Vec<String>,
    pub body: String,
}

fn is_import(trimmed: &str, lang: Language) -> bool {
    // Only top-level imports are hoisted: an indented `import` inside a
    // function or a conditional block is control flow, and moving it would
    // change behaviour.
    lang.import_prefixes()
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

/// Extract top-level import lines, preserving first-seen order and dropping
/// exact duplicates.
#[must_use]
pub fn split(source: &str, lang: Language) -> Split {
    if lang.import_prefixes().is_empty() {
        return Split {
            imports: Vec::new(),
            body: source.to_owned(),
        };
    }

    let mut imports: Vec<String> = Vec::new();
    let mut body = String::with_capacity(source.len());

    for line in source.lines() {
        // Indentation distinguishes a top-level import from a scoped one.
        let is_top_level = !line.starts_with(' ') && !line.starts_with('\t');
        let trimmed = line.trim();

        if is_top_level && is_import(trimmed, lang) {
            let owned = trimmed.to_owned();
            if !imports.contains(&owned) {
                imports.push(owned);
            }
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }

    Split { imports, body }
}

/// Split off the file preamble: everything that must stay *above* the imports.
///
/// Hoisting imports to the very top would push a module-level doc comment
/// (`//!`) or inner attribute (`#![...]`) below them, and both are only legal
/// at the top of a file. The result would not compile -- a compressor that
/// emits invalid code has failed at its one job.
///
/// Returns `(preamble, rest)`.
#[must_use]
pub fn split_preamble(source: &str) -> (String, String) {
    let mut preamble = String::new();
    let mut rest = String::new();
    let mut still_preamble = true;

    for line in source.lines() {
        let trimmed = line.trim_start();
        let belongs_up_top = trimmed.starts_with("//!")
            || trimmed.starts_with("#![")
            || trimmed.starts_with("#!")
            || trimmed.is_empty();

        if still_preamble && belongs_up_top {
            preamble.push_str(line);
            preamble.push('\n');
            continue;
        }
        still_preamble = false;
        rest.push_str(line);
        rest.push('\n');
    }

    // A preamble of nothing but blank lines is not a preamble.
    if preamble.trim().is_empty() {
        return (String::new(), source.to_owned());
    }
    (preamble, rest)
}

/// Render hoisted imports as a compact block, or an empty string if there are none.
#[must_use]
pub fn render(imports: &[String]) -> String {
    if imports.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for line in imports {
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn hoists_rust_use_statements_out_of_the_body() {
        let split = split(
            "use std::fmt;\nfn main() {}\nuse std::io;\n",
            Language::Rust,
        );
        assert_eq!(split.imports, vec!["use std::fmt;", "use std::io;"]);
        assert_eq!(split.body, "fn main() {}\n");
    }

    #[test]
    fn drops_exact_duplicates_but_keeps_first_seen_order() {
        let split = split("use b;\nuse a;\nuse b;\n", Language::Rust);
        assert_eq!(split.imports, vec!["use b;", "use a;"]);
    }

    #[test]
    fn leaves_indented_imports_in_the_body() {
        // A scoped `use` inside a function is not a file-level dependency.
        let src = "fn f() {\n    use std::io::Write;\n}\n";
        let split = split(src, Language::Rust);
        assert!(split.imports.is_empty());
        assert_eq!(split.body, src);
    }

    #[test]
    fn handles_python_import_and_from_forms() {
        let split = split("import os\nfrom sys import argv\nx = 1\n", Language::Python);
        assert_eq!(split.imports, vec!["import os", "from sys import argv"]);
        assert_eq!(split.body, "x = 1\n");
    }

    #[test]
    fn does_not_match_a_prefix_without_its_trailing_space() {
        // "used" and "important" must not be mistaken for "use " / "import ".
        let rust = split("used = 1\n", Language::Rust);
        assert!(rust.imports.is_empty());
        let python = split("important = 1\n", Language::Python);
        assert!(python.imports.is_empty());
    }

    #[test]
    fn languages_without_import_syntax_are_untouched() {
        let src = "import whatever\n";
        let split = split(src, Language::Other);
        assert!(split.imports.is_empty());
        assert_eq!(split.body, src);
    }

    #[test]
    fn render_is_empty_for_no_imports() {
        assert_eq!(render(&[]), "");
    }

    #[test]
    fn render_separates_the_block_from_the_body() {
        assert_eq!(render(&["use a;".to_owned()]), "use a;\n\n");
    }

    #[test]
    fn preamble_keeps_module_docs_above_the_imports() {
        let (preamble, rest) = split_preamble("//! Module doc.\n\nfn f() {}\n");
        assert_eq!(preamble, "//! Module doc.\n\n");
        assert_eq!(rest, "fn f() {}\n");
    }

    #[test]
    fn preamble_keeps_inner_attributes_too() {
        let (preamble, _) = split_preamble("#![forbid(unsafe_code)]\nfn f() {}\n");
        assert_eq!(preamble, "#![forbid(unsafe_code)]\n");
    }

    #[test]
    fn a_file_with_no_preamble_yields_an_empty_one() {
        let (preamble, rest) = split_preamble("fn f() {}\n");
        assert!(preamble.is_empty());
        assert_eq!(rest, "fn f() {}\n");
    }

    #[test]
    fn a_doc_comment_further_down_is_not_a_preamble() {
        // Only a leading run counts; `//!` after real code stays where it is.
        let (preamble, _) = split_preamble("fn f() {}\n//! stray\n");
        assert!(preamble.is_empty());
    }

    #[test]
    fn empty_input_produces_empty_output() {
        let split = split("", Language::Rust);
        assert!(split.imports.is_empty());
        assert_eq!(split.body, "");
    }
}
