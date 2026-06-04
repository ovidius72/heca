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
- [Foundations](#foundations) — `Base`, `Component`, builder traits, `Style`, `Theme`, `Color`, signals, events, `Action`, `Scene`/`PaintCx`, `Flash`
- [Widgets](#widgets)
  - Layout: [`Flex`/`Container`](#flex--container), [`Surface`](#surface), [`Card`](#card)
  - Text: [`Label`](#label)
  - Interactive: [`Button`](#button), [`Toggle`](#toggle), [`Checkbox`](#checkbox), [`Input`](#input), [`Tabs`](#tabs)
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
`Intensity`/`Theme`; and the `Action`/`SignalData` change-event types.

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

// (a) size the root to the window and compute layout
ui.base_mut().style.width  = Length::Px(win_w);
ui.base_mut().style.height = Length::Px(win_h);
LayoutEngine::new().compute(&mut ui, Size::new(win_w as f64, win_h as f64));

// (b) paint into a fresh Scene
let mut scene = Scene::new();
{
    let mut cx = PaintCx::new(&mut scene, &theme);
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

### `Component` trait

| Method | Default | Purpose |
|--------|---------|---------|
| `base(&self) -> &Base` | — | Required. |
| `base_mut(&mut self) -> &mut Base` | — | Required. |
| `focusable(&self) -> bool` | `false` | Interactive widgets return `true` (and `!disabled`). |
| `paint(&self, cx: &mut PaintCx)` | base chrome + children | Emit `DrawCommand`s. |
| `event(&mut self, ev: &Event) -> Handled` | route to children | Handle input. |
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

> Layout-only `Flex` deliberately does **not** implement `StyleExt` — wrap content in a
> `Surface`/`Card` to give it a background.

**`Parent`** (containers): `.child(impl Component + 'static)` appends a child.

### `Style` & layout enums

`Style` fields: `direction`, `justify`, `align` (default `Stretch`), `gap`, `padding`,
`width`/`height` (`Length`), `flex_grow`, `fill`, `border`, `glow`, `accent`, `fg`,
`radius`, `font_size`. Enums: `Direction{Row,Column}`, `Justify{Start,Center,End,SpaceBetween,SpaceAround}`,
`Align{Start,Center,End,Stretch}`, `Length{Auto,Px(f32)}`.

### `Theme` & `Intensity`

Token struct consumed by `PaintCx`. Presets: **`Theme::grid_tron()`** (cyan, dark — the
default) and **`Theme::grid_ares()`** (alternate). Tokens: `background`, `surface`,
`foreground`, `muted`, `border`, `accent`, `glow`, `danger`, `success`, `warning`,
`font_family`, `font_size`, `radius`, `intensity`, `show_focus_border`.

> **Never hard-code font family/size** — read `theme.font_family` / `theme.font_size`.

`Intensity{Off, Low, Medium, Heavy}` scales glow + scanlines globally: `.glow_scale()`,
`.scanline_opacity()`, `.next()` (cycles). At `Off`, `PaintCx` strips glow entirely.

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
  `Key{key, pressed}`, `ModifiersChanged(Modifiers)`.
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

### `Scene` / `DrawCommand` / `PaintCx` (for building widgets)

`paint` receives a `PaintCx` exposing shared Tron drawing helpers (all reused so widgets
stay DRY):

| `PaintCx` method | Draws |
|------------------|-------|
| `.theme() -> &Theme` | Active theme tokens. |
| `.rect(rect, fill, Option<Border>, radius, Option<Glow>)` | Rounded rect + optional border + glow. |
| `.corner_brackets(rect, color)` | L-shaped corner reticle (focus ring / decoration). |
| `.text(rect, &str, color, size, TextAlign, bold)` | Text centered in `rect` (per `align` horizontally, vertically centered). |
| `.flash(rect, amount, radius)` | Brightening press-flash overlay (see `Flash`). |
| `.dim(rect, radius)` | Background scrim — the standard disabled look. |
| `.paint_base(&Base)` | Background/border/glow from a base's style. |

`DrawCommand` variants: `Rect`, `Brackets`, `Text`, `Scanline`, `Gradient`, `PushClip`/`PopClip`
(clip TODO), `Custom`. `Scene`: `new()`, `push`, `clear`, `len`, `is_empty`, `iter`.

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
    .child(Label::new("98%").font_size(32.0));
```

### Label

A single text run bound to a `Signal<String>`.

- **Construct**: `Label::new(text)`.
- **Builders**: `.align(TextAlign)`, `.color(Color)`, `.font_size(f32)`.
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
- **Builders**: `.variant(ButtonVariant)`, `.size(ButtonSize)` (`Small`/`Medium`/`Large`),
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
- **Builders**: `.value(text)` (initial), `.placeholder(text)`, `.on_change(impl Fn(Action))`.
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
- **Builders**: `.selected(index)` (initial, clamped), `.on_change(impl Fn(Action))`.
- **Accessors**: `.state() -> Signal<usize>`, `.index() -> usize`.
- **Emits**: `"tab-change"` / `SignalData::Usize`.

```rust
Tabs::new(["OVERVIEW", "SIGNALS", "LOGS"]).selected(0)
    .on_change(|a| if let SignalData::Usize(i) = a.data { show_tab(i); });
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

Embed `Base`, implement `Component` (override `paint`/`event`/`tick` as needed), and opt
into builder traits. Reuse `PaintCx` helpers (`rect`, `corner_brackets`, `text`, `flash`,
`dim`) so the Tron look stays consistent and DRY.
