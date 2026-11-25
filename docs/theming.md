# Theming

A theme is a TOML file of colours and a few sizes and effects. It changes how heca looks, never how
it behaves. This page says where heca finds a theme, how to write one, how to change only part of
one, and what every key means.

The fastest start is [Change only what you want](#change-only-what-you-want--inherit): five lines
are a theme.

## Choosing a theme

In `~/.config/heca/config.toml`:

```toml
theme = "mocha"
```

The value is a **file name** without `.toml`. The `name` written inside the file is only the
theme's label, and heca never looks a theme up by it. A reload (`prefix+Shift+r`) applies a change
without restarting.

Bundled themes: `grid_tron`, `mocha` (dark) and `latte` (light). Their full files are in
[`heca-theme/src/themes/`](../heca-theme/src/themes/), the best examples to copy from.

## Where heca looks

For `theme = "<name>"`, heca reads the first of these that exists:

1. `~/.config/heca/themes/<name>.toml`
2. `themes/<name>.toml` in the system's configuration folder: `~/Library/Application Support/heca/`
   on macOS, `~/.config/heca/` on Linux.
3. the bundled theme `<name>`.

These are the same folders, in the same order, that `config.toml` and `keybindings.toml` are read
from. A file of yours named after a bundled theme (`mocha.toml`) **replaces** it.

## Change only what you want — `inherit`

A theme can start from another one and list only what it changes. Everything it does not list is
the inherited theme's:

```toml
# ~/.config/heca/themes/my-theme.toml — chosen with `theme = "my-theme"`
name = "My theme"
inherit = "mocha"
pane_border = "#ff7edb"
pane_active_border = "#36f9f6"
float_pane_border = "#fede5d"
```

- The inherited theme can be bundled or one of yours, and it may inherit in turn. heca follows the
  chain to its end.
- Keys are laid over the inherited theme one by one, with the same merge that lays your
  `config.toml` over heca's defaults. A group such as `[interaction]` or `shadow` is merged key by
  key too, so `[interaction]` with only `scrim = 140` keeps every other alpha.
- A file that inherits **its own name** (a `mocha.toml` of yours saying `inherit = "mocha"`) builds
  on the bundled theme of that name. That is how to adjust a bundled theme in place.
- A file that inherits needs no other key, not even `name`.

## When a theme cannot be read

If the theme's file does not parse, if `inherit` names a theme that does not exist, or if a chain of
themes comes back on itself (`a` inherits `b`, `b` inherits `a`), heca shows a warning card at
start-up and after each reload. The card says which theme and why, for example *themes inherit in a
circle: a → b → a*. heca then shows the bundled theme of that name, else `grid_tron`, so the window
always has colours.

## Writing a whole theme

Without `inherit`, a theme must set these seven keys. Every other key has a default or is derived
from these:

```toml
name = "My dark blue"
background = "#111827"
foreground = "#e5e7eb"
border = "#374151"
accent = "#38bdf8"
border_radius = 6.0
border_width = 1.0
```

Colours are `"#rrggbb"`, or `"#rrggbbaa"` with transparency.

## Every key

### Palette

| key | what it colours | unset |
| --- | --- | --- |
| `name` | the theme's label | required without `inherit` |
| `background` | the window behind everything | required |
| `surface` | raised cards, pills and panels | a default |
| `foreground` | main text and icons | required |
| `muted` | secondary text and icons | a default |
| `border` | every border in the chrome | required |
| `accent` | highlights, the cursor ring, the active marks | required |
| `glow` | the glow around lit things | a default |
| `danger` / `success` / `warning` | destructive and error / positive / attention | defaults |

### Pane frames

The panes have their own border colours, so a theme can change them without touching `border` and
`accent`, which every other border and highlight uses.

| key | what it colours | unset |
| --- | --- | --- |
| `pane_border` | the frame of a pane without the keyboard | `border` at half strength |
| `pane_active_border` | the frame of the pane with the keyboard | `accent` |
| `float_pane_border` | the frame of every floating pane | a default. Its earlier name `float_accent` is still read |
| `float_background` | a floating pane's background | a default |
| `float_focus` | the focused floating pane's mark | a default |

The frame's **style, width, radius and padding** are configuration, not theme:
`[appearance.pane]` in `config.toml` ([Configuration](configuration.md)).

### Selection and "where you were"

Each is derived from `surface` and `accent` when unset, so a theme of your own gets coherent marks
without naming them.

| key | what it colours |
| --- | --- |
| `selected_background` | the panel a selected row sits on (the cursor is a ring, the selection is a fill) |
| `previous_background` | the row the back-and-forth binding (`prefix+i`) returns to |
| `workspace_active_background` | the frame of the workspace you are in |
| `workspace_previous_background` | the frame of the workspace `prefix+Shift+i` returns to |
| `card_previous_background` | the exposé card `prefix+i` returns to. Derived from `foreground`, so it never looks like the accent frame of the current card |

### Chrome backgrounds

| key | what it colours | unset |
| --- | --- | --- |
| `left_sidebar_background` / `right_sidebar_background` | the side docks | `background`, slightly darker |
| `top_bottom_pane_background` | the top bar, the status bar and the pane title bands | `background`, a bit darker again |
| `background_gradient_top` / `background_gradient_bottom` | the gradient behind the panes | `background` / `background` slightly darker |

### Geometry

| key | meaning | unset |
| --- | --- | --- |
| `border_radius` | corner radius, logical px | required |
| `border_width` | border width, logical px | required |
| `pane_padding` | space between a pane's frame and its content | a default |

### Effects

| key | meaning | unset |
| --- | --- | --- |
| `glow_size` | `none` · `thin` · `medium` · `large`: the only key that controls glow (whether there is one, its size and its strength) | `none` |
| `intensity` | `off` · `low` · `medium` · `heavy`: only the scanline (CRT) overlay. It does not affect glow | `off` |
| `show_focus_border` | draw the keyboard focus border | `true` |
| `overlay_frame` | frame of dialogs, menus and the palette: `bracketed` (corner marks) · `bordered` · `none` | `bracketed` |
| `focus_ring` | colour of the keyboard focus outline | `accent` shifted toward `foreground` |
| `hint_color` | colour of the picker's letters (`prefix+/` and the pane, column and workspace picks) | `accent` |
| `shadow` | `{ color, alpha, blur }`, the shadow under raised panels | a default |
| `active_glow_radius` / `active_glow_strength` | the halo around the active pane, and its card in the exposé | defaults |
| `overlay_frost_radius` | blur behind a layer that asks for one (the exposé). `0.0` = sharp | a default |
| `icon_secondary_alpha` | opacity of an icon's second tone | a default |
| `active_wash_alpha` | opacity of a dock's active and previous workspace washes. `0.0` = off | `0.0` |
| `card_background_alpha` | opacity of a resting card's tint | a default |

`glow_size` and `intensity` can also be set per user under `[appearance]` in `config.toml`. There
they win over the theme.

### Dragging

| key | what it colours | unset |
| --- | --- | --- |
| `drag_ghost_bg` / `drag_ghost_fg` | the label that follows the pointer | defaults |
| `drag_source_bg` / `drag_source_border` | the thing being dragged, where it was | defaults |
| `drop_target_bg` / `drop_target_border` | where it would land | defaults |
| `drop_insertion` | the insertion line | a default |
| `drag_edge_color` | the line a dragged pane can be dropped on | `accent` |
| `drag_edge_target_color` | that line where the drop would land now | `warning` |
| `drag_edge_width` | that line's thickness, logical px | a default |
| `drag_image_alpha` / `drag_image_scale` | the picture of a dragged widget: opacity, and size against the original | defaults |

### Terminal

Unset, each one is the terminal's own default.

| key | meaning |
| --- | --- |
| `terminal_foreground` / `terminal_background` | default text and background. Keep the background opaque: a terminal's transparency is `[appearance.terminal] transparency` in `config.toml` |
| `terminal_cursor_foreground` / `terminal_cursor_background` / `terminal_cursor_border` | the cursor |
| `terminal_selection_foreground` / `terminal_selection_background` | selected text |
| `terminal_ansi` / `terminal_brights` | the 8 normal and 8 bright colours, as lists of 8 |

### Groups

Each group is a table. Set only the keys you want; the rest keep their defaults (or, with
`inherit`, the inherited theme's).

| group | keys |
| --- | --- |
| `[interaction]` | opacities (`0`–`255`) of interaction states: `toggle_on_fill`, `control_hover_fill`, `control_hover_border`, `control_active_fill`, `control_active_border`, `control_rest_border`, `control_rest_glow`, `row_hover_fill`, `row_active_fill`, `row_active_border`, `row_active_tint`, `row_hover_tint`, `nav_outline`, `nav_wash`, `nav_fill`, `badge_fill`, `tag_fill`, `tag_border`, `toast_tint`, `outline_rest`, `outline_hover`, `selection`, `hilite`, `scrim`, `keycap`, `panel_border`, `panel_row_border`, `panel_row_fill`, `tooltip_border`, `menu_shortcut`, `menu_shortcut_dim`, `thumb_rest`, `thumb_hover`, `unlit` |
| `[input]` | text fields: `width`, `padding`, `caret_width`, `blink_period`, `glow_radius`, `max_lines` |
| `[dialog]` | `max_width_em`, the readable width of a dialog |
| `[flash]` | the flash that marks a change: `enabled`, `duration_s`, `pause_s`, `pulses`, `frames_per_second`, `border_width_em`, `border_alpha`, `fill_alpha`, `glow_radius_em`, `glow_strength` |
| `sidebar_label_font_size` / `sidebar_button_font_size` | sizes of the sidebar's labels and buttons |

## What a theme does not hold

- **Fonts.** They depend on what is installed on each machine, so a theme that named one would break
  elsewhere. Fonts are `[font]` in `config.toml` ([Configuration](configuration.md)).
- **Border style, width, radius, padding and gaps of panes and docks.** These are `[appearance]` in
  `config.toml`.
- **Border colours in `config.toml`.** `[appearance] border_color` and
  `[appearance.pane] border_color` / `active_border_color` / `floating_border_color` are no longer
  read. A config that still sets them is told which theme key replaced each: `border` /
  `pane_border` / `pane_active_border` / `float_pane_border`.

## Where it lives in the code

If this page and the code disagree, the code is right, and this page should be fixed:

- `heca-theme/src/theme/mod.rs`: every key, its default and what derives it;
- `heca-theme/src/loader.rs`: where heca looks, `inherit` and the problems it reports;
- `heca-theme/src/merge.rs`: the one merge, shared with the configuration;
- `heca-theme/src/themes/*.toml`: the bundled themes.
