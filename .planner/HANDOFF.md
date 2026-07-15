Reason: session pause

# Handoff

## Done this session (branch `feat/action-task-c`, 4 commits, NOT pushed, on main ce6e02b)
- **action-task-C** (F003/P010/T002) DONE, confirm dialog runtime-verified.
  - confirm moved onto `ActionMeta.confirm` (keyed by action name; `close`→toggle `delete_pane`).
  - RPC introspection: `list-actions` / `describe-action` → `ActionInfo` (JSON).
  - `register(ActionSpec)` native one-call API.
  - Docs: docs/widgets.md + AGENTS.md "Adding New Actions".
- Also merged earlier: plugin-04 (#241), context-menu-5 (#242).

## Next
- Push `feat/action-task-c` → new PR when wanted.
- Deferrals already tracked in their tasks: plugin-08/`wasm-action-registry` (WASM adapter), `action-task-D` (built-in→register migration, optional).

## Rule
Keep task notes/handoffs SHORT.