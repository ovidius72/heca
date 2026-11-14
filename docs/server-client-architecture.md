# Server and client

How heca's state is split between the **server** — the content every window shares — and the
**client** — one window's view of it — and the one seam between them.

This is the model F012 builds on (accepted decisions
[`d108b88c`](#decisions), [`463ccd07`](#decisions), [`2c249a29`](#decisions)). Standalone heca is the
server **and** one client in one process, so there is one code path; the **same seam also runs over a
local socket**, so a window can detach and reattach and a session outlives its window (P099).

- [The model](#the-model)
- [What is shared, what is per window](#what-is-shared-what-is-per-window)
- [The seam](#the-seam)
- [The connection — a local socket, detach and attach](#the-connection--a-local-socket-detach-and-attach)
  — [socket](#the-socket-and-the-runtime-folder), [scripts](#scripts--heca-rpc),
  [attach and detach](#the-background-server-attach-and-detach),
  [several windows](#several-windows-on-one-session), [wire model](#the-wire-model)
- [Extension services](#extension-services)
- [Per-window state is keyed by id, resolved at read time](#per-window-state-is-keyed-by-id-resolved-at-read-time)
- [Terminals](#terminals)
- [Decisions](#decisions)

---

## The model

> The **server** holds the content: every session's workspaces, columns, panes, the terminal
> processes behind them, notifications. A **window is a client** attached to one session; it draws
> everything — terminals, sidebars, docks, overlays — from what the server sends. **The server sends
> state and changes, never interface.**

A window closing leaves its session and its programs running; another window can attach to the same
session and see the same content. Two windows on one session share that content and **move
independently**.

**Standalone = server + one client in one process.** Everything the server owns is reached the one
way — `state.server.<part>` — whether the caller is this window or, later, a connection. The server
is built and driven **without a window**: `ServerState` (`heca/src/server/mod.rs`) is the state that
cannot need one.

---

## What is shared, what is per window

| Shared — the **server** owns it | Per window — the **client** owns it |
| --- | --- |
| Workspaces, columns, panes (`Session`) | Which workspace is shown; the switch animation |
| Which panes float, and where each floated from | Where each float is drawn, how big, which is active |
| Terminal processes (`BackendStore`) | The scroll offset and active column of each workspace |
| Notifications, git facts, the program catalog | Focused pane; tiled-vs-floating focus (`focus_domain`) |
| | The window's viewport size, scale, and device cell size |
| | Open overlays / menus / palette; which docks show where |

A split made in one window appears in the other (shared content); the workspace you are looking at,
the pane you have focused and how far you have scrolled are yours alone (per-window view).

The per-window view lives in `WindowView` / `ScrollView` (`heca-core/src/layout/window_view.rs`),
held by the client. Layout math is shared and **takes the view as an argument**: a caller holds a
`Layout` (reads) or `LayoutMut` (moves) made with `session.through(&view)` /
`session.through_mut(&mut view)` and never has to pass a view along — `layout.focus_right()` moves
the focus *and* the scroll that follows it, in the window that asked. The server never reads a
window's view; it reads what it needs from the **Asker** (see below).

---

## The seam

One interface, two directions, and neither side sends the other any interface:

```text
client ──(action / input + Asker)──▶ server     server mutates the shared content
client ◀──(Change: a past fact)──── server     the window reacts over its own view
client ◀──(TerminalSnapshot + TerminalDamage)── server     the window draws the terminal
```

- **The client sends an action and an `Asker`.** The `Asker` (`heca/src/server/cx.rs`) is the plain
  data a server action needs that only the window knows — its focused pane, the workspace it shows,
  the active column, the **area it lays that workspace out in**, and its terminal **cell size**.
  With several windows each request carries its own, so two windows asking at once are two askers;
  the server never reaches into a window to find any of it.
- **The server replies with `Change`s** (`heca/src/server/change.rs`) — *past facts* about the
  content (`PaneAdded`, `PaneMoved`, `Floated`, `WorkspaceRemoved`, `Arranged`, …), never an
  instruction. The window does what only a window can: it reacts over its own view
  (`heca/src/app_state/reaction/mod.rs` — `window_reacts`), deciding for itself which workspace to
  show, which pane to focus, how to slide the change in from where things were.
- **A terminal's screen** is read the same way — not by reaching into the server's backends but
  through `ServerState::terminal_view(id, scale)`, which hands back the `TerminalSnapshot` and the
  rows changed since last asked (`TerminalDamage`). See [Terminals](#terminals).

A `Change` says *what changed*, never *what to do about it* — the same rule events follow everywhere
else in heca.

---

## The connection — a local socket, detach and attach

The seam above also runs **over a local socket**, so a session outlives its window (P099). It is the
same interface carried on a wire; nothing new is invented for it. The next sections say who may
connect, what a script can do, how a window attaches and detaches, and what travels on the wire.

- [The socket and the runtime folder](#the-socket-and-the-runtime-folder)
- [Scripts — `heca rpc`](#scripts--heca-rpc)
- [The background server, attach and detach](#the-background-server-attach-and-detach)
- [Several windows on one session](#several-windows-on-one-session)
- [The wire model](#the-wire-model)

### The socket and the runtime folder

**Who may connect.** Only the user who owns the socket. The server never touches the network;
clients from another computer (which need login and encryption) are later work. Unix only; Windows
(named pipes) is a later step.

**Where it lives.** In a folder called `heca` inside the user's runtime folder: `$XDG_RUNTIME_DIR/heca/`
where the platform has one (Linux), otherwise the per-user temp folder (macOS has no runtime folder;
its temp folder is already per user). The socket is `default.sock` in that folder, with a `default.lock`
file beside it. One socket for now; sessions give it a name later (P100). If the platform gives no
folder heca can write to, the socket is simply not served and heca still runs standalone.
Code: `heca/src/server/socket/mod.rs` (`runtime_dir`, `socket_path`).

**How it is kept private.** The folder is created owner-only (`0700`) and its permissions are set
again each time; if something other than a real folder is in its place, heca refuses it. The socket
itself is set to `0600` once bound. Code: `heca/src/server/socket/mod.rs`, `listener.rs`.

**One server per socket, and stale sockets.** The server takes an exclusive lock on `default.lock`
before it binds; a second server cannot start on the same socket. If the socket file is already
there, heca tries to connect to it: if nothing is listening (connection refused) it is a leftover from
a crash, so it is deleted and bound again. If something answers, or the answer is unclear, the file
is **never** removed and the second server fails to start. When a server stops it removes its socket,
but only if the file is still the one it created. Code: `heca/src/server/socket/listener.rs`.

**One door, three kinds of visitor.** The listener reads the first line of each connection. That line
is `@heca-attach 1` (a window attaching — see below), `@heca-events 1` (a script subscribing to
events) or anything else, which is a one-shot script request. The line is limited to
`max_frame_bytes` and must arrive within `request_timeout_ms`, so a silent peer cannot hold a
handshake thread. Each connection is handled on its own thread, so a slow one never delays the next.
Code: `heca/src/server/socket/runtime.rs`.

### Scripts — `heca rpc`

**`heca rpc <line>`** sends one line to the running server and prints the answer. It never opens a
window. The line is an action by its catalog name and arguments, exactly what
[`rpc.rs`](../heca/src/rpc.rs) reads (`split_horizontal`, `focus_pane 42`, `place_pane 5 0 1 pane_idx=0`),
so every registered action — built in or from a plugin — is reachable with nothing added per action.
Two queries return JSON instead of running anything: `list-actions` and `describe-action <name>`.

- **A good line** answers `ok`, or the action's output (for example the session list as JSON).
- **A bad line** answers `error: …` (unknown action, missing argument, not allowed right now).
- **No line** prints a usage message. **No server running** prints `could not reach a running heca`
  and `(is heca running?)`; it does not start one.
- The request must be answered within `request_timeout_ms`.

**Who runs the line.** A service's action runs in the service. A session-management action
(create, rename, list) runs in the server. Otherwise the line goes to the most recently used session
that has a window attached, and that window runs it and sends the answer back. With no window
attached, the server runs it itself in the most recent session. `heca rpc kill-server` — which is what
`heca --kill-server` sends — stops the server and every pane and terminal. Code:
`heca/src/server/socket/mod.rs` (`run_rpc_cli`, `request`), `heca/src/server/run/mod.rs`
(`begin_rpc`), `heca/src/server/run/rpc.rs`; the command line is declared in
`heca/src/app/cli/catalog.rs`.

**`heca rpc --events`** subscribes instead of asking once. It connects, sends the preface
`@heca-events 1`, then prints one JSON line per event until the script closes it (or the server
stops). It is read-only, has no time limit, and sees the whole server, every session. Nothing is
sent when nobody is subscribed. A script that falls more than `queue_capacity` events behind is
disconnected. Each line is one `StreamEvent` (`heca/src/server/events.rs`), written
as a JSON object named by its kind:

| Kind | Meaning | Carries |
| --- | --- | --- |
| `Change` | A shared change to the session content — the same `Change` a window reacts to | the change |
| `Focus` | A window's focused pane changed | `window`, `workspace`, `pane` |
| `Workspace` | A window's shown workspace changed | `window`, `workspace` |
| `Session` | A window attached to, or switched to, a session | `window`, `session`, `name` |

**The per-window tag.** `Focus`, `Workspace` and `Session` carry `window`: the window's connection id,
the same id the window directory lists. A script uses it to tell windows apart. `Change` has no tag,
because it is about the shared content and is the same for every window. `Focus` and `Workspace` come
from each window reporting where it is looking and are sent only when the value moves.

### The background server, attach and detach

**The server runs without a window.** `heca --server` (`heca/src/server/runtime/headless.rs`, `serve`)
loads the config, binds the socket, starts the extension services and runs the server loop. It needs
no display and no GPU. `heca --kill-server` stops it.

**A window attaches.** Plain `heca` is a pure client (`ClientState::connect`,
`heca/src/client/mod.rs`):

1. It connects to the socket (`heca/src/client/transport.rs`). If nothing answers it starts
   `heca --server` as a detached daemon, then retries every `connect_retry_ms` until the socket
   answers, the daemon exits, or `request_timeout_ms` passes.
2. It sends `@heca-attach 1`, then `ClientMsg::Attach` — or `AttachTo { name }` for
   `heca --attach <name>`. `heca --new` first asks the server to create the session, then attaches to it.
3. The server replies `ServerMsg::Attached`: the session, the window's id and number, and the whole
   shared content. The window rebuilds its own `Session` from it.
4. The server then sends the notifications, the feedback state, the window directory and one
   terminal frame per terminal, and ends with `ServerMsg::ProjectionReady`. Until that arrives the
   window keeps its last complete picture and ignores actions, so it never acts on half a session. If
   it does not arrive within `request_timeout_ms`, the window treats the connection as failed.

**Detach keeps everything running.** Closing a window sends `ClientMsg::Detach`
(`ClientState`'s drop) and the server forgets that window. The session, its programs and their
scrollback stay as they were; the next `heca` attaches to the session and finds them live. A window
that dies without saying so is treated the same way when its socket closes.

**`LeaveSession`** (the `detach_session` action) is smaller: the window stays connected but leaves its
session. It gets `SessionDetached`, shows no session, and may only manage sessions until it attaches
to one again. `switch_session` sends `AttachTo`, which moves the window to another session on the same
connection — its window id does not change. Code: `heca/src/server/run/mod.rs` (`pump`),
`heca/src/client/sessions.rs`.

**One code path.** `heca --standalone` is the same server plus one client in one process
(`heca/src/client/local.rs`). `ClientState::connect` builds a `LocalServer` and joins client and
server with `in_process_with_limits` (`heca/src/server/wire/mod.rs`) instead of the socket; from there the
attach above is identical, and neither side knows which link it holds. This is decision `d108b88c`.
Standalone does **not** attach to the background server and ends with its window. It does try to
serve the same socket, so scripts can reach it; if a background server already holds the socket, that
fails with a message on stderr and standalone runs without a socket.

### Several windows on one session

Any number of windows can attach to the same session (decisions `463ccd07`, `2c249a29`). They share the
content: a split in one appears in the others. Each keeps its own view, because the server holds no
per-window view and every request brings its own `Asker`.

- **Window directory.** The server keeps the list of windows on each session and sends it
  (`ServerMsg::Windows`) to every window of that session after each join, leave and title change.
- **Titles.** Each window formats its own title and reports it (`ClientMsg::ReportWindowTitle`). The
  first window on a session has no number; each later one is numbered. Both formats are in
  `[settings.session_text]` (`window_title = "{folder}"`,
  `window_title_numbered = "{folder} #{number}"`), so two windows read `~/projects/heca` and
  `~/projects/heca #2`. `{folder}` is the session folder shown from home, or the session name when it
  has none.
- **`follow`.** The `follow` action (a picker of the other windows) makes a window **mirror** another
  one: it shows the same workspace and scroll position, but keeps its own size and scale. It sends
  `ClientMsg::Follow { target }`; `None` stops. The server tells the followed window
  (`ServerMsg::Followed`) that it is followed, and only then does that window report its view
  (`ReportView`); the server relays it to each follower (`FollowView`) and sends the leader's last
  view at once, so following is not blank. If the followed window leaves, the follower goes back to its
  own view. The top bar shows the window you follow after your own title.
- **Pane size.** A running terminal has one size, but two windows may show it at different sizes. The
  setting `pane_size` in `[settings]` decides (default **`latest`**, in `config.default.toml` and
  `heca-config/src/settings/enums.rs`, `PaneSize`):
  - `smallest` — the smallest showing window, so the pane fits in every window.
  - `latest` — the window last typed in or resized, so the window you work in gets the full size.
  - `largest` — the largest showing window.

  A pane shown by one window always takes that window's size. Only rows and columns are shared; each
  window still draws with its own font. When a window leaves, the pane is sized again for the windows
  that remain. Code: `heca/src/server/sizing.rs`, `heca/src/server/stream.rs`.

### The wire model

**Plain data only.** Everything that crosses the seam is a serializable value in
`heca/src/server/wire/`: `ClientMsg` (what a window asks) and `ServerMsg` (what the server tells it).
No handles, no window types, no types from the terminal library.

- **Terminals.** A frame (`wire/terminal.rs`, `TerminalFrame`) carries a `WireSnapshot` — the same
  cell, line and cursor types the renderer draws from — plus the changed rows, the title and the
  pane's runtime facts. Input back is plain too (`ProcessInput`: bytes, key, mouse, paste, focus,
  resize).
- **Images.** A snapshot never holds pixels. Its placements name an image by a content id; the
  window asks for it (`ImageInfo`, then `ImageChunk`s of RGBA data) and puts it together
  (`wire/image.rs`, `heca/src/client/images.rs`). Chunks are sized to fit one frame; an image whose
  size or dimensions do not add up, or that exceeds `max_image_bytes`, is refused.
- **Serialization is a transport detail.** Over the socket each message is one **frame**: a 4-byte
  length (big-endian) followed by JSON (`wire/frame.rs`). A zero length, or one over
  `max_frame_bytes`, closes the connection. **In process** (standalone) the same values move through
  a queue with no serialization at all (`wire/mod.rs`).
- **A stalled peer is dropped, never waited on.** Each socket end has a reader thread and a writer
  thread (`wire/socket.rs`) and a queue per direction of at most `queue_capacity` messages
  (`wire/queue.rs`). The server only ever polls these queues. If a queue is full, the send fails and
  the connection is closed.
- **Newer state replaces older.** When a message would overtake one still waiting in the queue, it
  takes its place instead of adding to the line. This applies to: `Content` (whole content), `Notifications`
  (whole snapshot), `Feedback` for the same session, `Terminal` for the same terminal at the same
  scroll offset, and `Changed` of `LayoutChanged`, `NamesChanged` or `NotificationsChanged`. Everything
  else — input, actions, other `Changed` facts, replies — keeps its order and is never merged away.
  Replacement never crosses a **boundary**: `Attached`, `ProjectionReady`, `SessionTerminated`,
  `SessionDeleted` and `SessionDetached`. So a session's snapshot can never be overwritten by state
  from the session before or after it.

**The limits** are in `[settings.connection]` (`heca-config/src/settings/connection.rs`, defaults in
`config.default.toml`). All must be above zero. They apply to each connection:

| Key | Default | What it limits |
| --- | --- | --- |
| `max_frame_bytes` | 16777216 (16 MiB) | One message, either direction; also a script's request line |
| `queue_capacity` | 64 | Queued messages per direction; queued events per `heca rpc --events` subscriber |
| `request_timeout_ms` | 5000 | Identifying a peer, a one-shot answer, flushing on close, waiting for a server to start and for a first attach |
| `connect_retry_ms` | 20 | Pause between connection attempts while starting the background server |
| `max_image_bytes` | 67108864 (64 MiB) | One decoded image, all animation frames |
| `max_terminal_cells` | 262144 | Columns × rows a connection may ask a terminal to have |

---

## Extension services

An extension can run a long-lived **service** inside the server (see
[A service inside the server](extending-heca.md#a-service-inside-the-server)). The server owns one
host for all of them (`heca/src/server/services/`) and shares it with its sessions: an action from a
window reaches the service through the session that received it, an action from a script through the
server. Services start once when the server starts and stop once when it shuts down, so detaching a
window or switching session never touches them.

**Two messages carry them.** `ServerMsg::Service` sends one window one update (the service's name,
its schema version, the server epoch, a revision, and a snapshot, a delta or a removal).
`ClientMsg::ServiceResync` is the window asking for a fresh snapshot of one service.

**Each window is shown its own projection.** After a service says its data changed, the server asks it
once per attached window (its session and connection) what that window shows, and remembers the
revision it last sent each one. A delta names the revision it builds on; if the window does not hold
exactly that, it keeps what it has, sends `ServiceResync`, and the server starts that window again from a
snapshot. The **server epoch** is a number for this run of the server: a window holding data from
another epoch cannot apply a delta, so a restarted server always begins with snapshots. Attaching or
reattaching also clears what the server remembers for that window, and a window that changes session is
sent a snapshot for its new session.

---

## Per-window state is keyed by id, resolved at read time

When a piece of state is per window, it moves out of the shared content into the window's view
**keyed by id, not by index or position**, and is resolved against the current content when read —
so content another window changes never leaves this window pointing at the wrong thing. The pattern,
built across T525:

- **The shown workspace** is a `WorkspaceId`, not an index (C9). A workspace another window removes
  renumbers nothing here: the id still resolves, or falls back to the first.
- **Each column's focused pane** is a `HashMap<ColumnId, PaneId>` in `ScrollView` (C12), resolved
  against the column's current panes and falling back to its first; `retain_columns` forgets columns
  the content no longer has.
- **Floating panes** (C10): the shared `FloatingPane` is content only (`pane`, `origin`); where a
  window draws each float (`HashMap<PaneId, Rectangle>`), which float it has active
  (`Option<PaneId>`) and whether it is focused on the tiling or the floats (`focus_domain`) all live
  in `ScrollView`. A float with no stored rect falls back to the centred default; `retain_floats`
  forgets floats the content no longer has. The server never sets any of this — it emits
  `Change::Floated { pane, workspace, rect }` and the window places and focuses the float when it
  reacts; another window gets only the content and draws it at its own default.

The read handles are the one door: `WorkspaceRef` / `WorkspaceMut` resolve the view against the
content (`active_pane()`, `floating()`, `focus_domain()`, `active_float()`), so a render or exposé
pass never reaches for geometry the shared content does not carry.

---

## Terminals

The server owns the process; the window owns the size and draws the pixels.

- **The window's size reaches the process** as a `TerminalInput::Resize(grid)` the window emits only
  when its box or font changes (`chrome/panes/terminal/shared.rs::report_grid` →
  `terminal_host/input.rs::on_resize`), which resizes the backend. So **rows×cols are the window's**.
- **The window reads the screen** through `ServerState::terminal_view(id, scale)`
  (`heca/src/server/mod.rs`): the server sets the device **cell size / scale** (the window's) and
  returns the `TerminalSnapshot` plus the rows changed since last asked. The client builds its
  `TerminalMount` from that; it does not reach into `state.server.backends`.

When several windows show one pane at different sizes the process still has **one** size, chosen by a
setting (`smallest` | `latest` | `largest`, default `latest` — decision `463ccd07`). The server
applies it when a window resizes or types (`heca/src/server/sizing.rs`); the read above stays a pure
read and takes no grid. See [Several windows on one session](#several-windows-on-one-session).

---

## Decisions

The accepted decisions behind this model (in the planner, feature F012):

- **`d108b88c`** — heca runs standalone or as a server; the server holds every session, a window is a
  client. The server sends state and changes, never interface. Standalone = server + one client in
  one process.
- **`463ccd07`** — two windows on one session share the content and move independently; a running
  terminal has one size, chosen by a setting (default `latest`).
- **`2c249a29`** — a window's view lives in the window; the server only remembers the last view. The
  shared content stays in the session; `ViewOffset`, shown workspace, switch, viewport and scale move
  into a `WindowView` the layout takes by reference.

See also: [`layout.md`](layout.md) for how a caller arranges widgets, and
[`architecture.md`](architecture.md) for the scrolling layout engine.
