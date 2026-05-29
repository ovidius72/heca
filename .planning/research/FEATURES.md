# Research: Features

## 2025-06-29

**Context:** heca is a *native GUI application* (not a TUI). It opens its own OS window, renders via GPU, and manages other applications as panes inside that window. This changes feature expectations compared to terminal multiplexers.

---

## Table Stakes (Must Have)

Without these, it's not a usable workspace.

| Feature | Why It's Table Stakes |
|---------|----------------------|
| Tiling pane layout (HSplit/VSplit) | Core value. Users expect splits like i3/tmux. |
| Floating panes | For dialogs, popups, tool windows. |
| Scratchpad panes | i3-style toggle. Essential for quick terminals/notes. |
| Keyboard navigation | HJKL focus, split creation, resize. Prefix or modifier-driven. |
| Mouse support | Click to focus, drag splits, drag floats, scroll. Desktop apps must support mouse. |
| Session persistence | Save layout and pane metadata; restore on launch. |
| Config file (`config.toml`) | Users expect to customize keybindings, fonts, theme. |
| Tab bar / workspace chrome | GUI apps need visual chrome. Tabs switch workspaces. |
| Terminal pane | PTY + VTE. The baseline pane type. |
| Neovim GUI pane | msgpack-RPC grid rendering. The reason many users will try this. |

## Differentiators (Competitive Advantage)

| Feature | Why It Differentiates |
|---------|----------------------|
| GPU-native text rendering (`cosmic-text` + `wgpu`) | Sharp fonts, ligatures, variable fonts, smooth animations. Terminal emulators can't match this easily. |
| Single-frame compositing | All panes + chrome render in one GPU pass. No window borders or tearing between panes. |
| JSON-RPC remote control | External scripts, CLIs, and AI agents can drive the workspace. |
| Plugin protocol for GUI apps | Not just terminal apps — any app that speaks the protocol can be a pane. |
| Predefined + savable layouts | Start a new project with one command and get your preferred layout instantly. |
| Session/window/tab/pane hierarchy | More structured than tmux (session→window→pane flat). Closer to a full desktop session manager. |
| Cross-platform from day one | Linux, macOS, Windows. Most native tiling tools are Linux-only. |

## Anti-Features (Deliberately NOT Building)

| Feature | Reason |
|---------|--------|
| TUI / terminal-drawing mode | This is a GUI app. No ANSI output, no terminal backend. |
| Remote SSH attach (v1) | Herdr does this well. We focus on local GPU experience first. |
| Full window manager / compositor for the OS | We are an app, not a Wayland/X11 compositor. We don't manage other apps' windows unless they explicitly opt-in via our protocol. |
| Plugin package manager / marketplace | Out of scope. Plugins are loaded from local paths. |
| Touch / gesture input | Keyboard + mouse only for v1. |
| Web-based UI (HTML/CSS/JS) | We render with GPU primitives, not a webview. |

## v2 Features (Deferred)

| Feature | Complexity | Notes |
|---------|------------|-------|
| Browser pane (Chromium + DevTools) | Very High | CEF offscreen rendering or window embedding. |
| Out-of-process plugin protocol | High | Socket-based, shared memory, or draw-command stream. |
| Real-time collaboration / shared sessions | High | Multiple clients viewing the same session. |
| Plugin marketplace | Medium | Distribution infrastructure. |
| Advanced animations (physics-based resize) | Medium | GPU makes this possible, but not v1-critical. |

## Feature Dependencies

```
GPU Shell (winit+wgpu+cosmic-text)
  └─► Tiling Layout Engine
        ├─► Terminal Pane (needs PTY + VTE)
        ├─► Neovim Pane (needs msgpack client + grid state)
        ├─► Custom Plugin Pane (needs App trait)
        └─► Chrome (tab bar, status, borders)
              └─► Config System
              └─► Theme System
  └─► Input Router (keyboard + mouse)
        └─► Config System (keybindings)
  └─► Session Persistence
  └─► JSON-RPC Server
```

---
*Key insight: Because this is a GUI app (not TUI), every pane renders as pixels in our GPU surface. There is no "terminal backend" — the app IS the compositor.*
