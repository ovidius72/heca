# Shared Tasks

## Purpose

This file is the coordination board for work delegated to other agents on the
post-merge terminal backlog.

Rules:

1. Work one task at a time unless a task explicitly says it can be parallelized.
2. The assigned agent must update only the task they are working on.
3. When the assigned agent finishes, they must write their completion notes
   under that task in the `Agent Completion` section.
4. After completion notes are added, the reviewer agent will inspect the code
   and write one of:
   - `Accepted`
   - `Needs Edit`
   - `Rejected`
5. If review requests changes, the assigned agent should keep appending under
   the same task instead of creating a new one.

Review policy:

- acceptance is based on code, verification, and architectural alignment
- completion notes alone are not sufficient
- terminal-only shortcuts are not acceptable when the task explicitly requires
  shared host capability behavior

Status values:

- `Open`
- `In Progress`
- `Completed by Agent`
- `Accepted`
- `Needs Edit`
- `Rejected`

---

## Task 01 — Shared Selection State Model

**Status:** Completed by Agent

**Goal****

Define and implement the first reusable selection state model for heca so
selection is not terminal-only and can be reused later by:

- terminal panes
- future custom Neovim GUI panes
- future embedded browser panes
- future heca-native content surfaces

**Scope**

This task is intentionally limited to the shared state/model layer.

It should define:

- a host-owned selection owner concept
- shared selection lifecycle state
- selection source classification
- selection rendering mode classification

It should not yet implement:

- clipboard copy
- paste
- full keyboard selection mode
- browser-native selection integration
- final mouse policy

Those belong to later tasks/phases.

**Required Architecture**

The implementation must follow the contract in
[`terminal-implementation.md`](/Users/antonio/projects/heca/terminal-implementation.md):

- selection is a host capability
- selection is not stored as terminal-only ad hoc state
- the model must be usable by non-terminal pane types later

The implementation should introduce shared terminology and types for:

- `selection owner`
- `selection source`
- `selection phase/state`
- `selection rendering mode`

The exact Rust names can differ if they are clearly better, but the model must
cover those concepts directly.

**Suggested Files**

- [app_state.rs](/Users/antonio/projects/heca/heca/src/app_state.rs)
- new app module if needed under [app](/Users/antonio/projects/heca/heca/src/app)
- possibly [mod.rs](/Users/antonio/projects/heca/heca/src/app/mod.rs)

Do not touch:

- sidebar / `heca-grid-ui` migration files
- browser implementation files
- clipboard/system integration

**Acceptance Criteria**

- there is one shared selection model owned by the host/app layer
- the model is not terminal-specific in naming or ownership
- the code compiles
- no `#[allow]` / `#[expect]` shortcuts are introduced to force it through
- the implementation does not break current bell work

**Verification**

At minimum:

```bash
cargo check -p heca
cargo clippy -p heca --all-targets
```

**Agent Completion**

- Created `heca/src/app/selection_model.rs` defining the host-owned shared selection model:
  - `SelectionOwner` — pane/surface that owns the active selection (`Pane(PaneId)` now, open to future variants).
  - `SelectionSource` — `MouseDrag`, `KeyboardMode`, `Rpc`.
  - `SelectionPhase` — `Inactive`, `Selecting`, `Selected`.
  - `SelectionRenderMode` — `HostGrid`, `BackendNative`.
  - `SelectionRegion` — mode-specific region that locks anchor semantics to the render mode by construction. `HostGrid { anchor_row, anchor_col, focus_row, focus_col }` for host-rendered selection; `BackendNative` for opaque backend selection.
  - `ActiveSelection` — active payload bundling owner, source, and region; render mode is derived from the region.
  - `SelectionState` — host-owned state struct with lifecycle methods: `begin`, `update_focus`, `end`, `clear`, plus getters and `is_active`/`is_selecting`/`has_selection`.
- Re-exported `SelectionState` from `heca/src/app_state.rs` and added `pub selection: SelectionState` to `AppState`.
- Initialized the field in `heca/src/app/startup.rs`.
- Registered the new module in `heca/src/app/mod.rs`.
- Added focused unit tests for the selection state machine in `selection_model.rs`.
- Did not touch sidebar / `heca-grid-ui`, browser implementation, or clipboard/system integration.
- Did not add `#[allow]` / `#[expect]` shortcuts.
- Did not break current bell work (`heca/src/app/lifecycle.rs` unchanged; bell polling still works).

**Fixes after reviewer feedback**

- Render mode and anchor semantics are now coupled: `SelectionRegion` variants directly imply their render mode, and `ActiveSelection` bundles the region. The public `begin()` API takes a `SelectionRegion`, so callers cannot supply a mismatched anchor/render-mode pair.
- `render_mode()` now returns `Option<SelectionRenderMode>` and is `None` when inactive. The cleared/default state no longer leaks a fake `HostGrid` render mode.
- Refactored `SelectionState` from a struct with parallel `active`/`phase` fields into an enum: `Inactive | Selecting(ActiveSelection) | Selected(ActiveSelection)`. This makes the "selection payload exists exactly when phase is not inactive" invariant impossible to violate.
- Added tests requested by the reviewer:
  - `update_focus_no_op_when_selected`
  - `end_is_idempotent_when_selected`
  - `selection_region_render_mode_matches_variant`
  - `active_selection_render_mode_derives_from_region`
- Updated this completion note to accurately state that `cargo clippy` is green but emits expected `dead_code` warnings for the new, not-yet-consumed API.

**Verification**

```bash
cargo check -p heca                    # passes
cargo clippy -p heca --all-targets     # passes (with expected dead_code warnings)
cargo clippy --workspace --all-targets --all-features  # passes (with expected dead_code warnings)
cargo test -p heca selection_model     # 15 passed
```

**Expected warnings**

Because the model is intentionally not consumed by handlers/renderers in this task, the following warnings remain:
- `SelectionOwner::Pane` / `SelectionSource` variants / `SelectionPhase` / `SelectionRenderMode` / `SelectionRegion` variants / `SelectionState::Selecting|Selected` are unused outside tests.
- `SelectionState` API methods and `AppState::selection` field are never read.

These warnings will resolve once Task 02+ wires `WmAction` selection variants, terminal selection adapter, and rendering overlay. They are not suppressed.

**Reviewer Decision**

- Accepted.

**Reviewer Notes**

- Accepted after the invariant fixes:
  - render mode is now derived from `SelectionRegion`, so callers cannot create
    mismatched render-mode/anchor combinations
  - inactive selection no longer reports a fake render mode; the API now uses
    `Option<SelectionRenderMode>` for inactive state
- Verification claims are now accurate: the task documents that the current
  `cargo check`/`cargo clippy` pass is green with expected dead-code warnings
  because the model is not wired into handlers/renderers yet.

---

## Task 02 — Selection Actions and Input Mode

**Status:** Open

**Goal**

Wire the accepted shared selection model into the action/input architecture so
selection is no longer only a data model. This task should establish the action
surface and mode plumbing needed for later mouse integration, terminal
rendering, and clipboard work.

**Scope**

This task is limited to:

- `WmAction` additions
- action parsing / registration
- input-mode additions
- minimal app-state/mode transitions

This task should not yet implement:

- terminal selection rendering
- text extraction / clipboard copy
- paste
- browser-native selection
- final mouse gesture policy

**Required Architecture**

The implementation must build on the accepted Task 01 model and preserve the
shared host capability design:

- actions must be generic selection actions, not terminal-only actions
- mode transitions must go through the app action/input architecture
- selection must remain reusable across future pane types

Selection actions should be designed so later tasks can reuse them from:

- keyboard bindings
- mouse/UI dispatch
- RPC

**Required Action Surface**

Add or prepare the following action concepts:

- `EnterSelectionMode`
- `ClearSelection`
- `CopySelection` placeholder action surface only
- `PasteClipboard` placeholder action surface only

Only the first two need functional behavior in this task unless there is a
clean way to add the others as parsed/registered placeholders without forcing
fake implementation.

**Suggested Files**

- [input.rs](/Users/antonio/projects/heca/heca/src/input.rs)
- [actions.rs](/Users/antonio/projects/heca/heca/src/actions.rs)
- [handlers.rs](/Users/antonio/projects/heca/heca/src/handlers.rs)
- [app_state.rs](/Users/antonio/projects/heca/heca/src/app_state.rs)
- input-routing modules under [app](/Users/antonio/projects/heca/heca/src/app)

Do not touch:

- renderer selection overlays
- terminal host selection geometry
- clipboard/platform integration
- sidebar / `heca-grid-ui` migration files

**Acceptance Criteria**

- selection has generic action names, not terminal-only ones
- selection mode is represented in `InputMode`
- actions are wired through the normal action architecture
- no registry bypasses are introduced
- code compiles
- no `#[allow]` / `#[expect]` shortcuts are introduced to force it through

**Verification**

At minimum:

```bash
cargo check -p heca
cargo clippy -p heca --all-targets
```

**Agent Completion**

- Pending.

**Reviewer Decision**

- Pending.

**Reviewer Notes**

- Pending.
