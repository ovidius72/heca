# Phase 1: The Shell — Plan

**Phase:** 1
**Name:** The Shell
**Created:** 2026-05-29
**Waves:** 3
**Requirements Addressed:** COMP-01, COMP-02, COMP-03, COMP-04, COMP-10, CONF-01, CONF-03, CONF-04

---

## Must-Haves (Goal-Backward Verification)

1. A native OS window opens and stays responsive.
2. The window renders text via `cosmic-text` with a configurable font.
3. The window renders colored rectangles with borders and rounded corners.
4. Config and theme load from the user's config directory with sensible fallbacks.
5. A mock tiling layout is visible, proving the compositor concept.

---

## Wave 1: Cargo Workspace + winit + wgpu Window

### Plan 01: Scaffold Cargo Workspace

**Objective:** Create the multi-crate workspace structure and a runnable binary that opens a blank window.

**Requirements:** COMP-01, COMP-10

**Tasks:**

#### Task 1.1: Create workspace `Cargo.toml`
<read_first>
- None (greenfield)
</read_first>
<acceptance_criteria>
- `Cargo.toml` exists at workspace root with `[workspace]` section
- Workspace lists at minimum: `heca-core`, `heca-renderer`, `heca-config`, `heca` (binary)
- `cargo check` at workspace root succeeds with no errors
</acceptance_criteria>
<action>
Create `/Users/antonio/projects/myvim/Cargo.toml`:
```toml
[workspace]
members = ["heca-core", "heca-renderer", "heca-config", "heca"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["heca contributors"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/yourname/heca"
rust-version = "1.85"

[workspace.dependencies]
winit = "0.30"
wgpu = "25"
```
</action>

#### Task 1.2: Create crate directories with `Cargo.toml` files
<read_first>
- `/Users/antonio/projects/myvim/Cargo.toml` (workspace definition)
</read_first>
<acceptance_criteria>
- `heca-core/Cargo.toml`, `heca-renderer/Cargo.toml`, `heca-config/Cargo.toml`, `heca/Cargo.toml` all exist
- Each crate has `name`, `version`, `edition`, and correct `workspace = true` or `workspace.dependencies` usage
- `cargo check` at workspace root succeeds
</acceptance_criteria>
<action>
Create each crate's `Cargo.toml`:

`heca-core/Cargo.toml`:
```toml
[package]
name = "heca-core"
version.workspace = true
edition.workspace = true

[dependencies]
```

`heca-renderer/Cargo.toml`:
```toml
[package]
name = "heca-renderer"
version.workspace = true
edition.workspace = true

[dependencies]
heca-core = { path = "../heca-core" }
wgpu = { workspace = true }
```

`heca-config/Cargo.toml`:
```toml
[package]
name = "heca-config"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "6"
```

`heca/Cargo.toml`:
```toml
[package]
name = "heca"
version.workspace = true
edition.workspace = true

[dependencies]
heca-core = { path = "../heca-core" }
heca-renderer = { path = "../heca-renderer" }
heca-config = { path = "../heca-config" }
winit = { workspace = true }
wgpu = { workspace = true }
```

Create `heca-core/src/lib.rs`, `heca-renderer/src/lib.rs`, `heca-config/src/lib.rs`, `heca/src/main.rs` as empty/minimal files so `cargo check` passes.
</action>

#### Task 1.3: Open a winit window with a wgpu surface
<read_first>
- `/Users/antonio/projects/myvim/heca/Cargo.toml`
- `/Users/antonio/projects/myvim/heca/src/main.rs`
</read_first>
<acceptance_criteria>
- Running `cargo run -p heca` opens a resizable native window titled "heca"
- Window clears to a solid color (e.g., `#1e1e2e` Catppuccin Mocha base)
- Window responds to resize events without crashing
- `cargo check` succeeds on Linux, macOS, and Windows (CI-ready)
</acceptance_criteria>
<action>
Implement `heca/src/main.rs`:

1. Initialize `winit` event loop.
2. Create a window with title "heca" and default size 1280x800.
3. Initialize `wgpu`: create instance, request adapter, request device + queue.
4. Create surface and configure it with the window's size.
5. Handle `Resized` events: reconfigure surface.
6. Handle `RedrawRequested`: clear to `#1e1e2e` using a simple render pass, present.
7. Handle `WindowEvent::CloseRequested`: exit cleanly.

Use `winit` 0.30's `ApplicationHandler` trait or `EventLoop::run` callback.
Use `wgpu` `SurfaceConfiguration` with `present_mode: AutoVsync` initially.
</action>

---

## Wave 2: Text + Shape Rendering + Theme System

### Plan 02: cosmic-text + Primitive Renderer

**Objective:** Render styled text and GPU primitives (rectangles, borders, rounded corners) using a theme loaded from TOML.

**Requirements:** COMP-02, COMP-03, COMP-04, CONF-01, CONF-03, CONF-04

**Tasks:**

#### Task 2.1: Theme system — structs + TOML loading
<read_first>
- `/Users/antonio/projects/myvim/heca-config/src/lib.rs`
- `/Users/antonio/projects/myvim/.planning/phases/01-the-shell/01-CONTEXT.md` (decisions D-02, D-03)
</read_first>
<acceptance_criteria>
- `heca-config/src/lib.rs` defines `Theme` struct with: `background`, `foreground`, `border`, `accent` colors; `font_family`, `font_size`; `border_radius`, `border_width`, `shadow` fields
- Colors use a custom type supporting hex strings like `"#1e1e2e"` via `serde`
- `Config` struct holds `theme: String` (theme file name) and `general` settings
- `Config::load()` reads from `dirs::config_dir() / "heca" / "config.toml"`
- `Theme::load(name)` reads from `dirs::config_dir() / "heca" / "themes" / "{name}.toml"`
- Missing files use hardcoded defaults (Catppuccin Mocha)
- `cargo test -p heca-config` has at least one test verifying fallback behavior
</acceptance_criteria>
<action>
Implement in `heca-config/src/lib.rs`:

1. Define `Color` struct with `r: u8, g: u8, b: u8, a: u8`. Implement `FromStr` for hex parsing (`#RRGGBB` and `#RRGGBBAA`). Implement `Serialize`/`Deserialize` via a custom visitor that accepts hex strings.
2. Define `Theme` struct with all visual fields.
3. Define `Config` struct with `theme: String` and `general: GeneralConfig`.
4. Implement `Config::load() -> Result<(Config, Theme), ConfigError>`:
   - Try `dirs::config_dir() / "heca" / "config.toml"`
   - If missing, return defaults
   - Parse with `toml::from_str`
   - Resolve theme by name via `Theme::load()`
   - If theme missing, return bundled default (Catppuccin Mocha hardcoded)
5. Implement `Theme::default()` returning Catppuccin Mocha palette.
6. Add unit test: `test_fallback_when_config_missing` asserts default theme colors match Mocha.
</action>

#### Task 2.2: Bundle default theme files
<read_first>
- `/Users/antonio/projects/myvim/heca-config/src/lib.rs`
</read_first>
<acceptance_criteria>
- `heca-config/src/themes/mocha.toml` exists with Catppuccin Mocha palette
- `heca-config/src/themes/latte.toml` exists with Catppuccin Latte palette
- Both files loaded via `include_str!` at compile time as fallback when filesystem themes are absent
- `Theme::load("mocha")` returns the bundled version when no user theme exists
</acceptance_criteria>
<action>
Create `heca-config/src/themes/mocha.toml`:
```toml
name = "Catppuccin Mocha"
background = "#1e1e2e"
foreground = "#cdd6f4"
border = "#313244"
accent = "#89b4fa"
font_family = "JetBrainsMono Nerd Font"
font_size = 13.0
border_radius = 6.0
border_width = 1.0
shadow = { color = "#000000", alpha = 0.3, blur = 8.0 }
```

Create `heca-config/src/themes/latte.toml` with the light variant palette.

In `heca-config/src/lib.rs`, add a `BUNDLED_THEMES: HashMap<&str, &str>` built with `include_str!` macros.
</action>

#### Task 2.3: GPU primitive renderer (rectangles, borders, rounded corners)
<read_first>
- `/Users/antonio/projects/myvim/heca-renderer/src/lib.rs`
- `/Users/antonio/projects/myvim/heca-config/src/lib.rs` (Theme type)
</read_first>
<acceptance_criteria>
- `heca-renderer/src/lib.rs` exposes `PrimitiveRenderer` struct
- Can draw a filled rectangle with a solid color to a `wgpu` surface
- Can draw a rectangle border (outline only) with configurable width
- Can draw a rounded rectangle (filled + border) using a simple shader approach
- All drawing uses a single `wgpu` render pass per frame
- `cargo check -p heca-renderer` succeeds
</acceptance_criteria>
<action>
Implement `PrimitiveRenderer` in `heca-renderer/src/lib.rs`:

1. Create a simple `wgpu` pipeline for solid-color quads:
   - Vertex shader: 2D position + color (or use a uniform color)
   - Fragment shader: output solid color
   - Vertex buffer: 4 vertices per rect, 6 indices (two triangles)
2. `draw_rect(x, y, w, h, color)` — append vertices to a CPU-side buffer, upload at end of frame
3. `draw_border(x, y, w, h, color, width)` — draw 4 line segments or thin quads
4. `draw_rounded_rect(x, y, w, h, color, border_color, radius)` — for v1, approximate with a larger rect + border, or use a simple SDF shader if feasible
5. `render(&mut self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder)` — upload vertex buffer, set pipeline, draw all queued primitives, clear queue

Keep it simple: use `wgpu::PrimitiveTopology::TriangleList` and build a batched vertex buffer.
</action>

#### Task 2.4: cosmic-text text renderer
<read_first>
- `/Users/antonio/projects/myvim/heca-renderer/src/lib.rs`
- `/Users/antonio/projects/myvim/heca-config/src/lib.rs` (Theme type)
</read_first>
<acceptance_criteria>
- `heca-renderer/src/lib.rs` exposes `TextRenderer` struct using `cosmic-text`
- Can render a line of text at a given (x, y) position with a given font size and color
- Uses `cosmic-text` font system with system fonts + bundled fallback
- Text is crisp on HiDPI displays (respects winit scale factor)
- `cargo check -p heca-renderer` succeeds
</acceptance_criteria>
<action>
Implement `TextRenderer` in `heca-renderer/src/lib.rs`:

1. Add `cosmic-text` dependency to `heca-renderer/Cargo.toml`.
2. Initialize `cosmic_text::FontSystem` with a fallback chain:
   - Try JetBrains Mono Nerd Font (bundled or system)
   - Fallback to system monospace
   - Fallback to `cosmic-text` default fonts
3. Initialize `cosmic_text::SwashCache` for glyph rasterization.
4. Create `cosmic_text::Buffer` with metrics for a given font size.
5. `render_text(text, x, y, font_size, color)`:
   - Set buffer text
   - Shape with font system
   - Rasterize glyphs with swash cache
   - Upload glyphs to a `wgpu` texture atlas (or use a simple approach: render each glyph as a small textured quad)
   - For v1 simplicity: use `cosmic-text` to produce a pixmap, upload to a temporary texture, blit to screen
6. Track `scale_factor: f64` from winit and multiply font sizes accordingly.
</action>

---

## Wave 3: Mock Tiling Layout + Integration

### Plan 03: Mock Layout + Integration

**Objective:** Wire everything together. The window renders a fake tiling layout with multiple panes, borders, titles, and reflows on resize.

**Requirements:** COMP-02, COMP-03, COMP-04 (integration test)

**Tasks:**

#### Task 3.1: Mock layout data structure
<read_first>
- `/Users/antonio/projects/myvim/heca-core/src/lib.rs`
- `/Users/antonio/projects/myvim/heca-renderer/src/lib.rs` (PrimitiveRenderer, TextRenderer)
</read_first>
<acceptance_criteria>
- `heca-core/src/lib.rs` defines `MockPane` struct with `id: usize`, `title: String`, `background: Color`
- Defines `MockLayout` struct holding a `Vec<MockPane>` with hardcoded split ratios
- `MockLayout::compute_rects(window_width, window_height) -> Vec<(Rect, &MockPane)>` assigns pixel rectangles using simple splits
- At least 4 mock panes arranged in: left 60% (one tall pane), right 40% split horizontally (two panes)
</acceptance_criteria>
<action>
Implement in `heca-core/src/lib.rs`:

1. Define `Rect { x: f32, y: f32, w: f32, h: f32 }`.
2. Define `MockPane { id: usize, title: String, background: [f32; 4] }`.
3. Define `MockLayout { panes: Vec<MockPane> }`.
4. Hardcode a layout with 4 panes:
   - Pane 1: "Editor" — left 60%, full height — background `#1e1e2e`
   - Pane 2: "Terminal" — top-right 40%, 50% height — background `#181825`
   - Pane 3: "Files" — bottom-right 40%, 50% height — background `#11111b`
   - Pane 4: "Preview" — overlay float, centered 300x200 — background `#313244`
5. `compute_rects` takes window size and returns pane positions using simple ratio math.
</action>

#### Task 3.2: Integrate renderer into main loop
<read_first>
- `/Users/antonio/projects/myvim/heca/src/main.rs`
- `/Users/antonio/projects/myvim/heca-core/src/lib.rs` (MockLayout)
- `/Users/antonio/projects/myvim/heca-renderer/src/lib.rs` (PrimitiveRenderer, TextRenderer)
- `/Users/antonio/projects/myvim/heca-config/src/lib.rs` (Config, Theme)
</read_first>
<acceptance_criteria>
- `cargo run -p heca` opens a window showing 4 panes with distinct backgrounds
- Each pane has a visible border (1-2px) and a title rendered at the top-left
- Window resizing causes layout to recalculate and redraw
- Event-driven: no redraw happens when window is idle
- Colors match the active theme (default: Catppuccin Mocha)
</acceptance_criteria>
<action>
Update `heca/src/main.rs`:

1. On startup: call `Config::load()`, get the `Theme`.
2. Initialize `PrimitiveRenderer` and `TextRenderer` with the wgpu device/queue.
3. Create a `MockLayout`.
4. On `Resized`: recalculate mock pane rectangles using `MockLayout::compute_rects()`.
5. On `RedrawRequested`:
   a. Get current window size.
   b. For each pane in mock layout:
      - `primitive_renderer.draw_rect(rect, pane.background)`
      - `primitive_renderer.draw_border(rect, theme.border, theme.border_width)`
      - `text_renderer.render_text(&pane.title, rect.x + 8.0, rect.y + 4.0, theme.font_size, theme.foreground)`
   c. Submit both primitive and text draw calls in a single `wgpu` render pass.
   d. Present.
6. Track `scale_factor` from winit and pass to `TextRenderer`.
</action>

#### Task 3.3: Cargo workspace check + platform gates
<read_first>
- `/Users/antonio/projects/myvim/Cargo.toml`
- `/Users/antonio/projects/myvim/heca/Cargo.toml`
</read_first>
<acceptance_criteria>
- `cargo check` succeeds at workspace root
- `cargo build` succeeds at workspace root
- `cargo clippy` runs with no warnings (or warnings are explicitly allowed with comments)
- `cargo test` passes all tests in `heca-config`
- `Cargo.toml` files specify minimum Rust version 1.85
</acceptance_criteria>
<action>
1. Ensure all `Cargo.toml` files have `rust-version = "1.85"`.
2. Run `cargo check`, `cargo build`, `cargo test`.
3. Fix any compilation errors.
4. Add `.clippy.toml` or inline `#[allow(...)]` for any justified warnings.
5. Verify build succeeds on the current platform (macOS from the user's environment).
</action>

---

## Verification

After all waves complete, verify:

1. `cargo run -p heca` opens a window with 4 mock panes.
2. Each pane has a distinct background color, border, and title text.
3. Resizing the window recalculates layout and redraws.
4. No continuous redraw when idle (event-driven).
5. Config system works: delete `~/.config/heca/`, run app, it still opens with default Catppuccin Mocha theme.
6. `cargo test` passes.
