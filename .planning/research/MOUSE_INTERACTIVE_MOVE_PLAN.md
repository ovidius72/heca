# Mouse Interactive Move — Implementation Plan

Based on analysis of NIRI's `interactive_move` system (`src/layout/mod.rs`, `scrolling.rs`, `monitor.rs`) and heca's current architecture.

## Requirements

1. **Focus follows mouse** (configurable, default `true`)
   - On `CursorMoved`, if mouse is over a pane, focus moves to it (no click needed).
   - Debounce to avoid excessive focus switching when crossing gaps.

2. **Edge scroll during drag** (with animation)
   - When dragging a pane near left/right viewport edges, auto-scroll the layout.
   - Uses the existing `ViewOffset::Gesture` / `SwipeTracker` system.

3. **Meta+click grab & move** (configurable modifier)
   - Click with modifier (e.g. Super/Cmd) on a pane to start dragging.
   - Phase 1 — *Starting*: pane stays in layout, rubberbands with pointer (visual feedback).
   - Phase 2 — *Moving*: pane removed from layout, follows pointer, semi-transparent.
   - Visual placeholder (insert hint) shows drop target — new column or within column.
   - Release to drop — pane animates into new position.

---

## NIRI Architecture Reference

### InteractiveMoveState (enum)

```rust
enum InteractiveMoveState<W> {
    Starting {
        window_id: W::Id,
        pointer_delta: Point,     // cumulative movement from start
        pointer_ratio_within_window: (f64, f64), // for keeping pointer inside
    },
    Moving(InteractiveMoveData<W>),
}
```

**Phase 1 (Starting):**
- Window stays in layout.
- `tile.interactive_move_offset = pointer_delta * rubberband_factor`
- Factor is `RubberBand::band(sq_dist / threshold)` — stiffness 1.0, limit 0.5.
- If distance exceeds threshold → transition to Moving.

**Phase 2 (Moving):**
- Window removed from layout via `remove_window()`.
- `tile.interactive_move_offset` reset to 0.
- `tile.animate_alpha(1.0 → 0.3, movement_anim)` — semi-transparent.
- `tile.animate_move_from(tile_pos - new_tile_pos)` — smooth transition from in-layout pos to pointer.
- Window rendered at pointer position: `pointer_pos - pointer_offset_within_window`.

**End (Drop):**
- Compute `InsertPosition` from pointer position.
- Re-insert tile into layout at computed position.
- `tile.animate_move_from(old_render_loc - new_render_loc)` — animate into slot.
- `tile.animate_alpha(0.3 → 1.0)` — restore opacity.

### InsertPosition (enum)

```rust
enum InsertPosition {
    NewColumn(usize),           // insert before column N
    InColumn(usize, usize),     // insert into column N at tile index M
    Floating,
}
```

Computed in `scrolling.insert_position(pos)`:
1. `x = pos.x + view_pos()` — transform to space coords
2. Find closest column gap (horizontal decision)
3. Find closest tile gap within that column (vertical decision)
4. Return whichever gap is closer

### DnD Edge Scroll

In `workspace.dnd_scroll_gesture_scroll(pos, speed)`:
1. Compute `x` relative to working area.
2. If `x < trigger_width`: delta = `-(trigger_width - x)`
3. If `width - x < trigger_width`: delta = `trigger_width - (width - x)`
4. Normalize to `[0, 1]`, multiply by speed.
5. Call `scrolling.dnd_scroll_gesture_scroll(delta)` on all workspaces.

This uses the existing `ViewGesture` system with `dnd_last_event_time` tracking.

### Insert Hint (visual placeholder)

`scrolling.insert_hint_area(position)` returns a `Rectangle`:
- **NewColumn at edge**: 300×(working_h - 2*gaps) rectangle beside edge column.
- **NewColumn in middle**: same size, centered on gap.
- **InColumn at top/bottom**: full column width × 150px at edge.
- **InColumn in middle**: full column width × 300px centered on tile gap.

---

## heca Implementation Plan

### Phase 0: Focus Follows Mouse (Small)

**File: `heca/src/main.rs`**

In `CursorMoved` handler:
```rust
if state.mouse_enabled && state.focus_follows_mouse {
    // Hit-test against pane positions
    // If over a different pane than focused_pane:
    //   state.focused_pane = Some(pane_id);
    //   sync_focus(state);
    //   state.needs_redraw = true;
}
```

Add `focus_follows_mouse: bool` to `AppConfig` (default `true`).

---

### Phase 1: Interactive Move Core (Large)

#### 1.1 Data Model

**`heca/src/app_state.rs`** — Expand `DragState`:

```rust
pub enum DragState {
    None,
    // ... existing variants ...

    /// Interactive move: starting (rubberband, window still in layout).
    InteractiveMoveStarting {
        pane_id: u64,
        start_mouse: (f32, f32),
        pointer_delta: (f32, f32),
        threshold_sq: f32,
    },

    /// Interactive move: moving (window detached from layout, follows pointer).
    InteractiveMove {
        pane_id: u64,
        /// Mouse position minus tile top-left at grab start (screen coords).
        offset: (f32, f32),
        /// Original column index (for potential restore).
        original_col: usize,
        /// Original pane index within column.
        original_pane: usize,
        /// Whether the pane was removed from the layout.
        removed_from_layout: bool,
    },
}
```

**`heca-core/src/layout/scrolling.rs`** — Add `insert_position`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InsertPosition {
    NewColumn(usize),
    InColumn(usize, usize),
}

impl ScrollingSpace {
    /// Compute where a pane dropped at `pos` (in space coordinates) should go.
    pub fn insert_position(&self, pos: Point) -> InsertPosition {
        if self.columns.is_empty() {
            return InsertPosition::NewColumn(0);
        }
        let x = pos.x + self.view_pos();
        let y = pos.y;

        // Adjust for gap centering.
        let x = x + self.options.gaps / 2.0;
        let y = y + self.options.gaps / 2.0;

        if x < 0.0 {
            return InsertPosition::NewColumn(0);
        }

        // Find closest column gap.
        let (closest_col_idx, col_x) = self.column_xs()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        // Find column containing the position.
        let (col_idx, _) = self.column_xs()
            .enumerate()
            .take_while(|(_, cx)| *cx <= x)
            .last()
            .unwrap_or((0, 0.0));

        if col_idx >= self.columns.len() {
            return InsertPosition::NewColumn(closest_col_idx);
        }

        let col = &self.columns[col_idx];

        // Find closest tile gap within column.
        let mut tile_ys: Vec<(usize, f64)> = Vec::new();
        let mut tile_y = self.working_area.loc.y + self.options.gaps;
        for (ti, _) in col.panes.iter().enumerate() {
            tile_ys.push((ti, tile_y));
            let size = col.pane_sizes.get(ti).copied().unwrap_or_default();
            tile_y += size.h + self.options.gaps;
        }
        tile_ys.push((col.panes.len(), tile_y)); // gap after last

        let (closest_tile_idx, tile_y) = tile_ys.into_iter()
            .min_by(|(_, a), (_, b)| (a - y).abs().partial_cmp(&(b - y).abs()).unwrap())
            .unwrap();

        let vert_dist = (col_x - x).abs();
        let hor_dist = (tile_y - y).abs();

        if vert_dist <= hor_dist {
            InsertPosition::NewColumn(closest_col_idx)
        } else {
            InsertPosition::InColumn(col_idx, closest_tile_idx)
        }
    }
}
```

#### 1.2 Starting Phase

**`heca/src/main.rs`** — In `MouseInput` (Left + meta modifier):

```rust
if button == MouseButton::Left && button_state == ElementState::Pressed {
    let meta_held = state.modifiers.state().super_key(); // or config
    if meta_held {
        // Hit-test to find pane under cursor.
        if let Some(pane_id) = hit_test_pane(state, mouse_pos) {
            state.drag_state = DragState::InteractiveMoveStarting {
                pane_id,
                start_mouse: mouse_pos,
                pointer_delta: (0.0, 0.0),
                threshold_sq: 64.0, // 8px squared
            };
            // Lock view for DnD scroll (begin gesture on all workspaces).
            for ws in state.session.workspaces_mut() {
                ws.scrolling.dnd_scroll_gesture_begin();
            }
        }
    } else {
        // Existing: normal click to focus.
        // ...
    }
}
```

**`heca/src/main.rs`** — In `CursorMoved`, handle Starting:

```rust
match &mut state.drag_state {
    DragState::InteractiveMoveStarting { pane_id, start_mouse, pointer_delta, threshold_sq } => {
        let dx = mouse_pos.0 - start_mouse.0;
        let dy = mouse_pos.1 - start_mouse.1;
        *pointer_delta = (dx, dy);

        // Apply rubberband to the pane's move_offset in layout.
        let sq_dist = dx * dx + dy * dy;
        let factor = rubberband(sq_dist / threshold_sq); // 0.0 → 1.0
        if let Some(ws) = state.session.active_workspace_mut() {
            if let Some((ci, pi)) = find_pane(ws, *pane_id) {
                ws.scrolling.columns[ci].panes[pi].move_offset = Animated::Static(
                    Point::new(dx as f64 * factor, dy as f64 * factor)
                );
            }
        }

        // If exceeded threshold, transition to Moving.
        if sq_dist > *threshold_sq {
            start_interactive_move(state, *pane_id, mouse_pos);
        }
    }
    // ...
}
```

#### 1.3 Moving Phase

**`heca/src/main.rs`** — `start_interactive_move`:

```rust
fn start_interactive_move(state: &mut AppState, pane_id: u64, mouse_pos: (f32, f32)) {
    let ws = state.session.active_workspace_mut().unwrap();

    // Find and remove pane from layout.
    let mut found = None;
    for (ci, col) in ws.scrolling.columns.iter().enumerate() {
        for (pi, pane) in col.panes.iter().enumerate() {
            if pane.id.0 == pane_id {
                found = Some((ci, pi));
                break;
            }
        }
    }
    let (col_idx, pane_idx) = found.unwrap();

    // Store original position for animation.
    let old_col_x = ws.scrolling.column_x(col_idx);
    let old_pane_y = ws.scrolling.pane_y_in_column(col_idx, pane_idx);
    let old_render_pos = Point::new(
        old_col_x + ws.scrolling.view_offset.current(),
        old_pane_y,
    );

    // Remove pane (but keep backend alive).
    let removed = ws.scrolling.remove_pane(col_idx, pane_idx).unwrap();
    let pane_title = removed.title.clone();

    // Animate columns that shift.
    ws.scrolling.update_all_column_widths();

    // Store detached pane info.
    state.detached_pane = Some(DetachedPane {
        pane: removed,
        original_col: col_idx,
        original_pane: pane_idx,
        alpha: 0.3,
        render_pos: old_render_pos, // will animate to pointer
    });

    state.drag_state = DragState::InteractiveMove {
        pane_id,
        offset: (/* pointer - pane_top_left */),
        original_col: col_idx,
        original_pane: pane_idx,
        removed_from_layout: true,
    };

    // Animate the detached pane from its old position to the pointer position.
    // ...
}
```

**`heca/src/main.rs`** — In `CursorMoved`, handle Moving:

```rust
DragState::InteractiveMove { pane_id, offset, .. } => {
    let pointer_in_content = (
        mouse_pos.0 - pane_area.x,
        mouse_pos.1 - pane_area.y,
    );

    // Update detached pane render position.
    if let Some(detached) = &mut state.detached_pane {
        detached.render_pos = Point::new(
            (pointer_in_content.0 - offset.0) as f64,
            (pointer_in_content.1 - offset.1) as f64,
        );
    }

    // Compute insert position for placeholder rendering.
    if let Some(ws) = state.session.active_workspace() {
        let space_pos = Point::new(
            (pointer_in_content.0 as f64) + ws.scrolling.view_pos(),
            pointer_in_content.1 as f64,
        );
        state.insert_hint = Some(ws.scrolling.insert_position(space_pos));
    }

    // DnD edge scroll.
    dnd_edge_scroll(state, mouse_pos.0, pane_area.w);
}
```

#### 1.4 DnD Edge Scroll

```rust
fn dnd_edge_scroll(state: &mut AppState, mouse_x: f32, content_width: f32) {
    let trigger = 80.0f32; // px from edge
    let speed = 400.0f32;  // px/sec

    let delta = if mouse_x < trigger {
        -(trigger - mouse_x)
    } else if content_width - mouse_x < trigger {
        trigger - (content_width - mouse_x)
    } else {
        0.0
    };

    if delta != 0.0 {
        let normalized = delta / trigger; // [0, 1]
        let scroll_delta = normalized * speed * (1.0 / 60.0); // per frame at 60fps

        if let Some(ws) = state.session.active_workspace_mut() {
            // Apply directly to view_offset for now.
            // In the future, use the Gesture system.
            ws.scrolling.view_offset.offset(scroll_delta as f64);
        }
    }
}
```

#### 1.5 Drop / End

**`heca/src/main.rs`** — In `MouseInput` (Left release):

```rust
if button == MouseButton::Left && button_state == ElementState::Released {
    match &state.drag_state {
        DragState::InteractiveMoveStarting { pane_id, .. } => {
            // Cancel: animate back to origin.
            cancel_interactive_move(state, *pane_id);
        }
        DragState::InteractiveMove { pane_id, original_col, original_pane, .. } => {
            // Drop at computed insert position.
            if let Some(hint) = state.insert_hint.take() {
                drop_pane(state, *pane_id, hint);
            } else {
                // Fallback: restore to original position.
                restore_pane(state, *pane_id, *original_col, *original_pane);
            }
        }
        _ => {}
    }
    state.drag_state = DragState::None;
    state.detached_pane = None;

    // End DnD scroll on all workspaces.
    for ws in state.session.workspaces_mut() {
        ws.scrolling.dnd_scroll_gesture_end();
    }
}
```

**`drop_pane`**:

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

    // Animate the dropped pane from its detached position to its new layout position.
    // (Compute old and new render positions, set animate_move_from on the pane.)
}
```

#### 1.6 Rendering

**`heca/src/main.rs`** — In `render()`:

```rust
// 1. Render normal layout (existing code).
//    Panes that were removed won't appear here.

// 2. Render detached pane (if any).
if let Some(detached) = &state.detached_pane {
    let px = pane_area.x + detached.render_pos.x as f32;
    let py = pane_area.y + detached.render_pos.y as f32;
    // Use the pane's original size or compute from original column.
    let pw = detached.pane_size.w as f32;
    let ph = detached.pane_size.h as f32;

    // Render with reduced alpha.
    render_backend_data_with_alpha(..., detached.alpha);
    primitive_renderer.draw_border(px, py, pw, ph, accent_color, border_width * 2.0);
}

// 3. Render insert hint (placeholder).
if let Some(hint) = state.insert_hint {
    let rect = compute_insert_hint_rect(&state.session, hint, pane_area);
    primitive_renderer.draw_rect(rect.x, rect.y, rect.w, rect.h,
        [accent_color[0], accent_color[1], accent_color[2], 0.15]);
    primitive_renderer.draw_border(rect.x, rect.y, rect.w, rect.h,
        [accent_color[0], accent_color[1], accent_color[2], 0.6], 2.0);
}
```

---

### Phase 2: Configurability

**`heca-config/src/lib.rs`** — Add to config:

```toml
[general]
focus_follows_mouse = true
interactive_move_modifier = "Super"  # or "Alt", "Ctrl", "None"
```

**`heca/src/input.rs`** — Parse modifier string to `ModifiersState` check:

```rust
fn modifier_is_held(modifiers: &ModifiersState, config: &str) -> bool {
    match config.to_lowercase().as_str() {
        "super" => modifiers.super_key(),
        "alt" => modifiers.alt_key(),
        "ctrl" => modifiers.control_key(),
        "shift" => modifiers.shift_key(),
        "none" => true,
        _ => false,
    }
}
```

---

### Phase 3: Animations & Polish

1. **Rubberband** during Starting phase.
2. **Alpha fade** (1.0 → 0.3) when transitioning to Moving.
3. **Alpha restore** (0.3 → 1.0) on drop.
4. **Move animation** on drop: animate from detached position to new layout position.
5. **Column animations** when removing/adding: existing `animate_move_from` on columns.
6. **DnD scroll** should use the existing `ViewOffset::Gesture` system rather than raw offset manipulation.

---

## Files to Modify

| File | Changes |
|------|---------|
| `heca/src/app_state.rs` | Expand `DragState`, add `DetachedPane`, `insert_hint` to `AppState` |
| `heca/src/main.rs` | Mouse event handling for all three phases, render detached pane + hint, `drop_pane`/`cancel_interactive_move` |
| `heca-core/src/layout/scrolling.rs` | Add `insert_position()`, `insert_hint_area()` (optional) |
| `heca-core/src/layout/column.rs` | Add `interactive_move_offset` to `Pane` (or reuse `move_offset`) |
| `heca-config/src/lib.rs` | Add `focus_follows_mouse`, `interactive_move_modifier` |
| `heca-config/src/theme.rs` | Load new config fields |

---

## Open Questions

1. **Should we use the existing `ViewOffset::Gesture` for DnD scroll, or direct offset manipulation?**
   - NIRI uses the Gesture system. For simplicity, Phase 1 can use direct offset. Phase 3 should migrate to Gesture.

2. **Should the detached pane render at its original size or resize dynamically?**
   - NIRI keeps original column width. For simplicity, keep original size; resize on drop.

3. **How to handle multi-workspace moves?**
   - Deferred. Phase 1 targets single-workspace only. Multi-workspace would need workspace hit-testing similar to NIRI's `monitor.insert_position()`.
