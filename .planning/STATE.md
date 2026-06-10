---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
current_phase: 10 of 10 (refactoring track)
status: executing
last_updated: "2026-06-09T23:30:00.000Z"
progress:
  total_phases: 10
  completed_phases: 9
  total_plans: 1
  completed_plans: 0
  percent: 90
---

# State: heca

**Current Phase:** 10 — Final Whole-Program Verification
**Status:** Running automated validation; manual smoke tests pending
**Last Action:** Completed Phase 9 (Focus Separation / Interaction Policy)

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
| 9 — Focus Separation | ✅ Done | Interaction policy layer, FocusDomain, 29 regression tests |
| 10 — Final Verification | 🔄 In Progress | Full validation, smoke tests, doc reconciliation |

## Product Phase Progress

| Phase | Status | Requirements |
|-------|--------|-------------|
| 1 — The Shell | ✅ Done | 8/8 |
| 2 — The Workspace | ✅ Done | 28/28 |
| 3 — The Content | 🔄 In Progress | PANE-01/02 done, Neovim pending |
| 4 — The Platform | ⬜ Pending | Session, RPC, plugins |

## Validation (Phase 10.1)

- `cargo check --workspace`: ✅ clean
- `cargo clippy --workspace --all-targets --all-features`: ✅ 0 heca warnings
- `cargo test --workspace`: ✅ 264 tests pass, 0 failures

## Remaining Phase 10 Items

- 10.2: Manual smoke tests (requires running the app)
- 10.3: Reconcile docs/checklists/roadmaps ✅ (in progress)
- 10.4: Confirm no phase-level regressions without explicit deferment ✅ (only 9.V manual test remains)

## Open Deferments

- 8.3: Terminal render-data cloning (terminal backend not fully wired)
- 9.V: Manual smoke test of floating focus vs sidebar/content click
- Interaction policy: sidebar intent routing, chrome sources, RPC source (future phases)
- Interaction policy R6: convert `focused_pane_id` return type to `Option<PaneId>` (future polish)

## Blockers

None.