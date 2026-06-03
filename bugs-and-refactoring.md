# Heca — Bugs, Bad Practices and Refactoring Roadmap

**Scope:** `heca-core`, `heca-renderer`, `heca-config`, `heca/src` (read-only analysis)  
**Excluded:** `heca-grid-ui`, `heca-ui`  
**Date:** 2026-06-03

---

## 1. Executive Summary

The core data model is architecturally sound — the NIRI-inspired `Session → Workspace → ScrollingSpace → Column → Pane` hierarchy is correctly structured. The critical problem is that **all behavior lives outside the types**, in ~1,400 lines of free functions in `handlers.rs` and `main.rs`. This creates deep index chains, makes correctness hard to enforce, and causes the `SidebarTree` (a complete second copy of the workspace/column/pane hierarchy) to drift from the live session state.

---

## 2. Critical: No Domain Encapsulation

### Problem

Every window-manager operation is a top-level `fn` that reaches into `AppState` internals:

```rust
// handers.rs — the canonical pattern
pub fn handle_swap_left(state: &mut AppState, _action: &WmAction) {
    if let Some(ws) = state.session.workspaces.get_mut(ws_idx) {
        ws.scrolling.columns[ci].panes[pi]           // bare index access
    }
}
```

`main.rs` also exposes standalone helpers like `destroy_empty_workspace()`, `focus_pane_by_id()`, `find_pane_location()`, `move_pane_to_column()`, `move_pane_to_workspace_column()`, `pane_name()`, `collect_all_pane_candidates()`, `switch_workspace_tracked()`, `sync_focus()`, `update_session_viewport()`.

No domain type owns its own operations:

| Operation | Lives in | Should live in |
|-----------|----------|----------------|
| focus left/right/up/down | `handlers.rs` | `ScrollingSpace` / `Workspace` |
| add/remove pane or column | `handlers.rs` | `ScrollingSpace`, `Workspace` |
| resize column / pane | `handlers.rs` | `ScrollingSpace`, `Column` |
| rename pane / workspace | `handlers.rs` | `Pane`, `Workspace` |
| swap / move panes between columns | `handlers.rs` | `ScrollingSpace` |
| destroy empty workspace | `handlers.rs` | `Workspace` |

### Impact

- Callers must know the exact indexing scheme (`workspaces[].scrolling.columns[].panes[]`).
- Any structural change to the hierarchy ripples across every handler.
- Impossible to enforce invariants (e.g. "destroy column when empty") because there is no single owner.

### Target shape

```rust
// ScrollingSpace
pub fn focus_left(&mut self);
pub fn focus_right(&mut self);
pub fn move_active_pane_left(&mut self);
pub fn swap_columns(&mut self, a: usize, b: usize);
pub fn move_pane_to_column(&mut self, src_col: usize, pane: Pane, dst_col: usize);

// Workspace
pub fn add_pane(&mut self, pane: Pane, backend: Box<dyn PaneBackend>);
pub fn remove_column(&mut self, idx: usize);
pub fn maybe_destroy_if_empty(&mut self) -> bool;
pub fn rename(&mut self, new_name: String);
pub fn is_empty(&self) -> bool;

// Session
pub fn handle_focus_pane(&mut self, pane_id: u64);
pub fn handle_swap_panes(&mut self, a_id: u64, b_id: u64);
```

---

## 3. High: Sidebar Duplication — Two Representations of the Same Tree

### Problem

The session hierarchy exists in two full copies:

1. **Scrolling area (source of truth)**
   ```
   Session.workspaces[usize]
     .scrolling.columns[Vec<Column>]
       .panes[Vec<Pane>]
   ```

2. **Sidebar (derived copy)**
   ```
   SidebarTree.workspaces[Vec<SidebarWsEntry>]
     .columns[Vec<SidebarColEntry>]
       .panes[Vec<SidebarPaneEntry>]
   ```

And the sidebar adds a parallel navigation state (`cursor`, `scroll_offset`, `flat_items`).

### Symptoms

| Symptom | Root cause |
|---------|-----------|
| Forgetting to call `tree.rebuild()` after a mutation | No enforced sync point |
| `rebuild()` unconditionally sets `collapsed: false` | UI forgetfulness bug |
| Sidebar rendering has O(n) `find()` calls per pane per frame for drag highlight | No index by pane_id |
| Workspace/column rename must update two separate tree traversals | Duplicated data |
| Panic risk: sidebar `ws_idx`/`col_idx` diverging from session indices | Two independent `Vec` containers |

### Recommended approach: Option A — Sidebar as View

`SidebarTree` becomes a transient render state. It holds only **UI-only** state (cursor, scroll, collapse flags) and rebuilds its `workspaces[]` / `flat_items[]` from `Session` on every sync.

```rust
impl SidebarTree {
    pub fn sync(&mut self, session: &Session, focused_pane: Option<u64>, ...) {
        // Preserve collapsed state keyed by (ws_idx) / (ws_idx, col_idx)
        // Rebuild workspaces[] and flat_items[] from session
        // Clamp cursor to new item count
    }
}
```

**Contract:**

| Event | Mechanism |
|-------|-----------|
| Sidebar → Session | Sidebar interaction emits `WmAction` → `ActionRegistry::execute()` → handler mutates `Session` |
| Session → Sidebar | After every handler runs, a single `sidebar_tree.sync(&session)` call is guaranteed |

This leverages the existing `ActionRegistry` as the shared integration point — no new coupling needed.

### Bonus fix: preserve collapsed state

```rust
let old_collapsed_ws: HashMap<usize, bool> =
    self.workspaces.iter().map(|ws| (ws.ws_idx, ws.collapsed)).collect();
let old_collapsed_col: HashMap<(usize, usize), bool> =
    self.workspaces.iter().flat_map(|ws| {
        ws.columns.iter().map(move |col| ((ws.ws_idx, col.col_idx), col.collapsed))
    }).collect();
```

Apply `old_collapsed_*` values during rebuild instead of hardcoding `false`.

---

## 4. High: `AppState` Is a God Object

`AppState` holds 25+ fields:

```rust
pub struct AppState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device>,
    pub queue: wgpu::Queue,
    pub primitive_renderer: PrimitiveRenderer,
    pub text_renderer: TextRenderer,
    pub session: Session,
    pub backends: HashMap<u64, Box<dyn PaneBackend>>,
    pub theme: Theme,
    pub input_mode: InputMode,
    pub sidebar: SidebarState,
    pub sidebar_tree: SidebarTree,
    pub mouse: MouseState,
    pub modifiers: ModifiersState,
    pub focused_pane: Option<u64>,
    pub last_focused: Option<u64>,
    pub last_visited_ws_idx: Option<usize>,
    pub last_visited_pane_per_ws: Vec<Option<u64>>,
    pub mouse_enabled: bool,
    pub auto_scroll_edge: bool,
    pub interactive_move_modifier: ModifierKey,
    pub prefix_entered_at: Option<Instant>,
    pub prefix_combo: KeyCombo,
    pub pending_reload: bool,
    pub needs_redraw: bool,
    pub scale_factor: f64,
    pub active_tab: usize,
    pub tab_names: Vec<String>,
}
```

Every handler receives all of this and reaches into it directly. Once behavior moves onto the domain types (`Session`, `Workspace`, etc.), handlers will shrink to:

```rust
pub fn handle_split_horizontal(state: &mut AppState, _: &WmAction) {
    let pane = state.session.next_pane();
    state.session.add_pane(pane);
    state.backends.insert(pane.id.0, Box::new(FakeBackend::new(80, 24)));
    state.sidebar_tree.sync(&state.session, ...);
    state.needs_redraw = true;
}
```

`AppState` then becomes a composition of smaller state objects rather than a flat grab bag.

---

## 5. High: `#[allow(dead_code)]` Pervasiveness

AGENTS.md: *"NEVER add `#[allow(dead_code)]` without a clear reason."*

Current violations:

| File | Line | Item suppressed |
|------|------|-----------------|
| `actions.rs` | 9 | `#![allow(dead_code)]` on the entire module |
| `actions.rs` | 205 | `action_discriminant()` |
| `keymap.rs` | 119 | `unbind()` |
| `keymap.rs` | 126 | `rebind()` |
| `keymap.rs` | 141 | `bindings_in_mode()` |
| `keymap.rs` | 148 | `has_mode()` |
| `input.rs` | 48 | `WmAction` |
| `input.rs` | 205 | `build_action()` |
| `input.rs` | 486 | `parse_key()` |
| `input.rs` | 407 | `action_priority()` |

Suggested policy:
- Remove unused items entirely, **or**
- Add a `// Reason:` comment above each `#[allow(dead_code)]` explaining why it's needed for a future phase.

Items gated by features should use `#[cfg(feature = "rpc")]` instead.

---

## 6. Medium: Debug `eprintln!()` in Production Paths

`handlers.rs` contains ~40 unconditional `eprintln!()` calls in swap, move, and close handlers:

```rust
eprintln!("[swap] handler called: a={} b={}", a_id, b_id);
eprintln!("[swap] a_loc={:?} b_loc={:?}", a_loc, b_loc);
eprintln!("[move-ws] pane not found or already in target workspace: pane={} target_ws={}", pane_id, ws_idx);
```

These fire on every user-triggered operation.

**Replace with a compile-time-gated macro:**
```rust
#[cfg(debug_assertions)]
macro_rules! wm_debug { ($($arg:tt)*) => (eprintln!($($arg)*)); }
#[cfg(not(debug_assertions))]
macro_rules! wm_debug { ($($arg:tt)*) => {}; }
```

Or gate behind a runtime `state.settings.verbose` flag via `AppState::debug_log(&self, ...)`.

---

## 7. Medium: `SidebarTree::rebuild()` Bugs and Performance

### Bug: collapses are reset

```rust
// sidebar.rs:120 — runs on every rebuild
let mut ws_entry = SidebarWsEntry {
    ws_idx,
    name: ...,
    collapsed: false,    // <-- always resets to expanded
    state,
    columns: Vec::new(),
};
```

This means if the user collapses a workspace, **one subsequent rebuild collapses it back to expanded**. With Option A's enforced `sync()`, this will trigger on every action — making collapse literally unusable.

### Performance: full rebuild on every change

`rebuild()` does `workspaces.clear(); flat_items.clear()` and allocates new `Vec`s + clones every `pane.title` `String`. In a typical frame loop or after every keystroke in sidebar nav mode this is unnecessary allocation pressure.

**Mitigations:**
- Pre-allocate with `Vec::with_capacity(session.pane_count())`.
- Avoid re-cloning stable strings when names haven't changed.
- Track a `dirty` flag and skip rebuilds when nothing changed.

---

## 8. Medium: Inconsistent Rectangle Types

Three rectangle representations exist:

| Type | Location | Precision |
|------|----------|-----------|
| `Rect { x, y, w, h: f32 }` | `heca-core/src/types.rs` | f32 |
| `Rectangle { loc: Point<f64>, size: Size<f64> }` | `heca-core/src/layout/types.rs` | f64 |
| (wgpu internal) | renderer | f32 |

`Rect` in `types.rs` is dead code from an earlier design — nothing uses it.

**Action:** Remove `Rect` and `Point` from `heca-core/src/types.rs`. All layout math lives in `layout/types.rs` with `f64` for sub-pixel correctness.

---

## 9. Medium: `PaneBackend` Lifecycle — No Ownership

`AppState.backends: HashMap<u64, Box<dyn PaneBackend>>` is maintained manually:

```rust
// handlers.rs — every close handler remembers to do this:
if let Some(removed) = ws.scrolling.remove_pane(col_idx, pane_idx) {
    state.backends.remove(&removed.id.0);  // manual cleanup
}
```

If any future code path removes a pane without updating `backends`, the backend leaks. There is no compile-time guarantee that the two stay in sync.

**Suggestion:**
```rust
// Column::remove_pane() takes ownership of the backend
fn remove_pane(&mut self, idx: usize) -> Option<(Pane, Box<dyn PaneBackend>)> {
    ...
}
```

Call it as:
```rust
let (pane, backend) = col.remove_pane(idx)?;
drop(backend); // always runs when `(pane, backend)` is dropped
```

---

## 10. Low-Medium: Error Handling Strategy

`PtyHandle::new_unix()` returns `Result<String, String>` — errors are generic `String`s.

**Suggestion: use `thiserror` for domain errors:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("pane not found: {id}")]
    PaneNotFound { id: u64 },
    #[error("column index out of range: {idx}")]
    ColumnOutOfRange { idx: usize },
    #[error("workspace index out of range: {idx}")]
    WorkspaceOutOfRange { idx: usize },
}
```

`ActionRegistry::execute()` silently does nothing when no handler is registered:
```rust
// actions.rs:96-103
pub fn execute(...) {
    if let Some(handler) = self.handlers.get(&disc) {
        handler(state, action);
    } else {
        eprintln!("No handler registered for {:?}", action); // only stderr
    }
}
```

This should at minimum propagate an error or set a `state.error` flag that the status bar displays.

---

## 11. Low: Magic Numbers

Scattered unnamed constants:

| Value | File | Meaning |
|-------|------|---------|
| `vw * 0.9`, `vh * 0.9` | `handlers.rs` | Max animation delta ratio for cross-workspace swap |
| `500ms` | various | Prefix timeout |
| `50.0` | `column.rs` | Minimum pane height |
| `200.0` | `column.rs` | Default preferred pane height |
| `velocity * 0.3` | `animation.rs` | Deceleration projection factor |
| `ITEM_HEIGHT = 24.0` | `sidebar.rs` | Sidebar row height |
| `BTN_SIZE = 20.0` | `sidebar.rs` | Plus/minus button size |
| `BTN_RADIUS = 4.0` | `sidebar.rs` | Button corner radius |
| `40.0` | `handlers.rs` | Pane height resize step (px) |
| `0.05` | `handlers.rs` | Column width resize step (fraction) |

**Collect into named constants in a shared `consts.rs` or at the top of each module:**
```rust
const SWAP_MAX_DELTA_RATIO: f64 = 0.9;
const PREFIX_TIMEOUT: Duration = Duration::from_millis(500);
const MIN_PANE_HEIGHT: f64 = 50.0;
```

---

## 12. Low: Unsafe Without `// SAFETY:` Comments

`heca-core/src/backend/terminal.rs` uses `unsafe` for:
- `openpty()` — fork/exec safety
- `dup()` — FD duplication invariants
- `from_raw_fd()` — ownership transfer
- `ioctl(fd, TIOCSWINSZ, ...)` — FD must be valid PTY master

Every `unsafe` block must have a `// SAFETY:` comment explaining which invariants it relies on. This is required by the Rust API guidelines and `clippy::undocumented_unsafe_blocks`.

---

## 13. What Is Well-Designed

| Component | Strength |
|-----------|----------|
| `ActionRegistry` | Clean discriminant-based `HashMap` dispatch, extensible registry, static metadata catalog for command palette |
| `KeymapRegistry` | Per-mode keymaps, case-insensitive matching via custom `Hash`/`PartialEq` on `KeyCombo` |
| `WmAction` enum | Comprehensive, organized by category (Navigation, Layout, Pane, Workspace, Chrome, System) |
| `Animated<T>` | Generic "tweenable" wrapper with spring easing; cleanly separated from domain types |
| `ViewOffset` | Three-state (Static / Animation / Gesture) scroll model matching NIRI exactly |
| `ScrollingSpace` / `Workspace` / `Session` hierarchy | Correct NIRI structure; data model is sound |

---

## 14. Refactoring Roadmap

### Phase 1: Sidebar sync fix (stops drift)

```rust
impl SidebarTree {
    pub fn sync(&mut self, session: &Session, focused_pane: Option<u64>, ...) {
        let old_collapsed_ws: HashMap<usize, bool> = ...;
        let old_collapsed_col: HashMap<(usize,usize),bool> = ...;

        self.workspaces.clear();
        self.flat_items.clear();
        for (ws_idx, ws) in session.workspaces.iter().enumerate() {
            let collapsed = *old_collapsed_ws.get(&ws_idx).unwrap_or(&false);
            // rebuild preserving collapsed flag
        }
        self.rebuild_flat_items();
        self.clamp_cursor();
    }
}
```

Then commit to: **every mutation of `session` must be followed by `sidebar_tree.sync(&session)`** before the frame renders. This can be enforced by making `sync()` part of a single post-mutation hook in the event loop.

### Phase 2: Move behaviors onto domain types

Target: handlers drop from 1,400 lines to ~200 lines.

**`Session`** should own:
- `focus_left()`, `focus_right()`, `focus_up()`, `focus_down()`
- `focus_pane(pane_id: u64)`
- `handle_swap_panes(a_id: u64, b_id: u64)`
- `switch_to_workspace(idx: usize)`

**`Workspace`** should own:
- `is_empty()`, `maybe_destroy_if_empty() -> bool`
- `rename(&mut self, name: String)`
- `find_pane(&self, id: PaneId) -> Option<&Pane>`
- `find_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane>`

**`ScrollingSpace`** should own:
- `add_column(pos, col)`, `add_pane_to_column(col_idx, pane, focus)`
- `remove_pane(col_idx, pane_idx) -> Option<Pane>`
- `focus_left()`, `focus_right()`, `focus_up()`, `focus_down()`
- `move_active_pane_left()`, `move_active_pane_right()`
- `swap_columns(a, b)`
- `resize_active_column(delta: f64)`
- `move_pane_to_column(src_col, pane, dst_col)`

**`Column`** should own:
- `remove_pane(idx: usize) -> Option<Pane>`
- `swap_panes(a: usize, b: usize)`
- `rename_pane(id: PaneId, name: String)`

This removes the need for `find_pane_location()`, `focus_pane_by_id()`, `destroy_empty_workspace()`, `move_pane_to_column()`, `move_pane_to_workspace_column()` from `main.rs`.

### Phase 3: Thin handlers

After Phase 2, each handler is a 5-15 line dispatch:

```rust
pub fn handle_focus_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusPane { pane_id } = action else { return };
    state.session.focus_pane(*pane_id);
    sync_focus(state);
    state.sidebar_tree.sync(&state.session, ...);
    state.needs_redraw = true;
}
```

Cross-workspace swap logic would become `Session::swap_panes(a_id, b_id)` — a single 50-line method instead of 300 lines spread across `handlers.rs`.

### Phase 4: Cleanup

- Remove `Rect`, `Point` from `heca-core/src/types.rs`
- Remove empty `Neovim` / `Browser` variants from `BackendRenderData`
- Replace all `eprintln!()` debug calls with `wm_debug!()` macro
- Add `// SAFETY:` comments to all `unsafe` blocks in `terminal.rs`
- Consolidate magic numbers into named constants
- Replace `String` errors with `thiserror` enums
- Split `heca-config/src/theme.rs` into submodules (`color.rs`, `settings.rs`, `keys.rs`) — 676 lines is doing too much

---

## 15. Summary Table: All Findings

| ID | Severity | Category | Description |
|----|----------|----------|-------------|
| D1 | Critical | Architecture | All domain behavior in free functions; none on Session/Workspace/ScrollingSpace/Column/Pane |
| S1 | High | Sync | SidebarTree drifts from Session; no enforced sync point; `rebuild()` resets `collapsed` |
| S2 | High | Sync | Sidebar rendering does O(n) std::find lookups per frame for drag highlights |
| A1 | High | God Object | AppState is a flat 25-field struct with no ownership boundaries |
| A2 | High | Lifecycle | PaneBackend cleanup is manual; leak risk if any code path forgets |
| L1 | High | Dead code | `#[allow(dead_code)]` on 10 items violates AGENTS.md rule |
| L2 | High | Dead code | `Rect` type in `heca-core/src/types.rs` is unused |
| L3 | High | Dead code | `Neovim` / `Browser` variants in `BackendRenderData` are empty placeholders |
| D2 | Medium | Debug | ~40 `eprintln!()` calls in production handler paths |
| E1 | Medium | Errors | `Result<String, String>` errors instead of typed `thiserror` enums |
| E2 | Medium | Errors | `ActionRegistry::execute()` silently swallows missing handlers |
| T1 | Medium | Types | `Rect` (f32) coexists with `Rectangle` (f64) — one must go |
| M1 | Medium | Perf | `SidebarTree::rebuild()` allocates+clones on every sync; no capacity hints |
| P1 | Medium | Precision | wgpu uses f32, layout math uses f64 — no clear boundary |
| U1 | Low | Safety | Unsafe blocks in terminal.rs lack `// SAFETY:` comments |
| C1 | Low | Hygiene | 15+ magic numbers scattered across handlers, sidebar, animation |
| T2 | Low | Traits | `Rectangle`, `Point`, `Size` could implement `Default`, `Add`, `Sub` for ergonomics |
| R1 | Low | Render | `draw_rounded_rect()` accepts `_radius` parameter that is ignored |
