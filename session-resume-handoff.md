# Session Resume Handoff — 2026-06-05

## Current state

- Branch: `feature/gpt-refactoring`
- HEAD: `3f7d23f` — `docs: refresh handoff after merge sync`
- `origin/main` was pulled into the branch before this slice
- Working tree is currently dirty only from the docs/defaults refresh in progress

## Must-follow workflow rules

These are the active rules to keep following:

- Before starting any new phase or major sub-phase, use the **`/grill-me`** skill first.
- Keep work in small, behavior-preserving slices.
- Before committing / opening a PR, wait for user review/approval.
- When token usage is getting high (~70–80%), write a detailed handoff with enough info to restart without losing context.
- Before starting a new task/phase slice, pull/rebase from `origin/main`.

## What is completed

### Sidebar Phase 1.5
Completed and recorded in the checklist:
- `1.5.1` normalize sidebar navigation contract
- `1.5.2` add sidebar-only mutation keymap
- `1.5.3` make sidebar actions selection-driven
- `1.5.4` add mouse semantics for entering/exiting sidebar mode
- `1.5.5` add disclosure hit targets and visual symbols for workspace + column rows
- `1.5.6` add global sidebar-tree collapse action family
- `1.5.7` preserve public config/action surface for future RPC work
- `1.5.8` add/update tests for sidebar tree behavior
- `1.5.9` update docs and defaults

### RPC exposure added
The new collapse actions are RPC-exposed in `heca/src/rpc.rs`:
- `collapse-current-workspace`
- `expand-current-workspace`
- `toggle-current-workspace-collapsed`
- `collapse-current-column`
- `expand-current-column`
- `toggle-current-column-collapsed`

### Sidebar test coverage added/confirmed
`heca/src/sidebar/tests.rs` now covers:
- workspace collapse moving cursor to workspace row
- column collapse moving cursor to column row
- explicit workspace index toggle behavior
- explicit column index toggle behavior
- collapse persistence across rebuilds

### Docs/defaults refreshed
Updated user-facing references:
- `README.md` sidebar docs
- `keybindings.toml`
- `heca-config/src/keys.rs`

### Validation that passed in this slice
- `cargo test -p heca --quiet`
- `cargo clippy -p heca --all-targets --quiet`
- `cargo check --workspace --quiet`
- `cargo clippy --workspace --all-targets --all-features --quiet`

## Still open

- none for Phase 1.5

## Important rules already settled

- Sidebar mode is selection-driven.
- `j/k` and arrow keys move sidebar cursor only.
- Main scrolling/focus state does not auto-follow sidebar cursor movement.
- Global collapse actions use active main-view state.
- Global collapse actions do **not** open the sidebar if it is hidden.
- Floating focus / no valid tiled current column ⇒ current-column collapse actions no-op.
- When collapse hides the selected row, move the cursor to the collapsed parent row.

## Files to reference next

Planning / rules:
- `AGENTS.md`
- `bugs-and-refactoring-plan.md`
- `bugs-and-refactoring-plan-with-checklist.md`
- `pluggable-chrome-plugin-plan.md`

Implementation areas:
- `heca/src/input.rs`
- `heca/src/actions.rs`
- `heca/src/app/registry.rs`
- `heca/src/handlers.rs`
- `heca/src/rpc.rs`
- `heca/src/sidebar/model.rs`
- `heca/src/sidebar/tests.rs`
- `heca-config/src/keys.rs`
- `keybindings.toml`
- `README.md`

## Recent commits

- `3f7d23f` — `docs: refresh handoff after merge sync`
- `c60baf3` — merge of PR #27 into `main`
- `da66b26` — `feat(sidebar): expose collapse actions via rpc`
- `f884723` — `updated agents. add graphify`

## Next recommended step

Phase 1.5 is complete; move on to the next phase from the plan unless redirected.
