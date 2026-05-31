# Layout Configuration

> Complete reference for the `layout {}` section.

## All Options

```kdl
layout {
    gaps 16
    center-focused-column "never"
    always-center-single-column
    empty-workspace-above-first
    default-column-display "tabbed"
    background-color "#003300"

    preset-column-widths {
        proportion 0.33333
        proportion 0.5
        proportion 0.66667
    }
    default-column-width { proportion 0.5; }
    preset-window-heights {
        proportion 0.33333
        proportion 0.5
        proportion 0.66667
    }

    focus-ring { ... }
    border { ... }
    shadow { ... }
    tab-indicator { ... }
    insert-hint { ... }
    struts { left 64; right 64; top 64; bottom 64; }
}
```

## Per-output and Per-workspace Overrides

Since 25.11. Layout settings can be overridden per output and per named workspace:

```kdl
output "HDMI-A-1" {
    layout { gaps 8; }
}
workspace "chat" {
    layout { gaps 32; }
}
```

## Gaps

```kdl
layout { gaps 16; }
```

Pixel gap around and between windows. Fractional values rounded to physical pixels according to scale. Can emulate "inner vs. outer" gaps with negative struts: `struts { left -16; right -16; top -16; bottom -16; }` with `gaps 16`.

## Column Width Options

- `center-focused-column`: "never" (default), "always", or "on-overflow"
- `always-center-single-column`: center single column regardless
- `preset-column-widths`: cycle with Mod+R (default: 1/3, 1/2, 2/3)
- `default-column-width`: initial width for new windows. `{}` = let window decide.

## Focus Ring

Drawn only around the active window on each monitor. Behind windows by default (shows through transparent windows).

```kdl
layout {
    focus-ring {
        // off    ← disable
        on
        width 4
        active-color "#7fc8ff"
        inactive-color "#505050"
        urgent-color "#9b0000"
        active-gradient from="#80c8ff" to="#bbddff" angle=45
        inactive-gradient from="#505050" to="#808080" angle=45 relative-to="workspace-view"
        urgent-gradient from="#800" to="#a33" angle=45 in="srgb-linear"
    }
}
```

## Border

Drawn around ALL windows. Windows shrink to make space for it.

Same options as focus ring. Difference: focus ring = active only; border = all windows, affects sizing.

## Shadow

Since 25.02.

```kdl
layout {
    shadow {
        on
        softness 30         // blur radius (logical pixels)
        spread 5            // expansion distance
        offset x=0 y=5      // shadow offset
        draw-behind-window true
        color "#00000070"
        inactive-color "#00000054"   // optional
    }
}
```

Shadows follow `geometry-corner-radius` window rule. `draw-behind-window` fixes shadow artifacts inside CSD rounded corners.

## Tab Indicator

Since 25.02.

```kdl
layout {
    tab-indicator {
        off                 // hide indicator
        on
        hide-when-single-tab
        place-within-column  // inside column (affects sizing)
        gap 5               // distance to window
        width 4             // thickness
        length total-proportion=0.5  // relative to window height
        position "right"    // left, right, top, bottom
        gaps-between-tabs 2
        corner-radius 8
        active-color "red"
        inactive-color "gray"
        urgent-color "blue"
    }
}
```

## Struts

Shrink the area occupied by windows, similar to outer gaps:

```kdl
layout {
    struts {
        left 64
        right 64
        top 64
        bottom 64
    }
}
```

Left/right struts make the next window peek out. Top/bottom add outer gaps on top of layer-shell panels. Negative values push windows outward.

## Insert Hint

Window insert position preview during interactive move:

```kdl
layout {
    insert-hint {
        off
        color "#ffc87f80"
        gradient from="#ffbb6680" to="#ffc88080" angle=45
    }
}
```

## Background Color

Since 25.05. Default workspace background:

```kdl
layout {
    background-color "#003300"
}
```

## Empty Workspace Above First

Since 25.01. Always add empty workspace at the top in addition to bottom:

```kdl
layout {
    empty-workspace-above-first
}
```
