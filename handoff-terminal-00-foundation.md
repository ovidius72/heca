# Handoff — Terminal `terminal-00` Foundation And Review Cleanup

Last updated: 2026-06-24
Worktree: `/Users/antonio/projects/myvim-terminal-followups`
Branch: `feature/terminal-followups`
HEAD at handoff: `1a2c684`
Commit state: no commit made for this pass; user explicitly wants review before any commit

## 1. What this pass did

This pass advanced the terminal damage-preservation prerequisite, not the full
dirty-row renderer itself.

Implemented:
- `terminal-task-00` — preserve terminal damage through the app path
- `terminal-task-00a` — produce visible row-range damage from the backend
- partial `terminal-task-00b` — retained terminal-content foundation
- resumed `terminal-task-00b` — removed the damaged-frame presentation guard after
  fixing the retained scratch sizing bug
- runtime verification: resize now looks good again on the restored retained path
- planning follow-up: tracked a new host scrollback phase because shell-history
  scrolling/navigation is still missing
- review-fix cleanup for the user's reported Rust hygiene findings

Did **not** complete:
- `terminal-task-00b` end-to-end runtime behavior
- `terminal-task-00c` full prerequisite verification
- `terminal-task-01` dirty-row rendering as the default live path

Current nuance:
- the first retained-layer presentation attempt caused a real runtime regression:
  - letters became oversized while resizing
  - freshly typed text could become temporarily invisible
- resumed analysis found a concrete cause: the shared offscreen scratch texture
  was reused at "at least" the requested size, so smaller pane updates rendered
  into an oversized target and then copied only the top-left sub-rect into the
  retained layer
- the scratch now matches each pane's exact physical size and live
  damaged-frame retained presentation is re-enabled
- runtime confirmation is still required before marking `terminal-task-00b` done
- runtime validation also exposed a separate terminal UX gap:
  - wheel does not scroll shell history
  - PageUp/PageDown do not scroll shell history
  - selection mode cannot scroll past the currently visible rows
  This is not just a missing wheel binding; the host has no terminal viewport /
  scrollback model yet

## 2. Non-negotiable workflow and coding rules

The next agent must behave like a strong Rust developer and follow the repo/user
rules exactly.

Read first, in this order:
1. `AGENTS.md`
2. `BACKLOG.md`
3. `terminal-implementation.md`
4. this handoff

Work rules:
- use the Rust skill / Rust best-practice mindset
- use `apply_patch` for manual edits
- prefer `rg`/`sed`/`git diff` for inspection
- do **not** use destructive git commands
- do **not** run `cargo fmt` over the dirty tree
- do **not** hardcode colors/styles or bypass registries if you touch UI/input
- keep terminal-specific retained-content work scoped to terminal panes; do **not**
  silently broaden it into general compositor optimization
- when touching scrollback/navigation, keep the work scoped to a terminal host
  viewport model; do **not** mutate unrelated compositor/layout scrolling code
- do **not** "fix blur" ad hoc in `render.rs`; blur/compositor work is still tracked
  separately in `BACKLOG.md` as `app-07`

User workflow rules:
- each new phase normally starts by syncing `origin/main`
- however, this worktree is already dirty, so **do not auto-merge `origin/main`
  into this worktree blindly**
- if resuming this exact in-flight work, preserve the current diff first and let
  the user decide whether to continue here, restack in a fresh worktree, or split
  the dirty tree
- no commit until the user reviews and explicitly asks for a commit

## 3. Current git/worktree state

Important:
- the live terminal work is in `/Users/antonio/projects/myvim-terminal-followups`
- the main checkout `/Users/antonio/projects/myvim` is **not** the active worktree
  for this task
- a previous `/tmp` worktree mistake already happened and must not be repeated

Current state:
- branch: `feature/terminal-followups`
- upstream tracking currently shows `origin/feature/terminal-followups [gone]` in
  `git status`; do not assume the remote branch still exists
- worktree is currently scoped to the terminal follow-up changes from this phase
- do not touch the main checkout `/Users/antonio/projects/myvim` for this task

Practical consequence:
- if the next agent wants to continue this exact work, they should inspect the
  current diff carefully and keep changes scoped to the terminal task and its
  review fixes
- if the user asks to start a fresh phase from synced `origin/main`, that likely
  needs a fresh clean worktree rather than merging into this dirty one

## 4. Terminal plan state after this pass

Honest status:
- `terminal-task-00`: done
- `terminal-task-00a`: done
- `terminal-task-00b`: in progress
- `terminal-task-00c`: in progress
- `terminal-task-01`: not started
- `terminal-01a` (new host scrollback viewport phase): not started

Why `00b` is still in progress:
- retained terminal layer/cache exists
- row-band copy/update logic exists
- renderer can consume `TerminalDamage::Rows(...)`
- the retained layer is presented again on damaged frames after the scratch-size
  fix
- resize looks good again in user runtime validation
- the remaining runtime sign-off gap is broader terminal interaction coverage

That means:
- the retained prerequisite is structurally present
- unchanged-row presentation during damaged frames is the live path again
- the task stays open until manual/runtime verification signs off on it
- the next real product gap is scrollback/navigation, now tracked explicitly as
  a separate phase instead of being left as an untracked caveat

## 5. Code changes from this pass

### 5.1 `terminal-task-00` — preserve damage through the app path

Main effect:
- terminal mount/render preparation now carries `TerminalDamage` alongside
  `TerminalSnapshot` instead of draining damage too early

Primary files:
- `heca/src/app/terminal_host.rs`
- `heca/src/app/terminal_render.rs`

### 5.2 `terminal-task-00a` — backend row-range damage

Main effect:
- backend damage is no longer only `dirty: bool -> Full|None`
- ordinary output/cursor changes can now become `TerminalDamage::Rows(...)`

Primary files:
- `heca-core/src/backend/terminal.rs`
- `heca-core/src/backend/terminal/engine.rs`

Relevant symbols to inspect first:
- `changed_visible_rows_since`
- `take_terminal_damage`
- `coalesce_terminal_damage_rows`

### 5.3 `terminal-task-00b` — retained terminal-content foundation

Main effect:
- app now has a terminal-specific retained-content foundation instead of assuming
  the frame can be fully cleared and partially redrawn without consequences

Primary files:
- `heca/src/app_state.rs`
- `heca/src/app/startup.rs`
- `heca/src/app/events.rs`
- `heca/src/app/render.rs`
- `heca/src/app/terminal_render.rs`
- `heca-renderer/src/terminal.rs`
- `heca/src/main.rs`

What exists now:
- per-pane retained terminal layer/cache state
- terminal-specific row-band copy/update logic
- renderer entrypoint that can render full snapshots or only damaged rows
- cache reset on scale-factor change and config reload

Current retained-path behavior:
- `heca/src/app/render.rs` now blits the retained layer on damaged frames again
- `heca/src/app_state.rs` exact-sizes the shared scratch texture per pane update
  so retained copies no longer crop/scale smaller panes incorrectly

### 5.4 Newly confirmed runtime gap — host terminal scrollback/navigation

Observed live behavior after the resize fix:
- pane resizing now looks good
- shell-history scrolling still does not work
- this is true for:
  - mouse wheel
  - PageUp / PageDown
  - selection mode when the caret/selection reaches the viewport edge

What the code does today:
- wheel is forwarded through `forward_mouse_wheel(...)` in
  `heca/src/app/terminal_host.rs` as `BackendMouseButton::Wheel*`
- PageUp / PageDown are forwarded through structured terminal key input
  (`BackendKeyCode::PageUp` / `PageDown`) via `heca/src/app/keyboard.rs` and
  `heca-core/src/backend/terminal/engine.rs`
- keyboard selection movement in `move_focused_terminal_selection(...)`
  (`heca/src/app/terminal_host.rs`) clamps to `terminal_snapshot().rows` /
  `cols`, so it cannot move beyond the currently visible viewport

Why that is insufficient:
- a normal shell prompt usually does not consume wheel/PageUp/PageDown as host
  scrollback commands in the way a GUI terminal emulator does
- the host always projects the live bottom viewport from `wezterm-term`; there is
  no host-managed scrollback offset to render history rows above the live viewport
- therefore all three symptoms are the same architectural gap: missing host
  viewport state

Tracked plan update:
- `BACKLOG.md` now includes a new phase `terminal-01a`:
  - `terminal-task-01a` add host-managed terminal viewport state + snapshot projection
  - `terminal-task-01b` route wheel / PageUp / PageDown / selection-edge movement through the host scrollback policy

### 5.5 Review-fix cleanup

The user later supplied a code review of uncommitted changes. This pass fixed the
real findings and rejected stale ones when the live tree showed they were no
longer valid.

Primary review-fix files:
- `heca-config/src/loader.rs`
- `heca-core/src/backend/terminal/engine.rs`
- `heca-core/src/layout/session.rs`
- `heca-renderer/src/backdrop.rs`
- `heca/src/actions.rs`
- `heca/src/app/interaction.rs`
- `heca/src/app/selection_model.rs`
- `heca/src/chrome/mod.rs`
- `heca/src/chrome/state.rs`
- `heca/src/input.rs`
- `heca/src/keymap.rs`
- `heca/src/mouse/target.rs`
- `heca/src/rpc.rs`
- `heca/src/sidebar/render.rs`

## 6. Regression found during this pass

User-reported runtime regression after the first retained-layer attempt:
- big letters while resizing
- typed text hidden / not visible immediately

What this implies technically:
- the retained-content foundation is real
- the earlier bug was in retained presentation geometry/scale state, not in the
  existence of row damage itself
- next work should validate the restored path first, not reopen the backend
  damage model unless new evidence appears
- the terminal runtime UX still lacks a scrollback viewport model; input
  forwarding alone is not enough

## 7. Validation already run

Validated in this pass:
- `cargo check -p heca`
- `cargo test -p heca app::terminal_render::tests -- --nocapture`
- `cargo test -p heca-core terminal_backend_output_produces_row_damage -- --nocapture`
- `cargo test -p heca app::interaction::tests -- --nocapture`
- `cargo test -p heca app::selection_model::tests -- --nocapture`
- user runtime validation:
  - retained resize regression appears fixed
  - shell scrollback/navigation still missing

Also validated earlier in the same terminal-foundation workstream:
- targeted terminal-render tests in `heca`
- targeted backend damage tests in `heca-core`

Important nuance:
- current validation proves the retained-path fix compiles and the focused
  helper/backend tests still pass
- it does **not** replace runtime verification of the previously failing resize
  + typing scenarios
- it also does not mean wheel/key forwarding is "good enough": without host
  viewport state, scrollback behavior remains absent even if events are forwarded

## 8. Exact next task for the next agent

Do this next:
1. preserve the current retained-path diff; do **not** back out the resize fix
2. start `terminal-01a` (new host scrollback viewport phase)
3. keep `terminal-task-00c` coverage work aligned with that viewport work
4. only after viewport/navigation is real, return to broader dirty-region work

Concrete next engineering target:
- introduce host-managed terminal viewport/scrollback state
- let the host render historical rows instead of always rendering the live bottom viewport
- define one policy boundary for:
  - shell/history scrollback
  - mouse-enabled TUIs that should still receive raw wheel events
  - selection mode moving beyond visible rows

Likely implementation path:
1. extend terminal backend/engine wrappers so a host viewport offset can be stored and changed
2. expose that viewport in the terminal snapshot/render contract
3. make viewport motion conservatively emit `TerminalDamage::Full` at first
4. route wheel / PageUp / PageDown through host viewport movement when appropriate
5. teach selection-mode movement to scroll the viewport when the caret/focus hits the top/bottom visible row
6. only after that, tighten damage semantics for viewport movement if needed

Do **not** do this:
- do not mark `terminal-task-00b` done just because the cache exists
- do not jump straight to `terminal-task-01` dirty-row optimization
- do not treat this as "just add a wheel binding"
- do not broaden the task into general compositor damage optimization
- do not patch symptoms with ad hoc style/render hacks in unrelated layers

## 9. Files the next agent should inspect first

Planning/docs:
- `AGENTS.md`
- `BACKLOG.md`
- `terminal-implementation.md`
- `handoff-terminal-00-foundation.md`

Implementation:
- `heca/src/app/render.rs`
- `heca/src/app/terminal_render.rs`
- `heca/src/app/terminal_host.rs`
- `heca/src/app_state.rs`
- `heca/src/app/events.rs`
- `heca/src/app/keyboard.rs`
- `heca/src/handlers.rs`
- `heca/src/app/startup.rs`
- `heca/src/main.rs`
- `heca-renderer/src/terminal.rs`
- `heca-core/src/backend/terminal.rs`
- `heca-core/src/backend/terminal/engine.rs`

## 10. Resume protocol for the next agent

When resuming:
- confirm you are in `/Users/antonio/projects/myvim-terminal-followups`
- read the docs above before editing
- inspect `git status` and `git diff --stat` before assuming scope
- keep the next patch focused on `terminal-01a` plus any directly related `00c`
  coverage needed to support it
- rerun focused validation after each meaningful step
- stop before commit and hand back to the user for review
