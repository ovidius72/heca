# Session Resume Handoff — 2026-06-05

## Current status

We are now working **solo** — no more intercom/subagent coordination.

Active branch:
- `feature/gpt-refactoring`

Important workflow state:
- branch has already been **rebased onto `origin/main`** during this session
- current working tree is **clean**
- latest work is committed locally and ready to push / PR

## Latest commits from this session

- `4929e0e` — `docs(sidebar): document sidebar space binding`
- `84bf249` — `feat(sidebar): add sidebar mouse and disclosure behavior`
- `88746ad` — `feat(sidebar): add sidebar mutation actions`
- `16308ca` — `feat(sidebar): normalize sidebar nav contract`
- `66b2079` — `docs(plan): expand sidebar phase 1.5 tasks`

## What was completed

### Planning / checklist work

`bugs-and-refactoring-plan-with-checklist.md` was rewritten for **Phase 1.5** with a much more detailed breakdown based on a long clarification pass.

The 1.5 checklist now captures:
- sidebar navigation contract
- sidebar-only mutation keys
- selection-driven behavior
- mouse enter/exit semantics
- disclosure icon behavior
- future global collapse action family
- future RPC/token compatibility constraints
- tests/docs follow-up

### Sidebar Phase 1.5 implementation progress

Completed in code so far:

#### 1.5.1 Normalize sidebar navigation contract
Implemented:
- `j/k` and `Up/Down` move cursor in sidebar mode
- `h/l` and `Left/Right` do tree collapse/expand on structural rows
- `Enter` behavior was normalized
- `Esc` now exits sidebar mode while focusing contextually relevant content
- leaf focus-target resolution was added for:
  - workspace rows
  - column rows
  - pane rows
  - floating-pane rows
- removed obsolete `cursor_workspace_index()` helper from `SidebarTree`

Key behavior now:
- pane / floating pane leaf activation exits `SidebarNav`
- workspace / column structural navigation stays in `SidebarNav`
- `Esc` exits and focuses contextual target using:
  - workspace active/first pane fallback
  - column active/first pane fallback
  - exact pane/floating pane ids when directly selected

#### 1.5.2 Add sidebar-only mutation keymap
Implemented new sidebar-only actions:
- `SidebarCreateWorkspace`
- `SidebarCreateColumn`
- `SidebarSplitInColumn`
- `SidebarZoomSelectedColumn`
- `SidebarDeleteSelected`

Implemented new sidebar-mode keys:
- `w` → create workspace
- `c` → create column in selected workspace context
- `v` → split/add pane in selected column context
- `z` → zoom selected column
- `d` → delete selected item with confirmation

Behavior details:
- all sidebar mutation actions are **selection-driven**
- `w/c/v/z` stay in `SidebarNav`
- `d` reuses confirm-delete flow and returns to `SidebarNav`
- floating-pane row is a no-op for actions that require a workspace/column target

Also fixed related behavior:
- `ClosePaneById` now handles floating panes too
- workspace deletion now cleans up floating-pane backends too
- `ConfirmDelete` now has `resume_sidebar: bool`

#### 1.5.3 Make sidebar actions selection-driven
Implemented in the same slice as 1.5.2:
- sidebar mutation actions derive targets from current sidebar selection
- they no longer depend on active main-area focus as the target source

#### 1.5.4 Add mouse semantics for entering/exiting sidebar mode
Implemented:
- clicking in sidebar enters `SidebarNav`
- row-body click behavior differs depending on item type and current mode
- pane/floating-pane clicks now support the intended two-step flow:
  - first click enters/selects in sidebar mode
  - second click while already in sidebar mode focuses leaf and exits
- column row click activates parent workspace if needed but stays in `SidebarNav`
- workspace row click activates workspace and stays in `SidebarNav`

Implementation note:
- `SidebarDragStarting.click_action` is now optional
- this was necessary so first pane click can enter sidebar mode without immediately focusing the pane on release

#### 1.5.5 Add disclosure hit targets and visual symbols for workspace + column rows
Implemented:
- column rows now render disclosure arrows too
- symbols used:
  - collapsed = `▶`
  - expanded = `▼`
- disclosure glyph area now gets its own sidebar hit target
- disclosure clicks toggle tree state only
- row-body click remains distinct from disclosure click

### Sidebar-mode default binding update

Added default sidebar-mode behavior:
- `Space` now maps to sidebar `Right` behavior
- on a pane leaf in sidebar mode this means **focus/select + exit**
- `keybindings.toml` was updated to document this default binding

## Validation status

All of the above was validated repeatedly.

Passing commands during this session:
- `cargo test -p heca --quiet`
- `cargo clippy -p heca --all-targets --quiet`

Also earlier in the session:
- `cargo test -p heca-config --quiet`
- `cargo clippy -p heca-config --all-targets --quiet`
- `cargo check --workspace --quiet`

At the current checkpoint, `heca` tests/clippy were green after the latest sidebar changes.

## What is still not done

The remaining **Phase 1.5** items are still open:

- `1.5.6 Add global sidebar-tree collapse actions`
- `1.5.7 Preserve public config/action surface for future RPC work`
- `1.5.8 Add/update tests for sidebar tree behavior`
- `1.5.9 Update docs and defaults`

## Recommended next step

### Start with 1.5.6

Add the global action family for sidebar-tree UI collapse state:

Workspace:
- `collapse_current_workspace`
- `expand_current_workspace`
- `toggle_current_workspace_collapsed`

Column:
- `collapse_current_column`
- `expand_current_column`
- `toggle_current_column_collapsed`

Requirements already agreed in planning:
- these are **sidebar tree UI state** actions only, not compositor/layout collapse
- they must be real `WmAction` variants
- must be registered in `ActionRegistry`
- must be bindable from config
- for now default bindings only needed:
  - `prefix+(` → toggle current column collapsed
  - `prefix+<` → toggle current workspace collapsed
- “current” for these global actions means:
  - active main-view workspace
  - focused pane’s column in active workspace
- they should work even if the sidebar is hidden

### Suggested implementation order for 1.5.6

1. Extend `SidebarTree` with explicit helpers by index:
   - collapse / expand / toggle workspace by `ws_idx`
   - collapse / expand / toggle column by `(ws_idx, col_idx)`
2. Add new `WmAction` variants in `heca/src/input.rs`
3. Add `action_from_name()` mappings
4. Update `action_priority()` exhaustive matches
5. Add `ActionRegistry::ALL` descriptors in `heca/src/actions.rs`
6. Register handlers in `heca/src/app/registry.rs`
7. Implement handlers in `heca/src/handlers.rs`
8. Add default keybindings in `heca-config` and update `keybindings.toml`
9. Add focused tests

## Important semantic agreements already settled

Do **not** re-decide these unless intentionally changing product behavior:

- sidebar navigation is **selection-driven**
- main scrolling area is **not** auto-synced on `j/k`
- `h/l` and `Left/Right` are tree navigation on structural rows
- pane/floating-pane `l` / `Right` / `Enter` focus leaf and exit
- `Esc` exits sidebar mode and focuses contextual target
- sidebar mutation keys are sidebar-only
- global prefix collapse actions use **active main-view state**, not sidebar selection
- sidebar tree collapse is **UI-only**, not layout collapse
- explicit expand/collapse/toggle family should exist for future RPC friendliness, even if only toggles get default bindings now

## Files most relevant for the next slice

- `heca/src/input.rs`
- `heca/src/actions.rs`
- `heca/src/app/registry.rs`
- `heca/src/handlers.rs`
- `heca/src/sidebar/model.rs`
- `heca-config/src/keys.rs`
- `keybindings.toml`
- `bugs-and-refactoring-plan-with-checklist.md`

## Push / PR intent

The user explicitly asked to:
- commit
- push
- create PR
- write detailed handoff

At the time of writing this handoff:
- local commits are ready
- next immediate task is to push branch and create PR
