# AI Agent Integration — Status Tracking + Sounds — Implementation Plan

> **Status:** research complete (2026-06-18) · design locked · **pre-implementation**.
> **Starts only after:** `pluggable-chrome-plugin-plan.md` is complete (ChromeHost + dynamic
> actions + built-in providers + WASM runtime exist). This plan consumes that world: agent
> drivers are built-in **providers** now and third-party **WASM plugins** later, through the
> same `AgentDriver` contract.
> **Orchestration board:** `agent-integration/agent-integration-tasks.md`.
> **Single planning index:** `PLAN.md` references this doc; deep rationale + full research live here.

> This plan turns every heca pane that runs an AI agent (Claude Code, Codex, pi, …) into a
> **live, observable agent object**: a structured `AgentStatus` (Working / WaitingForInput /
> WaitingForPermission / Finished / Error / Compacting / SubagentRunning) sourced from each
> agent's own **lifecycle hooks**, carried over a per-driver **transport** into heca's
> canonical `PaneRuntime`, mirrored reactively into the chrome store, emitted as a typed
> `pane.agent.changed` event any plugin can subscribe to (`app.on('pane.agent.changed', …)`),
> and displayed alongside the existing program/git rows. It also adds **transition sounds**
> (rodio, own thread, config-driven, aligned with the `agent-indicator` convention).
>
> **Generic + pluggable by design:** a `AgentDriver` trait + registry is the strategy-pattern
> seam. Built-in Rust drivers (Claude Code / Codex / pi) ship now; the same trait becomes the
> WASM plugin host contract for third-party agents (Aider, Cursor, …) — no rework.

---

## 0. Locked decisions (do not re-litigate — settled in research 2026-06-18)

### 0.1 Timing — this is a post-plugin-plan feature
- Built **after** `pluggable-chrome-plugin-plan.md` is complete (ChromeHost, dynamic
  ActionRegistry, built-in providers, WASM runtime all exist).
- Designed **for** that world: agent drivers = first-party **providers** (Rust impls of
  `AgentDriver`) + third-party **WASM plugins** implementing the same contract through the host SDK.
- The `AgentDriver` trait is the **seam**, not a parallel system: built-in now, WASM later, no rework.

### 0.2 State model — two separate fields, both shown
- `PaneRuntime.status: ProcessStatus` (OS-detected, Phase 2 of pane-runtime: Running/Idle/Success/Error) **stays unchanged**.
- **NEW** `PaneRuntime.agent: Option<AgentState>` (agent-integration-derived; `Some` only when a
  driver is active for this pane). Carries `driver_id` (provenance) + `AgentStatus` + optional
  `detail` / `turn_id`.
- Display shows **both** when both are meaningful (e.g. Row 1 = `Icon · Claude Code · [WaitingForInput]`,
  Row 2 = git). Agent status is **preferred** for the status badge when `agent.is_some()`; OS status
  is the fallback for non-agent panes (shells, nvim).
- `ProcessStatus` and `AgentStatus` are **distinct enums** — never merged. `driver_id` distinguishes
  agent-derived status from OS-detected.

### 0.3 Agent status set — locked, extensible
```
AgentStatus = Working | WaitingForInput | WaitingForPermission
            | Finished | Error | Compacting | SubagentRunning
            | (extensible — #[non_exhaustive]; future variants as drivers need them)
```
- Drivers map their native events onto this set; an unmapped event → `Working` + a `detail` string.
- `#[non_exhaustive]` on the enum so a WASM plugin reporting an unknown state doesn't break the host.

### 0.4 Transport — per-driver, abstracted by the trait
| Agent | Primary transport | Injection (user installs nothing) |
|---|---|---|
| **Claude Code** | **in-band OSC 9** via hook `terminalSequence` (v2.1.141+) | `claude --settings /tmp/heca-claude-<pane>.json` — temp file with only heca hooks; **hooks arrays concatenate** across scopes, user config untouched |
| **Codex** | **in-band OSC 9** from Codex's **native** notification emission (turn-complete / permission) | **none** (snooper captures Codex's own OSC 9) — zero install for coarse status; richer states via `notify` adapter deferred |
| **pi** | **side-channel AF_UNIX socket** (Node `net` → `$HECA_STATUS_SOCKET`) + `ctx.ui.setTitle` (OSC 2) bonus | heca-shipped pi extension in `~/.pi/agent/extensions/heca-status.ts` (installed once, like shell integration) |

- heca defines **one payload schema** (`AgentStatus` JSON: `{agent, status, detail?, cwd?, turn_id?}`).
  Transport differs per driver; the `AgentDriver` trait abstracts it (`transport()` + `parse_osc()` /
  `parse_sidechannel()`).
- **In-band OSC**: heca-namespaced payload inside the **allowlisted** OSC 9 / 99 / 777 (Claude Code's
  `terminalSequence` rejects custom OSC like 1337, and rejects cursor/clipboard/hyperlink). Payload
  prefix `heca:` distinguishes heca-status from real terminal notifications. heca's existing
  `OscSnooper` (Phase 3 of pane-runtime) is extended to surface OSC 9/99/777 payloads to the agent
  monitor — **zero new infra for the in-band path**.
- **Side-channel**: **AF_UNIX socket** (Rust std `os::unix::net` / `os::windows::net`, Win10 1803+
  — heca already requires Win10+). One per pane, path passed via `$HECA_STATUS_SOCKET`; a tiny
  reader (tokio task) feeds an `mpsc` → app loop. No ports, cross-platform.

### 0.5 Driver registry in AppState — plugin-visible via the event bus
- `AgentDriverRegistry` lives in `AppState` (like `ActionRegistry` / `KeymapRegistry`). Built-in
  drivers registered at startup; WASM plugins register drivers dynamically later.
- Per-pane agent state lives on `PaneRuntime.agent` (canonical, heca-core) + reactive mirror in
  the chrome store (per-pane signal) — **same split as pane-runtime Phase 1**.
- Plugins get the state through the **existing typed event bus**
  (`ChromeEvent::PaneAgentChanged { pane, agent }` → `"pane.agent.changed"` + catch-all `"*"`)
  — the same bus pane-runtime Phase 0 introduced. WASM host (plugin plan Phase 9) bridges it to
  `app.on('pane.agent.changed', data)`. **No new event mechanism.**

### 0.6 Auto-inject — user installs nothing, never clobber user config
- **Claude Code**: `claude --settings <temp>` — hooks concatenate (arrays merge across scopes),
  user's `~/.claude/settings.json` + project `.claude/settings.json` still load untouched.
  Limitation: org-managed `allowManagedHooksOnly` / `disableAllHooks` blocks heca hooks too
  (documented).
- **Codex v1**: none (native OSC 9). The `notify = [...]` adapter chain-merge into
  `~/.codex/config.toml` (the clobber-risk spot, handled with agent-indicator's chain-wrapper
  pattern) is **deferred** to a later iteration for richer states.
- **pi**: heca-shipped extension copied to `~/.pi/agent/extensions/heca-status.ts` (one-time, like
  shell-integration assets). The extension is TypeScript, auto-discovered by pi.

### 0.7 Sounds — rodio, own thread, agent-indicator-aligned
- **rodio** (cpal backend, cross-platform; own audio thread, non-blocking from the winit loop).
  Chosen over **kira** (richer — fades/mixer/scheduling — overkill for SFX).
- Config `[sounds]`: `enabled`, `volume` (0..1), per-event sound files (`on_finished`,
  `on_error`, `on_waiting_for_input`, `on_working`, `on_compacting`, `on_subagent_running`),
  `pack` (CESP-style dir), `no_repeat`. Per-agent overrides `[sounds.agents.<driver_id>]`.
- Triggered on **status transitions** (diff old/new `AgentStatus`), not every state. Subscribe to
  `pane.agent.changed` on the event bus; play in the audio thread via a channel.
- **Aligns with `agent-indicator`** (accessd/agent-indicator) state vocabulary
  (running / needs-input / done / off) + per-state sound packs. Optional: if the user has
  `agent-state.sh` installed, heca can delegate sounds to it; otherwise built-in rodio + shipped assets.

### 0.8 Convention alignment — `agent-indicator` is a convention, not a wire protocol
- `agent-indicator` does **not** define a new OSC wire format — it uses standard OSC 2 (title) +
  OSC 11 (bg) + OSC 9/99/777 (notify) + BEL, and the agent→indicator integration is a shell CLI
  (`agent-state.sh --state <running|needs-input|done|off>`). It's a **recipient-side aggregator
  with backends** (terminal/sound/desktop/push).
- heca adopts its **state vocabulary + sound-event semantics** (so heca is interoperable +
  familiar), but keeps its **own wire format** (heca-namespaced OSC payload + JSON-over-socket).
  heca's richer 7-state `AgentStatus` maps onto agent-indicator's 4 for sound/display parity.

---

## 1. Goal & scope

### Goal
Every heca pane running an integrated AI agent reports a structured `AgentStatus` to heca in real
time, surfaced in the pane-info widgets and emitted as a typed event any plugin can react to, with
configurable transition sounds — all without the user editing agent config.

### In scope
- `AgentStatus` / `AgentState` / `AgentDriver` trait + `AgentDriverRegistry` (heca-core + heca).
- Transport plumbing: OSC snooper extension (in-band) + AF_UNIX socket reader (side-channel).
- Three built-in drivers: Claude Code (in-band OSC via `--settings`), Codex (native OSC 9), pi
  (shipped extension + side-channel).
- `PaneRuntime.agent` field + reactive mirror + `PaneAgentChanged` event.
- Display widgets (agent status row alongside program/git).
- Sounds (rodio, config, per-event + per-agent, agent-indicator-aligned).
- WASM plugin contract surface (design + trait adapter stub) — real wiring after plugin plan Phase 9.
- Docs (`keybindings.toml`, `AGENTS.md`, `README.md`, `.planning/research/ARCHITECTURE.md`).

### Out of scope (explicitly deferred)
- Codex `notify`-adapter chain-merge into `~/.codex/config.toml` for richer Codex states (v1 = native
  OSC 9 only).
- Background image / wallpaper (belongs to `compositor-blur-refactor-plan.md`).
- OS-level desktop notifications (agent-indicator's desktop backend) — heca focuses on in-app
  status + sounds; desktop notify is the OS/terminal's job.
- Per-agent custom keybindings / RPC actions beyond what the dynamic ActionRegistry already gives.
- Non-AI agent process tracking (plain shells, nvim) — that's pane-runtime Phase 2's job.

---

## 2. Research findings (the evidence base — read before coding)

### 2.1 Claude Code — hooks + `terminalSequence` (richest surface)
Claude Code fires lifecycle hooks on exactly the states heca wants:
`SessionStart` · `UserPromptSubmit` (turn begins) · `PreToolUse`/`PostToolUse`/`PostToolBatch`
(working) · `Notification` with matcher `idle_prompt` (**WaitingForInput**) · `Notification`
matcher `permission_prompt` / `PermissionRequest` (**WaitingForPermission**) · `Elicitation`
(MCP requesting input) · `Stop` (**Finished**) · `StopFailure` matchers `rate_limit`/`overloaded`/
`auth_failed` (**Error**) · `SubagentStart`/`SubagentStop` (**SubagentRunning**) · `SessionEnd`.
Each hook receives JSON on stdin (`session_id`, `cwd`, `tool_name`, `tool_input`,
`last_assistant_message`, `error`, …).

**`terminalSequence` (v2.1.141+)**: a hook returns `{ "terminalSequence": "\u001b]9;…\u007" }`
and Claude Code emits it through its own terminal write path (race-free, works in tmux/ssh/Windows;
`/dev/tty` is unavailable to hooks since v2.1.139). **Allowlisted to OSC 0/1/2/9/99/777 + BEL only**
(no 1337, no cursor/clipboard/hyperlink — a security control). The OSC never enters the agent
transcript (zero token cost). heca's snooper captures OSC 9/99/777 → driver parses the heca payload.

**Settings layering (no-clobber auto-inject)**: scopes Managed > CLI `--settings <file>` > Local
(`.claude/settings.local.json`) > Project (`.claude/settings.json`) > User (`~/.claude/settings.json`).
`--settings <file>` merges with file-based settings; **`hooks` arrays concatenate** across scopes.
So heca writes a temp `{ "hooks": {…heca OSC 9 emitter…} }` and passes `--settings /tmp/heca-<pane>.json`;
the user's hooks still load and **add**, not replace. No `CLAUDE_CONFIG_DIR` redirector exists;
`--settings` is THE merge mechanism. Limitation: `allowManagedHooksOnly`/`disableAllHooks`
(managed settings) blocks heca hooks too.

### 2.2 Codex — native OSC 9 + hooks (no `terminalSequence`)
Codex emits **OSC 9 itself, natively**, on turn-complete and permission-request (confirmed via
linw1995's investigation: `printf '\033]9;%s\a' "…"`). heca's snooper captures this with **zero
injection** → free coarse status (`Finished` + `WaitingForInput`). Codex also has a hooks system
(SessionStart, PreToolUse/PostToolUse, PermissionRequest, Stop, SubagentStop, PreCompact/PostCompact)
receiving JSON on stdin, but hook output is JSON-only (`systemMessage`/`decision`) — **no
`terminalSequence` equivalent**, so Codex hooks cannot emit OSC to the PTY. For richer states,
Codex integrates via `~/.codex/config.toml` `notify = ["adapter.sh"]` (an array; agent-indicator
uses a chain wrapper to avoid clobber). **v1 decision: native OSC 9 only, no config.toml writing.**

### 2.3 pi — in-process extensions with full system access
pi extensions are TypeScript modules (`~/.pi/agent/extensions/*.ts`, auto-discovered), loaded via
jiti, running **in-process with full system permissions** (fs, `pi.exec`, Node built-ins,
`process.stdout`). Rich lifecycle event surface:
`session_start` · `session_shutdown` · `agent_start` · `agent_end` · `turn_start` · `turn_end` ·
`message_start/update/end` · `tool_execution_start/update/end` · `tool_call` (can block) ·
`tool_result` (can modify) · `context` · `before_agent_start` · `model_select` ·
`thinking_level_select` · `session_before_compact/compact` (→ **Compacting**) · `input` ·
`user_bash` · `before_provider_request` · `after_provider_response`.

A community pi extension (gist) already does agent-status tracking via `ctx.ui.setTitle` (OSC 2):
`agent_start`→running, `tool_call`→current tool + interview→waiting, `agent_end`→done/waiting/error
(from `stopReason` + `sawInterview`), `session_shutdown`→reset. heca's shipped extension will use
the **side-channel socket** (Node `net.connect($HECA_STATUS_SOCKET)`) for structured `AgentStatus`
as primary, with `ctx.ui.setTitle` as a free title bonus. pi is **not sandboxed** (unlike Claude Code
hooks), so it can write the socket directly.

### 2.4 `agent-indicator` — the de-facto convention (accessd/agent-indicator)
States: `running` / `needs-input` / `done` / `off`. Backends: terminal (OSC 2 title + OSC 11 bg tint +
OSC 9/99/777 notify + BEL), sound (CESP packs, per-state, no-repeat), desktop (osascript/notify-send),
push (ntfy/pushover/telegram). Agent integration is a **shell CLI** (`agent-state.sh --state X`) —
NOT a wire protocol. heca aligns **semantics** (state names + per-state sounds) but keeps its own
wire format. Sound config mirrors agent-indicator (`pack`, `volume`, `states.<event>`,
`overrides.<event>`).

### 2.5 Audio — rodio vs kira
rodio = simpler (play a sound file, own thread, cpal backend, cross-platform macOS CoreAudio / Linux
ALSA·Pulse / Windows WASAPI). kira = richer (fades/loops/mixer/clocks — overkill for SFX). **rodio
chosen.** Initialize once on a separate `std::thread`; send `Play { event, volume }` commands via
an `std::sync::mpsc`. Modern cpal runs its own thread; compatible with winit 0.30. Ship default
assets in `heca/assets/sounds/`.

---

## 3. Cross-cutting rules (every phase obeys — from AGENTS.md + project memory)

- **New UI = generic, theme-driven `heca-grid-ui` widget** — embed `Base`, read ALL styling from
  `Theme`, domain-neutral name, no hardcoded sizes/colors/alphas. The agent status row is a
  composition of existing widgets (`Item`/`Badge`/`Tag`/`Icon`), not a new domain widget.
- **All actions through `ActionRegistry` + `KeymapRegistry`** — if any agent feature is
  user-triggerable (e.g. "replay last sound", "toggle agent integration"), it goes through the
  registry, configurable in `config.toml`, reachable from mouse + keybinding + RPC.
- **No hardcoded color/style/theme** — everything via `config.toml` / theme / appearance plumbing.
- **`heca-core` stays UI-free** — `AgentStatus`/`AgentState`/`AgentDriver` live in heca-core with
  no widget deps. The reactive mirror + display live in heca/heca-grid-ui.
- **Event-on-mutation** — every `PaneRuntime.agent` change emits `ChromeEvent::PaneAgentChanged`.
- **No `cargo fmt`**; hand-format to match; `cargo clippy --workspace --all-targets --all-features`
  clean (0 new warnings); `#[allow(dead_code)]` only with a `//` comment explaining why.
- **No commit until the user has tested**; rust-skill review at **phase end** (not per task).
- **Pull/rebase from `origin/main` before starting each task slice.**
- Branch naming `feature/agent-integration-<phase>`; one PR per phase; never merge without explicit OK.
- **Don't repeat yourself** — reuse `OscSnooper`, `ChromeEventBus`, `SharedChromeState`,
  `sync_pane_runtime_state`, `PaneRuntimeSignals`, the reactive pattern (defer `.set()` outside
  `panes.update()` borrow — `PaneRuntimeSignals` is `Copy`).

---

## 4. Data model (target Rust shapes — agree before coding)

```rust
// heca-core/src/runtime.rs — EXTEND existing PaneRuntime (additive; default = None)
/// Agent-integration-derived status. `Some` only when an AgentDriver is active for this pane.
/// Distinct from `ProcessStatus` (OS-detected). Display prefers `agent` when present.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentState {
    pub driver_id: DriverId,            // "claude-code" | "codex" | "pi" | <plugin>
    pub status: AgentStatus,
    pub detail: Option<String>,         // e.g. last tool name, error message
    pub turn_id: Option<String>,        // agent-native turn/session id if available
}

/// Lifecycle status reported by an integrated AI agent. Maps agent-native hook events.
/// `#[non_exhaustive]` so a future WASM plugin reporting an unknown state doesn't break the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum AgentStatus {
    #[default]
    Working,                // agent actively processing (PreToolUse/agent_start)
    WaitingForInput,        // agent idle, wants user input (Notification:idle_prompt)
    WaitingForPermission,  // permission/elicitation prompt (Notification:permission_prompt)
    Finished,               // turn done (Stop)
    Error,                  // turn ended in error (StopFailure:rate_limit/overloaded/auth_failed)
    Compacting,             // context compaction in progress (session_before_compact)
    SubagentRunning,        // a subagent is active (SubagentStart)
}

/// Stable identifier for an agent driver. Built-in ids are reserved; plugins use their own.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DriverId(pub String);

// PaneRuntime gains (additive, serde-defaulted):
//   pub agent: Option<AgentState>,
// Default = None (no driver active). No change to existing fields.
```

```rust
// heca-core/src/agents.rs (NEW) — the contract (UI-free, no wgpu)
/// What a pane-side agent integration driver provides. Strategy-pattern seam.
/// Built-in Rust impls now; the same contract is exposed to WASM plugins later.
pub trait AgentDriver: Send + Sync {
    fn id(&self) -> DriverId;
    /// Detection: does this program name invoke this agent? ("claude" → ClaudeCodeDriver)
    fn matches_program(&self, raw_program: &str) -> bool;
    /// Which transport(s) this driver consumes.
    fn transport(&self) -> AgentTransport;
    /// Parse an in-band OSC payload (OSC 9/99/777) the snooper surfaced. None if not ours.
    fn parse_osc(&self, osc_code: u32, payload: &[u8]) -> Option<AgentStatus>;
    /// Parse a side-channel socket message (one-line JSON). None if not ours / malformed.
    fn parse_sidechannel(&self, msg: &[u8]) -> Option<AgentStatus>;
    /// Assets to inject at PTY spawn so the agent emits status (hook configs, env vars, extension
    /// install). None if the driver needs no injection (e.g. Codex v1 native OSC 9).
    fn integration_assets(&self) -> Option<&AgentIntegrationAssets>;
    /// Human-readable name + default icon glyph for display.
    fn display(&self) -> AgentDisplay;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTransport { InBandOsc, SideChannel, Both }

#[derive(Debug, Clone)]
pub struct AgentDisplay { pub name: String, pub icon: String }

/// What a driver needs heca to set up at spawn. Driver-specific; resolved by the app.
#[derive(Debug, Clone, Default)]
pub struct AgentIntegrationAssets {
    pub env: Vec<(String, String)>,                 // e.g. HECA_STATUS_SOCKET=/tmp/...
    pub claude_settings_overlay: Option<String>,    // temp --settings JSON (Claude Code)
    pub pi_extension: Option<&'static str>,         // bundled .ts (pi)
    // Codex v1: None
}
```

```rust
// heca/src/agents/registry.rs (NEW) — global registry in AppState
pub struct AgentDriverRegistry { drivers: Vec<Box<dyn AgentDriver>> }
impl AgentDriverRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, driver: Box<dyn AgentDriver>);
    pub fn detect(&self, raw_program: &str) -> Option<&dyn AgentDriver>;   // first match
    pub fn driver(&self, id: &DriverId) -> Option<&dyn AgentDriver>;
    pub fn parse_osc(&self, osc_code: u32, payload: &[u8]) -> Option<(DriverId, AgentStatus)>;
    pub fn parse_sidechannel(&self, msg: &[u8]) -> Option<(DriverId, AgentStatus)>;
}

// heca/src/chrome/events.rs — EXTEND ChromeEvent (additive)
pub enum ChromeEvent {
    // …existing…
    PaneAgentChanged { pane: PaneId, agent: AgentState },   // NEW → "pane.agent.changed"
}
impl ChromeEvent {
    fn name(&self) -> &'static str {
        match self {
            // …existing…
            ChromeEvent::PaneAgentChanged { .. } => "pane.agent.changed",
        }
    }
}
```

---

## 5. Architecture (data flow)

```
Agent (Claude Code / Codex / pi) runs in a PTY heca owns
   │
   │  (transport per driver)
   ├─ Claude Code: hook returns {terminalSequence: OSC 9;heca:…} → Claude emits to PTY
   ├─ Codex:       emits OSC 9 natively on turn-complete / permission
   └─ pi:         heca-shipped extension writes JSON to $HECA_STATUS_SOCKET (AF_UNIX)
        │
        ▼
heca-core OSC snooper (extended: surfaces OSC 9/99/777 payloads)   ── in-band path
heca-core side-channel reader (AF_UNIX, tokio task → mpsc)         ── side-channel path
        │
        ▼
AgentDriverRegistry.parse_osc / parse_sidechannel  → (DriverId, AgentStatus)
        │
        ▼
process monitor (heca/src/app/agent_monitor.rs) — per wake / per socket msg:
   detect driver at spawn (registry.detect(program)); write PaneRuntime.agent; emit on change
        │
        ▼
PaneRuntime.agent  (canonical, heca-core) — source of truth
        │
        ▼
sync_pane_runtime_state (existing Phase 1 projection) — mirrors agent → store signal + event
        │
        ├─► ChromeEventBus.emit(PaneAgentChanged)  → "pane.agent.changed" + "*"
        │      └─► (future WASM host) app.on('pane.agent.changed', data)   — plugin-visible
        └─► reactive signal → display widgets repaint (agent status row)
        │
        ▼
Sound player (heca/src/audio/mod.rs, rodio, own thread) — subscribes to pane.agent.changed,
   diffs old/new AgentStatus, sends Play{event} to the audio thread
```

---

## 6. Task-relation convention

- **Relations** list task ids a task depends on (must be done first).
- **Check** is the concrete, runnable verification for the task (command / visual / test).
- **A task is `checked` only at the end of a review of that task** — the review runs the Check,
  inspects, then marks `[ ]` → `[x]`. A box ticked without a completed review is invalid.
- **Phase exit** = tests + a combined rust-skill review before starting the next phase.
- Per AGENTS.md: no commit until the user has tested; clippy clean; rust-skill review at phase end.

---

## 7. Phases & tasks

> Each phase is a board entry in `agent-integration/agent-integration-tasks.md`. An agent picks an
> *Open* phase, implements per the checklist here, verifies, and reports on the board.

### Phase 0 — Setup & baseline
**Goal:** clean branch off latest `origin/main` with a green build, before any change.

- [ ] **0.1 Sync with main** — `git fetch origin && git checkout -b feature/agent-integration-setup origin/main`.
  - Relations: none. Check: `git log -1` shows latest `origin/main` HEAD; branch clean.
- [ ] **0.2 Baseline green** — record the starting build/test/clippy state.
  - Relations: 0.1. Check:
    - [ ] `cargo build --workspace --all-targets` → green.
    - [ ] `cargo test --workspace` → green.
    - [ ] `cargo clippy --workspace --all-targets --all-features` → only known pre-existing warnings
          (record the exact count + locations; selection_model.rs dead_code + too_many_arguments).

**Phase 0 exit** — [ ] tests + clippy baseline recorded; no review needed (no code changed).

### Phase 1 — Core types: `AgentStatus` / `AgentState` / `AgentDriver` trait (heca-core, headless)
**Goal:** the contract + canonical types, UI-free, unit-tested. No app wiring yet.

- [ ] **1.1 `heca-core/src/runtime.rs` — extend `PaneRuntime`** — add `pub agent: Option<AgentState>`
  (additive, `#[derive(Default)]` → `None`). Add `AgentStatus` (`#[non_exhaustive]`, `Copy`,
  `Default = Working`) + `AgentState` (`driver_id`, `status`, `detail`, `turn_id`) + `DriverId`
  newtype. Doc-comment each (rust-skill: `///` on every public item; `Display` for `AgentStatus` +
  `DriverId`). No UI deps.
  - Relations: none. Check: `cargo build -p heca-core` green; `cargo test -p heca-core` green.
- [ ] **1.2 `heca-core/src/agents.rs` (NEW) — `AgentDriver` trait + supporting types** — `AgentDriver`,
  `AgentTransport`, `AgentDisplay`, `AgentIntegrationAssets` (§4 shapes). `AgentDriver: Send + Sync`.
  All items `pub(crate)`-minimal / `pub` only where the app needs them. Doc the trait contract.
  - Relations: 1.1. Check:
    - [ ] `cargo build -p heca-core` green.
    - [ ] Unit test: a `#[cfg(test)]` `struct DummyDriver` impl proving the trait is object-safe
          (`Box<dyn AgentDriver>` compiles) + `parse_osc` returns `Some` for its payload, `None`
          otherwise. `cargo test -p heca-core` green.
- [ ] **1.3 Export + `lib.rs`** — `pub mod agents; pub use agents::{AgentDriver, ...};` and re-export
  `AgentStatus`/`AgentState`/`DriverId` from `runtime`.
  - Relations: 1.1, 1.2. Check: `cargo build -p heca-core` + `cargo test -p heca-core` green.

**Phase 1 exit** — [ ] heca-core tests green + clippy clean. **Review:** trait is object-safe, UI-free,
`#[non_exhaustive]` on `AgentStatus`, no `unwrap`/`expect` in the new code, `Result` where fallible.
Proceed to Phase 2.

### Phase 2 — `AgentDriverRegistry` + AppState wiring (heca)
**Goal:** the global registry in `AppState`, with built-in drivers registered at startup (stub
bodies — real parsing lands in Phases 5–7). Detection at spawn returns a driver id.

- [ ] **2.1 `heca/src/agents/registry.rs` (NEW)** — `AgentDriverRegistry` (§4). `register`,
  `detect`, `driver`, `parse_osc`, `parse_sidechannel`. `Vec<Box<dyn AgentDriver>>` (first-match
  wins on `detect`). No mutation after startup (built-ins); dynamic registration is a later phase.
  - Relations: 1.3. Check: unit test — register two dummy drivers, `detect` returns the right one
    per program, `parse_osc` routes to the right driver. `cargo test -p heca` green.
- [ ] **2.2 `heca/src/agents/mod.rs`** — module + `pub use`. Built-in driver registration helper
  `register_builtins(&mut registry)` (registers stub `ClaudeCodeDriver`/`CodexDriver`/`PiDriver`
  from Phases 5–7; for now `DummyDriver`-style stubs that match `"claude"`/`"codex"`/`"pi"` and
  return `None` from parse — real impls replace the stubs).
  - Relations: 2.1. Check: `cargo build -p heca` green.
- [ ] **2.3 AppState field** — add `agent_drivers: AgentDriverRegistry` to `AppState`; initialize in
  the existing app setup (`main.rs` / `app_state.rs`) via `register_builtins`. No behavior change yet.
  - Relations: 2.2. Check: `cargo run -p heca` builds + launches without panic; `cargo clippy -p heca` clean.

**Phase 2 exit** — [ ] heca tests + clippy green. **Review:** registry is `Send` (lives in `AppState`
across the winit loop), no interior mutability needed (read-only after startup), first-match-detect
semantics documented. Proceed to Phase 3.

### Phase 3 — OSC snooper extension: surface OSC 9/99/777 to the agent monitor (heca-core)
**Goal:** extend the existing `OscSnooper` (pane-runtime Phase 3) to also surface OSC 9/99/777
payloads (currently it only handles OSC 133 + OSC 7) as a new `OscEvent::AgentOsc { code, payload }`
variant. **No parsing here** — the driver registry does that.

- [ ] **3.1 Extend `OscEvent`** — add `AgentOsc { code: u32, payload: Vec<u8> }` to the existing
  `OscEvent` enum in `heca-core/src/backend/terminal/osc.rs`. Emit it for OSC 9, 99, 777 (the
  allowlisted notification OSCs). Keep the existing 133/7 handling untouched.
  - Relations: 1.3. Check: unit tests in `osc.rs` — feed `\x1b]9;heca:test\x07` → one `AgentOsc{code:9,...}`;
    feed `\x1b]777;notify;x;y\x1b\\` → `AgentOsc{code:777,...}`; feed `\x1b]133;A\x07` → still `PromptStart`
    (unchanged). `cargo test -p heca-core` green.
- [ ] **3.2 Route `AgentOsc` in `TerminalBackend::apply_osc_event`** — when an `AgentOsc` arrives,
  stash it in a per-backend queue (e.g. `pending_agent_osc: Vec<(u32, Vec<u8>)>`) for the monitor to
  drain, mirroring the existing `pending_exit` pattern. **Do not mutate `PaneRuntime.agent` from the
  backend** — the monitor (Phase 8) does that, going through the registry.
  - Relations: 3.1. Check: `cargo build -p heca-core` green; a test feeding OSC 9 to a
    `TerminalBackend` then draining `take_agent_osc()` returns the payload.

**Phase 3 exit** — [ ] heca-core tests green + clippy clean. **Review:** snooper change is additive
(133/7 unchanged), `AgentOsc` carries raw bytes (no parsing in core), the queue matches the existing
`pending_exit` shape. Proceed to Phase 4.

### Phase 4 — Side-channel transport: AF_UNIX socket + per-pane reader (heca-core + heca)
**Goal:** a cross-platform AF_UNIX socket per pane (`$HECA_STATUS_SOCKET`) + a reader feeding the
app loop. Used by the pi driver (Phase 7); designed generically.

- [ ] **4.1 `heca-core/src/agents/socket.rs` (NEW) — `StatusSocket`** — create a temp AF_UNIX socket,
  return its path + an accept loop. Cross-platform via `std::os::unix::net` (`cfg(unix)`) and
  `std::os::windows::net` (`cfg(windows)`, Win10 1803+). API:
  ```rust,ignore
  pub struct StatusSocket { path: PathBuf, rx: Receiver<SocketMsg> }
  impl StatusSocket {
      pub fn spawn() -> std::io::Result<Self>;          // spawns a reader thread
      pub fn path(&self) -> &Path;                       // → set as $HECA_STATUS_SOCKET
  }
  pub struct SocketMsg { pub pane: PaneId, pub bytes: Vec<u8> }   // one line JSON per msg
  ```
  Use `std::sync::mpsc`; the reader thread accepts + reads line-by-line. Document the wire contract
  (one JSON object per line: `{"agent":"<driver_id>","status":"<AgentStatus>","detail?":"…","turn_id?":"…"}`).
  - Relations: 1.3. Check: unit test (cfg(unix) + cfg(windows)) — spawn, write a JSON line from a
    client, assert the `rx` receives it. `cargo test -p heca-core` green on both.
- [ ] **4.2 heca app wiring** — at PTY spawn, if the detected driver `transport()` is `SideChannel`
  or `Both`, create a `StatusSocket`, set `HECA_STATUS_SOCKET` in the child env, store the `rx`
  handle keyed by `PaneId`. The per-wake monitor (Phase 8) drains `rx` non-blockingly.
  - Relations: 4.1, 2.3. Check: `cargo build -p heca` green; manual: spawn a pane running `pi`,
    confirm `$HECA_STATUS_SOCKET` is set in the child env (`echo $HECA_STATUS_SOCKET`).

**Phase 4 exit** — [ ] tests green + clippy clean + manual env-var check. **Review:** socket path
is temp + cleaned on pane close; reader thread never blocks the winit loop (non-blocking drain +
mpsc); cross-platform cfg guards correct. Proceed to Phase 5.

### Phase 5 — Claude Code driver (in-band OSC via `--settings` temp file)
**Goal:** the first real, end-to-end driver — Claude Code → OSC 9 → registry → `PaneRuntime.agent`
→ event → (display stub). Validates the whole in-band pipeline.

- [ ] **5.1 `heca/src/agents/claude_code.rs` (NEW)** — `ClaudeCodeDriver` impl of `AgentDriver`.
  `matches_program`: program basename starts with `claude`. `transport = InBandOsc`.
  `parse_osc(code, payload)`: recognize OSC 9 with payload prefix `heca:claude:` → parse the heca
  status JSON (`{"status":"working",...}`). Map hook events → `AgentStatus`:
  `UserPromptSubmit`→Working, `Notification:idle_prompt`→WaitingForInput,
  `Notification:permission_prompt`→WaitingForPermission, `Stop`→Finished, `StopFailure`→Error,
  `SubagentStart`→SubagentRunning. `integration_assets().claude_settings_overlay` = the temp
  `--settings` JSON (the hooks each emit their `terminalSequence`).
  - Relations: 1.3, 2.1. Check: unit tests for `parse_osc` — each hook event → correct `AgentStatus`;
    non-heca OSC 9 → `None`. `cargo test -p heca` green.
- [ ] **5.2 Hook emitter asset** — `heca/assets/agent-integration/claude-hooks.json` — the hook
  commands that read the hook JSON on stdin + emit `terminalSequence: "\u001b]9;heca:claude:<status-json>\u0007"`.
  A small shell snippet per event (Stop, Notification, UserPromptSubmit, SubagentStart). The temp
  `--settings` JSON wraps these.
  - Relations: 5.1. Check: a shell unit test feeding sample hook stdin → the snippet prints the
    expected `terminalSequence` JSON.
- [ ] **5.3 Spawn injection** — in the PTY spawn path, if `registry.detect(program)` is the
  ClaudeCodeDriver, write the temp `--settings` JSON to a per-pane temp file + append
  `--settings <path>` to the claude argv. **Hooks concatenate** — user's own `~/.claude/settings.json`
  still loads. Respect `settings.agent_integration` + `settings.claude_integration` switches
  (new settings fields, Phase 8 config).
  - Relations: 5.1, 5.2, 2.3. Check: manual — spawn a pane running `claude` in heca, run a turn,
    confirm `PaneRuntime.agent` updates (debug log or the Phase 9 display) and that the user's own
    Claude settings still work (e.g. their permissions/hooks fire).

**Phase 5 exit** — [ ] tests green + clippy clean + manual end-to-end. **Review:** no user config
clobbered (hooks concatenate), `--settings` temp file cleaned on pane close, managed-lockdown
limitation documented. Proceed to Phase 6.

### Phase 6 — Codex driver (native OSC 9 capture, zero injection)
**Goal:** Codex → its own OSC 9 → registry → `PaneRuntime.agent`. Zero install.

- [ ] **6.1 `heca/src/agents/codex.rs` (NEW)** — `CodexDriver` impl. `matches_program`: basename
  starts with `codex`. `transport = InBandOsc`. `parse_osc`: Codex emits OSC 9 with a free-form
  notification string (e.g. `Codex: <last-assistant-message>` on turn-complete). Parse heuristically
  → `Finished` on turn-complete tokens, `WaitingForInput` on permission tokens (map per agent-
  indicator's Codex table: `agent-turn-complete/complete/done/stop`→Finished,
  `permission*/approve*/needs-input`→WaitingForInput). `integration_assets = None` (no injection v1).
  - Relations: 1.3, 2.1. Check: unit tests for `parse_osc` mapping. `cargo test -p heca` green.
- [ ] **6.2 Register + document** — flip the Codex stub in `register_builtins` to the real driver.
  Document the deferred richer-states path (`notify` adapter chain-merge) in this plan's §8.
  - Relations: 6.1, 2.2. Check: manual — spawn `codex`, run a turn, confirm `PaneRuntime.agent`
    flips to `Finished` on turn-complete.

**Phase 6 exit** — [ ] tests + clippy green + manual. **Review:** Codex path uses zero injection,
heuristic mapping is documented as coarse, deferral noted. Proceed to Phase 7.

### Phase 7 — pi driver (shipped extension + side-channel socket)
**Goal:** pi → heca-shipped extension writes JSON to `$HECA_STATUS_SOCKET` → registry → `PaneRuntime.agent`.

- [ ] **7.1 `heca/assets/agent-integration/heca-status.ts` (NEW)** — pi extension subscribing to
  `agent_start`(Working), `tool_call`(Working + detail=toolName), `tool_result`(Working),
  `agent_end`(Finished/Error from stopReason + sawInterview→WaitingForInput),
  `session_before_compact`(Compacting), `tool_call` on subagent tools (SubagentRunning),
  `session_shutdown`(reset). Writes one-line JSON to `process.env.HECA_STATUS_SOCKET` via Node
  `net.connect`. Also calls `ctx.ui.setTitle` for a free tab-title bonus.
  - Relations: 4.1. Check: a Node test invoking the extension's handler functions with mock events
    → correct JSON written to a mock socket.
- [ ] **7.2 `heca/src/agents/pi.rs` (NEW)** — `PiDriver` impl. `matches_program`: basename starts
  with `pi`. `transport = SideChannel`. `parse_sidechannel`: parse the JSON line → `AgentStatus`.
  `integration_assets().pi_extension = Some(HECA_STATUS_TS)` + env `HECA_STATUS_SOCKET`.
  - Relations: 7.1, 4.2, 1.3. Check: unit tests for `parse_sidechannel`. `cargo test -p heca` green.
- [ ] **7.3 Extension install** — on first spawn of `pi` (or app startup if `settings.pi_integration`),
  copy `heca-status.ts` to `~/.pi/agent/extensions/heca-status.ts` (one-time, idempotent; like shell
  integration asset shipping). Pass `HECA_STATUS_SOCKET` env.
  - Relations: 7.1, 7.2, 4.2. Check: manual — spawn `pi`, run a turn, confirm `PaneRuntime.agent`
    updates + `~/.pi/agent/extensions/heca-status.ts` exists.

**Phase 7 exit** — [ ] tests + clippy green + manual. **Review:** extension is idempotent + never
overwrites a user-modified version (version check or skip-if-exists + differs); socket path per
pane. Proceed to Phase 8.

### Phase 8 — `PaneRuntime.agent` mirror + `PaneAgentChanged` event + config switches (heca)
**Goal:** wire the canonical `PaneRuntime.agent` → reactive store signal → typed event, reusing
the pane-runtime Phase 1 projection. Add config switches.

- [ ] **8.1 Monitor** — `heca/src/app/agent_monitor.rs` (NEW) `sync_pane_agent_from_transports`:
  per wake, drain `take_agent_osc()` (in-band) + non-blocking `rx.try_recv()` (side-channel) →
  `registry.parse_*` → write `Pane.runtime.agent` (only on change). Called from `sync_chrome_state`
  before the existing `sync_pane_runtime_state`.
  - Relations: 3.2, 4.2, 5.1/6.1/7.2. Check: unit test with a fake backend queueing an `AgentOsc` →
    monitor writes `PaneRuntime.agent` + the projection emits `PaneAgentChanged`. `cargo test -p heca`.
- [ ] **8.2 Reactive mirror + event** — extend `PaneRuntimeSignals` (heca/src/chrome/state.rs) with
  an `agent` signal + a guarded `set_pane_agent(pane, agent)` setter that emits
  `ChromeEvent::PaneAgentChanged`. Use the **defer-`.set()`-outside-`panes.update()`** pattern
  (pane-runtime Phase 1 fix): decide inside the borrow, capture the `Copy` signal handle, `.set()`
  + emit outside. Extend `sync_pane_runtime_state` to mirror `agent`.
  - Relations: 8.1. Check: unit tests — round-trip, change-guard (no event on equal), pruning.
- [ ] **8.3 Config switches** — `heca-config/src/settings.rs`: `agent_integration: bool` (master,
  default true) + `claude_integration: bool`, `codex_integration: bool`, `pi_integration: bool`
  (defaults true). Gating in `register_builtins` + spawn injection. Reload-aware (config reload
  re-registers drivers — `ActionPolicy::Global` per pane-runtime's reload-when-floating fix).
  - Relations: 8.1. Check: `cargo test -p heca-config` green (defaults, override, reload).

**Phase 8 exit** — [ ] tests + clippy green. **Review:** reactive hazard (the Phase 1 fix) respected;
no registry bypass (all `PaneRuntime.agent` writes through the monitor); event-on-mutation proven
by test. Proceed to Phase 9.

### Phase 9 — Display widgets (agent status row alongside program/git)
**Goal:** surface `PaneRuntime.agent` in the pane-info widgets, preferring agent status for the
badge when present, falling back to OS `status`. Uses existing `heca-grid-ui` widgets.

- [ ] **9.1 Agent status badge** — compose the sidebar pane card's status row from `Item`/`Badge`/
  `Icon` (existing widgets; theme-driven). Row 1: `Icon(agent.icon) · Name(driver.display.name) ·
  Badge(AgentStatus)` when `agent.is_some()`; else the existing OS-status row. Row 2: git (unchanged).
  Status→theme color: Working=accent, WaitingForInput=warning, WaitingForPermission=warning,
  Finished=success, Error=danger, Compacting=muted, SubagentRunning=accent.
  - Relations: 8.2. Check: visual — run the showcase + heca; spawn Claude Code / pi, confirm the
    badge updates through a turn. `cargo run -p heca-renderer --example showcase` still green.
- [ ] **9.2 Theme tokens** — add `agent_status_*` color tokens to `Theme` (mocha + latte) wired to
  the above mapping. No hardcoded colors.
  - Relations: 9.1. Check: `cargo test -p heca-config` green (theme has the tokens).

**Phase 9 exit** — [ ] showcase + heca visual check + clippy green. **Review:** no new domain widget
(composed from existing ones), all colors from theme, no hardcoded sizes/alphas, AGENTS.md UI rule
satisfied. Proceed to Phase 10.

### Phase 10 — Sounds (rodio, own thread, config, agent-indicator-aligned)
**Goal:** transition sounds on `AgentStatus` changes, configurable, cross-platform.

- [ ] **10.1 `heca/src/audio/mod.rs` (NEW) — `SoundPlayer`** — rodio `OutputStream` + `Sink` on a
  `std::thread`; `std::sync::mpsc::Receiver<SoundCmd>`; `SoundCmd::Play { event, volume } |
  SetVolume(f32) | Stop`. `SoundPlayer::spawn() -> Self` (returns the handle with a `Sender`).
  Non-blocking `play(event)` from the winit loop. Guard `cfg(any(target_os = "macos",
  target_os = "linux", target_os = "windows"))` (rodio is cross-platform).
  - Relations: 8.2. Check: unit test (with `--features` off-audio or a no-op backend in CI) —
    `play()` doesn't block; the thread starts/stops cleanly. `cargo test -p heca` green.
- [ ] **10.2 Config** — `heca-config/src/sounds.rs` (NEW) `SoundsConfig { enabled: bool, volume: f32,
  pack: String, no_repeat: bool, on_working/on_finished/on_error/on_waiting_for_input/on_compacting/
  on_subagent_running: Option<PathBuf>, agents: HashMap<DriverId, SoundsConfig> }`. Defaults:
  `enabled=false`, `volume=0.5`, `pack="default"`. Align field names with `agent-indicator`.
  - Relations: 10.1. Check: `cargo test -p heca-config` green (defaults, per-event override,
    per-agent override, parse).
- [ ] **10.3 Bus subscription + assets** — subscribe to `pane.agent.changed` on the `ChromeEventBus`;
    diff old/new `AgentStatus` (capture old in `PaneRuntime` before the monitor writes); send
    `Play{event: new_status}` if `sounds.on_<event>` is set. Ship default assets in
    `heca/assets/sounds/default/` (a tasteful chime/buzz/ping). Optional: if `agent-state.sh` is on
    `$PATH`, delegate (documented).
  - Relations: 10.1, 10.2, 8.2. Check: manual — enable sounds in config, spawn Claude Code, run a
    turn, confirm the `on_finished` sound plays on `Stop`; no sound on every tool call (transition-only).

**Phase 10 exit** — [ ] tests + clippy green + manual sound. **Review:** audio thread never blocks
the render loop; sounds on transitions only; config-gated (off by default); assets shipped;
agent-indicator alignment. Proceed to Phase 11.

### Phase 11 — WASM plugin contract surface (AgentDriver via host SDK) — design + adapter stub
**Goal:** expose the `AgentDriver` contract to third-party WASM plugins (plugin plan Phase 9 must be
done). A WASM plugin implements `AgentDriver` through the host SDK; the host adapts plugin→driver.

- [ ] **11.1 Host SDK surface** — define the JS/TS-facing contract `app.agents.registerDriver({
  id, matches, transport, parseOsc, parseSidechannel, display })` in the WASM host SDK (plugin plan
  Phase 9 host API). Document the payload schema (the `AgentStatus` JSON).
  - Relations: plugin plan Phase 9 done. Check: design doc + a stub host binding compiles.
- [ ] **11.2 `WasmAgentDriver` adapter** — `heca/src/agents/wasm.rs` (NEW) wraps a loaded WASM plugin
  as a `Box<dyn AgentDriver>` and registers it dynamically in `AgentDriverRegistry` (add a
  `register_dynamic` path; keep `Send+Sync` via the host's thread-safe plugin handle).
  - Relations: 11.1. Check: a stub WASM plugin (e.g. an Aider driver returning `Working`) loads +
    `detect("aider")` returns it + `parse_osc` routes to it. `cargo test -p heca` green.

**Phase 11 exit** — [ ] tests + clippy green. **Review:** the contract is identical for built-in +
WASM (the seam held); no rework of Phases 1–10; `#[non_exhaustive]` `AgentStatus` handles unknown
plugin states. Proceed to Phase 12.

### Phase 12 — Docs + plan/board finalize
**Goal:** everything documented; the plan + board reflect reality.

- [ ] **12.1 `keybindings.toml`** — document `[settings]` agent_integration switches + the
  `[sounds]` section + per-agent overrides, with examples. Check: a user can copy the examples into
  `config.toml` and it parses.
- [ ] **12.2 `AGENTS.md`** — add an "Agent Integration" section (the trait + registry + transport +
  event + sound model; the "new agent = a driver, built-in or WASM plugin" rule).
- [ ] **12.3 `README.md` + `.planning/research/ARCHITECTURE.md`** — user-facing blurb + the
  data-flow diagram from §5.
- [ ] **12.4 Board finalize** — tick all phases in `agent-integration-tasks.md`; mark the initiative
  Accepted; update `PLAN.md` status snapshot + `.planning/STATE.md` Last Action.

**Phase 12 exit** — [ ] docs reviewed + clippy green. **Final rust-skill review** of the whole
initiative against `/Users/antonio/.agents/skills/rust/SKILL.md` (§13 below).

---

## 8. Deferred / future (tracked, not built in v1)

- **Codex richer states** via `notify` adapter chain-merge into `~/.codex/config.toml` (the
  clobber-risk spot — use agent-indicator's chain-wrapper pattern; restore on pane close).
- **Desktop notifications** (agent-indicator's desktop backend) — out of scope; the OS/terminal
  owns that.
- **Per-agent custom keybindings/RPC** beyond the dynamic ActionRegistry.
- **`agent-state.sh` delegation** for sounds (Phase 10 optional path).
- **Token/segment customization** of the agent status row (belongs to
  `pluggable-chrome-plugin-plan.md` §8.1/§8.2).

---

## 9. Sequencing & parallelism
- **0 first, alone.** Then **1** (foundation types).
- After 1: **2** (registry) → **3** (snooper) and **4** (socket) can go in parallel (independent
  transports); both gate the drivers.
- **5, 6, 7** (the three drivers) are independent of each other — parallel after 3+4 — but each
  depends on 2 (registry) + its transport (3 for in-band; 4 for side-channel). Suggested first:
  **5 (Claude Code)** end-to-end to validate the whole pipeline, then 6 + 7 in parallel.
- **8** after the drivers (needs at least one real driver to test the monitor).
- **9** after 8 (display reads the signal). **10** after 8 (sounds subscribe to the event) — 9 and
  10 parallel.
- **11** after the plugin plan Phase 9 is done (independent of 9/10 otherwise).
- **12** last.
- Suggested single-threaded order: 0 → 1 → 2 → 3 → 4 → 5 → 8 → 9 → 10 → 6 → 7 → 11 → 12.

---

## 10. Risks / watch-items
- **Claude Code `--settings` merge semantics** — confirm on each Claude Code minor version that
  `hooks` still concatenate (the doc says arrays merge; smoke-test on upgrade). `terminalSequence`
  allowlist could tighten — re-verify OSC 9 is still allowed.
- **Codex native OSC 9 payload format** is free-form string; the heuristic mapping is coarse by
  design — document, don't over-fit.
- **pi extension TUI interleaving** — `ctx.ui.setTitle` is sanctioned/safe; raw `process.stdout`
  writes are NOT used (avoid TUI corruption). The side-channel socket is the structured path.
- **AF_UNIX on Windows** — requires Win10 1803+ (heca already requires Win10+ for wgpu); verify the
  `os::windows::net` path compiles + runs. Fallback: `interprocess` crate.
- **rodio + winit** — modern cpal owns its thread; verify no main-thread block on init. CI: gate
  audio tests behind a no-op backend so headless CI doesn't need a sound device.
- **Reactive hazard (Phase 8.2)** — the pane-runtime Phase 1 fix (defer `.set()` outside
  `panes.update()` borrow; `PaneRuntimeSignals` is `Copy`) MUST be followed. A `.set()` inside the
  borrow panics if an effect reads `panes` during the set.
- **Managed-lockdown** — Claude Code `allowManagedHooksOnly`/`disableAllHooks` blocks heca hooks;
  document as a limitation, don't try to bypass.
- **`heca-core` stays UI-free** — `AgentStatus`/`AgentDriver` have no widget deps; the store + display
  are the only reactive/UI layer.

---

## 11. Rust-skill review checklist (run at each phase exit + final)

Per `/Users/antonio/.agents/skills/rust/SKILL.md`:
- [ ] `Result<T, E>` over panicking; `expect("descriptive")` only for init-time invariants; no `unwrap`
      in new code.
- [ ] `pub(crate)` over `pub` where possible; expose only what the app needs.
- [ ] `#[non_exhaustive]` on `AgentStatus` (forward-compat with WASM plugins).
- [ ] `#[derive(Debug, Clone, ...)]` + `Display` for `AgentStatus`/`DriverId`; `Default` where sensible.
- [ ] `///` doc comments on every public item; `//!` module-level for `agents.rs` / `audio/mod.rs`.
- [ ] `Send + Sync` for `AgentDriver` + `AgentDriverRegistry` (live in `AppState` across the winit loop).
- [ ] Ownership: `&str`/`Cow` in parse APIs (no needless `String` allocs on the hot OSC path);
      `Box<dyn AgentDriver>` for the registry (object-safe trait).
- [ ] Trait object-safety: no `Self` in args/return; `Send+Sync` supertraits.
- [ ] No `cargo fmt`; hand-format; `cargo clippy --workspace --all-targets --all-features` 0 new warnings.
- [ ] Tests: unit tests in each new module (drivers, registry, snooper extension, socket, monitor,
      audio no-op); integration via the existing `cargo test --workspace`.
- [ ] No `#[allow(dead_code)]` without a `//` reason; remove dead code instead.
- [ ] Error handling in `StatusSocket::spawn` → `io::Result`; propagate, don't panic.