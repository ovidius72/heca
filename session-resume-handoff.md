# Session Resume Handoff — 2026-06-05

## Current state

- Branch: `feature/gpt-refactoring`
- HEAD: `68b9415` — `docs(sidebar): finish phase 1.5 docs and tests`
- `origin/main` is current at `c60baf3`
- `origin/feature/gpt-refactoring` matches local `HEAD`
- Working tree is dirty only from the sidebar-nav configurability follow-up in progress
- PR #28 is open against `main`

## Must-follow workflow rules

These are the active rules to keep following:

- Before starting any new phase or major sub-phase, use the **`/grill-me`** skill first.
- Keep work in small, behavior-preserving slices.
- Before committing / opening a PR, wait for user review/approval.
- When token usage is getting high (~70–80%), write a detailed handoff with enough info to restart without losing context.
- Before starting a new task/phase slice, pull/rebase from `origin/main`.
- Every keybinding and theme variable must be configurable from `config.toml`.

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
- `1.5.10` make sidebar nav bindings configurable from `config.toml`

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
- configurable sidebar-mode default merging/override behavior

### Docs/defaults refreshed
Updated user-facing references:
- `README.md` sidebar docs
- `keybindings.toml`
- `heca-config/src/keys.rs`
- `AGENTS.md` coding standard update

### Validation that passed so far in this slice
- `cargo test -p heca --quiet`
- `cargo test -p heca-config --quiet`
- `cargo clippy -p heca --all-targets --quiet`

## Still open

- run the workspace-wide validation pass
- commit the configurability slice
- update / push PR #28 after review

## Important rules already settled

- Sidebar mode is selection-driven.
- `j/k` and arrow keys move sidebar cursor only.
- Main scrolling/focus state does not auto-follow sidebar cursor movement.
- Global collapse actions use active main-view state.
- Global collapse actions do **not** open the sidebar if it is hidden.
- Floating focus / no valid tiled current column ⇒ current-column collapse actions no-op.
- When collapse hides the selected row, move the cursor to the collapsed parent row.
- Sidebar-nav defaults are configurable via `[[keys.mode]] name = "sidebar"`.

## Files to reference next

Planning / rules:
- `AGENTS.md`
- `bugs-and-refactoring-plan.md`
- `bugs-and-refactoring-plan-with-checklist.md`
- `pluggable-chrome-plugin-plan.md`

Implementation areas:
- `heca/src/input.rs`
- `heca/src/app/input.rs`
- `heca/src/app/registry.rs`
- `heca/src/handlers.rs`
- `heca/src/rpc.rs`
- `heca/src/sidebar/model.rs`
- `heca/src/sidebar/tests.rs`
- `heca-config/src/keys.rs`
- `keybindings.toml`
- `README.md`

## Recent commits

- `68b9415` — `docs(sidebar): finish phase 1.5 docs and tests`
- `3f7d23f` — `docs: refresh handoff after merge sync`
- `c60baf3` — merge of PR #27 into `main`
- `da66b26` — `feat(sidebar): expose collapse actions via rpc`

## Next recommended step

Finish the workspace-wide validation pass, then commit and update PR #28 with the sidebar-nav configurability follow-up.
