# Recent Windows (Alt-Tab)

> Complete reference for the `recent-windows {}` section. Since 25.11.

```kdl
recent-windows {
    off
    debounce-ms 750
    open-delay-ms 150

    highlight {
        active-color "#999999ff"
        urgent-color "#ff9999ff"
        padding 30
        corner-radius 0
    }

    previews {
        max-height 480
        max-scale 0.5
    }

    binds {
        Alt+Tab         { next-window; }
        Alt+Shift+Tab   { previous-window; }
        Alt+grave       { next-window     filter="app-id"; }
        Alt+Shift+grave { previous-window filter="app-id"; }
        Mod+Tab         { next-window; }
        Mod+Shift+Tab   { previous-window; }
        Mod+grave       { next-window     filter="app-id"; }
        Mod+Shift+grave { previous-window filter="app-id"; }
    }
}
```

- `off` — disable the Alt-Tab switcher
- `debounce-ms` — delay before a focused window is committed to the recent list (prevents intermediate windows from polluting it)
- `open-delay-ms` — delay before visual appearance (quick taps don't show UI)

Bind actions: `next-window`, `previous-window`. Properties: `filter="app-id"`, `scope="all"|"output"|"workspace"`.

Hardcoded keys inside switcher: Escape (cancel), Enter (confirm), A/W/O (scope select), S (cycle scope).
