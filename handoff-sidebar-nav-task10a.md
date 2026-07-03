# Handoff — Sidebar-nav highlight (task-10a) + sidebar buttons/actions requirement

**Date:** 2026-07-03. **Branch:** `feat/plugin-03-providers` (worktree
`/Users/antonio/projects/myvim-plugin-03`, sibling; docs arc lives separately on
`docs/plugin-architecture`). Read fully before resuming.

## What SHIPPED in this branch (task-10a — sidebar-nav cursor highlight)
The known regression "in sidebar mode `prefix+e` → `j/k` moves the cursor but
nothing highlights" is fixed, plus refinements from live user feedback.

- **State/event** (`heca/src/chrome/state.rs`, `events.rs`): `nav_selection:
  Signal<Option<SidebarSelection>>` on `WorkspacesContainerState` + `set_nav_selection`
  chokepoint emitting `ChromeEvent::SidebarSelectionChanged`. `SidebarSelection` is a
  `Copy` enum (Workspace/Column/Pane/FloatingPane) in `events.rs`.
- **Projection** (`heca/src/chrome/mod.rs` `sync_chrome_state`): while
  `InputMode::SidebarNav`, projects `sidebar_tree.current_item()` → `set_nav_selection`;
  `None` otherwise. Central pull, not per-handler. Helper `sidebar_selection_from_item`.
- **Render** (`heca/src/chrome/mod.rs`): `ChromeSignals` gained `pane_nav` + `ws_nav`
  (NO `col_nav` — columns are not navigable, see decisions). `sync_chrome_signals`
  drives them from `nav_selection()`. Cards/workspace docks get `.nav_selected(false)`
  + push their `nav_state()`.
- **grid-ui widgets extended** (+ showcase demos + `docs/widgets.md`):
  `Row::nav_selected/nav_state` (border **accent + glow**, drawn ALWAYS incl. on the
  active row), `MarkerGroup::nav_selected/nav_state` (kept as API but NOT wired in
  chrome), `DockFrame::nav_selected/nav_state` (thick accent border + faint fill +
  glow).
- **Navigation** (`heca/src/sidebar/model.rs`): `is_navigable` now = **Pane OR
  Workspace only** (skips Column + FloatingPane). Removed unused `is_selectable`.
- **Expansion fix** (`heca/src/handlers.rs` `handle_sidebar_focus`): now touches
  NEITHER `left_mode` NOR `left_size` — a contracted sidebar stays contracted. The
  sidebar look is driven by `left_size` (`app/render.rs:191`: width < 80 → rail).
  **User confirmed this is resolved** (incl. collapse/expand in sidebar mode).
- **Tests**: `nav_selection_round_trips_and_emits_on_change`; `test_cursor_movement`
  rewritten to assert properties (never lands on a Column). **297 heca tests pass,
  clippy clean, showcase builds.**

## Decisions taken (with why)
- **Cursor stops on panes + workspaces, NOT columns** — user: landing on a column
  "reads as a jump into nothing". Workspaces stay navigable so a **collapsed**
  workspace can be re-expanded with `l`.
- **Cursor drawn ALWAYS (even on the active row/ws), in a DISTINCT treatment** —
  first tried `!active` (invisible when cursor == active), then a white/foreground
  border (user disliked white), now **theme `accent` + glow**. It's thematic and the
  glow/thickness distinguishes it. NOTE: when cursor coincides with the active pane
  the two are both accent — if the user still wants them more distinct, add a
  dedicated configurable "cursor" theme token.
- **`handle_sidebar_focus` = zero side effects on mode/size** — the root of the
  "entering sidebar mode expands a contracted sidebar" bug.
- **Sidebar-mode keybinds are already fully configurable** — `build_modes`
  (`app/registry.rs:182`) builds the `"sidebar"` mode keymap from `[[keys.mode]]` in
  config; user list replaces defaults wholesale. `j/k/h/l/arrow/Space/Enter` →
  `sidebar_up/down/left_nav/right_nav` (nominal actions), rebindable.

## Verification / how to test
`cd /Users/antonio/projects/myvim-plugin-03 && cargo run -p heca`; `Ctrl+B` `e` →
`j/k`: cursor skips columns, accent+glow outline on pane/workspace incl. the active
one; `h/l` collapse/expand works; a contracted sidebar stays contracted.

## REMAINING follow-ups (user-requested, NOT yet done)
1. **Collapsed rail — pane initials not updated on rename**: `collapsed_pane_label`
   (`heca/src/sidebar/render.rs`) uses the process name; must prefer `custom_name`
   when set (mirror the expanded side's `pane_custom_name`).
2. **KeyHint missing in the collapsed rail** (an old impl exists): bring the universal
   `KeyHint` (leader/pick letters) into the hand-drawn collapsed rail. This is the
   `app-task-21` scope (also: show pane name, render nav cursor in collapsed).
3. **New `[settings]` toggles**: `show_left_sidebar`, `show_right_sidebar`,
   `show_top_bar`, `show_bottom_bar` (bool). `false` = **fully hide** that chrome
   widget. Touches `heca-config` (settings schema, `heca-config/src/`) + applying it
   to the chrome regions (`RegionMode::Hidden` / skip render). Wire into startup +
   config reload.

## NEW REQUIREMENT — restore sidebar add/remove buttons + actions + context menu
The old sidebar had **buttons/actions to add/remove workspaces/columns/panes**; they
were lost in the grid-ui chrome rebuild. Restore them on three surfaces:
- **Buttons in the sidebar** (grid-ui widgets) to: **addPane** (into the highlighted
  column), **addCol** (into the highlighted workspace), **addWs** (at the top).
- **Actions** runnable in sidebar mode acting on the **highlighted** item (cursor).
  NOTE: the *handlers already exist* — `handle_sidebar_create_workspace`,
  `handle_sidebar_create_column`, `handle_sidebar_split_in_column`,
  `handle_sidebar_delete_selected` (`heca/src/handlers.rs:1471/1485/1504/1532`), and
  they act on `current_item()`. So mostly the **UI buttons** + wiring are missing, not
  the actions — verify each acts on the right highlighted target (col for addPane, ws
  for addCol, top for addWs) and add any missing (e.g. explicit addPane-into-column).
- **Context menu (mouse)** offering the same add/remove operations, anchored on the
  right-clicked ws/col/pane. There's an existing context-menu overlay pattern
  (`chrome/mod.rs` paint of the right-click menu, "app's first stateful overlay").

## Status of commits
task-10a code is committed on this branch and a PR was opened to `main` (see the PR).
Backlog updated with the follow-ups + new requirement. NOT reviewed line-by-line by
the user yet — treat the PR as the review vehicle.
