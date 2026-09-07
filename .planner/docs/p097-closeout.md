# P097 closeout — what a resumer needs that the task records do not carry

P097 (F003, "One Tree — the input architecture") is **complete**: T494–T497 and T499–T502 done,
**T498 canceled** (duplicated `P094(F011)/T449`). The planner refuses a handoff on a finished phase,
so this is where the operational half lives. The *what and why* is in the task records —
`planner-task-show P097(F003)/T501 full=true` and `.../T502 full=true`. Read those first; this
file is only the things they do not hold.

## Open, and the first one is a live bug

Three defects from Antonio's drive of the T502 surface, none fixed:

1. **`view intent 'close_pane' did not resolve to a known action`** — the described confirm's Delete
   button fires an action name that does not exist, so **Delete does nothing**. It needs the real
   by-id action; the target pane is not necessarily the focused one.
   `heca/src/chrome/described_confirm.rs`, in `register`.
2. **`a Button at 0/2/0 … declares no key, and it is one of 2 siblings of that kind`** — the
   identity reporter wants keys on the two sibling buttons, and it is right: two buttons of one kind
   side by side is a **collection**, which is the one case a `key` is for.
3. **`key 'y' … CopySelection overwritten by ShowLayer`** — the binding suggested for testing
   (`prefix+y`) clobbers copy-selection. Pick another; nothing in the code depends on it.

## Operational

- **`/Users/antonio/projects/heca`**, branch **`feat/hint-collapses-to-the-tree`**, cut from `main`
  at `069e3b3`, **not pushed**. Five commits: `4cb7e24` (T499), `1b8a5b1` (T500 fixes), `d013f5c`
  (T501 part), `89a3b3e` (T501 complete), `673356d` (T502).
- **Gates:** `./scripts/lint-changed.sh test`, then `./scripts/lint-changed.sh`. Never
  `--workspace`. Baseline **1483 tests**, clippy exit 0 with exactly **one** warning (pre-existing
  `toast/stack.rs` "very complex type") plus a third-party `block v0.1.6` note.
- ⭐ **The test gate now runs doctests.** It passed `--all-targets`, which **excludes** them, so no
  `///` example in the workspace had ever been run — found when a broken one passed the gate four
  times. It runs both now and takes the worse exit code; a broken example makes it exit 101.
- ⭐ **The rustfmt recipe in every earlier handoff was WRONG.** It said `--edition 2021`; the
  workspace is **2024**, so it errored and counted **zero blocks on both sides** — "no file's count
  raised" was never actually measured. Correct form, per file:
  `git show HEAD:<f> | rustfmt --edition 2024 --check | grep -c "^Diff in"` versus the working copy.
  Count violation **blocks**, not lines. Never `cargo fmt --all`, never raise a count.
- ⚠️ **Do not pipe a gate through `tail`** — the pipeline's exit code is `tail`'s.
- ⚠️ Flaky, not ours: `heca-core`'s
  `terminal_backend_command_spawn_runs_and_stays_open_for_policy` fails intermittently in a full
  run, passes alone.
- **A handoff cannot be written on a done phase**, and completing its last task clears the one it
  had. Write the closeout before completing the final task, or into a file like this one.

## Rejected — do not retry

**Collapsing only Tab onto `Overlay`** while leaving the other three focus operations on `Dialog`.
Tried; it broke `input_edit_reaches_focused_field_nav_moves_to_button`. Half a centralisation moves
the disagreement rather than removing it — all four (open-quiet, Tab, arrow motion, click-in-panel)
go together or none do.

## Pending verification

The `Dialog` focus centralisation — the `prefix+x` first-Tab fix — was verified headlessly and
through the *described* surface, never by driving a real dialog. Worth one pass over `prefix+x` with
Tab, Shift+Tab, arrows, and a rename dialog's text field.

## What bit hardest, and cost the most

- **Fixing at the call site when the fault is in the library.** Three times in one session. The tell:
  reaching for the numbers, the key or the flag at the place that consumes them.
- **A guard proves nothing until it is run against its own bug**, and **two guards here were
  vacuous** as first written: a Tab test passed with the traversal deleted, because the activate key
  reached the far button anyway. **When a sabotage does not turn a guard red, suspect the sabotage
  first** — one of mine did not compile and read as a pass.
- ⚠️ **Never `git checkout` a modified file to undo a test sabotage.** It discards every uncommitted
  change in that file, and did. Restore from a copy.
- ⚠️ **A scripted write can destroy a file.** Write to a temp file and move it; check the tail
  afterwards. A `python` script that asserts *after* mutating and then never writes will silently
  drop half an edit — that happened twice, leaving the showcase's two halves out of step.

## Runtime limitations

No agent can see the UI. Instrument — `#[cfg(debug_assertions)] eprintln!` with one greppable
prefix, hand Antonio the exact gesture, strip it after. An integration test does **not** enable
`cfg(test)` in the library, so such a probe silently never compiles. `AppState` needs a window, so
there is **no headless call** to any handler taking `&mut AppState` — `heca/tests/by_id_actions.rs`
is written as a source lint for exactly that reason.

## Related records

- `.planner/docs/t497-open-issues.md` — this phase's defects, real causes, and disproved diagnoses.
- `.planner/docs/t497-header-buttons.md` — the pane header's action row and the API it added.
- `.planner/docs/p097-input-seams.md` — drag, pointer routing and the hint walk.
- `.planner/docs/session-2026-09-04-cli-and-resize-edges.md` — the resize edge argument and the CLI.
- `.planner/docs/oversized-files-2026-09-04.md` — the 65 files over 600 lines and who owns each.
- `.planner/docs/t468-oversized-files.md` — the split order and its traps.
