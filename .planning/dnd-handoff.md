# DnD System Handoff

> Created: 2026-06-09
> Session: Track 1 refactoring complete. Track 2 (DnD system) not yet started.
> Branch: `feature/gpt-refactoring` (PR #34 merged)

---

## 1. What Was Done (Track 1)

Six refactoring commits landed on `feature/gpt-refactoring`:

| # | Commit | What |
|---|--------|------|
| 1 | `0f8732b` | Delete `mouse/drop.rs` (142 lines dead code), clarify `InputMode::Chord` allow attribute |
| 2 | `dfec127` | Add descriptive messages to 4 `unreachable!()` calls |
| 3 | `9caa0e9` | Replace `(f32, f32, f32, f32)` tuples with `Rectangle` in `render_detached_pane` and `render_insert_hint` |
| 4 | `52d16f6` | Extract `DEFAULT_TAB_BAR_HEIGHT`/`DEFAULT_STATUS_BAR_HEIGHT` constants into `chrome.rs` |
| 5 | `a9af698` | Split `on_cursor_moved()` from 163 lines into 4 named helpers |
| 6 | `3a54281` | Extract mouse release handlers into `mouse/release.rs` |
| 7 | `7324096` | Trailing newline fix, improve `on_cursor_moved` doc comment |

All pass `cargo check`, `cargo clippy --workspace`, `cargo test --workspace`.

---

## 2. Current State of the DnD System

### 2.1 Files and Their Responsibilities

```
heca/src/mouse/
├── mod.rs              ← Event routing (on_mouse_input, on_cursor_moved, on_modifiers_changed, process_edge_scroll)
├── drag.rs             ← Drag state transitions (4 helpers + public API)
├── release.rs          ← Release handlers (handle_interactive_move_release, handle_sidebar_drag_release, handle_sidebar_drag_starting_release)
├── hit_test.rs         ← Content-area pane hit testing + sidebar pane hit testing
├── render.rs           ← GPU drag visuals (detached pane, insert hint, swap target highlight)
├── sidebar.rs          ← Sidebar click routing (click → WmAction)
├── sidebar_drop.rs     ← Sidebar drag drop logic (drag_drop, handle_drop)
└── tests.rs           ← Chrome config + rubberband tests

heca/src/mouse.rs       ← Module root (re-exports, helpers, mouse state setup)

heca/src/app_state.rs   ← DragState enum, MouseState struct, SidebarDragLabel, DetachedPane

heca/src/sidebar/
├── mod.rs              ← Re-exports (SidebarTree, SidebarItem, etc.)
├── model.rs            ← SidebarTree, SidebarItem, SidebarWsEntry, SidebarColEntry, SidebarPaneEntry
├── hit_test.rs         ← sidebar_hit_test(), sidebar_button_hit_test()
├── render.rs           ← render_sidebar_expanded(), render_sidebar_collapsed() + drag highlight rendering
└── tests.rs            ← SidebarTree tests

heca/src/app/
├── render.rs           ← Main render loop, sidebar rendering with drag highlights, drag ghost label
├── mutations.rs        ← after_layout_change(), after_focus_change()
└── pane_ops.rs         ← insert_pane_at_position(), remove_pane_by_id(), swap helpers

heca/src/chrome.rs      ← ChromeConfig, DEFAULT_TAB_BAR_HEIGHT, DEFAULT_STATUS_BAR_HEIGHT
```

### 2.2 DragState Machine (Current)

```
DragState::None
    │
    ├─ Meta+click on content pane ─→ InteractiveMoveStarting { pane_id, original_ws, start_mouse, threshold_sq, swap }
    │                                      │
    │                                      ├─ threshold exceeded ─→ InteractiveMove { _pane_id, _original_ws, offset, swap }
    │                                      │                          │
    │                                      │                          ├─ release (swap) ─→ swap with target, or sidebar drop, or move to insert hint, or cancel
    │                                      │                          ├─ release (move) ─→ sidebar drop, or move to insert hint, or cancel
    │                                      │                          └─ cancel ─→ None
    │                                      │
    │                                      └─ release before threshold ─→ None (cancel)
    │
    └─ Click on sidebar pane item ─→ SidebarDragStarting { pane_id, original_ws, start_mouse, threshold_sq, swap, click_action }
                                         │
                                         ├─ threshold exceeded ─→ SidebarDrag { pane_id, original_ws, swap }
                                         │                          │
                                         │                          ├─ release on sidebar ─→ drag_drop (move or swap)
                                         │                          └─ release on content ─→ drag_drop (move or add pane)
                                         │
                                         └─ release before threshold ─→ execute click_action (e.g. FocusPane)
```

### 2.3 MouseState Fields (Current)

```rust
pub struct MouseState {
    pub pos: (f32, f32),
    pub drag_state: DragState,
    pub detached_pane: Option<DetachedPane>,
    pub insert_hint: Option<PaneInsertTarget>,
    pub last_edge_scroll_time: Option<Instant>,
    pub drag_hover_sidebar_fi: Option<usize>,      // ← LEFT SIDEBAR ONLY
    pub sidebar_drag_source_fi: Option<usize>,      // ← LEFT SIDEBAR ONLY
    pub sidebar_drag_label: Option<SidebarDragLabel>, // ← LEFT SIDEBAR ONLY
    pub sidebar_hovered_btn_idx: Option<usize>,
}
```

### 2.4 Data Flow During a Drag

**Sidebar drag (left sidebar → content area drop):**

```
1. Press on sidebar pane item
   → mouse::on_mouse_input() Press arm
   → sidebar_pane_hit_test() identifies pane_id
   → DragState::SidebarDragStarting { pane_id, original_ws, start_mouse, threshold_sq, swap, click_action }

2. Cursor moves past threshold
   → drag::handle_sidebar_drag_starting()
   → DragState::SidebarDrag { pane_id, original_ws, swap }
   → Sets sidebar_drag_source_fi, sidebar_drag_label

3. Cursor moves over sidebar
   → drag::update_sidebar_drag_hover()
   → Sets mouse.drag_hover_sidebar_fi (flat index into sidebar tree)

4. Release on sidebar item
   → release::handle_sidebar_drag_release()
   → sidebar_drop::drag_drop()
   → Removes pane from origin, inserts at target (workspace/column/after pane)
   → DragState::None
```

**Interactive move (content area drag):**

```
1. Meta+click on content pane
   → drag::start_interactive_move()
   → DragState::InteractiveMoveStarting { pane_id, original_ws, start_mouse, threshold_sq, swap }

2. Cursor moves past threshold
   → drag::handle_interactive_move_starting()
   → transition_to_moving()
   → DragState::InteractiveMove { _pane_id, _original_ws, offset, swap }
   → Updates insert_hint, interactive_move_offset

3. Release
   → release::handle_interactive_move_release()
   → Swap: swap with target pane (or move to insert hint)
   → Move: remove from origin, re-insert at insert hint
   → DragState::None
```

---

## 3. Problems with the Current System

### 3.1 Hardcoded Left Sidebar

Seven locations assume the left sidebar is the only drag surface:

| Location | What's hardcoded |
|----------|-----------------|
| `drag.rs::handle_sidebar_drag_starting()` | `sidebar.left_visible`, `chrome.left_sidebar_width` |
| `drag.rs::update_sidebar_drag_hover()` | `sidebar.left_visible`, `chrome.left_sidebar_width` |
| `sidebar_drop.rs::drag_drop()` | `sidebar.left_visible`, `chrome.left_sidebar_width` |
| `sidebar_drop.rs::handle_drop()` | `sidebar.left_visible`, `chrome.left_sidebar_width` |
| `sidebar.rs::click()` | Left sidebar geometry only |
| `hit_test.rs::sidebar_pane_hit_test()` | Left sidebar geometry only |
| `mouse.rs::on_mouse_input()` Press arm | `sidebar_pane_hit_test()` → left sidebar only |

### 3.2 Single Global State

`MouseState` has ONE set of sidebar drag fields:

- `drag_hover_sidebar_fi: Option<usize>` — flat index into left sidebar tree
- `sidebar_drag_source_fi: Option<usize>` — flat index into left sidebar tree
- `sidebar_drag_label: Option<SidebarDragLabel>` — ghost label for left sidebar drag

Adding a right sidebar would require duplicating all three fields, and branching every drag path on "is this left or right?"

### 3.3 SidebarDrop Knows Layout Semantics

`sidebar_drop.rs` contains 418 lines of deeply nested match arms that:
1. Determine what sidebar item was dropped on (Workspace, Column, Pane, FloatingPane)
2. Remove the pane from its original position
3. Insert it at the target position using `insert_pane_at_position()`
4. Handle swap semantics

This logic is correct but tightly coupled to the left sidebar's `SidebarTree` model. A right sidebar with a different tree model would need a completely separate file with duplicated logic.

### 3.4 DragState Variants Are Sidebar-Specific

`SidebarDragStarting` and `SidebarDrag` carry `original_ws` (workspace index) which is a left-sidebar-tree concept. A right sidebar with different content wouldn't have "workspace" items.

---

## 4. Proposed Architecture (Track 2)

### 4.1 Core Design Decisions

**Decision 1: Per-surface state, not global**

Each drag surface (left sidebar, right sidebar, content area) owns its own `SurfaceDragState`:

```rust
pub struct SurfaceDragState {
    pub phase: SurfaceDragPhase,
    pub hover_item: Option<DragItemId>,
    pub source_item: Option<DragItemId>,
    pub ghost_label: Option<DragLabel>,
}
```

Left sidebar and right sidebar have independent hover/source highlighting. Only one surface is actively dragging at a time (single mouse).

**Decision 2: Enum dispatch, not trait objects**

`DragSurfaceId` is a closed enum:

```rust
pub enum DragSurfaceId {
    LeftSidebar,
    // RightSidebar,  — add when needed
    // Inspector,     — add when needed
}
```

Free functions dispatch by enum match:

```rust
pub fn surface_item_at(id: DragSurfaceId, state: &AppState, pos: (f32, f32)) -> Option<DragItem> {
    match id {
        DragSurfaceId::LeftSidebar => surfaces::left_sidebar::item_at(state, pos),
    }
}
```

Why not traits? Three reasons:
1. **Borrow conflict**: `Box<dyn DragSurface>` + `&mut AppState` = self-referential borrow. The surface needs `&mut AppState` to mutate session state, but `AppState` owns the surfaces. Rust won't allow both borrows simultaneously.
2. **Closed set**: The number of surfaces is known at compile time (left sidebar, right sidebar, inspector). The compiler should enforce exhaustiveness.
3. **Zero cost**: No vtable indirection, fully inlined.

**Decision 3: Framework in heca-grid-ui, dispatch in heca/src**

```
heca-grid-ui/src/drag/          ← Pure types and state machine (GPU-free, headless, testable)
├── mod.rs                        ← pub re-exports
├── item.rs                       ← DragSurfaceId, DragItemId, DragItemKind, DragItem
├── state.rs                      ← SurfaceDragPhase, SurfaceDragState, DragLabel
├── context.rs                     ← DragContext (active_surface, per-surface state map)
└── math.rs                        ← rubberband(), threshold constants

heca/src/mouse/                   ← App-specific dispatch + content-area drag
├── mod.rs                         ← Event routing (on_mouse_input, on_cursor_moved)
├── drag.rs                        ← Surface-drag routing + DragContext updates
├── interactive.rs                  ← InteractiveMove (content-area drag, stays here)
├── target.rs                      ← Enum dispatch: surface_bounds, surface_item_at, surface_can_accept, etc.
├── surface_left.rs                ← Left sidebar: item_at, ghost_label, can_accept, accept_drop, click_action
├── hit_test.rs                    ← Content-area pane hit testing
├── render.rs                      ← GPU drag visuals (detached pane, insert hint)
└── (sidebar_drop.rs)             ← REMOVED (absorbed into surface_left.rs)
    (sidebar.rs)                   ← REMOVED (absorbed into surface_left.rs)
```

**Decision 4: InteractiveMove stays separate**

Content-area drag (`InteractiveMove`) is fundamentally different from surface drag:
- It has rubberband threshold, swap mode, in-layout offset tracking, detached pane rendering
- It uses `insert_hint` and `DetachedPane` which are content-area concepts
- It does NOT need a per-surface state

So `InteractiveMove` stays in `AppState` directly, not inside `SurfaceDragState`.

### 4.2 Types (in heca-grid-ui)

```rust
// drag/item.rs

/// Opaque identifier for a drag surface (closed set).
/// Add new variants as new surfaces are created.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DragSurfaceId {
    LeftSidebar,
}

/// Opaque item identifier within a surface.
/// Each surface interprets this internally (flat index, grid coordinate, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DragItemId(pub usize);

/// What kind of item is being dragged / dropped onto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragItemKind {
    Pane,
    Workspace,
    Column,
    FloatingPane,
}

/// An item on a drag surface.
#[derive(Clone, Debug)]
pub struct DragItem {
    pub surface: DragSurfaceId,
    pub id: DragItemId,
    pub kind: DragItemKind,
    pub pane_id: Option<u64>,
}
```

```rust
// drag/state.rs

/// Phase of a surface-local drag.
#[derive(Clone, Debug)]
pub enum SurfaceDragPhase {
    /// Not dragging.
    Idle,
    /// Threshold phase: mouse pressed, waiting to see if it's a drag or click.
    Starting {
        item: DragItem,
        start_pos: (f32, f32),
        threshold_sq: f32,
        swap: bool,
        click_action: Option<Box<WmAction>>,
    },
    /// Active drag: threshold exceeded, following cursor.
    Dragging {
        item: DragItem,
        swap: bool,
    },
}

/// Per-surface drag state. Each surface owns one.
#[derive(Clone, Debug)]
pub struct SurfaceDragState {
    pub phase: SurfaceDragPhase,
    pub hover_item: Option<DragItemId>,
    pub source_item: Option<DragItemId>,
    pub ghost_label: Option<DragLabel>,
}

impl Default for SurfaceDragState {
    fn default() -> Self {
        Self {
            phase: SurfaceDragPhase::Idle,
            hover_item: None,
            source_item: None,
            ghost_label: None,
        }
    }
}
```

```rust
// drag/context.rs

use std::collections::HashMap;

/// Top-level drag coordinator. Routes events to the active surface.
/// Owns no layout state — just tracks which surface is dragging.
#[derive(Clone, Debug)]
pub struct DragContext {
    /// Which surface is currently being dragged from, if any.
    pub active_surface: Option<DragSurfaceId>,
    /// Per-surface drag state.
    pub surfaces: HashMap<DragSurfaceId, SurfaceDragState>,
}

impl Default for DragContext {
    fn default() -> Self {
        let mut surfaces = HashMap::new();
        surfaces.insert(DragSurfaceId::LeftSidebar, SurfaceDragState::default());
        Self {
            active_surface: None,
            surfaces,
        }
    }
}
```

```rust
// drag/math.rs

/// NIRI rubberband formula: `(1.0 - (1.0 / (x * c / d + 1.0))) * d`
/// with `c = 1.0`, `d = 0.5`.
pub fn rubberband(x: f32) -> f32 {
    let c = 1.0;
    let d = 0.5;
    (1.0 - (1.0 / (x * c / d + 1.0))) * d
}

/// Default drag threshold in squared pixels (10px movement to start drag).
pub const DEFAULT_DRAG_THRESHOLD_SQ: f32 = 100.0;
```

### 4.3 AppState Changes

```rust
// app_state.rs — current MouseState becomes:

pub struct MouseState {
    pub pos: (f32, f32),
    // Per-surface drag state (replaces drag_state + sidebar fields)
    pub drag_ctx: DragContext,
    // Content-area interactive move (separate from surface drag)
    pub interactive_move: Option<InteractiveMoveState>,
    // Detached pane rendering (kept here, content-area only)
    pub detached_pane: Option<DetachedPane>,
    // Computed drop target during content-area drag
    pub insert_hint: Option<PaneInsertTarget>,
    // Edge scroll
    pub last_edge_scroll_time: Option<std::time::Instant>,
    // Button hover in sidebar
    pub sidebar_hovered_btn_idx: Option<usize>,
    // Mouse interactions enabled
    pub mouse_enabled: bool,
}

/// InteractiveMove state (content-area drag, NOT in SurfaceDragState)
#[derive(Clone, Debug)]
pub struct InteractiveMoveState {
    pub pane_id: u64,
    pub original_ws: usize,
    pub start_mouse: (f32, f32),
    pub offset: (f32, f32),
    pub swap: bool,
}
```

### 4.4 How Cross-Surface Drops Work

When a drag starts on surface A and the cursor moves over surface B:

```
on_cursor_moved:
    if let Some(active_id) = drag_ctx.active_surface:
        // Update ghost label position (surface A's item follows cursor)
        // Update active surface state (InteractiveMove offset, etc.)
    
    // For EVERY surface (including inactive ones):
    for surface_id in [LeftSidebar, RightSidebar, ...]:
        if surface_contains_cursor(surface_id, pos):
            let item = surface_item_at(surface_id, state, pos)
            if surface_can_accept(surface_id, state, source_item, item, swap):
                surface.hover_item = Some(item)  // "I can receive here"
        else:
            surface.hover_item = None

on_mouse_release:
    // Find which surface the cursor is over
    for surface_id in [LeftSidebar, RightSidebar, ...]:
        if surface_contains_cursor(surface_id, pos):
            if let Some(target_item) = surface_item_at(surface_id, state, pos):
                if surface_can_accept(surface_id, state, source_item, target_item, swap):
                    surface_accept_drop(surface_id, state, source_item, target_item, swap)
                    return
    // No target found — cancel drag
    cancel_drag(state)
```

Both sidebars show drop targets simultaneously. On release, only the surface the cursor is over accepts the drop.

### 4.5 Rendering Integration

Currently `sidebar/render.rs` takes `drag_hover_fi: Option<usize>` and `drag_source_fi: Option<usize>`. After the refactor:

```rust
// sidebar/render.rs — before:
pub fn render_sidebar_expanded(
    tree, x, y, width, height, is_sidebar_nav, accent, foreground, cursor_bg,
    visited_color, candidates, focused_pane, text_renderer, primitive_renderer,
    drag_hover_fi: Option<usize>,     ← flat index into left sidebar
    drag_source_fi: Option<usize>,     ← flat index into left sidebar
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
    ...
)

// sidebar/render.rs — after:
pub fn render_sidebar_expanded(
    tree, x, y, width, height, is_sidebar_nav, accent, foreground, cursor_bg,
    visited_color, candidates, focused_pane, text_renderer, primitive_renderer,
    drag_hover: Option<DragItemId>,     ← opaque item ID
    drag_source: Option<DragItemId>,     ← opaque item ID
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
    ...
)
```

The render function compares `DragItemId` values instead of raw `usize` flat indices. The left sidebar surface maps its flat index to `DragItemId(fi)` internally.

### 4.6 Implementation Phases (Track 2)

Full step-by-step plan is in `.planning/refactoring-and-dnd-plan.md`.

**Phase order (each is a single commit, compiles and tests independently):**

1. **Phase 2.1**: Create `heca-grid-ui/src/drag/` with type definitions. No behavior changes.
2. **Phase 2.2**: Add `heca-grid-ui` dependency to `heca`. Replace `DragState`, `SidebarDragLabel`, `MouseState` fields with framework types. Mechanical refactor, behavior identical.
3. **Phase 2.3**: Create `mouse/target.rs` (enum dispatch) and `mouse/surface_left.rs` (left sidebar handler). Delete `sidebar_drop.rs` and `sidebar.rs`. Route through target.rs.
4. **Phase 2.4**: Extract `InteractiveMove` into `mouse/interactive.rs`. Slim down `drag.rs`.
5. **Phase 2.5**: Update render calls to use `DragContext` + `DragItemId` instead of flat indices.

**Future (not in this plan):**
- **Phase 2.6**: Add `DragSurfaceId::RightSidebar` and `mouse/surface_right.rs` when the right sidebar gets its tree model.

---

## 5. Key Files to Read Before Starting

| File | Why |
|------|-----|
| `heca/src/mouse/drag.rs` | Current drag state machine, 4 helpers |
| `heca/src/mouse/release.rs` | Release handlers for all 3 drag types |
| `heca/src/mouse/sidebar_drop.rs` | Left sidebar drop logic (will move to surface_left.rs) |
| `heca/src/mouse/sidebar.rs` | Left sidebar click logic (will move to surface_left.rs) |
| `heca/src/mouse.rs` | Event routing, chrome_config, edge scrolling |
| `heca/src/app_state.rs` | DragState, MouseState, SidebarDragLabel |
| `heca/src/sidebar/model.rs` | SidebarTree, SidebarItem, flat_items |
| `heca/src/sidebar/hit_test.rs` | sidebar_hit_test, sidebar_button_hit_test |
| `heca/src/sidebar/render.rs` | drag_hover_fi, drag_source_fi rendering |
| `heca/src/app/render.rs` | Lines 340–440 (sidebar drag ghost rendering) |
| `.planning/refactoring-and-dnd-plan.md` | Full task-by-task plan |
| `.planning/dnd-refactor-plan.md` | Original DnD architecture document |

---

## 6. Important Constraints

- **Never modify existing heca-grid-ui files** — only add new ones in `drag/`
- **Never change existing behavior** during refactoring phases 2.1–2.5
- **Enum dispatch, not trait objects** — closed set of surfaces, compiler-checked
- **Per-surface state** — each surface has its own `SurfaceDragState`
- **InteractiveMove stays separate** — content-area drag is a different workflow from surface drag
- **Use `Rectangle` from `heca_core::layout`** — not `(f32, f32, f32, f32)` tuples
- **Use `DEFAULT_TAB_BAR_HEIGHT` / `DEFAULT_STATUS_BAR_HEIGHT`** — not hardcoded `32.0` / `24.0`
- **Every WM action goes through `registry.execute()`** — no direct function calls in event handlers
- **Run `cargo clippy --workspace --all-targets --all-features` before committing** — must be clean