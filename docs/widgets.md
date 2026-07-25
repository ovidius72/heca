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
- [Foundations](#foundations) — `Base`, `Component`, builder traits, `Style`, [Font sizing](#font-sizing), `Theme`/`GlowLevel`/`Intensity`, **[the glow model](#the-glow-model--who-owns-what)**, **[the focus model](#the-focus-model--ring-visibility)**, `Color`, signals, events, `Action`, `Scene`/`PaintCx`, `Flash`, `Attention`
- [Widgets](#widgets)
  - Layout: [`Flex`/`Container`](#flex--container), [`Surface`](#surface), [`Card`](#card), [`Pane`](#pane), [`Grid`](#grid), [`ScrollRegion`](#scrollregion), [`ScrollBar`](#scrollbar)
  - Text: [`Label`](#label)
  - Interactive: [`Button`](#button), [`IconButton`](#iconbutton), [`Toggle`](#toggle), [`Checkbox`](#checkbox), [`Input`](#input), [`Tabs`](#tabs), [`Select`](#select), [`Choice`](#choice), [`Item`](#item), [`Row`](#row), [`BadgeButton`](#badgebutton)
  - Display: [`Badge`](#badge), [`StatusDot`](#statusdot), [`Separator`](#separator), [`Spinner`](#spinner), [`Alert`](#alert), [`Toast`](#toast), [`ProgressBar`](#progressbar), [`Gauge`](#gauge), [`Icon`](#icon), [`Tag`](#tag)
  - Chrome (sidebars/docks): [`ItemGroup`](#itemgroup), [`MarkerGroup`](#markergroup), [`DockFrame`](#dockframe), [`ChromeRegion`](#chromeregion), [`RailCell`](#railcell), [`KeyHint`](#keyhint)
  - Overlays: [`Overlay`](#overlay) (the base layer), [`Tooltip`](#tooltip), [`Dialog`](#dialog), [`CommandPalette`](#commandpalette), [`ToastStack`](#toaststack)
- [Declarative UI model (`ViewNode`)](#declarative-ui-model-viewnode) — props/events by kind, slots, options-as-children, and **[the action an `Intent` names](#the-other-half-of-an-intent--the-action-it-names)** + [registering a custom action](#registering-a-custom-name-keyed-action)
- [Patterns](#patterns) — change events, reactive binding, focus, disabled, [placing a widget at an app-chosen rect](#placing-a-widget-at-an-app-chosen-rect), custom widgets

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
| `focusable` | `bool` | Whether the widget **opts into** keyboard focus (default `false`). Interactive widgets set it `true` — in the constructor (always-focusable controls) or when a callback is wired (e.g. a `Row`'s `.on_activate`). The `Component::focusable()` default is `focusable && !disabled`, so widgets no longer re-implement that check; only genuinely dynamic ones (an overlay focusable only while open) override the method. |
| `focus_barrier` | `bool` | Whether this widget is the **only** focus target in its subtree — focus traversal visits it but never descends into its children (default `false`). Set by controls that **compose** their content (`Button`, `Item`): a control is one click target, so it must be one Tab stop, whatever it holds. Without it a focusable child (a `Toggle` used as decoration) would take its own Tab stop while being click-dead, since the control consumes the press in its own `event`. Orthogonal to `style.hidden`, which drops a subtree from layout *and* focus. |
| `focus_visible` | `Signal<bool>` | Keyboard-vs-mouse focus flag (set by `FocusManager`: `advance` → `true`, click-focus → `false`). **Every widget gates its focus ring on `Base::shows_focus_ring()`** (= `focused && focus_visible` — CSS `:focus-visible` semantics), so a mouse click focuses a widget (Enter/Space work, the caret shows) **without** drawing the ring; only keyboard navigation rings. The ring is also removable app-wide via `[appearance] show_focus_border = false` (theme token `show_focus_border`). |
| `tab_index` | `Option<i32>` | Explicit Tab order (HTML-like). Set via `LayoutExt::tab_index`. |
| `children` | `Vec<Box<dyn Component>>` | Child components. |
| `font` | `f32` | **Resolved** font size in logical px, written by the layout pass (see [Font sizing](#font-sizing)). Widgets read **this** for text + measurement, not `style.font_size`. |

### `Component` trait

| Method | Default | Purpose |
|--------|---------|---------|
| `base(&self) -> &Base` | — | Required. |
| `base_mut(&mut self) -> &mut Base` | — | Required. |
| `focusable(&self) -> bool` | `base.focusable && !disabled` | Set `base.focusable = true` on an interactive widget instead of overriding this; override only for dynamic focusability (focusable only while open). |
| `overlay_active(&self) -> bool` | `false` | `true` while the widget owns an open overlay (e.g. a `Select` dropdown), so the host routes input to it first. |
| `overlay_occludes(&self, pos: Point) -> bool` | `false` | Whether the widget's **overlay surface geometrically covers** `pos`. Distinct from `overlay_active` (input grab): a non-grabbing toast card still occludes the points it covers; a **modal** ([`Dialog`](#dialog) scrim, open [`CommandPalette`](#commandpalette)) occludes the whole viewport; an open [`Select`](#select) occludes its panel rect. A host checks it (via the free fn `heca_grid_ui::overlay_occluded_at(root, pos)`, which scans a tree) before synthesizing a page-level action from raw input — e.g. right-click → context menu must not fire under an overlay. `ContextMenu` deliberately keeps the default so a second right-click re-anchors it. |
| `text_summary(&self) -> Option<String>` | first child that has one | The **accessible name** of the widget's content: the plain text of a composed subtree. `Label` supplies it; a `Choice` holding an `Icon` + `Label("HIGH")` summarizes to `"HIGH"`. It exists because a control sometimes needs the *text* of content whose type it cannot see (children are `impl Component`) — it is how [`Select`](#select) reports its value as text (`selected_label()`). Override it in a widget that renders text it owns. |
| `paint(&self, cx: &mut PaintCx)` | base chrome + children | Emit `DrawCommand`s. |
| `event(&mut self, ev: &Event) -> Handled` | route to children | Handle input. |
| `remeasure(&mut self)` | no-op | Recompute size from the resolved font (`Base::font`). The layout pass calls it on every node after resolving the font (see [Font sizing](#font-sizing)). Font-sized widgets override it. |
| `on_layout(&mut self)` | no-op | Called post-order once this node's (and its descendants') bounds are freshly computed. Override to **place** children the engine could not put where they are drawn — a [`ScrollRegion`](#scrollregion) re-bakes its scroll offset, a [`Select`](#select) re-places its option rows into the overlay panel. Bounds are natural again on entry, so the widget re-derives its shift from scratch instead of compounding it. |
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
| `.align(Align)` | Cross-axis alignment of the **children** (`Start`/`Center`/`End`/`Stretch`). In a `Row` that is vertical; in a `Column`, horizontal; in a [`Grid`](#grid), it is how items sit **vertically inside their cells**. |
| `.align_self(Align)` | Cross-axis alignment of **this** widget in its parent (CSS `align-self`), overriding the parent's `.align()` for it alone. `Align::Start` keeps an `Auto`-sized widget **hugging its content** instead of stretching to fill the parent — which is what the default `Stretch` would otherwise do (see [`Select`](#select), sized to its widest option). |
| `.justify_items(Align)` | **Grid only** — how the items sit **horizontally inside their cells** (CSS `justify-items`). Not the same as `.justify()`, which on a grid distributes the whole *track set*. |
| `.justify_self(Align)` | **Grid only** — horizontal placement of **this** item in its own cell, overriding the grid's `.justify_items()`. |
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

**`Parent`** (containers):

| Method | Effect |
|--------|--------|
| `.child(impl Component + 'static)` | Append a child. |
| `.child_boxed(Box<dyn Component>)` | Append an **already-boxed** subtree — one whose concrete widget type isn't known at the call site. `Box<dyn Component>` is not itself `Component`, so it cannot go through `.child()`. This is what the host's `realize()` (a [`ViewNode`](#declarative-ui-model-viewnode) tree) and a chrome provider's render seam both return. |

> A widget with **several** places to put children names them instead —
> [`DockFrame::header_boxed`](#dockframe), [`Dialog::body_boxed`](#dialog) — and those
> inherent methods win over the trait one. `.child_boxed` is the plain "append it to my
> children" case.

### `Style` & layout enums

`Style` is **two peer halves** — `style.layout` and `style.visual`. The split is the plugin
boundary, and it is one the library already lived by: every widget must read colours, fonts and
radii from the `Theme` and hardcode nothing, so "caller-owned" vs "theme-owned" was already a real
distinction here. The declarative boundary just falls on the same line.

**`Style.layout` — arrangement + the semantic `size` variant.** The half a declarative
[`ViewNode`](#declarative-ui-model-viewnode) may set: `direction`, `justify`, `align` (default
`Stretch`), `align_self` (`Option<Align>`, default `None` ⇒ follow the parent), `gap` (+
`gap_spacing`), `margin` (+ per-side overrides), `padding` (+ per-axis + spacing tokens),
`width`/`height` (`Length`), min/max sizes, `flex_grow`, `flex_shrink`, `hidden`, `grid_cell`,
`size`. `to_taffy()` lives here, because these are the fields it reads.

**`Style.visual` — appearance.** `fill`, `border`, `glow`, `radius`, `font_size`, `font_scale`.
A description may **never** set these: it carries semantic intent (a variant, a `size`, a colour
*name*) and the host resolves the pixels from the `Theme`
(`pluggable-chrome-plugin-plan.md` §2.6.1 rule C).

Why two types rather than a naming convention: `Layout` is serializable and `Visual` is not, so a
field added to `Visual` is unreachable from a description **by default** and a field added to
`Layout` is reachable **by default**. Neither needs an attribute, a list, or anyone remembering —
and there is no single line whose deletion would quietly open colours up to plugins.

`size` sits in `layout`, not `visual`, because it is semantic (`Small`/`Normal`/`Big`) rather than
a pixel value, and the layout pass both reads it and cascades it to children. `font_scale` is in
`visual` because it is a raw multiplier — use `size` for hierarchy a plugin may express.

Enums: `Direction{Row,Column}`, `Justify{Start,Center,End,SpaceBetween,SpaceAround}`,
`Align{Start,Center,End,Stretch}`, `Length{Auto,Px(f32)}`.

> `visual.font_size` defaults to `0.0` = **inherit the theme base font**; `visual.font_scale`
> defaults to `1.0`. See [Font sizing](#font-sizing). **Builder calls are unchanged** —
> `.gap(..)`, `.fill(..)`, `.font_size(..)` all work exactly as before; only the field path behind
> them moved, and `LayoutExt`/`StyleExt` write to the correct half for you.

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

**The variant cascades into composed content.** Like the base font, `WidgetSize` is **inherited down
the tree** by the layout pass: a child that never called `.size(..)` adopts its parent's variant, so
`Button::new("Save").icon(Glyph::Check).size(WidgetSize::Small)` shrinks the button *and* its `Icon`
and `Label`, at any depth. A child that *did* set one keeps it (and passes **that** to its own
children). `Style::size_explicit` records the difference — a raw `style.size = …` assignment is not
"explicit" and will be overwritten by the inherited value; use `LayoutExt::size` (or
`Style::set_size` for widgets whose own `size(..)` means something else, like `Icon`'s glyph pixels).

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
| `show_focus_border` | `bool` | Focus-ring **kill switch** (default `true`); every ring draw is gated on it. Overridable per-config via `[appearance] show_focus_border`. Rings additionally show only on **keyboard** focus (`Base::shows_focus_ring()`), never on click. |
| `glow_size` | `GlowLevel` | The **sole** owner of glow — scales every glow's halo radius **and strength**. `None` removes glow entirely. |
| `intensity` | `Intensity` | The **CRT scanline overlay** only (no longer touches glow). |
| `interaction.control_rest_glow` | `u8` | **Rest-state glow intensity** of control surfaces (×255; `30` ≈ 0.12; `0` = flat rest look). Button (Primary/Destructive/Outline), Input, Toggle track, Checkbox box, and the Select trigger halo faintly **at rest** with it — so a control shows the neon identity before hover/focus and `glow_size` visibly scales it at rest. Scaled by `glow_size` like every glow; disabled controls never halo. |
| `font_size` | `f32` | Base font every widget inherits (see [Font sizing](#font-sizing)). |
| `active_wash_alpha` | `f32` (0..1) | Opacity of the accent **wash** `DockFrame::active(true)` paints over an active frame (e.g. the active workspace). |
| `card_background_alpha` | `f32` (0..1) | Opacity of a sidebar/list card's resting background tint (e.g. each pane card). |

Helper: **`theme.control_radius()`** → `radius × 0.5` (corners for small controls).

> **Never hard-code font family/size, radius, border width or glow** — read them from the
> theme so a global change scales every widget proportionally.

- **`GlowLevel{None, Thin, Medium, Large}`** — glow halo size. `.radius_scale()` (0 / 0.5 /
  1.0 / 2.0), `.strength_scale()` (0 / 0.5 / 1.0 / 1.6), `.parse(&str)` (for config.toml),
  `GlowLevel::ALL`, `.label()`. `None` ⇒ no glow.
- **`Intensity{Off, Low, Medium, Heavy}`** — CRT scanline strength. `.scanline_opacity()`
  (Off=0 → Heavy=0.20), `.next()` (cycles). *Glow and intensity are independent* — glow is
  owned by `glow_size`, so changing intensity affects only the scanline overlay.

### The glow model — who owns what

One sentence: **the theme owns the glow COLOR, the `glow_size` setting owns how much glow there
is, and one token gives every surface a faint halo at rest.**

| Layer | Owner | What it controls |
|---|---|---|
| **Color** | theme `glow` token (per-widget **tone** overrides: a Destructive button halos in `danger`, a Toast/Alert/Tag in its severity/own color) | the halo hue |
| **Amount** | `glow_size` setting (`[appearance] glow_size` override → theme; `none\|thin\|medium\|large`) | presence + halo radius (`radius_scale`) + strength (`strength_scale`) of **every** glow, applied at the single `PaintCx` chokepoint (`scaled_glow`, inside `.rect(..)`) — nothing bypasses it |
| **Rest presence** | `interaction.control_rest_glow` token (×255 intensity; `0` = flat rest look) via **`PaintCx::rest_glow(radius)`** — the one shared definition; each widget passes only its halo radius | whether surfaces halo **before** any hover/focus/active state |
| **State glows** | each widget (hover sweep, toggle-on, checked pop, active pill/bar, attention pulse…) | drawn **on top** of the rest glow, same chokepoint |

**Carries the rest glow:** Button (every bordered variant — Primary/Secondary/Destructive/
Outline — at a tight `REST_GLOW_RADIUS`, far smaller than the hover halo), Input, Toggle track,
Checkbox **box**, Select trigger, Pane (all frame variants), DockFrame fill, **filled** `Row`
(list/pane cards), styled `ScrollRegion` (scrollable panel), Tag, Alert, Toast card (theme glow
color — a severity-toned halo under the always-accent bracket frame blended to a muddy fringe)
— plus the widgets that always glowed (Badge, StatusDot, active Tab, lit Gauge, MarkerGroup,
and any surface given an explicit StyleExt `.glow(..)`, which always **wins over** the rest
fallback).

**Deliberately flat at rest:** Ghost/Link buttons (surface-less — Ghost's fading-in hover
surface + border glow with the fade), unfilled `Row`/`Item` list rows, `Choice` option rows (the hosting
control owns the surface), the `Tabs` strip (no container surface; the active pill + underline
glow), `RailCell` (rest = bare icon), frameless `ScrollRegion`, layout shells
(`Flex`/`ChromeRegion`) — they have no surface, so there is nothing to halo. Disabled controls
never halo. `Button::glow(false)` opts a single button out of rest + hover glow.

**Icons/glyphs can glow** (since the glyph-halo work): `TextCmd` carries an optional glow just as
`RectCmd` does, and `heca-renderer` realizes it by **blurring the glyph's coverage mask** into its
own atlas entry and drawing that once behind the sharp glyph, with zero alpha so premultiplied
blending adds light without occluding.
Opt in per glyph with [`Icon::glow(true)`](#icon), or let a control publish a **content glow** its
composed glyphs inherit (how [`RailCell`](#railcell) lights its resting icon). It runs through the
same `scaled_glow` chokepoint as every other glow, so `glow_size` scales it and `none` removes it.
**Terminal cell glyphs never take it** — they are the hottest path in the app and a halo multiplies
a run's vertex count.

> The halo is a *renderer* choice, not a scene one: the scene says "this run glows", the renderer
> decides how. The blur is a true Gaussian (three box passes) computed once per
> `(glyph, size, sigma)` and cached in the atlas like any other glyph, so it costs **one** quad to
> draw and scales with `glow_size` for free.
>
> An earlier attempt drew the glyph many times around a ring instead. It was abandoned: a ring is a
> discrete shell, so the copies read as spokes and arcs rather than as light, and it degraded as the
> radius grew because a fixed tap count spreads thinner. Do not reintroduce it.

### The focus model — ring visibility

| Question | Answer |
|---|---|
| When does the ring draw? | Only on **keyboard** focus: every widget gates its ring on `Base::shows_focus_ring()` (= `focused && focus_visible`, CSS `:focus-visible`). A mouse click focuses the widget (Enter/Space work, the caret shows) but never rings; Tab/arrows (`FocusManager::advance`) ring. |
| Can I turn it off? | Yes — `[appearance] show_focus_border = false` (config override → theme `show_focus_border` token, default `true`); live-reloads with `prefix+Shift+r`. |
| How thick / what color? | `focus_border_width` (`[appearance]`, default 1.5 — independent of `border_width` so the ring survives borders-off) and the `focus_ring` theme token (unset ⇒ per-tone derivation via `effective_focus_ring()` / `focus_ring_tone()`). |
| What does it wrap? | The **control**, not its label: `Checkbox` rings its box only; `Toggle` its track; list rows (`Row`/`Item`/`Choice`) ring their row as the selectable unit. |

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
| `.drop_shadow(rect, radius, Shadow)` | Soft **drop shadow** behind a shape (dark, blurred, offset). Darkens the background (reads on dark themes, unlike the additive glow) and is independent of the glow/border tokens. Call before the shape's fill. Used by `Dialog` to lift off the scrim. |
| `.focus_ring(rect, color, radius)` | **The** keyboard focus outline for every widget — a thin accent-toned ring drawn *just outside* `rect` (CSS-`outline` style, offset gap), corner radius widened to stay concentric. Visible whether or not the widget has its own border (works on borderless Ghost/Link buttons). Width = `focus_border_width`; halo tracks `glow_size`. Pair with the theme's `focus_ring`/`effective_focus_ring()`/`focus_ring_tone()` for the color. |
| `.bracket_frame(rect)` | Decorative L-shaped corner-bracket reticle (Pane/DockFrame/Dialog chrome) — **decoration, not focus** (focus uses `.focus_ring`). |
| `.text(rect, &str, color, size, TextAlign, TextStyle)` | Text centered in `rect` (per `align` horizontally, vertically centered). `TextStyle { bold, italic }` is the **font** style — what the shaper does to the glyphs (`TextStyle::REGULAR` / `BOLD` / `ITALIC`, or `TextStyle::REGULAR.bold(is_active)`). Decorations (underline, strikethrough) are **not** here: a line is a rect, and the widget draws it — see [`Label`](#label). Italic is a *synthesized oblique*, since the embedded family has no italic face. |
| `.flash(rect, amount, radius)` | Brightening press-flash overlay (see `Flash`). |
| `.dim(rect, radius)` | Background scrim — the standard disabled look. |
| `.paint_base(&Base)` | Background/border/glow from a base's style. |
| `.with_overlay(\|cx\| …)` | Route the closure's draws to the scene's **overlay layer** (painted on top of everything) — used by dropdowns/popovers. Re-entrant: an overlay painted **inside** another overlay's paint (a `Select` in a `Dialog` body) records a **deeper segment**, and `Scene::overlay_segments()` yields segments depth-ordered — the nested panel composites above everything its parent draws, including what the parent paints *after* it. |
| `.with_content_color(color, \|cx\| …)` | Paint the closure's subtree with `color` as the **inherited content color** — `color` inheritance in the CSS sense. A control that *composes* its content (`Button`, `Item`) cannot set its children's colors (they are `impl Component`, so it doesn't know their types, and the `Theme` is only reachable in `paint`), so it publishes one state-derived value per frame and the children pull it. Because the control repaints while its hover eases, **the content animates with no per-child wiring**. |
| `.content_color() -> Option<Color>` | The inherited content color, if a parent published one. Widgets that render bare text/glyphs resolve: **own explicit color → this → a theme token** (usually `foreground`). A widget with an intrinsic semantic color (`Badge::danger`) ignores it. |
| `.with_translate(dx, dy, \|cx\| …)` | Paint the closure's subtree **translated** — the same components, drawn somewhere else. Deliberately narrow: a component is laid out in exactly one place, and its bounds are the contract for drawing *and* hit-testing alike. But a control occasionally has to render content it owns but does not hold — a [`Select`](#select) shows the chosen option in its trigger while that option is away in the open list. Nothing can be in two places, so the trigger draws a second **image** of it. What is drawn this way is **not interactive** (no bounds of its own ⇒ not hit-tested, focusable or hoverable); the control's own bounds are the click target. Never use it to *move* a widget — that is `shift_subtree` + `on_layout`, which keeps bounds honest. |

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

Layout-only flexible box — the workhorse for arranging children. **`Container`** and `container()`
are aliases. It's the tool for grouping: nest a `Flex` inside a `Flex` to build any arrangement,
including form fields (see below), so most layouts need no dedicated widget.

- **Construct**: `Flex::row()`, `Flex::column()`, `container()`.
- **Direction / distribution**: `.direction(Direction)`; `.justify(Justify)` (main-axis:
  `Start`/`Center`/`End`/`SpaceBetween`/`SpaceAround`/`SpaceEvenly`); `.align(Align)` (cross-axis:
  `Start`/`Center`/`End`/`Stretch` — the default `Stretch` makes an `Auto`-sized child fill the
  cross axis; `.align_self(Align)` overrides it for one child).
- **Gap between children**: `.gap(px)` for a raw value, or **`.gap_spacing(Spacing)`** for a
  **font-relative theme token** (`None`/`Xs`/`Sm`/`Md`/`Lg`) — resolved from the inherited font at
  layout, so it scales with the font, size variant, and UI zoom. **Prefer the token**; a raw px gap
  is tuned for one font size and wrong at every other.
- **Padding**: `.padding(px)` / `.padding_xy(x, y)` for raw px, or the tokens `.pad_all(Spacing)` /
  `.pad_x(Spacing)` / `.pad_y(Spacing)` (same font-relative scaling as `gap_spacing`).
- **Sizing** (from `LayoutExt`, shared by every widget): `.width(Length)` / `.height(Length)`
  (`Auto` / `Px` / `Pct`), `.grow(f32)` (flex-grow, absorb leftover space), `.margin*`.
- **Traits**: `LayoutExt`, `Parent`. (No `StyleExt` — it's purely arrangement; use
  [`Surface`](#surface) when you need a background/border/glow.)

```rust
Flex::row().gap(12.0).align(Align::Center)
    .child(StatusDot::online())
    .child(Label::new("GRID LINK"));
```

**Form fields — grouping with two gap scales (no `Field` widget needed).** A label and its control
are one *couple* (tight); couples are separated by a larger gap. Express it with two nested `Flex`
columns at different `gap_spacing` — the inner tight gap couples label↔control, the outer roomier gap
falls *between* fields:

```rust
let field = |label, control| Flex::column().gap_spacing(Spacing::Xs)   // tight: label ↔ its control
    .child(Label::new(label).color(theme.muted))
    .child(control);

Flex::column().gap_spacing(Spacing::Md)                                 // roomy: between fields
    .child(field("Confirm name", Input::new().value("pane-1")))
    .child(field("Archive target", Select::new(["SCRATCHPAD", "TRASH"])))
    .child(Checkbox::new().label("Also close its column"));
```

A [`Dialog`](#dialog) body already defaults its own children to `gap_spacing(Md)`, so dropping the
`field(...)` groups straight into `.body(...)` gives correct form spacing with no per-modal setup.

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
  MinContent, MaxContent}`); `.areas(["a b", "a c"])` named template areas (`.` or `_` = an empty
  cell); `.area(child, "name")` places a child in an area; `.cell(child, col, row, col_span,
  row_span)` explicit 1-based placement. A child placed by neither gets taffy's auto-placement; an
  unknown area name falls back to it too.
- **Boxed setters**: `.area_boxed(Box<dyn Component>, "name")` / `.cell_boxed(box, col, row,
  col_span, row_span)` — for a host mapper that has an *already-realized* subtree. (`Box<dyn
  Component>` is not itself `Component`, so it can't go through the `impl Component` setters; same
  seam as [`Dialog::body_boxed`](#dialog).)
- **Traits**: `LayoutExt`, `Parent`.

**Native:**

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

**Declarative:**

```rust
ViewNode::new(WidgetKind::Grid)
    .prop("columns", PropValue::List(vec![           // CSS-like track strings
        PropValue::Text("22px".into()),
        PropValue::Text("1fr".into()),
        PropValue::Text("auto".into()),
    ]))
    .prop("rows", PropValue::List(vec![PropValue::Text("auto".into())]))
    .prop("areas", PropValue::List(vec![PropValue::Text("dot title tag".into())]))
    // Placement is a prop on the CHILD: an area name…
    .child(ViewNode::new(WidgetKind::Icon)
        .prop("icon", PropValue::Glyph("terminal".into()))
        .prop("area", PropValue::Text("dot".into())))
    // …or an explicit 1-based cell (+ optional col_span / row_span; both default to 1).
    .child(ViewNode::new(WidgetKind::Label)
        .text("nvim")
        .prop("col", PropValue::Int(2))
        .prop("row", PropValue::Int(1)));
```

**Track vocabulary** (parsed by `realize`, case-insensitive, trimmed): `"22px"` (or a bare `22` /
`PropValue::Int`) → `Px` · `"1fr"` → `Fr` · `"auto"` → `Auto` · `"min"` / `"min-content"` →
`MinContent` · `"max"` / `"max-content"` → `MaxContent`. **Anything unrecognised degrades to
`Auto`** — never a panic, never an error: the model is untrusted input, so a typo costs its author a
differently-sized track, not a broken host. (No new schema is invented here; CSS grid already has
this vocabulary and plugin authors know it.)

#### The `areas` template defines the structure — `rows` / `columns` only *size* it

`.areas([...])` is the source of truth for the grid's shape: one string per row, one token per
column. `.rows(...)` / `.columns(...)` merely give sizes to the tracks the template implies. So if
the template has **more rows than there are row tracks**, the extra rows still exist — taffy creates
them **implicitly** (`Auto`-sized). Nothing errors; the grid just has more rows than you declared.

That is the source of the classic "my text isn't vertically centred" bug:

```rust
// WRONG — one row track, but a TWO-row template. `icon` spans both rows (the second is implicit),
// so it is centred over a taller area than `title` and the two stop sharing a centre line.
Grid::new()
    .rows([Track::Auto])
    .areas(["icon title status",
            "icon subtext ."])           // ← this line still creates a row
    .align(Align::Center)

// RIGHT — one row: one line in the template.
Grid::new()
    .rows([Track::Auto])
    .areas(["icon title status"])
    .align(Align::Center)
```

Rule of thumb: **count the lines in `areas` — that is how many rows you have**, regardless of what
`rows(...)` says.

#### Aligning items inside their cells (read this — the default surprises people)

**By default a grid item is pinned to the TOP-LEFT of its cell.** The default is `Stretch` on both
axes, and an item with an explicit size (which every leaf widget has — a `Label` measures to
`font × 1.4`, an `Icon` to `font`) has nothing to stretch, so it lands at the start of the cell in
both directions. Put an `Icon` and a `Label` in the same row and they will *not* share a centre
line — the icon sits a few pixels high. That is a real thing to fix, not a rendering artifact.

Four knobs, two axes — the grid sets the default, the item overrides it:

| | Vertical (block axis) | Horizontal (inline axis) |
|---|---|---|
| **on the Grid** (all items) | `.align(Align)` | `.justify_items(Align)` |
| **on one child** (overrides) | `.align_self(Align)` | `.justify_self(Align)` |

Values are `Align::{Start, Center, End, Stretch}` — `Stretch` (the default) makes an `Auto`-sized
item fill the cell, and pins a fixed-size one to the start.

> **The trap:** `.justify(...)` is **not** the horizontal item knob. On a grid it maps to CSS
> `justify-content`, which distributes the whole *track set* inside the container and leaves every
> item exactly where it was. Reach for `.justify_items(...)` / `.justify_self(...)`. (Vertically the
> word is unambiguous: `.align(...)` is what you want.)

**Native — a rich row whose items share a centre line:**

```rust
Grid::new()
    .columns([Track::Auto, Track::Fr(1.0), Track::Auto])
    .rows([Track::Auto, Track::Auto])
    .areas(["icon title   status",
            "icon subtext ."])
    .gap(8.0)
    // Vertical: centre every item in its cell. The icon spans both rows, so it centres across the
    // pair; the status dot centres against the title. Without this they all sit at the cell top.
    .align(Align::Center)
    .area(Icon::new(Glyph::Terminal), "icon")
    .area(Label::new("zsh").bold(true), "title")
    // Horizontal, for this item only: pin the dot to the right edge of its cell instead of
    // stretching it across the column.
    .area(StatusDot::online().justify_self(Align::End), "status")
    .area(Label::new("~/projects/heca").color(theme.colors.muted), "subtext");
```

**The same, per axis, in isolation:**

```rust
Grid::new().align(Align::Center)          // all items: centred vertically in their cell
Grid::new().justify_items(Align::Center)  // all items: centred horizontally in their cell
Grid::new().align(Align::Center).justify_items(Align::Center)   // dead centre

// One item departing from the grid's default (each axis is independent):
.area(Badge::success("RUN").align_self(Align::Start), "tag")     // top of its cell
.area(Badge::success("RUN").justify_self(Align::End), "tag")     // right of its cell
.area(Icon::new(Glyph::Terminal).align_self(Align::Stretch), "icon")  // fill the cell vertically
```

**Declarative — the same four knobs are props:**

```rust
ViewNode::new(WidgetKind::Grid)
    .prop("columns", PropValue::List(vec![
        PropValue::Text("auto".into()),
        PropValue::Text("1fr".into()),
        PropValue::Text("auto".into()),
    ]))
    .prop("areas", PropValue::List(vec![PropValue::Text("icon title status".into())]))
    // On the grid: the default placement of every item in its cell.
    .prop("align", PropValue::Align(ViewAlign::Center))            // vertical
    .prop("justify_items", PropValue::Align(ViewAlign::Center))    // horizontal
    .child(ViewNode::new(WidgetKind::Icon)
        .prop("icon", PropValue::Glyph("terminal".into()))
        .prop("area", PropValue::Text("icon".into())))
    .child(ViewNode::new(WidgetKind::Label)
        .text("zsh")
        .prop("area", PropValue::Text("title".into())))
    // On a child: override the grid, one axis each.
    .child(ViewNode::new(WidgetKind::StatusDot)
        .prop("area", PropValue::Text("status".into()))
        .prop("justify_self", PropValue::Align(ViewAlign::End)));   // hard right in its cell
```

`align_self` / `justify_self` are read for **every** kind, not just grid items — they describe a node
inside its parent, so they work on a flex child too (there, `align_self` is the cross axis and
`justify_self` is inert).

`Grid` is the one kind whose configuration is genuinely **list-shaped**, and the only reason
[`PropValue::List`](#the-model--a-node-is-four-things-all-its-own) exists. Placement lives on the
child rather than in a table on the parent, which keeps `ViewNode`'s shape flat — no second child
vector, nothing to keep in sync with the children.

### ScrollRegion

An embeddable **scroll viewport**: children laid out at their natural size (the
layout engine never shrinks them, so they overflow), clipped to the region's own
bounds. The visible window is the `ScrollRegion` itself; content beyond it is
clipped (`PushClip`). It is a **dumb viewport** — it owns no selection state;
selection/cursor is the host container's concern, and the region just scrolls
where it's told (see *Real-app integration* below).

**Axes.** Vertical by default (back-compat); opt into horizontal with
`.horizontal()` or both with `.both()`. Each axis whose content overflows grows
its own scrollbar (vertical on the right edge, horizontal on the bottom).

**Scrollable surface.** `ScrollRegion` implements `StyleExt`, so a plain one is
frameless while `.background(..).border(..).radius(..)` makes it a framed,
scrollable panel — all values from the `Theme`, none hardcoded.

**Mechanism — same as the whole-page scroll.** Rather than a separate
translation layer, `ScrollRegion` reuses the page-scroll pattern: it bakes
`-scroll_offset` (both axes) into its children's **bounds** (so paint,
hit-testing, and DnD all see the *visual* position — bounds === what's drawn) and
clips to its own rect via `PushClip`. Because bounds always match the visual,
pointer routing and the drag framework's `source_at`/`resolve_at` just work while
scrolled. A fresh layout pass would compound the shift, so the layout engine's
post-order `on_layout` hook resets the baked offset (children back at natural) and
re-applies it from scratch — no compounding across relayouts. `on_layout` also
**re-clamps both axes**: a relayout that grew the viewport (window resize,
zoom-out) would otherwise re-apply a stale offset beyond the new max, leaving
content shifted past the edge with no scrollbar to bring it back.

- **Construct**: `ScrollRegion::new()`. Append children with [`Parent::child`].
  Give it a fixed `.height()` (and usually `.width()`) via `LayoutExt` so the
  content actually overflows; otherwise it sizes to its children and never scrolls.
- **Axes**: `.horizontal()`, `.both()`, or `.axes(ScrollAxes::…)` (default
  `Vertical`). Only enabled axes shift/clip/scrollbar.
- **Builders**: `LayoutExt`, `StyleExt` (surface framing), `Parent`.
- **Scroll position**: `.scroll_offset() -> Signal<f32>` / `.scroll_to(f32)`
  (vertical) and `.scroll_offset_x() -> Signal<f32>` / `.scroll_to_x(f32)`
  (horizontal). Each `scroll_to*` clamps to `[0, max_offset]`, **bakes the shift
  into bounds immediately**, repaints, and returns the applied value. Prefer them
  over raw `.set()` — they keep the shifted bounds (paint/hit-testing/DnD) in sync.
- **Scroll-into-view** (host cursor following): `.ensure_visible(rect)` scrolls
  minimally so a descendant's current `bounds` (visual space) is fully inside the
  viewport — above → align tops, below → align bottoms, already visible → no-op.
  `.scroll_to_child(index)` is the convenience for a flat list of direct children.
  The widget recovers natural positions via its baked shift, so the host never
  tracks the offset or does offset math. (Vertical axis.)
- **Wheel** (built-in, hover-gated): plain wheel scrolls **vertically**,
  **`Shift`+wheel horizontally**, and a trackpad's 2-D delta drives both — the
  host maps modifiers→axis (`Event::Scroll` carries `delta_x`/`delta_y`). ~10% of
  the viewport per notch (viewport-proportional). `Event::Scroll` has no position,
  so the region tracks the cursor via `PointerMoved` and only swallows the wheel
  when hovered (and that axis is scrollable); otherwise it propagates to the host.
  **Nested regions compose**: the wheel is offered to **children first**, so the
  *innermost* hovered scrollable consumes it (each region gates on its own hover)
  and an outer whole-page region only scrolls when no descendant did.
- **Scrollbar thumbs** (built-in): auto-shown per overflowing axis; **draggable**.
  A theme-**accent** grip that brightens on hover/drag (mirroring `MarkerGroup`'s
  grip bar), in a wider invisible **grab lane** (16px) so the thin 8px thumb is
  easy to click. Radius from `Theme::control_radius()`, color from `theme.accent`
  (nothing hardcoded). Each bar reserves a **gutter**: content is clipped short of
  the lane so no content sits under a thumb, and each bar's track stops short of
  the other's gutter so they never overlap in the corner.
- **Click-in-track paging**: clicking the scrollbar track above/below (or left/
  right of) the thumb pages a screenful toward the click — the standard affordance.
  **Press-and-hold repeats**: after a short initial delay (0.35s) the held press
  keeps paging toward the cursor at a fixed rate (every 0.1s, via `tick`), pausing
  when the thumb reaches the pointer and stopping on release.
- **Keyboard — NONE, on purpose.** The region binds **no keys** and is **not
  focusable / not a tab-stop**. heca is tmux-style: plain keys belong to the
  underlying app (terminal/editor), so the widget must not swallow them. Keyboard
  scrolling is a **host** concern — the app dispatches **prefix-gated, configurable
  scroll actions** (`WmAction` → `ActionRegistry`, RPC-ready) that call
  `scroll_to`/`scroll_by`/`ensure_visible`. (App action layer: F003/P011/T012.)
- **Whole-page scroll**: size a `.both()` region to the window and put the page
  inside it — that IS the page scroll (the showcase does exactly this; no manual
  bounds-shifting). Lay the page child at its **natural width** (don't stretch it:
  `align(Align::Start)` on the region) so the region sees horizontal overflow from
  its direct child; overlay widgets (Dialog / palette / menus / toasts) must be
  hosted in a **layer above the region**, never inside the scrolled content (see
  §Dialog).
- **Traits**: `LayoutExt`, `StyleExt`, `Parent`.
- **Current scope**: two-axis, nested-region wheel composition. Future: a
  dedicated scrollbar color token and PageUp/PageDown as app actions.

**Native.**
```rust
// A two-axis, framed scrollable surface driven from the host.
let mut grid = ScrollRegion::new()
    .both()
    .height(Length::Px(180.0))
    .width(Length::Px(300.0))
    .background(theme.colors.surface)     // StyleExt → scrollable panel
    .border(theme.colors.accent, 1.0);
for i in 1..=25 {
    grid = grid.child(Item::new(format!("item {i:02}")));
}
grid.scroll_to(0.0);      // vertical
grid.scroll_to_x(40.0);   // horizontal
```

**Declarative (`ViewNode`).** `WidgetKind::Scroll` realizes to a `ScrollRegion`
(children attached); axis/style are host-side today (the app builds the styled,
two-axis region and mounts a realized subtree inside it).

> **Real-app integration (sidebar):** selection is container-owned, not widget
> state. Mount the sidebar tree (DockFrames + rows) inside a `ScrollRegion`; the
> `SidebarNav` cursor handler (selection-driven) calls
> `region.ensure_visible(selected_row.bounds)` (or `scroll_to_child`) after moving
> the cursor to keep it on screen. The row's visual state stays container-driven
> via `Item::marker`/`state`. Keyboard *scrolling* of the region is separate: it
> comes from the app's prefix-gated scroll actions, not from the widget. See
> [`heca-renderer/examples/showcase.rs`](../heca-renderer/examples/showcase.rs) for
> the wheel / thumb / click-track / two-axis demo.

### Label

A single text run bound to a `Signal<String>`.

- **Construct**: `Label::new(text)`.
- **Builders**: `.align(TextAlign)`, `.color(Color)`, `.font_size(f32)` (pin a size),
  `.font_scale(f32)` (multiplier vs the inherited base font — prefer this for hierarchy), and the
  four text attributes: `.bold(bool)`, `.italic(bool)`, `.underline(bool)`, `.strikethrough(bool)`.
- **Accessors**: `.text_signal() -> Signal<String>` (set it to update reactively), plus a signal per
  attribute — `.bold_signal()`, `.italic_signal()`, `.underline_signal()`, `.strikethrough_signal()`
  (all `Signal<bool>`). Flip one to restyle the label **in place**, with no rebuild: an enclosing
  widget drives it for a state-dependent look — this is how [`Item`](#item) bolds its label while
  active (the inherited paint context carries a color, but not a weight), and how a link underlines
  on hover.

#### The four text attributes — two are font, two are not

The split matters, because it is why underline exists at all without touching the renderer:

| | | Who renders it |
|---|---|---|
| `bold` | **font attribute** | The shaper picks the glyphs — the embedded family ships a real bold face. |
| `italic` | **font attribute** | A **synthesized oblique**: the glyphs are *sheared*. The embedded family (Geist Mono) has **no italic face**, and asking the shaper for a real one would substitute a *proportional* fallback — which would break the monospace advances every measure in this library assumes. The shear keeps the same face, same widths, just slanted. |
| `underline` | **decoration** | The **label** draws it — a line is not a glyph, it is a rect. |
| `strikethrough` | **decoration** | Likewise. |

The decorations are painted in the label's **resolved color** (own → inherited content color → theme
foreground), so an underlined label inside a `Button` tints with the button's hover/disabled state
like everything else — no wiring, same mechanism as the glyphs.

**Geometry** (all font-relative, so a rule under 10px text is hairline and one under a 28px header is
proportionate — there are no pixel constants to go stale):

| | Value |
|---|---|
| thickness | `font × 0.07`, floored at **1px** so it never vanishes |
| underline | `0.42 × font` **below** the run's centre line — clear of the descenders |
| strikethrough | `0.06 × font` **above** the centre line — a line through the exact middle reads low, because lowercase mass sits above the box centre |

And the part that is easy to get wrong: **a decoration follows the text run, not the label's box.** A
`Label` normally hugs its text, but a *container* can widen a child's bounds — a [`Select`](#select)
does exactly that to its option rows, so the selection pill spans the panel — and `align` then
decides where the run sits inside that wider box. A rule spanning the box would be mostly empty line.
An empty label has a zero-width run and draws no rule at all.

**Color is inherited when unset.** With no explicit `.color(..)` the label paints in the
[content color](#scene--drawcommand--paintcx-for-building-widgets) published by an enclosing control
(`Button`, `Item`, `Choice`), falling back to `theme.foreground` when there is none. That is what
makes a label composed inside a button track that button's hover/disabled state with no wiring
between the two. Calling `.color(..)` opts out of the inheritance.

**It names the subtree it sits in.** `Label` is the leaf that supplies
[`Component::text_summary`](#component-trait) — the accessible name a control reads when it needs the
*text* of content whose type it cannot see (a [`Select`](#select) reporting its current value).
Nothing to wire: composing a `Label` into an option is what gives that option a name.

**Native:**

```rust
let status = Label::new("ONLINE").color(theme.foreground).font_size(14.0);
let sig = status.text_signal();
// later: sig.set("OFFLINE".into());

// The four text attributes, alone and combined — they compose freely.
Label::new("BOLD").bold(true);
Label::new("ITALIC").italic(true);                      // synthesized oblique (see above)
Label::new("LINK").underline(true);
Label::new("DONE").strikethrough(true);
Label::new("ALL FOUR").bold(true).italic(true).underline(true).strikethrough(true);

// State-driven, in place — no rebuild. A link that underlines while hovered:
let link = Label::new("docs/widgets.md");
let underline = link.underline_signal();
// in the enclosing widget's event/tick:  underline.set(hovered);

// A completed to-do: the parent strikes its own label through when the row is done.
let row = Label::new("Ship the release");
let done = row.strikethrough_signal();
// done.set(true);
```

**Declarative** (`WidgetKind::Label`):

```rust
ViewNode::new(WidgetKind::Label)
    .text("DONE")
    .prop("bold", PropValue::Bool(true))
    .prop("italic", PropValue::Bool(true))
    .prop("underline", PropValue::Bool(false))
    .prop("strikethrough", PropValue::Bool(true));
```

Props `realize` reads: `text`, `bold`, `italic`, `underline`, `strikethrough` (all `Bool`, all
default `false`). The signals are **not** exposed declaratively — a `ViewNode` is static data and
cannot carry a live signal (same rule as [`ScrollBar`](#scrollbar) being host-only); a plugin author
re-emits the node with the new value instead.

### Button

Interactive surface; look driven by variant × size, with animated per-variant hover and a
press flash. Focusable; Space/Enter activate like a click. Every bordered variant
(Primary/Secondary/Destructive/Outline) carries a faint theme **rest glow**
(`interaction.control_rest_glow`, tone follows the variant) so `glow_size` visibly scales them
before hover/focus; the surface-less Ghost/Link stay flat at rest (Ghost's fading-in hover
surface glows with it); `.glow(false)` disables both the rest and hover glow.

**Its content is composed from child components** — the button paints only its own chrome (fill,
border, hover sweep, press flash, focus ring, all from the `Theme`) and lets the layout engine place
its children, which paint themselves. So a button can hold a label, an icon + a label, or an
arbitrary tree of any depth. The convenience forms are **sugar that builds those same children**;
there is no separate "simple mode".

#### The two ways to build a Button — same widget, same retained tree

Native code (chrome/sidebar) uses the **builder API** because it needs closures and signals;
plugins / RPC / modal bodies use the **declarative `ViewNode`** because it must serialize. `realize`
turns the second into the first. Both spellings, in both forms:

| | Native (builder API) | Declarative (`ViewNode`) |
|---|---|---|
| **Simple** — a label | `Button::destructive("Delete")` | `ViewNode::new(WidgetKind::Button).text("Delete").prop("variant", …Destructive)` |
| **Simple + icon** | `Button::destructive("Delete").icon(Glyph::Trash)` | …the same, plus `.prop("icon", PropValue::Glyph("trash".into()))` |
| **Composed** — any tree | `Button::empty().child(…)` | `ViewNode::new(WidgetKind::Button).child(…)` |

```rust
// ── 1. SIMPLE (native) ───────────────────────────────────────────────────────────────
// The label is sugar: it becomes a bold `Label` child. Every existing call site is this.
Button::destructive("Delete").on_click(|| confirm_delete());

// With a leading icon — sugar again: children become [Icon, Label].
Button::destructive("Delete").icon(Glyph::Trash).on_click(|| confirm_delete());

// ── 2. SIMPLE (declarative) ──────────────────────────────────────────────────────────
// A CHILDLESS node: the scalar props describe the content, and realize() builds the very
// same [Icon, Label] children the native sugar does.
ViewNode::new(WidgetKind::Button)
    .text("Delete")
    .prop("icon", PropValue::Glyph("trash".into()))
    .prop("variant", PropValue::Variant(ViewVariant::Destructive))
    .on_press(Intent::new("confirm_ok"));

// ── 3. COMPOSED (native) ─────────────────────────────────────────────────────────────
// Content is just children — any tree, any depth. The Icon and both Labels carry no color
// of their own, so they inherit the button's state color and animate with its hover.
Button::empty()
    .variant(ButtonVariant::Destructive)
    .child(Flex::column().gap(4.0)
        .child(Flex::row().gap(6.0)
            .child(Icon::new(Glyph::Trash))
            .child(Label::new("Delete")))
        .child(Label::new("Ctrl+D").font_scale(0.75)))
    .on_click(|| confirm_delete());

// ── 4. COMPOSED (declarative) ────────────────────────────────────────────────────────
// The identical tree, as data. Children WIN: with children present, `text`/`icon` are ignored.
ViewNode::new(WidgetKind::Button)
    .prop("variant", PropValue::Variant(ViewVariant::Destructive))
    .on_press(Intent::new("confirm_ok"))
    .child(ViewNode::new(WidgetKind::Column).prop("gap", PropValue::Int(4))
        .child(ViewNode::new(WidgetKind::Row)
            .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("trash".into())))
            .child(ViewNode::new(WidgetKind::Label).text("Delete")))
        .child(ViewNode::new(WidgetKind::Label).text("Ctrl+D")));
```

**Precedence: children win.** A node *with* children is realized as an empty button holding them; a
**childless** node falls back to the scalar sugar (`text` + optional `icon`). One content model, two
spellings — never two paint paths.

- **Construct**: `Button::new(label)` (= primary, one bold `Label` child) ·
  `Button::{primary,secondary,destructive,outline,ghost,link}(label)` · **`Button::empty()`** (no
  content — compose it yourself).
- **Content builders**: `.icon(Glyph)` (prepend a leading `Icon` → children `[Icon, Label]`) ·
  `.child(impl Component)` (`Parent` — append **any** component, at any depth) ·
  `.content_boxed(Box<dyn Component>)` (mount a subtree from a mapper — what `realize(&ViewNode)`
  returns; mirrors `Dialog::body_boxed`).
- **Look builders**: `.variant(ButtonVariant)` · `.size(WidgetSize)` (`Small`/`Normal`/`Large`/`Header`
  — scales font **and** padding, and **cascades into the content**) · `.font_size(f32)` (pin an
  explicit size) · `.glow(bool)` (hover glow, default on) · `.bordered(bool)` (default on).
- **Behavior builders**: `.on_click(impl Fn() + 'static)`, plus the shared `LayoutExt`
  (`.disabled(bool)`, `.tab_index(i32)`, `.width/.height`, …) and `.hint_target(id)`.
- **Accessor**: `.hovered() -> Signal<bool>`.
- **Variants**: `Primary`, `Secondary`, `Destructive`, `Outline`, `Ghost`, `Link`.

**Sizing — the button doesn't compute it.** It hugs its content (`Auto` + padding), so taffy
measures whatever it holds; richer content simply makes the button bigger. Horizontal padding
carries one character of breathing room per side, which reproduces the historical label-only
geometry exactly.

**Content color is inherited, not assigned.** Each frame the button publishes one state-derived
color via [`PaintCx::with_content_color`](#scene--drawcommand--paintcx-for-building-widgets), and
unstyled children (`Label`, `Icon`) pick it up — which is how composed content **animates with the
hover sweep and fades when disabled** without the button knowing its children's types. A child with
its own `.color(..)`, or an intrinsic semantic color (`Badge::danger`, `StatusDot::online`), keeps it.

**One control, one target.** The button is a single click target and — via
[`Base.focus_barrier`](#base) — a single **Tab stop**, whatever it contains. An interactive child
(a `Toggle`) would render but never get its own clicks or focus.

- **Focus indicator**: like every widget, Button draws `PaintCx::focus_ring` — a thin accent-toned
  **outline just outside** the button (so it shows even on borderless `Ghost`/`Link`), tinted by the
  theme's `focus_ring` token / `effective_focus_ring()` for accent variants and
  `focus_ring_tone(danger)` for `Destructive`. Shown whenever the button is `focused`.
- **Disabled look** (`.disabled(true)`): the chrome drops its vivid accent/danger tone to `muted`
  and the **content color** fades to `muted` at a reduced alpha, so the inactive state reads on
  **every** variant — including transparent `Ghost`/`Link`, where a background scrim is invisible.
  Theme-driven (no hardcoded colours); the button is also inert and unfocusable. Used e.g. by a
  modal's OK button while a required form field is blank ([`Dialog`](#dialog) → *Declaring a modal
  from data*).

**Native (builder API):**

```rust
// Sugar — the common cases.
Button::destructive("DEREZ").size(WidgetSize::Large).on_click(|| wm.derez_focused());
Button::primary("Save").icon(Glyph::Check);          // → children [Icon, Label]

// Composed — arbitrary tree, any depth. The Icon/Labels inherit the button's state color.
Button::empty()
    .variant(ButtonVariant::Destructive)
    .child(Flex::column().gap(4.0)
        .child(Flex::row().gap(6.0)
            .child(Icon::new(Glyph::Trash))
            .child(Label::new("Delete")))
        .child(Label::new("Ctrl+D")))
    .on_click(|| confirm_delete());
```

**Declarative (`ViewNode` — plugins / RPC / modal bodies):** `realize` reads `text`, `icon`
(Glyph **name**), `variant`, `size`, and the `press` event. **Precedence: `children` win** — a node
*with* children is realized as an empty button holding them; a **childless** node falls back to the
scalar sugar (`text` + optional `icon`), which produces the identical children.

```rust
// Sugar form (childless) — text + icon props.
ViewNode::new(WidgetKind::Button)
    .text("Delete")
    .prop("icon", PropValue::Glyph("trash".into()))
    .prop("variant", PropValue::Variant(ViewVariant::Destructive))
    .on_press(Intent::new("confirm_ok"));

// Composed form — children are the content (the props above are then ignored).
ViewNode::new(WidgetKind::Button)
    .prop("variant", PropValue::Variant(ViewVariant::Destructive))
    .on_press(Intent::new("confirm_ok"))
    .child(ViewNode::new(WidgetKind::Column).prop("gap", PropValue::Int(4))
        .child(ViewNode::new(WidgetKind::Row)
            .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("trash".into())))
            .child(ViewNode::new(WidgetKind::Label).text("Delete")))
        .child(ViewNode::new(WidgetKind::Label).text("Ctrl+D")));
```

Both spellings produce the **same retained tree** — see
[the declarative UI model](#declarative-ui-model-viewnode).

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

Sliding on/off switch (translucent accent fill + light knob + glow when on; the track carries
the faint theme rest glow — `interaction.control_rest_glow` — while off). Focusable;
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
either side. Focusable; Space/Enter or a click anywhere on box/label toggles it. The
**box** (not the label) carries the faint theme rest glow (`interaction.control_rest_glow`),
and the keyboard focus ring wraps the **box only** — the standard control ring; the label
stays clickable but un-ringed.

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
mouse/keyboard selection + editing model. Focusable. The field carries the faint theme
rest glow (`interaction.control_rest_glow`).

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
`Event::Widget(WidgetIntent::{EditDeleteBack, EditDeleteToLineStart, EditSelectAll})`. The **host**
resolves these from the `[keys.widgets]` bindings and delivers them field-first (through a `Dialog`
to the focused field):

| Intent | Default binding (config name) | Effect |
|--------|-------------------------------|--------|
| `EditDeleteBack` | `Ctrl+h` (`edit_delete_back`) | delete one char before the caret |
| `EditDeleteToLineStart` | `Ctrl+u` (`edit_delete_to_line_start`) | delete from the caret to line start |
| `EditSelectAll` | `Ctrl+a` / `Super+a` (`edit_select_all`) | select the whole field |

App wiring: `build_widget_keymap` → `AppState.widget_keymap` (`heca_grid_ui::Keymap`), dispatched in
the overlay key branch (`heca/src/app/events.rs`). See the `widget-keys-config` requirement and README.

```rust
let name = Input::new().placeholder("CALLSIGN")
    .on_change(|a| { if let SignalData::String(s) = a.data { store(s); } });
```

**From a plugin (`ViewNode`).** A plugin never sends an `Edit*` intent itself — it declares an
`Input`, and the host owns the keyboard model + shortcut resolution above. (See the plugin
props/events under the `ViewNode` note below.)

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

Horizontal segmented selector with an animated sliding underline. Focusable, and **one Tab stop**
([`Base.focus_barrier`](#base)) — the individual tabs are not separate stops. Click selects.

**Its segments are [`Choice`](#choice) children** — the same option primitive [`Select`](#select)
mounts as its dropdown rows. So a tab can be anything: a word, an icon + a label, a label with a
count `Badge`. `Tabs` owns only the strip's chrome (the underline, the focus ring); each tab draws
itself and tints its own content when selected.

**The underline slides between the selected child's real bounds.** It follows whatever the tab
actually *is* — no monospace metrics, no segment arithmetic — so it is correct for a tab holding an
icon or a badge, which a char-count could never have measured. The strip **hugs its tabs** in both
axes; the underline's band is reserved as a bottom margin on the tabs, so the strip measures to "the
tallest tab + the band" without anyone computing a height.

- **Construct**: `Tabs::new(labels)` — `labels: impl IntoIterator<Item = impl Into<String>>`,
  **sugar** that builds a `Choice::labeled(text, text)` per tab (the value *is* the text) ·
  `Tabs::empty()` — no tabs, compose them.
- **Content**: `.tab(Choice)` — appends a composed tab. Typed to `Choice` for the same reason
  `Select::option` is: the strip keeps the tab's `state()` / `hovered()` signals so it can drive them
  in place.
- **Builders**: `.selected(index)` (initial, clamped — call it **after** the tabs), `.font_size(f32)`
  (else inherits), `.on_change(impl Fn(Action))`, plus `LayoutExt`.
- **Accessors**: `.state() -> Signal<usize>`, `.index() -> usize`, `.selected_label() -> String` (the
  selected tab's [text summary](#component-trait)).
- **Emits**: `"tab-change"` / `SignalData::Usize` (the index — unchanged).
- **Keys** (`widget-keys-config`): navigation is host-configured, not hardcoded. As a **horizontal**
  selector the widget moves selection on the semantic `Event::Widget(WidgetIntent::{ItemPrevious,
  ItemNext})` (left/right). The host resolves the configurable `item_previous` / `item_next`
  `[keys.widgets]` bindings into it (defaults ←/`Ctrl+h` → previous, →/`Ctrl+l` → next) via
  `Keymap::dispatch` — delivering to the focused widget.

**Native:**

```rust
// Sugar — plain text tabs.
Tabs::new(["OVERVIEW", "SIGNALS", "LOGS"]).selected(0)
    .on_change(|a| if let SignalData::Usize(i) = a.data { show_tab(i); });

// Composed — a tab is a value plus any content; the underline spans whatever it is.
Tabs::empty()
    .tab(Choice::new("files").child(Icon::new(Glyph::FolderOpen)).child(Label::new("FILES")))
    .tab(Choice::new("issues").child(Label::new("ISSUES")).child(Badge::danger("3")))
    .selected(1)
    .on_change(|a| if let SignalData::Usize(i) = a.data { show_tab(i); });
```

**Declarative:**

```rust
ViewNode::new(WidgetKind::Tabs)
    .prop("selected", PropValue::Int(1))
    .on("change", Intent::new("show_tab"))
    .child(ViewNode::new(WidgetKind::Choice).prop("value", PropValue::Text("files".into()))
        .child(ViewNode::new(WidgetKind::Label).text("FILES")))
    .child(ViewNode::new(WidgetKind::Choice).prop("value", PropValue::Text("issues".into()))
        .child(ViewNode::new(WidgetKind::Label).text("ISSUES"))
        .child(ViewNode::new(WidgetKind::Badge).text("3")));
// The `change` intent fires with args {"value": "issues"} — the tab's value, not an opaque index.
```

Props `realize` reads: `selected` (`Int`). Children: `Choice` nodes (a non-`Choice` child is
ignored). The `change` intent carries the chosen option's **value**, not its index — see
[Options are children](#options-are-children-select--tabs--choice).

### Select

Single-select dropdown — the first **overlay** widget. The trigger shows the current value; the open
option list paints in the scene's overlay layer (on top of everything) and the widget reports
`overlay_active()` so the host routes input to it first (see
[Overlay layer](#scene--drawcommand--paintcx-for-building-widgets)); while open,
`overlay_occludes(pos)` reports the **panel rect**, so a host's page-level gates (e.g. right-click →
context menu) skip points the list covers (see [`Component` trait](#component-trait)). Focusable,
and **one Tab stop** ([`Base.focus_barrier`](#base)) — the options are not separate stops. The
trigger carries the faint theme rest glow (`interaction.control_rest_glow`).

**Its options are [`Choice`](#choice) children.** So an option is not a string: it is a value plus
whatever content you compose — an icon and a label, a two-line row, a `Badge`. `Select` owns only the
*chrome* (trigger, chevron, panel, keyboard cursor, scrollbar); each row draws itself, and tints its
own content when chosen.

**The trigger shows the chosen option itself — icon and all, open or closed.** Closed, the option
*is* the trigger's content: it is a real component, stood inside the trigger box, painting there.
Open, that same component has moved into the list — a component is laid out in exactly one place — so
the trigger draws a second **image** of its content with
[`PaintCx::with_translate`](#scene--drawcommand--paintcx-for-building-widgets), translated from the
panel back into the trigger. Only the content is echoed, never the row's chrome: the trigger's own
box is its chrome, and a selection pill inside it would be a box in a box.

The open panel is **opaque**: it consumes pointer moves over itself, so widgets behind it don't light
up as hovered.

The trigger **width hugs the widest option** (the engine measures the real rows — icons included —
instead of counting characters), and everything scales with the font and the size variant. The open
panel **flips above** the trigger when there's no room below, **caps** its visible rows to what fits
in the `PaintCx` viewport, and **scrolls** internally (scrollbar; wheel / keyboard) for longer lists.
The panel's geometry comes from the shared placement authority
([`place_anchored_on`](#overlay), anchored to the trigger rect and clamped into the viewport). The
flip **side is passed in, not re-derived**: opening the list picks the side and the visible-row count
*together* (the panel's height depends on the side), so the placement honours that decision rather
than risking a disagreement with the row count. Its **presentation** is the shared
[`paint_panel_chrome`](#overlay) (drop shadow + surface fill + bracket reticle) with the dropdown's
own accent border and glow layered on — so an open list reads as the same surface as a `Dialog`
panel, while still looking like an open control. The rows stay *placed children* of the `Select`
(that is why it paints the panel itself rather than composing an `Overlay`).

- **Construct**: `Select::new(options)` — `options: impl IntoIterator<Item = impl Into<String>>`,
  **sugar** that builds a `Choice::labeled(text, text)` per option (the value *is* the text) ·
  `Select::empty()` — no options, compose them.
- **Content**: `.option(Choice)` — appends a composed option. Typed to `Choice` on purpose: the
  control keeps the row's `state()` signal so it can flip the selection **in place** (a
  `Box<dyn Component>` would have erased it), and the rows of a select genuinely *are* options.
- **Builders**: `.selected(index)` (initial, clamped — call it **after** the options),
  `.font_size(f32)` (else inherits), `.on_change(impl Fn(Action))`, plus `LayoutExt`.
- **Accessors**: `.state() -> Signal<usize>`, `.index() -> usize`, `.selected_label() -> String` —
  the chosen option's [text summary](#component-trait), i.e. its content's accessible name (`"HIGH"`
  for an `Icon` + `Label("HIGH")` option). Use it to read the current value as text.
- **Emits**: `"select-change"` / `SignalData::Usize` (the index — unchanged; `realize` maps it back
  to the chosen `Choice`'s **value** for declarative authors).
- **Keys** (`widget-keys-config`): a **closed** trigger opens on a raw `Enter` / `Space` / `↓`
  (activation, like a button). The **open** list is a **vertical** overlay driven by the semantic
  `Event::Widget` intents — shared with [`ContextMenu`](#contextmenu) / [`CommandPalette`](#commandpalette):
  `MenuUp`/`MenuDown` move the cursor (scroll into view), `Activate` commits, `Dismiss` closes. The
  host resolves the configurable `menu_up`/`menu_down`/`activate`/`dismiss` `[keys.widgets]` keys into
  it (defaults ↑/`Ctrl+k`, ↓/`Ctrl+j`, Enter, Esc). Click a row to choose, click outside to close;
  wheel scrolls the open list.

```rust
// Sugar — plain text options.
Select::new(["LOW", "MEDIUM", "HIGH"]).selected(1)
    .on_change(|a| if let SignalData::Usize(i) = a.data { set_level(i); });

// Composed — a value plus any content. The Icon + Label tint together when the row is chosen.
Select::empty()
    .option(Choice::new("low").child(Icon::new(Glyph::Circle)).child(Label::new("LOW")))
    .option(Choice::new("high").child(Icon::new(Glyph::Lightning)).child(Label::new("HIGH")))
    .selected(0)
    .on_change(|a| if let SignalData::Usize(i) = a.data { set_level(i); });
```

**Declarative** — the same control, authored as data (a plugin / RPC / a modal body). The options are
**children**, never a `props["options"]` list of strings, which is the whole point: a declarative
option can compose content exactly like a native one.

```rust
ViewNode::new(WidgetKind::Select)
    .prop("selected", PropValue::Int(1))
    .prop("name", PropValue::Text("level".into())) // form field: chosen VALUE → data["level"]
    .on("change", Intent::new("set_level"))         // optional: also fire an intent live on change
    .child(
        ViewNode::new(WidgetKind::Choice)
            .prop("value", PropValue::Text("low".into()))
            .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("circle".into())))
            .child(ViewNode::new(WidgetKind::Label).text("LOW")),
    )
    .child(
        ViewNode::new(WidgetKind::Choice)
            .prop("value", PropValue::Text("high".into()))
            .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("lightning".into())))
            .child(ViewNode::new(WidgetKind::Label).text("HIGH")),
    );
// The `change` intent fires with args {"value": "high"} — the option's value, not an opaque index.
// With `name` set, submitting the enclosing modal also returns data["level"] = "high" (the value).
// `name` is realize-only (a form-submission concept); the native `Select` builder has no equivalent.
```

Props `realize` reads: `selected` (`Int`); `name` (`Text`) opts the Select into **form submission** —
inside a modal body its chosen option's value is returned in `ModalResult::Action.data[name]`
([`Dialog`](#dialog) → *Declaring a modal from data*). Children: `Choice` nodes (a non-`Choice` child
is ignored). A childless `Choice` with a `text` prop desugars to a `Label` child, exactly as `Button`
does; when it has children, **children win**. See
[Options are children](#options-are-children-select--tabs--choice).

#### How the rows can be children *and* live in an overlay

Worth understanding before you touch this widget (it is the one genuinely subtle thing in the
library). The panel is drawn **outside** the control's layout box and may flip above it — the layout
engine cannot put a taffy node there, because a node sits where its parent's flow puts it.

So the rows are laid out in the trigger's flow, which is where the engine **measures** them (each
`Choice` hugs its content), and `Select` then **places** them: it bakes the offset from that flow
into each visible row's bounds — the same trick [`ScrollRegion`](#scrollregion) uses to bake
`-scroll_offset` into its children (`component::shift_subtree`). The rows carry the control's
horizontal insets as **margins**, which is what widens the control around them while keeping their
content clear of the chevron and the scrollbar lane.

The invariant this buys, and the reason it is done this way:

> **bounds === what is drawn === what is clickable.**

Hit-testing goes through [`choice_at`](#choice), which reads the rows' real bounds — never row
arithmetic — so a click can only ever land on the row the user sees there, whatever the rows contain
and however tall they are. Rows outside the visible window (and every row while the list is closed)
are **collapsed to zero size**, so no stale rect is left behind to swallow a click. Placement is
re-derived after every layout pass (`Component::on_layout`) and is idempotent.

> **Host wiring**: while `overlay_active()`, route pointer + wheel to
> `FocusManager::deliver_to_overlay`, and key input through `Keymap::dispatch` (raw key first, then
> the resolved `WidgetIntent`s). See `route_overlay_key` in
> [`heca-renderer/examples/showcase.rs`](../heca-renderer/examples/showcase.rs); in the app this is
> `build_widget_keymap` → `AppState.widget_keymap` → the overlay branch of `heca/src/app/events.rs`.

### Choice

The **option primitive**: a selectable container that carries a **value** and composes **arbitrary
content**. One `Choice` = one alternative the user can pick. [`Select`](#select) mounts them as its
dropdown rows and [`Tabs`](#tabs) as its segments, so the *look* of an option is written once and its
*content* is whatever the caller composes.

The **value** is what the option *means* (`"high"`), independent of what it *shows* (an icon and the
word `HIGH`). Containers report a pick by index; the host maps that index back to this value — which
is what lets a declarative author receive `{"value": "high"}` instead of an opaque `1`.

- **Construct**: `Choice::new(value)` (no content — compose it) · `Choice::labeled(value, label)`
  (sugar → one `Label` child, exactly what you'd compose by hand).
- **Content**: `.child(impl Component)` (`Parent`) — any component, any depth.
- **Builders**: `.selected(bool)`, `.on_activate(impl Fn())` (click / `Enter` / `Space`; also makes
  it focusable), plus `LayoutExt` (`.size(WidgetSize)`, `.disabled(bool)`, …).
- **Accessors**: `.value() -> &str`, `.state() -> Signal<bool>` (selected — flip it in place, no
  rebuild), `.hovered() -> Signal<bool>`.
- **Chrome only**: it paints the selected pill / hover tint / press flash / focus ring from the
  `Theme` (the same interaction tokens as `Item`), and **publishes its state color** so unstyled
  `Label`/`Icon` children tint with the selection — a child with its own color keeps it.
- **One Tab stop** ([`Base.focus_barrier`](#base)), whatever it contains. It **hugs its content**
  (no char-count arithmetic), and the size variant cascades into the content.
- **Picking from real bounds**: `widgets::choice_at(&children, point) -> Option<usize>` resolves a
  pick from the children's laid-out bounds. Containers must use it (or the same rule) rather than
  row arithmetic, so what is drawn and what is clickable can never disagree.

#### `Choice` vs [`Item`](#item) — when to use which

Both are selectable, both compose their content, both are a single Tab stop. They differ in shape
and purpose:

| | [`Item`](#item) | [`Choice`](#choice) |
|---|---|---|
| Shape | a **row**: leading · label · trailing, fixed row height | **any content**, hugging it |
| Carries | a label | a **value** — the thing chosen |
| Marker | `ActiveMarker` (bar / check) | a selected pill |
| Use for | sidebar / menu lists that look like rows | choosing among **alternatives** (`Select`, `Tabs`) |

**Rule of thumb:** building a list of rows → `Item`. The user is **picking one of several** → `Choice`.

```rust
// Sugar — a plain text option.
Choice::labeled("high", "HIGH");

// Composed — any tree. The Icon + Labels inherit the option's state color.
Choice::new("high")
    .child(Flex::column().gap(2.0)
        .child(Flex::row().gap(6.0)
            .child(Icon::new(Glyph::Lightning))
            .child(Label::new("HIGH")))
        .child(Label::new("uses more power").font_scale(0.75)))
    .selected(true)
    .on_activate(|| set_level("high"));
```

**Declarative:**

```rust
// As an option of a Select/Tabs (the usual case — see "Options are children").
ViewNode::new(WidgetKind::Choice)
    .prop("value", PropValue::Text("high".into()))
    .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("lightning".into())))
    .child(ViewNode::new(WidgetKind::Label).text("HIGH"));

// Standalone — then it is activatable in its own right, and hintable like any actionable node.
ViewNode::new(WidgetKind::Choice)
    .prop("value", PropValue::Text("high".into()))
    .text("HIGH")                                   // childless sugar → one Label child
    .on_press(Intent::new("set_level"));
```

Props `realize` reads: `value` (`Text`/`Int` — what the option *means*), `text` (the **childless**
sugar → a `Label` child; with children, **children win**, the same precedence as `Button`). With no
`value`, the text stands in for it. Events: `press` — wired **only** when the `Choice` is standalone;
inside a `Select`/`Tabs` the container owns the click, so a `press` there is ignored rather than
half-wired. See [Options are children](#options-are-children-select--tabs--choice).

### Item

Generic list/menu/sidebar **row**: an optional leading slot, a label, and an optional trailing
slot, with hover, active (selected), and click-to-activate states. Slots accept any component
(a `StatusDot`, a kbd-hint `Label`, a `>` chevron, …). Row height tracks the font; the
active/hover highlight is an inset pill (rounds with the theme radius, so it tucks inside a
rounded `Pane`). Becomes focusable/clickable once `.on_activate(...)` is set.

**Content is composed** (like [`Button`](#button)): the label is a real `Label` child that grows to
fill the middle, so the row draws only its chrome. Its **state color** (active → accent, muted →
muted, else foreground) is published via `PaintCx::with_content_color` and inherited by the label
and any unstyled slot icon; the **bold-when-active** weight rides the label's own `bold` signal.
The row is a single Tab stop ([`Base.focus_barrier`](#base)) — its slots are content, not
independent focus targets.

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

> **`Item` or [`Choice`](#choice)?** Both are selectable, both compose their content, both are one
> Tab stop — but they answer different questions. An `Item` is a **row** (leading · label · trailing,
> fixed height, `ActiveMarker`): use it when you are building a list that looks like rows. A `Choice`
> is **any content plus a value**: use it when the user is **picking one of several alternatives**.
> Full comparison + rule of thumb: [`Choice` vs `Item`](#choice-vs-item--when-to-use-which).

**Declarative** (`WidgetKind::Item`):

```rust
ViewNode::new(WidgetKind::Item)
    .text("main.rs")                                        // the label (the row's middle)
    .on_press(Intent::new("open_file"))
    .child(ViewNode::new(WidgetKind::StatusDot)
        .prop("slot", PropValue::Text("leading".into())))   // ← leading slot
    .child(ViewNode::new(WidgetKind::Badge)
        .text("M")
        .prop("slot", PropValue::Text("trailing".into()))); // ← trailing slot
```

- **Props**: `text` (the label). **Event**: `press`.
- **Slots** (see [Named child slots](#named-child-slots-the-slot-prop)): `leading`, `trailing` — each
  takes an arbitrary subtree. There is **no default slot**: the row's middle *is* the label, which
  comes from `text`, so a child with no `slot` (or an unknown one) is **ignored** rather than dropped
  somewhere it doesn't belong. It is debug-logged, never a panic.

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

> #### ScrollBar is HOST-ONLY — it is not declarable, on purpose
>
> Look at the example above: the widget is **driven by live signals** the host writes every frame.
> A [`ViewNode`](#declarative-ui-model-viewnode) is static, serializable data — it cannot carry a
> signal, let alone update one — so a declarative `ScrollBar` would render a **dead control**: a
> thumb that never moves and never reports. `realize` therefore refuses it (and says so in a debug
> log) rather than producing something that looks right and does nothing.
>
> **A plugin that needs scrolling uses [`Scroll`](#scrollregion)** (a `ScrollRegion`), which owns its
> own offset, wheel and keyboard handling — no host wiring required.
>
> This is a **general rule, not a special case**: *a widget whose state is a live host signal is
> host-only.* It applies to individual **builders** too — [`DockFrame::rail(..)`](#dockframe) binds a
> host-owned `RegionMode` signal, so a declarative dock is simply never rail-aware. Apply the same
> reasoning to any future widget of that shape, instead of inventing a way to fake a signal in data.

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

A compact **notification card**: a bracket-framed surface (the shared Pane/Dialog reticle frame)
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

**Native:**

```rust
Toast::danger("Connection lost")
    .body("Reconnecting to the grid…")
    .action("Retry", || retry())
    .on_dismiss(|| dismiss(id));
```

**Declarative** (`WidgetKind::Toast`):

```rust
ViewNode::new(WidgetKind::Toast)
    .text("Build failed")                                         // the title
    .prop("severity", PropValue::Text("danger".into()))           // info | success | warning | danger
    .prop("icon", PropValue::Glyph("warning".into()))             // optional; severity picks a default
    .prop("body", PropValue::Text("3 errors in heca-grid-ui".into()))
    .prop("action_text", PropValue::Text("RETRY".into()))         // the inline action's label
    .prop("dismissible", PropValue::Bool(true))
    .on("action", Intent::new("rebuild"))                         // fires when RETRY is clicked
    .on("dismiss", Intent::new("close_toast"))                    // fires on the ×
    .on_press(Intent::new("open_build_log"));                     // whole card
```

- **Props**: `text` (title), `severity` (`Text` — an unknown name degrades to `info`), `icon`
  (Glyph name), `body`, `action_text`, `dismissible` (Bool).
- **Events**: `press` (the whole card), `dismiss` (the ×), `action` (the inline button — only wired
  when `action_text` is set).
- **No slots.** The inline action is a *labelled button*, not arbitrary content, so it is a prop plus
  an intent. A slot would have promised a composition the widget does not offer.

> **A declarative `Toast` is for INLINE use** — a notification row inside a panel. It is **not** how
> you fire an app notification: the host owns the queue and lifecycle through
> [`ToastStack`](#toaststack) + `ToastSpec` (which is plain data it can push, time out and dedup).
> Realizing a `Toast` node just renders a card wherever you put it.

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
theme-driven, never baked in. Square, font-sized.

**Color is inherited when unset** (like [`Label`](#label)): with no explicit `.color(..)` the primary
layer takes the [content color](#scene--drawcommand--paintcx-for-building-widgets) published by an
enclosing control, falling back to `theme.foreground`; the secondary layer follows the primary at
`theme.icon_secondary_alpha`. So an icon composed inside a `Button` tints and fades with it.

- **Construct**: `Icon::new(Glyph)`.
- **Builders**: `.size(px)` (glyph pixels — **not** the `WidgetSize` variant; use
  `Style::set_size` for that, since `size` is taken), `.color(Color)` (primary — opts out of
  inheritance), `.secondary_color(Color)`, `.glow(bool)` (default `false` — see below).

**A glyph can halo** (`.glow(true)`). Until now only bordered *surfaces* carried the resting glow
(`interaction.control_rest_glow`); a bare glyph was always flat, because glow lived only in the SDF
**rect** renderer. `TextCmd` now carries an optional glow the way `RectCmd` does, and the renderer
realizes it by drawing a pre-blurred copy of the glyph behind itself.

  - It goes through the **same `scaled_glow` chokepoint** as every surface, so `glow_size = none`
    removes it along with every other halo, and the other levels scale it. It is not a second glow
    system.
  - It is **opt-in, not automatic**: most icons sit inside a control that is already glowing (a
    Button's leading icon, a pane-header action), and haloing those too would double the light.
    Turn it on for a glyph that stands alone on the background.
  - Only the **primary** layer haloes — haloing the secondary wash too would double the light on
    every duotone glyph and cost a second set of taps for nothing.
  - A glyph inside a control that publishes a **content glow** inherits one without the flag. That
    is how [`RailCell`](#railcell) lights its resting bare icon: a control holds its children as
    `impl Component` and cannot style them, so it publishes and the glyph pulls — exactly the
    mechanism [content color](#scene--drawcommand--paintcx-for-building-widgets) already uses.
  - **Never on terminal text.** Cell glyphs are the hottest path in the app; the halo multiplies a
    run's vertex count, so the terminal path hard-codes no glow.
- **Declarative**: `WidgetKind::Icon`, prop `icon` (a Glyph **name**, e.g. `"trash"`, `"git_branch"`
  — resolved by `glyph_from_name` in `realize.rs`; an unknown name renders no icon, never panics).

```rust
ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("git_branch".into()))
```
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
// A standalone glyph that should read as lit:
Icon::new(Glyph::Lightning).color(theme.accent).size(34.0).glow(true);
// Render the whole set (what the showcase does):
for &g in Glyph::ALL { /* Icon::new(g) … */ }
```

> **Declarative note:** `glow` is not a `ViewNode` prop. It is a *rendering* decision the host
> makes about a glyph standing alone versus one inside a lit control — a plugin describes what the
> icon **is**, and the host decides how it is lit, the same split that keeps colors out of props.

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

**Native:**

```rust
ItemGroup::new("src")
    .child(Item::new("main.rs"))
    .child(Item::new("lib.rs"));
```

**Declarative:**

```rust
ViewNode::new(WidgetKind::ItemGroup)
    .text("src")                                    // the header label
    .prop("expanded", PropValue::Bool(true))
    .on("toggle", Intent::new("fold_group"))        // fires with args {"expanded": false}
    .child(ViewNode::new(WidgetKind::Item).text("main.rs"))
    .child(ViewNode::new(WidgetKind::Item).text("lib.rs"));
```

Props `realize` reads: `text` (header), `expanded` (Bool, default `true`). Children: the rows (the
group's own header is prepended by the widget). Event: **`toggle`** — the intent carries the state it
moved to in `args["expanded"]`, so one binding tells you which way it went.

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

**Native:**

```rust
MarkerGroup::new()
    .active(holds_focus)
    .child(Row::new().child(Label::new("pane 1")))
    .child(Row::new().child(Label::new("pane 2")));
```

**Declarative:**

```rust
ViewNode::new(WidgetKind::MarkerGroup)
    .prop("active", PropValue::Bool(true))
    .prop("nav_selected", PropValue::Bool(false))
    .child(ViewNode::new(WidgetKind::Item).text("pane 1"))
    .child(ViewNode::new(WidgetKind::Item).text("pane 2"));
```

Props `realize` reads: `active` (Bool), `nav_selected` (Bool). Children: the rows. **No events** — the
marker bar is an indicator; the rows inside carry their own intents.

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

**Native:**

```rust
let sidebar = ChromeRegion::vertical();
let mode = sidebar.mode_signal();
let files = DockFrame::new("EXPLORER")
    .rail(mode, Glyph::FolderOpen)
    .header(Badge::accent("3"))
    .child(ItemGroup::new("src").child(Item::new("main.rs")));
```

**Declarative** (`WidgetKind::DockFrame`):

```rust
ViewNode::new(WidgetKind::DockFrame)
    .text("EXPLORER")                                          // the title
    .prop("expanded", PropValue::Bool(true))
    .prop("frameless", PropValue::Bool(false))
    .prop("active", PropValue::Bool(false))                    // the accent wash
    .prop("nav_selected", PropValue::Bool(false))              // the nav-cursor outline
    .on("toggle", Intent::new("fold_dock"))                    // fires with args {"expanded": …}
    .child(ViewNode::new(WidgetKind::Badge)
        .text("3")
        .prop("slot", PropValue::Text("header".into())))       // ← the controls slot
    .child(ViewNode::new(WidgetKind::Item).text("main.rs"));   // ← no slot ⇒ body (the default)
```

- **Props**: `text` (title), `expanded` (Bool, default `true`), `frameless`, `active`,
  `nav_selected` (Bool).
- **Event**: `toggle` — carries the new state in `args["expanded"]`.
- **Slots** (see [Named child slots](#named-child-slots-the-slot-prop)): `header` (the controls slot).
  The **body is the default slot**, so an unslotted child is body content; an unknown slot name falls
  back to the body and is debug-logged.
- **`.rail(..)` is host-only** — it binds a host-owned `Signal<RegionMode>`, and static serializable
  data cannot drive a live signal (the same rule that makes [`ScrollBar`](#scrollbar) host-only). A
  declarative dock is never rail-aware; the host wires that.

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

**At rest it publishes a content glow so its bare glyph still haloes.** The cell draws no surface
at rest — the bare icon *is* the resting look — so there is nothing to carry the halo every
bordered surface gets. It cannot style its child either (children are `impl Component`), so it
publishes a glow via `PaintCx::with_content_glow` and the [`Icon`](#icon) pulls it, the same
publish/pull mechanism [content color](#scene--drawcommand--paintcx-for-building-widgets) uses.
Hover and active already have their own lit chrome, so they do not double it, and the glyph keeps
ownership of the halo's *reach* — only it knows how big it is.

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

// The child needs no `.glow(true)`: at rest the cell publishes one and the icon
// inherits it. Setting it explicitly would light the glyph in hover/active too,
// where the cell's own chrome is already lit.
RailCell::new(Icon::new(Glyph::Terminal).size(22.0)).on_activate(move || focus_pane(i));
```

**Declarative (`ViewNode`).** `WidgetKind::RailCell`, prop `icon` (a Glyph **name**) + `size`,
event `press`. The resting halo needs no prop — it comes from the cell.

```rust
ViewNode::new(WidgetKind::RailCell)
    .prop("icon", PropValue::Glyph("terminal".into()))
    .on_press(Intent::new("focus_pane").arg("pane_id", PropValue::Int(id)));
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
short delay. The bubble is drawn on the **overlay layer** so it sits above siblings. It captures
**no** input — the wrapped widget stays fully interactive (forwards events + focus).

**It hand-rolls neither its placement nor its surface** — both come from the shared authorities,
so a tooltip cannot drift away from the rest of the overlay family:

- **Placement** is [`place_beside`](#overlay), the four-sided authority: the bubble is **centered**
  on the chosen side of the target, **flips** to the opposite side when there's no room
  (`Top`↔`Bottom`, `Left`↔`Right`), and its **cross-axis** is clamped on-screen. Only the cross
  axis clamps — sliding the bubble along the main axis would move it *over* the thing it
  describes, so an unfittable bubble overflows instead.
- **Surface** is [`paint_panel_chrome`](#overlay), the same painter every overlay panel uses.
  So `[appearance] overlay_border_style` (`none | bordered | bracketed`) governs the tooltip's
  edge exactly as it governs a dialog or a dropdown. The tooltip supplies only its own identity:
  the accent edge colour (`interaction.tooltip_border`), a tighter/fainter halo than a panel's,
  and `PanelElevation::Hover` — the shared shadow at a quarter depth, because the full panel
  shadow is larger than a ~30px bubble.

- **Construct**: `Tooltip::new(child, text)`, or `Tooltip::new_signal(child, Signal<String>)` for
  a reactive label.
- **Builders**: `.side(TooltipSide)` (`Top` | `Bottom` | `Left` | `Right`, default `Top`),
  `.delay(seconds)` (hover delay before reveal, default `0.5`).
- **`TooltipSide` is `BesideSide`** — the same type, re-exported under the name that reads better
  at a call site. There is one four-sided vocabulary, not two.

```rust
Tooltip::new(
    IconButton::new(Icon::new(Glyph::Close).color(theme.danger).size(20.0)).on_click(|| close()),
    "Close",
).side(TooltipSide::Bottom);
```

**Declarative (`ViewNode`).** Host-only — there is no `WidgetKind::Tooltip`. A tooltip wraps a
widget in the *retained* tree and is revealed by hover state the host owns; in the heca app an
action button's tip is derived from its `WmAction` centrally (next section), never authored per
call site.

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

### Overlay

The **base overlay surface** every overlay widget shares (T009 overlay rework): a
viewport-filling, centering layer that decorates its single **panel** child with the common
overlay chrome — optional dimming scrim, drop shadow, theme surface fill, and the bracket
reticle. **Blocking is a property of this layer, not a per-widget reimplementation**: a
*blocking* overlay (default) paints the scrim and swallows outside input (modal); a
non-blocking one lets outside input fall through (light-dismiss). **Positioning lives here once**:
`Center` taffy-centers the panel on the viewport (so every descendant gets true bounds), and
`Anchored` hangs it off a trigger rect for dropdowns/popovers. The placement math and the panel
chrome are also exposed as **free functions** ([`place_anchored_on`](#overlay),
[`place_at_point`](#overlay), [`paint_panel_chrome`](#overlay)) so a widget that cannot hand its
content to an `Overlay` — like [`Select`](#select), whose option rows are *placed children* — still
shares the one implementation instead of hand-rolling a second one.

**Composition, not inheritance.** [`Dialog`](#dialog) *composes* an `Overlay` as its subtree:
the `Overlay` owns presentation + geometry ([`overlay_occludes`](#component-trait)); the
specialization owns content and behaviour (focus trap, keyboard, dismissal policy) and
intercepts events before the `Overlay`'s standalone handling runs. Used **directly** (a host
mounting an arbitrary — e.g. `realize`d — panel), the `Overlay` provides the standard layer
semantics itself: nested-overlay-first routing, outside-click callback, blocking swallow.

- **Construct**: `Overlay::new()` (closed, blocking), then `.panel(impl Component)` — the single
  child; the caller owns the panel's internal layout (padding/gaps/children), the overlay owns
  the chrome around it. `.panel_boxed(Box<dyn Component>)` takes a mapper-produced panel (e.g.
  `heca`'s `realize(ViewNode)`).
- **Builders**: `.blocking(bool)` (default `true` — scrim + swallow outside input; `false` = no
  scrim, outside input falls through), `.open(bool)`,
  `.on_outside_click(impl Fn())` (standalone dismissal hook; a composing widget applies its own
  policy instead).
- **Positioning**: `.position(OverlayPosition)` picks how the panel is placed —
  `OverlayPosition::Center` (default: fill the viewport, taffy-center the panel — the modal
  [`Dialog`](#dialog) case) or `OverlayPosition::Anchored { anchor, gap }` (dropdown/popover:
  below the trigger rect, flipped above when there's no room, left-edge aligned, clamped into the
  viewport). `.anchored(rect)` is sugar for the anchored mode with the default gap
  (`DEFAULT_ANCHOR_GAP`); `.set_anchor(rect)` re-anchors in place (a host following a moved
  trigger). Anchored placement is baked into the panel child's bounds on layout (the subtree-shift
  trick — so paint and hit-testing follow), and it is idempotent. Pair an anchored overlay with
  `.blocking(false)` for a light-dismiss popover. The placement math is the free function
  `place_anchored(anchor, panel, viewport, gap) -> Rectangle` — the single authority for
  rect-anchored (dropdown) flip/clamp placement. Its sibling
  `place_at_point(anchor, panel, viewport, inset, centered) -> Rectangle` is the authority for
  **point-anchored** placement (down-right of a cursor, flip up-left, or centered on the point).
  `place_anchored_on(..., side: AnchorSide)` is the full form of the former: `AnchorSide::Auto`
  (default — flip by available room), or `Below`/`Above` to **force** a side the caller already
  chose (still clamped into the viewport, never flipped away). Both
  [`Select`](#select) (rect-anchored, forced side) and [`ContextMenu`](#contextmenu)
  (point-anchored) delegate their placement here, so the flip/clamp rule exists once.
- **Sizing**: `.panel_size(width: Length, height: Length)` gives the panel an explicit size instead
  of letting it hug its content. Default = unset (hug). `Length::Auto` on an axis keeps the hug
  behaviour there; a `Length::Pct` resolves against the **viewport**, since the `Overlay` fills it
  (`Pct(0.6)` = 60% of the viewport). Call order does not matter — the size is stored and re-applied
  whenever `.panel()`/`.panel_boxed()` replaces the child.
  **Why it matters:** a [`ScrollRegion`](#scrollregion) only scrolls when its parent *bounds* it. An
  unsized panel grows with its content, so a long body never overflows and no scrollbar appears.
  Size the panel and the body can scroll inside it.
- **Accessors**: `.open_signal() -> Signal<bool>`; `.panel_bounds() -> Rectangle` (valid after
  layout).
- **Contract**: `focusable`/`overlay_active` only while open (host overlay scan);
  `overlay_occludes` = whole viewport when blocking, else the panel rect.
- **Shared panel chrome**: `paint_panel_chrome(cx, rect, PanelChrome { border, glow, elevation })` is the single
  authority for what an overlay panel *looks like* — drop shadow (lifting it off the page), the theme
  surface fill, the per-widget accents, and the panel's **edge**. The base `Overlay` passes
  `PanelChrome::default()`; [`Select`](#select), [`ContextMenu`](#contextmenu), and
  [`CommandPalette`](#commandpalette) call the same painter with their own accent border + glow,
  because each owns content that cannot be handed to an `Overlay` as a single panel child (`Select`'s
  option rows are *placed children*; the menu/palette draw their rows from data). Call it inside a
  `with_overlay` block — it does not open the overlay layer itself, and a blocking layer's **scrim**
  is separate from the panel chrome (the palette paints its own scrim first).
- **The panel edge is a user setting, not a widget decision** — `Theme.colors.overlay_frame`
  (`FrameStyle`), driven by the app's `[appearance] overlay_border_style` (`none | bordered |
  bracketed`, live-reloading like the rest). It owns the **whole** edge, so the three styles are
  genuinely distinct:

  | `overlay_frame` | Edge | Corner reticle |
  |---|---|---|
  | `bracketed` (default) | yes | yes |
  | `bordered` | yes | no |
  | `none` | **no** | no |

  `PanelChrome.border` is the widget's preferred edge **colour** — honoured when the style draws an
  edge, ignored under `none` (otherwise `none` could not remove a `Select`'s accent border). When a
  widget supplies no border, the theme's neutral `border` fills in, so `bordered` still shows an edge
  on the base `Overlay`. The **fill and glow are never suppressed**: the halo is the panel's neon
  identity, not a frame, so it survives `none`. The showcase drives the same token live via its
  **OVERLAY FRAME** select (it builds its own `Theme` and never reads `config.toml`).
- **Depth is a semantic variant, not a number** — `PanelChrome.elevation` (`PanelElevation`). The
  shadow's *shape* is defined once and scaled, so surfaces at different depths still read as the
  same material. A caller picks the depth; it never supplies a blur radius or an offset.

  | `PanelElevation` | Used by | Shadow |
  |---|---|---|
  | `Panel` (default) | `Dialog`/`Overlay`, [`Select`](#select), [`ContextMenu`](#contextmenu), [`CommandPalette`](#commandpalette) | full depth |
  | `Hover` | [`Tooltip`](#tooltip) | 25% of it (blur *and* drop scale together) |

  Why it exists: the panel shadow is tuned for surfaces hundreds of pixels across. Applied unscaled
  to a ~30px tooltip bubble it is **larger than the surface casting it** — verified in the showcase
  and rejected. `Hover` keeps the shared chrome at a depth that suits a transient bubble.
  **The shadow is not part of the frame policy** — it is depth, not an edge, so `overlay_frame:
  none` removes the border but never the shadow.
- **Painting**: everything goes through `with_overlay`, so an overlay opened *inside* the panel
  (a [`Select`](#select) dropdown in a modal body) records a **deeper scene segment** and
  composites above everything this layer draws — see the
  [`Scene`/`PaintCx` table](#scene--drawcommand--paintcx-for-building-widgets).
- **Traits**: `LayoutExt`.

**Native.**
```rust
// A host-mounted blocking layer around an arbitrary (here: realized) panel.
let overlay = Overlay::new()
    .panel_boxed(realized_panel)                  // Box<dyn Component> from realize(ViewNode)
    .on_outside_click(move || emit(close_intent)) // host's overlay-close path
    .open(true);
let visible = overlay.open_signal();
```

**Declarative (`ViewNode`).** Host-only — there is no `WidgetKind::Overlay`. A plugin never
mounts a layer itself; it submits a spec (e.g. `ModalSpec` with a `ViewNode` body) and the
**host** builds the layer (`Dialog`/`Overlay`) around the realized content — overlay hosting,
z-order, and blocking policy stay host-owned (§2.7 of the plugin plan).

#### Implementing an overlay-panel widget

There are **two ways** to give a widget an overlay panel. Pick by one question: *can the panel's
content be a single child component?*

**A. Compose an `Overlay`** — the default. Your content is one subtree, so hand it over and let the
layer own placement, chrome, scrim, and dismissal. This is what [`Dialog`](#dialog) does.

```rust
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Overlay, OverlayPosition, DEFAULT_ANCHOR_GAP};

// A light-dismiss popover anchored under its trigger.
let popover = Overlay::new()
    .blocking(false)                       // no scrim; outside input falls through
    .anchored(trigger.base().bounds)       // below the trigger, flip above, clamp on-screen
    .panel(Card::new().padding(10.0).child(Label::new("Popover body")))
    .on_outside_click(move || close())     // light-dismiss
    .open(true);

// Re-anchor in place when the trigger moves (scroll, resize) — no rebuild:
// popover.set_anchor(trigger.base().bounds);
```

**B. Paint the panel yourself, but reuse the authorities** — when the content *cannot* be one
child. [`Select`](#select) is the case: its option rows are **placed children of the `Select`**
(laid out in the trigger's flow, then moved into the panel by baking offsets into their bounds), so
they cannot be handed to an `Overlay` without breaking the `bounds === drawn === clickable`
invariant. Such a widget still must not hand-roll placement or chrome:

> `MySelect` below is a **cut-down sketch of the real [`Select`](#select)** — a trigger with a
> dropdown of rows — reduced to the parts that matter here. The shipped version is
> `heca-grid-ui/src/widgets/select.rs`; read it alongside this.

```rust
use heca_grid_ui::widgets::{paint_panel_chrome, place_anchored_on, AnchorSide, PanelChrome};

impl MySelect {
    /// The panel rect. MUST be a pure function of bounds + side + content size —
    /// see the invariant below.
    fn panel_rect(&self) -> Rectangle {
        let b = self.base.bounds;
        let side = if self.open_up { AnchorSide::Above } else { AnchorSide::Below };
        place_anchored_on(
            b,                                            // anchor = the trigger rect
            Size::new(b.size.w, self.panel_h()),          // panel size you computed
            Size::new(f64::INFINITY, f64::INFINITY),      // see the invariant below
            DEFAULT_ANCHOR_GAP,
            side,
        )
    }
}

impl Component for MySelect {
    fn paint(&self, cx: &mut PaintCx) {
        // …trigger chrome here…
        if self.open {
            cx.with_overlay(|cx| {                        // the overlay LAYER is yours to open
                let panel = self.panel_rect();
                // Your identity only — whether an edge is drawn at all is the
                // user's `overlay_frame` setting, applied by the painter.
                paint_panel_chrome(cx, panel, PanelChrome {
                    border: Some(cx.theme().colors.accent.into()), // preferred edge COLOUR
                    glow: None,
                });
                for child in self.visible_rows() { child.paint(cx); }
            });
        }
    }
}
```

> ##### ⚠️ The invariant: a rect used to *place* children must be **pure**
>
> If the same rect both positions child components (during layout / an `on_layout` placement
> pass) **and** is drawn during paint, it must be a pure function of already-settled inputs —
> bounds, a decided side, measured child sizes. Layout and paint run at **different moments**, so
> anything time-varying (most temptingly: clamping against a viewport that `paint` caches) makes
> the two calls disagree, and the rows visibly **detach from their panel**.
>
> This is a real regression that shipped and was reverted: clamping `Select`'s panel to the
> viewport put the rows outside the panel when the list flipped above (a negative `y` snapped to
> `0`), and pushed row content onto the border for a trigger near the right edge. Keep such a
> panel on-screen by **capping how much content you show** (`Select` caps the visible row count
> when it opens), not by moving the panel after the fact. Pass an infinite viewport to
> `place_anchored_on` to opt out of clamping, and add a test that mutates the cached viewport and
> asserts the rect does not move (`open_panel_rect_is_independent_of_the_cached_viewport`).
>
> An `Overlay` you *compose* (path A) is not exposed to this: it places its panel child in
> `on_layout` and paints from the child's real bounds, so there is only one source of truth.

**Which authority to call**

| You have | Call | Used by |
|---|---|---|
| A trigger **rect** (dropdown/popover) | `place_anchored_on(anchor, panel, vp, gap, side)` — or `place_anchored(..)` for `AnchorSide::Auto` | [`Select`](#select), `Overlay`'s `Anchored` mode |
| A cursor **point** (context menu) | `place_at_point(anchor, panel, vp, inset, centered)` | [`ContextMenu`](#contextmenu) |
| A target rect, **centered on any of 4 sides** (hover bubble) | `place_beside(anchor, panel, vp, gap, side)` with `BesideSide::{Top,Bottom,Left,Right}` | [`Tooltip`](#tooltip) |
| A panel to **decorate** | `paint_panel_chrome(cx, rect, PanelChrome { border, glow })` | `Overlay`, [`Select`](#select), [`ContextMenu`](#contextmenu), [`CommandPalette`](#commandpalette), [`Tooltip`](#tooltip) |

The three placement authorities differ in **alignment**, which is why they are three functions and
not one with flags:

| Authority | Anchor | Cross-axis alignment | Flips between |
|---|---|---|---|
| `place_anchored_on` | a rect | leading-edge aligned | below ↔ above |
| `place_at_point` | a point | corner-offset from the point | down-right ↔ up-left |
| `place_beside` | a rect | **centered** | all four sides |

> ⚠️ **`place_beside` results may depend on the viewport** (both the flip and the clamp read it),
> which the [purity invariant](#-the-invariant-a-rect-used-to-place-children-must-be-pure) forbids
> for a rect that positions children. It is safe for `Tooltip` only because that bubble is
> **drawn**, never laid out into — its text is a `cx.text` call, not a child component. If you use
> `place_beside` to place real children, you inherit the detach bug. Draw-only, or don't use it.

Use `AnchorSide::Auto` unless you already decided the side. Force `Below`/`Above` when the
decision and the panel's **size** are computed together (`Select` picks the side and its visible
row count in one pass, because the height depends on the side) — otherwise the placement could
flip to a side the size was not computed for.

### Dialog

A centered overlay **panel that holds real child components**, composing the base
[`Overlay`](#overlay) for its layer presentation (blocking scrim + chrome + centering). It lays
out a padded panel of `[title, body, action-row]` where the `body` is an arbitrary component and
each action is a real [`Button`](#button). Because the buttons are real children (not manually
painted), they get the
universal hint picker (`prefix+/`), standard focus traversal, and pointer routing **for free** —
this is what makes an overlay's buttons hintable.

Same overlay contract as [`Select`](#select): `overlay_active` + `focusable` only while open, so the host
routes input here first. Keyboard is an embedded [`FocusManager`](#) over the panel subtree.
`Dialog` carries no result closures: a button's own `on_click` is the action, and dismissal
is a callback the host points at its overlay-close path (e.g. emit `CloseOverlay`).

**Nav keys are host-configured, except the universal focus primitive (`widget-keys-config`).**
**Tab / Shift+Tab always move focus** within the modal (classic, always-on, via the embedded
`FocusManager` — not a rebindable binding). Beyond that the dialog's button row is a **horizontal**
focus strip, so configurable traversal / submit / cancel arrive as the semantic
`Event::Widget(WidgetIntent::{ItemPrevious, ItemNext, Activate, Dismiss})`. The **host** resolves
these from `[keys.widgets]` and only sends them after a raw key was **not** consumed by a focused
field (field-first):

| Intent | Default binding (config name) | Effect |
|--------|-------------------------------|--------|
| *(always on)* | `Tab` / `Shift+Tab` | focus next / previous (classic, not configurable) |
| `ItemNext` | `→`, `Ctrl+l` (`item_next`) | focus the next field/button |
| `ItemPrevious` | `←`, `Ctrl+h` (`item_previous`) | focus the previous field/button |
| `Activate` | `Enter` (`activate`) | activate the **primary** (first) action |
| `Dismiss` | `Esc` (`dismiss`) | dismiss (also fired by a **scrim** click) |

**Form bodies (text input) — field-first.** A raw `Event::Key` is handed to the **focused descendant
first**, so a [`Input`](#input) body (or a `realize`d `ViewNode` form) receives typed characters,
caret motion, Backspace/Delete. The `Edit*` intents (and any vertical `Menu*`) are also forwarded
field-first; only an unconsumed `Item*` moves focus — so an arrow moves the caret **inside** the
input but navigates when a **button** is focused, and `Enter`→`Activate` fires OK even while typing.
App wiring: `build_widget_keymap` → `AppState.widget_keymap`, dispatched in the overlay key branch
(`heca/src/app/events.rs`). This is what makes the host-owned rename / prompt dialogs (an `Input` +
OK/Cancel) work.

Centering is real taffy layout, owned by the composed [`Overlay`](#overlay) (it fills the
viewport and centers the panel), so every descendant gets true bounds (which the hint picker +
hit-testing need).

**Nested overlays work** (T009 step 4): a [`Select`](#select) opened inside the body composites
**above** the dialog's action buttons (its dropdown records a deeper scene segment) and captures
hover/wheel/keys over them — the dialog offers pointer moves, `Scroll`, and the semantic
`Widget*` intents to an overlay-active descendant first, so `Dismiss` closes the *dropdown* (not
the dialog) and `Activate` commits its row.

> **Host it as a top-level overlay layer — never in-flow inside scrolled content.** The taffy
> centering centers the panel **within the Dialog's own box**, so the box must BE the viewport:
> in `heca` the Dialog is a layer root (`LayerRegistry` / the `Modal` band); the showcase mounts
> it in its overlay tree above the page's root [`ScrollRegion`](#scrollregion). Mounted as an
> in-flow `.child(...)` of a scrolled column instead, its box is that slot and the panel centers
> off-screen once the page scrolls (and a self-recentering `on_layout` cannot fix it — inside a
> scrolled subtree bounds are in scrolled-tree coordinates, not screen coordinates; this was
> tried and reverted, see F003/P011/T009 BUG B).

- **Construct**: `Dialog::new(title)`, then `.body(impl Component)` and `.action(impl Component)`
  (a wired `Button`), in that order. Buttons sit in a right-aligned row in call order.
- **Builders**: `.dismissible(bool)` (default `true`; `false` = forced-decision — `Dismiss`/scrim
  swallowed without dismissing), `.on_dismiss(impl Fn())` (fired on `WidgetIntent::Dismiss` / scrim),
  `.open(bool)`
  (focuses the first focusable — a text field body if present, so the user types immediately;
  otherwise the first button as a safe default), plus `.body_boxed(Box<dyn Component>)` for a body
  from a mapper (e.g. `realize`).
- **Sizing + a scrollable body**: `.panel_size(width: Length, height: Length)` bounds the panel
  instead of letting it hug its content (default = hug; `Length::Auto` keeps hugging on that axis;
  `Length::Pct` resolves against the **viewport**). This is what makes a long body scrollable: a
  [`ScrollRegion`](#scrollregion) only scrolls when its parent bounds it, so wrap the body in one and
  size the panel. Put **only the body** in the region — the title and the action row stay fixed:

  ```rust
  Dialog::new("Pick a container")
      .panel_size(Length::Pct(0.5), Length::Pct(0.6))   // 50% × 60% of the viewport
      .body(ScrollRegion::new().child(long_list))       // only this scrolls
      .action(Button::secondary("Cancel"))
  ```
  A [`Select`](#select) inside a scrolled body still composites **above** the action buttons (the
  nested-overlay routing from the T009 rework), so overlay-in-scrolled-overlay is supported.
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
its current value under that name (`Input` → `Text`, `Toggle`/`Checkbox` → `Bool`, `Select` → the
**chosen option's value** as `Text`, not its index). Unnamed value nodes render but aren't
collected. Collection is in the body's declaration order.

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
                    .prop("name", PropValue::Text("name".into())), // → data["name"] (Text)
            )
            // A named Select is a form field too: its chosen option's *value* comes back.
            .child(
                ViewNode::new(WidgetKind::Select)
                    .prop("name", PropValue::Text("scope".into())) // → data["scope"] (Text)
                    .prop("selected", PropValue::Int(0))
                    .child(
                        ViewNode::new(WidgetKind::Choice)
                            .prop("value", PropValue::Text("pane".into()))
                            .text("This pane"),
                    )
                    .child(
                        ViewNode::new(WidgetKind::Choice)
                            .prop("value", PropValue::Text("column".into()))
                            .text("Whole column"),
                    ),
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
                let name = data.get("name").and_then(PropValue::as_text);
                let scope = data.get("scope").and_then(PropValue::as_text); // "pane" | "column"
                /* dispatch the rename with `name` + `scope` */
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

A fuzzy **command launcher** overlay (same input-capturing contract as `Dialog`): a query line
over a scrollable list of commands. The query line is a real [`Input`](#input), so full editing
comes for free — selection, multi-click, and char/word/line delete (Ctrl/Alt/⌘ + Backspace/Delete).
Typing filters with a **fuzzy subsequence** match, **smart-case** (case-insensitive unless the
query has an uppercase letter), ranked, with matched characters highlighted in the accent.
Selecting a command fires its callback and closes.

- **Construct**: `CommandPalette::new()`; add commands with `.command(Command::new(label, on_run)
  .icon(Glyph)?.key("⌘K")?)`; `.placeholder(text)`, `.open(bool)`.
- **Accessor**: `.open_signal() -> Signal<bool>` — bind a chord (e.g. Ctrl+K) to open it.
- **Occlusion**: while open, `overlay_occludes(pos)` is `true` for **every** point (it grabs the
  viewport: typing, nav, outside-click dismiss), so a host must not synthesize page-level actions
  (e.g. right-click → context menu) anywhere under it (see [`Component` trait](#component-trait)).
- **Nav (host-driven, configurable)**: the palette carries **no hardcoded nav keys**. As a vertical
  list it responds to the semantic `Event::Widget(WidgetIntent::{MenuUp,MenuDown,Activate,Dismiss})`;
  the **host** resolves the configurable `menu_up` / `menu_down` / `activate` / `dismiss`
  `[keys.widgets]` bindings into these (defaults: ↑/Ctrl+k, ↓/Ctrl+j, Enter, Esc). Raw `Event::Key`
  goes to the query field (typing / editing). App wiring: `build_widget_keymap` → `Keymap::dispatch`.

```rust
let palette = CommandPalette::new()
    .command(Command::new("Split pane", || wm.split()).icon(Glyph::Sidebar).key("⌥⌘S"))
    .command(Command::new("Close pane", || wm.close()).key("⌘W"));
let open = palette.open_signal();
// host: on Ctrl+K → open.set(true); add `palette` to the tree
```

> Needs the same host wiring as `Dialog` (route keys to the overlay). Because it tracks `Ctrl` for
> Ctrl+J/K, the host must also broadcast `Event::ModifiersChanged` to the tree (most hosts do).

### ContextMenu

A **cursor-anchored action menu** overlay — the pointer counterpart to the keyboard pick flows
(same input-capturing contract as `CommandPalette`). A floating list of entries, each with
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
- **Occlusion**: deliberately keeps the default `overlay_occludes` = `false` even while open — a
  second right-click **re-anchors** the menu at the new point (the standard menu affordance), so its
  panel must not block the host's right-click gate (see [`Component` trait](#component-trait)).

**Host right-click gate (native).** Right-click detection is app-level, and the host must not open
the menu on a point an overlay above the page owns — an open `Dialog`/`CommandPalette`, a toast
card, an open `Select` panel. Gate it with `overlay_occluded_at` (scans a tree for
`Component::overlay_occludes` hits); page content (buttons, inputs, panes) never occludes, so
right-click there opens the menu as usual:

```rust
// On the host's right-click event (winit/etc.):
let occluded = heca_grid_ui::overlay_occluded_at(&overlay_layer_tree, cursor)
    || heca_grid_ui::overlay_occluded_at(&page_tree, cursor); // open Select panels live here
if !occluded {
    menu_anchor.set(cursor); // ContextMenu re-anchors even while already open
    menu_open.set(true);
}
```

(Host-side only — occlusion is a `Component` method, not a `ViewNode` prop; a declarative tree gets
this behavior from the host that realizes and mounts it.)
- **Nav (host-driven, configurable)**: **no hardcoded nav keys** — as a vertical list the menu
  responds to `Event::Widget(WidgetIntent::{MenuUp,MenuDown,Activate,Dismiss})`, which the host
  resolves from the configurable `menu_up` / `menu_down` / `activate` / `dismiss` `[keys.widgets]`
  bindings (defaults ↑/Ctrl+k, ↓/Ctrl+j, Enter, Esc). Raw `Event::Key` is only a **quick-pick
  letter** that runs its entry directly.
  Hover highlights; click runs; outside-click dismisses.

```rust
let menu = ContextMenu::new()
    .entry(MenuEntry::new("Rename", || wm.rename()).icon(Glyph::FileCode).key('r'))
    .entry(MenuEntry::new("Close", || wm.close()).icon(Glyph::XSquare).key('x').danger(true));
let (open, anchor) = (menu.open_signal(), menu.anchor_signal());
// host: on right-click → anchor.set(cursor); open.set(true); add `menu` to the tree
```

> Same host wiring as `CommandPalette` (route keys to the overlay). The app decides *when*
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
Its `overlay_occludes(pos)` reports the **cards'** rects (not the whole corner), so a host gate
like "right-click opens the page menu" skips points a toast covers while staying live elsewhere
(see [`Component` trait](#component-trait)).

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

> Same host wiring as `Dialog` (route pointer to the overlay first). Auto-dismiss/timers live in the
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

> **A `Button`'s children are its content, and they win.** A Button node *with* children is realized
> as an empty button holding them (an arbitrary tree, any depth); a **childless** node falls back to
> its scalar sugar — `text` → a bold `Label`, `icon` → a leading `Icon` — which builds the very same
> children. See [Button → the two ways to build one](#the-two-ways-to-build-a-button--same-widget-same-retained-tree).
> A widget with **several places** for children (an `Item`'s leading/trailing, a `DockFrame`'s
> header) takes a `slot` prop on the child — see
> [Named child slots](#named-child-slots-the-slot-prop).

| Kind | Props it reads | Events |
|------|----------------|--------|
| `Column` / `Row` | `gap` (Int/Float), `align` (Align) | — |
| `Card` | `text` (title) + children | — |
| `Surface` / `Panel` / `Scroll` | (container — children only) | — |
| `Label` | `text`, `bold`, `italic`, `underline`, `strikethrough` (Bool) | — |
| `Badge` / `Tag` / `Alert` | `text` | — |
| **`Button`** | `variant`, `size`, **+ children** (the content); `text`, `icon` = the **childless sugar** | `press` |
| `BadgeButton` | `text`, `variant`, `size` | `press` |
| `Icon` / `IconButton` / `RailCell` | `icon` (Glyph **name**), `size` | `press` (button/rail) |
| `Input` | `text` (value), `name` | `change` |
| `Toggle` | `on` (Bool), `name` | `change` |
| `Checkbox` | `checked` (Bool), `text` (label), `name` | `change` |
| `Gauge` | `value` (Float) | — |
| **`Choice`** | `value` (Text/Int), **+ children** (the content); `text` = the **childless sugar** | `press` (standalone only) |
| **`Select`** / **`Tabs`** | `selected` (Int), **+ `Choice` children** (the options) | `change` — carries the chosen **value** |
| **`ItemGroup`** | `text` (header), `expanded` (Bool), **+ children** (the rows) | `toggle` — carries the new `expanded` |
| **`MarkerGroup`** | `active` (Bool), `nav_selected` (Bool), **+ children** | — (an indicator) |
| **`Grid`** | `columns` / `rows` / `areas` (List of CSS-like strings), `align` + `justify_items`; per-**child**: `area` or `col`/`row`/`col_span`/`row_span`, `align_self` / `justify_self` | — |
| **`DockFrame`** | `text` (title), `expanded` / `frameless` / `active` / `nav_selected` (Bool); **slots**: `header`, else body | `toggle` |
| **`Item`** | `text` (label); **slots**: `leading`, `trailing` (no default) | `press` |
| **`Toast`** | `text` (title), `severity`, `icon`, `body`, `action_text`, `dismissible` | `press` · `dismiss` · `action` |

**The event vocabulary** is three names: **`press`** (activated), **`change`** (the value changed),
and **`toggle`** (a collapsible group folded/unfolded). Each carries what the author actually needs
to act on: a `change` on a picker carries the chosen option's `value`, a `toggle` carries the new
`expanded` state in its args — so an author binds one action and learns which way it went, instead of
tracking the widget's state on their side.

`PropValue` variants: `Bool` · `Int` · `Float` · `Text` · `Size`(`ViewSize`) · `Variant`(`ViewVariant`)
· `Align`(`ViewAlign`) · `Color`(name/`#rrggbb`) · `Glyph`(name) · `List`(`Vec<PropValue>`). A
**`"name"` prop** on a value widget opts it into a submitted modal's returned `data` (see
[Dialog](#dialog) → *Declaring a modal from data*). **`align_self` / `justify_self` are read on every
kind** — they describe a node inside its parent (see [Grid → aligning items](#grid)).

**`List` is deliberately rare.** The option-shaped widgets do *not* use it — their options are
**children**, because an option is a node with a value and content, not a string. What is genuinely
list-shaped is a [`Grid`](#grid)'s track templates (`columns` / `rows` / `areas`), and that is what
it exists for.

**Coverage**: every `WidgetKind` realizes to its widget. The one exception is `ScrollBar`, which is
**host-only by design** — its state is live host signals (`content_extent` / `viewport_extent` /
`offset`), and static serializable data fundamentally cannot drive a signal, so a declarative one
would render a dead control. A plugin uses [`Scroll`](#scrollregion) (a `ScrollRegion`), which owns
its own offset. Same rule applies to any *individual builder* that binds a host signal — e.g.
`DockFrame::rail(..)`.

### Named child slots (the `slot` prop)

Some widgets have **more than one place to put a child**: a [`DockFrame`](#dockframe) has a header
and a body, an [`Item`](#item) has a leading and a trailing slot. `ViewNode.children` is one flat
`Vec`, so the **child says where it goes**, with a `slot` prop:

```rust
ViewNode::new(WidgetKind::DockFrame)
    .text("EXPLORER")
    .child(ViewNode::new(WidgetKind::Badge)
        .text("3")
        .prop("slot", PropValue::Text("header".into())))   // ← the controls slot
    .child(ViewNode::new(WidgetKind::Item).text("src"))    // ← no slot ⇒ the body (default)
    .child(ViewNode::new(WidgetKind::Item).text("tests"));
```

This needs **zero** model surgery — no `slots: Map<..>` on the node, no second child vector, and the
JSON stays flat — and it generalizes to every future slotted widget for free.

Two rules, both of which keep `realize` total for untrusted input:

- **A widget may have a default slot.** `DockFrame`'s is the body, so an unslotted child lands there.
  `Item` has **none** — its middle is the label, which comes from `text` — so a child with no slot is
  **ignored** rather than dropped somewhere it doesn't belong.
- **An unknown slot name is not an error.** It is debug-logged, and the child falls back to the
  widget's default slot (or is ignored, where there is none). A plugin's typo costs it a misplaced
  child, never a panic.

The slot names are part of a widget's declarative contract, exactly like its props — each entry below
lists its own.

### Options are children (`Select` / `Tabs` / `Choice`)

An option is **a node with a value and arbitrary content**, and a picker's options are its
**children** — never a `props["options"]` list of strings. That is what lets a declarative option
compose an icon and a label exactly like a native one:

```rust
ViewNode::new(WidgetKind::Select)
    .prop("selected", PropValue::Int(1))
    .on("change", Intent::new("set_level"))
    .child(ViewNode::new(WidgetKind::Choice)
        .prop("value", PropValue::Text("low".into()))
        .child(ViewNode::new(WidgetKind::Label).text("LOW")))
    .child(ViewNode::new(WidgetKind::Choice)
        .prop("value", PropValue::Text("high".into()))
        .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("lightning".into())))
        .child(ViewNode::new(WidgetKind::Label).text("HIGH")));
```

**The chosen value comes back, not an index.** The widgets track a selected index — they are
indexable lists, that is their business — but an index means nothing to a plugin and breaks silently
the moment the options are reordered. So `realize` captures the options' `value` props and maps the
index back through them: the bound `change` intent fires with **`args["value"]`** set —
`{"value": "high"}`. An option carrying no `value` falls back to `args["index"]`.

Two rules keep `realize` total for untrusted input: a childless `Choice` desugars its `text` to a
`Label` child (**children win**, the same precedence as `Button`), and a child of a `Select`/`Tabs`
that is **not** a `Choice` is ignored rather than realized into a broken option.

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

### The other half of an `Intent` — the action it names

Every `.on_press(Intent::new("rename"))` above is half a contract. The other half is the **action**
that id resolves to. An `Intent` is `{ action: String, args: PropMap }` — a name and some data, nothing
more — which is exactly why the same button works from native code, from a serialized plugin tree, and
from RPC. Two kinds of id resolve, and both go through **one** dispatch door
(`dispatch_view_intent`, `heca/src/app/interaction.rs`):

| Id | Resolves to | Examples |
|----|-------------|----------|
| A **built-in** name | a `WmAction` variant, run by its native handler | `close`, `split_vertical`, `zoom_column`, `chrome.container.move_to_region` |
| A **name-keyed** id | an action registered at **runtime** by a provider or plugin | `plugin.docker.restart` |

Built-in names are snake_case (`close`, `focus_left`); the chrome container-placement actions are the
one dotted-namespace group, because dotted is the scheme plugin ids use. **Args ride both paths**: a
parameterized built-in is constructed from them through the very same `build_action` a `config.toml`
binding uses — so these three produce an identical `WmAction`.

```rust
// From a widget:
ViewNode::new(WidgetKind::Button)
    .text("Send to right sidebar")
    .on_press(
        Intent::new("chrome.container.move_to_region")
            .arg("container_id", PropValue::Text("workspaces".into()))
            .arg("region", PropValue::Text("right-sidebar".into())),
    );
```
```toml
# From config (a mode binding — flat bindings take no args):
[[keys.mode.bindings]]
action = "chrome.container.move_to_region"
keys = "l"
args = { container_id = "workspaces", region = "right-sidebar" }
```
```
# From RPC:
move-container-to-region workspaces right-sidebar
```

**An unknown id is never a crash.** A menu item or a binding may legitimately name an action whose
provider isn't mounted (config is even read *before* providers register). It logs a debug warning and
does nothing.

### Registering a custom (name-keyed) action

`WmAction` is a **closed enum** — a provider or plugin cannot add a variant to it. An action of your
own is therefore keyed by a **stable string id** and registered at runtime, after which every surface
treats it like a built-in: it has a label and an icon, it shows up in menus and introspection, it can
be bound in `config.toml`, and it is judged by the same interaction policy.

Metadata and handler live in two places for a borrow reason, not a design one: an action handler is
`fn(&mut AppState, …)` and gets no registry, so **metadata** must be reachable from `AppState` (the
`ActionCatalog`) while the **handler** table must be borrowable alongside `&mut AppState` (the
`ActionRegistry`). One call registers both.

```rust
use crate::actions::{register_dynamic, unregister_dynamic, ActionCategory, ActionMeta};
use crate::app::interaction::ActionPolicy;
use crate::chrome::PropValue;
use heca_grid_ui::Glyph;
use std::rc::Rc;

// At mount — metadata joins the ONE catalog, the handler joins the registry.
let handle = register_dynamic(
    registry,   // &mut ActionRegistry
    catalog,    // &mut ActionCatalog (lives on AppState)
    ActionMeta {
        name: "plugin.docker.restart".into(),  // the id an Intent / a binding / RPC names
        label: "Restart Container".into(),     // menus, tooltips, the command palette
        description: "Restart the selected Docker container.".into(),
        category: ActionCategory::System,
        default_binding: String::new(),        // no default key; the user may bind it by name
        icon: Some(Glyph::Play),
        policy: ActionPolicy::Global,          // REQUIRED — see below
    },
    // The handler receives the Intent, so args arrive as DATA (never a closure across a plugin
    // boundary). `&mut AppState` is the sanctioned write path: a provider may not mutate state
    // from its build/observe path, but running an action is exactly how it is meant to.
    Some(Rc::new(|state, intent| {
        let Some(PropValue::Text(container)) = intent.args.get("container") else {
            return;
        };
        restart_container(state, container);
        state.needs_redraw = true;
    })),
);

// At unmount — retires BOTH halves (handler and metadata), so the id stops resolving.
unregister_dynamic(registry, catalog, &handle.0);
```

Now any widget can fire it, and the call site looks no different from a built-in:

```rust
ViewNode::new(WidgetKind::Button)
    .text("Restart")
    .prop("variant", PropValue::Variant(ViewVariant::Destructive))
    .on_press(
        Intent::new("plugin.docker.restart")
            .arg("container", PropValue::Text("web".into())),
    );
```

…and a user can bind it, even though it does not exist when their config is read:

```toml
[keys]
"prefix+d" = "plugin.docker.restart"
```

#### `policy` is required, and there is no permissive default

It answers one question: **should this run while a floating pane owns the focus domain?** For a
built-in, an exhaustive `match` makes forgetting to answer a *compile error*; a name-keyed action has
no variant, so the field forces the question instead. A default would mean "the author forgot"
silently resolves to the most permissive setting in the system.

| Policy | Use it when |
|--------|-------------|
| `Global` | App-level with **no layout impact** — must keep working while a floating pane is up (`reload_config` is the model). |
| `FocusedPaneLocal` | Acts on the focused pane in either domain (close, rename, copy). |
| `TiledOnly` | Only meaningful in the tiled column layout (focus, split, resize, move, swap). |
| `WorkspaceLevel` | Switches or mutates workspaces. |
| `SourceDependent` | Allowed only for some interaction sources / targets. |
| `AlwaysAllowed` | ⚠️ **A misnomer** — the router *blocks* it when floating. App-level but layout-affecting (command palette, spawn). **`Global` is the only truly-always-allowed policy.** |

`ActionPolicy` is crate-visible, so an **in-tree** provider names the Rust enum directly (as above). A
WASM plugin, living outside the crate, will declare the same choice as serialized data — the host
honours what the plugin declares. Today's providers are first-party and compiled in, so they are
already fully trusted; the real trust boundary appears at the WASM edge and is designed there.

#### No handler ⇒ a `Declarative` action

Pass `None` instead of a closure and the action is **declared** — it has an id, metadata and a policy,
and it appears in menus and introspection — but the host cannot run it. It is forwarded to its owner
across the plugin boundary. This is how a WASM plugin's actions will be modelled; dispatching one
today is a no-op with a debug warning.

#### Making an action confirm first

Any action — built-in or your own — can declare that it must **confirm before it runs**, as data on
its `ActionMeta.confirm`. The central dispatch gate reads it, so *every* surface that fires the action
(a keybinding, a header button, a **context-menu entry**, RPC) confirms identically; you never add a
prompt at the call site.

```rust
use crate::actions::{ConfirmSpec, ResponseButton};

let mut meta = ActionMeta { /* … name: "plugin.docker.remove", policy, … */ };
meta.confirm = Some(ConfirmSpec {
    message: "Remove the container? This cannot be undone.".into(),
    buttons: vec![
        ResponseButton::cancel("cancel", "Cancel"),
        ResponseButton::proceed("confirm", "Remove", /* danger */ true),
    ],
    dismissible: false,                       // forced choice — Esc / click-outside don't dismiss
    config_name: "plugin.docker.remove".into(), // the [confirm] toggle key (may differ from the name)
    default_enabled: true,                    // used when the user hasn't set [confirm].<key>
});
```

The user turns it off with `[confirm] plugin.docker.remove = false`. `config_name` is the **toggle
key**, deliberately separate from the action name — heca's own `close` action carries a spec keyed
`delete_pane`, so `[confirm] delete_pane = false` disables the pane-close prompt.

> **Native-only escape hatch — `Outcome::Callback`.** A response button's outcome is normally
> declarative (`Proceed` / `Cancel` / `Dispatch` another named action), which serializes and works
> across RPC and (later) WASM. A **native** button may instead run an `Rc<dyn Fn(&mut AppState,
> &ActionRegistry)>` closure — but a closure is not serializable, so the plugin-facing builder does
> **not** expose it, and when metadata is serialized for RPC introspection a `Callback` outcome is
> rendered opaquely, never dropped. Prefer `Dispatch` (portable, testable); reach for `Callback`
> only when the logic genuinely cannot be a named action.

#### `register(ActionSpec)` — the native one-call form

For a native action that *has* a `WmAction` variant (in-tree work, not a plugin), `register` wires the
handler and the metadata together in one call — the counterpart to `register_dynamic`:

```rust
use crate::actions::{register, ActionSpec};

register(registry, catalog, ActionSpec {
    action: WmAction::MyThing,     // dispatched by discriminant (parameterized variants share one)
    handler: handle_my_thing,      // fn(&mut AppState, &WmAction)
    meta: my_meta,                 // label / icon / policy / confirm — the same ActionMeta
});
```

#### Discovering actions at runtime — introspection

The catalog is queryable, so a tool can ask a *running* heca (with whatever plugins are mounted) what
it can do. Two RPC commands, both returning JSON:

```
list-actions              → [ {name,label,description,category,default_binding,policy,confirm}, … ]
describe-action <name>    → one such object, or an error if the name is unknown
```

`confirm` is the toggle key when the action prompts, `null` otherwise; `policy` is the interaction
policy as a stable string (`global`, `tiled_only`, …). A native `Callback` outcome is never
serialized — introspection reports only *that* a prompt exists. In Rust: `ActionCatalog::describe_all()`
/ `describe(name)` → `ActionInfo`.

For the full **in-tree** built-in checklist (a `WmAction` variant, `action_policy` classification, a
default binding, RPC parity), see **[README → Actions System](../README.md#actions-system)**.

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

### Placing a widget at an app-chosen rect

Sometimes the host knows *where* something goes but the widget should still own *what it
looks like* — a chip pinned to a pane's corner, a badge over a cell. The temptation is to
measure the text yourself and call `cx.rect` + `cx.text`; don't. **Let the layout engine
position it**: build a box the size of the target region, offset it with margins, and let
`justify`/`align` place the content inside.

```rust
// A tag pinned to the bottom-right of a pane at (px, py, pw, ph).
let mut root = Flex::row()
    .justify(Justify::End)          // ← push to the right edge
    .align(Align::End)              // ← push to the bottom edge
    .width(Length::Px(pw))          // ← the target region…
    .height(Length::Px(ph))
    .margin_left(px)                // ← …offset to where it lives
    .margin_top(py)
    .padding(Spacing::Sm.scale() * theme.font_size)   // token, not a literal
    .child(Tag::new(label).color(theme.colors.accent));

LayoutEngine::new().base_font(theme.font_size).compute(&mut root, viewport);
root.paint(&mut cx);
```

Margins work on the **root** as well as on a child — `compute` offsets the root by its own
margin, so the box above lands at `(px, py)` with no wrapper needed. (It did not always:
the root was pinned at `(0, 0)` and its margin silently discarded, which drew `heca`'s
search bar over the sidebar, a whole pane from the terminal it described. Fixed in the
engine rather than worked around, so this reads the way it looks.)

Nothing here measures text or computes a size — `Tag` hugs its content and the engine does
the rest. This is what `heca`'s scrollback-search bar does; it previously guessed its own
width from a hardcoded glyph advance ratio (`chars × font × 0.62`), which broke for any
font whose advance differed and had to be re-tuned by hand.

**When a painted primitive is still correct:** content-area *effects* that track something
other than the widget tree — a bell flash washing the pane, a highlight rect over terminal
cells — stay `cx.rect` calls. A widget per terminal match would be absurd. The rule is about
**chrome**: anything the user reads as UI is a widget. Even then the values come from the
theme (`theme.colors.control_radius()`), never literals.

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

If the widget is **interactive**, set `base.focusable = true` (in the constructor for an
always-focusable control, or inside the `.on_click`/`.on_activate` builder for one that is
interactive only once a callback is wired) — do **not** re-implement `Component::focusable()`. The
trait default already returns `base.focusable && !disabled`; override the method only for genuinely
dynamic focusability (e.g. an overlay focusable only while its `open` signal is set).

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
