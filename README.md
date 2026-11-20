# heca

> Heca takes its name from the Hecatoncheires — hundred‑handed giants of myth — because it lets you juggle terminals, editors, and tools in a single GPU‑accelerated window.
> Think prefix keys, smooth animations, and zero ambrosia required.

A GPU-native terminal workspace compositor for developers. Inspired by [NIRI](https://github.com/YaLTeR/niri)'s scrollable-tiling model, powered by [wgpu](https://wgpu.rs/) and [cosmic-text](https://github.com/pop-os/cosmic-text).

Think tmux meets NIRI meets Neovide — all panes render in a single GPU-accelerated window with smooth animations, sharp fonts, and zero window seams.

![Status](https://img.shields.io/badge/status-alpha-orange)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-blue)
![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green)

---

## What it is

heca is a **workspace compositor** — not a traditional terminal emulator or window manager. It hosts
terminals (and, later, other tools) inside one native window, laid out the way NIRI lays out windows.

- **Scrolling columns.** Panes sit in horizontal columns that scroll smoothly; each column keeps its own width, and opening a pane never resizes the others.
- **Vertical stacks and workspaces.** Panes stack inside a column; workspaces stack vertically and switch with an animation.
- **Floating panes.** Any pane can leave the layout and float.
- **Prefix keys.** Every command is `Ctrl+B`, then a key, like tmux, so nothing clashes with the program in the pane. Every key can be changed.
- **GPU-native chrome.** Tab bar, sidebars, status bar and pane borders are drawn in the same GPU pass as the terminals.
- **Sessions.** heca runs as a background server. Close the window and your panes keep running; run `heca` again to come back to them.

[What heca is](docs/overview.md) has the longer version, including how it differs from tmux.

## Requirements

- **Rust** 1.100 or newer. The repository pins the exact version in [`rust-toolchain.toml`](rust-toolchain.toml), so [rustup](https://rustup.rs/) installs it for you.
- **GPU** with Vulkan (Linux), Metal (macOS), or DX12 (Windows) support.
- **OS**: Linux, macOS, or Windows.

## Download

Every release is on the [GitHub Releases page](https://github.com/ovidius72/heca/releases): an
installer and an archive for each system — `.dmg` on macOS (Apple Silicon and Intel), `.deb` and
`.AppImage` on Linux, `.msi` on Windows. The builds are not signed yet; the release page says how to
open them. Or build heca from source, as below.

## Build from source

```bash
git clone https://github.com/ovidius72/heca.git
cd heca
cargo build --release
```

The program is `target/release/heca` (`target\release\heca.exe` on Windows). Run it from there,
or copy it somewhere on your `PATH`.

On Linux, install the system packages first (Debian/Ubuntu names):

```bash
sudo apt-get install pkg-config libasound2-dev
```

## Start developing

```bash
cargo run -p heca                    # run heca from the source tree
cargo run -p heca -- --standalone    # a session of its own that ends with the window,
                                     # without attaching to a heca already running
cargo run -p heca --example extend_heca            # heca with the extension example
cargo run -p heca-renderer --example showcase      # the gallery of UI widgets
```

Before a pull request, run what CI runs on Linux, macOS and Windows:

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Day to day, `./scripts/lint-changed.sh` (and `./scripts/lint-changed.sh test`) checks only the
crates you changed, which is much faster.

[Developing heca](docs/development.md) has the details. Contributors, human or agent, read
[`AGENTS.md`](AGENTS.md) first: it holds the project's rules.

## First run

On first launch, heca creates a default config at:

- **Linux/macOS**: `~/.config/heca/config.toml`
- **Windows**: `%APPDATA%\heca\config.toml`

The window opens with a single pane. All WM commands use a **prefix key** (`Ctrl+B` by default), like tmux:

```
Ctrl+B → h   Focus left column
Ctrl+B → l   Focus right column
Ctrl+B → Enter  New column (horizontal split)
Ctrl+B → v   New pane in column (vertical split)
Ctrl+B → x   Close active pane
Ctrl+B → f   Toggle floating
```

Close the window and heca keeps running in the background; run `heca` again to reattach.

## Documentation

All documentation is in [`docs/`](docs/README.md). Start here:

- [Keyboard commands](docs/keyboard-commands.md) — every default key.
- [Configuration](docs/configuration.md) — `config.toml`: theme, fonts, appearance, notifications.
- [Keybindings](docs/keybindings.md) — rebind a key, add your own, write a mode.
- [Sessions](docs/sessions.md) — detach, reattach, follow another window.
- [Architecture](docs/architecture.md) — the stack and how the crates fit together.
- [Developing heca](docs/development.md) — running, testing and the examples.
- [Building on heca](docs/extending-heca.md) and [writing a plugin](docs/plugins.md).

What is planned and in progress lives in the project planner (`.planner/`, loaded with `/planner load`).

## License

heca is free and open source: anyone can use, copy, modify and share it. It is licensed under
either of

- the [MIT License](LICENSE-MIT), or
- the [Apache License, Version 2.0](LICENSE-APACHE),

at your option.

## Acknowledgments

- **[NIRI](https://github.com/YaLTeR/niri)** by Ivan Molodetskikh — The scrollable-tiling compositor that inspired heca's layout engine.
- **[cosmic-text](https://github.com/pop-os/cosmic-text)** by System76 — GPU text rendering with excellent font shaping.
- **[wgpu](https://wgpu.rs/)** — Safe, portable GPU API for Rust.
