# heca — Agent Guide

> Everything an AI coding agent needs to work effectively on the heca project.
> Last updated: 2026-06-01

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
Ctrl+B → j    Focus pane down / next workspace
Ctrl+B → k    Focus pane up / prev workspace
Ctrl+B → Enter Split horizontal (new column to the right)
Ctrl+B → v    Split vertical (new pane in current column)
Ctrl+B → x    Close active pane
Ctrl+B → f    Toggle pane floating
Ctrl+B → q    Quick-select pane (overlay letters, all workspaces)
Ctrl+B → Shift+q  Quick-swap pane (all columns, all workspaces)
Ctrl+B → m    Swap and focus (all columns, all workspaces)
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
Ctrl+B → p    Command palette (backend ready, UI pending)
```

**Key rules:**
- Prefix mode is intentional (like tmux), NOT a bug. This avoids conflicts with hosted apps.
- The prefix key `Ctrl+B` is hardcoded — add config support when possible.
- In Normal mode, all key events are forwarded to the focused backend (terminal/nvim).
- Only the prefix key and explicitly bound keys trigger WM actions.
- **Prefix timeout:** auto-exits Prefix mode after 500ms of inactivity.
- **Pane letter limit:** PaneSelect/Swap modes use a-z, A-Z (52 unique labels). Sessions with >52 panes/columns fall back to sidebar navigation.
- **Mouse:** Click on sidebar items focuses them. Click on pane content area focuses that pane.

---

## Mouse Interaction System

heca has a fully configurable, multi-phase mouse interaction system. All mouse features are gated by `state.mouse_enabled` (set via config `general.mouse`, default `true`).

### Architecture

Mouse events flow through `heca/src/main.rs` in the `ApplicationHandler::window_event` match on `WindowEvent`:

```
CursorMoved  →  rubberband / drag update / focus-follows-mouse / insert-hint
MouseInput   →  sidebar hit-test / drag start / drag drop / click-to-focus
MouseWheel   →  (not yet implemented — see PANE-03)
```

### Phase 0: Focus Follows Mouse

When enabled (`general.focus_follows_mouse`, default `true`), hovering over a different pane automatically focuses it — no click required.

**Implementation:**
- In `CursorMoved`, after drag-state handling, calls `hit_test_pane(state, mouse_pos)`.
- Only activates when:
  - `drag_state == DragState::None` (not currently dragging)
  - `mouse_enabled == true`
  - `input_mode` is `Normal` or `Prefix` (disabled during `PaneSelect`, `PaneSwap`, `SidebarNav`, `Rename`)
  - Pointer is inside the **content area** (not over sidebar/chrome)
  - The hovered pane is different from `state.focused_pane`
- Calls `focus_pane_by_id(state, pane_id)` which compensates `view_offset` so the layout stays visually fixed.

### Phase 1: Click to Focus

A normal left-click (no modifier held) inside the content area focuses the clicked pane.

**Implementation:**
- In `MouseInput` (Left + Pressed), if no modifier is held:
  - Calls `hit_test_pane(state, mouse_pos)`
  - If a pane is hit, calls `focus_pane_by_id(state, pane_id)`
- Floating panes are hit-tested **before** scrolling panes since they render on top.
- Hit-test uses **exclusive bounds** (`<` not `<=`) so border points don't belong to two panes.

### Phase 2: Interactive Move (Meta+Click Drag-and-Drop)

Move a pane between columns or workspaces by dragging it with a modifier key held.

**Config:**
- `general.interactive_move_modifier`: `Super` (default), `Alt`, `Ctrl`, `Shift`
- Stored as strongly-typed `ModifierKey` enum (not bare String) for zero-cost dispatch and parse-time validation.

**Two-phase state machine (`DragState`):**

```
DragState::None
    ↓  Left-click + modifier held on pane
DragState::InteractiveMoveStarting { pane_id, start_mouse, threshold_sq }
    │
    │  CursorMoved:
    │    • Compute dx, dy from start_mouse
    │    • Apply rubberband: factor = rubberband(sq_dist / threshold_sq)
    │    • Set pane.interactive_move_offset = Point::new(dx * factor, dy * factor)
    │    • If sq_dist > threshold_sq → transition to Moving
    │
    ↓  transition_to_moving()
DragState::InteractiveMove { pane_id, offset }
    │
    │  CursorMoved:
    │    • Update detached_pane.render_pos = mouse_pos - pane_area - offset
    │    • Compute insert_hint from space coordinates
    │
    │  about_to_wait:
    │    • Edge scroll active during drag (wider trigger, faster speed)
    │
    ↓  Left-click release
    • drop_pane() with insert_hint, or
    • cancel_interactive_move() if still in Starting phase
```

**Rubberband formula** (exact from NIRI `src/rubber_band.rs`):
```rust
fn rubberband(x: f32) -> f32 {
    let c = 1.0;
    let d = 0.5;
    (1.0 - (1.0 / (x * c / d + 1.0))) * d
}
```

**Drop behavior:**
- `InsertPosition::NewColumn(col_idx)` — creates a new column with the pane
- `InsertPosition::InColumn { col_idx, pane_idx }` — inserts pane into existing column
- Uses **fresh ColumnId** (`ColumnId(state.session.next_id())`) to avoid ID collisions
- Animates the pane from its detached position to the new layout position via `animate_move_from()`

**Key invariant:** NIRI Principle 1 — removing a pane from a column does NOT affect widths of other columns. Only remove the column entirely if it becomes empty.

### Phase 3: Edge Scroll

Auto-scroll the horizontal layout when the pointer is near the left/right edge of the content area. Works both during drag and on plain hover.

**Config:**
- `general.auto_scroll_edge`: `bool` (default `true`)

**Dual-mode design:**

| Mode | Trigger | Speed | Inset | Content restriction |
|------|---------|-------|-------|---------------------|
| **Hover** | 80px | 300 px/sec | 8px | Must be inside content area |
| **Drag** | 150px | 1000 px/sec | 0px | Anywhere on screen |

**Content bounds clamping:**
```rust
min_view_pos = -padding;
max_view_pos = (total_content_width - viewport_width + padding).max(min_view_pos);
view_pos = proposed_view_pos.clamp(min_view_pos, max_view_pos);
```
- Stops scrolling at first/last column
- No scrolling when all content fits (`max_view_pos == min_view_pos`)

**Frame-rate independence:** Runs in `about_to_wait` (not just `CursorMoved`) with `dt`-based delta for smooth scrolling even when the mouse is stationary.

### Rendering

| Element | When | Style |
|---------|------|-------|
| Detached pane | During `InteractiveMove` | Rendered at `detached_pane.render_pos` with 3× accent border |
| Insert hint | During `InteractiveMove` | Translucent rectangle: 24px × 60% height for new column, full-width × 24px for in-column |
| Pane name | Always | Large centered label (distinct colors) so you can see what's moving |

### Files

| File | Role |
|------|------|
| `heca/src/main.rs` | Event handling (`CursorMoved`, `MouseInput`), `hit_test_pane()`, `dnd_edge_scroll()`, `transition_to_moving()`, `drop_pane()`, `cancel_interactive_move()`, `compute_insert_hint_rect()` |
| `heca/src/app_state.rs` | `DragState`, `DetachedPane` |
| `heca-core/src/layout/scrolling.rs` | `insert_position()` — computes drop target from pointer coordinates |
| `heca-core/src/layout/column.rs` | `Pane::interactive_move_offset` |
| `heca-config/src/theme.rs` | `ModifierKey` enum, `auto_scroll_edge`, `interactive_move_modifier` |

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

## Project Structure

```
myvim/
├── AGENTS.md              ← This file
├── Cargo.toml             ← Workspace root
├── heca/                  ← Main binary (event loop, app state, rendering)
│   ├── src/
│   │   ├── main.rs        ← HecaApp, ApplicationHandler, render(), execute_action()
│   │   ├── app_state.rs   ← AppState, InputMode, SidebarState, RenameTarget
│   │   ├── input.rs       ← WmAction enum, KeyBindings, resolve(), resolve_mode()
│   │   ├── sidebar.rs     ← SidebarTree, rendering, hit-testing, navigation
│   │   ├── actions.rs     ← ActionRegistry, ActionDescriptor, ActionCategory
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
- Pass `WmAction` through `execute_action()` for all WM commands.
- Use `Animated<T>` for any value that should animate smoothly over time.
- Store column widths as `ColumnWidth::Proportion(f64)` or `ColumnWidth::Fixed(f64)`. NEVER normalize column widths.
- Store `working_area` in the layout engine; apply chrome offsets in the renderer.
- Add `#[cfg(debug_assertions)]` for debug logging.

### Don't

- **Do NOT add a second layout engine.** The NIRI-inspired scrolling-column engine is canonical. Old BSP code in `heca-core/src/pane.rs` is kept for reference only — do not wire it in.
- **Do NOT call `update_all_column_widths()` more than necessary.** Prefer stored column widths.
- **Do NOT use BSP tree concepts** (split direction, child ratios, etc.). NIRI layout is a flat column list with vertical pane stacks.
- **Do NOT hardcode `ctrl=false` in prefix mode.** The prefix key is a mechanism, not a modifier eraser.
- **Do NOT use the old `Rect` type** from `heca-core/src/types.rs`. Use `Rectangle` from `heca-core/src/layout/types.rs` for new code.
- **Do NOT add a webview.** All chrome renders via `wgpu` primitives.
- **Do NOT add tokio to the main event loop** without careful thought. winit events must not block. Use `pollster` for async init.

### Prefix Mode Design Rules

The project deliberately uses tmux-style prefix architecture (`Ctrl+B → key`). This is NOT a bug to be fixed. However:

- ✅ Pass real modifier state (`state.modifiers.control_key()`) in prefix mode — don't hardcode `false`.
- ✅ Add a prefix timeout (~500ms) so the user can't get stuck in prefix mode.
- ✅ Make the prefix key configurable (when config supports it).
- ✅ Forward literal prefix key on double-press (`Ctrl+B Ctrl+B` → send 0x02 to backend).
- ❌ Do not eliminate prefix mode — it prevents conflicts with hosted terminal apps.
- ❌ Do not make prefix mode modeless — that defeats the purpose.
- ❌ Do not forget that Ctrl-modified bindings (`Ctrl+h`, `Ctrl+]`) should work after prefix.

### Keybinding System Rules

- `WmAction` enum: one variant per action. Add new variants as needed.
- `action_from_name()`: maps config string names to actions. Keep in sync.
- `action_priority()`: **do NOT use `_ =>` catch-all** — explicitly match every variant.
- `resolve()`: case-insensitive key matching, modifier-exact. Physical key fallback for macOS.
- `execute_action()`: every variant must do something — no empty match arms.
- Default keybindings in `heca-config/src/theme.rs`: add new bindings here.

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
cargo clippy --workspace -- -D warnings

# Watch (auto-rebuild on changes)
cargo watch -x check

# Validate config parsing (standalone)
cargo run --bin niri validate   # if you built niri separately
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
| 3b — Sidebar + Actions | ✅ **DONE** (sidebar tree, naming, cross-ws ops, command palette backend) | 9 phases complete |
| 4 — The Platform | ❌ Pending | Session persistence, RPC, plugins |

---

## Known Issues (from code review)

See `niri-compatibility-review.md` for full details. Key issues to be aware of:

| ID | Issue | Severity | Status |
|----|-------|----------|--------|
| K1 | Prefix mode hardcodes `ctrl=false`, breaking all Ctrl+key bindings | Critical | ✅ **FIXED** — passes real modifier state |
| K2 | Prefix key hardcoded to Ctrl+B (not configurable) | High | Open |
| K3 | No prefix timeout (sticky prefix mode) | Medium | ✅ **FIXED** — 500ms auto-exit in `about_to_wait` |
| K4 | Shift+special-char bindings fail on many layouts | High | Open |
| K5 | 6 actions (Scratchpad, Hide, ResizeL/R/U/D) are no-ops | Medium | Open |
| L1 | `update_all_column_widths()` runs on every mutation (violates niri principle 1) | Critical | Open |
| L2 | Proportion widths not stored persistently (window resize resets interactive resize) | High | Open |
| L4 | Focus up/down conflated with workspace switch | Medium | ✅ **FIXED** — `j/k` stay within workspace; `u/d` switch workspace |
| L6 | Tabbed display, maximize, fullscreen, preset widths defined but dead code | Medium | Open |
| N1 | PaneSelect/Swap limited to 52 unique labels (a-z, A-Z) | Low | By design — use sidebar for >52 panes |

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
4. Run `cargo check` before and after changes — the project must compile.

### When Writing Code
1. Use the NIRI layout engine, not BSP (`pane.rs` is dead reference code).
2. Always use `Rectangle` from `layout/types.rs`, not `Rect` from `types.rs`.
3. Every `WmAction` variant in `execute_action()` must have a non-empty implementation.
4. Add new keybindings to both `heca-config/src/theme.rs` (defaults) and `heca/src/input.rs` (action enum + parser + execute_action).
5. Test prefix mode: verify both plain key and Ctrl-modified key bindings work.
6. Do NOT remove or refactor layout code without consulting the NIRI skill.
7. **NEVER add `#[allow(dead_code)]` without a clear reason.** Remove dead code instead. If a lint must be suppressed, add a `//` comment explaining why right above the attribute.
8. **After every task, run `cargo clippy --workspace --all-targets --all-features` and fix all warnings.** The codebase must stay clippy-clean. Use `cargo clippy --fix` for auto-fixable issues.

### When Reviewing
1. Check for BSP tree references that should be NIRI scrolling columns.
2. Verify `update_all_column_widths()` isn't called unnecessarily.
3. Ensure `execute_action()` has no empty match arms.
4. Check prefix mode passes real modifier state, not hardcoded `false`.
5. Verify column widths are stored per-column, not normalized.
