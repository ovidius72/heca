# Codex brief — F012 / P102: agent contract, agent status in the pane row, sounds

You (Codex) own the implementation of **phase P102** of heca's feature F012. It is **independent** of the
other F012 work (Claude is on P099 — the server/client socket connection — in a separate worktree), so
the two of you never touch the same files. Work only from this document; you do **not** touch the planner.

## 0. Setup & isolation (read before anything)

- **Your worktree / branch:** work in your own git worktree on branch `feat/p102-agent` (created off `main`).
  Do not work in `~/projects/heca` (that's Claude's P099 worktree).
- **Stay out of these — they are Claude's P099 lane:** `heca/src/server/`, `heca/src/rpc.rs`,
  `heca/src/main.rs` and any CLI/startup entry, and `heca-core/src/layout/`.
- **Do NOT touch the planner or planning state:** never edit `.planner/` or `.planning/`. You don't mark
  anything done — when you finish, tell Antonio; Claude reviews; if it's good, Claude marks P102 done.
- **Commit only on Antonio's word.** Build and test freely, leave it in the tree, report. Open a PR to
  `main` only when he says so. No session links in commits/PRs.
- Pre-existing: 2 `surface_drift` tests fail on `main` (a palette-merge drift) — **not yours**, ignore them.

## 1. Read first (do not skip)

- **`AGENTS.md` in full**, especially **§ 0**: the pre-flight (three written answers per thing you build),
  **widget-vs-component**, the **DOM-shaped event system** (capture → target → bubble; status changes are
  events), and **Rule Zero** (write the line a *plugin author* would type — if it needs a registry, an id,
  or branching on a program name, the API is wrong).
- `CLAUDE.md`, `docs/layout.md` (Flex/Grid/Surface, sizes as shares), `docs/widgets.md` (the catalog —
  the agent line is **composed from existing widgets**, never hand-drawn).
- The knowledge graph: run `graphify query "<question>"` to orient before grepping.

## 2. Standing rules (every change)

- **Fix library-side, never at the call site. Nothing hard-coded** — every value comes from config or the
  `Theme`. Status set, sounds, the pane-row line: all config/theme-driven.
- **Compose, don't invent.** The agent line reuses existing grid-ui widgets (Label/Icon/Badge/Tag/row),
  theme-driven. No ad-hoc `Flex`/`Surface` with literal sizes/colours.
- **One contract, no name-branching (Rule Zero).** Code must *never* branch on a program's name. Claude
  Code, Codex, pi and OpenCode are built-in implementations of the **same contract a plugin would write**.
- SOLID; one file one job (aim < 400 lines, hard limit 800 — agree a split before exceeding).
- **Verify by running, not grepping.** Every status path proven by a test or the running app.
- Scoped builds: `./scripts/lint-changed.sh` / `./scripts/lint-changed.sh test`, or
  `cargo clippy -p <crate> --all-targets -- -D warnings` / `cargo test -p <crate>`. Full workspace **only**
  before a PR, prefixed `HECA_FULL_WORKSPACE=1`. Don't run `cargo fmt` (no pinned config — hand-format).

## 3. The phase — what to build

Design reference: heca-pro `docs/decisions.md` decisions 13, 19, 20 (decision 13 = "Claude Code, Codex, pi
and OpenCode"). This replaces the status+sounds part of the old F008 (`agent-integration/agent-integration-plan.md`,
P048 tasks T173/T175, deferred) — **reuse that research on hooks, rewrite the design against today's code**.
Rooms / orchestration / agent-comms are heca-pro, **not** here.

### Current state (where things are)
- `heca-core/src/backend/terminal/osc.rs:4` — the escape-code reader today knows only prompt start, command
  start, pre-exec, program, command finished, cwd. It must learn **notification codes (OSC 9 / 99 / 777)**.
- `heca-core/src/runtime.rs:72` — `PaneRuntime` has **no agent state** today; add it here.
- Process status + pane rows: `heca/src/app/process_monitor.rs`,
  `heca/src/providers/workspaces/pane_row.rs` (and the sidebar line widgets in
  `heca/src/providers/workspaces/row_lines.rs` — a line is a self-contained widget subscribing to its pane).

### Goal
- **One agent contract** any agent program implements: **recognise** it (by program), **start** it (with
  whatever it needs to report status), **read** its status. Built-ins (Claude Code, Codex, pi, OpenCode) are
  implementations of the same contract a plugin would add — never a name `match`.
- **Status set:** working, waiting for input, waiting for permission, finished, error, compacting,
  subagent running — open to more (not a closed enum a plugin can't extend).
- **Status sources:** Claude Code hooks printing a notification escape code (installed via `--settings`,
  the user's own settings untouched); Codex's own notification codes; a small pi extension writing to a
  side channel; OpenCode (see facts below). The OSC reader learns the notification codes.
- **Agent line in the pane row:** name, status, current task — composed from existing widgets, theme-driven.
- **Sounds on status changes**, configurable per status (and per project).
- **Status changes are events** (DOM-shaped) and must be able to appear on the socket's event stream — but
  you only emit the event; the socket/event-stream transport is **P099 (Claude's lane)**. Emit a proper
  event; don't build the stream.

### OpenCode facts (v2.0.19, 2026-10-03)
- Per-run setup without touching the user's config: env `OPENCODE_CONFIG_DIR` (a folder of plugins) or
  `OPENCODE_CONFIG_CONTENT`, merged over the user's config.
- Status/events: local server (default `127.0.0.1:4096`) streams events at `GET /event`; plugins receive
  bus events (`permission.asked`, `tool.execute.before`, …).
- Usage: tokens + cost per message in `~/.local/share/opencode/opencode.db` (table `message`).
- Resume: `opencode --session <id>` / `--continue`.

### Preserve
- Non-agent panes unchanged; **process status stays separate from agent status**.
- AGENTS.md § 0; nothing hard-coded.

## 4. The four tasks (order is a suggestion)

1. **T171 — The agent contract.** One set of rules every agent program implements (recognise / start /
   read status), built-ins and plugins alike. This is the keystone: get the trait/contract shape right
   (Rule Zero — the line a plugin author writes) before wiring built-ins. Add agent state to `PaneRuntime`
   and the OSC notification-code reader.
2. **T173 — Agent line in the pane row.** name + status + current task, composed from existing widgets,
   theme-driven; a self-contained line widget like the existing cwd/git lines in `row_lines.rs`.
3. **T174 — Claude Code, Codex and pi through the contract.** The built-in implementations (OpenCode too,
   per the facts above). Each is just an implementation of T171's contract — no name-branching in the host.
4. **T175 — Sounds when status changes.** Configurable per status (and per project); status change → event
   → sound. Add settings to `config.default.toml` + README.

## 5. Done when

Claude Code in a pane shows **working → waiting → finished** in its pane row and plays the configured
sound; the same for Codex, pi and OpenCode; and a **test agent added through the contract** (as a plugin
would) works identically. All gates green (scoped clippy `-D warnings` clean, tests pass), nothing
hard-coded, no name-branching.

## 6. When you finish

Tell Antonio it's ready. He'll have Claude review it (code quality + AGENTS.md + rust-best-practices). If
it passes, **Claude** marks P102 done in the planner and the PR is cut on Antonio's word. Ask Antonio if any
of the design (the contract shape, the status set, where state lives) needs his decision before you build it.
