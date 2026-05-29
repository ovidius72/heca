# State: heca

**Current Phase:** 1 — The Shell
**Status:** Context gathered, ready for planning
**Last Action:** Phase 1 context captured

## Progress

| Phase | Status | Requirements | Success Criteria |
| ----- | ------ | ------------ | ---------------- |
| 1 — The Shell | ○ Pending | 7 | 0/4 |
| 2 — The Workspace | ○ Pending | 26 | 0/7 |
| 3 — The Content | ○ Pending | 13 | 0/6 |
| 4 — The Platform | ○ Pending | 13 | 0/7 |

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-05-29)

**Core value:** A keyboard-native workspace where every tool lives in a tiled, floating, or scratchpad pane — all rendered in one GPU-accelerated window, fully restorable across sessions.

**Current focus:** Phase 1 — The Shell. Open a native window and render styled text and shapes via GPU.

## Blockers

None.

## Phase 1 Decisions

- **Visual target:** Mock tiling layout (colored rectangles + text labels)
- **Config:** `config.toml` + `themes/` directory
- **Default theme:** Catppuccin Mocha
- **Frame loop:** Event-driven redraw only

## Notes

- Browser pane deferred to v2.
- Out-of-process plugins deferred to v2.
- Cross-platform from day one: Linux, macOS, Windows.
