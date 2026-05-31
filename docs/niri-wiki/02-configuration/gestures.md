# Gestures Configuration

> Complete reference for the `gestures {}` section. Since 25.02.

```kdl
gestures {
    dnd-edge-view-scroll {
        trigger-width 30
        delay-ms 100
        max-speed 1500
    }

    dnd-edge-workspace-switch {
        trigger-height 50
        delay-ms 100
        max-speed 1500
    }

    hot-corners {
        // off
        top-left
        // top-right
        // bottom-left
        // bottom-right
    }
}
```

## dnd-edge-view-scroll

Scroll tiling view when mouse cursor hits monitor edge during drag-and-drop.

- `trigger-width` (px logical) — edge zone size
- `delay-ms` — prevent unwanted scroll when crossing monitors
- `max-speed` (px/s logical) — max scroll speed (linear from trigger-width to edge)

## dnd-edge-workspace-switch

Since 25.05. Switch workspaces during DnD in overview. Same options as above but with `trigger-height`.

## hot-corners

Since 25.05. Toggle overview by putting mouse at monitor corner.

- `off` — disable all hot corners
- Named corners to enable specific ones (since 25.11)
- Per-output override via `output { hot-corners { ... } }` (since 25.11)
