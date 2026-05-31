# NIRI Architecture Analysis & heca Implementation Plan

## What I Discovered by Reading NIRI's Source Code

I read through NIRI's actual implementation (`src/layout/scrolling.rs`, `src/layout/workspace.rs`, `src/layout/mod.rs`, `src/layout/monitor.rs`) — approximately 3,500 lines of layout code. Here is what NIRI actually does, corrected from common misconceptions.

---

## 1. NIRI's Real Architecture

### Hierarchy

```
Monitor (output/screen)
├── workspaces: Vec<Workspace>     ← arranged VERTICALLY (stacked)
│   ├── Workspace 1
│   │   ├── scrolling: ScrollingSpace   ← horizontal COLUMNS
│   │   │   ├── Column 1: [Tile a]
│   │   │   ├── Column 2: [Tile b, Tile c]   ← vertical stack
│   │   │   └── Column 3: [Tile d]
│   │   └── floating: FloatingSpace
│   ├── Workspace 2
│   │   └── ...
│   └── Workspace 3
│       └── ...
```

### Key Insight: Two Axes, Two Scrolling Models

| Axis | Container | Scroll Type | Mechanism |
|------|-----------|-------------|-----------|
| **Horizontal** | `ScrollingSpace.columns` | Continuous scroll + snap | `ViewOffset` (animated f64) |
| **Vertical** | `Monitor.workspaces` | Discrete switch + animation | `WorkspaceSwitch` (animated index) |

### What the User's Screenshot Shows

The user's screenshot (4 columns, column 2 has 2 vertically stacked apps) maps exactly to:

```
ScrollingSpace
├── Column 1: [Pane 1]          ← semi-hidden (scrolled off-screen left)
├── Column 2: [Pane 2, Pane 3]  ← two panes vertically stacked
├── Column 3: [Pane 4]          ← single pane
└── Column 4: [Pane 5]          ← semi-hidden (scrolled off-screen right)
```

This is **one workspace**, not multiple workspaces. The horizontal scrolling is driven by `ViewOffset`.

---

## 2. Core Data Structures (from NIRI source)

### `ScrollingSpace<W: LayoutElement>` (scrolling.rs, ~600 lines)

```rust
pub struct ScrollingSpace<W> {
    columns: Vec<Column<W>>,
    data: Vec<ColumnData>,          // cached widths
    active_column_idx: usize,
    view_offset: ViewOffset,        // THE horizontal scroll
    interactive_resize: Option<...>,
    view_size: Size<f64, Logical>,
    working_area: Rectangle<f64, Logical>,
    scale: f64,
    options: Rc<Options>,
}
```

### `Column<W: LayoutElement>` (scrolling.rs, ~400 lines)

```rust
pub struct Column<W> {
    tiles: Vec<Tile<W>>,            // Must be non-empty
    data: Vec<TileData>,            // cached heights
    active_tile_idx: usize,
    width: ColumnWidth,             // Proportion(f64) | Fixed(f64)
    is_full_width: bool,
    is_pending_fullscreen: bool,
    is_pending_maximized: bool,
    display_mode: ColumnDisplay,    // Normal | Tabbed
    view_size: Size<f64, Logical>,
    working_area: Rectangle<f64, Logical>,
    scale: f64,
    options: Rc<Options>,
}
```

### `ViewOffset` — The Horizontal Scroll Engine (scrolling.rs)

```rust
enum ViewOffset {
    Static(f64),
    Animation(Animation),           // smooth scroll to new position
    Gesture(ViewGesture),           // touchpad/mouse drag
}
```

**Key behavior**: `view_pos() = column_x(active_column_idx) + view_offset.current()`

When you focus a new column, NIRI:
1. Computes the new `view_offset` so the active column is visible (fit or centered)
2. Animates `ViewOffset` from current to target
3. The view "snaps" to column boundaries when gestures end

### `Workspace` (workspace.rs, ~300 lines)

```rust
pub struct Workspace<W> {
    scrolling: ScrollingSpace<W>,
    floating: FloatingSpace<W>,
    floating_is_active: FloatingActive,
    original_output: OutputId,
    output: Option<Output>,
    scale: Scale,
    view_size: Size<f64, Logical>,
    working_area: Rectangle<f64, Logical>,
    shadow: Shadow,                  // For overview rendering
    background_buffer: SolidColorBuffer,
    clock: Clock,
    options: Rc<Options>,
    name: Option<String>,
    id: WorkspaceId,
}
```

### Overview/Expose Mode (monitor.rs + mod.rs, ~200 lines)

NIRI calls this **"overview"**:

- **Trigger**: 3-finger swipe down (or keybind)
- **State**: `overview_open: bool` + `overview_progress: Option<OverviewProgress>`
- **Animation**: `overview_progress` goes 0 → 1, `zoom = 1.0 / overview_scale_factor`
- **Layout**: All workspaces rendered as scaled-down thumbnails stacked vertically
- **Shadows**: Each workspace gets a `Shadow` element in overview mode
- **Render**: `RescaleRenderElement` + `RelocateRenderElement` composes each workspace
- **Interaction**: Click a workspace to focus it, scroll to switch while zoomed out

```rust
enum OverviewProgress {
    Animation(Animation),
    Gesture(OverviewGesture),
    Open,                           // fully open
}
```

### Workspace Switching (monitor.rs, ~200 lines)

```rust
enum WorkspaceSwitch {
    None,
    Animation { center_idx, current_idx, animation },
    Gesture { tracker, current_idx, is_touchpad, ... },
}
```

Workspaces are arranged vertically. Switching is animated (slide up/down).

---

## 3. What NIRI Does NOT Have (Important for heca)

| Feature | NIRI Status | heca Plans |
|---------|-------------|------------|
| Horizontal arrangement within a column | ❌ No | ✅ Phase 2 |
| Vertical scrolling within columns | ❌ No (tiles always fit height) | ✅ Phase 3 (optional) |
| Grid layouts | ❌ No | ✅ Future |
| Mixed layout modes in one column | ❌ No | ✅ Future |
| Multiple embedded Neovim instances | N/A (Wayland compositor) | ✅ Phase 2 |
| Embedded browser (CEF) | N/A | ✅ Phase 3 |
| GPU-rendered chrome (borders/tabs) | Partial (Smithay renders) | ✅ Phase 1 (wgpu) |

---

## 4. heca Layout Engine Implementation

### Files Created

| File | Purpose | Lines |
|------|---------|-------|
| `heca-core/src/layout/types.rs` | Shared types, enums, geometry | ~300 |
| `heca-core/src/layout/animation.rs` | Animation, SwipeTracker, easing | ~200 |
| `heca-core/src/layout/view_offset.rs` | ViewOffset, gesture handling | ~180 |
| `heca-core/src/layout/column.rs` | Column, Pane, height distribution | ~260 |
| `heca-core/src/layout/scrolling.rs` | ScrollingSpace, focus, add/remove | ~550 |
| `heca-core/src/layout/workspace.rs` | Workspace, floating panes | ~170 |
| `heca-core/src/layout/session.rs` | Session, overview, workspace switch | ~360 |
| `heca-core/src/layout.rs` | Module re-exports + architecture docs | ~60 |

### Architecture Summary

```
Session                          ← manages all workspaces + overview
├── workspaces: Vec<Workspace>   ← vertical stack
│   └── Workspace
│       ├── scrolling: ScrollingSpace   ← horizontal columns
│       │   ├── view_offset: ViewOffset ← animated horizontal scroll
│       │   ├── columns: Vec<Column>
│       │   │   ├── width: ColumnWidth
│       │   │   ├── panes: Vec<Pane>    ← vertical stack
│       │   │   └── pane_sizes: Vec<Size>  ← computed heights
│       │   └── active_column_idx
│       └── floating_panes: Vec<FloatingPane>
├── overview: OverviewState      ← zoom progress, open/closed
├── workspace_switch: WorkspaceSwitch  ← animated vertical transitions
└── active_workspace_idx
```

### Key Design Decisions

1. **`ViewOffset` abstraction preserved** — NIRI's elegant three-state scroll (Static/Animation/Gesture) is the core of smooth horizontal scrolling
2. **Column height distribution** — NIRI's algorithm: one fixed-height pane allowed per column, rest auto-weighted to fill available height
3. **Workspace stack** — Vertical arrangement matching NIRI's model. Discrete switches with animation (not continuous scroll)
4. **Overview as zoom** — Render all workspaces at reduced scale with gaps, like NIRI's `RescaleRenderElement`
5. **Pane is content-agnostic** — Layout engine never cares if content is terminal, neovim, or browser

---

## 5. Milestones & Phases

### Phase 0: Foundation (Complete)
- ✅ Read NIRI source code (scrolling.rs, workspace.rs, mod.rs, monitor.rs)
- ✅ Document actual NIRI architecture
- ✅ Implement core layout types (Column, ScrollingSpace, Workspace, Session)
- ✅ Implement animation system (Animation, SwipeTracker, easing)
- ✅ Implement ViewOffset (horizontal scroll with gesture support)

### Phase 1: Terminal Shell (Current)
**Goal**: A working GPU terminal with NIRI-style layout.

| Milestone | Description | Est. Effort |
|-----------|-------------|-------------|
| 1.1 | **Pane content abstraction** — Define `PaneBackend` trait for terminal, neovim, browser | Small |
| 1.2 | **Terminal backend** — Integrate `alacritty_terminal` VT parser | Medium |
| 1.3 | **Text renderer** — cosmic-text atlas-based rendering in wgpu | Medium |
| 1.4 | **Chrome rendering** — GPU borders, title bars, tab indicators | Medium |
| 1.5 | **Input routing** — Keyboard → pane, mouse → hit testing | Small |
| 1.6 | **Session persistence** — Save/restore workspace layouts to disk | Small |
| 1.7 | **Polish** — Gaps, animations, resize, config file | Medium |

**Deliverable**: `heca` binary opens as a GPU terminal. Multiple panes, columns, workspaces. Switch workspaces, overview mode. Session saved on quit.

### Phase 2: Neovim GUI
**Goal**: Embed Neovim as first-class panes, not just a terminal running nvim.

| Milestone | Description | Est. Effort |
|-----------|-------------|-------------|
| 2.1 | **msgpack-RPC client** — Connect to `nvim --embed` | Medium |
| 2.2 | **Grid protocol** — Parse redraw events (grid_line, grid_cursor_goto, etc.) | Medium |
| 2.3 | **Neovim backend** — Implement `PaneBackend` for Neovim grid | Medium |
| 2.4 | **Multi-nvim** — Multiple independent Neovim instances in different panes | Small |
| 2.5 | **LSP integration** — Shared LSP servers across nvim instances (future) | Large |

**Deliverable**: Open Neovim panes alongside terminal panes. Each pane is a real nvim instance with full GUI capabilities.

### Phase 3: Browser
**Goal**: Embedded Chromium via CEF offscreen rendering.

| Milestone | Description | Est. Effort |
|-----------|-------------|-------------|
| 3.1 | **CEF integration** — Offscreen rendering to shared texture | Large |
| 3.2 | **Browser backend** — Implement `PaneBackend` for CEF | Medium |
| 3.3 | **Input forwarding** — Mouse/keyboard → CEF | Small |
| 3.4 | **Multi-browser** — Multiple browser panes, shared CEF process | Medium |

**Deliverable**: Browser panes alongside terminals and nvim. Full web rendering.

### Phase 4: Advanced Layouts
**Goal**: Extend beyond NIRI's model.

| Milestone | Description | Est. Effort |
|-----------|-------------|-------------|
| 4.1 | **Horizontal column layout** — Panes side-by-side within a column | Medium |
| 4.2 | **Tabbed/stacked columns** — Only active pane visible with tab bar | Small |
| 4.3 | **Vertical column scrolling** — Scroll within tall columns | Medium |
| 4.4 | **Grid layout** — Rows × cols grid within a column | Medium |
| 4.5 | **Floating panes** — Free-floating windows alongside tiling | Medium |
| 4.6 | **Column groups** — Named groups of columns (like NIRI workspaces but horizontal) | Large |

### Phase 5: Polish & Ecosystem
**Goal**: Production-ready with plugins and ecosystem.

| Milestone | Description | Est. Effort |
|-----------|-------------|-------------|
| 5.1 | **Plugin system** — WASM or Lua plugins | Large |
| 5.2 | **Remote sessions** — Attach to remote heca sessions | Medium |
| 5.3 | **Collaboration** — Multiple users in one session | Large |
| 5.4 | **Mobile/tablet** — Touch-optimized interface | Large |

---

## 6. Next Steps (Immediate)

1. **Wire layout engine into renderer** — `Session` → render commands → wgpu
2. **Implement terminal backend** — `alacritty_terminal` + cosmic-text
3. **Implement chrome rendering** — borders, gaps, title bars in wgpu
4. **Input handling** — Route keyboard/mouse to active pane
5. **Test the layout** — Create multiple panes, columns, verify scrolling/switching

---

## Appendix: NIRI Source References

| File | Lines | What I Learned |
|------|-------|----------------|
| `src/layout/scrolling.rs` | ~600 | `ScrollingSpace`, `Column`, `ViewOffset`, `Tile`, horizontal scroll snapping, column animations |
| `src/layout/workspace.rs` | ~300 | `Workspace` contains both scrolling and floating, output management, shadow/background buffers |
| `src/layout/mod.rs` | ~900 | `Layout` (monitors + workspaces), `OverviewProgress`, `InteractiveMove`, `DndData`, workspace switching |
| `src/layout/monitor.rs` | ~700 | `Monitor` manages workspace stack, `workspaces_render_geo()`, overview zoom, workspace gestures |

---

*Document written after reading actual NIRI source code (commit at time of analysis).*
