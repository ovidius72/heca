# Shared Tasks

## Purpose

This file is the coordination board for delegated work on the remaining
post-merge terminal backlog.

Rules:

1. Only active or not-yet-accepted tasks stay in this file.
2. The assigned agent updates only the task they are working on.
3. When the assigned agent finishes, they append notes under `Agent Completion`.
4. The reviewer writes the result in the same task under:
   - `Reviewer Decision`
   - `Reviewer Notes`
5. If review passes, the completed task is removed from this file.

Review policy:

- acceptance is based on code, verification, and architecture
- completion notes alone are not sufficient
- no hardcoded feature-key behavior is acceptable for keyboard interactions; feature keys must route through `WmAction` + registry + keymaps + config
- actions must remain reachable from keyboard, mouse/UI, and RPC whenever that surface makes sense

Status values:

- `Open`
- `In Progress`
- `Completed by Agent`
- `Needs Edit`
- `Accepted`
- `Rejected`

---

## Task 07 — heca-grid-ui Pane Replacement and Pane Gap Config

**Status:** Open

**Goal**

Replace the current custom pane container presentation with the existing
`heca-grid-ui` pane widget/path, and make pane chrome geometry configurable
through `config.toml`.

This task is about the pane surface/container, not sidebar replacement.

**Source Material**

Read first:

- [docs/widgets.md](/Users/antonio/projects/heca/docs/widgets.md)
- [showcase.rs](/Users/antonio/projects/heca/heca-renderer/examples/showcase.rs)

Relevant facts:

- `heca-grid-ui` already has a `Pane` widget
- pane/card/widget border width and radius are theme-driven
- the showcase demonstrates runtime-configurable radius and border width
- layout gaps are part of the widget/layout vocabulary already

**Scope**

This task should:

- replace pane presentation/container chrome with the `heca-grid-ui` pane path
- keep terminal/content rendering mounted inside the pane content area
- make pane border width configurable through app config/theme
- make pane radius configurable through app config/theme
- add a user-configurable pane gap setting

This task must not:

- redesign sidebar shell behavior
- refactor workspace tree semantics
- change terminal backend semantics
- implement blur itself

**Required Architecture**

- pane presentation should move toward the future `heca-grid-ui` shell/container direction
- terminal content must remain mounted inside the pane surface, not become the owner of pane chrome again
- border/radius/gap must come from config/theme, not hardcoded render literals
- gap must be a layout/input to pane positioning, not just a visual fake margin

**Implementation Requirements**

1. Pane widget path
- use the existing `heca-grid-ui` pane/container capability rather than inventing a new custom pane chrome path
- keep terminal and other pane contents rendered inside the resulting content rect

2. Configurable border and radius
- pane border width must be configurable via `config.toml`
- pane radius must be configurable via `config.toml`
- values must flow through theme/config, not be hardcoded in render code

3. Configurable pane gap
- add a pane gap setting in config
- pane gap must affect the actual layout spacing between panes/columns
- it must not be simulated only by shrinking visuals after layout

4. Backward compatibility
- existing panes must still render correctly after the container swap
- focus and active-pane treatment must still work
- float/zoom states must still respect clipping and pane separation

**Acceptance Criteria**

- pane container presentation uses the `heca-grid-ui` path
- pane border width is configurable from `config.toml`
- pane radius is configurable from `config.toml`
- pane gap is configurable from `config.toml`
- terminal pane content still renders correctly inside the pane content rect
- code compiles and tests remain green

**Verification**

At minimum:

```bash
cargo check -p heca
cargo check -p heca-grid-ui
cargo clippy -p heca --all-targets
cargo clippy -p heca-grid-ui --all-targets
```

If practical, also live-check:

1. changing pane radius in config changes the pane chrome
2. changing border width in config changes the pane chrome
3. changing pane gap in config visibly changes pane spacing
4. terminal panes still clip/render correctly inside the new pane surface

**Agent Completion**

- Pending.

**Reviewer Decision**

- Pending.

**Reviewer Notes**

- Pending.
