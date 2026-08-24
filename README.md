# context-compressor-mcp

A local MCP server that turns source files into high-signal **context packs**
for coding agents. 100% local, no API keys, no network, no telemetry.

Give it a file, get back the same file with the noise removed. Give it a large
file, get back its shape — every function, class, and doc summary, bodies
elided — and pull any one body back when you actually need it.

```
// context pack | Python | outline | 441238 -> 28829 bytes (93.5% saved)
// bodies elided. To read one whole: get_symbol(filePath, symbol)

def ttest_ind(a, b, *, axis=0, equal_var=True, nan_policy='propagate',
              alternative="two-sided", trim=0, method=None):
    """Calculate the T-test for the means of *two independent* samples of scores."""
    # ...
```

That is a 441 KB module of scipy in 29 KB, with all 138 functions, all 9
classes, and all 88 docstring summaries intact.

## Why this and not just reading the file

Two reasons, both measured rather than assumed.

**Compression here is reversible.** Outline mode is not lossy in the usual
sense — `get_symbol` brings any elided body back whole, with its decorators and
doc comment. So the pack can afford to cut hard: pack plus one retrieved symbol
came to 9.5% of that scipy file.

**Context is re-billed every turn.** On one measured session, 97.3% of billed
tokens were cache reads: every unique token was re-read about 620 times. A file
read once at turn 5 keeps being paid for at turn 50. Cutting 19 KB out of it is
not a 19 KB saving. See [docs/WHERE-TOKENS-GO.md](docs/WHERE-TOKENS-GO.md),
which also records where this tool does *not* help.

## Install

Needs Rust 1.75+.

```bash
git clone https://github.com/PedroRicoPaula/Context-Compress-Engine
cd Context-Compress-Engine
cargo install --path .
```

Installs to `~/.cargo/bin/context-compressor-mcp`. Install rather than pointing
at `target/release/` — cleaning build output would break your config.

## Connect it

**Claude Code**, from inside the project you want to compress:

```bash
claude mcp add --scope project context-compressor -- ~/.cargo/bin/context-compressor-mcp
```

**Cursor**, or any global config — pin the root, because a global server
otherwise inherits whatever directory your editor launched from:

```json
{
  "mcpServers": {
    "context-compressor": {
      "command": "/Users/you/.cargo/bin/context-compressor-mcp",
      "env": { "CCE_ROOT": "/Users/you/code" }
    }
  }
}
```

## Try it without a client

Line-delimited JSON-RPC 2.0 on stdio, so a pipe is enough:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"compress_file","arguments":{"filePath":"src/main.rs"}}}' \
  | context-compressor-mcp
```

## The tools

**`compress_file`** — reduce a file to a context pack.

| Argument | Type | |
|---|---|---|
| `filePath` | string | required, relative to the security root |
| `taskDescription` | string | optional. Accepted and echoed; **not yet used** for ranking |
| `outlineThreshold` | integer | optional. Bytes above which bodies are elided. Default 8192 |

**`get_symbol`** — read one definition back, whole.

| Argument | Type | |
|---|---|---|
| `filePath` | string | required |
| `symbol` | string | required. Exact name, as it appears in the outline |

## What it does to a file

Ordered passes, each independently tested:

1. **Comments** — strips `//` and `#`, **keeps** `///`, `//!`, `/** */`, and
   Python docstrings. The scanner is quote-aware, so a `//` inside a URL string
   is not mistaken for a comment.
2. **Imports** — hoisted, deduplicated, order preserved. Re-emitted *below* any
   module doc comment, which is only legal at the top of a file.
3. **Whitespace** — blank-line runs collapsed, trailing space trimmed.
4. **Outline**, above the threshold — declarations and doc summaries kept,
   bodies replaced with an elision marker in the language's own comment syntax.
   Multi-line signatures are followed to their closing parenthesis: a truncated
   signature reads as complete and is worse than none.

Languages with real rules: Rust, Python, JavaScript, TypeScript, Go. Anything
else gets the whitespace pass only.

## Security

The agent calling this is not a trusted caller. Every path argument is
validated before a file is opened:

- Canonicalized first, so `..` and symlinks resolve before the check
- Refused if outside the security root (`CCE_ROOT`, or the working directory)
- Regular files only — no directories, FIFOs, or devices
- 8 MB size cap, checked from metadata before any read
- Deny list for `.env`, `*.pem`, `*.key`, `id_rsa`, `.git/`, and similar
- Errors return a category, never an OS message that would leak directory
  structure
- `#![forbid(unsafe_code)]`; no `.unwrap()` / `.expect()` / `panic!` outside tests

Full rules and rationale in [docs/SECURITY.md](docs/SECURITY.md).

## Measured, not claimed

| | |
|---|---|
| 441 KB scipy module | 93.5% compressed, structure fully intact |
| Memory to do it | **4.4 MB RSS, 30 ms** |
| Release binary | 427 KB |
| Tests | 169 |

Structural passes alone save ~12%, and that is flat across every file size —
nearly all the value is in outline mode. Which is why the threshold was swept
over 274 real files rather than guessed at. See
[bench/threshold.py](bench/threshold.py) and ADR-010.

## Status

**V1: heuristics only.** No AI in the loop, deliberately — deterministic,
testable, and effectively free to run. `tree-sitter` is not a dependency yet;
line heuristics ship now and the known failure cases are pinned as `#[ignore]`d
tests so the swap has a target. Reasoning in
[docs/DECISIONS.md](docs/DECISIONS.md) (ADR-004).

What is deliberately *not* built, and what would justify building it, is in
[docs/BACKLOG.md](docs/BACKLOG.md).

## Development

Use the wrappers in `.claude/skills/` rather than raw cargo — they emit one
line per finding instead of screens of ASCII art:

```bash
./.claude/skills/rust-check.sh          # fast type check
./.claude/skills/rust-clippy-filter.sh  # lint gate
./.claude/skills/rust-test-filter.sh    # failures only
./.claude/skills/loc-guard.sh           # 300-line module cap
```

`docs/` holds the rest: [ARCHITECTURE.md](docs/ARCHITECTURE.md) (layers and the
decoupling rule), [DECISIONS.md](docs/DECISIONS.md) (why each crate and
pattern), [ERRORS.md](docs/ERRORS.md) (every mistake made here and the rule it
taught), [SECURITY.md](docs/SECURITY.md), [BACKLOG.md](docs/BACKLOG.md),
[PROGRESS.md](docs/PROGRESS.md).

## License

MIT — see [LICENSE](LICENSE).
