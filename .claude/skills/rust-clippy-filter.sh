#!/usr/bin/env bash
# Lint gate. Same one-line-per-finding contract as rust-check.sh.
# Denies warnings so CI and local agree. Run before every commit.
# Usage: ./.claude/skills/rust-clippy-filter.sh [extra cargo args]
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 2

# The toolchain lives in ~/.cargo/bin, which non-interactive shells do not pick
# up from ~/.zshenv. Put it on PATH ourselves rather than depending on the caller.
[ -d "$HOME/.cargo/bin" ] && PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "FAIL: cargo not found. Install it: see docs/ENVIRONMENT.md"
  exit 127
fi
here=".claude/skills"

out=$(cargo clippy --message-format=json --all-targets "$@" -- -D warnings 2>/dev/null)
status=$?

echo "$out" | jq -r -f "$here/_cargo-json.jq" 2>/dev/null | sort -u

if [ $status -eq 0 ]; then echo "OK clippy"; else echo "FAIL clippy (exit $status)"; fi
exit $status
