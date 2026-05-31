# heca Code Review

**Date:** 2026-05-31  
**Scope:** Full workspace — `heca-core`, `heca-renderer`, `heca-config`, `heca`  
**Status:** Does not compile (1 error, 20 warnings)

---

## 🚨 Critical (Must Fix)

### C1. Dual Layout Engines — PaneTree is Orphaned Dead Code

**Files:** `heca-core/src/pane.rs` (~600 lines), `heca/src/main.rs`, `heca-core/src/layout/`  
**Severity:** High

There are **two completely separate layout engines**, and only one is used:

| Engine | Location | Used? | Lines |
|--------|----------|-------|-------|
| BSP tree (`PaneTree`, `LayoutNode`) | `heca-core/src/pane.rs` | ❌ Never referenced by `heca` binary | ~600 |
| NIRI scrolling (`Session`, `ScrollingSpace`, `Column`) | `heca-core/src/layout/*.rs` | ✅ Used by `main.rs` | ~1500 |

The `PaneTree` type and all its methods (`split`, `remove`, `toggle_float`, `find_neighbor_geo`, `swap_panes`, `resize`, `move_pane`, `cycle_focus`, etc.) are completely unused. Additionally:

- `main.rs` imports `use heca_core::pane::{SplitDirection, GeoDir}` where `GeoDir` is **never used** (compiler warning)
- `main.rs` manually re-implements pane swap, float toggle, and resize logic ad-hoc instead of using PaneTree's built-in methods
- Two different `Pane` types (`heca_core::pane::Pane` vs `heca_core::layout::column::Pane`) confuse readers
- Two different floating systems (`PaneTree.floats: Vec<(u64, Rect)>` vs `Workspace.floating_panes: Vec<FloatingPane>`)

**Recommendation:** Either (a) delete `pane.rs` entirely and move any essential types into the layout module, or (b) if BSP tree layout is a future requirement, add a ticket and keep the code gated behind a feature flag. Do not ship dead code.

---

### C2. Does Not Compile — Non-Exhaustive Match

**File:** `heca/src/main.rs`, `execute_action()` function  
**Severity:** Blocking

```
error[E0004]: non-exhaustive patterns
  WmAction::MovePaneLeft, MovePaneRight, PaneHeightIncrease, PaneHeightDecrease not covered
```

These four `WmAction` variants are defined in `input.rs`, parsed from config, given priority slots in `action_priority()`, but have **no match arms** in `execute_action()`. The match is non-exhaustive and the project cannot compile.

**Recommendation:** Add match arms. If these are placeholders for future work, add a wildcard arm `_ => {}` that covers them, or implement them properly.

---

### C3. GPU Texture Created Per Text Command

**File:** `heca-renderer/src/text.rs`, `build_labels()` method  
**Severity:** High (performance)

Every call to `queue_text()` results in a **separate `device.create_texture()`** call:

```rust
// For each text command:
let texture = device.create_texture(&wgpu::TextureDescriptor { ... });
let view = texture.create_view(...);
let bind_group = device.create_bind_group(...);
```

For a terminal rendering 24 lines of text with mixed colors, this creates **24+ GPU textures and bind groups per frame**. These are immediately destroyed at the end of the frame.

**Impact:** Creates GPU memory allocation churn, stalls the GPU command queue, and prevents the driver from batching. On high-resolution terminals (e.g., `htop`, `bat --paging`) this will cause visible frame drops.

**Recommendation:** Implement a glyph atlas:
1. Cache glyph bitmaps from `cosmic-text` in a single large `R8Unorm` texture
2. Batch all glyph instances into a single vertex/index buffer per frame
3. Use a single bind group + texture for all text
4. Only re-upload glyphs when the cache misses (rare after warmup)

---

### C4. Staging Buffers Allocated Every Frame

**Files:** `heca-renderer/src/primitive.rs`, `heca-renderer/src/text.rs`  
**Severity:** High (performance)

Both renderers use:
```rust
let staging_vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { ... });
let staging_index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { ... });
```

This allocates new GPU memory for staging every frame. Pre-allocated buffers with `write_buffer` or a ring buffer pattern are standard practice.

**Recommendation:** Pre-allocate a large staging buffer (tens of MB) and use `queue.write_buffer()` with sub-regions and frame cycled offsets. Reset the write cursor each frame.

---

### C5. Terminal Child Death Not Detected

**File:** `heca-core/src/backend/terminal.rs`  
**Severity:** High (leak)

```rust
fn should_close(&self) -> bool {
    self.exited  // initialized to false, NEVER SET TO TRUE
}
```

When the shell process dies:
- The reader thread sees `read()` return `0` (EOF) and exits
- No message is sent to indicate process death
- `should_close()` still returns `false`
- The pane lives on forever as a dead terminal

Additionally, `PtyHandle::Drop` only closes the `master` fd but never waits on or kills the `child` process, leaving orphan shell processes.

**Recommendation:** Have the reader thread send a "closed" signal (e.g., send `Vec::new()` or use an enum, or a second channel) when read returns 0. Set `self.exited = true` on receipt. Also add `child.wait()` in the Drop impl.

---

### C6. Keybinding Defaults Duplicated

**File:** `heca-config/src/theme.rs`, `Config::default()`  
**Severity:** Medium (correctness)

The entire keybinding insert block (~30 lines) appears **twice verbatim**:

```rust
keybindings.insert("focus_left".to_string(), "h,ArrowLeft".to_string());
// ... ~28 more inserts ...
keybindings.insert("focus_left".to_string(), "h,ArrowLeft".to_string());  // DUPLICATE
```

The second block is a copy-paste error. `HashMap::insert` silently overwrites, so behavior is correct, but the code is confusing and wastes 30 unnecessary lines.

**Recommendation:** Delete the second duplicate block.

---

### C7. PTY Non-Unix Stub Is Non-Functional

**File:** `heca-core/src/backend/terminal.rs`, `PtyHandle::new_stub()`  
**Severity:** Medium

The non-Unix PTY implementation uses `cmd.exe` with piped stdin/stdout, which does not create a real PTY. This means terminal backends won't function on Windows.

**Recommendation:** Either mark `TerminalBackend` as `#[cfg(unix)]` and provide a stub/no-op backend on other platforms, or implement a Windows PTY via `CreatePseudoConsole`.

---

## 🐛 Bugs

### B1. Hardcoded Font Metrics

**File:** `heca/src/main.rs`, `render_backend_data()`  
**Severity:** Medium

```rust
let cell_h = 14.0f32;
let cell_w = 8.4f32; // ~0.6 * cell_h for monospace
```

These values assume a specific font at a specific size. At `theme.font_size: 32.0` (the default), a cell would be 14px high while the font is 32px. This will produce completely wrong character positioning.

**Recommendation:** Derive cell dimensions from `cosmic-text` metrics after shaping. Or at minimum, use `theme.font_size` (scaled) as the cell height and compute width from the font's `advance_width` for a representative character (e.g., `'W'` or `'M'`).

---

### B2. Pane Height Distribution Loses Pixels

**File:** `heca-core/src/layout/column.rs`, `compute_pane_sizes()`  
**Severity:** Low (visual)

```rust
let auto_height = (height_left / auto_count as f64).max(1.0);
```

When `height_left` is not evenly divisible by `auto_count`, the remaining fractional pixels are silently dropped. This leaves a gap at the bottom of the column. NIRI's algorithm gives the remainder to the last auto-sized pane.

**Recommendation:** After distributing auto heights, give the last auto pane the remaining height:
```rust
let remainder = height_left - auto_height * auto_count as f64;
if remainder > 0.5 { /* add to last auto pane */ }
```

---

### B3. Terminal Scroll Region Reset on Resize

**File:** `heca-core/src/backend/terminal.rs`, `Grid::resize()`  
**Severity:** Low (edge case)

```rust
fn resize(&mut self, width: usize, height: usize) {
    // ...
    self.scroll_region_top = 0;
    self.scroll_region_bottom = height;
}
```

Full-screen terminal apps (`less`, `vim`, `tmux`) set custom scroll regions via `\e[r`. Resetting both boundaries on resize causes visual corruption during live window resize. NIRI preserves scroll regions when possible.

**Recommendation:** On resize, if the current scroll region fits within the new height, keep it. Only reset if the new height is smaller than the old region.

---

### B4. Overview Click Targets Non-Functional During Animation

**File:** `heca-core/src/layout/session.rs`, `workspace_under()`  
**Severity:** Low

`workspace_under()` uses `overview_workspace_geometries()` which only computes the **final** overview positions. During the overview open/close animation, the actual rendered workspaces are transitioning between normal and overview positions, but click hit-testing uses only the overview positions.

**Recommendation:** Interpolate workspace positions during animation progress, or disable click-to-focus during transitions.

---

### B5. Tab Layout Renders Tabs That Don't Do Anything

**File:** `heca/src/main.rs`, render section  
**Severity:** Low

The tab bar renders tabs and tracks `active_tab` index, but there is no tab data model behind it. `tab_names` is hardcoded to `["Main"]`. Tab switching just rotates the index but has no effect on the layout.

**Recommendation:** Either implement real tab/window management (session → windows → tabs → panes) or remove the tab bar cosmetic rendering until that architecture exists.

---

## ⚠️ Compiler Warnings (20 Total)

### heca-core (16 warnings)

| Location | Warning | Fix |
|----------|---------|-----|
| `animation.rs:1` | Unused import `Duration` | Remove it |
| `animation.rs:128` | Unnecessary parens | Remove parens |
| `session.rs:3` | Unused import `ScrollingSpace` | Remove it |
| `session.rs:5` | Unused import `ViewOffset` | Remove it |
| `types.rs:1` | Unused import `std::fmt` | Remove it |
| `view_offset.rs:2` | Unused import `Instant` | Remove it |
| `workspace.rs:1` | Unused imports `Animation`, `AnimationConfig` | Remove them |
| `workspace.rs:4` | Unused import `ViewOffset` | Remove it |
| `terminal.rs:466` | Irrefutable `if let` pattern | Replace with `let` |
| `scrolling.rs:196` | Unused variable `col_x` | Prefix `_col_x` or use it |
| `scrolling.rs:558,559` | Unnecessary `mut` | Remove `mut` |
| `terminal.rs:362` | Field `child` never read | Add `#[allow(dead_code)]` or use it |
| `animation.rs:28` | Field `velocity` never read | Remove or add `#[allow(dead_code)]` |
| `animation.rs:10` | Function pointer `PartialEq` comparison | Remove `PartialEq` derive or box the fn |

### heca binary (4 warnings)

| Location | Warning | Fix |
|----------|---------|-----|
| `main.rs:9` | Unused import `ViewOffset` | Remove it |
| `main.rs:10` | Unused imports `Animation`, `AnimationConfig` | Remove them |
| `main.rs:11` | Unused imports `SplitDirection`, `GeoDir` | Remove them |
| `main.rs:21` | Unused import `NamedKey` | Remove it |

---

## 🔧 Design Issues

### D1. Two Floating Systems

- `heca_core::pane::PaneTree` floats: `Vec<(u64, Rect)>` (dead code)
- `heca_core::layout::workspace::Workspace` floats: `Vec<FloatingPane>`

The app uses the second system, but the float/unfloat toggle logic is hand-written in `main.rs` (~80 lines of ad-hoc copy-paste) instead of being a method on `Workspace`. A `fn toggle_float(&mut self, pane_id: PaneId, position: Point, size: Size)` on `Workspace` would encapsulate this.

### D2. Three Coordinate/Type Systems

| Type | Module | Precision | Used For |
|------|--------|-----------|----------|
| `Rect { x,y,w,h: f32 }` | `heca-core/src/types.rs` | f32 | Chrome config, app-level rendering |
| `Rectangle { loc: Point, size: Size }` | layout/types.rs | f64 | Layout engine |
| `Point/Size` with `f64` | layout/types.rs | f64 | Layout engine |

Every call site that bridges chrome → layout or layout → rendering does manual `as f32` / `as f64` casting. This is fragile and error-prone. Pick one precision (f64 internally, f32 at the GPU boundary) and convert at the boundary.

### D3. `crossbeam-channel` Dependency Unused

`heca-core/Cargo.toml` lists `crossbeam-channel = "0.5"` but `terminal.rs` uses `std::sync::mpsc`. This is a dead dependency.

### D4. Sidebar Content Is Placeholder

The left sidebar renders `"Sessions"` and `"(empty)"`. The right sidebar renders `"Details"`. Both are hardcoded strings. If sidebars are part of the v1 requirements (COMP-11, COMP-12), they need real content. If not, defer rendering.

### D5. Mouse Drag Operations Not Implemented

`DragState` has variants for `Resizing`, `MovingFloat`, `ResizingFloat` but these are never set or handled in the event loop. Mouse input only handles click-to-focus. Requirements LAY-07, LAY-08 (mouse resize, mouse drag float) are unimplemented.

### D6. `PaneBackend::render_data()` Clones the Entire Grid

`TerminalBackend::render_data()` allocates a full `Vec<Vec<TerminalCell>>` copy every frame. For a 100×40 terminal that's 4,000 cells × 4 allocations each. This should use zero-copy references or batched rendering.

### D7. No Error Handling for GPU Surface

```rust
let surface_format = surface_caps.formats.iter().find(|f| f.is_srgb())
    .copied().unwrap_or(surface_caps.formats[0]);
```

`surface_caps.formats[0]` panics if the vector is empty. While unlikely in practice, this is an avoidable panic.

---

## 📋 Summary

### Priority Order

| # | Issue | Type | Effort |
|---|-------|------|--------|
| 1 | **C2** — Non-exhaustive match (doesn't compile) | Blocking | 5 min |
| 2 | **C1** — Delete or wire PaneTree | Architecture | 1 hr |
| 3 | **C6** — Remove duplicate keybindings | Cleanup | 2 min |
| 4 | All **warnings** (20) | Cleanup | 20 min |
| 5 | **C5** — Terminal should_close detection | Bug | 30 min |
| 6 | **C3** — Glyph atlas for text renderer | Performance | 2-3 days |
| 7 | **C4** — Ring buffer for staging buffers | Performance | 1 day |
| 8 | **B1** — Font metrics from cosmic-text | Bug | 2 hr |
| 9 | **D2** — Unify coordinate types | Architecture | 2 hr |
| 10 | **D1** — Encapsulate float toggle on Workspace | Design | 1 hr |

### Quick Wins (can be done in <1 hour)
- Add match arms for `MovePaneLeft`, `MovePaneRight`, `PaneHeightIncrease`, `PaneHeightDecrease`
- Delete `pane.rs` (600 lines of dead code)
- Remove all 20 unused imports/variables
- Delete duplicate keybindings
- Remove `crossbeam-channel` from `Cargo.toml`
- Add `child.wait()` in `PtyHandle::Drop`

### Structural Concerns
The biggest architectural issue is the **two layout engines**. Shipping both creates confusion and maintenance burden. Either commit to the NIRI scrolling model or the BSP tree model — don't hedge.
