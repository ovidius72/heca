# Roadmap: heca

**Created:** 2026-05-29
**Granularity:** Coarse (4 phases)
**Mode:** Interactive

---

## Phase Overview

| # | Phase | Goal | Requirements | Success Criteria |
| --- | --- | --- | --- | --- |
| 1 | The Shell | Open a native window and render styled text and shapes via GPU | 7 | 4 |
| 2 | The Workspace | Tiling layout engine with full mouse and keyboard control | 28 | 7 |
| 3 | The Content | Terminal and Neovim render as first-class panes | 13 | 6 |
| 4 | The Platform | Session persistence, remote control, and dynamic plugins | 14 | 7 |

**Total:** 4 phases | 58 v1 requirements | 24 success criteria

---

## Phase 1: The Shell

**Goal:** Open a native OS window and render styled text, rectangles, and borders using `wgpu` and `cosmic-text`. Establish the config and theme system.

**Requirements:**
- COMP-01: App opens a native OS window via `winit` with a `wgpu` surface
- COMP-02: GPU renders colored rectangles, borders, and rounded corners
- COMP-03: `cosmic-text` renders text with a configurable font family and size
- COMP-04: Theme system loads from TOML (colors, fonts, border styles, shadows)
- COMP-10: Cross-platform: Linux, macOS, Windows
- CONF-01: Config loaded from user config dir (`config.toml`)
- CONF-03: Theme configurable (colors, fonts, border radius, shadows)
- CONF-04: Fallback defaults if config file missing or key omitted

**Success Criteria:**
1. Window opens successfully on Linux, macOS, and Windows.
2. Renders styled text in a custom font with theme-defined colors.
3. Renders colored rectangles with rounded corners and borders.
4. Loads config from user config directory; uses sensible fallbacks if missing.

**Dependencies:** None (foundation phase)

---

## Phase 2: The Workspace

**Goal:** Build the tiling layout engine, chrome, and input router. This is the core "window manager" experience.

**Requirements:**
- COMP-05: Tab bar chrome rendered at the top of the window
- COMP-06: Pane borders and titles rendered around each pane
- COMP-07: Status bar or minimal chrome rendered at the bottom
- LAY-01: BSP tree tiling layout with horizontal and vertical splits
- LAY-02: Floating panes with absolute x/y, size, and z-index
- LAY-03: Scratchpad panes — hidden by default, toggled visible as floating
- LAY-04: Predefined layout templates (TOML) applied to a tab
- LAY-05: Save current tab layout as a named template
- LAY-06: Resize splits via keyboard (grow/shrink)
- LAY-07: Resize splits via mouse (drag borders)
- LAY-08: Move pane to another location in the tree
- LAY-09: Swap two panes
- LAY-10: Toggle pane between embedded (tiling) and floating
- LAY-11: Hide/show panes without killing them
- LAY-12: Empty leaf placeholders when a pane is moved/killed
- INP-01: Keyboard events captured globally before OS shortcuts intercept
- INP-02: Configurable prefix key or modifier for WM bindings
- INP-03: Keyboard focus navigation (HJKL or arrows)
- INP-04: Keyboard split creation (vertical/horizontal)
- INP-05: Keyboard pane operations (kill, float, scratchpad, hide)
- INP-06: Mouse click focuses a pane
- INP-07: Mouse drag on border resizes splits
- INP-08: Mouse drag on floating title bar moves the float
- INP-09: Mouse scroll forwarded to focused pane
- INP-10: Cursor icon changes on hover (resize, move, default)
- CONF-02: Keybindings configurable in config
- COMP-11: Left sidebar chrome — collapsible panel for session/agent list
- COMP-12: Right sidebar chrome — collapsible panel for secondary information
- PANE-10: Placeholder pane for empty slots

**Success Criteria:**
1. Can create horizontal and vertical splits via keyboard shortcut.
2. Can navigate between panes using keyboard (HJKL or arrows).
3. Can resize splits via keyboard and by dragging borders with mouse.
4. Can toggle a pane to floating mode and drag it by its title bar.
5. Can toggle scratchpad panes on and off.
6. Tab bar renders at the top and clicking a tab switches the active workspace.
7. Pane borders and titles render with active/inactive styling.

**Dependencies:** Phase 1 (GPU Shell)

---

## Phase 3: The Content

**Goal:** Terminal and Neovim run as live panes inside the workspace. Define the `App` trait that all pane content implements.

**Requirements:**
- PANE-01: Terminal pane — spawns PTY, renders `alacritty_terminal` cell grid
- PANE-02: Terminal pane forwards keyboard input to PTY
- PANE-03: Terminal pane forwards mouse input as SGR mouse sequences
- PANE-04: Neovim pane — spawns `nvim --embed`, speaks msgpack-RPC
- PANE-05: Neovim pane renders grid updates from `redraw` events
- PANE-06: Neovim pane handles multiple grids (floating windows)
- PANE-07: Neovim pane forwards keyboard input as nvim input notation
- PLUG-01: `App` trait defines pane lifecycle (init, resize, input, render, shutdown)
- PLUG-02: Terminal and Neovim are built-in implementations of `App`
- PLUG-04: Plugin receives pane-local input events and RenderContext
- PLUG-05: Plugin can draw text and primitives via host's RenderContext

**Success Criteria:**
1. Terminal pane spawns a shell (e.g., zsh) and renders its output accurately.
2. Terminal pane accepts keyboard input and mouse scrolling.
3. Neovim pane spawns, renders the editor grid, and updates on edits.
4. Neovim floating windows (hover docs, diagnostics) render correctly over the base grid.
5. Keyboard modifiers (Ctrl, Alt, Shift) translate correctly into Neovim input.
6. `App` trait is stable enough that both Terminal and Neovim use it exclusively.

**Dependencies:** Phase 2 (The Workspace)

---

## Phase 4: The Platform

**Goal:** Make sessions persistent, expose remote control, and load third-party plugins.

**Requirements:**
- PANE-08: Neovim pane handles `ext_tabline` by updating chrome tab bar
- PANE-09: Neovim pane handles `ext_cmdline` / `ext_popupmenu` in chrome overlays
- SESS-01: Named sessions stored on disk
- SESS-02: Session save captures all windows, tabs, layout trees, pane metadata
- SESS-03: Session restore respawns panes in saved cwd with saved app type
- SESS-04: Session restore rebuilds layout trees, float registry, scratchpad registry
- SESS-05: Auto-save on quit (configurable)
- SESS-06: Auto-restore last session on start (configurable)
- RPC-01: JSON-RPC server listens on Unix domain socket
- RPC-02: `session.*` CRUD methods
- RPC-03: `window.*` CRUD methods
- RPC-04: `tab.*` CRUD methods
- RPC-05: `pane.*` CRUD methods
- RPC-06: `pane.float`, `pane.embed`, `pane.scratchpad`, etc.
- RPC-07: `layout.apply`, `layout.save_current`
- RPC-08: External CLI can connect to socket and invoke methods
- PLUG-03: Third-party plugins loaded dynamically via `libloading`
- SESS-07: Left sidebar displays list of all saved sessions
- COMP-08: Command palette overlay (keyboard-triggered, fuzzy search)
- COMP-09: Damage tracking — only redraw regions that changed
- CONF-05: Hot-reload config without restart

**Success Criteria:**
1. Neovim's tabline renders in the app's chrome tab bar instead of the terminal grid.
2. Command palette overlay opens via keyboard and fuzzy-searches commands.
3. Session saves to disk on quit and restores layout + pane metadata on launch.
4. JSON-RPC server accepts connections and executes CRUD commands.
5. External CLI can create, list, and kill panes via the socket.
6. A third-party plugin (e.g., a clock pane) loads dynamically at runtime.
7. Damage tracking reduces unnecessary redraws; idle CPU/GPU usage is minimal.

**Dependencies:** Phase 3 (The Content)

---

## Requirement Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| COMP-01 | Phase 1 | Pending |
| COMP-02 | Phase 1 | Pending |
| COMP-03 | Phase 1 | Pending |
| COMP-04 | Phase 1 | Pending |
| COMP-05 | Phase 2 | Pending |
| COMP-06 | Phase 2 | Pending |
| COMP-07 | Phase 2 | Pending |
| COMP-08 | Phase 4 | Pending |
| COMP-09 | Phase 4 | Pending |
| COMP-10 | Phase 1 | Pending |
| LAY-01 | Phase 2 | Pending |
| LAY-02 | Phase 2 | Pending |
| LAY-03 | Phase 2 | Pending |
| LAY-04 | Phase 2 | Pending |
| LAY-05 | Phase 2 | Pending |
| LAY-06 | Phase 2 | Pending |
| LAY-07 | Phase 2 | Pending |
| LAY-08 | Phase 2 | Pending |
| LAY-09 | Phase 2 | Pending |
| LAY-10 | Phase 2 | Pending |
| LAY-11 | Phase 2 | Pending |
| LAY-12 | Phase 2 | Pending |
| INP-01 | Phase 2 | Pending |
| INP-02 | Phase 2 | Pending |
| INP-03 | Phase 2 | Pending |
| INP-04 | Phase 2 | Pending |
| INP-05 | Phase 2 | Pending |
| INP-06 | Phase 2 | Pending |
| INP-07 | Phase 2 | Pending |
| INP-08 | Phase 2 | Pending |
| INP-09 | Phase 2 | Pending |
| INP-10 | Phase 2 | Pending |
| PANE-01 | Phase 3 | Pending |
| PANE-02 | Phase 3 | Pending |
| PANE-03 | Phase 3 | Pending |
| PANE-04 | Phase 3 | Pending |
| PANE-05 | Phase 3 | Pending |
| PANE-06 | Phase 3 | Pending |
| PANE-07 | Phase 3 | Pending |
| PANE-08 | Phase 4 | Pending |
| PANE-09 | Phase 4 | Pending |
| PANE-10 | Phase 2 | Pending |
| SESS-01 | Phase 4 | Pending |
| SESS-02 | Phase 4 | Pending |
| SESS-03 | Phase 4 | Pending |
| SESS-04 | Phase 4 | Pending |
| SESS-05 | Phase 4 | Pending |
| SESS-06 | Phase 4 | Pending |
| CONF-01 | Phase 1 | Pending |
| CONF-02 | Phase 2 | Pending |
| CONF-03 | Phase 1 | Pending |
| CONF-04 | Phase 1 | Pending |
| CONF-05 | Phase 4 | Pending |
| RPC-01 | Phase 4 | Pending |
| RPC-02 | Phase 4 | Pending |
| RPC-03 | Phase 4 | Pending |
| RPC-04 | Phase 4 | Pending |
| RPC-05 | Phase 4 | Pending |
| RPC-06 | Phase 4 | Pending |
| RPC-07 | Phase 4 | Pending |
| RPC-08 | Phase 4 | Pending |
| PLUG-01 | Phase 3 | Pending |
| PLUG-02 | Phase 3 | Pending |
| PLUG-03 | Phase 4 | Pending |
| PLUG-04 | Phase 3 | Pending |
| PLUG-05 | Phase 3 | Pending |

**Coverage:** 55 v1 requirements | 55 mapped | 0 unmapped ✓

---
*Roadmap created: 2026-05-29*
