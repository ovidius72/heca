# Application-Specific Issues

> Common workarounds for specific applications on niri.

## Electron Apps

```kdl
// Electron ≥ 39: --ozone-platform=wayland CLI flag
// Electron < 39:
environment { ELECTRON_OZONE_PLATFORM_HINT "auto"; }
// Electron ≤ 28: --enable-features=UseOzonePlatform --ozone-platform-hint=auto
```

## VSCode

May need running Xwayland + `DISPLAY=:0` even with Wayland backend (queries X server for keymap).

## JetBrains IDEs

Add `-Dawt.toolkit.name=WLToolkit` in Help → Edit Custom VM Options.

## WezTerm

Two bugs:
1. Waits for zero-sized configure event → add window rule: `default-column-width {}`
2. Wrong size with `prefer-no-csd` → comment out `prefer-no-csd` and restart WezTerm

## Ghidra (Java)

Under xwayland-satellite: `_JAVA_AWT_WM_NONREPARENTING=1` env var.

## Zen Browser

Set `widget.dmabuf.force-enabled=true` in `about:config` for screencasting.

## GTK 4 Dead Keys / Compose

GTK 4.20 stopped handling dead keys. Run IBus/Fcitx5 or set:
```kdl
environment { GTK_IM_MODULE "simple"; }
```

## Fullscreen Games

Use gamescope:
```sh
gamescope -f -w 1920 -h 1080 -W 1920 -H 1080 --force-grab-cursor --backend sdl -- <game>
```

## Steam

Black window → Settings → Interface → disable GPU accelerated rendering in web views. Or use `-system-composer` flag.

## Waybar (GTK 3)

Rounded corners with black pixels → set opacity to 0.99.

## Firefox PIP

Built-in window rule floats the PIP window.

## Spotify Cage Issue

Run in Cage: `cage -- flatpak run com.spotify.Client`
