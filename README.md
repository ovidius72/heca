# heca

> Heca takes its name from the Hecatoncheires — hundred‑handed giants of myth — because it lets you juggle terminals, editors, and tools in a single GPU‑accelerated window.
> Think prefix keys, smooth animations, and zero ambrosia required.

A GPU-native terminal workspace compositor for developers. Inspired by [NIRI](https://github.com/YaLTeR/niri)'s scrollable-tiling model, powered by [wgpu](https://wgpu.rs/) and [cosmic-text](https://github.com/pop-os/cosmic-text).

Think tmux meets NIRI meets Neovide — all panes render in a single GPU-accelerated window with smooth animations, sharp fonts, and zero window seams.

![Status](https://img.shields.io/badge/status-alpha-orange)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-blue)
![License](https://img.shields.io/badge/license-MIT-green)

---

## Table of Contents

- [What It Does](#what-it-does)
- [Tech Stack](#tech-stack)
- [Requirements](#requirements)
- [Installation](#installation)
- [Getting Started](#getting-started)
- [Keyboard Commands](#keyboard-commands)
- [Configuration](#configuration)
- [Actions System](#actions-system)
- [Keybindings](#keybindings)
- [Modes](#modes)
- [Spawning Applications](#spawning-applications)
- [Architecture](#architecture)
- [Plugins (Planned)](#plugins-planned)
- [Roadmap](#roadmap)
- [License](#license)

---

## What It Does

heca is a **workspace compositor** — not a traditional terminal emulator or window manager. It hosts multiple content panes (terminals, Neovim, browsers) inside a single native window with a NIRI-inspired layout engine:

- **Scrolling columns** — Panes are arranged in horizontal columns that scroll smoothly. Focus left/right animates the view to the active column.
- **Vertical stacks** — Within each column, panes stack vertically with intelligent height distribution.
- **Workspaces** — Multiple workspaces stacked vertically, switched with animated transitions.
- **Floating panes** — Any pane can be detached from the scrolling layout and positioned freely.
- **GPU-native chrome** — Tab bar, sidebars, status bar, pane borders — all rendered in one GPU pass.
- **Future pluggable chrome** — the long-term architecture is evolving toward left/right/top/bottom chrome regions that host built-in containers first and WASM/plugin containers later.

**Why not tmux + a terminal?**

| | tmux in a terminal | heca |
|---|---|---|
| Rendering | CPU text grid | GPU text atlas + primitives |
| Fonts | Limited ligature support | Full HarfBuzz shaping, variable fonts |
| Animations | None | Smooth scroll, zoom, slide |
| Panes | Terminal only | Terminal, Neovim, browser, future container/plugin integrations |
| Chrome | Text-only | GPU-rendered shapes, shadows, rounded corners |

**Why NIRI-inspired?**

[NIRI](https://github.com/YaLTeR/niri) is a scrollable-tiling Wayland compositor. heca replicates its core layout principles:

1. **Opening a new window does not affect sizes of existing windows.**
2. **The focused window does not move around on its own.**
3. **Column widths are NOT normalized** — each column keeps its own width.
4. **Only the active column's width changes on resize.**
5. **Horizontal scrolling with animated snap** when switching focus.

---

## Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Windowing** | [winit](https://github.com/rust-windowing/winit) | Cross-platform window creation, input events |
| **GPU API** | [wgpu](https://wgpu.rs/) | Vulkan/Metal/DX12/WebGPU abstraction |
| **Text** | [cosmic-text](https://github.com/pop-os/cosmic-text) | Font shaping, glyph atlas, ligatures |
| **Terminal** | [vte](https://github.com/alacritty/vte) + PTY | ANSI terminal emulation |
| **Config** | [serde](https://serde.rs/) + TOML | User configuration |
| **Language** | Rust | Memory-safe, zero-cost abstractions |

---

## Requirements

- **Rust** 1.80+ (install via [rustup](https://rustup.rs/))
- **GPU** with Vulkan (Linux), Metal (macOS), or DX12 (Windows) support
- **OS**: Linux, macOS, or Windows

### Platform Notes

- **macOS**: Uses `WaitUntil` event loop control flow to avoid 100% CPU busy loops. Physical key fallback for empty `key_text` on winit.
- **Linux**: Native Wayland/X11 via winit. PTY spawning via `libc::openpty()`.
- **Windows**: wgpu DX12 backend. PTY via Windows ConPTY (planned).

---

## Installation

```bash
# Clone
git clone https://github.com/ovidius72/heca.git
cd heca

# Build
cargo build --release

# Run
cargo run -p heca
```

---

## Getting Started

On first launch, heca creates a default config at:

- **Linux/macOS**: `~/.config/heca/config.toml`
- **Windows**: `%APPDATA%\heca\config.toml`

The window opens with a single pane. All WM commands use a **prefix key** (`Ctrl+B` by default), like tmux:

```
Ctrl+B → h   Focus left column
Ctrl+B → l   Focus right column
Ctrl+B → Enter  New column (horizontal split)
Ctrl+B → v   New pane in column (vertical split)
Ctrl+B → x   Close active pane
Ctrl+B → f   Toggle floating
```

---

## Keyboard Commands

### Prefix Key

heca uses **tmux-style prefix mode**: press `Ctrl+B`, release, then press the action key. This avoids conflicts with applications running inside panes.

- **Prefix timeout**: 500ms — if you don't press a key, prefix mode exits automatically.
- **Double prefix**: pressing the configured prefix twice sends the literal configured prefix key to the focused pane (for example `Ctrl+B Ctrl+B` → `Ctrl+B`, `Ctrl+A Ctrl+A` → `Ctrl+A`).
- **Bare modifiers**: Holding Shift/Ctrl/Alt alone in prefix mode does nothing — wait for the actual key.
- **Display symbol**: in the UI (context menus, hints) the prefix is shown as the symbol **`λ`** — e.g. `prefix+f` renders as `λ f`. This is display-only: config and keybinding strings keep the literal `prefix` token. The substitution lives in one helper, `heca::shortcut::format_shortcut(keys, with_prefix)` (change the symbol there).

### Navigation

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Focus left | `h` | Activate column left, scroll view |
| Focus right | `l` | Activate column right, scroll view |
| Focus up | `k` | Activate pane above in column |
| Focus down | `j` | Activate pane below in column |
| Next pane | `]` | Cycle to next pane across columns |
| Prev pane | `[` | Cycle to prev pane across columns |
| Workspace next | `d` | Switch to next workspace (down) |
| Workspace prev | `u` | Switch to prev workspace (up) |
| Focus toggle (local) | `i` | Toggle between last two panes in same workspace |
| Focus toggle (global) | `Shift+L` | Toggle between last two panes globally |

### Pane Operations

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Split horizontal | `Enter` | New column to the right of active |
| Split vertical | `v` | New pane below active in same column |
| Close | `x` | Close active pane |
| Float | `f` | Toggle pane between scrolling and floating |
| Pane select | `q` | Overlay letters on panes; press letter to focus |
| Swap pane | `Shift+Q` | Overlay letters; swap panes (stay at current position) |
| Swap and focus | `m` | Overlay letters; swap panes (follow to destination) |
| Move column to workspace | `c` | Overlay letters on workspaces; press letter to move the active column there |
| Move pane to workspace | `g` | Overlay letters on workspaces; press letter to move the active pane there |
| Move pane to column | `Ctrl+C` | Overlay letters on columns (any workspace); press letter to stack the active pane into that column |

### Resize

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Resize increase | `=` | Widen active column |
| Resize decrease | `-` | Narrow active column |
| Pane height increase | `Shift+=` | Grow active pane height |
| Pane height decrease | `Shift+-` | Shrink active pane height |

### Font zoom

Two explicit scopes (no implicit resolution): **`Ctrl`** = whole app (chrome/UI
font **and** every terminal pane), **`Ctrl+Shift`** = the focused terminal pane
only, layered on top of the app-wide size. The same gesture is also on
**`Ctrl`/`Meta`+mouse-wheel**, resolved by what's under the pointer (over a pane →
that pane; over chrome/empty → whole app). (`Alt` is avoided: on macOS the Option
key rewrites the typed character, so `Alt+=` never matches.)

| Command | Default Binding | Description |
|---------|----------------|-------------|
| App font bigger (everything) | `Ctrl+=` | Increase chrome/UI + every terminal pane |
| App font smaller (everything) | `Ctrl+-` | Decrease chrome/UI + every terminal pane |
| App font reset (everything) | `Ctrl+0` | Reset to the configured sizes |
| Terminal font bigger (pane) | `Ctrl+Shift+=` | Increase the focused pane's font size |
| Terminal font smaller (pane) | `Ctrl+Shift+-` | Decrease the focused pane's font size |
| Terminal font reset (pane) | `Ctrl+Shift+0` | Focused pane follows the app-wide size |

**Sticky font-size modes** (avoid re-pressing the chord): enter once, then tap keys
repeatedly. `prefix+!` → **app** font mode (whole app), `prefix+@` → **pane** font
mode (focused pane). Inside either mode: `k`/`↑` bigger, `j`/`↓` smaller, `0` reset,
`Esc`/`Enter` exit. Like the built-in `resize` mode (`prefix+r`), fully rebindable.

### Move / Swap (adjacent)

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Move pane left | `Ctrl+[` | Move active pane to column on left |
| Move pane right | `Ctrl+]` | Move active pane to column on right |
| Swap left | `Ctrl+H` | Swap with pane to the left |
| Swap right | `Ctrl+L` | Swap with pane to the right |
| Swap up | `Ctrl+K` | Swap with pane above |
| Swap down | `Ctrl+J` | Swap with pane below |

### Workspace & Sidebar

| Command | Default Binding | Description |
|---------|----------------|-------------|
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

The dock can also be named, which skips the pick: `focus-dock workspaces` over RPC, or a mode binding
carrying `args = { dock = "workspaces" }` (no flat binding form takes args yet). It is one action
either way — the pick is only how a keyboard supplies an argument it cannot type. Aiming it at the
dock that already has focus is the way back out, so one key both takes the keyboard and gives it
back; `unfocus-dock` (RPC) and `Esc` do the same thing explicitly.

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

1. **the surface's own** — `[[keys.surface]]`, below (this placement first, then the
   component as a whole);
2. **the focus layer** — a built-in mode keymap, `[[keys.mode]] name = "focus"`, overridable in
   `keybindings.toml` like any other. It carries what the *widgets* answer — paging and edges for
   whatever scroll area the focused container nests — deliberately not a per-container vocabulary: a
   scroll region behaves the same wherever it is mounted, so nothing has to declare it, and a
   container with nothing scrollable simply declines and the key does nothing.

### A surface's own keys — `[[keys.surface]]`

A surface declares the actions only it can do; the keys for them live in a `[[keys.surface]]`
entry. `name` says which surface — a **field**, not the table name, so a surface may be called
`unbind` or `widgets` without colliding with a config keyword. An optional `id` narrows the entry to
one placement, layered over the id-less one, so two seatings can differ while cursor, scroll position
and focus are per placement anyway.

**A dock and an overlay are the same thing to the keyboard**: something that holds it for a while
and answers keys of its own. So an overlay declares its keys the same way, naming itself by the
addressable name `show_layer` and `hide_layer` use:

```toml
[[keys.surface]]
name             = "heca.expose"   # the pane map
delete_pane      = "x"             # the card the cursor is on
delete_column    = "r"             # the column that card sits in
delete_workspace = "d"             # the workspace that column sits in
```

While a surface holds the keyboard, the app's own bindings behind it are refused — `prefix+j` does
not move the pane behind the map — except for actions that are app-level in every context
(`reload_config`, `close_overlay`, the `prefix+/` picker, `prefix+>`). Which those are is a property
of the **action**, not of the binding, because the same action is reached from a key, a menu entry,
the palette, a button and RPC.

A **way out** of a surface is declared once for every surface of that kind, not per surface: the
`[[keys.mode]] name = "layer"` block is the floor every overlay answers (`Escape`, plus `q` and
`Ctrl+q` as shipped), and `name = "focus"` is the same thing for a focused dock. Declared there they
exist **only while that kind of surface holds the keyboard**, so the program running in a pane keeps
those keys — `:q` still quits vim, and `Ctrl+q` still reaches readline. That is what makes them
different from the global `[keys]` map, which is consulted whether or not anything is in front.

> `[[keys.component]]` is the older spelling of this block and still works unchanged — the two are
> read as one list. New entries should use `[[keys.surface]]`.

Binding names are **short**: under `name = "docker"`, `restart_selected` means the action id
`docker.restart_selected`. You never repeat the component on every line of its own block. An id heca
already knows keeps its own name — a component *binds* existing actions rather than redeclaring them.

```toml
[[keys.surface]]
name             = "docker"
restart_selected = "r"        # → docker.restart_selected
next_pane        = "n"        # …an EXISTING action id, simply bound here — never redeclared

[[keys.surface]]              # the same surface, one placement only
name = "docker"
id   = "docker.right"
restart_selected = "R"        # everything else is inherited from the entry above

[[keys.surface.bind]]         # the arg-carrying form
action = "spawn_command"
keys   = "t"
args   = { command = "lazydocker", float = "true" }

[keys.surface.unbind]         # explicit removal, keyed by the combo
"s" = true
```

**Merge rules, and why the two forms differ.** A TOML table already merges per key, so changing one
`action = "key"` entry keeps every other default. An **array** is replaced wholesale, which for
`[[keys.surface.bind]]` would mean adding one binding silently drops every shipped default — so
those merge **by their `keys` field** instead: a keymap *is* a map from combo to action, so merging on
the combo is the ordinary table rule applied to what the array is really keyed by. `unbind` is applied
last and keyed by the **combo**, so it retires a binding whatever it points at.

**Where the defaults are.** heca's own components ship their keys in `keybindings.default.toml`, in
this same shape, at the bottom of the file — one place a key is written and one place to change it. A
**plugin** has no entry in that merge, so it registers its keys at runtime instead; whatever your
config says still wins, and a registration never overwrites a combo you bound or gives a rebound
action a second key. Binding an id whose component is not mounted is not an error: like every binding
it resolves at press time, and simply does nothing until that component appears. Nor is an `id` for a
placement that never exists.

### The workspaces dock's own keys

heca's own workspaces component ships these in `keybindings.default.toml`. They apply **only while
that dock holds the keyboard** (`prefix+e`, or click it), so they need no prefix and no mode.

| Key | Action | What it acts on |
|-----|--------|-----------------|
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

### Two keys every container has for free

A container does not have to declare anything to be usable:

- **`global_focus`** — written in its `[[keys.surface]]` entry, but it applies while the container
  does *not* have focus, which is the only time it is useful. heca therefore binds it in the global
  map rather than the container's own layer. With an `id` it aims at that seating; without one it
  names the component and lands on the seating you were last in. Pressing it again while that
  container holds the keyboard gives it back. The name is reserved — a component cannot have an
  action called `global_focus`.
- **`Escape`** — gives the keyboard back to the main region. Always bound, for every container, and
  **not removable**. You can add other ways out by binding `unfocus_dock`; you cannot take this one
  away, because a dock that declares nothing must still be leavable without the mouse.

Two components asking for the same `global_focus` combo is reported at startup like any other
collision.

### Finding a key — `heca --keys-show`

Keys are no longer all in one file, so reading config can no longer answer "what runs this action":
a mode's keys are in `[[keys.mode]]`, a surface's in `[[keys.surface]]`, and a plugin's are in no
file at all. `heca --keys-show` prints every binding name, the key it resolves to and the layer it
came from — read out of the **built keymaps**, so it sees all of them alike. `--json` emits the same
three facts per line for scripting.

```
$ heca --keys-show
BINDING                       KEY              LAYER
close                         prefix+x         [keys]
workspaces.cursor_down        j                [[keys.surface]] workspaces
docker.restart                r                plugin
```

| Key | Action |
|-----|--------|
| `PageUp` / `PageDown` | Scroll the focused dock one page up / down |
| `Home` / `End` | Jump the focused dock to top / bottom |
| `Alt+PageUp` / `Alt+PageDown` | Scroll the focused dock one page left / right |
| `Alt+Home` / `Alt+End` | Jump the focused dock to its left / right edge |
| `Esc` | Give the keyboard back to the focused pane |

The bare keys are free here precisely because nothing is being forwarded to a backend. The `Shift+`
scroll bindings under "Direct (non-prefix) keybindings" are unchanged and, while a dock is focused,
aim at the dock too — one binding, one meaning: *scroll whatever has the keyboard*. Horizontal
scrolling exists only here (`scroll_page_left`, `scroll_page_right`, `scroll_to_left_edge`,
`scroll_to_right_edge`, also reachable over RPC as `direct-scroll-page-left` and friends), because a
terminal viewport has a single axis.

### Navigating the workspaces dock

**There is no sidebar mode.** Which dock the keyboard is aimed at is a *container id* held in the
chrome store, not an input mode, so nothing has to be entered or left: `prefix+e` (or a click) gives
the keyboard to the workspaces dock, `Esc` gives it back, and while it is there the dock's own keys
apply — [the table above](#the-workspaces-docks-own-keys), rebindable in `[[keys.surface]]`.

`l` / `Right` / `Enter` focus the row **and hand the keyboard back**; `Space` focuses it **and keeps
the keyboard on the dock**, so you can keep walking with `j`/`k` and preview each row. The mutation
keys act on the row the cursor is on; the global collapse bindings (`<` and `(`) act on the active
main-view workspace/column instead and do not touch the dock.

**A click does the same thing as a key.** Clicking anywhere in a container gives it the keyboard and
moves its cursor to the row you clicked — so `j` continues from there — and clicking outside every
container gives the keyboard back. Right-clicking a row opens that row's menu, and aims the keyboard
the same way first.

### Terminal Scrollback & Selection

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
|-----|--------|
| `h` / `l` / `j` / `k` or arrow keys | Move caret left / right / up / down (the viewport auto-scrolls at edges) |
| `PageUp` / `PageDown` | Move caret up / down by one page |
| `u` / `d` | Scroll viewport up / down by half-page |
| `Ctrl+u` / `Ctrl+d` | Scroll viewport up / down by one full page |
| `g` | Jump to top of scrollback history |
| `Shift+g` | Jump to the live bottom (stay in selection mode) |
| `v` / `Space` | Begin selection (toggle highlighting) |
| `o` | Toggle selection endpoint |
| `Shift+o` | Open the hyperlink under the caret |
| `y` | Copy selection to clipboard |
| `/` | Search the scrollback — type the query (matches highlight live, the caret jumps to the nearest); `Enter` keeps the matches, `Esc` cancels |
| `n` / `Shift+n` | Jump to the next / previous search match (after a `/` search) |
| `Esc` | Exit selection mode (snap to bottom + clear selection) |

**Wheel:**
- At a normal shell prompt → scrolls host scrollback viewport by
  `terminal_wheel_scroll_lines` rows per notch (configurable).
- Over a mouse-grabbed TUI (vim, htop, less, etc.) → forwarded to the terminal
  as mouse events.
- **Shift+wheel** → always scrolls host viewport, bypassing any mouse grab.
- Scrolling up from the live bottom automatically enters Selection mode.
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

### Direct (non-prefix) keybindings

For users who prefer not to use prefix mode, heca provides direct bindings
that work without the prefix key. These are intercepted before reaching the
terminal:

| Combo | Action |
|-------|--------|
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

### Scrollback GUI

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

### System

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Command palette | `p` | Open command palette |
| Reload config | `Shift+R` | Reload config.toml at runtime — a toast confirms success, or reports the parse error with a **Retry** button (the working config is kept on failure; the error also prints to stderr) |

#### The command palette

Every action heca knows about, searchable from one key — including those a mounted
component contributes, which are listed under that component's name and focus it
before running.

- **Fuzzy, smart-case matching.** Type lowercase and case is ignored; type an
  uppercase letter and it starts to matter. Set `search_case` to `"sensitive"` or
  `"insensitive"` in `config.toml` if you'd rather it never guessed.
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

---

## Configuration

heca loads config from `~/.config/heca/config.toml` (general settings) and
`~/.config/heca/keybindings.toml` (keybindings) — `%APPDATA%\heca\…` on Windows.
Both are optional and deep-merged over the built-in defaults, which live in two
versioned, embedded files (the single source of truth — copy and edit them):
- `config.default.toml` — default `[settings]`, `[appearance]`, `[font]`, `[program]`
- `keybindings.default.toml` — default `[keys]` (prefix, bindings, modes)

### Minimal Config

```toml
# Prefix key (default: "ctrl+b")
prefix = "ctrl+a"

# Theme
# Built-in: "mocha" (dark), "latte" (light)
theme = "mocha"

# Window settings
[settings]
window_width = 1280
window_height = 800
mouse = true
shell_integration = true

# Keybindings — prefix+ syntax for prefix bindings, direct for global
[keys]
focus_left = ["prefix+h", "prefix+ArrowLeft"]
focus_right = ["prefix+l", "prefix+ArrowRight"]
focus_up = ["prefix+k", "prefix+ArrowUp"]
focus_down = ["prefix+j", "prefix+ArrowDown"]
split_horizontal = "prefix+Enter"
split_vertical = "prefix+v"
zoom_column = "prefix+z"
close = "prefix+x"
float = "prefix+f"
pane_select = "prefix+q"
swap_pane = "prefix+Shift+q"
swap_and_focus_pane = "prefix+m"
move_column_to_workspace_pick = "prefix+c"
move_pane_to_workspace_pick = "prefix+g"
move_pane_to_column_pick = "prefix+Ctrl+c"
rename_workspace = "prefix+Shift+w"
rename_pane = "prefix+$"
rename_column = "prefix+Shift+c"
```

### Config File Format

The config uses TOML with a flat `[keys]` table for simple actions, plus structured tables for richer bindings:

```toml
[keys]
# Single binding
focus_left = "prefix+h"

# Multiple bindings (list syntax)
focus_left = ["prefix+h", "prefix+ArrowLeft"]

# Global binding (no prefix — works in Normal mode)
# These are checked before forwarding to terminal
Alt+Enter = "spawn_lazygit"   # Requires [[keys.command]]

# Unbind defaults
[keys.unbind]
"prefix+f" = true   # Remove float toggle
```

### Planned parameterized keybindings contract

The agreed next-step config shape for richer actions is:

```toml
[[keys.bind]]
keys = "prefix+z"
action = "zoom_column"

[[keys.bind]]
keys = "prefix+g"
action = "spawn_pane"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

[[keys.bind]]
keys = "prefix+Shift+b"
action = "spawn_pane"
args = { kind = "browser", float = true, width = "1200", height = "800" }

[[keys.bind]]
keys = "prefix+Shift+n"
action = "spawn_pane"
args = { kind = "nvim_gui", float = true, width = "1000", height = "700" }

[[keys.bind]]
keys = "prefix+Shift+f"
action = "float_active_at"
args = { width = "95%", height = "95%" }
```

Mode bindings should support the same action+args structure:

```toml
[[keys.mode]]
name = "spawn"
trigger = "prefix+s"
sticky = true

[[keys.mode.bindings]]
action = "spawn_pane"
keys = "t"
args = { kind = "terminal", program = "btm", argv = [], float = true, width = "800", height = "400" }
```

Size parsing contract:
- `800` → `800px`
- `800px` → explicit pixels
- `80%` → percentage of available content area

Floating spawns should open **centered by default** when `x/y` are omitted.

Complete example set:

```toml
[keys]
focus_left = "prefix+h"
float = "prefix+f"

# Unit action via parameterized-normal-binding syntax
[[keys.bind]]
keys = "prefix+z"
action = "zoom_column"

# Tiled terminal pane
[[keys.bind]]
keys = "prefix+g"
action = "spawn_pane"
args = { kind = "terminal", program = "lazygit", argv = [] }

# Floating terminal pane with implicit px values
[[keys.bind]]
keys = "prefix+Shift+t"
action = "spawn_pane"
args = { kind = "terminal", program = "btm", argv = [], float = true, width = "800", height = "400" }

# Floating terminal pane with explicit px suffix
[[keys.bind]]
keys = "prefix+Shift+y"
action = "spawn_pane"
args = { kind = "terminal", program = "htop", argv = [], float = true, width = "800px", height = "400px" }

# Floating terminal pane with percent sizing
[[keys.bind]]
keys = "prefix+Shift+g"
action = "spawn_pane"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

# Floating browser pane
[[keys.bind]]
keys = "prefix+Shift+b"
action = "spawn_pane"
args = { kind = "browser", float = true, width = "1200", height = "800" }

# Floating nvim GUI pane
[[keys.bind]]
keys = "prefix+Shift+n"
action = "spawn_pane"
args = { kind = "nvim_gui", float = true, width = "1000", height = "700" }

# Float active pane with geometry helper
[[keys.bind]]
keys = "prefix+Shift+f"
action = "float_active_at"
args = { width = "95%", height = "95%" }

[[keys.mode]]
name = "spawn"
trigger = "prefix+s"
sticky = true

# Tiled spawn from a mode
[[keys.mode.bindings]]
action = "spawn_pane"
keys = "g"
args = { kind = "terminal", program = "lazygit", argv = [] }

# Floating percent-sized spawn from a mode
[[keys.mode.bindings]]
action = "spawn_pane"
keys = "n"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

# Zoom from a mode
[[keys.mode.bindings]]
action = "zoom_column"
keys = "z"
```

### Key Combo Syntax

```
prefix+h           → Press prefix, then h
prefix+Shift+q     → Press prefix, then Shift+q
prefix+Ctrl+h      → Press prefix, then Ctrl+h
Alt+Enter          → Global Alt+Enter (no prefix)
Super+t            → Global Super+t (no prefix)
F1                 → Global F1
```

**Modifiers:** `Ctrl`, `Shift`, `Alt`, `Super` (also accepts `Win`, `Cmd`)

**Named keys:** `Enter`, `Tab`, `Escape`, `Backspace`, `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`, `Space`, `Delete`

### Theme

Built-in themes: `mocha` (dark), `latte` (light). Create custom themes in `~/.config/heca/themes/mytheme.toml`:

```toml
name = "My Theme"
background = "#1e1e2e"
foreground = "#cdd6f4"
border = "#313244"
accent = "#89b4fa"
# Fonts are decoupled from the color theme (they are system-local, not
# theme-portable) — set them under [font] (see below), not here.
border_radius = 6.0
border_width = 1.0

# Sidebar drag-and-drop colors
sidebar_drag_ghost_bg = "#89b4faD9"     # Ghost label background (accent @ 85%)
sidebar_drag_ghost_fg = "#ffffff"        # Ghost label text (white)
sidebar_drag_source_bg = "#89b4fa26"     # Source item background (accent @ 15%)
sidebar_drag_source_border = "#89b4fa"   # Source item border (accent)

# Sidebar font sizes
sidebar_label_font_size = 14.0    # Workspace/column/pane labels
sidebar_button_font_size = 11.0   # [+w] [+c] [+p] [-] buttons

# Effect tokens (also overridable under [appearance])
glow_size = "medium"   # none | thin | medium | large — glow presence + halo radius + strength
intensity = "medium"   # off | low | medium | heavy — scanline/CRT overlay opacity only (NOT glow)

# Overlay panels (dialogs, dropdowns, context menus, command palette)
overlay_border_style = "bracketed"   # bracketed | bordered | none — the shared frame around an
                                     # overlay panel. `bracketed` = accent corner reticle (default),
                                     # `bordered` = plain edge, `none` = no frame. Each widget's own
                                     # accent border/glow is separate and unaffected.

# Keyboard focus outline
show_focus_border = true   # draw the focus ring at all
# focus_ring = "#7fd3ff"   # focus-outline color; unset = the accent shifted toward `foreground`
                           # (auto-brightens on dark themes, darkens on light themes). The
                           # destructive/danger focus ring always derives the same way.

# Sidebar highlight alphas (0.0–1.0; optional, shown with their defaults)
active_wash_alpha = 0.11          # Accent wash over the active workspace
card_background_alpha = 0.02      # Resting background tint of each pane card

[shadow]
color = "#000000"
alpha = 0.3
blur = 8.0
```

### Appearance & Frost (z=0 background layer)

Frosted-glass frost is **heca-owned**, not OS-dependent: heca renders a blurred
vertical gradient as the bottom-most (z=0) layer, and panes composite
translucently over it. This works identically on Linux, Windows, and macOS — no
OS vibrancy required (vibrancy is an optional platform backdrop material, off by
default).

`[appearance]` is organized as a flat app-wide section plus three nested
per-surface tables — `[appearance.terminal]`, `[appearance.pane]`,
`[appearance.sidebar]`. Border fields inherit: a surface value wins; unset →
the global `[appearance]` value; unset → the theme.

```toml
[appearance]
# z=0 background layer (the frosted gradient behind everything)
background_blur          = 0    # 0..=100 — blur strength (0 = sharp gradient)
background_transparency  = 0    # 0..=100 — 0 = opaque (default); >0 = translucent
# background_gradient_top    = "#1e1e2e"   # optional; unset → theme.background
# background_gradient_bottom = "#181825"   # optional; unset → derived darker shade

[appearance.terminal]
# Tiled panes: frost = z=0 showing through the translucent terminal surface.
transparency = 0             # 0..=100 — surface alpha over z=0
# Floating panes: their own REAL blur of the tiled content behind them.
floating_transparency = 0
floating_blur         = 0    # 0..=100
# Scrollback scrollbar visibility for terminal panes with history:
#   always       — always show the scrollbar
#   when_needed  — show only while scrolled away from the live bottom (default)
#   never        — never show
show_scrollbar = "when_needed"
# Scrolled-up indicator badge ("N lines above", click snaps to bottom).
# true | false (default true)
show_scrolled_up_badge = true
```

**How frost is produced:**
- **Tiled panes** — set `background_blur > 0` (frosts the gradient) +
  `[appearance.terminal] transparency > 0` (lets it show through the terminal
  surface). There is **no per-terminal tiled blur knob** — the z=0 layer is the
  frost source.
- **Floating panes** — set `[appearance.terminal] floating_blur > 0` +
  `floating_transparency > 0`; the blurred tiled content is stamped
  behind the floating pane at full opacity (no sharp text leak).

> **Migration:** the old `terminal_blur` and `terminal_frost_color` knobs are
> **removed**. Use `background_blur` for tiled frost strength. (`terminal_blur`
> only ever tinted behind panes and could not blur the desktop; the z=0 layer is
> the real cross-platform fix.)

Reload any of these at runtime with `prefix+Shift+r`.

#### Effect tokens (`glow_size` / `intensity`)

The two effect tokens live on the theme but are also overridable under `[appearance]` (unset → inherits the theme):

```toml
[appearance]
glow_size  = "medium"   # none | thin | medium | large — glow presence + halo radius + strength
intensity  = "medium"   # off | low | medium | heavy — scanline/CRT overlay opacity only
```

They are **independent dimensions**: `glow_size` is the sole owner of glow
(presence + radius + strength); `intensity` owns the scanline/CRT overlay
opacity only and does **not** affect glow (the older docs that said `intensity`
drove "glow + scanlines" were wrong). Reload at runtime with `prefix+Shift+r`.

### Settings

```toml
[settings]
window_width = 1280           # Initial window width
window_height = 800           # Initial window height
mouse = true                  # Enable mouse interactions
focus_follows_mouse = true    # Focus pane on hover
auto_scroll_edge = true       # Auto-scroll near edges
interactive_move_modifier = "Super"  # Modifier for drag-and-drop
shell_integration = true      # Auto-inject OSC 133/OSC 7 shell hooks for runtime status + cwd
pane_renamed_add_process_name = true  # Renamed pane shows its process name small, e.g. `MyPane (nvim)`
pane_show_cwd = false         # Show each pane's cwd as a row in the sidebar card (folder icon + path)
terminal_mouse = true         # Enable host scrollback on wheel (vs forwarding to terminal)
terminal_wheel_scroll_lines = 3  # Rows per wheel notch when scrolling host viewport
terminal_scroll_animations = true  # Smooth animated terminal viewport jumps

[settings.notification_system]
mode = "app"                  # "app" (in-app toast stack) | "system" (OS notifications —
                              # reserved, falls back to "app" until the backend exists) |
                              # "none" (no notifications at all)
auto_dismiss_ms = 4000        # How long an in-app toast stays before dismissing itself (ms).
                              # A notification a producer marks sticky, or one with its own
                              # lifetime, ignores this. Applies on prefix+Shift+r.
                              # The countdown pauses while the pointer rests on a card.
max_visible = 5               # How many toasts are on screen at once. The rest queue in the
                              # order they were raised — nothing is dropped — and each takes
                              # the slot that frees. Minimum 1.
```

### Confirmation prompts

Destructive actions can pop a **confirm dialog** before running. This is declared
**per action** and configured in the generic `[confirm]` table, keyed by action
name — `true` prompts (Cancel / \<action\>), `false` runs immediately:

```toml
[confirm]
close = true            # confirm before closing a pane
delete_column = true    # confirm before deleting a column (and its panes)
delete_workspace = true # confirm before deleting a workspace (and its contents)
```

The guard lives on the **action**, so *every* surface that triggers it — a
keybinding, the pane-header close button, a right-click menu, or RPC — confirms
identically. Any action (including a plugin's) is configurable by name here; an
action not listed uses its own declared default. Changes apply on
`prefix+Shift+r`.

> Replaces the old `[settings] confirm_close_pane` / `confirm_delete_column` /
> `confirm_delete_workspace` flags — move any you had set into `[confirm]` as
> `close` / `delete_column` / `delete_workspace`.
>
> The pane toggle was once `delete_pane`; it still works, but `close` is the name
> now — one word for the action, its key, its label and its dialog. **Close** is
> for the pane you work in; **Delete** stays for a column or workspace, which
> destroys everything inside it.

### Notifications

heca shows short **toast notifications** in the top-right corner — a pane was
created, a process exited, the config reloaded, a command could not be run.

```toml
[settings.notification_system]
mode = "app"          # "app" (in-app toast stack) | "system" (OS notifications —
                      # reserved, falls back to "app" until that backend exists) |
                      # "none" (no notifications at all — stderr logs are unaffected)
auto_dismiss_ms = 4000 # how long a toast stays before dismissing itself
max_visible = 5        # how many are on screen at once; the rest queue
```

- **Severity is tone, not importance.** `info` / `success` / `warning` / `danger`
  change the accent colour; they do **not** change how long a toast stays. Every
  toast auto-dismisses after `auto_dismiss_ms` unless the code that raised it
  marked it sticky (a config-reload failure is sticky — you fix it and press
  **Retry**).
- **Interacting:** click a toast's × or one of its buttons; `prefix+/` puts pick
  letters on the visible toasts' buttons and their × , and `prefix+n` dismisses.
  **Resting the pointer on a card holds the whole stack still**, so nothing retires
  from under the click you are aiming at — the time that costs is given back when
  you move away, and each card resumes with what it had left.
- **More than fits:** only `max_visible` cards show at once. The rest wait in the
  order they were raised — nothing is dropped. Closing one slides the cards after it
  up so the column never has a blank in it, and the next in line appears at the
  bottom, where a card that has just arrived belongs.
- **Raising one** — from a keybinding, a script, or a plugin — is the `notify`
  action: `notify title="Build finished" body="3 warnings" severity=success`.
  Rust code in the app uses `Notification::info("…").body("…").send()`.
- What produces a toast today: a new pane, a pane whose process exited while the
  pane stays open, `prefix+Shift+r` (success or the parse error), and a command
  that fails to spawn.

### Fonts

Fonts are **system-local, not theme-portable** — a color theme that shipped a
`font_family` would break on a system without that font. So fonts live in a
dedicated `[font]` block, independent of the color theme: switching theme keeps
your fonts, and a theme never requires a specific installed font.

```toml
[font.family.ui]
normal = "Geist Mono"          # UI/chrome font (omit → embedded Geist Mono fallback)
# bold = "..."                  # optional → falls back to normal
# italic = "..."                # optional → falls back to normal + synthesized oblique
# bold_italic = "..."           # optional → falls back to italic → bold → normal

[font.family.terminal]
normal = "Maple Mono Normal NF"  # Terminal font (omit → embedded Maple Mono fallback)
# italic = "..."                # optional distinct italic family

[font.size]
ui = 15.0                      # UI/chrome font size (default 15.0)
terminal = 14.0                # Terminal font size (default 14.0)
```

`normal` is optional — if omitted, the surface falls back to its **embedded**
font (Geist Mono for UI, Maple Mono Normal NF for terminal), so a theme never
depends on a system-installed font. `bold` / `italic` / `bold_italic` are
optional per-style family slots. When unset, the renderer falls back to
`normal` and selects the face via weight/style within the family (bold face via
`Weight::BOLD`; italic via a synthesized oblique when the family has no italic
face). When set, the renderer uses the named family for that style — so you can
point bold/italic at a different installed font without touching the regular
family.

#### Terminal ligatures

The terminal shapes each row a **run at a time** (consecutive cells sharing a
font face are shaped together), so a coding font's OpenType ligatures and
contextual alternates — `=>`, `==`, `>=`, `!=`, `->`, … — render in terminal
panes. Glyphs are still snapped to the cell grid, and ligatures form across
foreground-color boundaries (each half keeps its own cell's color), matching how
kitty/WezTerm behave.

**Which ligatures appear is up to the font** — whatever your terminal font
ligates under standard shaping (`calt`/`liga`/`clig`); heca enables no font
features of its own. The embedded default, **Maple Mono Normal NF**, renders the
full set (`=>` `->` `==` `!=` `>=` `|>` …). To use a different coding font, point
the terminal family at any installed font:

```toml
[font.family.terminal]
normal = "JetBrainsMono Nerd Font Mono"   # any installed system font is found
```

To turn ligatures off entirely (each character renders standalone), set
`ligatures = false` under `[appearance.terminal]`:

```toml
[appearance.terminal]
ligatures = false   # default true; disables calt/liga/clig for the terminal font
```

Reload at runtime with `prefix+Shift+r` — the toggle and font both apply live.

#### Terminal hyperlinks

OSC 8 hyperlinks emitted by programs are decorated in terminal panes. The
decoration and color are configurable under `[appearance.terminal]`:

```toml
[appearance.terminal]
hyperlink_style = "underline"   # none | color | underline | undercurl (default underline)
# hyperlink_color = "#5fafff"   # omit → theme accent
```

`none` leaves links looking like normal text; `color` recolors only; `underline`
and `undercurl` add a straight or wavy line in the link color. Plain-text URLs
(`echo "https://…"`, log output) are auto-detected as links too; set
`link_detection = false` to mark only explicit OSC 8 links.

**Opening links.** Detected and OSC 8 links open the same way through the OS
handler (`open` / `xdg-open` / `start`):

- **Mouse** — hold the interactive-move modifier (Cmd by default) and click a
  link; the cursor turns into a pointer while it is held over one. A plain click
  still goes to the terminal program.
- **Keyboard** — `prefix+Shift+o` (`follow_link`) labels every visible link across
  all on-screen panes with a letter; press the letter to open it, or `Esc` to cancel.
- **Selection mode** — `Shift+o` opens the link under the caret (`o` stays the
  flip-endpoint binding).
- **Context menu** — right-click a terminal pane for a menu with **Open link**
  (shown only when the click cell is a link) plus pane actions (split, zoom,
  float, close), each with its icon and keybinding hint. Click an entry or use the arrow
  keys + Enter; `Esc` or an outside-click dismisses. (Right-clicking right on a
  resize seam still starts a right-drag resize.)

When `shell_integration = false`, heca spawns a bare interactive shell and you can source the generated snippets manually from `~/.config/heca/runtime/shell-integration/`.

#### Terminal bell

A terminal bell (`\a`) can drive up to three independent cues, configured under
`[appearance.terminal]`:

```toml
[appearance.terminal]
bell_attention = true   # OS attention cue (Dock bounce / taskbar flash) while unfocused
bell_visual    = false  # brief accent flash over the content area
bell_audible   = false  # system beep (macOS only for now; no-op elsewhere)
```

`bell_attention` only fires while the window is unfocused; the visual and audible
bells fire on any bell. All default off except `bell_attention`. Changes apply
live with `prefix+Shift+r` (no restart).

### Pane Info Bar

Each pane shows a small **info bar** along its top: configurable **segments** on the
left (what the pane is) and **action buttons** on the right. Both are configured
under `[appearance.pane]` as ordered lists — order in the list is the order shown
(left → right). An empty list hides that side; if **both** are empty the bar (and
its reserved space) disappears entirely.

When a pane is **renamed** (given a custom name), its **sidebar card** shows the custom
name followed by the process name in a small dimmed label — e.g. `MyPane (nvim)` — so the
sidebar keeps surfacing what's actually running. The process label is not part of the name
(it's never edited by rename); toggle it with `[settings] pane_renamed_add_process_name`
(default `true`). In the pane **info bar**, the `app_name` segment always shows the program
name regardless of a rename; add the `pane_name` segment to show the custom name there.

```toml
[appearance.pane]
# Left side — what to show, in order. A segment with no data for a pane is skipped
# (e.g. git segments outside a repo).
title_segments = ["location", "app_name", "git_branch", "git_status"]

# Right side — action buttons, in order. Each button's tooltip shows its real
# configured keybinding.
title_actions = ["split", "close"]

[appearance.sidebar]
# Sidebar shell appearance (independent of the panes; all optional):
width            = 300          # Sidebar width in px (clamped 160..=560)
border_style     = "bordered"   # none | bordered | bracketed
border_width     = 1.0          # frame width px; unset → global border_width
border_radius    = 12.0         # corner radius px; unset → global/theme radius
border_color     = "#40e0ff"    # bordered-frame color; unset → global border_color
background_color = "#0b0f14"     # shell fill; unset → theme sidebar surface
```

With `border_style = "bordered"`, the frame is drawn at `border_width` in
`border_color` (which falls back to the global `[appearance] border_color`). With
`"bracketed"` it uses the theme accent corner-reticle, now sized by the same
`border_width` + `border_radius`. `border_width = 0` removes the border.

**Supported segments** (`[appearance.pane] title_segments`):

| Value         | Shows                                                        |
|---------------|-------------------------------------------------------------|
| `location`    | Working directory (home-relative path)                      |
| `app_name`    | Resolved program / app name (from the [Process Catalog](#process-catalog)) — always the running program, never a rename |
| `pane_name`   | The pane's own name: the custom rename when set, else the program name (`custom`-wins). Opt-in — not in the default segments |
| `git_branch`  | Git branch — hidden outside a repo                          |
| `git_status`  | Git change counts `+A ~M -D` — hidden when clean / no repo  |

Default: `["location", "app_name"]`.

**Supported actions** (`[appearance.pane] title_actions`):

| Value        | Button does                          |
|--------------|--------------------------------------|
| `split`      | Add a pane to the column             |
| `close`      | Close the pane                       |
| `zoom`       | Toggle zoom (maximise) the column    |
| `float`      | Toggle floating for the pane         |
| `move_left`  | Move the pane left                   |
| `move_right` | Move the pane right                  |

Default: `["split", "close"]` (move actions are omitted by default since panes are
already movable by mouse-dragging, but they remain valid config values). Each
button's tooltip shows the **real configured keybinding** for that action.

---

## Actions System

Every WM command in heca is an **action**. Actions are the core abstraction — everything from focusing a pane to resizing a column to spawning an application is an action.

### Action Types

**Unit actions** — Simple commands with no arguments:
- `focus_left`, `focus_right`, `split_horizontal`, `close`, `float`

**Parameterized actions** — Commands with arguments:
- `FocusPane { pane_id }` — Focus a specific pane by ID
- `FocusWorkspace { ws_idx }` — Focus a workspace by index
- `Swap { a_id, b_id }` — Swap two panes
- `Resize { target, axis, amount }` — Resize column or pane
- `SpawnCommand { command, kind, float, close_policy }` — Run an external command in a new pane

**Name-keyed actions** — contributed at **runtime** by a provider or plugin, identified by a stable
string id (`plugin.docker.restart`). They have no `WmAction` variant — the enum is closed — but they
are otherwise ordinary actions: same catalog, same metadata, same policy, same dispatch.

### Registry Dispatch

Every action goes through one dispatch door, whatever surface triggered it:

```
Keyboard  → KeyCombo → KeymapRegistry → ActionRef ─┐
Mouse/UI  → Intent {action, args} ─────────────────┤
RPC       → command ───────────────────────────────┤
                                                   ▼
                                          [interaction policy]
                                                   │
                        built-in ──► WmAction ──► native handler
                        name-keyed ─────────────► registered handler
```

A binding resolves to an `ActionRef`: a **built-in** is resolved when config loads (same cost and same
arg-parsing errors as before), while a **name-keyed** id is resolved at **press** time — because heca
reads your config before any provider has registered its actions.

This design means:
- Every action is traceable and hookable, and is judged by the same interaction policy
- Any action can be triggered **by name** — from a keybinding, a menu, a plugin, or RPC
- Arguments take one path: a menu item, an RPC call and a config binding construct the identical action
- Important capabilities should not be trapped behind one surface: when meaningful, the same action should be reachable from mouse/UI, keybindings, and RPC

### Creating Custom Actions

There are **two** ways to add an action, and which one you want depends on where the code lives.

- **A built-in action** — you're editing heca itself. It gets a `WmAction` variant and a native
  handler. This is the path for a new WM capability.
- **A name-keyed action** — a provider or (later) a WASM plugin contributes an action at **runtime**.
  `WmAction` is a closed enum, so it *cannot* get a variant; it is identified by a stable **string
  id** instead. This is the path for anything that isn't core heca.

Both end up equal citizens: same catalog, same metadata, same interaction policy, same single dispatch
door — so a menu item, a keybinding and an RPC call all reach either one identically.

#### A. A built-in action (in-tree)

Say you want `my_custom_action`.

**1. Add the variant** — `heca/src/input.rs`:
```rust
pub enum WmAction {
    // ... existing variants
    MyCustomAction,
}
```

**2. Give it a name.** A unit action goes in `action_from_name()`; one that takes arguments goes in
`build_action()` instead (that's what parses a binding's `args`, an `Intent`'s args, and RPC args —
one constructor for all three):
```rust
// unit:
"my_custom_action" => Some(WmAction::MyCustomAction),

// or parameterized, in build_action():
"my_custom_action" => Some(WmAction::MyCustomAction { amount: get_usize(args, "amount")? }),
```

**3. Classify it — `action_policy()` in `heca/src/app/interaction.rs`.** This match is **exhaustive**:
if you skip this step, **it will not compile.** That is deliberate — it forces you to answer "should
this run while a floating pane owns the focus domain?"
```rust
WmAction::MyCustomAction => ActionPolicy::TiledOnly,
```
> ⚠️ `AlwaysAllowed` is a **misnomer** — the router *blocks* it when a floating pane is active.
> `Global` is the only policy that is truly always allowed. See
> [Interaction Policy](#interaction-policy) below.

**4. Write the handler** — `heca/src/handlers.rs`:
```rust
pub fn handle_my_custom_action(state: &mut AppState, _action: &WmAction) {
    // Your logic here.
    state.needs_redraw = true;
}
```

**5. Register the handler** in `build_registry()` — `heca/src/app/registry.rs`:
```rust
registry.register(&WmAction::MyCustomAction, handle_my_custom_action);
```

**6. Describe it** in `ActionRegistry::ALL` — `heca/src/actions.rs`. This is what gives the action its
label, icon and category everywhere it is shown (context menu, tooltip, command palette), **and what
says which arguments it takes**:
```rust
ActionDescriptor {
    name: "my_custom_action",
    label: "My Custom Action",
    description: "What it does, in one line.",
    category: ActionCategory::Pane,
    default_binding: "y",
    icon: Some(Glyph::Gear),   // any Glyph; None if it has no icon yet
    args: &[],                 // takes none — see below if it does
},
```

`args` has no default: a struct literal has to fill every field, so you cannot add an action without
answering the question. **Empty means it takes none**, and every argument handed to it is then an
argument nobody declared — which is reported, not ignored.

An action that reads arguments in `build_action()` declares each one here, in the same order:
```rust
args: &[
    ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to act on."),
    ArgDescriptor::optional("focus_after", ArgKind::Bool, "Focus it afterwards. Default false."),
    // A vocabulary argument points at the list beside its own parser, never a copy of it:
    ArgDescriptor::required_enum("step", <FontZoomStep as EnumArg>::VALUES, "Which way to step."),
],
```

This is what makes a mistake audible. heca compares every call against this list — a `config.toml`
binding when config is read, an `Intent` when it is dispatched — and names what is wrong:

```
[heca] binding 'my_custom_action': unknown argument 'pane_di' — did you mean 'pane_id'?
[heca] binding 'my_custom_action': missing required argument 'pane_id'
[heca] binding 'my_custom_action' cannot be built and will do nothing when pressed
```

A missing required argument stops the action (nothing can build it). An unknown or malformed one
costs only itself: the rest of the call still stands. Three tests keep the declaration and the arm
that reads it in step, so a list that stops matching the code fails the build rather than misleading
someone later.

> **An action that requires an argument gets no default keybinding**, and cannot be reached from its
> bare name at all. A key supplies no pane id, so a name that needs one would have to guess — which
> heca used to do, answering `delete_workspace` with *workspace 0*. Bind such an action with an
> explicit `args` table (see [`[keys.mode]`](#modes)) or dispatch it from a menu, a plugin or RPC.

**7. Bind it by default** in **`keybindings.default.toml`** — *not* in Rust. The embedded default
files are the single source of truth for defaults; a default that lives only in code is invisible to
users:
```toml
[keys]
my_custom_action = "prefix+y"
```

**8. Give it RPC parity** — `heca/src/rpc.rs`. A capability must not be trapped behind one surface: it
should be reachable from **mouse/UI, keyboard, and RPC** whenever each is meaningful.
```rust
"my-custom-action" => Ok(WmAction::MyCustomAction),
```

**9. Optional — make it confirm first.** Destructive actions declare a `ConfirmSpec`, and *every*
surface that triggers them confirms identically, because the guard lives on the **action**, not the
call site. Users toggle it in the [`[confirm]`](#confirmation-prompts) table.

#### B. A name-keyed action (provider / plugin)

Registered at runtime, identified by a **stable string id**. Because `WmAction` is closed, this is the
only way for code outside core heca to contribute an action.

Metadata and handler are registered together but stored apart, for a borrow reason rather than a
design one: a handler is `fn(&mut AppState, …)` and never receives the registry, so **metadata** must
be reachable from `AppState` (the `ActionCatalog`) while the **handler** table must be borrowable
alongside `&mut AppState` (the `ActionRegistry`).

```rust
use crate::actions::{register_dynamic, unregister_dynamic, ActionCategory, ActionMeta};
use crate::app::interaction::ActionPolicy;
use crate::chrome::PropValue;
use heca_grid_ui::Glyph;
use std::rc::Rc;

// At mount:
let handle = register_dynamic(
    registry,   // &mut ActionRegistry
    catalog,    // &mut ActionCatalog
    ActionMeta {
        name: "plugin.docker.restart".into(),   // the id bindings / menus / RPC name
        label: "Restart Container".into(),
        description: "Restart the selected Docker container.".into(),
        category: ActionCategory::System,
        default_binding: String::new(),
        icon: Some(Glyph::Play),
        policy: ActionPolicy::Global,           // REQUIRED — no permissive default exists
    },
    // Receives the Intent, so its args arrive as plain data — no closure ever crosses a plugin
    // boundary. `&mut AppState` is the sanctioned write path for an action.
    Some(Rc::new(|state, intent| {
        let Some(PropValue::Text(container)) = intent.args.get("container") else {
            return;
        };
        restart_container(state, container);
        state.needs_redraw = true;
    })),
);

// At unmount — retires the handler AND the metadata:
unregister_dynamic(registry, catalog, &handle.0);
```

`policy` is a **required field with no default**. A built-in is forced to classify itself by an
exhaustive match; a name-keyed action has no variant, so nothing else would force the question — and a
default would mean "the author forgot" silently resolves to the most permissive setting in the system.

Once registered, the id behaves like any other action. Bind it:

```toml
[keys]
"prefix+d" = "plugin.docker.restart"

# With arguments — use a mode binding (flat bindings take no args):
[[keys.mode.bindings]]
action = "plugin.docker.restart"
keys = "w"
args = { container = "web" }
```

**A binding to an action that doesn't exist yet is not an error.** heca reads your config *before* any
provider or plugin has registered, so a dynamic id simply cannot resolve at load; it resolves when the
key is actually pressed. The flip side is that a **typo in an action name** is not an error either —
it just never fires. If a binding seems dead, check the name.

A typo in an **argument** name is different: heca knows what each built-in action takes, so it says so
at startup rather than leaving you to work it out from a key that does nothing.
```
[heca] binding 'delete_workspace': unknown argument 'ws_idxx' — did you mean 'ws_idx'?
[heca] binding 'delete_workspace': missing required argument 'ws_idx'
[heca] binding 'delete_workspace' cannot be built and will do nothing when pressed
```
The `describe-action <name>` introspection command lists an action's arguments, their types and which
are required — including the ones a provider or plugin contributed. See
**[docs/widgets.md → Discovering actions at runtime](docs/widgets.md#discovering-actions-at-runtime--introspection)**.

**Running one over RPC — `action <name> [key=value …]`.** Every named built-in keeps its own RPC
spelling (`focus-left`, `resize target=column axis=x amount=-50`), but a component's or plugin's
declared action has no such spelling and never can: it is not a variant of any enum. The generic
verb takes any action **id** instead, so the whole catalog is reachable:

```
action workspaces.cursor_down          # a component's own verb
action close_pane_by_id pane_id=7      # a built-in, by id, with arguments
```

Arguments are judged against the action's declaration, so a misspelled one says so rather than
silently defaulting. A call is policy-routed exactly like a keypress — a `tiled_only` action is
refused while a floating pane owns the screen, and a component's cursor verb is refused unless that
dock holds the keyboard. And an action whose component is **not mounted** is reported as such rather
than answered with success.

**Passing `None` instead of a handler** registers a *declarative* action: it has an id, metadata and a
policy, and it appears in menus and introspection, but the host cannot run it — it is forwarded to its
owner across the plugin boundary (this is how WASM plugin actions will work).

For how a widget or a plugin's UI fires one of these, see
**[docs/widgets.md → The other half of an `Intent`](docs/widgets.md#the-other-half-of-an-intent--the-action-it-names)**.

### Interaction Policy

Not every action is allowed in every context. heca tracks a per-workspace **focus domain** — `Tiled` (the scrolling column layout) or `Floating` (a detached floating pane is active) — and an interaction policy layer decides whether each action is allowed before it runs.

**When a floating pane is active**, most actions are blocked so the floating pane stays put and the tiled layout isn't disturbed. These still work while floating:

- **Pane-local actions** — `float` (toggle back to tiled), `close`, `rename`, and text selection.
- **Global app actions** — `reload_config` (hot-reload always works, even with a floating pane open).

Blocked while floating: focus/split/resize/swap/move, sidebar navigation, workspace switching, command palette, spawn, and pane select/swap overlays. The only ways to leave the floating domain are `prefix+f` (toggle float) or closing the floating pane.

**An overlay that covers the panes blocks the same things.** Whether an action may run is judged
against what owns the screen: a pane (`Tiled`), a floating pane (`Floating`), a **dock** holding the
keyboard (`Container`), or something **covering the tiled area** (`Overlay`) — a confirm dialog, or a
plugin panel that declared it obscures the panes. In that last one only truly global actions
(`reload_config`) get through, so nothing splits, zooms or closes a pane you cannot see. A focused
dock changes nothing about the app's own keys: `prefix+Enter` still splits the pane you last worked
in — what it adds is that a component's own cursor verbs (`r`, `x`, …) are reachable *only* while its
dock is being driven, from a key, the command palette or RPC alike.

**The action flow:**

```
Keyboard → KeyCombo → WmAction → dispatch_action() ──[policy]──► registry.execute() → handler
                                          └─ blocked → no-op
```

This policy layer is why `prefix+Shift+r` (reload config) hot-applies appearance/keymap/theme changes without a restart — `reload_config` is a **global** action that stays reachable even when a floating pane is active. (This was a real bug: reload used to be silently blocked while floating, so style only applied on a full restart.)

For the full policy table (all six policy categories) and how to classify a new action, see [`AGENTS.md`](./AGENTS.md) → "Interaction Policy".

---

## Keybindings

### Default Keybindings

See [`keybindings.default.toml`](keybindings.default.toml) for a complete reference of all default keybindings that can be customized.

### Customizing Keybindings

Edit `~/.config/heca/config.toml`:

```toml
[keys]
# Change prefix key
prefix = "ctrl+a"

# Rebind actions
focus_left = "prefix+h"
focus_right = "prefix+l"
zoom_column = "prefix+z"

# Multiple bindings for same action
focus_left = ["prefix+h", "prefix+ArrowLeft"]

# Global bindings (no prefix needed)
# Checked before forwarding to terminal
Alt+Enter = "spawn_terminal"
```

For richer bindings with arguments, use `[[keys.bind]]` (planned contract):

```toml
[[keys.bind]]
keys = "prefix+g"
action = "spawn_pane"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

[[keys.bind]]
keys = "prefix+z"
action = "zoom_column"
```

### Widget navigation & editing (`[keys.widgets]`)

**One generic, cross-widget vocabulary** drives every interactive widget/overlay — the context
menu, command palette, `Select` lists, `Tabs`, and `Dialog` focus. These apply **only while such a
widget/overlay is focused** (they never affect normal-mode input). They are split by **axis**:

```toml
[keys.widgets]
# Horizontal (left/right) — Tabs, a dialog's button row.
item_previous = ["ArrowLeft", "Ctrl+h"]   # vim Ctrl+h
item_next     = ["ArrowRight", "Ctrl+l"]   # vim Ctrl+l
# Vertical (up/down) — menus, Select lists, the command palette.
menu_up       = ["ArrowUp", "Ctrl+k"]      # vim Ctrl+k
menu_down     = ["ArrowDown", "Ctrl+j"]    # vim Ctrl+j
# Shared.
activate      = "Enter"                     # Commit / submit the selected entry or primary action
dismiss       = "Escape"                    # Close / cancel the overlay
# Text-input editing shortcuts (plain typing / Backspace / arrows / word-motion are built in).
edit_delete_back          = "Ctrl+h"                 # Delete one char before the caret
edit_delete_to_line_start = "Ctrl+u"                 # Delete from the caret to line start
edit_select_all           = ["Ctrl+a", "Super+a"]    # Select the whole field (Cmd+a on macOS)
```

Notes:

- **Tab / Shift+Tab** are the universal focus-traversal primitive (handled by the focus system, and
  trapped inside a modal dialog) — always on, **not** listed here.
- A focused text field keeps its own keys first (**field-first**): `←` moves the caret inside an
  `Input` but navigates when a button is focused. The shared `Ctrl+h` disambiguates by focus — it
  deletes in an `Input` but is `item_previous` on a `Tabs`/dialog.
- Menu/palette entries with a quick-pick letter can also be run by typing that letter; in the command
  palette, all other keys type into its filter field.

### Unbinding Defaults

To remove a default keybinding, add it to `[keys.unbind]`:

```toml
[keys.unbind]
"prefix+f" = true        # Disable float toggle
"prefix+q" = true        # Disable pane select
"prefix+Shift+q" = true  # Disable swap pane
```

**Why unbind?**
- Free up keys for custom bindings
- Disable features you don't use
- Resolve conflicts with custom bindings

The action still exists — you can rebind it to a different key in `[keys]`. For example, if you unbind `prefix+f` (float), you can rebind it to `prefix+Shift+f`.

### Zooming the active column

By default, `prefix+z` toggles the active column between:
- its normal stored width
- a viewport-wide zoomed width

Each column tracks its own zoom state, so you can zoom multiple columns independently.
Press `prefix+z` again on the active column to restore that column's previous width.

### Modes

Modes are groups of bindings that stay active until `Escape` or `Enter` is pressed. The default resize mode is an example:

```toml
# Default built-in: prefix+r enters resize mode
# In resize mode the key points the way the DIVIDER travels, whichever
# column or pane is active — `amount` moves the boundary along the axis,
# and +x is right, +y is down:
#   h / l  → move the column's right edge left / right (narrower / wider)
#   j / k  → move the pane's divider down / up
#   Arrow keys work too
#   Escape / Enter → exit mode
#
# A pane's divider is the one below it, or the one above when it is the LAST
# pane — so on the last pane `j` moves that divider down and the pane gets
# shorter. `prefix+Shift+=` / `prefix+Shift+-` are the size verbs instead:
# they grow / shrink the active pane whichever edge has to move.

# Custom modes
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

#### Adding a key to a built-in mode

The built-in modes (`resize`, `sidebar`, `selection`) are merged back in by name, so
you can add a key without redeclaring the whole mode — but the fields are not merged
the same way:

| field | when you redefine a built-in mode |
|---|---|
| `bindings` | **added** to the built-in ones — you keep every default key |
| `trigger`, `sticky` | **replace** the built-in values |

So repeat `trigger` and `sticky` exactly as the default file has them. Omitting
`trigger` sets it to `""`, which for a mode like `resize` silently means `prefix+r`
stops entering it.

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

An **empty `trigger`** means the mode has no key of its own — something else in heca
puts you there. `selection` works this way: you enter it with the
`enter_selection_mode` action (`prefix+s`) or by scrolling up at a prompt.

These blocks work in `config.toml` as well as `keybindings.toml`: both files are
merged into one configuration before it is read.

**Selection mode** is a built-in sticky mode entered with `prefix+s`, or
automatically by scrolling up with the wheel at a non-grabbed prompt.
It provides tmux copy-mode-like navigation keys: `h`/`j`/`k`/`l` for cursor
movement, `y` to copy, `u`/`d` for half-page scroll, `Ctrl+u`/`Ctrl+d` for
full-page scroll, `g`/`Shift+g` for top/bottom, and `Esc` to exit (snap to
bottom + clear selection). The trigger field is ignored because Selection mode
is entered via actions, not a keybinding trigger.

**Sticky vs Non-sticky modes:**
- **Sticky** (`sticky = true`): Stay in mode until `Escape` or `Enter`. Resize mode is sticky.
- **Non-sticky** (`sticky = false`, or chord): Execute one action then exit. Like `prefix+w` → `1` creates workspace 1.

### Binding Precedence

heca checks bindings in this order:

1. **Mode bindings** — If in a custom mode, check mode-specific keymap
2. **Prefix mode** — If in prefix mode, check "normal" keymap for `prefix+key` combos
3. **Global bindings** — In Normal mode, check "global" keymap for direct `Alt+key` / `Super+key` combos
4. **Forward to terminal** — If no binding matched, send key to focused pane's backend

---

## Spawning Applications

heca can launch PTY-backed command panes using `[[keys.command]]`:

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
keys = "Alt+Enter"
command = "alacritty"
```

Supported options today:
- `kind = "terminal"` (default). `app` and `plugin` are reserved and currently report "not yet implemented".
- `float = true` to open as a centered floating pane.
- `close_pane = true` to close the pane when the command exits.
- `keep_on_error = true` or `keep_on_success = true` to override `close_pane` for that exit outcome.

Commands run in a real PTY using the user's shell (`-ic` on Unix, `/C` on Windows), so existing shell-style command strings keep working while preserving interactive shell behavior.

## Process Catalog

You can override how foreground programs are presented in pane chrome through a
canonical `[program.<id>]` entry with optional raw-process aliases:

```toml
[program.nvim]
name = "Neovim"
processes = ["v", "nvim", "nv"]
icon = "file_code"
description = "modal editor"
color = "#89b4fa"
```

Rules:
- `id` is your canonical app entry name; it does not need to match the detected process exactly.
- `processes` lists raw foreground process names that should resolve to that entry.
- All fields are optional. Partial overrides are merged over built-in defaults.
- Built-in defaults are seeded in code for common shells and a small well-known catalog:
  `nvim`, `vim`, `helix`, `yazi`, `ranger`, `claude`, `codex`, `opencode`, `pi`.
- Unknown programs fall back to their raw name and the same terminal icon used by pane cards today.
- `icon` is a semantic Phosphor icon name. Canonical spellings are snake_case
  such as `terminal`, `file_code`, `folder`, `folder_open`, `git_branch`,
  `gear`, and `search`. `kebab-case` aliases also parse.
- `[programs.<id>]` still parses for backward compatibility.
- `disabled = true` removes a built-in entry cleanly and restores raw-name + terminal-icon fallback.
- `color` accepts `#rrggbb` or `#rrggbbaa`.

Example override / removal:

```toml
[program.nvim]
name = "Neovim"
processes = ["v", "nvim"]
icon = "file_code"
color = "#89b4fa"

[program.ranger]
disabled = true
```

### Planned `spawn_pane` action contract

The agreed future-ready action model is `spawn_pane`, designed for multiple pane kinds:

- `terminal`
- `browser`
- `nvim_gui`
- temporary/mock fallback while a backend is not implemented

Examples:

```toml
# Terminal-like pane, tiled
[[keys.bind]]
keys = "prefix+g"
action = "spawn_pane"
args = { kind = "terminal", program = "lazygit", argv = [] }

# Terminal-like pane, floating, centered, 800x400 with implicit px
[[keys.bind]]
keys = "prefix+Shift+t"
action = "spawn_pane"
args = { kind = "terminal", program = "btm", argv = [], float = true, width = "800", height = "400" }

# Terminal-like pane, floating, centered, 800px x 400px with explicit px suffix
[[keys.bind]]
keys = "prefix+Shift+y"
action = "spawn_pane"
args = { kind = "terminal", program = "htop", argv = [], float = true, width = "800px", height = "400px" }

# Terminal-like pane, floating, centered, 80% of content area
[[keys.bind]]
keys = "prefix+Shift+g"
action = "spawn_pane"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

# Future browser pane
[[keys.bind]]
keys = "prefix+Shift+b"
action = "spawn_pane"
args = { kind = "browser", float = true, width = "1200", height = "800" }

# Future nvim GUI pane
[[keys.bind]]
keys = "prefix+Shift+n"
action = "spawn_pane"
args = { kind = "nvim_gui", float = true, width = "1000", height = "700" }

# Float the active pane using explicit geometry
[[keys.bind]]
keys = "prefix+Shift+f"
action = "float_active_at"
args = { width = "95%", height = "95%" }

# Zoom toggle on a normal binding
[[keys.bind]]
keys = "prefix+z"
action = "zoom_column"

# Same spawn ideas inside a mode
[[keys.mode]]
name = "spawn"
trigger = "prefix+s"
sticky = true

[[keys.mode.bindings]]
action = "spawn_pane"
keys = "g"
args = { kind = "terminal", program = "lazygit", argv = [] }

[[keys.mode.bindings]]
action = "spawn_pane"
keys = "n"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

[[keys.mode.bindings]]
action = "zoom_column"
keys = "z"
```

### Float / unfloat contract

The agreed behavior for float toggling is:

- `prefix+f` remains the float/unfloat toggle
- if a pane was originally tiled, unfloat restores it to its original tiled position
- if a pane was spawned directly as floating with no original tiled slot, unfloat should place it into a **new column**
- `prefix+z` is reserved for **column zoom toggle**

---

## Architecture

```
┌───────────────────────────────────────────────────────┐
│  heca (main binary)                                   │
│  ├── winit event loop                                 │
│  ├── input routing (prefix/keymap)                    │
│  ├── ActionRegistry dispatch                          │
│  ├── render() — GPU compositor                        │
│  └── current chrome + future pluggable chrome host    │
├───────────────────────────────────────────────────────┤
│  heca-renderer                                        │
│  ├── PrimitiveRenderer (rects, borders)               │
│  └── TextRenderer (cosmic-text atlas)                 │
├───────────────────────────────────────────────────────┤
│  heca-core                                            │
│  ├── layout/                                          │
│  │   ├── Session → Workspace → Column → Pane          │
│  │   ├── ViewOffset (animated scroll)                 │
│  │   └── Animation (easing, springs)                  │
│  └── backend/                                         │
│      ├── PaneBackend trait                            │
│      ├── TerminalBackend (PTY + vte)                  │
│      └── FakeBackend (layout testing)                 │
├───────────────────────────────────────────────────────┤
│  heca-config                                          │
│  └── TOML config + theme loading                      │
├───────────────────────────────────────────────────────┤
│  future direction                                     │
│  ├── chrome host with left/right/top/bottom regions   │
│  ├── built-in containers (first: WorkspacesContainer) │
│  └── dynamic actions + later WASM/plugin containers   │
└───────────────────────────────────────────────────────┘
```

**Key design decisions:**

- **NIRI layout engine**: Horizontal scrolling columns, not BSP trees. Column widths are independent.
- **Separation of concerns**: Layout in `heca-core`, GPU in `heca-renderer`, orchestration in `heca`.
- **ActionRegistry**: All WM commands go through one dispatch point. Today that is centered on `WmAction`; the long-term direction is toward dynamic/string-based actions for pluggable chrome containers too.
- **KeymapRegistry**: Mode-specific keymaps. Modes are groups of bindings active until Esc/Enter.
- **Prefix mode**: Intentionally tmux-style to avoid conflicts with hosted applications.
- **PaneBackend trait**: All content sources (terminal, Neovim, browser) implement the same interface.
- **Sidebar shell vs container**: a **widget** is a component, placeable anywhere. A **container** (dock) holds widgets. A **sidebar** is a shell that hosts containers and knows almost nothing about them — *left and right are only positions*, and the same container behaves identically in either. What differs between the two sidebars is their content, not what they are. Placing a container is placing it: a second placement is one call (`WorkspacesContainerProvider::placed(id, region)`), not a second implementation, and the content comes from the shared store while state that is per placement — its scroll position — is keyed by mount id.
- **Containers share a region by fraction, never by fixed size**: a container declares its share of the region as a flex grow factor, defaulting to an equal share. One container takes the whole region, two take half each, `2.0` beside `1.0` takes two thirds, and `0.0` means content-sized. It is the region that applies the share, because a share only means something relative to siblings a container cannot see. A fraction survives a window resize; a pixel height does not. See [`docs/widgets.md`](docs/widgets.md#a-containers-share-of-its-region).
- **Scrolling is composed, not owned**: a scroll area *is* a container, so it is nested wherever content should scroll — including inside another container. Two containers in one sidebar scroll independently because each nests its own; the shell does not wrap them in a viewport, because a viewport measures content at its natural height and would leave the containers with no height to divide. Nesting decides which one acts: the innermost that can move on that axis, for the wheel and the keyboard alike. See [`docs/widgets.md`](docs/widgets.md#scrolling-is-composed--nest-a-scroll-area-where-the-scrolling-belongs).
- **Action reachability**: important actions should be reachable from mouse/UI, keybindings, and RPC when meaningful on those surfaces.
- **Surface compositor (layering)**: on-screen surfaces (background, panes, sidebar, floating panes, overlays/modals, future exposé) form a **tree** whose position defines z-order — no hardcoded levels. One uniform rule (context activation + geometric occlusion) decides what is interactive, starting with the universal KeyHint picker (`prefix+/`). See the planner (F003/P019) — see the planner (F003/P019) — the contract for adding any new layer, surface, overlay, or button.

---

## Plugins (Planned)

> **Not yet available.** This describes the *planned* plugin model (target Phase 9)
> so early adopters can see where it's going. The SDK below does not exist yet.
> Full guide with detailed examples: **[docs/chrome-and-ui.md](docs/chrome-and-ui.md)**.

heca will be extensible via **WASM plugins**. A plugin observes app state, dispatches
**intents** (never mutating state directly), and contributes UI as a declarative
**`ViewNode`** tree — a recursive widget tree like Flutter's `Widget` or SwiftUI's
`View`, where a container node holds a vector of child widgets. The host maps that
tree to real `heca-grid-ui` widgets, themes them, and owns focus/clipping/overlays.

```rust
// A container is just a ViewNode with children — compose arbitrarily:
Panel::new().title("Hello")
    .child(VStack::new().gap(6).padding(10)
        .child(Label::new(format!("Active pane: {name}")))
        .child(Button::new("Refresh")
            .variant(Variant::Accent)                 // semantic, themed by the host
            .on_press(intent("example.hello.refresh", {}))))  // intent, not a callback
```

Complex widgets (tables, forms) and rich **modals** (a modal `body` is itself a
`ViewNode`) are built the same way. **Context menus and KeyHints come from the host, not
from nested widgets**: a context menu is a host-owned dropdown the plugin *requests* (or
declares with `.on_context`), and any plugin widget with an `on_press` intent is
automatically leader-**hintable**. See the full guide for panel / table / modal-with-form
examples and the "Context menus & KeyHint" section:
**[docs/chrome-and-ui.md](docs/chrome-and-ui.md)**.

## Roadmap

| Phase | Status | Description |
|-------|--------|-------------|
| **1 — The Shell** | ✅ Complete | GPU window, text rendering, theme system |
| **2 — The Workspace** | ✅ Complete | NIRI layout, animations, input, sidebar |
| **3 — The Content** | 🔄 In Progress | Terminal backend, Neovim msgpack-RPC |
| **4 — The Platform** | 📋 Planned | Session persistence, JSON-RPC/RPC, pluggable chrome, dynamic actions, WASM/plugin containers |

See `.planning/ROADMAP.md` for detailed requirements.

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

## Acknowledgments

- **[NIRI](https://github.com/YaLTeR/niri)** by Ivan Molodetskikh — The scrollable-tiling compositor that inspired heca's layout engine.
- **[cosmic-text](https://github.com/pop-os/cosmic-text)** by System76 — GPU text rendering with excellent font shaping.
- **[wgpu](https://wgpu.rs/)** — Safe, portable GPU API for Rust.
