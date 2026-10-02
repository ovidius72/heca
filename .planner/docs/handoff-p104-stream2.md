# Stream 2 handoff — P104 (server/client split) and the state of everything stream 2 had open

Written 2026-10-02 by the stream-2 session (session name `stream2`) at ~72% context. A fresh session
continues. The orchestrator is `heca-pro-commercial-agents` (reply to it with SendMessage); stream 1 is
`heca-82`. Stream 2 works in the git worktree `~/projects/heca-f012` (own `target/`); Antonio checks
only from `~/projects/heca-check`, never from the working folder. The shared planner is
`~/projects/heca-planner/.planner` (worktree on branch `planner`): change it ONLY through the planner
tools after `/mcp` reconnects agent-plan for the session; never commit anything under `.planner/`/
`.planning/` from the worktree.

## How the orchestrator wants work done (read before touching code)

- Read AGENTS.md **in full** (1879 lines), then `docs/layout.md`, `docs/widgets.md`, CLAUDE.md. Name the
  sections that cover what you touched in every report.
- Flow per task: plan to the orchestrator first, build, report (what changed, files, tests, clippy,
  plugin-author lines for every public API added/changed, old code touched and refactored), the
  orchestrator reviews, Antonio checks by eye from heca-check, THEN rebase on origin/main, re-test,
  commit (code and docs only), push, PR, complete the planner task, send the PR number.
- Never commit/push unasked; the orchestrator's word carries Antonio's authority for order/commits/PRs.
- Standing rules (the orchestrator repeated them): nothing hard-coded (no numbers in UI code, no named
  look constants: `heca/tests/no_literal_sizes.rs` now fails on them); fix at the source, never at the
  call site; one way to do a thing (never a second path); a plugin author must be able to write one
  line (`Label::new("x").size(WidgetSize::Caption)`), no ids/registries/host-private types; when you meet
  OLD code that breaks the rules next to your change, refactor it in the same work; every file under
  ~400 lines (tests that would push a file past it go in `<file>/tests.rs`); a test per behaviour,
  break-checked (remove the fix, see it red, restore); `./scripts/lint-changed.sh` (clippy -D warnings)
  before every commit; do NOT run `cargo fmt` on the tree (AGENTS.md Gotchas); load
  `~/.agents/skills/rust/SKILL.md` and review against it before every commit; scope cargo to crates
  (`-p`), one cargo run at a time; never `git checkout --` a modified file; plain short English with
  Antonio, answer first.
- Stay out of stream 1's files until told: `app/backend_store.rs`, `app/terminal_process.rs`,
  `app/terminal_host/`, `chrome/terminal/`, `chrome/pane_header.rs`, column, `app/render.rs`, grid-ui
  Scene/PaintCx/Pane widget. 2b (terminals) waits for the orchestrator's "go" (stream 1's refactor PR).

## Branches and PRs (all in ~/projects/heca-f012)

| branch | state |
|---|---|
| `fix/no-literal-sizes-in-providers` | **PR #277 open** (T537): vocabulary + app constants -> theme/config + stopper test + audit refactors + Caption fix. Antonio checked it by eye at d59c5833 (OK). Merge is the orchestrator's/Antonio's. |
| `refactor/config-loader-split` | **PR #276 open**: heca-config `loader/{mod,project,sources,tests}.rs`, paths unchanged. |
| `feat/p104-server-state-2` | P104 step 1, commit 18415dbc, local only (parent: origin/main d3242597). `ServerState { backends, notifications, git_runtime_cache, programs }`, `AppState.server`. |
| `feat/p104-notifications-server` | P104 2a + 2a-2 + 2c, local only (HEAD **10dd94bf**): f4f457c3 (ServerAction/Change/execute/tick/next_wake/toast_action; runtime private; `AppState::ask_server`/`apply`) and a1fa26e8 (store owns no signal; window owns `AppState::toasts`), 10dd94bf (2c: git cache private, `ServerState::refresh_git(session, now)`). **Continue from this branch.** |
| `feat/p104-server-state` | stale first attempt, ignore (can be deleted). |
| merged already | #271 T532 (one visibility state), #273 trust notice every start (planner: T533, re-filed T535 after a rollback), #275 T508 (notification text wraps, max_lines). |

Not pushed: both P104 branches (the orchestrator decides when; they conflict with #277 in app_state.rs/
startup.rs hunks: rebase and resolve, keep both sides).

## P104 status (the task is P104(F012)/T525; its description holds the field map and agreed steps)

Done: step 1 (ServerState carve-out incl. backends, notifications, git cache, programs; terminals stays
on the client, session untouched) and step 2a/2a-2 (notifications through a plain-data
`ServerAction` -> `ServerState::execute(action, now) -> Vec<Change>`, `tick`, `next_wake`,
`visible_toasts`, `toast_action`; window glue `ask_server`/`apply`/`sync_toasts`; windowless tests
break-checked). Files: `heca/src/server/{mod.rs 224, action.rs 43, change.rs 12}`.

Approved design (orchestrator, 2026-10-02): `ServerAction` is plain data built from the catalog names
(no second vocabulary); `Change`s are drained by the client and say what happened, never what to do;
reads go through `Server`; `SharedChromeState` stays the client's mirror fed by Changes; each action
gets a declared SIDE (server | client) as a descriptor field with NO default (like `args`), the dispatch
chokepoint routes by it; a test drives the server with no window; plugin-author lines must not change
(`pro.action(..).run(..)` already runs against the store, not the window; layers/docks/pane items stay
client side and read `PaneFacts`).

Remaining slices, each its own PR, behaviour-neutral:
- **2c facts + git — DONE as far as the session allows (10dd94bf)**: the git cache is private in
  `ServerState` and `refresh_git(session, now)` is the one way in. **LIMIT: the facts move with step 3.**
  The git sync writes into each pane's runtime, which lives in `Session`, so `PaneFacts` production,
  `Server::facts(pane)` and `Change::FactsChanged{pane}` cannot move behind the server until the
  session is split (step 3); until then `refresh_git` takes the session by `&mut`.
  `chrome/pane_items/*` keep reading plain `PaneFacts`.
- **2b terminals** — WAIT for the orchestrator's go. Move `state.server.backends` readers
  (`ensure/kill/input/resize/snapshot`) to `Server` calls; readers move to `TerminalId` as agreed with
  stream 1 (no separate sweep). `terminals` (the chrome Terminal components) stay client state.
- **2d action SIDE** descriptor field + routing at `dispatch_intent`. Only 4 handler files touch window
  fields directly, so most of the 140 handlers sort by reading.
- **Step 3 Session view split** — a DESIGN DECISION: `Session` (heca-core `layout/session.rs`) mixes
  shared layout (workspaces, columns, panes, options) with per-window view (`active_workspace_idx`,
  `workspace_switch` animation, `viewport_size`, `scale`, and the view offset inside each workspace's
  `ScrollingSpace`). Write the OPTIONS to the orchestrator (it takes the decision to Antonio); do not
  guess. F012's accepted decision: the view is per window.
- Mixed items still to decide once the interface exists: `SharedChromeState` (sidebar content mirror =
  server-sent; region size/visible/focus/cursors = client), `action_catalog`, `search_store`/history,
  `confirm` config. Notification projection: `ToastSpec` (plain data) is still the list element type;
  optional further step: store hands out `AppNotification` and the window builds specs.

## Open items to file as planner tasks (once tools are reconnected)

1. **Complete P045(F006)/T537** (No literal sizes in app UI code; PR #277). Evidence: tests for heca,
   grid-ui, config, theme, view, view-realize; clippy -D warnings; break-checks; Antonio's eye check at
   d59c5833; reviewed by the orchestrator.
2. **Built-ins' confirm resolved from the catalog**: today `handlers/confirm.rs::confirm_owner_name`
   maps a raw `WmAction` to its confirm owner (single mapping, test `gate_covers` derives from it).
   Full fix: every `WmAction` carries its catalog name (reverse of `action_from_name`) so built-ins
   resolve like named actions and the mapping goes.
3. **Drag preview composed from widgets** (`heca/src/mouse/render.rs`, 286 lines of raw-renderer
   drawing: literal colours, 0.6 advance, 24px strip, 0.2/0.6 shares, inline column/pane geometry;
   called from `app/render.rs:903-904`): three widgets (ghost, insert marker, swap highlight) placed in
   the window root, geometry moved to layout (core), theme colours; delete the two render.rs lines.
   Timing: its own PR right AFTER stream 1's `render.rs` flush rewrite (2b slice 3) merges and before
   P104 step 2.
4. Optional: make the tooltip delay theme-driven (needs the layout pass to stamp it; today
   `QUICK_DELAY = DEFAULT_DELAY/2` is a library const) — only if the orchestrator wants it.
5. Old code I listed and did NOT fix: library widget internals (tooltip bubble GAP/PAD/glow consts,
   toast `DEFAULT_WIDTH` 320 / `GLOW_RADIUS` 14 / `ICON_SCALE`), `MAX_UNITS` (kept, in NOT_A_LOOK).

Planner housekeeping when reconnected: check P104/T525's description (field map, agreed steps) is
present; `T537` shows done; the T533/T535 pair: the completed one is the trust-notice task. Start T525
properly (`planner-task-start P104(F012)/T525`) when taking 2c (read phase + feature first).

## What changed in the code since the T525 description was written (facts for the next session)

- `heca/src/server/` exists; `AppState.server`; the notification runtime's field is private in
  `ServerState`.
- `AppState::ask_server(ServerAction)` stamps the server clock; `AppState::apply(changes)` does
  `sync_toasts()` + `needs_redraw`. Startup builds `ServerState::new(backends, notification_runtime,
  programs)`.
- `chrome/startup_queue.rs::StartupQueue<T>` is the one "declared before start, taken once, refused
  after" queue (pane items, entry/action, entry/layer, regions); `pane_items/listing.rs::add_unique`.
- `WidgetSize::Caption` (0.72x) is the supporting-text step; `tooltip_quick(true)` is the quick tooltip.
- Config/theme additions are in `config.default.toml`/README (top_bar_height, bottom_bar_height,
  drag_label_*, new_column_slot_share, bell_flash_alpha, search_match_alpha,
  search_current_match_alpha, edge_scroll_distance; theme active_glow_radius/strength).

## Things that went wrong and how not to repeat them

- A rebase/checkout in `~/projects/heca` rolled the shared planner back once (T533 lost, re-filed T535).
  The planner now lives in its own worktree; do not touch it by hand.
- Antonio ran `cargo run` in my working folder mid-edit and saw compile errors: he now checks only from
  `~/projects/heca-check`.
- I once said "within a pixel" about a size change without measuring: measure with a test (lay out,
  compare `base().font`) before claiming a look is unchanged.

## Step 3 status (2026-10-02)

Options sent to `master` (orchestrator, Antonio's authority): A = view on the client (`WindowView {
active_ws, switch, viewport, scale, offsets }`, engine takes it by reference, server remembers each
window's last view by window id) — RECOMMENDED; B = per-window clone of the layout fed by deltas (a
trap: two copies); C = view on the server keyed by window id (per-frame work + animation clocks on the
server). `master` takes the decision to Antonio and will send his answer to whoever resumes. Under A do
it in two PRs: (1) extract the view fields out of `Session`/`ScrollingSpace` with all layout tests
green, standalone behaviour identical; (2) the server/client interface for layout ops
(`ServerAction::{Split,Close,Move,..}` named from the catalog). The facts (2c limit above) and
`SharedChromeState` mirror follow it.

**DECIDED (2026-10-02): Antonio chose option A** (view on the client; the server remembers each window's
last view by window id). Recorded on F012 as accepted decision **2c249a29**. Whoever resumes starts with
**PR 1: move the view out of `Session`/`ScrollingSpace` into `WindowView`, standalone behaviour
identical (all layout tests stay green)**, then PR 2 (the layout-op interface). 2b stays parked until
stream 1's refactor (A) and 5a merge.
