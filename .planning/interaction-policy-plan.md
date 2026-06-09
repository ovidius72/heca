# Interaction Policy / Focus Domain Plan

_Last updated: 2026-06-09_

## Workflow rules

### Before starting always remember the rules

1. before starting always remember the rules
2. pull `main` -> read the task
3. use the `/grill-me` skill to ask everything that is in doubt
4. write the code and relative tests
5. after the task call the rust skill to review the written code. Do not commit yet
6. ask the user to acknowledge the rust skill review
7. wait for user input
8. after the user approves, fix issues, run tests and clippy
9. make PR -> `main`
10. update checklist and handoff

---

## Why this plan exists

The floating-pane bug exposed a broader architectural problem:

> the app has no single authoritative layer that decides whether an interaction is allowed.

Today, interaction policy is scattered across:

- keyboard handlers
- mouse/content click handlers
- sidebar click routing
- drag/drop paths
- focus helpers
- future RPC/menu/chrome paths

This causes policy drift.

The floating pane is only the first place where the problem became obvious.

---

## Current residual work (before this plan)

The current Phase 9 WIP only partially solves the issue:

### Already started

- `FocusDomain` added in `heca-core/src/layout/workspace.rs`
- `floating_is_active: bool` replaced by `focus_domain: FocusDomain`
- initial policy notes added in `heca/src/app/focus_domain.rs`
- local modal guards added in:
  - `heca/src/app/focus.rs`
  - `heca/src/mouse.rs`
  - `heca/src/mouse/surface_left.rs`

### Still missing

- no central interaction router
- no source-aware policy
- no action classification table
- no unified focus-target permission helper
- no regression tests proving consistent behavior across all paths
- no scalable strategy for future surfaces:
  - right sidebar
  - top menu bar
  - status bar
  - command palette
  - RPC

### Conclusion

The current local-guard approach should **not** be expanded further.
It should be replaced by a central interaction policy layer.

---

## Core architectural decision

Interaction behavior should be modeled on **orthogonal axes**:

### 1. `FocusDomain`
What semantic target currently owns focus?

```rust
enum FocusDomain {
    Tiled,
    Floating,
}
```

This belongs near `Workspace` in `heca-core`.

### 2. `InputMode`
How are keys interpreted right now?

This already exists in `heca/src/app_state.rs` and should remain separate.
Examples:

- `Normal`
- `Prefix`
- `SidebarNav`
- `Rename`
- `ConfirmDelete`
- `PaneSelect`
- `PaneSwap`
- custom `Mode { name }`

### 3. `InteractionSource`
Where did the interaction come from?

```rust
enum InteractionSource {
    Keyboard,
    MouseContent,
    MouseLeftSidebar,
    MouseRightSidebar,
    TopMenu,
    StatusBar,
    CommandPalette,
    Rpc,
}
```

Not every source must be implemented immediately.
Start with the sources that already exist.

### 4. `InteractionIntent`
What is the interaction trying to do?

```rust
enum InteractionIntent {
    FocusPane(u64),
    FocusWorkspace(usize),
    EnterSidebarNav,
    StartSidebarDrag { pane_id: u64 },
    ActivateAction(WmAction),
}
```

This is the key abstraction.
The policy layer should operate on **intents**, not raw key presses or hit-test positions.

---

## Central router

Add one central policy function in the app layer:

```rust
fn route_interaction(
    state: &AppState,
    source: InteractionSource,
    intent: InteractionIntent,
) -> RouteDecision
```

With:

```rust
enum RouteDecision {
    Allow(InteractionIntent),
    Block,
}
```

`Retarget(...)` can be added later only if a real use case appears.
Do not overbuild it initially.

---

## Action policy classification

Add one authoritative action-policy classifier:

```rust
enum ActionPolicy {
    AlwaysAllowed,
    TiledOnly,
    FocusedPaneLocal,
    WorkspaceLevel,
    SourceDependent,
}
```

And:

```rust
fn action_policy(action: &WmAction) -> ActionPolicy
```

### Expected first classifications

#### `TiledOnly`
- `ResizeIncrease`
- `ResizeDecrease`
- `ZoomColumn`
- `PaneHeightIncrease`
- `PaneHeightDecrease`
- `SwapLeft`
- `SwapRight`
- `SwapUp`
- `SwapDown`
- `MovePaneLeft`
- `MovePaneRight`
- `MoveColumnUp`
- `MoveColumnDown`
- `MoveColumnToWorkspace`
- `MoveParam`
- `MovePaneToWorkspace`
- `MovePaneToColumn`
- `Resize`
- `ResizeTo`
- `RenameColumn`
- `DeleteColumn`
- `AddPaneToColumn`
- pane selection/swap/take overlays if they target tiled layout

#### `FocusedPaneLocal`
- `ClosePane`
- `RenamePane`
- `Float`
- `FloatAt`
- `ClosePaneById` (may stay partly special-cased due to explicit ID)
- `RenameTarget` (same note)

#### `WorkspaceLevel`
- `WorkspaceNext`
- `WorkspacePrev`
- `FocusWorkspace`
- `CreateWorkspace`
- `RenameWorkspace`
- `DeleteWorkspace`

#### `AlwaysAllowed`
- `ReloadConfig`
- `CommandPalette`
- `SpawnCommand`
- `EnterMode`

#### `SourceDependent`
- `FocusPane { .. }`
- future menu/chrome actions
- RPC-facing actions where explicit targets may bypass UI modal policy

---

## Policy goals for floating focus

When `FocusDomain::Floating` is active:

### Must be blocked
- focusing tiled panes from sidebar click
- entering sidebar navigation from tiled rows
- starting sidebar drag from tiled rows
- content click focusing tiled panes
- tiled-layout-only keyboard actions

### Must still be allowed
- interacting with the active floating pane itself
- pane-local actions on the floating pane (`close`, `rename`, `float/unfloat`)
- global/system actions (`reload_config`, palette, spawn command)

### Intentionally deferred
- whether top menu/status bar should remain interactive
- right sidebar policy
- RPC bypass rules for explicit non-UI targeting

These should be handled by the router design, but do not need to be fully implemented in the first slice.

---

## File-by-file implementation plan

### Phase A — Introduce the policy layer (no broad behavior changes yet)

#### A.1 Create `heca/src/app/interaction.rs`
Add:

- `InteractionSource`
- `InteractionIntent`
- `RouteDecision`
- `ActionPolicy`
- `action_policy(&WmAction) -> ActionPolicy`
- `route_interaction(...) -> RouteDecision`
- `can_focus_pane(...)` helper (or equivalent internal helper)

#### A.2 Re-export / module wire-up
Update `heca/src/app/mod.rs`.

#### A.3 Keep `FocusDomain` in `heca-core`
Do **not** move it into app code.

---

### Phase B — Route the existing high-risk paths

#### B.1 Keyboard action dispatch
Before `registry.execute()`, route:

```rust
InteractionIntent::ActivateAction(action.clone())
```
with `InteractionSource::Keyboard`.

#### B.2 Sidebar click routing
Convert sidebar pane/workspace hits into:

- `FocusPane(id)`
- `FocusWorkspace(idx)`
- `EnterSidebarNav`

and route them centrally.

#### B.3 Sidebar drag start
Before starting sidebar drag detection, route:

```rust
StartSidebarDrag { pane_id }
```
with `InteractionSource::MouseLeftSidebar`.

#### B.4 Content click focus
Before focusing a content pane from mouse hit-test, route:

```rust
FocusPane(id)
```
with `InteractionSource::MouseContent`.

---

### Phase C — Collapse ad hoc local guards

After the router is active for the key paths above:

- remove policy logic from `focus.rs` that duplicates routing decisions
- remove sidebar-specific modal checks from `mouse.rs`
- remove sidebar-specific modal checks from `surface_left.rs`
- keep only the minimal execution logic in those modules

Goal:

- input producers create intents
- router decides allow/block
- handlers execute allowed interactions

---

### Phase D — Focus-target helpers

Add central helpers for pane targeting semantics:

Candidates:

- `focused_pane_id(state)`
- `active_focus_domain(state)`
- `can_focus_pane(state, source, pane_id)`
- `close_focused_pane(state)`
- `rename_focused_pane(state)`

This reduces duplicated tiled-vs-floating resolution.

---

### Phase E — Regression tests

Add focused tests proving policy behavior.

#### E.1 Router unit tests
For `route_interaction(...)`:

- floating + sidebar click on tiled pane → `Block`
- floating + sidebar click on floating row → `Allow`
- floating + keyboard `ZoomColumn` → `Block`
- floating + keyboard `ClosePane` → `Allow`
- tiled + sidebar click on tiled pane → `Allow`

#### E.2 Integration-ish app tests
- floating focus does not allow sidebar pane focus changes
- floating focus does not allow content click focus changes
- tiled focus still allows normal sidebar/content focus
- allowed floating-local actions still work

#### E.3 Existing behavior preservation tests
- prefix mode still works
- sidebar normal behavior still works in tiled mode
- workspace focus/switch flows remain unchanged when not floating

---

## Minimal first slice recommendation

Do **not** implement all sources at once.
Start with the sources currently causing drift:

1. `Keyboard`
2. `MouseContent`
3. `MouseLeftSidebar`

This is enough to solve the floating bug correctly and prove the architecture.

Then add future sources later without redesign.

---

## RPC policy note

RPC should be treated separately from UI modal behavior.

Suggested rule for the first pass:

- UI-like RPC can later be routed through the same interaction layer
- explicit target RPC (`ClosePaneById`, `FocusWorkspace`, etc.) may remain allowed
- do **not** let RPC concerns block the first implementation slice

In other words:

- design `InteractionSource::Rpc` now if useful
- fully enforce RPC policy later if not immediately needed

---

## Relationship to existing Phase 9 plan

This plan **does not replace the intent** of Phase 9.
It provides a better implementation strategy for it.

Mapping:

- **9.1 Define the focus domain explicitly**
  - already started via `FocusDomain`
- **9.2 Define action policy by domain**
  - formalized here via `ActionPolicy`
- **9.3 Route handlers through the domain guard**
  - implemented here as centralized `route_interaction(...)`
- **9.4 Centralize focused-pane targeting helpers**
  - covered in Phase D above
- **9.5 Add regression tests**
  - covered in Phase E above

So this is effectively a deeper and more maintainable continuation of Phase 9.

---

## Checklist

### 0. Clean up current WIP
- [x] Identify and remove ad hoc modal checks that should be superseded by router logic
- [x] Preserve only `FocusDomain` if it remains the right core primitive
- [x] Keep behavior stable while transitioning

### 1. Core types
- [x] Add `heca/src/app/interaction.rs`
- [x] Add `InteractionSource`
- [x] Add `InteractionIntent`
- [x] Add `RouteDecision`
- [x] Add `ActionPolicy`
- [x] Add `action_policy(&WmAction)`
- [x] Add `route_interaction(...)`
- [x] Add `route_interaction_for_session(...)` (testable variant)
- [x] Add `dispatch_action(state, registry, source, action)`
- [x] Add `is_floating_domain(...)` and `current_focus_domain(...)` helpers
- [x] Module wire-up in `heca/src/app/mod.rs`

### 2. Wire first sources
- [x] Route keyboard action execution through interaction router
- [x] Route content click focus through interaction router
- [x] Route left sidebar click through interaction router
- [x] Route left sidebar drag-start through interaction router

### 3. Remove drift
- [x] Remove duplicated modal policy checks from `focus.rs` — no ad hoc guards found
- [x] Remove duplicated modal policy checks from `mouse.rs` — replaced by central router guard
- [x] Remove duplicated modal policy checks from `mouse/surface_left.rs` — no ad hoc guards found
- [x] Confirm policy now lives centrally

### 4. Focus helpers
- [x] Add central focused-pane/domain helper(s)
- [x] Update pane-local handlers to use shared helpers where appropriate
- [x] Reduce direct tiled/floating branching in handlers

### 5. Tests
- [x] Add router unit tests (10 tests: tiled allows, floating blocks, focused-pane-local allows, intent blocks, policy exhaustive, domain helpers, mouse content focus block, mouse left sidebar block)
- [x] Add sidebar/content floating-focus regression tests
- [x] Add keyboard tiled-only action blocking tests in floating domain
- [x] Add allowed floating-local action tests
- [x] Re-run full validation

### 6. Validation
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace --all-targets --all-features` — 0 heca warnings
- [x] `cargo test --workspace` — 250 tests pass (29 interaction tests)
- [ ] Manually smoke test floating focus vs sidebar/content click behavior

---

## Definition of done

This effort is done when:

- focus-domain policy is decided in **one central place**
- mouse/sidebar/keyboard no longer each invent their own modal rules
- floating focus cannot accidentally leak interaction into tiled surfaces
- tiled mode still behaves normally
- future surfaces can participate by emitting `InteractionIntent`s and calling the same router

---

## Important constraint

Do not overbuild this into a giant framework.

The first useful version only needs:

- `Keyboard`
- `MouseContent`
- `MouseLeftSidebar`
- enough intents to solve the real bug cleanly

Everything else can be layered in later.

---

## Handoff

_Update this section after each phase or on demand._

### Current status
- Active branch: feature/interaction-regression-tests
- Current phase/slice: Phase E (regression tests) complete
- Overall status: All interaction sources wired, focus-target helpers added, 29 regression tests passing
- Last validated at: 2026-06-09
- Tests: 29 interaction tests (16 router + helpers + 13 regression), 250 total workspace pass

### Completed work (Phase E — Regression tests)

**13 new regression tests in `heca/src/app/interaction.rs`:**

Floating-domain focus blocking:
- `floating_focus_allows_active_floating_pane_via_keyboard` — FocusPane on active floating pane → Allow
- `floating_focus_blocks_tiled_pane_via_keyboard` — FocusPane on tiled pane → Block
- `floating_focus_pane_intent_blocked_via_mouse_content` — FocusPane intent from MouseContent → Block
- `floating_blocks_sidebar_nav_intent` — EnterSidebarNav when floating → Block
- `floating_blocks_always_allowed_from_keyboard` — CommandPalette/ReloadConfig/SpawnCommand → Block

Floating-domain FocusedPaneLocal allowance:
- `floating_allows_focused_pane_local_via_keyboard` — Float/ClosePane/RenamePane → Allow
- `floating_allows_focused_pane_local_from_all_sources` — Float/ClosePane from Keyboard/MouseContent/MouseLeftSidebar → Allow

Tiled-domain behavior preservation:
- `tiled_sidebar_action_allowed_via_mouse_sidebar` — SidebarFocus/Left/Right → Allow
- `tiled_content_focus_pane_allowed_via_mouse_content` — FocusPane from MouseContent → Allow
- `tiled_keyboard_focus_navigation_allowed` — FocusLeft/Right/Up/Down → Allow
- `tiled_workspace_actions_allowed` — WorkspaceNext/Prev/CreateWorkspace → Allow
- `tiled_allows_sidebar_nav_intent` — EnterSidebarNav from MouseLeftSidebar → Allow

Focus-target helper regression:
- `can_focus_pane_allows_active_floating_pane` — active floating pane from all sources → true

**Refactoring:**
- Consolidated test imports at module level (PaneId, Size, Pane, FloatingPane, Point, Rectangle)
- Removed unused `session_with_floating_pane` helper

**Rust skill review findings:**
- R5: 3 tests partially overlap existing coverage — acceptable as regression tests that name behavioral contracts
- All other findings: clean

### Prior completed work

Core types and policy (Phases 1-3):
- InteractionSource, InteractionIntent, RouteDecision, ActionPolicy enums
- action_policy() with exhaustive WmAction matching
- route_interaction() / route_interaction_for_session()
- dispatch_action() entry point
- is_floating_domain() helper
- 10 router unit tests

Wiring (Phase 2):
- Keyboard dispatch through dispatch_action(Keyboard)
- MouseContent dispatch through dispatch_action(MouseContent)
- MouseLeftSidebar dispatch through dispatch_action(MouseLeftSidebar)
- Sidebar floating-domain guard in mouse.rs

### In-progress work
- None

### Pending work
- Sidebar intent routing: upgrade sidebar clicks to produce InteractionIntent variants
- Chrome sources (MouseTopMenu, MouseStatusBar)
- RPC source
- Future polish: convert `focused_pane_id` return type from `Option<u64>` to `Option<PaneId>` for type safety (R6 from Phase D review)

### Open decisions
- AlwaysAllowed + floating: blocked from Keyboard/MouseContent/MouseLeftSidebar when floating; may allow from chrome sources
- Scratchpad feature: may allow multiple floating panes
- RPC source policy: deferred

### Files changed in Phase D
- heca/src/app/interaction.rs (4 focus-target helpers + 6 tests)
- heca/src/handlers.rs (handle_float, handle_rename_pane, current_tiled_column_target use helpers)

### Validation run (Phase E)
- `cargo check --workspace`: ✅ clean
- `cargo clippy --workspace --all-targets --all-features`: ✅ 0 heca warnings
- `cargo test --workspace`: ✅ 250 tests pass (29 interaction tests)

### Rust skill review (Phase E)
- 14 findings, all pass:
  - R5: 3 tests partially overlap existing coverage — acceptable as regression tests
  - All others: clean (naming, isolation, ownership, doc comments, no unwrap issues)
- User review: approved

### Validation run (Phase D)
- `cargo check --workspace`: ✅ clean
- `cargo clippy --workspace --all-targets --all-features`: ✅ 0 heca warnings
- `cargo test --workspace`: ✅ 237 tests pass (16 interaction tests)

### Rust skill review (Phase D)
- Inline review against rust-skills SKILL.md
- Findings: all clear — pub(crate) visibility, doc comments, no unwrap, #[allow(dead_code)] with TODO+Phase E, minimal borrowing
- User review: approved

### PRs
- PR #52: feature/interaction-policy (merged)
- PR #54: feature/interaction-sidebar-wiring (merged, superseded by #56)
- PR #56: feature/interaction-sidebar-wiring (merged)
- PR #58: feature/interaction-focus-helpers (merged)
- PR #59: feature/interaction-regression-tests (open)

### Next recommended step
- Sidebar intent routing: upgrade sidebar clicks to produce InteractionIntent variants
- Chrome sources (MouseTopMenu, MouseStatusBar)
- RPC source policy
- Future polish: convert focused_pane_id return type to Option<PaneId>
