# Overview (Expose Mode)

> Zoomed-out view of all workspaces with drag-and-drop. Since 25.05.

## Triggers

- `toggle-overview` bind
- Top-left hot corner
- Touchpad 4-finger swipe up

## Interactions

- **Mouse**: click+drag to move windows; right-click+drag to scroll workspaces horizontally; scroll to switch workspaces
- **Touchpad**: 2-finger scrolling
- **Keyboard**: all shortcuts keep working
- **Drag-and-drop**: edge scroll switches workspaces; hold over workspace to activate it

## Layer-Shell Behavior

- Background + bottom layers: zoom with workspaces
- Top + overlay layers: stay on top of overview
- Bar should be on the top layer

## Configuration

```kdl
overview {
    zoom 0.5               // 0–0.75; lower = smaller thumbnails
    backdrop-color "#777777" // color behind workspaces
    workspace-shadow { off }
}
```

## Backdrop Customization

```kdl
// Put wallpaper in the backdrop (stationary, doesn't move with workspaces)
layer-rule {
    match namespace="^wallpaper$"
    place-within-backdrop true
}
layout { background-color "transparent"; }
```
