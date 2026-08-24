# Architecture

## Constraint that drives every decision

8GB RAM Mac, shared with a local Ollama model. The compressor is the *guest*,
not the host. Target: < 50MB RSS idle, stream instead of buffering whole files,
never hold two copies of a source file in memory.

## Layers

```
stdin (JSON-RPC 2.0, line-delimited)
   |
   v
+-------------------+   knows: protocol. knows NOTHING about compression.
| mcp/              |   transport.rs  - read lines, write lines, flush
|                   |   protocol.rs   - Request/Response/Error types (serde)
|                   |   dispatch.rs   - method name -> handler fn
+-------------------+
   |  ToolCall { name, arguments: serde_json::Value }
   v
+-------------------+   The ONLY place both sides are named.
| main.rs + tool.rs |   main.rs  - event loop, dispatch, notification rules
|     (wiring)      |   tool.rs  - compress_file: schema, execution, rendering
+-------------------+   Split when main.rs hit the 300-line cap; the boundary
                        rule is unchanged, the wiring layer is now two files.
   |  &str + CompressOptions
   v
+-------------------+   knows: text and code. knows NOTHING about JSON-RPC.
| compress/         |   whitespace.rs - blank line + trailing ws collapse
|                   |   comments.rs   - strip inline, KEEP doc comments
|                   |   imports.rs    - hoist + group import lines
|                   |   signatures.rs  - the outline state machine
|                   |   declaration.rs - what declares, and how far it reaches
|                   |   docstring.rs   - Python docstring summary extraction
|                   |   extract.rs     - pull one named symbol back out, whole
|                   |   block.rs       - where a block ends: braces or indent
|                   |   lang.rs        - extension -> Language enum
+-------------------+
   |
   v
CompressionResult { text, stats }
```

## The decoupling rule

`src/mcp/` must not `use crate::compress::*`, and `src/compress/` must not
`use crate::mcp::*`. They exchange nothing but `&str` and plain data via
`main.rs`. Consequence: the compressor is usable as a plain library, testable
without a protocol, and the protocol layer is swappable (HTTP, CLI) without
touching a single heuristic.

Enforced by review, not by the compiler (single crate). If it is ever violated,
split into a workspace with two crates — that is the escape hatch, not the
starting point.

## Two tools, one boundary

`compress_file` reduces; `get_symbol` restores one piece. They are counterparts,
not alternatives: outline mode can afford to cut hard *because* the cut is
reversible. Both enter through the same `guard.rs` validation — a second entry
point is a second chance to skip the trust boundary.

## Pipeline

`compress_file(filePath, taskDescription)`:

1. **Guard** — resolve path, reject traversal/symlink escape, size cap. `SECURITY.md`.
2. **Detect** — extension → `Language`. Unknown → generic text path.
3. **Reduce** — ordered passes, each `&str -> String`, each independently testable:
   comments → imports → whitespace → (outline, only above the threshold).
   The hoisted imports are re-emitted *below* the file preamble (`//!`, `#![`),
   which is only legal at the top of a file.
4. **Report** — return text + `{ original_bytes, compressed_bytes, ratio }`.

`taskDescription` is accepted and stored but **not yet used**. It is the hook
for V2 (relevance ranking) and V3 (Ollama semantic scoring). Wiring it now
would be a guess about a scoring function that does not exist.

## Deliberately absent

- No AI. V1 is pure heuristics — deterministic, testable, zero RAM cost.
- No tree-sitter yet. V1 uses line heuristics; tree-sitter grammars are ~2-5MB
  each of binary bloat and a C toolchain dependency. Earn it with a failing
  case first. See ADR-004 in `DECISIONS.md`.
- No caching layer, no config file, no plugin system.
