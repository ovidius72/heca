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

**Why not tmux + a terminal?**

| | tmux in a terminal | heca |
|---|---|---|
| Rendering | CPU text grid | GPU text atlas + primitives |
| Fonts | Limited ligature support | Full HarfBuzz shaping, variable fonts |
| Animations | None | Smooth scroll, zoom, slide |
| Panes | Terminal only | Terminal, Neovim, browser, plugins |
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
- **Double prefix**: `Ctrl+B` `Ctrl+B` sends a literal `Ctrl+B` (0x02) to the focused pane.
- **Bare modifiers**: Holding Shift/Ctrl/Alt alone in prefix mode does nothing — wait for the actual key.

### Navigation

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Focus left | `h` | Activate column left, scroll view |
| Focus right | `l` | Activate column right, scroll view |
| Focus up | `k` | Activate pane above in column |
| Focus down | `j` | Activate pane below in column |
| Next pane | `n` | Cycle to next pane across columns |
| Prev pane | `p` | Cycle to prev pane across columns |
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
| Rename pane | `Shift+P` | Rename active pane |
| Toggle left sidebar | `b` | Show/hide left sidebar |
| Toggle right sidebar | `.` | Show/hide right sidebar |
| Sidebar focus | `e` | Enter sidebar navigation mode |

### Sidebar Navigation Mode

When in sidebar mode (`Ctrl+B → e` or clicking sidebar):

| Key | Action |
|-----|--------|
| `j` / `k` | Move cursor down / up |
| `h` / `l` | Collapse / expand tree node |
| `Enter` | Activate selected item (focus pane/workspace) |
| `Escape` | Exit sidebar mode |

### System

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Command palette | `p` | Open command palette |
| Reload config | `Shift+R` | Reload config.toml at runtime |

---

## Configuration

heca loads config from `~/.config/heca/config.toml` (Linux/macOS) or `%APPDATA%\heca\config.toml` (Windows).

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

# Keybindings — prefix+ syntax for prefix bindings, direct for global
[keys]
focus_left = "prefix+h"
focus_right = "prefix+l"
focus_up = "prefix+k"
focus_down = "prefix+j"
split_horizontal = "prefix+Enter"
split_vertical = "prefix+v"
zoom_column = "prefix+z"
close = "prefix+x"
float = "prefix+f"
pane_select = "prefix+q"
swap_pane = "prefix+Shift+q"
swap_and_focus_pane = "prefix+m"
rename_workspace = "prefix+Shift+w"
rename_column = "prefix+Shift+c"
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
font_family = "JetBrainsMono Nerd Font"
font_size = 32.0
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

[shadow]
color = "#000000"
alpha = 0.3
blur = 8.0
```

### Settings

```toml
[settings]
window_width = 1280           # Initial window width
window_height = 800           # Initial window height
mouse = true                  # Enable mouse interactions
focus_follows_mouse = true    # Focus pane on hover
auto_scroll_edge = true       # Auto-scroll near edges
interactive_move_modifier = "Super"  # Modifier for drag-and-drop
```

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
- `SpawnCommand { command }` — Run an external command

### Registry Dispatch

All actions go through a central registry:

```
Keyboard input → KeyCombo → KeymapRegistry → WmAction → ActionRegistry → Handler
```

This design means:
- Every action is traceable and hookable
- Future scripting/IPC can trigger any action by name
- Actions can be composed and chained

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

5. **Register** in `build_registry()` in `heca/src/main.rs`:
```rust
registry.register(&WmAction::MyCustomAction, handle_my_custom_action);
```

6. **Add default binding** in `heca-config/src/theme.rs`:
```rust
bindings.insert("my_custom_action".to_string(), Single("prefix+y".to_string()));
```

---

## Keybindings

### Default Keybindings

See [`keybindings.toml`](keybindings.toml) for a complete reference of all default keybindings that can be customized.

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

Today, heca can launch simple command-bound panes using `[[keys.command]]`:

```toml
[[keys.command]]
keys = "prefix+g"
command = "lazygit"

[[keys.command]]
keys = "prefix+t"
command = "btm"  # bottom system monitor

[[keys.command]]
keys = "Alt+Enter"
command = "alacritty"
```

This currently creates a new pane with the command as its title; real backend execution is still evolving.

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
┌─────────────────────────────────────────┐
│  heca (main binary)                     │
│  ├── winit event loop                   │
│  ├── input routing (prefix/keymap)      │
│  ├── ActionRegistry dispatch            │
│  ├── render() — GPU compositor          │
│  └── chrome (tab bar, sidebars, status) │
├─────────────────────────────────────────┤
│  heca-renderer                          │
│  ├── PrimitiveRenderer (rects, borders) │
│  └── TextRenderer (cosmic-text atlas)   │
├─────────────────────────────────────────┤
│  heca-core                              │
│  ├── layout/                            │
│  │   ├── Session → Workspace → Column → Pane
│  │   ├── ViewOffset (animated scroll)   │
│  │   └── Animation (easing, springs)    │
│  └── backend/                           │
│      ├── PaneBackend trait              │
│      ├── TerminalBackend (PTY + vte)    │
│      └── FakeBackend (layout testing)   │
├─────────────────────────────────────────┤
│  heca-config                            │
│  └── TOML config + theme loading        │
└─────────────────────────────────────────┘
```

**Key design decisions:**

- **NIRI layout engine**: Horizontal scrolling columns, not BSP trees. Column widths are independent.
- **Separation of concerns**: Layout in `heca-core`, GPU in `heca-renderer`, orchestration in `heca`.
- **ActionRegistry**: All WM commands go through one dispatch point. Every action is a `WmAction` enum variant.
- **KeymapRegistry**: Mode-specific keymaps. Modes are groups of bindings active until Esc/Enter.
- **Prefix mode**: Intentionally tmux-style to avoid conflicts with hosted applications.
- **PaneBackend trait**: All content sources (terminal, Neovim, browser) implement the same interface.

---

## Roadmap

| Phase | Status | Description |
|-------|--------|-------------|
| **1 — The Shell** | ✅ Complete | GPU window, text rendering, theme system |
| **2 — The Workspace** | ✅ Complete | NIRI layout, animations, input, sidebar |
| **3 — The Content** | 🔄 In Progress | Terminal backend, Neovim msgpack-RPC |
| **4 — The Platform** | 📋 Planned | Session persistence, JSON-RPC, plugins |

See `.planning/ROADMAP.md` for detailed requirements.

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

## Acknowledgments

- **[NIRI](https://github.com/YaLTeR/niri)** by Ivan Molodetskikh — The scrollable-tiling compositor that inspired heca's layout engine.
- **[cosmic-text](https://github.com/pop-os/cosmic-text)** by System76 — GPU text rendering with excellent font shaping.
- **[wgpu](https://wgpu.rs/)** — Safe, portable GPU API for Rust.
