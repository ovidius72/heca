# niri Architecture

> Full reference of niri's system architecture, layout engine internals, rendering pipeline, and data flow.

## Relationship to heca (read first)

This document describes **upstream niri** — the Wayland compositor whose **scrollable-tiling layout model**
heca adopts (workspaces → columns → panes, view-offset scrolling, "open a window without resizing others").
heca is **not** a Wayland compositor: it is a terminal-multiplexer host rendering with **wgpu** (vs niri's
GLES/Vulkan + smithay), and it drives bindings through a **tmux-style prefix** + `ActionRegistry`/
`KeymapRegistry` (vs niri's direct `Mod`+key bindings).

- **Where heca matches niri:** the layout engine in `heca-core/src/layout/` (Session → Workspace →
  `ScrollingSpace` → Column) mirrors niri's data model and scrolling principles.
- **Where heca diverges (and known gaps):** see [`../../niri-compatibility-review.md`](../../niri-compatibility-review.md)
  — re-verified 2026-06-17. Notably still-open: per-mutation width recompute vs niri's "no reflow" (L1/L2),
  and pane-focus falling through to workspace-switch (L4). The keybinding gaps in that review are largely
  resolved by the registry rewrite.

> Keep both docs current: this file = upstream-niri reference; the compat review = the heca↔niri delta.

## System Architecture

```
┌─────────────────────────────────────────────────────┐
│                    niri compositor                    │
│  ┌─────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │ Layout   │  │  Input   │  │     Render         │  │
│  │ Engine   │◄─┤  Router  │──┤     Compositor     │  │
│  │          │  │          │  │                    │  │
│  │ Session  │  │ keyboard │  │ OpenGL ES / Vulkan │  │
│  │ ├─WSs    │  │ mouse    │  │ damage tracking    │  │
│  │ ├─Cols   │  │ touchpad │  │ offscreen render   │  │
│  │ ├─Floats │  │ gestures │  │ direct scanout     │  │
│  │ └─Overview│  │ DnD     │  │ VBlank sync        │  │
│  └─────────┘  └──────────┘  └────────────────────┘  │
│       │              │                │              │
│       ▼              ▼                ▼              │
│  ┌────────────────────────────────────────────────┐  │
│  │            Wayland Protocol Layer               │  │
│  │  (wlroots/smithay — protocols, surfaces, etc.)  │  │
│  └────────────────────────────────────────────────┘  │
│       │              │                │              │
│       ▼              ▼                ▼              │
│  ┌────────────────────────────────────────────────┐  │
│  │          DRM/KMS + libinput + dbus              │  │
│  │  (hardware: displays, input devices, portals)   │  │
│  └────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

## Layout Engine Internals

### Monitor/Output Layer

```
Output (physical display)
├── workspaces: Vec<Workspace>     ← arranged VERTICALLY (stacked)
│   ├── Workspace 1
│   │   ├── scrolling: ScrollingSpace   ← horizontal COLUMNS
│   │   └── floating: FloatingSpace
│   ├── Workspace 2
│   └── ...
├── scale: f64                      ← logical-to-physical pixel ratio
├── mode: (width, height, refresh)  ← current display mode
├── position: (x, y)               ← in global output space
├── transform: "normal" | "90" | ...
├── vrr: bool                      ← variable refresh rate
├── backdrop_color: [f32; 4]       ← between-workspace color
└── workspace_config: LayoutOptions ← per-output layout overrides
```

### Workspace

```
Workspace
├── scrolling: ScrollingSpace       ← tiling layout
│   ├── columns: Vec<Column>
│   ├── active_column_idx: usize
│   ├── view_offset: ViewOffset     ← animated horizontal scroll
│   ├── working_area: Rectangle
│   ├── gaps: f64
│   └── struts: { left, right, top, bottom }
├── floating: FloatingSpace         ← always-on-top windows
│   ├── tiles: Vec<FloatingTile>
│   └── focus_stack: Vec<ID>       ← z-order
├── floating_is_active: bool
├── id: WorkspaceId
├── name: Option<String>            ← "browser", "chat", etc.
├── original_output: OutputId       ← for monitor dis/reconnect
├── view_size: Size
├── scale: Scale
└── is_pinned: bool                 ← keep even when empty
```

### ScrollingSpace (Core Layout)

```
ScrollingSpace
├── columns: Vec<Column>
│   ├── tiles: Vec<Tile>            ← must be non-empty
│   ├── data: Vec<TileData>         ← cached heights
│   ├── active_tile_idx: usize
│   ├── width: ColumnWidth           ← Proportion | Fixed
│   ├── is_full_width: bool
│   ├── display_mode: ColumnDisplay  ← Normal | Tabbed
│   ├── move_offset: Animated<f64>   ← during reordering
│   └── sizing_mode: SizingMode      ← Normal | Maximized | Fullscreen
├── data: Vec<ColumnData>           ← cached widths
├── active_column_idx: usize
├── view_offset: ViewOffset         ← THE horizontal scroll (3 states)
├── interactive_resize: Option<...>
├── working_area: Rectangle
├── scale: f64
└── options: LayoutOptions
```

### ViewOffset (Three-State Scroll)

```rust
enum ViewOffset {
    Static(f64),            // Stationary
    Animation(Animation),   // Smooth scroll to target (spring or easing)
    Gesture(ViewGesture),   // Touchpad/mouse drag with velocity
}
```

`view_pos() = column_x(active_column_idx) + view_offset.current()`

On focus change:
1. Compute new `target_offset` so active column is visible
2. If difference > 1 physical pixel: start animation (spring by default)
3. Gesture: swipe tracker accumulates samples → velocity → projected end position
4. On gesture end: snap to nearest column boundary (with animation)

### Column Width Management

- **Proportion** = fraction of output width (bakes in gaps)
- **Fixed** = absolute logical pixels
- Resize changes **only the active column** — no rebalancing
- `set_column_width(delta)` → `self.width = width` (this column only)
- **Expand column**: special action that adds remaining available space to active column
- Preset widths: cycle through predefined proportions

### Window Height Distribution (NIRI Algorithm)

```
Given: working_height, gaps, N panes in column
1. total_gaps = gaps * (N + 1)
2. available = working_height - total_gaps
3. height_remaining = available

// First pass: subtract fixed heights
For each pane with preferred_height:
    height_remaining -= preferred_height
    auto_count -= 1

// Second pass: distribute to auto panes
auto_height = height_remaining / auto_count
For each auto pane:
    pane.height = auto_height

// Last auto pane gets remainder (to avoid pixel gaps)
last_auto.height += height_remaining % auto_count
```

### Workspace Switch

- Discrete index change between workspaces (vertical)
- `WorkspaceSwitch::Animation { from_idx, to_idx, progress }`
- Uses spring animation by default
- Input goes to target workspace immediately (doesn't wait for animation)

### Overview (Expose Mode)

```
OverviewState
├── open: bool
├── progress: Animated<f64>    // 0=normal view, 1=fully zoomed out
├── zoom: f64                  // final zoom factor (e.g. 0.25)
├── backdrop_color: [f32; 4]
├── workspace_shadow: ShadowConfig
└── workspace_gap: f64

OverviewProgress
└── Animation(Animation) | Gesture(OverviewGesture) | Open
```

**Rendering:** All workspaces rendered as `RescaleRenderElement` + `RelocateRenderElement`, scaled down with shadows. Layer background+bottom layers zoom with workspaces; top+overlay stay on top.

## Render Pipeline

```
Per-frame render:
1. Layout pass
   ├── Compute pixel rectangles for all tiles
   │   ├── scrolling: columns → tiles_with_render_positions()
   │   └── floating: absolute positions
   ├── Compute workspace geometries (overview vs normal)
   └── Determine damage regions

2. Content update
   ├── Poll all visible windows for new surface contents
   ├── Apply animations (advance all active animations)
   └── Mark damaged surfaces

3. Composite pass (single frame)
   ├── Clear to background color or backdrop
   ├── If overview: render scaled workspaces
   ├── Render tiled windows (ordered by position)
   │   ├── Shadow behind each window
   │   ├── Border / focus ring around each window
   │   ├── Window surface content
   │   └── Tab indicator (for tabbed columns)
   ├── Render floating windows (on top)
   ├── Render layer-shell overlays (top, overlay layers)
   ├── Render cursor
   └── Apply screen transitions (if any)

4. Present
   ├── Queue to DRM with predicted VBlank timestamp
   └── Wait for VBlank → advance RedrawState
```

## Data Flow

```
Input Event (kernel)
└── libinput event
    └── niri input handler
        ├── Gesture recognition (touchpad 3/4-finger, DnD, hot corner)
        ├── Key binding resolution ↓
        │   ├── Match found → execute action (focus, spawn, resize, etc.)
        │   └── No match → forward to focused window as Wayland input event
        └── Mouse event ↓
            ├── Hit-test against tile positions, floating windows
            ├── If on tile: focus / interactive move or resize
            ├── If on chrome (border, tab indicator): activate
            └── If on nothing: no-op

Action Execution
└── Mutate layout state
    ├── Change active column / focus
    ├── Resize / move / add / remove window
    ├── Start animations
    └── Queue redraw

Render Pass
└── Read current layout state
    ├── Compute pixel positions (with animation offsets applied)
    ├── Render window contents at computed positions
    └── Submit frame to DRM

Configuration Change
└── File watcher detects edit
    ├── Parse new config
    ├── Merge with current state
    ├── Apply changes (bindings, layout settings, rules)
    └── Queue redraw if visual changes

IPC Request
└── JSON-RPC on Unix socket
    ├── Query: read state (outputs, windows, workspaces)
    ├── Command: execute action (same as keybind actions)
    └── Event stream: subscribe to continuous state updates
```

## Coordinate Systems

| Space | Unit | Used For |
|-------|------|----------|
| Physical | Device pixels | DRM buffer sizes, surface textures |
| Logical | Scaled pixels (physical/scale) | Layout computation, tile positions |
| Global | Logical coordinates across all outputs | Monitor positioning, cursor movement across outputs |

**Rounding rule:** All sizes rounded to integer physical pixels: `(logical_size * scale).round() / scale`. This includes gaps, struts, border widths, working area. View offset is NOT rounded (continuous). Individual tile render positions ARE rounded.

## Configuration Loading

```
1. Check $NIRI_CONFIG env var (takes precedence)
2. Check --config CLI flag (takes precedence over env)
3. Check ~/.config/niri/config.kdl
4. Check /etc/niri/config.kdl
5. If none found: create ~/.config/niri/config.kdl from embedded default
```

Live-reload: file watcher monitors config file. On edit → parse → merge → apply. No restart needed.

## Key Protocols Implemented

- `wlr-layer-shell-unstable-v1` — desktop panels, notifications, launchers
- `wlr-screencopy-unstable-v1` — screen capture
- `ext-session-lock-v1` — secure lock screen
- `xdg-activation-v1` — window focus tokens
- `xdg-decoration-unstable-v1` — server-side decorations
- `ext-data-control-v1` — clipboard management
- `wlr-virtual-pointer-unstable-v1` — virtual input
- `virtual-keyboard-unstable-v1` — virtual keyboard
- `ext-background-effect-v1` — per-window blur requests (since 26.04)
- `security-context-v1` — filtered Wayland socket for sandboxed apps
