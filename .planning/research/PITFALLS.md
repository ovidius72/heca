# Research: Pitfalls

## 2025-06-29

**Domain:** Native GUI tiling compositor with GPU text rendering, Neovim integration, and embeddable panes.

---

## Critical Pitfalls

### 1. Treating This Like a TUI App

**Warning sign:** Using ANSI escape sequences, terminal colors, or assuming a monospace grid for chrome.
**Prevention:** The app is a GUI. Chrome renders with GPU primitives and `cosmic-text`. Pane content may be a cell grid (term, nvim), but the app chrome is free-form.
**Phase to address:** Milestone 1 (GPU Shell).

### 2. WebView or DOM-Based Rendering for Chrome

**Warning sign:** Pulling in a WebView to render the tab bar or command palette.
**Prevention:** This breaks the unified compositor model. Chrome must render via `wgpu` primitives or text batches in the same frame as pane content.
**Phase to address:** Milestone 1.

### 3. Keyboard Input Being Intercepted by OS or winit

**Warning sign:** `Ctrl+W` closes the window. `Alt+F4` quits. `Cmd+Q` quits on macOS.
**Prevention:** Aggressive `prevent_default` on all key events in the winit event loop. Platform-specific key capture. Test modifier combinations thoroughly on each OS.
**Phase to address:** Milestone 5 (Input Router).

### 4. Mouse Hit-Testing Drifting from Layout

**Warning sign:** Clicking a pane focuses the wrong one. Resizing via mouse doesn't match visual borders.
**Prevention:** The hit-test must use the EXACT same rects computed for rendering. Share the layout output between render and input code. Do not recompute.
**Phase to address:** Milestone 2 (Layout) and 5 (Input).

### 5. Assuming Neovim Grid = Single Grid

**Warning sign:** Floating windows in Neovim (hover docs, code actions) don't render or render in the wrong place.
**Prevention:** Neovim's UI protocol uses MULTIPLE grids. Grid 1 is the base. Other grids are floating windows with absolute positions relative to the base. You must maintain a grid registry and composite all grids.
**Phase to address:** Milestone 4 (Neovim Pane).

### 6. Session Restore Breaking Pane State

**Warning sign:** After restore, panes are empty shells. Neovim lost buffers. Terminal lost CWD.
**Prevention:** Save `cwd`, spawn command, and app type. Do NOT save PTY state or nvim buffers. For nvim, optionally integrate with `:mksession` so nvim restores its own internal state.
**Phase to address:** Milestone 6 (Session Persistence).

### 7. CEF / Browser Plugin Blocking v1

**Warning sign:** Spending months trying to embed Chromium before the core works.
**Prevention:** Browser is explicitly v2. Do not touch CEF, webviews, or window reparenting until terminal + nvim + tiling are rock solid.
**Phase to address:** Project scope (Out of Scope v1).

### 8. Plugin ABI Instability

**Warning sign:** Updating Rust compiler or dependencies breaks all plugins.
**Prevention:** The `App` trait must use `#[repr(C)]` or a stable C-compatible interface for v1 in-process plugins. Better yet: use a pure trait with `Box<dyn App>` but document that plugins must be compiled with the same Rust version and dependency tree. For true stability, v2's out-of-process protocol is the answer.
**Phase to address:** Milestone 8 (Plugin SDK).

### 9. Wayland Window Embedding Complexity

**Warning sign:** "Window reparenting works on X11 and macOS but is impossible on Wayland."
**Prevention:** Wayland does not allow arbitrary window reparenting. For Linux, prefer X11 for window-embedding plugins, or use xdg-shell surfaces with tight coordination. Document this limitation.
**Phase to address:** v2 Browser/Window embedding.

### 10. Performance: Redrawing Everything Every Frame

**Warning sign:** 100% GPU usage at idle. Fans spin up.
**Prevention:**
- Only redraw when content changes (damage tracking).
- Use `wgpu` surface presentation modes efficiently.
- Cache text atlas. `cosmic-text` does this.
- Do not re-upload cell buffers unless cells changed.
**Phase to address:** All milestones. Profile early.

---

## Medium Pitfalls

| Pitfall | Warning Sign | Prevention | Phase |
|---------|-------------|------------|-------|
| Config parsing panics on bad TOML | Invalid config crashes app | Use `serde` with fallback defaults. Validate after load. | Milestone 1 |
| Font loading fails silently | No text renders | Fallback font stack (system monospace → bundled). | Milestone 1 |
| HiDPI scaling bugs | Text blurry or misaligned | Use winit's scale factor everywhere. | Milestone 1 |
| Zombie PTY processes | Processes keep running after pane close | Explicit PTY close + SIGTERM/SIGKILL. | Milestone 3 |
| Nvim msgpack out of sync | Events desync, grid corruption | Strict event loop ordering. One tokio task reads, one writes. | Milestone 4 |
| RPC socket collisions | Multiple sessions conflict | Namespace sockets by session name + PID. | Milestone 7 |

---
*Key watch: The gap between "GUI app" and "terminal multiplexer" is where most architecture mistakes happen. If you find yourself thinking about ANSI sequences or terminal backends, stop and reorient.*
