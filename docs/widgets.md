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
  - Layout: [`Flex`/`Container`](#flex--container), [`Surface`](#surface), [`Card`](#card), [`Pane`](#pane), [`Grid`](#grid)
  - Text: [`Label`](#label)
  - Interactive: [`Button`](#button), [`IconButton`](#iconbutton), [`Toggle`](#toggle), [`Checkbox`](#checkbox), [`Input`](#input), [`Tabs`](#tabs), [`Select`](#select), [`Item`](#item), [`Row`](#row)
  - Display: [`Badge`](#badge), [`StatusDot`](#statusdot), [`Separator`](#separator), [`Spinner`](#spinner), [`Alert`](#alert), [`Toast`](#toast), [`ProgressBar`](#progressbar), [`Gauge`](#gauge), [`Icon`](#icon), [`Tag`](#tag)
  - Chrome (sidebars/docks): [`ItemGroup`](#itemgroup), [`MarkerGroup`](#markergroup), [`DockFrame`](#dockframe), [`ChromeRegion`](#chromeregion), [`RailCell`](#railcell), [`KeyHint`](#keyhint)
  - Overlays: [`Tooltip`](#tooltip), [`Modal`](#modal), [`CommandPalette`](#commandpalette), [`ToastStack`](#toaststack)
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
| `focus_visible` | `Signal<bool>` | Show the focus ring (keyboard focus only). |
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

### `Theme`, `GlowLevel` & `Intensity`

Token struct consumed by `PaintCx`. Presets: **`Theme::grid_tron()`** (cyan, dark — the
default) and **`Theme::grid_ares()`** (alternate). Tokens: `background`, `surface`,
`foreground`, `muted`, `border`, `accent`, `glow`, `danger`, `success`, `warning`,
`font_family`, `font_size`, `radius`, `border_width`, `glow_size` (`GlowLevel`),
`intensity`, `show_focus_border`.

| Token | Type | Drives |
|-------|------|--------|
| `radius` | `f32` | Base corner radius. Boxes use it directly; small controls use `control_radius()` (= `radius × 0.5`); pills (Badge/Toggle/ProgressBar) round at `radius × 2` clamped to their capsule. `0` ⇒ square. |
| `border_width` | `f32` | Border stroke width for every box/pill widget. `0` ⇒ no border. |
| `glow_size` | `GlowLevel` | The **sole** owner of glow — scales every glow's halo radius. `None` removes glow entirely. |
| `intensity` | `Intensity` | The **CRT scanline overlay** only (no longer touches glow). |
| `font_size` | `f32` | Base font every widget inherits (see [Font sizing](#font-sizing)). |

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
| `.corner_brackets(rect, color)` | L-shaped corner reticle (focus ring / decoration). |
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

A bracket-framed container for sidebars/panels: a dark surface with a subtle accent border
and **rounded corner brackets** (no glow/shadow); children stack inside (default column).
The brackets are segments of a theme-`radius` rounded border with the straight midsections
dimmed, so the corners share the border's radius exactly. Reads `theme.radius` /
`theme.border_width`.

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
                    .child(Tooltip::new(IconButton::new(Icon::new(Glyph::SquareSplitVertical)), "Add pane"))
                    .child(Tooltip::new(IconButton::new(Icon::new(Glyph::XSquare).color(theme.danger)), "Close")),
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
with `.size(px)`. Focusable once `.on_click(...)` is set.

- **Construct**: `IconButton::new(Icon)`.
- **Builders**: `.size(px)` (pin a square), `.tone(Color)` (hover/press hue, default accent),
  `.glow(bool)`, `.on_click(impl Fn() + 'static)`.
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
  field height tracks the font), `.on_change(impl Fn(Action))`.
- **Accessors**: `.text() -> Signal<String>`, `.value_str() -> String`,
  `.selection() -> Option<(usize,usize)>`, `.selected_text() -> Option<String>`.
- **Emits**: `"input-change"` / `SignalData::String` (the full new text) on every edit.

**Keyboard model** (modifier = Ctrl on Win/Linux, Option/Alt or Cmd on macOS, as noted):

| Keys | Action |
|------|--------|
| printable / Space | insert at caret (replaces selection) |
| ← / → | move caret by char |
| Ctrl/Cmd + ← / → | move caret to start / end |
| Alt + ← / → | move caret by word |
| Home / End | caret to start / end |
| Shift + (any of the above) | extend/shrink selection the same distance |
| double / triple click | select word / select all (4th click clears) |
| Cmd/Ctrl + A | select all |
| Backspace / Delete | delete char before / after caret (or the selection) |
| Ctrl/Alt + Backspace/Delete | delete word |
| Cmd/meta + Backspace/Delete | delete to start / end |

```rust
let name = Input::new().placeholder("CALLSIGN")
    .on_change(|a| { if let SignalData::String(s) = a.data { store(s); } });
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
  `.marker(ActiveMarker)`, `.highlight(Color)` (override the derived hue),
  `.attention(Signal<bool>)` + `.attention_color(Color)`, plus `StyleExt` for a persistent
  background under the selection overlay.
- **Accessors**: `.state() -> Signal<bool>` (active).
- **Attention**: when the host sets the bound `attention` signal `true`, the row flashes a few
  times (see [`Attention`](#attention)) and consumes the signal. The host plays any **sound** —
  the library is audio-free.

```rust
let row = Row::new().background(color.with_alpha(22)).radius(theme.control_radius())
    .child(/* a Grid of icon + title + Tag + Badge */)
    .on_activate(move || select(i));
```

### Badge

Self-sizing neon pill for status/metadata (display-only).

- **Construct**: `Badge::new(label)` (= accent) or `Badge::{accent,neutral,success,warning,danger,outline}(label)`.
- **Builders**: `.variant(BadgeVariant)` (`Accent`/`Neutral`/`Success`/`Warning`/`Danger`/`Outline`).

```rust
Badge::success("ONLINE");  Badge::outline("BETA");
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

- **Construct**: `Icon::new(Glyph)` (curated set) or `Icon::from_codepoint(secondary_cp)`.
- **Builders**: `.size(px)`, `.color(Color)` (primary), `.secondary_color(Color)`.
- **`Glyph`**: a curated enum — `Folder`, `FolderOpen`, `File`, `FileCode`, `GitBranch`,
  `GitCommit`, `GitMerge`, `GitPullRequest`, `Terminal`, `Gear`, `Search`, `Close`, `Check`,
  `Play`, `Pause`, `Stop`, `Warning`, `Info`, `Lightning`, `List`, `Sidebar`, … (or use
  `from_codepoint` for any glyph).

```rust
Icon::new(Glyph::GitBranch).color(theme.warning).size(18.0);
```

### Tag

A bordered metadata chip — git branch / path / filter, hue-configurable and domain-neutral.
**Multi-segment**: chain `.segment_*` to render thin-divided sections (e.g. `path │ ⎇ main │ 5
+152 -12`). Theme-driven radius + border; generous per-axis padding. In a `Grid` cell, wrap in
`Flex::row().child(tag)` so it hugs its content instead of stretching.

- **Construct**: `Tag::new(label)`.
- **Builders**: `.leading(impl Component)` (e.g. an `Icon`), `.segment(impl Component)` /
  `.segment_text(label, Option<leading>)` (add a divided segment), `.color(Color)` (hue).

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
- **Builders**: `.active(bool)`.
- **Accessors**: `.state() -> Signal<bool>` (active) — bind it; the host writes it when the
  group's selection changes and the bar repaints without a rebuild.

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
  `.rail(Signal<RegionMode>, Glyph)` (fold to an icon in `CollapsedRail`).
- **Accessors**: `.state() -> Signal<bool>` (expanded).

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
- **`RegionMode`**: `Expanded`, `CollapsedRail` (thin icon rail), `Hidden` (`display: none`).

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

- **Construct**: `RailCell::new(Icon)`.
- **Builders**: `.cell_size(px)`, `.active(bool)`, `.on_activate(impl Fn())`.
- **Accessors**: `.state() -> Signal<bool>` (active/selected).

```rust
RailCell::new(Icon::new(Glyph::Terminal).color(theme.success).size(22.0))
    .cell_size(44.0).active(true).on_activate(move || focus_pane(i));
```

### KeyHint

A **generic** transparent wrapper that overlays a glowing accent **keycap letter** on any
actionable child while a host-owned `Signal<Option<String>>` is `Some` — the keyboard pick /
jump prefix (move/swap/select, command palettes, content panes). It is transparent to focus and
events (the wrapped widget stays clickable/focusable); it only adds paint. Signal-driven, so
mouse, keyboard, and RPC all light it up identically.

- **Construct**: `KeyHint::new(child)`.
- **Builders**: `.hint(Signal<Option<String>>)`, `.placement(HintPlacement)`
  (`TopCenter` for compact square targets | `Center` for large panes | `CenterRight`
  for wide list rows — keycap pinned to the right edge so the row label stays readable),
  `.size(px)`. The wrapper is **transparent to a stretching parent**: a wide child row
  fills its column instead of shrinking to content width.
- **Accessors**: `.hint_signal() -> Signal<Option<String>>`.

```rust
let pick = signal(None);
let cell = KeyHint::new(RailCell::new(icon).on_activate(/* … */))
    .hint(pick).placement(HintPlacement::Center);
// during a pick the host sets pick.set(Some("a".into())); clears it on exit
```

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

### Modal

A centered **confirm / alert dialog** over a dimming scrim. Like `Select`, it captures input
while open — it reports `overlay_active` + is `focusable` only while open, so the host routes
pointer/keys to it first; its content (title, message, one or two buttons) is **drawn + hit-tested
manually** on the overlay layer (no child subtree to relocate). Open/close is a host-owned
`Signal<bool>` (mouse/keyboard/RPC all drive it). Dismissal: the buttons, **Esc** (= cancel), or a
**scrim** click (= cancel) — each fires its callback and closes.

- **Construct**: `Modal::new(title, message)`.
- **Builders**: `.confirm(label, impl Fn())` (default `OK`), `.cancel(label, impl Fn())`
  (optional; Esc/scrim also cancel), `.danger(bool)` (danger-tinted confirm),
  `.dismissible(bool)` (default `true`; `false` = **forced-decision** — Esc/scrim are swallowed,
  only the buttons close it), `.open(bool)`.
- **Accessor**: `.open_signal() -> Signal<bool>` — bind a trigger to it to show the dialog.

```rust
let modal = Modal::new("Delete pane?", "This action cannot be undone.")
    .confirm("Delete", || wm.delete_focused())
    .cancel("Cancel", || {})
    .danger(true);
let open = modal.open_signal();
// … Button::destructive("DELETE").on_click(move || open.set(true)); add `modal` to the tree
```

> Host wiring: while `focus.overlay_active(root)`, route pointer **and keys** to the overlay
> (`focus.deliver_to_overlay(root, &ev)`) so Esc/Enter reach the dialog.

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
- **Nav (built-in)**: ↑/↓ and **Ctrl+J / Ctrl+K** move; **Enter** runs; **Esc** / scrim-click
  close. Also exposed as intents — `select_next()`, `select_prev()`, `run_selected()` — so a host
  can bind its own configurable keys.

```rust
let palette = CommandPalette::new()
    .command(Command::new("Split pane", || wm.split()).icon(Glyph::Sidebar).key("⌥⌘S"))
    .command(Command::new("Close pane", || wm.close()).key("⌘W"));
let open = palette.open_signal();
// host: on Ctrl+K → open.set(true); add `palette` to the tree
```

> Needs the same host wiring as `Modal` (route keys to the overlay). Because it tracks `Ctrl` for
> Ctrl+J/K, the host must also broadcast `Event::ModifiersChanged` to the tree (most hosts do).

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
on Tab/Shift+Tab, `focus_at` on click, `deliver_key` for everything else. The focus-visible
ring (corner brackets) shows only for keyboard focus and only when `theme.show_focus_border`.
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
        cx.corner_brackets(self.base.bounds, cx.theme().accent);
    }
}
impl LayoutExt for Reticle {}   // opt into .width/.height/.padding/… for free
```

Embed `Base`, implement `Component` (override `paint`/`event`/`tick` as needed, plus
`remeasure` if the widget's size depends on the font — read `self.base.font`), and opt into
builder traits. Reuse `PaintCx` helpers (`rect`, `corner_brackets`, `text`, `flash`, `dim`)
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
