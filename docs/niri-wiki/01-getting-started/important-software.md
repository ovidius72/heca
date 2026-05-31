# Important Software

> Companion programs required for a normal desktop experience with niri.

| Purpose | Recommended | Notes |
|---------|-------------|-------|
| Notification daemon | mako | `systemctl --user add-wants niri.service mako.service` |
| Application launcher | fuzzel | Bind: `Mod+D { spawn "fuzzel"; }` |
| Status bar | Waybar | Default config spawns it |
| Portal backend | xdg-desktop-portal-gnome | Required for screencasting |
| Portal backend | xdg-desktop-portal-gtk | Default fallback for file pickers |
| Screen locker | swaylock | Bind: `Super+Alt+L { spawn "swaylock"; }` |
| Wallpaper | swaybg | Or awww (swww successor) |
| Polkit agent | plasma-polkit-agent | For root auth dialogs |
| Clipboard | wl-clipboard | `wl-copy`/`wl-paste` |
| Idle management | swayidle | DPMS, lock on idle |
| X11 support | xwayland-satellite | ≥ 0.7 for auto-integration |
| Key info | wev | Find key names for bindings |

## systemd Integration

```bash
# Add services to niri session
systemctl --user add-wants niri.service mako.service
systemctl --user add-wants niri.service waybar.service

# Or use spawn-at-startup in config
# (simpler but less monitorable)
```
