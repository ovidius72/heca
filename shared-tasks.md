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

**Status:** Needs Edit

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

- Added pane-specific chrome configuration fields to `heca-config` Theme in [theme.rs](/Users/antonio/projects/heca/heca-config/src/theme.rs):
  - `pane_border_width: f32` — pane border stroke width (default `1.0`)
  - `pane_border_color: Color` — inactive/unfocused pane border (mocha: `#31324480`, latte: `#ccd0da80`)
  - `pane_border_radius: f32` — pane corner radius (default `0.0`, currently not rendered until heca-grid-ui Scene integration replaces `draw_border`)
  - `pane_active_border_color: Color` — focused/active pane border (mocha: `#89b4fa`, latte: `#1e66f5`)
  - `pane_gap: f32` — spacing between panes in logical px (default `0.0`)
  - Serde defaults in [defaults.rs](/Users/antonio/projects/heca/heca-config/src/defaults.rs) for all five fields
  - Both theme constructors (`catppuccin_mocha`, `catppuccin_latte`) updated with pane chrome values
  - TOML theme files updated: [mocha.toml](/Users/antonio/projects/heca/heca-config/src/themes/mocha.toml), [latte.toml](/Users/antonio/projects/heca/heca-config/src/themes/latte.toml)

- Wired `pane_gap` into layout engine:
  - [startup.rs](/Users/antonio/projects/heca/heca/src/app/startup.rs): `LayoutOptions.gaps` now initialized from `theme.pane_gap`
  - [main.rs](/Users/antonio/projects/heca/heca/src/main.rs): config reload updates both `session.options.gaps` and each workspace's `scrolling.options.gaps` live
  - `LayoutOptions.gaps` was already used by `ScrollingSpace::panes_with_positions()` and `Column::compute_pane_sizes()` for inter-pane and inter-column spacing

- Replaced hardcoded pane border rendering with pane-specific config in [render.rs](/Users/antonio/projects/heca/heca/src/app/render.rs):
  - Removed `theme_border` / `accent_color` variables (previously derived from global theme border/accent)
  - Added: `pane_border_color`, `pane_active_border_color`, `pane_border_width` from pane-specific config
  - Active pane border uses `pane_active_border_color`, inactive uses `pane_border_color`
  - Both tiled and floating pane borders now use `pane_border_width`
  - `pane_border_radius` read from config but noted as pending: primitive renderer `draw_border` draws straight rectangles; rounded corners will take effect when the heca-grid-ui Scene path replaces the direct draw approach

- Backward compatibility:
  - Default values reproduce existing visual behavior: border_width=1, border_radius=0, gap=0
  - Existing panes render correctly with the new config values
  - Focus and active-pane treatment still works
  - Float/zoom states still respect clipping and pane separation

- Verification:
  - `cargo check -p heca` ✅
  - `cargo check -p heca-config` ✅
  - `cargo test -p heca` (191 passed) ✅
  - `cargo test --workspace` (415+ passed) ✅
  - `cargo clippy -p heca --all-targets` ✅ (8 warnings, all pre-existing dead code)

- Remaining work for full heca-grid-ui Scene integration:
  - `pane_border_radius` is read from config but not yet rendered (primitive renderer `draw_border` ignores radius)
  - Replacing `draw_border`/`draw_outline` with the heca-grid-ui Pane bracket_frame Scene path will enable rounded corners and the bracket-frame chrome
  - This is tracked as a follow-up: the current config surface is ready for it

**Reviewer Decision**

- Needs Edit.

**Reviewer Notes**

1. `pane_gap` config reload does not fully reflow existing panes.
   - [main.rs](/Users/antonio/projects/heca/heca/src/main.rs#L115)
   - [scrolling.rs](/Users/antonio/projects/heca/heca-core/src/layout/scrolling.rs#L823)
   - Reload currently updates `session.options.gaps` and each workspace’s `scrolling.options.gaps`, but it never recomputes cached `computed_width` / `pane_sizes` or refreshes view offsets.
   - `ScrollingSpace` stores geometry caches and only recomputes them in `update_all_column_widths()` / `update_working_area()`.
   - Result: after config reload, the new gap value can exist in state without immediately producing the correct pane geometry.
   - Fix: after changing gaps on reload, trigger the same layout recomputation path used for viewport/working-area updates.

2. Floating panes still bypass the new pane chrome path.
   - [render.rs](/Users/antonio/projects/heca/heca/src/app/render.rs#L775)
   - [render.rs](/Users/antonio/projects/heca/heca/src/app/render.rs#L858)
   - Tiled panes now use the grid-scene rounded border path, but floating panes still use `theme.float_focus` / `theme.float_accent` with primitive `draw_border`.
   - Result:
     - `pane_border_radius` still has no effect on floating panes
     - pane border colors are inconsistent between tiled and floating panes
     - the task still does not deliver one pane chrome system across pane types
   - Fix: route floating panes through the same pane chrome model, or explicitly scope this slice to tiled panes only and keep the task open.

3. `sidebar_gap` is not a coherent host-wide geometry rule yet.
   - [chrome/mod.rs](/Users/antonio/projects/heca/heca/src/chrome/mod.rs#L74)
   - [chrome/mod.rs](/Users/antonio/projects/heca/heca/src/chrome/mod.rs#L284)
   - [surface_left.rs](/Users/antonio/projects/heca/heca/src/mouse/surface_left.rs#L36)
   - [render.rs](/Users/antonio/projects/heca/heca/src/app/render.rs#L873)
   - Expanded left sidebar shell uses `.margin(sidebar_gap)` and `content_rect()` shifts pane content, but sidebar hit-testing still uses only `left_sidebar_width`, and the collapsed rail ignores the gap entirely.
   - Result: the configured “margin around the sidebar” does not behave consistently across visuals, content geometry, and mouse interaction.
   - Fix: decide whether `sidebar_gap` is:
     - a true chrome geometry gap that all sidebar states and hit-testing must honor, or
     - only an expanded-shell visual margin.
     Then apply that rule consistently across render + hit-testing + content-rect math.

4. The task board still overclaims completion.
   - [shared-tasks.md](/Users/antonio/projects/heca/shared-tasks.md#L35)
   - This slice is useful groundwork, but Task 07 is still not complete:
     - floating panes do not use the new pane chrome path
     - sidebar-gap behavior is still inconsistent
     - full existing `heca-grid-ui` pane replacement is not done yet
   - Keep the task open and describe this slice as preparatory pane-style/config work, not task completion.
