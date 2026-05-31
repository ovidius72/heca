# niri Wiki — Reference Documentation

> This directory contains distilled knowledge from the [niri](https://github.com/niri-wm/niri) Wayland compositor's wiki, organized for heca development reference. niri is a scrollable-tiling Wayland compositor written in Rust by [YaLTeR](https://github.com/YaLTeR).

## Organization

```
docs/niri-wiki/
├── README.md                          ← This file
├── 01-getting-started/
│   ├── overview.md                    ← What is niri, quick start, building
│   ├── example-systemd-setup.md       ← systemd integration examples
│   ├── important-software.md           ← Required companion software
│   └── xwayland.md                    ← Running X11 apps
├── 02-configuration/
│   ├── introduction.md                ← Config file syntax, loading, live-reload
│   ├── input.md                       ← Keyboard, touchpad, mouse, tablets
│   ├── outputs.md                     ← Monitor configuration
│   ├── key-bindings.md               ← Bind syntax, scroll binds, actions
│   ├── layout.md                     ← Gaps, columns, borders, shadows, tabs
│   ├── window-rules.md               ← Matching windows, per-window overrides
│   ├── layer-rules.md                ← Layer-shell surface rules
│   ├── animations.md                 ← Spring and easing animations
│   ├── gestures.md                   ← Drag-and-drop, hot corners
│   ├── named-workspaces.md           ← Persistent workspace declarations
│   ├── recent-windows.md             ← Alt-Tab switcher
│   ├── miscellaneous.md              ← spawn-at-startup, cursor, overview, blur
│   ├── switch-events.md              ← Lid close/open, tablet mode
│   ├── debug.md                      ← Experimental/debug options
│   └── include.md                    ← Multi-file config includes
├── 03-usage/
│   ├── workspaces.md                 ← Dynamic workspaces, workflow
│   ├── floating-windows.md           ← Floating layout, toggle, positioning
│   ├── tabs.md                       ← Column display mode
│   ├── overview.md                   ← Expose mode, drag-and-drop
│   ├── fullscreen-and-maximize.md    ← Sizing modes comparison
│   ├── gestures.md                   ← Touchpad, mouse, DnD gestures
│   ├── window-effects.md             ← Blur, xray, background effects
│   ├── screencasting.md              ← Portal/pipewire, block-out, dynamic cast
│   ├── ipc.md                        ← niri msg, event stream, socket protocol
│   ├── application-issues.md         ← Electron, VSCode, WezTerm, GTK4
│   ├── accessibility.md              ← Orca screen reader support
│   └── security-model.md             ← Trust model, sandboxing, lock screen
├── 04-development/
│   ├── design-principles.md          ← niri's design philosophy
│   ├── fractional-layout.md          ← Physical pixel alignment
│   ├── animation-timing.md           ← LazyClock, adjustable timing
│   ├── redraw-loop.md                ← RedrawState machine, VBlank
│   ├── developing.md                 ← Build, debug, test
│   └── releasing.md                  ← Versioning, packaging
└── architecture.md                   ← Full architecture reference (this file)
```

## Source

This documentation is derived from the [niri wiki](https://github.com/niri-wm/niri/tree/main/docs/wiki) at commit HEAD, retrieved 2026-05-31.

## Key Facts

| Attribute | Detail |
|-----------|--------|
| Language | Rust |
| Type | Wayland compositor (not an app) |
| Layout | Scrollable-tiling (horizontal columns, vertical workspaces) |
| Config format | KDL |
| IPC | JSON-RPC over Unix socket |
| Rendering | OpenGL ES / Vulkan via Smithay + drm |
| Input | libinput + libxkbcommon |
| License | CC0-1.0 |
| Started | ~2022 |
