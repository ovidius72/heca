# Session Resume Handoff — 2026-06-10

## Current state

- Branch: `main` (all refactoring PRs merged)
- Working tree: **clean**
- 279 tests pass, clippy clean
- Refactoring track (Phases 0–10) is **complete**

## What was completed (refactoring track)

| Phase | Focus | PR |
|-------|-------|-----|
| 0 — Safety Net | Test audit, behavior coverage | early PRs |
| 1 — File Reorganization | Split monolithic files | #34 |
| 2 — Central Mutation Boundary | after_layout_change hook, mutation helpers | #36 |
| 3 — Shared Pane Ops | pane_ops, DnD refactoring, ad hoc scan removal | #36 |
| 4 — Sidebar Projection Model | SidebarItemKind, sync, HashMap pane lookup | #36 |
| 5 — Backend Lifecycle | BackendStore wrapper, lifecycle contract | #42 |
| 6 — Stale State | Remove dead Rect, dormant fields, #[allow] audit | #42 |
| 7 — Typed Errors | ConfigError, PtyError, RpcError, SAFETY comments | various |
| 8 — Constants & Polish | Centralize constants, renderer API doc | various |
| 9 — Focus Separation | Interaction policy, FocusDomain, close pane fix, arrow keys | #58, #59, #64, #69 |
| 10 — Final Verification | Automated validation, doc reconciliation | #62 |

Additional:
- PR #72: Intent dispatch wiring (FocusPane/FocusWorkspace/EnterSidebarNav reach handlers)

## Key rules to remember

1. **Load Rust skill file and check every rule before committing** — not just a quick summary
2. **Run `cargo clippy --workspace --all-targets --all-features` without filtering warnings**
3. **Wait for user approval before committing**
4. **`block v0.1.6` warning** — external dep (metal→wgpu), not ours to fix

## Important files

- Roadmap: `.planning/ROADMAP.md`
- State: `.planning/STATE.md`
- Interaction policy plan: `.planning/interaction-policy-plan.md`
- Chrome/plugin architecture: `pluggable-chrome-plugin-plan.md`
- Agent rules: `AGENTS.md`

## Next steps (post-refactoring)

1. **Sidebar intent routing** (Phase B/C) — Convert mouse/sidebar clicks from raw `WmAction` to `InteractionIntent` variants, remove ad hoc local guards
2. **Phase 3 — The Content** — Neovim backend, terminal mouse forwarding
3. **Phase 4 — The Platform** — Session persistence, RPC server, plugin runtime
4. **Chrome/plugin architecture** — See `pluggable-chrome-plugin-plan.md`