# Screencasting

> Portal/pipewire screencasting, block-out, dynamic cast target.

## Requirements

- Working D-Bus session
- pipewire
- xdg-desktop-portal-gnome
- niri running as session (niri-session or display manager)

## Features

### Block-out Windows

Replace specific windows with black rectangles on screencast:

```kdl
window-rule {
    match app-id=r#"^org\.keepassxc\.KeePassXC$"#
    block-out-from "screencast"   // only portal screencasts
    // block-out-from "screen-capture"  // ALL screen captures
}
```

### Dynamic Cast Target

Since 25.05. Special "niri Dynamic Cast Target" stream that can be switched:
- `set-dynamic-cast-window` — cast focused window
- `set-dynamic-cast-monitor` — cast focused monitor
- `clear-dynamic-cast-target` — back to empty stream

### Screencast Indication

Visual indication of screencasted windows via `is-window-cast-target` window rule.

### Windowed Fullscreen

`toggle-windowed-fullscreen` — tells app it's fullscreen without actually making it fullscreen. Useful for browser presentations.

### Screen Mirroring

Third-party tool: `wl-mirror`. Bind example:
```kdl
binds {
    Mod+P { spawn-sh "wl-mirror $(niri msg --json focused-output | jq -r .name)"; }
}
```
