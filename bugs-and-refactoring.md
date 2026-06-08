# Heca — Bugs, Gaps, and Refactoring Roadmap

**Scope:** `heca-core`, `heca-renderer`, `heca-config`, `heca/src`  
**Excluded:** `heca-grid-ui`, `heca-ui`  
**Date:** 2026-06-04  
**Last updated:** 2026-06-05

---

## Plan Change Notice — 2026-06-05

This roadmap changed on **2026-06-05** after new requirements emerged around:
- a pluggable chrome architecture rather than a sidebar treated only as a workspace tree
- region-level extensibility for left sidebar, right sidebar, top bar, and bottom bar
- a future built-in `WorkspacesContainer` rather than a monolithic hardcoded sidebar
- future **WASM plugins** with event subscriptions, host-owned overlays, and action dispatch
- dynamic plugin-provided actions that must later integrate with config keybindings

Those new requirements are captured in:
- `pluggable-chrome-plugin-plan.md`

Important interpretation rule:
- this document still describes the current codebase problems and why the **current cleanup/refactor** is still necessary
- but the long-term architecture target has changed and should be read alongside the new plan document above

---

## 1. Executive Summary

The codebase is in a **better state than the 2026-06-03 review** suggested, but it is still hard to reason about because too much behavior is spread across a few very large files and functions.

The layout model is still the right one:

```text
Session -> Workspace -> ScrollingSpace -> Column -> Pane
```

That part remains a good NIRI-inspired foundation.

What is no longer the main problem:
- production debug logging in the touched paths was removed
- floating border rendering and "multiple active floating panes" were fixed
- behavior is **not** entirely outside the domain types anymore: `Session`, `Workspace`, and especially `ScrollingSpace` already own meaningful behavior

What is now the main problem:
- **module and function bloat**
- **unclear ownership boundaries** between layout state, UI state, backend lifecycle, and event handling
- **duplicated interaction logic** between keyboard handlers, mouse drag/drop, and sidebar activation flows
- **sidebar projection drift** and UI-state rebuild issues
- **focus-domain drift** between floating focus state and tiled action targeting

The biggest readability and maintainability win is still **reorganizing the code first**, then moving the remaining cross-cutting behavior behind clearer boundaries.

However, as of **2026-06-05**, that cleanup is no longer the whole story. The long-term direction now includes a pluggable chrome host, built-in container providers, dynamic actions, and future WASM plugins. That new architecture should be started only after the current refactor reaches its planned stopping point.

---

## 2. What Changed Since the Previous Review

## Resolved / no longer accurate as written

| Previous finding | Current status | Notes |
|---|---|---|
| Production `eprintln!()` spam in hot paths | **Fixed** | The debug logging called out in the prior review is gone from the touched app paths. |
| "All behavior lives outside the types" | **Partially stale** | `ScrollingSpace` now owns substantial behavior: focus changes, add/remove column/pane, insert positions, pane moves, resize logic, etc. `Session` and `Workspace` also own more behavior than before. |
| Floating border visibility / active float state inconsistency | **Fixed** | Border draw order is corrected and floating active state now has a dedicated normalization path. |
| "Interaction code is almost untested" | **No longer accurate** | There are focused unit tests in `sidebar.rs`, `mouse.rs`, `session.rs`, `keymap.rs`, `input.rs`, etc. The problem is more about **coverage shape** than total test count. |

## Still valid, but severity/shape changed

| Previous finding | Current status | Updated interpretation |
|---|---|---|
| Sidebar drift / rebuild resets collapse | **Still valid** | Still a real bug and now more important because floating sidebar rows added more special cases. |
| `AppState` is a god object | **Still valid** | Slightly smaller than before, but responsibility boundaries are still blurred. |
| `#[allow(dead_code)]` overuse | **Partially valid** | Some suppressions now have comments/reasons, but the broad ones are still a smell. |
| `Rect` vs `Rectangle` split | **Still valid** | `heca-core/src/types.rs::Rect` is still dead legacy baggage. |
| manual `backends` lifecycle | **Still valid** | Pane removal and backend removal are still coupled only by discipline. |
| typed errors missing | **Still valid** | `Result<_, String>` still appears in key infrastructure. |
| unsafe blocks lack `// SAFETY:` comments | **Still valid** | This has not been cleaned up. |

---

## 3. Current High-Priority Findings

## A1. The code is dominated by a handful of oversized files and functions

### Current file sizes

| File | LOC |
|---|---:|
| `heca/src/main.rs` | 2440 |
| `heca/src/mouse.rs` | 1782 |
| `heca/src/handlers.rs` | 1646 |
| `heca/src/sidebar.rs` | 1564 |
| `heca-config/src/theme.rs` | 854 |
| `heca-core/src/layout/scrolling.rs` | 966 |

### Largest functions right now

| Function | LOC | File |
|---|---:|---|
| `window_event` | 558 | `heca/src/main.rs` |
| `render` | 543 | `heca/src/main.rs` |
| `handle_swap_param` | 453 | `heca/src/handlers.rs` |
| `sidebar_handle_drop` | 209 | `heca/src/mouse.rs` |
| `sidebar_drag_drop` | 205 | `heca/src/mouse.rs` |
| `drop_pane` | 195 | `heca/src/mouse.rs` |
| `render_sidebar_expanded` | 421 | `heca/src/sidebar.rs` |
| `render_sidebar_collapsed` | 195 | `heca/src/sidebar.rs` |
| `build_registry` | 184 | `heca/src/main.rs` |

### Why this matters

This is the clearest maintainability problem in the repo.

These functions mix:
- domain mutation
- input interpretation
- UI state transitions
- rendering decisions
- post-mutation synchronization
- backend bookkeeping

That makes local changes expensive and increases the chance of subtle regressions.

### Recommendation

**First refactor for shape, then for semantics.**

Do not start by rewriting behavior. Start by splitting these files into smaller modules with stable public seams.

---

## A2. Ownership boundaries are still blurry

`AppState` still carries too many unrelated concerns at once:
- GPU and window objects
- layout/session state
- pane backends
- sidebar UI state
- input mode state
- focus history
- mouse drag state
- config-driven runtime flags

This is not just "too many fields". The deeper issue is that **mutation ownership is unclear**.

Examples:
- `Session` owns layout topology
- `Workspace` owns tiled + floating pane placement
- `AppState.backends` owns backend runtime objects
- `sync_focus()` updates focus bookkeeping **and** rebuilds the sidebar
- `main.rs` still owns helpers like `focus_pane_by_id()`, `move_pane_to_column()`, `move_pane_to_workspace_column()`, `destroy_empty_workspace()`

### Recommendation

Introduce a clearer split:

| Concern | Suggested owner |
|---|---|
| layout topology and geometry | `Session` / `Workspace` / `ScrollingSpace` |
| pane runtime lifecycle | `BackendStore` or `PaneRuntimeStore` |
| focus history + sidebar sync bookkeeping | `FocusTracker` / `UiState` |
| input mode, prefix/chord/sidebar mode | `UiState` |
| GPU render resources | `RenderState` |
| orchestration across all of the above | `AppController` / `WmRuntime` |

This keeps the NIRI-like layout core pure without forcing all app-specific concerns into it.

---

## A3. Session mutation still depends on scattered post-hooks

A large amount of behavior still relies on remembering to call:
- `sync_focus(state)`
- `state.sidebar_tree.rebuild(...)`
- `state.needs_redraw = true`

`sync_focus(state)` appears in many handler and mouse paths, and `sidebar_tree.rebuild(...)` still happens manually.

### Why this matters

The layout mutation itself is not the full operation. The real mutation contract is more like:

```rust
mutate_session();
normalize_focus();
update_focus_history();
rebuild_sidebar_projection();
request_redraw();
```

Right now that contract is implicit.

### Recommendation

Create a single post-mutation hook, e.g.:

```rust
fn after_layout_change(state: &mut AppState) {
    sync_focus(state);
    state.needs_redraw = true;
}
```

Then evolve that into a more explicit controller boundary.

This will dramatically reduce the mental overhead of changing behavior.

---

## A4. `SidebarTree` is still a duplicated projection, and it still forgets UI state

**Updated architectural interpretation — 2026-06-05:** this finding still matters, but the future target should now be understood as a `WorkspacesContainer` projection/view-model living inside a broader pluggable chrome system, not as the final architecture of the entire sidebar.

`SidebarTree::rebuild()` still clears and reconstructs the sidebar model from scratch:

- `self.workspaces.clear()`
- `self.flat_items.clear()`
- pane/workspace names are cloned again
- `collapsed: false` is still hardcoded for rebuilt workspace/column entries

### Confirmed current bug

Collapsed state is still reset on rebuild.

That means the previous review’s warning is still valid.

### New maintainability wrinkle

Floating pane rows are now shown in the sidebar as **display-only** rows, but the sidebar model still stores them in `flat_items` alongside navigable/selectable items.

That forces the code to special-case them in multiple places:
- `is_navigable()`
- `is_navigable_collapsed()`
- `sidebar_hit_test()`
- sidebar handlers in `handlers.rs`

This is a design smell: the model cannot clearly express the difference between:
- selectable tree nodes
- display-only rows
- drag/drop targets

### Recommendation

Keep the sidebar as a projection, but make that explicit:

```rust
SidebarTree = projection + UI state
```

Split it into:
- **projection data** derived from `Session`
- **UI state** preserved across rebuilds (`cursor`, `scroll_offset`, collapsed maps)
- **row kind metadata** (`Selectable`, `DisplayOnly`, `DropTarget`, etc.)

That removes the current "rebuild then patch behavior with ad hoc skip logic" pattern.

---

## A5. Interaction logic is duplicated across keyboard, mouse, and sidebar flows

This finding becomes even more important after the 2026-06-05 plan change, because a future provider/plugin system will need cleaner host-side interaction seams rather than more hardcoded sidebar-special behavior.

The same conceptual operations are implemented in multiple places:
- pane focus by id
- sidebar activation behavior
- swap/move logic
- cross-workspace pane relocation
- drag-drop insertion

Examples:
- `handle_swap_param()` in `handlers.rs` is very large and owns swap semantics
- `drop_pane()`, `sidebar_drag_drop()`, and `sidebar_handle_drop()` in `mouse.rs` reimplement related placement logic
- sidebar activation behavior exists in both `main.rs` and `handlers.rs`

### Why this matters

Even when behavior is "correct", it is hard to know which path is canonical.

This is where readability is currently lost the most.

### Recommendation

Extract pure operation units first, e.g.:
- `swap_panes_same_column(...)`
- `swap_panes_same_workspace(...)`
- `swap_panes_cross_workspace(...)`
- `insert_pane_after_target(...)`
- `reinsert_detached_pane(...)`
- `activate_sidebar_item(...)`

Then let handlers and mouse code call those shared operations.

---

## A6. Floating panes have a focus-domain / action-targeting bug

The current floating model has improved, but there is still a real correctness bug:

- a floating pane can be visually and logically focused
- `AppState.focused_pane` can point at that floating pane
- `Workspace::active_pane()` can correctly report the active floating pane
- mouse hit testing checks floating panes first

However, many handlers still perform mutations directly against the tiled scrolling layout underneath the float.

### Root cause

There are effectively **two active-state domains** in the current design:

1. **Focused-pane domain**
   - `AppState.focused_pane`
   - `Workspace::floating_is_active`
   - `FloatingPane::is_active`
   - `Workspace::active_pane()`

2. **Tiled-layout active selection domain**
   - `ScrollingSpace::active_column_idx`
   - `Column::active_pane_idx`

When a pane is floated, the focused-pane domain switches to floating state, but the tiled active selection remains alive underneath.

That is not inherently wrong — it can be useful state to preserve. The bug is that many actions ignore which domain is active and go straight to `ws.scrolling...`.

### Observed symptom

When a floating pane is focused, actions like:
- zoom column
- resize column
- resize pane height
- move pane left/right
- swap column / swap pane
- close pane

can affect the tiled pane/column underneath instead of the focused floating pane, or can otherwise mutate the wrong domain.

This makes the floating pane look "not really focused" even though focus rendering and `focused_pane` bookkeeping are largely correct.

### Confirmed supporting evidence

These parts already work as intended:
- `focus_pane_by_id()` activates floating panes correctly
- `sync_focus()` reads focus from the workspace active pane
- `Workspace::active_pane()` prefers floating when `floating_is_active`
- `mouse::hit_test_pane()` tests floating panes before scrolling panes

So the bug is **not primarily focus painting**.
It is **action targeting**.

### Confirmed affected handler shape

A large class of handlers currently do this pattern:

```rust
if let Some(ws) = state.session.active_workspace_mut() {
    ws.scrolling.some_mutation(...);
}
```

without first checking whether the active domain is floating.

Confirmed examples include:
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

There are likely more in the same category anywhere handlers access:
- `ws.scrolling.active_column_idx`
- `ws.scrolling.active_column()`
- `ws.scrolling.columns[...]`

as the assumed target.

### Why this is architecturally important

This is not just a one-off floating bug. It exposes a missing boundary in the app model:

- the code can answer **which pane is focused**
- but many mutations still assume **the tiled layout is the active target**

In other words, the app lacks an explicit **active focus domain / active mutation target** concept.

### Recommendation

Introduce a small routing concept before fixing handlers one by one.

For example, make the workspace or app controller answer something like:

```rust
enum FocusDomain {
    Tiled,
    Floating,
}
```

Then apply a clear policy:

- if `Floating` is active:
  - pane-local actions should target the focused floating pane
  - tiled-layout-only actions should no-op, or explicitly switch semantics if desired
- if `Tiled` is active:
  - existing `ws.scrolling...` behavior remains valid

### Recommended product behavior

The safest behavior is:
- a focused floating pane should behave as actually focused
- tiled layout actions should **not** mutate panes/columns underneath a floating pane by accident
- close/rename/focus-sensitive actions should operate on the focused pane, whether tiled or floating
- layout-only tiled actions should no-op while floating focus is active unless a future explicit floating equivalent exists

This should be treated as a correctness phase, not merely polish.

---

## A7. `main.rs` still owns too much application behavior

`main.rs` currently mixes:
- `HecaApp` initialization
- WGPU rendering
- key event handling
- mouse event forwarding
- prefix/chord/sidebar mode control
- config reload flow
- session/layout helper functions
- registry construction
- some pane/workspace mutation helpers

This is the single biggest readability problem after the giant swap/drag functions.

### Recommendation

Split `main.rs` by concern before changing logic:

```text
heca/src/app/
  mod.rs
  lifecycle.rs      // HecaApp init/resume/about_to_wait
  events.rs         // window_event and key/mouse dispatch
  render.rs         // frame rendering
  focus.rs          // sync_focus, focus tracking
  mutations.rs      // post-mutation hooks / helper orchestration
  registry.rs       // build_registry
```

This keeps the event loop intact while making the code searchable and reviewable.

---

## A7. Action metadata has started to drift from real actions

`ActionRegistry::ALL` still contains descriptors like:
- `tab_next`
- `tab_prev`
- `swap_select`
- `swap_and_focus`

But these do not match the current `WmAction` / `action_from_name()` surface cleanly.

At the same time, the real action surface includes more programmatic actions not represented in the catalog.

### Why this matters

This will create confusion in:
- command palette integration
- docs generation
- future config validation
- keybinding discoverability

### Recommendation

Treat action metadata as a real contract.

Either:
1. keep only user-facing/configurable actions in `ActionRegistry::ALL`, or
2. generate descriptors from a single authoritative action definition layer

But do not let the metadata catalog drift independently.

---

## 4. Medium-Priority Findings

## M1. Domain behavior is better encapsulated than before, but the boundary is incomplete

This is the main place where the previous review must be corrected.

`ScrollingSpace` already owns meaningful operations:
- `add_column`
- `add_pane_to_column`
- `remove_pane`
- `remove_column`
- `move_active_pane_to_column`
- `move_active_pane_to_new_column`
- `move_column_to`
- `resize_active_column`
- `insert_position`
- focus and view-offset behavior

`Session` and `Workspace` also own non-trivial behavior.

So the problem is **not** "behavior lives nowhere".

The problem is that the remaining app-level orchestration is still too spread out and too large.

### Updated recommendation

Do **not** force every operation into `Session` just because it mutates layout.

Instead:
- keep pure layout invariants in layout types
- move app-specific orchestration into a dedicated controller/runtime layer
- stop leaving important orchestration in `main.rs`

This better matches heca’s architecture as a GPU app with PTY backends, not a literal NIRI clone.

---

## M2. Manual backend lifecycle is still fragile

`AppState.backends: HashMap<u64, Box<dyn PaneBackend>>` is still kept in sync manually.

There are still explicit `backends.remove(...)` calls in close/delete flows.

### Why this matters

The layout tree and runtime backend store are still coupled by convention.

### Recommendation

Create an explicit backend owner abstraction:

```rust
struct BackendStore {
    by_pane: HashMap<u64, Box<dyn PaneBackend>>,
}
```

Then expose narrow operations like:
- `insert_backend_for_pane(...)`
- `remove_backend_for_pane(...)`
- `backend_for_pane(...)`

That does not fully solve ownership coupling, but it removes raw `HashMap` mutation from unrelated code paths.

---

## M3. `BackendRenderData` and `PaneType` still carry dead placeholders

Still present:
- `PaneType::Neovim`
- `PaneType::Browser`
- `BackendRenderData::Neovim`
- `BackendRenderData::Browser`

These are not integrated into real behavior yet.

### Recommendation

Until those backends exist, remove the unused variants or gate them behind a future feature/phase boundary.

---

## M4. Legacy dead state and placeholder state remain in the model

Confirmed examples:
- `heca-core/src/types.rs::Rect` is still legacy dead code
- `Workspace::floating_visible` appears unused
- `Workspace::is_pinned` appears unused
- `AppState::active_tab` / `tab_names` are only used for drawing a placeholder tab bar

These fields increase cognitive load because readers have to keep asking "is this real state or future state?"

### Recommendation

Decide field-by-field:
- remove it now, or
- gate it explicitly for a future milestone, or
- document it as intentionally dormant

---

## M5. `theme.rs` is still a monolith

`heca-config/src/theme.rs` still mixes:
- color types
- theme palettes
- settings schema
- keybinding schema
- config loading
- defaults
- tests

### Recommendation

Split into at least:

```text
heca-config/src/
  color.rs
  theme.rs
  settings.rs
  keys.rs
  loader.rs
  defaults.rs
```

This is a readability refactor with very low semantic risk.

---

## M6. Hardcoded sidebar bindings live in app code

`build_keymap()` still hardcodes the sidebar-mode bindings.

That is okay temporarily, but it creates another source of truth outside config defaults and action metadata.

### Recommendation

Move sidebar bindings into the same declarative config/default system as the rest of the keymap, even if they are still mode-scoped.

---

## M7. Floating focus navigation is safer than mutation routing, but the policy is still implicit

Some navigation code is already more careful than the mutation handlers.

For example, `Workspace::focus_left/right/up/down()` checks `floating_is_active` and currently avoids blindly mutating the tiled layout when floating is active.

That means the floating bug shape is asymmetric:
- navigation is partially guarded
- mutation handlers are often not

### Why this matters

This makes the current behavior harder to reason about because there is no explicit documented policy for:
- what should happen when a floating pane is focused
- which actions are valid in floating context
- which actions should no-op
- which actions should target floating vs tiled domains

### Recommendation

Document and centralize the policy instead of relying on scattered conditionals.

The system should have one authoritative answer for:
- current focus domain
- current mutation target
- allowed actions for that domain

Until then, floating behavior will continue to regress as new handlers are added.

---

## M8. Typed error handling is still missing in core paths

Still present:
- `theme.rs::load_config_file() -> Result<Config, String>`
- PTY/terminal construction returning `Result<_, String>`
- `execute_rpc_command() -> Result<WmAction, String>`
- `ActionRegistry::execute()` silently doing nothing when a handler is missing

### Recommendation

Move toward typed error enums where the subsystem boundary is stable:
- config loader errors
- PTY/backend startup errors
- RPC parse/dispatch errors
- invalid action dispatch

At minimum, `ActionRegistry::execute()` should not silently swallow missing handlers.

---

## M8. Terminal rendering still clones the full grid every frame

`TerminalBackend::render_data()` still constructs a new `Vec<TerminalLine>` and clones each visible cell on every call.

That is not the first refactor to do, but it remains an important performance/ownership issue.

### Recommendation

After structural cleanup, redesign backend rendering around:
- borrowed render views, or
- row iterators, or
- a renderer-owned staging/cache model

Do not optimize this before untangling ownership boundaries.

---

## 5. Low-Priority but Worth Cleaning Up

| ID | Issue | Why it matters |
|---|---|---|
| L1 | Broad `#![allow(dead_code)]` in `actions.rs` | The reason comment is stale now that `registry.execute()` is already used. |
| L2 | Several targeted `#[allow(dead_code)]` items remain | Some are justified, but they should be reviewed against actual roadmap state. |
| L3 | `unsafe` blocks in `terminal.rs` still lack `// SAFETY:` comments | Important for Rust hygiene and future auditing. |
| L4 | magic numbers remain scattered (`500ms`, `16ms`, `40.0`, row/button sizes, etc.) | Makes tuning and review harder. |
| L5 | `draw_rounded_rect()` still ignores its radius parameter | API suggests behavior that does not exist. |

---

## 6. Updated Target Architecture

The target should be clearer than "move everything into the layout types".

Because heca is **not** a 1:1 niri port, a better target is:

```text
layout core          -> Session / Workspace / ScrollingSpace / Column
backend runtime      -> BackendStore / pane runtime management
ui state             -> sidebar state, input mode, focus history, prefix/chord
mouse interaction    -> drag state + hit testing + drop routing
rendering            -> GPU/frame composition only
app controller       -> orchestrates mutations and post-hooks
```

### Suggested module layout

```text
heca/src/
  app/
    mod.rs
    lifecycle.rs
    events.rs
    render.rs
    registry.rs
    focus.rs
    mutations.rs
  sidebar/
    mod.rs
    model.rs
    projection.rs
    render.rs
    nav.rs
  mouse/
    mod.rs
    hit_test.rs
    drag.rs
    drop.rs
    render.rs
```

And for config:

```text
heca-config/src/
  color.rs
  theme.rs
  settings.rs
  keys.rs
  loader.rs
  defaults.rs
```

This is the reorganization that will make the code feel clear.

---

## 7. Refactoring Plan

## Phase 0 — Lock in behavior before moving code

Goal: make refactoring safe.

- add or extend tests for:
  - sidebar collapse persistence across rebuilds
  - same-column / same-workspace / cross-workspace swaps
  - floating ↔ tiled transitions
  - sidebar display-only floating rows
- add a few integration-style tests around the current session mutation helpers

This phase should avoid architectural changes.

---

## Phase 1 — Pure file/module split, no semantic changes

Goal: make the code navigable.

### 1A. Split `main.rs`
Extract:
- rendering
- event handling
- registry build
- focus/sync helpers
- mutation helpers

### 1B. Split `mouse.rs`
Extract:
- hit testing
- drag state machine
- drop logic
- drag rendering helpers

### 1C. Split `sidebar.rs`
Extract:
- projection/model
- hit testing
- rendering
- navigation helpers

### 1D. Split `theme.rs`
Extract config schema and loader from palette/default definitions.

**Success criteria:** no behavior changes, just smaller files and smaller diffs.

---

## Phase 2 — Introduce a central mutation boundary

Goal: stop scattering post-mutation bookkeeping.

Create a narrow orchestration layer, e.g. `AppController` / `WmRuntime`, which owns:
- session mutation entry points
- backend lifecycle updates
- focus/sidebar synchronization
- redraw requests

This lets callers stop doing manual sequences like:

```rust
mutate session
sync_focus
sidebar rebuild
needs_redraw = true
```

and instead call one operation.

---

## Phase 3 — Unify pane move/swap/focus operations

Goal: remove duplicated interaction logic.

Extract shared pure operations for:
- focusing a pane by id
- moving panes between columns
- moving panes between workspaces
- swapping panes in the 3 main cases:
  - same column
  - same workspace, different columns
  - different workspaces
- detaching/reinserting panes during mouse drag

Then make:
- keyboard handlers
- sidebar handlers
- mouse drop code

all call the same shared operations.

This is the phase that will most improve correctness.

---

## Phase 4 — Fix sidebar projection design

Goal: make sidebar state explicit and stable.

- preserve collapsed state across rebuilds
- split projection data from UI state
- represent row interactivity explicitly
- remove the need for ad hoc floating-row skip logic scattered across multiple functions

This also makes future sidebar features much easier.

---

## Phase 5 — Remove stale and placeholder state

Goal: reduce cognitive noise.

Candidates:
- `Rect` in `heca-core/src/types.rs`
- unused workspace flags
- placeholder tab state in `AppState`
- dead backend enum variants
- stale action metadata entries
- outdated `dead_code` suppressions

Do this only after the structural split, so removal is easy and low-risk.

---

## Phase 6 — Polish and performance

Goal: tighten code quality after the architecture is clearer.

- replace `Result<_, String>` with typed errors where stable
- add `// SAFETY:` comments to all unsafe blocks
- centralize constants / magic numbers
- redesign terminal render-data cloning path
- decide whether `draw_rounded_rect()` should be real or renamed

---

## 8. Recommended Order of Work

If the goal is **clarity first**, the best order is:

1. **Phase 1** — split giant files
2. **Phase 2** — central mutation boundary
3. **Phase 3** — unify swap/move/focus logic
4. **Phase 4** — sidebar projection cleanup
5. **Phase 5** — remove stale state and placeholder APIs
6. **Phase 6** — typed errors / safety / perf

That order gives the biggest readability win earliest.

---

## 9. Summary Table

| ID | Severity | Area | Finding |
|---|---|---|---|
| A1 | Critical | Structure | File/function bloat is now the dominant maintainability issue |
| A2 | High | Architecture | Ownership boundaries between layout, UI, backends, and orchestration are blurry |
| A3 | High | Synchronization | Post-mutation bookkeeping is implicit and scattered |
| A4 | High | Sidebar | Sidebar projection still duplicates state and forgets collapsed UI state |
| A5 | High | Interactions | Swap/move/drag behavior is duplicated across handlers and mouse flows |
| A6 | High | Actions | Action metadata has drifted from the actual action surface |
| M1 | Medium | Layout/app boundary | Domain behavior is improved, but the app-layer boundary is incomplete |
| M2 | Medium | Lifecycle | Backend ownership is still manual |
| M3 | Medium | Dead state | Placeholder backend variants remain |
| M4 | Medium | Dead state | Legacy `Rect` and other dormant fields still add noise |
| M5 | Medium | Config | `theme.rs` remains too large and overloaded |
| M6 | Medium | Input | Sidebar bindings are hardcoded outside the normal declarative keymap flow |
| M7 | Medium | Errors | Typed error handling is still missing in core paths |
| M8 | Medium | Perf | Terminal render data still clones the visible grid every frame |
| L1 | Low | Hygiene | Broad `dead_code` suppressions should be narrowed or removed |
| L2 | Low | Safety | `unsafe` blocks still need `// SAFETY:` comments |
| L3 | Low | API clarity | `draw_rounded_rect()` still ignores radius |
| L4 | Low | Hygiene | Magic numbers should be collected into named constants |

---

## 10. Bottom Line

The codebase does **not** need a wholesale rewrite.

It needs:
1. **reorganization first**
2. **clear mutation ownership second**
3. **deduplication of interaction logic third**

That path will make the code far easier to read and maintain without forcing heca into an architecture that only makes sense for a pure compositor.
