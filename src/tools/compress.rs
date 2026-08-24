//! `compress_file`: reduce a file to a context pack.

use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::compress::{self, OUTLINE_THRESHOLD_BYTES};
use crate::mcp::dispatch;
use crate::mcp::protocol::{codes, ToolSpec};
use crate::usage;

pub const NAME: &str = "compress_file";

/// Arguments for `compress_file`, exactly as the JSON schema below declares them.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompressFileArgs {
    file_path: String,
    /// Accepted and echoed, not yet used for ranking. It is the hook for V2
    /// relevance scoring — see `docs/BACKLOG.md`.
    #[serde(default)]
    task_description: String,
    /// Bytes above which outline mode kicks in. Optional; defaults to
    /// `OUTLINE_THRESHOLD_BYTES`. Exposed so the threshold can be measured
    /// against real code instead of guessed at (docs/BACKLOG.md).
    #[serde(default)]
    outline_threshold: Option<usize>,
}

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: NAME,
        description: "Compress a source file into a high-signal context pack. Strips \
                      non-doc comments, hoists and dedupes imports, collapses whitespace, \
                      and falls back to a signature-only outline for large files.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "filePath": {
                    "type": "string",
                    "description": "Path to the file, relative to the server's working directory."
                },
                "taskDescription": {
                    "type": "string",
                    "description": "What the context is for. Reserved for relevance ranking; ignored in V1."
                }
            },
            "required": ["filePath"],
            "additionalProperties": false
        }),
    }
}

/// Compress one file into a context pack.
pub fn run(arguments: &Value, root: &Path) -> Result<Value, (i32, String)> {
    let args: CompressFileArgs = serde_json::from_value(arguments.clone()).map_err(|_| {
        (
            codes::INVALID_PARAMS,
            "expected { filePath: string }".to_owned(),
        )
    })?;

    let threshold = args.outline_threshold.unwrap_or(OUTLINE_THRESHOLD_BYTES);
    match compress::compress_file(&args.file_path, root, threshold) {
        Ok(report) => {
            let text = render_report(&report, &args.task_description);
            usage::record(&usage::Record {
                tool: NAME,
                file: &args.file_path,
                input_bytes: Some(report.original_bytes),
                output_bytes: text.len(),
                outcome: if report.outlined { "outline" } else { "full" },
            });
            Ok(dispatch::tool_result(&text, false))
        }
        // Guard errors are categories, never OS messages (docs/SECURITY.md).
        Err(error) => {
            let text = format!("compress_file failed: {error}");
            usage::record(&usage::Record {
                tool: NAME,
                file: &args.file_path,
                input_bytes: None,
                output_bytes: text.len(),
                outcome: "refused",
            });
            Ok(dispatch::tool_result(&text, true))
        }
    }
}

/// Render a compression report as the text the model will read.
fn render_report(report: &compress::Report, task: &str) -> String {
    let mode = if report.outlined { "outline" } else { "full" };
    let mut header = format!(
        "// context pack | {lang:?} | {mode} | {orig} -> {new} bytes ({saved}% saved)\n",
        lang = report.language,
        orig = report.original_bytes,
        new = report.compressed_bytes,
        saved = report.saved_percent(),
    );
    let task = task.trim();
    if !task.is_empty() {
        header.push_str("// task: ");
        header.push_str(task);
        header.push('\n');
    }
    // Without this line the elisions look like loss. They are deferral, but
    // only if the reader knows the way back.
    if report.outlined {
        header.push_str("// bodies elided. To read one whole: get_symbol(filePath, symbol)\n");
    }
    header.push('\n');
    header.push_str(&report.text);
    header
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
    use crate::compress::Language;
    use crate::tools::call_tool;

    #[test]
    fn an_explicit_threshold_overrides_the_default() {
        let root = std::env::current_dir().expect("cwd");
        // A tiny threshold must force outline mode on a file that would
        // otherwise pass through whole.
        let outlined = call_tool(
            NAME,
            &json!({ "filePath": "src/mcp/mod.rs", "outlineThreshold": 1 }),
            &root,
        )
        .expect("call succeeds");
        let text = outlined["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.contains("| outline |"), "{text}");

        let whole = call_tool(
            NAME,
            &json!({ "filePath": "src/compress/whitespace.rs" }),
            &root,
        )
        .expect("call succeeds");
        let text = whole["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.contains("| full |"), "{text}");
    }

    #[test]
    fn compressing_a_real_file_returns_a_stats_header() {
        // src/main.rs is guaranteed to exist relative to the crate root.
        let root = std::env::current_dir().expect("cwd");
        let result = call_tool(
            NAME,
            &json!({ "filePath": "src/main.rs", "taskDescription": "review the wiring" }),
            &root,
        )
        .expect("tool call succeeds");
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.starts_with("// context pack | Rust |"), "{text}");
        assert!(text.contains("// task: review the wiring"), "{text}");
        assert_eq!(result["isError"], false);
    }

    #[test]
    fn the_task_line_is_omitted_when_no_task_is_given() {
        let report = compress::compress_str("fn f() {}\n", Language::Rust, OUTLINE_THRESHOLD_BYTES);
        assert!(!render_report(&report, "   ").contains("// task:"));
    }

    #[test]
    fn an_outlined_pack_tells_the_reader_how_to_expand_it() {
        // An elision the reader cannot undo is loss. This line is the way back.
        let report = compress::compress_str("fn f() {\n    body();\n}\n", Language::Rust, 1);
        assert!(report.outlined);
        assert!(
            render_report(&report, "").contains("get_symbol"),
            "{}",
            render_report(&report, "")
        );
    }
}
