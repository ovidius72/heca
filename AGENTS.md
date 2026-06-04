# heca — Agent Guide

> Everything an AI coding agent needs to work effectively on the heca project.
> Last updated: 2026-06-02

---

## What This Is

**heca** is a native, GPU-accelerated tiling workspace compositor for developers. Think tmux meets NIRI meets Neovide — but rendered in a single GPU window via `wgpu` and `cosmic-text`, not in a terminal.

### Core Value

A keyboard-native workspace where every tool lives in a tiled, floating, or scratchpad pane — all rendered in one GPU-accelerated window, fully restorable across sessions.

### Key Differentiators

| Feature | Why It Matters |
|---------|---------------|
| GPU-native text rendering | Sharp fonts, ligatures, smooth animations. Terminal emulators can't match this. |
| Single-frame compositing | All panes + chrome render in one GPU pass. No window seams between panes. |
| NIRI-inspired scrolling columns | Horizontal scrollable columns (not BSP tree). View offset animates when switching focus. |
| tmux-style prefix bindings | `Ctrl+B → key` avoids conflicts with hosted terminal applications. |
| Cross-platform from day one | Linux, macOS, Windows. All GPU via wgpu. |
| Registry-based action dispatch | Every WM action goes through `ActionRegistry` — traceable, hookable, scriptable. |
| Config reload at runtime | `prefix+Shift+r` reloads keymaps, themes, settings without restart. |

---

## Architecture (NIRI Scrolling Layout)

```
Session                          ← manages all workspaces + overview/expose mode
├── workspaces: Vec<Workspace>   ← arranged VERTICALLY (discrete switching)
│   └── Workspace
│       ├── scrolling: ScrollingSpace   ← horizontal COLUMNS (continuous scroll)
│       │   ├── view_offset: ViewOffset ← animated horizontal scroll + snap
│       │   ├── columns: Vec<Column>
│       │   │   ├── width: ColumnWidth  ← Proportion | Fixed
│       │   │   ├── panes: Vec<Pane>    ← vertical stack within column
│       │   │   └── pane_sizes: Vec<Size>
│       │   └── active_column_idx
│       └── floating_panes: Vec<FloatingPane>
├── overview: OverviewState      ← zoom progress, open/closed
├── workspace_switch: WorkspaceSwitch  ← animated vertical transitions
└── active_workspace_idx
```

### Layout Rules (from NIRI)

1. **Opening a new window does not affect sizes of existing windows.**
2. **The focused window does not move around on its own.**
3. **Column widths are NOT normalized** — each column keeps its own width; columns overflow the viewport (that's the point of scrolling).
4. **Only the active column's width changes on resize.** Other columns are untouched.
5. **Animations are driven by easing functions** (spring physics planned).

### Scrolling Model

| Axis | Container | Scroll Type | Mechanism |
|------|-----------|-------------|-----------|
| **Horizontal** | `ScrollingSpace.columns` | Continuous scroll + snap | `ViewOffset` (animated `f64`) |
| **Vertical** | `Session.workspaces` | Discrete switch + animation | `WorkspaceSwitch` (animated index) |

### Keybinding Style (tmux-style prefix)

```
Ctrl+B → h    Focus column left (animated scroll)
Ctrl+B → l    Focus column right (animated scroll)
Ctrl+B → j    Focus pane down
Ctrl+B → k    Focus pane up
Ctrl+B → Enter Split horizontal (new column to the right)
Ctrl+B → v    Split vertical (new pane in current column)
Ctrl+B → x    Close active pane
Ctrl+B → f    Toggle pane floating
Ctrl+B → q    Quick-select pane (overlay letters, all workspaces)
Ctrl+B → Shift+q  Quick-swap pane (stay at current position)
Ctrl+B → m    Swap and focus (follow to destination)
Ctrl+B → =    Increase column width
Ctrl+B → -    Decrease column width
Ctrl+B → [    Move pane to column left
Ctrl+B → ]    Move pane to column right
Ctrl+B → e    Enter sidebar navigation mode
Ctrl+B → w    Create workspace + pane
Ctrl+B → Shift+w  Rename workspace
Ctrl+B → Shift+p  Rename pane
Ctrl+B → i    Toggle focus (local, same workspace)
Ctrl+B → Shift+l  Toggle focus (global, cross-workspace)
Ctrl+B → b    Toggle left sidebar
Ctrl+B → r    Enter resize mode (sticky)
Ctrl+B → Shift+r  Reload config at runtime
Ctrl+B → p    Command palette (backend ready, UI pending)
```

**Key rules:**

- Prefix mode is intentional (like tmux), NOT a bug. This avoids conflicts with hosted apps.
- The prefix key is **configurable** via `prefix = "ctrl+b"` in config.toml.
- All keybingings should be configurable in config.toml.
- All actions should be registered in the action refistry  and accessible with keybingings and from the RPC
- Actions can be assigned to more keys in config.toml.
- No harcoded keybinging or color, style and theme related data must be defined in the code. They must be configurable in config.toml.
- In Normal mode, all key events are forwarded to the focused backend (terminal/nvim).
- Only the prefix key and explicitly bound keys trigger WM actions.
- **Prefix timeout:** auto-exits Prefix mode after 500ms of inactivity.
- **Pane letter limit:** PaneSelect/Swap modes use a-z, A-Z (52 unique labels). Sessions with >52 panes/columns fall back to sidebar navigation.

---

## Action System Architecture

### Three-Layer Dispatch

```
Keyboard Input → KeyCombo → KeymapRegistry → WmAction → ActionRegistry → Handler
     │                                                            │
     │                                                            ├── handle_focus_pane()
     │                                                            ├── handle_swap_pane()
     │                                                            ├── handle_resize()
     │                                                            └── ...
     │
     └── Physical key normalization
         ├── macOS: Ctrl+letter → control char fallback
         ├── Shift+symbol → unshifted base key
         └── NamedKey mapping (Enter, Tab, ArrowLeft, ...)
```

### ActionRegistry (`heca/src/actions.rs`)

The central dispatch for all WM actions:

```rust
pub struct ActionRegistry {
    handlers: HashMap<Discriminant<WmAction>, ActionHandler>,
}

type ActionHandler = fn(&mut AppState, &WmAction);
```

- **Register** a handler: `registry.register(&WmAction::FocusPane { pane_id: 0 }, handle_focus_pane)`
- **Execute** an action: `registry.execute(&action, state)` — routes to the correct handler by discriminant
- **Parameterized variants** share one handler — the handler destructures the action to get arguments

**Registry bypasses are bugs.** All WM state changes must go through `registry.execute()`. Direct calls like `focus_pane_by_id(state, id)` are only allowed inside handlers (as part of their implementation).

### KeymapRegistry (`heca/src/keymap.rs`)

Mode-specific keymap lookup:

```rust
pub struct KeymapRegistry {
    keymaps: HashMap<String, HashMap<KeyCombo, WmAction>>,
}
```

- **Bind**: `keymap.bind("normal", combo, action)`
- **Resolve**: `keymap.resolve("normal", &combo)` → `Option<&WmAction>`
- **Modes**: `"normal"` (prefix bindings), `"global"` (direct bindings), `"sidebar"` (sidebar nav), custom mode names

### WmAction Enum (`heca/src/input.rs`)

```rust
pub enum WmAction {
    // Unit actions (no arguments)
    FocusLeft, FocusRight, FocusUp, FocusDown,
    SplitHorizontal, SplitVertical,
    ClosePane, Float, PaneSelect, SwapPane, SwapAndFocusPane,
    CreateWorkspace, RenameWorkspace, RenamePane,
    SidebarLeft, SidebarRight, SidebarFocus,
    CommandPalette, ReloadConfig, ...

    // Parameterized actions (arguments)
    FocusPane { pane_id: u64 },
    FocusWorkspace { ws_idx: usize },
    Swap { a_id: u64, b_id: u64 },
    Move { pane_id: u64, target_col: usize },
    Resize { target: ResizeTarget, axis: ResizeAxis, amount: f64 },
    ResizeTo { target: ResizeTarget, width: f64, height: f64 },
    FloatAt { pane_id: u64, x: f64, y: f64, width: f64, height: f64 },
    ClosePaneById { pane_id: u64 },
    RenameTarget { pane_id: u64, name: String },
    SpawnCommand { command: String },
    EnterMode { name: String },
}
```

### Input Modes (`heca/src/app_state.rs`)

```rust
pub enum InputMode {
    Normal,           // Forward keys to terminal, prefix triggers prefix mode
    Prefix,           // Waiting for action key after prefix
    PaneSelect { candidates: Vec<(char, u64)> },  // Quick-select overlay
    PaneSwap { candidates: Vec<(char, u64)>, focus_after: bool },  // Quick-swap overlay
    SidebarNav,       // Sidebar tree navigation
    Rename { target, buffer },  // Text input for renaming
    Chord { sequence },  // Multi-key chord (e.g., w → digit)
    Mode { name },     // Custom mode (resize, etc.)
}
```

**Mode triggers**: Config defines how to enter modes:

```toml
[[keys.mode]]
name = "resize"
trigger = "prefix+r"
sticky = true   # true = stay until Esc/Enter; false = one-shot (chord)
```

### KeyCombo (`heca/src/keymap.rs`)

```rust
pub struct KeyCombo {
    pub key: String,      // Normalized key name (lowercase, "enter", "arrowleft")
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_: bool,
}
```

- **Case-insensitive equality**: `"h"` matches `"H"`
- **Shift inference**: `"{"` parses as `"["` + `shift=true`
- **macOS physical key fallback**: When `key_text` is empty (Ctrl produces control char), physical key maps back to printable key
- **Named keys**: `Enter`, `Tab`, `Escape`, `ArrowLeft`, etc.

---

## Config System (`heca-config/src/theme.rs`)

### Config File Format

```toml
# ~/.config/heca/config.toml

prefix = "ctrl+a"   # Prefix key (default: "ctrl+b")
theme = "mocha"     # Theme name

[settings]
window_width = 1280
window_height = 800
mouse = true
focus_follows_mouse = true
auto_scroll_edge = true
interactive_move_modifier = "Super"

[keys]
# Prefix bindings (checked in Prefix mode)
focus_left = "prefix+h"
focus_right = "prefix+l"
# ...

# Global bindings (checked in Normal mode, before terminal forwarding)
Alt+Enter = "spawn_terminal"

# Multiple bindings
focus_left = ["prefix+h", "prefix+ArrowLeft"]

# Unbind defaults
[keys.unbind]
"prefix+f" = true

# Spawn external commands
[[keys.command]]
keys = "prefix+g"
command = "lazygit"

# Custom modes
[[keys.mode]]
name = "resize"
trigger = "prefix+r"
sticky = true

[[keys.mode.bindings]]
action = "resize"
keys = "h"
args = { target = "column", axis = "x", amount = "-50" }
```

### Config Loading

1. `~/.config/heca/config.toml` (user config, optional)
2. Built-in defaults from `heca-config/src/theme.rs`
3. User config **overrides** defaults (same key replaces)
4. `keys.unbind` removes specific defaults
5. Default modes are **always merged** with user modes (user modes override same name)

### Unbinding Keybindings

To remove a default binding, add it to `[keys.unbind]`:

```toml
[keys.unbind]
"prefix+f" = true        # Remove float toggle
"prefix+q" = true        # Remove pane select
"prefix+Shift+q" = true  # Remove swap pane
```

**How it works:**

- During config loading, all defaults are bound first
- Then `[keys.unbind]` entries are processed
- `keymap.unbind("normal", &combo)` removes the binding from the normal mode keymap
- If the combo was also bound in global mode, it is removed from there too
- The action itself still exists — you can rebind it to a different combo

**Use cases:**

- Free up a key for a custom binding
- Disable features you don't use
- Resolve conflicts between default and custom bindings

### Config Reload

`WmAction::ReloadConfig` triggers `reload_config()` on `HecaApp`:

- Rebuilds keymaps from config file
- Reloads theme
- Updates settings (mouse, focus_follows_mouse, etc.)
- Does NOT restart the app

---

## Stack

| Layer | Crate | Version | Why |
|-------|-------|---------|-----|
| Windowing | `winit` | 0.30+ | De-facto Rust standard, cross-platform, HiDPI. |
| GPU API | `wgpu` | 0.25+ | Cross-platform (Vulkan/Metal/DX12/WebGPU), safe Rust. |
| Text layout | `cosmic-text` | 0.14+ | Best pure-Rust text stack; atlas caching, ligatures, variable fonts. |
| Terminal emulation | `alacritty_terminal` | 0.25+ | Battle-tested VT parser library. |
| PTY spawning | `portable-pty` | 0.9+ | Cross-platform PTY creation. |
| Async runtime | `tokio` | 1.40+ | All IO (PTY, Neovim socket, RPC server). |
| MsgPack | `rmpv` | 1.3+ | Neovim msgpack-RPC protocol. |
| Config | `serde` + `toml` | latest | TOML parsing. |
| Session IO | `bincode` or JSON | — | Session persistence format. |
| Dynamic loading | `libloading` | 0.8+ | In-process plugin loading. |

### What NOT to use

| Technology | Reason |
|------------|--------|
| Dioxus / Tauri / Electron | WebView-based; can't own the GPU render loop freely. |
| GTK / Qt | Fight you for custom GPU surfaces. Heavy cross-platform packaging. |
| egui / iced | Immediate-mode or over-constrained layout; we need to own pane rectangle assignment. |
| Bevy | Game engine ECS fights traditional GUI event loops. |
| skia-safe | Proven (Neovide) but requires C++ toolchain. Rust-native is lighter. |

---

## heca-grid-ui — Grid UI Component Library

> Full plan: `grid-ui-plan.md`. Developed on the **`heca-grid-ui`** branch.

`heca-grid-ui` is a **GPU-free, signal-driven, composable component library** that gives heca a *Tron/GridCN* visual identity (glow, corner brackets, scanlines, HUD typography). It is a *component framework*, not a theme.

### Rules

- **No GPU in `heca-grid-ui`.** It emits a `Scene` (a `DrawCommand` display list); `heca-renderer` rasterizes it. `heca-grid-ui` must NOT depend on `wgpu`, `winit`, or `heca-renderer`. Same boundary as `heca-core` — headless and unit-testable.
- **Composition, not inheritance.** Every component embeds a `Base` struct and implements the `Component` trait. "Extends base" = embed `Base` + impl trait, with builder-style styling.
- **Reactivity** via `floem_reactive`, hidden behind the `heca_grid_ui::reactive` facade — component code never names the dependency (swappable).
- **Component layout** via `taffy` (Flexbox/Grid/Block). This is *intra-component* layout (widgets inside a sidebar/panel/pane). It is **NOT** a second WM layout engine — `taffy` never positions panes or columns; the niri scrolling engine remains canonical for that.
- **Coordinates**: `Scene` carries `f32` logical pixels; the renderer scales to physical by `scale_factor` (HiDPI crispness preserved at the GPU boundary).
- The existing chrome (sidebar, tab/status bar, pane frames) becomes a **consumer** of `heca-grid-ui`; panes render as Grid-styled `Pane` shells over the unchanged layout engine.

### Using the widgets

**Full per-widget API reference + examples: [`docs/widgets.md`](docs/widgets.md)** — read it before using the library. Quick orientation:

- Import via `use heca_grid_ui::prelude::*;` (widgets, builder traits, `Theme`, `Color`, signals, events).
- Build a retained tree (`Flex`/`Card`/`Button`/…), then each frame: `LayoutEngine::new().compute(&mut root, size)` → paint into a `Scene` with `PaintCx` → `heca_renderer::scene::enqueue_scene(grid, text, &scene)`.
- Widgets opt into builder methods via marker traits: **`LayoutExt`** (`.width/.height/.padding/.gap/.justify/.align/.grow/.disabled/.tab_index`), **`StyleExt`** (surfaces only: `.background/.border/.glow/.radius`), **`Parent`** (`.child`).
- **Change widgets report via callbacks**, not return values: `.on_change(|action: Action| …)` carrying `Action::value("<name>-change", SignalData::…)` (`toggle-change`/Bool, `checkbox-change`/Bool, `input-change`/String, `tab-change`/Usize). Buttons use `.on_click(|| …)`.
- **`Base.disabled`** (dim+inert+unfocusable) and **`Base.tab_index`** are common to all widgets. Focus via one `FocusManager` (Tab/Shift+Tab, click-focus, `deliver_key`). Animations via `tick(dt) -> bool`.

**Catalog (implemented):** layout `Flex`/`Container`, `Surface`, `Card`; text `Label`; interactive `Button` (6 variants × 3 sizes), `Toggle`, `Checkbox` (optional clickable label), `Input` (full keyboard/selection model), `Tabs`; display `Badge`, `StatusDot`, `Separator`, `Spinner`, `Alert`, `ProgressBar`, `Gauge`. Foundations: `Base`, `Component`, `Theme`/`Intensity`, `Color`, `Action`/`SignalData`, `GridKey`/`Modifiers`/`Event`, `FocusManager`, `Flash`, `Scene`/`DrawCommand`/`PaintCx`.

### Gotchas

- **Don't run `cargo fmt`** in this repo — the local rustfmt reflows many files (no pinned `rustfmt.toml`); hand-format to match and verify with `cargo clippy --all-targets`.
- **Renderer text API**: `TextRenderer::queue_text(text, x, y, size, color)` positions at a top-left point (used across the app); `queue_text_in_box(text, x, y, w, h, size, color, bold, align)` centers within a box (used by the grid scene). Don't conflate them.
- **Never hard-code font family/size** — read `theme.font_family` / `theme.font_size`.

### Extra dependencies (in `heca-grid-ui` only)

| Crate | Purpose |
|-------|---------|
| `floem_reactive` | Fine-grained signals/memos/effects (behind facade). |
| `taffy` | Flexbox/Grid/Block component layout. |

---

## Project Structure

```
myvim/
├── AGENTS.md              ← This file
├── README.md              ← User-facing documentation
├── keybindings.toml       ← Complete keybinding reference
├── Cargo.toml             ← Workspace root
├── heca/                  ← Main binary (event loop, app state, rendering)
│   ├── src/
│   │   ├── main.rs        ← HecaApp, ApplicationHandler, render(), registry setup
│   │   ├── app_state.rs   ← AppState, InputMode, DragState, SidebarState
│   │   ├── input.rs       ← WmAction enum, action_from_name(), action_priority()
│   │   ├── keymap.rs      ← KeymapRegistry, KeyCombo, event_combo_matches()
│   │   ├── actions.rs     ← ActionRegistry, ActionDescriptor, ActionCategory
│   │   ├── handlers.rs    ← All action handlers (handle_focus_pane, handle_swap, etc.)
│   │   ├── sidebar.rs     ← SidebarTree, rendering, hit-testing, navigation
│   │   └── chrome.rs      ← ChromeConfig (tab bar, sidebar, status bar)
│   └── Cargo.toml
├── heca-core/             ← Layout engine + backends (no GPU code)
│   ├── src/
│   │   ├── layout/
│   │   │   ├── mod.rs     ← Module re-exports
│   │   │   ├── types.rs   ← Shared geometry types (Point, Size, Rectangle, ColumnWidth, etc.)
│   │   │   ├── animation.rs  ← Animation, SwipeTracker, easing functions
│   │   │   ├── view_offset.rs  ← ViewOffset (Static/Animation/Gesture)
│   │   │   ├── column.rs  ← Column, Pane, height distribution
│   │   │   ├── scrolling.rs  ← ScrollingSpace, focus, add/remove, view positions
│   │   │   ├── workspace.rs  ← Workspace, FloatingPane
│   │   │   └── session.rs ← Session, OverviewState, WorkspaceSwitch
│   │   ├── backend/
│   │   │   ├── mod.rs     ← PaneBackend trait, BackendRenderData
│   │   │   ├── terminal.rs  ← TerminalBackend (PTY + vte)
│   │   │   └── fake.rs    ← FakeBackend (no PTY, for layout testing)
│   │   ├── pane.rs        ← DEPRECATED old BSP tree code (kept for reference)
│   │   └── types.rs       ← Old Rect type (for BSP legacy)
│   └── Cargo.toml
├── heca-renderer/          ← GPU rendering (wgpu)
│   ├── src/
│   │   ├── lib.rs         ← Renderer init
│   │   ├── primitive.rs   ← PrimitiveRenderer (rects, borders)
│   │   └── text.rs        ← TextRenderer (cosmic-text atlas)
│   └── Cargo.toml
├── heca-config/            ← Configuration loading
│   ├── src/
│   │   ├── lib.rs         ← Module exports
│   │   └── theme.rs       ← Config, AppConfig, Theme, keybinding defaults
│   └── Cargo.toml
├── heca-grid-ui/                ← Grid UI component library (GPU-free, signals + taffy) [heca-grid-ui branch]
│   ├── src/
│   │   ├── reactive/      ← Signal/Memo/Effect facade over floem_reactive
│   │   ├── scene.rs       ← Scene + DrawCommand display list (visual vocabulary)
│   │   ├── style.rs       ← Style (taffy::Style wrapper) + Theme tokens + builders
│   │   ├── component.rs   ← Component trait + Base struct + Layout/Paint contexts
│   │   ├── layout.rs      ← taffy tree sync + compute → Base.bounds
│   │   └── widgets/       ← Flex, Grid, Stack, Label, Button, Card, Hud, Gauge,
│   │                        CornerBrackets, StatusBar, Sidebar, Pane, ...
│   └── Cargo.toml
├── docs/
│   └── niri-wiki/          ← NIRI documentation (structured wiki reference)
├── .planning/              ← GSD planning artifacts
│   ├── PROJECT.md          ← Project overview
│   ├── REQUIREMENTS.md     ← v1/v2 requirements
│   ├── ROADMAP.md          ← Phase roadmap
│   ├── STATE.md            ← Current execution state
│   └── research/           ← Research docs (NIRI analysis, architecture, stack, etc.)
├── .agents/
│   └── skills/
│       └── niri/SKILL.md   ← NIRI knowledge skill
└── review.md               ← Previous code review
```

---

## Available Skills

### `niri` (`.agents/skills/niri/SKILL.md`)

**Activate when:** Working on layout engine, ViewOffset, scrolling, workspaces, overview mode, or any feature inspired by niri's scrollable-tiling model.

Contains:

- Complete niri architecture reference (ScrollingSpace, Column, ViewOffset, Workspace)
- Scrolling model (horizontal continuous + snap, vertical discrete)
- Column width management (no normalization)
- Workspace system (dynamic, named, addressing)
- Animation types (spring vs easing) and parameters
- Full action reference for keybindings
- Window rules, layer rules, output config
- IPC protocol
- Fractional layout (physical pixel alignment)
- Animation timing (LazyClock) and redraw loop (RedrawState)
- Source file references into niri's actual codebase

### `pi-intercom` (for multi-session coordination)

Use when delegating tasks to other pi sessions.

### `pi-subagents` (for subagent workflows)

Use for multi-step analysis, advisory review, or parallel implementation tasks.

---

## Coding Conventions

### Do

- Use `f64` for all layout coordinates (logical pixels). Convert to `f32` only at the GPU render boundary.
- Put layout logic in `heca-core/src/layout/`. No GPU code in the core crate.
- Put GPU rendering in `heca-renderer/src/`. No layout logic in the renderer.
- Put input handling and WM actions in `heca/src/`. This is the orchestrator.
- Use `PaneBackend` trait for all pane content sources (terminal, neovim, browser).
- **Route ALL WM actions through `registry.execute()`**. Direct function calls are registry bypasses.
- Use `Animated<T>` for any value that should animate smoothly over time.
- Store column widths as `ColumnWidth::Proportion(f64)` or `ColumnWidth::Fixed(f64)`. NEVER normalize column widths.
- Store `working_area` in the layout engine; apply chrome offsets in the renderer.
- Add `#[cfg(debug_assertions)]` for debug logging.
- Use `expect("descriptive message")` instead of `unwrap()` for initialization code.

### Don't

- **Do NOT add a second WM layout engine.** The NIRI-inspired scrolling-column engine is canonical for arranging panes/columns. Old BSP code in `heca-core/src/pane.rs` is kept for reference only — do not wire it in. (Note: `taffy` in `heca-grid-ui` is *component-internal* widget layout — a different altitude — and does not count; it never positions panes/columns.)
- **Do NOT call `update_all_column_widths()` more than necessary.** Prefer stored column widths.
- **Do NOT use BSP tree concepts** (split direction, child ratios, etc.). NIRI layout is a flat column list with vertical pane stacks.
- **Do NOT hardcode `ctrl=false` in prefix mode.** The prefix key is a mechanism, not a modifier eraser.
- **Do NOT use the old `Rect` type** from `heca-core/src/types.rs`. Use `Rectangle` from `heca-core/src/layout/types.rs` for new code.
- **Do NOT add a webview.** All chrome renders via `wgpu` primitives.
- **Do NOT add tokio to the main event loop** without careful thought. winit events must not block. Use `pollster` for async init.
- **Do NOT create registry bypasses.** All focus/workspace/layout changes must go through `registry.execute()`.
- **Do NOT repeat yourself.** Prefer reusable components, modules, and functions. If you find yourself writing the same pattern multiple times (e.g., button rendering, hit testing, animation logic), extract it into a shared function or struct. Duplication breeds bugs and makes maintenance harder.

### Prefix Mode Design Rules

The project deliberately uses tmux-style prefix architecture (`Ctrl+B → key`). This is NOT a bug to be fixed. However:

- ✅ Pass real modifier state (`state.modifiers.control_key()`) in prefix mode — don't hardcode `false`.
- ✅ Add a prefix timeout (~500ms) so the user can't get stuck in prefix mode.
- ✅ Make the prefix key configurable (via `prefix = "ctrl+b"` in config).
- ✅ Forward literal prefix key on double-press (`Ctrl+B Ctrl+B` → send 0x02 to backend).
- ❌ Do not eliminate prefix mode — it prevents conflicts with hosted terminal apps.
- ❌ Do not make prefix mode modeless — that defeats the purpose.
- ❌ Do not forget that Ctrl-modified bindings (`Ctrl+h`, `Ctrl+]`) should work after prefix.

### Keybinding System Rules

- `WmAction` enum: one variant per action. Add new variants as needed.
- `action_from_name()`: maps config string names to actions. Keep in sync.
- `action_priority()`: **do NOT use `_ =>` catch-all** — explicitly match every variant.
- `resolve()`: case-insensitive key matching, modifier-exact. Physical key fallback for macOS.
- **All WM state changes go through `registry.execute()`** — no direct `focus_pane_by_id()` calls outside handlers.
- Default keybindings in `heca-config/src/theme.rs`: add new bindings here.

### Adding New Actions

1. Add variant to `WmAction` in `heca/src/input.rs`
2. Add string mapping in `action_from_name()`
3. Add priority in `action_priority()`
4. Create handler in `heca/src/handlers.rs`
5. Register in `build_registry()` in `heca/src/main.rs`
6. Add default binding in `heca-config/src/theme.rs`
7. Add descriptor in `ActionRegistry::ALL` in `heca/src/actions.rs`

---

## Commands

```bash
# Build
cargo build
cargo build --release

# Check (no codegen, fast)
cargo check

# Run
cargo run -p heca

# Test
cargo test
cargo test -p heca-core
cargo test -p heca-renderer

# Lint (before committing)
cargo clippy --workspace --all-targets --all-features

# Fix auto-fixable issues
cargo clippy --fix --workspace --all-targets --all-features

# Watch (auto-rebuild on changes)
cargo watch -x check
```

---

## GSD Workflow

This project uses the Get Shit Done (GSD) system for structured development:

| Command | What it does |
|---------|-------------|
| `/gsd-help` | Show available GSD commands |
| `/gsd-start-phase` | Begin working on a phase |
| `/gsd-complete-milestone` | Mark a milestone as done |
| `/gsd-transition` | Transition to next development phase |

See `.planning/PROJECT.md` for project overview, `.planning/ROADMAP.md` for phase details.

### Current State

| Phase | Status | Requirements |
|-------|--------|-------------|
| 1 — The Shell | ✅ ~Complete (GPU shell, theme, chrome) | 8 of 8 |
| 2 — The Workspace | ✅ ~Complete (NIRI layout, animations, input) | 28 of 28 |
| 3 — The Content | 🔄 In Progress (terminal backend wired) | PANE-01, PANE-02 done |
| 3b — Sidebar + Actions | ✅ **DONE** (sidebar tree, naming, cross-ws ops, command palette backend, registry system) | 9 phases complete |
| 4 — The Platform | ❌ Pending | Session persistence, RPC, plugins |

---

## Known Issues (from code review)

See `niri-compatibility-review.md` for full details. Key issues:

| ID | Issue | Severity | Status |
|----|-------|----------|--------|
| K1 | Prefix mode hardcodes `ctrl=false` | Critical | ✅ **FIXED** — passes real modifier state |
| K2 | Prefix key not configurable | High | ✅ **FIXED** — `prefix = "ctrl+b"` in config |
| K3 | No prefix timeout | Medium | ✅ **FIXED** — 500ms auto-exit |
| K4 | Shift+special-char bindings fail on some layouts | High | ✅ **FIXED** — `KeyCombo::parse()` maps shifted symbols |
| K5 | Registry bypasses (direct function calls) | High | ✅ **FIXED** — all routing through `registry.execute()` |
| L1 | `update_all_column_widths()` on every mutation | Critical | ✅ **FIXED** — removed from float/unfloat path |
| L2 | Proportion widths not persistent | High | Open |
| L4 | Focus up/down conflated with workspace switch | Medium | ✅ **FIXED** — `j/k` stay within workspace; `u/d` switch |
| L6 | Tabbed display, maximize, fullscreen dead code | Medium | Open |
| N1 | PaneSelect/Swap limited to 52 labels | Low | By design — use sidebar for >52 panes |

---

## Key NIRI References

- `docs/niri-wiki/` — Structured NIRI wiki documentation (architecture, config, usage, development)
- `.agents/skills/niri/SKILL.md` — NIRI knowledge skill with source references
- `docs/niri-wiki/architecture.md` — Full architecture reference (layout engine internals, data flow)
- `docs/niri-wiki/04-development/design-principles.md` — NIRI's 5 design principles
- `docs/niri-wiki/04-development/fractional-layout.md` — Physical pixel alignment strategy

### NIRI Terminology Mapping

| NIRI term | heca equivalent | Notes |
|-----------|----------------|-------|
| `Tile<W>` | `Pane` | Content leaf node |
| `Column<W>` | `Column` | Vertical stack of panes |
| `ScrollingSpace<W>` | `ScrollingSpace` | Horizontal column strip |
| `Workspace<W>` | `Workspace` | Contains scrolling + floating |
| `Layout<W>` | `Session` | Top-level with overview + workspace switch |
| `ViewOffset` | `ViewOffset` | Three-state horizontal scroll |
| `Monitor` | (not implemented) | heca is single-window; monitor = output is future |

---

## Agent Rules

### When Reading Code

1. Read `.planning/research/ARCHITECTURE.md` and `.planning/PROJECT.md` for context first.
2. Read `.agents/skills/niri/SKILL.md` when working on layout features.
3. Check `heca/src/input.rs` and `heca-config/src/theme.rs` for keybinding concerns.
4. Check `heca/src/main.rs` for registry setup and bypasses.
5. Run `cargo check` before and after changes — the project must compile.

### When Writing Code

1. Use the NIRI layout engine, not BSP (`pane.rs` is dead reference code).
2. Always use `Rectangle` from `layout/types.rs`, not `Rect` from `types.rs`.
3. **Every WM action goes through `registry.execute()`** — no direct function calls in event handlers.
4. Add new keybindings to both `heca-config/src/theme.rs` (defaults) and `heca/src/input.rs` (action enum + parser + priority).
5. Test prefix mode: verify both plain key and Ctrl-modified key bindings work.
6. Do NOT remove or refactor layout code without consulting the NIRI skill.
7. **NEVER add `#[allow(dead_code)]` without a clear reason.** Remove dead code instead. If a lint must be suppressed, add a `//` comment explaining why right above the attribute.
8. **After every task, run `cargo clippy --workspace --all-targets --all-features` and fix all warnings.** The codebase must stay clippy-clean. Use `cargo clippy --fix` for auto-fixable issues.

### When Reviewing

1. Check for BSP tree references that should be NIRI scrolling columns.
2. Verify `update_all_column_widths()` isn't called unnecessarily.
3. **Verify no registry bypasses** — all state changes go through `registry.execute()`.
4. Check prefix mode passes real modifier state, not hardcoded `false`.
5. Verify column widths are stored per-column, not normalized.
6. Check `action_priority()` explicitly matches all variants.
