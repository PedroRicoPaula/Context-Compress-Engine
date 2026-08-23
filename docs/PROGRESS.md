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
OK tests        103 passed, 0 failed, 2 ignored (documented V1 limits)
release         427 KB binary, ~10 s from clean
```

End-to-end over stdio against the release binary: `initialize`, `tools/list`,
and `tools/call` all answer correctly; all five refusal paths (outside root,
missing, deny-listed, non-regular file, traversal) return the right category.

Seven defects were found and fixed in the process — see `ERRORS.md`. One of
them, invalid output from import hoisting, was found only by running the real
binary on a real file.

## Milestones

- [x] Docs + agentic tooling scaffold
- [x] MCP skeleton: `initialize`, `tools/list`, `tools/call`
- [x] V1 heuristic compressor with inline unit tests
- [x] First successful `cargo build`
- [x] Verified end-to-end over stdio
- [ ] **Verified from a real MCP client (Claude Code / Cursor)** ← next
- [ ] Tune the outline threshold against a foreign codebase (see BACKLOG.md)
- [ ] V2 relevance ranking
