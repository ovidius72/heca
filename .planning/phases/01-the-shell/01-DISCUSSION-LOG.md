# Phase 1: The Shell — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-29
**Phase:** 1 — The Shell
**Areas discussed:** Visual Target, Config Organization, Default Theme, Frame Loop

---

## Visual Target

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Mock tiling layout | Renders a fake grid of panes with titles and borders. Proves compositor concept. | ✓ |
| Hello World text | Just renders styled text in the center. Simpler but less validation. | |

**User's choice:** Mock tiling layout (colored rectangles + text labels) — proves the compositor concept immediately
**Notes:** User wants Phase 1 to validate the architecture, not just prove text rendering works.

---

## Config Organization

| Option | Description | Selected |
| ------ | ----------- | -------- |
| config.toml + themes/ | Clean separation. Users can drop in theme files. | ✓ |
| Single config.toml | Simpler but config gets large when themes are complex. | |

**User's choice:** config.toml + themes/ directory with separate .toml theme files

---

## Default Theme

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Catppuccin Mocha | Popular among developers, soft colors, good contrast. | ✓ |
| Tokyo Night | Darker, popular in Neovim/VS Code ecosystems. | |
| Custom minimal | We define our own. More work, more unique. | |

**User's choice:** Catppuccin Mocha (Recommended)

---

## Frame Loop

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Continuous 60fps | Easier to implement. Phase 4 adds damage tracking. | |
| Event-driven redraw | Only redraws when something changes. Efficient from day one. | ✓ |

**User's choice:** Event-driven redraw only (efficient from day one)

---

## the agent's Discretion

- wgpu surface configuration (present mode, format)
- Font fallback chain
- config.toml exact section structure
- Mock layout arrangement

## Deferred Ideas

None — discussion stayed within phase scope.
