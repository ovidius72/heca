# Floating Windows

> Since 25.01. Floating layout, toggle, positioning.

## Behavior

- Each workspace has its own floating layout (same as tiling)
- Always rendered on top of tiled windows
- No scrolling in floating space
- Auto-floating: dialogs (parent windows), splash screens, fixed-size windows
- Workspace tracks which layout is active (`floating_is_active`)

## Interactions

- **Toggle floating**: Mod+V or right-click during interactive move
- **Switch focus**: Mod+Shift+V between floating and tiling layouts
- **Position**: `niri msg action move-floating-window -x 100 -y 200` (IPC)
- **Window rules**: `open-floating true` to force-float, `open-floating false` to prevent
- **Float → Fullscreen**: transfers to scrolling layout (fullscreen only in tiling)
- **Float z-order**: `bring-to-front` via mouse click

## Configuration

Default floating position: center of screen. Customize with `default-floating-position` window rule (since 25.01):
```kdl
window-rule {
    match app-id="^dropdown$"
    open-floating true
    default-floating-position x=0 y=0 relative-to="top"
    default-window-height { proportion 0.5; }
    default-column-width { proportion 0.8; }
}
```
