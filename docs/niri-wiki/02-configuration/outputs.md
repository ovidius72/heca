# Outputs Configuration

> Complete reference for `output "name" {}` sections.

```kdl
output "eDP-1" {
    off
    mode "1920x1080@120.030"
    scale 2.0
    transform "90"
    position x=1280 y=0
    variable-refresh-rate
    focus-at-startup
    max-bpc 8

    hot-corners {
        off
        top-left
        top-right
        bottom-left
        bottom-right
    }

    layout {
        // Per-output layout overrides (since 25.11)
    }

    // Custom modes — CAUTION: may damage your display
    mode custom=true "1920x1080@100"
    modeline 173.00 1920 2048 2248 2576 1080 1083 1088 1120 "-hsync" "+vsync"
}
```

## Matching

Outputs matched by connector name (eDP-1, HDMI-A-1), or by manufacturer + model + serial (since 0.1.9). Case-insensitive (since 0.1.6). Find names via `niri msg outputs`.

## Properties

- `off` — disable output
- `mode "WxH@R"` — resolution and refresh rate. Run `niri msg outputs` to find exact rate.
- `mode custom=true "WxH@R"` — custom mode (⚠️ may damage display, since 25.11)
- `modeline ...` — raw modeline (⚠️ may damage display, since 25.11)
- `scale 2.0` — fractional scale supported (since 0.1.7). Guesses from physical size if unset (since 0.1.6).
- `transform "90"` — `"normal"`, `"90"`, `"180"`, `"270"`, `"flipped"`, `"flipped-90"`, `"flipped-180"`, `"flipped-270"`
- `position x=1280 y=0` — in logical (scaled) pixels. Automatic positioning if unset/overlap.
- `variable-refresh-rate` — enable VRR/FreeSync/G-Sync. Optional `on-demand=true` (since 0.1.9) — only when matching window rule requests it.
- `focus-at-startup` — focus this output by default (since 25.05)
- `max-bpc 8` — max bits per channel (since next)
- `background-color "#003300"` — deprecated since 25.11, use `layout { background-color; }`
- `backdrop-color "#001100"` — between-workspace/overview color (since 25.05)

## Hot Corners

Since 25.11. Per-output override of the global `gestures { hot-corners; }` setting:
- `off` — disable hot corners on this output
- Named corners to enable specific ones

## Automatic Positioning

1. Collect all connected monitors and their logical sizes
2. Sort by name (deterministic, independent of connection order)
3. Place outputs with explicit `position` first (skip if overlapping → place to right with warning)
4. Place remaining outputs to the right of everything placed so far
