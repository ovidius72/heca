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
- [ ] Add central focused-pane/domain helper(s)
- [ ] Update pane-local handlers to use shared helpers where appropriate
- [ ] Reduce direct tiled/floating branching in handlers

### 5. Tests
- [x] Add router unit tests (10 tests: tiled allows, floating blocks, focused-pane-local allows, intent blocks, policy exhaustive, domain helpers, mouse content focus block, mouse left sidebar block)
- [ ] Add sidebar/content floating-focus regression tests
- [ ] Add keyboard tiled-only action blocking tests in floating domain
- [ ] Add allowed floating-local action tests
- [ ] Re-run full validation

### 6. Validation
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace --all-targets --all-features` — 0 heca warnings
- [x] `cargo test --workspace` — 234 tests pass
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
- Active branch: feature/interaction-sidebar-wiring
- Current phase/slice: Phase 1+2+3 complete (core types, real policy, all three sources wired, drift removed)
- Overall status: All three interaction sources (Keyboard, MouseContent, MouseLeftSidebar) wired through central router. Sidebar floating guard in place. No ad hoc guards remain.
- Last validated at: 2026-06-09

### Completed work
- Core types in heca/src/app/interaction.rs:
  - InteractionSource enum (Keyboard, MouseContent, MouseLeftSidebar)
  - InteractionIntent enum (ActivateAction, FocusPane, FocusWorkspace, EnterSidebarNav, StartSidebarDrag)
  - RouteDecision enum (Allow(intent), Block)
  - ActionPolicy enum (private: AlwaysAllowed, TiledOnly, FocusedPaneLocal, WorkspaceLevel, SourceDependent)
  - action_policy() function with exhaustive WmAction matching (no catch-all)
  - route_interaction() / route_interaction_for_session() — real floating-domain policy
  - dispatch_action() — single public entry point, routes through router then registry
  - is_floating_domain() helper
- Floating-domain blocking policy implemented:
  - When FocusDomain::Floating, only Float/ClosePane/RenamePane allowed from keyboard
  - All TiledOnly, WorkspaceLevel, and AlwaysAllowed actions blocked when floating
  - FocusPane from mouse/sidebar blocked when floating
  - EnterSidebarNav and StartSidebarDrag blocked when floating
- Keyboard dispatch wired through dispatch_action() with InteractionSource::Keyboard in heca/src/app/input.rs
- Mouse content dispatch wired through dispatch_action() in heca/src/app/events.rs
- Mouse sidebar dispatch wired through dispatch_action() with InteractionSource::MouseLeftSidebar in heca/src/app/events.rs
- Sidebar floating-domain guard in heca/src/mouse.rs blocks all sidebar interaction when floating (before click_action and drag detection)
- on_mouse_input() returns (WmAction, InteractionSource) tuple — sidebar actions tagged MouseLeftSidebar, content actions tagged MouseContent
- handle_sidebar_drag_starting_release() returns (WmAction, InteractionSource) with MouseLeftSidebar
- 10 unit tests covering: tiled allows, floating blocks, focused-pane-local allows, intent blocks, policy exhaustive, domain helpers, mouse content focus block, mouse left sidebar block
- is_floating_domain() uses expect() instead of unwrap_or(false) per AGENTS.md
- MouseLeftSidebar no longer dead code — #[allow(dead_code)] removed
- Phase C (remove drift): SKIPPED — no ad hoc guards remained to remove
- Stashed ad hoc guards from old branch (stash@{0})
- FocusDomain enum preserved in heca-core

### In-progress work
- None — all wired sources complete

### Pending work
- Phase D: Add focus-target helpers (focused_pane_id, can_focus_pane, close_focused_pane, rename_focused_pane)
- Phase E: Regression tests for floating-domain blocking behavior
- Sidebar intent routing: use InteractionIntent::FocusPane/FocusWorkspace/EnterSidebarNav instead of ActivateAction(WmAction) for sidebar-originated interactions (future polish)

### Open questions / decisions needed
- AlwaysAllowed actions: blocked when floating from keyboard/sidebar, but may be allowed from future chrome sources
- Scratchpad feature (future): may allow multiple floating panes
- RPC source policy: deferred

### Files changed in current slice
- heca/src/app/interaction.rs (core types, policy, routing, 10 tests, expect fix, MouseLeftSidebar no longer dead)
- heca/src/app/mod.rs (added interaction module)
- heca/src/app/input.rs (dispatch_action with InteractionSource::Keyboard)
- heca/src/app/events.rs (dispatch_action with source from on_mouse_input() tuple)
- heca/src/mouse.rs (on_mouse_input returns (WmAction, InteractionSource), sidebar floating guard, InteractionSource import)
- heca/src/mouse/release.rs (handle_sidebar_drag_starting_release returns tuple, InteractionSource import)
- heca-core/src/layout/workspace.rs (FocusDomain enum — from prior commit)
- heca/src/app/focus.rs (FocusDomain import — from prior commit)

### Validation run
- `cargo check --workspace`: ✅ clean
- `cargo clippy --workspace --all-targets --all-features`: ✅ 0 heca warnings
- `cargo test --workspace`: ✅ 234 tests pass (10 interaction tests)

### Rust skill review status
- Review completed: yes (both rust-skills and rust-best-practices, full skill invocation)
- Findings addressed:
  - R2: release.rs used full path instead of import → fixed with `use crate::app::interaction::InteractionSource`
  - R4: is_floating_domain unwrap_or(false) → expect() per AGENTS.md
  - R5: MouseLeftSidebar no longer dead, #[allow(dead_code)] removed
  - R1/R3/R6: No issues — tuple return idiomatic, map() idiomatic, tests exist
- User acknowledged review: yes

### PRs
- PR #52: feature/interaction-policy (base commit with core types + keyboard/mouse wiring)
- PR #54: feature/interaction-sidebar-wiring (sidebar floating guard + source tagging + expect fix)

### Next recommended step
- Phase D: Add focus-target helpers (focused_pane_id, can_focus_pane, close_focused_pane, rename_focused_pane)
- Phase E: Regression tests for floating-domain blocking behavior
- Sidebar intent routing: upgrade sidebar clicks to produce InteractionIntent variants instead of ActivateAction(WmAction) for richer semantic routing
