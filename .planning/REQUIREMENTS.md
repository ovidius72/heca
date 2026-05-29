# Requirements: heca

**Defined:** 2026-05-29
**Core Value:** A keyboard-native workspace where every tool lives in a tiled, floating, or scratchpad pane — all rendered in one GPU-accelerated window, fully restorable across sessions.

## v1 Requirements

### Compositor (COMP)

- [ ] **COMP-01**: App opens a native OS window via `winit` with a `wgpu` surface
- [ ] **COMP-02**: GPU renders colored rectangles, borders, and rounded corners
- [ ] **COMP-03**: `cosmic-text` renders text with a configurable font family and size
- [ ] **COMP-04**: Theme system loads from TOML (colors, fonts, border styles, shadows)
- [ ] **COMP-05**: Tab bar chrome rendered at the top of the window
- [ ] **COMP-06**: Pane borders and titles rendered around each pane
- [ ] **COMP-07**: Status bar or minimal chrome rendered at the bottom
- [ ] **COMP-08**: Command palette overlay (keyboard-triggered, fuzzy search)
- [ ] **COMP-09**: Damage tracking — only redraw regions that changed
- [ ] **COMP-10**: Cross-platform: Linux, macOS, Windows

### Layout (LAY)

- [ ] **LAY-01**: BSP tree tiling layout with horizontal and vertical splits
- [ ] **LAY-02**: Floating panes with absolute x/y, size, and z-index
- [ ] **LAY-03**: Scratchpad panes — hidden by default, toggled visible as floating
- [ ] **LAY-04**: Predefined layout templates (TOML) applied to a tab
- [ ] **LAY-05**: Save current tab layout as a named template
- [ ] **LAY-06**: Resize splits via keyboard (grow/shrink)
- [ ] **LAY-07**: Resize splits via mouse (drag borders)
- [ ] **LAY-08**: Move pane to another location in the tree
- [ ] **LAY-09**: Swap two panes
- [ ] **LAY-10**: Toggle pane between embedded (tiling) and floating
- [ ] **LAY-11**: Hide/show panes without killing them
- [ ] **LAY-12**: Empty leaf placeholders when a pane is moved/killed

### Input (INP)

- [ ] **INP-01**: Keyboard events captured globally before OS shortcuts intercept
- [ ] **INP-02**: Configurable prefix key or modifier for WM bindings
- [ ] **INP-03**: Keyboard focus navigation (HJKL or arrows)
- [ ] **INP-04**: Keyboard split creation (vertical/horizontal)
- [ ] **INP-05**: Keyboard pane operations (kill, float, scratchpad, hide)
- [ ] **INP-06**: Mouse click focuses a pane
- [ ] **INP-07**: Mouse drag on border resizes splits
- [ ] **INP-08**: Mouse drag on floating title bar moves the float
- [ ] **INP-09**: Mouse scroll forwarded to focused pane
- [ ] **INP-10**: Cursor icon changes on hover (resize, move, default)

### Pane Types (PANE)

- [ ] **PANE-01**: Terminal pane — spawns PTY, renders `alacritty_terminal` cell grid
- [ ] **PANE-02**: Terminal pane forwards keyboard input to PTY
- [ ] **PANE-03**: Terminal pane forwards mouse input as SGR mouse sequences
- [ ] **PANE-04**: Neovim pane — spawns `nvim --embed`, speaks msgpack-RPC
- [ ] **PANE-05**: Neovim pane renders grid updates from `redraw` events
- [ ] **PANE-06**: Neovim pane handles multiple grids (floating windows)
- [ ] **PANE-07**: Neovim pane forwards keyboard input as nvim input notation
- [ ] **PANE-08**: Neovim pane handles `ext_tabline` by updating chrome tab bar
- [ ] **PANE-09**: Neovim pane handles `ext_cmdline` / `ext_popupmenu` in chrome overlays
- [ ] **PANE-10**: Placeholder pane for empty slots

### Session Management (SESS)

- [ ] **SESS-01**: Named sessions stored on disk
- [ ] **SESS-02**: Session save captures all windows, tabs, layout trees, pane metadata
- [ ] **SESS-03**: Session restore respawns panes in saved cwd with saved app type
- [ ] **SESS-04**: Session restore rebuilds layout trees, float registry, scratchpad registry
- [ ] **SESS-05**: Auto-save on quit (configurable)
- [ ] **SESS-06**: Auto-restore last session on start (configurable)

### Config & Theming (CONF)

- [ ] **CONF-01**: Config loaded from user config dir (`config.toml`)
- [ ] **CONF-02**: Keybindings configurable in config
- [ ] **CONF-03**: Theme configurable (colors, fonts, border radius, shadows)
- [ ] **CONF-04**: Fallback defaults if config file missing or key omitted
- [ ] **CONF-05**: Hot-reload config without restart (optional v1 nice-to-have)

### RPC API (RPC)

- [ ] **RPC-01**: JSON-RPC server listens on Unix domain socket
- [ ] **RPC-02**: `session.create`, `session.list`, `session.load`, `session.save`, `session.close`
- [ ] **RPC-03**: `window.create`, `window.close`, `window.focus`, `window.list`
- [ ] **RPC-04**: `tab.create`, `tab.close`, `tab.focus`, `tab.rename`, `tab.list`
- [ ] **RPC-05**: `pane.create`, `pane.kill`, `pane.focus`, `pane.resize`, `pane.move`, `pane.swap`
- [ ] **RPC-06**: `pane.float`, `pane.embed`, `pane.scratchpad`, `pane.hide`, `pane.show`
- [ ] **RPC-07**: `layout.apply`, `layout.save_current`
- [ ] **RPC-08**: External CLI can connect to socket and invoke methods

### Plugin System (PLUG)

- [ ] **PLUG-01**: `App` trait defines pane lifecycle (init, resize, input, render, shutdown)
- [ ] **PLUG-02**: Terminal and Neovim are built-in implementations of `App`
- [ ] **PLUG-03**: Third-party plugins loaded dynamically via `libloading`
- [ ] **PLUG-04**: Plugin receives pane-local input events and RenderContext
- [ ] **PLUG-05**: Plugin can draw text and primitives via host's RenderContext

## v2 Requirements

### Browser (BROW)

- **BROW-01**: Browser pane renders a real Chromium engine (CEF offscreen)
- **BROW-02**: Browser pane supports toggling DevTools as a sub-pane or attached window
- **BROW-03**: Browser pane forwards mouse and keyboard input to CEF

### Out-of-Process Plugins (OOPP)

- **OOPP-01**: Plugin protocol over socket (draw-command stream or shared texture)
- **OOPP-02**: Host spawns plugin process and manages its lifecycle

### Advanced Features (ADV)

- **ADV-01**: Window embedding for arbitrary GUI apps (X11 reparenting, Windows SetParent, macOS NSView)
- **ADV-02**: Smooth animations (pane resize, tab switch, float toggle)
- **ADV-03**: Advanced font features (variable axes, stylistic sets)
- **ADV-04**: Multi-monitor support (move windows between monitors)

## Out of Scope

| Feature | Reason |
|---------|--------|
| TUI / terminal output mode | This is a native GUI app, not a terminal program |
| Remote attach over SSH | Herdr already does this well; focus on local GPU experience |
| OS-level window manager / compositor | We manage our own window, not the entire desktop |
| Touch / gesture input | Mouse and keyboard only for v1 |
| Plugin marketplace / package manager | Distribution infrastructure is not core |
| Web-based UI rendering | Chrome is GPU-native, not HTML/DOM |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| COMP-01 | Phase 1 | Pending |
| COMP-02 | Phase 1 | Pending |
| COMP-03 | Phase 1 | Pending |
| COMP-04 | Phase 1 | Pending |
| COMP-05 | Phase 2 | Pending |
| COMP-06 | Phase 2 | Pending |
| COMP-07 | Phase 2 | Pending |
| COMP-08 | Phase 5 | Pending |
| COMP-09 | Phase 5 | Pending |
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
| INP-01 | Phase 3 | Pending |
| INP-02 | Phase 3 | Pending |
| INP-03 | Phase 3 | Pending |
| INP-04 | Phase 3 | Pending |
| INP-05 | Phase 3 | Pending |
| INP-06 | Phase 3 | Pending |
| INP-07 | Phase 3 | Pending |
| INP-08 | Phase 3 | Pending |
| INP-09 | Phase 3 | Pending |
| INP-10 | Phase 3 | Pending |
| PANE-01 | Phase 4 | Pending |
| PANE-02 | Phase 4 | Pending |
| PANE-03 | Phase 4 | Pending |
| PANE-04 | Phase 4 | Pending |
| PANE-05 | Phase 4 | Pending |
| PANE-06 | Phase 4 | Pending |
| PANE-07 | Phase 4 | Pending |
| PANE-08 | Phase 5 | Pending |
| PANE-09 | Phase 5 | Pending |
| PANE-10 | Phase 2 | Pending |
| SESS-01 | Phase 6 | Pending |
| SESS-02 | Phase 6 | Pending |
| SESS-03 | Phase 6 | Pending |
| SESS-04 | Phase 6 | Pending |
| SESS-05 | Phase 6 | Pending |
| SESS-06 | Phase 6 | Pending |
| CONF-01 | Phase 1 | Pending |
| CONF-02 | Phase 3 | Pending |
| CONF-03 | Phase 1 | Pending |
| CONF-04 | Phase 1 | Pending |
| CONF-05 | Phase 5 | Pending |
| RPC-01 | Phase 7 | Pending |
| RPC-02 | Phase 7 | Pending |
| RPC-03 | Phase 7 | Pending |
| RPC-04 | Phase 7 | Pending |
| RPC-05 | Phase 7 | Pending |
| RPC-06 | Phase 7 | Pending |
| RPC-07 | Phase 7 | Pending |
| RPC-08 | Phase 7 | Pending |
| PLUG-01 | Phase 5 | Pending |
| PLUG-02 | Phase 4 | Pending |
| PLUG-03 | Phase 8 | Pending |
| PLUG-04 | Phase 5 | Pending |
| PLUG-05 | Phase 5 | Pending |

**Coverage:**
- v1 requirements: 55 total
- Mapped to phases: 55
- Unmapped: 0 ✓

---
*Requirements defined: 2026-05-29*
*Last updated: 2026-05-29 after initial definition*
