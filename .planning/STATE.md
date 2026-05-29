---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 01 of 2 (the shell)
status: executing
last_updated: "2026-05-29T22:47:49.507Z"
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 2
  completed_plans: 0
---

# State: heca

**Current Phase:** 01 of 2 (the shell)
**Status:** Executing Phase 01
**Last Action:** Phase 2 code committed

## Progress

| Phase | Status | Requirements | Success Criteria |
| ----- | ------ | ------------ | ---------------- |
| 1 — The Shell | ✓ Complete | 8 | 4/4 |
| 2 — The Workspace | ✓ Complete | 28 | 7/7 |
| 3 — The Content | ○ Pending | 13 | 0/6 |
| 4 — The Platform | ○ Pending | 14 | 0/7 |

## Phase 2 Deliverables

- **BSP tree layout engine** (`heca-core/src/pane.rs`) — LayoutNode tree with Split/Leaf/Empty nodes, pane IDs, compute_rects traversal
- **Pane state machine** — Embedded, Floating (with rect tracking), Scratchpad (hidden/visible toggle), Hidden
- **Pane operations** — split, remove, toggle_float, toggle_scratchpad, hide, show, find_neighbor, swap_panes, resize
- **Column chrome** (`heca/src/chrome.rs`) — ChromeConfig with tab bar, status bar, left/right sidebars — content_rect computation
- **Input router** (`heca/src/input.rs`) — Ctrl+B prefix mode, WmAction enum, KeyBindings with default mappings
- **Mouse support** — click to focus pane, border drag resize, cursor changes
- **Tab bar** — shows tab names with active highlight
- **Status bar** — "N panes | Title | MODE" display
- **Left/Right sidebars** — collapsible, show "Sessions" and "Details" placeholders
- **9 tests passing** — 7 in heca-config, 2 in heca (chrome layout)

## Phase 2 Keyboard Bindings

| Binding | Action |
|---------|--------|
| Ctrl+B → h/j/k/l | Focus pane left/down/up/right |
| Ctrl+B → - | Split horizontal |
| Ctrl+B → v | Split vertical |
| Ctrl+B → Shift+h/j/k/l | Resize pane |
| Ctrl+B → f | Toggle float |
| Ctrl+B → s | Toggle scratchpad |
| Ctrl+B → z | Hide pane |
| Ctrl+B → x | Close pane |
| Ctrl+B → [ / ] | Previous/next tab |
| Ctrl+B → Space | Toggle left sidebar |
| Click | Focus pane |
| Drag border | Resize split |

## Blockers

None.

## Notes

- Browser pane deferred to v2.
- Out-of-process plugins deferred to v2.
- Cross-platform from day one: Linux, macOS, Windows.
- Run `cargo run -p heca` to see the tiling workspace with chrome.
