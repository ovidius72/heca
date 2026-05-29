# Phase 2: The Workspace — Context

**Gathered:** 2026-05-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the real tiling layout engine, chrome (tab bar, sidebars, status bar), and input router (keyboard + mouse). This replaces the MockLayout from Phase 1 with a proper binary BSP tree that end users can actually split, resize, navigate, float, and scratchpad.

</domain>

<decisions>
## Implementation Decisions

### Layout Model
- **D-01:** **Binary BSP tree** (i3-style). Each split has exactly two children. No multi-child containers. Resize adjusts the ratio between the two children. Move swaps subtrees or detaches/attaches leaves.

### Input Prefix
- **D-02:** **Prefix key model** (tmux-style). Press a configurable prefix key (`Ctrl+B` by default) to enter WM command mode, then press a key to execute a WM action. The focused pane sees only normal keys unless the prefix is pressed.

### Chrome Layout
- **D-03:** **Tab bar at the top, status bar at the bottom, left/right sidebars collapsible** to icons. Standard desktop application layout.

### Pane State Transitions
- **D-04:** **Distinct states with dedicated keybinds.** Every pane is in exactly one state: Embedded, Floating, Scratchpad (hidden until toggled), or Hidden. Each transition has a keybind. No automatic state changes.

### Left Sidebar (Placeholder)
- **D-05:** Left sidebar renders as a **collapsible panel with a header ("Sessions") and an empty tree view placeholder.** Proves the chrome is working before Phase 4 populates it with real data.

### Status Bar Content
- **D-06:** Status bar shows: **pane count, focused pane title, current WM mode** (e.g., `3 panes | Editor | NORMAL`).

### Default Prefix Key
- **D-07:** Default prefix is **`Ctrl+B`** (tmux-compatible). Configurable in `config.toml`.

### the agent's Discretion
- Keybinding letter assignments (which keys do what after prefix is pressed)
- Exact BSP tree implementation (data structure details, collapse behavior)
- Tab bar style (minimal tabs with just names, or styled with close buttons)
- Sidebar width and icon set
- Status bar font size and exact layout
- Mouse hover cursor shapes
- Scroll speed for mouse wheel

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Requirements
- `.planning/REQUIREMENTS.md` — COMP-05 through COMP-07, COMP-11, COMP-12, LAY-01 through LAY-12, INP-01 through INP-10, CONF-02, PANE-10
- `.planning/ROADMAP.md` — Phase 2 success criteria and goal

### Existing Code
- `heca/src/main.rs` — Current event loop and rendering structure (will be refactored)
- `heca-core/src/types.rs` — `Rect` type (reused by layout engine)
- `heca-core/src/layout.rs` — `MockLayout` (to be replaced by `PaneTree`)
- `heca-renderer/src/primitive.rs` — GPU rect/border rendering (reused for chrome)
- `heca-renderer/src/text.rs` — Text rendering (reused for chrome labels)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `heca-core/src/types.rs` — `Rect { x, y, w, h }` — core geometry type, used by all layout computations
- `heca-renderer/src/primitive.rs` — `draw_rect`, `draw_border` methods — used for pane backgrounds, borders, and chrome elements
- `heca-renderer/src/text.rs` — `queue_text` method — used for pane titles, tab labels, sidebar text

### Established Patterns
- Event-driven redraw: app only renders when `needs_redraw` is set
- Single `wgpu` render pass per frame: all draw calls queued, then submitted together
- `heca-config` loads TOML with `serde` — this same pattern extends to layout templates

### Integration Points
- `heca/src/main.rs` line ~190: `mock_layout.compute_rects(...)` — this gets replaced by `PaneTree::compute_rects()`
- `heca/src/main.rs` line ~195: render loop draws panes from layout — this gets the actual pane tree
- `heca-config/src/theme.rs` — theme colors used by chrome styles (tab bar bg, status bar bg)

</code_context>

<specifics>
## Specific Ideas

- Prefix key dead key behavior: when prefix is pressed, next key is consumed as WM command. If an unbound key is pressed, just cancel (don't send to pane). If prefix is pressed again (Ctrl+B Ctrl+B), send literal prefix to pane.
- Sidebar collapse: pressing the toggle key collapses sidebar to a thin strip (just icons), pressing again expands full width.
- Active/inactive pane borders: active pane has accent-colored border, inactive panes have lower-opacity border. Matches Catppuccin Mocha theme.
</specifics>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope.
</deferred>

---

*Phase: 02-the-workspace*
*Context gathered: 2026-05-29*
