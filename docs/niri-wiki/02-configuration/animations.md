# Animations Configuration

> Complete reference for the `animations {}` section.

## Types

### Easing (duration + curve)

```kdl
animations {
    window-open {
        duration-ms 150
        curve "ease-out-expo"
    }
}
```

Curves available: `ease-out-quad` (0.1.5), `ease-out-cubic`, `ease-out-expo`, `linear` (0.1.6), `cubic-bezier p1 p2 p3 p4` (25.08).

### Spring (physics-based)

```kdl
animations {
    workspace-switch {
        spring damping-ratio=1.0 stiffness=1000 epsilon=0.0001
    }
}
```

| Parameter | Range | Effect |
|-----------|-------|--------|
| `damping-ratio` | 0.1 – 10.0 | <1 oscillates, >1 overdamped, 1 = critically damped |
| `stiffness` | higher = faster | Lower = slower + more oscillation |
| `epsilon` | lower = smoother end | Prevents "jump" at end |

⚠️ Overdamped (>1.0) has numerical stability issues — not recommended.

## Global Controls

```kdl
animations {
    off          // Disable all animations
    slowdown 3.0 // Slow down by factor (values <1 speed up)
}
```

## Available Animations

### workspace-switch
Default: spring (1.0, 1000, 0.0001). Vertical workspace transitions.

### window-open
Default: easing (150ms, ease-out-expo). Supports `custom-shader`.

### window-close
Since 0.1.5. Default: easing (150ms, ease-out-quad). Supports `custom-shader`.

### horizontal-view-movement
Default: spring (1.0, 800, 0.0001). All horizontal camera movement (focus change, new windows, gestures).

### window-movement
Since 0.1.5. Default: spring (1.0, 800, 0.0001). Individual window movement within workspace.

### window-resize
Since 0.1.5. Default: spring (1.0, 800, 0.0001). Supports `custom-shader`. Small resizes (<10px) not animated.

### overview-open-close
Since 25.05. Default: spring (1.0, 800, 0.0001).

### screenshot-ui-open
Since 0.1.8. Default: easing (200ms, ease-out-quad).

### config-notification-open-close
Default: spring (0.6, 1000, 0.001). Underdamped (oscillates).

### exit-confirmation-open-close
Since 25.08. Default: spring (0.6, 500, 0.01).

### recent-windows-close
Since 25.11. Default: spring (1.0, 800, 0.001).

## Custom Shaders

GLSL fragment shaders for open/close/resize animation appearance. See `examples/open_custom_shader.frag`, `examples/close_custom_shader.frag`, `examples/resize_custom_shader.frag` in the niri repo.

⚠️ No backwards compatibility guarantee for custom shader API.

## Synchronization

When two animations play together (e.g., resize + view movement), the secondary uses the primary's config. Related animations share default parameters for this reason:

- `horizontal-view-movement`
- `window-movement`
- `window-resize`

(all default to spring 1.0, 800, 0.0001)
