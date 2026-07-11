# heca-grid-ui — Widget Reference

The canonical API reference for the **`heca-grid-ui`** component library: every
foundation type and widget, its properties / methods / events, and runnable
usage examples. (For the Tron/GridCN visual *vision* and component wishlist see
[`the-grid-ui.md`](./the-grid-ui.md); this file documents what is **actually
implemented**.)

`heca-grid-ui` is **GPU-free**: a component tree emits a `Scene` (a display
list of `DrawCommand`s) which `heca-renderer` rasterizes. It is **signal-driven**
(fine-grained reactivity via a `floem_reactive` facade) and **composable**
(every widget embeds a `Base` and implements the `Component` trait).

---

## Table of contents

- [Mental model](#mental-model)
- [Getting started](#getting-started) — depend, build a tree, lay out, paint, render, wire events
- [Foundations](#foundations) — `Base`, `Component`, builder traits, `Style`, [Font sizing](#font-sizing), `Theme`/`GlowLevel`/`Intensity`, `Color`, signals, events, `Action`, `Scene`/`PaintCx`, `Flash`, `Attention`
- [Widgets](#widgets)
  - Layout: [`Flex`/`Container`](#flex--container), [`Surface`](#surface), [`Card`](#card), [`Pane`](#pane), [`Grid`](#grid), [`ScrollRegion`](#scrollregion), [`ScrollBar`](#scrollbar)
  - Text: [`Label`](#label)
  - Interactive: [`Button`](#button), [`IconButton`](#iconbutton), [`Toggle`](#toggle), [`Checkbox`](#checkbox), [`Input`](#input), [`Tabs`](#tabs), [`Select`](#select), [`Item`](#item), [`Row`](#row), [`BadgeButton`](#badgebutton)
  - Display: [`Badge`](#badge), [`StatusDot`](#statusdot), [`Separator`](#separator), [`Spinner`](#spinner), [`Alert`](#alert), [`Toast`](#toast), [`ProgressBar`](#progressbar), [`Gauge`](#gauge), [`Icon`](#icon), [`Tag`](#tag)
  - Chrome (sidebars/docks): [`ItemGroup`](#itemgroup), [`MarkerGroup`](#markergroup), [`DockFrame`](#dockframe), [`ChromeRegion`](#chromeregion), [`RailCell`](#railcell), [`KeyHint`](#keyhint)
  - Overlays: [`Tooltip`](#tooltip), [`Modal`](#modal), [`Dialog`](#dialog), [`CommandPalette`](#commandpalette), [`ToastStack`](#toaststack)
- [Patterns](#patterns) — change events, reactive binding, focus, disabled, custom widgets

---

## Mental model

```
Build a tree         Lay out              Paint                 Rasterize
─────────────        ──────────           ──────────            ──────────
Flex / Card /        LayoutEngine         Component::paint  →   heca-renderer
Button / ...   ───►  .compute(root,  ───► writes DrawCommands   enqueue_scene
(retained tree)      window_size)         into a Scene          → GPU
```

- A **`Component`** owns a **`Base`** (style, computed `bounds`, visibility/disabled/focus
  signals, children) and implements `paint`/`event`/`tick`.
- **Layout** is `taffy` Flexbox at component-internal altitude — it lays widgets *inside*
  a panel; it never positions windows/panes.
- **Reactivity**: a `Signal<T>` is a `Copy` handle; `.get()` subscribes, `.set()` marks
  dirty. The host coalesces dirty signals into a repaint.
- The library **never** touches `wgpu`/`winit`. The host owns the window/event loop and
  feeds the emitted `Scene` to `heca-renderer`.

---

## Getting started

### 1. Depend on it

`heca-grid-ui` is a workspace crate. In a consumer crate's `Cargo.toml`:

```toml
[dependencies]
heca-grid-ui = { path = "../heca-grid-ui" }   # for rendering you also want heca-renderer
```

Pull the common API in via the prelude:

```rust
use heca_grid_ui::prelude::*;   // widgets, builders, Theme, Color, signals, events, TextAlign, Length…
```

The prelude re-exports: all widgets; the builder traits (`LayoutExt`, `StyleExt`,
`Parent`); `Color`; `Component`/`Event`/`GridKey`/`Handled`/`Modifiers`; `FocusManager`;
`signal`/`Signal`/`SignalGet`/`SignalUpdate`; `TextAlign`; `Align`/`Direction`/`Justify`/`Length`;
`GlowLevel`/`Intensity`/`Theme`; and the `Action`/`SignalData` change-event types.

### 2. Build a retained tree

```rust
let theme = Theme::grid_tron();
let mut ui = Flex::column()
    .padding(24.0)
    .gap(16.0)
    .child(Card::new("UPLINK")
        .background(theme.surface)
        .border(theme.accent, 1.5)
        .glow(theme.glow)
        .child(Label::new("ONLINE").color(theme.foreground)))
    .child(Button::primary("DEREZ").on_click(|| println!("clicked")));
```

### 3. Lay out, paint, render — each frame

```rust
use heca_grid_ui::{LayoutEngine, PaintCx, Scene, Size};

// (a) size the root to the window and compute layout. `base_font` is the size
//     every widget inherits (see "Font sizing") — pass theme.font_size so a
//     global font change reflows the whole tree.
ui.base_mut().style.width  = Length::Px(win_w);
ui.base_mut().style.height = Length::Px(win_h);
LayoutEngine::new()
    .base_font(theme.font_size)
    .compute(&mut ui, Size::new(win_w as f64, win_h as f64));

// (b) paint into a fresh Scene. Pass the window size as the viewport so overlay
//     widgets (Select) can flip/cap their popup against the screen edges.
let mut scene = Scene::new();
{
    let mut cx = PaintCx::new(&mut scene, &theme)
        .with_viewport(Size::new(win_w as f64, win_h as f64));
    ui.paint(&mut cx);
}

// (c) hand the Scene to the renderer (heca-renderer)
heca_renderer::scene::enqueue_scene(&mut grid_renderer, &mut text_renderer, &scene);
```

See [`heca-renderer/examples/showcase.rs`](../heca-renderer/examples/showcase.rs) for a
complete winit + wgpu host (`cargo run -p heca-renderer --example showcase`).

### 4. Wire input

The host maps platform keys onto the renderer-agnostic `GridKey`/`Modifiers` and drives
the tree:

```rust
// pointer
ui.event(&Event::PointerMoved   { pos });
focus.focus_at(&mut ui, pos);              // click focuses the hit widget
ui.event(&Event::PointerPressed { pos });

// modifiers (broadcast so text widgets can do word/line editing)
ui.event(&Event::ModifiersChanged(mods));

// keyboard
match key {
    GridKey::Tab    => focus.advance(&mut ui, !mods.shift),  // Shift+Tab = backward
    GridKey::Escape => focus.clear(&mut ui),
    other           => { focus.deliver_key(&mut ui, other); } // → focused widget
}

// animations (hover, caret blink, spinner, sliding underline, …)
let still_animating = ui.tick(dt_seconds);   // request another frame while true
```

---

## Foundations

### `Base`

Embedded by every widget; holds shared state. Access via `component.base()` /
`component.base_mut()`.

| Field | Type | Meaning |
|-------|------|---------|
| `style` | `Style` | Layout + visual style (see [`Style`](#style)). |
| `node` | `Option<taffy::NodeId>` | Layout-tree node (set during `compute`). |
| `bounds` | `Rectangle` | Absolute logical-pixel rect, filled in after layout. |
| `visible` | `Signal<bool>` | Whether the component renders. |
| `disabled` | `Signal<bool>` | Dimmed + inert + skipped by focus. Set via `LayoutExt::disabled`. |
| `focused` | `Signal<bool>` | Holds keyboard focus. |
| `focus_visible` | `Signal<bool>` | Keyboard-vs-mouse focus flag (set by `FocusManager`). Widgets now draw their `focus_ring` whenever `focused`, so the ring shows for both; this flag is retained for widgets that still want a keyboard-only distinction. |
| `tab_index` | `Option<i32>` | Explicit Tab order (HTML-like). Set via `LayoutExt::tab_index`. |
| `children` | `Vec<Box<dyn Component>>` | Child components. |
| `font` | `f32` | **Resolved** font size in logical px, written by the layout pass (see [Font sizing](#font-sizing)). Widgets read **this** for text + measurement, not `style.font_size`. |

### `Component` trait

| Method | Default | Purpose |
|--------|---------|---------|
| `base(&self) -> &Base` | — | Required. |
| `base_mut(&mut self) -> &mut Base` | — | Required. |
| `focusable(&self) -> bool` | `false` | Interactive widgets return `true` (and `!disabled`). |
| `overlay_active(&self) -> bool` | `false` | `true` while the widget owns an open overlay (e.g. a `Select` dropdown), so the host routes input to it first. |
| `paint(&self, cx: &mut PaintCx)` | base chrome + children | Emit `DrawCommand`s. |
| `event(&mut self, ev: &Event) -> Handled` | route to children | Handle input. |
| `remeasure(&mut self)` | no-op | Recompute size from the resolved font (`Base::font`). The layout pass calls it on every node after resolving the font (see [Font sizing](#font-sizing)). Font-sized widgets override it. |
| `on_focus(&mut self, visible: bool)` | set `focused`/`focus_visible` | Gained focus. |
| `on_blur(&mut self)` | clear them | Lost focus. |
| `tick(&mut self, dt: f32) -> bool` | recurse to children | Advance animations; `true` ⇒ animating. |

### Builder traits

Widgets opt into builder methods by implementing the marker trait (zero boilerplate). All
return `Self` for chaining.

**`LayoutExt`** (arrangement — most widgets):

| Method | Effect |
|--------|--------|
| `.direction(Direction)` | Main axis (`Row`/`Column`). |
| `.gap(f32)` | Space between children. |
| `.justify(Justify)` | Main-axis distribution (`Start`/`Center`/`End`/`SpaceBetween`/`SpaceAround`). |
| `.align(Align)` | Cross-axis alignment (`Start`/`Center`/`End`/`Stretch`). |
| `.padding(f32)` | Inner padding (all sides). |
| `.width(Length)` / `.height(Length)` | `Length::Auto` or `Length::Px(f32)`. |
| `.grow(f32)` | Flex-grow factor. |
| `.disabled(bool)` | Dim + make inert + drop from focus order. |
| `.tab_index(i32)` | Explicit Tab order; indexed widgets visited first, ascending. |

**`StyleExt`** (visual decoration — *surfaces only*: `Surface`, `Card`, `Button`):

| Method | Effect |
|--------|--------|
| `.background(Color)` | Fill. |
| `.border(Color, width: f32)` | Outline. |
| `.glow(Color)` | Neon outer glow (default falloff). |
| `.glow_with(Color, radius: f32, intensity: f32)` | Glow with explicit falloff. |
| `.radius(f32)` | Corner radius. |
| `.font_scale(f32)` | Semantic font multiplier vs the inherited base font (header ≈ 2.0, caption ≈ 0.8). See [Font sizing](#font-sizing). |

> Layout-only `Flex` deliberately does **not** implement `StyleExt` — wrap content in a
> `Surface`/`Card` to give it a background.

**`Parent`** (containers): `.child(impl Component + 'static)` appends a child.

### `Style` & layout enums

`Style` fields: `direction`, `justify`, `align` (default `Stretch`), `gap`, `padding`,
`width`/`height` (`Length`), `flex_grow`, `fill`, `border`, `glow`, `accent`, `fg`,
`radius`, `font_size`, `font_scale`. Enums: `Direction{Row,Column}`, `Justify{Start,Center,End,SpaceBetween,SpaceAround}`,
`Align{Start,Center,End,Stretch}`, `Length{Auto,Px(f32)}`.

> `font_size` defaults to `0.0` = **inherit the theme base font**; `font_scale` defaults
> to `1.0`. See [Font sizing](#font-sizing).

### Font sizing

Font size is a **theme token inherited by every widget**, resolved centrally during layout
— there is no per-widget font wiring.

- `Style.font_size` is a sentinel: `0.0` (default) = *inherit*; any `> 0` value = an
  explicit override for that widget.
- `Style.font_scale` (default `1.0`) is a **semantic multiplier** on the inherited base —
  e.g. a header sets `2.0`, a caption `0.8`. Ignored when `font_size` is set explicitly.
- `LayoutEngine::new().base_font(theme.font_size)` supplies the base. For each node the
  layout pass computes `resolved = font_size > 0 ? font_size : base_font × font_scale`,
  writes it to **`Base::font`**, then calls **`Component::remeasure()`** so widgets whose
  dimensions depend on the font (Input/Select height, Button/Tabs/Badge size, Item row,
  Label box) resize. Widgets read `Base::font` (not `style.font_size`) when painting text.

Result: changing `theme.font_size` (and re-running layout) reflows the entire tree live —
no tree rebuild, no per-widget `.font_size(...)` calls. Set `.font_scale(x)` for semantic
hierarchy, or `.font_size(x)` to pin a specific size.

### Size variants

`WidgetSize` is the **discrete size step** a control picks with `LayoutExt::size` — it scales
the inherited font **and** the widget's intrinsic padding / fixed dimensions together, so the
whole affordance grows or shrinks as one. The caller picks a *variant*, never pixels; the
widget owns the resulting geometry.

| Variant | `font_scale` | `pad_scale` | Use |
|---------|-------------|------------|-----|
| `Small` | `0.8` | `0.5` | Compact controls (sidebar, dense toolbars). |
| `Normal` *(default)* | `0.9` | `0.9` | The compact baseline for most controls. |
| `Large` | `1.0` | `1.0` | Roomy controls at the full base font — the historical un-sized look. |
| `Header` | `1.25` | `0.4` | Emphasized header / info-bar action buttons: glyph out-sizes the body text while a snug padding keeps the button cluster tight. |

`font_scale` multiplies the inherited base font (applied centrally in layout); `pad_scale`
multiplies the widget's intrinsic padding in its `remeasure`. `Header`'s padding is
deliberately *below* `Small` so an emphasized icon stays large without turning the cluster
into a row of chunky boxes — this is what the in-pane header action buttons use. Set it per
widget with `.size(WidgetSize::Header)`; **never** hand-compute the icon/cell size in the
caller.

### `Theme`, `GlowLevel` & `Intensity`

Token struct consumed by `PaintCx`. Presets: **`Theme::grid_tron()`** (cyan, dark — the
default) and **`Theme::grid_ares()`** (alternate). Tokens: `background`, `surface`,
`foreground`, `muted`, `border`, `accent`, `glow`, `danger`, `success`, `warning`,
`font_family`, `font_size`, `radius`, `border_width`, `focus_border_width`, `focus_ring`,
`glow_size` (`GlowLevel`), `intensity`, `show_focus_border`, `icon_secondary_alpha`,
`active_wash_alpha`, `card_background_alpha`.

| Token | Type | Drives |
|-------|------|--------|
| `radius` | `f32` | Base corner radius. Boxes use it directly; small controls use `control_radius()` (= `radius × 0.5`); pills (Badge/Toggle/ProgressBar) round at `radius × 2` clamped to their capsule. `0` ⇒ square. |
| `border_width` | `f32` | Decorative border stroke width for every box/pill widget **and** the `Pane`/`bracket_frame` reticle. `0` ⇒ no border anywhere. (App config: global `[appearance] border_width`.) |
| `focus_border_width` | `f32` | Width of the **affordance** outlines — the keyboard focus indicator (`focus_ring`) and selected-item highlight. Independent of `border_width`, so focus/selection stay visible even with borders off. Default `1.5`. (App config: `[appearance] focus_border_width`.) |
| `focus_ring` | `Option<Color>` | Color of the keyboard **focus outline** drawn by `PaintCx::focus_ring` (every widget). Unset ⇒ derived per-tone by `effective_focus_ring()` / `focus_ring_tone()`: the tone (accent, or `danger` for a destructive button) shifted toward `foreground`, which brightens the ring on dark themes and darkens it on light themes so it stays distinct from the widget's own border. Set it to pin the default/accent focus color; the `danger` ring always derives. |
| `glow_size` | `GlowLevel` | The **sole** owner of glow — scales every glow's halo radius. `None` removes glow entirely. |
| `intensity` | `Intensity` | The **CRT scanline overlay** only (no longer touches glow). |
| `font_size` | `f32` | Base font every widget inherits (see [Font sizing](#font-sizing)). |
| `active_wash_alpha` | `f32` (0..1) | Opacity of the accent **wash** `DockFrame::active(true)` paints over an active frame (e.g. the active workspace). |
| `card_background_alpha` | `f32` (0..1) | Opacity of a sidebar/list card's resting background tint (e.g. each pane card). |

Helper: **`theme.control_radius()`** → `radius × 0.5` (corners for small controls).

> **Never hard-code font family/size, radius, border width or glow** — read them from the
> theme so a global change scales every widget proportionally.

- **`GlowLevel{None, Thin, Medium, Large}`** — glow halo size. `.radius_scale()` (0 / 0.5 /
  1.0 / 2.0), `.parse(&str)` (for config.toml), `GlowLevel::ALL`, `.label()`. `None` ⇒ no glow.
- **`Intensity{Off, Low, Medium, Heavy}`** — CRT scanline strength. `.scanline_opacity()`
  (Off=0 → Heavy=0.20), `.next()` (cycles). *Glow and intensity are independent* — glow is
  owned by `glow_size`, so changing intensity affects only the scanline overlay.

### `Color`

`Color { r, g, b, a: u8 }`. Constructors: `Color::rgb(r,g,b)`, `Color::new(r,g,b,a)`,
`Color::TRANSPARENT`; parses hex via `FromStr` (`"#1affd4".parse()`). Helpers:
`.with_alpha(u8)`, `.lerp(other, t)`, `.to_f32x4()`.

### Signals (reactivity)

`signal(value)` creates a `Signal<T>` (a `Copy` handle). `.get()`/`.get_untracked()`
(`SignalGet`), `.set(v)`/`.update(|v| …)` (`SignalUpdate`). Calling `.get()` inside a
`paint` (or effect) subscribes that node so it repaints when the signal changes.

### Events, keys, focus

- **`GridKey`** (renderer-agnostic): `Char(char)`, `Enter`, `Space`, `Tab`, `Escape`,
  `Backspace`, `Delete`, `ArrowLeft/Right/Up/Down`, `Home`, `End`.
- **`Modifiers`** `{ ctrl, alt, shift, meta }` — `meta` is Cmd/Super/Win. The host
  broadcasts changes via `Event::ModifiersChanged`.
- **`Event`**: `PointerMoved{pos}`, `PointerPressed{pos}`, `PointerReleased{pos}`,
  `Key{key, pressed}`, `ModifiersChanged(Modifiers)`, `Scroll{delta}` (wheel — routed to an
  open overlay).
- **`Handled`** `{Yes, No}` — returned by `event`; `Yes` stops propagation.
- **`FocusManager`**: `new()`, `focused() -> Option<usize>`, `advance(root, forward)`
  (Tab/Shift+Tab, wraps, honors `tab_index`), `deliver_key(root, key)` (→ focused widget),
  `focus_at(root, pos)` (click-focus; mouse focus shows no ring), `clear(root)`.

### `Action` / `SignalData` (change events)

Change widgets report the new value through an `on_change(impl Fn(Action))` callback
rather than a return value:

```rust
pub enum SignalData { None, Bool(bool), Int(i64), Float(f64), String(String), Usize(usize) }
pub struct Action { pub name: String, pub data: SignalData }
// Action::new("save")                                  // value-less
// Action::value("toggle-change", SignalData::Bool(true)) // carries the new value
```

| Widget | Action name | Payload |
|--------|-------------|---------|
| `Toggle` | `"toggle-change"` | `Bool` |
| `Checkbox` | `"checkbox-change"` | `Bool` |
| `Input` | `"input-change"` | `String` (full new text) |
| `Tabs` | `"tab-change"` | `Usize` (selected index) |
| `Select` | `"select-change"` | `Usize` (selected index) |

### `Scene` / `DrawCommand` / `PaintCx` (for building widgets)

`paint` receives a `PaintCx` exposing shared Tron drawing helpers (all reused so widgets
stay DRY):

| `PaintCx` method | Draws / does |
|------------------|--------------|
| `.theme() -> &Theme` | Active theme tokens. |
| `.viewport() -> Size` | Visible window size (set by the host via `.with_viewport(size)`); overlay widgets use it to flip/cap their popup. Defaults to "infinite" for headless callers. |
| `.rect(rect, fill, Option<Border>, radius, Option<Glow>)` | Rounded rect + optional border + glow. |
| `.drop_shadow(rect, radius, Shadow)` | Soft **drop shadow** behind a shape (dark, blurred, offset). Darkens the background (reads on dark themes, unlike the additive glow) and is independent of the glow/border tokens. Call before the shape's fill. Used by `Modal` to lift off the scrim. |
| `.focus_ring(rect, color, radius)` | **The** keyboard focus outline for every widget — a thin accent-toned ring drawn *just outside* `rect` (CSS-`outline` style, offset gap), corner radius widened to stay concentric. Visible whether or not the widget has its own border (works on borderless Ghost/Link buttons). Width = `focus_border_width`; halo tracks `glow_size`. Pair with the theme's `focus_ring`/`effective_focus_ring()`/`focus_ring_tone()` for the color. |
| `.bracket_frame(rect)` | Decorative L-shaped corner-bracket reticle (Pane/DockFrame/Modal/Dialog chrome) — **decoration, not focus** (focus uses `.focus_ring`). |
| `.text(rect, &str, color, size, TextAlign, bold)` | Text centered in `rect` (per `align` horizontally, vertically centered). |
| `.flash(rect, amount, radius)` | Brightening press-flash overlay (see `Flash`). |
| `.dim(rect, radius)` | Background scrim — the standard disabled look. |
| `.paint_base(&Base)` | Background/border/glow from a base's style. |
| `.with_overlay(\|cx\| …)` | Route the closure's draws to the scene's **overlay layer** (painted on top of everything) — used by dropdowns/popovers. |

`DrawCommand` variants: `Rect`, `Brackets`, `Text`, `Scanline`, `Gradient`, `PushClip`/`PopClip`
(clip is currently a renderer no-op — embeddable scroll regions wait on it), `Custom`. `Scene`:
`new()`, `push`, `clear`, `len`, `is_empty`, `iter`, plus the overlay layer
(`begin_overlay`/`end_overlay`, `base_layer`/`overlay_layer`).

### `Flash`

A reusable press effect: `Flash::new()` / `Flash::with_duration(s)`; `.trigger()` on press,
`.tick(dt)` each frame (`true` while fading), `.amount()` (0–1) to paint via `cx.flash`.

### `Attention`

A "needs attention" pulse: `Attention::new()`; `.trigger(pulses)` runs a fixed number of
sawtooth flashes (snap to `1.0`, fade to `0.0`, repeat) then stops; `.tick(dt)` (`true` while
pulsing), `.amount()` (0–1), `.is_active()`. Used by [`Row.attention`](#row) — the widget
flashes; the host plays any **sound** (the library is audio-free).

---

## Widgets

Every widget implements `Component` and embeds `Base`, so all support `.disabled(true)`,
`.tab_index(n)`, `.width/.height`, visibility, etc. (via `LayoutExt`). Below, "Builders"
lists widget-specific methods; layout/style builders come from the traits above.

### Flex / Container

Layout-only flexible box. **`Container`** and `container()` are aliases.

- **Construct**: `Flex::row()`, `Flex::column()`, `container()`.
- **Traits**: `LayoutExt`, `Parent`. (No `StyleExt` — it's purely arrangement.)

```rust
Flex::row().gap(12.0).align(Align::Center)
    .child(StatusDot::online())
    .child(Label::new("GRID LINK"));
```

### Surface

A styled box: the base building block for backgrounds/borders/glow.

- **Construct**: `Surface::new()`, `Surface::row()`, `Surface::column()`.
- **Traits**: `LayoutExt`, `StyleExt`, `Parent`.

```rust
Surface::column().padding(16.0).gap(8.0)
    .background(theme.surface).border(theme.accent, 1.5).glow(theme.glow).radius(4.0)
    .child(Label::new("PANEL"));
```

### Card

A titled, padded column surface (header label + body).

- **Construct**: `Card::new(title)`.
- **Traits**: `LayoutExt`, `StyleExt`, `Parent`. Add body content with `.child(...)`.

```rust
Card::new("POWER").background(theme.surface).border(theme.accent, 1.5)
    .child(Label::new("98%").font_scale(2.0));
```

### Pane

A generic container for sidebars/panels with three **frame modes** (`PaneFrame`),
selectable via `.frame(..)` / `.frameless()` / `.bordered()` / `.bracketed()`:

- **`None`** — background fill only.
- **`None`** — background fill only.
- **`Bordered`** (default) — a clean continuous border. Width comes from
  `theme.border_width` (read at paint, so the global border control governs it)
  **unless** an explicit `.border_width(w)` override is set on the pane; color
  from an explicit `.border(color, _)` or else `theme.border`.
- **`Bracketed`** — the self-contained corner-bracket reticle (`PaintCx::bracket_frame`):
  bright rounded `theme.accent` corners over a dimmed continuous line, sharing the
  theme corner radius. It does **not** also draw `style.border`.

Border width follows `theme.border_width` (`0` ⇒ no frame) unless overridden per
pane with `.border_width(w)` — used to let one surface (e.g. a self-themed sidebar
shell) carry its own thickness independent of the global control. Reads
`theme.radius` (or `.radius(r)`) / `theme.border_width` / `theme.accent`. In the
app the frame mode + width + radius are configurable per surface under the nested
appearance tables (`[appearance.pane]` / `[appearance.sidebar]`, each with
`border_style` / `border_width` / `border_color` / `border_radius`), and a
bracketed surface sizes its reticle from the same per-surface width/radius via
`bracket_frame_with`.

- **Construct**: `Pane::new()` (column) / `Pane::row()`.
- **No built-in title.** The pane is a frame + child container only. The app's pane-info **header**
  is composed *inside* the pane top (see the showcase's in-pane info bar demo), so frame decoration
  and the header stay independent — composing widgets beats a bespoke border-straddling title that
  fought the transparent terminal pane. The header is a space-between [`Row`](#flex)/[`Flex`](#flex)
  of a segment [`Tag`](#tag) (left) and an [`IconButton`](#iconbutton) cluster (right) — each button a
  tooltip'd pane action (add-pane / move-left / move-right / close).
- **Traits**: `LayoutExt`, `StyleExt`, `Parent`.

```rust
Pane::new().width(Length::Px(320.0)).gap(2.0).background(theme.surface)
    .child(Item::new("DASHBOARD").marker(ActiveMarker::Bar).active(true))
    .child(Item::new("SETTINGS").marker(ActiveMarker::Bar));

// In-pane info bar: segments (left) + action buttons (right), inside the pane top.
Pane::new().bordered().background(theme.surface).border(theme.accent, 2.0).padding(8.0)
    .child(
        Flex::row().justify(Justify::SpaceBetween).align(Align::Center)
            .child(
                Tag::new("Neovim")
                    .leading(Icon::new(Glyph::FileCode).size(13.0).color(theme.accent))
                    .segment(/* branch · diff-stat … */),
            )
            .child(
                Flex::row().gap(2.0)
                    .child(Tooltip::new(IconButton::new(Icon::new(Glyph::FolderSimplePlus)), "Add pane"))
                    .child(Tooltip::new(IconButton::new(Icon::new(Glyph::FolderSimpleMinus).color(theme.danger)), "Close")),
            ),
    );
```

### Grid

CSS-grid layout (taffy `display: grid`): explicit column/row tracks, named template areas, and
per-child placement. Pure layout (no styling) — the building block for rich composed rows.

- **Construct**: `Grid::new()`.
- **Builders**: `.columns([Track])`, `.rows([Track])` (`Track::{Px(f32), Fr(f32), Auto,
  MinContent, MaxContent}`); `.areas(["a b", "a c"])` named template areas; `.area(child,
  "name")` places a child in an area; `.cell(child, col, row, col_span, row_span)` explicit
  placement.
- **Traits**: `LayoutExt`, `Parent`.

```rust
// icon · title · tag on the top row; subtitle under the title
Grid::new()
    .columns([Track::Px(22.0), Track::Fr(1.0), Track::Auto])
    .rows([Track::Auto, Track::Auto])
    .areas(["dot title tag", ".  sub   ."])
    .gap(4.0)
    .area(Icon::new(Glyph::Terminal), "dot")
    .area(Label::new("nvim"), "title")
    .area(Badge::success("RUN"), "tag");
```

### ScrollRegion

An embeddable **vertical scroll viewport**: a column of children laid out at their
natural height (the layout engine never shrinks them, so the column overflows),
clipped to the region's own bounds. The visible window is the `ScrollRegion`
itself; content beyond it is clipped (`PushClip`). It is a **dumb viewport** —
it owns no selection state; selection/cursor is the host container's concern,
and the region just scrolls where it's told (see *Real-app integration* below).

**Mechanism — same as the whole-page scroll.** Rather than a separate
translation layer, `ScrollRegion` reuses the page-scroll pattern: it bakes
`-scroll_offset` into its children's **bounds** (so paint, hit-testing, and DnD
all see the *visual* position — bounds === what's drawn) and clips to its own
rect via `PushClip` (a sub-region has no framebuffer, so it needs an explicit
clip). Because bounds always match the visual, pointer routing and the drag
framework's `source_at`/`resolve_at` (which hit-test against bounds) just work
while scrolled. A fresh layout pass would compound the shift, so the layout
engine's post-order `on_layout` hook resets the baked offset (children are back
at natural) and re-applies it from scratch — no compounding across relayouts.

- **Construct**: `ScrollRegion::new()`. Append children with [`Parent::child`].
  Give it a fixed `.height()` (and usually `.width()`) via `LayoutExt` so the
  content actually overflows; otherwise it sizes to its children and never
  scrolls.
- **Builders**: `LayoutExt`, `Parent`.
- **Scroll position**: `.scroll_offset() -> Signal<f32>` (read from the host);
  `.scroll_to(f32) -> f32` (clamped to `[0, max_offset]`, **bakes the shift into
  bounds immediately**, requests a repaint, returns the applied value). Prefer
  `scroll_to` over raw `scroll_offset().set()` — it keeps the shifted bounds
  (paint/hit-testing/DnD) in sync with the offset in the same call.
- **Scroll-into-view** (for keyboard cursor following): `.ensure_visible(rect)`
  scrolls minimally so a descendant's current `bounds` (visual space, read
  straight off the component) is fully inside the viewport — above → align tops,
  below → align bottoms, already visible → no-op. `.scroll_to_child(index)` is
  the convenience for a flat list whose selectable units are direct children.
  The widget recovers natural positions internally via its baked shift
  (`applied_offset`), so the host never tracks the scroll offset or does offset
  math. Minimal movement — it won't jump if the item is already on screen.
- **Wheel** (built-in): advances the offset by ~10% of the viewport per notch
  (viewport-proportional, so a small sidebar doesn't overshoot). **Hover-gated**:
  `Event::Scroll` carries no position, so the region tracks the cursor via
  `PointerMoved` and only swallows the wheel when hovered (and scrollable);
  otherwise the event propagates so the host page (or a nested region) can
  scroll. Single inline region only — nested scroll regions need host-side
  hit-testing (future).
- **Keyboard** (built-in, focus-gated): the region is `focusable()`, so click it
  or Tab to it to focus. `Event::Key` is delivered to the **focused component
  only** (`FocusManager`), so the gate is simply `focused` — no broadcast-key
  ambiguity. When focused: `ArrowUp`/`ArrowDown` and `j`/`k` (with or without
  `Ctrl`) move by one step (~10% of the viewport, matching the wheel), `Home`/
  `End` jump to top/bottom. A `focus_ring` (theme-derived accent outline, gated
  by `focused` + `show_focus_border`) shows which region receives the keys.
  A focused **child** (e.g. an `Input`) receives its keys directly via its own
  `event` and never has them stolen. PageUp/PageDown are future work (`GridKey`
  has no page keys yet).
- **Scrollbar thumb** (built-in): auto-shown when content overflows; **draggable**.
  A theme-**accent** grip that brightens on hover/drag (mirroring `MarkerGroup`'s
  grip bar), sitting in a wider invisible **grab lane** (16px) so the thin 8px
  thumb is easy to click. The thumb radius reads the `Theme::control_radius()`
  token (no hardcoded radius); the thumb color is `theme.accent` (no hardcoded
  color). The affordance alphas (rest/hover) are widget-internal constants,
  consistent with `MarkerGroup`.
- **Traits**: `LayoutExt`, `Parent`.
- **v1 scope**: vertical-only. Horizontal scroll, a dedicated scrollbar color
  token, PageUp/PageDown keys, and nested-region hit-testing are future work.

```rust
let mut list = ScrollRegion::new()
    .height(Length::Px(180.0))
    .width(Length::Px(300.0));
for i in 1..=25 {
    list = list.child(Item::new(format!("item {i:02}")));
}
// Drive from the host (a “jump to top” action):
list.scroll_to(0.0);
```

> **Real-app integration (sidebar):** selection is container-owned, not widget
> state. Mount the sidebar tree (DockFrames + rows) inside a `ScrollRegion`; the
> existing `SidebarNav` cursor handler (`j`/`k`, selection-driven) gains one line —
> after moving the cursor, call `region.ensure_visible(selected_row.bounds)` (or
> `scroll_to_child` for a flat list) to keep the cursor on screen. The selected
> row's visual state (accent bar) stays container-driven via `Item::marker`/
> `state`. See [`heca-renderer/examples/showcase.rs`](../heca-renderer/examples/showcase.rs)
> for the wheel/thumb/keyboard demo.
>
> **Host wiring:** route keys through `FocusManager::deliver_key` (keys go to the
> focused component only — that's the whole gate), and pointer/wheel through
> `FocusManager::dispatch`. Note: in the showcase `Ctrl+K` is host-bound to the
> command palette, so use `k`/`Ctrl+J`/arrows there; the chord is configurable in
> the app.

### Label

A single text run bound to a `Signal<String>`.

- **Construct**: `Label::new(text)`.
- **Builders**: `.align(TextAlign)`, `.color(Color)`, `.font_size(f32)` (pin a size),
  `.font_scale(f32)` (multiplier vs the inherited base font — prefer this for hierarchy),
  `.bold(bool)`.
- **Accessor**: `.text_signal() -> Signal<String>` (set it to update reactively).

```rust
let status = Label::new("ONLINE").color(theme.foreground).font_size(14.0);
let sig = status.text_signal();
// later: sig.set("OFFLINE".into());
```

### Button

Interactive surface; look driven by variant × size, with animated per-variant hover and a
press flash. Focusable; Space/Enter activate like a click.

- **Focus indicator**: like every widget, Button draws `PaintCx::focus_ring` — a thin accent-toned
  **outline just outside** the button (so it shows even on borderless `Ghost`/`Link`), tinted by the
  theme's `focus_ring` token / `effective_focus_ring()` for accent variants and
  `focus_ring_tone(danger)` for `Destructive`. Shown whenever the button is `focused` (not
  keyboard-only).
- **Construct**: `Button::new(label)` (= primary) or `Button::{primary,secondary,destructive,outline,ghost,link}(label)`.
- **Builders**: `.variant(ButtonVariant)`, `.size(ButtonSize)` (`Small`/`Medium`/`Large` — a
  font multiplier on the base font + padding), `.font_size(f32)` (pin an explicit size),
  `.glow(bool)`, `.bordered(bool)`, `.on_click(impl Fn() + 'static)`.
- **Accessor**: `.hovered() -> Signal<bool>`.
- **Variants**: `Primary`, `Secondary`, `Destructive`, `Outline`, `Ghost`, `Link`.

```rust
Button::destructive("DEREZ")
    .size(ButtonSize::Large)
    .on_click(|| wm.derez_focused());
```

### IconButton

The icon-only cousin of `Button` — a compact, clickable icon affordance for toolbars/headers.
Ghost at rest (just the icon); an animated tone-tinted hover frame (+ optional glow) fades in,
with a press flash and keyboard focus ring. Hugs its icon + padding by default; pin a square
with `.cell(px)`. Focusable once `.on_click(...)` is set.

- **Construct**: `IconButton::new(Icon)`.
- **Builders**: `.cell(px)` (pin a square), `.size(WidgetSize)` (size variant — see
  [Size variants](#size-variants); `Header` is the emphasized one for info-bar / pane-header
  buttons), `.tone(Color)` (hover/press hue, default accent),
  `.glow(bool)`, `.active(bool)`, `.on_click(impl Fn() + 'static)`.
- **Header buttons**: for an emphasized icon in a pane/info-bar header, use
  `IconButton::new(Icon::new(g).color(c)).size(WidgetSize::Header)` — **no** explicit icon px
  and **no** `.cell(...)`. The button self-sizes from the variant (glyph `1.25×` the bar font
  + a snug cluster padding) so the whole cluster scales with the bar font. Never hand-compute
  the icon/cell size in the caller.
- **`.active(true)`**: held-on (toggled) status — a persistent tone-tinted fill + firm border
  (the held version of the hover frame, matching the `Toggle` on-state), so the button reads as
  an active *status* not a passive icon. Hover/press still layer on top. Used by the in-pane
  zoom/float buttons when their column is zoomed / the pane is floating.
- **Accessor**: `.hovered() -> Signal<bool>`.

```rust
IconButton::new(Icon::new(Glyph::Close).color(theme.danger).size(20.0))
    .tone(theme.danger)
    .on_click(|| close());
```

### Toggle

Sliding on/off switch (translucent accent fill + light knob + glow when on). Focusable;
Space/Enter or click flips it.

- **Construct**: `Toggle::new()` (off).
- **Builders**: `.on(bool)` (initial state), `.on_change(impl Fn(Action))`.
- **Accessors**: `.state() -> Signal<bool>`, `.is_on() -> bool`.
- **Emits**: `"toggle-change"` / `SignalData::Bool`.

```rust
Toggle::new().on(true).on_change(|a| {
    if let SignalData::Bool(on) = a.data { set_grid_uplink(on); }
});
```

### Checkbox

Bordered box with a pop-in accent indicator, plus an optional **clickable label** on
either side. Focusable; Space/Enter or a click anywhere on box/label toggles it.

- **Construct**: `Checkbox::new()`.
- **Builders**: `.checked(bool)`, `.label(text)`, `.label_side(LabelSide)` (`Right` default,
  `Left`), `.on_change(impl Fn(Action))`.
- **Accessors**: `.state() -> Signal<bool>`, `.is_checked() -> bool`.
- **Emits**: `"checkbox-change"` / `SignalData::Bool`.

```rust
Checkbox::new().checked(true).label("ENCRYPT")
    .on_change(|a| println!("{a:?}"));
Checkbox::new().label("LEFT LABEL").label_side(LabelSide::Left);
```

### Input

Single-line editable text field with a blinking caret, placeholder, and a full
mouse/keyboard selection + editing model. Focusable.

- **Construct**: `Input::new()`.
- **Builders**: `.value(text)` (initial), `.placeholder(text)`, `.font_size(f32)` (else inherits;
  field height tracks the font), `.on_change(impl Fn(Action))`. Plus the shared `LayoutExt`:
  `.disabled(bool)` (dims + inert + unfocusable), `.tab_index(i32)`, `.width/.height`.
- **Accessors / mutators**: `.text() -> Signal<String>`, `.value_str() -> String`,
  `.set_value(text)` (replace the content imperatively), `.selection() -> Option<(usize,usize)>`,
  `.selected_text() -> Option<String>`.
- **Emits**: `"input-change"` / `SignalData::String` (the full new text) on every edit.

> **Focus + keys come from the container.** The Input consumes keys **only when focused** and the
> host delivers them (e.g. a [`FocusManager`](#), or a [`Dialog`](#dialog) which routes keys
> **field-first**). It reads modifiers from the broadcast `Event::ModifiersChanged`, so put an Input
> in a Dialog body and it types + selects with the full model below — **no re-declaration**.

**Keyboard model** (modifier = Ctrl on Win/Linux, Option/Alt or Cmd on macOS, as noted).
The **built-in** keys are handled by the widget from a raw `Event::Key`:

| Keys | Action |
|------|--------|
| printable / Space | insert at caret (replaces selection) |
| ← / → | move caret by char |
| Ctrl/Cmd + ← / → | move caret to start / end |
| Alt + ← / → | move caret by word |
| Home / End | caret to start / end |
| Shift + (any of the above) | extend/shrink selection the same distance |
| double / triple click | select word / select all (4th click clears) |
| Backspace / Delete | delete char before / after caret (or the selection) |
| Ctrl/Alt + Backspace/Delete | delete word |
| Cmd/meta + Backspace/Delete | delete to start / end |

**Configurable editing shortcuts (`widget-keys-config`).** The readline / select-all shortcuts are
**not** hardcoded — the widget ignores a modified char and instead consumes the semantic
`Event::InputEdit(InputEdit::{DeleteBackward, DeleteToLineStart, SelectAll})`. The **host** resolves
these from configurable `[keys]` bindings and delivers them field-first (through a `Dialog` to the
focused field):

| Intent | Default binding (config name) | Effect |
|--------|-------------------------------|--------|
| `InputEdit::DeleteBackward` | `Ctrl+h` (`input_delete_back`) | delete one char before the caret |
| `InputEdit::DeleteToLineStart` | `Ctrl+u` (`input_delete_to_line_start`) | delete from the caret to line start |
| `InputEdit::SelectAll` | `Ctrl+a` / `Super+a` (`input_select_all`) | select the whole field |

App wiring: `build_input_keymap` → `AppState.input_keymap`, consumed in the overlay key branch
(`heca/src/app/events.rs`). See the `widget-keys-config` requirement and README.

```rust
let name = Input::new().placeholder("CALLSIGN")
    .on_change(|a| { if let SignalData::String(s) = a.data { store(s); } });
```

**From a plugin (`ViewNode`).** A plugin never sends `InputEdit` itself — it declares an `Input`, and
the host owns the keyboard model + shortcut resolution above. (See the plugin props/events under the
`ViewNode` note below.)

**From a plugin (`ViewNode`).** Declare an input in a modal / panel body; the host `realize`s it to
this widget and owns styling + the whole keyboard model above. Supported props / events:

| Prop / event | Meaning |
|---|---|
| `.text(s)` (`"text"` prop) | initial value |
| `.prop("name", PropValue::Text("field".into()))` | opts the field into **form submission** — its live value is returned in `ModalResult::Action.data["field"]` when the overlay is submitted |
| `.on("change", Intent)` | intent dispatched (with the new text) on every edit |

```rust
// A rename field inside a modal body — pre-filled + submitted under "name".
ViewNode::new(WidgetKind::Input)
    .text(current_name)
    .prop("name", PropValue::Text("name".into()))
    .on("change", Intent::new("plugin.rename.changed"));
```

### Tabs

Horizontal segmented selector with an animated sliding underline; lays its own segments
from monospace metrics (no child components). Focusable; ←/→ move selection, click selects.

- **Construct**: `Tabs::new(labels)` — `labels: impl IntoIterator<Item = impl Into<String>>`.
- **Builders**: `.selected(index)` (initial, clamped), `.font_size(f32)` (else inherits;
  strip re-measures), `.on_change(impl Fn(Action))`.
- **Accessors**: `.state() -> Signal<usize>`, `.index() -> usize`.
- **Emits**: `"tab-change"` / `SignalData::Usize`.

```rust
Tabs::new(["OVERVIEW", "SIGNALS", "LOGS"]).selected(0)
    .on_change(|a| if let SignalData::Usize(i) = a.data { show_tab(i); });
```

### Select

Single-select dropdown — the first **overlay** widget. The trigger shows the current value;
the open option list paints in the scene's overlay layer (on top of everything) and the
widget reports `overlay_active()` so the host routes input to it first (see
[Overlay layer](#scene--drawcommand--paintcx-for-building-widgets)). Focusable; self-contained
(no child components). The trigger **width adapts** to the widest option at the current font;
both trigger and rows scale with the font. The open panel **flips above** the trigger when
there's no room below, **caps** its visible rows to what fits in the `PaintCx` viewport, and
**scrolls** internally (scrollbar; wheel / keyboard) for longer lists.

- **Construct**: `Select::new(options)` — `options: impl IntoIterator<Item = impl Into<String>>`.
- **Builders**: `.selected(index)` (initial, clamped), `.font_size(f32)` (else inherits),
  `.on_change(impl Fn(Action))`.
- **Accessors**: `.state() -> Signal<usize>`, `.index() -> usize`, `.selected_label() -> &str`.
- **Emits**: `"select-change"` / `SignalData::Usize`.
- **Keys**: ↑/↓ move highlight (scroll into view), Enter/Space open & commit, Esc closes;
  click a row to choose, click outside to close. Wheel scrolls the open list.

```rust
Select::new(["LOW", "MEDIUM", "HIGH"]).selected(1)
    .on_change(|a| if let SignalData::Usize(i) = a.data { set_level(i); });
```

> **Host wiring**: route pointer + `Esc` + wheel to `FocusManager::deliver_to_overlay` when
> `overlay_active()` (see [`heca-renderer/examples/showcase.rs`](../heca-renderer/examples/showcase.rs)).

### Item

Generic list/menu/sidebar **row**: an optional leading slot, a label, and an optional trailing
slot, with hover, active (selected), and click-to-activate states. Slots accept any component
(a `StatusDot`, a kbd-hint `Label`, a `>` chevron, …). Row height tracks the font; the
active/hover highlight is an inset pill (rounds with the theme radius, so it tucks inside a
rounded `Pane`). Becomes focusable/clickable once `.on_activate(...)` is set.

- **Construct**: `Item::new(label)`.
- **Builders**: `.leading(impl Component)`, `.trailing(impl Component)`,
  `.leading_bordered(bool)` / `.trailing_bordered(bool)` (chip frame around a slot),
  `.active(bool)`, `.marker(ActiveMarker)`, `.muted(bool)` (section-header look),
  `.font_size(f32)`, `.on_activate(impl Fn() + 'static)`.
- **Accessors**: `.state() -> Signal<bool>` (active), `.label_signal() -> Signal<String>`.
- **`ActiveMarker{None, Bar, Check}`** — how the active state reads: `Bar` = vivid left bar
  (sidebar), `Check` = small pip (menu), `None` = tinted bg + accent label only (dropdown).

```rust
// single-select sidebar: share each row's `active` signal, set on click
let rows = ["DASHBOARD", "PROFILE", "SETTINGS"];
// Item::new(label).marker(ActiveMarker::Bar).leading(StatusDot::online())
//     .on_activate(move || select(i))   // host flips this row's state on, others off
```

> **Single-select pattern**: `Item` deliberately does **not** self-select — clicking only
> fires `on_activate`, so a single source of truth can own which row is active (set the
> clicked row's `state()` to `true`, the rest to `false`). This keeps multi-select possible.

### Row

`Item`'s open cousin: the same interactive chrome (hover tint, active/selected pill +
`ActiveMarker`, press flash, focus ring, `on_activate` on click / Enter / Space) wrapped around
**any** children — e.g. a multi-line [`Grid`](#grid) of `Label`/`Icon`/`Badge`. Use it for rich,
clickable, selectable Dock rows. The active/hover highlight derives from the row's own
background (a stronger same-hue tint) so a state-tinted row never gets a clashing accent overlay.

- **Construct**: `Row::new()`. Add content with `.child(...)`.
- **Builders**: `.on_activate(impl Fn())` (also makes it focusable), `.active(bool)`,
  `.nav_selected(bool)` (a hollow same-hue **outline** for the sidebar-nav cursor — shown
  distinctly from the filled `active` pill; a row can show one, the other, or both),
  `.marker(ActiveMarker)`, `.highlight(Color)` (override the derived hue),
  `.attention(Signal<bool>)` + `.attention_color(Color)`, plus `StyleExt` for a persistent
  background under the selection overlay.
- **Accessors**: `.state() -> Signal<bool>` (active), `.nav_state() -> Signal<bool>` (nav
  cursor) — bind either so the host flips it in place without a rebuild.
- **Accessors**: `.state() -> Signal<bool>` (active).
- **Attention**: when the host sets the bound `attention` signal `true`, the row flashes a few
  times (see [`Attention`](#attention)) and consumes the signal. The host plays any **sound** —
  the library is audio-free.

```rust
let row = Row::new().background(color.with_alpha(22)).radius(theme.control_radius())
    .child(/* a Grid of icon + title + Tag + Badge */)
    .on_activate(move || select(i));
```

### ScrollBar

Standalone **vertical scrollbar** widget. The host supplies a **total content
extent**, **visible viewport extent**, and current **offset from the top**; the
widget renders a thumb and reports new offsets as the user clicks or drags.
Unlike [`ScrollRegion`](#scrollregion), it does **not** own children or clip
content — it is just the control.

- **Construct**: `ScrollBar::new()`.
- **Builders**: `.on_change(|Action| ...)` — emits
  `Action::value("scrollbar-change", SignalData::Float(offset_from_top))`.
- **Signals**: `.content_extent_signal()`, `.viewport_extent_signal()`,
  `.offset_signal()`.
- **Look**: same accent thumb language as `ScrollRegion` (thin glowing grip,
  brighter on hover/drag).
- **Use when**: the app already owns scrolling state and only needs a generic
  drag/click thumb to control it.

```rust
let mut bar = ScrollBar::new().height(Length::Px(180.0));
bar.content_extent_signal().set(240.0);   // total rows / px / items
bar.viewport_extent_signal().set(48.0);  // visible rows / px / items
bar.offset_signal().set(96.0);           // offset from TOP
```

### Badge

Self-sizing neon pill for status/metadata (display-only).

- **Construct**: `Badge::new(label)` (= accent) or `Badge::{accent,neutral,success,warning,danger,outline}(label)`.
- **Builders**: `.variant(BadgeVariant)` (`Accent`/`Neutral`/`Success`/`Warning`/`Danger`/`Outline`).

```rust
Badge::success("ONLINE");  Badge::outline("BETA");
```

### BadgeButton

Clickable badge/chip — the visual language of [`Badge`](#badge), but interactive
like a button (hover tint, press flash, focus ring, `Enter`/`Space` activation).
Useful for compact in-pane controls such as “N lines above”, filters, or small
mode toggles.

- **Construct**: `BadgeButton::new(label)` (= accent) or
  `BadgeButton::{accent,neutral,success,warning,danger,outline}(label)`.
- **Builders**: `.variant(BadgeVariant)`, `.on_click(f)`.
- **Signals**: `.label_signal()` (live text), `.hovered()`.

```rust
BadgeButton::accent("144 lines above").on_click(|| jump_to_live_bottom());
```

### StatusDot

Tiny glowing status dot in a semantic color (display-only).

- **Construct**: `StatusDot::new(DotStatus)` or `StatusDot::{online,warning,error,offline}()`.
- **`DotStatus`**: `Online` (success), `Warning`, `Error` (danger), `Offline` (muted, no glow).

```rust
Flex::row().gap(8.0).align(Align::Center)
    .child(StatusDot::online()).child(Label::new("UPLINK"));
```

### Separator

Thin divider line (display-only). Spans the container cross-axis under the default
`Align::Stretch`.

- **Construct**: `Separator::horizontal()`, `Separator::vertical()`.
- **Builder**: `.length(f32)` — force an explicit span when the parent centers instead of stretching.

```rust
Separator::horizontal().length(420.0);
```

### Spinner

Indeterminate loading ring (dots with a rotating brightness sweep). Animated — return its
`tick` to keep requesting frames.

- **Construct**: `Spinner::new()`.

### Alert

Callout surface: colored left bar + title + optional body, themed by variant (display-only).

- **Construct**: `Alert::new(title)` (= info) or `Alert::{info,success,warning,danger}(title)`.
- **Builders**: `.variant(AlertVariant)` (`Info`/`Success`/`Warning`/`Danger`), `.body(text)`.

```rust
Alert::warning("LINK UNSTABLE").body("retrying handshake…");
```

### Toast

A compact **notification card**: a bracket-framed surface (the shared Pane/Modal reticle frame)
with a severity-toned leading `Icon`, a strong title, optional small body, an optional inline
**action** button, and an optional **×** dismiss. **Presentation only** — it holds no queue,
timer, or global state; the host app owns lifecycle (when it appears, how long it lives,
auto-dismiss policy, sound) and reacts to the reported interactions. An ordinary in-tree
component (not overlay-drawn), so it is equally usable **inline** — e.g. a notification row in a
sidebar. Severity maps to theme tokens, never literals.

- **Construct**: `Toast::new(title)` (= info) or `Toast::{info,success,warning,danger}(title)`.
- **Builders**: `.severity(ToastSeverity)`, `.icon(Glyph)` / `.no_icon()`, `.body(text)`,
  `.action(label, on_click)`, `.dismissible(bool)` (default `true`).
- **Callbacks** (the host removes the toast / runs the effect): `.on_dismiss(f)` (×),
  `.on_action(f)` (via `.action(..)`), `.on_click(f)` (whole card — also makes it focusable;
  Enter/Space activates).

```rust
Toast::danger("Connection lost")
    .body("Reconnecting to the grid…")
    .action("Retry", || retry())
    .on_dismiss(|| dismiss(id));
```

### ProgressBar

Determinate progress track whose accent fill eases toward a value via `tick`.

- **Construct**: `ProgressBar::new()` (0).
- **Builders**: `.value(f32)` (initial, clamped 0–1).
- **Live update**: `.set(f32)` (animates), `.state() -> Signal<f32>`.

```rust
let bar = ProgressBar::new().value(0.4);
let v = bar.state();           // bind reactively, or:
bar.set(0.8);                  // animate to 80%
```

### Gauge

Segmented Tron energy meter; lit segments grow with the value and shift colour
(success → warning → danger).

- **Construct**: `Gauge::new()` (0).
- **Builders**: `.value(f32)` (clamped 0–1).
- **Live update**: `.set(f32)`, `.state() -> Signal<f32>`.

```rust
Gauge::new().value(0.85);
```

### Icon

A single **duotone** glyph from the embedded Phosphor Duotone font. Renders two stacked layers
— a dimmed *secondary* wash + a full-strength *primary* — in the same hue. Colors are
theme-driven (primary defaults to the foreground; secondary = primary at
`theme.icon_secondary_alpha`), never baked in. Square, font-sized.

- **Construct**: `Icon::new(Glyph)`.
- **Builders**: `.size(px)`, `.color(Color)` (primary), `.secondary_color(Color)`.
- **`Glyph`**: the curated icon set. **`Glyph::ALL` is the authoritative, enumerable list** —
  the showcase (`cargo run -p heca-renderer --example showcase`) renders every glyph by iterating
  it, and `Glyph::secondary()` gives each one's Phosphor Duotone codepoint. To **add** a glyph:
  add the variant, its codepoint in `secondary()`, and the variant to `ALL` (a unit test enforces
  unique codepoints). Enum names are heca-local and sometimes differ from the Phosphor icon name
  (shown in parentheses below only when they differ):

  > `Folder`, `FolderOpen`, `File`, `FileCode`, `GitBranch`, `GitCommit`, `GitMerge`,
  > `GitPullRequest`, `Terminal` (`terminal-window`), `Gear` (`gear-six`),
  > `Search` (`magnifying-glass`), `Close` (`x`), `Check`, `CaretRight`, `CaretDown`, `Play`,
  > `Pause`, `Stop`, `Warning`, `WarningCircle`, `Info`, `Circle`, `Lightning`, `List`,
  > `Sidebar` (`sidebar-simple`), `DotsThreeVertical`, `ArrowRight`, `ArrowLineLeft`,
  > `ArrowLineRight`, `Plus`, `Minus`, `SquareSplitVertical`, `XSquare`, `FrameCorners`, `Cards`,
  > `Pencil`, `NotePencil`, `Backspace`, `Trash`, `XCircle`, `PlusCircle`, `FolderSimpleMinus`,
  > `FolderSimplePlus`, `PlusSquare`, `StackPlus`, `StackMinus`, `ColumnsPlusLeft`,
  > `ColumnsPlusRight`, `SquareHalf`, `SquareSplitHorizontal`, `SquareHalfBottom`

```rust
Icon::new(Glyph::GitBranch).color(theme.warning).size(18.0);
// Render the whole set (what the showcase does):
for &g in Glyph::ALL { /* Icon::new(g) … */ }
```

### Tag

A bordered metadata chip — git branch / path / filter, hue-configurable and domain-neutral.
**Multi-segment**: chain `.segment_*` to render thin-divided sections (e.g. `path │ ⎇ main │ 5
+152 -12`). Theme-driven radius + border; generous per-axis padding. In a `Grid` cell, wrap in
`Flex::row().child(tag)` so it hugs its content instead of stretching.

- **Construct**: `Tag::new(label)`.
- **Builders**: `.leading(impl Component)` (e.g. an `Icon`), `.segment(impl Component)` /
  `.segment_text(label, Option<leading>)` (add a divided segment), `.append_to_segment(idx, impl
  Component)` (append **inside** an existing segment — no new segment, no divider — e.g. a small
  dimmed suffix next to a label, as the pane info bar does for a renamed pane's `(process)` label),
  `.color(Color)` (hue).

```rust
Tag::new("main").leading(Icon::new(Glyph::GitBranch).size(13.0))
    .segment_text("5 +152 -12", None).color(theme.warning);
```

### Visibility

A signal-driven wrapper that hides or shows exactly one child without rebuilding the tree.
Use it for optional metadata rows and exceptional-state indicators that should collapse
cleanly when absent.

- **Construct**: `Visibility::new(child, visible)`.
- **Live update**: `.visible_signal() -> Signal<bool>`.

```rust
let error = Visibility::new(StatusDot::error(), false);
error.visible_signal().set(true);
```

### ItemGroup

A collapsible group: a header `Item` (label + chevron) over a set of rows. Collapsing folds the
rows out of layout (`display: none`); a hidden subtree is never painted or Tab-focused.

- **Construct**: `ItemGroup::new(label)`. Add rows with `.child(...)`.
- **Builders**: `.expanded(bool)`, `.on_toggle(impl Fn(Action))` (`Action::value("group-toggle",
  Bool)`).
- **Accessors**: `.state() -> Signal<bool>` (expanded).

```rust
ItemGroup::new("src")
    .child(Item::new("main.rs"))
    .child(Item::new("lib.rs"));
```

### MarkerGroup

A vertical group of rows fronted by a left **marker bar** that brightens to the theme accent (+
glow) when the group is `active`. Domain-neutral: a host composes rows in and flips `active` to
show the group holds the current selection (e.g. a sidebar *column* whose bar lights when it holds
the focused pane). The bar reserves a fixed left gutter — children never overlap it — and is the
seam for a move/swap [`KeyHint`](#keyhint) target / drag handle (wrap the group, or mark it
draggable; the bar stays a pure indicator). All bar styling is read from the `Theme` at paint.

- **Construct**: `MarkerGroup::new()`. Add rows with `.child(...)`.
- **Builders**: `.active(bool)`, `.nav_selected(bool)` (sidebar-nav cursor — a full-opacity
  bar **without** the active glow, so it reads distinctly from the active column).
- **Accessors**: `.state() -> Signal<bool>` (active), `.nav_state() -> Signal<bool>` (nav
  cursor) — bind either; the host writes it when the group's selection changes and the bar
  repaints without a rebuild.

```rust
MarkerGroup::new()
    .active(holds_focus)
    .child(Row::new().child(Label::new("pane 1")))
    .child(Row::new().child(Label::new("pane 2")));
```

### DockFrame

A titled, collapsible, bracket-framed shell for a Dock: a title bar (drag-handle grip + chevron
+ title + a header-controls slot) over a foldable body. Reuses `Pane`'s corner brackets.
**Rail-aware**: bound to a region's [`RegionMode`](#chromeregion) signal, it folds header + body
away to a single centered `Icon` while the region is collapsed to a rail.

- **Construct**: `DockFrame::new(title)`. Add body with `.child(...)`.
- **Builders**: `.header(impl Component)` (fill the controls slot, e.g. a search field or count
  `Badge`), `.expanded(bool)`, `.on_toggle(impl Fn(Action))` (`"dock-toggle"`),
  `.rail(Signal<RegionMode>, Glyph)` (fold to an icon in `CollapsedRail`),
  `.active(bool)` (paint a faint accent **wash** over the whole frame — alpha =
  `Theme::active_wash_alpha` — to mark it as the current/active dock, e.g. the active workspace),
  `.nav_selected(bool)` (a hollow accent **border** marking the sidebar-nav cursor on a
  workspace frame — distinct from the filled active wash).
- **Accessors**: `.state() -> Signal<bool>` (expanded), `.active_state() -> Signal<bool>` (the
  wash flag), `.nav_state() -> Signal<bool>` (the nav-cursor outline flag) — bind them to flip
  the look in place without rebuilding the tree.

```rust
let sidebar = ChromeRegion::vertical();
let mode = sidebar.mode_signal();
let files = DockFrame::new("EXPLORER")
    .rail(mode, Glyph::FolderOpen)
    .header(Badge::accent("3"))
    .child(ItemGroup::new("src").child(Item::new("main.rs")));
```

### ChromeRegion

The generic, oriented **chrome shell** that hosts Docks — one widget for all four regions
(left/right sidebars = vertical; top/bottom bars = horizontal). A dumb, mode-aware container: it
stacks `DockFrame`s and sizes itself to its mode; it owns **no** tree/workspace/drag semantics.
Per the chrome plan's *read-via-signals, write-via-actions* rule, it reacts to a
[`RegionMode`](#chromeregion) signal the host drives.

- **Construct**: `ChromeRegion::vertical()` / `::horizontal()`. Add docks with `.dock(...)`.
- **Builders**: `.expanded_size(px)`, `.rail_size(px)`, `.mode(RegionMode)` (initial),
  `.with_mode_signal(Signal<RegionMode>)` (adopt a host-owned mode signal).
- **Accessors / intents**: `.mode_signal() -> Signal<RegionMode>` (binding point — share it with
  rail-aware Docks before `.dock(...)`), `.toggle()` (flip Expanded ⇄ CollapsedRail).
- **`RegionMode`**: `Expanded`, `CollapsedRail` (thin icon rail), `Hidden` (`display: none`). The
  library supports all three; **the heca app currently uses only `Expanded` and `Hidden`** (the
  collapsed rail was dropped — see [`../docs/sidebar-provider-modes.md`](../docs/sidebar-provider-modes.md)).

```rust
let sidebar = ChromeRegion::vertical().expanded_size(320.0).rail_size(64.0)
    .dock(explorer).dock(source_control);
sidebar.toggle();   // or the host sets mode_signal() from a key / RPC
```

### RailCell

A focusable **square icon cell** — the per-item unit a *list* Dock (workspaces / panes) shows
when collapsed to a rail, so every pane stays visible and addressable (vs a tool Dock folding to
one icon). Centers one `Icon`; active = accent tint + same-hue border + glow; hover/press flash;
focus ring. Wrap it in a [`KeyHint`](#keyhint) for the move/swap/select pick letters.

> **Not currently mounted in the app (2026-07-11).** The heca sidebar collapsed rail was dropped
> (a region is Expanded ⇄ Hidden), so nothing in the app builds `RailCell`s today. It remains a
> supported library widget, reserved for a future generic Provider icon rail — see
> [`../docs/sidebar-provider-modes.md`](../docs/sidebar-provider-modes.md) §4. If a future rail needs
> a `name`/`number` cell (a short text label instead of a glyph) or a `nav_selected` cursor state,
> those are additions to make then.

- **Construct**: `RailCell::new(Icon)`.
- **Builders**: `.cell_size(px)`, `.active(bool)`, `.on_activate(impl Fn())`.
- **Accessors**: `.state() -> Signal<bool>` (active/selected).

```rust
RailCell::new(Icon::new(Glyph::Terminal).color(theme.success).size(22.0))
    .cell_size(44.0).active(true).on_activate(move || focus_pane(i));
```

### KeyHint

> **From a plugin:** the leader/pick overlay is **host-owned and universal** — a plugin
> never creates a `KeyHint`; any plugin widget that exposes an `on_press` intent is
> auto-hintable ("intent ⇒ hintable"). Likewise a **context menu** is a host-owned
> dropdown the plugin *requests* (or declares via `.on_context`), not a nested widget.
> See **[plugin-authoring.md](plugin-authoring.md)** → "Context menus & KeyHint".

A **generic** transparent wrapper that overlays a glowing accent **keycap letter** on any
actionable child while a host-owned `Signal<Option<String>>` is `Some` — the keyboard pick /
jump prefix (move/swap/select, command palettes, content panes). It is transparent to focus and
events (the wrapped widget stays clickable/focusable); it only adds paint. Signal-driven, so
mouse, keyboard, and RPC all light it up identically.

- **Construct**: `KeyHint::new(child)`.
- **Builders**: `.hint(Signal<Option<String>>)`, `.placement(HintPlacement)`
  (`TopCenter` for compact square targets | `Center` for large panes | `CenterRight`
  for wide list rows — keycap pinned to the right edge | `TopRight` for tall targets like a
  workspace dock — right-aligned but anchored to the top edge, pair with `.offset_y` to land
  on the header row),
  `.size(px)`, `.color(Color)` (override the keycap tint — default theme `accent`; lets a
  host distinguish target *kinds*, e.g. workspace picks tinted `warning` vs pane picks),
  `.offset_y(px)` (nudge the cap down after placement — e.g. drop a `TopCenter` cap onto a
  tall target's header row). The wrapper is **transparent to a stretching parent**: a wide
  child row fills its column instead of shrinking to content width.
- **Accessors**: `.hint_signal() -> Signal<Option<String>>`.

```rust
let pick = signal(None);
let cell = KeyHint::new(RailCell::new(icon).on_activate(/* … */))
    .hint(pick).placement(HintPlacement::Center);
// during a pick the host sets pick.set(Some("a".into())); clears it on exit
```

#### Standalone keycap — `paint_keycap` / `keycap_size` / `KeycapVariant`

The keycap chip is factored out of `KeyHint` so it can be stamped directly into a
scene over targets that are **not** widgets (terminal hyperlink spans, via the host's
overlay pass) or reused by other overlays (the **context-menu** quick-pick). This is the
**single source** for the chip metric + visual — never re-derive keycap padding/sizing.

- `keycap_size(font: f32, text: &str) -> Size` — the exact chip size `KeyHint` uses.
- `paint_keycap(cx, cap, text, font, color: Option<Color>, variant: KeycapVariant)` —
  paints the chip: opaque glowing base + accent tint + centered dark glyph. `color`
  overrides the theme `accent` (fill + glow); the caller picks a **variant**, the
  primitive owns all styling (theme tokens only, no hardcoded values):
  - `KeycapVariant::Filled` — solid glowing chip. The default hint/pick look stamped
    over arbitrary content, where it must stay legible against whatever is beneath it.
  - `KeycapVariant::Bordered` — an **outline-only** chip: **no fill** (empty interior) + a
    **full-strength accent border** (the `color`/`accent` at full alpha, so it reads as the theme
    accent, not a washed tint), **no glow**, accent glyph. Matches the showcase KeyHint (which has
    no background). For keycaps on an already-dark, host-owned surface (e.g. the
    [ContextMenu](#contextmenu) quick-pick inside the menu panel). The [ContextMenu](#contextmenu)
    draws its letter at a **sub-font scale** so the chip stays compact.

```rust
let cap = Rectangle::new(Point::new(x, y), keycap_size(font, "a"));
paint_keycap(cx, cap, "a", font, None, KeycapVariant::Filled);   // over content
paint_keycap(cx, cap, "a", font, Some(accent), KeycapVariant::Bordered); // on a panel
```

**Plugins** never call this directly — a plugin widget with an `on_press` intent is
auto-hintable and the host stamps the keycap for it.

### Tooltip

A transparent wrapper that reveals a floating label when the pointer rests over its child past a
short delay. The bubble (rounded surface + accent border + soft glow + text) is drawn on the
**overlay layer** so it sits above siblings. Placement is **viewport-aware on all four sides**:
the preferred side flips to its opposite when there's no room (`Top`↔`Bottom`, `Left`↔`Right`)
and the cross-axis is clamped on-screen. It captures **no** input — the wrapped widget stays
fully interactive (forwards events + focus).

- **Construct**: `Tooltip::new(child, text)`.
- **Builders**: `.side(TooltipSide)` (`Top` | `Bottom` | `Left` | `Right`, default `Top`),
  `.delay(seconds)` (hover delay before reveal, default `0.5`).

```rust
Tooltip::new(
    IconButton::new(Icon::new(Glyph::Close).color(theme.danger).size(20.0)).on_click(|| close()),
    "Close",
).side(TooltipSide::Bottom);
```

> The raw `Tooltip::new(button, "Close")` above hardcodes the text. **In the heca app,
> do not do this for an action button** — see the next section: the tip (and its
> keybinding) is derived from the action, centrally.

### Action buttons — tooltip + KeyHint from the action (heca app pattern)

Any chrome button that triggers a `WmAction` gets its **tooltip** and its `prefix+/`
**KeyHint** from that action, automatically — the caller names the action, never a
shortcut string, the leader symbol, or a hand-built tip. This keeps every button
uniform and rebind-aware. The grid-ui primitives involved are **`IconButton`**,
**`Tooltip`**, and **`hint_target`** ([`KeyHint`](#keyhint) framework); the resolution
seam is app-side.

```rust
// heca/src/chrome/mod.rs — one call composes label + the live keybind(s):
let hint_id = hints.register(InteractionIntent::ActivateAction(action.clone()));
let button = IconButton::new(icon)
    .hint_target(hint_id)                       // prefix+/ can pick it (same intent as click)
    .on_click(move || emit(InteractionIntent::ActivateAction(action.clone())));
row.child(action_tooltip(button, "close", "Close", &state.action_shortcuts));
//                               ▲ action config name  ▲ label
```

- **Tooltip text is resolved by action name.** `ActionShortcuts` (on `AppState`, rebuilt
  at config load/reload) maps each action's config name → its display shortcut via
  `shortcut::shortcut_for_action`, which reads the **user's real binding when they've
  rebound it** (defaults only as fallback), supports **multiple** bindings (joined
  ` / `), and renders the leader through the `PREFIX_SYMBOL` constant — never a literal
  `λ`, never `⌃⌥⇧⌘`. So a rebind in `config.toml` updates the tip with no code change.
- **The name is the canonical key**, because the emitted `WmAction` may be a button-only
  variant that isn't itself bound (`ClosePaneById`, `AddPaneToColumn`).
- **KeyHint** = register the click's intent in the **shared** `HintTargetRegistry` (on
  `AppState`, a monotonic id allocator spanning the chrome tree **and** every per-pane
  header tree) and `.hint_target` it. Active-targeted buttons (zoom/float) register a
  `FocusPaneThenAction` intent so the hint focuses the pane first, exactly like the click.
  Full app-side rules are in **AGENTS.md → "Chrome buttons → action, tooltip, KeyHint"**.

### Modal

A centered **confirm / alert dialog** over a dimming scrim. Like `Select`, it captures input
while open — it reports `overlay_active` + is `focusable` only while open, so the host routes
pointer/keys to it first; its content (title, message, and a row of **N action buttons**) is
**drawn + hit-tested manually** on the overlay layer (no child subtree to relocate). Open/close is
a host-owned `Signal<bool>` (mouse/keyboard/RPC all drive it). Dismissal: a button, **Esc**
(= cancel), or a **scrim** click (= cancel) — each fires its callback and closes.

**Buttons are data-driven `ModalButton`s.** One button is **focused** (an accent focus ring);
the host moves focus and activates it (`focus_next` / `focus_prev` / `activate_focused`), each
button may carry a **letter shortcut** shown as `Label (x)` and fired by `activate_shortcut(c)`,
and `request_cancel()` activates the cancel button (or closes if dismissible). Initial focus is
the cancel button (safe default for a destructive dialog).

- **Construct**: `Modal::new(title, message)`.
- **Buttons**: `.button(ModalButton)` (general, N buttons). A `ModalButton::new(label, impl Fn())`
  takes `.shortcut(char)` (the `(x)` label + host-fired key), `.danger(bool)` (destructive tint),
  and `.cancel()` (Esc / scrim activate it; takes initial focus).
- **Convenience builders**: `.confirm(label, impl Fn())` (primary button), `.cancel(label, impl Fn())`,
  `.danger(bool)` (tints the primary), `.dismissible(bool)` (default `true`; `false` =
  **forced-decision** — Esc/scrim are swallowed, only the buttons close it), `.open(bool)`.
- **Host-driven keyboard** (the host supplies modifier awareness the widget lacks):
  `focus_next()` / `focus_prev()`, `activate_focused()`, `activate_shortcut(char) -> bool`,
  `request_cancel()`.
- **Accessor**: `.open_signal() -> Signal<bool>` — bind a trigger to it to show the dialog.

```rust
let modal = Modal::new("Delete pane?", "This action cannot be undone.")
    .button(ModalButton::new("Cancel", || {}).shortcut('n').cancel())
    .button(ModalButton::new("Delete", || wm.delete_focused()).shortcut('y').danger(true));
let open = modal.open_signal();
// … Button::destructive("DELETE").on_click(move || open.set(true)); add `modal` to the tree
```

> Host wiring: while `focus.overlay_active(root)`, route pointer **and keys** to the overlay
> (`focus.deliver_to_overlay(root, &ev)`) so Esc/Enter reach the dialog. `Modal::event` self-maps
> its raw keys (Tab / ←→ / Enter / Space / Char / Esc). Prefer [`Dialog`](#dialog) for new overlays:
> it carries **no** hardcoded nav keys — the host resolves them from config (`widget-keys-config`).

### Dialog

A centered overlay **panel that holds real child components** — the container counterpart to
[`Modal`](#modal). Where `Modal` draws its title/message/buttons **manually** (no child subtree,
so its buttons can't be hint targets or focus-traversed as components), `Dialog` lays out a
padded panel of `[title, body, action-row]` where the `body` is an arbitrary component and each
action is a real [`Button`](#button). Because the buttons are real children, they get the
universal hint picker (`prefix+/`), standard focus traversal, and pointer routing **for free** —
this is what makes an overlay's buttons hintable.

Same overlay contract as `Modal`: `overlay_active` + `focusable` only while open, so the host
routes input here first. Keyboard is an embedded [`FocusManager`](#) over the panel subtree. Unlike
`Modal`, `Dialog` carries no result closures: a button's own `on_click` is the action, and dismissal
is a callback the host points at its overlay-close path (e.g. emit `CloseOverlay`).

**Navigation keys are host-configured, not hardcoded (`widget-keys-config`).** The dialog carries no
literal nav keys; focus traversal / submit / cancel arrive as the semantic
`Event::DialogNav(DialogNav::{FocusNext, FocusPrev, Submit, Cancel})`. The **host** resolves these
from configurable `[keys]` bindings and only sends them after a raw key was **not** consumed by a
focused field (field-first):

| Intent | Default binding (config name) | Effect |
|--------|-------------------------------|--------|
| `DialogNav::FocusNext` | `Tab`, `↓`, `→`, `Ctrl+j` (`dialog_focus_next`) | focus the next field/button |
| `DialogNav::FocusPrev` | `Shift+Tab`, `↑`, `←`, `Ctrl+k` (`dialog_focus_prev`) | focus the previous field/button |
| `DialogNav::Submit` | `Enter` (`dialog_submit`) | activate the **primary** (first) action |
| `DialogNav::Cancel` | `Esc` (`dialog_cancel`) | dismiss (also fired by a **scrim** click) |

**Form bodies (text input) — field-first.** A raw `Event::Key` is handed to the **focused descendant
first**, so a [`Input`](#input) body (or a `realize`d `ViewNode` form) receives typed characters,
caret motion, Backspace/Delete, and its own [`InputEdit`](#input) shortcuts. Only a key the field
*doesn't* consume lets the host apply `DialogNav` — so an arrow moves the caret **inside** the input
but navigates when a **button** is focused, and `Enter`→`Submit` fires OK even while typing. App
wiring: `build_dialog_keymap` → `AppState.dialog_keymap`, consumed in the overlay key branch
(`heca/src/app/events.rs`). This is what makes the host-owned rename / prompt dialogs (an `Input` +
OK/Cancel) work.

Centering is real taffy layout: the root fills the viewport (`Pct(1.0)`²) with `Justify::Center`
+ `Align::Center`, so every descendant gets true bounds (which the hint picker + hit-testing need).

- **Construct**: `Dialog::new(title)`, then `.body(impl Component)` and `.action(impl Component)`
  (a wired `Button`), in that order. Buttons sit in a right-aligned row in call order.
- **Builders**: `.dismissible(bool)` (default `true`; `false` = forced-decision — `Cancel`/scrim
  swallowed without dismissing), `.on_dismiss(impl Fn())` (fired on `DialogNav::Cancel` / scrim),
  `.open(bool)`
  (focuses the first focusable — a text field body if present, so the user types immediately;
  otherwise the first button as a safe default), plus `.body_boxed(Box<dyn Component>)` for a body
  from a mapper (e.g. `realize`).
- **Accessor**: `.open_signal() -> Signal<bool>`.

```rust
let dialog = Dialog::new("Delete pane?")
    .body(Label::new("This action cannot be undone."))
    .action(Button::secondary("Cancel").hint_target(cancel_id).on_click(move || emit(submit_cancel)))
    .action(Button::destructive("Delete").hint_target(del_id).on_click(move || emit(submit_delete)))
    .on_dismiss(move || emit(close))
    .open(true);
```

> **Self-contained** — the host does nothing modal-specific. While a `Dialog` is up the host just
> forwards pointer + key + `ModifiersChanged` events to it (the same generic overlay routing every
> widget uses). `Dialog::event` owns only **Esc** (cancel) and **Tab / Shift+Tab** (focus); every
> other key is routed **field-first** to the focused descendant, and only keys it doesn't consume
> fall back to container nav (**Enter** → primary action, **↑ ↓ ← →** move focus, **Ctrl+h/j/k/l**
> move focus). So a focused `Input` body keeps its entire keyboard model (typing, Ctrl+h delete,
> Cmd/Ctrl+A select-all, caret motion) untouched — no re-declaration. Dismissal + activation flow out
> as callbacks — in `heca` the buttons emit `SubmitOverlay` and `on_dismiss` emits `CloseOverlay`, so
> one intent path resolves the overlay for click, KeyHint pick, and RPC alike. The developer only
> lists buttons; nav, tooltips, and KeyHint targets come from the widget + the centralized button path.

#### Declaring a modal from data — host code **and** plugins

App/agent code and plugins don't build the `Dialog` widget by hand — they **describe** a modal as
data and let the host own it. Both submit a `ModalSpec { title, body: ViewNode, actions }` to
`OverlayHost::open_modal` (`heca/src/chrome/overlay.rs`); the host `realize`s the `ViewNode` body,
injects the action buttons + KeyHint targets, shows it as a `Modal`-band layer, and returns the
outcome as `ModalResult::Action { id, data }` (or `Dismissed`).

The **body is any `ViewNode` tree** (labels, inputs, rows, cards…), so a modal can carry a form.
A value node opts into the returned `data` with a **`"name"` prop** — on submit the host collects
its current value under that name (`Input` → `Text`, `Toggle`/`Checkbox` → `Bool`).

**Validation — disable submit until a field is filled.** A `ModalAction` can be
`.disabled_when_empty("field")`: the host binds that button's `disabled` state to the named text
field's live value, so it greys out (and Enter / click do nothing) while the field is empty
(trimmed). This is how the rename dialog blocks a blank name — no per-modal validation code.

**Internal code** (a native handler; behaviour via a Rust completion closure):
```rust
open_modal(
    state,
    ModalSpec {
        title: "Rename pane".into(),
        body: ViewNode::new(WidgetKind::Column)
            .prop("gap", PropValue::Int(8))
            .child(ViewNode::new(WidgetKind::Label).text("New name"))
            .child(
                ViewNode::new(WidgetKind::Input)
                    .text(current_name)
                    .prop("name", PropValue::Text("name".into())), // collected into `data`
            ),
        actions: vec![
            ModalAction::new("cancel", "Cancel"),
            ModalAction::new("ok", "Rename").disabled_when_empty("name"), // OK greys out while blank
        ],
        danger: false,
        dismissible: true,
    },
    |state, registry, result| {
        if let ModalResult::Action { id, data } = result {
            if id == "ok" {
                if let Some(name) = data.get("name").and_then(PropValue::as_text) {
                    /* dispatch the rename with `name` */
                }
            }
        }
    },
);
// A plain confirm is the same shape: `ModalSpec::message(title, msg).action(...).danger(true)`.
```

**Plugin** (declarative + serializable): the **same `ModalSpec`/`ViewNode`**, authored as data —
no Rust closures. Behaviour is carried by `Intent`s and the plugin receives the `ModalResult`
(`id` + the named-field `data`) back over the boundary. Because it's the identical model, a modal
authored by a plugin is realized, hinted (`prefix+/`), keyboard-driven, and confirm-gated exactly
like a native one. (A first-class typed builder — `Column::new().gap(8).child(…)` — is
`plugin-task-ui-2`; today author the nodes with `ViewNode::new(kind).prop(…).child(…)`.)

### CommandPalette

A fuzzy **command launcher** overlay (same input-capturing contract as `Modal`): a query line
over a scrollable list of commands. The query line is a real [`Input`](#input), so full editing
comes for free — selection, multi-click, and char/word/line delete (Ctrl/Alt/⌘ + Backspace/Delete).
Typing filters with a **fuzzy subsequence** match, **smart-case** (case-insensitive unless the
query has an uppercase letter), ranked, with matched characters highlighted in the accent.
Selecting a command fires its callback and closes.

- **Construct**: `CommandPalette::new()`; add commands with `.command(Command::new(label, on_run)
  .icon(Glyph)?.key("⌘K")?)`; `.placeholder(text)`, `.open(bool)`.
- **Accessor**: `.open_signal() -> Signal<bool>` — bind a chord (e.g. Ctrl+K) to open it.
- **Nav (host-driven, configurable)**: the palette carries **no hardcoded nav keys**. It responds
  to the semantic `Event::MenuNav(MenuNav::{Prev,Next,Activate,Dismiss})`; the **host** resolves the
  configurable `menu_up` / `menu_down` / `menu_activate` / `menu_dismiss` keybindings into these
  (defaults: ↑/Ctrl+K, ↓/Ctrl+J, Enter, Esc). Raw `Event::Key` goes to the query field (typing /
  editing). App wiring lives in the `menu-nav` requirement / `build_menu_keymap`.

```rust
let palette = CommandPalette::new()
    .command(Command::new("Split pane", || wm.split()).icon(Glyph::Sidebar).key("⌥⌘S"))
    .command(Command::new("Close pane", || wm.close()).key("⌘W"));
let open = palette.open_signal();
// host: on Ctrl+K → open.set(true); add `palette` to the tree
```

> Needs the same host wiring as `Modal` (route keys to the overlay). Because it tracks `Ctrl` for
> Ctrl+J/K, the host must also broadcast `Event::ModifiersChanged` to the tree (most hosts do).

### ContextMenu

A **cursor-anchored action menu** overlay — the pointer counterpart to the keyboard pick flows
(same input-capturing contract as `CommandPalette`/`Modal`). A floating list of entries, each with
an optional **icon**, an optional **quick-pick keycap** (the shared
[`paint_keycap`](#standalone-keycap--paint_keycap--keycap_size--keycapvariant) primitive in its
`Bordered` variant — press the letter to run; never hand-drawn), an optional textual shortcut hint,
a `danger` flag (destructive entries render red), and an
`enabled` flag. Open/close **and the anchor point** are host-owned signals — right-click detection
lives at the app level (grid-ui pointer events carry no button), so the host sets the anchor to the
cursor and flips `open`. The panel sizes to its content and flips/clamps to stay on-screen.

- **Construct**: `ContextMenu::new()`; add entries with `.entry(MenuEntry::new(label, on_select)
  .icon(Glyph)?.key('x')?.shortcut("prefix+x")?.danger(bool)?.enabled(bool)?)`; `.open(bool)`, `.anchor(Point)`.
- **Accessors**: `.open_signal() -> Signal<bool>`, `.anchor_signal() -> Signal<Point>`.
- **Dismiss callback**: `.on_dismiss(impl Fn())` — fired on **Esc / outside-click** (a *dismissal*,
  not a selection; selecting an entry runs its `on_select` instead). The host points this at its
  overlay-close path (in `heca`, emit `CloseOverlay`), mirroring [`Dialog::on_dismiss`](#dialog).
- **Nav (host-driven, configurable)**: **no hardcoded nav keys** — the menu responds to
  `Event::MenuNav(MenuNav::{Prev,Next,Activate,Dismiss})`, which the host resolves from the
  configurable `menu_*` keybindings (defaults ↑/Ctrl+K, ↓/Ctrl+J, Enter, Esc; the `menu-nav`
  requirement). Raw `Event::Key` is only a **quick-pick letter** that runs its entry directly.
  Hover highlights; click runs; outside-click dismisses.

```rust
let menu = ContextMenu::new()
    .entry(MenuEntry::new("Rename", || wm.rename()).icon(Glyph::FileCode).key('r'))
    .entry(MenuEntry::new("Close", || wm.close()).icon(Glyph::XSquare).key('x').danger(true));
let (open, anchor) = (menu.open_signal(), menu.anchor_signal());
// host: on right-click → anchor.set(cursor); open.set(true); add `menu` to the tree
```

> Same host wiring as `Modal`/`CommandPalette` (route keys to the overlay). The app decides *when*
> (right-click) and *where* (cursor) to open it; the widget renders + captures input while open.

> **Declaring from data — host + plugins (shipped, `context-menu` phase).** The preferred path is a
> **data spec** the host owns, not hand-built closures: `OverlayHost::open_dropdown(DropdownSpec {
> anchor, entries, centered })`, where each `MenuEntrySpec { id, label, action, danger, enabled }`
> carries an **`Intent`/`WmAction`** (not a closure) and its **icon resolves from the action
> registry** (`ActionCatalog::icon`). The host realizes the entries into this widget, injects the
> `SubmitOverlay{overlay,id}` intent + a KeyHint target + the host-assigned quick-pick letter, pushes
> it as an Overlay-band **modal layer**, and on select dispatches the action **through the central
> confirm gate**. `centered = true` centers the panel on the anchor (keyboard-opened menus). Dismiss →
> `CloseOverlay` (the `on_dismiss` hook above).

> **Context-aware content (`ContextMenuRegistry`).** Which entries appear is resolved from **where**
> the menu is opened: a dotted **`ContextPath`** (`"pane"`, `"sidebar.pane"`, `"sidebar.column"`,
> `"sidebar.workspace"`, plugin paths) + an opaque **`ContextTarget"`**. The host resolves the path
> from the click / keyboard focus (`resolve_active_context`), looks up all providers registered for
> it, and **merges** them ordered by a Dewey `weight: Vec<i64>` — so a plugin inserts entries between
> built-ins. Same menu widget; different content per context.
>
> **From a plugin.** A plugin never draws the menu — it either attaches entries declaratively on a
> `ViewNode` (`.on_context([ item("restart","Restart"), … ])`), or registers a
> `Contribution::ContextMenu { context_path, weight, build(target) -> Vec<MenuEntrySpec> }`. On
> right-click / keyboard-open the host opens the (merged) menu, owns z-order / focus / Esc /
> click-outside, and returns the chosen entry as an **intent**. See
> **[plugin-authoring.md](plugin-authoring.md) → "Context menus & KeyHint"**.

> **Shortcut text (`.shortcut(...)`):** don't hand-format keybindings. The app renders the tmux-style
> `prefix` as a symbol (`λ`) while keeping `prefix` as the config/parse token, via the single helper
> `heca::shortcut::format_shortcut(keys, with_prefix)` — e.g. `format_shortcut("prefix+x", true)` →
> `"λ x"`. Feed that into `.shortcut(...)` so the symbol/formatting live in one place (the showcase
> mirrors this with its own `display_shortcut`).

### ToastStack

An overlay that arranges a **host-supplied** set of notifications into a corner stack. **Presentation
only** — it owns no queue, lifetimes, auto-dismiss timers, or dedup; that's the app's job. The host
owns a `Signal<Vec<ToastSpec>>` (its render list); the stack reconciles cached [`Toast`](#toast)
widgets by **id** (each keeps its hover/flash state), corner-anchors them on the overlay layer,
slides new ones in, routes events to the toast under the cursor, and reports
`on_dismiss(id)`/`on_action(id)` back — the host then removes the id (which reflows the rest). It is
overlay-active only while it has toasts, and **passes through** clicks that miss every toast.

- **Construct**: `ToastStack::new(items: Signal<Vec<ToastSpec>>)`; `.corner(ToastCorner)`,
  `.gap(px)`, `.margin(px)`.
- **Intents**: `.on_dismiss(|id| …)` (× clicked), `.on_action(|id| …)` (inline action clicked).
- **`ToastSpec`**: `ToastSpec::new(id, title).severity(..).icon(..)?.body(..)?.action(label)?.dismissible(bool)` — plain data the host owns.

```rust
let toasts = signal(Vec::<ToastSpec>::new());            // the app's render list
let stack = ToastStack::new(toasts)
    .corner(ToastCorner::TopRight)
    .on_dismiss(move |id| toasts.update(|v| v.retain(|s| s.id != id)));
// app pushes:  toasts.update(|v| v.push(ToastSpec::new(1, "Saved").severity(ToastSeverity::Success)));
```

> Same host wiring as `Modal` (route pointer to the overlay first). Auto-dismiss/timers live in the
> app: run a timer, then remove the id from `items`.

---

## Declarative UI model (`ViewNode`)

`ViewNode` (`heca/src/chrome/view.rs`) is the **serializable UI description** that both native code
and plugins author, and that the host mapper `realize()` turns into a retained tree of the widgets
above. It's the SwiftUI/Flutter-style layer: you *describe* the UI as data; the host builds it. This
is how a plugin declares UI (it can't ship Rust widgets), and the ergonomic native path too.

### The model — a node is four things, all its own

| Part | What it is |
|------|-----------|
| `kind` | which widget (`WidgetKind`: `Column`/`Row`/`Label`/`Button`/`Input`/…) |
| `props` | this node's **own** values (`name → PropValue`) — **per node, not inherited** |
| `events` | this node's **own** `event → Intent` bindings (`press` / `change`) — an action **id**, never a closure (keeps it serializable) |
| `children` | a **`Vec<ViewNode>`**, each a full node with its *own* props/events/children |

**Props are per-node.** `.prop("gap", …)` on a `Column` styles *the column*, not its children — the
props sitting next to `.child(…)` calls belong to the node you called `.prop` on (the container). A
child is styled by putting props on *that child*. The builder chains for ergonomics but children are
a plain vector: `.child(n)` appends one, `.children([a,b])` appends many — `Column().child(a).child(b)`
≡ `Column().children([a,b])`.

### Props & events by kind (what `realize` reads today)

Missing/mistyped props are ignored (the widget keeps its default) — the model is untrusted input.

| Kind | Props it reads | Events |
|------|----------------|--------|
| `Column` / `Row` | `gap` (Int/Float), `align` (Align) | — |
| `Card` | `text` (title) + children | — |
| `Surface` / `Panel` / `Scroll` | (container — children only) | — |
| `Label` / `Badge` / `Tag` / `Alert` | `text` | — |
| `Button` / `BadgeButton` | `text`, `variant`, `size` | `press` |
| `Icon` / `IconButton` / `RailCell` | `icon` (Glyph **name**), `size` | `press` (button/rail) |
| `Input` | `text` (value), `name` | `change` |
| `Toggle` | `on` (Bool), `name` | `change` |
| `Checkbox` | `checked` (Bool), `text` (label), `name` | `change` |
| `Gauge` | `value` (Float) | — |
| `Item` | `text` (label) | `press` |

`PropValue` variants: `Bool` · `Int` · `Float` · `Text` · `Size`(`ViewSize`) · `Variant`(`ViewVariant`)
· `Align`(`ViewAlign`) · `Color`(name/`#rrggbb`) · `Glyph`(name). A **`"name"` prop** on a value
widget opts it into a submitted modal's returned `data` (see [Dialog](#dialog) → *Declaring a modal
from data*). Not realized yet (need structured/list props — `plugin-task-ui-9`): `Select`, `Tabs`,
`Grid`, `ItemGroup`, `DockFrame`, `MarkerGroup`, `ScrollBar`, `Toast`.

### Declaring a tree — internal code and plugins (same model)

```rust
// A labelled input + a primary button. Each node carries ITS OWN props/events.
ViewNode::new(WidgetKind::Column)
    .prop("gap", PropValue::Int(8))                                  // ← the COLUMN's prop
    .child(ViewNode::new(WidgetKind::Label).text("New name"))
    .child(
        ViewNode::new(WidgetKind::Input)
            .text("current")
            .prop("name", PropValue::Text("name".into())),          // ← the INPUT's props (form field)
    )
    .child(
        ViewNode::new(WidgetKind::Button)
            .text("Rename")
            .prop("variant", PropValue::Variant(ViewVariant::Primary)) // ← the BUTTON's prop
            .on_press(Intent::new("rename")),                          // ← the BUTTON's event → action id
    );
```

- **Internal code** authors this directly (as above) and hands it to `realize` / `open_modal`.
- **Plugins** author the *same* nodes and ship them serialized (JSON); behaviour is the `Intent`
  action ids, so no closures cross the boundary. A typed SwiftUI-style builder
  (`Column::new().gap(8).child(…)`) is `plugin-task-ui-2`; until then use `ViewNode::new(kind)`.
- **Extending the vocabulary is host-side** (never a plugin): add a `WidgetKind` variant + a
  `realize` arm + the widget's showcase demo + its entry here. Plugins compose from existing kinds.

---

## Patterns

### Handling change events

All change widgets push a semantic `Action` to their `on_change` handler. A consumer
typically funnels these into one dispatch:

```rust
fn dispatch(action: Action) {
    match (action.name.as_str(), action.data) {
        ("toggle-change",   SignalData::Bool(on))   => set_uplink(on),
        ("checkbox-change", SignalData::Bool(b))    => set_flag(b),
        ("input-change",    SignalData::String(s))  => set_query(s),
        ("tab-change",      SignalData::Usize(i))   => select_tab(i),
        _ => {}
    }
}
// e.g. Toggle::new().on_change(dispatch)
```

### Reactive binding

Drive UI from app state by holding the widget's signal (or a `Label`'s text signal) and
`.set()`-ing it; the dependent component repaints. Mirror a widget's state into app state
from `on_change`.

### Focus & accessibility

Interactive widgets are `focusable()` (unless disabled). Use one `FocusManager`: `advance`
on Tab/Shift+Tab, `focus_at` on click, `deliver_key` for everything else. The `focus_ring`
outline shows whenever a widget is `focused` and `theme.show_focus_border` is on.
Order follows `tab_index` (ascending) then tree position.

### Disabled

`.disabled(true)` on any widget dims it (`PaintCx::dim` scrim), makes `event` inert, and
drops it from focus traversal — one shared `Base` property, consistent across widgets.

### Pane Info Rows

The final Phase 7 pane chrome recipe is a two-row composition:

- row 1 is a centered inline `status dot + icon + name`
- row 2 is a flat git metadata line: `branch + optional +N / ~N / -N`

Hide the full second row outside repos with `Visibility`. This matches the app
sidebar more closely than the earlier segmented-`Tag` experiment.

```rust
Flex::column()
    .gap(4.0)
    .child(
        Flex::row()
            .align(Align::Center)
            .gap(8.0)
            .child(
                Flex::row()
                    .align(Align::Center)
                    .width(Length::Px(12.0))
                    .child(StatusDot::online()),
            )
            .child(Icon::new(Glyph::FileCode).size(14.0))
            .child(Label::new("Neovim").bold(true)),
    )
    .child(
        Visibility::new(
            Flex::row()
                .align(Align::Center)
                .gap(6.0)
                .child(Icon::new(Glyph::GitBranch).size(12.0))
                .child(Label::new("feature/pane-runtime").font_scale(0.8))
                .child(
                    Flex::row()
                        .align(Align::Center)
                        .gap(4.0)
                        .child(Icon::new(Glyph::Plus).size(12.0))
                        .child(Label::new("+2").font_scale(0.8)),
                )
                .child(
                    Flex::row()
                        .align(Align::Center)
                        .gap(4.0)
                        .child(Icon::new(Glyph::Warning).size(12.0))
                        .child(Label::new("~3").font_scale(0.8)),
                ),
            true,
        ),
    );
```

### Building a custom widget

```rust
use heca_grid_ui::prelude::*;
use heca_grid_ui::component::{Base, Component, PaintCx};

struct Reticle { base: Base }
impl Reticle {
    fn new() -> Self { Self { base: Base::new() } }
}
impl Component for Reticle {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn paint(&self, cx: &mut PaintCx) {
        let ring = cx.theme().colors.effective_focus_ring();
        cx.focus_ring(self.base.bounds, ring, cx.theme().colors.control_radius());
    }
}
impl LayoutExt for Reticle {}   // opt into .width/.height/.padding/… for free
```

Embed `Base`, implement `Component` (override `paint`/`event`/`tick` as needed, plus
`remeasure` if the widget's size depends on the font — read `self.base.font`), and opt into
builder traits. Reuse `PaintCx` helpers (`rect`, `focus_ring`, `bracket_frame`, `text`, `flash`, `dim`)
and theme tokens (`radius`/`border_width`/`glow_size`) so the Tron look stays consistent and DRY.

### Drag and drop

A small, **domain-neutral, reusable** drag-and-drop framework lives in
`heca_grid_ui::drag`. It has three layers, each usable on its own:

**1. Universal opt-in (`DragExt`).** Every widget gets `.draggable(id)` and
`.drop_target(id)` for free (blanket impl, like `visible`/`disabled`). The id is an
opaque `DragItemId` the *app* maps back to its own model — widgets stay neutral and
no extra layout nodes are added.

```rust
Row::new().child(/* … */).draggable(DragItemId::new(i))   // a drag source
Flex::column().drop_target(DragItemId::new(zone_id))       // a drop zone
```

**2. Resolution over the laid-out tree.** Pure bounds walks replace hand-computed
hit-testing — they read each widget's `Base.bounds` (filled by layout each frame):

- `drag::source_at(root, point) -> Option<DragItemId>` — what a press would pick up.
- `drag::resolve_at(root, point) -> Option<DropHit>` — the topmost/deepest drop
  target under the cursor, with `DropHit { id, bounds, side }` where
  `DropSide` is `Before` / `Onto` / `After` (vertical thirds of the target).

**3. Gesture state + theme-driven visuals.** The state machine is generic over an
**app-defined payload** `P` — the framework never knows what's being dragged:

- `DragContext<P>` coordinates surfaces (`DragSurfaceId`); each `SurfaceDragState<P>`
  runs `DragPhase::{Idle, Starting, Dragging}` with a threshold
  (`DEFAULT_DRAG_THRESHOLD_SQ`, `rubberband`). Use `payload()` / `payload_mut()` to
  read/update the in-flight payload (e.g. toggle a swap flag mid-drag).
- `PaintCx::drag_ghost(rect, text, swap)` paints the cursor-following chip (overlay
  layer); `swap=true` adds an inset double frame (matching `swap_indicator`) so the chip
  reads as an **exchange**, not a move;
  `PaintCx::drop_indicator(bounds, side)` paints the insertion line / onto-wash for a
  **move**; `PaintCx::swap_indicator(bounds)` paints a whole-item **double frame** for a
  **swap** (an exchange has no before/after — so it deliberately avoids the insertion
  line). `drag::resolve_at_filtered(root, point, accept)` is the source-aware variant of
  `resolve_at` (the deepest *accepted* target wins) — e.g. while dragging a container,
  accept only container-level targets so nested leaves fall through. All derive from
  `theme.accent` — no hardcoded colors.

**Putting it together** (the app owns `DragContext<AppPayload>` and dispatches its
own pointer events into the retained tree):

```rust
// press: pick up the source under the cursor
if let Some(id) = drag::source_at(&tree, pos) {
    let payload = app.payload_for(id);            // app maps id -> its model
    ctx.surface_mut(surface).unwrap().phase = DragPhase::Starting {
        payload, start_pos: (pos.x as f32, pos.y as f32),
        threshold_sq: drag::DEFAULT_DRAG_THRESHOLD_SQ,
    };
}
// move (past threshold): track hover for the indicator
let hover = drag::resolve_at(&tree, pos);          // Option<DropHit>
// release: apply the drop
if let Some(DropHit { id, side, .. }) = hover {
    app.perform_drop(dragged_payload, id, side);   // app action (move/swap/insert)
}
```

Painted by the host after the tree paints: `cx.drop_indicator(hit.bounds, hit.side)`
for the hovered target and `cx.drag_ghost(chip_rect, label)` at the cursor.

> Why opaque ids + generic `P`: the framework hosts *any* future surface/container
> (panes, columns, Docker, agents, git, notes) without change — see
> `dnd-framework-refactor-plan.md`. Never put app concepts (pane/workspace/column)
> into the `drag` module.
