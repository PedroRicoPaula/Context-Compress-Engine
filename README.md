# context-compressor-mcp

A local MCP server that turns source files into high-signal **context packs**.
100% local, no API keys, no telemetry. Built for an 8GB Apple Silicon Mac that
is already running Ollama — the compressor is the guest, not the host.

Give it a file, get back the same file with the noise removed: non-doc comments
gone, imports hoisted and deduped, whitespace collapsed, and — for large files —
bodies elided down to a signature outline.

**Status: V1, heuristics only.** No AI in the loop yet. Deterministic, fast,
and it costs nothing but a few milliseconds. See `docs/BACKLOG.md` for V2
(relevance ranking) and V3 (Ollama).

## Build

Needs a Rust toolchain (1.75+):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

cargo build --release
# binary: target/release/context-compressor-mcp
```

## Try it without a client

The server speaks line-delimited JSON-RPC 2.0 on stdio, so a pipe is enough:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"compress_file","arguments":{"filePath":"src/main.rs","taskDescription":"review the wiring"}}}' \
  | ./target/release/context-compressor-mcp
```

Third response contains the context pack, prefixed with a stats header:

```
// context pack | Rust | full | 8214 -> 4120 bytes (49.8% saved)
// task: review the wiring
```

## Connect it

Paths must be absolute. The server's **working directory is its security root** —
it will refuse to read anything outside it (`docs/SECURITY.md`).

**Claude Code** — `~/.claude.json`, or `.mcp.json` in a project:

```json
{
  "mcpServers": {
    "context-compressor": {
      "command": "/absolute/path/to/target/release/context-compressor-mcp",
      "cwd": "/absolute/path/to/the/project/you/want/to/compress"
    }
  }
}
```

**Cursor** — `~/.cursor/mcp.json`, same shape.

Restart the client, then ask it to compress a file. `stderr` carries a one-line
startup banner naming the root — check it if paths are being refused.

## The tools

**`compress_file`** — reduce a file to a context pack.

| Argument | Type | |
|---|---|---|
| `filePath` | string | required. Relative to the server's working directory. |
| `taskDescription` | string | optional. Accepted and echoed; **not yet used** for ranking. |

**`get_symbol`** — read one definition back, whole.

| Argument | Type | |
|---|---|---|
| `filePath` | string | required. |
| `symbol` | string | required. Exact name, as it appears in the outline. |

The two are counterparts. Above 24 KB, `compress_file` switches to outline
mode: signatures and doc summaries, bodies elided. `get_symbol` brings back the
one body you actually need, with its decorators, doc comment, and line range.

On scipy's `_stats_py.py` (441 KB): the pack is 29 KB, and pulling `ttest_ind`
back whole adds 13 KB — 9.5% of the file, with nothing lost that mattered.

Refuses: paths outside the root, non-regular files, anything over 8MB, non-UTF-8,
and deny-listed names (`.env`, `*.pem`, `.git/`, …).

## Development

Use the wrappers in `.claude/skills/` rather than raw cargo — they emit one line
per finding instead of screens of ASCII art:

```bash
./.claude/skills/rust-check.sh          # fast type check
./.claude/skills/rust-clippy-filter.sh  # lint gate
./.claude/skills/rust-test-filter.sh    # failures only
./.claude/skills/loc-guard.sh           # 300-line module cap
```

`docs/` holds the rest: `ARCHITECTURE.md` (layers and the decoupling rule),
`DECISIONS.md` (why each crate and pattern), `SECURITY.md`, `BACKLOG.md`,
`PROGRESS.md`. `docs/BRIEF.md` is the original project brief.

## License

MIT.
