---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 01 of 2 (the shell)
status: executing
last_updated: "2026-06-08T08:47:57.350Z"
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 2
  completed_plans: 0
  percent: 0
---

# State: heca

**Current Phase:** 01 of 2 (the shell)
**Status:** Executing Phase 01
**Last Action:** NIRI layout engine built; terminal backend implemented; integrating into app

## Progress

| Phase | Status | Requirements | Success Criteria |
| ----- | ------ | ------------ | ---------------- |
| 1 — The Shell | ○ In Progress | 8 | 3/4 |
| 2 — The Workspace | ○ In Progress | 28 | 5/7 |
| 3 — The Content | ○ Pending | 13 | 0/6 |
| 4 — The Platform | ○ Pending | 14 | 0/7 |

## Phase 1 Deliverables

- ✅ **GPU shell** — winit + wgpu window, cosmic-text rendering, primitive shapes
- ✅ **Theme system** — TOML config with Catppuccin Latte/Mocha themes
- ✅ **Chrome** — Tab bar, status bar, left/right sidebars, pane borders

## Phase 2 Deliverables (NIRI Layout)

- ✅ **NIRI layout engine** (`heca-core/src/layout/`):
  - `Session` — workspace stack + overview/expose mode + workspace switching
  - `Workspace` — scrolling space + floating panes
  - `ScrollingSpace` — horizontal columns with `ViewOffset` animated scroll
  - `Column` — vertical pane stack with height distribution
  - `Pane` — content-agnostic layout leaf
  - `Animation` system — easing, swipe tracker, `Animated<T>`
  - `ViewOffset` — three-state scroll (Static/Animation/Gesture)
- ✅ **PaneBackend trait** — abstract interface for terminal, neovim, browser
- ✅ **Terminal backend** — PTY + `vte` parser + cell grid
- ✅ **NIRI layout wired into app** — `Session` replaces `PaneTree` in `main.rs`, render loop uses `panes_with_positions()`
- ⏳ **Input routing** — keyboard → active pane, focus left/right/up/down via session
- ⏳ **Overview mode rendering** — zoomed workspace thumbnails

## Keyboard Bindings (NIRI-style)

| Binding | Action |
|---------|--------|
| Ctrl+B → h | Focus column left (animated scroll) |
| Ctrl+B → l | Focus column right (animated scroll) |
| Ctrl+B → j | Focus pane down / next workspace |
| Ctrl+B → k | Focus pane up / prev workspace |
| Ctrl+B → - | Split horizontal (new column) |
| Ctrl+B → v | Split vertical (new pane in column) |
| Ctrl+B → x | Close active pane |
| Ctrl+B → Space | Toggle left sidebar |
| Ctrl+B → o | Toggle overview/expose mode |

## Blockers

None.

## Notes

- Old BSP tree (`heca-core/src/pane.rs`) kept for reference but no longer used.
- NIRI layout engine is the canonical layout system.
- Browser pane deferred to v2.
- Out-of-process plugins deferred to v2.
- Cross-platform from day one: Linux, macOS, Windows.
