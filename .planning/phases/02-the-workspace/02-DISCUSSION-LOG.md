# Phase 2: The Workspace — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-29
**Phase:** 2 — The Workspace
**Areas discussed:** Layout Model, Input Prefix, Chrome Layout, Pane Transitions, Left Sidebar, Status Bar, Prefix Key Default

---

## Layout Model

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Binary BSP tree | Each split has exactly two children. i3/sway style. | ✓ |
| Multi-child container | Each direction holds N children. tmux style. | |

**User's choice:** Binary BSP tree (i3-style)

---

## Input Prefix

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Prefix key (Ctrl+B) | Press prefix then key. tmux-style. | ✓ |
| Dedicated modifier | Hold Alt+Shift. i3-style. | |
| Both | Configurable, user chooses. | |

**User's choice:** Prefix key (tmux-style)

---

## Chrome Layout

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Tab top, status bottom, sidebars collapsible | Standard desktop layout. | ✓ |
| Tab top, left sidebar always visible, no status | Minimal chrome. | |

**User's choice:** Tab bar top, status bottom, sidebars collapsible

---

## Pane Transitions

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Distinct states with keybinds | Each pane is embedded, floating, scratchpad, or hidden. | ✓ |
| Floating as temporary | Drag to float, re-embed on release. | |

**User's choice:** Distinct states with keybinds

---

## Left Sidebar

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Placeholder + empty tree | Header "Sessions" with empty tree placeholder. | ✓ |
| Collapsed icon strip | Only icons, content appears when data exists. | |

**User's choice:** Placeholder text + tree view ready for items

---

## Status Bar

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Pane count, title, mode | "3 panes | Editor | NORMAL" | ✓ |
| Basic mode + hints | Minimal mode indicator. | |

**User's choice:** Pane count, focused pane title, current mode

---

## Default Prefix Key

| Option | Description | Selected |
| ------ | ----------- | -------- |
| Ctrl+B | Tmux-compatible. | ✓ |
| Ctrl+Space | Less shell collision. | |
| Ctrl+\ | Rarely used but awkward. | |

**User's choice:** Ctrl+B (tmux-compatible)

---

## the agent's Discretion

- Keybinding letter assignments after prefix
- BSP tree data structure details
- Tab bar visual style
- Sidebar width and icons
- Status bar layout and font
- Mouse cursor shapes
- Scroll speed

## Deferred Ideas

None — discussion stayed within phase scope.
