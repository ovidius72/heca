# Research Summary: heca

## 2025-06-29

---

## Stack Recommendation

**Core:** `winit` + `wgpu` + `cosmic-text` + `tokio`
**Terminal:** `portable-pty` + `alacritty_terminal`
**Neovim:** `rmpv` + `tokio` msgpack-RPC client
**Config:** TOML via `serde`, paths via `dirs`
**Plugins (v1):** `libloading` + Rust `App` trait
**RPC:** JSON-RPC 2.0 over Unix domain socket
**Browser (v2):** CEF offscreen (`cef-rs` / `wef`) or OS window reparenting

**Avoid:** WebView frameworks (Dioxus, Tauri, Electron), GTK/Qt, game engines (Bevy), immediate-mode UI (egui).

---

## Table Stakes

1. Tiling pane layout (HSplit/VSplit/BSP tree)
2. Floating panes with z-index
3. Scratchpad panes (i3-style toggle)
4. Full mouse support (focus, resize, drag, scroll)
5. Keyboard-first navigation with configurable keybindings
6. Config via `config.toml`
7. Session persistence (save/restore)
8. Terminal pane (PTY + VTE)
9. Neovim GUI pane (msgpack grid)
10. Cross-platform: Linux, macOS, Windows

## Differentiators

1. **GPU-native text** — `cosmic-text` + `wgpu` gives sharp fonts, ligatures, and smooth compositing.
2. **Unified compositor** — All panes + chrome render in a single GPU frame. No window seams.
3. **JSON-RPC control** — External tools can CRUD sessions, windows, tabs, panes.
4. **Plugin protocol for GUI apps** — Apps opt-in by speaking the pane protocol.
5. **Predefined layouts** — One command to spawn a full workspace layout.

## Out of Scope (v1)

- Browser pane (CEF too complex; deferred to v2)
- Out-of-process plugins (v1 is in-process only)
- OS-level window manager behavior
- Touch/gesture input
- Remote attach over SSH
- Plugin marketplace

## Architecture Pattern

**Host owns the GPU.** The host is the only process with `winit`, `wgpu`, and `cosmic-text`. All panes are plugins implementing an `App` trait. The host composites their output into a single frame.

**Herdr inspiration:** Borrow the client/server state split, BSP layout, socket API namespacing, and session persistence model — but replace the TUI renderer with a GPU compositor.

## Top Risks

1. **Browser embedding** — The hardest v2 feature. Requires CEF or OS window reparenting.
2. **Keyboard capture** — OS shortcuts (Ctrl+W, Alt+F4, Cmd+Q) must be intercepted.
3. **Neovim multi-grid** — Floating windows use separate grids. Easy to miss.
4. **Performance at idle** — Must damage-track and avoid redrawing unchanged regions.
5. **Wayland window embedding** — Wayland does not support arbitrary window reparenting.

---
*Next: Define v1 requirements and create roadmap.*
