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
- [Foundations](#foundations) — `Base`, `Component`, builder traits, `Style`, [Font sizing](#font-sizing), `Theme`/`GlowLevel`/`Intensity`, `Color`, signals, events, `Action`, `Scene`/`PaintCx`, `Flash`
- [Widgets](#widgets)
  - Layout: [`Flex`/`Container`](#flex--container), [`Surface`](#surface), [`Card`](#card), [`Pane`](#pane)
  - Text: [`Label`](#label)
  - Interactive: [`Button`](#button), [`Toggle`](#toggle), [`Checkbox`](#checkbox), [`Input`](#input), [`Tabs`](#tabs), [`Select`](#select), [`Item`](#item)
  - Display: [`Badge`](#badge), [`StatusDot`](#statusdot), [`Separator`](#separator), [`Spinner`](#spinner), [`Alert`](#alert), [`ProgressBar`](#progressbar), [`Gauge`](#gauge)
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
- **Traits**: `LayoutExt`, `StyleExt`, `Parent`.

```rust
Pane::new().width(Length::Px(320.0)).gap(2.0).background(theme.surface)
    .child(Item::new("DASHBOARD").marker(ActiveMarker::Bar).active(true))
    .child(Item::new("SETTINGS").marker(ActiveMarker::Bar));
```

> Today `Pane` is a framed container only — the HUD header (title + status) and tab bar from
> the design vision are still pending (see `grid-ui-plan.md` → Phase C7).

### Label

A single text run bound to a `Signal<String>`.

- **Construct**: `Label::new(text)`.
- **Builders**: `.align(TextAlign)`, `.color(Color)`, `.font_size(f32)` (pin a size),
  `.font_scale(f32)` (multiplier vs the inherited base font — prefer this for hierarchy).
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
