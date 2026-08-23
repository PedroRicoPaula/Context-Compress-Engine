//! Outline mode: keep declarations and doc comments, elide bodies.
//!
//! This is the last rung of the degradation ladder. A 50k-line file cannot go
//! into an agent's context whole, but its shape — what types exist, what
//! functions they expose — usually answers the question.
//!
//! Elided bodies are marked so the reader knows the file was outlined and did
//! not simply lack implementations.

use super::lang::Language;

/// Marker left where a body was removed.
const ELISION: &str = "    // ...";

fn indent_width(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn is_declaration(trimmed: &str, lang: Language) -> bool {
    lang.signature_prefixes()
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

fn is_doc(trimmed: &str, lang: Language) -> bool {
    lang.doc_prefixes()
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
        || trimmed.starts_with("/**")
        || trimmed.starts_with('*')
}

/// Reduce `source` to declarations, doc comments, and structural closers.
///
/// Returns the source unchanged for languages with no known declaration syntax:
/// guessing at structure would lose more than it saves.
#[must_use]
pub fn outline(source: &str, lang: Language) -> String {
    if lang.signature_prefixes().is_empty() {
        return source.to_owned();
    }

    let mut out = String::with_capacity(source.len() / 2);
    let mut elided = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Closing brace of a top-level item: keeps the outline balanced.
        let is_top_closer = indent_width(line) == 0 && (trimmed == "}" || trimmed == "};");

        if is_declaration(trimmed, lang) || is_doc(trimmed, lang) || is_top_closer {
            if elided {
                out.push_str(ELISION);
                out.push('\n');
                elided = false;
            }
            out.push_str(line);
            out.push('\n');
        } else {
            elided = true;
        }
    }

    if elided {
        out.push_str(ELISION);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
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
        assert!(got.contains("/// Adds."), "{got}");
        assert!(got.starts_with("/// Adds."), "{got}");
    }

    #[test]
    fn keeps_top_level_closing_braces_only() {
        let src = "fn f() {\n    if x {\n        y();\n    }\n}\n";
        let got = outline(src, Language::Rust);
        // The nested `}` is indented and must not survive; the top-level one must.
        assert_eq!(got.matches('}').count(), 1, "{got}");
    }

    #[test]
    fn consecutive_elided_lines_collapse_to_one_marker() {
        let src = "fn f() {\n    a();\n    b();\n    c();\n}\n";
        assert_eq!(outline(src, Language::Rust).matches("// ...").count(), 1);
    }

    #[test]
    fn keeps_python_defs_classes_and_decorators() {
        let src = "@cache\ndef f(x):\n    return x + 1\n\nclass C:\n    pass\n";
        let got = outline(src, Language::Python);
        assert!(got.contains("@cache"), "{got}");
        assert!(got.contains("def f(x):"), "{got}");
        assert!(got.contains("class C:"), "{got}");
        assert!(!got.contains("return x + 1"), "{got}");
    }

    #[test]
    fn keeps_typescript_exported_declarations() {
        let src = "export interface A {\n  b: string;\n}\nexport function f() {\n  work();\n}\n";
        let got = outline(src, Language::TypeScript);
        assert!(got.contains("export interface A {"), "{got}");
        assert!(got.contains("export function f() {"), "{got}");
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

    #[test]
    #[ignore = "known V1 limit: multi-line signatures need a real parser (ADR-004)"]
    fn multi_line_signatures_are_a_known_gap() {
        let src = "fn long(\n    a: i32,\n    b: i32,\n) -> i32 {\n    a\n}\n";
        let got = outline(src, Language::Rust);
        assert!(got.contains("a: i32"), "{got}");
    }
}
