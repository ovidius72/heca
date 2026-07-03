# Agents Communication — Plan & Backlog

> **Status:** first draft · analysis/design only · 2026-07-02.
> **Scope v1:** only agents running inside heca panes. Remote/mesh-only agents are explicitly out of scope.
> **Core principle:** heca owns local pane/workspace/session state; agent-comms owns room/DM/task messaging.
> **Depends on:** `pluggable-chrome-plugin-plan.md` for dock/region provider rendering, and
> `agent-integration/agent-integration-plan.md` for local pane-agent detection/status.

---

## 0. Locked Decisions

### 0.1 Local-only v1

v1 does **not** manage remote agents.

An agent is relevant only if it runs in a heca pane:

```text
heca session
└── workspace
    └── pane
        └── process: claude | codex | pi | other supported harness
```

agent-comms may technically expose remote peers, but v1 filters/ignores peers that cannot be
correlated to a local heca pane.

### 0.2 Room is a grouping/orchestration object, not a cwd object

A room has no semantic `cwd`.

A room can contain agents from different directories:

```text
Room "release"
├── orchestrator: claude@pane-7
├── members:
│   ├── claude@pane-7  cwd=/repo/frontend
│   ├── codex@pane-3   cwd=/repo/backend
│   └── pi@pane-9      cwd=/repo/docs
└── activity/messages/tasks
```

The cwd belongs to the **pane/process**, not to the room.

### 0.3 One active room per agent in v1

An agent can belong to **one room at a time** in v1. This avoids conflicting orchestration.

Future expansion may allow multiple rooms with one active/control room, but not in v1.

### 0.4 Detection/status and communication are separate integrations

Each harness needs two independent capabilities:

```text
Status/detection channel:
  agent → heca
  purpose: pane_id, cwd, lifecycle status, waiting/done/error

Communication/control channel:
  agent ↔ agent-comms
  purpose: room messages, DMs, orchestrator task assignment
```

heca should **not** invent a PTY-control protocol for agent-to-agent messaging if agent-comms already
solves that problem.

### 0.5 Add-to-room requires comms-ready agent

A detected local agent can be:

| State | Meaning | Room eligibility |
|---|---|---|
| `monitor_only` | heca detects status, but no agent-comms bridge is active | Cannot join room; show restart/setup prompt |
| `comms_ready` | agent has active agent-comms bridge/peer correlated to the pane | Can join room |
| `in_room` | comms-ready and assigned to a room | Can receive room tasks/messages |

If an agent is already running but not comms-ready, heca cannot magically inject an MCP bridge into
that running process. The UI should offer: **Restart with integration enabled** / **Setup integration**.

### 0.6 User opens panes manually in v1

The primary workflow is:

1. User opens a pane.
2. User runs an agent/harness (`claude`, `codex`, `pi`, ...).
3. Installed harness config sends status to heca.
4. agent-comms bridge connects the agent to the communication layer.
5. heca shows the agent in a dock/sidebar and allows adding it to a room.

Creating new pane/harness directly from a room is **v1.1**, not v1. The unresolved cwd question is
deferred by using an explicit future `LaunchProfile`, not by making room own a cwd.

---

## 1. Architecture Overview

```text
HecaApp
├── registry: ActionRegistry                 # outside AppState
├── keymap: KeymapRegistry                   # outside AppState
└── state: AppState
    ├── session: Session                     # canonical workspaces/columns/panes
    ├── chrome_state: SharedChromeState      # signal-backed pane/runtime projection
    ├── chrome_host: ChromeHost              # region host; provider mounting post-plugin-plan
    │
    ├── local_agent_registry: LocalAgentRegistry
    │   └── LocalAgentInstance[]             # canonical list of pane-backed agents
    │
    ├── agent_comms: AgentCommsControlPlane
    │   ├── sidecar process                  # Node.js agent-comms wrapper/control client
    │   └── room/member/activity mirror
    │
    └── agent_rooms: AgentRoomStore
        ├── rooms[]                          # grouping/orchestration state
        └── membership: local_agent_id -> room_id
```

### 1.1 Data ownership

| Data | Owner | Notes |
|---|---|---|
| Workspaces/panes/layout | `AppState.session` | Canonical; mutated only through actions |
| Pane runtime status | `PaneRuntime` + `chrome_state` mirror | Existing runtime projection pattern |
| Local agent identity/status | `LocalAgentRegistry` | Derived from agent-integration signals + pane runtime |
| Room definitions/membership | `AgentRoomStore` | heca grouping/orchestration model |
| Room messages/DM/task delivery | agent-comms | Communication/control layer |
| Provider UI state | `AgentsDockProvider` / `RoomsDockProvider` | Projection only; does not own panes |

### 1.2 Main flow

```text
User opens pane and runs claude
        │
        ▼
Claude status hook / pi extension / Codex notify
        │
        ▼
heca local agent detection
        │
        ▼
LocalAgentRegistry upsert: pane_id, cwd, harness, status
        │
        ├──────────────► Running Agents Dock
        │
        ▼
agent-comms bridge connects agent to comms layer
        │
        ▼
AgentCommsControlPlane correlates comms peer with local agent
        │
        ▼
Room Dock allows add-to-room if comms_ready
```

---

## 2. External References

### Project references

- `agent-integration/agent-integration-plan.md`
  - Source of truth for local agent detection/status research.
  - Claude Code hooks, Codex OSC/notify, pi extension/socket integration.
- `agent-integration/agent-integration-tasks.md`
  - Existing task breakdown for passive status tracking.
- `pluggable-chrome-plugin-plan.md`
  - Required for provider/dock integration.
  - Especially ChromeHost, RegionHost, built-in providers, dynamic actions, overlays.
- `AGENTS.md`
  - Mandatory rules: no ad-hoc UI, use heca-grid-ui widgets, route actions through registries.
- `docs/widgets.md`
  - Widget catalog and examples for DockFrame, ItemGroup, Item, Badge, Button, Input, Modal.

### Code references

- `heca-core/src/runtime.rs`
  - Target for additive `PaneRuntime.agent` / local agent status data if not already present.
- `heca-core/src/backend/terminal/osc.rs`
  - Existing OSC snooping path used by agent-integration.
- `heca/src/chrome/events.rs`
  - Typed chrome events for pane/agent/room updates.
- `heca/src/chrome/state.rs`
  - Shared chrome state mirror.
- `heca/src/input.rs`
  - `WmAction`, `action_from_name`, `action_priority`, parameterized action builder.
  - Already contains `MovePaneToWorkspace` and `MoveColumnToWorkspace`.
- `heca/src/actions.rs`
  - `ActionRegistry`, descriptors.
- `heca/src/app/interaction.rs`
  - `ActionPolicy` classification.

### External references

- agent-comms: <https://github.com/ExaDev/agent-comms>
  - `src/core/mesh-store.ts`
  - `src/core/tcp-transport.ts`
  - `src/core/wire-protocol.ts`
  - `src/core/types.ts`
  - `src/bridges/pi/index.ts`
  - `src/bridges/claude-code/channel.ts`
- Claude Code hooks documentation
  - Required for status hooks and MCP agent-comms bridge configuration.
- Codex configuration/notify documentation
  - Required for status signals and possible comms bridge integration.
- pi extension documentation
  - Required for status extension and agent-comms bridge/extension.

---

# Feature 1 — Harness Integration Installer

## Goal

Install and manage per-harness configuration so agents launched inside heca panes report status to
heca and, where supported, connect to agent-comms for room messaging.

This replaces the fragile idea of relying only on PATH shims. The installer makes normal commands
like `claude`, `codex`, and `pi` work without the user manually editing every config file.

## Scope

In scope:

- Install status hooks/extensions/config for supported harnesses.
- Install/enable agent-comms bridge config for supported harnesses.
- Backup and restore user config safely.
- Idempotent re-run.
- Per-harness health check: status-ready, comms-ready.

Out of scope:

- Remote agents.
- Forcing already-running processes to become comms-ready without restart.
- UI implementation; UI consumes installer state later.

## Dependencies

- `agent-integration/agent-integration-plan.md` for hook/event details.
- agent-comms bridge docs/source for each harness.
- `heca-config` for user settings controlling auto-install behavior.

## Phase 1.1 — Installer contract and config model

### Task 1.1.1 — Define `HarnessIntegration` model

**Description**

Define a typed model describing what heca knows how to install for each harness.

**Target shape**

```rust
pub struct HarnessIntegration {
    pub harness: HarnessId,
    pub display_name: String,
    pub status_capability: StatusCapability,
    pub comms_capability: CommsCapability,
    pub config_paths: Vec<PathBuf>,
}

pub enum StatusCapability {
    Unsupported,
    OscHook,
    SideChannelSocket,
    NotifyAdapter,
}

pub enum CommsCapability {
    Unsupported,
    McpBridge,
    ExtensionBridge,
    NativeBridge,
}
```

**Files**

- New: `heca/src/agents/integration.rs`
- New/extend: `heca-config/src/settings.rs`

**References**

- `agent-integration/agent-integration-plan.md` § Claude/Codex/pi transport table.

**Acceptance**

- Model can represent Claude, Codex, and pi separately.
- Does not store runtime pane state.

### Task 1.1.2 — Add config switches

**Description**

Add user-facing settings controlling whether heca may install/update integration files.

**Suggested config**

```toml
[agents]
enabled = true
auto_install_integrations = false
status_detection = true
agent_comms = true

[agents.harness.claude]
enabled = true
install_status_hooks = true
install_comms_bridge = true

[agents.harness.pi]
enabled = true
install_status_extension = true
install_comms_bridge = true

[agents.harness.codex]
enabled = true
install_status_notify = true
install_comms_bridge = false
```

**Files**

- `heca-config/src/settings.rs`
- `config.default.toml`
- `README.md`

**Acceptance**

- Defaults are conservative: do not mutate user config without explicit action/setting.
- Runtime reload reads settings.

## Phase 1.2 — Status integration installation

### Task 1.2.1 — Claude status hook installer

**Description**

Install a Claude Code hook/config that sends lifecycle status to heca when Claude runs inside a heca
pane. Prefer merge-safe config overlay or documented hook concatenation behavior; never clobber user
settings.

**Status events**

- SessionStart → agent detected/started
- UserPromptSubmit → working
- Notification idle_prompt → waiting for input
- Notification permission_prompt → waiting for permission
- Stop → finished
- StopFailure → error
- SubagentStart/SubagentStop → subagent running/finished

**Files**

- New: `heca/src/agents/installers/claude.rs`
- New assets: `heca/assets/agents/claude/heca-status-hook.*`

**External docs**

- Claude Code hooks docs.
- `agent-integration/agent-integration-plan.md` § Claude Code research.

**Acceptance**

- Installer is idempotent.
- Existing user hooks are preserved.
- Emits status only when `HECA_PANE_ID` / `HECA_AGENT_SOCKET` is present, so agents outside heca are ignored in v1.

### Task 1.2.2 — pi status extension installer

**Description**

Install a pi extension that listens to pi lifecycle events and sends structured status to heca.

**Files**

- New asset: `heca/assets/agents/pi/heca-status.ts`
- New: `heca/src/agents/installers/pi.rs`

**References**

- pi extension docs.
- `agent-integration/agent-integration-plan.md` § pi research.

**Acceptance**

- Extension connects to `$HECA_AGENT_SOCKET`.
- Sends `pane_id`, `cwd`, `harness`, `status`, optional `turn_id`.
- Does nothing if not running inside heca env.

### Task 1.2.3 — Codex status integration

**Description**

Implement best available Codex status detection. v1 may be coarse if Codex only emits native OSC/notify
for limited states.

**Files**

- New: `heca/src/agents/installers/codex.rs`

**Acceptance**

- Detects at least: running/working, waiting/finished where supported.
- Documents limitations clearly in README and plan.

## Phase 1.3 — agent-comms bridge installation

### Task 1.3.1 — Claude agent-comms bridge installer

**Description**

Install/enable the agent-comms bridge for Claude Code so Claude can receive room messages/DMs/tasks
through agent-comms.

**Important**

This is the communication/control path. Heca must not use PTY input as the primary room messaging path.

**Files**

- New: `heca/src/agents/installers/claude_comms.rs`

**External references**

- agent-comms Claude bridge: `src/bridges/claude-code/channel.ts`
- Claude MCP configuration docs.

**Acceptance**

- Claude launched inside heca with integration installed becomes `comms_ready`.
- If Claude is already running without bridge, UI reports restart required.

### Task 1.3.2 — pi agent-comms bridge installer

**Description**

Enable agent-comms bridge/extension for pi.

**Files**

- New: `heca/src/agents/installers/pi_comms.rs`

**References**

- agent-comms pi bridge: `src/bridges/pi/index.ts`

**Acceptance**

- pi launched inside heca can join room and receive messages/tasks.

### Task 1.3.3 — Codex comms capability assessment

**Description**

Determine whether Codex v1 can be comms-ready using available MCP/bridge support. If not, mark Codex
as `monitor_only` until bridge support exists.

**Files**

- `heca/src/agents/installers/codex.rs`
- Documentation in `README.md`

**Acceptance**

- Plan documents exact Codex limitation.
- UI does not offer add-to-room for Codex if comms bridge unavailable.

## Phase 1.4 — Installer UI/actions

### Task 1.4.1 — Add setup/check action

**Description**

Add actions to check/install integrations.

**Actions**

- `agents.check_integrations`
- `agents.install_integrations`
- `agents.repair_integrations`

**Files**

- `heca/src/input.rs`
- `heca/src/actions.rs`
- `heca/src/app/registry.rs`
- `heca/src/handlers.rs`
- `heca/src/app/interaction.rs`
- `keybindings.default.toml` only if keyboard shortcut is desired

**Acceptance**

- Actions go through `ActionRegistry`.
- Policy classified as `Global` if no layout mutation.
- RPC support added when RPC is available for actions.

### Task 1.4.2 — Integration health report

**Description**

Produce a structured health report consumed by the dock UI.

**Example**

```text
Claude: status-ready ✅, comms-ready ✅
pi:     status-ready ✅, comms-ready ✅
Codex:  status-ready partial, comms-ready ❌
```

**Acceptance**

- Does not mutate config.
- Can be run safely many times.

---

# Feature 2 — Local Pane Agent Detection

## Goal

Detect and track agents running in heca panes. This is based on `agent-integration/agent-integration-plan.md`.

## Scope

- Only agents in heca panes.
- Ignore signals without `HECA_PANE_ID` in v1.
- Track status, cwd, harness, pane_id, last event.

## Dependencies

- Feature 1 status hooks/extensions/config.
- Existing pane runtime/chrome state infrastructure.
- `agent-integration/agent-integration-plan.md`.

## Phase 2.1 — Core status model

### Task 2.1.1 — Define `AgentStatus`

**Description**

Use the status vocabulary from `agent-integration/agent-integration-plan.md`.

**Suggested enum**

```rust
#[non_exhaustive]
pub enum AgentStatus {
    Working,
    WaitingForInput,
    WaitingForPermission,
    Finished,
    Error,
    Compacting,
    SubagentRunning,
}
```

**Files**

- Preferred: `heca-core/src/runtime.rs` or `heca-core/src/agents.rs` for pure data types.

**Acceptance**

- UI-free.
- Serializable if pane runtime/session persistence needs it.

### Task 2.1.2 — Define `LocalAgentState`

**Description**

Store status data linked to a pane.

**Suggested shape**

```rust
pub struct LocalAgentState {
    pub local_agent_id: LocalAgentId,
    pub harness: HarnessId,
    pub pane_id: PaneId,
    pub cwd: PathBuf,
    pub status: AgentStatus,
    pub detail: Option<String>,
    pub turn_id: Option<String>,
    pub comms: CommsReadiness,
}

pub enum CommsReadiness {
    Unknown,
    MonitorOnly,
    CommsReady { comms_agent_id: String },
    InRoom { comms_agent_id: String, room_id: AgentRoomId },
}
```

**Files**

- `heca-core/src/runtime.rs` for pane-attached status, or `heca/src/agents/state.rs` for app-level registry.

**Acceptance**

- `pane_id` is required in v1.
- No remote-only agent representation.

## Phase 2.2 — Receive status signals

### Task 2.2.1 — OSC status ingestion

**Description**

Extend/consume existing OSC snooper so status hooks emitted through terminal sequences update the
corresponding pane agent.

**Files**

- `heca-core/src/backend/terminal/osc.rs`
- `heca/src/app/agent_monitor.rs` new or equivalent app module

**References**

- `agent-integration/agent-integration-plan.md` OSC 9/99/777 sections.

**Acceptance**

- Status signal is attributed to exact pane.
- Malformed payloads are ignored with debug logging, not panics.

### Task 2.2.2 — Side-channel socket ingestion

**Description**

Receive structured JSON from hooks/extensions through `$HECA_AGENT_SOCKET`.

**Payload example**

```json
{
  "pane_id": 7,
  "harness": "claude",
  "cwd": "/repo/frontend",
  "status": "waiting_for_input",
  "detail": "idle_prompt",
  "turn_id": "abc"
}
```

**Files**

- New: `heca/src/agents/status_socket.rs`

**Acceptance**

- Ignores events without pane_id in v1.
- Updates registry through app event loop safely.

## Phase 2.3 — Local agent registry

### Task 2.3.1 — Implement `LocalAgentRegistry`

**Description**

Canonical app-side list of local pane-backed agents.

**Responsibilities**

- Upsert agent on status event.
- Remove/mark gone when pane closes.
- Track one-room membership.
- Track comms readiness.

**Files**

- New: `heca/src/agents/local_registry.rs`
- `heca/src/app_state.rs`

**Acceptance**

- No worker exists without pane_id.
- Closing pane removes or tombstones the agent.

### Task 2.3.2 — Emit chrome events

**Description**

Emit typed events when local agent state changes.

**Events**

- `ChromeEvent::LocalAgentChanged`
- `ChromeEvent::LocalAgentRemoved`
- `ChromeEvent::LocalAgentCommsChanged`

**Files**

- `heca/src/chrome/events.rs`
- `heca/src/chrome/state.rs`

**Acceptance**

- Providers can subscribe through existing App facade/event bus.

---

# Feature 3 — Agent-Comms Control Plane

## Goal

Use agent-comms as the communication/control layer for local agents that are comms-ready.

Heca uses a sidecar/control client to:

- create/list rooms;
- add/remove local agents to/from rooms;
- send/read room messages;
- observe room activity;
- determine whether a local agent is comms-ready.

## Scope

In scope:

- Node.js sidecar preferred over Rust rewrite.
- Local agent correlation only.
- Filter remote/unmatched peers out of v1 UI.

Out of scope:

- Remote agent management.
- Reimplementing agent-comms protocol in Rust.

## Dependencies

- agent-comms installed/available.
- Feature 1 comms bridge installation.
- Feature 2 local agent registry.

## Phase 3.1 — IPC protocol

### Task 3.1.1 — Define request/response protocol

**Description**

JSON newline-delimited IPC between heca and the Node sidecar.

**Example**

```json
{"id":1,"method":"list_rooms","params":{}}
{"id":1,"result":{"rooms":[]}}
{"event":"room_message","data":{"room_id":"release","from":"claude@pane7","content":"done"}}
```

**Files**

- New: `heca/src/agents/comms_protocol.rs`

**Acceptance**

- Typed serde structs.
- Roundtrip tests.

### Task 3.1.2 — Define control methods

**Methods**

- `health`
- `list_local_comms_agents`
- `find_comms_agent_for_pane { pane_id, local_agent_id }`
- `create_room { name }`
- `delete_room { room_id }`
- `list_rooms`
- `add_agent_to_room { room_id, comms_agent_id }`
- `remove_agent_from_room { room_id, comms_agent_id }`
- `set_orchestrator { room_id, comms_agent_id }`
- `send_room_message { room_id, content }`
- `send_dm { comms_agent_id, content }`
- `read_room_activity { room_id, limit }`

**Acceptance**

- No method creates abstract agents.
- Methods operate on existing comms-ready local agents.

## Phase 3.2 — Sidecar implementation

### Task 3.2.1 — Node sidecar skeleton

**Description**

Create sidecar wrapper around agent-comms.

**Files**

- New: `heca/assets/agent-comms-sidecar/package.json`
- New: `heca/assets/agent-comms-sidecar/index.ts`

**References**

- agent-comms `src/core/mesh-store.ts`
- agent-comms `src/core/tcp-transport.ts`
- agent-comms `src/core/wire-protocol.ts`

**Acceptance**

- Sidecar connects to local agent-comms mesh.
- Sidecar exposes IPC to heca.

### Task 3.2.2 — Local peer filtering/correlation

**Description**

Correlate agent-comms peers to local heca agents using metadata from integration config.

**Preferred correlation keys**

- `HECA_PANE_ID`
- `HECA_LOCAL_AGENT_ID`
- harness id
- cwd as fallback only

**Acceptance**

- Remote/unmatched peers are ignored in v1.
- Correlation is deterministic when pane/local_agent ids are present.

### Task 3.2.3 — Room and message event forwarding

**Description**

Forward agent-comms room and DM events to heca.

**Events**

- `room.created`
- `room.deleted`
- `room.member_added`
- `room.member_removed`
- `room.orchestrator_changed`
- `room.message`
- `dm.message`

**Acceptance**

- Events include room_id and comms_agent_id.
- Heca can update room/activity mirrors without polling.

## Phase 3.3 — Rust sidecar lifecycle

### Task 3.3.1 — Manage sidecar process

**Description**

Start/stop/restart the Node sidecar from heca.

**Files**

- New: `heca/src/agents/comms_sidecar.rs`
- `heca/src/app_state.rs`

**Acceptance**

- Non-blocking startup.
- Graceful shutdown.
- Backoff restart on crash.

### Task 3.3.2 — Sidecar health in app state

**Description**

Expose sidecar health to UI.

**States**

- Disabled
- Starting
- Connected
- Degraded
- Failed

**Acceptance**

- Rooms dock can show "agent-comms unavailable".

---

# Feature 4 — Running Agents Dock

## Goal

Show all local pane-backed agents detected by heca.

This is not the room UI. It is the inventory of running agents.

## Dependencies

- Feature 2 local agent registry.
- Plugin plan provider system / region hosts.
- `heca-grid-ui` widgets.

## Phase 4.1 — Provider skeleton

### Task 4.1.1 — Create `RunningAgentsProvider`

**Description**

Built-in dock/provider that lists local agents.

**Files**

- New: `heca/src/providers/running_agents.rs`

**Acceptance**

- Mountable in left/right sidebar region.
- Uses `DockFrame`, `ItemGroup`, `Item`, `Badge`, `Button`.
- No hardcoded styling.

## Phase 4.2 — Agent list UI

### Task 4.2.1 — Render local agent rows

**Row content**

- harness icon/name;
- pane label/id;
- cwd;
- status badge;
- comms readiness badge;
- room membership if any.

**Example**

```text
Claude Code  pane 7  /repo/frontend
WaitingForInput · comms-ready · room: release
```

**Acceptance**

- Shows monitor-only agents clearly.
- Shows in-room agents clearly.

### Task 4.2.2 — Agent row actions

**Actions**

- Focus pane.
- Create room from this agent.
- Add to existing room if comms-ready and not in room.
- Remove from room if in room.
- Restart/setup integration if monitor-only.

**Files**

- Provider UI file.
- Action handlers in `heca/src/handlers.rs` or provider action bridge.

**Acceptance**

- All state-changing actions route through `ActionRegistry`.

---

# Feature 5 — Rooms Model and Rooms Dock

## Goal

Create and manage rooms as local grouping/orchestration objects backed by agent-comms for messaging.

## Dependencies

- Feature 2 local agent registry.
- Feature 3 agent-comms control plane.
- Plugin provider system.

## Phase 5.1 — Room data model

### Task 5.1.1 — Define `AgentRoom`

**Suggested shape**

```rust
pub struct AgentRoom {
    pub id: AgentRoomId,
    pub name: String,
    pub members: Vec<LocalAgentId>,
    pub orchestrator: Option<LocalAgentId>,
    pub created_from_pane: Option<PaneId>,
}
```

**Important**

No cwd field.

**Files**

- New: `heca/src/agents/rooms.rs`

**Acceptance**

- Room can contain agents with different cwd.
- One active room per local agent enforced.

### Task 5.1.2 — Define `AgentRoomStore`

**Responsibilities**

- Create/delete room.
- Add/remove member.
- Set orchestrator.
- Enforce one-room-per-agent.
- Mirror agent-comms room ids if needed.

**Acceptance**

- Membership cannot include monitor-only agents unless room is explicitly `monitoring_only` future mode.

## Phase 5.2 — Rooms provider skeleton

### Task 5.2.1 — Create `AgentRoomsProvider`

**Description**

Built-in dock/provider for room list and room detail.

**Files**

- New: `heca/src/providers/agent_rooms.rs`

**Acceptance**

- Mountable in sidebar region.
- Reads `AgentRoomStore`, not raw session.

## Phase 5.3 — Create room flows

### Task 5.3.1 — Create room from agent/pane

**Description**

From a running agent row or pane action, create a room and optionally add that agent as initial member.

**Behavior**

- Room has no cwd.
- `created_from_pane` is metadata only.
- If initial agent is comms-ready, add it to agent-comms room.
- If initial agent is monitor-only, show integration required.

**Acceptance**

- Does not create a new pane.

### Task 5.3.2 — Create room from Rooms Dock

**Description**

Create empty room from dock.

**Fields**

- name
- optional description

No cwd selector in v1.

**Acceptance**

- Room can be created empty.
- Agents can be added later.

## Phase 5.4 — Add/remove agents

### Task 5.4.1 — Add running agent to room

**Description**

Choose from currently running local agents.

**Rules**

- Agent must be comms-ready.
- Agent must not already be in another room.
- Agent keeps its real pane cwd.

**UI**

Dropdown/list should show:

```text
Claude Code · pane 7 · /repo/frontend · WaitingForInput
Codex · pane 3 · /repo/backend · monitor-only (disabled)
```

**Acceptance**

- Disabled options explain why.
- Adding member calls agent-comms add/join via Feature 3.

### Task 5.4.2 — Remove agent from room

**Description**

Remove member from room and agent-comms room.

**Acceptance**

- Clears `room_id` on local agent.
- If removed agent was orchestrator, room orchestrator becomes None.

## Phase 5.5 — Orchestrator assignment

### Task 5.5.1 — Set orchestrator

**Description**

Choose one room member as orchestrator.

**Rules**

- Orchestrator must be a member.
- Orchestrator must be comms-ready.

**Acceptance**

- UI shows orchestrator badge.
- Changing orchestrator emits event.

---

# Feature 6 — Room Communication and Orchestration

## Goal

Use agent-comms to let room members communicate and to let the orchestrator assign tasks.

## Dependencies

- Feature 3 agent-comms control plane.
- Feature 5 room model/provider.

## Phase 6.1 — Room activity view

### Task 6.1.1 — Render room message/activity stream

**Content**

- room messages;
- task assignments;
- status transitions from local detection;
- membership changes.

**Acceptance**

- Activity stream separates status events from chat/task messages.

### Task 6.1.2 — Unread/activity badges

**Description**

Show unread/activity badges in room list and running agent rows.

**Acceptance**

- No hardcoded colors.

## Phase 6.2 — User/heca messages

### Task 6.2.1 — Send room message as user/heca

**Description**

Allow user to send a message into a room through heca sidecar.

**Acceptance**

- Uses agent-comms, not PTY input.
- Disabled if sidecar unavailable.

### Task 6.2.2 — Send DM to member

**Description**

Allow user to send DM to a comms-ready local agent.

**Acceptance**

- Uses agent-comms DM.
- Disabled for monitor-only agents.

## Phase 6.3 — Orchestrator task assignment

### Task 6.3.1 — Orchestrator-to-member task flow

**Description**

Model task assignment as room message/DM generated by orchestrator through agent-comms.

**Example**

```text
orchestrator claude@pane7 → codex@pane3:
"Run backend tests and report failures."
```

**Acceptance**

- Assignment appears in room activity.
- Target agent receives through agent-comms bridge.

### Task 6.3.2 — Task status projection

**Description**

Use local status detection to infer task progress.

**Examples**

- assigned → agent status Working
- waiting → WaitingForInput
- done → Finished
- error → Error

**Acceptance**

- Does not pretend perfect task tracking if harness lacks structured task IDs.

---

# Feature 7 — Room Workspace Organization

## Goal

Let a room organize its member panes into a workspace without changing cwd/processes.

This uses existing heca pane/workspace actions.

## Dependencies

- Existing `WmAction::MovePaneToWorkspace`.
- Feature 5 room membership.

## Phase 7.1 — Move room panes to workspace

### Task 7.1.1 — Move all room members to existing workspace

**Description**

Dispatch `MovePaneToWorkspace` for each room member pane.

**Files**

- `heca/src/input.rs` already has `MovePaneToWorkspace`.
- New handler/action wrapper may live in provider/action integration.

**Acceptance**

- Does not mutate room cwd because room has no cwd.
- Preserves pane processes.

### Task 7.1.2 — Create workspace for room and move members

**Description**

Create a workspace named after the room, then move all member panes there.

**Acceptance**

- Uses existing workspace creation/move actions through registry.
- If workspace exists, ask/choose reuse behavior.

## Phase 7.2 — Focus room workspace

### Task 7.2.1 — Focus workspace containing room members

**Description**

If room members are grouped in one workspace, focus it.

**Acceptance**

- No direct state mutation outside action handlers.

---

# Feature 8 — Future: Create New Agent From Room (v1.1)

## Goal

Allow creating a new pane/harness from a room without requiring cwd selection every time.

This is explicitly **not v1**.

## Design rule

Room still does not own cwd. Instead, room may have an optional `LaunchProfile` used only for future
agent creation.

```rust
pub struct LaunchProfile {
    pub cwd: PathBuf,
    pub harness: Option<HarnessId>,
    pub workspace_policy: WorkspacePolicy,
}
```

## Phase 8.1 — Launch profile creation

### Task 8.1.1 — Create launch profile from pane

**Description**

From an existing pane, save its cwd as launch profile for a room.

**Acceptance**

- Does not change existing members.

### Task 8.1.2 — Create launch profile from dock

**Description**

User chooses cwd once when setting up launch profile.

**Acceptance**

- No cwd prompt on each new agent creation.

## Phase 8.2 — Spawn new agent from room

### Task 8.2.1 — Spawn pane using launch profile

**Description**

Create pane, launch selected harness in launch profile cwd, with integration env/config active.

**Acceptance**

- Uses action registry.
- New agent appears in Running Agents Dock after detection.
- If comms-ready, user can add it to room.

---

## Backlog Summary

### v1 recommended sequence

1. Feature 1 — Harness Integration Installer
2. Feature 2 — Local Pane Agent Detection
3. Feature 3 — Agent-Comms Control Plane
4. Feature 4 — Running Agents Dock
5. Feature 5 — Rooms Model and Rooms Dock
6. Feature 6 — Room Communication and Orchestration
7. Feature 7 — Room Workspace Organization

### v1.1

8. Feature 8 — Create New Agent From Room via LaunchProfile

---

## Open Questions

1. Which harnesses must be comms-ready in v1?
   - Claude: likely yes via agent-comms MCP bridge.
   - pi: likely yes via extension/bridge.
   - Codex: needs confirmation.

2. Should monitor-only agents be allowed in rooms as non-communicating observers?
   - Current draft says no; room members must be comms-ready.

3. Where should installed harness configs live?
   - User-level configs (`~/.claude`, `~/.pi`, `~/.codex`) vs project-level configs.
   - Installer should support backup/restore either way.

4. Should heca auto-install integrations on first launch or require explicit user action?
   - Current draft recommends explicit action by default.

5. Should a room be allowed to be empty?
   - Current draft says yes.

6. Should deleting a room remove members from agent-comms room but keep panes running?
   - Current draft says yes.

---

## Non-Goals v1

- Remote agent management.
- Agent creation as abstract mesh peers.
- PTY-based room messaging as primary mechanism.
- Multiple room membership per agent.
- Room-level cwd.
- Automatic cwd selector for every new agent.
- Rust rewrite of agent-comms.
