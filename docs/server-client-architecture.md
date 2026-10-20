# Server and client

How heca's state is split between the **server** — the content every window shares — and the
**client** — one window's view of it — and the one seam between them.

This is the model F012 builds on (accepted decisions
[`d108b88c`](#decisions), [`463ccd07`](#decisions), [`2c249a29`](#decisions)). It is being built in
one process first: standalone heca is the server **and** one client in one process, so there is one
code path, and the socket for several windows / detach-attach comes later (P099).

- [The model](#the-model)
- [What is shared, what is per window](#what-is-shared-what-is-per-window)
- [The seam](#the-seam)
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
setting (`smallest` | `latest` | `largest`, default `latest` — decision `463ccd07`). That
arbitration lives on the resize event and is added with the socket (P099); the read above stays a
pure read and takes no grid, so it does not foreclose it.

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
