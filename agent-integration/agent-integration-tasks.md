# AI Agent Integration — Orchestration Board

> Coordination board for the **agent-integration** initiative. **Full context + research + task
> checklists live in `agent-integration/agent-integration-plan.md`** — read the referenced phase
> section before starting; this board is only assignment + status + responses. (Separate from
> `.planning/archive/pane-runtime-tasks.md` and `shared-tasks.md` — do not mix them.)
>
> **Starts only after** `pluggable-chrome-plugin-plan.md` is complete (ChromeHost + dynamic actions +
> built-in providers + WASM runtime exist). Agent drivers are built-in providers now and WASM
> plugins later, through the same `AgentDriver` contract.

## Roles
- **Orchestrator** (lead): assigns phases, reviews completions, accepts/rejects, advances the order.
- **Agent**: claims one *Open* phase, implements per the plan's task checklist, verifies, reports
  under *Agent Completion*. Updates **only** the phase it owns.

## Rules
1. Only active / not-yet-accepted phases stay here; accepted ones are struck or removed.
2. One agent per phase at a time. Set status to `In Progress` and put your id in *Assigned*.
3. Honor **Depends-on** — do not start a phase whose dependency isn't `Accepted`.
4. Obey the plan's **§3 cross-cutting rules** (grid-ui widgets; registry/keymap/RPC;
   event-on-mutation; `heca-core` UI-free; no hardcoded color/style/theme; showcase+docs; tests;
   no `cargo fmt`; clippy clean; no commit until user tested; rust-skill review at phase end).
5. Report with **code + verification + architecture notes** — completion notes alone are not
   acceptance. Include the exact `cargo test` / `cargo clippy` commands run + results.
6. Branch per phase off latest `origin/main` (`feature/agent-integration-<phase>`); open a PR;
   never merge without explicit OK. Pull/rebase from `origin/main` before starting each task slice.
7. **Reactive hazard rule (Phase 8.2):** defer `.set()` outside `panes.update()` borrow;
   `PaneRuntimeSignals` is `Copy` — follow the pane-runtime Phase 1 fix exactly.

## Status values
`Open` · `In Progress` · `Completed by Agent` · `Needs Edit` · `Accepted` · `Rejected`

## Dependency graph
```
0 ─► 1 ─► 2 ─► 3 ─► 5 (Claude Code, in-band) ─► 8 ─► 9 (display) ─► 12
                └► 4 ─► 7 (pi, side-channel)  ─► 8 ─► 10 (sounds) ─► 12
                       └► 6 (Codex, in-band) ─► 8                      ─► 12
                                                              11 (WASM) ─► 12
```
- 3 and 4 are parallel after 2. 5/6/7 are parallel after their transport (3 in-band / 4 side-channel).
- 8 needs ≥1 real driver (5 recommended first to validate the pipeline).
- 11 needs the **plugin plan Phase 9** done (WASM runtime).

---

## Phase 0 — Setup & baseline
**Status:** Open · **Assigned:** — · **Depends-on:** none · **Plan:** §7 Phase 0
**One-liner:** clean branch off latest `origin/main`; record green build/test/clippy baseline.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 1 — Core types: `AgentStatus` / `AgentState` / `AgentDriver` trait (heca-core)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 0 · **Plan:** §4 + §7 Phase 1
**One-liner:** `AgentStatus`(`#[non_exhaustive]`) + `AgentState` + `DriverId` in `runtime.rs`;
`AgentDriver` trait + `AgentTransport`/`AgentDisplay`/`AgentIntegrationAssets` in `agents.rs`;
`PaneRuntime.agent: Option<AgentState>` (additive). UI-free, object-safe, unit-tested.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 2 — `AgentDriverRegistry` + AppState wiring (heca)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 1 · **Plan:** §7 Phase 2
**One-liner:** `AgentDriverRegistry` (first-match detect, parse_osc/parse_sidechannel) in
`AppState`; `register_builtins` with stub drivers for claude/codex/pi (real impls replace stubs).
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 3 — OSC snooper extension: surface OSC 9/99/777 (heca-core)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 1 · **Plan:** §7 Phase 3
**One-liner:** add `OscEvent::AgentOsc { code, payload }` for OSC 9/99/777 (133/7 unchanged);
`TerminalBackend` queues `pending_agent_osc` (mirrors `pending_exit`) for the monitor to drain.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 4 — Side-channel transport: AF_UNIX socket + per-pane reader (heca-core + heca)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 1 · **Plan:** §7 Phase 4
**One-liner:** `StatusSocket` (AF_UNIX, cross-platform `os::unix`/`os::windows::net`, mpsc reader
thread); spawn path sets `$HECA_STATUS_SOCKET` when the driver transport is SideChannel/Both.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 5 — Claude Code driver (in-band OSC via `--settings` temp file)
**Status:** Open · **Assigned:** — · **Depends-on:** Phases 2, 3 · **Plan:** §7 Phase 5
**One-liner:** `ClaudeCodeDriver` (matches `claude`; parse OSC 9 `heca:claude:` payload; maps
UserPromptSubmit/Notification/Stop/StopFailure/SubagentStart → AgentStatus); hook emitter assets;
spawn injects `claude --settings /tmp/heca-<pane>.json` (hooks concatenate, user config untouched).
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 6 — Codex driver (native OSC 9 capture, zero injection)
**Status:** Open · **Assigned:** — · **Depends-on:** Phases 2, 3 · **Plan:** §7 Phase 6
**One-liner:** `CodexDriver` (matches `codex`; parse Codex's native OSC 9 string heuristically →
Finished/WaitingForInput; no injection v1). Richer `notify`-adapter states deferred (§8).
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 7 — pi driver (shipped extension + side-channel socket)
**Status:** Open · **Assigned:** — · **Depends-on:** Phases 2, 4 · **Plan:** §7 Phase 7
**One-liner:** `heca/assets/agent-integration/heca-status.ts` pi extension (agent_start/tool_call/
agent_end/session_before_compact/session_shutdown → JSON to `$HECA_STATUS_SOCKET` via Node `net`);
`PiDriver` parses side-channel JSON; idempotent install to `~/.pi/agent/extensions/`.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 8 — `PaneRuntime.agent` mirror + `PaneAgentChanged` event + config switches (heca)
**Status:** Open · **Assigned:** — · **Depends-on:** ≥1 of Phases 5/6/7 · **Plan:** §7 Phase 8
**One-liner:** `agent_monitor` drains OSC + socket → writes `PaneRuntime.agent` via the registry;
`PaneRuntimeSignals.agent` signal + guarded `set_pane_agent` emitting `PaneAgentChanged`
(defer-`.set()`-outside-borrow pattern); `sync_pane_runtime_state` mirrors agent; config switches
`agent_integration` + per-agent `*_integration` (reload-aware, `ActionPolicy::Global`).
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 9 — Display widgets (agent status row alongside program/git)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 8 · **Plan:** §7 Phase 9
**One-liner:** compose the pane-card status row from existing `Item`/`Badge`/`Icon` (theme-driven);
prefer agent badge when `agent.is_some()`, fall back to OS `status`; add `agent_status_*` theme
tokens (mocha + latte). No new domain widget, no hardcoded colors/sizes/alphas.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 10 — Sounds (rodio, own thread, config, agent-indicator-aligned)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 8 · **Plan:** §7 Phase 10
**One-liner:** `SoundPlayer` (rodio `OutputStream`+`Sink` on a `std::thread`, mpsc-fed,
non-blocking); `SoundsConfig` (enabled/volume/pack/per-event/per-agent, agent-indicator-aligned
field names); subscribe to `pane.agent.changed`, diff old/new `AgentStatus`, play on transitions;
ship `heca/assets/sounds/default/`.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 11 — WASM plugin contract surface (AgentDriver via host SDK)
**Status:** Open · **Assigned:** — · **Depends-on:** Phase 8 + **plugin plan Phase 9 done** · **Plan:** §7 Phase 11
**One-liner:** expose `app.agents.registerDriver({id, matches, transport, parseOsc,
parseSidechannel, display})` in the WASM host SDK; `WasmAgentDriver` adapter wraps a plugin as
`Box<dyn AgentDriver>` + dynamic registration. Proves the seam held (no rework of 1–10).
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

## Phase 12 — Docs + plan/board finalize
**Status:** Open · **Assigned:** — · **Depends-on:** Phases 9, 10, 11 · **Plan:** §7 Phase 12
**One-liner:** `keybindings.toml` (settings + sounds examples); `AGENTS.md` (Agent Integration
section); `README.md` + `.planning/research/ARCHITECTURE.md` (data-flow); tick the board;
update `PLAN.md` status + `.planning/STATE.md`. Final rust-skill review of the whole initiative.
**Agent Completion:** —
**Reviewer Decision:** — · **Reviewer Notes:** —

---

## Activity log
- 2026-06-18 — research complete; design locked (§0.1–§0.8); plan + board created. Initiative
  parked until `pluggable-chrome-plugin-plan.md` is complete.