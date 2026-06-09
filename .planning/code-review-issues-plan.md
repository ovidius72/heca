# Code Review Issues — Resolution Plan

> Status: **Planning**
> Created: 2026-06-09
> Based on: initial Rust skill review of heca codebase (excluding heca-grid-ui)

## Issues Found

| ID | Severity | Description | File(s) |
|----|----------|-------------|---------|
| I1 | HIGH | Dead code `drop.rs::drop_pane()` — 100+ lines with `#[allow(dead_code)]` | `mouse/drop.rs` |
| I2 | MEDIUM | `#[allow(dead_code)]` on `InputMode::Chord` variant | `app_state.rs` |
| I3 | MEDIUM | `unreachable!()` in production paths | `mouse/drag.rs`, `mouse.rs` |
| I4 | MEDIUM | `on_cursor_moved()` too long (163 lines) | `mouse/drag.rs` |
| I5 | LOW-MED | Shadowed mutable borrows in `mouse.rs` release handler | `mouse.rs` |
| I6 | MEDIUM | `on_mouse_input()` release arm too long (~100 lines) | `mouse.rs` |
| I7 | LOW | Hardcoded chrome geometry (tab_bar_height: 32.0, etc.) duplicated 3x | `mouse.rs`, `app/render.rs`, `chrome.rs` |
| I8 | LOW | `pane_area` as `(f32, f32, f32, f32)` tuple instead of `Rectangle` | `mouse/render.rs` |

## Resolution Plan

### R1: Remove dead code (I1, I2)

**Action:** Delete `mouse/drop.rs` entirely. Remove `InputMode::Chord` variant.

**Verification:** `cargo check -p heca` passes. `cargo clippy --workspace` clean (no dead_code warnings).

**Risk:** Very low — `drop_pane()` is marked dead code, `Chord` is unused.

**Commit:** `refactor: remove dead code (drop.rs, InputMode::Chord)`

### R2: Replace `unreachable!()` with `expect()` (I3)

**Action:** Replace all `unreachable!()` in `mouse/drag.rs` and `mouse.rs` with
`expect("descriptive message")`.

Examples:
- `mouse/drag.rs:119` — `DragState::InteractiveMove { offset, .. }` after `matches!` guard → `expect("InteractiveMove variant after matches guard")`
- `mouse.rs:194` — `DragState::InteractiveMove { _pane_id, .. }` after match guard → `expect("InteractiveMove variant confirmed by outer match")`

**Verification:** `cargo test -p heca` passes. Code reads more defensively.

**Commit:** `refactor: replace unreachable() with expect() in mouse paths`

### R3: Split `on_cursor_moved()` into named helpers (I4)

**Action:** Extract 4 helper functions from `on_cursor_moved()`:

```rust
fn handle_interactive_move_starting(state: &mut AppState, pos: (f32, f32))
fn handle_sidebar_drag_starting(state: &mut AppState, pos: (f32, f32))
fn handle_interactive_move_drag(state: &mut AppState, pos: (f32, f32))
fn handle_sidebar_drag_move(state: &mut AppState, pos: (f32, f32))
```

The main `on_cursor_moved()` becomes a thin router calling these in order.

**Verification:** `cargo test -p heca` passes. Function lengths < 40 lines each.

**Commit:** `refactor: split on_cursor_moved into named helpers`

### R4: Extract release handlers from `on_mouse_input()` (I5, I6)

**Action:** Extract 3 named functions from the release arm:

```rust
fn handle_interactive_move_release(state: &mut AppState, pos: (f32, f32))
fn handle_sidebar_drag_release(state: &mut AppState, pane_id: u64, original_ws: usize, swap: bool, pos: (f32, f32))
fn handle_sidebar_drag_starting_release(state: &mut AppState) -> Option<WmAction>
```

This also resolves the shadowed mutable borrow issue (I5) because each function
gets its own clean `&mut AppState`.

**Verification:** `cargo test -p heca` passes. `on_mouse_input()` < 50 lines.

**Commit:** `refactor: extract mouse release handlers`

### R5: Consolidate chrome geometry constants (I7)

**Action:** Add a `ChromeConfig::defaults()` or remove the hardcoded values from
`mouse.rs` and `app/render.rs`, reading them from a shared source.

Currently:
- `mouse.rs:355` — `left_sidebar_width: if state.sidebar.left_visible { ... }`
- `app/render.rs:148-149` — same
- `app/startup.rs:107` — `right_sidebar_width: 200.0`

The `ChromeConfig` struct already exists in `chrome.rs`. The issue is that
`tab_bar_height: 32.0` and `status_bar_height: 24.0` are hardcoded in 3 places.

**Fix:** Add `const TAB_BAR_HEIGHT: f32 = 32.0;` and `const STATUS_BAR_HEIGHT: f32 = 24.0;`
to `ChromeConfig` impl or as associated constants, then use those in all 3 places.

**Verification:** `cargo test -p heca` passes. No behavioral change.

**Commit:** `refactor: consolidate chrome geometry constants`

### R6: Use `Rectangle` instead of `(f32, f32, f32, f32)` in render (I8)

**Action:** Change `render_detached_pane()` and `render_insert_hint()` signatures
from `pane_area: (f32, f32, f32, f32)` to use `Rectangle` from `heca_core::layout`.

**Verification:** `cargo test -p heca` passes. Visual test: drag overlay renders correctly.

**Commit:** `refactor: use Rectangle type in mouse render functions`

## Execution Order

These are independent of the DnD refactoring and should be done **before** it,
since several of them (R3, R4) make the DnD refactor easier by slimming down
the functions that will be restructured.

1. **R1** — Remove dead code (5 min)
2. **R2** — Replace `unreachable!()` (5 min)
3. **R6** — Use `Rectangle` type (10 min)
4. **R5** — Consolidate chrome constants (10 min)
5. **R3** — Split `on_cursor_moved()` (20 min)
6. **R4** — Extract release handlers (30 min)

Total estimated time: ~80 minutes. Each commit is independent and revertible.

## After R1–R6: DnD Refactoring

Once R3 and R4 are done (`on_cursor_moved` and `on_mouse_input` are slimmed),
the DnD refactoring phases A–E become much cleaner because the functions being
restructured are already decomposed.

See `dnd-refactor-plan.md` for the DnD-specific plan.