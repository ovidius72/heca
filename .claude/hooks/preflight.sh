#!/usr/bin/env bash
# AGENTS.md § 0 — the pre-flight, put in front of every Rust edit.
#
# The rules are in AGENTS.md and in memory, and they still get skipped: a file is open, the
# obvious move is to extend what is already in it, and the question that would have stopped that
# is six thousand lines away. This is the harness asking it instead.
set -euo pipefail

file=$(jq -r '.tool_input.file_path // empty' 2>/dev/null || true)
case "$file" in
  *.rs) ;;
  *) exit 0 ;;
esac

jq -n --arg f "$file" '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    additionalContext: ("AGENTS.md § 0 PRE-FLIGHT — answer before editing \($f):\n" +
      "1. WHICH EXISTING WIDGET DOES THIS? Name it. Search all three: docs/widgets.md, heca/src/components/, heca/src/chrome/<surface>/. Do not go by memory, including your own from earlier in this session.\n" +
      "2. If none — WHICH PLANNER TASK COVERS IT? Quote the id.\n" +
      "3. If neither — it is a PROPOSAL, not a commit. Say what and why, get the OK.\n" +
      "\n" +
      "And the shape: anything with MORE THAN ONE PART is a Grid with a track template (auto / 1fr) — never a hand-stacked Flex with tuned grow weights, and never a parent that counts its children to work out which one is which.\n" +
      "\n" +
      "The fix is centralized, never at the call site. The test is: will any other developer or agent ever have to know this problem exists?")
  }
}'
