# Miscellaneous Configuration

> Top-level config options not covered by other sections.

```kdl
spawn-at-startup "waybar"
spawn-at-startup "alacritty"
spawn-sh-at-startup "qs -c ~/source/qs/MyAwesomeShell"   // since 25.08

prefer-no-csd

screenshot-path "~/Pictures/Screenshots/Screenshot from %Y-%m-%d %H-%M-%S.png"

environment {
    QT_QPA_PLATFORM "wayland"
    DISPLAY null
}

cursor {
    xcursor-theme "breeze_cursors"
    xcursor-size 48
    hide-when-typing         // since 0.1.10
    hide-after-inactive-ms 1000  // since 0.1.10
}

overview {
    zoom 0.5
    backdrop-color "#262626"
    workspace-shadow { softness 40; spread 10; offset x=0 y=10; color "#00000050"; }
}

xwayland-satellite {
    off
    path "xwayland-satellite"
}

clipboard {
    disable-primary
}

hotkey-overlay {
    skip-at-startup
    hide-not-bound       // since 25.08
}

config-notification {
    disable-failed       // since 25.08
}

blur {
    off
    passes 3
    offset 3.0
    noise 0.02
    saturation 1.5
}
```

## spawn-at-startup

Runs programs at niri startup. Same semantics as `spawn` bind (no shell). See also systemd autostart (xdg-desktop-autostart.target).

## prefer-no-csd

Ask windows to omit client-side decorations. Since 25.08 also sets tiled state on windows. Restart apps after changing. Prevents edge resize handles (use Mod+right-click instead).

## screenshot-path

`strftime(3)` format. `null` to disable saving to disk. `~` expanded to home.

## environment

Set env vars for niri-spawned processes. `null` value removes variable. Does NOT propagate to systemd-started apps.

## cursor

- `xcursor-theme`, `xcursor-size` — set cursor theme and set corresponding env vars
- `hide-when-typing` — auto-hide cursor on key press
- `hide-after-inactive-ms` — auto-hide after inactivity period

## overview

Zoom level (0–0.75), backdrop color, workspace shadow settings.

## xwayland-satellite

Since 25.08. Auto-spawn xwayland-satellite on X11 client connection. `off` disables. `path` overrides binary location.

## clipboard

`disable-primary` — disable middle-click/primary clipboard.

## hotkey-overlay

`skip-at-startup` — don't show the hotkey dialog at startup. `hide-not-bound` — hide actions without bound keys.

## config-notification

`disable-failed` — suppress the "config parse error" notification.

## blur

Global blur settings for all background effects (since 26.04).

| Option | Description | Default |
|--------|-------------|---------|
| `off` | Disable all blur | — |
| `passes` | Dual kawase blur passes (higher = smoother, more GPU) | 3 |
| `offset` | Pixel offset multiplier (higher = smoother, no extra GPU cost) | 3.0 |
| `noise` | Random noise to reduce color banding | 0.02 |
| `saturation` | Color saturation (1 = normal, >1 = more, <1 = less) | 1.5 |
