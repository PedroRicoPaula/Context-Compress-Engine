# Project Bootstrapping: Local Open-Source Context Compressor (Rust Edition)

You are an expert systems programmer and architect specializing in Rust. My goal is to build my OWN open-source, 100% local, and completely free Context Compression and Intelligence tool. 

I do not want to use paid APIs. The tool must be extremely lightweight and memory-efficient, as it will run on an Apple Silicon Mac with strictly 8GB of RAM and 256GB of storage. It must leave as much memory as possible available for local Ollama models.

## 1. The Core Vision
We are building a tool (tentatively named "LocalContext-Compressor") that achieves "lossless" semantic context compression. 
- Input: Massive project files, SPECs, and logs.
- Process: Local heuristics, AST parsing, and local LLM evaluation (via Ollama) to identify what is strictly necessary for a given task.
- Output: A highly condensed, high-signal "Context Pack" that I can feed to coding agents (like Claude Code in Cursor) to save tokens and improve accuracy.

## 2. Architecture & Tech Stack
We will build this as a local MCP (Model Context Protocol) Server.
- Language: Rust
- Interface: MCP Server via stdio (Standard Input/Output) using JSON-RPC.
- Parsers: `tree-sitter` (Rust bindings) for code structure parsing.
- Serialization: `serde` and `serde_json`.
- Local AI: HTTP client (e.g., `reqwest`) to communicate with the local Ollama API (e.g., `qwen2.5-coder` or `llama3`) for semantic tasks.

## 3. Your Immediate Tasks

DO NOT write the entire system at once. We will build this iteratively. For this first prompt, execute the following specific steps:

### STEP 1: Scaffold the Project
Initialize a new Rust project for the MCP server.
- Run `cargo new context-compressor-mcp`.
- Update `Cargo.toml` with the necessary dependencies: `tokio`, `serde`, `serde_json`, `reqwest`, and any lightweight MCP crate if a stable one exists, otherwise prepare for custom stdio JSON-RPC handling.
- Set up the basic module structure (`src/main.rs`, `src/mcp.rs`, `src/parsers.rs`).

### STEP 2: Create the MCP Server Skeleton
Write the entry point (`src/main.rs` and `src/mcp.rs`) that initializes an async event loop listening to stdin/stdout for MCP protocol messages.
- Expose ONE initial tool called `compress_file`.
- Input arguments: `filePath` (String), `taskDescription` (String).
- Output: A compressed string representation of the file.

### STEP 3: Implement the V1 "Lossless" Heuristic Compressor
Inside `src/parsers.rs`, create a baseline compression module that DOES NOT use AI yet, but uses smart string/AST heuristics to reduce token count drastically without losing logic.
Implement functions to:
- Strip out excessive whitespace and consecutive blank lines.
- Remove inline comments (but keep docstrings as they contain semantic value).
- Extract imports and group them compactly.
- Extract function signatures and struct/trait definitions.

### STEP 4: Provide Local Testing Instructions
Write a brief `README.md` explaining how I can compile this server (`cargo build --release`), configure my `claude.json` or Cursor to connect to the generated binary via stdio, and test the `compress_file` tool immediately.

Let's get to work. Scaffold the project and show me the code for the MVP MCP Server.