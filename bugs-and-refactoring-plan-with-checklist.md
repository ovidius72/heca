# Heca — Bugs & Refactoring Implementation Plan

**Scope:** `heca`, `heca-core`, `heca-renderer`, `heca-config`  
**Excluded:** `heca-ui`, `heca-grid-ui`  
**Based on:** `bugs-and-refactoring.md`  
**Date:** 2026-06-04  
**Last updated:** 2026-06-08

---

## Plan Change Notice — 2026-06-05

This roadmap changed on **2026-06-05** after new requirements emerged around:

- a **pluggable chrome system** instead of treating the left sidebar as only a workspace tree
- future **left sidebar / right sidebar / top bar / bottom bar** region hosting
- built-in containers such as a future `WorkspacesContainer`
- a future **WASM plugin system**
- dynamic plugin-provided actions that must later be bindable from config

Those requirements are documented in:

- `pluggable-chrome-plugin-plan.md`

Important scope clarification:

- this document still governs the **current cleanup/refactor program** already in progress
- the new chrome/plugin architecture plan is to start **after** the current refactor reaches its intended stopping point
- after this current refactor is complete enough, this roadmap should be adapted again so its later phases align with the pluggable chrome/plugin architecture

---

## 1. Goals

This plan prioritizes:

1. **Code clarity and maintainability first**
2. **Behavior preservation during early refactors**
3. **Clear ownership boundaries** between layout, UI, input, rendering, and backend lifecycle
4. **NIRI-inspired layout invariants preserved**, without forcing a 1:1 niri architecture
5. **Rust hygiene improvements**: smaller modules, typed boundaries, fewer giant functions, clearer invariants

---

## 2. Guardrails

## Architectural guardrails

- Keep the canonical layout hierarchy:
  - `Session -> Workspace -> ScrollingSpace -> Column -> Pane`
- Do **not** introduce a second layout engine
- Keep layout logic in `heca-core`
- Keep rendering logic in `heca-renderer`
- Keep app orchestration / input dispatch in `heca`
- Preserve the action-registry dispatch model
- Preserve NIRI-inspired behavior where already intentional:
  - horizontal scrolling columns
  - no width normalization
  - focus/view movement relationship
  - immediate action semantics

## Refactoring guardrails

- Phase 1 should be **structure-only** as much as possible
- Every phase must end with:
  - `cargo check`
  - `cargo clippy --workspace --all-targets --all-features`
- Prefer **small PR slices** over mega-refactors
- Add or update tests before changing delicate behavior
- Avoid mixing:
  - structural split
  - semantic behavior changes
  - performance optimization
  in the same step unless necessary

---

## 3. Target End-State

By the end of this roadmap, the code should feel like:

```text
heca/
  app/
    lifecycle.rs
    events.rs
    render.rs
    registry.rs
    focus.rs
    mutations.rs
  mouse/
    hit_test.rs
    drag.rs
    drop.rs
    render.rs
  sidebar/
    model.rs
    projection.rs
    nav.rs
    render.rs
  handlers/
    navigation.rs
    layout.rs
    pane.rs
    workspace.rs
    sidebar.rs
    system.rs

heca-config/
  color.rs
  theme.rs
  settings.rs
  keys.rs
  loader.rs
  defaults.rs
```

And the main runtime responsibilities should be split into:

| Concern | Owner |
|---|---|
| Layout structure and geometry | `heca-core` layout types |
| Backend lifecycle | runtime/backend store |
| Focus/sidebar bookkeeping | app focus/ui state layer |
| Mouse interaction state machine | `heca::mouse` |
| Sidebar projection + rendering | `heca::sidebar` |
| Rendering orchestration | `heca::app::render` |
| Action dispatch glue | `heca::handlers` + app controller |

---

## 4. Phase Overview

| Phase | Theme | Main outcome |
|---|---|---|
| 0 | Safety net | Stronger tests around current behavior |
| 1 | File/module reorganization | Giant files split without major semantic changes |
| 2 | Central mutation boundary | Session mutation post-hooks become explicit |
| 3 | Shared pane operation layer | Focus/move/swap/drop logic deduplicated |
| 4 | Sidebar redesign | Sidebar projection and UI state cleaned up, in a way that can later feed a `WorkspacesContainer` inside a pluggable chrome host |
| 5 | Backend/runtime ownership cleanup | Backend lifecycle less fragile |
| 6 | Dead state and metadata cleanup | Remove stale types, fields, placeholders |
| 7 | Error handling and safety hygiene | Typed errors and `// SAFETY:` comments |
| 8 | Constants, polish, and perf follow-ups | Better ergonomics and targeted perf work |
| 9 | Focus-domain routing correctness | Floating vs tiled action targeting becomes explicit and reliable |

---

## 4A. Global Progress Checklist

This checklist is the **global phase-by-phase tracker** for the refactor program.

Interpretation rules:

- the checklist below tracks **overall program progress**
- the later **Live Execution Checklist** tracks only the **current active slice**
- when a subphase is completed, check the subphase item
- each phase now includes an explicit **test/validation checkpoint** near its end
- when every implementation subphase plus that phase's validation checkpoint is completed and its acceptance intent is satisfied, check the phase item
- do not delete the detailed phase descriptions below; they remain the source of truth for scope and acceptance details
- Phase 10 is the final whole-program verification pass after all refactor phases are complete

### Phase 0 — Safety Net Before Refactoring

- [x] Phase 0 complete
  - [x] 0.1 Audit current tests
  - [x] 0.2 Add high-value behavior tests
  - [x] 0.V Validate Phase 0 safety net coverage

### Phase 1 — Reorganize the Giant Files

- [x] Phase 1 complete
  - [x] 1.1 Split `heca/src/main.rs`
  - [x] 1.2 Split `heca/src/mouse.rs`
  - [x] 1.3 Split `heca/src/sidebar.rs`
  - [x] 1.4 Split `heca-config/src/theme.rs`
  - [x] 1.5 Sidebar mode keymap + tree interaction follow-up
    - [x] 1.5.1 Normalize sidebar navigation contract
      - Sidebar mode is a selection-driven tree navigator for the built-in workspace tree.
      - `j/k` and `Down/Up` move the sidebar cursor only.
      - Moving the sidebar cursor does **not** automatically focus/sync the main scrolling area.
      - `h/l` and `Left/Right` are tree-navigation keys:
        - on workspace/column rows: collapse/expand only
        - on pane/floating-pane rows:
          - `Left` / `h` = no-op
          - `Right` / `l` = activate that leaf item and exit `SidebarNav`
      - `Enter` on pane/floating-pane activates the item, focuses it, and exits `SidebarNav`.
      - `Esc` exits `SidebarNav`.
      - Expand/collapse/navigation actions stay in `SidebarNav`.
    - [x] 1.5.2 Add sidebar-only mutation keymap
      - These bindings exist only in sidebar mode.
      - `w` = create workspace
        - works from workspace/column/pane rows
        - floating-pane row = no-op
      - `c` = create new column in the selected row’s workspace
        - workspace row → create column in that workspace
        - column row → create sibling/new column in that row’s workspace
        - pane row → create column in that pane’s workspace
        - floating-pane row = no-op
      - `v` = add/split a new pane in the selected/current column
        - column row → add pane in that column
        - pane row → add pane in that pane’s column
        - workspace/floating-pane row = no-op
      - `z` = zoom the selected column
        - column row → zoom that column
        - pane row → zoom that pane’s column
        - workspace/floating-pane row = no-op
      - `d` = delete selected sidebar item with confirmation
        - workspace row → delete workspace
        - column row → delete column
        - pane row → delete pane
        - floating-pane row → remove floating pane
        - confirmation from sidebar returns to `SidebarNav`
      - `n` is removed from this phase as redundant.
      - Successful mutation actions stay in `SidebarNav`.
    - [x] 1.5.3 Make sidebar actions selection-driven, not main-focus-driven
      - Sidebar mutation keys act on the currently selected sidebar row context.
      - Main-area active pane/workspace is **not** the target source for sidebar-mode mutation keys.
      - Existing creation semantics should be preserved:
        - `w` uses current workspace-creation behavior
        - `c` uses normal new-column insertion behavior
        - `v` uses normal add-pane/split-in-column behavior
      - No surprise fallbacks to active main focus when the selected row does not imply a valid target.
      - Invalid row/action combinations no-op.
    - [x] 1.5.4 Add mouse semantics for entering/exiting sidebar mode
      - Clicking inside the sidebar should enter `SidebarNav`.
      - Row-body click behavior:
        - workspace row:
          - enter/stay in `SidebarNav`
          - move cursor to that workspace row
          - activate/select that workspace
          - remain in `SidebarNav`
        - column row:
          - enter/stay in `SidebarNav`
          - move cursor to that column row
          - do **not** change main focus yet
        - pane row:
          - if not already in `SidebarNav`:
            - enter `SidebarNav`
            - move cursor to that pane row
            - do **not** focus yet
          - if already in `SidebarNav`:
            - focus/select that pane
            - exit to `Normal`
        - floating-pane row:
          - same activation behavior as normal pane rows
      - This preserves a two-step mouse flow for panes:
        - first click = enter/select in sidebar
        - second click while already in sidebar mode = activate/focus leaf and exit
    - [x] 1.5.5 Add disclosure hit targets and visual symbols for workspace + column rows
      - Add expand/collapse disclosure symbols for column rows.
      - Use the same symbols for workspace and column rows:
        - collapsed = `▶`
        - expanded = `▼`
      - Clicking the disclosure/icon toggles only tree UI state:
        - enters/stays in `SidebarNav`
        - does **not** activate/focus main content
      - Row-body click and disclosure-icon click must remain distinct behaviors.
    - [x] 1.5.6 Add global sidebar-tree collapse actions
      - Introduce real WM actions for sidebar tree UI state:
        - `collapse_current_workspace`
        - `expand_current_workspace`
        - `toggle_current_workspace_collapsed`
        - `collapse_current_column`
        - `expand_current_column`
        - `toggle_current_column_collapsed`
      - These actions affect **sidebar tree UI collapse state only**, not compositor/layout visibility.
      - They must be:
        - added to `WmAction`
        - mapped in `action_from_name()`
        - given explicit priority entries
        - registered in `ActionRegistry`
        - represented in `ActionRegistry::ALL`
        - bindable from `config.toml`
      - For this phase, default keybindings only need:
        - `prefix+(` → `toggle_current_column_collapsed`
        - `prefix+<` → `toggle_current_workspace_collapsed`
      - “current” for these global actions means:
        - current workspace = active main-view workspace
        - current column = focused pane’s column in active workspace
      - These global actions should still work when the sidebar is hidden or not focused.
    - [x] 1.5.7 Preserve public config/action surface for future RPC work
      - Even if only toggle variants are default-bound now, the explicit expand/collapse action family should exist now.
      - This keeps the action surface ready for later RPC/target-token work.
      - Future token targeting (`$workspaceIndex`, `$paneIndex`, `current`, etc.) is deferred to the later phase added in `pluggable-chrome-plugin-plan.md` section 8.1.
      - RPC command parsing now exposes the new actions via `heca/src/rpc.rs`.
      - Merged into `main` via PR #27.
    - [x] 1.5.8 Add/update tests for sidebar tree behavior
      - Add tests for sidebar-mode key resolution:
        - `j/k`, `Up/Down`
        - `h/l`, `Left/Right`
        - `w/c/v/z`
      - Add tests for:
        - pane leaf activation exits `SidebarNav`
        - workspace/column expand-collapse stays in `SidebarNav`
        - disclosure click toggles only tree state
        - row-body click behavior differs from icon click behavior
        - global current-workspace/current-column collapse actions
        - behavior when sidebar is hidden
      - Preserve or extend existing collapse-persistence coverage.
    - [x] 1.5.9 Update docs and defaults
      - Update default keybindings in `heca-config`
      - Update README/keybinding docs for sidebar mode
      - Document that:
        - sidebar-mode mutation keys are sidebar-only
        - global prefix collapse bindings act on active main-view state
        - sidebar tree collapse is UI-only, not layout collapse
    - [x] 1.5.10 Make sidebar nav bindings configurable from `config.toml`
      - Built-in sidebar-nav defaults now live in `[[keys.mode]] name = "sidebar"`.
      - User configs can override sidebar-nav defaults by redefining that mode's bindings.
      - `SidebarNav` still uses the built-in `sidebar` mode name; its trigger field is ignored because sidebar mode is entered via `SidebarFocus` or mouse interaction.
  - [x] 1.5 complete — sidebar mode keymap + tree interaction follow-up
  - [x] 1.V Validate file/module reorganization invariants

Short description:

- the 1.5 block above is a clarified sidebar follow-up implementation plan based on agreed keyboard, mouse, action-registry, config-binding, and future RPC/token semantics gathered during planning discussion

### Phase 2 — Introduce a Central Mutation Boundary

- [x] Phase 2 complete
  - [x] 2.1 Create app-level mutation helpers
  - [x] 2.2 Standardize handler endings
  - [x] 2.V Validate centralized mutation/post-hook behavior

### Phase 3 — Extract Shared Pane Operation Logic

- [x] Phase 3 complete
  - [x] **3.1** Create shared pane-ops layer (`heca/src/app/pane_ops.rs`) — done in prior session
    - `insert_pane_at_position`, `remove_pane_by_id`, `swap_panes_same_column`,
      `swap_panes_same_workspace`, `swap_panes_cross_workspace`, `move_pane_between_columns`
  - [x] **3.2** Simplify handlers to dispatchers — **complete**
    - [x] Created `mouse/target.rs` enum dispatch layer (DragSurfaceId → surface handler)
    - [x] Extracted `mouse/interactive.rs` (content-area drag from drag.rs)
    - [x] Deleted `mouse/sidebar.rs` + `mouse/sidebar_drop.rs`, replaced with `surface_left.rs`
    - [x] Replaced `DragState` enum with `DragContext` (per-surface state) + `InteractiveMovePhase`
    - [x] Wired `target.rs` dispatch into app event routing (`mouse.rs`, `release.rs`)
    - [x] `handle_swap_param()` delegates to `swap_panes_same_column`/`diff_columns`/`cross_workspace` — done
    - [x] `handle_interactive_move_drop` swap case delegates to `handle_swap_param` — done
    - [x] `place_pane_at_sidebar_target` uses `insert_pane_at_position` for all variants — done
    - [x] `accept_drop` uses `find_pane_indices_in_workspace` instead of ad hoc scan — done
  - [x] **3.3** Reduce cross-file ad hoc search logic — **complete**
    - `handle_float`, `handle_float_at`, `handle_close_pane_by_id`, `move_pane_to_workspace_column`
      now use `find_pane_indices_in_workspace` instead of manual iteration
    - Sidebar-internal scans in `sidebar/model.rs` and `sidebar/render.rs` noted as
      projection-specific, deferred to Phase 4

### Phase 4 — Redesign Sidebar Projection and Interaction Model

- [x] Phase 4 complete
  - [x] 4.1 Preserve UI state across rebuilds — collapsed state persisted via prev_*_collapsed maps
  - [x] 4.2 Separate projection rows from interaction rules — SidebarItemKind, is_selectable(), is_expandable()
  - [x] 4.3 Introduce `sync()` semantics — rebuild → sync_from_session, rebuild_flat_items → sync_flat_items
  - [x] 4.4 Performance/readability cleanup — Vec::with_capacity, HashMap pane-id map for O(1) render lookups,
       replaces 15-line O(n) scan per pane
  - [x] 4.V Validate — all 204 tests pass, clippy clean

### Phase 5 — Backend Runtime Ownership Cleanup

- [x] Phase 5 complete
  - [x] 5.1 Wrap backend storage — `BackendStore` struct wrapping `HashMap`, methods `insert_for_pane`/`remove_for_pane`/`get`/`get_mut`/`values_mut`, updated 16 call sites across 5 files
  - [x] 5.2 Isolate lifecycle rules — `remove_all()` batch helper for column/workspace delete, lifecycle contract documented in `BackendStore`
  - [x] 5.V Validate — all 204 tests pass, clippy clean

### Phase 6 — Remove Stale, Dormant, and Drifting State

- [x] Phase 6 complete
  - [x] 6.1 Remove dead legacy geometry — migrated Rect→Rectangle, deleted heca-core/src/types.rs
  - [x] 6.2 Review dormant fields — removed floating_visible, is_pinned, active_tab, tab_names; removed dormant tab bar loop from render
  - [x] 6.3 Remove placeholder backend variants — removed PaneType::Neovim/Browser, BackendRenderData::Neovim/Browser; fixed irrefutable if let→let
  - [x] 6.4 Fix action metadata drift — fixed swap_and_focus→swap_and_focus_pane, removed swap_select/tab_next/tab_prev, added delete_column/delete_workspace/reload_config
  - [x] 6.5 Review #[allow(dead_code)] — replaced broad #![allow(dead_code)] in actions.rs with targeted per-item allows + reason comments
  - [x] 6.V Validate — all 204 tests pass, clippy clean

### Phase 7 — Typed Errors and Unsafe Hygiene

- [x] Phase 7 complete
  - [x] 7.1 Add typed errors — ConfigError, PtyError, RpcError replace Result<_, String>
  - [x] 7.2 Improve action dispatch failure — debug-assert on missing handler in execute()
  - [x] 7.3 Add // SAFETY: comments to all 11 unsafe blocks in terminal.rs
  - [x] 7.V Validate — all 205 tests pass, clippy clean

### Phase 8 — Constants, Polish, and Performance Follow-Ups

- [x] Phase 8 complete
  - [x] 8.1 Centralize constants — 9 new named constants in chrome.rs, replaced 13 ColumnWidth::Proportion(0.5) + 5 sidebar thresholds + 2 timing values + 4 pane name sizes + 1 chrome text size + 1 edge scroll trigger
  - [x] 8.2 Clarify renderer API — documented draw_rounded_rect radius is ignored, fixed PrimitiveRenderer module doc
  - [x] 8.3 Revisit terminal render-data cloning — DEFERRED (terminal backend not wired, no profiling data)
  - [x] 8.4 Improve docs — added invariant docs to AppState and ActionRegistry
  - [x] 8.V Validate — all 205 tests pass, clippy clean

### Phase 9 — Fix Floating vs Tiled Focus-Domain Routing

- [x] Phase 9 complete
  - [x] 9.1 Define the focus domain explicitly
  - [x] 9.2 Define action policy by domain
  - [x] 9.3 Route handlers through the domain guard
  - [x] 9.4 Centralize focused-pane targeting helpers
  - [x] 9.5 Add regression tests for domain routing
  - [ ] 9.V Validate floating vs tiled action targeting correctness (manual smoke test)

### Phase 10 — Final Whole-Program Verification

- [ ] Phase 10 complete
  - [x] 10.1 Run full workspace validation (`cargo check`, `cargo clippy`, `cargo test`)
  - [ ] 10.2 Run focused smoke tests for keybindings, sidebar/workspace behavior, floating/tiled behavior, and config reload
  - [x] 10.3 Reconcile docs/checklists/roadmaps with final refactor state
  - [x] 10.4 Confirm no phase-level regressions remain open without explicit deferment

---

## Phase 0 — Safety Net Before Refactoring

**Goal:** lock current expected behavior before moving code around.

### 0.1 Audit current tests

**Tasks**

- Inventory tests in:
  - `heca/src/sidebar.rs`
  - `heca/src/mouse.rs`
  - `heca-core/src/layout/session.rs`
  - `heca/src/input.rs`
  - `heca/src/keymap.rs`
- Identify behavior with no direct test coverage:
  - cross-workspace pane swap
  - floating ↔ tiled transitions
  - sidebar display-only floating rows
  - collapse persistence across rebuilds
  - pane drop insertion targets

**Deliverable**

- A short test-gap checklist added as comments/TODO in the relevant modules or a separate scratch note.

### 0.2 Add high-value behavior tests

**Tasks**

- Add tests for sidebar collapse persistence after rebuild
- Add tests for:
  - focus pane by id for tiled pane
  - focus pane by id for floating pane
- Add tests for move/swap in the 3 major cases:
  - same column
  - same workspace different columns
  - different workspaces
- Add tests for display-only floating rows:
  - rendered in projection
  - not navigable
  - not hit-test selectable

**Acceptance criteria**

- Key interaction invariants are covered before major file movement begins.

---

## Phase 1 — Reorganize the Giant Files

**Goal:** improve readability without changing behavior.

**Priority:** highest

---

### 1.1 Split `heca/src/main.rs`

Current problems:

- mixes app lifecycle, event handling, rendering, mutation helpers, focus bookkeeping, keymap building, registry wiring

**Target split**

- `heca/src/app/mod.rs`
- `heca/src/app/lifecycle.rs`
- `heca/src/app/events.rs`
- `heca/src/app/render.rs`
- `heca/src/app/registry.rs`
- `heca/src/app/focus.rs`
- `heca/src/app/mutations.rs`

**Status update — 2026-06-04**

Already extracted:

- `heca/src/app/registry.rs`
  - `build_registry()`
  - `build_keymap()`
  - `build_modes()`
- `heca/src/app/focus.rs`
  - `find_pane_workspace()`
  - `focus_pane_by_id()`
  - `switch_workspace_tracked()`
  - `sync_focus()`
- `heca/src/app/mutations.rs`
  - `move_pane_to_workspace_column()`
  - `move_pane_to_column()`
  - `move_column_to_workspace()`
  - `destroy_empty_workspace()`
- `heca/src/app/render.rs`
  - `render_backend_data()`
  - `update_session_viewport()`
  - `status_mode_parts()`
- `heca/src/app/selection.rs`
  - `collect_all_pane_candidates()`
  - `find_pane_location()`
- `heca/src/app/keyboard.rs`
  - `normalize_key_text()`
  - `event_combo_matches()`
  - `build_event_combo()`
  - `is_prefix_match()`
  - `typed_candidate_char()`
  - `winit_key_to_terminal_input()`
- `heca/src/app/startup.rs`
  - first-launch window / wgpu / session / fake-backend initialization
- `heca/src/app/input.rs`
  - rename / confirm-delete / prefix / chord / custom-mode / pane-select / pane-swap / pane-take / sidebar-nav keyboard handling

Current state:

- `heca/src/main.rs` is down to **168 LOC**.
- `render()` has been extracted into `app/render.rs`.
- `window_event()` has been extracted into `app/events.rs`.
- `about_to_wait()` has been extracted into `app/lifecycle.rs`.
- Validation passes with:
  - `cargo fmt`
  - `cargo check -q`
  - `cargo clippy --workspace --all-targets --all-features --quiet`

Remaining work in this phase:

- Optional cleanup only:
  - move `pane_name()` into a tiny helper if desired
  - further split `app/render.rs` internally if navigation becomes hard
- Treat the main acceptance target for Phase 1.1 as **met** and move to the next phase unless a follow-up cleanup is needed.

**Tasks**

- Move `render_backend_data()` into `app/render.rs`
- Move the frame `render()` implementation into `app/render.rs`
- Move `window_event()` into `app/events.rs`
- Move `about_to_wait()` into `app/lifecycle.rs` or `app/events.rs`
- Move `build_registry()` into `app/registry.rs`
- Move `build_keymap()` / `build_modes()` into `app/registry.rs` or `app/input_config.rs`
- Move helper functions out of `main.rs`:
  - `find_pane_workspace()`
  - `collect_all_pane_candidates()`
  - `find_pane_location()`
  - `focus_pane_by_id()`
  - `switch_workspace_tracked()`
  - `sync_focus()`
  - `update_session_viewport()`
  - `move_pane_to_workspace_column()`
  - `move_pane_to_column()`
  - `destroy_empty_workspace()`
- Keep `main.rs` as a thin binary entrypoint and module root only

**Acceptance criteria**

- `main.rs` drops below ~400 LOC
- no semantic changes intended
- all moved symbols still have clear owners and module docs

---

### 1.2 Split `heca/src/mouse.rs`

Current problems:

- combines hit testing, drag state transitions, sidebar click logic, drop logic, rendering helpers, edge scrolling

**Target split**

- `heca/src/mouse/mod.rs`
- `heca/src/mouse/hit_test.rs`
- `heca/src/mouse/drag.rs`
- `heca/src/mouse/drop.rs`
- `heca/src/mouse/render.rs`
- `heca/src/mouse/sidebar.rs`

**Status update — 2026-06-05**

Already extracted:

- `heca/src/mouse/hit_test.rs`
  - `hit_test_pane()`
  - `sidebar_pane_hit_test()`
- `heca/src/mouse/render.rs`
  - `render_detached_pane()`
  - `render_insert_hint()`
- `heca/src/mouse/drag.rs`
  - `on_cursor_moved()` internals
  - `update_sidebar_drag_hover()`
  - `start_interactive_move()`
  - `transition_to_moving()`
  - `cancel_interactive_move()`
- `heca/src/mouse/drop.rs`
  - `drop_pane()`
- `heca/src/mouse/sidebar.rs`
  - sidebar click routing
- `heca/src/mouse/sidebar_drop.rs`
  - sidebar drag-drop move/swap behavior
  - sidebar-targeted detached-pane drop handling
- `heca/src/mouse/tests.rs`
  - helper-focused mouse unit tests moved out of the main module file

Current state:

- `heca/src/mouse.rs` is down to **301 LOC**
- extracted submodules currently measure:
  - `mouse/hit_test.rs` — **102 LOC**
  - `mouse/render.rs` — **177 LOC**
  - `mouse/drag.rs` — **310 LOC**
  - `mouse/drop.rs` — **198 LOC**
  - `mouse/sidebar.rs` — **87 LOC**
  - `mouse/sidebar_drop.rs` — **396 LOC**
  - `mouse/tests.rs` — **153 LOC**
- public mouse entrypoints still live in `heca/src/mouse.rs`, but the file now reads mostly as event glue plus small shared helpers
- validation passes with:
  - `cargo fmt`
  - `cargo check -q`
  - `cargo clippy --workspace --all-targets --all-features --quiet`

Why this is now considered structurally complete:

- the giant mixed-responsibility `mouse.rs` file has been decomposed into domain-focused modules
- no mouse submodule is above the target ~400 LOC threshold
- click routing, hit testing, drag transitions, content drop logic, sidebar drop logic, rendering helpers, and tests now have separate homes

**Remaining tasks in this phase**

- only optional polish remains, such as extracting tiny shared helpers if future edits reveal repetition pressure
- otherwise treat Phase 1.2’s structural goal as met and move to `sidebar.rs`

**Acceptance criteria**

- no single mouse submodule contains more than ~400 LOC
- drag/drop code becomes locally navigable
- `mouse.rs` mainly reads as top-level event glue instead of a full state-machine implementation
- **Status:** effectively met

---

### 1.3 Split `heca/src/sidebar.rs`

**Important architectural note — updated 2026-06-05**

This phase should still proceed as a structure-first cleanup, but its output should now be understood as preparation for a future built-in `WorkspacesContainer`, not as the final architecture of "the sidebar" itself.

The new longer-term model is:

- `Sidebar` / chrome region host
- pluggable containers inside that host
- `WorkspacesContainer` as one such container

That future architecture is documented in `pluggable-chrome-plugin-plan.md` and starts only after the current cleanup roadmap reaches its stopping point.

Current problems:

- combines data model, projection rebuild, navigation, hit testing, rendering, tests

**Target split**

- `heca/src/sidebar/mod.rs`
- `heca/src/sidebar/model.rs`
- `heca/src/sidebar/projection.rs`
- `heca/src/sidebar/nav.rs`
- `heca/src/sidebar/hit_test.rs`
- `heca/src/sidebar/render.rs`

**Tasks**

- Move enum/struct definitions to `model.rs`
- Move `rebuild()` / `rebuild_flat_items()` into `projection.rs`
- Move cursor movement and expand/collapse into `nav.rs`
- Move `sidebar_hit_test()` and button hit test into `hit_test.rs`
- Move expanded/collapsed rendering into `render.rs`
- Keep tests split by concern where possible

**Acceptance criteria**

- the sidebar projection and rendering are readable without scrolling through test code and navigation code intermixed

---

### 1.4 Split `heca-config/src/theme.rs`

Current problems:

- theme definitions, settings config, key config, config loading, defaults, tests all mixed

**Target split**

- `heca-config/src/color.rs`
- `heca-config/src/theme.rs`
- `heca-config/src/settings.rs`
- `heca-config/src/keys.rs`
- `heca-config/src/loader.rs`
- `heca-config/src/defaults.rs`

**Status update — 2026-06-05**

Already extracted:

- `heca-config/src/color.rs`
- `heca-config/src/theme.rs`
- `heca-config/src/settings.rs`
- `heca-config/src/keys.rs`
- `heca-config/src/loader.rs`
- `heca-config/src/defaults.rs`

Current state:

- `theme.rs` is now focused on theme declarations/re-exports instead of acting as the whole config module
- config loading, key schema, settings, defaults, and color parsing each have an obvious home
- validation passes with:
  - `cargo check -q`
  - `cargo clippy --workspace --all-targets --all-features --quiet`
  - `cargo test -q --workspace`

Remaining work in this phase:

- Treat the structural goal for Phase 1.4 as **met** and move to Phase 2 unless a later naming cleanup is needed.

**Tasks**

- move `Color` + parsing to `color.rs`
- move palette/theme definitions to `theme.rs`
- move `SettingsConfig` to `settings.rs`
- move keybinding schema to `keys.rs`
- move config loading to `loader.rs`
- move default builder helpers to `defaults.rs`

**Acceptance criteria**

- each config concern has an obvious home
- the runtime config loader is separate from static theme declarations

---

## Phase 2 — Introduce a Central Mutation Boundary

**Goal:** make post-mutation behavior explicit and uniform.

Current problem:

- many code paths must remember to call `sync_focus(state)` and set redraw manually

### 2.1 Create app-level mutation helpers

**Tasks**

- introduce a small app orchestration layer, e.g.:
  - `after_layout_change(state)`
  - `after_focus_change(state)`
  - `after_config_change(state)`
- define what each hook guarantees
- remove direct scattered `sidebar_tree.rebuild(...)` calls outside the central hook

**Suggested contract**

```rust
fn after_layout_change(state: &mut AppState) {
    sync_focus(state);
    state.needs_redraw = true;
}
```

### 2.2 Standardize handler endings

**Tasks**

- update handlers to end through the central post-hook
- update mouse drag/drop mutation paths to use the same hook
- update direct event-driven focus changes to use the same hook

**Acceptance criteria**

- session-changing paths do not manually duplicate sync/redraw behavior
- number of direct `sync_focus(state)` call sites drops significantly

---

## Phase 9 — Fix Floating vs Tiled Focus-Domain Routing

**Goal:** make the active mutation target explicit so floating focus cannot accidentally mutate the tiled layout underneath.

Current problem:

- the app can correctly track that a floating pane is focused
- but many handlers still mutate `ws.scrolling...` directly
- this creates a mismatch between **visual/logical focus** and **action target**

### 9.1 Define the focus domain explicitly

**Tasks**

- introduce a small authoritative concept such as:
  - `FocusDomain::Tiled`
  - `FocusDomain::Floating`
- make `Workspace` and/or the app controller expose the current domain cleanly
- document how domain is derived from:
  - `floating_is_active`
  - active floating pane state
  - active tiled selection state

**Acceptance criteria**

- code can ask one authoritative question: "what domain is active right now?"
- handler code no longer infers that ad hoc from scattered state

### 9.2 Define action policy by domain

**Tasks**

- classify actions into categories:
  - focus/navigation
  - pane-local actions
  - tiled-layout-only actions
  - future floating-layout actions
- define expected behavior when `FocusDomain::Floating` is active:
  - close should target the focused floating pane
  - rename should target the focused floating pane
  - tiled-only actions such as column zoom/column resize/swap/move should no-op unless explicitly designed otherwise
- document the policy near the code, not only in roadmap notes

**Acceptance criteria**

- there is one written policy for what actions do in floating vs tiled context
- new handlers have a clear contract to follow

### 9.3 Route handlers through the domain guard

**Tasks**

- audit handlers that currently access:
  - `ws.scrolling.active_column_idx`
  - `ws.scrolling.active_column()`
  - `ws.scrolling.columns[...]`
  as the assumed target
- update them to branch through the active focus domain first
- begin with confirmed risky handlers:
  - `handle_zoom_column`
  - `handle_resize_increase`
  - `handle_resize_decrease`
  - `handle_pane_height_increase`
  - `handle_pane_height_decrease`
  - `handle_swap_left`
  - `handle_swap_right`
  - `handle_swap_up`
  - `handle_swap_down`
  - `handle_move_pane_left`
  - `handle_move_pane_right`
  - `handle_close_pane`
  - `handle_close_pane_by_id`

**Acceptance criteria**

- a focused floating pane cannot trigger tiled mutations underneath by accident
- tiled handlers become explicit about whether they no-op or retarget in floating context

### 9.4 Centralize focused-pane targeting helpers

**Tasks**

- add helpers for operations that should target the truly focused pane regardless of domain
- candidates include:
  - `focused_pane_id(...)`
  - `close_focused_pane(...)`
  - `rename_focused_pane(...)`
  - `remove_focused_pane(...)`
- reduce duplicated "scan tiled, else scan floating" logic in handlers

**Acceptance criteria**

- pane-local actions no longer depend on the tiled active column as a proxy for focus
- focused-pane behavior is consistent across tiled and floating states

### 9.5 Add regression tests for domain routing

**Tasks**

- add tests proving:
  - floating focus is visually and logically active
  - zoom/resize/move/swap do not mutate the tiled target underneath when floating is focused
  - close removes the floating pane when the floating pane is focused
  - restoring tiled focus re-enables tiled handlers
- prefer a combination of:
  - unit tests in layout/app helpers where possible
  - focused handler-level tests for action routing

**Acceptance criteria**

- the current bug becomes impossible to reintroduce silently

---

## Phase 3 — Extract Shared Pane Operation Logic

**Goal:** make move/swap/focus behavior canonical and reusable.

Current problem:

- keyboard, mouse, and sidebar each contain overlapping pane operation logic

### 3.1 Create a shared pane-ops layer

**Suggested module**

- `heca/src/app/pane_ops.rs` or `heca/src/ops/panes.rs`

**Tasks**

- extract pure/shared helpers for:
  - `focus_pane_by_id(...)`
  - `insert_pane_after_target(...)`
  - `move_pane_between_columns(...)`
  - `move_pane_between_workspaces(...)`
  - `swap_panes_same_column(...)`
  - `swap_panes_same_workspace(...)`
  - `swap_panes_cross_workspace(...)`
  - `remove_floating_or_tiled_pane(...)`
  - `reinsert_detached_pane(...)`

### 3.2 Simplify handlers to dispatchers

**Tasks**

- refactor `handle_swap_param()` to delegate to smaller shared helpers
- refactor move-related handlers to use shared ops
- refactor mouse drop code to call the same operations

### 3.3 Reduce cross-file ad hoc search logic

**Tasks**

- avoid repeating manual scans through:
  - workspaces
  - columns
  - panes
- centralize pane lookup helpers or location structs

**Acceptance criteria**

- `handle_swap_param()` shrinks dramatically
- `drop_pane()` / `sidebar_handle_drop()` / `sidebar_drag_drop()` lose duplicated reinsertion logic
- move/swap semantics are defined once per case

---

## Phase 4 — Redesign Sidebar Projection and Interaction Model

**Goal:** make the sidebar explicit, predictable, and maintainable.

### 4.1 Preserve UI state across rebuilds

**Tasks**

- preserve workspace collapsed state by `ws_idx`
- preserve column collapsed state by `(ws_idx, col_idx)`
- keep cursor and scroll position stable where possible

### 4.2 Separate projection rows from interaction rules

**Tasks**

- model row interactivity explicitly:
  - selectable
  - display-only
  - drop target
  - expandable
- remove special-case floating-row skip logic scattered across multiple functions

### 4.3 Introduce `sync()` semantics

**Tasks**

- rename or redesign `rebuild()` into something like `sync_from_session(...)`
- make it clear that sidebar tree is a projection of session + preserved UI state

### 4.4 Performance/readability cleanup

**Tasks**

- use `Vec::with_capacity` where rebuild cost is obvious
- avoid repeated O(n) lookups during render when feasible
- consider a temporary pane-id map during rebuild or render

**Acceptance criteria**

- collapsed state survives normal use
- floating display-only rows do not require ad hoc skip logic in many places
- sidebar code documents its model clearly

---

## Phase 5 — Backend Runtime Ownership Cleanup

**Goal:** make pane/backend lifecycle less fragile.

### 5.1 Wrap backend storage

**Tasks**

- replace raw `HashMap<u64, Box<dyn PaneBackend>>` access with a small owner type:
  - `BackendStore`
- expose narrow operations:
  - `insert_for_pane`
  - `remove_for_pane`
  - `get`
  - `get_mut`
  - iteration for updates

### 5.2 Isolate lifecycle rules

**Tasks**

- move backend insert/remove to the same orchestration layer that owns pane creation/destruction
- document which session mutation paths must remove pane backends

### 5.3 Optional deeper follow-up

**Tasks**

- evaluate whether pane creation/destruction should return typed lifecycle events that the runtime applies to backends

**Acceptance criteria**

- handlers no longer manipulate raw backend map everywhere
- pane/backend lifecycle contract is explicit

---

## Phase 6 — Remove Stale, Dormant, and Drifting State

**Goal:** reduce cognitive noise.

### 6.1 Remove dead legacy geometry

**Tasks**

- migrate remaining `heca_core::types::Rect` users to `heca-core/src/layout/types.rs::Rectangle`
- specifically replace the current `Rect` usage in `heca/src/chrome.rs`
- remove `heca-core/src/types.rs::Rect` once no real code depends on it
- ensure app/layout geometry uses `layout/types.rs` consistently unless a different geometry type is explicitly justified

### 6.2 Review dormant fields

**Tasks**

- evaluate and either remove or justify:
  - `Workspace::floating_visible`
  - `Workspace::is_pinned`
  - placeholder tab state in `AppState`

### 6.3 Remove placeholder backend variants

**Tasks**

- remove or gate:
  - `PaneType::Neovim`
  - `PaneType::Browser`
  - `BackendRenderData::Neovim`
  - `BackendRenderData::Browser`

### 6.4 Fix action metadata drift

**Tasks**

- reconcile `ActionRegistry::ALL` with actual action surface
- remove stale descriptors or add missing authoritative mapping
- decide whether metadata catalog is:
  - only user-facing actions, or
  - full action surface

### 6.5 Review `#[allow(dead_code)]`

**Tasks**

- remove stale suppressions
- keep only justified ones with a reason comment
- especially review:
  - broad module-level allow in `actions.rs`
  - RPC transitional code
  - keymap helpers

**Acceptance criteria**

- fewer placeholder concepts for readers to mentally filter out
- metadata and real action surface no longer drift

---

## Phase 7 — Typed Errors and Unsafe Hygiene

**Goal:** improve correctness boundaries and Rust quality.

### 7.1 Add typed errors where boundaries are stable

**Done.**

- `ConfigError` in `heca-config/src/loader.rs`: NotFound, Parse { path, source }, Io { path, source }
- `PtyError` in `heca-core/src/backend/terminal.rs`: OpenPtyFailed, DupSlaveFailed, SpawnFailed(io::Error), DupMasterFailed, NoStdio(&'static str)
- `RpcError::NotInitialized` added for app-not-ready state
- `execute_rpc_command` returns `Result<WmAction, RpcError>` instead of `String`
- `load_config_file()` now surfaces I/O errors as `ConfigError::Io` (previously swallowed by `if let Ok`)
- Debug-build `eprintln!` on config load failure

**Rust skill review findings (fixed):**
- `ConfigError::Io` was dead code — fixed by matching on `read_to_string` result instead of `if let Ok`
- `PtyError::source()` used `_ => None` catch-all — fixed to exhaustive match

### 7.2 Improve action dispatch failure behavior

**Done.**

- `ActionRegistry::execute()` now panics in debug builds when no handler is registered
- This catches registration bugs early — every `WmAction` variant must have a handler in `build_registry()`
- Release builds silently skip, preserving the current no-crash contract
- No API change — `execute()` still returns `()`

**Rust skill review finding (fixed):**
- Duplicate doc comment line removed

### 7.3 Add `// SAFETY:` comments to all unsafe blocks

**Done.**

- All 15 `unsafe` sites in `heca-core/src/backend/terminal.rs` documented with `// SAFETY:` comments
- Comments explain: fd validity, ownership transfer (from_raw_fd), pointer validity, syscall invariants, cleanup order (close slave after child inherits)

**Acceptance criteria**

- stable subsystem APIs have typed error boundaries
- all unsafe blocks have safety justifications

---

## Phase 8 — Constants, Polish, and Performance Follow-Ups

**Goal:** improve ergonomics after structure is fixed.

### 8.1 Centralize constants

**Tasks**

- name and collect constants such as:
  - prefix timeout
  - frame wait interval
  - sidebar row/button metrics
  - resize increments
  - pane size defaults
  - swap animation clamp ratios

### 8.2 Clarify renderer API truthfulness

**Tasks**

- either implement real rounded corners in `draw_rounded_rect()`
- or rename/document it so the API matches the behavior

### 8.3 Revisit terminal render-data cloning

**Tasks**

- redesign `TerminalBackend::render_data()` to reduce full-grid cloning
- evaluate borrowed render views or cached staging structures

### 8.4 Improve docs

**Tasks**

- add module-level `//!` docs to new split modules
- document invariants on key app/runtime structs

**Acceptance criteria**

- magic numbers mostly disappear from hot paths
- renderer API names reflect real behavior
- next wave of performance work becomes easier

---

## 5. Suggested PR Slices

To keep reviewable PRs, use this order:

### PR 1 — Split `main.rs`

- no semantic changes
- just move code into `app/*`

### PR 2 — Split `mouse.rs`

- no semantic changes
- keep tests green

### PR 3 — Split `sidebar.rs`

- no semantic changes yet

### PR 4 — Split `theme.rs`

- config/schema separation only

### PR 5 — Introduce `after_layout_change()` style post-hook

- remove duplicated sync/redraw endings

### PR 6 — Extract pane op helpers

- shrink `handle_swap_param()`
- shrink mouse drop logic

### PR 7 — Sidebar projection state preservation

- collapse persistence
- interaction row typing

### PR 8 — BackendStore

- wrap backend map

### PR 9 — Dead state cleanup

- `Rect`, placeholders, metadata drift, suppressions

### PR 10 — Typed errors + `// SAFETY:` comments

### PR 11 — Constants + renderer API polish + render-data follow-ups

### PR 12 — Fix focus-domain routing for floating vs tiled

- add explicit focus-domain guard
- make floating-focused actions stop mutating tiled targets underneath

---

## 6. Success Metrics

The roadmap is succeeding when:

### Structure metrics

- `main.rs` < 400 LOC
- `mouse.rs`, `sidebar.rs`, `handlers.rs` split into domain-focused modules
- no single non-test function > ~150 LOC except where strongly justified

### Ownership metrics

- fewer direct raw mutations of `AppState` internals from unrelated code
- backend lifecycle no longer manipulated via raw map in many handlers
- session mutation post-hooks centralized

### Readability metrics

- clear module names match concerns
- fewer ad hoc pane tree scans duplicated across files
- sidebar model clearly distinguishes display-only vs interactive rows

### Quality metrics

- `cargo check` clean
- `cargo clippy --workspace --all-targets --all-features` clean
- no stale action metadata drift
- unsafe blocks documented

---

## 7. Recommended Immediate Next Step

**Start Phase 2.1: create app-level mutation helpers.**

Immediate next slices:

1. add a small post-mutation helper contract in the app layer
2. centralize the common `sync_focus(state); state.needs_redraw = true;` tail
3. remove extra manual `sidebar_tree.rebuild(...)` paths where the shared helper can own that responsibility
4. keep behavior unchanged while shrinking handler/mouse duplication

Reason:

- Phase 1.3’s live slice is complete
- Phase 1.4 is now structurally complete in code
- fresh validation on **2026-06-05** passes:
  - `cargo check -q`
  - `cargo clippy --workspace --all-targets --all-features --quiet`
  - `cargo test -q --workspace`
- the biggest remaining refactor hotspot is the scattered post-mutation contract across handlers, mouse drop paths, and sidebar focus flows

---

## 8. Live Execution Checklist — Current Central Mutation Boundary Slice

This section is intentionally tactical. It exists so the active refactor can be tracked step by step inside the main plan document instead of only in ad hoc chat notes.

Important: this is **not** the global refactor completion board. The global status lives in **4A. Global Progress Checklist** above.

Rule for this checklist:

- update it after each completed slice/commit
- keep it behavior-preserving unless a later phase explicitly says otherwise
- use it to track the current active slice, but preserve completed slice history when it is still useful context

### Preserved completed slice history — Phase 1.3 `heca/src/sidebar.rs`

Short description:

- this completed checklist is intentionally preserved for history so prior refactor commit slices do not disappear from the plan document

#### Commit 1 — Extract rendering module

- [x] Extract sidebar data structures into `heca/src/sidebar/model.rs`
- [x] Extract sidebar hit testing into `heca/src/sidebar/hit_test.rs`
- [x] Create `heca/src/sidebar/render.rs`
- [x] Move `render_sidebar_expanded(...)` into `render.rs`
- [x] Move `render_sidebar_collapsed(...)` into `render.rs`
- [x] Move render-only constants into `render.rs`
- [x] Move render-only helpers (`pane_entry_by_id`, `pane_name_short`) into `render.rs` or a small render support module
- [x] Re-export render entrypoints from `heca/src/sidebar.rs`
- [x] Validate with `cargo check -q`
- [x] Validate with `cargo clippy --workspace --all-targets --all-features --quiet`

#### Commit 2 — Split render internals into helpers

- [x] Extract repeated button color helpers
- [x] Extract workspace row rendering helper(s)
- [x] Extract column row rendering helper(s)
- [x] Extract pane row rendering helper(s)
- [x] Extract floating-pane row rendering helper(s)
- [x] Extract collapsed-section rendering helper(s)
- [x] Confirm no behavior changes in ordering, colors, candidate letters, drag highlights, or actions
- [x] Validate with `cargo check -q`
- [x] Validate with `cargo clippy --workspace --all-targets --all-features --quiet`

#### Commit 3 — Move tests into dedicated module

- [x] Create `heca/src/sidebar/tests.rs`
- [x] Move inline sidebar tests out of `heca/src/sidebar.rs`
- [x] Add `#[cfg(test)] mod tests;` in `heca/src/sidebar.rs`
- [x] Validate with `cargo test -q --workspace`
- [x] Validate with `cargo clippy --workspace --all-targets --all-features --quiet`

#### Definition of done for the preserved completed slice

- [x] `heca/src/sidebar.rs` is a thin façade
- [x] rendering is fully extracted
- [x] tests are fully extracted
- [x] app behavior is unchanged
- [x] validation passes

### Current active slice — Phase 3.1 Shared pane-ops layer

#### Commit 1 — Extract the shared same-column swap helper

- [x] Add a small shared helper for same-column pane swaps
- [x] Delegate keyboard swap paths to the shared helper
- [x] Keep the existing swap semantics and animation behavior unchanged
- [x] Validate with `cargo check -q`
- [x] Validate with `cargo clippy --workspace --all-targets --all-features --quiet`
- [x] Validate with `cargo test -q --workspace`

#### Commit 2 — Expand shared pane ops beyond same-column swaps

- [x] Extract move/reinsert helpers used by mouse drop and cross-workspace swap logic
- [x] Reduce remaining ad hoc pane scan/reinsert logic in handlers and mouse paths
- [x] Keep behavior unchanged
- [x] Validate with `cargo check -q`
- [x] Validate with `cargo clippy --workspace --all-targets --all-features --quiet`
- [x] Validate with `cargo test -q --workspace`

#### Definition of done for the current slice

- [x] common pane swap mechanics live in a shared helper
- [x] handlers/mouse paths reuse the same pane-op building blocks where applicable
- [x] behavior is unchanged
- [x] validation passes

#### Phase 3.2 follow-up

- [x] same-workspace diff-column swap is now extracted into shared helpers
- [x] cross-workspace swap is now extracted into shared helpers
- [x] `handle_swap_param()` is now a dispatcher instead of a large inline implementation
- [x] validation passes after the refactor

#### Live drag-mode sync follow-up

- [x] Shift press/release now updates the active drag mode live during an in-progress drag
- [x] swap mode falls back to normal move behavior when the target is the sidebar
- [x] validation passes after the drag-mode sync change

#### Progress note

- Added `heca/src/app/pane_ops.rs` with shared same-column swap and pane insert/remove helpers.
- `handle_swap_param()`, `handle_swap_up()`, and `handle_swap_down()` delegate to the shared same-column helper instead of each carrying the same animation/swap code inline.
- `heca/src/mouse/drop.rs` now uses the shared insert/remove helpers for detached-pane reinsertion.
- `heca/src/mouse/sidebar_drop.rs` now uses the same helpers:
  - `remove_pane_by_id()` replaces the manual pane-by-id search+remove scan in `drag_drop()`.
  - `find_pane_location()` replaces the triply-nested manual pane location loop in `handle_drop()`.
  - `insert_pane_at_position()` replaces `add_pane_to_column`/`add_column` fork patterns in both `drag_drop()` and `handle_drop()` shift and non-shift paths.
  - `Column` import removed from `sidebar_drop.rs` since `insert_pane_at_position` handles column creation internally.
  - `PaneInsertTarget` import added.
- Phase 3.2 in progress:
  - `handle_swap_param()` now dispatches into shared helpers for same-column, same-workspace diff-column, and cross-workspace swaps.
  - the new swap helpers live in `heca/src/app/pane_ops.rs` so the handler is much thinner and easier to read.
  - drag modifier changes now sync live move↔swap mode during an active drag instead of only latching at drag start.
  - swap mode falls back to normal move behavior when the drop target is in the sidebar, so sidebar placement still works.
  - validation currently passes after the dispatch refactor and live drag-mode sync.
- Phase 3.2 began: `heca/src/app/pane_ops.rs` now also hosts shared same-workspace diff-column and cross-workspace swap helpers, and `handle_swap_param()` was slimmed to dispatch into them.

---

## 9. Final Note

This roadmap is now explicitly a **current cleanup/stabilization plan**, not the final long-term UI architecture plan. The long-term plan changed on **2026-06-05** after new requirements emerged around pluggable chrome regions, built-in container providers, dynamic action registration, and future WASM plugins. See `pluggable-chrome-plugin-plan.md`.

This roadmap deliberately avoids trying to force all behavior into one place.

Heca is not just a layout library and not just a compositor clone. It is a GPU app with input routing, pane backends, UI chrome, and NIRI-inspired layout semantics.

So the right answer is:

- **stronger layout ownership in `heca-core` where appropriate**
- **clear app/runtime orchestration in `heca`**
- **smaller modules everywhere**

That is the path that will make the code clear.
