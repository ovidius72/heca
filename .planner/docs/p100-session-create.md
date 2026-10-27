# P100 — Session create (CLI + palette) and default naming

Agreed with Antonio 2026-10-27. Refines decision `f8ccde05`'s `heca --new <name> [folder]`.
Owner: P100(F012)/T522. The window-title part is built in P099(F012)/T545 but kept here so the
naming stays in one place.

## CLI

`heca --new [path] [--name <name>]`

- **`[path]`** — the project folder.
  - omitted → the current working directory;
  - relative (`myfolder`) → under the cwd;
  - absolute or `~/` (`~/projects/newp`) → that path.
  - If the path **does not exist it is created**, otherwise it is used. `--new` is an explicit
    command, so creating a missing path is the declared intent, not a navigation typo.
- **`--name <name>`** — optional. Omitted → default name `heca #n` (or the folder basename — decide
  in T522). Present → that name.

Examples: `heca --new` · `heca --new ~/projects/foo` · `heca --new ~/projects/foo --name work`.

## Palette — "Create Session"

No OS folder picker (heavy and out of style). An input with **fuzzy path completion**, consistent
with the palette's existing fuzzy search:

- An **absolute or `~/`** path creates/uses the session there regardless of context: `~/Downloads/newp`.
- A **relative** path (no `~`/`/`) resolves against the **focused pane's cwd** (create next to the
  project you are in), falling back to home. There is no "GUI cwd": you state the folder in the input.
- A path that **does not exist** shows an explicit row **"Create folder: `<path>`"** — selecting it
  creates the folder (`mkdir -p` style) and then the session. Never create silently on a typo.
- The folder is **optional** (tmux-like): with none, the session is born on a default (home) and you
  navigate; the "project folder" shown in the `#` list is the assigned one, or derived from the
  focused pane's cwd.

## Default instance naming

- No name yet → default **`heca #n`**, `#n` a progressive counter — only to tell anonymous defaults
  apart.
- A real, **unique** name (create/rename) → just the name, no `#n`. P100 keeps names unique rather
  than appending numbers.
- Renamable via the session **rename** action (palette/key/RPC).

## Window title (built in T545, synced here)

`path` (e.g. `~/projects/heca`), plus ` #k` from the **second** window on the same instance. The
per-window suffix also lets `follow` target a window by its title. The instance name (`heca #n` /
the real name) is a session concept used by the `#` palette mode, not necessarily in the OS title.
