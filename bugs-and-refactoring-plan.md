# Heca — Bugs & Refactoring Implementation Plan

**Scope:** `heca`, `heca-core`, `heca-renderer`, `heca-config`  
**Excluded:** `heca-ui`, `heca-grid-ui`  
**Based on:** `bugs-and-refactoring.md`  
**Date:** 2026-06-04

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
| 4 | Sidebar redesign | Sidebar projection and UI state cleaned up |
| 5 | Backend/runtime ownership cleanup | Backend lifecycle less fragile |
| 6 | Dead state and metadata cleanup | Remove stale types, fields, placeholders |
| 7 | Error handling and safety hygiene | Typed errors and `// SAFETY:` comments |
| 8 | Constants, polish, and perf follow-ups | Better ergonomics and targeted perf work |
| 9 | Focus-domain routing correctness | Floating vs tiled action targeting becomes explicit and reliable |

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

**Status update — 2026-06-04**

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
  - `sidebar_drag_drop()`
  - `sidebar_handle_drop()`

Current state:
- `heca/src/mouse.rs` is down to **551 LOC**
- extracted submodules currently measure:
  - `mouse/hit_test.rs` — **102 LOC**
  - `mouse/render.rs` — **177 LOC**
  - `mouse/drag.rs` — **310 LOC**
  - `mouse/drop.rs` — **586 LOC**
- public mouse entrypoints still live in `heca/src/mouse.rs`, and the heavy drag/drop logic now delegates into submodules
- validation passes with:
  - `cargo fmt`
  - `cargo check -q`
  - `cargo clippy --workspace --all-targets --all-features --quiet`

Why this is the current stopping point:
- the biggest readability win has already landed: `mouse.rs` is no longer the sole home for hit testing, drag transitions, drag visuals, and pane drop placement
- however, `mouse/drop.rs` is still too large, which means the complexity has been isolated but not yet fully decomposed
- the remaining complexity is concentrated in:
  - detached-pane reinsertion
  - swap-vs-insert behavior
  - sidebar target routing
  - cross-workspace placement fallbacks

**Remaining tasks in this phase**
- split `mouse/drop.rs` further or factor shared placement helpers so no single mouse submodule stays oversized
- decide whether sidebar-target-specific logic belongs in a `mouse/sidebar.rs` helper or should stay in `drop.rs` with smaller internal helpers
- keep public entrypoints minimal:
  - `on_cursor_moved`
  - `on_mouse_input`
  - `process_edge_scroll`
  - render helpers

**Acceptance criteria**
- no single mouse submodule contains more than ~400 LOC
- drag/drop code becomes locally navigable
- `mouse.rs` mainly reads as top-level event glue instead of a full state-machine implementation

---

### 1.3 Split `heca/src/sidebar.rs`

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
- remove `heca-core/src/types.rs::Rect` if truly unused
- ensure all layout geometry uses `layout/types.rs`

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
**Tasks**
- introduce typed error enums for:
  - config loading
  - PTY/backend startup
  - RPC execution/parsing bridge if needed
- reduce `Result<_, String>` in stable subsystem APIs

### 7.2 Improve action dispatch failure behavior
**Tasks**
- decide what `ActionRegistry::execute()` should do when a handler is missing
- options:
  - return `Result<(), ActionDispatchError>`
  - debug assert in development
  - surface a user-visible status error

### 7.3 Add `// SAFETY:` comments to all unsafe blocks
**Tasks**
- document invariants for all `unsafe` use in `heca-core/src/backend/terminal.rs`
- ensure the comments explain ownership, FD validity, and syscall assumptions

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

**Continue Phase 1.2: finish decomposing mouse drop logic.**

Immediate next slices:
1. reduce `heca/src/mouse/drop.rs` below the target complexity/size threshold
2. extract shared reinsertion / swap fallback helpers or sidebar-target-specific helpers
3. leave behavior unchanged while improving local readability
4. re-run validation, then decide whether a tiny `mouse/sidebar.rs` helper split is still worthwhile

Reason:
- Phase 1.1 is complete enough; `main.rs` is already thin
- Phase 1.2 is **substantially progressed but not complete**
- `mouse.rs` itself is much better, but the hardest logic is now concentrated in `mouse/drop.rs`
- finishing that decomposition should make the later `sidebar.rs` and shared pane-operation cleanup safer

---

## 8. Final Note

This roadmap deliberately avoids trying to force all behavior into one place.

Heca is not just a layout library and not just a compositor clone. It is a GPU app with input routing, pane backends, UI chrome, and NIRI-inspired layout semantics.

So the right answer is:
- **stronger layout ownership in `heca-core` where appropriate**
- **clear app/runtime orchestration in `heca`**
- **smaller modules everywhere**

That is the path that will make the code clear.