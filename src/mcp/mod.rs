//! MCP transport and protocol layer.
//!
//! Boundary rule (`docs/ARCHITECTURE.md`): nothing under `mcp::` may reference
//! `crate::compress::`. The two meet only in `main.rs`, exchanging plain data.

pub mod dispatch;
pub mod protocol;
pub mod transport;
