#!/usr/bin/env bash
# Run clippy (or test) only on the workspace members that actually changed.
#
#   scripts/lint-changed.sh              # clippy the changed crates
#   scripts/lint-changed.sh test         # test the changed crates
#   scripts/lint-changed.sh fmt          # check formatting of the changed crates
#   scripts/lint-changed.sh clippy main  # diff against a different base
#
# Falls back to the whole workspace only if it cannot work out the base.
set -euo pipefail
cd "$(dirname "$0")/.."

CMD="${1:-clippy}"
BASE="${2:-main}"

# Every path git reports as touched: committed since the merge-base, staged,
# and unstaged. A crate counts as changed if any of them lands inside it.
changed_paths() {
  local mb
  if mb=$(git merge-base HEAD "$BASE" 2>/dev/null); then
    git diff --name-only "$mb"...HEAD
  fi
  git diff --name-only HEAD
  git diff --name-only --cached
  git ls-files --others --exclude-standard
}

# name<TAB>relative-dir for each workspace member, longest dir first so that a
# nested member wins over its parent.
members=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c '
import json, os, sys
root = os.getcwd()
for pkg in json.load(sys.stdin)["packages"]:
    d = os.path.relpath(os.path.dirname(pkg["manifest_path"]), root)
    print(pkg["name"] + chr(9) + d)
' | awk -F'\t' '{print length($2)"\t"$0}' | sort -rn | cut -f2-)

pkgs=$(changed_paths | sort -u | while read -r f; do
  [ -n "$f" ] || continue
  while IFS=$'\t' read -r name dir; do
    if [ "$dir" = "." ] || [ "${f#"$dir"/}" != "$f" ]; then
      echo "$name"; break
    fi
  done <<< "$members"
done | sort -u)

# A change to the root manifest or a lockfile affects everything.
all_changed=$(changed_paths | sort -u)
if grep -qE '^(Cargo\.toml|Cargo\.lock|rust-toolchain.*)$' <<< "$all_changed"; then
  echo "root manifest changed -> whole workspace"
  pkgs=$(cut -f1 <<< "$members" | sort -u)
fi

if [ -z "$pkgs" ]; then
  echo "no workspace crate changed against '$BASE' -- nothing to do."
  exit 0
fi

args=(); while read -r p; do args+=(-p "$p"); done <<< "$pkgs"

# `--all-targets` covers lib, bins, tests, benches and examples -- but NOT doctests. So for years
# this gate ran none of them: every `///` example in the workspace was unverified, including the
# runnable native + declarative examples AGENTS.md makes mandatory for every widget. Found when a
# broken one passed this gate four times in a row (F003/P097/T501).
#
# Doctests need their own invocation, so `test` is two runs and the exit code is the worse of them.
if [ "$CMD" = "test" ]; then
  echo "==> cargo test ${args[*]} --all-targets"
  echo "==> cargo test ${args[*]} --doc"
  [ -n "${DRY_RUN:-}" ] && exit 0
  cargo test "${args[@]}" --all-targets; targets=$?
  # `--doc` is refused outright when no selected crate is a library (a bins-only selection), which
  # is not a failure -- there is simply nothing to run.
  doc_out=$(cargo test "${args[@]}" --doc 2>&1); doc=$?
  printf '%s\n' "$doc_out"
  if [ $doc -ne 0 ] && grep -q "no library targets found" <<< "$doc_out"; then
    doc=0
  fi
  [ $targets -ne 0 ] && exit $targets
  exit $doc
fi

# **Formatting is asked of the CRATE, never of a file.**
#
# `rustfmt <file>` treats that file as its own root, and `rustfmt <crate root>` follows every `mod`
# and formats the whole crate — and the two do not agree. Checking files one at a time said this
# workspace was clean while `rustfmt lib.rs` rewrote 83 of them, all of it trailing commas and line
# wrapping inside test modules. `cargo fmt` asks the crate, which is the answer that counts.
#
# It was also simply missing: this gate ran clippy and tests and never once looked at formatting,
# which is why nobody knew the two answers differed.
if [ "$CMD" = "fmt" ]; then
  echo "==> cargo fmt ${args[*]} --check"
  [ -n "${DRY_RUN:-}" ] && exit 0
  exec cargo fmt "${args[@]}" --check
fi

echo "==> cargo $CMD ${args[*]} --all-targets"
[ -n "${DRY_RUN:-}" ] && exit 0
exec cargo "$CMD" "${args[@]}" --all-targets
