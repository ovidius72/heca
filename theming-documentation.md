# Theming Documentation

This document describes **exactly how theming works today** in this repo:

- where themes are loaded from
- how to select a theme
- how to create a new theme
- every supported theme variable
- the bundled default themes and their exact values
- how terminal theming currently works

---

## 1. How theming works

The canonical theme type lives in:

- `heca-theme/src/theme.rs`

The canonical theme loader lives in:

- `heca-theme/src/loader.rs`

### Resolution order

When a theme is loaded by name, resolution is:

1. `~/.config/heca/themes/{name}.toml`
2. bundled theme shipped in `heca-theme/src/themes/{name}.toml`
3. fallback to bundled `grid_tron.toml`

So theme loading never fails to produce a valid theme.

### Theme selection

Theme selection belongs under:

```toml
[settings]
theme = "grid_tron"
```

Not under `[appearance]`.

### Current bundled theme names

In `heca-theme` itself, the bundled names are currently:

- `grid_tron`
- `mocha`
- `latte`

---

## 2. How to create a theme

Create a TOML file in either location:

### User theme

```text
~/.config/heca/themes/my_theme.toml
```

### Bundled theme

```text
heca-theme/src/themes/my_theme.toml
```

If it is bundled, also add it to:

- `heca-theme/src/loader.rs`
  - `bundled_themes()`
  - `available_themes()`

### Minimal process

1. Copy an existing theme file.
2. Change `name`.
3. Adjust palette values.
4. Set explicit sidebar/bar tokens if you want exact control.
5. If omitted, optional tokens fall back or derive as documented below.

---

## 3. Theme file format

A theme is a TOML file deserialized into `heca_theme::Theme`.

### Full field list

```toml
name = "My Theme"

background = "#000000"
surface = "#111111"
foreground = "#ffffff"
muted = "#888888"
border = "#222222"
accent = "#00ccff"
glow = "#00ccff"
danger = "#ff4455"
success = "#33cc66"
warning = "#ffaa33"

font_family = "Geist Mono"
font_size = 15.0

border_radius = 6.0
border_width = 1.0
pane_padding = 4.0

left_sidebar_background = "#0d0d0d"
right_sidebar_background = "#0d0d0d"
top_bottom_pane_background = "#080808"

glow_size = "medium"
intensity = "medium"
show_focus_border = true
icon_secondary_alpha = 0.45
active_wash_alpha = 0.11
card_background_alpha = 0.02

shadow = { color = "#000000", alpha = 0.3, blur = 8.0 }

float_background = "#222222"
float_accent = "#66ccff"
float_focus = "#ffaa66"

drag_ghost_bg = "#66ccffd9"
drag_ghost_fg = "#ffffff"
drag_source_bg = "#66ccff26"
drag_source_border = "#66ccff"
drop_target_bg = "#66ccff2d"
drop_target_border = "#66ccff"
drop_insertion = "#66ccff"

sidebar_label_font_size = 14.0
sidebar_button_font_size = 11.0

terminal_font_family = "Maple Mono Normal NF"
terminal_italic_font_family = "Maple Mono Normal NF"
terminal_font_size = 14.0

# Optional terminal palette overrides
terminal_foreground = "#d0d0d0"
terminal_background = "#101010"
terminal_cursor_foreground = "#000000"
terminal_cursor_background = "#ffffff"
terminal_cursor_border = "#ffffff"
terminal_selection_foreground = "#000000"
terminal_selection_background = "#88ccff"
terminal_frost_color = "#101010"

# Optional ANSI palettes
terminal_ansi = [
  "#000000", "#aa0000", "#00aa00", "#aa5500",
  "#0000aa", "#aa00aa", "#00aaaa", "#aaaaaa"
]
terminal_brights = [
  "#555555", "#ff5555", "#55ff55", "#ffff55",
  "#5555ff", "#ff55ff", "#55ffff", "#ffffff"
]
```

---

## 4. Which fields are required vs optional

### Required in practice

These should be set explicitly in any real theme:

- `name`
- `background`
- `foreground`
- `border`
- `accent`
- `font_family`
- `font_size`
- `border_radius`
- `border_width`

### Optional with defaults/derivation

These have fallback behavior in `heca-theme/src/theme.rs`:

- `surface`
- `muted`
- `glow`
- `danger`
- `success`
- `warning`
- `pane_padding`
- `left_sidebar_background`
- `right_sidebar_background`
- `top_bottom_pane_background`
- `glow_size`
- `intensity`
- `show_focus_border`
- `icon_secondary_alpha`
- `active_wash_alpha`
- `card_background_alpha`
- `float_*`
- `drag_*`
- `drop_*`
- `sidebar_*_font_size`
- all `terminal_*`

### Derived background tokens

If these are omitted:

- `left_sidebar_background`
- `right_sidebar_background`
- `top_bottom_pane_background`

then `Theme` derives them from `background`:

- sidebars = `background` darkened slightly
- top/bottom pane bars = `background` darkened a bit more

This behavior lives in:

- `Theme::effective_left_sidebar_background()`
- `Theme::effective_right_sidebar_background()`
- `Theme::effective_top_bottom_pane_background()`

---

## 5. Meaning of the main tokens

### Core palette

- `background` — main app/content canvas background
- `surface` — elevated surfaces/cards/pills
- `foreground` — primary text/icon color
- `muted` — secondary/deemphasized text/icon color
- `border` — default border color
- `accent` — active/highlight color
- `glow` — glow color
- `danger` — destructive/error actions
- `success` — success/positive state
- `warning` — warning/attention state

### Chrome background tokens

- `left_sidebar_background` — left sidebar shell background
- `right_sidebar_background` — right sidebar shell background
- `top_bottom_pane_background` — top bar, bottom bar, and pane title/header band background

### Geometry / typography

- `font_family`
- `font_size`
- `border_radius`
- `border_width`
- `pane_padding`

### Effects

- `glow_size` — `none | thin | medium | large`
- `intensity` — `off | low | medium | heavy`
- `show_focus_border` — keyboard focus ring enable/disable
- `icon_secondary_alpha` — secondary alpha for duotone icons
- `active_wash_alpha` — opacity (`0.0–1.0`) of the accent wash painted over the **active** dock/workspace (the active-workspace highlight in the sidebar). Default `0.11`.
- `card_background_alpha` — opacity (`0.0–1.0`) of a sidebar/list card's resting background tint (each pane card). Default `0.02`.
- `shadow` — elevated shadow token

### Floating panes

- `float_background`
- `float_accent`
- `float_focus`

### Drag and drop

- `drag_ghost_bg`
- `drag_ghost_fg`
- `drag_source_bg`
- `drag_source_border`
- `drop_target_bg`
- `drop_target_border`
- `drop_insertion`

### Sidebar typography

- `sidebar_label_font_size`
- `sidebar_button_font_size`

### Terminal

- `terminal_font_family`
- `terminal_italic_font_family`
- `terminal_font_size`
- `terminal_foreground`
- `terminal_background`
- `terminal_cursor_foreground`
- `terminal_cursor_background`
- `terminal_cursor_border`
- `terminal_selection_foreground`
- `terminal_selection_background`
- `terminal_ansi`
- `terminal_brights`
- `terminal_frost_color`

---

## 6. Bundled themes and exact values

These are the exact shipped bundled theme files today.

## `grid_tron`

```toml
name = "Grid Tron"
background = "#060a0e"
surface = "#0c1218"
foreground = "#c6f0ff"
muted = "#608292"
border = "#143c4c"
accent = "#40e0ff"
glow = "#40e0ff"
danger = "#ff4654"
success = "#50ffaa"
warning = "#ffbe46"
font_family = "Geist Mono"
font_size = 15.0
border_radius = 8.0
border_width = 1.0
pane_padding = 4.0
left_sidebar_background = "#05090d"
right_sidebar_background = "#05090d"
top_bottom_pane_background = "#04080c"
glow_size = "medium"
intensity = "medium"
show_focus_border = true
icon_secondary_alpha = 0.45
active_wash_alpha = 0.11
card_background_alpha = 0.02
shadow = { color = "#000000", alpha = 0.3, blur = 8.0 }
float_background = "#313244"
float_accent = "#89b4fa"
float_focus = "#fab387"
drag_ghost_bg = "#89b4fad9"
drag_ghost_fg = "#ffffff"
drag_source_bg = "#89b4fa26"
drag_source_border = "#89b4fa"
drop_target_bg = "#89b4fa2d"
drop_target_border = "#89b4fa"
drop_insertion = "#89b4fa"
sidebar_label_font_size = 14.0
sidebar_button_font_size = 11.0
terminal_font_family = "Maple Mono Normal NF"
terminal_italic_font_family = "Maple Mono Normal NF"
terminal_font_size = 14.0
```

## `mocha`

```toml
name = "Catppuccin Mocha"
background = "#1e1e2e"
surface = "#313244"
foreground = "#cdd6f4"
muted = "#6c7086"
border = "#313244"
accent = "#89b4fa"
glow = "#89b4fa"
danger = "#f38ba8"
success = "#a6e3a1"
warning = "#fab387"
font_family = "Geist Mono"
font_size = 15.0
border_radius = 6.0
border_width = 1.0
pane_padding = 4.0
left_sidebar_background = "#1a1a28"
right_sidebar_background = "#1a1a28"
top_bottom_pane_background = "#151521"
glow_size = "none"
intensity = "off"
show_focus_border = true
icon_secondary_alpha = 0.45
active_wash_alpha = 0.11
card_background_alpha = 0.02
shadow = { color = "#000000", alpha = 0.3, blur = 8.0 }
float_background = "#313244"
float_accent = "#89b4fa"
float_focus = "#fab387"
drag_ghost_bg = "#89b4fad9"
drag_ghost_fg = "#ffffff"
drag_source_bg = "#89b4fa26"
drag_source_border = "#89b4fa"
drop_target_bg = "#89b4fa2d"
drop_target_border = "#89b4fa"
drop_insertion = "#89b4fa"
sidebar_label_font_size = 14.0
sidebar_button_font_size = 11.0
terminal_font_family = "Maple Mono Normal NF"
terminal_italic_font_family = "Maple Mono Normal NF"
terminal_font_size = 14.0
```

## `latte`

```toml
name = "Catppuccin Latte"
background = "#eff1f5"
surface = "#e6e9ef"
foreground = "#4c4f69"
muted = "#9ca0b0"
border = "#ccd0da"
accent = "#1e66f5"
glow = "#1e66f5"
danger = "#d20f39"
success = "#40a02b"
warning = "#df8e1d"
font_family = "Geist Mono"
font_size = 15.0
border_radius = 6.0
border_width = 1.0
pane_padding = 4.0
left_sidebar_background = "#e4e6ea"
right_sidebar_background = "#e4e6ea"
top_bottom_pane_background = "#d8dade"
glow_size = "none"
intensity = "off"
show_focus_border = false
icon_secondary_alpha = 0.2
active_wash_alpha = 0.11
card_background_alpha = 0.02
shadow = { color = "#000000", alpha = 0.15, blur = 8.0 }
float_background = "#e6e9ef"
float_accent = "#1e66f5"
float_focus = "#e67e22"
drag_ghost_bg = "#1e66f5d9"
drag_ghost_fg = "#ffffff"
drag_source_bg = "#1e66f526"
drag_source_border = "#1e66f5"
drop_target_bg = "#1e66f52d"
drop_target_border = "#1e66f5"
drop_insertion = "#1e66f5"
sidebar_label_font_size = 14.0
sidebar_button_font_size = 11.0
terminal_font_family = "Maple Mono Normal NF"
terminal_foreground = "#4c4f69"
terminal_background = "#e6e9ef00"
terminal_cursor_foreground = "#eff1f5"
terminal_cursor_background = "#4c4f69"
terminal_cursor_border = "#1e66f5"
terminal_selection_foreground = "#4c4f69"
terminal_selection_background = "#bccfef"
terminal_frost_color = "#e6e9ef"
terminal_italic_font_family = "Maple Mono Normal NF"
terminal_font_size = 14.0
```

---

## 7. Current latte behavior

`latte` is now a real bundled `heca-theme` file.

Current behavior:

- `heca-theme` bundled themes: `grid_tron`, `mocha`, `latte`
- `heca-config::theme::load("latte")` delegates directly to `heca_theme::load_theme("latte")`

So the light theme is now unified across the active theme system.

---

## 8. How terminal theming works today

Terminal theming is already **partly theme-driven**.

### Theme-driven today

The app already passes theme terminal defaults into the terminal backend via:

- `heca/src/app/backend_factory.rs`
- `heca_core::backend::TerminalPaletteDefaults`

These theme fields are already used:

- `terminal_foreground`
- `terminal_background`
- `terminal_cursor_foreground`
- `terminal_cursor_background`
- `terminal_cursor_border`
- `terminal_selection_foreground`
- `terminal_selection_background`
- `terminal_ansi`
- `terminal_brights`
- `terminal_font_family`
- `terminal_italic_font_family`
- `terminal_font_size`
- `terminal_frost_color`

### Renderer state

The terminal renderer uses:

- `TerminalSnapshot.default_fg`
- `TerminalSnapshot.default_bg`
- `TerminalSnapshot.cursor_color`

The cursor overlay was previously hardcoded white; it now follows the backend palette through `cursor_color`.

### What is still not fully tokenized

Terminal theming is **not fully finished** yet. Remaining areas include:

- some host-drawn overlays still use policy logic instead of dedicated theme tokens
- selection/caret overlays still derive from app accent in some paths
- some renderer fallback behaviors may still need explicit theme-level tokens if they are considered visual policy rather than protocol defaults

So the terminal is already theme-aware, but not yet fully tokenized end-to-end.

---

## 9. Appearance overrides vs theme values

Important distinction:

- **theme** = the base palette/token set
- **appearance** = local runtime/config overrides for behavior and some pane chrome values

Examples of active appearance overrides that still work:

- `pane_border_color`
- `pane_active_border_color`
- `pane_floating_border_color`
- `pane_border_width`
- `pane_border_radius`
- `pane_padding`
- transparency / blur controls

These appearance overrides take precedence where implemented.

### Not supported anymore

These old config concepts are not part of the current live surface:

- `pane_title_color`
- `pane_title_background`

Do not add those under `[appearance]`. Pane title/header color should be expressed by theme tokens instead.

---

## 10. Practical example

Example custom user theme:

```toml
name = "My Dark Blue"
background = "#111827"
surface = "#1f2937"
foreground = "#e5e7eb"
muted = "#94a3b8"
border = "#334155"
accent = "#38bdf8"
glow = "#38bdf8"
danger = "#fb7185"
success = "#4ade80"
warning = "#f59e0b"
font_family = "Geist Mono"
font_size = 15.0
border_radius = 8.0
border_width = 1.0
pane_padding = 4.0
left_sidebar_background = "#0f172a"
right_sidebar_background = "#0f172a"
top_bottom_pane_background = "#0b1220"
glow_size = "thin"
intensity = "low"
show_focus_border = true
icon_secondary_alpha = 0.35
shadow = { color = "#000000", alpha = 0.25, blur = 8.0 }
float_background = "#1f2937"
float_accent = "#38bdf8"
float_focus = "#f59e0b"
sidebar_label_font_size = 14.0
sidebar_button_font_size = 11.0
terminal_font_family = "Maple Mono Normal NF"
terminal_italic_font_family = "Maple Mono Normal NF"
terminal_font_size = 14.0
terminal_background = "#111827"
terminal_foreground = "#e5e7eb"
terminal_cursor_background = "#e5e7eb"
terminal_cursor_foreground = "#111827"
```

Save as:

```text
~/.config/heca/themes/my_dark_blue.toml
```

Select it with:

```toml
[settings]
theme = "my_dark_blue"
```

---

## 11. Source of truth

If this document and code ever disagree, the source of truth is:

- `heca-theme/src/theme.rs`
- `heca-theme/src/loader.rs`
- `heca-theme/src/themes/*.toml`
- temporary `latte` compat behavior in `heca-config/src/theme.rs`
