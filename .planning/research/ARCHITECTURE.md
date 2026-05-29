# Research: Architecture

## 2025-06-29

**Context:** heca is a *native GUI application* — an OS window that acts as a tiling compositor for embeddable applications. It does not run inside a terminal. It owns the GPU context and composites every pixel.

---

## Major Components

### 1. Host (Main Binary)

The host is the only process that owns:
- The `winit` event loop
- The `wgpu` device and surface
- The `cosmic-text` font atlas and text renderer
- The global input router

Everything else is a plugin or a pane runtime managed by the host.

### 2. Compositor / Renderer

Runs every frame:
1. **Layout pass** — Compute pixel rectangles for all Embedded panes from the BSP tree. Collect Floating and Scratchpad rects.
2. **Content update** — Ask each visible pane for its latest content.
   - Text-grid panes (term, nvim): update cell buffers.
   - Texture panes (future browser): update GPU texture.
   - Draw-command panes (plugins): emit primitive batches.
3. **Chrome draw** — Tab bar, pane borders/titles, status bar, command palette.
4. **Composite** — Single `wgpu` render pass:
   - Clear background
   - Draw embedded panes (back-to-front or front-to-back with borders)
   - Draw floating/scratchpad panes (with drop shadows if themed)
   - Draw global overlays (command palette, notifications)
5. **Present** — swapchain present.

### 3. Session / Window / Tab / Pane Tree

```
Session (named, persistable)
└── Window (OS window + winit event loop)
    └── Tab (active workspace)
        ├── Tiling Tree (BSP: HSplit / VSplit / Leaf / Empty)
        ├── Floating Registry (Vec<PaneId> with absolute rects + z-index)
        └── Scratchpad Registry (Vec<ScratchpadEntry>)
```

- **Session**: Environment, CWD, all windows/tabs/panes metadata. Saved to disk as JSON/TOML.
- **Window**: One `winit` window. Can host multiple tabs (only one visible).
- **Tab**: One layout root + float registry + scratchpad registry.
- **Pane**: A leaf containing an `App` implementation.

### 4. Layout Engine

**BSP Tree** (inspired by herdr, i3, and many others):
- Recursive binary splits.
- Each `Split` has `dir: Axis` and `ratios: Vec<f32>`.
- `Leaf` holds a `PaneId`.
- `Empty` is a placeholder.

**Operations:**
- Resize: nudge ratio in parent split.
- Move: detach leaf, graft into new split location.
- Swap: exchange `PaneId` in two leaves.
- Collapse: remove empty splits.

### 5. Input Router

**Keyboard:**
- Global keymap resolves chords against WM bindings first.
- If no match, forward to focused pane.
- WM bindings use a prefix key or dedicated modifier (e.g., `Alt+Space` then `v` to split vertical).

**Mouse:**
- Hit-test against current frame rects.
- Chrome clicks → tab switch, drag tab reorder.
- Border hover/drag → resize cursor, adjust split ratios.
- Float title drag → move float.
- Float edge/corner drag → resize float.
- Pane click → focus pane, forward local coords to pane.
- Scroll wheel → forward to pane (terminal scroll, nvim scroll).

### 6. Pane Runtime (`App` Trait)

Every pane content source implements:
```rust
trait App: Send {
    fn init(&mut self, ctx: &AppContext);
    fn resize(&mut self, size: Size);
    fn input(&mut self, event: InputEvent) -> bool;
    fn focus(&mut self, focused: bool);
    fn title(&self) -> String;
    fn cwd(&self) -> Option<PathBuf>;
    fn render(&mut self, cx: &mut RenderCx);
    fn shutdown(&mut self);
}
```

**Built-in implementations:**
- `TerminalApp`: PTY + `alacritty_terminal::Term` → produces cell grid.
- `NeovimApp`: `tokio` msgpack client → produces cell grid + chrome events.
- `PlaceholderApp`: Empty pane.

**Plugin implementations:**
- In-process (v1): `.so`/`.dll` via `libloading`.
- Out-of-process (v2): socket-based draw-command stream or shared texture.

### 7. RPC Server

- Unix domain socket per session (or one global socket with session namespace).
- JSON-RPC 2.0.
- Namespaced methods: `session.*`, `window.*`, `tab.*`, `pane.*`, `tree.*`, `layout.*`.
- External clients: CLI, scripts, AI agents.

### 8. Session Persistence

**What is saved:**
- Session metadata (name, created date)
- Window geometries
- Tab names
- Layout trees (with placeholder pane references)
- Pane metadata: `app_type`, `cwd`, spawn command, float positions, scratchpad configs

**What is NOT saved:**
- PTY scrollback (too large)
- Neovim buffer content (nvim has `:mksession` for this)
- GPU textures

**Restore flow:**
1. Deserialize session file.
2. Create winit windows with saved geometries.
3. Respawn each pane's app in its saved cwd.
4. Rebuild layout tree, float registry, scratchpad registry.
5. Assign new PaneIds.

---

## Data Flow

```
Neovim Process ──msgpack──► NvimApp ──cell grid──► Compositor
Terminal PTY ──bytes──► TerminalApp ──cell grid──► Compositor
Plugin (.so) ──draw cmds──► PluginApp ──primitives──► Compositor
                                        ▲
                                        │
Input Router ──events──► Focused Pane ──┘
                                        │
RPC Server ──commands──► Session Manager ──mutations──► Layout Engine
```

---

## Build Order

1. **GPU Shell** — winit + wgpu + cosmic-text. Prove we can render text and rects.
2. **Layout Engine** — BSP tree, rectangle computation, chrome rendering.
3. **Terminal** — PTY + alacritty_terminal. First real pane content.
4. **Neovim** — msgpack client + grid state. Most complex built-in pane.
5. **Input Router** — Full keyboard + mouse, keybindings, hit-testing.
6. **Session Persistence** — Save/restore.
7. **RPC Server** — External control.
8. **Plugin SDK** — Dynamic loading.
9. **Browser / Out-of-process** — v2.

---
*Key difference from TUI multiplexers (herdr, tmux, zellij): we do not stream ANSI to a terminal. We composite pixels in a GPU surface.*
