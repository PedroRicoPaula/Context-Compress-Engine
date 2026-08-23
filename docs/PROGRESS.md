# Progress

## Current sprint: MVP scaffold

| # | Step | Status |
|---|------|--------|
| 1 | Read README, fix scope | done |
| 2 | Git init + `docs/` + `CLAUDE.md` | done |
| 3 | `.claude/skills/` token-saving scripts | done |
| 4 | `cargo init` + `Cargo.toml` | done (hand-written, see blocker) |
| 5 | Module structure + MCP skeleton | done |

## Blocker

No Rust toolchain on this machine — `cargo`, `rustc`, `rustup` all absent.
Everything through Step 5 is written but **not compiled**. First action next
session:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
./.claude/skills/rust-check.sh
```

Expect first-compile errors; they have not been ironed out against a real
compiler. Log the ones that teach something in `ERRORS.md`.

## Milestones

- [x] Docs + agentic tooling scaffold
- [x] MCP skeleton: `initialize`, `tools/list`, `tools/call`
- [x] V1 heuristic compressor with inline unit tests
- [ ] **First successful `cargo build`** ← next
- [ ] Verified end-to-end from a real MCP client
- [ ] V2 relevance ranking (see BACKLOG.md)
