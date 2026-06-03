# heca-grid-ui — Grid UI Component Library: Plan

> A reusable, signal-driven, composable GPU component library that gives heca a *Tron/GridCN* visual identity.
> Branch: **`heca-grid-ui`** · Status: **A done · B done · C in progress** · Last updated: 2026-06-03

---

## ▶ Resume Here

**Where we are:** A polished `Button` + the full component foundation are built, tested, and committed on branch `heca-grid-ui`. The renderer + glow + text + accessibility all work. Next is building the rest of the catalog widgets.

**Run / verify:**
```bash
cargo run -p heca-renderer --example showcase          # the live demo (needs a display)
cargo test -p heca-grid-ui                              # 6 unit + 14 integration + doctests
cargo clippy -p heca-grid-ui -p heca-renderer --all-targets   # must be clean
```

**Done (committed):**
- **Phase A** — `heca-grid-ui` crate: `reactive` facade (`floem_reactive`), `taffy` layout, `Scene`/`DrawCommand`, `Component` trait + `Base` (composition), `Style`/`Theme` (dark `grid_tron` default), `Flex`/`Container`/`Label`.
- **Phase B** — `heca-renderer`: SDF rounded-rect pipeline `grid.rs`/`grid.wgsl` with **premultiplied additive glow** (real translucent halo), `scene.rs` bridge (`enqueue_scene`), embedded **Geist Mono Regular+Bold**, bold + **metric-based text centering** in `text.rs`, `examples/showcase.rs`.
- **Phase C (partial)** — builder traits `LayoutExt`/`StyleExt`/`Parent`; `Flex` is layout-only; `Surface`, `Card`, **`Button`** (6 GridCN variants × 3 sizes; animated per-variant hover; tuned glow; per-variant **press `Flash`**; focus-visible ring).
- **Accessibility** — `GridKey`, `Event::Key`, `FocusManager` (Tab/Shift+Tab + `focus_at` click-focus + wrap), `on_focus`/`on_blur`, Space/Enter activation, **focus-visible** (ring on keyboard focus only), `Theme.show_focus_border`.
- **Foundations** — `Action`/`SignalData` (`action.rs`, for change-event values — not yet wired), `Flash` (`effects.rs`, reusable press effect + `PaintCx::flash`).

**Key files (`heca-grid-ui/src/`):** `component.rs` (Component/Base/PaintCx/Event/GridKey/on_focus), `focus.rs` (FocusManager), `effects.rs` (Flash), `action.rs`, `builders.rs`, `theme.rs`, `style.rs`, `scene.rs`, `layout.rs`, `widgets/{flex,label,surface,card,button}.rs`. **Renderer:** `heca-renderer/src/{grid.rs,grid.wgsl,scene.rs,text.rs}` + `examples/showcase.rs`.

**Next steps (in priority order):**
1. **`Toggle`, `Checkbox`, `Input`** — they reuse `Flash` + `focusable()` for free, and are the first **change widgets**: return `Action::value("…-change", SignalData::…)` with the new value. This is when the `Action`/`SignalData` model gets wired into `event()` returns.
2. Then the rest of the catalog (`Badge`, `Tag`, `Chip`, `StatusDot`, `Separator`/`Divider`, `Spinner`, `Tooltip`, `Alert`, `Select`, `Modal`/`Dialog`, `CommandPalette`, `Sidebar`, `StatusBar`, `MenuBar`, …) — full list + GridCN reference links in `docs/the-grid-ui.md`.
3. **Phase D** — app adoption (dark grid theme default, real `Sidebar` + `Pane` shells over the niri layout).

**⚠️ Two-doc reconciliation (open):** `docs/the-grid-ui.md` holds the GridCN reference + the canonical **component catalog, reference links, and event/accessibility spec** — but it describes an *older architecture* (`ComponentBase` + `impl_component!` macro + `SignalBus` + manual layout). **The implemented code follows THIS plan's architecture** (signals + taffy + `Event`/`Handled` + builder traits). The doc's event/accessibility *directions* were implemented, mapped onto the real architecture. When convenient, reconcile the two docs into one.

---
>
> Locked deps: `floem_reactive 0.2.0`, `taffy 0.7.7`. Q1 resolved (local `Color`), Q3 resolved (standalone example).

---

## 1. Goal & Non-Goals

### Goal

Build a new crate, **`heca-grid-ui`**, that is a **component framework** — not a theme — with these properties:

- **Composable**: small components nest to build big ones (`Flex` of `Card`s of `Gauge`s).
- **Extensible**: every component extends one shared base (`Base` struct + `Component` trait); adding a new widget is embedding `Base` + implementing the trait.
- **Signal-driven**: fine-grained reactivity — when a `Signal<T>` changes, only the dependent components repaint.
- **Easy to style**: builder-style API + token-based `Theme`; colors and effects fully configurable.
- **Tron look, natively**: glow, corner brackets, scanlines, HUD typography — realized in `wgpu`, not a webview.

The existing chrome (sidebar, status bar, pane frames) becomes a **consumer** of this library, and panes render as Grid-styled components over the unchanged niri layout engine.

### Non-Goals

- **Not** a new window-management layout engine. `heca-core`'s scrolling-column engine stays canonical for arranging panes/columns. `heca-grid-ui` lays out *widgets inside* components.
- **Not** a port of GridCN's web stack. No `oklch`, no Tailwind, no Radix — we pick our own palette and tokens.
- **Not** terminal/Neovim content rendering. The `Pane` component renders the *frame* (border, brackets, header); the `PaneBackend` still renders its cell grid into the inner content rect.

---

## 2. Locked Architecture Decisions

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D1 | Crate | New `heca-grid-ui` crate in the workspace | Reusable, isolated, testable. |
| D2 | GPU boundary | `heca-grid-ui` is **GPU-free**; emits a `Scene` (display list). `heca-renderer` rasterizes it. | Matches the project's `heca-core` rule: headless, unit-testable, fast compiles, swappable renderer. Same model as Zed/GPUI, Flutter, WebRender. |
| D3 | Reactivity | `floem_reactive` behind a thin `heca_grid_ui::reactive` facade | Fine-grained-reactivity correctness (effect tracking, cleanup, diamond problem) is hard; reuse a proven, renderer-agnostic runtime. Facade keeps the dependency un-named in component code so it can be swapped later. |
| D4 | Layout | `taffy` (Flexbox + Grid + Block) | The proven pure-Rust layout engine (Floem, GPUI, Bevy, Dioxus). Operates at component-internal altitude, orthogonal to the niri WM layout. |
| D5 | "Extends base" | Composition: embed `Base` + implement `Component` trait (idiomatic Rust, not inheritance) | Rust has no inheritance; embedding + trait gives shared behavior with `Deref`-ergonomic access. |
| D6 | Default theme | Dark Grid theme (`grid_tron`) becomes the default; background set via `Theme.background` | Already the mechanism (`catppuccin_mocha` is the current dark default). |
| D7 | Glow strategy | Per-primitive **SDF glow** (no extra render targets); full-frame bloom deferred | Cheaper, no render-loop restructuring, looks great for neon borders/brackets/HUD. |
| D8 | Coordinate space | `Scene` reuses `heca-core`'s **f64 logical** geometry (`Rectangle`/`Point`/`Size`); taffy's f32 output is cast to f64 at write-back; renderer converts f64→f32 at the GPU boundary | *Revised during Phase A.* Reusing `heca-core` geometry is cleaner than a parallel f32 type and matches the project's established "f64 logical, f32 at GPU" rule. HiDPI crispness preserved. |

---

## 3. Crate Layering

```
heca-grid-ui  (NEW, no wgpu)
  ├── reactive/   ← Signal/Memo/Effect facade over floem_reactive
  ├── scene.rs    ← Scene + DrawCommand display list (the visual vocabulary)
  ├── style.rs    ← Style (taffy::Style wrapper) + Theme tokens + builders
  ├── component.rs← Component trait + Base struct + layout/paint/event contexts
  ├── layout.rs   ← taffy tree sync + compute → Base.bounds
  └── widgets/    ← Flex, Grid, Stack, Label, Button, Card, Hud, Gauge,
                    CornerBrackets, StatusBar, Sidebar, Pane, ...
        │
        │ emits Scene
        ▼
heca-renderer  ← render_scene(&Scene): SDF-glow + scanline + text pipelines
heca-core      ← geometry (Point/Size/Rectangle), reused by heca-grid-ui
heca           ← builds component trees for chrome; feeds Scene to renderer
```

**Dependency rule:** `heca-grid-ui` → `heca-core` (+ `floem_reactive`, `taffy`). `heca-grid-ui` must **not** depend on `wgpu`, `winit`, or `heca-renderer`.

---

## 4. Core Abstractions

### 4.1 `Component` trait + `Base` struct

```rust
/// Shared state every component owns. Concrete widgets embed this.
pub struct Base {
    pub style: Style,            // wraps taffy::Style + Tron visual tokens
    pub node: taffy::NodeId,     // this component's node in the layout tree
    pub bounds: Rectangle,       // filled in AFTER taffy computes layout
    pub visible: Signal<bool>,
    pub children: Vec<Box<dyn Component>>,
}

pub trait Component {
    fn base(&self) -> &Base;
    fn base_mut(&mut self) -> &mut Base;

    /// Register taffy nodes/styles. Default: container of children.
    fn register(&mut self, cx: &mut LayoutCx);

    /// Emit draw commands into the scene. Default: bg + border + children.
    fn paint(&self, cx: &mut PaintCx);

    /// Handle input; default routes to children, then self.
    fn event(&mut self, _ev: &Event) -> Handled { Handled::No }
}
```

A concrete widget "extends" `Base` by embedding it and adding its own signals:

```rust
pub struct Button { base: Base, label: Signal<String>, on_click: Callback }

impl Button {
    pub fn new(label: impl Into<String>) -> Self { /* default Style */ }
    pub fn glow(mut self, c: Color) -> Self { self.base.style.glow = Some(c.into()); self }
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self { /* ... */ self }
}

impl Component for Button {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn paint(&self, cx: &mut PaintCx) {
        let s = &self.base.style;
        cx.rect(self.base.bounds, s.fill, s.border, s.glow);   // SDF + glow
        cx.corner_brackets(self.base.bounds, s.accent);        // shared decorator (DRY)
        cx.text_centered(self.base.bounds, &self.label.get(), s.fg); // .get() subscribes
    }
}
```

### 4.2 Reactive facade (`reactive/`)

```rust
// Public API never names floem_reactive — swappable.
pub use facade::{Signal, RwSignal, Memo, Effect, batch};
```

- `Signal<T>` is a `Copy` handle. `.get()` subscribes the active node; `.set()`/`.update()` marks subscribers dirty.
- A dirty signal flips the app's existing `needs_redraw`; repaint is coalesced to one frame.

### 4.3 Layout (`layout.rs`, via taffy)

Per frame, only when the tree is dirty:
1. Sync each `Base.style` → `taffy::Style` on its node.
2. `taffy.compute_layout(root, available_space)`.
3. Walk the tree, copy each node's computed `(x, y, w, h)` → `Base.bounds`.
4. `paint` reads `bounds`.

`Flex` API:

```rust
Flex::row()                          // Flex::column() for vertical
    .gap(8.0)
    .justify(Justify::SpaceBetween)  // main axis
    .align(Align::Center)            // cross axis
    .padding(12.0)
    .child(Label::new("STATUS"))
    .child(Gauge::new(power).flex_grow(1.0))
    .child(Button::new("DEREZ").glow(theme.danger))
```

---

## 5. Visual Vocabulary (`scene.rs`)

The richness here is what makes the Tron look possible without `heca-grid-ui` touching the GPU.

```rust
pub struct Scene { commands: Vec<DrawCommand> }   // built fresh each repaint

pub enum DrawCommand {
    Rect(RectCmd),          // solid/SDF rounded rect, optional border + outer glow
    Brackets(BracketCmd),   // L-shaped corner reticle framing a rect
    Text(TextCmd),          // a positioned text run (color, size, weight, align)
    Scanline(ScanlineCmd),  // region scanline overlay (spacing, color, opacity)
    Gradient(GradientCmd),  // linear gradient fill
    PushClip(Rectangle),    // clip subsequent commands
    PopClip,
    Custom(CustomCmd),      // escape hatch: named shader effect + params (Phase E+)
}

pub struct RectCmd { pub rect: Rectangle, pub fill: Color, pub border: Option<Border>,
                     pub radius: f32, pub glow: Option<Glow> }
pub struct Glow { pub color: Color, pub radius: f32, pub intensity: f32 }
pub struct BracketCmd { pub rect: Rectangle, pub color: Color, pub len: f32,
                        pub thickness: f32, pub glow: Option<Glow> }
```

`heca-renderer::render_scene(&Scene)` maps each command to a pipeline (SDF-glow, scanline, text-atlas). Adding an effect = one variant + one shader branch; component code is untouched.

---

## 6. Component Catalog

Prioritized by what heca actually needs. Each extends `Base`.

| Tier | Components |
|------|-----------|
| **Primitives** | `Flex`/`Container` (layout-only), `Surface` (base styled box), `Grid`, `Stack`, `Spacer`, `Label` |
| **Interactive** | `Button` (6 variants: primary/secondary/destructive/outline/ghost/link × sm/md/lg — ref <https://thegridcn.com/components#button-example>), `IconButton`, `Toggle`, `Tabs` |
| **Display** | `Card`/`DataCard`, `Panel`/`Hud`, `Separator`, `Badge`, `StatusBar` |
| **Tron-flavor** | `CornerBrackets` (decorator), `Reticle`, `Gauge`, `EnergyMeter`, `SignalIndicator`, `ScanlineOverlay`, `CoordinateDisplay` |
| **heca-specific** | `Sidebar` (tree nav), `Pane` (frame + HUD header) |

---

## 7. Phases & Tasks

Build order: **A → B → C standalone; D adopts into the app** (lowest risk; "library + showcase first").

### Phase A — Crate skeleton & core model (no GPU)

**Goal:** `heca-grid-ui` compiles and is unit-tested with zero GPU; the reactive + layout + component model is proven by a trivial tree.

- [x] A1. Add `heca-grid-ui` to workspace members; `Cargo.toml` with `heca-core`, `floem_reactive`, `taffy` deps (pin versions; verify against rust 1.85 / edition 2024).
- [x] A2. `reactive/` facade: re-export `Signal`/`RwSignal`/`Memo`/`Effect`/`batch`; document the swap-ability contract in `//!`.
- [x] A3. **Resolved (Q1):** `heca-grid-ui` owns its own `Color` (richer helpers: `lerp`, `with_alpha`, hex `FromStr`). Keeps Phase A additive (zero churn in existing crates); `From`/`Into` conversions to `heca-config::Color` deferred to Phase D.
- [x] A4. `scene.rs`: `Scene` + `DrawCommand` enum + sub-structs (`RectCmd`, `Glow`, `BracketCmd`, `TextCmd`, `ScanlineCmd`). Derive `Debug`, `Clone`.
- [x] A5. `style.rs`: `Style` wrapping `taffy::Style` + Tron tokens (`fill`, `border`, `glow`, `accent`, `radius`, `intensity`); builder methods; `Theme` token struct with `grid_tron()` preset.
- [x] A6. `component.rs`: `Component` trait, `Base` struct, `LayoutCx`/`PaintCx`/`Event`/`Handled`; default `register`/`paint`/`event`.
- [x] A7. `layout.rs`: taffy tree build + `compute_layout` + write-back to `Base.bounds`.
- [x] A8. `widgets/`: `Container`, `Flex` (row/column/gap/justify/align/padding), `Label`.
- [x] A9. Tests: flex distribution (grow/justify/align/gap), nested layout rects, signal `set` → dirty propagation, `paint` emits expected `DrawCommand`s. **All headless.**

**Exit:** `cargo test -p heca-grid-ui` green; `Flex::row().child(..).child(..)` produces correct `bounds` and a `Scene` snapshot.

### Phase B — Renderer backend & showcase

**Goal:** First pixels. The Grid look is visible.

- [x] B1. `heca-renderer`: SDF rounded-rect + glow pipeline (`grid.wgsl`) — instanced quads carrying rect params + glow params; rounded-rect signed distance; interior fill + additive exponential outer falloff. Keep the existing flat pipeline intact.
- [~] B2. Scanlines — **basic** version done (thin glowing rects via the grid pipeline, intensity-gated through theme); dedicated full-region scanline shader still TODO.
- [x] B3. Corner-bracket rendering — 8 glowing arm-rects via the SDF pipeline (`scene::draw_brackets`).
- [x] B4. `heca-renderer::scene::enqueue_scene(grid, text, &Scene)`: maps every `DrawCommand` → grid/text renderer; basic text alignment. (Clip commands stubbed — see B-clip TODO.)
- [ ] B5. Physical-pixel alignment for crisp 1px borders on fractional scale (`docs/niri-wiki/04-development/fractional-layout.md`).
- [x] B7. **Embed default mono font** — Regular + Bold vendored (OFL 1.1), `DEFAULT_MONO_BYTES`/`DEFAULT_MONO_BOLD_BYTES`, loaded into `TextRenderer`'s `cosmic-text` font system; family stays theme-configurable. Italic = synthesized oblique (`OBLIQUE_SKEW`); Geist Mono has no italic face and no coding ligatures (`calt`/`dlig` absent).
- [x] B6. **Showcase** example — `cargo run -p heca-renderer --example showcase`: a row of glowing HUD cards (border + glow + corner brackets) with a scanline overlay on the dark `grid_tron` theme. *(Placed in `heca-renderer/examples` to avoid a dep cycle; needs a display to view.)* (`heca-grid-ui/examples/showcase.rs` driving a winit+wgpu window, or a `--showcase` flag in `heca`): a `Flex` of glowing `Card`s with brackets + scanline over a dark background.

**Exit:** `cargo run` shows the showcase: glowing rounded borders, corner brackets, scanline overlay, crisp text on retina.

### Phase C — Component library

**Goal:** The reusable Tron component set heca will consume.

- [x] C1. `Button` **done** — 6 GridCN variants (primary/secondary/destructive/outline/ghost/link) × 3 sizes, variant-driven look from theme tokens, hover + click. `IconButton`/`Toggle` still pending.
- [~] C2. `Surface` (base styled box) + `Card` **done**; `DataCard`/`Panel`/`Hud`/`Separator`/`Badge` pending.
- [x] C0. Builder traits `LayoutExt`/`StyleExt`/`Parent` enforcing layout-vs-surface separation; `Flex` made layout-only; showcase migrated to `Card`/`Button` with live hover/click.
- [ ] C3. `Gauge`, `EnergyMeter`, `SignalIndicator` (value-driven via signals).
- [ ] C4. `CornerBrackets` decorator + `Reticle` (shared, used by Pane/focus chrome).
- [ ] C5. `StatusBar` (segmented `Flex`, signal-bound segments).
- [ ] C6. `Sidebar` component (tree model, expand/collapse, cursor, drag affordance) — Grid-styled, mirrors current `heca/src/sidebar.rs` behavior.
- [ ] C7. `Pane` shell: glow border, corner brackets when focused, HUD header (title + status); exposes an inner content `Rectangle` for the backend.
- [ ] C8. Extend showcase to exercise every component; visual + snapshot tests.

**Exit:** All components render in the showcase; each is composable in a `Flex`/`Grid` and stylable via builders + theme.

### Phase D — App adoption

**Goal:** heca itself wears the Grid skin.

- [ ] D1. `grid_tron` becomes the default theme; dark background via `Theme.background`. Keep `mocha`/`latte` selectable.
- [ ] D2. Replace `heca/src/sidebar.rs` rendering with the `heca-grid-ui` `Sidebar` component, wired to WM-state signals (tree, focused item).
- [ ] D3. Wrap each laid-out pane in the `heca-grid-ui` `Pane` shell (frame/brackets/header); backend content renders into the inner rect. **Layout engine untouched.**
- [ ] D4. Rebuild tab bar + status bar as `heca-grid-ui` components (replaces scattered `draw_rect` calls in `main.rs` — DRY win).
- [ ] D5. Bridge WM state → signals: focused pane, active workspace, pane titles update reactively.
- [ ] D6. `CycleTronIntensity` action (Off→Low→Med→Heavy) through the **full action pipeline**: `WmAction` variant → `action_from_name()` → explicit `action_priority()` → handler → `build_registry()` → default binding → `ActionRegistry::ALL` descriptor.
- [ ] D7. `cargo clippy --workspace --all-targets --all-features` clean; update README + AGENTS.md.

**Exit:** heca launches dark with a Grid sidebar, Grid pane frames, and live intensity switching; niri layout/animations behave exactly as before.

### Phase E — (Optional, later) Full-frame bloom

- [ ] E1. Offscreen render target + resize handling.
- [ ] E2. Bright-pass threshold → separable Gaussian blur (downsampled) → additive composite.
- [ ] E3. `Custom` DrawCommand wiring for exotic effects.

**Exit:** Cinematic full-frame bloom toggle; only pursued if per-primitive glow (D7) isn't enough.

---

## 8. Cross-Cutting Conventions

Per `AGENTS.md` and the Rust skill:

- **Idiomatic Rust**: `enum` state machines over bool flags (`InputState`, `Handled`, `Justify`, `Align`); newtypes for ids; builder pattern for widget config; `impl From`/`Display`/`Debug` for conversions; `Result` over panic in fallible paths; `pub(crate)` by default, expose only the surface API.
- **Docs**: `//!` module headers explaining design; `///` on public types/methods; examples on key APIs.
- **DRY**: shared decorators (`corner_brackets`, `glow_border`, `scanline`) live once in `PaintCx`; never duplicate draw patterns.
- **HiDPI**: logical `f32` in the `Scene`; physical scaling + pixel alignment only in the renderer.
- **Hygiene**: `cargo fmt`, `cargo check` before/after, `cargo clippy --workspace --all-targets --all-features` clean. No `#[allow(dead_code)]` without a justifying comment.
- **No registry bypasses / no second WM layout engine** — taffy is component-internal only.

---

## 9. Dependencies to Add

| Crate | Where | Purpose | Notes |
|-------|-------|---------|-------|
| `floem_reactive` | `heca-grid-ui` | Signals/memos/effects | Behind `reactive` facade; renderer-agnostic. |
| `taffy` | `heca-grid-ui` | Flexbox/Grid/Block layout | Component-internal layout only. |
| (none new) | `heca-renderer` | SDF/scanline shaders | Built on existing `wgpu` + `cosmic-text`. |

Pin and verify both against **edition 2024 / rust 1.85**.

---

## 10. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `floem_reactive` API churn / transitive deps | Facade isolates it; can swap to a custom runtime with no component-code change. |
| taffy mistaken for a rival WM engine | Documented distinction in AGENTS.md; taffy never positions panes/columns. |
| `DrawCommand` vocabulary too thin to express effects | Design it rich in Phase B; `Custom` escape hatch for exotic shaders. |
| Per-primitive glow looks weaker than cinematic bloom | Phase E bloom available as escape hatch. |
| Repaint cost if whole tree repaints each frame | Signal dirty-tracking + coalesced `needs_redraw`; only repaint on change. |
| `Color` location coupling | Resolve Q1 early (promote to `heca-core` vs local type). |

---

## 11. Open Questions

- **Q1**: `Color` — promote to `heca-core` and re-export from `heca-config` (cleanest, DRY), or define a local `heca_grid_ui::Color`? *Recommendation: promote to `heca-core`.*
- **Q2**: Reactivity engine confirmed as `floem_reactive`? *Recommended.*
- **Q3**: Showcase as a `heca-grid-ui` example binary, or a `--showcase` flag in `heca`? *Recommendation: standalone example to keep `heca-grid-ui` self-demonstrating.*
