# Research: Architecture

## 2026-05-29

**Context:** heca is a *native GUI application* — an OS window that acts as a tiling workspace manager with GPU-native rendering. It uses a NIRI-inspired scrolling-column layout engine rather than a traditional BSP tree.

---

## Major Components

### 1. Host (Main Binary)

The host is the only process that owns:
- The `winit` event loop
- The `wgpu` device and surface
- The `cosmic-text` font atlas and text renderer
- The global input router

Everything else is a pane runtime managed by the host.

### 2. Layout Engine (NIRI-inspired)

**Hierarchy:**

```
Session                          ← manages all workspaces + overview/expose mode
├── workspaces: Vec<Workspace>   ← arranged VERTICALLY (discrete switching)
│   └── Workspace
│       ├── scrolling: ScrollingSpace   ← horizontal COLUMNS (continuous scroll)
│       │   ├── view_offset: ViewOffset ← animated horizontal scroll + snap
│       │   ├── columns: Vec<Column>
│       │   │   ├── width: ColumnWidth  ← Proportion | Fixed
│       │   │   ├── panes: Vec<Pane>    ← vertical stack within column
│       │   │   └── pane_sizes: Vec<Size>  ← computed heights
│       │   └── active_column_idx
│       └── floating_panes: Vec<FloatingPane>
├── overview: OverviewState      ← zoom progress, open/closed
├── workspace_switch: WorkspaceSwitch  ← animated vertical transitions
└── active_workspace_idx
```

**Key insight — scrolling model (two axes, two mechanisms):** Columns scroll horizontally as a continuous strip. Focus change animates the view to snap the active column into view. Workspaces are a vertical stack with animated discrete switching.

**Scrolling model:**

| Axis | Container | Scroll Type | Mechanism |
|------|-----------|-------------|-----------|
| **Horizontal** | `ScrollingSpace.columns` | Continuous scroll + snap | `ViewOffset` (animated `f64`) |
| **Vertical** | `Session.workspaces` | Discrete switch + animation | `WorkspaceSwitch` (animated index) |

**Operations:**
- **Focus left/right** — activate previous/next column, animate `ViewOffset`
- **Focus up/down** — activate previous/next pane in column, or switch workspace
- **Add pane** — new column (horizontal) or new pane in column (vertical)
- **Remove pane** — remove from column, remove column if empty
- **Move column** — reorder columns with animation
- **Overview** — zoom out to see all workspaces as thumbnails

### 3. Compositor / Renderer

Runs every frame:
1. **Layout pass** — Compute pixel rectangles for all panes from `Session.workspace_geometries()` and `ScrollingSpace.panes_with_positions()`
2. **Content update** — Ask each visible pane's backend for latest content
   - Text-grid panes (terminal): update cell buffers
   - Future: Texture panes (browser), draw-command panes (plugins)
3. **Chrome draw** — Tab bar, pane borders/titles, status bar, sidebars
4. **Composite** — Single `wgpu` render pass:
   - Clear background
   - Draw workspace content (panes with borders)
   - Draw floating panes (future)
   - Draw global overlays (overview, command palette)
5. **Present** — swapchain present

### 4. Pane Runtime (`PaneBackend` Trait)

Every pane content source implements:

```rust
pub trait PaneBackend: Send {
    fn pane_type(&self) -> PaneType;
    fn title(&self) -> &str;
    fn set_size(&mut self, cols: usize, rows: usize);
    fn process_input(&mut self, data: &[u8]);
    fn update(&mut self) -> bool;  // returns true if new data arrived
    fn render_data(&self) -> BackendRenderData;
    fn should_close(&self) -> bool;
}
```

**Built-in implementations:**
- `TerminalBackend`: PTY + `vte` parser → produces cell grid
- Future: `NeovimBackend`: msgpack-RPC → produces cell grid + chrome events
- Future: `BrowserBackend`: CEF offscreen → produces GPU texture

**BackendRenderData:**
```rust
pub enum BackendRenderData {
    Terminal {
        lines: Vec<TerminalLine>,
        cursor_col: usize,
        cursor_row: usize,
    },
    // Future: Neovim { ... }, Browser { texture_id }
}
```

### 5. Input Router

**Keyboard:**
- Global keymap resolves chords against WM bindings first.
- If no match, forward to focused pane's backend.
- WM bindings use a prefix key (e.g., `Ctrl+B`) then action key.

**Navigation (NIRI-style):**
- `h`/`l` — focus left/right column (with animated scroll)
- `j`/`k` — focus down/up pane in column (or switch workspace at boundaries)
- `-` — split horizontal (new column to the right)
- `v` — split vertical (new pane in current column)

**Mouse:**
- Hit-test against current frame pane rects from `pane_under()`
- Click pane → focus it
- Chrome clicks → tab switch, sidebar toggle

### 6. Session Persistence

**What is saved:**
- Session metadata (name, created date)
- Workspace list with column/pane structure
- Pane metadata: backend type, spawn command, CWD
- Layout options (gaps, column widths, etc.)

**What is NOT saved:**
- PTY scrollback (too large)
- Neovim buffer content (nvim has `:mksession` for this)
- GPU textures

**Restore flow:**
1. Deserialize session file.
2. Create winit window.
3. Rebuild `Session` with workspaces, columns, panes.
4. Respawn each pane's backend in its saved CWD.

---

## Data Flow

```
Terminal PTY ──bytes──► TerminalBackend ──cell grid──► Compositor
Neovim Process ──msgpack──► NeovimBackend ──cell grid──► Compositor
                                        ▲
                                        │
Input Router ──events──► Focused Pane ──┘
                                        │
RPC Server ──commands──► Session ──mutations──► Layout Engine
```

---

## Build Order

1. **GPU Shell** — winit + wgpu + cosmic-text. Prove we can render text and rects.
2. **Layout Engine** — NIRI-style scrolling columns, animations, overview mode.
3. **Terminal** — PTY + vte parser. First real pane content.
4. **Neovim** — msgpack client + grid state.
5. **Input Router** — Full keyboard + mouse, keybindings, hit-testing.
6. **Session Persistence** — Save/restore.
7. **RPC Server** — External control.
8. **Browser / Out-of-process** — v2.

---

## Files

| Component | Path |
|-----------|------|
| Layout types | `heca-core/src/layout/types.rs` |
| Animation system | `heca-core/src/layout/animation.rs` |
| ViewOffset (scroll) | `heca-core/src/layout/view_offset.rs` |
| Column / Pane | `heca-core/src/layout/column.rs` |
| ScrollingSpace | `heca-core/src/layout/scrolling.rs` |
| Workspace | `heca-core/src/layout/workspace.rs` |
| Session / Overview | `heca-core/src/layout/session.rs` |
| PaneBackend trait | `heca-core/src/backend/mod.rs` |
| Terminal backend | `heca-core/src/backend/terminal.rs` |
| Renderer | `heca-renderer/src/lib.rs` |
| App / Event loop | `heca/src/main.rs` |

---

*Key differences: Unlike TUI multiplexers (herdr, tmux, zellij), we do not stream ANSI to a terminal — we composite pixels in a GPU surface. Unlike traditional tiling WMs (i3, sway), we use horizontal scrolling columns rather than fixed-grid trees. Like NIRI, our layout is a scrollable-tiling model where columns overflow the viewport and the view offset animates to reveal them.*
