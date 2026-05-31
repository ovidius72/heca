# Accessibility

> Screen reader (Orca) support. Since 25.08.

## Supported

- Orca screen reader via `org.freedesktop.a11y.KeyboardMonitor` D-Bus
- AccessKit UI element exposure:
  - Workspace switching announcements ("Workspace 2")
  - Exit confirmation dialog
  - Alt-Tab window switcher (since 25.11) — announces selected window title
  - Screenshot UI and overview entry
  - Config parse errors
  - Important hotkeys list (one announcement, no tab navigation)

## Requirements

- Run niri as session (display manager or niri-session)
- Working Xwayland (for Orca)
- Working EGL/GPU acceleration
- Bind: `Super+Alt+S` toggles Orca (default config)

## Distribution Recommendations

- Change default terminal to GNOME Console or GNOME Terminal (screen-reader-friendly)
- Change launcher to xfce4-appfinder
- Add startup sound to indicate niri loaded
- Add `spawn-at-startup "orca"`

## Limitations

- No bind to focus layer-shell panels
- Requires a connected and enabled screen
- No screen curtain functionality
