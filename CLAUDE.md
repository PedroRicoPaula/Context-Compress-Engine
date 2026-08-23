# CLAUDE.md — Index Only

**This file is an index. It holds no specs.** Read only the doc you need for
the task at hand. Do not load all of `docs/` into context by default.

## Docs Map

| File | Read it when |
|------|--------------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Touching module boundaries, data flow, or adding a layer |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Choosing a crate/pattern, or asking "why is it like this?" |
| [docs/BACKLOG.md](docs/BACKLOG.md) | Picking up new work, or logging tech debt |
| [docs/PROGRESS.md](docs/PROGRESS.md) | Starting a session — what is done, what is in flight |
| [docs/SECURITY.md](docs/SECURITY.md) | Touching file I/O, paths, subprocess, or network |
| [docs/ERRORS.md](docs/ERRORS.md) | **Read before the first compile of a session** — mistakes already made, and the Rust rule each one taught |
| [docs/ENVIRONMENT.md](docs/ENVIRONMENT.md) | Toolchain state, 8GB RAM / 85%-full disk limits, install and cleanup commands |

## Skills (`.claude/skills/`)

Run these instead of raw cargo. They strip compiler ASCII art and emit only
actionable lines — a full `cargo build` failure is ~4k tokens, `rust-check.sh`
is ~200.

| Script | Use for | Output |
|--------|---------|--------|
| `./.claude/skills/rust-check.sh` | Type errors, fast feedback loop | `file:line:col E0308 mismatched types` |
| `./.claude/skills/rust-clippy-filter.sh` | Lint gate before commit | one line per warning/error |
| `./.claude/skills/rust-test-filter.sh` | Test run | failed test names + assert message only |
| `./.claude/skills/rust-fmt.sh` | Format | silent on success |
| `./.claude/skills/loc-guard.sh` | Find modules over the 300-line cap | `path LINES` |

All exit non-zero on failure. All accept extra cargo args passthrough.

## Core Commands

```bash
cargo build --release          # binary -> target/release/context-compressor-mcp
./.claude/skills/rust-check.sh # fast type check (use this while iterating)
cargo run                      # MCP server on stdio; speaks JSON-RPC 2.0
```

Manual smoke test of the server (no client needed):

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | cargo run -q
```

## Hard Rules

1. No `.unwrap()` / `.expect()` / `panic!` in `src/` outside `#[cfg(test)]`.
2. Module over 300 lines → split into a sub-module directory. Check with `loc-guard.sh`.
3. MCP protocol code (`src/mcp/`) must never `use crate::compress` types directly —
   they meet only in `src/main.rs`. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
4. Every heuristic gets an inline `#[cfg(test)] mod tests` with edge cases.
5. Commits: Conventional Commits, small and atomic. One logical change each.
6. Never log file *contents* to stderr — see [docs/SECURITY.md](docs/SECURITY.md).
7. Fixed a real mistake? Add it to [docs/ERRORS.md](docs/ERRORS.md) — but only if
   it taught a language rule or happened twice. Everything else is churn.
