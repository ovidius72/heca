# Session Resume Handoff — 2026-06-05

## Current state

- Branch: `feature/gpt-refactoring`
- HEAD: `f201534` — `refactor(app): extract shared same-column swap helper`
- `origin/main` is current at `245e725`
- `origin/feature/gpt-refactoring` is behind the local work-in-progress slice
- Working tree has Phase 3 pane-op extraction edits in progress
- PR #29 is merged into `main`

### Current Phase 3 progress

- Added `app::mutations` as the shared post-mutation hook module
- `after_layout_change(...)` centralizes the common post-mutation redraw/focus-sync path for layout mutations
- `after_metadata_change(...)` handles rename/title-only refreshes
- `after_mutation_change(state, MutationKind::Config)` is used for config-driven refreshes in the app event path
- Mouse drop / drag / input rename paths now flow through the shared hooks instead of hand-rolled sync/redraw tails
- Manual `sidebar_tree.rebuild(...)` calls are now limited to startup and the focus-sync path
- Added `app::pane_ops::swap_panes_same_column(...)` and delegated swap-up/down + same-column swap handling to it
- Added `app::pane_ops::remove_pane_by_id(...)` and `insert_pane_at_position(...)`; `heca/src/mouse/drop.rs` now uses them for detached-pane reinsertion
- Validation passed: `cargo check -p heca`, `cargo test -p heca`, `cargo clippy --workspace --all-targets --all-features`
- Phase 3.1 slice is in progress; latest commit is `f201534`

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
- Phase 1.5 complete marker added to the checklist

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

### Validation that passed
- `cargo test -p heca --quiet`
- `cargo test -p heca-config --quiet`
- `cargo check --workspace --quiet`
- `cargo clippy --workspace --all-targets --all-features --quiet`

## Still open

- PR #29 needs review/merge
- Phase 2 is next after this branch is merged or otherwise advanced

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

- `08eabab` — `feat(sidebar): make sidebar nav configurable`
- `68b9415` — `docs(sidebar): finish phase 1.5 docs and tests`
- `3f7d23f` — `docs: refresh handoff after merge sync`
- `c60baf3` — merge of PR #27 into `main`

## Next recommended step

Wait for PR #29 review/merge, then move to **Phase 2** unless redirected.
