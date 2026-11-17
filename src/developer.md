# Developer Guide

## Architecture

```
┌─────────────────────────────────────────┐
│  heca (main binary)                     │
│  ├── winit event loop                   │
│  ├── input routing (prefix/keymap)      │
│  ├── ActionRegistry dispatch            │
│  ├── render() — GPU compositor          │
│  └── chrome (tab bar, sidebars, status) │
├─────────────────────────────────────────┤
│  heca-renderer                          │
│  ├── PrimitiveRenderer (rects, borders) │
│  └── TextRenderer (cosmic-text atlas)   │
├─────────────────────────────────────────┤
│  heca-core                              │
│  ├── layout/                            │
│  │   ├── Session → Workspace → Column → Pane
│  │   ├── ViewOffset (animated scroll)   │
│  │   └── Animation (easing, springs)    │
│  └── backend/                           │
│      ├── PaneBackend trait              │
│      ├── TerminalBackend (PTY+wezterm)  │
│      └── FakeBackend (layout testing)   │
├─────────────────────────────────────────┤
│  heca-config                            │
│  └── TOML config + theme loading        │
└─────────────────────────────────────────┘
```

## Project Structure

```
myvim/
├── AGENTS.md              ← This file
├── README.md              ← Landing page: what heca is, install, first run, link to docs/
├── config.default.toml        ← Default settings/appearance/font/program (embedded)
├── keybindings.default.toml   ← Default keybindings (embedded)
├── Cargo.toml             ← Workspace root
├── heca/                  ← Main binary (event loop, app state, rendering)
│   ├── src/
│   │   ├── main.rs        ← HecaApp, ApplicationHandler, render(), registry setup
│   │   ├── app_state.rs   ← AppState, InputMode, DragState, SidebarState
│   │   ├── input.rs       ← WmAction enum, action_from_name(), action_priority()
│   │   ├── keymap.rs      ← KeymapRegistry, KeyCombo, event_combo_matches()
│   │   ├── actions.rs     ← ActionRegistry, ActionDescriptor, ActionCategory
│   │   ├── handlers.rs    ← All action handlers (handle_focus_pane, handle_swap, etc.)
│   │   ├── sidebar.rs     ← SidebarTree, rendering, hit-testing, navigation
│   │   └── chrome.rs      ← ChromeConfig (tab bar, sidebar, status bar)
│   └── Cargo.toml
├── heca-core/             ← Layout engine + backends (no GPU code)
│   ├── src/
│   │   ├── layout/
│   │   │   ├── mod.rs     ← Module re-exports
│   │   │   ├── types.rs   ← Shared geometry types (Point, Size, Rectangle, ColumnWidth, etc.)
│   │   │   ├── animation.rs  ← Animation, SwipeTracker, easing functions
│   │   │   ├── view_offset.rs  ← ViewOffset (Static/Animation/Gesture)
│   │   │   ├── column.rs  ← Column, Pane, height distribution
│   │   │   ├── scrolling.rs  ← ScrollingSpace, focus, add/remove, view positions
│   │   │   ├── workspace.rs  ← Workspace, FloatingPane
│   │   │   └── session.rs ← Session, OverviewState, WorkspaceSwitch
│   │   ├── backend/
│   │   │   ├── mod.rs     ← PaneBackend trait, BackendRenderData
│   │   │   ├── terminal/    ← TerminalBackend (PTY + terminal emulation; see docs/architecture.md)
│   │   │   └── fake.rs    ← FakeBackend (no PTY, for layout testing)
│   │   ├── pane.rs        ← DEPRECATED old BSP tree code (kept for reference)
│   │   └── types.rs       ← Old Rect type (for BSP legacy)
│   └── Cargo.toml
├── heca-renderer/          ← GPU rendering (wgpu)
│   ├── src/
│   │   ├── lib.rs         ← Renderer init
│   │   ├── primitive.rs   ← PrimitiveRenderer (rects, borders)
│   │   └── text.rs        ← TextRenderer (cosmic-text atlas)
│   └── Cargo.toml
├── heca-config/            ← Configuration loading
│   ├── src/
│   │   ├── lib.rs         ← Module exports
│   │   └── theme.rs       ← Config, AppConfig, Theme, keybinding defaults
│   └── Cargo.toml
├── docs/
│   └── niri-wiki/          ← NIRI documentation (structured wiki reference)
├── .planner/               ← Planner state: features, phases, tasks, decisions (`/planner load`)
├── .agents/
│   └── skills/
│       └── niri/SKILL.md   ← NIRI knowledge skill
└── review.md               ← Previous code review
```

## Stack

The stack is written down once, in `docs/architecture.md` ("Technology").

## Commands

```bash
# Build
cargo build
cargo build --release

# Check (no codegen, fast)
cargo check

# Run
cargo run -p heca

# Test
cargo test
cargo test -p heca-core
cargo test -p heca-renderer

# Lint (before committing)
cargo clippy --workspace --all-targets --all-features

# Fix auto-fixable issues
cargo clippy --fix --workspace --all-targets --all-features

# Watch (auto-rebuild on changes)
cargo watch -x check
```

## Known Issues

| ID | Issue | Severity | Status |
|----|-------|----------|--------|
| K1 | Prefix mode hardcodes `ctrl=false` | Critical | ✅ **FIXED** — passes real modifier state |
| K2 | Prefix key not configurable | High | ✅ **FIXED** — `prefix = "ctrl+b"` in config |
| K3 | No prefix timeout | Medium | ✅ **FIXED** — 500ms auto-exit |
| K4 | Shift+special-char bindings fail on some layouts | High | ✅ **FIXED** — `KeyCombo::parse()` maps shifted symbols |
| K5 | Registry bypasses (direct function calls) | High | ✅ **FIXED** — all routing through `registry.execute()` |
| L1 | `update_all_column_widths()` on every mutation | Critical | ✅ **FIXED** — removed from float/unfloat path |
| L2 | Proportion widths not persistent | High | Open |
| L4 | Focus up/down conflated with workspace switch | Medium | ✅ **FIXED** — `j/k` stay within workspace; `u/d` switch |
| L6 | Tabbed display, maximize, fullscreen dead code | Medium | Open |
| N1 | PaneSelect/Swap limited to 52 labels | Low | By design — use sidebar for >52 panes |

## Roadmap

What is planned and in progress lives in the project planner (`.planner/`, loaded with `/planner load`).
