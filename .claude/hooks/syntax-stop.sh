#!/usr/bin/env bash
# THE SYNTAX, before a turn ends.
#
# The check (scripts/syntax-check.py) runs in the lint and in CI, but by then the work has already
# been reported as done. This runs it when an agent is about to answer: if the lines its branch
# adds break THE SYNTAX, the turn does not end — the agent gets the list and fixes it first, so no
# person has to point at the same break again.
#
# It blocks once per turn: when the agent has already been sent back (`stop_hook_active`), it lets
# the turn end, so a line the agent cannot fix alone is reported instead of looping.
set -euo pipefail

input=$(cat)
if [ "$(jq -r '.stop_hook_active // false' <<< "$input")" = "true" ]; then
  exit 0
fi

cd "$CLAUDE_PROJECT_DIR"
if found=$(python3 scripts/syntax-check.py --base main 2>&1); then
  exit 0
fi

jq -n --arg found "$found" '{
  decision: "block",
  reason: ("THE SYNTAX (AGENTS.md § 0) is broken by lines this branch adds. Fix them before reporting the work as done — a missing builder is built in the library, never worked around:\n\n" + $found)
}'
