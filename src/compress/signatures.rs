//! Outline mode: keep declarations and documentation, elide bodies.
//!
//! This is the last rung of the degradation ladder. A 400k-line file cannot go
//! into an agent's context whole, but its shape — what types exist, what
//! functions they expose, and what each one is for — usually answers the
//! question.
//!
//! Three things make the difference between a useful outline and a misleading
//! one, and all three were found by running this on real code:
//!
//! 1. A declaration that spans lines is followed to its closing parenthesis.
//!    A half-signature is worse than no signature: it reads as complete.
//! 2. A Python docstring is a string, not a comment, so the declaration rules
//!    do not see it. Its summary line is lifted out and re-emitted closed.
//! 3. `*` only starts a doc line in languages that have `/** */` blocks.
//!    In Python it just catches bullets from inside docstrings.

use super::declaration::{
    indent_width, is_declaration, is_doc, paren_delta, MAX_CONTINUATION_LINES,
};
use super::docstring::{Docstring, Opening};
use super::lang::Language;

/// Marker left where a body was removed, in the language's own comment syntax.
///
/// A hard-coded `//` reads as a comment in Rust and JS but is a syntax error in
/// Python — the same class of mistake as hoisting imports above a module doc.
fn elision(lang: Language) -> String {
    format!("    {} ...", lang.line_comment().unwrap_or("//"))
}

/// Reduce `source` to declarations, documentation, and structural closers.
///
/// Returns the source unchanged for languages with no known declaration syntax:
/// guessing at structure would lose more than it saves.
#[must_use]
#[allow(clippy::too_many_lines)] // one state machine; splitting it hides the flow
pub fn outline(source: &str, lang: Language) -> String {
    if lang.signature_prefixes().is_empty() {
        return source.to_owned();
    }
    let python = lang == Language::Python;

    let marker = elision(lang);
    let mut out = String::with_capacity(source.len() / 2);
    let mut elided = false;

    let mut open_parens = 0_i32;
    let mut continuation = 0_usize;
    // A module docstring opens the file, with no declaration before it.
    let mut expect_docstring = python;
    let mut docstring: Option<Docstring> = None;

    for line in source.lines() {
        let trimmed = line.trim();

        // --- inside a multi-line declaration -------------------------------
        if open_parens > 0 {
            push_line(&mut out, &mut elided, &marker, line);
            open_parens += paren_delta(line);
            continuation += 1;
            if open_parens <= 0 || continuation >= MAX_CONTINUATION_LINES {
                open_parens = 0;
                continuation = 0;
                expect_docstring = python;
            }
            continue;
        }

        // --- inside a docstring --------------------------------------------
        if let Some(state) = docstring.as_mut() {
            if let Some(closed) = state.feed(line) {
                docstring = None;
                if let Some(rendered) = closed.rendered {
                    push_line(&mut out, &mut elided, &marker, &rendered);
                }
                elided |= closed.elided_any;
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        // --- a docstring opening right after a declaration ------------------
        if expect_docstring {
            expect_docstring = false;
            match Docstring::open(line) {
                Some(Opening::Complete(closed)) => {
                    if let Some(rendered) = closed.rendered {
                        push_line(&mut out, &mut elided, &marker, &rendered);
                    }
                    continue;
                }
                Some(Opening::Ongoing(state)) => {
                    docstring = Some(state);
                    continue;
                }
                None => {}
            }
        }

        // --- declarations, docs, and top-level closers ----------------------
        let is_top_closer = indent_width(line) == 0 && (trimmed == "}" || trimmed == "};");
        let declares = is_declaration(trimmed, lang);

        if declares || is_doc(trimmed, lang) || is_top_closer {
            push_line(&mut out, &mut elided, &marker, line);
            let delta = paren_delta(line);
            if declares && delta > 0 {
                open_parens = delta;
                continuation = 0;
            } else if declares {
                expect_docstring = python;
            }
        } else {
            elided = true;
        }
    }

    if elided {
        out.push_str(&marker);
        out.push('\n');
    }
    out
}

/// Append `line`, first flushing a pending elision marker.
fn push_line(out: &mut String, elided: &mut bool, marker: &str, line: &str) {
    if *elided {
        out.push_str(marker);
        out.push('\n');
        *elided = false;
    }
    out.push_str(line);
    out.push('\n');
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

    #[test]
    fn keeps_signatures_and_elides_bodies() {
        let src = "fn add(a: i32, b: i32) -> i32 {\n    let c = a + b;\n    c\n}\n";
        let got = outline(src, Language::Rust);
        assert!(got.contains("fn add(a: i32, b: i32) -> i32 {"), "{got}");
        assert!(!got.contains("let c"), "{got}");
        assert!(got.contains("// ..."), "{got}");
    }

    #[test]
    fn keeps_struct_trait_and_impl_headers() {
        let src = "pub struct S {\n    field: u8,\n}\ntrait T {\n    fn m(&self);\n}\nimpl T for S {\n    fn m(&self) {}\n}\n";
        let got = outline(src, Language::Rust);
        for expected in ["pub struct S {", "trait T {", "impl T for S {"] {
            assert!(got.contains(expected), "missing {expected} in {got}");
        }
        assert!(!got.contains("field: u8"), "{got}");
    }

    #[test]
    fn keeps_doc_comments_attached_to_their_declaration() {
        let src = "/// Adds.\nfn add() {\n    body();\n}\n";
        let got = outline(src, Language::Rust);
        assert!(got.starts_with("/// Adds."), "{got}");
    }

    #[test]
    fn keeps_top_level_closing_braces_only() {
        let src = "fn f() {\n    if x {\n        y();\n    }\n}\n";
        assert_eq!(outline(src, Language::Rust).matches('}').count(), 1);
    }

    #[test]
    fn consecutive_elided_lines_collapse_to_one_marker() {
        let src = "fn f() {\n    a();\n    b();\n    c();\n}\n";
        assert_eq!(outline(src, Language::Rust).matches("// ...").count(), 1);
    }

    // --- multi-line declarations --------------------------------------------

    // --- Python docstrings ---------------------------------------------------

    #[test]
    fn keeps_the_docstring_summary_and_elides_the_rest() {
        let src = "def f(x):\n    \"\"\"\n    Compute a thing.\n\n    Parameters\n    ----------\n    x : int\n    \"\"\"\n    return x\n";
        let got = outline(src, Language::Python);
        assert!(got.contains("Compute a thing."), "{got}");
        assert!(!got.contains("Parameters"), "{got}");
        assert!(!got.contains("return x"), "{got}");
    }

    #[test]
    fn keeps_the_module_docstring() {
        let src = "\"\"\"This module does things.\"\"\"\nimport os\ndef f():\n    pass\n";
        assert!(outline(src, Language::Python).contains("This module does things."));
    }

    #[test]
    fn keeps_the_docstring_of_a_multi_line_signature() {
        let src = "def f(\n    a,\n    b,\n):\n    \"\"\"Summary.\"\"\"\n    pass\n";
        let got = outline(src, Language::Python);
        assert!(got.contains("Summary."), "{got}");
    }

    // --- no orphan bullets ---------------------------------------------------

    #[test]
    fn does_not_keep_docstring_bullets_as_orphan_lines() {
        // Regression: `*` was treated as a doc-continuation marker in every
        // language, so scipy's docstring bullets survived without their text.
        let src = "def f(x):\n    \"\"\"\n    Summary.\n\n    * 'propagate': returns nan\n    * 'raise': throws\n    \"\"\"\n    pass\n";
        let got = outline(src, Language::Python);
        assert!(!got.contains("'propagate'"), "{got}");
        assert!(!got.contains("* 'raise'"), "{got}");
    }

    #[test]
    fn a_bare_star_is_still_a_doc_line_in_rust() {
        let src = "/**\n * Kept.\n */\nfn f() {}\n";
        let got = outline(src, Language::Rust);
        assert!(got.contains("* Kept."), "{got}");
    }

    // --- unchanged behaviour -------------------------------------------------

    #[test]
    fn keeps_python_defs_classes_and_decorators() {
        let src = "@cache\ndef f(x):\n    return x + 1\n\nclass C:\n    pass\n";
        let got = outline(src, Language::Python);
        for expected in ["@cache", "def f(x):", "class C:"] {
            assert!(got.contains(expected), "missing {expected} in {got}");
        }
        assert!(!got.contains("return x + 1"), "{got}");
    }

    #[test]
    fn keeps_typescript_exported_declarations() {
        let src = "export interface A {\n  b: string;\n}\nexport function f() {\n  work();\n}\n";
        let got = outline(src, Language::TypeScript);
        assert!(got.contains("export interface A {"), "{got}");
        assert!(!got.contains("work();"), "{got}");
    }

    #[test]
    fn unknown_languages_pass_through_unchanged() {
        let src = "some prose\nmore prose\n";
        assert_eq!(outline(src, Language::Other), src);
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert_eq!(outline("", Language::Rust), "");
    }

    #[test]
    fn a_file_of_only_bodies_still_reports_the_elision() {
        assert_eq!(
            outline("    a();\n    b();\n", Language::Rust),
            "    // ...\n"
        );
    }

    #[test]
    fn never_grows_the_input() {
        let src = "fn f() {\n    a();\n    b();\n}\n";
        assert!(outline(src, Language::Rust).len() <= src.len());
    }
}
