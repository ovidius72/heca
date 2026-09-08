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
> that silently does nothing. `heca --keys-show` lists every binding and what it resolved to.

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
succeeds — useful when you want to read what it printed.

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
"prefix+f" = true
```

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

## 3. A mode — a sub-keymap behind one key

While a mode is active its bindings apply instead of the normal ones. `sticky = true` stays until
Escape; `sticky = false` runs one binding and returns.

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

An optional `id` narrows an entry to **one placement**, layered over the entry without one:

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

`[[keys.component]]` is the older spelling of `[[keys.surface]]` and still works.

**Two keys every surface has, with nothing declared:** `global_focus` (bind it to take the keyboard;
press it again to give it back) and `Escape`, which always hands the keyboard back and **cannot be
unbound** — a surface must always be leavable without the mouse.

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
Named keys: `Enter`, `Tab`, `Escape`, `Backspace`, `ArrowLeft/Right/Up/Down`, `Home`, `End`,
`PageUp`, `PageDown`, `F1`–`F12`, `ContextMenu`.

Change the prefix itself:

```toml
[keys]
prefix = "ctrl+a"
```

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

## See also

- `keybindings.default.toml` — every default, with its comments. The single source of truth.
- `heca --keys-show` — every binding and the key it resolves to, per layer.
- [`README.md`](../README.md) — the configuration reference.
