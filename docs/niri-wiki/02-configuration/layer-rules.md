# Layer Rules

> Complete reference for `layer-rule {}` blocks. Since 25.01.

```kdl
layer-rule {
    match namespace="waybar"
    match at-startup=true
    match layer="top"

    opacity 0.5
    block-out-from "screencast"

    shadow { on; ... }
    geometry-corner-radius 12
    place-within-backdrop true
    baba-is-float true

    background-effect { xray true; blur true; noise 0.05; saturation 3; }
    popups { opacity 0.5; geometry-corner-radius 6; background-effect { ... }; }
}
```

## Matchers

- `namespace="regex"` — match on surface namespace. Find via `niri msg layers`.
- `at-startup=true/false` — first 60 seconds
- `layer="background" | "bottom" | "top" | "overlay"` (since 26.04)

## Properties

Same as window rules: `opacity`, `block-out-from`, `shadow`, `geometry-corner-radius`, `background-effect`, `popups`.

### place-within-backdrop

Since 25.05. Places the background surface into the overview backdrop. Only works for background layer surfaces with no exclusive zone.

### Shadow Note

Layer shadows must be enabled with a layer rule — the global `layout { shadow {} }` does not apply to them.
