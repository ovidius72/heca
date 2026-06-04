# Session Resume Handoff — 2026-06-04

## Purpose

This file is for resuming the current refactor after clearing the session.

Primary active goal:
- begin **Phase 1.2** and split `heca/src/mouse.rs` after the Phase 1.1 `main.rs` split milestone

Secondary context that must not be lost:
- pull / merge `origin/main` **before starting each new task**
- preserve the recently restored keybindings:
  - `prefix+Ctrl+k/j` → `swap_up` / `swap_down`
  - `prefix+Ctrl+p/n` → workspace prev / next aliases
  - `prefix+Ctrl+Shift+k/j` → move column up / down
- floating-vs-tiled focus-domain routing is **deferred intentionally** to the last phase in `bugs-and-refactoring-plan.md`

---

## Last committed state

Latest relevant commits before the current commit:
- `a13439a` — `Restore workspace key aliases and continue app refactor`
- `577033a` — `Add column rename support and continue app refactor`

The current work should be committed as the Phase 1.1 runtime split milestone.

---

## Current uncommitted working tree

This handoff was originally written before the Phase 1.1 milestone commit.
After committing the current runtime split, this section should effectively become:

- no intended uncommitted Phase 1.1 runtime-split changes remain

If you are resuming after the commit, verify with:

```bash
git status --short
```

---

## What was extracted already

### Earlier committed extractions
Already living under `heca/src/app/`:
- `focus.rs`
- `mutations.rs`
- `registry.rs`
- `render.rs` (initial helpers)
- `selection.rs`

### Runtime extractions completed in the Phase 1.1 milestone
New or expanded in that slice:

#### `heca/src/app/startup.rs`
Owns first-launch app initialization:
- window creation
- wgpu surface / adapter / device setup
- renderer initialization
- initial `Session` creation
- initial fake pane/backend creation
- initial sidebar tree build
- initial `AppState` construction

`main.rs` now delegates `init_state()` to this helper.

#### `heca/src/app/input.rs`
Owns the large keyboard input-mode dispatch that used to live inside `main.rs` `window_event()`:
- rename input handling
- confirm-delete handling
- prefix mode
- chord mode
- custom mode handling
- pane select
- pane swap
- pane take
- sidebar navigation mode

Main entrypoint there:
- `handle_keyboard_input(...)`

#### `heca/src/app/keyboard.rs`
Now also owns:
- `build_event_combo(...)`
- `is_prefix_match(...)`

It already owned:
- `normalize_key_text(...)`
- `event_combo_matches(...)`
- `typed_candidate_char(...)`
- `winit_key_to_terminal_input(...)`

#### `heca/src/app/render.rs`
Now also owns:
- `status_mode_parts(...)`

It already owned:
- `render_backend_data(...)`
- `update_session_viewport(...)`

---

## Current main.rs state

`heca/src/main.rs` is now down to about **168 LOC**.

Phase 1.1’s main acceptance target is effectively met.

What still remains in `main.rs`:
- `pane_name()` helper
- `HecaApp` shell / binary entrypoint glue
- very thin delegation to app modules

What has already been removed from `main.rs`:
- registry/keymap construction helpers
- focus bookkeeping helpers
- mutation helpers
- candidate lookup helpers
- startup bootstrap details
- keyboard-mode logic
- frame rendering
- `window_event()` internals
- `about_to_wait()` internals

---

## Validation status

This uncommitted slice passed:
- `cargo fmt`
- `cargo check -q`
- `cargo clippy --workspace --all-targets --all-features --quiet`

Run them again after resuming if anything changes.

Recommended command:

```bash
cargo fmt && cargo check -q && cargo clippy --workspace --all-targets --all-features --quiet
```

---

## Exact resume procedure

### 1. Sync first
User explicitly asked for this workflow rule:

```bash
git fetch origin main && git merge --no-edit origin/main
```

Do this **before starting the next task**.

### 2. Confirm working tree
Check:

```bash
git status --short
```

You should still see the uncommitted files listed above.

### 3. Re-read these files first
To reload context quickly:
- `bugs-and-refactoring-plan.md`
- `session-resume-handoff.md`
- `heca/src/main.rs`
- `heca/src/app/input.rs`
- `heca/src/app/startup.rs`
- `heca/src/app/render.rs`
- `heca/src/app/keyboard.rs`
- `heca/src/app/mod.rs`

### 4. Re-run validation before new edits
Even if the tree looks unchanged:

```bash
cargo check -q
cargo clippy --workspace --all-targets --all-features --quiet
```

### 5. Continue Phase 1.1 only
Do **not** jump ahead to floating focus-domain fixes or parameterized spawn work yet.

---

## Recommended next steps

### Best next slice
Start Phase 1.2 and split `heca/src/mouse.rs`.

Suggested shape:
- `heca/src/mouse/mod.rs`
- `heca/src/mouse/hit_test.rs`
- `heca/src/mouse/drag.rs`
- `heca/src/mouse/drop.rs`
- `heca/src/mouse/render.rs`
- `heca/src/mouse/sidebar.rs`

Goal:
- keep public mouse entrypoints thin
- separate hit-testing, drag transitions, drop semantics, and rendering helpers

### After that
Move on to:
- `sidebar.rs` split
- shared pane-operation deduplication

---

## Important behavior constraints while resuming

### Keybindings
Do not regress these again:
- `prefix+Ctrl+k` → `swap_up`
- `prefix+Ctrl+j` → `swap_down`
- `prefix+Ctrl+p` → workspace previous
- `prefix+Ctrl+n` → workspace next
- `prefix+Ctrl+Shift+k` → move column up
- `prefix+Ctrl+Shift+j` → move column down

Relevant files:
- `heca-config/src/theme.rs`
- `keybindings.toml`
- `heca/src/app/registry.rs` tests

### Rename bindings
Recently updated and should stay:
- `prefix+$` → rename pane/tab
- `prefix+Shift+w` → rename workspace
- `prefix+Shift+c` → rename column

### Zoom behavior
Already implemented and should not be disturbed:
- `prefix+z` toggles active-column zoom
- multiple columns can remain zoomed independently

### Floating focus-domain bug
Still intentionally deferred.
Do **not** “quick-fix” it during the `main.rs` refactor.
That work is scheduled for the final phase.

---

## If you want to finish the current slice cleanly

Suggested order:

1. sync with `origin/main`
2. validate current working tree
3. extract `render()` helpers into `app/render.rs`
4. validate
5. extract `window_event()` into `app/events.rs`
6. validate
7. extract `about_to_wait()` into `app/lifecycle.rs`
8. validate
9. update `bugs-and-refactoring-plan.md` with new status
10. commit the whole Phase 1.1 slice

Suggested commit message when that slice is ready:

```bash
git commit -m "Continue splitting main runtime into app modules"
```

---

## Sanity checks after more refactor work

Run at least:

```bash
cargo fmt
cargo check -q
cargo clippy --workspace --all-targets --all-features --quiet
```

And ideally do a manual smoke test after one or two more extractions:

```bash
cargo run -p heca
```

Check:
- app launches
- prefix mode works
- `prefix+Ctrl+k/j` swap still works
- `prefix+Ctrl+p/n` workspace switching still works
- `prefix+$`, `prefix+Shift+w`, `prefix+Shift+c` rename flows still work
- `prefix+z` zoom still works
- sidebar navigation still works

---

## Summary

Resume from the **committed Phase 1.1 `main.rs` split milestone**.

That work moved substantial runtime behavior into:
- `app/startup.rs`
- `app/input.rs`
- `app/events.rs`
- `app/lifecycle.rs`
- `app/keyboard.rs`
- `app/render.rs`

The next best move is to start **Phase 1.2** and split `mouse.rs` while keeping behavior unchanged.
