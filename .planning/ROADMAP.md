# Roadmap: heca

**Created:** 2026-05-29
**Granularity:** Coarse (4 phases)
**Mode:** Interactive

---

## Phase Overview

| # | Phase | Goal | Requirements | Success Criteria |
| --- | --- | --- | --- | --- |
| 1 | The Shell | Open a native window and render styled text and shapes via GPU | 7 | 4 |
| 2 | The Workspace | NIRI-inspired layout engine with scrolling columns, workspaces, and overview | 28 | 7 |
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

## Phase 2: The Workspace (NIRI Layout)

**Goal:** Build the NIRI-inspired layout engine with horizontal scrolling columns, vertical workspace stacks, animated transitions, and overview/expose mode.

**Requirements:**

- COMP-05: Tab bar chrome rendered at the top of the window
- COMP-06: Pane borders and titles rendered around each pane
- COMP-07: Status bar or minimal chrome rendered at the bottom
- COMP-11: Left sidebar chrome — collapsible panel for session/agent list
- COMP-12: Right sidebar chrome — collapsible panel for secondary information
- LAY-01: NIRI-style scrolling columns — horizontal arrangement with animated scroll
- LAY-02: Column width modes — Proportion and Fixed width columns
- LAY-03: Vertical pane stacks within columns — height distribution algorithm
- LAY-04: Workspace stack — vertical arrangement with animated switching
- LAY-05: Overview/expose mode — zoom out to see all workspaces as thumbnails
- LAY-06: Focus left/right — activate column with animated view scroll
- LAY-07: Focus up/down — activate pane in column or switch workspace
- LAY-08: Add column (horizontal split) — new column to the right of active
- LAY-09: Add pane to column (vertical split) — new pane in current column
- LAY-10: Remove pane/column — remove active pane, remove column if empty
- LAY-11: Move column left/right — reorder columns with animation
- LAY-12: ViewOffset scroll engine — Static, Animation, and Gesture states
- INP-01: Keyboard events captured globally before OS shortcuts intercept
- INP-02: Configurable prefix key or modifier for WM bindings
- INP-03: Keyboard focus navigation (HJKL)
- INP-04: Keyboard split creation (new column / new pane in column)
- INP-05: Keyboard pane operations (kill, overview toggle)
- INP-06: Mouse click focuses a pane
- CONF-02: Keybindings configurable in config
- ANIM-01: Smooth column scroll animations with easing
- ANIM-02: Workspace switch animations (vertical slide)
- ANIM-03: Overview zoom animation
- ANIM-04: Column move animations (slide when columns added/removed)

**Success Criteria:**

1. Can create new columns (horizontal split) and new panes in columns (vertical split).
2. Can navigate between columns with H/L — view scrolls smoothly to active column.
3. Can navigate between panes with J/K — wraps to next/prev column at boundaries.
4. Can toggle overview mode to see all workspaces as scaled thumbnails.
5. Can switch workspaces with animated vertical transitions.
6. Tab bar renders at the top and clicking a tab switches the active workspace.
7. Pane borders and titles render with active/inactive styling.

**Dependencies:** Phase 1 (GPU Shell)

---

## Phase 3: The Content

**Goal:** Terminal and Neovim run as live panes inside the workspace.

**Requirements:**

- PANE-01: Terminal pane — spawns PTY, renders cell grid via `vte` parser
- PANE-02: Terminal pane forwards keyboard input to PTY
- PANE-03: Terminal pane forwards mouse input as SGR mouse sequences
- PANE-04: Neovim pane — spawns `nvim --embed`, speaks msgpack-RPC
- PANE-05: Neovim pane renders grid updates from `redraw` events
- PANE-06: Neovim pane handles multiple grids (floating windows)
- PANE-07: Neovim pane forwards keyboard input as nvim input notation
- PLUG-01: `PaneBackend` trait defines pane lifecycle (init, resize, input, render, shutdown)
- PLUG-02: Terminal and Neovim are built-in implementations of `PaneBackend`
- PLUG-04: Plugin receives pane-local input events and RenderContext
- PLUG-05: Plugin can draw text and primitives via host's RenderContext

**Success Criteria:**

1. Terminal pane spawns a shell (e.g., zsh) and renders its output accurately.
2. Terminal pane accepts keyboard input and mouse scrolling.
3. Neovim pane spawns, renders the editor grid, and updates on edits.
4. Neovim floating windows (hover docs, diagnostics) render correctly over the base grid.
5. Keyboard modifiers (Ctrl, Alt, Shift) translate correctly into Neovim input.
6. `PaneBackend` trait is stable enough that both Terminal and Neovim use it exclusively.

**Dependencies:** Phase 2 (The Workspace)

---

## Phase 4: The Platform

**Goal:** Make sessions persistent, expose remote control, and load third-party plugins.

**Requirements:**

- PANE-08: Neovim pane handles `ext_tabline` by updating chrome tab bar
- PANE-09: Neovim pane handles `ext_cmdline` / `ext_popupmenu` in chrome overlays
- SESS-01: Named sessions stored on disk
- SESS-02: Session save captures all workspaces, columns, panes, pane metadata
- SESS-03: Session restore respawns panes in saved cwd with saved app type
- SESS-04: Session restore rebuilds Session with workspaces, columns, panes
- SESS-05: Auto-save on quit (configurable)
- SESS-06: Auto-restore last session on start (configurable)
- RPC-01: JSON-RPC server listens on Unix domain socket
- RPC-02: `session.*` CRUD methods
- RPC-03: `window.*` CRUD methods
- RPC-04: `workspace.*` CRUD methods
- RPC-05: `pane.*` CRUD methods
- RPC-06: `pane.float`, `pane.embed`, etc.
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
| COMP-01 | Phase 1 | Done |
| COMP-02 | Phase 1 | Done |
| COMP-03 | Phase 1 | Done |
| COMP-04 | Phase 1 | Done |
| COMP-05 | Phase 2 | Done |
| COMP-06 | Phase 2 | Done |
| COMP-07 | Phase 2 | Done |
| COMP-08 | Phase 4 | Pending |
| COMP-09 | Phase 4 | Pending |
| COMP-10 | Phase 1 | Done |
| LAY-01 | Phase 2 | Done |
| LAY-02 | Phase 2 | Done |
| LAY-03 | Phase 2 | Done |
| LAY-04 | Phase 2 | Done |
| LAY-05 | Phase 2 | Done |
| LAY-06 | Phase 2 | Done |
| LAY-07 | Phase 2 | Done |
| LAY-08 | Phase 2 | Done |
| LAY-09 | Phase 2 | Done |
| LAY-10 | Phase 2 | Done |
| LAY-11 | Phase 2 | Done |
| LAY-12 | Phase 2 | Done |
| ANIM-01 | Phase 2 | Done |
| ANIM-02 | Phase 2 | Done |
| ANIM-03 | Phase 2 | Done |
| ANIM-04 | Phase 2 | Done |
| INP-01 | Phase 2 | Done |
| INP-02 | Phase 2 | Done |
| INP-03 | Phase 2 | Done |
| INP-04 | Phase 2 | Done |
| INP-05 | Phase 2 | Done |
| INP-06 | Phase 2 | Pending |
| CONF-01 | Phase 1 | Done |
| CONF-02 | Phase 2 | Done |
| CONF-03 | Phase 1 | Done |
| CONF-04 | Phase 1 | Done |
| CONF-05 | Phase 4 | Pending |
| PANE-01 | Phase 3 | Done |
| PANE-02 | Phase 3 | Done |
| PANE-03 | Phase 3 | Pending |
| PANE-04 | Phase 3 | Pending |
| PANE-05 | Phase 3 | Pending |
| PANE-06 | Phase 3 | Pending |
| PANE-07 | Phase 3 | Pending |
| PANE-08 | Phase 4 | Pending |
| PANE-09 | Phase 4 | Pending |
| PLUG-01 | Phase 3 | Done |
| PLUG-02 | Phase 3 | Done |
| PLUG-04 | Phase 3 | Pending |
| PLUG-05 | Phase 3 | Pending |
| SESS-01 | Phase 4 | Pending |
| SESS-02 | Phase 4 | Pending |
| SESS-03 | Phase 4 | Pending |
| SESS-04 | Phase 4 | Pending |
| SESS-05 | Phase 4 | Pending |
| SESS-06 | Phase 4 | Pending |
| SESS-07 | Phase 4 | Pending |
| RPC-01 | Phase 4 | Pending |
| RPC-02 | Phase 4 | Pending |
| RPC-03 | Phase 4 | Pending |
| RPC-04 | Phase 4 | Pending |
| RPC-05 | Phase 4 | Pending |
| RPC-06 | Phase 4 | Pending |
| RPC-07 | Phase 4 | Pending |
| RPC-08 | Phase 4 | Pending |
| PLUG-03 | Phase 4 | Pending |

**Coverage:** 58 v1 requirements | 58 mapped | 0 unmapped ✓

---
*Roadmap updated: 2026-06-09*

*Phase 9 (Focus Separation) completed: interaction policy layer with 29 regression tests.*

---

## Code Refactoring Track (cross-cutting)

A 10-phase code quality initiative running alongside the product phases, cleaning
up architecture debt introduced during rapid prototyping.

| Phase | Status | Focus |
|-------|--------|-------|
| 1 — File Reorganization | ✅ Done | Split monolithic files (main.rs, mouse.rs, sidebar.rs, config) |
| 2 — Central Mutation Boundary | ✅ Done | after_layout_change hook, mutation helpers |
| 3 — Shared Pane Ops | ✅ Done | pane_ops.rs, DnD refactoring, ad hoc scan removal |
| 4 — Sidebar Projection Model | ✅ Done | SidebarItemKind, rebuild→sync, HashMap pane lookup |
| 5 — Backend Lifecycle | ✅ Done | BackendStore wrapper, lifecycle contract, batch helper |
| 6 — Stale State | ✅ Done | Remove dead Rect, dormant fields, placeholder variants, metadata drift, #[allow] audit |
| 7 — Typed Errors | ✅ Done | ConfigError, PtyError, RpcError, debug-assert dispatch, SAFETY comments |
| 8 — Constants & Polish | ✅ Done | Centralize constants, renderer API doc, invariant docs (8.3 deferred) |
| 9 — Focus Separation | ✅ Done | Interaction policy layer: FocusDomain, ActionPolicy, route_interaction, dispatch_action, focus-target helpers, 29 regression tests |
| 10 — Final Verification | ⬜ Pending | Full workspace validation, smoke tests, doc reconciliation |
