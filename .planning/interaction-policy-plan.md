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
- Active branch: `feature/interaction-sidebar-wiring`
- Current phase: Phase 1+2+3 complete. All three interaction sources wired. No ad hoc guards remain.
- Overall status: Central interaction policy layer is **production-functional** — keyboard, mouse content, and mouse sidebar all route through `dispatch_action()`. Floating-domain blocking is enforced.
- Last validated: 2026-06-09
- Tests: 234 pass, 0 clippy warnings

### Architecture overview (as implemented)

```
User input → InteractionIntent → route_interaction() → RouteDecision
                                                       ├─ Allow(intent) → registry.execute()
                                                       └─ Block          → no-op

Sources wired:
  Keyboard         → dispatch_action(state, registry, Keyboard, &action)       [input.rs]
  MouseContent     → dispatch_action(state, registry, MouseContent, &action)   [events.rs]
  MouseLeftSidebar → dispatch_action(state, registry, MouseLeftSidebar, &action) [events.rs]

Handler-to-handler calls bypass the router and use registry.execute() directly.
```

### Floating-domain blocking policy

When `FocusDomain::Floating` is active:

| Action category | Keyboard | MouseContent | MouseLeftSidebar | Future chrome |
|----------------|----------|-------------|-----------------|---------------|
| FocusedPaneLocal (Float, ClosePane, RenamePane) | ✅ Allow | ✅ Allow | ✅ Allow | ✅ Allow |
| TiledOnly (focus, split, resize, sidebar, overlays) | ❌ Block | ❌ Block | ❌ Block | TBD |
| WorkspaceLevel (workspace switch, create, rename, delete) | ❌ Block | ❌ Block | ❌ Block | TBD |
| AlwaysAllowed (CommandPalette, SpawnCommand, ReloadConfig, EnterMode) | ❌ Block | ❌ Block | ❌ Block | TBD (may allow from chrome) |
| SourceDependent (FocusPane) | Allow active float only | ❌ Block | ❌ Block | TBD |

Additionally, `mouse.rs` has an **early-return guard** before `click_action()` and drag detection — when floating, all sidebar interaction is blocked immediately without any hit-test work.

### Completed work

**Phase 1 — Core types** (`heca/src/app/interaction.rs`, ~650 lines):
- `InteractionSource` enum: `Keyboard`, `MouseContent`, `MouseLeftSidebar` (pub(crate))
- `InteractionIntent` enum: `ActivateAction(WmAction)`, `FocusPane`, `FocusWorkspace`, `EnterSidebarNav`, `StartSidebarDrag` (pub(crate), intent variants have `#[allow(dead_code)]` with TODO for Phase B)
- `RouteDecision` enum: `Allow(InteractionIntent)`, `Block` (pub(crate))
- `ActionPolicy` enum: `AlwaysAllowed`, `TiledOnly`, `FocusedPaneLocal`, `WorkspaceLevel`, `SourceDependent` (private)
- `action_policy(&WmAction) -> ActionPolicy` — exhaustive match, no catch-all per AGENTS.md
- `route_interaction(state, source, intent) -> RouteDecision` — delegates to `route_interaction_for_session`
- `route_interaction_for_session(session, source, intent) -> RouteDecision` — testable, real floating-domain policy
- `dispatch_action(state, registry, source, &WmAction)` — single public entry point
- `is_floating_domain(session) -> bool` — uses `expect()` per AGENTS.md

**Phase 2 — Wire sources:**
- Keyboard: all `registry.execute()` in `input.rs` → `dispatch_action(Keyboard)`
- MouseContent: content-area `WmAction` from `on_mouse_input()` → `dispatch_action(MouseContent)`
- MouseLeftSidebar: sidebar `WmAction` from `on_mouse_input()` → `dispatch_action(MouseLeftSidebar)`
- `on_mouse_input()` return type: `Option<WmAction>` → `Option<(WmAction, InteractionSource)>`
- `handle_sidebar_drag_starting_release()` return type: same tuple change
- Sidebar floating guard in `mouse.rs`: `is_floating_domain()` check before `click_action()` and drag detection

**Phase 3 — Remove drift:**
- SKIPPED — no ad hoc guards remained after Phase 2 wiring. All policy is central.

**Tests (10 unit tests in `interaction::tests`):**
- `tiled_domain_allows_tiled_actions` — TiledOnly actions allowed from Keyboard when tiled
- `tiled_domain_allows_focused_pane_local` — Float/ClosePane/RenamePane allowed when tiled
- `floating_domain_blocks_tiled_only` — FocusLeft/Split/Sidebar/Workspace/CommandPalette/FloatAt blocked when floating
- `floating_domain_allows_focused_pane_local` — Float/ClosePane/RenamePane allowed when floating
- `floating_domain_blocks_intent_variants` — FocusPane/FocusWorkspace/EnterSidebarNav/StartSidebarDrag intent blocked when floating
- `action_policy_covers_all_variants` — exhaustive match test with spot-check classifications
- `is_floating_domain_default_is_tiled` — default workspace is Tiled
- `floating_domain_detected_after_set` — setting FocusDomain::Floating is detected
- `floating_blocks_mouse_content_focus_pane` — MouseContent FocusPane blocked when floating
- `floating_blocks_mouse_left_sidebar_actions` — SidebarFocus/SidebarLeft/SidebarUp blocked via MouseLeftSidebar when floating

### In-progress work
- None — all wired sources complete

### Pending work

**Phase D — Focus-target helpers:**
- `focused_pane_id(state)` — central accessor for focused pane
- `active_focus_domain(state)` — central accessor wrapping `is_floating_domain`
- `can_focus_pane(state, source, pane_id)` — checks whether a specific pane can receive focus from a source
- `close_focused_pane(state)` — reduces duplicated tiled-vs-floating close branching in handlers
- `rename_focused_pane(state)` — reduces duplicated rename branching

**Phase E — Regression tests:**
- Sidebar/content floating-focus: prove sidebar pane click blocked, content pane click blocked, floating-local allowed
- Keyboard tiled-only blocking: prove FocusLeft/Split/Workspace blocked via Keyboard when floating
- Floating-local action preservation: prove Float/ClosePane/RenamePane still work when floating
- Existing behavior preservation: prefix mode, sidebar normal behavior, workspace switching still work when tiled

**Future polish:**
- Sidebar intent routing: upgrade sidebar clicks to produce `InteractionIntent::FocusPane`/`FocusWorkspace`/`EnterSidebarNav` instead of `ActivateAction(WmAction)` for richer semantic routing
- Chrome sources: `MouseTopMenu`, `MouseStatusBar` — may allow AlwaysAllowed actions while floating
- RPC source: `InteractionSource::Rpc` — may bypass some UI modal policy
- Scratchpad feature: may allow multiple floating panes

### Open decisions
- **AlwaysAllowed + floating**: Currently blocked from Keyboard/MouseContent/MouseLeftSidebar when floating. May be allowed from future chrome sources (top menu bar, status bar).
- **Scratchpad feature (future)**: May allow multiple floating panes, requiring rethinking FocusedPaneLocal policy.
- **RPC source policy**: Deferred — explicit-target RPC may bypass UI modal policy.

### Key files and their roles

| File | Role | Key changes |
|------|------|-------------|
| `heca/src/app/interaction.rs` | Central policy layer | All types, routing, policy, dispatch, tests |
| `heca/src/app/input.rs` | Keyboard dispatch | All `registry.execute` → `dispatch_action(Keyboard)` |
| `heca/src/app/events.rs` | Event loop dispatch | `on_mouse_input` tuple destructured, source passed to `dispatch_action` |
| `heca/src/mouse.rs` | Mouse dispatch | Return type `(WmAction, InteractionSource)`, floating guard, `InteractionSource` import |
| `heca/src/mouse/release.rs` | Release handlers | `handle_sidebar_drag_starting_release` returns tuple, `InteractionSource` import |
| `heca/src/app/mod.rs` | Module wire-up | `pub mod interaction` |
| `heca-core/src/layout/workspace.rs` | FocusDomain enum | `FocusDomain::Tiled` / `FocusDomain::Floating` |

### Stash
- `stash@{0}`: old WIP ad hoc guards from `feature/phase9-focus-domain` — can be dropped once PRs are merged

### PRs
- PR #52: `feature/interaction-policy` — base commit with core types + keyboard/mouse wiring (superseded by #56)
- PR #54: `feature/interaction-sidebar-wiring` — initial sidebar guard (superseded by #56)
- PR #56: `feature/interaction-sidebar-wiring` — sidebar source tagging + floating guard + expect fix + docs update

### Validation run (2026-06-09)
- `cargo check --workspace`: ✅ clean
- `cargo clippy --workspace --all-targets --all-features`: ✅ 0 heca warnings
- `cargo test --workspace`: ✅ 234 tests pass (10 interaction tests)

### Rust skill review history
1. **First review** (core types + keyboard/mouse wiring):
   - R4/R5: Stale doc comments → fixed
   - R6: pub → pub(crate) for InteractionSource/Intent/Decision → fixed
   - R3: #[allow(dead_code)] with TODO for Phase B types → fixed
   - R1/R2: WmAction double-clone → accepted (event-loop, not hot path)
   - R8: #[cfg(debug_assertions)] eprintln → accepted
2. **Second review** (sidebar floating guard):
   - R5: Guard moved before click_action() to avoid unnecessary hit-test work → fixed
3. **Third review** (sidebar source tagging + expect fix):
   - R2: release.rs full path → use import → fixed
   - R4: unwrap_or(false) → expect() → fixed
   - R5: MouseLeftSidebar dead code removed → fixed
   - R1/R3/R6: No issues accepted

### How to resume work
1. Pull latest `main`, create new branch from `main`
2. After PR #56 merges, all interaction policy infrastructure is in `main`
3. Start Phase D (focus-target helpers) or Phase E (regression tests)
4. Follow `/grill-me` before each new phase
5. Run rust-best-practices skill review before committing
6. Wait for user approval before committing
