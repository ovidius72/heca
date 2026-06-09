# DnD Refactoring Plan

> Status: **Planning**
> Created: 2026-06-09
> Branch: main (target: heca-grid-ui branch for framework, main for integration)

## Goal

Extract the drag-and-drop system from hardcoded left-sidebar geometry into a
generic, surface-agnostic framework. The framework lives in `heca-grid-ui/`
(new `drag/` module, no changes to existing files). The left-sidebar-specific
dispatch stays in `heca/src/mouse/`. Future surfaces (right sidebar, inspector)
add one file each.

## Design Principles

1. **Surface-agnostic**: The DnD engine never branches on "left sidebar" or
   "right sidebar". It routes through `DragSurfaceId` (enum dispatch).
2. **Per-surface state**: Each surface owns its `SurfaceDragState`. No shared
   `drag_hover_sidebar_fi` — left and right sidebar have independent hover/source.
3. **One active drag**: Only one drag at a time (single mouse). The `DragContext`
   coordinator tracks which surface is active.
4. **GPU-free framework**: Everything in `heca-grid-ui/src/drag/` depends only on
   `heca-core` geometry types and the existing `heca-grid-ui` primitives. No `wgpu`,
   no `winit`, no `AppState`.
5. **App-side dispatch**: The actual mutation logic (pane swap, move, layout change)
   stays in `heca/src/mouse/surface_left.rs`. The framework only says "source X
   dropped on target Y" — the app decides what that means.
6. **No changes to existing heca-grid-ui files**: All new code goes in
   `heca-grid-ui/src/drag/` (new directory). Existing modules untouched.

## Architecture

```
heca-grid-ui/src/drag/          ← NEW: pure framework (GPU-free, headless, testable)
├── mod.rs                       ← pub re-exports
├── item.rs                      ← DragSurfaceId, DragItemId, DragItemKind, DragItem
├── state.rs                     ← SurfaceDragPhase, SurfaceDragState, DragLabel
├── context.rs                   ← DragContext (coordinator: active tracking, routing)
└── math.rs                      ← rubberband(), threshold constants

heca/src/mouse/                  ← REFACTORED: app-specific dispatch + content-area drag
├── mod.rs                       ← on_mouse_input, on_cursor_moved, on_modifiers_changed
├── target.rs                    ← NEW: enum dispatch free functions (surface_bounds, surface_item_at, etc.)
├── drag.rs                      ← REFACTORED: InteractiveMove drag only (content-area)
├── interactive.rs               ← NEW: extracted from drag.rs (starting/moving/cancel)
├── surface_left.rs               ← NEW: LeftSidebar surface dispatch (was sidebar_drop.rs + sidebar.rs)
├── hit_test.rs                   ← KEPT: content-area pane hit testing
├── render.rs                     ← REFACTORED: accept DragContext for hover/source highlighting
└── (drop.rs)                     ← REMOVED: dead code
└── (sidebar_drop.rs)             ← REMOVED: absorbed into surface_left.rs
└── (sidebar.rs)                  ← REMOVED: absorbed into surface_left.rs

heca/src/app_state.rs            ← REFACTORED: MouseState uses DragContext + per-surface state
```

## Phase Plan

### Phase A: Framework types in heca-grid-ui (non-breaking)

Create `heca-grid-ui/src/drag/` with pure types. No behavior changes to anything.
Just add types that heca will consume later.

**Files created:**
- `heca-grid-ui/src/drag/mod.rs`
- `heca-grid-ui/src/drag/item.rs`
- `heca-grid-ui/src/drag/state.rs`
- `heca-grid-ui/src/drag/context.rs`
- `heca-grid-ui/src/drag/math.rs`

**Types defined:**
```rust
// drag/item.rs
pub enum DragSurfaceId { LeftSidebar }  // open: add RightSidebar, Inspector, etc.
pub struct DragItemId(pub usize);
pub enum DragItemKind { Pane, Workspace, Column, FloatingPane }
pub struct DragItem { surface, id, kind, pane_id }

// drag/state.rs
pub enum SurfaceDragPhase { Idle, Starting { ... }, Dragging { ... } }
pub struct SurfaceDragState { phase, hover_item, source_item, ghost_label }
pub struct DragLabel { text, x, y, width, height }

// drag/context.rs
pub struct DragContext { active_surface, surfaces: HashMap<DragSurfaceId, SurfaceDragState> }

// drag/math.rs
pub fn rubberband(x: f32) -> f32    // moved from heca/src/mouse.rs
pub const DEFAULT_DRAG_THRESHOLD_SQ: f32 = 100.0;
```

**Validation:** `cargo check -p heca-grid-ui` passes. No changes to existing files.

### Phase B: Heca consumes framework types (refactor only)

Add `heca-grid-ui` as a dependency of `heca`. Replace `DragState`,
`SidebarDragLabel`, `MouseState` fields with the framework types. All existing
behavior preserved — mechanical rename/refactor.

**Changes:**
- `heca/Cargo.toml`: add `heca-grid-ui = { path = "../heca-grid-ui" }`
- `heca/src/app_state.rs`:
  - Remove `DragState` enum (replaced by `SurfaceDragPhase`)
  - Remove `SidebarDragLabel` (replaced by `DragLabel`)
  - `MouseState` gets `drag_ctx: DragContext` + `interactive_move: Option<InteractiveMoveState>`
  - Remove `drag_hover_sidebar_fi`, `sidebar_drag_source_fi`, `sidebar_drag_label`
- `heca/src/mouse/drag.rs`: use `SurfaceDragPhase` for sidebar drag, keep
  `InteractiveMove` as separate state
- `heca/src/mouse.rs`: route through `drag_ctx` instead of `drag_state`

**Validation:** All existing tests pass. `cargo clippy --workspace` clean.
Behavior identical to before.

### Phase C: Extract left-sidebar dispatch (behavior preserved)

Move sidebar-specific logic into `surface_left.rs`. The `mouse/target.rs`
enum dispatch calls into it.

**Files created:**
- `heca/src/mouse/target.rs` — enum dispatch free functions
- `heca/src/mouse/surface_left.rs` — left sidebar hit test, ghost label, can_accept, accept_drop, click_action

**Files removed:**
- `heca/src/mouse/sidebar_drop.rs` — absorbed into surface_left.rs
- `heca/src/mouse/sidebar.rs` — absorbed into surface_left.rs
- `heca/src/mouse/drop.rs` — dead code, removed

**Files modified:**
- `heca/src/mouse/drag.rs` — sidebar drag transitions routed through DragContext
- `heca/src/mouse.rs` — mouse input routed through target.rs dispatch

**Validation:** `cargo test` passes. Manual test: left sidebar drag/drop, content
interactive move, swap, edge scroll — all unchanged.

### Phase D: Split content-area drag into interactive.rs

Extract `InteractiveMove` logic from `drag.rs` into `interactive.rs`.

**Files created:**
- `heca/src/mouse/interactive.rs` — start_interactive_move, on_interactive_move_cursor, cancel_interactive_move, etc.

**Files modified:**
- `heca/src/mouse/drag.rs` — slimmed to just surface-drag routing + context updates

**Validation:** `cargo test` passes. Behavior identical.

### Phase E: Render integration

Update `render.rs` and `app/render.rs` to use `DragContext` for drag
highlighting instead of hardcoded sidebar fields.

**Changes:**
- `heca/src/mouse/render.rs` — accept `&DragContext` for hover/source highlights
- `heca/src/app/render.rs` — iterate surfaces for drag overlay rendering
- `heca/src/sidebar/render.rs` — pass `hover_item: Option<DragItemId>` and
  `source_item: Option<DragItemId>` instead of `drag_hover_fi: Option<usize>`
  and `drag_source_fi: Option<usize>`

**Validation:** Visual test: drag highlights render correctly on left sidebar.

### Phase F: Add right sidebar surface (future, not in this plan)

When the right sidebar gets its own tree model and content, add:
- `DragSurfaceId::RightSidebar`
- `heca/src/mouse/surface_right.rs`
- Register right sidebar drag state in `DragContext`

This is trivial because all the framework is in place.

## Per-phase checklist

Each phase:
1. `cargo check -p heca-grid-ui` passes (if touching that crate)
2. `cargo check -p heca` passes
3. `cargo clippy --workspace --all-targets --all-features` clean
4. Manual test: drag from left sidebar, drop on content, swap, edge scroll
5. Manual test: interactive move (Meta+drag), cancel with Esc
6. Commit with descriptive message

## Key constraints

- **Never modify existing heca-grid-ui files** — only add new ones in `drag/`
- **Never change existing behavior** during refactoring phases B–E
- **Enum dispatch, not trait objects** — closed set of surfaces, compiler-checked
- **Per-surface state, not global** — each surface has its own `SurfaceDragState`
- **InteractiveMove stays separate** — content-area drag is a different workflow
  from surface drag and doesn't belong in the DragContext