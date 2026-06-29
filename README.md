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
| Move pane to column | `Shift+C` | Overlay letters on columns (any workspace); press letter to stack the active pane into that column |

### Resize

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Resize increase | `=` | Widen active column |
| Resize decrease | `-` | Narrow active column |
| Pane height increase | `Shift+=` | Grow active pane height |
| Pane height decrease | `Shift+-` | Shrink active pane height |

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
| Rename workspace | `Shift+W` | Rename current workspace |
| Rename pane | `$` | Rename active pane |
| Toggle left sidebar | `b` | Show/hide left sidebar |
| Toggle right sidebar | `.` | Show/hide right sidebar |
| Sidebar focus | `e` | Enter sidebar navigation mode |
| Collapse current workspace | `<` | Collapse the active workspace tree row (UI only) |
| Collapse current column | `(` | Collapse the focused tiled column tree row (UI only) |

Today, sidebar navigation operates on the built-in workspace tree shown in the left sidebar. Long-term, the sidebar is expected to evolve into a shell/host for pluggable containers, with the current workspace tree becoming a built-in `WorkspacesContainer`.

The default sidebar-mode bindings are defined via `[[keys.mode]] name = "sidebar"` and can be overridden in `config.toml`. The trigger field is ignored for this built-in mode because `SidebarNav` is entered via `SidebarFocus` or mouse interaction.

Sidebar-mode mutation keys (`w`, `c`, `v`, `z`, `d`) only work while in sidebar navigation mode and act on the selected sidebar row. Global collapse bindings (`<` and `(`) act on the active main-view workspace/column and do not open the sidebar.

### Sidebar Navigation Mode

When in sidebar mode (`Ctrl+B → e` or clicking the current workspace-tree sidebar):

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor down / up |
| `Up` / `Down` | Move cursor up / down |
| `h` / `l` | Collapse / expand tree node |
| `Left` / `Right` | Collapse / expand tree node |
| `Space` | Same as `l` / `Right` (leaf focus or expand) |
| `Tab` | Toggle collapse of the selected row |
| `w` / `c` / `v` / `z` / `d` | Sidebar-only mutation keys (create workspace/column, split pane, zoom, delete) |
| `b` | Toggle the left sidebar |
| `Enter` | Activate selected item (focus pane/workspace) |
| `Escape` | Exit sidebar mode |

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
| `y` | Copy selection to clipboard |
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
| Reload config | `Shift+R` | Reload config.toml at runtime |

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
move_pane_to_column_pick = "prefix+Shift+c"
rename_workspace = "prefix+Shift+w"
rename_pane = "prefix+$"
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
terminal_mouse = true         # Enable host scrollback on wheel (vs forwarding to terminal)
terminal_wheel_scroll_lines = 3  # Rows per wheel notch when scrolling host viewport
terminal_scroll_animations = true  # Smooth animated terminal viewport jumps
```

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
and `undercurl` add a straight or wavy line in the link color. (Opening links on
click is a separate, upcoming feature.)

When `shell_integration = false`, heca spawns a bare interactive shell and you can source the generated snippets manually from `~/.config/heca/runtime/shell-integration/`.

### Pane Info Bar

Each pane shows a small **info bar** along its top: configurable **segments** on the
left (what the pane is) and **action buttons** on the right. Both are configured
under `[appearance.pane]` as ordered lists — order in the list is the order shown
(left → right). An empty list hides that side; if **both** are empty the bar (and
its reserved space) disappears entirely.

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
| `app_name`    | Resolved program / app name (from the [Process Catalog](#process-catalog)) |
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

### Registry Dispatch

All actions go through a central registry:

```
Keyboard input → KeyCombo → KeymapRegistry → WmAction → ActionRegistry → Handler
```

This design means:
- Every action is traceable and hookable
- Future scripting/IPC/RPC can trigger any action by name
- Actions can be composed and chained
- Important capabilities should not be trapped behind one surface: when meaningful, the same action should be reachable from mouse/UI, keybindings, and RPC

### Creating Custom Actions

To add a new action to heca:

1. **Add to `WmAction` enum** in `heca/src/input.rs`:
```rust
pub enum WmAction {
    // ... existing variants
    MyCustomAction,
}
```

2. **Add name mapping** in `action_from_name()`:
```rust
"my_custom_action" => Some(WmAction::MyCustomAction),
```

3. **Add priority** in `action_priority()`:
```rust
WmAction::MyCustomAction => 1,  // Lower = higher priority
```

4. **Create handler** in `heca/src/handlers.rs`:
```rust
pub fn handle_my_custom_action(state: &mut AppState, _action: &WmAction) {
    // Your logic here
    state.needs_redraw = true;
}
```

5. **Register** in `build_registry()` in `heca/src/app/registry.rs`:
```rust
registry.register(&WmAction::MyCustomAction, handle_my_custom_action);
```

6. **Add default binding** in `heca-config/src/theme.rs`:
```rust
bindings.insert("my_custom_action".to_string(), Single("prefix+y".to_string()));
```

### Interaction Policy

Not every action is allowed in every context. heca tracks a per-workspace **focus domain** — `Tiled` (the scrolling column layout) or `Floating` (a detached floating pane is active) — and an interaction policy layer decides whether each action is allowed before it runs.

**When a floating pane is active**, most actions are blocked so the floating pane stays put and the tiled layout isn't disturbed. These still work while floating:

- **Pane-local actions** — `float` (toggle back to tiled), `close`, `rename`, and text selection.
- **Global app actions** — `reload_config` (hot-reload always works, even with a floating pane open).

Blocked while floating: focus/split/resize/swap/move, sidebar navigation, workspace switching, command palette, spawn, and pane select/swap overlays. The only ways to leave the floating domain are `prefix+f` (toggle float) or closing the floating pane.

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
# In resize mode:
#   h / l  → resize column narrower / wider
#   j / k  → resize pane shorter / taller
#   Arrow keys work too
#   Escape / Enter → exit mode

# Custom modes
[[keys.mode]]
name = "my_mode"
trigger = "prefix+o"      # How to enter the mode
sticky = true             # true = stay until Esc/Enter

[[keys.mode.bindings]]
action = "focus_left"
keys = "h"

[[keys.mode.bindings]]
action = "focus_right"
keys = "l"
```

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
- **Sidebar shell vs container**: the long-term design separates the sidebar shell from the mounted content container. The current workspace tree should evolve into a built-in `WorkspacesContainer`, not remain the definition of the sidebar itself.
- **Action reachability**: important actions should be reachable from mouse/UI, keybindings, and RPC when meaningful on those surfaces.

---

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
