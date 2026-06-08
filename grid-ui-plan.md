# heca-grid-ui — Grid UI Component Library: Plan

> A reusable, signal-driven, composable GPU component library that gives heca a *Tron/GridCN* visual identity.
> Status: **A done · B done · C ~mostly done · D pending** · Last updated: 2026-06-08

---

## ✅ Task Board

**Workflow** (agreed 2026-06-04): one **feature branch per task** off up-to-date `main`; **PR after each task**; mark a task **IN PROGRESS** before starting and **DONE** when finished; on stopping mid-task add a **Resume note** (what/why/agreed/how-to-continue) and delete it once resumed-and-completed.

| Task | Status | PR |
|------|--------|----|
| Phase A — crate skeleton + core model | DONE | — |
| Phase B — renderer (SDF glow) + showcase | DONE | — |
| Phase C — component catalog (Button, Toggle, Checkbox, Input, Tabs, Badge, StatusDot, Separator, Spinner, Alert, ProgressBar, Gauge; `disabled`, `tab_index`, full Input keyboard model) | DONE | [#21](https://github.com/ovidius72/heca/pull/21) ✅ merged |
| Docs reconciliation — `docs/widgets.md` (per-widget API + examples), `AGENTS.md` agent guide, `docs/the-grid-ui.md` reconcile | DONE | [#22](https://github.com/ovidius72/heca/pull/22) |
| Overlay / popover layer (`Scene` overlay layer + `PaintCx::with_overlay` + `Component::overlay_active` + `FocusManager` overlay routing) | DONE | [#23](https://github.com/ovidius72/heca/pull/23) |
| `Select` dropdown (first overlay consumer; `select-change`) | DONE | [#23](https://github.com/ovidius72/heca/pull/23) |
| `Select` scrollable long list (max-visible rows + scrollbar + wheel) & showcase overlap demos | DONE | (extends #23) |
| `Item` widget — generic row (leading · label · trailing slots, selected/hover/activate) for menus/options/sidebar | DONE | [#24](https://github.com/ovidius72/heca/pull/24) |
| `ActiveMarker { None, Bar, Check }` on `Item` — context-driven indicator (Bar=sidebar, Check=menu pip, None=dropdown) | DONE | (extends #24) |
| Bug: dropdown hover bleeds to items below Select panel (`PointerMoved` now routed only to overlay when active) | DONE | (extends #24) |
| Bug: Tabs underline overlaps text on height resize (fixed `flex_shrink: 0` in `style.rs`) | DONE | (extends #24) |
| Pane border: 1px accent at 30% opacity + small radius (4px) | DONE | (extends #24) |
| Theme glow token `GlowLevel { None, Thin, Medium, Large }` — scales every glow's halo radius; `scaled_glow` folds in glow size + intensity | DONE | — (pending PR) |
| Theme-driven radius + border width (`Theme::control_radius()`) across `Input`/`Select`/`Checkbox`/`Item` slot/`Pane`; `border: 0` supported | DONE | — (pending PR) |
| `Pane` rounded corner brackets — radius-matched border + dimmed straight midsections (replaces square `BracketCmd`); active-row inset selection pill | DONE | — (pending PR) |
| Single-select sidebar (immediate-mode shared `active` signals); clicked row highlights, others clear | DONE | — (pending PR) |
| Whole-page scroll in showcase (wheel; natural-height layout + translate; window framebuffer clips) | DONE | — (pending PR) |
| `Select` dropdown flip-up when no room below + viewport-capped internal scroll (`PaintCx::with_viewport`) | DONE | — (pending PR) |
| Live theme controls in showcase (GLOW/RADIUS/BORDER/FONT/INTENSITY selects); re-measuring `font_size()` builders on `Input`/`Select`/`Button`/`Tabs` | DONE | — (pending PR) |
| Wire `Item` into `Select` options (label + `value: SignalData` + slots) | PENDING | — |
| Icon support (icon-font glyphs, no renderer texture work) | PENDING | — |
| `Pane` container — prominent corner brackets, **no glow/shadow**; wraps sidebars/panes (rows = `Item`s) | DONE | (extends #24) |
| `Tooltip`, `Modal`/`Dialog` (need overlay) | PENDING | — |
| Misc widgets: `IconButton`, `Tag`/`Chip`, `EnergyMeter`, `SignalIndicator`, `DataCard`/`Panel`/`Hud` | PENDING | — |
| Phase D — app adoption (default `grid_tron`, real `Sidebar`/`Pane` shells, status/tab bars, intensity action) | PENDING | — |
| Reconcile `docs/the-grid-ui.md` with implemented architecture | PENDING | — |
| **PLANNED** — Multi-select (`Select` multi mode or `SelectMulti`): multiple active rows via `Vec<usize>`, `Check` markers, toggle-on-click semantics | PLANNED | — |
| **PLANNED** — `Item` drag-and-drop reordering (sidebar rows) | PLANNED | — |
| **PLANNED** — `Item` description text (secondary line below label, muted color) | PLANNED | — |
| **PLANNED** — `Item` custom fg/bg colors per row | PLANNED | — |
| **PLANNED** — `Item` progress bar (inline fill strip inside the row) | PLANNED | — |
| **PLANNED** — `Item` custom widget composition (arbitrary child in the label slot) | PLANNED | — |
| Disentangle `intensity` vs `glow_size`: `glow_size` is the **sole** owner of glow (presence + halo size); `intensity` owns only the CRT scanline overlay (Off=0 → Heavy=0.20, no floor). Removed double-scaling in `scaled_glow` | DONE | — (pending PR) |
| **Central font inheritance**: `Style.font_size` is a `0.0`=inherit sentinel + `font_scale` semantic multiplier; `Base.font` resolved by `LayoutEngine.base_font` + a `Component::remeasure()` hook. Every text/actionable widget (Label/Input/Select/Button/Tabs/Item/Badge/Alert/Card) inherits the theme font and re-measures live — no per-widget wiring, no tree rebuild. Explicit `.font_size(x)` overrides | DONE | — (pending PR) |
| Theme radius applies proportionally to Badge + Toggle (pill ×2 mult, clamped) + Card + Alert + ProgressBar; `border: 0` supported | DONE | — (pending PR) |
| `Select` dropdown: font-derived **row height** (no clipping) + **content-adaptive width** (fits widest option at current font, no fixed 200px floor) | DONE | — (pending PR) |
| Showcase window: grip/resize cursor when the pointer is near a window edge/corner (winit `CursorIcon` from edge hit-test) | DONE | — (pending PR) |
| Bug: `Item` keymap-hint slot top-aligned (row was `Align::Stretch`) — now vertically centered (`Align::Center`) | DONE | — (pending PR) |
| Keybindings / `ActionSink` action-registry integration — **design locked, deferred** (see §12); covers all actionable widgets | DEFERRED | — |
| **PLANNED (final)** — Proper documentation pass: `docs/widgets.md` + theme-token reference (`glow_size`, `intensity`, `radius`, `border_width`, `font_size`), config.toml configurability, the `ActionSink` port for the action registry, dropdown flip/scroll + page-scroll behavior | PLANNED | — |

**Resume notes (active):** *none.*

---

## ▶ Resume Here

### 🤝 Handoff — theme-token pass (glow / radius / border / font) + dropdown polish

**Branch:** `heca-grid-ui` → PR into `main`. Build + `cargo test -p heca-grid-ui` are
green (65 tests). `heca-renderer/src/` and `heca`/`heca-core`/`heca-config` were
**not touched** (other devs own them; only `heca-renderer/examples/showcase.rs` is
ours). **Never run `cargo fmt`** (rustfmt 1.9 churns unrelated files).

**What this branch delivered (all DONE this pass, on top of the Item/Pane work):**

1. **Configurable theme tokens, applied uniformly across widgets:**
   - `GlowLevel { None, Thin, Medium, Large }` — glow halo size; sole owner of glow.
   - `intensity` — **CRT scanline overlay only** (Off=0 → Heavy=0.20). Fully
     separate from glow (no more overlap). Visible in the showcase now.
   - `radius` + `border_width` — every box/pill widget reads them. Pills (Badge,
     Toggle, ProgressBar) round at `radius × 2` clamped to their capsule max;
     `border: 0` is honored. `control_radius()` = `radius × 0.5` for small controls.
   - **Font is a theme token, inherited centrally** — see the architecture note below.
2. **Central font architecture** (the important one): `Style.font_size` is a
   `0.0`=inherit sentinel; `Style.font_scale` is a semantic multiplier (header 2.0,
   caption 0.8, etc.); the resolved size lives in `Base.font`. `LayoutEngine` carries
   `base_font` (= `theme.font_size`) and, for every node, resolves
   `explicit ? font_size : base_font × font_scale` → `Base.font`, then calls the new
   `Component::remeasure()` hook (widgets size from `Base.font`). One global font
   change reflows the whole tree live — no per-widget wiring, no rebuild.
3. **Dropdown (`Select`) polish:** opens **up** when no room below; **caps + scrolls**
   internally to the viewport; **row height** derives from font (no clipping);
   **width adapts** to the widest option at the current font (no fixed 200px).
4. **Showcase:** live GLOW/RADIUS/BORDER/FONT/INTENSITY selects (mutate the theme
   each frame); whole-page **scroll** (wheel; window framebuffer clips); grip/resize
   **cursor** near window edges; **single-select** sidebar (immediate-mode shared
   signals).
5. **Docs:** §12 documents the **deferred** keybinding / `ActionSink` action-registry
   design (covers all actionable widgets).

**How to resume / what's next (in rough priority):**

- **PR review feedback**, then merge.
- **Documentation pass** (the only remaining PLANNED task): `docs/widgets.md` +
  theme-token reference + the §12 `ActionSink` design.
- **`ActionSink` keybinding integration** — implement §12 *when Phase D wires these
  widgets into the `heca` app* (add `ActionSink` trait + `action(&str)` builder to
  Button/Checkbox/Toggle/Select/Item/Tabs; adapter in `heca`).
- **Renderer dependency:** an embeddable (non-page) scroll region needs the renderer
  to implement `PushClip`/`PopClip` (currently a no-op in `heca-renderer/src/scene.rs`
  — not ours to edit). Until then, scroll is whole-page only.
- New components backlog + Phase D app adoption (see §11 / Task Board).

**Gotchas for the next session:**
- `Base.font` is the resolved font; widgets must read it (not `style.font_size`) for
  text + `remeasure()`. `style.font_size > 0` means an explicit override.
- Changing `LayoutEngine` font resolution affects sizing of all font-derived widgets;
  the integration tests use *relative* centers (`b.size.h/2`) so they tolerate it.
- Two tests were updated this pass to match new behavior:
  `pane_draws_rounded_accent_border_no_brackets` and `glow_none_suppresses_glow`.

**Older status:** Phases A–C complete (full widget catalog + renderer); overlay layer,
`Select`, `Item`, `Pane` all landed (PRs #21/#23/#24). Next big milestone after this
PR is **Phase D** (app adoption).

**Run / verify:**

```bash
cargo run -p heca-renderer --example showcase          # the live demo (needs a display)
cargo test -p heca-grid-ui                              # 6 unit + 59 integration + doctests
cargo clippy -p heca-grid-ui -p heca-renderer --all-targets   # must be clean
```

**Done (committed):**

- **Phase A** — `heca-grid-ui` crate: `reactive` facade (`floem_reactive`), `taffy` layout, `Scene`/`DrawCommand`, `Component` trait + `Base` (composition), `Style`/`Theme` (dark `grid_tron` default), `Flex`/`Container`/`Label`.
- **Phase B** — `heca-renderer`: SDF rounded-rect pipeline `grid.rs`/`grid.wgsl` with **premultiplied additive glow** (real translucent halo), `scene.rs` bridge (`enqueue_scene`), embedded **Geist Mono Regular+Bold**, bold + **metric-based text centering** in `text.rs`, `examples/showcase.rs`.
- **Phase C (partial)** — builder traits `LayoutExt`/`StyleExt`/`Parent`; `Flex` is layout-only; `Surface`, `Card`, **`Button`** (6 GridCN variants × 3 sizes; animated per-variant hover; tuned glow; per-variant **press `Flash`**; focus-visible ring), **`Toggle`** (sliding switch: accent-filled-on track, light knob + glow; `on_change(Action)`; `disabled`), **`Checkbox`** (box + pop-in glowing accent indicator; `checkbox-change`), **`Input`** (single-line editable buffer, blinking caret, placeholder; `input-change`); display widgets **`Separator`**, **`Badge`** (6 variants), **`StatusDot`**; **`Tabs`** (segmented selector, sliding underline, arrow-key nav; `tab-change`), **`Spinner`** (ring-of-dots brightness sweep via `tick`), **`Alert`** (4 variants, left accent bar + title/body), **`ProgressBar`** (eased signal-driven fill), **`Gauge`** (12-segment energy meter, success→warning→danger).
- **Accessibility** — `GridKey`, `Event::Key`, `FocusManager` (Tab/Shift+Tab + `focus_at` click-focus + wrap), `on_focus`/`on_blur`, Space/Enter activation, **focus-visible** (ring on keyboard focus only), `Theme.show_focus_border`. **`disabled`** widgets are skipped by traversal (`focusable()` returns `!disabled`).
- **Foundations** — `Action`/`SignalData` (`action.rs`) **wired** via `on_change(Fn(Action))` callbacks on change widgets; `Flash` (`effects.rs`, reusable press effect); **`Base.disabled`** common property + `LayoutExt::disabled` builder. `PaintCx::flash`/`PaintCx::dim` both take a **corner `radius`** so overlays follow rounded shapes (no square corners poking past a pill).

**Key files (`heca-grid-ui/src/`):** `component.rs` (Component/Base/PaintCx/Event/GridKey/on_focus/dim), `focus.rs` (FocusManager), `effects.rs` (Flash), `action.rs`, `builders.rs`, `theme.rs`, `style.rs`, `scene.rs`, `layout.rs`, `widgets/{flex,label,surface,card,button,toggle}.rs`. **Renderer:** `heca-renderer/src/{grid.rs,grid.wgsl,scene.rs,text.rs}` + `examples/showcase.rs`.

**Established pattern (Toggle, done):** change widgets flip a `Signal<T>` and fire `on_change(Fn(Action))` carrying `Action::value("…-change", SignalData::…)` with the new value. **Decision:** the `Action` model is wired via a callback (consistent with `Button.on_click`), *not* by changing `event() -> Handled` (which `focus.rs` depends on). All interactive widgets honor `Base.disabled` (dimmed via `PaintCx::dim`, inert, unfocusable) and reuse `Flash` + `focusable()`.

**Change widgets done:** `Toggle`, `Checkbox`, `Input` — all three emit `…-change` Actions via `on_change(Fn(Action))`, honor `disabled`, and are keyboard-operable. `Input` has an editable buffer, a click-to-place + blinking caret (monospace advance), a placeholder, a **multi-click selection cycle** (double=word, triple=all, fourth=clear; typing/backspace replace the selection), **granularity delete** (Ctrl/Alt + Backspace/Delete = word; Cmd/meta + Backspace/Delete = to start/end), and **keyboard selection** (anchor-based): Shift+Arrow = char, Shift+Ctrl/Cmd+Arrow = to start/end, Shift+Alt+Arrow = by word; plain Ctrl/Alt+Arrow move the caret by word/boundary. **Home/End** (+Shift) move/select to start/end; **Cmd/Ctrl+A** selects all (other modified chars are swallowed, not typed). Caret movement + delete share a `Granularity` (Char/Word/Line). Modifiers ride a renderer-agnostic `Modifiers` broadcast via `Event::ModifiersChanged` (host maps platform keys; word-mod = Ctrl **or** Alt for cross-platform Win/Linux + macOS). `Checkbox` has an optional **clickable label** with configurable side (`LabelSide`). Focus traversal honors an optional **`Base.tab_index`** (HTML-like), else tree position.

**Display widgets done:** `Separator` (cross-axis stretch + `.length()`), `Badge` (6 theme-mapped variants, neon chip), `StatusDot` (semantic glowing dot).

**Next steps (in priority order):**

1. **Overlay/popover layer** — the first real infra gap (`Select`/`Tooltip`/`Modal` all need it): paint above the tree + route events to the topmost layer. Design before building `Select`.
2. heca-specific shells: `StatusBar`, `Sidebar`, `Pane` (Phase C5–C7), then Phase D app adoption.
3. Remaining catalog polish: `Tag`/`Chip`, `EnergyMeter`/`SignalIndicator`, `IconButton`, `DataCard`/`Panel`/`Hud`.
4. Then the rest of the catalog (`Badge`, `Tag`, `Chip`, `StatusDot`, `Separator`/`Divider`, `Spinner`, `Tooltip`, `Alert`, `Select`, `Modal`/`Dialog`, `CommandPalette`, `Sidebar`, `StatusBar`, `MenuBar`, …) — full list + GridCN reference links in `docs/the-grid-ui.md`.
5. **Phase D** — app adoption (dark grid theme default, real `Sidebar` + `Pane` shells over the niri layout).

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

- [x] C1. `Button` **done** — 6 GridCN variants (primary/secondary/destructive/outline/ghost/link) × 3 sizes, variant-driven look from theme tokens, hover + click. `Toggle` **done** (sliding switch + `on_change(Action)` + `disabled`). `Checkbox` **done** (box + pop-in accent indicator + `checkbox-change`). `Input` **done** (editable buffer, blinking caret, placeholder, `input-change`). `IconButton` still pending.
- [~] C2. `Surface` (base styled box) + `Card` + `Separator` + `Badge` (6 variants) + `StatusDot` **done**; `DataCard`/`Panel`/`Hud` pending.
- [x] C0. Builder traits `LayoutExt`/`StyleExt`/`Parent` enforcing layout-vs-surface separation; `Flex` made layout-only; showcase migrated to `Card`/`Button` with live hover/click.
- [~] C3. `Gauge` **done** (segmented energy meter) + `ProgressBar` **done** (eased fill); `EnergyMeter`/`SignalIndicator` pending.
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
- **Glow level** configurabel (off, low, medium, hight)
- **NEW COMPONENTS** TO ADD:
  - `HUD Frame` [[https://thegridcn.com/components#hud-frame]]
  - Metric Row (SidebarItem ? [[https://thegridcn.com/components#metric-row]])
  - Modal ([[https://thegridcn.com/components#modal]])
  - Notification and icons ([[https://thegridcn.com/components#notification]])
  - Search Input (for ws filtering) [[https://thegridcn.com/components#search-input]]
  - Workspaces Container ([[https://thegridcn.com/components#command-example]]) seaarch matching ws, cols, panes
  - SidebarItem 1 ([[https://thegridcn.com/components#changelog]])
  - SidebarItem 2 ([[https://thegridcn.com/components#sidebar-nav]])
  - SidebarItem 3 ([[https://thegridcn.com/components#activity-feed]])
  - SidebarItem 4 ([[https://thegridcn.com/components#kanban-board]])
  - Status Dots ([[https://thegridcn.com/components#status-dot]])
  - Tags ([[https://thegridcn.com/components#tag]])
  - Toast ([[https://thegridcn.com/components#toast]]) (Can be used as sidebar item ?)
  - Accordion ([[https://thegridcn.com/components#tron-accordion]]) (worspace and columns container)
  - Command Menu (Palette) [[https://thegridcn.com/components#command-menu]]

---

## 12. Keybindings & the Action Registry (DEFERRED — design locked)

**Status:** design agreed, implementation **deferred** until the `heca` app actually
consumes the widgets (so the port is designed against a real adapter, not an
imaginary one). The kbd-hint chips in the showcase (`CMD P`, `CMD ,`) are
**decorative** until this lands.

**Scope:** this is **not an `Item`-only feature**. It must cover **every actionable
widget** — `Button`, `Checkbox`, `Toggle`, `Select`/dropdown, `Item` (menu/sidebar
rows), `Tabs`, and any future actionable component (Command Menu, Modal buttons,
Search Input submit, etc.).

**Goal:** keybindings are real and configurable through `heca`'s `config.toml`,
routed through the app's action registry, **without coupling `heca-grid-ui` to that
registry**.

### Locked decisions

- **Dependency direction stays one-way:** `heca` → `heca-renderer` → `heca-grid-ui`
  → `heca-core`. `WmAction`, `ActionRegistry`, `KeymapRegistry`, and config.toml
  parsing all live in the top `heca` crate. `heca-grid-ui` never reaches *up* to it.
- **Dependency inversion via an `ActionSink` port** — trait defined in
  `heca-grid-ui`, implemented in `heca`. Widgets speak in **opaque string action
  IDs**, never `WmAction`:

  ```rust
  // heca-grid-ui — generic, no WmAction, no config
  pub trait ActionSink {
      fn dispatch(&self, action: &str);                  // opaque id → effect
      fn shortcut_hint(&self, action: &str) -> Option<String>; // for the kbd chip
  }
  ```
  ```rust
  // heca — the adapter over the real registries
  impl ActionSink for HecaActions {
      fn dispatch(&self, id: &str) { /* id → WmAction → ActionRegistry::execute */ }
      fn shortcut_hint(&self, id: &str) -> Option<String> { /* keymap reverse-lookup */ }
  }
  ```
- **Every actionable widget gains an optional `action: &'static str`** (the opaque
  id) alongside its existing `on_*` closure. The closure stays the simple path; the
  `action` id is the registry path. A widget with an `action` set routes through the
  shared `ActionSink`.
- **grid-ui does NOT match accelerators.** Flow: raw key → `heca` resolves
  `KeyCombo → WmAction` via the keymap → `ActionRegistry::execute` → mutates a signal
  → grid-ui re-renders. The app intercepts e.g. Cmd+P **globally**, before focus
  routing. grid-ui's only shortcut responsibility is **displaying** the hint.
- **The kbd-hint chip text is reverse-looked-up from the keymap** (`shortcut_hint`),
  so `config.toml` is the single source of truth — rebind in config and the chip
  updates, no duplicated strings in widget code.

### When we implement it

1. Add the `ActionSink` trait + an `action(&str)` builder to each actionable widget
   in `heca-grid-ui` (Button, Checkbox, Toggle, Select, Item, Tabs, …).
2. Thread a shared `&dyn ActionSink` (likely `Rc<dyn ActionSink>`) to widgets, or hand
   it in at paint/event time via the host.
3. In `heca`, implement `ActionSink` over `KeymapRegistry` + `ActionRegistry`; wire
   global accelerator dispatch in the input loop.

> Note: `heca-ui` is deprecated and being removed — all of this lands in
> **`heca-grid-ui`**.

