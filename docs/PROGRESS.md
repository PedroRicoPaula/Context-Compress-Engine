# Progress

## Current sprint: MVP scaffold

| # | Step | Status |
|---|------|--------|
| 1 | Read README, fix scope | done |
| 2 | Git init + `docs/` + `CLAUDE.md` | done |
| 3 | `.claude/skills/` token-saving scripts | done |
| 4 | `cargo init` + `Cargo.toml` | done (hand-written, see blocker) |
| 5 | Module structure + MCP skeleton | done |

## Verified 2026-08-23

Toolchain installed, and everything below was **run**, not just written:

```
OK check        rustc 1.98.0, zero warnings
OK clippy       pedantic, unwrap/expect/panic/indexing denied
OK fmt
OK loc          all modules <= 300 lines
OK tests        155 passed, 0 failed, 1 ignored (documented V1 limit)
release         427 KB binary, ~10 s from clean
```

End-to-end over stdio against the release binary: `initialize`, `tools/list`,
and `tools/call` all answer correctly; all five refusal paths (outside root,
missing, deny-listed, non-regular file, traversal) return the right category.

Eleven defects found and fixed — see `ERRORS.md`. Four of them came from
compressing a real 441 KB scipy module rather than a fixture: truncated
signatures, dropped docstrings, orphan bullet lines, and a `//` elision marker
that is a syntax error in Python. None were caught by any unit test.

Result on that file: 93.5% compressed, all 138 functions and 9 classes kept,
all 88 docstrings kept as PEP 257 summaries, zero truncated signatures.

## Milestones

- [x] Docs + agentic tooling scaffold
- [x] MCP skeleton: `initialize`, `tools/list`, `tools/call`
- [x] V1 heuristic compressor with inline unit tests
- [x] First successful `cargo build`
- [x] Verified end-to-end over stdio
- [x] `get_symbol` retrieval — compression is no longer one-way (ADR-009)
- [ ] **Verified from a real MCP client (Claude Code / Cursor)** ← next, and
      everything else is gated on what it tells us
- [ ] Tune the outline threshold against a foreign codebase (see BACKLOG.md)

Measured cost, for reference when weighing anything heavier: **4.4 MB RSS,
30 ms** to compress a 441 KB file. The architecture goal was < 50 MB.
- [ ] V2 relevance ranking
