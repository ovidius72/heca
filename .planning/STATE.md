---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 10 of 10 (refactoring track)
status: executing
last_updated: "2026-06-10T12:00:00.000Z"
progress:
  total_phases: 10
  completed_phases: 9
  total_plans: 1
  completed_plans: 0
  percent: 90
---

# State: heca

**Current Phase:** 10 — Final Whole-Program Verification
**Status:** Bug fix + refactoring complete; manual smoke tests pending
**Last Action:** Fixed floating pane close bug, removed FakeBackend recreation, added arrow key bindings

## Refactoring Track Progress

| Phase | Status | Focus |
|-------|--------|-------|
| 0 — Safety Net | ✅ Done | Test audit, behavior coverage |
| 1 — File Reorganization | ✅ Done | Split monolithic files |
| 2 — Central Mutation Boundary | ✅ Done | after_layout_change hook, mutation helpers |
| 3 — Shared Pane Ops | ✅ Done | pane_ops, DnD refactoring, ad hoc scan removal |
| 4 — Sidebar Projection Model | ✅ Done | SidebarItemKind, sync, HashMap pane lookup |
| 5 — Backend Lifecycle | ✅ Done | BackendStore wrapper, lifecycle contract |
| 6 — Stale State | ✅ Done | Remove dead Rect, dormant fields, placeholders, #[allow] audit |
| 7 — Typed Errors | ✅ Done | ConfigError, PtyError, RpcError, SAFETY comments |
| 8 — Constants & Polish | ✅ Done | Centralize constants, renderer API doc (8.3 deferred) |
| 9 — Focus Separation | ✅ Done | Interaction policy, FocusDomain, close pane fix, arrow keys |
| 10 — Final Verification | 🔄 In Progress | Full validation, smoke tests, doc reconciliation |

## Product Phase Progress

| Phase | Status | Requirements |
|-------|--------|-------------|
| 1 — The Shell | ✅ Done | 8/8 |
| 2 — The Workspace | ✅ Done | 28/28 |
| 3 — The Content | 🔄 In Progress | PANE-01/02 done, Neovim pending |
| 4 — The Platform | ⬜ Pending | Session, RPC, plugins |

## Validation

- `cargo check --workspace`: ✅ clean
- `cargo clippy --workspace --all-targets --all-features`: ✅ 0 heca warnings
- `cargo test --workspace`: ✅ 274 tests pass, 0 failures

## Changes in this session (Phase 9.6 + 9.7)

### Bug fix: floating pane close targets wrong pane
- `handle_close_pane` now uses `focused_pane_id()` + `pane_is_floating()` to correctly identify which pane to close
- Floating close path: removes the floating pane, switches domain to Tiled only when no floats remain, focuses the last visited tiled pane
- Tiled close path: unchanged behavior, but now uses `!ws.has_panes()` for emptiness check (includes floating panes)

### Remove FakeBackend recreation on empty workspace
- Both `handle_close_pane` and `handle_close_pane_by_id` no longer create a `FakeBackend` pane when a workspace becomes empty
- If a workspace is the only one and becomes empty, it's left empty (user creates new panes with prefix+Enter or prefix+v)
- If multiple workspaces exist and one becomes empty, it's destroyed

### Handler decomposition
- Extracted `close_floating_pane()`, `close_tiled_pane()`, `close_workspace_if_empty()` helpers from `handle_close_pane`
- `handle_close_pane_by_id` reuses `close_workspace_if_empty()` (was duplicating the logic + FakeBackend recreation)
- `handle_close_pane_by_id` now calls `deactivate_floating_panes()` when switching domain (was missing)

### Arrow key bindings
- Added `prefix+ArrowLeft/Right/Up/Down` as aliases for `prefix+h/l/k/j` (focus navigation)
- Updated defaults in `heca-config/src/keys.rs`, `keybindings.toml`, `README.md`, `AGENTS.md`

### Rust hygiene
- `let else` instead of `match` for early return in `handle_close_pane`
- `is_floating` instead of `is_flt`
- `#[cfg(debug_assertions)]` diagnostic for state inconsistency in floating close
- Doc comments on `handle_close_pane`, `handle_close_pane_by_id`, and all three helpers
- `#[allow(dead_code)]` with reason comments replacing bare `#[allow(dead_code)]` or `#[expect(dead_code)]`
- Assert messages on key workspace test assertions
- 9 regression tests in `heca-core/src/layout/workspace.rs`

### Interaction policy cleanup
- Removed `#[expect(dead_code)]` from `InteractionIntent` variants (they're used in match arms)
- Cleaned up TODO comments (removed stale `TODO(wire-intents)`, kept actionable `#[allow(dead_code)]` with reasons)

## Remaining Phase 10 Items

- 10.2: Manual smoke tests (requires running the app)
- 10.3: Reconcile docs/checklists/roadmaps ✅ (done in this session)
- 10.4: Confirm no phase-level regressions without explicit deferment ✅

## Open Deferments

- 8.3: Terminal render-data cloning (terminal backend not fully wired)
- 9.V: Manual smoke test of floating focus vs sidebar/content click
- Interaction policy: sidebar intent routing, chrome sources, RPC source (future phases)
- Interaction policy R6: convert `focused_pane_id` return type to `Option<PaneId>` (future polish)

## Blockers

None.