# Skills

Wrappers whose only job is to shrink what lands in the agent's context.

Cargo's human output is designed for a terminal: colour, ASCII underlines,
repeated code frames, progress spam. A single failing build can cost ~4k tokens
and repeats the same three facts. These scripts parse
`--message-format=json` and emit **one line per finding**:

```
src/compress/imports.rs:42:9 ERROR E0308 mismatched types: expected `&str`, found `String`
```

## Contract

Every script: run from anywhere, `cd`s to repo root itself; exits non-zero on
failure; last line is `OK <name>` or `FAIL <name>`; extra args pass through to cargo.

| Script | Wraps |
|--------|-------|
| `rust-check.sh` | `cargo check --all-targets` |
| `rust-clippy-filter.sh` | `cargo clippy --all-targets -- -D warnings` |
| `rust-test-filter.sh` | `cargo test`, failures only |
| `rust-fmt.sh` | `cargo fmt` (`--check` to report only) |
| `loc-guard.sh` | 300-line module cap, no cargo |

`_cargo-json.jq` is the shared diagnostic filter — not executable, not a skill.

## Loop

`rust-check.sh` while iterating (fastest, no codegen) → `rust-fmt.sh` →
`rust-clippy-filter.sh` + `rust-test-filter.sh` → `loc-guard.sh` before commit.

Requires `jq` (preinstalled on macOS via Xcode CLT, else `brew install jq`).
