//! The MCP tools, one module each.
//!
//! Part of the wiring layer with `main.rs` — the only place the MCP side and
//! the compression side are allowed to meet (`docs/ARCHITECTURE.md`).

pub mod compress;
pub mod symbol;

use std::path::Path;

use serde_json::Value;

use crate::mcp::protocol::{codes, ToolSpec};

pub use compress::NAME as TOOL_COMPRESS_FILE;
pub use symbol::NAME as TOOL_GET_SYMBOL;

#[must_use]
pub fn tool_specs() -> Vec<ToolSpec> {
    vec![compress::spec(), symbol::spec()]
}

/// Execute a tool call.
///
/// Tool-level failures come back as `Ok(error text)` so the model sees the
/// reason and can retry; only a malformed *call* is a JSON-RPC error.
pub fn call_tool(name: &str, arguments: &Value, root: &Path) -> Result<Value, (i32, String)> {
    match name {
        TOOL_COMPRESS_FILE => compress::run(arguments, root),
        TOOL_GET_SYMBOL => symbol::run(arguments, root),
        other => Err((codes::INVALID_PARAMS, format!("unknown tool: {other}"))),
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
    use serde_json::json;

    #[test]
    fn both_tools_are_advertised_in_order() {
        let names: Vec<&str> = tool_specs().iter().map(|t| t.name).collect();
        assert_eq!(names, vec![TOOL_COMPRESS_FILE, TOOL_GET_SYMBOL]);
    }

    #[test]
    fn every_advertised_tool_is_dispatchable() {
        // A spec with no arm behind it advertises a tool that always fails.
        let root = std::env::current_dir().expect("cwd");
        for spec in tool_specs() {
            let error = call_tool(spec.name, &json!({}), &root);
            assert!(
                !matches!(&error, Err((_, m)) if m.starts_with("unknown tool")),
                "{} is advertised but not dispatched",
                spec.name
            );
        }
    }

    #[test]
    fn an_unknown_tool_name_is_an_invalid_params_error() {
        let root = std::env::current_dir().expect("cwd");
        let (code, _) = call_tool("nope", &json!({}), &root).expect_err("unknown tool");
        assert_eq!(code, codes::INVALID_PARAMS);
    }
}
