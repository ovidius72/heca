# heca Refactoring & DnD Plan

> Created: 2026-06-09
> Two tracks: (1) code review fixes, (2) DnD system extraction into heca-grid-ui

---

## Track 1: Code Review Refactoring

Preparatory cleanups that make the DnD refactor easier. Each task is a single
commit, independently revertible.

### Phase 1.1: Remove dead code

| Task | What | Files |
|------|------|-------|
| 1.1.1 | Delete `mouse/drop.rs` entirely (142 lines, `#[allow(dead_code)]`) | `heca/src/mouse/drop.rs` |
| 1.1.2 | Remove `InputMode::Chord` variant and its `#[allow(dead_code)]` | `heca/src/app_state.rs` |
| 1.1.3 | Remove `mod drop` from `mouse.rs` | `heca/src/mouse.rs` |
| 1.1.4 | Remove `use crate::mouse::drop` references if any | `heca/src/mouse.rs` |
| 1.1.5 | Run `cargo clippy --workspace --all-targets --all-features`, fix warnings | workspace |
| 1.1.6 | Run `cargo test --workspace`, verify all pass | workspace |
| 1.1.7 | Commit: `refactor: remove dead code (drop.rs, InputMode::Chord)` | |

### Phase 1.2: Replace `unreachable!()` with `expect()`

| Task | What | Files |
|------|------|-------|
| 1.2.1 | Replace `unreachable!()` in `mouse/drag.rs` with `expect("...")` | `heca/src/mouse/drag.rs` |
| 1.2.2 | Replace `unreachable!()` in `mouse.rs` with `expect("...")` | `heca/src/mouse.rs` |
| 1.2.3 | Replace any other `unreachable!()` in mouse/ submodules | `heca/src/mouse/*.rs` |
| 1.2.4 | Run `cargo test --workspace`, verify all pass | workspace |
| 1.2.5 | Commit: `refactor: replace unreachable() with expect() in mouse paths` | |

### Phase 1.3: Use `Rectangle` type in mouse render functions

| Task | What | Files |
|------|------|-------|
| 1.3.1 | Change `render_detached_pane(state, pane_area: (f32, f32, f32, f32))` → `pane_area: Rectangle` | `heca/src/mouse/render.rs` |
| 1.3.2 | Change `render_insert_hint(state, pane_area: (f32, f32, f32, f32))` → `pane_area: Rectangle` | `heca/src/mouse/render.rs` |
| 1.3.3 | Update all call sites in `app/render.rs` to pass `Rectangle` | `heca/src/app/render.rs` |
| 1.3.4 | Run `cargo test --workspace`, verify all pass | workspace |
| 1.3.5 | Commit: `refactor: use Rectangle type in mouse render functions` | |

### Phase 1.4: Consolidate chrome geometry constants

| Task | What | Files |
|------|------|-------|
| 1.4.1 | Add `pub const TAB_BAR_HEIGHT: f32 = 32.0` and `pub const STATUS_BAR_HEIGHT: f32 = 24.0` to `ChromeConfig` | `heca/src/chrome.rs` |
| 1.4.2 | Replace hardcoded `32.0` / `24.0` in `mouse.rs` `chrome_config()` | `heca/src/mouse.rs` |
| 1.4.3 | Replace hardcoded `32.0` / `24.0` in `app/render.rs` | `heca/src/app/render.rs` |
| 1.4.4 | Replace hardcoded `32.0` / `24.0` in `app/startup.rs` if present | `heca/src/app/startup.rs` |
| 1.4.5 | Run `cargo test --workspace`, verify all pass | workspace |
| 1.4.6 | Commit: `refactor: consolidate chrome geometry constants` | |

### Phase 1.5: Split `on_cursor_moved()` into named helpers

| Task | What | Files |
|------|------|-------|
| 1.5.1 | Extract `handle_interactive_move_starting(state, pos)` from `on_cursor_moved` | `heca/src/mouse/drag.rs` |
| 1.5.2 | Extract `handle_sidebar_drag_starting(state, pos)` from `on_cursor_moved` | `heca/src/mouse/drag.rs` |
| 1.5.3 | Extract `handle_interactive_move_drag(state, pos)` from `on_cursor_moved` | `heca/src/mouse/drag.rs` |
| 1.5.4 | Extract `handle_sidebar_drag_move(state, pos)` from `on_cursor_moved` | `heca/src/mouse/drag.rs` |
| 1.5.5 | Rewrite `on_cursor_moved` as thin router calling the 4 helpers | `heca/src/mouse/drag.rs` |
| 1.5.6 | Move `update_sidebar_drag_hover` into a helper (still hardcoded for now, will be generalized in Track 2) | `heca/src/mouse/drag.rs` |
| 1.5.7 | Run `cargo test --workspace`, verify all pass | workspace |
| 1.5.8 | Commit: `refactor: split on_cursor_moved into named helpers` | |

### Phase 1.6: Extract release handlers from `on_mouse_input()`

| Task | What | Files |
|------|------|-------|
| 1.6.1 | Extract `handle_interactive_move_release(state, pos)` from `on_mouse_input` release arm | `heca/src/mouse.rs` |
| 1.6.2 | Extract `handle_sidebar_drag_release(state, pane_id, original_ws, swap, pos)` | `heca/src/mouse.rs` |
| 1.6.3 | Extract `handle_sidebar_drag_starting_release(state) -> Option<WmAction>` | `heca/src/mouse.rs` |
| 1.6.4 | Rewrite `on_mouse_input` release arm as thin match routing to the 3 handlers | `heca/src/mouse.rs` |
| 1.6.5 | Also extract the press-arm logic into `handle_mouse_press(state, pos) -> Option<WmAction>` | `heca/src/mouse.rs` |
| 1.6.6 | Run `cargo test --workspace`, verify all pass | workspace |
| 1.6.7 | Commit: `refactor: extract mouse press/release handlers` | |

---

## Track 2: DnD System

Generic, surface-agnostic drag-and-drop framework. Framework types live in
`heca-grid-ui/src/drag/`. App-specific dispatch lives in `heca/src/mouse/`.

### Design

- **Per-surface state**: each sidebar/container owns its own `SurfaceDragState`
- **One active drag**: `DragContext` tracks which surface is dragging
- **Enum dispatch**: `DragSurfaceId` is a closed enum, not trait objects
- **GPU-free framework**: `heca-grid-ui/src/drag/` depends on `heca-core` only
- **App-side effects**: pane mutation stays in `heca/src/mouse/surface_left.rs`

### Phase 2.1: Create framework types in heca-grid-ui

New directory, new files only. No changes to existing heca-grid-ui files.

| Task | What | Files |
|------|------|-------|
| 2.1.1 | Create `heca-grid-ui/src/drag/mod.rs` — module re-exports | new file |
| 2.1.2 | Create `heca-grid-ui/src/drag/item.rs` — `DragSurfaceId`, `DragItemId`, `DragItemKind`, `DragItem` | new file |
| 2.1.3 | Create `heca-grid-ui/src/drag/state.rs` — `SurfaceDragPhase`, `SurfaceDragState`, `DragLabel` | new file |
| 2.1.4 | Create `heca-grid-ui/src/drag/context.rs` — `DragContext` (coordinator: active surface, per-surface state map) | new file |
| 2.1.5 | Create `heca-grid-ui/src/drag/math.rs` — `rubberband()`, threshold constants | new file |
| 2.1.6 | Add `pub mod drag;` to `heca-grid-ui/src/lib.rs` | `lib.rs` (one line) |
| 2.1.7 | Add re-exports to `heca-grid-ui/src/lib.rs` prelude | `lib.rs` (few lines) |
| 2.1.8 | Run `cargo check -p heca-grid-ui`, verify it compiles |  |
| 2.1.9 | Run `cargo check --workspace`, verify nothing broke |  |
| 2.1.10 | Commit: `feat(grid-ui): add drag framework types (DragSurfaceId, DragContext, SurfaceDragState)` | |

### Phase 2.2: Heca consumes framework types (mechanical refactor)

Add `heca-grid-ui` dependency to `heca`. Replace `DragState`, `SidebarDragLabel`,
and `MouseState` sidebar-specific fields with the new framework types. All
existing behavior preserved.

| Task | What | Files |
|------|------|-------|
| 2.2.1 | Add `heca-grid-ui = { path = "../heca-grid-ui" }` to `heca/Cargo.toml` | `heca/Cargo.toml` |
| 2.2.2 | Replace `DragState` enum in `app_state.rs` with `SurfaceDragPhase` from `heca_grid_ui::drag` | `heca/src/app_state.rs` |
| 2.2.3 | Replace `SidebarDragLabel` with `DragLabel` from `heca_grid_ui::drag` | `heca/src/app_state.rs` |
| 2.2.4 | Add `InteractiveMoveState` struct to `app_state.rs` for content-area drag (separate from surface drag) | `heca/src/app_state.rs` |
| 2.2.5 | Restructure `MouseState`: replace `drag_state`, `drag_hover_sidebar_fi`, `sidebar_drag_source_fi`, `sidebar_drag_label` with `drag_ctx: DragContext` + `interactive_move: Option<InteractiveMoveState>` | `heca/src/app_state.rs` |
| 2.2.6 | Update all `MouseState` field access in `mouse/` submodules | `heca/src/mouse/*.rs` |
| 2.2.7 | Update all `DragState` match arms in `mouse/drag.rs`, `mouse.rs` | `heca/src/mouse/drag.rs`, `mouse.rs` |
| 2.2.8 | Update drag ghost rendering in `mouse/render.rs` to use `DragLabel` | `heca/src/mouse/render.rs` |
| 2.2.9 | Update sidebar render calls in `app/render.rs` to use `DragContext` fields | `heca/src/app/render.rs` |
| 2.2.10 | Run `cargo test --workspace`, verify all pass | workspace |
| 2.2.11 | Run `cargo clippy --workspace --all-targets --all-features`, fix warnings | workspace |
| 2.2.12 | Manual test: left sidebar drag/drop, content interactive move, swap, edge scroll |  |
| 2.2.13 | Commit: `refactor: consume DnD framework types from heca-grid-ui` | |

### Phase 2.3: Extract surface dispatch (enum dispatch)

Create `target.rs` with free functions that dispatch by `DragSurfaceId`.
Create `surface_left.rs` with all left-sidebar-specific logic. Delete the
files it absorbs.

| Task | What | Files |
|------|------|-------|
| 2.3.1 | Create `heca/src/mouse/target.rs` — enum dispatch functions: `surface_bounds`, `surface_item_at`, `surface_ghost_label`, `surface_should_highlight_hover`, `surface_should_highlight_source`, `surface_can_accept`, `surface_accept_drop`, `surface_click_action` | new file |
| 2.3.2 | Create `heca/src/mouse/surface_left.rs` — move all left-sidebar hit testing, ghost label, drop logic from `sidebar_drop.rs` and `sidebar.rs` | new file |
| 2.3.3 | Wire `target.rs` dispatch functions to call `surface_left::*` for `DragSurfaceId::LeftSidebar` | `heca/src/mouse/target.rs` |
| 2.3.4 | Update `mouse/drag.rs` to route through `DragContext` + `target.rs` instead of hardcoded sidebar geometry | `heca/src/mouse/drag.rs` |
| 2.3.5 | Update `mouse.rs::on_mouse_input` press arm to use `surface_item_at` + `surface_can_start_drag` | `heca/src/mouse.rs` |
| 2.3.6 | Update `mouse.rs::on_mouse_input` release arm to use `surface_accept_drop` | `heca/src/mouse.rs` |
| 2.3.7 | Delete `heca/src/mouse/sidebar_drop.rs` (absorbed into `surface_left.rs`) | delete file |
| 2.3.8 | Delete `heca/src/mouse/sidebar.rs` (absorbed into `surface_left.rs`) | delete file |
| 2.3.9 | Delete `heca/src/mouse/drop.rs` (dead code, should already be gone from Phase 1.1) | delete file |
| 2.3.10 | Update `mod` declarations in `mouse.rs` | `heca/src/mouse.rs` |
| 2.3.11 | Run `cargo test --workspace`, verify all pass | workspace |
| 2.3.12 | Run `cargo clippy --workspace --all-targets --all-features`, fix warnings | workspace |
| 2.3.13 | Manual test: left sidebar drag/drop to content area, sidebar-to-sidebar drop, swap, cancel |  |
| 2.3.14 | Commit: `refactor: extract surface dispatch and left-sidebar handler` | |

### Phase 2.4: Split content-area drag into interactive.rs

Extract `InteractiveMove` logic from `drag.rs` into `interactive.rs`. Slim down
`drag.rs` to only surface-drag routing and `DragContext` updates.

| Task | What | Files |
|------|------|-------|
| 2.4.1 | Create `heca/src/mouse/interactive.rs` — move all `InteractiveMoveStarting`/`InteractiveMove` handlers from `drag.rs` | new file |
| 2.4.2 | Create `heca/src/mouse/interactive.rs` — move `start_interactive_move`, `cancel_interactive_move`, `reset_interactive_move_offset`, `InteractiveMoveState` helpers | new file |
| 2.4.3 | Slim `drag.rs` down to surface-drag routing + `DragContext` updates only | `heca/src/mouse/drag.rs` |
| 2.4.4 | Update `mod` declarations in `mouse.rs` | `heca/src/mouse.rs` |
| 2.4.5 | Run `cargo test --workspace`, verify all pass | workspace |
| 2.4.6 | Run `cargo clippy --workspace --all-targets --all-features`, fix warnings | workspace |
| 2.4.7 | Commit: `refactor: extract content-area InteractiveMove into interactive.rs` | |

### Phase 2.5: Render integration — per-surface drag highlighting

Update drag rendering to use `DragContext` per-surface state instead of global
sidebar-specific fields.

| Task | What | Files |
|------|------|-------|
| 2.5.1 | Update `sidebar/render.rs` signatures: replace `drag_hover_fi: Option<usize>` + `drag_source_fi: Option<usize>` with `drag_hover: Option<DragItemId>` + `drag_source: Option<DragItemId>` | `heca/src/sidebar/render.rs` |
| 2.5.2 | Update `sidebar/render.rs` internals: compare `DragItemId` instead of raw `usize` | `heca/src/sidebar/render.rs` |
| 2.5.3 | Update `app/render.rs` call sites: extract `DragItemId` from `DragContext` per surface | `heca/src/app/render.rs` |
| 2.5.4 | Update `mouse/render.rs` to use `InteractiveMoveState` + `DragContext` instead of `DragState` + `MouseState` sidebar fields | `heca/src/mouse/render.rs` |
| 2.5.5 | Update sidebar drag ghost rendering to use `DragContext.active_surface` + `SurfaceDragState.ghost_label` | `heca/src/app/render.rs` |
| 2.5.6 | Run `cargo test --workspace`, verify all pass | workspace |
| 2.5.7 | Visual test: drag highlights (hover, source, ghost label) render correctly on left sidebar |  |
| 2.5.8 | Visual test: swap-mode highlight (full pane overlay) renders correctly |  |
| 2.5.9 | Visual test: interactive move insert hint renders correctly |  |
| 2.5.10 | Commit: `refactor: per-surface drag rendering via DragContext` | |

### Phase 2.6: (Future) Add right sidebar DnD surface

Not in this plan's scope, but documented for completeness. When the right sidebar
gets its own tree model:

| Task | What | Files |
|------|------|-------|
| 2.6.1 | Add `DragSurfaceId::RightSidebar` variant | `heca-grid-ui/src/drag/item.rs` |
| 2.6.2 | Create `heca/src/mouse/surface_right.rs` — hit test, ghost label, can_accept, accept_drop | new file |
| 2.6.3 | Register right sidebar drag state in `DragContext` init | `heca/src/app/startup.rs` |
| 2.6.4 | Add right sidebar cases to `target.rs` dispatch | `heca/src/mouse/target.rs` |
| 2.6.5 | Add right sidebar rendering in `app/render.rs` | `heca/src/app/render.rs` |

---

## Execution Order

```
Track 1 (refactoring):
  1.1 → 1.2 → 1.3 → 1.4 → 1.5 → 1.6

Track 2 (DnD system):
  After 1.5 and 1.6 are done (functions are decomposed):
  2.1 → 2.2 → 2.3 → 2.4 → 2.5
```

Track 1 phases 1.5 and 1.6 should be completed before Track 2 because they
decompose the long functions that will be restructured during the DnD refactor.
Phases 2.1–2.5 depend on each other sequentially.

## Validation Checklist (after every phase)

- [ ] `cargo check -p heca-grid-ui` passes (if touched)
- [ ] `cargo check -p heca` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets --all-features` clean
- [ ] Manual test: left sidebar drag/drop works
- [ ] Manual test: content-area interactive move works
- [ ] Manual test: swap mode (Shift+drag) works
- [ ] Manual test: edge scroll during drag works
- [ ] Manual test: cancel drag (release without threshold) works