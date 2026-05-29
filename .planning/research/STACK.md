# Research: Stack

## 2025-06-29

### Windowing & GPU

| Layer | Choice | Confidence | Rationale |
|-------|--------|------------|-----------|
| Windowing | `winit` 0.30+ | **High** | De-facto Rust standard. Cross-platform, handles HiDPI, monitor enumeration, raw keyboard scan codes. |
| GPU API | `wgpu` 0.25+ | **High** | Cross-platform (Vulkan/Metal/DX12/WebGPU), safe Rust, async, excellent ecosystem. |
| Text/Fonts | `cosmic-text` 0.14+ | **High** | Best pure-Rust text stack. Shaping, atlas caching, ligatures, variable fonts. Used by System76 Cosmic. |
| 2D Primitives | Custom `wgpu` | **Medium** | Rounded rects, borders, shadows. Could later integrate `vello` for vector-quality chrome. |

### Terminal

| Layer | Choice | Confidence | Rationale |
|-------|--------|------------|-----------|
| VTE / Grid | `alacritty_terminal` 0.25+ | **High** | Available as a Rust library crate. Battle-tested. You feed PTY bytes, get a cell grid. Kitty/Ghostty are applications, not libraries. |
| PTY Spawning | `portable-pty` 0.9+ | **High** | Cross-platform PTY creation. Works on Linux/macOS/Windows. |

### Neovim Integration

| Layer | Choice | Confidence | Rationale |
|-------|--------|------------|-----------|
| Async Runtime | `tokio` 1.4+ | **High** | All IO flows through this. Neovim socket, PTYs, RPC server. |
| MsgPack | `rmpv` + `tokio-util` codec | **High** | Stable, proven. Neovim speaks msgpack-RPC over stdio or socket. |

### Browser Plugin (v2)

| Approach | Viability | Complexity | Notes |
|----------|-----------|------------|-------|
| CEF offscreen (`cef-rs` / `wef`) | **High** | Very High | Renders Chromium to a shared texture. Cross-platform but ~100MB+ binaries. DevTools supported. |
| Platform WebView (`wry`) | **Medium** | High | Creates own window per webview. Hard to composite into our GPU surface without reparenting. |
| OS Window Reparenting | **Medium** | Very High | X11 `XReparentWindow`, Windows `SetParent`, macOS `NSView` addSubview. Platform-specific, fragile on Wayland. |
| **Recommendation** | CEF offscreen for the browser plugin specifically. OS window embedding as a general plugin protocol for arbitrary GUI apps. |

### Config & Data

| Layer | Choice | Confidence | Rationale |
|-------|--------|------------|-----------|
| Config Format | TOML | **High** | Human-readable, typed, standard in Rust ecosystem. |
| Config Paths | `dirs` / `etcetera` | **High** | Cross-platform user config directories. |
| Serialization | `serde` + `toml` + `bincode` (future) | **High** | Standard. `bincode` for fast session snapshots if needed. |

### IPC / RPC

| Layer | Choice | Confidence | Rationale |
|-------|--------|------------|-----------|
| Transport | Unix domain socket (named pipes on Windows) | **High** | Fast, local, standard. |
| Protocol | JSON-RPC 2.0 | **High** | Simple, debuggable, CLI-friendly. Herdr uses length-prefixed JSON over sockets. |

### Plugin Loading (v1)

| Layer | Choice | Confidence | Rationale |
|-------|--------|------------|-----------|
| In-process | `libloading` 0.8+ | **High** | Dynamic `.so`/`.dll` loading at runtime. Zero IPC overhead. |

### What NOT to Use

| Option | Why Not |
|--------|---------|
| Dioxus / Tauri / Electron | WebView-based. Cannot own the GPU render loop or composite external pane textures freely. |
| GTK / Qt | Fight you for custom GPU-rendered surfaces. Heavy cross-platform packaging. |
| `egui` | Immediate-mode; struggles with complex text layout and desktop-app feel. |
| `iced` | Retains too much layout control; we need to own pane rectangle assignment. |
| Bevy | Game engine ECS fights traditional GUI event loops. |
| `skia-safe` | Proven (Neovide uses it) but requires C++ toolchain and is heavier. `cosmic-text` + `wgpu` is the modern Rust-native path. |

### Versions (Verified 2025-06)

- `winit`: 0.30.x (latest stable)
- `wgpu`: 0.25.x
- `cosmic-text`: 0.14.x
- `alacritty_terminal`: 0.25.x
- `portable-pty`: 0.9.x
- `tokio`: 1.40+
- `rmpv`: 1.3+
- `libloading`: 0.8+
