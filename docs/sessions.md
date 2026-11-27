# Sessions — detach and reattach

heca runs as a background **server** that holds your session — the panes and the programs running
in them — and a **window** that attaches to it, the way tmux does.

- `heca` — attach to the last session. The first time, it starts the background server for you.
- `heca --sessions [--json]` — list names, folders, attached windows, running agents and lifecycle state.
- `heca --attach <name>` — open a window on an existing session.
- **`prefix+Ctrl+w`** (`new_window`) — another window of the same heca, on this window's session.
  It is one program with several windows (one Dock icon on macOS), each its own connection to the
  server: its own workspace, focus and scroll, and its own session — switch one with `prefix+#` and
  the others stay where they are. Closing a window closes only it; heca ends with its last window.
  On Windows, where each window runs its server inside itself, there is no shared server for a
  second window to join, so the key says so instead.
- `heca --new [path] [--name <name>]` — create a session. The folder defaults to the caller's
  current folder; relative paths resolve there, and missing folders are created. An omitted name
  uses `[settings] session_name_format` (default `heca #{number}`). Names are unique.
- **Close the window** (e.g. `Meta+Q`) — the window detaches; the server and everything running in
  it keep going.
- `heca` again — **reattach**: the same panes come back, still running, with their scrollback.
- `heca --standalone` — a one-off session that lives only in its window and ends when you close it.
  It does **not** attach to the background server.

To stop the background server and everything in it — every pane and terminal, like tmux's
`kill-server` — and detach every window:

```bash
heca --kill-server
heca rpc kill_server      # the same thing, from a script
```

Both run the `kill_server` action, the one way to stop the server. It is also in the command
palette ("Kill Server") and can be bound to a key in `[keys]`; no key is bound by default. An
action policy can forbid it for the user; a request another service hands in can never run it.
(`heca rpc kill-server`, the old spelling, is gone.)

Press `prefix+#` to open the session palette directly (default prefix: `Ctrl+b`). The key runs
`list_sessions` — the same action as the palette's "Sessions" row and `heca rpc list_sessions`; in a
window it opens the palette on the `#` list.
The command palette's `#` mode lists sessions and switches this window only; type a name **or part
of its folder** to find one (a name match is listed first). The New Session
action uses fuzzy folder completion: relative paths start at the focused pane's folder (or home),
and a missing folder requires choosing the explicit Create folder row. Tab completes the selected
path without creating it or closing the picker; Enter confirms. The binding is configurable as
`[keys.widgets] complete` (default `Tab`). An empty folder chooses home.
Folder matching is fuzzy and defaults to case-insensitive, including capitalized paths inserted
by completion. `[settings] folder_search_case` accepts `insensitive` (default), `smart`, or
`sensitive`, independently of `search_case`. Matching existing folders precedes the explicit
creation option; an unmatched path still offers creation.
Session rows keep the name, abbreviated path and window/agent counts on separate lines. The full
path is available in its tooltip.
The current terminated session is not offered as its own alternative in the session picker;
its unavailable dialog already offers explicit Restart. Other stopped sessions remain listed
and require explicit Restart — their row's ⋯ offers it. Detach still allows reattaching to the same active
session, and the CLI session inventory keeps all retained sessions.
Each window remembers its workspace, focused pane and scroll separately for every session.
Returning to a session restores that window's view without changing another window's focus.

The attach response carries `session` (`SessionInfo`), `client_id`, `window_ordinal` and `content`.
`session.id` is the existing `SessionId`; `session.folder` is the optional project path. Client IDs
identify physical connections and remain unchanged when switching sessions. A window's display
ordinal is the lowest number no window on that session holds when it joins (a window alone is the
first); it keeps it while it stays, whoever joins or leaves, and through a restart of the session.

Each window titles itself from the program's name, the session's name and folder, and its ordinal:
the first window on a session reads `heca - work (~/projects/heca)` and each later one adds its
number — `heca - work (~/projects/heca) - #2`. The two formats are `[settings.session_text]
window_title` (default `{app} - {session} ({folder})`) and `window_title_numbered` (default
`{app} - {session} ({folder}) - #{number}`); `{app}` is the program's fixed name (`heca`, or
`Heca Pro` for heca-pro — given by the program to `heca::run(..)`, not a setting), `{session}` the
session's name, `{folder}` the folder shown home-relative, or the session name when it has none. The
title follows a rename. Before it attaches to a session a window is titled with the program's name alone. The window shows its title in the top bar and reports it to the server, which
keeps a per-session window directory (each window's id, ordinal and title) and sends it to every
window on join, leave and title change — the list a follow picker chooses a target from.

A window can **follow** another: open the command palette and switch to the follow mode (`~`), then
pick a window by its title. The follower mirrors where the leader is looking — the shown workspace,
the focused pane and the scroll — while keeping its own size. The leader relays its view through the
server; the follower resumes its own view the moment you touch its keyboard or mouse, or when the
followed window leaves. A target is stored by id, so renaming the leader never breaks the follow.

Session actions use the same catalog as keys, plugins and RPC — one name, judged and confirmed the
same way wherever it comes from. `heca --describe-action <action>` prints the same arguments:

| Action | Arguments | What it does |
|---|---|---|
| `list_sessions` | — | In a window, opens the `#` list (`prefix+#`); over RPC, the sessions as JSON. |
| `new_session` | — | Asks for a folder and creates a session there. |
| `create_session` | `folder` (required), `name`, `label` | Creates a session in an absolute folder (created if missing). |
| `rename_session` | `session` | Opens the rename dialog for `session` (default: this window's). |
| `rename_session_to` | `session`, `name` (required) | Renames `session` (default: this window's). |
| `switch_session` | `name` (required) | Attaches this window to that session. |
| `detach_session` | — | Leaves this session running; the window offers another, a new one, or exit. |
| `terminate_session` | `session` | Stops its programs and keeps it for Restart. Its windows move to the most recent running session. |
| `restart_session` | `session` | Starts a terminated session again. |
| `delete_session` | `session` | Removes it (never the project files). Its windows move to the most recent running session. |
| `reconnect_session` | — | Starts a missing server and reconnects this window to its session. |
| `new_window` | `session` | Opens another window (`prefix+Ctrl+w`) on the same server, on `session` or this window's. |
| `close_window` | — | Closes this window (`Cmd+W` / `Super+w`); its session keeps running, and heca ends with its last window. |
| `last_session` | — | Back to the session this window showed before (`prefix+Ctrl+s Tab`); again, and it comes back. |
| `kill_server` | — | Stops the server and every session. |

`session` defaults to the window's own session (for unattached RPC, the last session used). For
example:

```bash
heca --new ~/projects/demo --name demo
heca rpc list_sessions
heca rpc rename_session_to session=demo name=work
heca --kill work
heca rpc restart_session session=work
heca rpc delete_session session=work
```

Detach leaves the session and processes running and removes only this window's attachment, and
the window offers another session, a new one, or Exit. Terminate (`--kill <name>`) stops the
processes but keeps the session definition; Delete removes the definition; project files are never
removed. **A window whose session is terminated or deleted moves to the most recent session still
running**, and says so in its bottom bar (`[settings.session_text] moved_terminated` /
`moved_deleted`). Only when no running session is left does it stay and offer Restart (for a
terminated one), another session, a new session, or Exit; canceling a picker returns to those
choices. A plain `heca` follows the same rule: it attaches to the most recent running session and
skips a terminated one, even the last used; with nothing running it starts a new session (the
terminated ones stay in the list for Restart).

**Acting on a session from its row.** In the `#` list (`prefix+#`) the selected row ends in a ⋯;
it, a right-click on the row, and `prefix+>` open the row's menu: Rename (the rename dialog, with the
session's name to edit), Terminate or Restart as the session's state allows, and Delete. The current session is
offered too. Each entry is the action above with that session named, so Delete asks the same
confirmation as `heca rpc delete_session`. Server loss leaves windows open for reconnect or exit;
disconnected windows do not restart the server automatically.

Sessions are saved and come back when the server starts again: name, folder, layout, key, and what
ran in each pane. A terminated session stays terminated until Restart; a deleted one is gone.
`save_sessions = false` keeps nothing. `resume_programs` and `resume_harness` are independent
`[settings]` switches, both defaulting to true, for relaunching ordinary programs (with their exact
arguments) and resuming harnesses through their driver and native session ID. A disabled category,
and a harness that cannot be resumed (it never reported, or a driver without resume), leaves the pane
with your normal shell; a harness is never started as a new conversation in place of one, and you are
told. A harness that never had a conversation (nothing typed in Codex or OpenCode yet) simply starts
again as it was. A program heca brought back (an agent, or `nvim` typed in a shell) runs as the pane's
own program, so when it ends the pane gets your shell in its folder — as quitting it did before the
restart — unless the pane's close policy closes it. A missing folder
gives a shell in your home folder. See
[Server and client](server-client-architecture.md#sessions-across-a-restart).
Session dialog text is configurable under `[settings.session_text]`.

**A new pane starts where you are.** A pane made from a pane — a split (`split_horizontal`,
`split_vertical`), a pane added to a column, a new column — opens its shell in the folder the
focused pane is in at that moment (what its shell last reported); with no focused pane, or no folder
known for it, the shell starts in its default folder. A new workspace is not made from a pane: its
shell starts in `[settings] new_workspace_cwd` (default `"~"`, the home folder; a path, `~` or `~/…`),
and a folder that does not exist falls back to the home folder with a notice.
