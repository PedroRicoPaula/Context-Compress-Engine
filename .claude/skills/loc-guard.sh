#!/usr/bin/env bash
# CLAUDE.md rule 2: no module over 300 lines. Small files = small context
# when fixing one bug. Prints only offenders.
# Usage: ./.claude/skills/loc-guard.sh [limit]
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 2
limit="${1:-300}"

[ -d src ] || { echo "OK loc (no src/ yet)"; exit 0; }

over=$(find src -name '*.rs' -type f -exec wc -l {} + \
  | awk -v L="$limit" '$2 != "total" && $1 > L { print $2, $1 }')

if [ -z "$over" ]; then echo "OK loc (<= $limit)"; exit 0; fi
echo "$over"
echo "FAIL loc: split these into sub-modules"
exit 1
