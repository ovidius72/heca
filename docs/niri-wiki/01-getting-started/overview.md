# Getting Started

> Quick start, installation, building niri from source.

## What is niri?

niri is a scrollable-tiling Wayland compositor. Unlike traditional tiling WMs that use BSP trees or fixed grids, niri arranges windows in horizontally scrolling columns. Workspaces are arranged vertically and can be switched with smooth animations.

## Quick Start

```bash
# Fedora COPR
sudo dnf copr enable yalter/niri
sudo dnf install niri

# Arch Linux
sudo pacman -Syu niri xwayland-satellite xdg-desktop-portal-gnome xdg-desktop-portal-gtk alacritty

# Ubuntu 25.10+
sudo add-apt-repository ppa:avengemedia/danklinux
sudo add-apt-repository ppa:avengemedia/dms
sudo apt install niri dms

# NixOS
# See niri-flake: https://github.com/sodiboo/niri-flake
```

After installing, log out and choose "Niri" in your display manager, or run `niri-session` on a TTY.

## Building from Source

```bash
# Install dependencies (see README for full list)
# Ubuntu
sudo apt-get install gcc clang libudev-dev libgbm-dev libxkbcommon-dev \
  libegl1-mesa-dev libwayland-dev libinput-dev libdbus-1-dev \
  libsystemd-dev libseat-dev libpipewire-0.3-dev libpango1.0-dev \
  libdisplay-info-dev

# Fedora
sudo dnf install gcc libudev-devel libgbm-devel libxkbcommon-devel \
  wayland-devel libinput-devel dbus-devel systemd-devel libseat-devel \
  pipewire-devel pango-devel cairo-gobject-devel clang libdisplay-info-devel

# Build
cargo build --release

# (Do NOT build with --all-features)
```

## Default Keybindings

| Key | Action |
|-----|--------|
| Mod+T | Spawn terminal (alacritty) |
| Mod+D | Spawn launcher (fuzzel) |
| Mod+Q | Close focused window |
| Mod+H/L or ←/→ | Focus column left/right |
| Mod+J/K or ↓/↑ | Focus window down/up in column |
| Mod+U/I or PageDown/Up | Switch workspace down/up |
| Mod+Ctrl+H/L | Move column left/right |
| Mod+Shift+F | Toggle fullscreen |
| Mod+F | Maximize column |
| Mod+M | Maximize window to edges |
| Mod+W | Toggle column tabbed display |
| Mod+V | Toggle window floating |
| Mod+R | Cycle preset column widths |
| Mod+Shift+R | Cycle backward |
| Mod+ - / = | Decrease/increase column width |
| Mod+Shift+E | Exit niri |
| PrtSc | Interactive screenshot |
