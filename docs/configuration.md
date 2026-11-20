# Configuration

heca loads config from `~/.config/heca/config.toml` (general settings) and
`~/.config/heca/keybindings.toml` (keybindings) — `%APPDATA%\heca\…` on Windows.
Both are optional and deep-merged over the built-in defaults, which live in two
versioned, embedded files (the single source of truth — copy and edit them):

- `config.default.toml` — default `[settings]`, `[appearance]`, `[font]`, `[program]`
- `keybindings.default.toml` — default `[keys]` (prefix, bindings, modes)

An extension's own settings go in `[extensions.<name>]`, a table per extension. heca has no keys of
its own there, so the defaults list none; see [Building on heca](extending-heca.md).

## Per-project settings

A `.heca/config.toml` in the folder heca was started in (or the nearest parent that has a `.heca/`
folder) is merged over your config **key by key**: any key it sets wins, tables merge, and nothing
else changes. It takes the same keys as `config.toml` — a theme colour, a key binding, anything. A
project file with an error is reported and left out, and your configuration stands. `prefix+Shift+r`
reloads it with the rest. With no `.heca/`, nothing changes.

**A project file is ignored until you trust it** — it ships with the repository, so a cloned one could
bind a key to a command. heca says so, and `heca --trust` (in the project, or `heca --trust <folder>`)
trusts it as it is now; if the file changes, it is ignored and you are asked again. `heca --untrust`
forgets it, and `heca --show-config` marks an untrusted file "(not trusted, ignored)".

## Minimal Config

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
prefix_timeout_ms = 1000      # how long the prefix waits for the next key
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
swap_pane = "prefix+Shift+m"
swap_and_focus_pane = "prefix+m"
move_column_to_workspace_pick = "prefix+Shift+g"
move_pane_to_workspace_pick = "prefix+g"
move_pane_to_column_pick = "prefix+Ctrl+c"
move_pane_to_new_column  = "prefix+Shift+Enter"
rename_workspace = "prefix+Shift+w"
rename_pane = "prefix+$"
rename_column = "prefix+Shift+c"
```

## Asking heca from the command line

Every one of these answers and exits **before the window opens**, so a script can ask without a
display:

```bash
heca -h  --help                    # these commands
heca -k  --keys-show [--json]      # every binding name and the key it resolves to
heca -a  --list-actions [--json]   # every action, its category, and how many arguments it takes
heca -d  --describe-action resize  # one action in full: what it does, every argument, every value
heca -d resize -j                  # the same, as JSON (-j is --json)
```

Each command has a one-letter form. One letter, not two: `-la` would read as the combined `-l -a`
everywhere else on the machine, and a flag that looks like a convention it does not follow is worse
than no short form at all.

`--describe-action` is the reference for an action's arguments — including which are optional and
what values an enum accepts — because the declaration lives beside the parser, so the two cannot
drift. `--help` is rendered from the same table that dispatches, so it cannot list a command that
does nothing or miss one that works.

The keybinding guide is [Keybindings](keybindings.md) — the five binding shapes, which surface
answers a key, and worked examples. This page does not repeat it.

## Theme

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

# The two bars (px, clamped 16..=96) — only used while the bar is shown
# top_bar_height    = 32.0   # the tab bar
# bottom_bar_height = 24.0   # the status bar

# The `prefix+/` picker's letters
hint_font_size = 12.0      # one size for EVERY letter in the app, in logical px. A letter is drawn
                           # over whatever it points at, so it is not scaled by the size of that
                           # thing — a header button and a whole pane wear the same letter.
# hint_color = "#ffb86c"   # the letters' colour. Unset = the theme's own `hint_color`, which falls
                           # back to the accent. The colour lives in the theme so it travels with a
                           # palette; this overrides it for every theme you load.

# Keyboard focus outline
show_focus_border = true   # draw the focus ring at all
# focus_ring = "#7fd3ff"   # focus-outline color; unset = the accent shifted toward `foreground`
                           # (auto-brightens on dark themes, darkens on light themes). The
                           # destructive/danger focus ring always derives the same way.

# Sidebar highlight alphas (0.0–1.0; optional, shown with their defaults)
active_wash_alpha = 0.11          # Accent wash over the active workspace
card_background_alpha = 0.02      # Resting background tint of each pane card
drag_image_alpha = 0.6            # Opacity of the picture of a pane that follows the pointer while it is carried
drag_image_scale = 0.5            # Its size next to the pane (1.0 = life size)

[shadow]
color = "#000000"
alpha = 0.3
blur = 8.0
```

## Appearance & Frost (z=0 background layer)

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
# Space between the pane edge and the terminal grid (logical px):
#   padding                  — all four sides
#   padding_top/bottom/left/right — override one side
# The grid reflows into the smaller box (the program is told fewer cells), it is
# never cropped. Default 0 (unchanged rendering).
padding = 0
```

Give a startup screen breathing room, the way most terminals do:

```toml
[appearance.terminal]
padding = 6            # or per side: padding_top = 6, padding_left = 6, …
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

### Effect tokens (`glow_size` / `intensity`)

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


### Tooltips

```toml
[appearance]
tooltip_delay_ms = 500   # how long the pointer rests before a tooltip appears
```

One value for every tooltip. A label whose text is cut shows the whole text on hover after half of
it; a tooltip that sets its own delay in code keeps it. A reload (`prefix+Shift+r`) applies a change.

## Settings

```toml
[settings]
window_width = 1280           # Initial window width
window_height = 800           # Initial window height
mouse = true                  # Enable mouse interactions
focus_follows_mouse = true    # Focus pane on hover
auto_scroll_edge = true       # Auto-scroll near edges
edge_scroll_distance = 80.0   # How close (px) a dragged pane must come to the edge for that to start
edge_scroll_speed = 1000.0    # How fast (px per second) the view scrolls while a dragged pane is at the edge
column_focus = "last"  # Pane focus lands on in the next column: "last" used there, or the one level with "row"
interactive_move_modifier = "Super"  # Modifier for drag-and-drop
float_size = 0.95             # Share of the working area a pane takes when it floats with nowhere given
move_slide_reach = 0.9        # How far (fraction of the window) a moved or swapped pane animates in from; 0 = jump
shell_integration = true      # Auto-inject OSC 133/OSC 7 shell hooks for runtime status + cwd
new_workspace_cwd = "~"       # Folder a new workspace's shell starts in: a path, "~" or "~/..."; missing → home
pane_renamed_add_process_name = true  # Renamed pane shows its process name small, e.g. `MyPane (nvim)`
pane_name_delay = 1000        # Display-name delay in milliseconds; 0 updates immediately
# pane_show_cwd is now [appearance.expose] show_cwd (still read; see below)
terminal_mouse = true         # Enable host scrollback on wheel (vs forwarding to terminal)
terminal_wheel_scroll_lines = 3  # Lines per wheel notch: history rows, and wheel steps sent to a program (Claude Code, vim)
terminal_scroll_animations = true  # Smooth animated terminal viewport jumps
pane_size = "latest"          # When several windows show the SAME pane at once, the one running
                              # terminal still has a single size, so the windows must agree on it:
                              # "smallest" (fits every window) | "latest" (the window last typed in,
                              # the default) | "largest". A pane shown by one window takes that
                              # window's size. Only rows×cols are shared; each window draws at its own font.

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
history_limit = 100           # How many dismissed or expired notifications the server keeps for
                              # the notification history. Oldest forgotten first; 0 keeps none.
                              # Applies on prefix+Shift+r.
```

## Confirmation prompts

Destructive actions can pop a **confirm dialog** before running. This is declared
**per action** and configured in the generic `[confirm]` table, keyed by action
name — `true` prompts (Cancel / \<action\>), `false` runs immediately:

```toml
[confirm]
close = true            # confirm before closing a pane
delete_column = true    # confirm before deleting a column (and its panes)
delete_workspace = true # confirm before deleting a workspace (and its contents)
terminate_session = true # stop its processes, retain the session for restart
delete_session = true    # remove the session, never project files
```

Session confirmations name the target and select Cancel initially. Terminate retains the
session for explicit restart; Delete removes it and stops its processes, without deleting
project files. Detach needs no confirmation and leaves its processes running. The warning
text and button labels are configurable under `[settings.session_text]`:
`confirm_terminate` and `confirm_delete` accept a `{name}` placeholder; `terminate`, `delete`
and `cancel` supply the button labels. The two `[confirm]` switches accept only `true` or
`false` and default to `true`. An RPC without a window cannot answer an enabled prompt
and is refused rather than silently executing.

`heca --kill <name>` also asks before stopping a session: `Confirm? [y/N]:`.
Only `y` or `Y` confirms. `n`, `N`, Enter, EOF or any other answer cancels without
changing the session. The warning and `confirm_prompt` come from
`[settings.session_text]` in the target project's effective configuration. Changing the
prompt or GUI button labels does not change the y/N response grammar. The prompt goes
to stderr, leaving stdout for the result. Setting `[confirm].terminate_session=false`
skips this prompt too.

The guard lives on the **action**, so keybindings, buttons, menus and RPC use the
same confirmation policy. Requests handled by a window can show the dialog.
Session lifecycle RPC calls are handled by the server: while their confirmation
is enabled, they return an error instead of opening a dialog in another window.
Use the GUI or interactive `--kill`, or explicitly disable the corresponding
`[confirm]` switch for unattended scripts. Any action (including a plugin's) is
configurable by name here; an unlisted action uses its declared default. Changes
apply on `prefix+Shift+r`.

> Replaces the old `[settings] confirm_close_pane` / `confirm_delete_column` /
> `confirm_delete_workspace` flags — move any you had set into `[confirm]` as
> `close` / `delete_column` / `delete_workspace`.
>
> The pane toggle was once `delete_pane`; it still works, but `close` is the name
> now — one word for the action, its key, its label and its dialog. **Close** is
> for the pane you work in; **Delete** removes a container or session. Terminating
> a session stops its processes but retains its definition.

## Where docks start

A dock is a panel in a sidebar or a bar — heca's workspaces, and any dock a program built on heca
adds. `[[docks]]` says where each one goes and in what order. One entry per **kind** of dock
(`name`), or per **dock** (`name` + `id`); the entry naming a dock's id wins over the one for its
kind:

```toml
[[docks]]
name = "workspaces"          # every workspaces dock…
location = "sidebar.left"
order = 1

[[docks]]
name = "workspaces"
id = "aaa"                   # …except this one
location = "sidebar.right"

[[docks]]
name = "docker"
location = "sidebar.left"
order = -1                   # before the workspaces dock
```

Two docks of one kind in one sidebar — the second is made by its entry, since no program adds it:

```toml
[[docks]]
name = "workspaces"
id = "workspaces"            # the one heca starts with
location = "sidebar.left"
order = 0

[[docks]]
name = "workspaces"
id = "workspaces.2"          # new: made from this entry
location = "sidebar.left"
order = 1                    # under the first
flex = 2                     # twice its height
```

| Key | Meaning |
|---|---|
| `name` | the kind of dock (required) |
| `id` | one dock of that kind; unset = every dock of the kind |
| `location` | `"sidebar.left"`, `"sidebar.right"`, `"bar.top"`, `"bar.bottom"`, or `"none"` (not shown; its keyboard actions still work) |
| `order` | where it sits among the region's docks: lower first, ties keep the order the docks were added in (CSS `order`) |
| `flex` | how much of the region it takes: `2` is twice a dock at `1` (CSS `flex`); unset, equal parts |

What an entry does not say is what the program that adds the dock says. An entry with an `id` no
dock has **adds a dock** of that kind, if the kind allows more than one (the workspaces dock does);
otherwise it does nothing. Taking that entry out takes the dock out. A `location` heca does not know
is reported and the dock goes where its program says. A reload (`prefix+Shift+r`) applies the entries that changed: a dock is moved, reordered, resized,
taken out, brought back or added; a dock whose entry did not change stays where it is, even if you moved it by
hand. Writing your own `[[docks]]` replaces the default list, which is one entry:
`name = "workspaces"`, `location = "sidebar.left"`.

## Notifications

heca shows short **toast notifications** in the top-right corner — a pane was
created, a process exited, the config reloaded, a command could not be run.

```toml
[settings.notification_system]
mode = "app"          # "app" (in-app toast stack) | "system" (OS notifications —
                      # reserved, falls back to "app" until that backend exists) |
                      # "none" (no notifications at all — stderr logs are unaffected)
auto_dismiss_ms = 4000 # how long a toast stays before dismissing itself
max_visible = 5        # how many are on screen at once; the rest queue
history_limit = 100    # how many dismissed/expired ones the server remembers; 0 = none
```

- **Severity is tone, not importance.** `info` / `success` / `warning` / `danger`
  change the accent colour; they do **not** change how long a toast stays. Every
  toast uses `auto_dismiss_ms` unless its producer supplies an explicit duration
  or marks it sticky (a config-reload failure is sticky — you fix it and press
  **Retry**). If the setting is omitted, the bundled configuration supplies the fallback.
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
  Native Rust extensions use `heca::Notification::info("…").body("…").send()`.
  An optional `.auto_dismiss(std::time::Duration::from_secs(10))` overrides the
  configured duration for that notification. `.sticky()` disables automatic expiry.
  A config reload affects future notifications, not deadlines already assigned.
- **Notification history.** A toast that went away is not lost. `prefix+Ctrl+Alt+n` (the
  `notification_history` action — also in the command palette and over RPC) opens the command
  palette's **notifications mode** (`!`): every notification still on screen and every one the
  server remembers, **newest first**, each with its level (the toast's own icon), title, full
  body (the selected row shows it whole, wrapped), how long ago it was raised and whether it is
  still `visible` or was `dismissed` / `expired` (`just now · dismissed`). Where it came from is
  named only when it is not heca itself (a plugin, a provider, a pane). A notification raised
  again under the same key is one entry with a count: `2 min ago · visible · ×3`. Type to search by title **or
  body** (the same fuzzy matcher as the rest of the palette; a title match is listed above a
  body-only match, and the match is highlighted in whichever text it hit). A notification still on screen that has
  buttons also lists them as `Title › Button` rows; choosing one does what the toast's button
  does. A dismissed notification has no buttons left, and choosing it just closes the palette.
  The history belongs to the server, so every window sees the same list, a window that
  reattached included. `[settings.notification_system] history_limit` (default `100`) says how
  many dismissed or expired ones are kept, the oldest forgotten first; `0` keeps none, so the
  list only shows what is still on screen. A config reload applies a new limit at once.
  **Every way into the mode asks the server again** — the key, the action over RPC, or typing
  `!` in a palette that is already open — so the list is never an old one. Typing `!` shows an
  empty list for a moment, then the palette is replaced by one holding the answer, keeping
  what you had typed.
- What produces a toast today: a new pane, a pane whose process exited while the
  pane stays open, `prefix+Shift+r` (success or the parse error), and a command
  that fails to spawn.

## Fonts

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

### Terminal ligatures

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

### Terminal hyperlinks

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

### Terminal bell

A terminal bell (`\a`) can drive up to three independent cues, configured under
`[appearance.terminal]`:

```toml
[appearance.terminal]
bell_attention = true   # OS attention cue (Dock bounce / taskbar flash) while unfocused
bell_visual    = false  # brief accent flash over the content area
bell_audible   = false  # system beep (macOS only for now; no-op elsewhere)
bell_flash_alpha = 56   # how bright the visual bell's flash is at its peak, 0..=255
search_match_alpha = 64          # scrollback-search match highlight, 0..=255
search_current_match_alpha = 150 # the match the search is on (bolder)
```

`bell_attention` only fires while the window is unfocused; the visual and audible
bells fire on any bell. All default off except `bell_attention`. Changes apply
live with `prefix+Shift+r` (no restart).

## Pane Info Bar

Each pane shows a small **info bar** along its top: configurable **segments** on the
left (what the pane is) and **action buttons** on the right. Both are configured
under `[appearance.pane]` as ordered lists — order in the list is the order shown
(left → right). An empty list hides that side; if **both** are empty the bar (and
its reserved space) disappears entirely.

When a pane is **renamed** (given a custom name), its **sidebar card** shows the custom
name followed by the process name in a small dimmed label — e.g. `MyPane (nvim)` — so the
sidebar keeps surfacing what's actually running. The command palette and the exposé name the
pane the same way. The process label is not part of the name
(it's never edited by rename); toggle it with `[settings] pane_renamed_add_process_name`
(default `true`). In the pane **info bar**, the `app_name` segment always shows the program
name regardless of a rename; add the `pane_name` segment to show the custom name there.

`[settings] pane_name_delay` keeps brief shell commands from repeatedly changing the displayed
program name. It accepts a nonnegative integer in milliseconds (default `1000`, one second);
`0` disables the delay. Negative numbers, fractions and quoted strings are invalid.
The name and program icon change only after the new running program remains for that duration.
When the command finishes and the shell returns, its name is restored without waiting.
Custom pane names stay unchanged; their optional process suffix uses the same delayed name.
Sidebar, info bar, pane palette and exposé share this projection. Raw process and agent state,
exit codes, sounds and feedback remain immediate. Normal configuration precedence applies;
startup and reload read the same setting, and changing it does not restart a process.

```toml
[appearance.pane]
# Left side — what to show, in order. A segment with no data for a pane is skipped
# (e.g. git segments outside a repo).
title_segments = ["location", "app_name", "git_branch", "git_status"]

# Right side — action buttons, in order. Each button's tooltip shows its real
# configured keybinding.
title_actions = ["split", "close"]

# The name drawn over a pane while it is dragged: a share of the pane's shorter side, kept
# between two sizes (px). And how wide the "new column" opening beside a column is (a share).
# drag_label_size_factor = 0.25
# drag_label_min_size    = 24.0
# drag_label_max_size    = 72.0

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

A program built on heca can add its own chips, named `<extension>.<short>` (for example
`demo.status`); they follow the same rule as the buttons below — shown after the defaults while this
list is still the default, only if you list them once you write your own, and reported once when one
you have not listed is available. A name nothing provides is reported and skipped.

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

A program built on heca can add its own buttons, named `<extension>.<short>` (for example
`demo.show_notes`). They show after the defaults while this list is still the default; once you write
your own list, only the names you list show, and heca tells you once when a button you have not
listed is available. A name nothing provides is reported and skipped, with the nearest real one.

**Lines under a pane's name in the sidebar** (`[appearance.sidebar] pane_lines`, default
`["cwd", "git", "agent"]`): `cwd` is the working directory and `git` is the branch with change counts, shown
only inside a repository. **A fresh install shows the folder line under each pane's name in the
sidebar.** Which lines show is decided by this list alone. A program built
on heca can add its own, named `<extension>.<short>`; the same rules apply as for buttons and chips.

`agent` shows the reporting program's name, status and optional task. It hides itself in other panes.
Its labels and glyphs come from `[settings.agents.statuses]`; unlisted status ids are shown as reported.

## Agent status and sounds

**Reporting starts off for every program.** Integrations are installed explicitly and persist until
removed. Installation adds Heca's hooks or extension without replacing existing hooks, plugins or
other native settings. It also enables that program in Heca's user config. Installed integrations
stay inactive outside Heca.

`heca::agent::integration::Integrations::machine_local()` builds the shared installation service from
normal Heca configuration and registered drivers. It provides `list`, `inspect` and `apply` with
`IntegrationOperation::{Install, Update, Uninstall}`. Construction installs nothing and returns invalid
configuration to the caller. The future settings page will use the same operations.

The command-line entry uses the same installation service:

- `heca install|update|uninstall integration <program> [--json]`
- `heca inspect integration <program> [--json]`
- `heca list integrations [--json]`

Files ship with Heca, with a version per integration. Inspection checks the actual installed files
and registrations. Updates are explicit and preserve reporting choices. Heca refuses to overwrite
modified integration files; an interrupted write is recovered before another installation change.
Uninstall removes owned files and registrations, restores native feature settings, and disables the
program in Heca. No installation server or separate download is required.

The first `install integration <program>` enables that program's reporting. Updating or installing
it again preserves any later choice to disable it. This setting controls reporting to Heca, not
whether the agent program can run. The master reporting switch and the sound switch are separate.

Pane feedback includes success and failure when a shell command or directly launched program
finishes, including editors. It has no program allowlist or minimum-duration threshold. Each
enabled integration's optional `feedback_events` also selects native attention events. Work/tool
progress, startup, compaction, subagent activity and voluntary interruptions are silent by default.
A background pane repeats soft, theme-controlled pulses until acknowledged or until
`[settings] notification_flash` milliseconds have elapsed (default `8000`, eight seconds).
`0` has no timeout. A pane with actual
input focus in the active OS window gets none. The timeout stops the animation, not the pending
feedback; acknowledgement is still a separate shared operation.

The server session owns feedback revisions and acknowledgement. Focusing the pane in any active
window acknowledges the observed revision for every window, without changing the agent's status.
A palette, dialog or focused dock prevents that acknowledgement. New windows receive the current
pending state and do not replay acknowledged events. A late acknowledgement cannot consume a newer
event. Completion feedback does not require a reporting integration. Richer program-specific
attention events still require one; an executable name alone cannot identify an editor's question.
Commands inside an interactive terminal use the configured shell-integration hooks; directly
launched programs also report their process exit without those hooks.

Program and event choices are separate from installation:

```toml
[settings.agents.programs.codex]
enabled = true
# Optional: report only these native events. Omit this list to use every configured report.
enabled_events = ["UserPromptSubmit", "Stop"]
```

Use the same `enabled` setting for `claude`, `pi`, `opencode` or an added driver. An empty
`enabled_events = []` disables reporting without uninstalling anything. Native event names are listed in
[`config.default.toml`](../config.default.toml). Open a new terminal after changing launch setup;
each terminal owns its reporting files and wrappers for its lifetime.

Automatic setup uses the existing Bash, Zsh and Fish integration on Unix; it does not change Windows commands.
Only the terminal identity, reporting environment and report files are temporary. Launches do not
inject native settings, hook definitions or plugin lists. Explicit install/update/uninstall are the
only operations that change native configuration, preserving unrelated entries.
The master `[settings.agents] enabled = false` disables all reporting setup. `max_report_bytes` limits each incoming report;
`channel_capacity` limits queued reports and transitions, keeping the latest when full.
`launch_timeout_ms` bounds the wait for per-run setup. Codex keeps its normal hook-trust review;
review its installed reporting hooks when prompted. Heca never bypasses that review.
Native policy can disable user hooks, so installed files alone do not prove that reporting is active.
See the [Claude hooks](https://code.claude.com/docs/en/hooks),
[Codex hooks](https://learn.chatgpt.com/docs/hooks), [Pi extensions](https://pi.dev/docs/latest/extensions)
and [OpenCode CLI plugins](https://dev.opencode.ai/v2/docs/cli/plugins) documentation.

`[settings.agents.programs.<driver>]` sets `enabled`, `name`, `executables`, `enabled_events` and `reports` (native event
name to status id). Built-in and added drivers use the same contract. Status ids are open: additional
drivers and project settings can introduce their own.
Added drivers keep their declared name and executables unless those fields are overridden.

## Pane Feedback Settings

These keys belong directly in `[settings]`. They apply to shell commands, editor/program exits
and the configured attention events of enabled integrations, not just to agents.

| Setting | Default | Accepted Values | Behaviour |
| --- | --- | --- | --- |
| `notification_flash` | `8000` | Non-negative TOML integer, in milliseconds | Maximum **total** flashing time, including pauses. Positive values stop the flash at the deadline or earlier on acknowledgement. `0` repeats until acknowledged; it does **not** disable flashing. |
| `notification_sound` | `true` | TOML boolean `true` or `false` | `true` permits one configured sound for each new background feedback event. `false` mutes feedback sounds without disabling flashes, reporting or status updates. |

Write both values without quotes. For example, `8000` is eight seconds; `8` is eight
milliseconds. Negative or fractional durations, strings such as `"8000"`, boolean durations,
and sound values such as `1`, `"true"` or `"false"` are invalid.

The pane with actual input focus in the active window does not flash or play a feedback sound;
it acknowledges the event for every window attached to that session. A selected pane in an
inactive window, or behind a palette/dialog/focused dock, is not input-focused. Timing expiry
only stops the flash: it does not acknowledge the event or change the program's status. A newly
attached window can still show pending feedback.

The theme's `[flash] duration_s` and `pause_s` control each pulse's cadence, in **seconds**;
`notification_flash` controls the total time, in **milliseconds**. It does not change colours,
rounding, toast dismissal (`notification_system.auto_dismiss_ms`) or terminal-bell settings.

The following example uses the defaults and customises one status sound:

```toml
[settings]
notification_flash = 8000
notification_sound = true

[settings.agents.sounds]
volume = 0.7
queue_capacity = 32

[settings.agents.statuses.finished]
sound = "/path/to/finished.wav"
```

Precedence is embedded defaults, then user `config.toml`, then a **trusted** project's
`.heca/config.toml`. Each layer replaces only the keys it supplies: omitted keys inherit,
while explicit `0` and `false` are preserved. An untrusted project's overrides are ignored.
The old `[settings.agents.sounds] enabled` key has been replaced by `[settings] notification_sound`;
volume, queue size and per-status sound settings keep their existing paths shown above.

Use the config-reload action after editing. A changed flash timeout applies to existing repeating
flashes without resetting their original start time. If the new deadline has already passed, the
flash stops; `0` removes the deadline while the event remains pending. Muting applies to future
playback; a sound already playing may finish. Enabling sound does not replay old feedback.

Playback happens once for a new, unobserved feedback event, on a background worker. The server
grants playback to at most one client per revision, so several windows do not duplicate the sound.
Each completion is a new event even when its status matches the previous one. Acknowledged events,
repeated animation pulses, retransmissions and task-title changes do not repeat a sound. Success
and error have bundled defaults; the same status table accepts custom sounds. A missing sound file
or audio device does not interrupt the terminal.
Volume is limited to `0.0`–`1.0`; the sound queue holds at least one entry.
Supported sound files are WAV, Ogg Vorbis, MP3 and FLAC.

**The exposé's folders** are `[appearance.expose] show_cwd` (on by default; `false` hides them). It replaces
`[settings] pane_show_cwd`, which is still read while the new key is not written (once it is, it
decides), with a one-time notice to move it; it no longer affects the sidebar.

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
