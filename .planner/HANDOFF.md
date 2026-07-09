# Handoff

Created at: 2026-07-09T11:14:16.899Z
Updated at: 2026-07-09T14:45:17.408Z
Reason: Session paused mid-implementation of context-menu-7; context-menu-6 completed; user requested handoff with ALL captured context (design decisions, architecture, plugin contract, mode flow).

## Progress snapshot
- Features: 2/10 done, 1 active
- Phases: 22/73 done, 1 active/discovery
- Tasks: 83/272 done, 2 active

## Current focus
- Feature: `ebb9ceb0-ea12-4374-af6e-12aa4256bcc3` — 🧩 Pluggable Chrome Architecture (in-progress)
- Phase: `8f760a9b-3ce8-4932-8286-67d742397f2e` — context-menu: Contextual menu → OverlayHost + plugin-declarable (in-progress)
- Task: `63d4e696-bacc-4556-8d7c-a074816097cc` — context-menu-3: OpenContextMenu keyboard/RPC action + item polish (in-progress)

## What was being done
Implementing context-menu-7 (keyboard context-aware menus). context-menu-6 was completed earlier in this session: built the unified ContextMenuRegistry + open_context_menu_for path with 4 built-in providers (pane, sidebar.pane, sidebar.column, sidebar.workspace), overlay_origin_mode field on AppState with mode-restore in overlay::resolve(), and refactored mouse.rs (open_context_menu, open_focused_context_menu, open_sidebar_context_menu) to route through open_context_menu_for. 5 unit tests pass, clippy clean. The remaining work (context-menu-7) is: resolve_active_context() keyboard impl, pending_context field on AppState to carry sidebar selection through Prefix→Normal dispatch, prefix arm handler in handle_sidebar_nav_mode (mirroring selection mode pattern), and wiring handle_open_context_menu to consume pending_context.

## Locked Design Decisions (cm-d1..cm-d6, recorded in planner phase 8f760a9b)

### cm-d1: ContextPath dotted + target opaco
- `ContextPath` is a dotted string: `"pane"`, `"sidebar.pane"`, `"sidebar.column"`, `"sidebar.workspace"`, `"floating_pane"`, future plugin paths.
- `ContextTarget` is an opaque enum: `Pane{pane_id, hyperlink:Option<String>}`, `SidebarPane{pane_id}`, `SidebarColumn{ws_idx,col_idx}`, `SidebarWorkspace{ws_idx}`.
- The host resolves the path implicitly (keyboard: `resolve_active_context(state)` reads InputMode + selection/focus) or explicitly (mouse: hit-test → ChromeDragItem → (path,target)).

### cm-d2: prefix+> everywhere via prefix arm
- `prefix+>` triggers context menu on ANY surface: pane (Normal mode), sidebar item (SidebarNav mode).
- In SidebarNav, a prefix arm (mirroring `handle_selection_mode` :738/:744) transitions to Prefix mode and captures `pending_context` BEFORE the transition (because `handle_prefix_mode` sets Normal before dispatch, so the handler can't read SidebarNav).

### cm-d3: mode-restore via overlay_origin_mode
- `overlay_origin_mode: Option<InputMode>` on AppState (app_state.rs:664).
- Captured in `open_context_menu_for` when `overlay_origin_mode.is_none()` (don't overwrite on stack).
- Restored in `overlay::resolve()` (overlay.rs:372-380) when the LAST overlay closes, BEFORE the completion runs (so a follow-up confirm opens with origin already restored).
- `restorable_mode()` whitelist: only `SidebarNav` returns `Some`; `Normal` returns `None` (no-op).
- Stacked overlays: origin set once, not overwritten; restore only when no overlay remains.

### cm-d4: pending_context preserves target through dispatch
- `pending_context: Option<PendingContext>` on AppState (to be added in context-menu-7).
- `PendingContext` = `(ContextPath, ContextTarget, Option<InputMode>)` — path, target, and optional origin mode.
- Set by the sidebar prefix arm BEFORE the Prefix transition. Consumed by `handle_open_context_menu`.
- After consumption, cleared (`.take()`).

### cm-d5: mouse refactored to unified path, preserving parity + Open link via target
- `open_context_menu` (mouse pane) → `open_context_menu_for(PANE, PaneTarget{pane_id,hyperlink}, MouseContent, None)`.
- `open_focused_context_menu` (keyboard pane) → `open_context_menu_for(PANE, PaneTarget{focused,None}, window_center, Keyboard, None)`.
- `open_sidebar_context_menu` → maps `ChromeDragItem`→(path,target) → `open_context_menu_for(...,MouseLeftSidebar,None)`.
- `pane_action_items()` moved from mouse.rs into context_menu.rs (pane provider).
- "Open link" entry: only when `hyperlink` is `Some` (mouse path resolves hyperlink at click pos; keyboard path has `None` → no link entry).
- `centered` flag: derived from `source == Keyboard` (keyboard → centered at window center; mouse → anchored at click pos).

### cm-d6: plugin passes context_path + build, not mode/origin
- Plugin declares `context_path` (dotted string, WHERE the menu attaches) + `weight` (Dewey `Vec<i64>`) + `build(target)` closure.
- Plugin does NOT pass mode/origin (host-internal, derived from input mode).
- Host includes plugin menu when context is active (keyboard `resolve_active_context`) or clicked (mouse hit-test).
- Full design in context-menu-5 (3724e701, planned).

## Architecture: ContextMenuRegistry + open_context_menu_for (context-menu-6, DONE)

### New module: `heca/src/chrome/context_menu.rs`
- `ContextPath` — consts: `PANE = "pane"`, `SIDEBAR_PANE = "sidebar.pane"`, `SIDEBAR_COLUMN = "sidebar.column"`, `SIDEBAR_WORKSPACE = "sidebar.workspace"`.
- `ContextTarget` — enum with 4 variants (see cm-d1).
- `ContextMenuProvider` — `{ weight: Vec<i64>, build: fn(&AppState, &ContextTarget) -> Vec<DropdownItem> }`.
- `ContextMenuRegistry` — `HashMap<String, Vec<ContextMenuProvider>>` with methods:
  - `with_builtins()` — registers 4 built-in providers (pane, sidebar.pane, sidebar.column, sidebar.workspace).
  - `register(path, provider)` — for plugins (context-menu-5).
  - `items_for(path, state, target)` — merges all providers for a path, sorted by Dewey weight.
  - `ordered_providers(path)` — returns providers sorted by weight.
- `open_context_menu_for(state, path, target, anchor, source, origin)` — unified open:
  1. Looks up providers via `registry.items_for(path, state, target)`.
  2. If empty, returns (no menu).
  3. Builds `DropdownSpec` with `centered = matches!(source, Keyboard)`.
  4. Captures `overlay_origin_mode` if `None` (from `origin` param or `restorable_mode(input_mode)`).
  5. Calls `open_dropdown(state, spec)`.
- `restorable_mode(mode) -> Option<InputMode>` — returns `Some(SidebarNav)` only; `Normal` → `None`.
- `pane_action_items()` — the 5 pane-action DropdownItems (SplitHorizontal, SplitVertical, ZoomColumn, Float, ClosePane).
- 5 unit tests: `pane_action_items_has_five_entries`, `registry_seeds_four_built_in_providers`, `unknown_path_yields_no_providers`, `ordered_providers_sorts_by_weight`, `restorable_mode_only_sidebar_nav`.

### Modified: `heca/src/chrome/overlay.rs`
- `resolve()` (:362-380): after `state.layers.remove`, before completion runs:
  ```rust
  if top_modal(state).is_none() && let Some(mode) = state.overlay_origin_mode.take() {
      state.input_mode = mode;
  }
  ```

### Modified: `heca/src/app_state.rs`
- `overlay_origin_mode: Option<InputMode>` (:664) — captured at overlay open, restored on last close.
- `context_menu_registry: ContextMenuRegistry` (:709) — seeded with builtins at startup.

### Modified: `heca/src/app/startup.rs`
- `overlay_origin_mode: None` (:368).
- `context_menu_registry: ContextMenuRegistry::with_builtins()` (:378).

### Modified: `heca/src/chrome/mod.rs`
- `mod context_menu;` + pub(crate) exports: `open_context_menu_for`, `ContextMenuProvider`, `ContextMenuRegistry`, `ContextPath`, `ContextTarget`.

### Modified: `heca/src/mouse.rs`
- `open_context_menu` → `open_context_menu_for(PANE, PaneTarget{pane_id,hyperlink}, MouseContent, None)`.
- `open_focused_context_menu` → `open_context_menu_for(PANE, PaneTarget{focused,None}, window_center, Keyboard, None)`.
- `open_sidebar_context_menu` → maps `ChromeDragItem`→(path,target) → `open_context_menu_for(...,MouseLeftSidebar,None)`.
- `pane_action_items()` removed (moved to context_menu.rs).
- `window_center_logical()` kept (computes `(inner_size / scale_factor) / 2`).

## Mode Flow: Normal → Prefix → dispatch → overlay → restore

### Keyboard from Normal mode (pane menu)
1. User presses `prefix+>` in Normal mode.
2. `handle_keyboard_input` → `is_prefix` → `InputMode::Prefix`.
3. `handle_prefix_mode` → `keymap.resolve("normal", "prefix+>")` → `OpenContextMenu`.
4. `handle_prefix_mode` sets `InputMode::Normal` before `dispatch_action`.
5. `handle_open_context_menu` → `resolve_active_context(state)` → `("pane", PaneTarget{focused,None})`.
6. `open_context_menu_for("pane", ..., Keyboard, origin=None)` → `overlay_origin_mode` stays `None` (Normal not restorable).
7. Menu opens centered. User picks item or Esc.
8. `resolve()` → `top_modal.is_none()` true, `overlay_origin_mode` is `None` → no-op. Stays Normal.

### Keyboard from SidebarNav mode (sidebar item menu) — context-menu-7
1. User presses `prefix+>` in SidebarNav mode.
2. `handle_sidebar_nav_mode` → new `is_prefix` arm → `resolve_active_context(state)` → `("sidebar.workspace", SidebarWorkspace{ws_idx})`.
3. Sets `pending_context = (path, target, Some(SidebarNav))`.
4. Transitions to `InputMode::Prefix` + arms `prefix_entered_at`.
5. `handle_prefix_mode` → `keymap.resolve("normal", "prefix+>")` → `OpenContextMenu`.
6. `handle_prefix_mode` sets `InputMode::Normal` before `dispatch_action`.
7. `handle_open_context_menu` → `state.pending_context.take()` is `Some` → uses it (path,target,origin=Some(SidebarNav)).
8. `open_context_menu_for(path, target, Keyboard, origin=Some(SidebarNav))` → `overlay_origin_mode = Some(SidebarNav)`.
9. Menu opens centered. User picks item or Esc.
10. `resolve()` → `top_modal.is_none()` true, `overlay_origin_mode.take()` = `Some(SidebarNav)` → `state.input_mode = SidebarNav`. ✅

### Mouse right-click (unchanged flow)
1. `on_mouse_input` → `MouseButton::Right, Pressed`.
2. Content area: `open_context_menu(state, pane_id, pos)` → `open_context_menu_for(PANE, ..., MouseContent, None)`.
3. Sidebar: `sidebar_item_at(state, pos)` → `open_sidebar_context_menu(state, item, pos)` → `open_context_menu_for(path, ..., MouseLeftSidebar, None)`.
4. `overlay_origin_mode` stays `None` (mouse from Normal).

## Plugin Contract (context-menu-5, planned)
- Plugin registers via `ContextMenuRegistry::register(path, provider)`.
- `provider.weight: Vec<i64>` — Dewey/fractional-index ordering (e.g. `[1,1]` for entry 1, `[1,2]` for entry 2, plugin inserts `[1,1,1]` between them).
- `provider.build(&AppState, &ContextTarget) -> Vec<DropdownItem>` — builds items for the given target.
- Plugin does NOT pass mode/origin (host-internal).
- Full native+WASM design now (C3 locked).

## Sidebar Item → (path, target) Mapping (for resolve_active_context)
- `SidebarItem::Pane{pane_id}` → `("sidebar.pane", SidebarPane{pane_id})`.
- `SidebarItem::FloatingPane{pane_id,..}` → `("pane", Pane{pane_id, hyperlink:None})` (keyboard gives useful pane menu; mouse path today produces empty menu for FloatingPane — latent gap, noted).
- `SidebarItem::Column{ws_idx,col_idx}` → `("sidebar.column", SidebarColumn{ws_idx,col_idx})`.
- `SidebarItem::Workspace{ws_idx}` → `("sidebar.workspace", SidebarWorkspace{ws_idx})`.
- `state.sidebar_tree.current_item()` returns the current cursor item in SidebarNav mode.

## Files Touched This Session

### New files
- `heca/src/chrome/context_menu.rs` — ContextPath, ContextTarget, ContextMenuRegistry, open_context_menu_for, pane_action_items, restorable_mode, 5 unit tests.
- `heca/src/providers/workspaces.rs` — WorkspacesContainerProvider (plugin-03 T2, done earlier).

### Modified source files
- `heca/src/chrome/mod.rs` — `mod context_menu;` + pub(crate) exports.
- `heca/src/chrome/overlay.rs` — mode-restore in `resolve()`.
- `heca/src/mouse.rs` — refactored 3 menu builders to route through `open_context_menu_for`; removed `pane_action_items`.
- `heca/src/app_state.rs` — `overlay_origin_mode` + `context_menu_registry` fields.
- `heca/src/app/startup.rs` — init both fields.
- `heca/src/app/interaction.rs` — `OpenContextMenu` reclassified from `TiledOnly` to `FocusedPaneLocal` (context-menu-3 item 3.2).
- `heca/src/actions.rs` — `OpenContextMenu` default_binding changed from `"."` to `">"` (context-menu-3 item 3.3).
- `heca/src/providers/mod.rs` — `mod workspaces;`.
- `heca-grid-ui/src/widgets/context_menu.rs` — `centered` flag + layout offset (context-menu-3 item 3.2 follow-up).
- `keybindings.default.toml` — `open_context_menu` binding changed from `prefix+.` to `prefix+>`.
- `BACKLOG.md` — context-menu-1/2 marked done; context-menu-5 rewritten with full design; menu-nav requirement updated to exclude sidebar.

### Planner files
- `.planner/features.json` — added Improvements feature (2272d270).
- `.planner/phases/8f760a9b-*.json` — context-menu phase: 7 tasks, cm-d1..cm-d6 decisions, descriptions updated.
- `.planner/phases/73c320aa-*.json` — plugin-03: task reorganization (6 tasks, T1-T4 + selection bridge).
- `.planner/phases/dcf9ff70-*.json` — Notification phase (Improvements feature).
- `.planner/phases/6d582a8a-*.json` — Actions/Keybindings phase (Improvements feature).
- `.planner/resume.json` — inProgressTaskIds updated.
- `.planner/generated/` — 84 files regenerated.

## Current Task Statuses (phase 8f760a9b)
- ✅ context-menu-1 (6da5a4c5) — DropdownSpec + OverlayHost::open_dropdown
- ✅ context-menu-2 (a7226c35) — grid-ui entry widget + ContextMenu::on_dismiss
- 🚧 context-menu-3 (63d4e696) — OpenContextMenu keyboard/RPC action + item polish (3/4 done: only 3.1 bordered keycap via shared paint_keycap remains)
- ✅ context-menu-4 (0af2be02) — Migration onto OverlayHost::open_dropdown
- 📋 context-menu-5 (3724e701) — plugin Contribution::ContextMenu (full design locked, depends on cm-6)
- ✅ context-menu-6 (4f505d9d) — ContextMenuRegistry + open_context_menu_for + mouse refactor (DONE this session)
- 🚧 context-menu-7 (a721d49b) — resolve_active_context + prefix arm + pending_context + mode-restore (IN-PROGRESS, just started, 0% implementation)

## Known Gaps / Notes
- **FloatingPane mouse menu**: the mouse right-click path produces an empty menu for FloatingPane items (latent gap). The keyboard path (context-menu-7) will give a useful pane menu via the "pane" mapping. Noted, not blocking.
- **Harness edit-tool issue**: the `edit` tool can silently fail to apply (returns the reminder but file unchanged). Happened twice this session (mouse.rs, overlay.rs). Always verify edits with re-read (`sed`/`read`) before moving on. When a large multi-edit block fails, retry as smaller single-purpose edits with oldText copied exactly from a fresh read.
- **heca-core flaky test**: a terminal backend test fails intermittently under full-workspace concurrent load (spawns real processes). Passes in isolation (88/88). Preexisting, not caused by this session's changes.
- **context-menu-3 remaining**: only 3.1 (bordered keycap via shared `paint_keycap` in `key_hint.rs`, replacing hand-drawn keycap in `context_menu.rs` ~L95-113). Can be done anytime, independent of cm-6/cm-7.

## How to resume
1. Re-read `.planner/HANDOFF.md` and compare with latest planner data via plan_get / phase_get / task_list.
2. Confirm context-menu-7 (a721d49b) is in-progress. If not, task_start it.
3. Open `heca/src/app/input.rs` (`handle_sidebar_nav_mode` at ~L601) — add the is_prefix arm that transitions to Prefix mode and captures pending_context.
4. Open `heca/src/app_state.rs` — add the pending_context: Option<PendingContext> field.
5. Open `heca/src/app/startup.rs` — init pending_context to None.
6. Open `heca/src/handlers.rs` (~L190 handle_open_context_menu) — consume pending_context if present.
7. Open `heca/src/chrome/context_menu.rs` — implement resolve_active_context() for InputMode::SidebarNav mapping.
8. Run `cargo check`, then `cargo clippy --workspace --all-targets --all-features`, then `cargo test --workspace`.
9. Manual verify: prefix+e (sidebar nav), j/k to select, prefix+> opens context menu centered, Esc returns to SidebarNav (not Normal).

## Files to inspect first
- .planner/project.json
- .planner/features.json
- .planner/phases/8f760a9b-3ce8-4932-8286-67d742397f2e.json
- .planner/resume.json
- .planner/HANDOFF.md
- .planner/generated/PLAN.md

## Blockers
- None recorded

## Next steps
- Continue with plugin-03: Built-in provider system and WorkspacesContainer migration

## Recent activity
- Latest feature update: Improvements (planned) at 2026-07-09T13:15:09.863Z
- Latest phase update: context-menu: Contextual menu → OverlayHost + plugin-declarable (in-progress) at 2026-07-09T14:23:11.329Z
- Latest task update: context-menu-7: resolve_active_context + prefix arm + pending_context + mode-restore (keyboard context-aware) (in-progress) at 2026-07-09T14:23:11.329Z

## Reminder
- When work is fully resumed and this handoff is no longer needed, delete `.planner/HANDOFF.md`.