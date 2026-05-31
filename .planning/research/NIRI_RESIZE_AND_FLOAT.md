# NIRI Resize and Floating Behavior (from Source)

Read from: `YaLTeR/niri/src/layout/scrolling.rs`, `src/layout/workspace.rs`, `src/layout/mod.rs`

---

## 1. Column Resize (`set_column_width`)

**Location**: `scrolling.rs` ~line 4859

NIRI's resize changes **ONLY the active column's width**. Other columns are **NOT adjusted**.

```rust
fn set_column_width(&mut self, change: SizeChange, tile_idx: Option<usize>, animate: bool) {
    let current = self.width;  // Only this column
    let current_px = self.resolve_column_width(current);

    let width = match change {
        SizeChange::AdjustFixed(delta) => {
            ColumnWidth::Fixed((current_px + delta).clamp(1., MAX_PX))
        }
        SizeChange::AdjustProportion(delta) => {
            // If currently Proportion: add delta directly
            // If currently Fixed: convert to proportion first, then add delta
            ColumnWidth::Proportion(new_proportion)
        }
    };

    self.width = width;        // Only this column changes
    self.update_tile_sizes(animate);
}
```

**Key insight**: No rebalancing. If you make column 1 wider and the total exceeds the viewport, you scroll horizontally to see the rest. This is intentional — NIRI is a *scrolling* layout.

**`expand_column_to_available_width`** (special command, not default resize):
- Only used for a dedicated "expand" action
- Computes available space by counting fully-visible columns
- Sets active column to `Fixed(active_width + available_width)`
- Does NOT use proportion normalization

---

## 2. Floating (`toggle_window_floating`)

**Location**: `workspace.rs` ~line 1388

NIRI has **two separate spaces** in each workspace:
- `scrolling: ScrollingSpace` — the tiling columns
- `floating: FloatingSpace` — floating windows

### Tiling → Floating
```rust
let mut removed = self.scrolling.remove_tile(&id, Transaction::new());
// Position = original render position + (50, 50) offset
let pos = render_pos + Point::from((50., 50.));
removed.tile.floating_pos = Some(pos);
self.floating.add_tile(removed.tile, target_is_active);
self.floating_is_active = FloatingActive::Yes;  // switch focus to floating
```

### Floating → Tiling
```rust
let removed = self.floating.remove_tile(&id);
self.scrolling.add_tile(None, removed.tile, target_is_active,
    removed.width, removed.is_full_width, None);
self.floating_is_active = FloatingActive::No;
```

**Key insight**: Floating is a **first-class space**, not an afterthought. Windows move between the two spaces. The workspace tracks which space is active (`floating_is_active`).

---

## 3. What We Got Wrong

| Feature | Our Approach | NIRI's Approach |
|---------|-------------|-----------------|
| Resize | Normalize all columns to sum=1.0 | Only change active column |
| Float | Remove pane, lose backend, ad-hoc | Move between ScrollingSpace and FloatingSpace |
| Resize gaps | Our normalization creates gaps | No normalization = natural scroll overflow |

---

## 4. Correct Implementation Plan

### Resize
- `resize_active_column(delta)`: only adjust the active column's `ColumnWidth`
- Other columns keep their widths
- View scrolls naturally via existing `compute_view_offset_for_column`
- No rebalancing, no normalization

### Float
- Add `FloatingSpace` to `Workspace` (or simplify: `Vec<FloatingPane>` + `floating_is_active`)
- `toggle_float`: move active pane from scrolling to floating (or back)
- Keep the backend alive — floating pane still renders content
- Render floating panes after tiling panes, with their own position/size
- Do NOT use `remove_pane` for floating (that destroys columns)
