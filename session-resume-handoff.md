# Session Resume Handoff — 2026-06-04

## Purpose

This file is the current resume note for the active refactor.

Primary active goal:
- continue **Phase 1.2** and finish decomposing the mouse drag/drop system

Non-negotiable workflow rule from the user:
- **pull / merge `origin/main` before starting each new task**

Important deferred item:
- floating-vs-tiled focus-domain routing is **intentionally deferred** to the **last phase** of `bugs-and-refactoring-plan.md`

---

## Current branch state

Latest already-committed milestones relevant to this refactor:
- `577033a` — `Add column rename support and continue app refactor`
- `a13439a` — `Restore workspace key aliases and continue app refactor`
- `ad1acaa` — `Finish main runtime split into app modules`
- `85aaed6` — `Fix keybinding conflicts and prefix handling`

Current uncommitted work is the ongoing **Phase 1.2 mouse split**.

At the moment, the working tree should show:
- modified: `bugs-and-refactoring-plan.md`
- modified: `session-resume-handoff.md`
- modified: `heca/src/mouse.rs`
- new: `heca/src/mouse/hit_test.rs`
- new: `heca/src/mouse/render.rs`
- new: `heca/src/mouse/drag.rs`
- new: `heca/src/mouse/drop.rs`

---

## What is already done

### 1. Phase 1.1 is effectively complete

`heca/src/main.rs` was reduced to a thin module root / binary shell.

Work already extracted into `heca/src/app/`:
- `registry.rs`
- `focus.rs`
- `mutations.rs`
- `render.rs`
- `selection.rs`
- `keyboard.rs`
- `startup.rs`
- `input.rs`
- `events.rs`
- `lifecycle.rs`

Why this matters:
- runtime ownership is much clearer
- `main.rs` is no longer the place where every system detail accumulates
- future behavior changes can happen in narrower modules with less review risk

### 2. Keybinding and prefix regressions from review were fixed

Already done:
- `prefix+p` is command palette again
- pane cycling moved to `prefix+[` / `prefix+]`
- `prefix+Ctrl+k/j` swap bindings restored
- `prefix+Ctrl+p/n` workspace aliases restored
- configurable double-prefix forwarding fixed
- built-in and user custom modes now merge by mode name
- startup/reload conflict logging made deterministic and visible
- quick-select / swap / take overflow now falls back instead of partially labeling panes

Why this matters:
- keyboard behavior is now deterministic again
- config reload behavior is safer and easier to debug
- future input/config work has a cleaner base

### 3. Rename and zoom work is already in place

Already done:
- `prefix+z` toggles active-column zoom
- zoom is per-column and independent
- pane rename is `prefix+$`
- workspace rename is `prefix+Shift+w`
- column rename is `prefix+Shift+c`
- column names render in the sidebar
- RPC support exists for zoom and column rename

Why this matters:
- the product-level behavior the user asked for is already landed
- current refactor work should preserve these paths, not redesign them

### 4. Phase 1.2 is substantially progressed

Extracted from `heca/src/mouse.rs` so far:

#### `heca/src/mouse/hit_test.rs`
Owns:
- `hit_test_pane()`
- `sidebar_pane_hit_test()`

#### `heca/src/mouse/render.rs`
Owns:
- `render_detached_pane()`
- `render_insert_hint()`

#### `heca/src/mouse/drag.rs`
Owns:
- `on_cursor_moved()` internals
- `update_sidebar_drag_hover()`
- `start_interactive_move()`
- `transition_to_moving()`
- `cancel_interactive_move()`

#### `heca/src/mouse/drop.rs`
Owns:
- `drop_pane()`
- `sidebar_drag_drop()`
- `sidebar_handle_drop()`

Current file sizes:
- `heca/src/mouse.rs` — **551 LOC**
- `heca/src/mouse/hit_test.rs` — **102 LOC**
- `heca/src/mouse/render.rs` — **177 LOC**
- `heca/src/mouse/drag.rs` — **310 LOC**
- `heca/src/mouse/drop.rs` — **586 LOC**

Why this matters:
- the top-level mouse module is much easier to navigate
- the major responsibilities now have clearer homes
- the remaining complexity is concentrated in one place instead of smeared across the whole file

---

## What is not done yet

### 1. Phase 1.2 is not fully complete

Still true:
- `mouse/drop.rs` is too large for the intended end-state
- pane placement / reinsertion logic is isolated, but not yet internally decomposed enough

Why it is not fully done yet:
- `mouse.rs` itself is much cleaner now, but the most delicate logic was moved largely intact
- `mouse/drop.rs` still mixes:
  - detached-pane reinsertion
  - insert-vs-swap behavior
  - sidebar drop targeting
  - column/workspace placement rules
  - redraw/focus cleanup
- that means complexity has been localized, but not fully simplified

### 2. Parameterized normal keybindings are still future work

Still not implemented:
- `[[keys.bind]]` for parameterized normal bindings
- generalized structured spawn action / size parsing

Why not now:
- the agreed order is structure-first
- finishing `mouse.rs` / `sidebar.rs` cleanup lowers risk before deeper config/runtime changes

### 3. Floating focus-domain routing is still deferred

Still not fixed:
- handlers that should act on the focused floating pane often still mutate tiled `ws.scrolling...` state instead

Why deferred:
- the user explicitly asked for this to be the **last phase** in the plan
- it is a semantic correctness project, not a structural refactor task

---

## What we are done with, and why

We are effectively **done with Phase 1.1**.

Reason:
- `heca/src/main.rs` already hit the intended outcome: thin entrypoint, delegated runtime systems, better navigability
- more Phase 1.1 work would mostly be optional micro-cleanup, not a meaningful risk reducer

We are **done with the first-pass extraction of `mouse.rs`**.

Reason:
- hit testing, rendering, drag transitions, and drop logic now have distinct files
- the big structural separation work has landed

We are **not done with Phase 1.2 refinement**.

Reason:
- `mouse/drop.rs` is still oversized
- the densest placement logic still needs one more decomposition pass
- finishing that pass is higher value than reopening already-thin `main.rs`

So the current strategy is:
1. stop reopening completed `main.rs` work
2. finish decomposing `mouse/drop.rs`
3. then move to `sidebar.rs`
4. only after the structure work is calmer, continue parameterized keybinding / spawn work

---

## Validation status

The current uncommitted mouse-split slice passed:
- `cargo fmt`
- `cargo check -q`
- `cargo clippy --workspace --all-targets --all-features --quiet`

Recommended command before any new edits:

```bash
cargo fmt && cargo check -q && cargo clippy --workspace --all-targets --all-features --quiet
```

After one or two more slices, also run a smoke test:

```bash
cargo run -p heca --quiet
```

Expected behavior for that smoke test:
- app starts and stays in event loop
- no startup error before timeout
- prefix mode still works
- rename bindings still work
- zoom still works
- sidebar interactions still work

---

## Exact next step

### Best next slice

Decompose:
- `heca/src/mouse/drop.rs`

Likely options:
- extract shared reinsertion helpers
- extract sidebar-target-specific helper paths
- reduce duplication between content-drop and sidebar-drop swap fallback logic

Goal of that slice:
- keep behavior unchanged
- make drop logic locally readable
- bring the remaining oversized mouse submodule closer to the target end-state

### Recommended extraction order

1. sync with `origin/main`
2. validate current tree
3. identify the most repeated reinsertion / fallback patterns in `mouse/drop.rs`
4. extract one helper at a time
5. validate after each small step
6. only then decide whether `mouse/sidebar.rs` is worth adding

### Why this is the best next move

Because the main structural split already succeeded, and the last big readability hotspot inside the mouse system is now concentrated in `mouse/drop.rs`.

---

## Exact resume procedure

### 1. Sync first

Always do this before starting a new task:

```bash
git fetch origin && git merge --ff-only origin/main
```

### 2. Confirm working tree

```bash
git status --short
```

You should see the current uncommitted Phase 1.2 mouse files.

### 3. Re-read these files first

- `bugs-and-refactoring-plan.md`
- `session-resume-handoff.md`
- `heca/src/mouse.rs`
- `heca/src/mouse/hit_test.rs`
- `heca/src/mouse/render.rs`
- `heca/src/mouse/drag.rs`
- `heca/src/mouse/drop.rs`

### 4. Re-run validation before changing behavior

```bash
cargo check -q
cargo clippy --workspace --all-targets --all-features --quiet
```

### 5. Continue only the current structure slice

Do **not** jump to:
- floating focus-domain routing fixes
- parameterized keybinding implementation
- spawn-pane backend redesign

until the current mouse split is in a cleaner state.

---

## Behavior constraints that must not regress

### Keybindings
Keep these exactly:
- `prefix+Ctrl+k` → `swap_up`
- `prefix+Ctrl+j` → `swap_down`
- `prefix+Ctrl+p` → workspace previous
- `prefix+Ctrl+n` → workspace next
- `prefix+Ctrl+Shift+k` → move column up
- `prefix+Ctrl+Shift+j` → move column down
- `prefix+p` → command palette
- `prefix+[` / `prefix+]` → prev / next pane

Relevant files:
- `heca-config/src/theme.rs`
- `keybindings.toml`
- `heca/src/app/registry.rs`

### Rename bindings
Keep these exactly:
- `prefix+$` → rename pane/tab
- `prefix+Shift+w` → rename workspace
- `prefix+Shift+c` → rename column

### Zoom behavior
Keep these exactly:
- `prefix+z` toggles zoom for the active column
- zoom is a toggle
- multiple columns may remain zoomed independently

### Float/unfloat behavior
Keep these exactly:
- `prefix+f` remains float/unfloat toggle
- manually floated tiled panes should restore to their original tiled location when possible
- panes spawned floating without an original tiled slot should unfloat into a new column

### Deferred bug policy
Do not silently “fix” the floating focus-domain routing bug inside the structural mouse split. Documented deferment is intentional.

---

## Short summary

We are **done with the `main.rs` runtime split**.

We are **done with the first-pass `mouse.rs` split**:
- hit testing extracted
- drag transitions extracted
- detached-pane rendering extracted
- drop logic extracted

We are **not done with Phase 1.2 refinement** because `mouse/drop.rs` is still too large.

The next best move is to decompose that drop module further, validate, and then continue to the planned `sidebar.rs` split.
