# heca-dev2 — continue F012 (server/client → sessions)

You (heca-dev2, a Claude agent) are taking over the **F012** work in the `heca` repo: the
server/client model and the local-socket connection. heca-dev1 built the first slice; this is the
handoff with full context. **The planner is the single source of truth** — load it (`/planner load`)
and read the phase/task descriptions; they are now complete. Refer to work by ids only (F012, P099,
T543…).

## Where to work (don't recreate the worktree mess)

- The work is on branch **`feat/p099-connection`**. If you share the `~/projects/heca` folder with
  another agent, make your **own git worktree** so you never edit the same tree:
  `git worktree add ../heca-dev2 feat/p099-connection`. Otherwise work in `~/projects/heca` if it's
  yours alone and already on that branch.
- A task isn't "done" until its branch is merged **and** its worktree is removed. Don't leave
  worktrees or unmerged "done" branches behind.

### Another agent is live — Codex, on P102

**Codex (ChatGPT's CLI) is actively working in parallel**, so stay out of its lane:
- Codex owns **P102** (`~/projects/heca-pro/docs` decisions 13/19/20): the agent contract, agent
  status in the pane row, and sounds — in the **`heca-p102`** worktree (branch `feat/p102-agent`),
  reading its own brief `CODEX-P102.md`. Codex also maintains **`heca-pro`**.
- **Do not edit P102's files:** `heca-core/src/backend/terminal/osc.rs`, `heca-core/src/runtime.rs`
  (`PaneRuntime` agent state), `heca/src/app/process_monitor.rs`, and the sidebar pane row /
  `heca/src/providers/workspaces/row_lines.rs` (the agent line), plus any sound path. Don't touch
  the `heca-p102` or `heca-pro` worktrees.
- Your P099 lane (`heca/src/server/`, `rpc.rs`, `cli.rs`, `heca-core/src/layout/`,
  `heca-core/src/backend/snapshot.rs` for the wire snapshot) and Codex's P102 lane are mostly
  disjoint. The one place to watch is **`heca-core`** and the `Change` enum: if P102 surfaces agent
  status as a `Change`/event, coordinate by **not editing the same file** — refer to work by planner
  id, and if your change and Codex's would touch one file, raise it with Antonio rather than both
  editing it. Codex is **not** a Claude session, so you can't message it; coordination is through
  the repo, the planner, and Antonio.
- Codex does **not** touch the planner; when it finishes P102, Antonio has it reviewed and the
  planner is marked done on his word — not by you.

## The model — DECIDED, do not re-open

heca runs standalone or as a server. The server holds the **shared content** (workspaces, columns,
panes, terminals, agents); a **window is a client** with its own **view**; the server sends state +
changes, never interface. **Detach/attach like tmux** — closing a window leaves the session running.
**Standalone = server + one client in one process, one code path.** Several windows share content and
move independently; a running pane shown at different sizes uses a setting `smallest|latest|largest`
(default `latest`). Planner decisions: **d108b88c, 463ccd07, 2c249a29** (feature F012).

## Where P099 stands

- **DONE — T521:** local-only, owner-only Unix socket in a runtime folder (stale-socket cleanup) +
  one-shot `heca rpc <line>`. Files: `heca/src/server/socket.rs`; the `AppEvent::Rpc { line, reply }`
  seam (`heca/src/app/events.rs`, handled in `heca/src/lib.rs` `user_event`); `heca/src/app/cli.rs`
  dispatch. The RPC language already existed (`heca/src/rpc.rs`, via the `ActionCatalog`); the socket
  just feeds it. Unix only. Committed on `feat/p099-connection`.
- **NEXT — the chain (each task's description has the file map + approach + done-when):**
  - **T543** serializable wire model: plain-data `Change` (`heca/src/server/change.rs`) + layout
    effects (`heca-core/src/layout/scrolling/{pane_effect,effect}.rs`, `types.rs`) + a **new
    plain-data terminal snapshot** — heca-core has no serde, and `TerminalSnapshot`/`TerminalDamage`
    (`heca-core/src/backend/snapshot.rs`) hold wezterm cells + decoded images, so **never serialize
    those types**; convert to a plain wire form (text runs, colours, cursor, viewport; images by
    handle/hash, bytes separate). Add serde to heca-core. **Start here.**
  - **T544** headless server (no winit/GPU) + attach protocol over the socket; detach/reattach with
    scrollback. The in-process interface to carry is `AppState.ask_server(ServerAction)` → `Change`
    → `window_reacts` (`heca/src/app_state/reaction/mod.rs`); the server reads per-window facts from
    the `Asker` (`heca/src/server/cx.rs`), never from a window.
  - **T545** several windows + `follow` + the pane size setting · **T546** `heca rpc --events` ·
    **T547** Windows named pipe · **T548** docs (`docs/server-client-architecture.md`).

## How to work

- Read **AGENTS.md in full** — §0 especially (pre-flight per UI type; widget-vs-component;
  DOM-shaped events capture→target→bubble; **Rule Zero**: the line a plugin author would type, no
  registry/id/host-private type). Also `CLAUDE.md`, `docs/layout.md`, `docs/widgets.md`.
- **Orient with graphify before reading/grepping source:** `graphify query "<question>"` (a repo
  hook enforces this). `graphify update .` after code changes.
- **Fix library-side, never at the call site. Nothing hard-coded** (config / theme / layout
  options). Compose, don't invent (search `docs/widgets.md` first). SOLID; one file, one job (aim
  <400 lines, hard <800 — agree a split first).
- **Verify by running, not grepping.** Scoped cargo: `./scripts/lint-changed.sh` /
  `cargo check -p <crate>` while iterating; **one** `cargo test -p <crate>` per coherent slice; full
  workspace **only** before a PR, prefixed `HECA_FULL_WORKSPACE=1`. Don't run `cargo fmt`.
  Break-check new tests (put the bug back, see red). Keep the tree compiling at every step.
- **Planner is part of the project — commit `.planner` with your work, always.** Keep the task's
  checklist/status honest. Don't create / rename / rescope / re-prioritise tasks without Antonio's
  OK; proposing is fine.
- **Commit at the END of a task, only on Antonio's word.** PRs only with his OK. Co-Authored-By
  trailer; **no session links** in commits/PRs.
- Be frugal with tokens (shared account): scoped commands, targeted reads (offset/limit), never
  `planner-task-list` with no scope (it dumps the whole project), batch independent reads.
- Short, plain answers — conclusion first.

## What NOT to do

- Don't re-open the decided model (daemon/detach-attach, per-window view, size setting).
- Don't serialize wezterm/image types onto the wire — make a plain-data snapshot (T543).
- Don't fix at call sites; don't hard-code; don't invent widgets.
- Don't stop to ask after every checklist item — work autonomously; if context runs out, **checkpoint
  in the planner** (pause with a resume note) rather than leave a broken tree.
- Antonio tests by eye and from the running app; don't claim a visual or socket round-trip verified
  that you couldn't actually run — say it needs his eye-check.
- Don't leave worktrees/branches behind; a merged task's worktree gets removed.

When a task is done: tell Antonio, he reviews / eye-checks, and commits + PRs on his word.
