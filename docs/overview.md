# What heca is

heca is a **workspace compositor** — not a traditional terminal emulator or window manager. It hosts multiple content panes (terminals, Neovim, browsers) inside a single native window with a NIRI-inspired layout engine:

- **Scrolling columns** — Panes are arranged in horizontal columns that scroll smoothly. Focus left/right animates the view to the active column.
- **Vertical stacks** — Within each column, panes stack vertically with intelligent height distribution.
- **Workspaces** — Multiple workspaces stacked vertically, switched with animated transitions.
- **Floating panes** — Any pane can be detached from the scrolling layout and positioned freely.
- **GPU-native chrome** — Tab bar, sidebars, status bar, pane borders — all rendered in one GPU pass.
- **Future pluggable chrome** — the long-term architecture is evolving toward left/right/top/bottom chrome regions that host built-in containers first and WASM/plugin containers later.

**Why not tmux + a terminal?**

| | tmux in a terminal | heca |
| --- | --- | --- |
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
