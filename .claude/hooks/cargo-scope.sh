#!/usr/bin/env bash
# AGENTS.md "Lint (before committing)" — cargo is scoped to the crates that changed.
#
# The rule is written down, and it still gets broken: the full-workspace command is the one that
# comes to mind, and the section that says not to use it is not open at the time. This stops the
# command instead of hoping the section is remembered.
#
# Blocks clippy, test, doc, build and check when they would run on the whole workspace: `--workspace`
# / `--all`, or no `-p` / `--package` / `--manifest-path` at the workspace root (the root manifest is
# a virtual one, so a bare `cargo test` there is every crate). `./scripts/lint-changed.sh` is the
# way in; it passes `-p` itself. The full-workspace run AGENTS.md keeps for "before opening a PR"
# is prefixed with `HECA_FULL_WORKSPACE=1`, and only when Antonio has asked for that PR.
set -euo pipefail

input=$(cat)
command=$(jq -r '.tool_input.command // empty' <<< "$input" 2>/dev/null || true)
cwd=$(jq -r '.cwd // empty' <<< "$input" 2>/dev/null || true)
root="${CLAUDE_PROJECT_DIR:-}"

[ -n "$command" ] || exit 0
grep -qE '(^|[^A-Za-z_])cargo([[:space:]]+\+[^[:space:]]+)?[[:space:]]+(clippy|test|doc|build|check|nextest)([[:space:]]|$)' \
  <<< "$command" || exit 0
grep -q 'HECA_FULL_WORKSPACE=1' <<< "$command" && exit 0

deny() {
  jq -n --arg why "$1" '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: ("AGENTS.md \"Lint (before committing)\": " + $why + "\n" +
        "Use ./scripts/lint-changed.sh (clippy), ./scripts/lint-changed.sh test, or cargo with -p <crate>.\n" +
        "The full workspace is only for before opening a PR, on Antonio'"'"'s word: prefix HECA_FULL_WORKSPACE=1.")
    }
  }'
  exit 0
}

# Each command of a `&&` / `;` / `|` chain on its own, so a `-p` on one does not excuse another.
while IFS= read -r part; do
  grep -qE '(^|[^A-Za-z_])cargo([[:space:]]+\+[^[:space:]]+)?[[:space:]]+(clippy|test|doc|build|check|nextest)([[:space:]]|$)' \
    <<< "$part" || continue
  if grep -qE '(^|[[:space:]])--(workspace|all)([[:space:]]|$)' <<< "$part"; then
    deny "\`--workspace\` runs every crate, not the ones that changed."
  fi
  grep -qE '(^|[[:space:]])(-p|--package|--manifest-path)([[:space:]=]|$)' <<< "$part" && continue
  # No crate named: it is the whole workspace when run from the root, and scoped from a crate's
  # own folder. A `cd` earlier in the chain is the folder it runs in.
  dir=$(grep -oE '(^|[;&|][[:space:]]*)cd[[:space:]]+[^;&|]+' <<< "${command%%"$part"*}" | tail -1 |
    sed -E 's/^[;&|[:space:]]*cd[[:space:]]+//; s/[[:space:]]+$//' || true)
  [ -n "$dir" ] || dir="$cwd"
  case "$dir" in
    /*) ;;
    *) dir="$cwd/$dir" ;;
  esac
  dir=$(cd "$dir" 2>/dev/null && pwd -P || echo "$dir")
  rootp=$(cd "$root" 2>/dev/null && pwd -P || echo "$root")
  if [ "$dir" = "$rootp" ]; then
    deny "no -p at the workspace root runs every crate."
  fi
done < <(sed -E 's/(&&|\|\||;|\|)/\n/g' <<< "$command")

exit 0
