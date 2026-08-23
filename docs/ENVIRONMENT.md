# Environment

Facts about the machine this project is developed on. They are not trivia —
they are the reason for several decisions in `DECISIONS.md`. Re-verify before
trusting; hardware facts rot.

*Last verified: 2026-08-23.*

## Machine

| | |
|---|---|
| Platform | macOS (Darwin 25.6.0), Apple Silicon |
| RAM | **8 GB**, shared with a local Ollama model |
| Disk | 228 GB total, **31 GB free (85% full)** |
| Shell | zsh |

The RAM figure is the hard constraint. This binary is the *guest* on this
machine — Ollama is the host. Every dependency, every buffer, every `String`
clone is weighed against that. See `ARCHITECTURE.md`.

The disk figure matters more than it looks: Rust is not a light install and
its caches grow silently. See "Disk hygiene" below.

## Toolchain status

**No Rust toolchain installed** as of the date above — `cargo`, `rustc`,
`rustup` all absent, `~/.cargo` and `~/.rustup` do not exist, no shell rc file
mentions them, and Homebrew has no rust formula. Nothing in `src/` has ever
been compiled or run.

Verified clean, so a rustup install cannot collide with anything:
`cargo`, `rustc`, `rustup`, `rustfmt`, `clippy` are all unused command names.

To install:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

To reverse it completely (removes both directories and the shell line):

```bash
rustup self uninstall
```

## Why Rust does not pollute other projects

Worth stating because it is the opposite of the Node/Python habit:

- **There is no global package install** in Rust. No `npm install -g`, no
  `pip install`. Dependencies are declared per project in `Cargo.toml` and
  compiled into that project's own `target/`.
- **Compiler versions are per project too** (`rust-toolchain.toml`), so two
  projects can pin different Rust versions without interfering.
- Installing rustup adds command names to `PATH` and nothing else. It starts
  no background service, opens no port, and does not touch Python, Node, or
  Homebrew.

## Disk hygiene

Expected footprint: rustup + toolchain ~1.5–2 GB, this project's `target/`
~300–500 MB after a release build, plus `~/.cargo/registry` growing over time
with the source of every dependency ever downloaded, across all projects.

Both of these are caches and regenerate on the next build:

```bash
rm -rf target                    # this project's build output
rm -rf ~/.cargo/registry/cache   # downloaded crate archives
```

At 85% full, check `df -h ~` before a long build session.

## Consequences for how we work

- Close Ollama before the first full compile. Compiling is memory-hungry and
  8 GB does not stretch to both.
- Prefer `cargo check` over `cargo build` while iterating: it type-checks
  without generating a binary, so it is far faster and writes far less to disk.
- `opt-level = "z"` and `lto` in the release profile (see `Cargo.toml`) trade
  compile time for a smaller binary. That is the right trade here.
