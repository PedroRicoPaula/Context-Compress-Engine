#!/usr/bin/env bash
# Lint gate. Same one-line-per-finding contract as rust-check.sh.
# Denies warnings so CI and local agree. Run before every commit.
# Usage: ./.claude/skills/rust-clippy-filter.sh [extra cargo args]
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 2
here=".claude/skills"

out=$(cargo clippy --message-format=json --all-targets "$@" -- -D warnings 2>/dev/null)
status=$?

echo "$out" | jq -r -f "$here/_cargo-json.jq" 2>/dev/null | sort -u

if [ $status -eq 0 ]; then echo "OK clippy"; else echo "FAIL clippy (exit $status)"; fi
exit $status
