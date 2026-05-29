---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 01 of 1 (the shell)
status: executing
last_updated: "2026-05-29T20:48:41.754Z"
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 1
  completed_plans: 0
---

# State: heca

**Current Phase:** 01 of 1 (the shell)
**Status:** Executing Phase 01
**Last Action:** Phase 1 code committed

## Progress

| Phase | Status | Requirements | Success Criteria |
| ----- | ------ | ------------ | ---------------- |
| 1 — The Shell | ✓ Complete | 8 | 4/4 |
| 2 — The Workspace | ○ Pending | 28 | 0/7 |
| 3 — The Content | ○ Pending | 13 | 0/6 |
| 4 — The Platform | ○ Pending | 14 | 0/7 |

## Phase 1 Deliverables

- **Cargo workspace** with 4 crates: `heca` (binary), `heca-core`, `heca-renderer`, `heca-config`
- **winit + wgpu window** opens at 1280×800, event-driven redraw, handles resize
- **Theme system** loads from `~/.config/heca/config.toml` + `themes/*.toml`, falls back to bundled Catppuccin Mocha/Latte
- **GPU primitive renderer** draws rectangles, borders, and rounded rects via wgpu
- **Text renderer** using `cosmic-text` with per-frame atlas, HiDPI scale factor support
- **Mock tiling layout** renders 4 panes (Editor, Terminal, Files, Preview float) with distinct colors, borders, and titles
- **7 tests passing** in `heca-config` (color parsing, theme loading, fallback behavior)

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-29)

**Core value:** A keyboard-native workspace where every tool lives in a tiled, floating, or scratchpad pane — all rendered in one GPU-accelerated window, fully restorable across sessions.

## Blockers

None.

## Notes

- Browser pane deferred to v2.
- Out-of-process plugins deferred to v2.
- Cross-platform from day one: Linux, macOS, Windows.
- Run `cargo run -p heca` to see the mock tiling layout window.
