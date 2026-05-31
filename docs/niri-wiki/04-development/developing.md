# Developing niri

> Build instructions, debugging, testing.

## Build

```bash
cargo build --release
# Feature flags (see Cargo.toml)
# Default: systemd, dbus, xdp-gnome-screencast
# Alternative: --no-default-features --features dinit,dbus,xdp-gnome-screencast
```

⚠️ Do NOT build with `--all-features`.

## Windowed Mode

Run `niri` inside an existing desktop session for testing. Some limitations:
- Mod key becomes Alt (to avoid host conflicts)
- Hotkeys sometimes buggy
- No real Wayland socket

## Testing

Run `cargo test`. niri uses a `LazyClock` design that allows tests to advance time deterministically.

## Debug Features

See debug options. Key debug binds during development:
- `Mod+Shift+Ctrl+T` — toggle debug tint (green = direct scanout)
- `Mod+Shift+Ctrl+O` — toggle opaque region visualization
- `Mod+Shift+Ctrl+D` — toggle damage visualization
