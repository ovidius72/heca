# Mouse Interactive Move — Detailed Implementation Plan

**Status**: Reviewed and corrected by `niri-skills` agent against actual NIRI source code.

- **Click-to-focus bug fixed** — `sync_focus()` was overwriting the clicked pane's focus because `active_column_idx` / `active_pane_idx` were never updated. Replaced with `focus_pane_by_id()` which updates both `focused_pane` and session state.
- **Phase order corrected** — Interactive move (Phase 2) must come before edge scroll (Phase 3), since edge scroll is useless without something to drag.
- **Code-level corrections applied**:
  - Use exact 3-line NIRI rubberband formula
  - Add `interactive_move_offset` to `Pane` (don't reuse `move_offset`)
  - `InsertPosition` already exists in `types.rs` (don't duplicate in `scrolling.rs`)
  - Do NOT call `update_all_column_widths()` on pane removal (NIRI Principle 1)

---

## Architecture Reference: NIRI's Interactive Move System

Analyzed from `niri/src/layout/mod.rs` lines 353–4350, `scrolling.rs` lines 801–900, `monitor.rs` lines 1587–1660.

### Core Data Model

```rust
// mod.rs:404
enum InteractiveMoveState<W: LayoutElement> {
    Starting {
        window_id: W::Id,
        pointer_delta: Point<f64, Logical>,
        pointer_ratio_within_window: (f64, f64),
    },
    Moving(InteractiveMoveData<W>),
}

// mod.rs:421
struct InteractiveMoveData<W: LayoutElement> {
    tile: Tile<W>,           // the detached window
    output: Output,
    pointer_pos_within_output: Point<f64, Logical>,
    width: ColumnWidth,
    is_full_width: bool,
    is_floating: bool,
    pointer_ratio_within_window: (f64, f64),
}
```

### Phase 1: Starting (Rubberband)

`interactive_move_begin()` (mod.rs:3793):
1. Store `window_id`, `pointer_delta = (0, 0)`, `pointer_ratio_within_window`.
2. Call `dnd_scroll_gesture_begin()` on all workspaces to lock view for edge scrolling.

`interactive_move_update()` — Starting branch (mod.rs:3854):
1. `pointer_delta += delta` (cumulative movement).
2. Compute `sq_dist = cx*cx + cy*cy`.
3. Apply **rubberband**: `factor = RubberBand { stiffness: 1.0, limit: 0.5 }.band(sq_dist / threshold)`.
4. `tile.interactive_move_offset = pointer_delta * factor` — pane follows pointer but with resistance.
5. If `sq_dist < threshold` → stay in Starting.
6. If `sq_dist >= threshold` → transition to **Moving**:
   - Unset fullscreen/maximized.
   - `remove_window()` to detach from layout.
   - `tile.stop_move_animations()`.
   - `tile.animate_alpha(1.0 → INTERACTIVE_MOVE_ALPHA, movement_anim)`.
   - `tile.animate_move_from(tile_pos - new_tile_pos)` — smooth transition from in-layout pos to pointer.

### Phase 2: Moving (Detached)

`interactive_move_update()` — Moving branch (mod.rs:~3950):
1. Update `pointer_pos_within_output`.
2. If moved to different output → update config, scale, transform.
3. Re-insert on `interactive_move_end()`.

### Drop Target Computation

`scrolling.insert_position(pos)` (scrolling.rs:801):
```
x = pos.x + view_pos()   // transform to space coords
x += gaps / 2             // aim for center of gap
y += gaps / 2

1. If x < 0 → NewColumn(0)
2. Find closest column gap → (closest_col_idx, col_x)
3. Find column containing x → (col_idx, _)
4. If past last column → NewColumn(closest_col_idx)
5. Find closest tile gap in column → (closest_tile_idx, tile_y)
6. Compare |col_x - x| vs |tile_y - y|
   - vertical gap closer → NewColumn(closest_col_idx)
   - horizontal gap closer → InColumn(col_idx, closest_tile_idx)
```

### Edge Scroll (DnD)

`workspace.dnd_scroll_gesture_scroll(pos, speed)` (workspace.rs:~1880):
```
x = pos.x - working_area.loc.x
trigger_width = config.trigger_width.clamp(0, width / 2)

if x < trigger_width:
    delta = -(trigger_width - x)
elif width - x < trigger_width:
    delta = trigger_width - (width - x)
else:
    delta = 0

delta = (delta / trigger_width) * speed   // normalize to [0, 1]
scrolling.dnd_scroll_gesture_scroll(delta)
```

This feeds into `ViewOffset::Gesture` with `dnd_last_event_time` tracking (scrolling.rs:3034–3095).

### Insert Hint

`scrolling.insert_hint_area(position)` (scrolling.rs:2436):
- `NewColumn(0)` or `NewColumn(len)`: 300×(h - 2*gaps) rectangle beside edge column.
- `NewColumn(middle)`: same size, centered on gap.
- `InColumn(_, 0)` or `InColumn(_, len)`: full_width × 150px at top/bottom edge.
- `InColumn(_, middle)`: full_width × 300px centered on tile gap.

### End / Drop

`interactive_move_end()` (mod.rs:4081):
1. For Starting: cancel — `tile.animate_move_from(offset)` back to origin.
2. For Moving:
   - Compute `InsertPosition` from pointer.
   - Re-insert tile: `add_tile()` for `NewColumn`, `add_tile_to_column()` for `InColumn`.
   - `tile.animate_move_from(old_render_loc - new_render_loc)` — animate into slot.
   - `tile.animate_alpha(INTERACTIVE_MOVE_ALPHA → 1.0)` — restore opacity.
   - End DnD scroll gestures.

---

## heca Implementation: Phased Plan

### Phase 0: Focus Follows Mouse (Small)

**Goal**: Moving the cursor over a pane focuses it (no click needed).

**File**: `heca/src/main.rs` — `CursorMoved` handler

```rust
WindowEvent::CursorMoved { position, .. } => {
    state.mouse_pos = (...);
    state.needs_redraw = true;
    if !state.mouse_enabled { return; }

    // Focus follows mouse
    if state.focus_follows_mouse && state.drag_state == DragState::None {
        if let Some(pane_id) = hit_test_pane(state, state.mouse_pos) {
            if state.focused_pane != Some(pane_id) {
                focus_pane_by_id(state, pane_id);
            }
        }
    }
}
```

**Config** (`heca-config/src/lib.rs`):
```toml
[general]
focus_follows_mouse = true
```

---

### Phase 1: Click-to-Focus Fix (DONE)

**Bug**: Click handler set `focused_pane` then called `sync_focus()`, which overwrote it with the session's old active pane.

**Fix**: `focus_pane_by_id(state, pane_id.0)` instead of `state.focused_pane = Some(pane_id.0); sync_focus(state);`.

**Verification**: Click on any pane now properly updates `active_column_idx` / `active_pane_idx` and keeps focus.

---

### Phase 2: Meta+Click Interactive Move (Large)

> **Phase reordered per NIRI review:** Interactive move must come before edge scroll, since edge scroll is useless without something to drag.

#### 2.1 Data Model Expansion

### Phase 3: Edge Scroll During Drag (Medium)

> **Phase reordered per NIRI review:** Edge scroll is polish on top of interactive move. Build the drag-and-drop core first, then add viewport auto-scroll.

**Goal**: When dragging a pane near left/right viewport edges, auto-scroll the layout.

**Approach**: Direct `view_offset` manipulation for v1 (simpler than full Gesture system).

**File**: `heca/src/main.rs` — new function, called from `CursorMoved` during drag

```rust
fn dnd_edge_scroll(state: &mut AppState, mouse_x: f32, content_width: f32) {
    let trigger = 80.0;
    let speed = 15.0;  // px per frame at 60fps

    let delta = if mouse_x < trigger {
        -(trigger - mouse_x)
    } else if content_width - mouse_x < trigger {
        trigger - (content_width - mouse_x)
    } else {
        0.0
    };

    if delta != 0.0 {
        let normalized = delta / trigger; // [0, 1]
        let scroll = normalized * speed;
        if let Some(ws) = state.session.active_workspace_mut() {
            ws.scrolling.view_offset.offset(scroll as f64);
        }
    }
}
```

**Future**: Migrate to `ViewOffset::Gesture` system for momentum and snap behavior.

---

#### 3.1 Data Model Expansion

**`heca-core/src/layout/column.rs`** — Add `interactive_move_offset` to `Pane`:

```rust
pub struct Pane {
    // ... existing fields ...
    /// Offset applied during interactive move Starting phase (rubberband).
    /// Cleared on transition to Moving. Not used by entry/exit animations.
    pub interactive_move_offset: Point,
}

impl Pane {
    pub fn new(id: PaneId, title: String, width: ColumnWidth) -> Self {
        Self {
            // ... existing fields ...
            interactive_move_offset: Point::new(0.0, 0.0),
        }
    }
}
```

**`heca/src/app_state.rs`**:

```rust
pub enum DragState {
    None,
    // ... existing variants ...

    /// Interactive move: starting (rubberband, pane still in layout).
    InteractiveMoveStarting {
        pane_id: u64,
        start_mouse: (f32, f32),
        pointer_delta: (f32, f32),
        threshold_sq: f32,
    },

    /// Interactive move: moving (pane detached, follows pointer).
    InteractiveMove {
        pane_id: u64,
        offset: (f32, f32),      // pointer - pane top-left
        original_col: usize,
        original_pane: usize,
    },
}

/// A pane temporarily removed from the layout for interactive move.
pub struct DetachedPane {
    pub pane: heca_core::layout::column::Pane,
    pub size: heca_core::layout::types::Size,
    pub render_pos: heca_core::layout::types::Point,
    pub alpha: f32,
}

pub struct AppState {
    // ... existing fields ...
    pub detached_pane: Option<DetachedPane>,
    pub insert_hint: Option<heca_core::layout::types::InsertPosition>,
}
```

**`heca-core/src/layout/scrolling.rs`**:

```rust
// InsertPosition already defined in heca-core/src/layout/types.rs:
// pub enum InsertPosition {
//     NewColumn(usize),
//     InColumn(usize, usize),
//     Floating,
// }

impl ScrollingSpace {
    pub fn insert_position(&self, pos: Point) -> InsertPosition {
        // See NIRI reference above for full algorithm.
        // Simplified: find closest column gap vs closest tile gap.
    }
}
```

#### 2.2 Mouse Event Wiring

**`MouseInput` — Press**:
```rust
if button == MouseButton::Left && button_state == ElementState::Pressed {
    let meta_held = state.modifiers.state().super_key(); // or config modifier
    if meta_held {
        if let Some(pane_id) = hit_test_pane(state, mouse_pos) {
            state.drag_state = DragState::InteractiveMoveStarting {
                pane_id,
                start_mouse: mouse_pos,
                pointer_delta: (0.0, 0.0),
                threshold_sq: 64.0, // 8px
            };
        }
    } else {
        // Normal click → focus
        if let Some(pane_id) = hit_test_pane(state, mouse_pos) {
            focus_pane_by_id(state, pane_id);
        }
    }
}
```

**`MouseInput` — Release**:
```rust
if button == MouseButton::Left && button_state == ElementState::Released {
    match &state.drag_state {
        DragState::InteractiveMoveStarting { pane_id, .. } => {
            cancel_interactive_move(state, *pane_id);
        }
        DragState::InteractiveMove { pane_id, .. } => {
            if let Some(hint) = state.insert_hint.take() {
                drop_pane(state, *pane_id, hint);
            }
        }
        _ => {}
    }
    state.drag_state = DragState::None;
    state.detached_pane = None;
}
```

**`CursorMoved` — Drag handling**:
```rust
fn rubberband(x: f32) -> f32 {
    // NIRI's exact formula from src/rubber_band.rs:
    // RubberBand { stiffness: 1.0, limit: 0.5 }
    let c = 1.0;
    let d = 0.5;
    (1.0 - (1.0 / (x * c / d + 1.0))) * d
}

match &mut state.drag_state {
    DragState::InteractiveMoveStarting { pane_id, start_mouse, pointer_delta, threshold_sq } => {
        let dx = mouse_pos.0 - start_mouse.0;
        let dy = mouse_pos.1 - start_mouse.1;
        *pointer_delta = (dx, dy);

        let sq_dist = dx * dx + dy * dy;
        let factor = rubberband(sq_dist / threshold_sq);

        // Apply rubberband offset to pane's interactive_move_offset
        // (NOT move_offset, which is reserved for entry/exit animations)
        if let Some((ci, pi)) = find_pane_in_layout(state, *pane_id) {
            ws.scrolling.columns[ci].panes[pi].interactive_move_offset =
                Point::new(dx as f64 * factor, dy as f64 * factor);
        }

        if sq_dist > *threshold_sq {
            transition_to_moving(state, *pane_id, mouse_pos);
        }
    }

    DragState::InteractiveMove { pane_id, offset, .. } => {
        // Update detached pane position
        if let Some(detached) = &mut state.detached_pane {
            detached.render_pos = Point::new(
                (mouse_pos.0 - offset.0) as f64,
                (mouse_pos.1 - offset.1) as f64,
            );
        }

        // Compute insert hint
        if let Some(ws) = state.session.active_workspace() {
            let space_pos = Point::new(
                (mouse_pos.0 - pane_area.x) as f64 + ws.scrolling.view_pos(),
                (mouse_pos.1 - pane_area.y) as f64,
            );
            state.insert_hint = Some(ws.scrolling.insert_position(space_pos));
        }

        // Edge scroll (Phase 3)
        dnd_edge_scroll(state, mouse_pos.0, pane_area.w);
    }

    _ => {
        // Focus follows mouse (Phase 0)
        if state.focus_follows_mouse {
            // ...
        }
    }
}
```

#### 2.3 Transition to Moving

```rust
fn transition_to_moving(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    let ws = state.session.active_workspace_mut().unwrap();

    // Find and remove pane from layout
    let (col_idx, pane_idx) = find_pane_in_layout(state, pane_id).unwrap();
    let old_col_x = ws.scrolling.column_x(col_idx);
    let old_pane_y = ws.scrolling.pane_y_in_column(col_idx, pane_idx);

    let removed = ws.scrolling.remove_pane(col_idx, pane_idx).unwrap();
    let pane_size = compute_pane_size(&removed, col_idx); // from original column

    // Do NOT call update_all_column_widths() here.
    // NIRI Principle 1: removing a pane should not affect widths of other columns.
    // Only if the column becomes empty should it be removed entirely.
    // (If column still has panes, its width stays unchanged.)

    // Clear interactive_move_offset from the removed pane
    let mut detached_pane = removed;
    detached_pane.interactive_move_offset = Point::new(0.0, 0.0);

    // Store detached pane
    state.detached_pane = Some(DetachedPane {
        pane: detached_pane,
        size: pane_size,
        render_pos: Point::new(old_col_x + ws.scrolling.view_offset.current(), old_pane_y),
        alpha: 0.3,
    });

    state.drag_state = DragState::InteractiveMove {
        pane_id,
        offset: (mouse_pos.0 - old_col_x as f32, mouse_pos.1 - old_pane_y as f32),
        original_col: col_idx,
        original_pane: pane_idx,
    };
}
```

#### 2.4 Drop

```rust
fn drop_pane(state: &mut AppState, pane_id: u64, hint: InsertPosition) {
    let ws = state.session.active_workspace_mut().unwrap();
    let detached = state.detached_pane.take().unwrap();

    match hint {
        InsertPosition::NewColumn(col_idx) => {
            let mut col = Column::new(
                ColumnId(pane_id),
                detached.pane,
                ColumnWidth::Proportion(0.5),
            );
            ws.scrolling.add_column(Some(col_idx), col, true);
        }
        InsertPosition::InColumn(col_idx, pane_idx) => {
            ws.scrolling.add_pane_to_column(col_idx, Some(pane_idx), detached.pane, true);
        }
    }

    // Animate the dropped pane from its detached position to new layout position
    let new_col_x = ws.scrolling.column_x(/* find new col with pane_id */);
    let new_pane_y = ws.scrolling.pane_y_in_column(/* ... */);
    let new_pos = Point::new(new_col_x + ws.scrolling.view_offset.current(), new_pane_y);
    let delta = detached.render_pos - new_pos;

    // Find the pane in its new location and animate
    if let Some((ci, pi)) = find_pane_in_layout(state, pane_id) {
        ws.scrolling.columns[ci].panes[pi].animate_move_from(delta, AnimationConfig::default());
    }
}
```

#### 2.5 Rendering

In `render()`, after normal panes:

```rust
// 1. Render detached pane (if any)
if let Some(detached) = &state.detached_pane {
    let px = pane_area.x + detached.render_pos.x as f32;
    let py = pane_area.y + detached.render_pos.y as f32;
    let pw = detached.size.w as f32;
    let ph = detached.size.h as f32;

    render_backend_data_with_alpha(..., detached.alpha);
    primitive_renderer.draw_border(px, py, pw, ph, accent_color, border_width * 2.0);
}

// 2. Render insert hint (placeholder)
if let Some(hint) = state.insert_hint {
    let rect = compute_insert_hint_rect(state, hint, pane_area);
    primitive_renderer.draw_rect(rect.x, rect.y, rect.w, rect.h,
        [accent[0], accent[1], accent[2], 0.15]);
    primitive_renderer.draw_border(rect.x, rect.y, rect.w, rect.h,
        [accent[0], accent[1], accent[2], 0.6], 2.0);
}
```

#### 2.6 Config

```toml
[general]
focus_follows_mouse = true
interactive_move_modifier = "Super"  # "Alt", "Ctrl", "Shift", "None"
```

---

## Task List

### Phase 0: Focus Follows Mouse
- [ ] Add `focus_follows_mouse: bool` to `AppConfig` (default `true`)
- [ ] Wire `CursorMoved` to call `focus_pane_by_id()` when hovering over a new pane
- [ ] Debounce: only focus if mouse has been over pane for >50ms (optional)

### Phase 1: Click-to-Focus Fix
- [x] Replace `state.focused_pane = Some(id); sync_focus(state);` with `focus_pane_by_id(state, id)`
- [x] Verify floating pane click also works (add floating hit-test)

### Phase 2: Meta+Click Interactive Move (was Phase 3)
- [ ] Add `interactive_move_offset: Point` field to `Pane` (for rubberband phase)
- [ ] Implement `scrolling.insert_position()` (uses existing `InsertPosition` from `types.rs`)
- [ ] Expand `DragState` with `InteractiveMoveStarting` and `InteractiveMove`
- [ ] Add `DetachedPane` and `insert_hint` to `AppState`
- [ ] Wire `MouseInput` press: detect modifier, start Starting phase
- [ ] Wire `CursorMoved`: handle Starting (rubberband) and Moving (follow pointer)
- [ ] Wire `MouseInput` release: drop or cancel
- [ ] Implement `transition_to_moving()` — remove pane, do NOT call `update_all_column_widths()`
- [ ] Implement `drop_pane()` — re-insert at hint, animate into place
- [ ] Implement `cancel_interactive_move()` — animate back to origin
- [ ] Render detached pane with alpha
- [ ] Render insert hint rectangle
- [ ] Add config: `interactive_move_modifier`

### Phase 3: Edge Scroll During Drag (was Phase 2)
- [ ] Add `dnd_edge_scroll()` function
- [ ] Call from `CursorMoved` when `drag_state == InteractiveMove`
- [ ] Tune trigger width and speed values
- [ ] **Future**: Migrate to `ViewOffset::Gesture` system for momentum and snap behavior

---

---

## NIRI Skill Review — Answers

Reviewed by the project-level `niri-skills` agent against actual NIRI source code (`mod.rs`, `scrolling.rs`, `workspace.rs`, `monitor.rs`, `rubber_band.rs`).

### 1. Rubberband math — Custom, not standard; implement exactly

NIRI's `RubberBand` is **not** a standard easing function. The exact formula from `src/rubber_band.rs` is:

```rust
pub fn band(&self, x: f64) -> f64 {
    let c = self.stiffness; // 1.0
    let d = self.limit;     // 0.5
    (1. - (1. / (x * c / d + 1.))) * d
}
```

This produces a soft resistance curve that asymptotically approaches `limit` (0.5). For interactive move, `x = sq_dist / INTERACTIVE_MOVE_START_THRESHOLD` where the threshold is `256.0 * 256.0`.

**Applied**: The exact 3-line formula is now in the plan's `rubberband()` function.

### 2. DnD scroll gesture — Acceptable for v1, with documented limitations

NIRI integrates edge scroll deeply into `ViewOffset::Gesture` with `dnd_last_event_time`, `dnd_nonzero_start_time` debouncing, `SwipeTracker` momentum, and bounds clamping.

The plan's direct `view_offset.offset()` manipulation is acceptable for a **minimal v1**, but lacks:
- Momentum / inertial scrolling
- Snap-to-column on release
- Debouncing (may jitter near edge)
- Bounds clamping (can scroll past content)

**Applied**: Phase 3 now documents direct manipulation as a temporary approach, with a future migration to `ViewOffset::Gesture`.

### 3. Multi-workspace moves — Single-workspace is correct scope

NIRI's cross-workspace support requires:
- Workspace hit-testing during drag
- Workspace creation/destruction on drop
- 750ms hold-to-activate timers
- Output scale/config changes

NIRI has ~150 lines just for cross-output `interactive_move_end` handling.

**Applied**: Confirmed single-workspace scope for Phase 2. Cross-workspace deferred.

### 4. Insert hint rendering — No concerns with rect overlay

NIRI draws hints as first-class compositor render elements because it *is* a Wayland compositor and must integrate with layer shells, scaling, and damage tracking.

heca owns its GPU renderer entirely. A simple semi-transparent rectangle + border drawn via `PrimitiveRenderer` is architecturally correct and much simpler.

**Applied**: No changes needed — rect overlay approach confirmed correct.

### 5. Pointer ratio within window — Skip it for fixed-size panes

NIRI tracks `pointer_ratio_within_window` because Wayland windows resize asynchronously, moving between outputs changes fractional scale, and unfullscreen/unmaximize changes size. In `tile_render_location()`, NIRI computes:

```rust
let pointer_offset = (
    window_size.w * pointer_ratio_within_window.0,
    window_size.h * pointer_ratio_within_window.1,
);
pos = pointer_pos - pointer_offset;
```

For heca's current fixed-size panes (`FakeBackend`, `TerminalBackend`), pane sizes do not change during a move. We can skip `pointer_ratio_within_window` and simply offset the pane by `(mouse_pos - pane_top_left)`.

**Applied**: Plan uses simple `(mouse_pos - pane_top_left)` offset. `pointer_ratio_within_window` deferred.

### 6. Missing features — v1 vs deferred

| Should be in v1 | Deferred |
|-----------------|----------|
| Meta+click drag with threshold | DnD edge scroll |
| `InsertPosition` computation | Alpha fade during move |
| Insert hint overlay | `pointer_ratio_within_window` |
| Drop animation | Floating toggle during move |
| Cancel on release (under threshold) | Hold-to-activate (cross-workspace) |
| | Overview integration |

### 7. Order of implementation — Phases 2 and 3 swapped

**Original**: Phase 2 edge scroll, Phase 3 interactive move.  
**Revised**: Phase 2 interactive move, Phase 3 edge scroll.

**Rationale**: Edge scroll is completely useless without interactive move. You cannot test edge scroll until you have something to drag.

**Applied**: Task list and phase headers reordered accordingly.
