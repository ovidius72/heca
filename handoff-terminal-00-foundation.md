# Handoff — Terminal `terminal-00` Foundation And Review Cleanup

Last updated: 2026-06-23
Worktree: `/Users/antonio/projects/myvim-terminal-followups`
Branch: `feature/terminal-followups`
HEAD at handoff: `7b7d123`
Commit state: no commit made for this pass; user explicitly wants review before any commit

## 1. What this pass did

This pass advanced the terminal damage-preservation prerequisite, not the full
dirty-row renderer itself.

Implemented:
- `terminal-task-00` — preserve terminal damage through the app path
- `terminal-task-00a` — produce visible row-range damage from the backend
- partial `terminal-task-00b` — retained terminal-content foundation
- review-fix cleanup for the user's reported Rust hygiene findings

Did **not** complete:
- `terminal-task-00b` end-to-end runtime behavior
- `terminal-task-00c` full prerequisite verification
- `terminal-task-01` dirty-row rendering as the default live path

Reason:
- the first retained-layer presentation attempt caused a real runtime regression:
  - letters became oversized while resizing
  - freshly typed text could become temporarily invisible
- the regression was mitigated safely, but that mitigation means retained
  damaged-row presentation is **not** fully live yet

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
- worktree is heavily dirty across many files, not just terminal ones
- some of that spread came from earlier broad formatting/churn in the worktree
- do not assume `git status` is a clean task slice

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

Why `00b` is still in progress:
- retained terminal layer/cache exists
- row-band copy/update logic exists
- renderer can consume `TerminalDamage::Rows(...)`
- but the retained layer is only **presented** on clean frames right now
- damaged frames still use the direct render path while updating the retained
  cache in the background

That means:
- the retained prerequisite is structurally present
- the unsafe regression was stopped
- but unchanged-row presentation during damaged frames is not yet the live path

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

Critical safety guard now in place:
- `heca/src/app/render.rs` only presents the retained layer when
  `mount.damage.is_empty()`
- when there is damage, the live frame falls back to the old direct render path
  while still updating the retained cache

### 5.4 Review-fix cleanup

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

Current mitigation:
- present retained layer only on clean frames
- damaged frames direct-render and also refresh the retained cache

What this implies technically:
- the retained-content foundation is real
- the bug is in the live damaged-frame presentation path, not in the existence of
  row damage itself
- the next agent should debug retained-layer presentation/geometry/scale behavior,
  not reopen the backend damage model first

## 7. Validation already run

Validated in this pass:
- `cargo check -p heca`
- `cargo test -p heca app::interaction::tests -- --nocapture`
- `cargo test -p heca app::selection_model::tests -- --nocapture`

Also validated earlier in the same terminal-foundation workstream:
- targeted terminal-render tests in `heca`
- targeted backend damage tests in `heca-core`

Important nuance:
- current validation proves the review-fix pass is clean and the code compiles
- it does **not** prove the retained damaged-row presentation path is ready for
  general runtime use, because that path is still guarded off on damaged frames

## 8. Exact next task for the next agent

Do this next:
1. finish `terminal-task-00b` honestly
2. add/finish `terminal-task-00c` coverage
3. only after that, move to `terminal-task-01`

Concrete next debugging target:
- inspect the retained damaged-frame presentation path in:
  - `heca/src/app/render.rs`
  - `heca/src/app/terminal_render.rs`
  - `heca-renderer/src/terminal.rs`
- determine why presenting the retained layer during damage caused:
  - resize-time oversized glyphs
  - temporarily invisible typed text

Do **not** do this:
- do not mark `terminal-task-00b` done just because the cache exists
- do not jump straight to `terminal-task-01`
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
- keep the next patch focused on closing `00b`/`00c`
- rerun focused validation after each meaningful step
- stop before commit and hand back to the user for review
