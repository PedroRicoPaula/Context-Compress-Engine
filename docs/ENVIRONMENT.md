# Environment

The target environment, and why it shapes the code. These are constraints, not
trivia — they are the reason for several decisions in `DECISIONS.md`.

## Target machine

An 8 GB Apple Silicon Mac, already running a local model via Ollama.

The RAM figure is the hard constraint. This binary is the **guest** on that
machine — the model is the host. Every dependency, every buffer, every `String`
clone is weighed against it. See `ARCHITECTURE.md`.

It holds up: **4.4 MB RSS and 30 ms** to compress a 441 KB file, against an
architecture goal of under 50 MB. Measured with `/usr/bin/time -l`, not
estimated.

Nothing here is macOS-specific except the shell snippets. The crate is portable
Rust with no platform dependencies.

## Toolchain status

Built and tested against **Rust 1.98.0** (stable). `rust-version` in
`Cargo.toml` sets the floor at 1.75.

Footprint of the toolchain itself: `~/.rustup` about 1.3 GB, `~/.cargo` about
11 MB. Not a light install — worth knowing before starting on a full disk.

Non-interactive shells do not read `~/.zshenv`, so the scripts in
`.claude/skills/` put `~/.cargo/bin` on `PATH` themselves — see `ERRORS.md` for
the bug that taught us.

Release build: **427 KB binary, ~10 s from clean.**

To install the toolchain:

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

Both are caches and regenerate on the next build:

```bash
rm -rf target                    # this project's build output
rm -rf ~/.cargo/registry/cache   # downloaded crate archives
```

Because of that, install the binary with `cargo install --path .` rather than
pointing your MCP config at `target/release/` — otherwise cleaning up disk
breaks the config.

## Consequences for how we work

- Close Ollama before the first full compile. Compiling is memory-hungry and
  8 GB does not stretch to both.
- Prefer `cargo check` over `cargo build` while iterating: it type-checks
  without generating a binary, so it is far faster and writes far less to disk.
- `opt-level = "z"` and `lto` in the release profile (see `Cargo.toml`) trade
  compile time for a smaller binary. That is the right trade here.
