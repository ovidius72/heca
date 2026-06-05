# Session Resume Handoff — 2026-06-05

## Purpose

This file is the current resume note for the active refactor and architecture-doc update.

Primary status now:
- **Phase 1.3 (`heca/src/sidebar.rs`) is structurally complete enough**
- the repo also contains new **architecture/planning docs** for the future pluggable chrome / WASM plugin direction

Non-negotiable workflow rule from the user:
- **pull / merge `origin/main` before starting each new task**

Important deferred item:
- floating-vs-tiled focus-domain routing is still **intentionally deferred** to the **last phase** of `bugs-and-refactoring-plan.md`

---

## Current branch state

Latest already-committed milestones relevant to this refactor:
- `577033a` — `Add column rename support and continue app refactor`
- `a13439a` — `Restore workspace key aliases and continue app refactor`
- `ad1acaa` — `Finish main runtime split into app modules`
- `85aaed6` — `Fix keybinding conflicts and prefix handling`
- `2e2cdeb` — `Split mouse logic into focused submodules`
- `212a20f` — `Finish splitting mouse interactions into focused modules`
- `660c0f7` — `Start splitting sidebar model and hit testing`

Current uncommitted / untracked work now consists of:

### Code refactor completion for Phase 1.3
- modified: `heca/src/sidebar.rs`
- new: `heca/src/sidebar/render.rs`
- new: `heca/src/sidebar/tests.rs`

### Planning / architecture docs
- modified: `AGENTS.md`
- modified: `README.md`
- modified: `bugs-and-refactoring-plan.md`
- modified: `bugs-and-refactoring.md`
- new: `pluggable-chrome-plugin-plan.md`
- new: `sidebar-gap.md`

---

## What is already done

### 1. Phase 1.1 is complete enough

`heca/src/main.rs` was reduced to a thin module root / binary shell.

Work extracted into `heca/src/app/`:
- `registry.rs`
- `focus.rs`
- `mutations.rs`
- `render.rs`
- `selection.rs`
- `keyboard.rs`
- `startup.rs`
- `input.rs`
- `events.rs`
- `lifecycle.rs`

Why this matters:
- runtime ownership is much clearer
- `main.rs` is no longer the place where every system detail accumulates
- future behavior changes can happen in narrower modules with less review risk

### 2. Keybinding and prefix regressions from review were fixed

Already done:
- `prefix+p` is command palette again
- pane cycling moved to `prefix+[` / `prefix+]`
- `prefix+Ctrl+k/j` swap bindings restored
- `prefix+Ctrl+p/n` workspace aliases restored
- configurable double-prefix forwarding fixed
- built-in and user custom modes now merge by mode name
- startup/reload conflict logging made deterministic and visible
- quick-select / swap / take overflow now falls back instead of partially labeling panes

Why this matters:
- keyboard behavior is deterministic again
- config reload behavior is safer and easier to debug
- future input/config work has a cleaner base

### 3. Rename and zoom work is already in place

Already done:
- `prefix+z` toggles active-column zoom
- zoom is per-column and independent
- pane rename is `prefix+$`
- workspace rename is `prefix+Shift+w`
- column rename is `prefix+Shift+c`
- column names render in the sidebar
- RPC support exists for zoom and column rename

Why this matters:
- the product-level behavior the user asked for is already landed
- current refactor work should preserve these paths, not redesign them

### 4. Phase 1.2 is structurally complete

Mouse logic is already split into:
- `heca/src/mouse/hit_test.rs`
- `heca/src/mouse/render.rs`
- `heca/src/mouse/drag.rs`
- `heca/src/mouse/drop.rs`
- `heca/src/mouse/sidebar.rs`
- `heca/src/mouse/sidebar_drop.rs`
- `heca/src/mouse/tests.rs`

Why this matters:
- the top-level mouse module is thin enough to read quickly
- each major mouse concern has a clearer home
- no mouse submodule exceeds the target ~400 LOC threshold

### 5. Phase 1.3 is now structurally complete enough

Sidebar logic is now split into:
- `heca/src/sidebar.rs` — thin façade / re-exports
- `heca/src/sidebar/model.rs` — tree/projection/navigation/collapse state
- `heca/src/sidebar/hit_test.rs` — hit testing
- `heca/src/sidebar/render.rs` — expanded/collapsed rendering + render helpers
- `heca/src/sidebar/tests.rs` — sidebar tests

What was completed in this slice:
- rendering extracted from `sidebar.rs`
- render internals split into smaller helpers
- tests moved out of `sidebar.rs`
- `bugs-and-refactoring-plan.md` live checklist updated through the end of the current sidebar split

Why this matters:
- the last major mixed-responsibility UI file in the current structure-first wave is now much thinner
- current workspace-tree behavior is easier to reinterpret later as a built-in `WorkspacesContainer`
- future chrome-host work will not need to start from a monolithic `sidebar.rs`

### 6. Architecture docs were updated to match the new direction

Already documented:
- sidebar shell vs mounted container separation
- current workspace tree should evolve into built-in `WorkspacesContainer`
- future pluggable chrome host with left/right/top/bottom regions
- dynamic action evolution
- future WASM/plugin direction
- important action reachability rule: features should be reachable from mouse/UI, keyboard/action dispatch, and RPC when meaningful on those surfaces
- container movement across compatible regions should be host-managed and action-addressable

Files carrying this new direction:
- `pluggable-chrome-plugin-plan.md`
- `sidebar-gap.md`
- `AGENTS.md`
- `README.md`
- `bugs-and-refactoring-plan.md`
- `bugs-and-refactoring.md`

---

## What is not done yet

### 1. Parameterized normal keybindings are still future work

Still not implemented:
- `[[keys.bind]]` for parameterized normal bindings
- generalized structured spawn action / size parsing

Why not now:
- the agreed order was structure-first
- the `main.rs` / `mouse.rs` / `sidebar.rs` cleanup was the immediate priority

### 2. Floating focus-domain routing is still deferred

Still not fixed:
- handlers that should act on the focused floating pane still often mutate tiled `ws.scrolling...` state instead

Why deferred:
- the user explicitly asked for this to be the **last phase** in the plan
- it is a semantic correctness project, not a structural refactor task

### 3. Pluggable chrome / provider / WASM runtime work has not started

Important:
- the architecture is documented
- the implementation has **not** started
- current code is still the existing app architecture, now with better structural seams

---

## What we are done with, and why

We are effectively **done with Phase 1.1**.

Reason:
- `heca/src/main.rs` already hit the intended outcome: thin entrypoint, delegated runtime systems, better navigability

We are **done with Phase 1.2’s structural split**.

Reason:
- mouse concerns now have distinct files and the top-level module is thin

We are now **done enough with Phase 1.3’s structural split**.

Reason:
- `sidebar.rs` is now a thin façade
- rendering, hit-testing, model, and tests are separated
- validation passed after the split

So the current strategy should be:
1. do **not** reopen completed `main.rs` / `mouse.rs` / `sidebar.rs` structure work unless needed
2. commit the current sidebar/doc/architecture batch cleanly
3. then choose the next planned refactor slice deliberately

---

## Validation status

The current sidebar split and related refactor slices passed:
- `cargo check -q`
- `cargo clippy --workspace --all-targets --all-features --quiet`
- `cargo test -q --workspace`

Recommended validation before any new behavior work:

```bash
cargo check -q
cargo clippy --workspace --all-targets --all-features --quiet
cargo test -q --workspace
```

Optional smoke test after commit:

```bash
cargo run -p heca --quiet
```

Expected smoke-test behavior:
- app starts and stays in event loop
- no startup error before timeout
- prefix mode still works
- rename bindings still work
- zoom still works
- sidebar interactions still work

---

## Exact next step

### Immediate practical next move

Create clean commits for the current work.

### Recommended commit plan

#### Commit A — Finish sidebar structural split
Include:
- `heca/src/sidebar.rs`
- `heca/src/sidebar/render.rs`
- `heca/src/sidebar/tests.rs`
- `bugs-and-refactoring-plan.md`

Suggested message:
- `Finish splitting sidebar into focused modules`

#### Commit B — Document architecture shift
Include:
- `AGENTS.md`
- `README.md`
- `bugs-and-refactoring.md`
- `pluggable-chrome-plugin-plan.md`
- `sidebar-gap.md`

Suggested message:
- `Document pluggable chrome and container architecture`

(Optional)
If you want plan/history docs kept separate from architecture docs:

#### Commit C — Update refactor-plan context docs
Include:
- `bugs-and-refactoring-plan.md` if not already grouped with Commit A
- `session-resume-handoff.md`

Suggested message:
- `Update refactor handoff after sidebar split`

### After the commits

Re-evaluate the next code slice from the refactor roadmap.

Most likely candidates:
- continue with the next planned PR slice (`theme.rs` / config split)
- or jump to **Phase 2 — Introduce a Central Mutation Boundary** if that now gives the best readability payoff

Do **not** start:
- floating focus-domain routing
- plugin runtime implementation
- ChromeHost implementation

until the current batch is committed and the next slice is chosen explicitly.

---

## Exact resume procedure

### 1. Sync first

Always do this before starting a new task:

```bash
git fetch origin && git merge --ff-only origin/main
```

### 2. Confirm working tree

```bash
git status --short
```

### 3. Re-read these files first

- `bugs-and-refactoring-plan.md`
- `session-resume-handoff.md`
- `pluggable-chrome-plugin-plan.md`
- `sidebar-gap.md`
- `AGENTS.md`
- `README.md`
- `heca/src/sidebar.rs`
- `heca/src/sidebar/model.rs`
- `heca/src/sidebar/hit_test.rs`
- `heca/src/sidebar/render.rs`
- `heca/src/sidebar/tests.rs`

### 4. Re-run validation before changing behavior

```bash
cargo check -q
cargo clippy --workspace --all-targets --all-features --quiet
cargo test -q --workspace
```

### 5. Continue only the explicitly chosen next slice

Do **not** jump implicitly to:
- floating focus-domain routing fixes
- parameterized keybinding implementation
- plugin runtime implementation
- ChromeHost implementation

until the current sidebar/doc batch is committed and the next step is explicitly selected.

---

## Behavior constraints that must not regress

### Keybindings
Keep these exactly:
- `prefix+Ctrl+k` → `swap_up`
- `prefix+Ctrl+j` → `swap_down`
- `prefix+Ctrl+p` → workspace previous
- `prefix+Ctrl+n` → workspace next
- `prefix+Ctrl+Shift+k` → move column up
- `prefix+Ctrl+Shift+j` → move column down
- `prefix+p` → command palette
- `prefix+[` / `prefix+]` → prev / next pane

Relevant files:
- `heca-config/src/theme.rs`
- `keybindings.toml`
- `heca/src/app/registry.rs`

### Rename bindings
Keep these exactly:
- `prefix+$` → rename pane/tab
- `prefix+Shift+w` → rename workspace
- `prefix+Shift+c` → rename column

### Zoom behavior
Keep these exactly:
- `prefix+z` toggles zoom for the active column
- zoom is a toggle
- multiple columns may remain zoomed independently

### Float/unfloat behavior
Keep these exactly:
- `prefix+f` remains float/unfloat toggle
- manually floated tiled panes should restore to their original tiled location when possible
- panes spawned floating without an original tiled slot should unfloat into a new column

### Deferred bug policy
Do not silently “fix” the floating focus-domain routing bug inside unrelated refactor or doc work. Documented deferment is intentional.

---

## Short summary

We are **done with the `main.rs` runtime split**.

We are **done with the structural `mouse.rs` split**.

We are now **done enough with the structural `sidebar.rs` split**:
- model extracted
- hit testing extracted
- rendering extracted
- tests extracted
- `sidebar.rs` is thin

We also now have a documented future architecture direction:
- sidebar shell vs `WorkspacesContainer`
- pluggable chrome host
- dynamic actions
- host-managed container movement
- future WASM/plugin boundary

The next best move is to **commit the current sidebar split + architecture doc updates cleanly**, then explicitly choose the next refactor/code slice.
