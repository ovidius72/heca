# Keyboard commands

The default keys, what each one does, and how the keyboard behaves while a dock holds it or while
you scroll a terminal. Every key here is a default: change or remove any of them, and add your own,
as described in [Keybindings](keybindings.md). `keybindings.default.toml` is the source of truth,
and `heca --show-keys` prints what is bound right now.

In the tables, a key shown without a modifier prefix is pressed **after the prefix key**. `h` means
`prefix+h` (by default `Ctrl+B`, then `h`).

## Prefix Key

heca uses **tmux-style prefix mode**: press `Ctrl+B`, release, then press the action key. This avoids conflicts with applications running inside panes.

- **Prefix timeout**: one second by default (`[keys] prefix_timeout_ms`) — if you don't press a key, prefix mode exits automatically.
- **Double prefix**: pressing the configured prefix twice sends the literal configured prefix key to the focused pane (for example `Ctrl+B Ctrl+B` → `Ctrl+B`, `Ctrl+A Ctrl+A` → `Ctrl+A`).
- **Bare modifiers**: Holding Shift/Ctrl/Alt alone in prefix mode does nothing — wait for the actual key.
- **Display symbol**: in the UI (context menus, hints) the prefix is shown as the symbol **`λ`** — e.g. `prefix+f` renders as `λ f`. This is display-only: config and keybinding strings keep the literal `prefix` token. The substitution lives in one helper, `heca::shortcut::format_shortcut(keys, with_prefix)` (change the symbol there).

## Navigation

| Command | Default Binding | Description |
| --------- | ---------------- | ------------- |
| Focus left | `h` | Activate column left, scroll view |
| Focus right | `l` | Activate column right, scroll view |
| Focus up | `k` | Activate pane above in column |
| Focus down | `j` | Activate pane below in column |
| Next pane | `]` | Cycle to next pane across columns |
| Prev pane | `[` | Cycle to prev pane across columns |
| Workspace next | `Ctrl+N` | Switch to next workspace (down) |
| Workspace prev | `Ctrl+P` | Switch to prev workspace (up) |
| Focus toggle (local) | `i` | Toggle between last two panes in same workspace |
| Focus toggle (global) | `Shift+I` | Toggle between the current and the last-visited workspace |
| Focus toggle (last) | `Ctrl+I` | Toggle between the current pane and the last one you were in, whichever workspace it is in (`focus_toggle_last`) |

## Pane Operations

| Command | Default Binding | Description |
| --------- | ---------------- | ------------- |
| Split horizontal | `Enter` | New column to the right of active |
| Split vertical | `v` | New pane below active in same column |
| Close | `x` | Close active pane |
| Float | `f` | Toggle pane between scrolling and floating |
| Pane select | `q` | Overlay letters on panes; press letter to focus |
| Swap pane | `Shift+M` | Overlay letters; swap panes (stay at current position) |
| Swap and focus | `m` | Overlay letters; swap panes (follow to destination) |
| Move column to workspace | `Shift+G` | Overlay letters on workspaces; press letter to move the active column there |
| Move pane to workspace | `g` | Overlay letters on workspaces; press letter to move the active pane there |
| Move pane to column | `Ctrl+C` | Overlay letters on columns (any workspace); press letter to stack the active pane into that column |
| Move pane to a new column | `Shift+Enter` | Move the active pane into a new column beside it (a pane alone in its column stays put) |
| Take pane | `t` | Overlay letters; take the picked pane to the bottom of the active column |
| Take pane and focus | `Shift+T` | Take the picked pane and focus it |
| Follow link | `Shift+O` | Overlay letters on terminal links; press letter to open |
| Pick anything | `/` | Overlay letters on every actionable chrome target; press letter to activate it |
| Zoom column | `z` | Toggle the active column between its width and a viewport-wide width |
| Delete column | `Shift+X` | Delete the focused pane's whole column |

## Resize

| Command | Default Binding | Description |
| --------- | ---------------- | ------------- |
| Resize increase | `=` | Widen active column |
| Resize decrease | `-` | Narrow active column |
| Pane height increase | `Shift+=` | Grow active pane height |
| Pane height decrease | `Shift+-` | Shrink active pane height |

## Font zoom

Two explicit scopes (no implicit resolution): **`Ctrl`** = whole app (chrome/UI
font **and** every terminal), **`Ctrl+Shift`** = the terminal the keyboard is on
only (a docked terminal that holds it, else the focused pane's), layered on top of
the app-wide size. The same gesture is also on **`Ctrl`/`Meta`+mouse-wheel**,
resolved by what's under the pointer (over a terminal, in a pane or a dock → that
terminal; over chrome/empty → whole app). (`Alt` is avoided: on macOS the Option
key rewrites the typed character, so `Alt+=` never matches.)

| Command | Default Binding | Description |
| --------- | ---------------- | ------------- |
| App font bigger (everything) | `Ctrl+=` | Increase chrome/UI + every terminal pane |
| App font smaller (everything) | `Ctrl+-` | Decrease chrome/UI + every terminal pane |
| App font reset (everything) | `Ctrl+0` | Reset to the configured sizes |
| Terminal font bigger | `Ctrl+Shift+=` | Increase the font of the terminal the keyboard is on |
| Terminal font smaller | `Ctrl+Shift+-` | Decrease the font of the terminal the keyboard is on |
| Terminal font reset | `Ctrl+Shift+0` | That terminal follows the app-wide size |

**Sticky font-size modes** (avoid re-pressing the chord): enter once, then tap keys
repeatedly. `prefix+!` → **app** font mode (whole app), `prefix+@` → **terminal** font
mode (the terminal the keyboard is on). Inside either mode: `k`/`↑` bigger, `j`/`↓` smaller, `0` reset,
`Esc`/`Enter` exit. Like the built-in `resize` mode (`prefix+r`), fully rebindable.

## Move / Swap (adjacent)

| Command | Default Binding | Description |
| --------- | ---------------- | ------------- |
| Move pane left | `Ctrl+[` | Move active pane to column on left |
| Move pane right | `Ctrl+]` | Move active pane to column on right |
| Move column up | `Ctrl+Shift+K` | Move the active column up |
| Move column down | `Ctrl+Shift+J` | Move the active column down |
| Swap left | `Ctrl+H` | Swap with pane to the left |
| Swap right | `Ctrl+L` | Swap with pane to the right |
| Swap up | `Ctrl+K` | Swap with pane above |
| Swap down | `Ctrl+J` | Swap with pane below |

## Workspace & Sidebar

| Command | Default Binding | Description |
| --------- | ---------------- | ------------- |
| Create workspace | `w` | Create new workspace with a pane |
| Rename workspace | `Shift+W` | Rename the **active** workspace (a focused container's cursor row has its own key — see below) |
| Rename pane | `$` | Rename the **focused** pane (likewise) |
| Rename column | `Shift+C` | Rename the **active** column (likewise) |
| Toggle left sidebar | `b` | Show/hide left sidebar |
| Toggle right sidebar | `.` | Show/hide right sidebar |
| Sidebar focus | `e` | Give the keyboard to the workspaces dock |
| Focus dock | `Shift+E` | Letters over every dock; press one to give it keyboard focus |
| Collapse current workspace | `<` | Collapse the active workspace tree row (UI only) |
| Collapse current column | `(` | Collapse the focused tiled column tree row (UI only) |

Today, sidebar navigation operates on the built-in workspace tree, which is a **container** (dock) mounted in a sidebar shell. Long-term the sidebar hosts several pluggable containers side by side.

**Chrome keyboard focus is a dock, not a side.** `focus_dock` (`Shift+E`) lights a letter over every
dock on screen and focuses the one you pick; the focused dock shows a focus ring, and keyboard
scrolling acts on it. Because focus is held by *container id*, moving a dock from one sidebar to the
other takes its focus with it — nothing in this path names left or right. `sidebar_focus` (`e`) is the
same idea aimed at navigation: it focuses the dock that has keyboard navigation of its own, reveals
whichever region that dock is seated in, and enters nav mode. With no navigable dock mounted it does
nothing (it will not expand an empty sidebar to show you a blank frame).

The dock can also be named, which skips the pick: `focus_dock workspaces` over RPC, or a mode binding
carrying `args = { dock = "workspaces" }` (no flat binding form takes args yet). It is one action
either way — the pick is only how a keyboard supplies an argument it cannot type. Aiming it at the
dock that already has focus is the way back out, so one key both takes the keyboard and gives it
back; `unfocus_dock` (RPC) and `Esc` do the same thing explicitly.

**Focus is the mode.** While a dock holds chrome focus the keyboard is *redirected to it* — there is
no separate mode to enter, because focus already answers where the keys go. Concretely:

- unprefixed keys resolve in the **focus layer** (below) instead of being forwarded to the terminal;
- an **unbound** key while a dock is focused does nothing — it is swallowed, never leaked into the
  shell behind the dock;
- `prefix+…` keeps working exactly as it does otherwise, and falls through to the focus layer when
  the global map has no binding for the key;
- the focused pane is **not** changed. Only the keyboard moves, so `prefix+Enter` still splits the
  pane you last worked in;
- the status bar shows the dock's name where it would say `NORMAL`, so a swallowed key is never
  silent about where it went. The focus ring is the other half of that.

Two layers are consulted while a dock is focused, in this order:

1. **the surface's own** — [`[[keys.surface]]`](keybindings.md#4-a-surfaces-own-keys--keyssurface) (this placement first, then the
   component as a whole);
2. **the focus layer** — a built-in mode keymap, `[[keys.mode]] name = "focus"`, overridable in
   `keybindings.toml` like any other. It carries what the *widgets* answer — paging and edges for
   whatever scroll area the focused container nests — deliberately not a per-container vocabulary: a
   scroll region behaves the same wherever it is mounted, so nothing has to declare it, and a
   container with nothing scrollable simply declines and the key does nothing.

## The workspaces dock's own keys

heca's own workspaces component ships these in `keybindings.default.toml`. They apply **only while
that dock holds the keyboard** (`prefix+e`, or click it), so they need no prefix and no mode.

| Key | Action | What it acts on |
| ----- | -------- | ----------------- |
| `k` / `j` (also `Up`/`Down`) | `cursor_up` / `cursor_down` | the dock's own cursor |
| `h` (also `Left`) | `collapse_row` | fold the row, or step out to its parent |
| `l` / `Enter` (also `Right`) | `activate_selected` | focus the row **and hand the keyboard back** |
| `Space` | `peek_selected` | focus the row, **keep** the keyboard on the dock |
| `Tab` | `toggle_selected` | fold / unfold a structural row |
| `w` / `p` / `c` | `create_workspace` / `create_pane` / `create_column` | a new one where the cursor is |
| `r` | `rename_selected` | rename the row under the cursor — a pane or a workspace |
| `x` | `delete_selected` | delete the row under the cursor |
| `Shift+X` | `delete_selected_column` | the cursor's whole column |
| `z` | `zoom_selected` | zoom / unzoom the cursor's column |

The cursor-row verbs (`r`, `x`, …) are the **component's**, because "the row my cursor is on" is a
fact nothing outside the component can know. The app's own `prefix+$` / `prefix+Shift+w` are a
different question — the focused pane and the active workspace — and keep that meaning wherever the
keyboard is. Everything else here **binds an action that already exists** rather than redeclaring it.

## Keys while a dock is focused

The focus layer (`[[keys.mode]] name = "focus"`) carries these as shipped:

| Key | Action |
| ----- | -------- |
| `PageUp` / `PageDown` | Scroll the focused dock one page up / down |
| `Home` / `End` | Jump the focused dock to top / bottom |
| `Alt+PageUp` / `Alt+PageDown` | Scroll the focused dock one page left / right |
| `Alt+Home` / `Alt+End` | Jump the focused dock to its left / right edge |
| `Esc` | Give the keyboard back to the focused pane |

The bare keys are free here precisely because nothing is being forwarded to a backend. The `Shift+`
scroll bindings under "Direct (non-prefix) keybindings" are unchanged and, while a dock is focused,
aim at the dock too — one binding, one meaning: *scroll whatever has the keyboard*. Horizontal
scrolling exists only here (`scroll_page_left`, `scroll_page_right`, `scroll_to_left_edge`,
`scroll_to_right_edge`, also reachable over RPC by the same names), because a
terminal viewport has a single axis.

## Finding a key — `heca --show-keys`

Keys are no longer all in one file, so reading config can no longer answer "what runs this action":
a mode's keys are in `[[keys.mode]]`, a surface's in `[[keys.surface]]`, and a plugin's are in no
file at all. `heca --show-keys` prints every binding name, the key it resolves to and the layer it
came from — read out of the **built keymaps**, so it sees all of them alike. `--json` emits the same
three facts per line for scripting.

```
$ heca --show-keys
BINDING                       KEY              LAYER
close                         prefix+x         [keys]
workspaces.cursor_down        j                [[keys.surface]] workspaces
docker.restart                r                plugin
```


## Navigating the workspaces dock

**There is no sidebar mode.** Which dock the keyboard is aimed at is a *container id* held in the
chrome store, not an input mode, so nothing has to be entered or left: `prefix+e` (or a click) gives
the keyboard to the workspaces dock, `Esc` gives it back, and while it is there the dock's own keys
apply — [the table above](#the-workspaces-docks-own-keys), rebindable in [`[[keys.surface]]`](keybindings.md#4-a-surfaces-own-keys--keyssurface).

`l` / `Right` / `Enter` focus the row **and hand the keyboard back**; `Space` focuses it **and keeps
the keyboard on the dock**, so you can keep walking with `j`/`k` and preview each row. The mutation
keys act on the row the cursor is on; the global collapse bindings (`<` and `(`) act on the active
main-view workspace/column instead and do not touch the dock.

**A click does the same thing as a key.** Clicking anywhere in a container gives it the keyboard and
moves its cursor to the row you clicked — so `j` continues from there — and clicking outside every
container gives the keyboard back. Right-clicking a row opens that row's menu, and aims the keyboard
the same way first.

## Terminal Scrollback & Selection

heca provides a **host-managed scrollback viewport** — when you scroll up at a
normal shell prompt, you are scrolling the host's viewport into terminal history,
not sending escape sequences to the PTY. This gives you tmux-style scrollback
navigation without a terminal multiplexer.

**Mouse-grab awareness:** when a TUI program (vim, htop, less, lazygit) enables
mouse reporting, the host automatically forwards wheel events to the program.
Shift+wheel always scrolls the host viewport, bypassing any mouse grab.

**Selection mode** is heca's copy/scrollback mode (tmux copy-mode style), entered
with `prefix+s` (caret stays where the terminal cursor was). It is also entered
automatically when you scroll up with the wheel at a non-grabbed prompt.

Inside Selection mode:

| Key | Action |
| ----- | -------- |
| `h` / `l` / `j` / `k` or arrow keys | Move caret left / right / up / down (the viewport auto-scrolls at edges) |
| `PageUp` / `PageDown` | Move caret up / down by one page |
| `u` / `d` | Scroll viewport up / down by 12 lines (the `amount` argument of `scrollback_line_up` / `scrollback_line_down`) |
| `Ctrl+u` / `Ctrl+d` | Scroll viewport up / down by one full page |
| `g` / `Home` | Jump to top of scrollback history |
| `Shift+g` / `End` | Jump to the live bottom (stay in selection mode) |
| `v` / `Space` | Begin selection (toggle highlighting) |
| `o` | Toggle selection endpoint |
| `Shift+o` | Open the hyperlink under the caret |
| `y` | Copy selection to clipboard |
| `p` / `Super+v` / `Ctrl+Shift+v` | Paste the clipboard |
| `/` | Search the scrollback — type the query (matches highlight live, the caret jumps to the nearest); from the field `Enter`, `↓` or `Ctrl+j` go to the next match and `↑` or `Ctrl+k` to the previous one (the `[keys.widgets]` `activate` / `menu_down` / `menu_up` bindings), the bar's arrows do the same, the field's × or `Cmd+Shift+Backspace` / `Ctrl+Shift+Backspace` (`clear_input`) empties it, and `Esc` closes |

Outside Selection mode the same search opens directly with `Super+F` (`Cmd+F` on macOS) or `Ctrl+Shift+F` (`search_scrollback`).
| `n` / `Shift+n` | Jump to the next / previous search match (after a `/` search) |
| `Esc` | Exit selection mode (snap to bottom + clear selection) |

**Wheel:**

- At a normal shell prompt → scrolls host scrollback viewport by
  `terminal_wheel_scroll_lines` rows per notch (configurable).
- Over a mouse-grabbed TUI (vim, htop, less, etc.) → forwarded to the terminal
  as mouse events.
- **Shift+wheel** → always scrolls host viewport, bypassing any mouse grab.
- The wheel only scrolls — it never switches to Selection mode. Typing snaps the view back to the
  bottom; `prefix+s` enters Selection mode.
- **Shift+drag** selects with the mouse and copies on release, then clears the highlight — the same
  as `y` in Selection mode. It does not enter Selection mode.
- **Alternate-screen TUIs** (`nvim`, `less`, …): while a program owns the
  alternate screen, the host has no exposed scrollback history to scroll into.
  Plain wheel is forwarded to the program in that case (so `less` can still
  react when it isn't mouse-grabbed); `Shift+wheel` is a no-op because it
  bypasses the program and there is nothing to scroll host-side. This is an
  inherent limitation — wezterm preserves the pre-alt-screen history but does
  not expose it while the alternate screen is active.

**Relevant settings:**

- `terminal_scrollback_lines` — total host-retained history capacity (rows above
  the live viewport).
- `terminal_mouse` — when `true`, wheel input controls host scrollback unless
  the terminal app has grabbed the mouse.
- `terminal_wheel_scroll_lines` — rows moved per wheel notch.
- `terminal_scroll_animations` — enable/disable backend-side easing for
  discrete terminal viewport jumps in Normal mode (for example
  `Shift+PageUp/PageDown`, `Shift+Home/End`, and the scrolled-up badge click).
  `false` makes those jumps immediate.

## Direct (non-prefix) keybindings

For users who prefer not to use prefix mode, heca provides direct bindings
that work without the prefix key. These are intercepted before reaching the
terminal:

| Combo | Action |
| ------- | -------- |
| `Shift+PageUp` | Scroll up by one page (repeatable, no selection mode entry) |
| `Shift+PageDown` | Scroll down by one page (repeatable) |
| `Shift+Up` | Scroll up by a few lines (repeatable) |
| `Shift+Down` | Scroll down by a few lines (repeatable) |
| `Shift+Home` | Jump to the top of scrollback (repeatable) |
| `Shift+End` | Jump to the live bottom (repeatable) |

These are intercepted as global keybindings before reaching the terminal.
They stay in Normal mode, so holding the key repeats the scroll without
entering Selection mode. Page jumps and top/bottom jumps honor
`terminal_scroll_animations`; line steps remain immediate.

While a chrome dock holds keyboard focus these same four page/edge bindings are aimed at **the dock**
instead of the pane — see "Chrome keyboard focus is a dock, not a side" above. The line steps
(`Shift+Up` / `Shift+Down`) stay with the pane: a scroll area answers pages and edges, not lines.

## Scrollback GUI

Terminal panes with scrollback history show two GUI affordances (theme-driven,
configurable under `[appearance.terminal] show_scrollbar`):

- **Scrollbar** — a draggable accent thumb on the right edge of the pane. Drag
  it to jump to an arbitrary viewport position; clicking the track jumps there
  immediately. The thumb size reflects how much history is visible. Visibility:
  - `always` — always shown when there is scrollable history.
  - `when_needed` (default) — shown only while scrolled away from the live
    bottom.
  - `never` — never shown.
- **Scrolled-up badge** — a clickable chip reading `N lines above` that appears
  at the top-right of the pane while the viewport is scrolled up. Clicking it
  jumps back to the live bottom and honors `terminal_scroll_animations`.

## System

| Command | Default Binding | Description |
| --------- | ---------------- | ------------- |
| Command palette | `p` | Open command palette |
| Reload config | `Shift+R` | Reload config.toml at runtime — a toast confirms success, or reports the parse error with a **Retry** button (the working config is kept on failure; the error also prints to stderr) |
| Pane palette | `o` | The command palette on the panes list |
| Workspace palette | `Ctrl+O` | The command palette on the workspaces list |
| Session palette | `#` | The command palette on the sessions list |
| Pane map (exposé) | `Tab` | Toggle the exposé layer |
| Context menu | `>` | Open the context menu of whatever the keyboard is on (also `Shift+F10` and the `ContextMenu` key, without the prefix) |
| Scroll view left / right | `Shift+Left` / `Shift+Right` | Pan the horizontal view to reach off-screen columns |

### Selection, clipboard and notifications

| Command | Default Binding | Description |
| --------- | ---------------- | ------------- |
| Selection mode | `s` | Enter selection mode |
| Clear selection | `Shift+S` | Clear the selection and leave selection mode |
| Copy selection | `y` | Copy the selection to the clipboard |
| Paste | `Shift+P` | Paste the clipboard (also `Super+v` and `Ctrl+Shift+v`, without the prefix) |
| Pick a notification | `n` | Letters over the visible toast actions and their close buttons |
| Dismiss last notification | `Shift+N` | Dismiss the first eligible visible notification |
| Dismiss all notifications | `Alt+N` | Dismiss every visible notification |
| Notification history | `Ctrl+Alt+N` | The notifications still on screen and the ones dismissed or expired, newest first |

### The command palette

Every action heca knows about, searchable from one key — including those a mounted
component contributes, which are listed under that component's name and focus it
before running.

- **Fuzzy, smart-case matching.** Type lowercase and case is ignored; type an
  uppercase letter and it starts to matter. Set `search_case` to `"sensitive"` or
  `"insensitive"` in `config.toml` if you'd rather it never guessed.
  Folder pickers use their separate `folder_search_case`, defaulting to `"insensitive"`.
- **It learns what you use.** Commands you pick often — and recently — rise to the
  top, so an empty palette shows your habits rather than an alphabet. Running
  something a second time counts for more than having run something else a moment
  ago.
- **Past searches come back.** `Ctrl+p` / `Shift+↑` walks back through what you
  searched for, `Ctrl+n` / `Shift+↓` forward; step past the newest and whatever you
  were part-way through typing is handed back to you.
- **It remembers across restarts**, in a small file under your data directory. Delete
  it whenever you like — you lose nothing but the convenience — or set
  `search_history = false` to keep nothing at all. `search_history_size` and
  `search_usage_size` decide how much is kept.
- **Several heca windows share it** without trampling each other: a save merges with
  what is already on disk rather than replacing it, the way neovim's shada file
  works. Two windows both using a command each add to its count.
- **Forget it whenever you like.** `Clear Search History` drops the queries you have
  typed, `Clear Search Ranking` drops what heca has learned about your habits — both
  from the palette itself, or over RPC.
- The selected row shows its full description, wrapped, and the rows below make room
  for it as you move.
