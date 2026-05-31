# Key Bindings Configuration

> Complete reference for the `binds {}` section.

## Syntax

```kdl
binds {
    // Simple bind
    Mod+T { spawn "alacritty"; }

    // Bind with properties
    Mod+T repeat=false cooldown-ms=500 { spawn "alacritty"; }

    // Multiple actions possible, but typically one
    Mod+Q { close-window; }
}
```

**Modifiers:** `Ctrl`/`Control`, `Shift`, `Alt`, `Super`/`Win`, `ISO_Level3_Shift`/`Mod5` (AltGr), `Mod` (Super on TTY, Alt in nested window).

**Note:** `binds {}` does NOT auto-fill with defaults — must copy from default config.

## Scroll Bindings

```kdl
binds {
    // Mouse wheel (respects natural-scroll)
    Mod+WheelScrollDown cooldown-ms=150 { focus-workspace-down; }
    Mod+WheelScrollUp   cooldown-ms=150 { focus-workspace-up; }
    Mod+WheelScrollRight                { focus-column-right; }
    Mod+WheelScrollLeft                 { focus-column-left; }

    // Touchpad scroll (respects natural-scroll, so these are "inverted")
    Mod+TouchpadScrollDown { spawn "wpctl" "set-volume" "@DEFAULT_AUDIO_SINK@" "0.02+"; }
    Mod+TouchpadScrollUp   { spawn "wpctl" "set-volume" "@DEFAULT_AUDIO_SINK@" "0.02-"; }
}
```

## Mouse Click Bindings

Since 25.01.

```kdl
binds {
    Mod+MouseLeft    { close-window; }
    Mod+MouseRight   { close-window; }
    Mod+MouseMiddle  { close-window; }
    Mod+MouseForward { close-window; }
    Mod+MouseBack    { close-window; }
}
```

⚠️ `Mod+MouseLeft/Right` overrides the corresponding gesture (move/resize window).

## Custom Hotkey Overlay Titles

Since 25.02.

```kdl
binds {
    Mod+Shift+S hotkey-overlay-title="Toggle Dark/Light" { spawn "script.sh"; }
    Mod+Q hotkey-overlay-title=null { close-window; }  // hide from overlay
}
```

Supports Pango markup.

## Complete Actions Reference

Run `niri msg action` inside niri for a complete list. Key actions:

### Navigation
- `focus-column-left`, `focus-column-right`
- `focus-window-up`, `focus-window-down`
- `focus-column-first`, `focus-column-last`
- `focus-monitor-left/right/up/down`, `focus-monitor-next/previous`
- `focus-workspace-down`, `focus-workspace-up`
- `focus-workspace {reference: Index(n) | Name("name")}`
- `focus-workspace-previous` (auto-back-and-forth)
- `switch-focus-between-floating-and-tiling`

### Movement
- `move-column-left`, `move-column-right`
- `move-window-up`, `move-window-down`
- `move-column-to-workspace-down`, `move-column-to-workspace-up`
- `move-column-to-monitor-left/right/down/up/next/previous`
- `move-workspace-down`, `move-workspace-up`
- `move-workspace-to-monitor-left/right/down/up/next/previous`
- `consume-or-expel-window-left`, `consume-or-expel-window-right`
- `consume-window-into-column`, `expel-window-from-column`

### Sizing
- `maximize-column`, `maximize-window-to-edges`, `fullscreen-window`
- `toggle-windowed-fullscreen`
- `set-column-width "50%"`, `set-column-width "1280"`
- `set-window-height "50%"`, `set-window-height "720"`
- `switch-preset-column-width`, `switch-preset-column-width-back`
- `switch-preset-window-height`, `switch-preset-window-height-back`
- `resize-column-left/right`, `resize-column-increase/decrease`
- `resize-window-height-increase/decrease`
- `reset-window-height`
- `switch-preset-column-width-to 2` (index-based, since 25.11)
- `switch-preset-window-height-to 1` (since 25.11)

### Windows
- `close-window`
- `toggle-window-floating`
- `toggle-column-tabbed-display`
- `toggle-window-rule-opacity`
- `set-workspace-name "name"`, `unset-workspace-name`

### Workspace Management
- `toggle-overview`
- `toggle-workspace-last`

### Misc
- `spawn "binary" "arg1" "arg2"` (no shell)
- `spawn-sh "shell command"` (since 25.08, uses `sh`)
- `quit` (with `skip-confirmation=true` to bypass dialog)
- `screenshot` (interactive UI), `screenshot-screen`, `screenshot-window`
- `do-screen-transition` (freeze + crossfade, since 0.1.6)
- `toggle-keyboard-shortcuts-inhibit` (escape hatch, since 25.02)
