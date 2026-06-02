# Mouse System Redesign Plan

**Worktree:** `../heca-mouse-redo/`  
**Base:** `main` at `48b8ee9` (registry feature merged)  
**Status:** Awaiting go-ahead

---

## Decisions Made

### Task 1 (HIGHEST PRIORITY): `always_center_single_column` Configurable

**Problem:** First pane in a workspace is centered instead of left-aligned. NIRI defaults to `false`.

**Root cause:** `LayoutOptions::default()` hardcodes `always_center_single_column: true`.

**Fix:**
1. Add `always_center_single_column` field to config TOML schema
2. Thread it through to `LayoutOptions` at session creation time
3. Default: `false` (NIRI-compatible)
4. Use `#[serde(default)]` so existing configs continue working

**Files:**
- `heca-config/src/theme.rs` — add field to config struct
- `heca-core/src/layout/types.rs` — remove hardcoded default, accept from caller
- `heca-core/src/layout/session.rs` — pass config value into `LayoutOptions`

---

### Task 2: Fix Registry Bypasses in `main.rs`

Input mode handlers still call helper functions directly instead of `registry.execute()`:

| Location | Current | Fix |
|----------|---------|-----|
| `PaneSelect` mode | `focus_pane_by_id(...)` | `self.registry.execute(&FocusPane { ... }, state)` |
| `PaneSwap` same-ws | `swap_panes(...)` | `self.registry.execute(&Swap { ... }, state)` |
| `PaneSwap` cross-ws | `move_pane_to_workspace_column(...)` | `self.registry.execute(&Swap { ... }, state)` |
| `Chord` ws switch | `switch_workspace_tracked(...)` | `self.registry.execute(&FocusWorkspace { ... }, state)` |
| `SidebarNav` pane | `switch_workspace_tracked` + `focus_pane_by_id` | `self.registry.execute(&FocusPane { ... }, state)` |
| `SidebarNav` ws | `switch_workspace_tracked` + `sync_focus` | `self.registry.execute(&FocusWorkspace { ... }, state)` |
| `MouseInput` click | `focus_pane_by_id(...)` | `self.registry.execute(&FocusPane { ... }, state)` |

---

### Task 3: Rebuild Mouse System

Create `heca/src/mouse.rs` (single file, split to `mouse/` only when >400 lines).

#### Phase 0: Focus Follows Mouse
- Config: `focus_follows_mouse_delay_ms: u32` (default 150)
- Cursor must stay over pane for 150ms before focusing
- Disabled during modal modes
- Only active in `Normal`/`Prefix`
- Gated by `mouse_enabled` and config

#### Phase 1: Click to Focus
- Normal left-click → `registry.execute(&FocusPane { pane_id }, state)`
- Floating panes hit-tested first
- Exclusive bounds (`<` not `<=`)

#### Phase 2: Interactive Move
**Trigger:** Configured modifier + left-click

Two-phase state machine:
```
None → Starting (rubberband) → Moving (detached) → Drop
```

- Rubberband formula: `(1.0 - (1.0 / (x * c / d + 1.0))) * d` with `c=1.0, d=0.5`
- Drop targets: `InsertPosition::NewColumn` or `InsertPosition::InColumn`
- Fresh `ColumnId` on drop
- NIRI Principle 1: removing pane doesn't affect other column widths

#### Phase 3: Edge Scroll
Dual-mode:

| | Hover | Drag |
|--|-------|------|
| Trigger | 80px | 150px |
| Speed | 300 px/s | 1000 px/s |
| Inset | 8px | 0px |
| Content restriction | Inside content area only | Anywhere |

**Sidebar buffer:** Hover mode requires 40px buffer from sidebar boundary.

**Content bounds clamping:**
```rust
min_view_pos = -padding;
max_view_pos = (total_content_width - viewport_width + padding).max(min_view_pos);
```

---

### Task 4: Shift+Drag for Swap

- Shift + configured modifier + click on pane A → drag
- Drop on pane B → `registry.execute(&Swap { a_id: A, b_id: B }, state)`
- Drop on empty space / sidebar / chrome → **no-op** (cancel)

**Conflict:** If `interactive_move_modifier = "Shift"`, swap-drag needs alternative (e.g., `Ctrl+Shift+drag`).

---

### Task 5: New `WmAction` Variants

```rust
MovePaneToWorkspace { pane_id: u64, ws_idx: usize }
MovePaneToColumn { pane_id: u64, ws_idx: usize, col_idx: usize }
```

Handlers: `handle_move_pane_to_workspace`, `handle_move_pane_to_column`
Register in `build_registry()`
RPC commands: `move-pane-to-workspace`, `move-pane-to-column`

---

### Task 6: Config Additions

```rust
#[serde(default = "default_focus_follows_mouse_delay_ms")]
pub focus_follows_mouse_delay_ms: u32,
```

```toml
[settings]
focus_follows_mouse = true
focus_follows_mouse_delay_ms = 150
auto_scroll_edge = true
interactive_move_modifier = "Super"

[layout]
always_center_single_column = false
```

---

### Task 7: Mouse State Grouping

```rust
pub struct MouseState {
    pub pos: (f32, f32),
    pub hover_target: Option<u64>,
    pub hover_started: Option<Instant>,
    pub drag: DragState,
    pub detached: Option<DetachedPane>,
    pub insert_hint: Option<InsertPosition>,
}
```

`AppState` gets `pub mouse: MouseState`.

---

## Files to Modify (in order)

| Order | File | Task |
|-------|------|------|
| 1 | `heca-config/src/theme.rs` | Add config fields |
| 2 | `heca-core/src/layout/types.rs` | Make `always_center_single_column` configurable |
| 3 | `heca-core/src/layout/session.rs` | Pass config into `LayoutOptions` |
| 4 | `heca/src/input.rs` | Add `MovePaneToWorkspace`, `MovePaneToColumn` |
| 5 | `heca/src/handlers.rs` | Add handlers for new variants |
| 6 | `heca/src/actions.rs` | Register new handlers |
| 7 | `heca/src/app_state.rs` | Add `MouseState` sub-struct |
| 8 | `heca/src/main.rs` | Fix registry bypasses; thin mouse glue |
| 9 | `heca/src/mouse.rs` | New file — all mouse logic |
| 10 | `heca/src/rpc.rs` | Add commands for new variants |

---

## Build Verification

After each batch: `cargo build` then `cargo clippy --workspace --all-targets --all-features` — both zero warnings.

---

## Open Questions

1. Should `always_center_single_column` live under `[settings]` or `[layout]`?
2. If `interactive_move_modifier = "Shift"`, what modifier combo initiates swap-drag?
3. Should sidebar items include `Column` entries for column-level drop targets?

---

*Plan written. Awaiting go-ahead.*
