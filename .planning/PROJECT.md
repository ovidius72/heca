# heca

## What This Is

A native, GPU-accelerated tiling workspace compositor for developers. It combines a terminal multiplexer, a Neovim GUI, and (eventually) embeddable applications into a single keyboard-and-mouse-driven desktop environment. Sessions, windows, tabs, and panes are fully persistent and remotely controllable via JSON-RPC.

## Core Value

A keyboard-native workspace where every tool lives in a tiled, floating, or scratchpad pane — all rendered in one GPU-accelerated window, fully restorable across sessions.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Tiling pane layout engine with horizontal/vertical splits
- [ ] Floating panes with absolute positioning and z-index
- [ ] Scratchpad panes (i3-style hide/show toggle)
- [ ] Pane operations: create, kill, focus, resize, move, swap, float, embed, hide, show
- [ ] Predefined layout templates and save-current-as-layout
- [ ] Mouse support: focus, resize splits, drag floats, scroll
- [ ] Keyboard-first input routing with configurable keybindings
- [ ] Config loaded from user config dir (`config.toml`)
- [ ] Session persistence: save/restore to disk
- [ ] JSON-RPC API over Unix socket for external CRUD on sessions/windows/tabs/panes
- [ ] Terminal pane via PTY + `alacritty_terminal`
- [ ] Neovim GUI pane via msgpack-RPC (`--embed`)
- [ ] `App` trait plugin protocol for in-process plugins
- [ ] Cross-platform: Linux, macOS, Windows

### Out of Scope

- **Native browser pane (v1)** — Chromium embedding is deferred to v2. It requires platform-specific window reparenting or offscreen CEF integration, which is an order of magnitude harder than the core tiling compositor.
- **Out-of-process plugin protocol (v1)** — v1 plugins are in-process via `libloading`. Socket-based IPC plugins come later.
- **Wayland-native compositor** — We are an application, not a display server compositor.
- **Touch / gesture input** — Mouse and keyboard only for v1.
- **Plugin marketplace / package manager** — Not a distribution platform.

## Context

The user wants a real desktop app that looks graphical (fonts, styling, shadows) but is keyboard-oriented like tmux/i3. It must act as a multiplexer: sessions contain windows, windows contain tabs, tabs contain panes. Panes can be terminals, Neovim, or (later) any app that speaks the pane protocol.

The app should feel native, not like a terminal or webview. GPU text rendering via `cosmic-text` and `wgpu` is chosen to own the entire render loop and composite all panes in a single frame.

No deadline. Personal daily-driver first, then open-sourced.

## Constraints

- **Tech stack:** Rust. `winit` + `wgpu` + `cosmic-text` + `alacritty_terminal` + `tokio`.
- **Platforms:** Linux, macOS, Windows from v1.
- **No webview:** Rendering is GPU-native, not HTML/DOM-based.
- **Input:** Must support both keyboard (primary) and mouse (non-negotiable for desktop feel).
- **Plugin model:** In-process Rust trait for v1. Out-of-process protocol for v2.

## Key Decisions

| Decision | Rationale | Outcome |
| --- | --- | --- |
| `wgpu` + `cosmic-text` over Skia | Pure Rust, lighter dependency tree, modern GPU API | - Pending |
| `alacritty_terminal` over Kitty/Ghostty | Available as a Rust library crate; others are applications | - Pending |
| Browser deferred to v2 | CEF/offscreen Chromium is the highest-risk component; core must be solid first | - Pending |
| In-process plugins for v1 | Zero IPC overhead, simpler debugging, faster iteration | - Pending |
| JSON-RPC over Unix socket | Simple, debuggable, widely supported by CLI tools | - Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check - still the right priority?
3. Audit Out of Scope - reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-29 after initialization*
