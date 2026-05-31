# Window Rules

> Complete reference for `window-rule {}` blocks.

## Matching

Multiple matchers in one directive = AND. Multiple directives = OR (any match applies):

```kdl
window-rule {
    match app-id="firefox" title="Gmail"    // Firefox AND Gmail in title
    match app-id=r#"^org\.telegram\.desktop$"#   // OR Telegram
    exclude app-id=r#"^org\.telegram\.desktop$"# title="^Media viewer$"  // except media viewer
}
```

Matchers are **regular expressions** that match anywhere in the string.

### Matchers

- `title="regex"` — window title
- `app-id="regex"` — Wayland app ID. Find via `niri msg pick-window` or Waybar tooltip.
- `is-active=true/false` — active window (one per workspace on focused monitor)
- `is-focused=true/false` — keyboard-focused window (only one)
- `is-active-in-column=true/false` — last-focused in each column (since 0.1.6)
- `is-floating=true/false` — floating windows (since 25.01)
- `is-window-cast-target=true/false` — window screencast target (since 25.02)
- `is-urgent=true/false` — attention request (since 25.05)
- `at-startup=true/false` — first 60 seconds after niri start (since 0.1.6)

## Window-Opening Properties

Apply once when window opens:

- `default-column-width { proportion 0.5; }` or `{ fixed 1200; }`
- `default-window-height { fixed 500; }` (since 25.01)
- `open-on-output "HDMI-A-1"` (since 0.1.9 accepts manufacturer+model+serial)
- `open-on-workspace "chat"` (since 0.1.6)
- `open-maximized true/false`
- `open-maximized-to-edges true/false` (since 25.11)
- `open-fullscreen true/false`
- `open-floating true/false` (since 25.01)
- `open-focused true/false` (since 25.01)

## Dynamic Properties

Apply continuously:

- `block-out-from "screencast"` or `"screen-capture"` (black rectangle on cast)
- `opacity 0.5` (0.0–1.0)
- `variable-refresh-rate true` (since 0.1.9)
- `default-column-display "tabbed"` (since 25.02)
- `default-floating-position x=100 y=200 relative-to="bottom-left"` (since 25.01)
- `scroll-factor 0.75` (since 25.02)
- `draw-border-with-background true/false`
- `geometry-corner-radius 12` (since 0.1.6) — also supports 4 values per-corner
- `clip-to-geometry true` (since 0.1.6)
- `tiled-state true/false` (since 25.05)
- `baba-is-float true` — April Fools floating animation (since 25.02)
- `min-width 100`, `max-width 200`, `min-height 300`, `max-height 300`

### Sub-blocks

- `focus-ring { off; on; width; active-color; inactive-color; urgent-color; gradients; }`
- `border { ... }` — same options
- `shadow { on; off; softness; spread; offset; draw-behind-window; color; }` (since 25.02)
- `tab-indicator { active-color; inactive-color; urgent-color; gradients; }` (since 25.02)
- `background-effect { xray true; blur true; noise 0.05; saturation 3; }` (since 26.04)
- `popups { opacity; geometry-corner-radius; background-effect {} }` (since 26.04)
