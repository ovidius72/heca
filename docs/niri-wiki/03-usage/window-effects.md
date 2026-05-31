# Window Effects

> Background effects: blur, xray, saturation, noise. Since 26.04.

## Effects

| Effect | Description |
|--------|-------------|
| **Blur** | Blurs content behind window (dual kawase) |
| **Xray** | Makes window background see-through to wallpaper only (more efficient) |
| **Noise** | Adds pixel noise to reduce color banding |
| **Saturation** | Color saturation of blurred background |

## Enabling

```kdl
// Global blur settings
blur { passes 3; offset 3.0; noise 0.02; saturation 1.5; }

// Per-window rule
window-rule {
    match app-id="^Alacritty$"
    background-effect { blur true; }
}

// Per-layer rule
layer-rule {
    match namespace="^launcher$"
    background-effect { blur true; }
}
```

## Xray vs Non-xray

- **Xray** (default with blur): only blurs the wallpaper, reuses the result across windows. Much more GPU-efficient.
- **Non-xray** (experimental): blurs everything below the window. More expensive, disappears during open/close animations and window drag.

## Window-side request

Apps can request blur via the `ext-background-effect` protocol. The blur follows the window's exact shape (including rounded corners) without manual niri configuration.
