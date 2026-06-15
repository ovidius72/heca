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

**Status:** Completed by Agent

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

- Added four new `WmAction` variants in `heca/src/input.rs`:
  - `EnterSelectionMode` (unit) — enters `InputMode::Selection`.
  - `ClearSelection` (unit) — clears the active selection and exits selection mode if active.
  - `CopySelection` (unit placeholder) — Phase 10 will own real clipboard integration.
  - `PasteClipboard` (unit placeholder) — Phase 10 will own real paste integration.
- Added name mappings in `action_from_name`:
  - `enter_selection_mode`, `clear_selection`, `copy_selection`, `paste_clipboard`.
- Updated `action_priority` explicitly to match the new variants (priority 1, same as pane management). Updated the exhaustive test in the same file to keep the priority list complete.
- Added `InputMode::Selection` unit variant in `heca/src/app_state.rs` with doc comment explaining its keyboard contract.
- Added four handlers in `heca/src/handlers.rs`:
  - `handle_enter_selection_mode` — sets `state.input_mode = InputMode::Selection`.
  - `handle_clear_selection` — calls `state.selection.clear()` and exits `InputMode::Selection` if active.
  - `handle_copy_selection` — placeholder that sets `needs_redraw`; documents Phase 10 ownership.
  - `handle_paste_clipboard` — placeholder that sets `needs_redraw`; documents Phase 10 ownership.
- Registered all four handlers in `build_registry()` in `heca/src/app/registry.rs`.
- Added an `InputMode::Selection` arm in `heca/src/app/input.rs` and a `handle_selection_mode` function:
  - `Esc` → clears selection and returns to `Normal`.
  - `Enter` → confirms selection (`selection.end()`) and returns to `Normal`.
  - `prefix` → returns to `Normal` (prefix mode) without mutating the selection.
  - Other keys → ignored; not forwarded to the focused backend.
- Added `InputMode::Selection` arm in `action_policy` in `heca/src/app/interaction.rs` under `ActionPolicy::FocusedPaneLocal` (selection acts on the focused pane in both tiled and floating domains).
- Added `InputMode::Selection` label in `status_mode_parts` in `heca/src/app/render.rs` as `"SELECTION"`.
- Added four `ActionDescriptor` entries in `heca/src/actions.rs` under `ActionCategory::Pane` with the required labels, descriptions, and non-empty `default_binding` fields.
- Added default flat bindings in `heca-config/src/keys.rs`:
  - `enter_selection_mode` → `prefix+s`
  - `clear_selection` → `prefix+Shift+s`
  - `copy_selection` → `prefix+y`
  - `paste_clipboard` → no default flat binding (intentional, to avoid collision; users bind via `config.toml`; descriptor `default_binding = "unbound"`).
- Added focused unit tests:
  - `test_action_from_name_known` in `heca/src/input.rs` now asserts the four new action names parse correctly.
  - `test_selection_action_descriptors_exist` in `heca/src/actions.rs` asserts all four descriptors are present with non-empty label/description/default_binding.
  - `default_selection_bindings_resolve` in `heca/src/app/registry.rs` asserts the default keymap resolves `prefix+s`, `prefix+Shift+s`, `prefix+y` to the correct actions.
- Did not touch renderer selection overlays, terminal host selection geometry, clipboard/platform integration, or sidebar / `heca-grid-ui`.
- Did not introduce any new `#[allow]` / `#[expect]` shortcuts.
- Did not introduce registry bypasses; all selection state mutations from actions go through `registry.execute()`.
- Did not break current bell work (`heca/src/app/lifecycle.rs` unchanged).

**Verification**

```bash
cargo check -p heca -p heca-config                                 # passes
cargo clippy -p heca --all-targets                                # passes
cargo clippy --workspace --all-targets --all-features              # passes
cargo test -p heca                                                 # 151 passed
cargo test -p heca-config --lib                                    # 25 passed
```

**Expected warnings**

The only remaining warnings are the same expected `dead_code` warnings on the selection-model API surface from Task 01 (the model is still not consumed by surface adapters; that is the next task). Task 02 introduced zero new warnings.

**Reviewer Decision**

- Accepted.

**Reviewer Notes**

- **Medium**: `InputMode::Selection -> InputMode::Prefix` does not arm the
  prefix timeout.
  In [heca/src/app/input.rs](/Users/antonio/projects/heca/heca/src/app/input.rs)
  `handle_selection_mode()` sets `state.input_mode = InputMode::Prefix` on the
  prefix key, but it does not set `state.prefix_entered_at`. The existing
  timeout logic in
  [heca/src/app/lifecycle.rs](/Users/antonio/projects/heca/heca/src/app/lifecycle.rs)
  only times out prefix/chord mode when `prefix_entered_at` is set, so this
  path can leave the app stuck in prefix mode indefinitely. The comment saying
  "the prefix-mode arm will set `prefix_entered_at`" is incorrect for this
  event path.
- **Medium**: the new clear-selection action surface is bypassed inside
  selection mode.
  In [heca/src/app/input.rs](/Users/antonio/projects/heca/heca/src/app/input.rs)
  `Esc` calls `state.selection.clear()` directly instead of dispatching the new
  `WmAction::ClearSelection` through the action architecture. Task 02's
  acceptance criteria explicitly require "actions are wired through the normal
  action architecture" and "no registry bypasses are introduced", so this
  direct mutation is not acceptable.
- Verification is otherwise good:
  - `cargo check -p heca`
  - `cargo clippy -p heca --all-targets`
  - `cargo test -p heca`
  - `cargo check -p heca-config`
  - `cargo clippy -p heca-config --all-targets`

**Reviewer fixes applied**

After two parallel subagent reviews (rust-best-practices and rust-skills), the following medium and low findings were applied. The user chose option (a) for the prefix-in-selection behavior question: fix the code to set `InputMode::Prefix` directly.

- **RBP-M1 / RUST-MED-2**: Extended `action_policy_covers_all_variants` in `heca/src/app/interaction.rs` to include the four new unit variants (`EnterSelectionMode`, `ClearSelection`, `CopySelection`, `PasteClipboard`) in the `unit_actions` vec.
- **RBP-M2**: Extended `tiled_domain_allows_focused_pane_local`, `floating_allows_focused_pane_local_via_keyboard`, and `floating_allows_focused_pane_local_from_all_sources` to include the four new selection actions in their action lists.
- **RBP-M3 / user decision (a)**: `handle_selection_mode` in `heca/src/app/input.rs` now sets `state.input_mode = InputMode::Prefix` directly in the `ctx.is_prefix` arm (no longer writes `prefix_entered_at` — the prefix-mode arm will set it on its own event). Updated the function-level comment to describe the new behavior.
- **RUST-MED-1**: Added a `#[cfg(debug_assertions)]` `debug_assert!` in `handle_enter_selection_mode` that documents the precondition (selection should be inactive) and points surface adapters at the right call site. Updated the doc comment to make the contract explicit.
- **RUST-MED-3**: Removed the duplicated `match` body in `test_action_priority_exhaustive`. The test now builds an `each_variant()` `Vec<WmAction>` listing every variant and calls the real `action_priority` (now `pub(crate)` and `#[cfg(test)]`). This removes ~70 lines of duplication and makes the test enforce both exhaustiveness of the call site and that the real function handles every variant. Also added the four new selection variants to the `each_variant()` list.
- **RUST-LOW-1**: Updated the doc comments on `handle_copy_selection` and `handle_paste_clipboard` to explicitly say they "route the action today" rather than implying a future TODO, matching what the functions actually do.
- **RUST-LOW-2**: Added negative-case assertions in `test_action_from_name_known` (`copy_selection_42` and `enter_selection_mode_now` should return `None`).
- **RBP-L5**: Added a "Selection" section to `keybindings.toml` documenting the new actions and their default bindings, and explaining the `paste_clipboard` placeholder.

**Verification after fixes**

```bash
cargo check -p heca -p heca-config                                 # passes
cargo clippy --workspace --all-targets --all-features              # passes
cargo build -p heca                                                 # passes
cargo test -p heca                                                  # 151 passed
cargo test -p heca-config --lib                                     # 25 passed
```

**Out-of-scope observations (filed for future tasks)**

- `action_from_name`'s catch-all pattern (RUST-LOW-4) is undocumented; could be a future doc-only PR.
- `ActionDescriptor::default_binding` as a string sentinel `"unbound"` (LOW-6) is a pre-existing pattern; converting to `Option<&str>` would touch every descriptor and is out of scope.
- The `_action` parameter on `handle_copy_selection` / `handle_paste_clipboard` is intentionally unused (RUST-LOW-1) and must stay that way to keep the `ActionHandler` signature compatible.
- `keybindings.toml` was updated (RBP-L5 fixed).

**Coordinator review fixes applied**

The coordinator flagged two **Medium** issues in `heca/src/app/input.rs::handle_selection_mode`. Both are now fixed.

- **Coordinator-M1 (prefix timeout)**: `handle_selection_mode` previously set `state.input_mode = InputMode::Prefix` on the prefix keypress but did **not** set `state.prefix_entered_at`. The timeout in `lifecycle::handle_about_to_wait` only fires when `prefix_entered_at` is `Some(_)`, and `handle_prefix_mode` only ever clears that field, never sets it. The path would leave the app stuck in Prefix mode until the user typed something that cleared it. **Fix**: the `ctx.is_prefix` arm now sets `state.prefix_entered_at = Some(std::time::Instant::now())` alongside the mode transition, matching the `Normal → Prefix` promotion in `handle_keyboard_input`. The function-level comment was updated to document the timeout arming.
- **Coordinator-M2 (action architecture bypass on Esc)**: `handle_selection_mode` previously called `state.selection.clear()` directly on Esc, bypassing the new `WmAction::ClearSelection` action surface. Task 02's acceptance criteria require actions to be wired through the action architecture with no registry bypasses. **Fix**:
  - `handle_selection_mode` now takes `registry: &ActionRegistry` as a parameter (call site updated in `handle_keyboard_input`).
  - The Esc path sets `state.input_mode = InputMode::Normal` and then dispatches `WmAction::ClearSelection` through `dispatch_action(state, registry, InteractionSource::Keyboard, &WmAction::ClearSelection)` — the only entry point for clearing the selection is now the action architecture.
  - The Enter path keeps `state.selection.end()` as a direct call because there is no `ConfirmSelection` action in the Task 02 surface. The function-level comment explains this is a mode-internal key (consistent with `handle_rename_input` / `handle_confirm_delete_input` precedent).
- Added a note in the `app/input.rs` test module explaining why `handle_selection_mode` is not unit-tested directly (it requires a fully-constructed `AppState` with winit + wgpu) and listing the four coverage mechanisms that verify the contract: `selection_model` unit tests, registry integration, `default_selection_bindings_resolve` keymap test, and compile-time exhaustiveness.

**Verification after coordinator fixes**

```bash
cargo check -p heca                                                  # passes
cargo clippy --workspace --all-targets --all-features              # passes
cargo build -p heca                                                  # passes
cargo test -p heca                                                   # 151 passed
```

**Final review fixes applied**

The final reviewer flagged two **Medium** issues. Both are now fixed.

- **Final-M1 (`EnterSelectionMode` self-contradictory contract)** in `heca/src/handlers.rs::handle_enter_selection_mode`:
  - The doc comment promised a future surface adapter may call `state.selection.begin(...)` before transitioning the mode, but the handler immediately `debug_assert!(!state.selection.is_active())`. That also conflicted with the `Enter`-then-re-enter flow: confirming a selection leaves it in `Selected`, and re-entering selection mode would trip the debug assert even though that flow is architecturally valid.
  - **Fix**: Removed the `debug_assert!` and rewrote the doc comment to:
    - State that the handler only transitions the input mode and does **not** start a selection gesture.
    - Explicitly allow pre-existing selections: a user who confirmed a selection with `Enter` (leaving it in the `Selected` phase) can re-enter selection mode to reposition it, and the surface adapter is responsible for calling `state.selection.begin(...)` to refresh the gesture.
    - Document that mode-internal keyboard behavior (`Esc` clears, `Enter` confirms, `prefix` returns to Prefix) is owned by `app::input::handle_selection_mode`.
- **Final-M2 (selection actions not reachable from RPC)** in `heca/src/rpc.rs`:
  - The final reviewer noted that `parse_rpc_command` had no branches for `enter-selection-mode`, `clear-selection`, `copy-selection`, or `paste-clipboard`, even though the task claims the surface is "prepared for RPC" and the project rule says meaningful actions should be reachable from mouse/UI, keyboard/actions, and RPC.
  - **Fix**: Added four new command branches in `parse_rpc_command`:
    - `enter-selection-mode` → `WmAction::EnterSelectionMode`
    - `clear-selection` → `WmAction::ClearSelection`
    - `copy-selection` → `WmAction::CopySelection`
    - `paste-clipboard` → `WmAction::PasteClipboard`
  - Updated the module-level doc comment header to list the new commands and explain that selection is a host capability reachable from RPC, keyboard bindings, and future mouse/UI dispatch.
  - Added `test_selection_commands` in `heca/src/rpc.rs` that asserts all four commands parse correctly.

**Verification after final fixes**

```bash
cargo check -p heca                                                  # passes
cargo clippy --workspace --all-targets --all-features              # passes
cargo build -p heca                                                  # passes
cargo test -p heca                                                   # 152 passed
```

**Final reviewer notes**

- Accepted after the final two fixes:
  - `handle_enter_selection_mode()` now has a coherent contract: it allows
    pre-existing selections and no longer contradicts its own documentation
    with a debug assertion.
  - the selection action surface is now actually reachable from RPC through
    `enter-selection-mode`, `clear-selection`, `copy-selection`, and
    `paste-clipboard`.
- Verification rerun by reviewer:
  - `cargo check -p heca`
  - `cargo clippy -p heca --all-targets`
  - `cargo test -p heca`
