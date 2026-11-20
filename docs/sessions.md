# Sessions — detach and reattach

heca runs as a background **server** that holds your session — the panes and the programs running
in them — and a **window** that attaches to it, the way tmux does.

- `heca` — attach to the last session. The first time, it starts the background server for you.
- `heca --sessions [--json]` — list names, folders, attached windows, running agents and lifecycle state.
- `heca --attach <name>` — open a window on an existing session.
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

Press `prefix+#` to open the session palette directly (default prefix: `Ctrl+b`). This uses the
registered `command_palette` action with `mode = "session"`, like `prefix+o` uses `mode = "pane"`.
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
and require explicit Restart after selection. Detach still allows reattaching to the same active
session, and the CLI session inventory keeps all retained sessions.
Each window remembers its workspace, focused pane and scroll separately for every session.
Returning to a session restores that window's view without changing another window's focus.

The attach response carries `session` (`SessionInfo`), `client_id`, `window_ordinal` and `content`.
`session.id` is the existing `SessionId`; `session.folder` is the optional project path. Client IDs
identify physical connections and remain unchanged when switching sessions. Display ordinals start
at one within each session, permit gaps and remain stable through a restart of that session.

Each window titles itself from the program's name, the session folder and its ordinal: the first
window on a session reads `heca - ~/projects/heca` and each later one adds its number —
`heca - ~/projects/heca - #2`. The two formats are `[settings.session_text] window_title` (default
`{app} - {folder}`) and `window_title_numbered` (default `{app} - {folder} - #{number}`); `{app}` is
the program's fixed name (`heca`, or `Heca Pro` for heca-pro — given by the program to
`heca::run(..)`, not a setting), `{folder}` the folder shown home-relative, or the session name when
it has none. Before it attaches to a session a window is titled with the program's name alone. The window shows its title in the top bar and reports it to the server, which
keeps a per-session window directory (each window's id, ordinal and title) and sends it to every
window on join, leave and title change — the list a follow picker chooses a target from.

A window can **follow** another: open the command palette and switch to the follow mode (`~`), then
pick a window by its title. The follower mirrors where the leader is looking — the shown workspace,
the focused pane and the scroll — while keeping its own size. The leader relays its view through the
server; the follower resumes its own view the moment you touch its keyboard or mouse, or when the
followed window leaves. A target is stored by id, so renaming the leader never breaks the follow.

Session actions use the same catalog as keys, plugins and RPC:
`list_sessions`, `kill_server`, `create_session`, `rename_session_to`, `terminate_session`, `restart_session`, `delete_session`,
`switch_session`, and `detach_session`. `new_session` and `rename_session` open the GUI prompts.
Use `heca --describe-action <action>` for arguments. For example:

```bash
heca --new ~/projects/demo --name demo
heca rpc list_sessions
heca rpc rename_session_to session=demo name=work
heca --kill work
heca rpc restart_session session=work
heca rpc delete_session session=work
```

Detach leaves the session and processes running and removes only this window's attachment.
Terminate (`--kill <name>`) stops the processes but keeps the session definition. Delete removes
the definition; project files are never removed. All three leave affected windows open with
choices for an existing session, a new session, and Exit. Terminate also offers explicit Restart.
Canceling a picker returns to those choices. Server loss leaves windows open for reconnect or exit;
disconnected windows do not restart the server automatically.

Sessions are saved and come back when the server starts again: name, folder, layout, key, and what
ran in each pane. A terminated session stays terminated until Restart; a deleted one is gone.
`save_sessions = false` keeps nothing. `resume_programs` and `resume_harness` are independent
`[settings]` switches, both defaulting to true, for relaunching ordinary programs (with their exact
arguments) and resuming harnesses through their driver and native session ID. A disabled category,
and a harness that cannot be resumed (no ID, or a driver without resume), leaves the pane with your
normal shell; a harness is never started as a new conversation, and you are told. A missing folder
gives a shell in your home folder. See
[Server and client](server-client-architecture.md#sessions-across-a-restart).
Session dialog text is configurable under `[settings.session_text]`.
