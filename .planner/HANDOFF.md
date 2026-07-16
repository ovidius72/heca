# Handoff

**Created at:** 2026-07-16
**Updated at:** 2026-07-16
**Reason:** Context ~70% used. Pause at a clean boundary: F003/P011/T010 committed; next step (showcase→ScrollRegion) is a render-loop refactor that needs a fresh session + interactive GPU verification.

## Current focus
F003 (🧩 Pluggable Chrome) / P011 (plugin-ui). The scroll/overlay rework. Branch `feat/action-task-c` — **NOT pushed**.

## What was being done
Building the reusable scrollable surface (ScrollRegion) and settling keyboard-scroll policy. T005 (Select modal-form marshalling) and T010 (ScrollRegion two-axis + surface) are DONE + user-verified. Discovered the app ALREADY has terminal scroll actions/keybindings, so the redundant "app scroll actions" task was deleted.

## How to resume
`/planner load`, then read this handoff. Immediate next task = the showcase→ScrollRegion conversion (see Next steps #1). Run `cargo run -p heca-renderer --example showcase` to verify each visual change (mandatory — headless can't see rendering).

## Files touched (committed on feat/action-task-c)
- `heca-grid-ui/src/widgets/scroll_region.rs` — two-axis, scrollbars, gutter/corner, click-track paging, StyleExt, keys/focusability REMOVED.
- `heca-grid-ui/src/component.rs` — `Event::Scroll` now two-axis (`delta_x`/`delta_y`).
- `heca-grid-ui/src/widgets/select.rs`, `lib.rs`, `widgets/mod.rs` (ScrollAxes export).
- `heca-renderer/examples/showcase.rs` — two-axis demo + wheel handler (host Shift→horizontal).
- `docs/widgets.md` (ScrollRegion + Select/Dialog), `docs/overlay-design.md` (T009 rework design), `heca-grid-ui/tests/phase_a.rs`.
- Commits: `c84528a` (ScrollRegion feat), `9c92696` (Select marshalling), `70b7292`/`c152ef7`/`6162a51` (planner/design).

## Blockers
- Showcase page scroll = manual `offset_tree` (showcase.rs:2314-2334); converting to a root ScrollRegion is a render-loop refactor AND couples with hosting overlays above the scroll (else Dialog/dropdowns scroll+clip). Needs GPU verification.
- App does NOT mount a chrome ScrollRegion yet (only `realize` maps `WidgetKind::Scroll`→ScrollRegion). So chrome-side keyboard scroll has no target until the sidebar/chrome uses ScrollRegion.

## Next steps
1. **Showcase → `ScrollRegion.both()`** (T010 step 5 / T009 Part 2, coupled): replace `offset_tree`/`scroll_y` with a root ScrollRegion sized to the window; host the overlays (Dialog/Select/ContextMenu/CommandPalette/ToastStack) ABOVE it (not inside the scrolled tree). Verify horizontal+vertical page scroll and that overlays stay fixed/centered. This also fixes BUG C (no horizontal page scroll) and BUG B (Dialog off-screen).
2. **T012 — glow regression** (broad: separator/grid/Inputs/Buttons/Panes/containers/drop-indicator have NO rest glow, only focused). Global rest-glow loss. Leads: `scaled_glow` (component.rs ~1093) scales by `theme.colors.glow_size`; git-diff refactored widgets' paint() glow usage vs pre-refactor.
3. **T009 (plugin-task-ui-7)** — base Overlay widget + nested-overlay z/occlusion (BUG A).

## Recent decisions
- tmux model: NO plain-key defaults; scroll bindings are prefix/modifier-gated + configurable. The ScrollRegion WIDGET binds no keys and is not a tab-stop.
- App ALREADY has scroll actions + keybindings (scroll_page_up/down→Shift+PageUp/Down, scroll_line_up/down, scrollback_*, scroll_view_left/right→prefix+Shift+Arrow, scroll_to_offset for RPC). The redundant T011 task was DELETED.
- Do NOT ship unverified rendering/layout changes — the user drives the GPU showcase.

## Reminder
Refer to features/phases/tasks by Fxxx/Pxxx/Txxx. Update docs/widgets.md (both audiences + code examples) on any widget change. Don't push without asking.
