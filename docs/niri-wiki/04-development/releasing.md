# Releasing niri

> Versioning, packaging, CHANGELOG.

niri uses calendar versioning: `YY.MM` (e.g., 25.02, 25.05). Patch releases as `YY.MM.P`.

## Version scheme

- 0.1.x — initial development series
- 25.x → calendar versioning (2025 onwards)

## Packaging

See [Packaging niri](https://github.com/niri-wm/niri/blob/main/docs/wiki/Packaging-niri.md) for distribution-specific guidance. Install files:

| File | Destination |
|------|-------------|
| `target/release/niri` | `/usr/bin/` |
| `resources/niri-session` | `/usr/bin/` |
| `resources/niri.desktop` | `/usr/share/wayland-sessions/` |
| `resources/niri-portals.conf` | `/usr/share/xdg-desktop-portal/` |
| `resources/niri.service` | `/etc/systemd/user/` |
| `resources/niri-shutdown.target` | `/etc/systemd/user/` |
