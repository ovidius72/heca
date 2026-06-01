# heca

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
- [Mouse Interaction](#mouse-interaction)
- [Configuration](#configuration)
- [Adding Panes and Workspaces](#adding-panes-and-workspaces)
- [Customizing Keybindings](#customizing-keybindings)
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
- **Overview mode** — Zoom out to see all workspaces as scaled thumbnails.
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
| Focus left | `h` / `←` | Activate column left, scroll view |
| Focus right | `l` / `→` | Activate column right, scroll view |
| Focus up | `k` / `↑` | Activate pane above in column |
| Focus down | `j` / `↓` | Activate pane below in column |
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
| Close | `x` | Close active pane (protected: won't close last pane) |
| Float | `f` | Toggle pane between scrolling and floating |
| Pane select | `q` | Overlay letters on panes; press letter to focus |
| Swap select | `Shift+Q` | Overlay letters; press letter to swap positions |
| Swap and focus | `m` | (Used with SwapSelect) focus target after swap |

### Resize

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Resize increase | `=` | Widen active column |
| Resize decrease | `-` | Narrow active column |
| Pane height increase | `Shift+=` | Increase active pane's preferred height |
| Pane height decrease | `Shift+-` | Decrease active pane's preferred height |

### Move

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Move pane left | `[` | Move active pane to column on left (or new column) |
| Move pane right | `]` | Move active pane to column on right (or new column) |
| Swap left | `Ctrl+H` | Swap active pane with pane to the left |
| Swap right | `Ctrl+L` | Swap active pane with pane to the right |
| Swap up | `Ctrl+K` | Swap active pane with pane above |
| Swap down | `Ctrl+J` | Swap active pane with pane below |

### Workspace & Sidebar

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Create workspace | `w` | Create new workspace with a pane |
| Rename workspace | `Shift+W` | Rename current workspace |
| Rename pane | `Shift+P` | Rename active pane |
| Toggle left sidebar | `b` | Show/hide left sidebar |
| Toggle right sidebar | `.` | Show/hide right sidebar |
| Sidebar focus | `e` | Enter sidebar navigation mode |

### Sidebar Navigation Mode (when sidebar is focused)

| Command | Binding | Description |
|---------|---------|-------------|
| Down | `j` / `↓` | Move cursor down |
| Up | `k` / `↑` | Move cursor up |
| Expand/collapse | `Tab` / `Space` | Toggle folder expansion |
| Activate | `l` / `Enter` | Focus selected workspace/pane |

### Tabs

| Command | Default Binding | Description |
|---------|----------------|-------------|
| Tab next | `Ctrl+]` | Next workspace tab |
| Tab prev | `Ctrl+[` | Previous workspace tab |

---

## Mouse Interaction

heca has a fully configurable mouse interaction system. All features are enabled by default (`general.mouse = true`).

### Click to Focus

**Left-click** anywhere inside a pane to focus it. Works for both scrolling and floating panes.

### Focus Follows Mouse

When enabled (`general.focus_follows_mouse = true`, default), hovering over a pane automatically focuses it — no click required. Disabled during modal modes (Pane Select, Swap, Rename, Sidebar Nav) to prevent accidental focus changes.

### Interactive Pane Move (Drag and Drop)

Hold a **modifier key** + **left-click** on a pane to grab and drag it to a new position.

**Config:**
```toml
[general]
interactive_move_modifier = "Super"  # Options: "Super", "Alt", "Ctrl", "Shift"
```

**How it works:**

1. **Grab** — Modifier+click on a pane. The pane rubberbands with your cursor (visual feedback).
2. **Drag** — Move the mouse. Once you drag past a threshold (8px), the pane detaches from the layout and follows your cursor.
3. **Insert hint** — A translucent bar shows where the pane will drop:
   - **Vertical bar** (24px wide, 60% height) = new column
   - **Horizontal bar** (full width, 24px tall) = within existing column
4. **Drop** — Release to place the pane. It animates smoothly into its new position.

### Edge Scroll

When enabled (`general.auto_scroll_edge = true`, default), hovering near the left/right edge of the content area automatically scrolls the layout. Works both during drag-and-drop and on plain hover.

| Mode | Trigger zone | Speed |
|------|-------------|-------|
| Hover | 80px from edge | 300 px/sec |
| Drag | 150px from edge | 1000 px/sec |

Scrolling stops at the first/last column — it never scrolls past content bounds.

### Sidebar Click

Click on items in the left sidebar to focus them directly:
- **Pane item** — Switches to that pane's workspace and focuses it
- **Workspace item** — Switches to that workspace

---

## Configuration

heca loads config from `~/.config/heca/config.toml` (Linux/macOS) or `%APPDATA%\heca\config.toml` (Windows).

### Minimal Config

```toml
theme = "mocha"

[general]
window_width = 1280
window_height = 800
mouse = true
focus_follows_mouse = true
auto_scroll_edge = true
interactive_move_modifier = "Super"

[keybindings]
focus_left = "h,ArrowLeft"
focus_right = "l,ArrowRight"
focus_up = "k,ArrowUp"
focus_down = "j,ArrowDown"
split_horizontal = "Enter"
split_vertical = "v"
close = "x"
float = "f"
pane_select = "q"
swap_select = "Shift+q"
resize_increase = "="
resize_decrease = "-"
move_pane_left = "["
move_pane_right = "]"
sidebar_left = "b"
```

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

[shadow]
color = "#000000"
alpha = 0.3
blur = 8.0
```

### General Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `window_width` | `u32` | `1280` | Initial window width |
| `window_height` | `u32` | `800` | Initial window height |
| `mouse` | `bool` | `true` | Enable mouse interactions |
| `focus_follows_mouse` | `bool` | `true` | Focus pane on hover |
| `auto_scroll_edge` | `bool` | `true` | Auto-scroll near edges |
| `interactive_move_modifier` | `String` | `"Super"` | Modifier for drag-and-drop |

---

## Adding Panes and Workspaces

### Add a Pane (Vertical Split)

```
Ctrl+B → v
```

Creates a new pane below the active one in the same column.

### Add a Column (Horizontal Split)

```
Ctrl+B → Enter
```

Creates a new column to the right of the active column.

### Create a Workspace

```
Ctrl+B → w
```

Creates a new workspace with a single pane and switches to it.

### Close a Pane

```
Ctrl+B → x
```

Closes the active pane. The last pane in a workspace is protected — you cannot close it. To close a workspace, close all its panes or switch away.

### Float a Pane

```
Ctrl+B → f
```

Toggles the active pane between the scrolling layout and floating. Floating panes:
- Render on top of scrolling panes
- Are positioned freely within the workspace
- Remember their original position for restoration

---

## Customizing Keybindings

All keybindings live in the `[keybindings]` table of `config.toml`. The format is:

```toml
[keybindings]
action_name = "key"
action_name = "key1,key2"  # Multiple keys for same action
```

### Modifier Syntax

```toml
# Single key
focus_left = "h"

# With modifier
swap_left = "Ctrl+h"
resize_increase = "Shift+="

# Multiple bindings (comma-separated)
focus_left = "h,ArrowLeft"
```

### Supported Modifiers

- `Ctrl` — Control key
- `Shift` — Shift key
- `Shift+Ctrl` — Both (order doesn't matter)

### Special Keys

Use the key name directly: `Enter`, `Space`, `Tab`, `Escape`, `Backspace`, `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`, `Home`, `End`, `PageUp`, `PageDown`, `Delete`.

### Full Action List

| Action Name | Description |
|-------------|-------------|
| `focus_left` / `focus_right` / `focus_up` / `focus_down` | Navigate between panes |
| `split_horizontal` | New column |
| `split_vertical` | New pane in column |
| `close` | Close active pane |
| `float` | Toggle floating |
| `resize_increase` / `resize_decrease` | Column width |
| `pane_height_increase` / `pane_height_decrease` | Pane height |
| `move_pane_left` / `move_pane_right` | Move pane to adjacent column |
| `swap_left` / `swap_right` / `swap_up` / `swap_down` | Swap with adjacent pane |
| `pane_select` | Quick-select by letter |
| `swap_select` | Quick-swap by letter |
| `swap_and_focus` | Focus after swap |
| `next_pane` / `prev_pane` | Cycle panes |
| `workspace_next` / `workspace_prev` | Switch workspace |
| `create_workspace` | New workspace |
| `rename_workspace` | Rename workspace |
| `rename_pane` | Rename pane |
| `focus_toggle_local` / `focus_toggle_global` | Toggle last focused |
| `sidebar_left` / `sidebar_right` | Toggle sidebars |
| `sidebar_focus` | Focus sidebar |
| `tab_next` / `tab_prev` | Workspace tabs |
| `command_palette` | Command palette |

---

## Architecture

```
┌─────────────────────────────────────────┐
│  heca (main binary)                     │
│  ├── winit event loop                   │
│  ├── input routing (prefix mode)        │
│  ├── render() — GPU compositor          │
│  └── chrome (tab bar, sidebars, status) │
├─────────────────────────────────────────┤
│  heca-renderer                          │
│  ├── PrimitiveRenderer (rects, borders) │
│  └── TextRenderer (cosmic-text atlas)   │
├─────────────────────────────────────────┤
│  heca-core                              │
│  ├── layout/                            │
│  │   ├── Session → Workspace → Column → Pane │
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
- **`PaneBackend` trait**: All content sources (terminal, Neovim, browser) implement the same interface.
- **Prefix mode**: Intentionally tmux-style to avoid conflicts with hosted applications.

---

## Roadmap

| Phase | Status | Description |
|-------|--------|-------------|
| **1 — The Shell** | ✅ Complete | GPU window, text rendering, theme system |
| **2 — The Workspace** | ✅ Complete | NIRI layout, animations, input, sidebar |
| **3 — The Content** | 🔄 In Progress | Terminal backend, Neovim msgpack-RPC, mouse forwarding |
| **4 — The Platform** | 📋 Planned | Session persistence, JSON-RPC, plugins, damage tracking |

See `.planning/ROADMAP.md` for detailed requirements.

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

## Acknowledgments

- **[NIRI](https://github.com/YaLTeR/niri)** by Ivan Molodetskikh — The scrollable-tiling compositor that inspired heca's layout engine.
- **[cosmic-text](https://github.com/pop-os/cosmic-text)** by System76 — GPU text rendering with excellent font shaping.
- **[wgpu](https://wgpu.rs/)** — Safe, portable GPU API for Rust.
