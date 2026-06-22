# Research: Architecture

## 2026-05-29

**Context:** heca is a *native GUI application* — an OS window that acts as a tiling workspace manager with GPU-native rendering. It uses a NIRI-inspired scrolling-column layout engine rather than a traditional BSP tree.

---

## Major Components

### 1. Host (Main Binary)

The host is the only process that owns:
- The `winit` event loop
- The `wgpu` device and surface
- The `cosmic-text` font atlas and text renderer
- The global input router

Everything else is a pane runtime managed by the host.

### 2. Layout Engine (NIRI-inspired)

**Hierarchy:**

```
Session                          ← manages all workspaces + overview/expose mode
├── workspaces: Vec<Workspace>   ← arranged VERTICALLY (discrete switching)
│   └── Workspace
│       ├── scrolling: ScrollingSpace   ← horizontal COLUMNS (continuous scroll)
│       │   ├── view_offset: ViewOffset ← animated horizontal scroll + snap
│       │   ├── columns: Vec<Column>
│       │   │   ├── width: ColumnWidth  ← Proportion | Fixed
│       │   │   ├── panes: Vec<Pane>    ← vertical stack within column
│       │   │   └── pane_sizes: Vec<Size>  ← computed heights
│       │   └── active_column_idx
│       └── floating_panes: Vec<FloatingPane>
├── overview: OverviewState      ← zoom progress, open/closed
├── workspace_switch: WorkspaceSwitch  ← animated vertical transitions
└── active_workspace_idx
```

**Key insight — scrolling model (two axes, two mechanisms):** Columns scroll horizontally as a continuous strip. Focus change animates the view to snap the active column into view. Workspaces are a vertical stack with animated discrete switching.

**Scrolling model:**

| Axis | Container | Scroll Type | Mechanism |
|------|-----------|-------------|-----------|
| **Horizontal** | `ScrollingSpace.columns` | Continuous scroll + snap | `ViewOffset` (animated `f64`) |
| **Vertical** | `Session.workspaces` | Discrete switch + animation | `WorkspaceSwitch` (animated index) |

**Operations:**
- **Focus left/right** — activate previous/next column, animate `ViewOffset`
- **Focus up/down** — activate previous/next pane in column, or switch workspace
- **Add pane** — new column (horizontal) or new pane in column (vertical)
- **Remove pane** — remove from column, remove column if empty
- **Move column** — reorder columns with animation
- **Overview** — zoom out to see all workspaces as thumbnails

### 3. Compositor / Renderer

Runs every frame:
1. **Layout pass** — Compute pixel rectangles for all panes from `Session.workspace_geometries()` and `ScrollingSpace.panes_with_positions()`
2. **Content update** — Ask each visible pane's backend for latest content
   - Text-grid panes (terminal): update cell buffers
   - Future: Texture panes (browser), draw-command panes (plugins)
3. **Chrome draw** — Tab bar, pane borders/titles, status bar, sidebars
4. **Composite** — `wgpu` render passes (see `heca/src/app/render.rs`), in order:
   - **Clear** the scene target
   - **z=0 background blit** — `BackgroundLayer::render()` produces a (optionally
     blurred) vertical gradient, stamped fullscreen via `Backdrop::draw` at
     `background_alpha()` **pre-stencil** (`stencil = None`) so the tiled
     content-clip never clips it. This is the bottom-most layer panes composite over.
   - **Tiled stencil + content** — write the rounded content-clip mask, then draw
     tiled panes translucently (`surface_alpha`) over z=0; the frost IS z=0
     showing through, not a per-pane tint.
   - **Borders** (Pass 3).
   - **Floating panes** — capture a real blur of the tiled content
     (`state.blur.process`), then per floating pane stamp the blurred backdrop at
     **100% opacity** (no sharp leak) + terminal content + shell, clipped to the
     floating union stencil.
   - **Grid-ui chrome** (sidebar shell + status bar) — last.
   - **Global overlays** (overview, command palette)
5. **Present** — swapchain present

#### z=0 background frost model (heca-owned, cross-platform)

Frost is **heca-owned**, not OS-dependent. heca is an *application*, not a
Wayland compositor — it cannot blur the real desktop cross-platform (the
original `terminal_blur` "no effect" bug on macOS: vibrancy composites the
desktop *behind* the window, outside heca's render target). So heca renders and
blurs its own content: a vertical gradient (`heca-renderer/src/gradient.rs`)
blurred into a cached texture (`heca-renderer/src/background.rs` → `BackgroundLayer`).

- **`BackgroundLayer`** owns `z0_tex` (gradient render target) + `cache_tex`
  (blurred result) + a `dirty` flag. `render()` re-runs gradient+blur only when
  dirty (resize or `set_params` change); otherwise it returns the cached view.
  It **snapshots** the blurred result into `cache_tex` (via
  `encoder.copy_texture_to_texture`) *before* returning, so the shared
  `state.blur` is free to be reused afterwards by the floating-pane frost pass —
  z=0 MUST render before any other `state.blur` user in a frame.
- **Translucency channel:** theme `terminal_background` is opaque;
  `terminal_transparency` → `surface_alpha` is the only translucency channel
  (grill-me Q1, Option A). A theme `terminal_background` with alpha 0 is a bug.
- **Removed:** `terminal_blur`, `terminal_frost_color`, `terminal_frost_opacity()`,
  `effective_terminal_frost_color()`. Tiled frost strength = `background_blur`;
  floating frost = `terminal_floating_blur` + `terminal_floating_transparency`.
- **OS vibrancy** (`Vibrancy`) is an optional platform backdrop material only,
  `Vibrancy::None` by default (grill-me Q6: try z=0 without vibrancy first).

### 4. Pane Runtime (`PaneBackend` Trait)

Every pane content source implements:

```rust
pub trait PaneBackend: Send {
    fn pane_type(&self) -> PaneType;
    fn title(&self) -> &str;
    fn set_size(&mut self, cols: usize, rows: usize);
    fn process_input(&mut self, data: &[u8]);
    fn update(&mut self) -> bool;  // returns true if new data arrived
    fn render_data(&self) -> BackendRenderData;
    fn should_close(&self) -> bool;
}
```

**Built-in implementations:**
- `TerminalBackend`: PTY + `vte` parser → produces cell grid
- Future: `NeovimBackend`: msgpack-RPC → produces cell grid + chrome events
- Future: `BrowserBackend`: CEF offscreen → produces GPU texture

**BackendRenderData:**
```rust
pub enum BackendRenderData {
    Terminal {
        lines: Vec<TerminalLine>,
        cursor_col: usize,
        cursor_row: usize,
    },
    // Future: Neovim { ... }, Browser { texture_id }
}
```

### 5. Input Router

**Keyboard:**
- Global keymap resolves chords against WM bindings first.
- If no match, forward to focused pane's backend.
- WM bindings use a prefix key (e.g., `Ctrl+B`) then action key.

**Navigation (NIRI-style):**
- `h`/`l` — focus left/right column (with animated scroll)
- `j`/`k` — focus down/up pane in column (or switch workspace at boundaries)
- `-` — split horizontal (new column to the right)
- `v` — split vertical (new pane in current column)

**Mouse:**
- Hit-test against current frame pane rects from `pane_under()`
- Click pane → focus it
- Chrome clicks → tab switch, sidebar toggle

#### Interaction Policy Layer (`heca/src/app/interaction.rs`)

Every user-initiated WM action flows through a central policy chokepoint before it reaches the `ActionRegistry`:

```
User input → InteractionIntent → route_interaction() → RouteDecision
                                                        ├─ Allow(intent) → registry.execute()
                                                        └─ Block          → no-op (debug log under cfg(debug_assertions))
```

- **Keyboard:** `KeyCombo → WmAction → dispatch_action(state, Keyboard, &action)`
- **Mouse/sidebar:** `Click/Drag → InteractionIntent::FocusPane{..} → dispatch_action(state, MouseContent, &WmAction)`
- **Handler-to-handler** calls bypass the router and use `registry.execute()` directly (they're already inside an allowed interaction).

**`InteractionSource`** (where the interaction came from): `Keyboard`, `MouseContent`, `MouseLeftSidebar` (future: `MouseRightSidebar`, `MouseTopMenu`, `MouseStatusBar`, `Rpc`).

**`InteractionIntent`** (what the interaction is trying to do):
- `ActivateAction(WmAction)` — keyboard shortcut resolved to an action.
- `FocusPane { pane_id }` — focus a specific pane (sidebar click, content click, RPC).
- `FocusWorkspace { ws_idx }` — focus a workspace (sidebar click).
- `EnterSidebarNav` — enter sidebar navigation mode.
- `StartSidebarDrag { pane_id }` — mouse-layer drag start (no registry dispatch; policy-routed only).

**`RouteDecision`:** `Allow(intent)` (carries the intent forward for future retargeting) | `Block`.

**`FocusDomain`** (`heca-core/src/layout/workspace.rs`): per-workspace `Tiled` (default) | `Floating`.

**`ActionPolicy` enum** — classifies each `WmAction` variant; the router decides Allow/Block from it:

| Policy | Tiled | Floating | Meaning | Examples |
|--------|-------|----------|---------|----------|
| `Global` | Allow | Allow | True app-level, no layout impact; must stay reachable while floating | `ReloadConfig` |
| `AlwaysAllowed` | Allow | Block (current sources) | App-level but layout-affecting; blocked when floating from keyboard/mouse (future chrome sources may allow) | `CommandPalette`, `SpawnCommand`, `EnterMode` |
| `TiledOnly` | Allow | Block | Only meaningful in the tiled scrolling layout | `Focus*`, `Split*`, `ZoomColumn`, `Resize*`, `Swap*`, `Move*`, `Sidebar*`, `PaneSelect/Swap/Take`, `FloatAt`, `RenameColumn`, `DeleteColumn`, collapse/expand workspace+column |
| `FocusedPaneLocal` | Allow | Allow | Operates on the focused pane in either domain | `Float`, `ClosePane`, `ClosePaneById`, `RenamePane`, `RenameTarget`, `EnterSelectionMode`, `Selection*`, `ClearSelection`, `CopySelection`, `PasteClipboard`, `BeginSelection`, `ToggleSelectionEndpoint` |
| `WorkspaceLevel` | Allow | Block | Affects workspace structure | `WorkspaceNext`, `WorkspacePrev`, `FocusWorkspace`, `CreateWorkspace`, `RenameWorkspace`, `DeleteWorkspace` |
| `SourceDependent` | Allow | depends | Policy depends on source | `FocusPane` (allowed when floating only if it targets the active floating pane; else blocked) |

**`route_action()` logic** (per policy):
- `Global` → Allow always.
- `AlwaysAllowed` → Block if floating, else Allow.
- `TiledOnly` → Block if floating, else Allow.
- `FocusedPaneLocal` → Allow always.
- `WorkspaceLevel` → Block if floating, else Allow.
- `SourceDependent` (`FocusPane`) → if floating: Allow only if `pane_id` == active floating pane; else Block. If tiled: Allow. (Other source-dependent actions: if floating, Block from `Keyboard`/`MouseContent`/`MouseLeftSidebar`; else Allow.)

**Intent-level routing** (`route_interaction_for_session`): `ActivateAction` delegates to `route_action`; `FocusPane`/`FocusWorkspace`/`EnterSidebarNav`/`StartSidebarDrag` → Block if floating, else Allow.

**Floating-domain summary:** when `FocusDomain::Floating` is active, only `FocusedPaneLocal` + `Global` actions pass from keyboard/mouse sources. The only escape from floating is `prefix+f` (Float toggle) or `ClosePane`.

**`Global` vs `AlwaysAllowed`:** the name `AlwaysAllowed` is misleading — the router *blocks* it when floating. `Global` is the only truly always-allowed policy. Use `Global` for app-level actions with **zero layout impact** that must stay reachable while floating.

**Hot-reload bug (fixed 2026-06-18):** `ReloadConfig` was classified `AlwaysAllowed`, so `prefix+Shift+r` was silently blocked whenever a floating pane was active — config/style only applied on full restart. Fix: `ReloadConfig` is now `Global` (always allowed, even when floating). Regression test: `floating_allows_global_reload`.

**Invariants:**
- Every `WmAction` variant MUST be classified in `action_policy()` (exhaustive match, no wildcard) — enforced by the `action_policy_covers_all_variants` test.
- `ActionPolicy` (interaction.rs — Allow/Block per focus domain) is **unrelated** to `action_priority()` (input.rs — keybinding resolution priority). Do not conflate.
- Blocked actions are silently discarded (debug `eprintln!` under `cfg(debug_assertions)`).

### 6. Session Persistence

**What is saved:**
- Session metadata (name, created date)
- Workspace list with column/pane structure
- Pane metadata: backend type, spawn command, CWD
- Layout options (gaps, column widths, etc.)

**What is NOT saved:**
- PTY scrollback (too large)
- Neovim buffer content (nvim has `:mksession` for this)
- GPU textures

**Restore flow:**
1. Deserialize session file.
2. Create winit window.
3. Rebuild `Session` with workspaces, columns, panes.
4. Respawn each pane's backend in its saved CWD.

---

## Data Flow

```
Terminal PTY ──bytes──► TerminalBackend ──cell grid──► Compositor
Neovim Process ──msgpack──► NeovimBackend ──cell grid──► Compositor
                                        ▲
                                        │
Input Router ──events──► Focused Pane ──┘
                                        │
RPC Server ──commands──► Session ──mutations──► Layout Engine
```

---

## Build Order

1. **GPU Shell** — winit + wgpu + cosmic-text. Prove we can render text and rects.
2. **Layout Engine** — NIRI-style scrolling columns, animations, overview mode.
3. **Terminal** — PTY + vte parser. First real pane content.
4. **Neovim** — msgpack client + grid state.
5. **Input Router** — Full keyboard + mouse, keybindings, hit-testing.
6. **Session Persistence** — Save/restore.
7. **RPC Server** — External control.
8. **Browser / Out-of-process** — v2.

---

## Files

| Component | Path |
|-----------|------|
| Layout types | `heca-core/src/layout/types.rs` |
| Animation system | `heca-core/src/layout/animation.rs` |
| ViewOffset (scroll) | `heca-core/src/layout/view_offset.rs` |
| Column / Pane | `heca-core/src/layout/column.rs` |
| ScrollingSpace | `heca-core/src/layout/scrolling.rs` |
| Workspace | `heca-core/src/layout/workspace.rs` |
| Session / Overview | `heca-core/src/layout/session.rs` |
| PaneBackend trait | `heca-core/src/backend/mod.rs` |
| Terminal backend | `heca-core/src/backend/terminal.rs` |
| Renderer | `heca-renderer/src/lib.rs` |
| App / Event loop | `heca/src/main.rs` |

---

*Key differences: Unlike TUI multiplexers (herdr, tmux, zellij), we do not stream ANSI to a terminal — we composite pixels in a GPU surface. Unlike traditional tiling WMs (i3, sway), we use horizontal scrolling columns rather than fixed-grid trees. Like NIRI, our layout is a scrollable-tiling model where columns overflow the viewport and the view offset animates to reveal them.*
