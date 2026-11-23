# Keybindings — the five ways to bind a key

Every binding in heca is one of five shapes. They differ only in **what they can carry** and **when
they apply**; all five go through the same keymap and the same action registry.

| Shape | Carries `args` | Applies |
|---|---|---|
| `action = "combo"` under `[keys]` | no | always |
| `[[keys.bind]]` | yes | always |
| `[[keys.command]]` | — (its own fields) | always |
| `[[keys.mode]]` + `[[keys.mode.bindings]]` | yes | while that mode is active |
| `[[keys.surface]]` | yes (via `[[keys.surface.bind]]`) | while that surface holds the keyboard |

Your file is deep-merged over `keybindings.default.toml`, so you write only what you change. Put it
in `~/.config/heca/keybindings.toml` — or in `config.toml`, which is merged with it.

> **A binding to an action that does not exist is not an error.** Action names resolve when the key
> is pressed, so a plugin's action can be bound before the plugin loads — and a typo produces a key
> that silently does nothing. `heca --show-keys` lists every binding and what it resolved to.

---

## Open lazygit in a terminal pane

The short answer, and the one to copy:

```toml
[[keys.command]]
keys       = "prefix+Ctrl+g"
command    = "lazygit"
kind       = "terminal"   # terminal | app | plugin   (default: terminal)
float      = true         # floating pane instead of a tiled one
close_pane = true         # close the pane when lazygit exits
```

`[[keys.command]]` is for launching a program and names no action. Two more fields:
`keep_on_error = true` keeps the pane when the command fails, `keep_on_success = true` when it
succeeds — useful when you want to read what it printed. Each overrides `close_pane` for that exit
outcome.

More examples:

```toml
[[keys.command]]
keys = "prefix+g"
command = "lazygit"
float = true
close_pane = true

[[keys.command]]
keys = "prefix+t"
command = "btm"  # bottom system monitor
close_pane = true
keep_on_error = true

[[keys.command]]
keys = "Alt+Enter"   # no prefix — a global key
command = "alacritty"
```

- `kind = "terminal"` is the default. `app` and `plugin` are reserved and currently report
  "not yet implemented".
- `float = true` opens a centered floating pane.
- Commands run in a real PTY using the user's shell (`-ic` on Unix, `/C` on Windows), so existing
  shell-style command strings keep working and the shell stays interactive.

Merged by `keys`, like every other binding list: an entry with the same combo replaces that
default, and the rest are left alone.

The same thing as an action, which is what `[[keys.command]]` becomes internally:

```toml
[[keys.bind]]
keys   = "prefix+Ctrl+g"
action = "spawn_command"
args   = { command = "lazygit", float = "true", close_pane = "true" }
```

`spawn_command` takes `command` (required), `kind`, `float`, `close_pane`, `keep_on_error`,
`keep_on_success`. **There is no size or position argument** — a floating pane takes its default
geometry.

---

## 1. A key with no arguments

```toml
[keys]
zoom_column = "prefix+z"
close       = "prefix+x"

# One action, several keys — one entry, either spelling
focus_left  = ["prefix+h", "prefix+ArrowLeft"]
focus_right = "prefix+l, prefix+ArrowRight"
```

The value is the **key**, the name is the **action** — that way round, because one action can have
several keys and one key cannot have several actions.

Free a default key:

```toml
[keys.unbind]
"prefix+f" = true        # Disable float toggle
"prefix+q" = true        # Disable pane select
"prefix+Shift+q" = true  # Disable swap pane
```

Why unbind: to free a key for a binding of your own, to turn off a feature you do not use, or to
resolve a conflict with a custom binding. The action still exists — you can rebind it to a
different key in `[keys]`. For example, after unbinding `prefix+f` (float) you can rebind it to
`prefix+Shift+f`.

A key written without `prefix+` is **global**: it is checked in normal mode before the key is
forwarded to the terminal.

## 2. A key that carries arguments — `[[keys.bind]]`

The flat form is one string, so it has nowhere to put an argument. This is the same normal-mode
keymap written long-hand:

```toml
[[keys.bind]]
keys   = "prefix+o"
action = "command_palette"
args   = { mode = "pane" }

[[keys.bind]]
keys   = "prefix+Tab"
action = "toggle_layer"
args   = { name = "heca.expose" }
```

The default palette shortcuts share the registered `command_palette` action:

| List | Default binding | Argument |
|---|---|---|
| Actions | `prefix+p` | none |
| Panes | `prefix+o` | `mode = "pane"` |
| Workspaces | `prefix+Ctrl+o` | `mode = "workspace"` |
| Sessions | `prefix+#` | `mode = "session"` |

Press your configured prefix, then the remaining key. The default prefix is `Ctrl+b`.
For sessions, press the prefix and then `#`; no extra character is needed in the palette.
Override a binding with a `[[keys.bind]]` entry using the same combo, or remove it with
`"prefix+#" = true` under `[keys.unbind]`. No additional action registration is needed.

**An action that needs arguments asks for them in the palette.** Choosing one (its row ends in `…`)
opens a step for each required argument it has not been given: its words for a fixed list, the docks
mounted right now for a dock, true or false, or what you type for a name or a number. When nothing
required is missing, it runs. A key can open the same steps for one action, with some arguments
already chosen:

```toml
[[keys.bind]]
keys   = "prefix+m"
action = "command_palette"
args   = { action = "chrome.container.move_to_region", arguments = "container_id=workspaces" }
```

The same works from a script: `heca rpc command_palette action=chrome.container.move_to_region`.

## 3. A mode — a sub-keymap behind one key

While a mode is active its own bindings come first. A key the mode does not bind falls through to
your normal bindings — so Cmd+V still pastes while you select — and a key nobody binds is dropped,
never typed into the program in the pane. In every mode, Escape leaves it, Enter confirms and leaves,
and the prefix key starts a prefix. `sticky = true` stays until Escape; `sticky = false` runs one of
its own bindings and returns.

```toml
[[keys.mode]]
name    = "git"
trigger = "prefix+Shift+d"
sticky  = false

[[keys.mode.bindings]]
action = "spawn_command"
keys   = "g"
args   = { command = "lazygit", float = "true" }

[[keys.mode.bindings]]
action = "spawn_command"
keys   = "d"
args   = { command = "lazydocker", float = "true" }
```

⚠️ **Redefining a built-in mode** (`resize`, `selection`, `layer`, `focus`) merges your *bindings*
into it, but **replaces** `trigger` and `sticky` — so repeat them exactly, or `resize` quietly stops
answering `prefix+r`.

### Built-in modes

heca ships six: `resize` (`prefix+r`), `app_font_size` (`prefix+!`), `pane_font_size` (`prefix+@`),
`selection`, `layer` and `focus`. The last three have an empty `trigger` — something else in heca
puts you there: `selection` is entered with the `enter_selection_mode` action (`prefix+s`) or by
scrolling up at a prompt (its keys are in [Keyboard commands](keyboard-commands.md#terminal-scrollback--selection)),
`layer` while an overlay holds the keyboard and `focus` while a dock does.

**`resize`** is sticky. In it the key points the way the **divider** travels, whichever column or
pane is active. The *target* decides the axis — a column is resized across, a pane down — so only
`amount` and, optionally, `edge` are needed. Positive is right / down:

- `h` / `l` — move the column's right edge left / right (narrower / wider)
- `j` / `k` — move the pane's divider down / up
- the arrow keys work too, and `Escape` / `Enter` exit the mode

A pane's divider is the one below it, or the one above when it is the **last** pane — so on the
last pane `j` moves that divider down and the pane gets shorter. `prefix+Shift+=` and
`prefix+Shift+-` are the size verbs instead: they grow and shrink the active pane whichever edge
has to move.

*Choosing the edge.* A pane has two horizontal edges, and `edge` says which one moves. Omit it and
you get the one above. `edge = "top"` takes the pane's upper edge instead, so the defaults are
`Shift+j` / `Shift+k` — move the pane's **top** edge down / up. Positive is still down the screen,
so `Shift+j` shrinks the pane from above. Nothing happens on the first pane, which has no edge
above it.

```toml
[[keys.mode.bindings]]
action = "resize"
keys = "Shift+k"
args = { target = "pane", amount = "-40", edge = "top" }
```

The same holds on the x axis. A column's **left** edge is where the column before it ends, so
`edge = "left"` moves that boundary: `Shift+h` / `Shift+l` move the column's left edge left / right.
Positive is still right, so `Shift+l` shrinks the column from the left. Nothing happens on the
first column, which has nothing to its left. `edge` accepts `"auto"` (the default), `"top"`,
`"bottom"`, `"left"` and `"right"`.

*Moving while still resizing.* The mode is sticky — it runs an action and stays until `Escape` or
`Enter` — so a focus action bound inside it moves the focus without leaving: `Ctrl+h/j/k/l` (and
`Ctrl+arrows`) focus another column or pane, still resizing. Nothing special is needed for this:
any action can be bound inside a mode, and a sticky mode keeps running.

### Custom modes

```toml
[[keys.mode]]
name = "my_mode"
trigger = "prefix+o"      # Key that enters the mode; "" = no key of its own
sticky = true             # true = stay until Esc (default); false = one key, then exit

[[keys.mode.bindings]]
action = "focus_left"
keys = "h"

[[keys.mode.bindings]]
action = "focus_right"
keys = "l"
```

A **sticky** mode (`sticky = true`) stays until `Escape` or `Enter`; `resize` is one. A non-sticky
one (`sticky = false`, or a chord) runs one action and exits.

### Adding a key to a built-in mode

A built-in mode is merged back in by name, so you can add a key without redeclaring the whole
mode — but the fields are not merged the same way:

| field | when you redefine a built-in mode |
| --- | --- |
| `bindings` | **added** to the built-in ones — you keep every default key |
| `trigger`, `sticky` | **replace** the built-in values |
| `unbind = ["…"]` | takes keys **out** of the mode, the shipped ones included |

So repeat `trigger` and `sticky` exactly as the default file has them. Omitting `trigger` sets it
to `""`, which for a mode like `resize` silently means `prefix+r` stops entering it.

```toml
# Add `p` as a second "previous search match" key, keeping every other selection key.
[[keys.mode]]
name = "selection"
trigger = ""      # as in keybindings.default.toml — omitting it would clear the trigger
sticky = true     # as in keybindings.default.toml — omitting it would reset stickiness

[[keys.mode.bindings]]
action = "search_prev_match"
keys = "p"
```

`unbind` is how the way out of a dock or an overlay is moved (see the `focus` and `layer` blocks):
bind the action to another key, then unbind `Escape`.

These blocks work in `config.toml` as well as `keybindings.toml`: both files are merged into one
configuration before it is read.

## 4. A surface's own keys — `[[keys.surface]]`

A dock and an overlay are the same thing to the keyboard: something that holds it for a while and
answers keys of its own. These apply **only while that surface holds the keyboard**, so they need no
prefix.

```toml
# The workspaces dock, wherever it is seated
[[keys.surface]]
name            = "workspaces"
global_focus    = "prefix+e"     # takes the keyboard — applies while it does NOT have focus
cursor_down     = "j,ArrowDown"
delete_selected = "x"

# The exposé, which is a layer rather than a dock — same table
[[keys.surface]]
name        = "heca.expose"
delete_pane = "x"
pick        = "s"
```

The workspaces dock's own keys, as shipped — each acts on the row under its cursor:

| Key | Action | What it does |
|---|---|---|
| `j` / `k`, `↓` / `↑` | `cursor_down` / `cursor_up` | Move the cursor. |
| `Shift+j` / `Shift+k`, `Shift+↓` / `Shift+↑` | `move_selected_down` / `move_selected_up` | Move the **workspace** the cursor is in one place down / up — on any row, since the dock is the list of workspaces. The cursor stays on its row. |
| `Ctrl+j` / `Ctrl+k`, `Ctrl+↓` / `Ctrl+↑` | `swap_selected_down` / `swap_selected_up` | Swap the **pane** under the cursor with its neighbour in the column (nothing on a workspace row, nothing past the end). |
| `x`, `Shift+x`, `r`, `z`, `w`, `p`, `c` | `delete_selected`, `delete_selected_column`, `rename_selected`, `zoom_selected`, `create_workspace`, `create_pane`, `create_column` | As named. |

**Docks sharing a sidebar** are folded and resized by plain actions too, bindable with `args`:
`chrome.container.toggle_collapsed` (`container_id` optional — omitted, the dock that holds the
keyboard) and `chrome.container.resize container_id=… by=40` (pixels; negative shrinks; the room comes
from the neighbouring dock; `container_id` omitted is the dock that holds the keyboard). The header's
caret and the edge between two docks send the same ones. A dock binds them in its own
`[[keys.surface]]` entry — the workspaces dock ships `Alt+j` / `Alt+k` (and `Alt+↓` / `Alt+↑`), which
move its edge down / up as `prefix+r` moves a pane's:

```toml
[[keys.surface]]
name = "workspaces"
[[keys.surface.bind]]
action = "chrome.container.resize"
keys   = ["Alt+j", "Alt+Down"]
args   = { by = "40" }
```

Moving workspaces is also a plain action, wherever the keyboard is: `move_workspace_up` /
`move_workspace_down` (`prefix+Ctrl+Shift+p` / `prefix+Ctrl+Shift+n`) move the workspace the window
shows, and `move_workspace ws_idx=3 to_idx=0` (from a key with `args`, a script or a plugin) moves any
workspace to any place — `to_idx` counted as the list is after the move. Dragging a workspace row by
its grip sends the same action.

An optional `id` narrows an entry to **one placement**, layered over the entry without one — for a
dock placed more than once (heca starts with one workspaces dock; a program built on heca may add
another, e.g. `workspaces.right`):

```toml
[[keys.surface]]
name = "workspaces"
id   = "workspaces.right"
delete_selected = "X"
```

For a surface key that needs `args`, use `[[keys.surface.bind]]` — **it attaches to the
`[[keys.surface]]` entry above it**:

```toml
[[keys.surface]]
name = "workspaces"

[[keys.surface.bind]]            # while the workspaces dock has the keyboard, `t` runs this
action = "spawn_command"
keys   = "t"
args   = { command = "lazygit", float = "true" }

[keys.surface.unbind]            # remove a key from this surface, keyed by the combo
"x" = true
```

`[[keys.component]]` is the older spelling of `[[keys.surface]]` and still works unchanged — the two are read as one list. New entries should use `[[keys.surface]]`.

**While a surface holds the keyboard**, the app's own bindings behind it are refused — `prefix+j`
does not move the pane behind the map — except for actions that are app-level in every context
(`reload_config`, `close_overlay`, the `prefix+/` picker, `prefix+>`). Which those are is a property
of the **action**, not of the binding, because the same action is reached from a key, a menu entry,
the palette, a button and RPC.

`name` says which surface — a **field**, not the table name, so a surface may be called `unbind` or
`widgets` without colliding with a config keyword. An overlay declares its keys the same way,
naming itself by the addressable name `show_layer` and `hide_layer` use. The `id` narrows an entry
to one placement, because cursor, scroll position and focus are per placement anyway.

Binding names are **short**: under `name = "docker"`, `restart_selected` means the action id
`docker.restart_selected`. You never repeat the component on every line of its own block. An id
heca already knows keeps its own name — a component *binds* existing actions rather than
redeclaring them.

```toml
[[keys.surface]]
name             = "docker"
restart_selected = "r"        # → docker.restart_selected
next_pane        = "n"        # …an EXISTING action id, simply bound here — never redeclared

[[keys.surface]]              # the same surface, one placement only
name = "docker"
id   = "docker.right"
restart_selected = "R"        # everything else is inherited from the entry above
```

**A way out of a surface** is declared once for every surface of that kind, not per surface: the
`[[keys.mode]] name = "layer"` block is the floor every overlay answers (`Escape`, `q` and `Ctrl+q`
as shipped), and `name = "focus"` is the same thing for a focused dock (`Escape`). Declared there
they exist **only while that kind of surface holds the keyboard**, so the program running in a pane
keeps those keys — `:q` still quits vim, and `Ctrl+q` still reaches readline. That is what makes
them different from the global `[keys]` map, which is consulted whether or not anything is in front.

**Merge rules, and why the two forms differ.** A TOML table already merges per key, so changing one
`action = "key"` entry keeps every other default. An **array** is replaced wholesale, which for
`[[keys.surface.bind]]` would mean adding one binding silently drops every shipped default — so
those merge **by their `keys` field** instead: a keymap *is* a map from combo to action, so merging
on the combo is the ordinary table rule applied to what the array is really keyed by. `unbind` is
applied last and keyed by the **combo**, so it retires a binding whatever it points at.

**Where the defaults are.** heca's own components ship their keys in `keybindings.default.toml`, in
this same shape, at the bottom of the file — one place a key is written and one place to change it.
A **plugin** has no entry in that merge, so it registers its keys at runtime instead; whatever your
config says still wins, and a registration never overwrites a combo you bound or gives a rebound
action a second key. Binding an id whose component is not mounted is not an error: like every
binding it resolves at press time, and simply does nothing until that component appears. Nor is an
`id` for a placement that never exists.

The workspaces dock's own keys are listed in
[Keyboard commands](keyboard-commands.md#the-workspaces-docks-own-keys).

**Two keys every surface has, with nothing declared:**

- **`global_focus`** — written in its `[[keys.surface]]` entry, but it applies while the container
  does *not* have focus, which is the only time it is useful. heca therefore binds it in the global
  map rather than the container's own layer. With an `id` it aims at that seating; without one it
  names the component and lands on the seating you were last in. Pressing it again while that
  container holds the keyboard gives it back. The name is reserved — a component cannot have an
  action called `global_focus`. Two components asking for the same `global_focus` combo is reported
  at startup like any other collision.
- **`Escape`** — gives the keyboard back to the main region. Bound for every container as shipped
  (the `focus` block of `keybindings.default.toml`), so a dock that declares nothing is still
  leavable without the mouse. **Every panel always has a way out, and by default it is `Escape`**:
  to move it, bind `unfocus_dock` to another key (`unfocus_dock = "prefix+Escape"`, or in the block)
  and give `Escape` up with `unbind`:

  ```toml
  [[keys.mode]]
  name = "focus"
  unbind = ["Escape"]
  [[keys.mode.bindings]]
  action = "unfocus_dock"
  keys = "Ctrl+g"
  ```

  A config that leaves a floor with no key — in the block or behind the prefix — is refused for
  that floor, `Escape` is put back, and the start-up report says so. With `Escape` given up, it
  reaches whatever is typed into inside the dock (a terminal's program, say) like any key nobody
  claimed. The same holds for overlays (`name = "layer"`, `close_overlay`).

---

## Which surface answers a key

A key acts on whatever is in front of you. Three surfaces, front to back, and exactly one holds the
keyboard:

| | |
|---|---|
| a **layer** | the exposé, a dialog, a menu, a plugin's panel |
| a focused **dock** | a chrome container with the focus ring |
| `heca.panes` | the scrolling area — what is in front the rest of the time |

One resolution order, for every key:

> the focused surface's own `[[keys.surface]]` entry → the floor its kind is guaranteed → the global
> `[keys]` map → then swallowed (a layer, a dock) or sent to the program in the pane (`heca.panes`).

Nearest declaration wins, so a surface key shadows a global one with the same combo.

**This is why `Escape` is never a global binding.** A global one outranks all three at once. The
floors are: a layer closes itself, a dock hands the keyboard back, and `heca.panes` has none — so
Escape reaches the program in the pane and vim still works.

---

## Combo syntax

```
prefix+h          prefix key (Ctrl+B by default), then h
prefix+Shift+q    prefix, then Shift+q
prefix+Ctrl+h     prefix, then Ctrl+h
Alt+Enter         global — no prefix
Super+v           global — Cmd+V on macOS
```

Modifiers: `Ctrl`, `Shift`, `Alt`, `Super` (also `Win` / `Cmd`).

`Alt+…` works on macOS. Option rewrites the character a key produces — Option+L types `¬` — so heca
reads the key off the keyboard itself whenever the character it was handed is not one a binding
could name. A layout that produces an ordinary character keeps its own answer.
Named keys: `Enter`, `Tab`, `Escape`, `Backspace`, `Space`, `Delete`, `ArrowLeft/Right/Up/Down`,
`Home`, `End`, `PageUp`, `PageDown`, `F1`–`F12`, `ContextMenu`. A function key alone is a global
binding: `F1` is the F1 key with no prefix.

Change the prefix itself:

```toml
[keys]
prefix = "ctrl+a"
```

### How long the prefix waits

After the prefix key, heca waits for the next key and then gives up, so a prefix pressed by
accident does not leave the keyboard in a mode you cannot see. The wait is yours to set:

```toml
[keys]
prefix_timeout_ms = 1000   # default
```

It governs [modes](#3-a-mode--a-sub-keymap-behind-one-key) too, which is the same question one key
later. Raise it if the second key keeps arriving too late; lower it if a stray prefix sits there
waiting.

---

## Widget navigation and editing — `[keys.widgets]`

One generic, cross-widget vocabulary drives every interactive widget and overlay — the context menu,
command palette, `Select` lists, `Tabs`, and `Dialog` focus. These apply **only while such a
widget or overlay is focused**; they never affect normal-mode input. They are split by **axis**.
The defaults:

```toml
[keys.widgets]
# Horizontal (left/right) — Tabs, a dialog's button row.
item_previous = ["ArrowLeft", "Ctrl+h"]   # vim Ctrl+h
item_next     = ["ArrowRight", "Ctrl+l"]   # vim Ctrl+l
# Vertical (up/down) — menus, Select lists, the command palette.
menu_up       = ["ArrowUp", "Ctrl+k"]      # vim Ctrl+k
menu_down     = ["ArrowDown", "Ctrl+j"]    # vim Ctrl+j
menu_history_up   = ["Shift+ArrowUp", "Ctrl+p"]     # older query
menu_history_down = ["Shift+ArrowDown", "Ctrl+n"]   # newer query / back to the draft
complete = "Tab"                           # complete a selected suggestion; otherwise normal focus traversal
# Shared.
activate      = ["Enter", "Space"]         # commit / submit the selected entry or primary action
dismiss       = "Escape"                   # close / cancel the overlay
# Text-input editing shortcuts (plain typing / Backspace / arrows / word-motion are built in).
edit_delete_back          = "Ctrl+h"                 # delete one char before the caret
edit_delete_to_line_start = "Ctrl+u"                 # delete from the caret to line start
edit_select_all           = ["Ctrl+a", "Super+a"]    # select the whole field (Cmd+a on macOS)
clear_input               = ["Super+Shift+Backspace", "Ctrl+Shift+Backspace"]  # empty the field wherever the caret is (Cmd+Shift+Backspace on macOS); a clearable field's × does the same
```

- **Tab / Shift+Tab** traverse focus and stay inside a modal dialog. A selected suggestion may
  consume Tab through `complete`; without a completion, normal focus traversal remains available.
- A focused text field keeps its own keys first (**field-first**): `←` moves the caret inside an
  `Input` but navigates when a button is focused. The shared `Ctrl+h` disambiguates by focus — it
  deletes in an `Input` but is `item_previous` on a `Tabs`/dialog.
- Menu and palette entries with a quick-pick letter can also be run by typing that letter; in the
  command palette, all other keys type into its filter field.

## Zooming the active column

By default, `prefix+z` toggles the active column between its normal stored width and a
viewport-wide zoomed width. Each column tracks its own zoom state, so you can zoom several columns
independently. Press `prefix+z` again on the active column to restore that column's previous width.

## Binding precedence in the scrolling area

For a key pressed while the panes hold the keyboard, heca checks bindings in this order:

1. **Mode bindings** — if in a mode, the mode's own keymap.
2. **Prefix mode** — if in prefix mode, the normal keymap for `prefix+key` combos.
3. **Global bindings** — in normal mode, the global keymap for direct `Alt+key` / `Super+key` combos.
4. **Forward to the terminal** — if no binding matched, the key goes to the focused pane's backend.

[Which surface answers a key](#which-surface-answers-a-key) says what happens before this, when a
layer or a dock is in front.

---

## Binding a plugin or provider action

An action name is not limited to heca's built-ins — it can be the id of an action registered at
runtime, e.g. `plugin.docker.restart` or `chrome.container.move_to_region`:

```toml
[keys]
"prefix+Shift+d" = "plugin.docker.restart"
```

These resolve at press time, so binding one before its plugin loads is fine.

---

## Changed actions — update an old binding

A binding to an action name that no longer exists does nothing (a name is resolved when the key is
pressed, so no error is raised). If you bound one of these, rewrite it:

**2026-09-29 — showing and hiding a region is one action.** The twelve per-region actions are gone:

| Removed | Now |
|---|---|
| `show_left_sidebar` · `hide_left_sidebar` · `toggle_left_sidebar` | `set_region_visible`, `region = "sidebar.left"` |
| `show_right_sidebar` · `hide_right_sidebar` · `toggle_right_sidebar` | `set_region_visible`, `region = "sidebar.right"` |
| `show_top_bar` · `hide_top_bar` · `toggle_top_bar` | `set_region_visible`, `region = "bar.top"` |
| `show_bottom_bar` · `hide_bottom_bar` · `toggle_bottom_bar` | `set_region_visible`, `region = "bar.bottom"` |

`visible` says what to do — `show`, `hide`, or `toggle` (the default when it is left out). It takes
arguments, so it is written long-hand:

```toml
# was: toggle_bottom_bar = "prefix+Ctrl+b"
[[keys.bind]]
keys   = "prefix+Ctrl+b"
action = "set_region_visible"
args   = { region = "bar.bottom" }

# was: hide_left_sidebar = "prefix+Ctrl+h"
[[keys.bind]]
keys   = "prefix+Ctrl+h"
action = "set_region_visible"
args   = { region = "sidebar.left", visible = "hide" }
```

The `[settings] show_left_sidebar = true` switches in `config.toml` are not affected — they are the
startup state, not actions. The command palette offers the new action once per region
("Set Region Visibility › sidebar.left", …), each toggling.

---

## See also

- `keybindings.default.toml` — every default, with its comments. The single source of truth.
- `heca --show-keys` — every binding and the key it resolves to, per layer.
- [Keyboard commands](keyboard-commands.md) — the default keys, the docks' keys, scrollback and selection.
- [Configuration](configuration.md) — the configuration reference.
- [Actions](actions.md) — how an action is dispatched, and how to add one.
