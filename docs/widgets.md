# heca-grid-ui — Widget Reference

The canonical API reference for the **`heca-grid-ui`** component library: every
foundation type and widget, its properties / methods / events, and runnable
usage examples. (For the Tron/GridCN visual *vision* and component wishlist see
docs/widgets.md — see docs/widgets.md; this file documents what is **actually
implemented**.)

`heca-grid-ui` is **GPU-free**: a component tree emits a `Scene` (a display
list of `DrawCommand`s) which `heca-renderer` rasterizes. It is **signal-driven**
(fine-grained reactivity via a `floem_reactive` facade) and **composable**
(every widget embeds a `Base` and implements the `Component` trait).

---

## Table of contents

- [Mental model](#mental-model)
- [Getting started](#getting-started) — depend, build a tree, lay out, paint, render, wire events
- [Foundations](#foundations) — `Base`, `Component`, **[the event model](#the-event-model--the-framework-resolves-the-pointer-and-walks-the-tree)**, builder traits (incl. **[`ComponentExt`](#componentext--what-every-widget-gets)**), `Style`, [Font sizing](#font-sizing), `Theme`/`GlowLevel`/`Intensity`, **[the glow model](#the-glow-model--who-owns-what)**, **[the focus model](#the-focus-model--ring-visibility)**, `Color`, signals, events, `Action`, `Scene`/`PaintCx`, `Flash`, `Attention`, **[Animations](#animations--how-a-surface-arrives-and-leaves)**, **[Search](#search--matching-ranking-by-use-and-query-history)**
- [Widgets](#widgets)
  - Layout: [`Flex`/`Container`](#flex--container), [`Surface`](#surface), [`Card`](#card), [`Pane`](#pane), [`Grid`](#grid), [`ScrollRegion`](#scrollregion), [`ScrollBar`](#scrollbar)
  - Text: [`Label`](#label)
  - Interactive: [`Button`](#button), [`ButtonGroup`](#buttongroup), [`IconButton`](#iconbutton), [`Toggle`](#toggle), [`Checkbox`](#checkbox), [`Input`](#input), [`Tabs`](#tabs), [`Select`](#select), [`Choice`](#choice), [`Item`](#item), [`Row`](#row), [`BadgeButton`](#badgebutton)
  - Display: [`Badge`](#badge), [`StatusDot`](#statusdot), [`Separator`](#separator), [`Spinner`](#spinner), [`Alert`](#alert), [`Toast`](#toast), [`ProgressBar`](#progressbar), [`Gauge`](#gauge), [`Icon`](#icon), [`Tag`](#tag)
  - Chrome (sidebars/docks): [`ItemGroup`](#itemgroup), [`MarkerGroup`](#markergroup), [`DockFrame`](#dockframe), [`ChromeRegion`](#chromeregion), [`RailCell`](#railcell), [`KeyHint`](#keyhint), [`KeyHintGroup`](#keyhintgroup), [`FocusScope`](#focusscope)
  - Overlays: [`Overlay`](#overlay) (the base layer), [`Tooltip`](#tooltip), [`Dialog`](#dialog), [`CommandPalette`](#commandpalette), [`ToastStack`](#toaststack)
  - Menus: [`MenuItem` / `Menu` / `ContextMenu`](#menus--menuitem-menu-contextmenu) — declared on the widget they belong to
  - Glyphs: [`Icon`](#icon) (Phosphor pictograms), [`NfIcon`](#nficon) (Nerd Font — the keyboard set)
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

**Layout comes first, and nothing is painted before it has a box.** A widget's layout node is
written as the engine walks, so one the walk has never reached has no bounds — and its default ones
sit at the window's origin with no size. The paint pass skips such a widget entirely rather than
drawing it there. You get this without asking for it, in both directions:

- **Building a tree, then painting it without laying it out, draws nothing.** That is the order the
  app always uses, and a test that paints must compute layout first.
- **A widget added to a tree *while* that tree is being laid out** — a container putting a child
  back once the room returns, say — is simply not drawn until the next layout reaches it. Without
  the rule it appeared in the top-left corner of the window for one frame, which during a drag is a
  continuous flicker.

### 4. Wire input

The host maps platform keys onto the renderer-agnostic `GridKey`/`Modifiers` and drives
the tree:

```rust
// Pointer: ONE kind of event, carrying the button and the modifiers. The framework hit-tests it,
// pairs the press with the release, counts the run, and delivers what it meant — a `Click`, a
// `RightClick`, an `Enter`/`Leave`, a drag. `FocusManager::dispatch` also focuses what was pressed.
focus.dispatch(&mut ui, &Event::pointer_moved(pos));
focus.dispatch(&mut ui, &Event::pointer_pressed(pos, PointerButton::Left));
focus.dispatch(&mut ui, &Event::pointer_released(pos, PointerButton::Left));
focus.dispatch(&mut ui, &Event::wheel(pos, delta_x, delta_y));

// The pointer left the window: hover, capture and any drag end with it.
heca_grid_ui::dispatch(&mut ui, &Event::pointer_cancelled());

// modifiers (broadcast so text widgets can do word/line editing)
heca_grid_ui::dispatch(&mut ui, &Event::ModifiersChanged(mods));

// keyboard: the key for shortcuts and navigation, the TEXT for typing — a field types from
// `TextInput` alone, so nothing has to reconstruct a character from a combo key.
if let Some(text) = committed_text {
    heca_grid_ui::dispatch(&mut ui, &Event::TextInput(text));
}
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
| `on_event_capture(&mut self, ev) -> Handled` | `No` | Handle an event **before** this widget's children see it. `Yes` stops the walk. See [the event model](#the-event-model--the-framework-resolves-the-pointer-and-walks-the-tree). |
| `on_event(&mut self, ev) -> Handled` | `No` | Handle an event the children **declined**. Registered [`ComponentExt`](#componentext--what-every-widget-gets) handlers run just before it, so an author's handler can take the event from the widget's own behaviour. |
| `hit_bounds(&self) -> Option<Rectangle>` | `Some(bounds)` | The rect this widget occupies **for input**, or `None` when it takes none at all. Overridden by a widget that draws a floating panel (a menu, a palette) or that is inert right now (a closed [`Overlay`](#overlay), whose subtree is still laid out). |
| `after_subtree(&mut self, ev, handled)` | no-op | Called **after this widget's subtree has seen `ev`**, consumed or not — for a container that reports what its own rows did ([`ItemGroup`](#itemgroup), [`DockFrame`](#dockframe)). Observation only: it cannot consume the event or revive a consumed one. |
| `clips_children(&self) -> bool` | `false` | `true` when the widget clips its children, so a press or move **outside its bounds** must not reach them — content scrolled out of sight stops being clickable. |
| `wants_visible(&self) -> bool` | `focused` | `true` when an enclosing [`ScrollRegion`](#scrollregion) should **keep this widget in view**. See [following the cursor](#following-the-cursor). |
| `remeasure(&mut self)` | no-op | Recompute size from the resolved font (`Base::font`). The layout pass calls it on every node after resolving the font (see [Font sizing](#font-sizing)). Font-sized widgets override it. |
| `on_layout(&mut self)` | no-op | Called post-order once this node's (and its descendants') bounds are freshly computed. Override to **place** children the engine could not put where they are drawn — a [`ScrollRegion`](#scrollregion) re-bakes its scroll offset, a [`Select`](#select) re-places its option rows into the overlay panel. Bounds are natural again on entry, so the widget re-derives its shift from scratch instead of compounding it. |
| `on_focus(&mut self, visible: bool)` | set `focused`/`focus_visible` | Gained focus. |
| `on_blur(&mut self)` | clear them | Lost focus. |
| `tick(&mut self, dt: f32) -> bool` | recurse to children | Advance animations; `true` ⇒ animating. |

### The event model — the framework resolves the pointer and walks the tree

**A widget never routes events to its children, and never works out what an event meant.** Both are
`heca_grid_ui`'s, once, for every widget:

```
Event::Raw(RawPointer)                     ← the ONLY pointer event a host builds
       │
       ├─ hit_test ──────────► the target under the pointer, and the ancestors above it
       ├─ hover diff ────────► PointerEnter / PointerLeave
       ├─ press+release ─────► PointerDown / PointerUp / Click / DoubleClick / RightClick / …
       ├─ drag threshold ────► DragStart / Drag / DragEnter / DragOver / DragLeave / Drop / DragEnd
       └─ delivery:
              on_event_capture   root → … → target      (Yes stops here)
              handlers, on_event  target → … → root      (Yes stops here)

Event::Key / TextInput / Widget            ← the keyboard, which has no position
       │
       └─ the target is THE FOCUS OWNER, and delivery is the same walk:
              on_event_capture   root → … → focus owner
              handlers, on_event  focus owner → … → root
```

Two halves of the vocabulary, and the line between them is the whole design:

| | |
|---|---|
| **Raw** | `Event::Raw(RawPointer)` — a device fact: moved, pressed, released, wheel, cancelled. A host builds these and **a widget never sees one**. |
| **Resolved** | `Click`, `RightClick`, `PointerEnter`, `Scroll`, `Drop`, `Focus`, `Mount`, … — what happened, already hit-tested and already paired. This is what a widget handles. |

#### What a widget author has to know

Nothing. Position a widget and the events arrive, hit-tested:

```rust
impl Component for MyControl {
    /// The press is mine, and my composed content must not take it first.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        match ev {
            Event::PointerDown(_) => Handled::Yes,   // …and this captures the pointer
            _ => Handled::No,
        }
    }

    /// What a click on me does. No `bounds.contains(pos)`: the router already asked that,
    /// including which widget is on top and what is clipped away.
    fn on_event(&mut self, ev: &Event) -> Handled {
        match ev {
            Event::Click(_) => { self.activate(); Handled::Yes }
            _ => Handled::No,
        }
    }
}
```

**Why it is the framework's.** It used to be every widget's: each one received every pointer event
wherever the pointer was, and each re-derived the same four facts by hand. Six widgets carried a
private copy of `bounds.contains(pos)` to know whether they were hovered; `Input` kept its own clock
to count clicks; a scrollbar thumb tracked its own grab so a move outside its bounds still reached
it; and **no widget could receive a right-click at all**, because a press carried no button — so the
app rebuilt "what did you click" from a position and a registry of row identities, and the whole
chain fell silent the day one row forgot to declare its key. N copies of a rule is a missing API.

#### The vocabulary

| Event | When |
|---|---|
| `PointerDown(PointerEvent)` | a button went down on this widget. **Consuming it captures the pointer** (below). |
| `PointerUp(PointerEvent)` | the button came up, ending that gesture. |
| `PointerMove(PointerEvent)` | the pointer moved over this widget — or anywhere, while it holds capture. |
| `PointerEnter` / `PointerLeave` | the pointer came over / left this widget. Sent to the whole ancestor chain, like CSS `:hover`. Never consumable — leaving is not something a neighbour can veto. |
| `Click` / `DoubleClick` / `TripleClick` | press and release on the same widget. **Every click is a `Click`**; a run *adds* `DoubleClick` (then `TripleClick`) on top, so a control that only understands single clicks still fires on the second. |
| `RightClick` / `MiddleClick` | the same, with that button. A right-click **never** also fires a `Click`. |
| `PointerDownOutside` | a press landed somewhere that is not this widget or a descendant — the whole of "click away to close", with no geometry of the popup's own. |
| `Scroll(PointerEvent)` | the wheel turned over this widget; deltas are in `delta_x`/`delta_y`. A region that cannot scroll the axis asked for declines and it bubbles outward — which is what makes nested scroll areas work with nothing declared. |
| `DragStart` / `Drag` / `DragEnd` | a drag from this widget: it declared a [`draggable`](#dragext) id and the pointer passed the 8 px threshold while held. `DragEnd` always arrives, dropped or not. |
| `DragEnter` / `DragOver` / `DragLeave` / `Drop` | a drag over this **drop target**; `DragEvent::side` (`Before`/`Onto`/`After`) follows the pointer, so an insertion marker tracks it for free. |
| `Key { key, pressed }` | a key, delivered to the **focus owner** and then up its ancestors (see [the keyboard](#the-keyboard--delivery-follows-focus)). |
| `TextInput(String)` | text the user **committed** — typed, pasted, or composed by an IME. Distinct from `Key`: `Shift+2` is `Char('2')` there and `"@"` here. A field types from this and from nothing else. |
| `ModifiersChanged` | broadcast; observers return `Handled::No`. |
| `Widget(WidgetIntent)` | a semantic, configurable intent resolved from `[keys.widgets]`. |
| `Hint(HintEvent)` | a picker's letter resolved to a widget. The picked widget's own `on_hint` runs in the **target phase only**; what bubbles is this event, so a container can watch — and take — picks from its children. Carries the picked widget's `key` and `bounds`. |
| `Focus` / `Blur` | this widget gained or lost keyboard focus — the moment to select-all, commit an edit, or close a popup. |
| `Mount` / `Unmount` | it entered a live tree (first layout pass), or is being dropped (a rebuilt tree throwing the old one away). |

`PointerEvent` carries `pos`, `button`, `modifiers`, `click_count` and the wheel deltas — one
payload for the whole vocabulary. **The position is already hit-tested**: a widget receiving one is
the target or an ancestor of it.

#### Hit-testing, in three rules

- **children before the parent**, last-added first — what is drawn on top is what is hit;
- **an overlay wins over child order**: a dropdown panel drawn above a row that comes *after* it in
  the list still takes the click, because the panel says it occludes that point;
- **a clipping widget's children are unreachable outside it** — content scrolled out of sight stops
  being clickable, which is what `clips_children` means.

A widget's own rect is tested **after** its children, and descent does not require the parent to
contain the point: a [`Select`](#select)'s option list is a child placed outside the trigger it
belongs to, and it is still the thing under the cursor.

A widget whose input surface is not its layout box says so with **`hit_bounds() -> Option<Rectangle>`**
— a [`ContextMenu`](#menus--menuitem-menu-contextmenu) or [`CommandPalette`](#commandpalette) reports the panel it draws;
a closed [`Overlay`](#overlay) returns `None`, which takes its whole subtree out of the pointer's
reach while leaving it laid out. It is the input twin of `damage_bounds`, for the same reason: what
a widget draws, what it damages and where it can be clicked are three questions.

> These two are still here, and are meant to go: a floating widget should *place* itself
> (bake its offset into its own bounds) rather than describe a second rect. The first attempt at
> that overwrote `base.bounds` in `on_layout`, which fights every other reader of bounds — paint,
> damage, placement — and produced ghosting and stray hit targets. Doing it properly means the
> floating panels become **real placed children**, which is not built yet.

#### Capture

**A widget that consumes a `PointerDown` captures the pointer**: every move, and the release, come
to it wherever the cursor goes, until the button is up. That is what a scrollbar thumb, a slider and
a rubber-band selection need, and it is why gating a release on position welds a thumb to the
cursor. The click that press turns into is delivered to the capture holder too, which is what makes
"one control, one click target" a framework rule rather than something each control arranges by
swallowing events from its own content.

#### `Handled` — the answer every widget gives, and what it costs

`Handled::Yes` is the only lever a widget has over the walk, and it does more than it looks like.

**If you know the DOM:** `Yes` from `on_event_capture` is `stopPropagation()` in a capture listener
(nothing below sees it); `Yes` from `on_event` is the same in a bubble listener (no ancestor sees
it). But the DOM splits "nobody else handles this" from "and the built-in behaviour must not run";
here there is **one** answer and `Yes` means both — closer to jQuery's `return false`. A `Base`
handler says it by name: `cx.stop_propagation()`.

**A third meaning the DOM has no equivalent for: the host reads it.** `dispatch` returns the
accumulated answer, and heca uses it to decide whether the input *also* belongs to what sits behind
the tree — the terminal. `No` on a wheel is what lets the pane scroll instead of the sidebar; `No`
on a right press is what lets the context menu open. `Yes` is not private: it tells the application
the input is spent.

> **Claim what you act on, and nothing else.** An overlay once returned `Yes` for every key it was
> offered, including ones it ignored. `q` — catalogued, and bound to `close_overlay` in the `layer` floor —
> did nothing at all while a layer was up, because the layer swallowed it before the host could
> resolve it.

**`Yes` on a `PointerDown` does three things at once:**

1. **stops the walk** — the control's composed content never sees the press;
2. **captures the pointer** — every move and the release come back to it wherever the cursor goes
   (the DOM's `setPointerCapture`, without asking for it);
3. **claims the click** — the `Click` that press becomes is delivered to it, not to the deepest
   widget under the cursor.

Which is why a control does not write it by hand: it declares `Base::one_click_target` and the
router does all three, **for the primary button only**. Nine widgets wrote the claim themselves once
and all nine claimed *every* button — which is how a right-click on a list row reached nothing while
the same click on empty space opened a menu.

| Situation | Answer |
|---|---|
| I acted on this event | `Yes` |
| I looked and it is not mine | `No` — including from capture, which just means "I looked, carry on" |
| I observed it and others should still get it (a modifier broadcast, a hover cue) | `No` |
| I am a container and my child should decide | `No` — you forward nothing; the framework already walked there |

**Events whose answer is ignored:** `PointerEnter`/`PointerLeave` (leaving is an announcement — a
widget must not be able to veto its neighbour's), `PointerDownOutside` (the press belongs to
whatever it landed on), `Mount`/`Unmount`, and `after_subtree`.

#### Choosing a hook

**Capture** for something you take *away* from your subtree: a press that must not reach composed
content, a swallow, or state that must be current before anything below is hit-tested. Capture
returning `Handled::No` is normal — it means "I looked, carry on".

**Bubble** (`on_event`) for what you do with what nobody below wanted. This is also where a widget's
own behaviour belongs, so an [`ComponentExt`](#componentext--what-every-widget-gets) handler registered on
it gets first refusal.

**`after_subtree(ev, handled)`** for a container that **watches what its own subtree did**, whether
or not something in there consumed the event: an [`ItemGroup`](#itemgroup) reports a toggle when the
header row inside it flips `expanded`, and the header consuming that click is the normal case.
Before it existed, those containers took over the whole child walk to get the same observation —
which made every event kind depend on that container forwarding it correctly, forever.

**There are no exceptions left.** `Select`, `Dialog`, `Overlay`, `CommandPalette`, `ContextMenu` and
`FocusScope` each used to declare `routes_own_subtree` and walk their own children for the events
that have no position to route by. That predicate is **gone**: keys route by focus,
which is the same statement said in the vocabulary the rest of the framework already used, so there
is nothing left for those widgets to gate. **No container forwards any event any more.**
`tests/pointer_delivery.rs` mounts a probe inside each container and fails if a pointer kind goes
missing; `tests/pointer_routing.rs` holds the resolution itself to the rules above.

### The keyboard — delivery follows focus

`Event::Key`, `Event::TextInput` and `Event::Widget` carry no position, so the framework routes them
by the only other thing that says where input is aimed: **`Base::focused`**.

- **The target is the deepest focused widget.** Capture runs from the root down its ancestor chain,
  then handlers and `on_event` run back up — the same target-and-bubble walk the pointer uses.
- **A key or typed text stops at the owner.** It belongs to one widget. This is the rule that stops
  the first row in a list eating an Enter meant for the row the cursor is on; seven widgets used to
  patch that by hand, six of them by forgetting to.
- **A `Widget` intent enters the focused region.** An intent is not a key — it is what a key
  *resolved to*, a capability named out loud (`ScrollPageDown`, `Dismiss`). It is addressed to the
  focused region, so that subtree is walked and whichever widget owns the capability answers, and a
  host sends one intent without knowing where any of them sits.
- **Nothing focused, nothing delivered.** A keyboard event with no owner belongs to no widget.

A surface that wants keys must **hold focus**, which it already had to do to draw a focus ring:

| widget | how it says the keyboard is here |
|---|---|
| `Input`, `Button`, `Row`, `Item`, `Choice`, `Tabs` | ordinary focus — a click or Tab, via `FocusManager` |
| `Overlay`, `ContextMenu`, `CommandPalette` | `Base::focused` is bound to the **open** signal: open *is* focused |
| `Select` | opening the list focuses it |
| `Dialog` | its own `FocusManager` focuses a field or button, so text reaches the field and the dialog hears what the field declined on the way back up |
| `FocusScope`, `ScrollRegion` | `Base::focused` is bound to the **host's** keyboard-target signal. A dock binds the **same** signal to both: the wrapper draws the ring, the region answers the keys |
| `CardGrid` | focusable in its constructor — it moves a cursor with the keyboard |

Three predicates paid for this before: `takes_raw_keys` ("I take keys without being focused"),
`takes_text_input` ("I may claim typed text") and `routes_own_subtree`. All three were questions a
widget had to answer *about itself* so the framework could route correctly — and each had a wrong
answer that failed silently. `ContextMenu` answered "I take raw keys" and ended its match with
`_ => Handled::Yes`, which ate `Event::TextInput`; because a host offers text before it resolves the
key, **every quick-pick letter in every menu did nothing** while the suite stayed green.

> **Surfaces:** build one kind of pointer event — `Event::Raw(RawPointer)` — with the **button**
> and the **modifiers** on it, and send it to every tree you mount. Send
> `Event::pointer_cancelled()` when the pointer leaves the window, or hover survives the cursor
> going elsewhere. For the keyboard, hand the whole press to **`Keymap::deliver_press(&KeyPress, deliver)`**
> and write none of the order yourself: it sends the committed text, then the key, then the intents
> the key resolves to. heca's app and its showcase each wrote that sequence out privately, and the
> showcase's copy had no text step at all — so the identical `CommandPalette` typed in one surface
> and was deaf in the other, with nothing wrong to look at in either. **A component author never
> meets any of this**: they mount a widget in a surface and type into it.

### Following the cursor

A widget can say it is the current one, and **every `ScrollRegion` it is ever placed inside brings it
into view** — no `ensure_visible` call at any host:

```rust
impl Component for MyRow {
    /// The navigation cursor is "the current one" for this list.
    fn wants_visible(&self) -> bool {
        self.nav.get_untracked() || self.base.focused.get_untracked()
    }
}
```

The default is keyboard focus, which is what browsers do. [`Row`](#row),
[`MarkerGroup`](#markergroup) and [`DockFrame`](#dockframe) add their nav cursor, which is what makes
the app's keyboard-driven sidebar scroll to follow.

It follows the selection **without fighting the wheel**: the region remembers the target's *natural*
(unscrolled) position, which changes when the selection moves and stays put when you scroll by hand.
So it comes to the cursor once, and then leaves you alone.

#### The reveal follows the keyboard, never the pointer

**Pointing at something never scrolls it.** A reveal exists to bring into view what the user cannot
see — the keyboard's case, where the cursor can move off-screen. What the mouse is on is visible by
definition, and scrolling it moves it out from under the mouse that asked.

#### `CardGrid` — described

A picker surface — a map, a chooser, a palette of tiles — is a `CardGrid`. **Each child is one card:
any kind, any subtree.** There is no card type to conform to.

```rust
ViewNode::new(WidgetKind::CardGrid)
    .on("activate", Intent::new("docker.open"))     // the chosen card's key arrives as `key`
    .on("move", Intent::new("docker.preview"))      // optional: every cursor move
    .on("dismiss", Intent::new("docker.close"))     // optional: Escape
    .child(
        ViewNode::new(WidgetKind::Surface)
            .key("nginx")                            // optional — see the `key` rule above
            .child(ViewNode::new(WidgetKind::Icon).prop("icon", "container"))
            .child(ViewNode::new(WidgetKind::Label).text("nginx")),
    )
```

Arrow keys move the cursor, hovering moves it, Enter activates, Escape dismisses — none of it
declared. **Each child is its own column, laid out left to right**, so ←/→ walk the cards in the
order you see them; a grid of several rows is built natively, with one `row(..)` call per row.

The card's own `key` is what activation hands back, so a plugin gets its own vocabulary (`"nginx"`,
never an index). **A card that declares none is named by what it reads as** — the same
accessible-name algorithm the rest of the library uses (`Component::text_summary`), so an
`Icon` + `Label("nginx")` card comes back as `"nginx"` with nothing wired. Two cards that read the
same are still two cards, indexed exactly as anonymous widgets are elsewhere: `nginx`, `nginx[1]`.
Declare a `key` when the name must outlive a change of wording — a derived name moves when the
text does.

⚠️ **The lit card is not something a description wires.** Natively a caller hands the grid each
card's own state signal (`GridCell::new(key, card.nav_state())`), which a description cannot express
— it has no way to name another node's signal. The realizer builds **both** the card and the cell,
so it makes that connection itself. That is why a `CardGrid` can be described while a `ScrollBar`
cannot: there, the signal genuinely comes from outside, and a described one would be a dead control.

A list whose cursor only the keyboard moves needs nothing. One whose cursor **also follows the
mouse** gates the request on a signal: [`CardGrid`](#cardgrid) publishes `reveal_state()` — true
while the keyboard moved the cursor, false while the mouse did — and each card takes it:

```rust
let grid = CardGrid::new();
let reveal = grid.reveal_state();
let card = Row::new().reveal_when(reveal);   // one line per card
```

It is given to the **cards**, not read off the grid, because the `ScrollRegion` that scrolls is
*inside* the grid: the walk that collects reveal requests starts at the region and never passes
through its ancestors. (An earlier attempt put the rule on the parent as a `reveals_subtree`
override; with that nesting the region never consults it, and it silently did nothing.)

Without it the three correct behaviours compose into a wrong one: hover moves the cursor → the
cursor card asks to be visible → the region centres it → the card slides out from under the pointer,
possibly onto another card, which slides again.

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
| `.padding_xy(x, y)` | Per axis: `x` left+right, `y` top+bottom. |
| `.padding_left/right/top/bottom(f32)` | **One side**, overriding the axis and the uniform value (side → axis → uniform, the same cascade as the per-side margins). Use it to reserve space along a single edge without moving the opposite one — a [`ScrollRegion`](#scrollregion) keeping content clear of its scrollbar is the case that asked for it. Also settable from a description, for free: the declarative property surface *is* `Layout`'s own fields. |
| `.width(..)` / `.height(..)` / `.min_*` / `.max_*` | **Any spelling a size is written in** — see the table below. **Set neither width nor height and the widget fills its parent across the cross axis**, exactly as CSS `align-items: stretch` does, so a panel with no width in a 600px column is 600px wide and `.width(Length::FULL)` on a child that already fills says nothing; leave it off. |
| `.grow(f32)` | Flex-grow factor — a share of what is **left over** after the fixed children. That is what flex-grow means, so "a share of the widest sibling" is a **percentage against one denominator**, not a grow weight. |
| `.disabled(bool)` | Dim + make inert + drop from focus order. |
| `.tab_index(i32)` | Explicit Tab order; indexed widgets visited first, ascending. |

#### Writing a size — one vocabulary, native and described alike

Every sizing builder takes `impl Into<Length>`, so a size is written the way it is said:

```rust
.width(200)              .width(200.0)     // pixels — integer or decimal
.width("200px")          .width("200")     // the px suffix is optional
.width("50%")                              // a fraction of the parent
.width("auto")                             // sized by content and flex rules
.width(Length::HALF)                       // FULL / HALF / THIRD / QUARTER
// …and nothing at all                     // already fills the parent
```

| written | means |
|---|---|
| `200` / `200.0` / `"200"` / `"200px"` | logical pixels |
| `"50%"` | a fraction of the parent |
| `"auto"` | sized by content and flex rules |
| `Length::FULL` / `HALF` / `THIRD` / `QUARTER` | the same fractions, with no number to mistype |
| *nothing* | fills the parent across the cross axis — CSS `align-items: stretch` |

**There is exactly one parser** (`Length: FromStr`), and `Deserialize` calls it — so a call site,
a plugin's description, an RPC message and `config.toml` can never come to disagree about what
`"50%"` means. Held by `the_wire_and_a_call_site_read_a_size_through_the_same_parser`.

⚠️ **`"50"` is fifty pixels, not half** — exactly as in CSS. The `%` is what makes it a fraction.

⚠️ **`Length::Percent` takes a fraction, `0.0..=1.0`, not a 0–100 percentage.** Half is
`Percent(0.5)`; `Percent(50.0)` is fifty times the parent and nothing warns you. The fraction is
taffy's convention underneath and `"50%"` is the human spelling, so `Percent(0.5)` serializes to
`"50%"` and parses back from it. **Prefer `"50%"` or `Length::HALF` at a call site** and let the
variant stay inside the library, where the convention is consistent.

⚠️ **A string nobody can read becomes `Auto`, it does not panic** — the same rule the grid's track
vocabulary follows, because these spellings arrive from a plugin and from config as well as from
Rust: a typo costs its author a differently-sized box rather than taking the host down. Use
`"…".parse::<Length>()` when you want to be told instead.

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

<a id="componentext--what-every-widget-gets"></a>
**`ComponentExt`** — **everything every component gets**: handlers (what happens to it), `key`
(who it is), `hintable` (whether the picker may reach it) and the drag slots. It was four traits — `ComponentExt`, `ComponentExt`, `DragExt`, `HintExt` —
split by nothing but the order they were added in; all four were unconditional, so the split carried
no rule. The two that remain separate do carry one, enforced by the type system: `StyleExt` is
surfaces only, `Parent` is containers only.

**One spelling, one argument, every kind.** A handler takes an `&mut EventCx` carrying the event
**and** the one lever over the walk, and `.on(kind, f)` is that same function with the kind as a
parameter — not a different API:

```rust
let row = Row::new()
    .child(Label::new("pane-1"))
    .on_click(|e| { e.stop_propagation(); select(); })  // mine
    .on_right_click(|e| open_menu_at(e.pos()))          // watched, not claimed
    .on_key(|e| { let _ = e.event(); });                // keys, same shape
```

`EventCx` answers `event()`, `pointer()`, `drag()`, `pos()`, `modifiers()` — and
`stop_propagation()`.

> **Nothing is consumed for you** (the DOM's rule). The named builders
> used to consume the event whether or not you wanted it, so anything that needed to *watch* a click
> without claiming it had to drop to the differently-shaped `.on(kind, |cx| …)`, which was the only
> form able to say `stop_propagation`. Two spellings over one input; now there is one. **A handler
> that used to rely on the implicit claim must now say `e.stop_propagation()`.**

| Method | Effect |
|--------|--------|
| `.on_click(f)` / `.on_double_click(f)` / `.on_triple_click(f)` | A left click on this widget (or a descendant that did not take it). |
| `.on_right_click(f)` / `.on_middle_click(f)` | The same for those buttons. This is the whole of "a widget can have its own menu": it hears the click, on itself, with the position. |
| `.on_pointer_down(f)` / `.on_pointer_up(f)` / `.on_pointer_move(f)` | The raw halves of a gesture. Stopping propagation on a `pointer_down` **captures the pointer** until the release. |
| `.on_pointer_enter(f)` / `.on_pointer_leave(f)` | Hover transitions. Several widgets on one path enter together, so claiming one is almost always wrong. |
| `.on_scroll(f)` | The wheel over this widget. Stop propagation to keep it from also scrolling whatever contains this widget. |
| `.on_pointer_down_outside(f)` | A press landed somewhere else — how a popup closes itself. |
| `.on_hint(f)` | **What a pick does to this widget, when that differs from acting on it** — the override, on every widget. Fires in the **target phase only**; listen with `.on(EventKind::Hint, …)` to watch picks from your children instead. |
| `.on_key(f)` / `.on_text_input(f)` | A key, or committed text, that reached this widget — it holds the keyboard, or contains what does. Intercepting a shortcut or a quick-pick letter is the same one line as intercepting a click. |
| `.on_focus_gained(f)` / `.on_focus_lost(f)` | Keyboard focus arrived or left. (Named this way because `Component::on_focus` is the widget's own hook for the same moment.) |
| `.on_mount(f)` / `.on_unmount(f)` | Entered a live tree (first layout pass) / is being dropped. `on_unmount` is where a widget releases what it registered with the host, at the moment the tree that registered it goes away. |
| `.on_drag_start(f)` / `.on_drag(f)` / `.on_drag_end(f)` | The drag this widget is the source of. |
| `.on_drag_enter(f)` / `.on_drag_over(f)` / `.on_drag_leave(f)` / `.on_drop(f)` | A drag over this drop target; `DragEvent::side` says where in it. |
| `.on(kind, f)` | The general form the rest are sugar over. |

**`Base::one_click_target`** — a control declares that the **primary press lands on it, not on the
content it composes**, and the router applies it during capture. The pointer twin of
`focus_barrier`: one thing to click, one thing to Tab to, whatever it holds. `Button`, `Row`,
`Item`, `Choice`, `Checkbox`, `Toggle`, `IconButton`, `BadgeButton` and `RailCell` set it. It claims
the **left** button only — every other button carries on to whatever answers it, which is what lets
a right-click on a row reach a menu instead of dying on the row.

**Handlers run in the bubble phase**, after this widget's descendants have had the event and
**before** the widget's own `on_event` — so a handler sees what its children declined, and
`stop_propagation()` takes the event from the widget itself (a `Button` will not fire).

A widget's own inherent builder wins where it has one: `Button::on_click(|| …)` is the **button's
action** — what a click *or* Enter *or* `activate()` runs — rather than an event listener, so it
takes no argument and there is no event to carry.

**`Parent`** (containers):

| Method | Effect |
|--------|--------|
| `.child(impl IntoComponent)` | Append a child — a widget, **or a subtree someone else already built**. |

> **One builder per slot, never two.** `Box<dyn Component>` is not itself a `Component`, and for
> that single reason every child-taking builder used to come in twos — `child`/`child_boxed`,
> `body`/`body_boxed`, `leading`/`leading_boxed`, sixteen in all. A caller holding a dynamically
> built subtree, which is what `realize()` returns from a description, had to know which spelling to
> reach for; a widget author had to remember to write both; and a new slot that forgot its twin was
> simply unreachable from the declarative side, with nothing to report it.
>
> `IntoComponent` is the bound that covers both, and it is implemented for you — a caller never
> names it. An already-boxed subtree is **handed through, not boxed again**
> (`an_already_boxed_subtree_is_handed_through_rather_than_wrapped`). Every named slot takes it on
> the same terms: `Dialog::body`, `DockFrame::header`, `Item::leading`, `Overlay::panel`,
> `Grid::cell`, `KeyHint::new`.
>
> **When you add a slot to a widget, take `impl IntoComponent`.** That is the whole rule.

<a id="hintable-and-being-pickable"></a>
**Being pickable** (part of `ComponentExt`):

| Method | Effect |
|--------|--------|
| `.hintable(bool)` | Keep this widget **out of the picker**, however actionable it is. Default `true`. |

**Being pickable is not opt-in**. A widget you can act on — a click, a double click,
a key — is offered a letter by `prefix+/` and by any enclosing [`KeyHintGroup`](#keyhintgroup) with
nothing declared, and picking it does what clicking it does. So the only thing left to say is
"not me":

```rust
Button::new("×").hintable(false)   // a close button on every row would eat a letter each
```

`.hintable(true)` is the default and does nothing on a widget nobody can act on — there would be
nothing for the letter to run.

**And a widget nobody can see gets no letter.** A target scrolled out of a clipping ancestor — a
sidebar row past the fold — is dropped by the candidacy walk, through the same
`Component::clips_children` that paint and input already honour. Nothing to write:
put a widget in a `ScrollRegion` and its letters follow the fold. **A row you can half see keeps
its letter**, and the keycap is drawn whole rather than cut, so you can still read what to press.

| what you write | what happens |
|---|---|
| nothing | actionable → gets a letter; picking it does what clicking it does |
| `.on_hint(…)` | gets a letter; picking it does **this** instead (heca's sidebar row: a click leaves the sidebar, a pick stays). On **every** widget — it was a `KeyHint` builder before |
| `.hintable(false)` | never gets a letter, however actionable it is |

**"Actionable" is `Base::activatable`**, set wherever an action is wired: once in `ComponentExt::on`
for the generic listeners (`Click`, `DoubleClick`, `Key` — so `on_click`, `on_double_click`,
`on_key_down`, `on_key_up`), and in each widget that keeps its own callback instead (`Button`,
`IconButton`, `BadgeButton`, `Toast` via `on_click`; `Choice`, `RailCell`, `Item`, `Row` via
`on_activate`). It is a field and not a question asked of the handler list, because those eight
store their action privately — `Handlers::has(Click)` is `false` for a `Button`. Right-click and
middle-click are deliberately excluded: a right-click opens a context menu rather than doing the
thing, so it should not spend one of the 52 letters.

**Letters are scarce.** One picker hands out 52, one keystroke each — `.hintable(false)` is how a
dense surface keeps them for the targets that matter.

> **Declarative form:** `hintable` is an ordinary boolean prop on the node, read for every kind — see [Identity props](#identity-props--key-and-hintable-on-every-kind).

**Identity** (part of `ComponentExt`):

| Method | Effect |
|--------|--------|
| `.key(impl Into<String>)` | The identity of **this item**, when it is one of a collection you are iterating. |

> **`key` replaced `nav_key`** — do not reintroduce the old name. `scope_key` is **still here**:
> folding it into `key` turned out to change behaviour — see the note at the end of
> [Nesting](#nesting-is-structure-not-a-second-concept) — and it is tracked in the planner.

#### The rule: two cases, and only two

| what you are building | what you write |
|---|---|
| anything at all — a button, an icon, a card, a label | **nothing** |
| an item in a collection you are iterating | **`.key(…)`** — the item's own id, from your data |

```rust
IconButton::new(Glyph::ChevronDown).on_click(move || expand(id))   // nothing

for pane in &column.panes {
    Item::new(&pane.name).key(pane.id)                             // its own id
}
```

That is the whole surface. No role to declare, no region to name, nothing to remember on an ordinary
widget.

**Why the old names went.** `nav_key` and `scope_key` described *how the framework used the string*
rather than what it was, so a developer adding a widget had no reason to guess either existed. A
plugin author should not have to carry a strange, confusing name they will forget to add.

#### `key` is React's `key`, and means the same thing

When you render a collection, each child carries the identity of *the thing it represents*, so the
framework can tell "this row again" from "a different row" after a rebuild. Exactly what is needed
here, for the same reason: a chrome tree is rebuilt for reasons that have nothing to do with
navigation — a pane's git status changing is enough — and a cursor, a letter or a right-click target
that resets every rebuild is not one.

**You never count.** `key` is never a position and never a counter; it comes from the data you are
already iterating. A key like `pane:7` should never mean "count the panes you are adding". If you are reaching for a counter the key is wrong — an index
is exactly the thing that changes when the list changes, which is what identity is for.

**Where it is required:** in a collection, and nowhere else — the same rule React uses, and the same
place a developer already expects to think about it.

#### Nesting is structure, not a second concept

The sidebar is a **tree**: a workspace row holds column rows, which hold pane rows. Every level is
both *a row the cursor stops on* and *a container of the next level*.

```rust
DockFrame::new(&ws.name).key(ws.id)                 // a row, and a container
    .child(MarkerGroup::new().key(col.id)           // a row, and a container
        .child(Item::new(&pane.name).key(pane.id))) // a leaf row
```

One property at every level. Nesting is expressed by the tree, exactly as in React's nested lists.

**This is why `scope_key` was meant to disappear.** It answered *"which region did this press land
in"* — a second declaration for the same point in the tree, and the reason two fields existed. With a
key at every level it looks **derived**: the region is the **nearest keyed ancestor**, declared by
nobody and unable to fall out of step with the row it encloses.

> ⚠️ **It has not disappeared, and the reason is worth keeping.** `mouse.rs` states a deliberate
> rule — *"clicking a container's padding is not a request to move the cursor"* — and merging the
> two fields makes `key_at` answer a padding click with the **container's own** key, moving the
> cursor to something that is not a row. The obvious repair ("a keyed node with keyed children is a
> region") fails too: the sidebar is a tree, so a workspace row contains column rows while being a
> perfectly good cursor target itself. `Base::scope_key` therefore stays for now, with its own task.

> The old `scope_key` documentation argued the two must stay separate, because folding them would
> make a region turn up in `collect_keys` as a steppable row. That held while identity and role
> were the same declaration. Once every node is keyed, "region" is a question you *ask* of the tree
> rather than something a widget asserts.

#### `key` is OPTIONAL — never require one, and never gate on one

**A widget never has to be named.** If you are building something that needs to know *which* widget
— a drag, a picker, a grid's cursor, a menu — take the declared `key` when there is one and the
**derived identity** when there is not. One function, read by every side, so the two can never
disagree (`drag::resolve::drag_identity` is the worked example).

```rust
// ✅ works on any widget, named or not
let id = drag_identity(node);

// ❌ silently does nothing on every widget nobody had reason to name
let Some(id) = node.base().key.clone() else { return };
```

**Two capabilities have shipped broken this way.** `.draggable()` read `key` directly, so dragging
did nothing on unnamed widgets — with no error, and forcing an author to learn an internal rule
before anything worked. A described card grid keyed off its cards' declared keys, so an unnamed card
was never reported.

**The tell:** you are adding `.key("…")` at a call site so that something *else* works, or reading a
key and treating `None` as "nothing to do". Both mean the identity rule belongs one level down, in
the thing that needs it — not in every caller's head.

#### Everything else gets an identity anyway

A widget that declares no `key` still needs one, or a hint letter cannot stay with it between
openings of the picker. It is derived, in three levels — each used only when the one above is
ambiguous:

1. **its name, within the nearest keyed ancestor** — `pane:7 / ×`
2. **name + index among identically-named siblings in that scope** — `topbar / ×[1]`
3. nothing else; that is the floor

The name comes from `Component::text_summary()`, which already computes one *"from the contents, like
the web's accessible-name algorithm"*.

⚠️ **Derive from content, never from position.** `Flex/Row[2]/Button[0]` looks automatic and drifts on
every tree change — which is the bug this exists for: expanding a pane moved a button's hint letter
from `k` to `j`. Content-based identity does not move. The level-2 index counts only
identically-named siblings *in one scope*, so it shifts when a `×` is added beside other `×`s and
never because something changed elsewhere.

**Known limit:** a derived identity changes if the label changes. Fine for a remembered letter; not
fine for anything durable, which is what an explicit `key` is for.

#### Forcing a `key` — a warning, not a type

Considered and rejected: a typestate builder where an item in a collection does not compile without a
key. It puts the rule in the compiler, but it is noise on every widget, and a plugin sending JSON
never meets the Rust compiler. What is built instead, again as React does — three reports, none of
them a failure:

| | walk | what it looks at | who reads it |
|---|---|---|---|
| a **warning** | `nav::ambiguous_identities(&dyn Component)` | a live widget tree | us, while building chrome (debug builds) |
| a **test** | the same walk, over heca's own sidebar | our rows | CI, so our rows stay keyed |
| an **error** | `heca_view::unkeyed_collection_items(&ViewNode)` | a description, before it is realized | a **plugin author** |

**A collection is two or more siblings of the same container**, and each half says that in the
vocabulary it has: over widgets, two or more that derive the **same name**; over a description, two
or more of the same **`WidgetKind`**, which is real type information and needs nothing inferred. So
an `[Icon, Label]` control never trips either one, and three rows always trip both.

**Only what you can act on is reported** — `Base::activatable` natively, a bound `press` in a
description. Identity is what a cursor, a right-click, a drag and a remembered hint letter are kept
*on*, and all four need something to act on: two labels inside one row are that row's **content**,
and the row above them is what carries the key. Without that clause the widget walk reports every
transparent wrapper in the tree, because a [`KeyHint`](#keyhint) around a keyed row inherits the
row's name through `text_summary` while carrying no key of its own.

Both walks are **pure functions returning findings** — the library holds the data, the app owns the
failure behaviour, the same split the [search](#persistence-is-the-hosts) history uses. heca reports
each distinct finding **once**: a chrome tree is rebuilt for reasons that have nothing to do with
identity, and a description is realized again on every theme reload, so a diagnostic repeating with
them is one nobody reads. The description half is therefore checked **at the bridge, per
description** rather than inside `realize`.

Demanding a name from everyone up front is how you get `"btn1"`, which is worse than no name.

#### Naming

**`key`, not `id`.** `id` suggests global uniqueness across a document, the way HTML means it. This is
scoped to its collection — two lists may both have a `key("1")` — which is precisely React's meaning.

**No `Hintable…` prefix on anything.** The identity serves the cursor, the right-click, drag *and* the
picker; naming it after one of four readers would be wrong, and `HintableItem` would suggest a second
kind of `Item` rather than a property of the one that exists.

#### One declaration, four readers

The host derives the keyboard **cursor**, the **right-click target**, the **drag identity** and the
**hint picker's** stable letters from this single string — instead of a closed enum of row kinds that
only the app could extend, which is what made a plugin row impossible to point at.

| Free function | What it answers |
|---|---|
| `collect_keys(&dyn Component) -> Vec<(String, Rectangle)>` | Every keyed item with its laid-out bounds, in **document order** — the order the user sees, which is what "next row" means. Hidden subtrees are skipped, so a collapsed group's rows are not steppable. |
| `key_at(&dyn Component, Point) -> Option<String>` | The **topmost, deepest** item under a point — what a right-click is aimed at. Same walk as `drag::source_at`, deliberately: a right-click and a drag must agree about what they are pointing at. |
| `scope_at(&dyn Component, Point) -> Option<String>` | The **nearest keyed ancestor** of that point — which region the press landed in. |
| `identity_of(&dyn Component, &[usize]) -> Option<String>` | One widget's full identity, keyed or derived — the string a remembered hint letter is filed under. |

**Innermost wins**, the same rule as the deepest row: a keyed node nested inside another resolves to
the inner one, so nesting composes instead of needing a flag. And it is **independent of
consumption** — a press a widget consumes (a scrollbar thumb, a button) still resolves to the region
containing it, because "which panel did the user click in" is not the same question as "did anything
handle the click". That is what makes *click a panel to focus it* work everywhere with nothing
declared per panel.

The string is **opaque to the library** — nothing here parses it — and must survive a tree rebuild.

> **Declarative form:** `key` is an ordinary prop on the node, and `realize` reads it **once for
> every kind** — alongside style and the `hint` event, never in a per-widget arm, because a
> collection can be built from any kind and so the identity of an item belongs to no widget in
> particular. It lands in the same `Base::key` slot a native `.key(..)` writes, which is what makes
> a described row and a native row indistinguishable to the cursor, the right-click, the drag and
> the picker.
>
> ```rust
> for pane in panes {
>     Row::new().key(pane.id).on_press(Intent::new("focus_pane").arg("pane_id", PropValue::Int(pane.id as i64)))
> }
> ```
>
> ```rust
> // …or, without the typed builder:
> ViewNode::new(WidgetKind::Row)
>     .prop("key", PropValue::Text(format!("pane:{}", pane.id)))
>     .on_press(Intent::new("focus_pane"));
> ```

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

**Two rules the layout pass applies for you, so no widget has to.** A child of a container that is
*not* a scroll viewport gets, unless it said otherwise:

- **`max_width: 100%` — nothing is wider than what holds it.** A widget carrying a design width
  (`Alert` 360, `Toast` 320, `Input` 240) is capped against the box it was given instead of painting
  through its parent's border.
- **`min_width: 0` — giving way is not optional once the row is out of room.** Flexbox otherwise
  floors every item at its own content width, so a row whose children *are* willing to shrink still
  cannot fit them and lays the overflow past its own edge — which is how a leading icon, a caret or
  a drag handle pushed a title clean outside its frame at narrow widths.

A widget that must keep its size still says so — an explicit `min_width`, or `flex_shrink(0.0)` —
and both rules leave it alone. Inside a **viewport** (anything that clips its children) neither
applies: there, exceeding the box is the feature, and the floor is what keeps a 600px column 600px
wide in a 100px scroll region.

#### Two things a container publishes to what it holds — content colour, and control tone

A container cannot style its children: they arrive as `impl Component`, so it does not know their
types, and the `Theme` is only reachable at paint. So instead of assigning, it **publishes one
value per frame and the children pull it**. There are two such channels, and they are deliberately
separate:

| | publishes | who pulls it | resolves as |
|---|---|---|---|
| **content colour** | `PaintCx::with_content_color(c, …)` | bare text and glyphs — an unstyled `Label`, an `Icon` | own explicit colour → published colour → a theme token (usually `foreground`) |
| **control tone** | `PaintCx::with_control_tone(c, …)` | **controls**, for their own chrome — a `Button`, an `IconButton` | own `.tone(..)` → published tone → `theme.accent` |

**Why not one channel.** Content colour is what a glyph is *painted in*; a control's tone is the
hue it derives a whole state machine from — its border, its hover sweep, its press flash, its focus
ring. Six widgets already publish a content colour today, and widening that one channel to also
mean "and re-tint every control inside me" would have changed all six at once. A separate channel
is retro-compatible **by construction**: nothing publishes a tone unless it says so.

The case it was built for is a [`Toast`](#toast): a danger card wants its buttons in the danger
hue, without the caller passing a colour to each one and without the card painting their faces
itself. The card publishes its severity; the controls tone themselves; each keeps its own hover,
press and focus ring, because they are real controls rather than something the card drew.

```rust
// A container publishing both: its content reads in `tone`, and controls inside it take it too.
cx.with_control_tone(tone, |cx| {
    cx.with_content_color(tone, |cx| paint_child(&self.base.children[ICON], cx));
    // …a Button in here resolves its chrome hue as: own tone → this → theme.accent
});
```

A control with an intrinsic semantic hue ignores both — a `Destructive` button stays danger-toned
inside a success card, the same way `Badge::danger` keeps its colour inside a tinted parent.

**`Style.visual` — appearance.** `fill`, `border`, `glow`, `radius`, `font_size`, `font_scale`.
A description may **never** set these: it carries semantic intent (a variant, a `size`, a colour
*name*) and the host resolves the pixels from the `Theme`
(`chrome-and-ui.md` §2.6.1 rule C).

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

Token struct consumed by `PaintCx`. The bundled themes are **`grid_tron`** (cyan, dark — the
default), **`mocha`** and **`latte`**, loaded by name with `heca_theme::load_theme(name)`; it falls
back through `~/.config/heca/themes/{name}.toml` → bundled → `grid_tron`, so it never fails.
`Theme::grid_tron()` is the one preset built in code. Tokens: `background`, `surface`,
`foreground`, `muted`, `border`, `accent`, `glow`, `danger`, `success`, `warning`,
`font_family`, `font_size`, `radius`, `border_width`, `focus_border_width`, `focus_ring`,
`hint_color`, `glow_size` (`GlowLevel`), `intensity`, `show_focus_border`, `icon_secondary_alpha`,
`active_wash_alpha`, `card_background_alpha`.

| Token | Type | Drives |
|-------|------|--------|
| `radius` | `f32` | Base corner radius. Boxes use it directly; small controls use `control_radius()` (= `radius × 0.5`); pills (Badge/Toggle/ProgressBar) round at `radius × 2` clamped to their capsule. `0` ⇒ square. |
| `border_width` | `f32` | Decorative border stroke width for every box/pill widget **and** the `Pane`/`bracket_frame` reticle. `0` ⇒ no border anywhere. (App config: global `[appearance] border_width`.) |
| `focus_border_width` | `f32` | Width of the **affordance** outlines — the keyboard focus indicator (`focus_ring`) and selected-item highlight. Independent of `border_width`, so focus/selection stay visible even with borders off. Default `1.5`. (App config: `[appearance] focus_border_width`.) |
| `focus_ring` | `Option<Color>` | Color of the keyboard **focus outline** drawn by `PaintCx::focus_ring` (every widget). Unset ⇒ derived per-tone by `effective_focus_ring()` / `focus_ring_tone()`: the tone (accent, or `danger` for a destructive button) shifted toward `foreground`, which brightens the ring on dark themes and darkens it on light themes so it stays distinct from the widget's own border. Set it to pin the default/accent focus color; the `danger` ring always derives. |
| `hint_color` | `Option<Color>` | Colour of the **picker's letters** — every keycap `prefix+/` and the pane / column / workspace picks stamp over their targets. Unset ⇒ the `accent`, via `effective_hint_color()`. **One colour for the whole app**, like `hint_font_size` is one size: a letter is chrome drawn *over* a target, never part of it, so it must not take the colour of whatever it lands on. A host may still tint one *kind* of target apart — workspace picks use `warning` — with [`ComponentExt::hint_color`](#componentext--what-every-widget-gets), which overrides this per widget. |
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
| What if the focused thing is an **area**, not a control? | Wrap it in [`FocusScope`](#focusscope) and drive it from a host signal — it gates the subtree's keys on that focus as well as drawing the ring. A control owns its focus so it draws its own ring; an area the keyboard is *aimed* at (a sidebar dock the scroll keys act on) has no owner in the tree — only the host knows which subtree holds it. Same outline, same theme tokens. |

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
- **`PointerButton`**: `Left`, `Right`, `Middle`, `Other(u16)` — on every press and release, which
  is what lets a widget answer a right-click itself.
- **`Event`**: see [the event model](#the-event-model--the-framework-resolves-the-pointer-and-walks-the-tree)
  for the whole vocabulary. In short: hosts build `Event::Raw(RawPointer)` and the framework
  delivers the resolved events (`Click`, `RightClick`, `PointerEnter`, `Scroll`, `Drop`, `Focus`,
  `Mount`, …) plus `Key`, `TextInput`, `ModifiersChanged` and `Widget(WidgetIntent)`.
- **`Handled`** `{Yes, No}` — returned by `on_event`/`on_event_capture`; `Yes` stops propagation.
- **`Base::hovered()`** — whether the pointer is over this widget **or a descendant** (the CSS
  rule). Read it instead of testing `bounds.contains(pos)`: the router already resolved which
  widget the pointer is over, including what is on top and what is clipped away.
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
| `.with_control_tone(color, \|cx\| …)` | The **chrome** counterpart of the line above: publish a hue that **controls** inside the closure derive their own chrome from — border, hover sweep, press flash, focus ring. A [`Button`](#button)/[`IconButton`](#iconbutton) resolves own `.tone(..)` → this → `theme.accent`. Separate from content colour on purpose: six widgets publish a content colour already, and widening that one channel to also re-tint every control would have changed all six at once. **Nothing publishes a tone by default**, so it is retro-compatible by construction. See [the two channels](#two-things-a-container-publishes-to-what-it-holds--content-colour-and-control-tone). |
| `.control_tone() -> Option<Color>` | The inherited control tone, if a parent published one. A control with an intrinsic semantic hue (a `Destructive` button) ignores it. |
| `.content_color() -> Option<Color>` | The inherited content color, if a parent published one. Widgets that render bare text/glyphs resolve: **own explicit color → this → a theme token** (usually `foreground`). A widget with an intrinsic semantic color (`Badge::danger`) ignores it. |
| `.with_translate(dx, dy, \|cx\| …)` | Paint the closure's subtree **translated** — the same components, drawn somewhere else. Deliberately narrow: a component is laid out in exactly one place, and its bounds are the contract for drawing *and* hit-testing alike. But a control occasionally has to render content it owns but does not hold — a [`Select`](#select) shows the chosen option in its trigger while that option is away in the open list. Nothing can be in two places, so the trigger draws a second **image** of it. What is drawn this way is **not interactive** (no bounds of its own ⇒ not hit-tested, focusable or hoverable); the control's own bounds are the click target. Never use it to *move* a widget — that is `shift_subtree` + `on_layout`, which keeps bounds honest. |
| `.surface(rect, id)` | Place content **something else rasterised** — a terminal, an image, a video, a plugin's own canvas — in `rect`. The widget says where; the host owns the texture. `id` is opaque here: nothing about textures, formats or devices crosses into this crate. See [`Host`](#host--work-only-the-host-can-do). |
| `.backdrop_blur(rect, radius, alpha)` | **Blur whatever is already drawn behind this widget**, within `rect`. Recorded in scene order, so it blurs what came before it and nothing after. `alpha` fades the blurred copy, so a surface arriving fades its backdrop in with itself. A zero radius or alpha records nothing — a host asked to do no work still pays for a full-screen pass. |

`DrawCommand` variants: `Rect`, `Brackets`, `Text`, `Scanline`, `PushClip`/`PopClip`, `Host`.
`Scene`: `new()`, `push`, `clear`, `len`, `is_empty`, `iter`, plus the overlay layer
(`begin_overlay`/`end_overlay`, `base_layer`/`overlay_layer`).

#### `Host` — work only the host can do

Some content cannot be expressed as rectangles and text, and must not be forced into them. A
**terminal** is rasterised into a texture because its cell glyphs are the hottest path in the app —
drawing them as ordinary commands is rejected. A **frosted backdrop** is the frame so far, blurred,
which is a pass over what is already drawn rather than a shape.

Neither is a special case in this crate. A widget says *what it wants* and where; the host owns the
GPU and does it — the same bargain `Text` already makes, where the scene names a role and the
renderer owns the atlas.

Requests are recorded **in scene order with the clip stack resolved**, so a surface inside a
`ScrollRegion` clips like anything else, and a backdrop blurs exactly what was drawn before it and
nothing after. They carry the paint context's opacity like every other command, so a surface inside a
fading overlay fades with it — and a frost fades in with the surface that asked for it, instead of
holding the session out of focus and snapping sharp in one frame at the end of the fade.

**Native:**

```rust
// Place content something else rasterised. `id` is opaque here — no texture,
// format or device crosses into this crate.
cx.surface(self.base.bounds, self.surface_id);

// Blur whatever is already behind this widget.
cx.backdrop_blur(self.base.bounds, theme.colors.overlay_frost_radius, 1.0);
```

**Declarative:** neither is describable, and deliberately so — a described tree names widgets, and
both of these are a widget's own paint. A plugin rendering its own content gets a surface id from the
host and places it with one builder on its own widget; it never names a `DrawCommand`.

### Asking the host to lay the tree out again — `needs_layout`

**A repaint cannot fix a structural change.** When a widget adds or removes children between
frames — reconciling a host-owned list, revealing a subtree — the widgets *around* it are still
laid out around the shape the tree used to have. Marking it dirty repaints the same wrong
positions.

So a widget says so, and the host runs the pass:

**Showing and hiding is the common case, and it has its own setter.** `hidden` is the engine's
`display: none`, so flipping it moves every sibling — never write `style.layout.hidden` yourself:

```rust
self.base.set_hidden(true);        // sets it AND asks, in one call
```

It asks **only when the value actually changed**, which is what makes it safe to call from
`remeasure` (that runs *inside* the layout pass, so a widget syncing itself every pass cannot
request one every pass). A test fails the build if anything writes the field directly.

```rust
// For a tree that changed some other way — children added or removed:
self.base.children.remove(i);
self.base.mark_needs_layout();

// In the host's frame, BEFORE it decides whether to lay out:
if heca_grid_ui::needs_layout(&root) {
    layout_dirty = true;
}
```

`needs_layout(root)` walks the tree, clears the flags as it reads them, and answers a **bool** —
there is nothing to union, because layout is a whole-tree pass. It is the layout twin of the damage
walk, and a host calls it the same way, once a frame.

**Keep the two apart.** They cost different things: a repaint is per-frame and cheap, a layout pass
re-measures the whole tree. A widget that only changed colour must call `mark_needs_paint` and
nothing else.

> The case that asked for it: a [`ToastStack`](#toaststack) drops a dismissed card only once its
> exit has **played**, which is several frames after the click. Nothing re-laid-out at that moment,
> so the cards below kept their old positions — and the gap where the card had been simply sat
> there until an unrelated click happened to trigger a layout.

### `Flash`

A reusable press effect: `Flash::new()` / `Flash::with_duration(s)`; `.trigger()` on press,
`.tick(dt)` each frame (`true` while fading), `.amount()` (0–1) to paint via `cx.flash`.

### `Attention`

A "needs attention" pulse: `Attention::new()`; `.trigger(pulses)` runs a fixed number of
sawtooth flashes (snap to `1.0`, fade to `0.0`, repeat) then stops; `.tick(dt)` (`true` while
pulsing), `.amount()` (0–1), `.is_active()`. Used by [`Row.attention`](#row) — the widget
flashes; the host plays any **sound** (the library is audio-free).

### Animations — how a surface arrives and leaves

`Flash` and `Attention` are things a widget does to **itself** while it is present. An **animation**
is something done to a whole **surface** as it comes and goes.

**A caller names one. Nobody composes one at a call site.**

```rust
Overlay::new().panel(body).animation(Animation::Fade)
Overlay::new().panel(body).animation(Animation::ZoomFade)           // the exposé's gesture
Overlay::new().panel(body).animation(Animation::Zoom.from(0.8))     // tuned
Overlay::new().panel(body).animation(Animation::of(MyWhirl::new())) // …or one you wrote
```

| | |
|---|---|
| `Animation` | The vocabulary: `None` (the default — a cut) · `Fade` · `Zoom` · `Slide` · `ZoomFade` · `Custom`, built with **`Animation::of(impl Animate)`**. Tuners: `.from(scale)` (how far away it starts — below `1.0` grows in from smaller, above it pulls back from larger) and `.seconds(s)`; a built-in with no such dimension, and a `Custom` one, are returned unchanged. The variants are also the **names a description writes** (`"zoom_fade"`, `"slide"`) — one builder, both authors |
| `Animate` | The **trait**, and the extension point: `enter()` / `leave()` begin an arrival and an exit · `cancel()` settles fully present · `tick(dt) -> bool` advances it · `is_leaving() -> bool` says whether the surface may be taken away yet · `frame() -> AnimationFrame` is what to draw · `duration() -> f32` (default `0.0`) is how long one gesture takes |
| `AnimationFrame` | `{ opacity, scale, offset }` — the whole vocabulary a surface's presentation needs, plus `IDENTITY`, `over(other)` (compose: multiply, and add the offsets) and `apply(cx, origin, f)`, **the one place a frame becomes a picture** |
| `Presence` | Whether a surface is up, plus the **`Option<Box<dyn Animate>>`** carrying it — `enter()` / `leave()` (which own the two rules below), `follow(open)` for a signal-driven surface, `assume_open(open)` (adopt without playing — a surface born open, or one carried across a rebuild), `is_open()`, `is_animated()`, `is_leaving()`, `tick(dt)`, `frame()`. **No animation is an absence, not a null object**: nothing is ever mid-gesture, so nothing waits for it |

The parts behind the names are public too — `Fade` (`new()` both ways, `out()` for a cut in and a
dissolve out, `.seconds`), `Zoom` (`.from`, `.seconds`), `Slide` (`.from(SlideFrom)` for the edge,
`.distance(px)`, `.seconds`), `Sequence::new(lead, follow).lag(share)` and `ZoomFade` — but reach
for them only to build a gesture the vocabulary does not have. `Sequence`
is where "the dissolve **rides** the movement, lagging by a *share* of it" lives: a share, never a
second duration, so tuning the lead keeps the sequencing.

**Two rules you never write twice.** Entering a surface that is already up is not an arrival (a
rebuilt surface must not zoom open again), and entering one that is **leaving** does not resurrect
it (a re-open mid-exit made the map snap back to full opacity and start leaving again). Both live in
`Presence`, so every host — the layer stack, a plugin panel host, a composing widget — gets them for
free.

**Writing one is a single new file.** `heca-grid-ui/src/animation/` is one file per animation
(`fade.rs`, `zoom.rs`, `slide.rs`, `sequence.rs`, `zoom_fade.rs`, and `vocabulary.rs` for the names). A **third party** adds nothing
anywhere: they implement `Animate` in their own crate and pass `Animation::of(..)`. A built-in
shipped *by this library* is a new file plus one arm in `Animation` — the name is the only thing
written down. Either way nothing in the painter, the widgets or any host changes:

```rust
/// A surface that swings in from the left. Nothing else in the library knows this type exists.
impl Animate for SlideIn {
    fn enter(&mut self)  { self.leaving = false; self.left = Some(self.duration); }
    fn leave(&mut self)  { self.leaving = true;  self.left = Some(self.duration); }
    fn cancel(&mut self) { self.leaving = false; self.left = None; }
    fn tick(&mut self, dt: f32) -> bool { /* count down; true while going */ }
    fn is_leaving(&self) -> bool { self.leaving && self.left.is_some() }
    fn frame(&self) -> AnimationFrame { AnimationFrame::offset(self.x(), 0.0) }
    fn duration(&self) -> f32 { self.duration }
}
```

If a new animation ever makes you touch a second file, `AnimationFrame` is missing a channel — widen
it **once**, here, where every surface picks it up at the same time.

**A frame is a paint transform, never a layout number.** The tree is laid out once, at life size;
opacity, scale and offset are applied to the picture on the way out through `PaintCx`. Nothing is
re-measured, no bounds move — so a surface mid-animation is still exactly where its bounds say, and
hit-testing, focus and drag are untouched.

### Search — matching, ranking by use, and query history

`heca_grid_ui::search` — a capability a widget **embeds**, the way anything that needs to scroll
nests a [`ScrollRegion`](#scrollregion). It owns the matcher, the ranking and the history so a
searchable widget owns none of them.

**No filesystem, no clock, and no widget type is named in the module.** That is what keeps it
testable without a window and reusable by whatever searches next.

```rust
use heca_grid_ui::search::{SearchModel, SearchStore};

// The store is the HOST'S: a palette is rebuilt every time it opens, and a memory
// living in the widget would be empty every time.
let store = Rc::new(RefCell::new(SearchStore::new()));
let mut search = SearchModel::new("command", store);   // "command" is the scope
```

| Call | When | What it does |
|------|------|--------------|
| `.rank(&items, query, key)` | filtering | Returns `Vec<Ranked>` — index into `items`, score, and the matched character indices. `key` yields `(Option<&str> id, &str text)`. |
| `.handle(intent, query)` | on `MenuHistoryUp`/`Down` | Walks the history. Returns `SearchAction::SetQuery(..)` to apply, or `Ignored`. **The whole of history navigation** — a widget that walks a history itself has copied this. |
| `.query_changed()` | the user typed | Leaves the history walk; the field is theirs again. |
| `.record_run(query, id)` | something ran | Remembers the query (for recall) and the id (for ranking). Only on a run — an abandoned search is not one anyone wants back. |

**Identity is optional.** `key` returns `Option<&str>`; an item without an id matches and sorts
normally and simply carries no ranking boost, so nothing has to grow an id field to become
searchable.

**Ranking** is the match score plus a frecency boost — use count (which wins from the *second* use)
and recency, capped so a much-used entry never overtakes a clearly better textual match. Recency is
**distance in a use-sequence, never a timestamp**: no clock in a UI library means no skew and tests
that assert exact numbers.

**Matching takes the best alignment, not the first.** A single greedy pass reads the query out of
whatever comes earliest — `left` against `Toggle Left Sidebar` took the `l`/`e` from *Toggle* and
then the `f`/`t` of *Left*, never matching the word, and scored the same as a clean match.

#### Scopes — a second search surface

**A scope is not built, it is named.** It is a plain string you choose — a namespace label for
"whose memory is this" — handed to `SearchModel::new`, created on first use and registered nowhere.
Any string works; the convention is a lowercase singular noun for *what is being searched*:
`"command"`, `"pane"`, `"workspace"`, `"file"`.

It is **not** a sigil and not an icon. VS Code's `>` / `@` / `:` are *query prefixes* — a UI gesture
for switching which list you are looking at — and a scope is the storage key behind one. A surface
could read the prefix and repoint its model at a different scope, but the two are separate ideas and
neither implies the other.

```rust
// Three surfaces that will not tread on each other. The strings are chosen here and
// nowhere else — there is no enum to extend and nothing to register.
let commands = SearchModel::new("command",   state.search_store.clone());
let panes    = SearchModel::new("pane",      state.search_store.clone());
let files    = SearchModel::new("file",      state.search_store.clone());
```

They become keys in the persisted file the first time each one records something:

```json
{
  "version": 2,
  "scopes": {
    "command": {
      "history": [{ "query": "clo",  "at": 1754300000 }],
      "uses":    [{ "id": "close_pane", "count": 7, "last_used_at": 1754300000 }]
    },
    "pane": {
      "history": [{ "query": "nvim", "at": 1754300100 }],
      "uses":    [{ "id": "~/projects/heca:nvim", "count": 3, "last_used_at": 1754300100 }]
    }
  }
}
```

Note the second scope's ids: **an id is whatever you rank by**, and it is persisted, so it has to
mean the same thing tomorrow. An action name does. A pane's numeric id does not — it is unique
within a session and meaningless after a restart, so a ranking keyed on it would accumulate dead
keys and rank nothing. For rows that outlive nothing, rank on a natural key (a path plus a program,
a workspace's name) or pass `None` and take pure text matching.

Worked end to end — a widget that filters a list of its own and remembers what was picked:

```rust
struct PanePicker {
    base: Base,
    panes: Vec<PaneEntry>,      // { id: String, title: String }
    query: Input,
    selected: usize,
    search: SearchModel,        // handed in by the host, over a scope of its own
}

impl PanePicker {
    /// Filtered + ordered. The matcher, the smart-case rule and the ranking by past
    /// picks all live in the model; this reads the answer back.
    fn results(&self) -> Vec<Ranked> {
        self.search.rank(&self.panes, &self.query.value_str(), |p| {
            (Some(p.id.as_str()), p.title.as_str())
        })
    }

    /// Painting a row: the marks come from the match, not from the widget.
    fn row_label(&self, r: &Ranked) -> Label {
        Label::new(self.panes[r.index].title.clone())
            .truncate(Ellipsis::End)
            .marks(r.hits.clone())
    }

    fn run_selected(&mut self) {
        let results = self.results();
        if let Some(r) = results.get(self.selected) {
            let query = self.query.value_str();
            let id = self.panes[r.index].id.clone();
            (self.on_pick)(&id);
            // The only persistence call a consumer makes — and it is not a save.
            self.search.record_run(&query, Some(&id));
        }
    }
}

impl Component for PanePicker {
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        match ev {
            // History: forwarded, never walked here.
            Event::Widget(i @ (WidgetIntent::MenuHistoryUp | WidgetIntent::MenuHistoryDown)) => {
                let q = self.query.value_str();
                if let SearchAction::SetQuery(text) = self.search.handle(*i, &q) {
                    self.query.set_value(&text);
                    self.selected = 0;      // NOT `query_changed` — that ends the walk
                }
                Handled::Yes
            }
            // A real keystroke leaves the walk behind.
            Event::Key { pressed: true, .. } => {
                let handled = dispatch(&mut self.query, ev);
                self.search.query_changed();
                self.selected = 0;
                handled
            }
            _ => Handled::No,
        }
    }
}
```

Nothing here saves anything: the host writes when `SearchStore::revision()` moves.

The two share the store and share nothing else: separate query histories, separate rankings. The
scope name is what the persisted file is keyed by, so a new surface appears there on its first save
and `clear_search_history scope=symbol` addresses exactly it.

Keyed from the first line ever written, deliberately: retrofitting a key into a file that already
exists means migrating it, so `"command"` was never allowed to be implicit even while it was the only
one.

**What does not exist yet:** nothing switches scope at runtime. VS Code-style `>` / `@` / `:` modes
would be one surface changing which scope its model points at as the query prefix changes — the
storage supports it, the UI does not do it.

**The two intents are shared vocabulary**, not one widget's feature:
`WidgetIntent::MenuHistoryUp` / `MenuHistoryDown`, bound in `[keys.widgets]` as
`menu_history_up` / `menu_history_down` (Shift+Arrows and Ctrl+p / Ctrl+n by default). Up is older;
past the newest, the field gets back the draft the walk interrupted.

**Marks:** feed `Ranked::hits` to [`Label::marks`](#label) and the widget draws the highlight — do
not paint it yourself.

#### Persistence is the host's

The library holds the data; the app owns the path, the format and the failure behaviour. In heca
that is `heca/src/search_state.rs` → `~/.local/share/heca/search-history.json`
(`~/Library/Application Support/heca/` on macOS), loaded once at startup.

**A consumer never calls save.** `SearchStore::revision()` bumps on every `record_run`, and the host
calls `search_state::persist_if_changed(state)`, which writes only when it moved. Making every
consumer remember to save would mean the second one silently stops being remembered, with nothing to
report it.

Settings: `search_history` (off ⇒ the file is neither written nor read), `search_history_size`
(queries per scope, 50), `search_usage_size` (ranked entries per scope, 500).

---

## Widgets

Every widget implements `Component` and embeds `Base`, so all support `.disabled(true)`,
`.tab_index(n)`, `.width/.height`, visibility, etc. (via `LayoutExt`). Below, "Builders"
lists widget-specific methods; layout/style builders come from the traits above.

### Flex / Container

Layout-only flexible box — the workhorse for arranging children. **`Container`** and `container()`
are aliases. It's the tool for grouping: nest a `Flex` inside a `Flex` to build any arrangement,
including form fields (see below), so most layouts need no dedicated widget.

⚠️ **A `Flex` is for a list of LIKE things** — a row of buttons, a column of rows — where the
children are interchangeable. The moment the children have **different jobs** (a header and a body;
a toolbar, a list and a status line), it is a [`Grid` with a track template](#grid) instead: see
**⭐ THE RULE** there. A flex stack of unlike parts ends up carrying one tuned number per child, and
a parent that counts its children to tell them apart.

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
- **Sizing** (from `LayoutExt`, shared by every widget): `.width(..)` / `.height(..)`
  (`Auto` / `Px` / `Percent`), `.grow(f32)` (flex-grow, absorb leftover space), `.margin*`.
- **Traits**: `LayoutExt`, `Parent`. **No `StyleExt`, deliberately** — a `Flex` arranges, it does
  not paint, so `.background(..)` on one is a compile error rather than a missing feature. Put the
  colour on a [`Surface`](#surface) and the `Flex` inside it. See
  [Surface or Flex?](#surface-or-flex--decoration-vs-arrangement).

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

**The generic container — the `div`.** A box that both *decorates* and *arranges*: background,
border, glow and radius, plus padding, gap, direction and children. Reach for it whenever something
needs a background or padding around its content.

- **Construct**: `Surface::new()` / `Surface::column()` (a column), `Surface::row()` (a row).
- **Decoration** (`StyleExt`): `.background(Color)`, `.border(Color, width)`, `.radius(px)`,
  `.glow(Color)` / `.glow_with(..)`.
- **Arrangement** (`LayoutExt`, `Parent`): the same `.padding*` / `.pad_*(Spacing)` / `.gap*` /
  `.width` / `.height` / `.grow` / `.child(..)` as [`Flex`](#flex--container).
- **Traits**: `LayoutExt`, `StyleExt`, `Parent`.

#### Surface or Flex? — decoration vs arrangement

They are the same box split by job, and the split is **enforced**, not merely advised: `Flex` has no
`StyleExt`, so `.background(..)` on one does not compile. That error is the rule doing its work —
it is not a missing feature, and the answer is never to add the colour somewhere else.

| you need | reach for |
|---|---|
| arrange children — direction, justify, align, gap | **`Flex`** |
| a background, border, radius or glow behind content | **`Surface`** |
| both | a **`Surface`** with a `Flex` inside it |

The last row is the common shape, and it composes exactly like HTML: the surface is the painted
box, the flex is how its contents line up.

```rust
// A strip with its own background, contents pushed to either end.
Surface::new()
    .background(theme.colors.surface)
    .pad_y(Spacing::Xs)                       // token, not px — see Flex
    .width(Length::Percent(1.0))
    .child(
        Flex::row()
            .justify(Justify::SpaceBetween)
            .align(Align::Center)
            .child(Tag::new("~").segment_text(Glyph::Terminal, "zsh"))
            .child(Flex::row().gap_spacing(Spacing::Xs).child(close_button)),
    );
```

**Do not give the box a height to make it fill a strip.** Let it size to its content and let the
parent give it the space — a height computed from the font is a measurement standing in for "as
tall as what is in me", and anything else placed in the same slot then has to reproduce the same
arithmetic. If something outside needs to know how tall it came out, **measure the laid-out tree**
rather than recomputing it.

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

### Panel

A titled **section** container: an optional heading over body content.

The difference from [`Card`](#card) is presentation, not structure. A `Card` is a card — padded 18,
framed with the theme border and radius, meant to stand apart. A `Panel` is a slice of a region (a
plugin's section of a sidebar), so it is quiet by default: no frame of its own and lighter padding.
Give it a fill or a border through `StyleExt` when it should stand out.

- **Construct**: `Panel::new()` (untitled) or `Panel::titled(title)`.
- **Builders**: `.title(impl Into<String>)` — an empty title hides the header **and its rule**,
  rather than leaving a blank line or a bare rule across the top of the content.
- **Shape**: heading, a [`Separator`](#separator) under it, then the body. The rule is what makes
  the title read as a header band rather than the first line of content; it takes the theme's
  border colour like any other separator.
- **Accessor**: `.title_signal() -> Signal<String>` — retitle a mounted panel with no rebuild.
- **Traits**: `LayoutExt`, `StyleExt`, `Parent`.

```rust
Panel::titled("Containers")
    .child(Label::new("nginx"))
    .child(Label::new("postgres"));
```

**Declarative:**

```rust
ViewNode::new(WidgetKind::Panel)
    .text("Containers")                       // `text` is the heading, as it is for Card/DockFrame
    .child(ViewNode::new(WidgetKind::Label).text("nginx"));
```

The heading is a real `Label` child, created up front and hidden until a title is set — so `title`
works whether it is applied before or after the children, which is what a description needs since
properties are applied after children are attached.

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

> ### ⭐ THE RULE — anything with more than one PART is a `Grid` with a track template
>
> A header and a body. A header, a body and a footer. A toolbar above a list. Two parts or more,
> and the arrangement is a **track template** — `auto` for what sizes itself, `1fr` for what takes
> the rest — never a stack of `Flex` children with `grow` weights and heights tuned by hand, and
> never a parent that **counts its children** to work out which one is which.
>
> ```rust
> Grid::new()
>     .rows([Track::Auto, Track::Fr(1.0), Track::Auto])   // header · body · footer
>     .cell(header,  1, 1, 1, 1)
>     .cell(body,    1, 2, 1, 1)
>     .cell(footer,  1, 3, 1, 1)
> ```
>
> **Why this and not a `Flex`.** The template says the whole arrangement in one line a reader can
> check against the picture. A flex stack says it in as many tuned numbers as there are children,
> spread across the file, and each one is right only for the child count it was written for. Add a
> footer and every other term needs revisiting.
>
> **The counting failure is the one to watch for.** A pane holds `[header, content]` — so the host
> asks *"does this pane have two children?"* to decide whether it has a header. Put two things in
> the body and the first is mistaken for a header. A template has no such question: a part is in the
> row it was placed in, and a missing part is a missing row.
>
> **A part is optional by being absent**, not by a flag: build the template from the parts you have.
>
> Use a [`Flex`](#flex--container) for a *list of like things* — a row of buttons, a column of rows
> — where the children are interchangeable and no one of them has a job the others don't.

- **Construct**: `Grid::new()`.
- **Builders**: `.columns([Track])`, `.rows([Track])` (`Track::{Px(f32), Fr(f32), Auto,
  MinContent, MaxContent}`); `.areas(["a b", "a c"])` named template areas (`.` or `_` = an empty
  cell); `.area(child, "name")` places a child in an area; `.cell(child, col, row, col_span,
  row_span)` explicit 1-based placement. A child placed by neither gets taffy's auto-placement; an
  unknown area name falls back to it too.
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

An embeddable **scroll viewport**: children laid out at their natural size (the layout engine never shrinks them, so they overflow), clipped to the region's own
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
- **Wheel** (built-in): plain wheel scrolls **vertically**, **`Shift`+wheel
  horizontally**, and a trackpad's 2-D delta drives both — the host maps
  modifiers→axis (`Event::Scroll` carries `delta_x`/`delta_y`). ~10% of the
  viewport per notch (viewport-proportional). **The wheel carries its position**,
  so the router delivers it to the region under the cursor and this widget has no
  hover gate of its own to keep in step; it simply declines an axis it cannot
  scroll, and the event carries on outward. **Nested regions compose**: the
  innermost region under the pointer gets it first, and an outer whole-page region
  only scrolls when nothing inside it could.
- **Scrollbar thumbs** (built-in): auto-shown per overflowing axis; **draggable**.
  A theme-**accent** grip that brightens on hover/drag (mirroring `MarkerGroup`'s
  grip bar), in a wider invisible **grab lane** (16px) so the thin 5px thumb is
  easy to click. Radius from `Theme::control_radius()`, color from `theme.accent`
  (nothing hardcoded). Each bar reserves a **gutter equal to the whole grab lane**, and each bar's
  track stops short of the other's gutter so they never overlap in the corner.
  **The hit area never reaches outside the reserved gutter** — it used to reserve 7px while grabbing
  across 16, so 9px of lane sat on the row beside it and one pixel belonged to two widgets. A host
  cannot arbitrate that: a press there was both "grab the thumb" and "start dragging this row", and
  the workaround (let whichever widget consumes the press win) stopped rows being draggable at all.
  If you add a hit area wider than what your widget drew, widen the reservation with it.
- **The bar takes layout space, it is not drawn over content.** When an axis overflows, the region
  reserves the bar's lane as padding on that side, so a child is laid out **beside** the bar and
  keeps its rounded corner. Clipping alone was not enough and looked wrong: a card laid out full
  width and trimmed at the lane ends on a hard vertical cut, because it still *is* wider than the
  space it has. The reservation takes `max(existing padding, gutter)` rather than the sum — where the
  padding is already roomy the bar simply sits in it and both sides stay even. It is applied after
  layout and lands on the next pass, like a classic scrollbar, and it cannot oscillate: narrowing
  content only ever makes it taller. `SCROLLBAR_W` is the visible thickness — turn it to make the bar
  look thinner; `THUMB_HIT_W` is the grab target **and** the space reserved, so it is the one to turn
  if the bar claims too much room. The clearance between bar and content is what is left over (11px).
- **The lane belongs to the scrollbar.** A move over it is consumed, so the row *behind* the bar does
  not light up as hovered. The whole lane, not just the thumb — a press in the track pages, so the
  track is part of the control, not content.
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
  `scroll_to`/`scroll_by`/`ensure_visible`.
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

**Keyboard scrolling — semantic, not keys.** `ScrollRegion` acts on eight
`WidgetIntent` variants: `ScrollPageUp` / `ScrollPageDown` / `ScrollToTop` /
`ScrollToBottom` and the four horizontal ones. The host says *one page on*; the
region decides what a page is (0.9 of its viewport, so the line you were reading
stays on screen) and where the end is, because it is the only thing that knows its
viewport and its content. Delivering `PageUp` as a *key* instead would need
`GridKey` variants and would put paging arithmetic in the app.

A region **declines** (`Handled::No`) any axis it cannot scroll — the axis is off,
or the content fits — so a nested region or the host still gets a turn. Consuming
instead would make the key do nothing at all, silently. Handled after the children,
like the wheel, so the innermost scrollable region wins.

`keyboard_target(Signal<bool>)` says whether *this* region is the keyboard's
target. Unset means yes, so a single-region app needs no wiring; a host with
several regions in one tree binds it on each and depends on no default. That is how
"scroll the focused surface" works without the host knowing where any region sits
in the tree.

**In heca that signal *is* chrome keyboard focus.** Every mounted container binds it to
"am I the focused dock" (`StateView::container_keyboard_target`, keyed by mount id), and
the same signal drives the [`FocusScope`](#focusscope) the host wraps the container in — so
what the ring shows and what the scroll keys reach cannot disagree. `focus_dock` moves it.

### Scrolling is composed — nest a scroll area where the scrolling belongs

A scroll area **is a container**. Nest one wherever content should scroll, including
inside another container. Nothing hands the capability down and no layer owns it:

- A container that needs its own scrolling nests its own `ScrollRegion`, with its
  own offset. Two containers in one sidebar scroll independently because each has
  one — not because anything relocated a shared viewport.
- A shell that holds containers **does not** wrap them in a viewport. A viewport
  measures its content at natural height — that is the point of one — so wrapping
  the stack leaves the containers content-sized, and a fractional share then has no
  height to divide. The shell's job is to give containers bounds.
- Nesting resolves which one acts: the innermost that *can* move on that axis, for
  the wheel and the keyboard alike.

**An offset belongs to whatever nests the scroll area, per placement.** Keyed by the
container's mount id, so the same container placed twice keeps two positions while
both show the same content — the split a component reused twice has. Keying an
offset by container *kind* makes two placements scroll together, which is invisible
until there are two.

### A container's share of its region

A container declares its share as a **flex grow factor**, defaulting to `1.0`:

| Declared | Result |
|---|---|
| nothing | an equal share — alone it takes the whole region, two take half each |
| `2.0` beside `1.0` | two thirds |
| `0.0` | content-sized: no share of the leftover |

**Fractional, never fixed.** A share survives a window resize; a pixel height does
not. It is `flex_grow` because that is exactly what it is — the widget library has
had it all along, so there is no second sizing language to learn.

**Share of the region's MAIN AXIS, not of its height.** `flex_grow` is main-axis
relative, so one number is the height in a sidebar (a column) and the width in a bar
(a row), with nothing to add when bar regions arrive.

The *region* applies the share, not the container: a share only means something
relative to siblings, which a container cannot see and should not have to.

> **Implementation note, and a debt.** `flex_grow` distributes only *positive* free
> space, and container content is routinely taller than a sidebar — measured, two
> containers took **1214px each inside a 600px body** and divided nothing. A share
> therefore also needs a **zero base size and permission to shrink** (CSS
> `flex: 1 1 0`); then the free space is the whole region, and the two measure 296px
> each. `Layout` has no `flex_basis`, so that zero is written as a height today.
> **Do not copy that into new code** — expressing a proportion by writing a fixed
> measure is wrong. Giving the library one `share(n)` setter with the trio behind it
> is tracked in the planner.

### Using one — the whole surface

**A caller implements nothing.** The wheel, the thumb drag, the click in the track and the
click-and-hold repeat are all inside the widget. Mount it, give it a size, put children in it:

```rust
let files = ScrollRegion::new()
    .both()
    .height(Length::Px(240.0))
    .child(rows);
```

That is a working scroll area. Everything below is optional.

**To watch it**, attach any of three listeners. They only report; none of them makes it scroll:

```rust
ScrollRegion::new()
    .both()
    .height(Length::Px(240.0))
    .on_scroll_start(|_| status.set("scrolling…"))
    .on_scroll(|s| gutter.set(s.offset_y / s.max_y))     // every movement — keep it cheap
    .on_scroll_end(|s| {
        // `event` is None when OUR OWN `scroll_to` moved it. Reacting to that is how a
        // "follow the cursor" call ends up fighting the user's wheel.
        if s.event.is_some() && s.offset_y == s.max_y {
            load_more_rows();
        }
    })
    .child(rows);
```

`ScrollInfo` answers everything without asking the region back:

| Field | Is |
|---|---|
| `offset_x` / `offset_y` | where it is, in content px (`scrollLeft` / `scrollTop`) |
| `max_x` / `max_y` | as far as it goes — `offset_y == max_y` is "at the bottom" |
| `content` | the full size being scrolled (`scrollWidth` / `scrollHeight`) |
| `viewport` | the visible window (`clientWidth` / `clientHeight`) |
| `event` | the wheel or press that moved it — **`None` when the host's own `scroll_to` did** |

**When each fires:** `scroll_start` on the first movement, `scroll` on every one, `scroll_end` on the
release that ends a drag or a held track press. A wheel gesture has no release — nothing tells you
the user stopped turning it — so its end is a short pause, the same way browsers settle `scrollend`.

**The host's side is one line**, and it is the same line for every widget: deliver the pointer
events it receives. A container never forwards anything by hand — `dispatch` walks the tree — and a
host that hands over a subset is caught by `tests/pointer_delivery.rs` and `heca/tests/pointer_funnel.rs`
rather than by someone eventually noticing a scroll area that doesn't scroll.

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
with its children attached, and **`axes` is a prop** — a described region can be
horizontal or two-axis, not just vertical:

```rust
ViewNode::new(WidgetKind::Scroll)
    .prop("axes", ViewScrollAxes::Both.into())   // Vertical (default) | Horizontal | Both
    .prop("width", PropValue::Int(300))
    .prop("height", PropValue::Int(180))
    .child(wide_and_tall_content);
```

The axis name is `ScrollAxes`' own variant, snake_cased, so adding a variant
extends the accepted vocabulary with no list to update; an **unknown name keeps
the widget's default** (vertical) rather than failing. `.horizontal()` /
`.both()` stay host-only — they carry no value, and `axes` is the property form
of the same setting. **Styling is still host-side**: the app builds the framed
region and mounts a realized subtree inside it.

> **Real-app integration (a mounted container):** selection is container-owned, not widget
> state. Mount the container's rows (DockFrames + rows) inside a `ScrollRegion`; the
> component's own cursor action (`workspaces.cursor_up`/`cursor_down`, selection-driven) calls
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

**Sizing: it hugs its content, and once it is down to its icon it refuses to give way** (CSS
`flex-shrink: 0`). A row shares a shortfall among whatever will take it, and a button carrying words
can take some — its `Label` ellipses. One showing only an icon has nothing left to give, so
shrinking it just eats the control: the box narrows around a glyph that does not, leaving a sliver
too thin to click. You get this wherever the button is put, including a plain `Flex` — the author
does not have to know to ask. A container may hold a *worded* button rigid for its own reasons
(`ButtonGroup` does), and the button never writes that declaration back.

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
    .child(ViewNode::new(WidgetKind::VStack).prop("gap", PropValue::Int(4))
        .child(ViewNode::new(WidgetKind::HStack)
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
  `.content(..)` takes a widget **or** a subtree from a mapper — what `realize(&ViewNode)` returns.
- **Look builders**: `.variant(ButtonVariant)` · `.size(WidgetSize)` (`Small`/`Normal`/`Large`/`Header`
  — scales font **and** padding, and **cascades into the content**) · `.font_size(f32)` (pin an
  explicit size) · `.glow(bool)` (hover glow, default on) · `.bordered(bool)` (default on).
- **Behavior builders**: `.on_click(impl Fn() + 'static)`, plus the shared `LayoutExt`
  (`.disabled(bool)`, `.tab_index(i32)`, `.width/.height`, …). **It is pickable by `prefix+/` with
  nothing written** — a click handler is what makes it so; use `.hintable(false)` to keep it out, or
  `.on_hint(…)` when a pick should do something other than the click.
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
    .child(ViewNode::new(WidgetKind::VStack).prop("gap", PropValue::Int(4))
        .child(ViewNode::new(WidgetKind::HStack)
            .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("trash".into())))
            .child(ViewNode::new(WidgetKind::Label).text("Delete")))
        .child(ViewNode::new(WidgetKind::Label).text("Ctrl+D")));
```

Both spellings produce the **same retained tree** — see
[the declarative UI model](#declarative-ui-model-viewnode).

### ButtonGroup

**A row of related actions that fits the space it is given.**

A toolbar is not a `Flex` of buttons, because a `Flex` has no answer for the moment the room runs
out. Left alone, a row of icon buttons is **squashed to slivers** — the layout makes every child
willing to give way once its row is short, which is what stops a long title shoving a caret outside
its frame. Told not to shrink, the same row **overflows its container** instead. Neither is a design;
both are the layout doing exactly what it was asked. `ButtonGroup` owns that question.

As the space narrows it gives things up in the order that costs least:

| stage | what goes | what stays |
| --- | --- | --- |
| 1 | the **words** | the icons — and the words become what the button says on hover |
| 2 | the **buttons that no longer fit** | a single trailing `⋮`, whose menu reads their words again |

Nothing is ever squashed, and nothing is ever silently unreachable.

```rust
ButtonGroup::new()
    .size(WidgetSize::Header)                       // one size for every button in the group
    .variant(ButtonVariant::Ghost)                  // …and one variant
    .gap_spacing(Spacing::Xs)                       // a token, never a pixel count
    .child(Button::new("Split").icon(Glyph::Plus).on_click(split))
    .child(Button::new("Zoom").icon(Glyph::FrameCorners).on_click(zoom))
    .child(Button::new("Close").icon(Glyph::Minus).on_click(close))
```

#### Its children are `Button`s, and that is the point

Typed to `Button` deliberately — the same way [`Select`](#select) types its options to
[`Choice`](#choice). **Every `Button` constructor takes its text**, so a button in a group cannot be
built without words. That is what makes a collapsed row readable, with nothing required of the
author and no runtime check anyone can forget.

The alternative was forcing text with a typestate builder, which this library
[considered and rejected](#forcing-a-key--a-warning-not-a-type) for `key`: noise on every widget, and
meaningless to a plugin sending JSON. The type does it instead.

#### A collapsed button runs its own click

There is no handler on the group. Each button keeps its `on_click`, and a menu row runs **that same
button** through [`Component::activate`] — the one entry every way of pressing a button already goes
through (pointer, keyboard, a caller invoking it). So the visible button and the collapsed row do
not merely agree: they are the same handler, and cannot drift.

#### Builders

| builder | what it does |
| --- | --- |
| `.child(Button)` | add an action. Its text is its menu label and its hover words; its icon is what it shows once there is no room for words. |
| `.display(Display)` | `IconOnly` (default) — always icons, words kept for hover and the menu · `Full` — always words · `Auto` — words while they fit. ⚠️ **`Auto` is not settled**: taking the words off makes the row narrower, so it then fits, which is the condition for putting them back; at some widths it still alternates. Use `IconOnly` or `Full`. |
| `.variant(ButtonVariant)` | the variant the group's buttons take. **A button that named its own keeps it** — which is what lets a toolbar be uniformly quiet while its close button still reads as destructive, without either fact being written twice. |
| `.size(WidgetSize)` | from [`LayoutExt`](#builder-traits), and it cascades: children inherit their parent's size variant. |
| `.gap_spacing(Spacing)` / `.gap(px)` | from `LayoutExt`, applied to the row inside. **Prefer the token.** |
| `.shown_count()` / `.is_collapsed()` | what the group decided, for a caller that needs to know. |

> ⚠️ **`display` governs stages 1 and 2 only.** Collapsing into the menu still happens whenever the
> buttons genuinely do not fit, whichever display is pinned — otherwise pinning `Full` would bring
> back the squashing this widget exists to end.

#### How it decides — it reads the layout, it does not measure

**Nothing here measures a button or works out a budget.** The group takes the room that is left and
lays its buttons out at their own size, aligned to the **end** of it. Anything too wide for that room
is placed *outside* the box — off the left, exactly as an end-aligned row overflows in a browser —
and the group counts those and drops the same number from the trailing end. All of it is read off the
finished layout, the way the pane header reads its own height.

Two things make it stable:

1. **It grows into the room it is given.** A group that hugs its content is as wide as whatever it
   decided to show, so asking it how much room there is returns the answer it just produced — hide a
   button and the room shrinks, which is the reading that hid it. It costs nothing visually, because
   the buttons sit at the end of that room.
2. **The buttons are built in the mode they will be shown in.** Otherwise the first layout is of
   buttons with their words whatever the mode says, and the first decision is made from an
   arrangement that was never going to be drawn.

#### The ⋮ is one of the row's own buttons

It is a `Button` like the rest, not an icon button — a different control has different padding and a
different height, and a group's own affordance has to be one of the things the group arranges.

**It declares nothing about picking, and does not need to.** It is a button, so it wears a `prefix+/`
letter for that reason alone, and picking it runs its click. Where it sits does not come into it.

That was not always true. The picker used to switch letters off for anything inside something that
had declared a pick of its own — so every button in a pane's bar repeated its own click as a hint to
win its letter back, and this one control, which the widget builds for itself, had no author to do
that for it and silently wore none. See [the picker's rule](#which-widgets-get-a-letter).

#### Composition, and where it sits in a header

It **arranges with a `Flex`**, like anything else would — the group decides *what* is in the row and
`Flex` decides where those things sit. A widget that sets direction, align and justify on its own
base has quietly re-implemented a row, and then owns every question a row already answers.

**It hugs its buttons and gives way when the row is short** — it does not fill the space it is
offered. That is what lets it be one end of a header:

```rust
Flex::row().justify(Justify::SpaceBetween).align(Align::Center)
    .child(title)
    .child(ButtonGroup::new().display(Display::IconOnly) /* … */)
```

A group that filled the row would leave `SpaceBetween` nothing to distribute, and the title and the
actions would sit side by side at the left. Hugging while remaining **shrinkable** is also what makes
the decision well founded: a flex item that may shrink is laid out at `min(its content, the room
there is)`, so when the buttons do not fit, the group's own width *is* the room available.

#### Its first caller — the pane header

heca's in-pane header is a `ButtonGroup`. It replaced a hand-built row of icon buttons plus a second
throwaway layout pass whose only job was to measure that row, so the title's width budget could be
guessed from a character count and two font multiples — with the per-pane render clip named in the
code as the backstop for when the guess was wrong. The bar is a child of its pane now, so that clip
is gone and nothing was catching it. The row divides the space instead.

#### Declarative (`ViewNode`)

`WidgetKind::ButtonGroup`. `display` and `variant` are ordinary props; `child` is **host-only**,
because a description adds actions through `children` like every other container.

```rust
use heca_view::build::*;
use heca_view::{Intent, ViewDisplay, ViewGlyph};

ButtonGroup::new()
    .display(ViewDisplay::IconOnly)
    .child(
        Button::new()
            .text("Close")                      // its menu row, and its words on hover
            .icon(ViewGlyph::Close)             // what it shows once there is no room for words
            .on_press(Intent::new("docker.stop").arg("id", id)),
    )
    .child(
        Button::new()
            .text("Split")
            .icon(ViewGlyph::SquareSplitHorizontal)
            .on_press(Intent::new("pane.split").arg("id", id)),
    )
```

| prop | type | default | what it does |
| --- | --- | --- | --- |
| `display` | `"auto"` \| `"icon_only"` \| `"full"` | `"icon_only"` | how wide each action is before the row starts collapsing. `"auto"` is **not settled** — see the warning above. |
| `variant` | the shared variant vocabulary | *(each button's own)* | the look every action takes, said once. A button that named its own variant keeps it. |

**Give every action both `text` and `icon`.** The group reads a label, a glyph and a click out of
each child to build the ⋮ menu, so an action missing its text produces a blank menu row that does
nothing. **Children that are not buttons are skipped**, not wrapped: there is nothing in a `Label`
for the group to read.

The actions a described group holds are built by the same path a standalone described button is, so
a grouped action is a real button — press intent, composed children and all. Each is pickable by
`prefix+/` in its own right, and so is the ⋮, which is how a collapsed action stays reachable from
the keyboard.

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

**From a plugin (`ViewNode`).** Declare an input in a modal / panel body; the host `realize`s it to
this widget and owns styling + the whole keyboard model above — a plugin never sends an `Edit*`
intent itself, it declares the field and the host resolves the shortcuts. Supported props / events:

| Prop / event | Meaning |
|---|---|
| `.text(s)` (`"text"` prop) | initial value |
| `.prop("placeholder", PropValue::Text("filter…".into()))` | the placeholder shown while the field is empty and unfocused. **Its own key** — `text` is the *value*, so the two are never confused |
| `.prop("name", PropValue::Text("field".into()))` | opts the field into **form submission** — its live value is returned in `ModalResult::Action.data["field"]` when the overlay is submitted |
| `.on("change", Intent)` | intent dispatched (with the new text) on every edit |

```rust
// A rename field inside a modal body — pre-filled + submitted under "name".
ViewNode::new(WidgetKind::Input)
    .text(current_name)
    .prop("name", PropValue::Text("name".into()))
    .on("change", Intent::new("plugin.rename.changed"));

// An empty filter field that says what to type.
ViewNode::new(WidgetKind::Input)
    .prop("placeholder", PropValue::Text("filter containers…".into()));
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
- **Accessors**: `.state() -> Signal<usize>`, `.index() -> usize`, `.selected_label() -> String` (the selected tab's [text summary](#component-trait)).
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
  `Event::Widget` intents — shared with [`ContextMenu`](#menus--menuitem-menu-contextmenu) / [`CommandPalette`](#commandpalette):
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
  `.attention(Signal<bool>)` + `.attention_color(Color)`,
  `.reveal_when(Signal<bool>)` (gate this row's request to be scrolled into view — see
  [Following the cursor](#following-the-cursor); only needed when the cursor also follows the
  mouse), plus `StyleExt` for a persistent background under the selection overlay.
- **Accessors**: `.state() -> Signal<bool>` (active), `.nav_state() -> Signal<bool>` (nav
  cursor) — bind either so the host flips it in place without a rebuild.

> **`active` and `nav_selected` are two different questions, and only one of them follows focus.**
> `active` is "this row *is* the focused thing"; `nav_selected` is "this is where the container's
> cursor is", which the user moves with the keyboard or a click and which is theirs to keep. In
> `heca` the cursor is pulled to the newly active row **only when the active one actually changes**
> (`cursor_follow`, `heca/src/app/focus.rs`). The rule is easy to get wrong in the other direction:
> the sync that writes it runs after *every* layout change, so writing it unconditionally moved the
> user's cursor on changes that touched no focus at all — renaming a row that was not the active one
> was the report that found it. If you add a "the cursor should follow X" rule, guard it on X having
> changed, and put it beside that one.
- **Attention**: when the host sets the bound `attention` signal `true`, the row flashes a few
  times (see [`Attention`](#attention)) and consumes the signal. The host plays any **sound** —
  the library is audio-free.

**Declarative:**

```rust
ViewNode::new(WidgetKind::Row)
    .prop("active", PropValue::Bool(selected))
    .on_press(Intent::new("docker.select").arg("id", PropValue::Text(id)))
    .child(ViewNode::new(WidgetKind::Label).text(name))
    .child(ViewNode::new(WidgetKind::Badge).text(status));
```

Props `realize` reads: `active`, `nav_selected`, `marker`. Events: `press` — wired to a click, to
Enter/Space, **and** to a KeyHint target, so `prefix+/` reaches the row like any other actionable
node. With no `press` intent the row is deliberately inert: not focusable, no hover — a described
row that nothing can activate should not look like a control.

> **`Row` is not `HStack`.** The plain horizontal and vertical boxes are `HStack` / `VStack`;
> `Row` means the same thing in the model as it does in `heca-grid-ui` — the selectable row.

> **A native row's click is a NAME too.** `.on_activate` takes a closure, so it is
> tempting for host code to write one that does the thing directly — and then that gesture is
> reachable from the click and from nowhere else: not the `prefix+/` picker, not a menu entry, not a
> keybinding, not RPC, and never a plugin. **A component declares its rows' gestures as `Intent`s,
> exactly as a described node does**, and `heca`'s `chrome::fires` turns one into the closure a
> builder wants — the mirror of `realize`'s `press_intent` / `hint_intent`:
>
> ```rust
> // declarative (heca-view-realize)              native (heca/src/chrome)
> node.on_press(intent)                           row.on_activate(fires(intent, emit))
> node.on_hint(intent)                            row.on_hint(picks(mount, intent, emit))
> ```
>
> **The click and the pick are two declarations, not one**. A click on a sidebar
> row means *go there and leave*; a `prefix+/` pick means *look at that one* and stays in the dock.
> Serving both from one intent is what made the picker walk out of the sidebar. A described node
> that binds only `press` still gets a pick for free — `hint` falls back to it.
>
> **Each item kind declares its own**, and nothing is inherited or forced: in the workspaces
> component a pane row declares `focus_pane { pane_id }`, a workspace row `focus_workspace { ws_idx }`,
> and a column row declares none — a real answer, not a gap. Both of those **bind actions that
> already exist** rather than inventing new ones: apply the ownership test (*remove the component;
> does the action still make sense?*) before declaring a new id for a click.

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
- **Builders**: `.status(Signal<DotStatus>)` — drive it from state the host already keeps.
- **Change it**: `dot.set(DotStatus::Warning)` — the pip changes in place, nothing rebuilt.
- **Signals**: `.status_signal()` — the same value `set` writes, for a host that binds rather than calls.

```rust
Flex::row().gap(8.0).align(Align::Center)
    .child(StatusDot::online()).child(Label::new("UPLINK"));
```

**One dot that changes, not one dot per state.** A retained tree — the sidebar's pane rows — writes
the status into the signal when a process changes. Building one dot per state and revealing one of
them costs a slot four times too wide and four signals to keep in step, and it only ever looked
right because the hidden ones were being squeezed to nothing by a row out of room.

```rust
let dot = StatusDot::new(DotStatus::Idle);
dot.set(DotStatus::Warning);               // say it directly…
dot.set(DotStatus::Error);

let status = dot.status_signal();          // …or bind the signal the host already keeps
status.set(DotStatus::Online);
```

**It never gives way.** Everything shrinks by default (see the two layout rules above), which is
right for text and for a card carrying a design width and wrong for a circle: squeezing one axis of
a dot flattens it rather than making it smaller. So it declares `flex_shrink(0.0)` for itself, and a
transparent wrapper around it inherits that refusal.

### Separator

Thin divider line (display-only), 1px in the theme's border colour. Spans its container — it asks
to be stretched itself (`align_self`), so it spans whether or not the container stretches its
children. A rule dropped into a centring row used to lay out one pixel by zero and simply not
appear; nothing has to be passed to avoid that.

- **Construct**: `Separator::horizontal()`, `Separator::vertical()`.
- **Builders**: `.orientation(Orientation)` (`Horizontal` \| `Vertical`, default horizontal),
  `.length(f32)` — cut the line shorter than the container. An explicit length is a definite size,
  so the container's own alignment then places it.

```rust
Separator::horizontal();                // spans the column's width
Separator::vertical();                  // spans the row's height
Separator::vertical().length(24.0);     // a short rule, placed by the container
```

**Declarative:**

```rust
// A rule between a table header and its body.
ViewNode::new(WidgetKind::Separator);

// A vertical rule of a fixed length. Either property may be set first: the widget recomputes
// both axes from the pair, so `length` never lands on the axis the rule runs across.
ViewNode::new(WidgetKind::Separator)
    .prop("orientation", ViewOrientation::Vertical.into())
    .prop("length", PropValue::Float(24.0));
```

Props `realize` reads: `orientation`, `length`. Both come from the widget's own builders through the
generated surface — there is no list of names in `realize`.

> A separator is **themed**: its colour and thickness come from the `Theme`. Faking one with a thin
> sized `Surface` hardcodes both and stops following a theme reload, which is why this is a widget
> and not something to compose.

### Spinner

Indeterminate loading ring (dots with a rotating brightness sweep). Animated — return its
`tick` to keep requesting frames.

*Something is happening and nobody knows for how long.* The moment you can say **how far along**,
reach for [`ProgressBar`](#progressbar) instead — a spinner is what you show when you cannot.

- **Construct**: `Spinner::new()`.
- **Builders**: none of its own. Its diameter is `width`/`height` like any other widget's
  (default 28px square), and it animates itself off the frame clock.

**Native:**

```rust
Spinner::new()                              // the default 28px ring
Spinner::new().width(Length::Px(16.0)).height(Length::Px(16.0))   // a smaller one
```

**Declarative (`ViewNode`)** — `WidgetKind::Spinner`, and it reads no props of its own:

```rust
use heca_view::build::*;

Spinner::new()                              // the default ring
Spinner::new().width(16.0).height(16.0)     // sized like anything else
```

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
- **Look**: `.severity(ToastSeverity)` (default `Info` — sets the hue *and* the default leading
  glyph), `.icon(Glyph)` to override that glyph / `.no_icon()` to drop it, `.dismissible(bool)`
  (default `true` — the × affordance).
- **Content is slots, and each takes any component**:
  - `.body(..)` — the column under the title. Takes a widget **or** an already-realized subtree,
    through the one builder. `.body_text(text)` is **sugar** that builds the small ellipsised
    `Label` you would have built — one code path, not two.
  - `.action(..)` — **repeatable**; call it again for a second action and they sit in a row that
    **wraps** when the card is too narrow. Takes a widget or a realized subtree alike. The caller
    says *what* the action is; the card says where it sits and what hue it takes.
- **Placement**: `.position(ToastPosition)` — `TopRight`/`TopLeft`/`TopCenter`/`BottomRight`/
  `BottomLeft`/`BottomCenter`, resolved to **auto margins**, never pixels, so it lands correctly in
  a container of any size. Unset, it sits wherever its parent puts it. The same vocabulary places a
  [`ToastStack`](#toaststack).
- **How it arrives and leaves**: `.animation(Animation)`, `.opened(bool)`, and the verbs
  `open()` / `hide()` / `toggle()`. **A card slides in by default** — that is what a notification
  does, it arrives from the edge it lives on rather than materialising in place — and any other
  gesture is one builder away (`Animation::Fade`, `Animation::of(mine)`, `Animation::None` for a
  cut). A card stays laid out while it *leaves*, which is what the exit plays over; once it has
  gone it takes no space at all.
- **Callbacks** (the host removes the toast / runs the effect): `.on_dismiss(f)` — the ×;
  `.on_click(f)` — the whole card, which also makes it focusable so Enter/Space activate it. There
  is **no `on_action`**: an action is a real control you passed in, so its own `on_click` is its
  callback.
- **Accessors**: `.title_signal() -> Signal<String>`, `.open_signal() -> Signal<bool>`,
  `.is_showing() -> bool` (true **including while leaving**, which is when it is still drawn and no
  longer interactive).
> The title child owns the text, so setting `title_signal` retitles a live card with no rebuild.

**Its content is children, and the engine places them.** The card paints only its own chrome — the
tinted surface, the bracket frame, the action's face, the press flash, the focus ring — while the
leading icon, the text column (title / body / action) and the × are real components laid out by the
layout engine. It measured and placed them itself until F003/P082/T481, and a card squeezed
narrower than its own icon column then laid its title out past its right edge, because a constant
column cannot consult the width the card was actually given. What follows from that:

- **Narrow it and the text is cut, not moved.** The title and body carry an end ellipsis.
- **The action button hugs its label**; the title and body fill the column.
- **The severity tone is published, not painted on**: the icon and the title inherit it (the same
  mechanism `Item` uses for its row colour), so anything composed into the card follows it. The
  body line is the theme `muted` token, and the × is `muted` at rest, `foreground` under the pointer.
- **The action and the × take their own press** and stop it there; `on_click` fires for a press they
  declined, which is what "the whole card" means.

**Native:**

```rust
// Sugar — one line of body text, one action.
Toast::danger("Connection lost")
    .body_text("Reconnecting to the grid…")
    .action(Button::outline("Retry").on_click(|| retry()))
    .on_dismiss(|| dismiss(id));

// Composed — the body is anything, and actions repeat. Neither Button carries a colour:
// the card publishes its severity as a control tone and they take it.
Toast::danger("Build failed")
    .body(Flex::column().gap_spacing(Spacing::Xs)
        .child(Label::new("3 errors in heca-grid-ui"))
        .child(Label::new("cargo check exited 1").font_scale(0.85)))
    .action(Button::new("Retry").on_click(|| rebuild()))
    .action(Button::ghost("View log").on_click(|| open_log()))
    .animation(Animation::Fade)        // it slides in unless you say otherwise
    .on_dismiss(|| dismiss(id));
```

**Declarative** (`WidgetKind::Toast`):

```rust
ViewNode::new(WidgetKind::Toast)
    .text("Build failed")                                         // the title
    .prop("severity", ViewSeverity::Danger.into())           // Info | Success | Warning | Danger
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
  when `action_text` is set **and** no `actions` child was given).
- **Slots**: **`body`** — the **default** slot, so an unslotted child is the body — and
  **`actions`**, one control per child. A described action is an ordinary described
  [`Button`](#button) carrying its own `press` intent, which is what makes it a `prefix+/` target
  with nothing hint-related written: being pickable is not opt-in. An unknown slot name is
  debug-logged and falls back to the body, never an error.
- **Precedence: children win** over `body_text` / `action_text`, the way a `Button`'s children win
  over its `text`/`icon`. One content model, two spellings — the text props remain because they are
  the plain-data path a [`ToastSpec`](#toaststack) uses.

> The catalog said **"no slots, deliberately"** until F003/P096/T488. The reason was the widget's
> own limitation — it hand-drew its card and could not hold arbitrary content — and that limitation
> is gone. Do not restore the old rule.

**Declarative, composed** — a rich body and two actions, each firing its own intent:

```rust
ViewNode::new(WidgetKind::Toast)
    .text("Build failed")
    .prop("severity", ViewSeverity::Danger.into())
    // No slot named: the body is the DEFAULT slot.
    .child(ViewNode::new(WidgetKind::VStack)
        .child(ViewNode::new(WidgetKind::Label).text("3 errors in heca-grid-ui"))
        .child(ViewNode::new(WidgetKind::Label).text("cargo check exited 1")))
    .child(ViewNode::new(WidgetKind::Button)
        .text("Retry")
        .prop("slot", PropValue::Text("actions".into()))
        .on_press(Intent::new("rebuild")))
    .child(ViewNode::new(WidgetKind::Button)
        .text("View log")
        .prop("variant", PropValue::Variant(ViewVariant::Ghost))
        .prop("slot", PropValue::Text("actions".into()))
        .on_press(Intent::new("open_log")));
```

> **A declarative `Toast` is for INLINE use** — a notification row inside a panel. It is **not** how
> you fire an app notification: the host owns the queue and lifecycle through
> [`ToastStack`](#toaststack) + `ToastSpec` (which is plain data it can push, time out and dedup).
> Realizing a `Toast` node just renders a card wherever you put it.

### ProgressBar

Determinate progress track whose accent fill eases toward a value via `tick`.

- **Construct**: `ProgressBar::new()` (0).
- **Builders**: `.value(f32)` (initial, clamped 0–1).
- **Live update**: `.set(f32)` (animates), `.state() -> Signal<f32>`.

**Native:**

```rust
let bar = ProgressBar::new().value(0.4);
let v = bar.state();           // bind reactively, or:
bar.set(0.8);                  // animate to 80%
```

**Declarative (`ViewNode`)** — `WidgetKind::Progress`:

```rust
use heca_view::build::*;

Progress::new().value(0.4)
```

| prop | type | default | what it does |
| --- | --- | --- | --- |
| `value` | number `0.0..=1.0` | `0.0` | how far along. Out-of-range values are **clamped**, not refused — a description is untrusted input, and a bar that renders nothing is worse than a full one. |

**The easing is the widget's, not the caller's.** A described tree re-sent with a new `value`
animates toward it rather than jumping, with nothing declared — which is why there is no
"animate" property to set. There is no declarative `.set(..)` or `.state()`: both are live host
signals, and a description has no way to name one (the same reason a
[`ScrollBar`](#scrollbar) is host-only). A described bar changes by being re-described.

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
  > `Search` (`magnifying-glass`), `Close` (`x`), `Check`, `CaretRight`, `CaretLeft`, `CaretDown`, `Play`,
  > `Pause`, `Stop`, `Warning`, `WarningCircle`, `Info`, `Circle`, `Lightning`, `List`,
  > `Sidebar` (`sidebar-simple`), `DotsThreeVertical`, `ArrowRight`, `ArrowLineLeft`,
  > `ArrowLineRight`, `Plus`, `Minus`, `SquareSplitVertical`, `XSquare`, `FrameCorners`, `Cards`,
  > `Pencil`, `NotePencil`, `Backspace`, `Trash`, `XCircle`, `PlusCircle`, `FolderSimpleMinus`,
  > `FolderSimplePlus`, `PlusSquare`, `StackPlus`, `StackMinus`, `ColumnsPlusLeft`,
  > `ColumnsPlusRight`, `SquareHalf`, `SquareSplitHorizontal`, `SquareHalfBottom`

### NfIcon

A single glyph from the embedded **Nerd Font** — the app's *second* glyph set, and **single-layer**
(a Nerd Font glyph is one codepoint, not a duotone pair).

**Three faces, three jobs** — `FontRole` names which one a text run is shaped with:

| role | face | for |
|---|---|---|
| `Text` | Geist Mono | all UI text |
| `Icon` | Phosphor Duotone | [`Icon`](#icon) — the pictogram set |
| `NerdFont` | Maple Mono NF (also the terminal face) | `NfIcon` — what Phosphor has none of |

**Why a second set at all.** Phosphor has **no keyboard glyphs whatsoever**, and the UI face has
`⇧ ↑ ↓ ← → ⏎ ␣ ⇥ ⌫ ⌦` but **not `⌃ ⌥ ⌘ ⎋ ⇞ ⇟`** — so a shortcut spelled as plain text renders half
its keys as empty boxes. Both facts are measured, not assumed:
`heca-renderer/tests/font_coverage.rs` reads the embedded faces' `cmap` and fails if any named glyph
is missing (and if the UI face ever *gains* the four, it says so, so the decision can be revisited).

The font is the one already embedded for the terminal — a real Nerd Font, shipped in the binary, so
a glyph looks identical on every platform. It is addressed **by family name** and loaded
unconditionally at renderer init, so a user configuring their own terminal font cannot take the
glyphs out from under the UI.

- **Construct**: `NfIcon::new(NfGlyph)`; `.size(px)`, `.color(Color)` (inherits the enclosing
  control's content color when unset, like [`Icon`](#icon)).
- **`NfGlyph`**: the curated **keyboard set** — `Shift`, `Control`, `Option`, `Command`, `CapsLock`,
  `Enter`, `Escape`, `Tab`, `Space`, `Backspace`, `ArrowUp/Down/Left/Right`. `NfGlyph::ALL` is the
  authoritative list; the showcase renders it as a gallery with each name and codepoint.
- **Adding one**: add the variant, its codepoint in `codepoint()`, its `name()`, and the variant to
  `ALL`. The coverage test proves the codepoint is *in* the font; **it cannot prove the name matches
  the picture** — Nerd Font glyphs are named `uniF0636` in the file, so the five
  `nf-md-apple_keyboard_*` codepoints are read off the Material block's alphabetical order and
  **confirmed by eye in the showcase gallery**. Same caveat `Glyph::CaretLeft` carries.
- **Keycaps**: `KeyCap::Nf(NfGlyph)` draws one inside the shared keycap chip
  (`paint_keycap_nf`) — the path the [command palette](#commandpalette) uses. A glyph char passed to
  the plain `paint_keycap` would be shaped in the **UI** face, which does not have it: a silent
  empty box. That is why the two are separate entry points.

**Native:**

```rust
NfIcon::new(NfGlyph::Command).size(14.0);
NfIcon::new(NfGlyph::Shift).color(theme.colors.accent);
// Render the whole set (what the showcase gallery does):
for &g in NfGlyph::ALL { /* NfIcon::new(g) … */ }
```

**Declarative (`ViewNode`)** — `WidgetKind::NfIcon`, with its **own** glyph vocabulary
(`ViewNfGlyph`) because it is its own font:

```rust
use heca_view::build::*;
use heca_view::ViewNfGlyph;

// A plugin spelling out its own shortcut, the way heca's key hints do
HStack::new()
    .gap(2.0)
    .child(NfIcon::new().glyph(ViewNfGlyph::Command).size(14.0))
    .child(Label::new("K"))
```

| prop | type | default | what it does |
| --- | --- | --- | --- |
| `glyph` | key name — `"shift"`, `"control"`, `"option"`, `"command"`, `"caps_lock"`, `"enter"`, `"escape"`, `"tab"`, `"space"`, `"backspace"`, `"arrow_up"`, `"arrow_down"`, `"arrow_left"`, `"arrow_right"` | *(required)* | which key. **An unknown or missing name renders nothing**, rather than a different key — a ⌘ where the author asked for ⇧ reads as correct, which is worse than a gap. |
| `size` | number | *(inherited font size)* | glyph size in logical px. |
| `color` | colour **token name** | *(the enclosing control's content colour)* | glyph tint, so it follows a theme change. |

The two vocabularies are held to their libraries in both directions by the same guard, which also
takes one name of each through to a painted glyph — matching names is not the same as a name being
understood, and the codepoints behind them are a separate mapping that a name check cannot see.

**Icon examples, for comparison:**

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

> ⛔ **Attach it always, and wrap nothing around it.**
>
> The wrapper is what lets a line sit in the tree while it has **nothing to say**, so put it there
> unconditionally and let the signal decide. Adding it only when its content already exists cannot
> work — a signal reveals a child, it cannot create one, and a tree that is not rebuilt when the
> content arrives never gets a second chance:
>
> ```rust
> // ✅ the line is there, saying nothing, ready to be revealed
> column.child(Visibility::new(folder_line, cwd.is_some()))
>
> // ❌ if the directory arrives later, nothing can ever show it
> if cwd.is_some() { column = column.child(Visibility::new(folder_line, true)) }
> ```
>
> And nothing may go **around** it. An inset, an alignment box or a spacer wrapped around a
> `Visibility` is a visible widget holding an invisible one: it keeps its own box and its parent
> still spends a gap on it, which is the empty strip that makes hiding look broken. Put that
> decoration on the child inside, as padding, so it goes with the line.
>
> Both mistakes shipped together in heca's sidebar pane card: the directory line was attached only
> when the pane already had a directory, and the shell reports one *after* the row is on screen, so
> whether a pane showed its path came down to timing and neighbouring rows disagreed.

> ⚠️ **Reach for the wrapper only when there is no widget to declare on.** Both ways of not showing
> something are properties of **every** widget, and the wrapper cannot be put around a widget a
> typed container holds. Same rule as [`Tooltip`](#tooltip) and [`KeyHint`](#keyhint).

#### The two ways of not showing something

They are CSS's two, and they are **not** interchangeable:

| | property | what happens | reach for it when |
| --- | --- | --- | --- |
| `display: none` | `hidden` | out of the layout — **neighbours close up** | the space should collapse: an optional metadata row, a folded group |
| `visibility: hidden` | `visible` | ink gone, **box kept** | the space must not move: one of four status slots showing at a time |

```rust
// Native — one builder on any widget, no wrapper
StatusDot::error().visible(false)     // keeps its slot; the row does not shift when it appears
Row::new().hidden(true)               // gone from the layout entirely
```

```rust
// Described — the same two, on any node
use heca_view::build::*;

StatusDot::new().visible(false)
Row::new().hidden(true)
```

Between them there is nothing left for a `Visibility` **kind** to do, which is why there is none.
Note the asymmetry that used to exist and no longer does: `hidden` is a layout property and was
always describable, while `visible` is a signal on the base and was reachable from **neither**
authoring path — so the "keep the box" half silently did not exist for a described tree.

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

Props `realize` reads: `text` (header), `expanded` (Bool, default `true`). Children: the rows (the group's own header is prepended by the widget). Event: **`toggle`** — the intent carries the state it
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
- **Accessors**: `.state() -> Signal<bool>` (expanded), `.active_state() -> Signal<bool>` (the wash flag), `.nav_state() -> Signal<bool>` (the nav-cursor outline flag) — bind them to flip
  the look in place without rebuilding the tree.

> **The drag-handle grip is drawn but wired to nothing.** If you see it on a workspace header in
> heca and nothing drags, that is why. **Two different features claim it, and they are not the same
> thing:**
>
> - the frame as a **container** — drag the whole dock into another chrome region. The actions for
>   this already exist (`chrome.container.move_to_region` and friends); only the mouse surface is
>   missing.
> - the frame as a **row inside** a container — reorder it in the list. heca mounts one frameless
>   `DockFrame` per workspace row, so this is the drag the grip actually sits beside. This one still
>   needs its own action and drop logic.
>
> Both are tracked in the planner, and the grip is left in place deliberately as a placeholder for
> them.

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
  library supports all three; **the heca app currently uses only `Expanded` and `Hidden`** (the collapsed rail was dropped — see [`../docs/sidebar-provider-modes.md`](../docs/sidebar-provider-modes.md)).

```rust
let sidebar = ChromeRegion::vertical().expanded_size(320.0).rail_size(64.0)
    .dock(explorer).dock(source_control);
sidebar.toggle();   // or the host sets mode_signal() from a key / RPC
```

### RailCell

A focusable **square icon cell** — the per-item unit a *list* Dock (workspaces / panes) shows
when collapsed to a rail, so every pane stays visible and addressable (vs a tool Dock folding to
one icon). Centers one `Icon`; active = accent tint + same-hue border + glow; hover/press flash;
focus ring. It wears a move/swap/select pick letter with nothing written, because `on_activate`
makes it actionable — see [`.hintable`](#hintable-and-being-pickable).

**At rest it publishes a content glow so its bare glyph still haloes.** The cell draws no surface
at rest — the bare icon *is* the resting look — so there is nothing to carry the halo every
bordered surface gets. It cannot style its child either (children are `impl Component`), so it
publishes a glow via `PaintCx::with_content_glow` and the [`Icon`](#icon) pulls it, the same
publish/pull mechanism [content color](#scene--drawcommand--paintcx-for-building-widgets) uses.
Hover and active already have their own lit chrome, so they do not double it, and the glyph keeps
ownership of the halo's *reach* — only it knows how big it is.

> **Not currently mounted in the app.** The heca sidebar collapsed rail was dropped
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

> **From a plugin:** the leader/pick overlay is **host-owned and universal**. A described node with
> a `press` intent is reachable by `prefix+/` with nothing written, and a node that wants a pick to
> mean something *else* binds `hint` (see the declarative example below). Likewise a **context
> menu** is a host-owned dropdown the plugin declares with `.context_menu(…)`, not a nested widget.
> See **[plugins.md](plugins.md)** → §0 and "Menus and keyboard hints".

A **transparent wrapper that carries a hint letter on behalf of a region** — a group of widgets, or
something that is not a widget you can put a builder on. It is transparent to focus, layout and
events (the wrapped widget stays clickable and focusable); it only adds a declaration.

> ⚠️ **It no longer draws the letter, and it is no longer how a widget becomes pickable.** Reach
> for it only when there is nothing to hang a declaration on.

**What replaced it, and why.** Two rules used to live here and now live in the framework:

1. **Drawing.** Only `KeyHint::paint` knew how to draw a keycap, so a widget got one by being
   wrapped. The drawing moved into `component::paint_child`, which every container already funnels
   its children through, so **any** widget carrying a letter shows it — a plugin's included. The four
   placement knobs moved with it, onto `Base::hint_style`.
2. **Being pickable.** It used to say *"being pickable is something you opt a region into, so
   `Label::on_hint` is a method that never has to exist."* That is no longer true. **Anything you can
   act on wears a letter with nothing declared** — see [`.hintable`](#hintable-and-being-pickable) —
   and `on_hint` is the *override*, for a region that answers a pick differently from a click.

The slot itself is universal — `Base::hint` — so the framework's collector stays one uniform walk
with no downcasting.

- **Construct**: `KeyHint::new(child)` — a widget or a subtree built
  dynamically — a `realize`d `ViewNode` tree, or a chrome provider's render seam — where the concrete
  widget type is not known at the call site 
- **Builders**: `.hint(Signal<Option<String>>)`, `.placement(HintPlacement)`
  (`TopCenter` for compact square targets | `Center` for large panes | `CenterRight`
  for wide list rows — keycap pinned to the right edge | `TopRight` for tall targets like a
  workspace dock — right-aligned but anchored to the top edge, pair with `.offset_y` to land
  on the header row | `TopLeft` for a large target whose picture the letter must stay clear of —
  a card in the exposé, a content pane — pinned just inside the top-left and centred within a
  shallow band from the top edge, so it lands on the card's top line rather than floating in the
  middle of it),
  `.size(px)`, `.color(Color)` (override the keycap tint — default theme `accent`; lets a
  host distinguish target *kinds*, e.g. workspace picks tinted `warning` vs pane picks),
  `.offset_y(px)` (nudge the cap down after placement — e.g. drop a `TopCenter` cap onto a
  tall target's header row). The wrapper is **transparent to a stretching parent**: a wide
  child row fills its column instead of shrinking to content width.

  > ⚠️ **Those four are on every widget too**, as `.hint_placement`, `.hint_size`, `.hint_color`
  > and `.hint_offset_y` — the slot they write (`Base::hint_style`) was always universal, and now
  > the way to set it is. **Prefer them; wrap only when there is no widget to declare on.** The
  > wrapper's four delegate to them, so there is one writer per field rather than two that drift.
  > This is the same move the tooltip made, for the same reason: wrapping is impossible on a widget
  > a typed container holds, so a grouped button could not move its own letter at all.
  **`on_hint` is no longer here** — it is
  [`ComponentExt::on_hint`](#componentext--what-every-widget-gets), on every widget, so the two
  facts about a target (who it is, and what picking it does) stop living on two different nodes.
  A `KeyHint::new(row).on_hint(…)` call reads exactly the same; what changed is that a widget which
  can carry the declaration itself no longer has to be wrapped to say what a pick does to it.
- **Accessors**: `.hint_signal() -> Signal<Option<String>>`.

**Native:**

```rust
let pick = signal(None);
let cell = KeyHint::new(RailCell::new(icon).on_activate(/* … */))
    .hint(pick)
    .placement(HintPlacement::Center)
    // A pick is not a click: this row activates and leaves on a click, and stays put on a hint.
    .on_hint(move || emit(hint_intent.clone()));
// during a pick the host sets pick.set(Some("a".into())); clears it on exit
```

**Declarative** — there is no `KeyHint` node, because a description does not draw the letter: the
host does. A described node says *what a pick does* and *where the letter goes*, and `realize` writes
both into the same slots a native widget uses:

```rust
use heca_view::build::*;
use heca_view::ViewHintPlacement;

Row::new()
    .on_press(Intent::new("docker.select").arg("id", id))   // click: go there
    .on_hint(Intent::new("docker.reveal").arg("id", id))    // pick: look at it, stay
    .hint_placement(ViewHintPlacement::CenterRight)         // a wide row: cap on the right
    .child(Label::new(name))
```

Bind neither and the node is not a pick target; bind only `press` and a pick does what a click does.

| prop | type | default | what it does |
| --- | --- | --- | --- |
| `hint_placement` | `"top_center"` \| `"center"` \| `"center_right"` \| `"top_right"` \| `"top_left"` | `"top_center"` | where the cap sits over the node. |
| `hint_size` | number | *(from the font)* | cap font size in logical px. An integer is accepted as well as a fraction. |
| `hint_color` | colour **token name** | `accent` | cap tint, glow included. A token, never a hex literal, so it follows a theme change. |
| `hint_offset_y` | number | `0` | nudge applied after placement; positive moves the cap down. |

**Placement does not make a node pickable.** Anything actionable already wears a letter with nothing
declared — these only say where it goes, so a node with nothing to act on styles a letter it will
never show.

#### Which widgets get a letter

**Anything you can act on, wherever it is.** A button is a pick target because it is a button. Put it
in a pane's bar, a sidebar row, a plugin's panel or on its own — same button, same letter, and its
author never has to know which. Declaring a pick is for saying a pick means something *other* than
the click, never for winning back a letter.

**One letter per thing, not per layer.** The one case needing care is a wrapper: a node that exists
only to hold one other node. A decorator saying what picking does, around a card that can itself be
activated, is two nodes and one card. So:

- a wrapper and the single node it holds are **one thing** and share one letter — and if either of
  them *declared* a pick, that is the layer the letter runs, because a declaration is precisely the
  statement that a pick is not the click;
- a node holding **more than one** child is a real container, and what is inside it are separate
  things: each keeps its letter, and so does the container if it is a target itself.

A pane holds a bar and its content, so it is a container: the pane keeps its letter and every button
in its bar keeps one too.

**And nothing you cannot see.** Candidacy asks one question with two halves, and a target failing
either is dropped by the collector rather than at the letter — so it does not spend one of the 52:

- **hidden by a clipping ancestor.** Any overlap at all counts as visible, so a row half past a
  sidebar's fold keeps its letter — you can see it, so you can aim at it. Its keycap is drawn whole
  rather than clipped to match, which is the point of lettering a row you can only half see.
- **squeezed to nothing of its own** — laid out with no width, or no height. Such a widget draws
  nothing, so there is nothing to aim at, and the cap would not even land on it: a placement is
  computed from the target's box, so `Center` on a target of zero width puts the keycap half a cap
  to the *left* of it, outside a thing with no inside.

> ⚠️ **A box with no geometry AT ALL has not been laid out yet** — no position and no size, which is
> what every widget's bounds are before the first layout pass. That is *"no answer yet"*, never
> *"invisible"*: a retained tree is rebuilt with zero bounds and laid out afterwards, and judging it
> in between calls every row hidden and takes its letter back. A widget that is genuinely gone is
> hidden or invisible, which the walk already skips.

Both halves are one predicate, asked in both places a letter is decided: by `collect_hints` when it
spends one, and by the offer walk when it hands one over. Two copies would be two answers, and only
one of them is what you see. **Withdrawal is never refused** — a view that was lettered and has
since collapsed still gives the letter back, or the keycap outlives the picker that put it up.

> ⚠️ **The rule this replaced, so it is not reinstated.** A declared hint used to silence mere
> actionability *anywhere* beneath it. That silenced layers, and could not tell a decorator speaking
> for one card from a pane that merely contains buttons — so a button's letter depended on what it
> had been put inside. Every button in a pane's bar had to repeat its own click as a hint, and the
> `⋮` a [`ButtonGroup`](#buttongroup) builds for itself wore no letter at all.

**How the framework uses it** (`heca_grid_ui::hint`): `collect_hints(root)` walks the laid-out tree
and returns every declaration in document order with the rect its letter goes over; `fire_hint(root,
&path)` **delivers the pick as an `Event::Hint`** on the walk every other event uses, answering
`false` when the tree was rebuilt under the letters.

**Whose target is whose — `collect_hints_by_surface(root)`.** A host that puts things on *layers*
needs more than "what can be lettered": it needs to know which surface each target belongs to, so it
can ask that surface what it hides and stop at the one holding the keyboard. This answers it, and
hands back everything needed with it — no host walks the tree a second time:

```rust
for group in collect_hints_by_surface(&root) {
    group.path;      // where the surface is; EMPTY means the page itself
    group.key;       // the surface's own declared key, for a host that looks it up
    group.bounds;    // its laid-out box, for a host that occludes by geometry
    group.targets;   // its targets, each addressed FROM THE ROOT
}
```

Three rules it holds so that nobody re-derives them:

- **Front → back**, which is lexicographic on the path reversed — sibling order is paint order and a
  child is drawn above its parent, so descending path order *is* "nearest the viewer first". Not a
  second ordering to keep in step with the tree's own.
- **The page is the group with no path**, so "page or surface" is answered by structure and never by
  matching a name.
- **A target belongs to the deepest surface enclosing it**, so a menu inside a dialog groups under
  the menu. A host that reads the first step of a path instead is assuming every surface is a direct
  child of the root — true only while whatever seats them keeps making it true.

A surface is any node marked `Base::surface`, which is what seating one sets; an author writes
nothing. Built **on** `collect_hints`, so the candidacy rules live in one place.

**What a surface says about itself, and what it does not have to say:**

```rust
Overlay::new().lock(true).child(my_panel)   // the ONE line an author writes
```

- **`lock`** is a declaration, because nothing in the tree implies it. A map of the
  working area covers every pixel it draws over and still says `false` — seeing the panes through it
  is the point — while a dialog says `true`. `group.lock` hands it back.
- **`holds_keyboard` is read, never declared.** A surface that wants keys holds focus, and every
  layer widget binds its open signal to `Base::focused`, so an open overlay answers `true` by being
  open and a toast stack answers `false` by holding no focus. `Base::captures_keyboard` overrides it
  for a surface the framework cannot read — reach for it almost never.
- Neither is `overlay_occludes`, which is the *geometric* question the pointer asks.

**A declaration can say what it *is*, not only what it runs.** `on_hint` takes a closure or a
`Hint` — `Hint::of(intent, run)` — carrying the `Intent` the act names. That is what lets a host ask
its own policy about a candidate *before* spending a letter on it: `hint_intent(root, &path)` hands
the declaration back and the library takes no view. heca uses it for one rule in one place — a pick
whose action the current focus domain would refuse is dropped from the candidate list, so
`prefix+/` stops lettering panes you cannot focus while a pane is floating. A plain closure has
nothing to ask about and is always offered. **Nothing is
registered** — a target is addressed by its path for exactly as long as the letters are up, so there
is no allocator to keep in step with three rebuild cadences and nothing to un-register. This
replaced an opaque `HintTargetId` the host mapped back to a `pub(crate)` enum, which a plugin could
not construct.

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
    [ContextMenu](#menus--menuitem-menu-contextmenu) quick-pick inside the menu panel). The [ContextMenu](#menus--menuitem-menu-contextmenu)
    draws its letter at a **sub-font scale** so the chip stays compact.

```rust
let cap = Rectangle::new(Point::new(x, y), keycap_size(font, "a"));
paint_keycap(cx, cap, "a", font, None, KeycapVariant::Filled);   // over content
paint_keycap(cx, cap, "a", font, Some(accent), KeycapVariant::Bordered); // on a panel
```

**Plugins** never call this directly — a described node with a `press` (or `hint`) intent is a pick
target and the host stamps the keycap for it.

### KeyHintGroup

A **picker you can declare**, over a subtree you choose. [`KeyHint`](#keyhint) carries one region's
declaration; this opens a picker over *many*: while open it letters every target beneath it, holds
the keyboard, and runs the one whose letter you type.

**Why it is a widget and not a host facility.** It used to be host-only, which made the picker a
shipped feature nobody outside the app could have — a plugin could contribute *targets* to heca's
picker and never own one. Nothing about it needed the host: a widget can hold focus, and a focused
widget gets the keys. So the whole picker is (1) walk my own descendants for targets, (2) hand each
one a letter — `Base::hint_label`, which the declaring widget **draws itself**, (3) hold focus while
open and fire on the next typed character.

The host keeps exactly one thing, because only it can answer it: **which surfaces are eligible** when
the picker's scope is the whole screen. `prefix+/` is this widget's behaviour at screen scope with
that filter applied; a plugin's is the same behaviour scoped to its own panel.

- **Construct**: `KeyHintGroup::new(child)` — a widget or a
  subtree built dynamically 
- **Builders**:
  - `.opens_on("mypanel.pick")` — **the verb that opens it**, and the whole of what a picker costs
    its author. The widget owns its open signal, holds the keyboard while it is up, and declares the
    name; config binds the key to that name (`[[keys.surface]]`). Reach past it only when the host
    already has the state — when something *other* than this verb also opens the picker.
  - `.open_when(Signal<bool>)` — the **host-owned** open state, for that case. Host-only. Binding it
    also makes the group hold focus while open: holding focus *is* taking the keyboard, so there is
    no gate to write and nothing to decline.
  - `.letters(impl IntoIterator<Item = char>)` — **the letters this picker hands out, in order.**
    Defaults to `DEFAULT_LETTERS`. Host-only (an app's choice of alphabet, not data a described tree
    carries).
- **Accessors**: `.is_open() -> bool`, `.open_signal() -> Signal<bool>`.
- **Dismissal** comes from `[keys.widgets]`, so the widget names no key of its own.
- **Letters are claimed in capture**, before the subtree sees them: the regions a picker covers
  commonly answer typed characters themselves — the exposé's cards take `x`, `r`, `d` — and while it
  is open those letters are the picker's.
- A group **does not letter itself**, so a group nested in another is a target of the outer one only
  through its children.

**`DEFAULT_LETTERS`** — the shared alphabet, `pub` from `heca_grid_ui::widgets`:

```
asdfghjklbceimnopqrtuvwxyzASDFGHJKLBCEIMNOPQRTUVWXYZ
```

**Home row first**, so the targets a picker finds first get the keys your fingers rest on; lower case
before capitals because they are one keystroke on every layout. **One letter per pick, always, and 52
is the cap** — past the end of the sequence a target simply gets no letter. Two-key sequences were
raised and refused — one letter, always, and 52 is the cap: anything needing more than 52 at once is a picker covering too much, and the answer is a
smaller picker, never a longer keystroke.

The app reads the same constant, so `prefix+/`, the pane / column / workspace / dock picks and a
widget picker all spend letters in the same order.

**Native:**

```rust
let picker = KeyHintGroup::new(
    Flex::column()
        .child(Item::new("one").on_activate(|| choose(1)))
        .child(Item::new("two").on_activate(|| choose(2))),
)
.opens_on("mypanel.pick")          // config binds the key to this name
.letters("asdfghjkl".chars());     // optional — home row only, for a small panel
```

Note neither item declares anything to be pickable: both are actionable, so both wear a letter — see
[`.hintable`](#hintable-and-being-pickable).

**Declarative** — a described tree opens a picker over its own subtree the same way, and binds the
key through `[[keys.surface]]` on the name it declares:

```json
{ "kind": "KeyHintGroup",
  "props": { "opens_on": "mypanel.pick" },
  "children": [
    { "kind": "Row", "events": { "hint": { "action": "docker.restart", "args": { "id": "web" } } } }
  ] }
```

```toml
[[keys.surface]]
name = "mypanel"
pick = "s"          # → mypanel.pick
```

That is the whole of it: a string in the plugin's own tree and a key in the user's own config. No
registry, no id to hold, no signal and no closure — which is why the picker is the *same* widget the
exposé uses and not a cut-down copy of it.

**Any node can declare a verb**, not just a picker: `on_action(name, intent)` is on the SDK's shared
builder trait, because `ComponentExt::on_action` is on every widget natively. A described surface
therefore owns verbs of its own instead of borrowing ones the app compiled in, and whichever node on
screen declares the name is the one that runs — so a verb whose surface is not up resolves to
nothing. A verb whose intent names *itself* is refused at realize time: it would resolve back to the
same widget and re-post itself forever.


### FocusScope

A **generic** transparent wrapper that makes its child subtree a **keyboard focus scope**: keys enter
only while it holds focus, and it outlines itself while it does. Both from one host-owned
`Signal<bool>`.

Every focusable *control* already rings itself and answers for its own keys, because it owns its
focus. `FocusScope` is for the other case: when the thing holding keyboard focus is **a whole area** —
a sidebar dock the scroll keys act on, a panel a mode is aimed at — no single widget in the tree owns
that focus, so none can answer for it. The host does, by flipping one signal (read-via-signals /
write-via-actions), exactly as it drives [`KeyHint`](#keyhint).

It adds exactly two things:

1. **The gate.** `Event::Key` and `Event::Widget` enter the subtree only while the scope holds focus.
   An unfocused scope neither reacts nor **consumes**: it declines, so the next sibling — the scope
   that does hold focus — still gets its turn. That is what lets a host broadcast one semantic intent
   (say `WidgetIntent::ScrollPageDown`) into a tree of scopes and have the right one answer, without
   knowing where any of them sits. Consuming instead would mean the first scope in a region silently
   ate everything.
2. **The outline**, in place — no tree rebuild.

**The pointer is never gated.** Click, drag, hover and wheel reach an unfocused scope exactly as
before: the mouse carries its own target, so it needs no focus to say where it meant — and a click on
an unfocused dock is how you focus it. (`tests/pointer_delivery.rs` holds this to the whole pointer
set, focused and unfocused.)

The two halves share the signal on purpose: a ring that says "the keys come here" while the keys go
elsewhere is worse than no ring.

- **Construct**: `FocusScope::new(child)` — a widget or a
  dynamically built subtree (a `realize`d tree, a provider's render seam).
- **Builders**:
  - `.focus(Signal<bool>)` — the host-owned focus state. **Host-only** (a live signal, which static
    data cannot drive). Default: an internal signal that is `false`, i.e. no outline.
  - `.radius(px)` — corner radius of the outline. Default: the theme's `control_radius()`.
  - `.color(Color)` — outline colour. Default: `effective_focus_ring()` (the `focus_ring` token, or
    the accent shifted toward `foreground`). Override to mark a *kind* of focus distinctly, the way
    `KeyHint::color` distinguishes kinds of pick target.
- **Accessors**: `.focus_signal() -> Signal<bool>`.
- **Routing**: none of its own. `.focus(sig)` binds that signal to `Base::focused`, and the
  framework delivers keyboard events to the focus owner's chain — so an unfocused scope is simply
  not on the path, and the focused one is, with nothing gated, declined or forwarded. It used to
  claim `routes_own_subtree` and skip the walk per event kind; that predicate is gone.
- **Theme**: the outline is [`PaintCx::focus_ring`] — the same primitive every control's ring uses,
  at `focus_border_width`, offset outside the bounds like a CSS `outline`. A theme with
  `show_focus_border = false` hides this one too: whether focus outlines are drawn is the theme's
  decision, uniformly, not each caller's.

```rust
// The host owns the signal; an action moves focus and the ring follows, with no rebuild.
let focused = signal(false);
let framed = FocusScope::new(my_container).focus(focused);
// …later, from an action:
focused.set(true);
```

Declaratively there is nothing to author: the whole widget is a live host signal, so a described
`FocusScope` would be a dead frame (the same reason [`ScrollBar`](#scrollbar) is host-only). A plugin
that wants its container to show focus gets it for free — **heca wraps every mounted container
itself**, together with its dock-pick keycap, and drives the ring from the same signal the
container's scroll area binds as its keyboard target. See
[chrome-and-ui.md](chrome-and-ui.md) → chrome keyboard focus.

### Tooltip

**A tooltip is a property of a widget, not a box around it.** Every widget takes one, on the same
terms, with one builder:

```rust
Button::new("Close").icon(Glyph::Minus).tooltip("Close the pane")
IconButton::new(Icon::new(Glyph::Gear)).tooltip("Settings").tooltip_side(TooltipSide::Bottom)
```

The framework owns everything behind it. The reveal is timed from the hover clock the **pointer
router already keeps** (`PointerState::hovered_for`), and the bubble is drawn in `paint_child` — the
one place every widget passes through — beside the hint letter and the drag feedback. Those three
are the same kind of thing: something the framework draws *over* any widget from state it already
has, so **no widget opts in and no host paints on their behalf**.

| builder | what it does |
| --- | --- |
| `.tooltip(text)` | what this widget says on hover. Unset = it says nothing, and costs nothing. |
| `.tooltip_signal(Signal<String>)` | the same, from a live signal — an action's current keybinding, a changing status — so the bubble follows without the widget being rebuilt. |
| `.tooltip_side(TooltipSide)` | which side to prefer (`Top` default). Flipped automatically when there is no room, so it is a preference, not a placement. No-op with no tooltip declared. |
| `.tooltip_delay(seconds)` | how long the pointer must rest (default `0.5`). No-op with no tooltip declared. |

All four are on `ComponentExt`, so they apply to **every** widget — and the first, third and fourth
have the described spellings below. `tooltip_signal` is native-only, because a signal is a live host
value a description cannot name.

> ⚠️ **The bubble is drawn by `paint_child`, not by the widget's own `paint`.** Anything that paints
> a tree with a bare `.paint(cx)` shows the widget and none of the three things the framework draws
> over it — no tooltip, no hint letter, no drag feedback. Hosts and tests must go through
> `paint_child`, exactly as the letter already requires.

#### The wrapper — for a region that is not a widget

`Tooltip::new(child, text)` still exists, and is now **implemented in terms of the property**: it is
a transparent wrapper carrying a tip on its own base, so there is one reveal, one placement and one
bubble rather than two that can drift.

Reach for it only when there is no widget to declare the tooltip on. That is exactly the role
[`KeyHint`](#keyhint) kept when the pick declaration moved onto every widget, and it is kept here
for the same reason.

**Why the property had to exist**, beyond the wrapping being noise: a widget held by a **typed**
container cannot be wrapped. [`ButtonGroup`](#buttongroup) takes `Button` children, and
`Tooltip::new(button, …)` is a `Tooltip`, not a `Button` — so under the old design a grouped button
could not carry a tip at all.

The bubble is drawn on the **overlay layer** so it sits above siblings. It captures **no** input —
the widget stays fully interactive (events + focus pass through untouched).

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

- **Construct the wrapper**: `Tooltip::new(child, text)`, or
  `Tooltip::new_signal(child, Signal<String>)` for a reactive label.
- **Wrapper builders**: `.side(TooltipSide)` (`Top` | `Bottom` | `Left` | `Right`, default `Top`),
  `.delay(seconds)` (hover delay before reveal, default `0.5`) — the same two values the
  `.tooltip_side` / `.tooltip_delay` builders set on any widget.
- **`TooltipSide` is `BesideSide`** — the same type, re-exported under the name that reads better
  at a call site. There is one four-sided vocabulary, not two.

```rust
Tooltip::new(
    IconButton::new(Icon::new(Glyph::Close).color(theme.danger).size(20.0)).on_click(|| close()),
    "Close",
).side(TooltipSide::Bottom);
```

**Declarative (`ViewNode`).** There is no `WidgetKind::Tooltip`, and there must not be one — for the
same reason the wrapper stopped being the native answer. A tooltip is a **declaration on any node**,
exactly as a [menu](#menus--menuitem-menu-contextmenu) is:

```rust
ViewNode::new(WidgetKind::Button)
    .text("Close")
    .prop("tooltip", PropValue::Text("Close the pane".into()))
    .prop("tooltip_side", PropValue::Text("bottom".into()))  // optional, default "top"
    .prop("tooltip_delay", PropValue::Float(0.25))           // optional, default 0.5
```

```rust
// The same, through the typed SDK — universal, like `.key(..)`
Button::new("Close").tooltip("Close the pane").tooltip_side(ViewTooltipSide::Bottom)
```

| prop | type | default | what it does |
| --- | --- | --- | --- |
| `tooltip` | text | *(none)* | what the node says on hover. Absent = it says nothing. |
| `tooltip_side` | `"top"` \| `"bottom"` \| `"left"` \| `"right"` | `"top"` | which side to prefer; flipped when there is no room. Ignored with no `tooltip`. |
| `tooltip_delay` | number | `0.5` | seconds the pointer must rest. An integer is accepted as well as a fraction. Ignored with no `tooltip`. |

There is no declarative `tooltip_signal`: a signal is a live host value, and a description has no way
to name one — the same reason a `ScrollBar` is host-only. A described tooltip whose words change is
a re-described node.

In the heca app an action button's tip is derived from its `WmAction` centrally (next section),
never authored per call site.

> The raw `Tooltip::new(button, "Close")` above hardcodes the text. **In the heca app,
> do not do this for an action button** — see the next section: the tip (and its
> keybinding) is derived from the action, centrally.

### Action buttons — tooltip + KeyHint from the action (heca app pattern)

Any chrome button that triggers a `WmAction` gets its **tooltip** and its `prefix+/`
**KeyHint** from that action, automatically — the caller names the action, never a
shortcut string, the leader symbol, or a hand-built tip. This keeps every button
uniform and rebind-aware. The grid-ui primitives involved are **`IconButton`**,
**`Tooltip`**, and **`ComponentExt::on_hint`** ([`KeyHint`](#keyhint) framework); the
resolution seam is app-side.

```rust
// heca/src/chrome/mod.rs — one call composes label + the live keybind(s):
let fire = move || emit(InteractionIntent::ActivateAction(action.clone()));
let hint = fire.clone();                        // this button's pick IS its click
let button = IconButton::new(icon).on_click(fire);
row.child(action_tooltip(KeyHint::new(button).on_hint(hint), "close", "Close", &state.action_shortcuts));
//                                                            ▲ action config name  ▲ label
```

- **Tooltip text is resolved by action name.** `ActionShortcuts` (on `AppState`, rebuilt
  at config load/reload) maps each action's config name → its display shortcut via
  `shortcut::shortcut_for_action`, which reads the **user's real binding when they've
  rebound it** (defaults only as fallback), supports **multiple** bindings (joined
  ` / `), and renders the leader through the `PREFIX_SYMBOL` constant — never a literal
  `λ`, never `⌃⌥⇧⌘`. So a rebind in `config.toml` updates the tip with no code change.
- **The name is the canonical key**, because the emitted `WmAction` may be a button-only
  variant that isn't itself bound (`ClosePaneById`, `AddPaneToColumn`).
- **KeyHint** = declare on the wrapper what a pick does. Nothing is registered: the
  framework collects the declarations out of the laid-out tree (`collect_hints`) and runs
  one (`fire_hint`), so a plugin's button is as pickable as the app's. Active-targeted
  buttons (zoom/float) emit a `FocusPane` first, exactly like the click. Full app-side
  rules are in **AGENTS.md → "Chrome buttons → action, tooltip, KeyHint"**.

### Overlay

The **base overlay surface** every overlay widget shares: a
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
  the chrome around it. `.panel(..)` also takes a mapper-produced panel (e.g.
  `heca`'s `realize(ViewNode)`).
- **Builders**: `.blocking(bool)` (default `true` — scrim + swallow outside input; `false` = no
  scrim, outside input falls through), `.frosted(bool)` (default `false` — see *The frosted
  backdrop* below), `.opened(bool)` (the **initial** state — see *Showing and
  hiding* below), `.on_outside_click(impl Fn())` (standalone dismissal hook; a composing widget applies its own
  policy instead).

#### The frosted backdrop — `.frosted(true)`

**Blur what is behind the surface.** The scrim's counterpart: a scrim *tints* what is underneath,
a frost takes its *detail* away, and a surface may want either, both or neither.

```rust
Overlay::new().blocking(true).frosted(true).panel(map)   // the exposé's backdrop
```

```json
{"kind": "overlay", "props": {"blocking": true, "frosted": true}}
```

- **Strength is the theme's**, never the caller's — `overlay_frost_radius`, beside the scrim alpha
  it is the counterpart of. A theme that wants a flat backdrop sets it to `0` and every frosted
  surface answers together. A described surface asks for the *effect*, never a radius.
- **It blurs exactly what it occludes** — the viewport when `blocking`, the panel alone when not.
  The same reach [`overlay_occludes`](#component-trait) reports, read from one place, so the frost
  and the input policy can never disagree about how far a surface goes.
- **It fades with the surface that asked for it.** Left at full strength it holds the whole session
  out of focus for the length of the fade and then snaps sharp in one frame — the exact pop the
  fade exists to remove. It takes the animation's *opacity* and nothing else: a blurred **region**
  that also zoomed would be blurring somewhere the surface is not.
- **It is recorded, not performed.** A blur is GPU work, so the widget emits a
  [`backdrop_blur`](#painting--paintcx) request at its place in the drawing order and the host
  performs it (`docs/surface-compositor.md` § 0.5). The request goes into the **base** band, not
  the deferred overlay band the surface's own visuals use — the host flushes the base, performs the
  requests, then flushes the overlay bands, which is what "after everything beneath me, before me"
  means in one pass order.

A frosted surface therefore needs **nothing** from whoever places it: no registration, no host pass
keyed on it, no declaration outside the tree.
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
  [`Select`](#select) (rect-anchored, forced side) and [`ContextMenu`](#menus--menuitem-menu-contextmenu)
  (point-anchored) delegate their placement here, so the flip/clamp rule exists once.
- **Sizing**: `.panel_size(width: Length, height: Length)` gives the panel an explicit size instead
  of letting it hug its content. Default = unset (hug). `Length::Auto` on an axis keeps the hug
  behaviour there; a `Length::Percent` resolves against the **viewport**, since the `Overlay` fills it
  (`Percent(0.6)` = 60% of the viewport). Call order does not matter — the size is stored and re-applied
  whenever `.panel()` replaces the child.
  **Why it matters:** a [`ScrollRegion`](#scrollregion) only scrolls when its parent *bounds* it. An
  unsized panel grows with its content, so a long body never overflows and no scrollbar appears.
  Size the panel and the body can scroll inside it.
- **Arriving and leaving**: `.animation(Animation)` — see
  [Animations](#animations--how-a-surface-arrives-and-leaves). The `Overlay` owns appearing and
  disappearing as a *concept* and hands the **how** to the animation, so showing and hiding stop
  being instant: a dismissed overlay stays on screen, **inert**, until its exit has played out.
  Unset ⇒ a cut, which costs nothing.
  ```rust
  Overlay::new().panel(body).animation(Animation::Fade)
  Overlay::new().panel(body).animation(Animation::ZoomFade)           // the exposé's gesture
  Overlay::new().panel(body).animation(Animation::Zoom.from(0.8))     // tuned
  Overlay::new().panel(body).animation(Animation::of(MyWhirl::new())) // …or one you wrote
  ```
  **One builder, both authors** — a description names the same animation, because `Animation`'s
  variants *are* the vocabulary:
  ```jsonc
  { "kind": "overlay",
    "props": { "blocking": true, "open": true, "animation": "zoom_fade" },
    "children": [ { "kind": "panel", "props": { "title": "MAP" }, "children": [ … ] } ] }
  ```
  ```rust
  // …or through the typed SDK (heca-view):
  build::Overlay::new()
      .animation(ViewAnimation::ZoomFade)
      .child(build::Panel::new().title("MAP").child(build::Label::new("…")))
  ```
  **Input follows open; painting follows the gesture.** A leaving surface holds no input — it is
  not focusable, occludes nothing and hit-tests to nothing — while it keeps being drawn. (Counting a
  dissolving map as coverage refused every act on the pane it exists to let you choose.)
- **Showing and hiding**: `open()` / `hide()` / `toggle()` — on the widget, and on `Component` so a
  host can drive one it only holds as `Box<dyn Component>`. All three are safe at any time: opening
  something already up is not an arrival, and one already on its way out is not resurrected (those
  rules live in `Presence`, so no caller repeats them).
  ```rust
  let mut o = Overlay::new().panel(body);                       // no animation — that is fine
  let mut o = Overlay::new().panel(body).animation(Animation::ZoomFade);

  o.open();     // animation declared? it plays. None? it is simply up.
  o.hide();     // animation declared? the exit begins, and it stays on screen, inert, until the
                // gesture has played out. None? it is gone now.
  o.toggle();
  ```
  `.opened(bool)` is the **build-time** state: a surface born open is already there and plays no
  arrival. `open_signal()` is the same fact as a signal, which is how a *composing* widget drives it
  ([`Dialog`](#dialog)'s buttons flip it); `Overlay::tick` follows the signal, so a flip is an
  arrival or a dismissal exactly as the verbs are.
- **Who calls them for a mounted surface** — in `heca`, the **layer stack**, not the surface itself.
  The exposé is the worked example, and it calls none of the three:
  1. `prefix+Tab` → `WmAction::ToggleLayer { name: "heca.expose" }` →
     `handlers::handle_layer_visibility`.
  2. That **rebuilds** the surface first — `chrome::rebuild_named_layer(state, name)`, which calls
     `expose::register(state)`: the content is structural (a pane opened, a column went), and a
     signal replaces a value, never a child. `register` is the only part that touches `AppState`; it
     gathers the session and calls `expose::map(..)`, which builds the `Overlay` — that returned
     `Box<dyn Component>` is the layer's **root**.
  3. Then `LayerRegistry::show(id)` marks the layer visible and calls `l.root_mut().open()` — the
     `Overlay` from step 2. `hide(id)` calls `root.hide()` and keeps the layer mounted while
     `is_leaving()`.
  4. A rebuild *while it is up* hands the old tree's `Presence` to the new one, so an arrival
     already played does not play again and one still in flight carries on.

  A surface that is **not** a layer — a popover inside a page — is driven directly: hold it (or its
  `open_signal`) and call `open()` / `hide()` / `toggle()` yourself.
- **Accessors**: `.open_signal() -> Signal<bool>`; `.panel_bounds() -> Rectangle` (valid after
  layout); `.presence()` / `.presence_mut()` (`Component`) — the surface's arrival and exit as one
  value, which is how a host carries a gesture across a **rebuild**: swap the `Presence` into the
  fresh tree and an arrival already played does not play again, while one still in flight carries on.
- **Declarative kind**: `WidgetKind::Overlay`. Its `children` are the panel — one child *is* the
  panel, several are stacked into one. Props: `blocking`, `open`, `animation`. Placement is
  host-only (an anchored overlay carries a host-computed trigger rect).
- **Contract**: `focusable`/`overlay_active` only while open (host overlay scan);
  `overlay_occludes` = whole viewport when blocking, else the panel rect.
- **Shared panel chrome**: `paint_panel_chrome(cx, rect, PanelChrome { border, glow, elevation })` is the single
  authority for what an overlay panel *looks like* — drop shadow (lifting it off the page), the theme
  surface fill, the per-widget accents, and the panel's **edge**. The base `Overlay` passes
  `PanelChrome::default()`; [`Select`](#select), [`ContextMenu`](#menus--menuitem-menu-contextmenu), and
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
  | `Panel` (default) | `Dialog`/`Overlay`, [`Select`](#select), [`ContextMenu`](#menus--menuitem-menu-contextmenu), [`CommandPalette`](#commandpalette) | full depth |
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
    .panel(realized_panel)                        // Box<dyn Component> from realize(ViewNode)
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
| A cursor **point** (context menu) | `place_at_point(anchor, panel, vp, inset, centered)` | [`ContextMenu`](#menus--menuitem-menu-contextmenu) |
| A target rect, **centered on any of 4 sides** (hover bubble) | `place_beside(anchor, panel, vp, gap, side)` with `BesideSide::{Top,Bottom,Left,Right}` | [`Tooltip`](#tooltip) |
| A panel to **decorate** | `paint_panel_chrome(cx, rect, PanelChrome { border, glow })` | `Overlay`, [`Select`](#select), [`ContextMenu`](#menus--menuitem-menu-contextmenu), [`CommandPalette`](#commandpalette), [`Tooltip`](#tooltip) |

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

**Nested overlays work**: a [`Select`](#select) opened inside the body composites
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
> tried and reverted).

- **Construct**: `Dialog::new(title)`, then `.body(impl Component)` and `.action(impl Component)`
  (a wired `Button`), in that order. Buttons sit in a right-aligned row in call order.
- **Builders**: `.dismissible(bool)` (default `true`; `false` = forced-decision — `Dismiss`/scrim
  swallowed without dismissing), `.on_dismiss(impl Fn())` (fired on `WidgetIntent::Dismiss` / scrim),
  `.open(bool)`
  (focuses the first focusable — a text field body if present, so the user types immediately;
  otherwise the first button as a safe default), and `.body(..)` also takes a realized body
  from a mapper (e.g. `realize`).
- **Sizing + a scrollable body**: `.panel_size(width: Length, height: Length)` bounds the panel
  instead of letting it hug its content (default = hug; `Length::Auto` keeps hugging on that axis;
  `Length::Percent` resolves against the **viewport**). This is what makes a long body scrollable: a
  [`ScrollRegion`](#scrollregion) only scrolls when its parent bounds it, so wrap the body in one and
  size the panel. Put **only the body** in the region — the title and the action row stay fixed:

  ```rust
  Dialog::new("Pick a container")
      .panel_size(Length::Percent(0.5), Length::Percent(0.6))   // 50% × 60% of the viewport
      .body(ScrollRegion::new().child(long_list))       // only this scrolls
      .action(Button::secondary("Cancel"))
  ```
  A [`Select`](#select) inside a scrolled body still composites **above** the action buttons (the nested-overlay routing above), so overlay-in-scrolled-overlay is supported.
- **Accessor**: `.open_signal() -> Signal<bool>`.

```rust
let dialog = Dialog::new("Delete pane?")
    .body(Label::new("This action cannot be undone."))
    .action(KeyHint::new(Button::secondary("Cancel").on_click(cancel)).on_hint(cancel_hint))
    .action(KeyHint::new(Button::destructive("Delete").on_click(delete)).on_hint(delete_hint))
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
        body: ViewNode::new(WidgetKind::VStack)
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
like a native one. (A first-class typed builder — `VStack::new().gap(8).child(…)` — is
`plugin-task-ui-2`; today author the nodes with `ViewNode::new(kind).prop(…).child(…)`.)

### CommandPalette

A fuzzy **command launcher** overlay (same input-capturing contract as `Dialog`): a query line
over a scrollable list of commands. The query line is a real [`Input`](#input), so full editing
comes for free — selection, multi-click, and char/word/line delete (Ctrl/Alt/⌘ + Backspace/Delete).
Typing filters with a **fuzzy subsequence** match, **smart-case** (case-insensitive unless the
query has an uppercase letter), ranked, with matched characters highlighted in the accent.
Selecting a command fires its callback and closes.

- **Construct**: `CommandPalette::new()`; add commands with `.command(Command::new(label, on_run)
  .description(text)?.icon(Glyph)?.keys([KeyCap…])*)`; `.placeholder(text)`, `.open(bool)`.
- **Bindings**: `.keys([KeyCap])` adds **one binding**, drawn as a row of keycap chips
  (`[λ] [⇧] [e]`) through the shared `paint_keycap` primitive — never hand-drawn. Call it once per
  binding: a second call **stacks** a second row under the first, so an action bound three times
  shows three rows and the row grows to fit (one height for the whole list, as above). Every row
  reserves the **same** width for chips — the widest chord in the list — so they form a column
  instead of tracking each label's length, and the label box stops short of it.
- **`KeyCap`**: `Nf(NfGlyph)` for a key with a picture (shift, control, option, command, escape,
  tab, space, backspace, the arrows), `Text(String)` for anything else (a letter, `]`, `F5`, the
  `λ` prefix). Two cases because the two are drawn in **different faces** — see
  [`NfIcon`](#nficon). The app builds them from a binding in one place
  (`heca/src/shortcut.rs::chord_caps`).
- **Description**: an optional muted **second line** under the label. As soon as **any** command has
  one, every row in the list is two lines high — one row height for the whole list, because
  `row_rect` is what the paint, the hover-select and the click hit-test all share, and per-row
  heights would make that a running sum in three places. Filtering still matches the **label** only:
  a description explains a command the user has already found, and ranking on it would surface a
  command whose label the query never mentioned.
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
    .command(
        Command::new("Split pane", || wm.split())
            .description("New column to the right of the active pane.")
            .icon(Glyph::Sidebar)
            .key("⌥⌘S"),
    )
    .command(Command::new("Close pane", || wm.close()).key("⌘W"));
let open = palette.open_signal();
// host: on Ctrl+K → open.set(true); add `palette` to the tree
```

> Needs the same host wiring as `Dialog` (route keys to the overlay). Because it tracks `Ctrl` for
> Ctrl+J/K, the host must also broadcast `Event::ModifiersChanged` to the tree (most hosts do).

### Menus — `MenuItem`, `Menu`, `ContextMenu`

**A menu is a DECLARATION, not something you place.** Both authoring paths say it the same way — one
line on the thing that opens it — and neither builds, sizes or anchors a panel. The framework takes
the anchor from whatever triggered it, dismisses it, and gives it the keyboard.

```rust
// Native — one builder on ANY widget, like `key`
row.context_menu(my_menu)
```

```rust
// Described — one field on ANY node, entries carrying Intents
ViewNode::new(WidgetKind::Row)
    .menu([DropdownItem::with_intent("stop", "Stop", Intent::new("docker.stop").arg("id", id))])
```

Each entry is a `DropdownItem { id, label, intent, danger, enabled }`: `id` is its visual identity
(the icon and label resolve from the action catalog), `intent` is what it actually runs. They are
separate on purpose — a "Close pane" entry shows the `close` icon while dispatching
`close_pane_by_id` with the row's pane. Every choice goes through the one dispatch door, so the
interaction policy and the confirm gate apply exactly as they would for a keypress, and a plugin's
entry runs **its own** registered action rather than only heca's.

⚠️ **Do not add a `ContextMenu` widget kind.** A menu a plugin has to assemble out of parts is the
second path, and it will get the anchor, the dismissal and the keyboard half only approximately
right. Nothing declared means nothing opens; bubbling stops at the nearest declaration.


**Menus, split by what each part actually knows.**

| type | what it is |
|---|---|
| `MenuItem` | one row: either sugar (a label and an icon) or **any widget subtree** |
| `Menu` | a titled list of items. **Content only** — it knows nothing about triggers, anchors or keys |
| `ContextMenu` | a named presenter: contains one `Menu` and shows it on right-click or the host's `open_context_menu` action |
| `MenuBar` | *not built yet*: contains `Menu`s and shows them as a strip, on a click of a title or its own keybinding |

There is no fourth type, and **the panel is not one** — a `ContextMenu` *is* the panel it shows: it
holds the rows, lays them out, paints them and hit-tests them. The split above is at the joint a
menu bar proves is real: a `MenuBar` will show the **same `Menu` value** as a strip, with its own
trigger and keyboard convention. The trigger, the anchor and the shortcut belong to whatever
*contains* the menu; the menu is content, and stays reusable across every surface that shows one.

#### What you write

```rust
let ctx = ContextMenu::new("pane-menu").child(
    Menu::new("Pane", "What you can do with this pane")
        // sugar form — a label and an optional icon
        .child(MenuItem::new().label("Rename").icon(Glyph::Pencil).on_click(move || rename(id)))
        // composed form — any widget subtree, with props
        .child(MenuItem::new().child(|| {
            Grid::new().prop("gap", 10)
                .child(Icon::new(Glyph::Trash))
                .child(Label::new("Close"))
        })),
);

Row::new().child(Label::new(&pane.name)).context_menu(ctx);
```

**There is no row identity to declare, no path string, no menu id to register and no builder
registry**: the closure captured `id` in the loop that was already drawing that row. That is the
whole point of declaring the menu on the widget.

Getting a menu onto a row used to take **four** things, three of them invisible: a `context_path`
mapping a row key to a menu-id string, a builder registered for that id, the items, and — the one
nobody would think of — a `.key(..)` on the row (spelled `nav_key` then), because the host resolved
*what did you right-click* from a **position** and read the answer off that slot. A workspace header had the
first three and not the fourth: right-clicking it opened **nothing**, with no error and no failing
test. The design was the bug.

#### The two forms of a row, and why `child` takes a closure

`MenuItem::label` / `MenuItem::icon` are **sugar** for the row nearly every menu wants.
`MenuItem::child` takes any widget subtree instead, and **children win when both are given** — the
same precedence `Button` has.

`child` takes a `Fn() -> impl Component` rather than a widget **value** because a menu can be shown
more than once, and a widget subtree is owned (`Box<dyn Component>`): handed over once, it is gone.
A *builder* can run again, which is what makes the whole chain — `MenuItem`, `Menu`, `ContextMenu` —
`Clone`, so one menu value can be declared on several rows and captured by handlers. It also means
the rows are built **at the moment the menu opens**, so `enabled(is_custom_name)` is an answer about
the state you are opening it in rather than the state it was written in.

The rows are **real children**, laid out by the same engine as everything else — `gap`, `grow`,
padding and font inheritance all work inside a row, and this widget contains no layout code of its
own beyond placing the finished panel at its anchor (the same `shift_subtree` trick `Overlay` and
`Select` use).

A row's colour is decided at **paint**, from the theme and the row's state, and published to the
subtree with `PaintCx::with_content_color` — which is what lets a composed row (an `Icon` and a
`Label` an author wrote) read as `danger`, or dim when disabled, without knowing anything about
menus. The quick-pick keycap and the textual shortcut are the *menu's* affordances rather than the
row's content, so the panel paints them into space the row reserved, and a composed row never has to
lay them out.

#### Declaring it: a value or a closure

`ComponentExt::context_menu` accepts either, via `IntoContextMenu`:

```rust
Row::new().context_menu(ctx.clone());            // a value — `ContextMenu` is `Clone`
Row::new().context_menu(move || build_menu(id)); // a closure — rows read state at open time
```

Universal, like `key`: an `Icon`, a `Label` and a plugin's own widget carry one on the same
terms as a `Row`, because the declaration lives on `Base`.

#### Two roads to show a menu

```rust
// 1. Declared on the widget — covers the two standard triggers.
Row::new().context_menu(ctx);

// 2. Shown from a handler — for a trigger you invent.
Button::new("More").on_click(move |ev| ctx.show(ev));
```

The declaration exists **because the keyboard needs it**: with the menu only inside a closure,
`prefix+>` / `Shift+F10` has nothing to find. Both roads run the same code — the declaration is
implemented in terms of `ContextMenu::show`.

#### Triggers, anchors, bubbling

| Trigger | Target | Anchor |
|---|---|---|
| `Event::RightClick` | the widget under the pointer | the pointer |
| the host's `open_context_menu` action (`prefix+>`) | the focused widget | under the widget |

**The anchor is read out of the event**, never chosen by an author: `Event::position()` gives the
cursor, `Event::target_bounds()` gives the widget the event was delivered to (the router stamps it
once, at delivery). An event with neither shows nothing rather than guessing a corner of the screen.

Both triggers **bubble to the nearest ancestor that declares a menu**: you right-click the `Label`
inside a row, not the row; focus sits on a cell, and the menu belongs to the row. Bubbling stops at
the first declaring ancestor — **menus are never merged**, because a menu is a statement about one
thing. Nothing in the chain declares one ⇒ nothing opens, with no hidden fallback.

A host that wants **"right-click empty space"** puts a menu on the **root**, which needs no
empty-space hit-test: a click that lands between rows or below the last one simply finds nothing on
the way down and bubbles out to it.

> ⚠️ **A right-click needs a press *and* a release.** `RightClick` is synthesised from the pair on
> the same widget, so a host that delivers only presses produces no clicks at all and **no declared
> menu ever opens**. heca lints this in `heca/tests/pointer_funnel.rs`; it is not visible to
> behaviour tests, which dispatch both halves themselves.

#### The one host dependency

A menu opens above everything, which is a *layer*, and a widget cannot reach one. So the host
installs a sink once at startup — the same shape as `install_frame_request` — and every show posts
to it:

```rust
heca_grid_ui::install_menu_sink(move |menu, anchor| {
    // The menu arrives with its anchor; mount it as a layer.
    queue.borrow_mut().push((menu, anchor));
});
```

No `AppState`, no host type, in any closure a widget holds. `ContextMenu::after_select` lets the
host take the layer down after an entry ran, so an item stays a plain closure that knows nothing
about layers — requiring every author to close the menu they opened is a rule that gets forgotten
exactly once per menu.

#### Contributions — the plugin surface

`Menu::name("workspaces.pane")` is **optional**, and it is the only thing left of the contribution
design: a named menu is one a host can offer to everything mounted before it is shown, so a plugin's
"Open in container" can appear on a row it does not own. A menu without a name is closed and needs
nothing.

A contributed row has **no per-row payload**, so it acts on app state (the focused pane, the selected
row) rather than on the row the menu was opened for.

**A plugin never writes a closure.** It registers an action and contributes a row that names it; the
host turns those into `MenuItem`s whose behaviour is an `Intent` dispatched through the central gate,
so a menu entry gets exactly the policy and destructive-confirm a keypress or an RPC call gets. An
entry's **icon comes from the action** (`ActionCatalog::icon`), which is why the sidebar's "Close
pane" and the command palette's `close` cannot drift apart.

#### Behaviour and nav

- **Nav (host-driven, configurable)**: **no hardcoded nav keys** — as a vertical list the menu
  responds to `Event::Widget(WidgetIntent::{MenuUp,MenuDown,Activate,Dismiss})`, which the host
  resolves from the configurable `menu_up` / `menu_down` / `activate` / `dismiss` `[keys.widgets]`
  bindings (defaults ↑/Ctrl+k, ↓/Ctrl+j, Enter, Esc). Raw `Event::Key` is only a **quick-pick
  letter** that runs its entry directly. Hover highlights; click runs; outside-click dismisses.
- **`on_dismiss`** fires on **Esc / outside-click** — a *dismissal*, not a selection. Selecting an
  entry runs its `on_click` and then `after_select`.
- **Occlusion**: deliberately keeps the default `overlay_occludes` = `false` even while open, so a
  second right-click **re-anchors** the menu at the new point (the standard menu affordance).


### ToastStack

An overlay that arranges a **host-supplied** set of notifications into a corner stack. **Presentation
only** — it owns no queue, lifetimes, auto-dismiss timers, or dedup; that's the app's job. The host
owns a `Signal<Vec<ToastSpec>>` (its render list); the stack reconciles cached [`Toast`](#toast)
widgets by **id** (each keeps its hover/flash state), corner-anchors them on the overlay layer,
slides new ones in, routes events to the toast under the cursor, and reports
`on_dismiss(id)`/`on_action(id)` back — the host then removes the id (which reflows the rest). It is
overlay-active only while it has toasts, and **passes through** what misses every toast.
Its `overlay_occludes(pos)` reports the **cards'** rects (not the whole corner), so a host gate
like "right-click opens the page menu" skips points a toast covers while staying live elsewhere
(see [`Component` trait](#component-trait)).

**A card occludes hover too.** A pointer *move* over a card is consumed, so controls behind it stop
lighting up while a toast is over them; moves between and outside the cards still fall through, and
a button next to the stack keeps hovering as it always did.

**The cards are real children, and the engine lays them out.** The stack is a viewport-sized box
whose `justify`/`align` come from its corner, with its gap between the cards and its margin as
padding — it measures, positions, hit-tests and routes nothing. That is what makes a notification's
action **reachable by keyboard at all**: the picker walks the laid-out tree, so a card kept beside
it in a private list could never be lettered, and hover never reached inside one (the framework
marks hover along the hit-test target's ancestor chain, and a hand-delivered move marks nothing).

**A corner means the window's corner.** A host mounts its layers however it likes — a column of
viewport-sized siblings gives each one a *share* of the height — so the stack anchors the finished
group against the viewport the layout pass stamped on it, in `on_layout`. The arrangement is still
entirely the engine's; only which corner meets which corner is left.

- **Construct**: `ToastStack::new(items: Signal<Vec<ToastSpec>>)`; `.position(ToastPosition)`,
  `.gap(px)`, `.margin(px)`.
- **Intents**: `.on_dismiss(|id| …)` (× clicked), `.on_action(|id, key| …)` (an action clicked —
  `key` is that action's own name, because a card may offer several and "which card" alone would
  not say what to do).
- **`ToastSpec`**: `ToastSpec::new(id, title).severity(..).icon(..)?.body(..)?.action(key, label)*.dismissible(bool)`
  — plain data the host owns, with **no closures**: an action is a `key` the host maps back, which
  is the only form that also survives an RPC call or a plugin.
- **Several actions, each a real control.** `actions: Vec<ToastAction>` where
  `ToastAction { key, label, variant }`. `.action(key, label)` appends one in the default variant;
  `.action_with(ToastAction::new(..).variant(..))` is the full form and the sugar builds it, so
  there is one path and not two. A notification that can be retried *and* inspected needs two, and
  "one action" was a widget limit rather than a real rule.
- **The stack never chooses how an action looks.** The variant rides on each `ToastAction` (default
  `Primary`), so a card can carry a primary "Retry" beside a ghost "Dismiss". One face hardcoded in
  the widget is wrong for somebody every time.
- **One position vocabulary for the card and the stack.** [`ToastPosition`](#toast) places both —
  `TopRight`/`TopLeft`/`TopCenter`/`BottomRight`/`BottomLeft`/`BottomCenter` — so "top right" means
  the same thing said either way. It replaced a separate four-member `ToastCorner`, which said the
  same thing in a second spelling and could not express the centres.
- **A dropped card leaves before it goes.** Remove its id and the stack plays that card's exit,
  keeping it in place until the gesture has finished — so a notification is never cut off
  mid-dismissal.

```rust
let toasts = signal(Vec::<ToastSpec>::new());            // the app's render list
let stack = ToastStack::new(toasts)
    .position(ToastPosition::TopRight)
    .on_dismiss(move |id| toasts.update(|v| v.retain(|s| s.id != id)))
    .on_action(move |id, key| run(id, key));             // `key` says WHICH action

// The app pushes plain data — two actions, each with its own face:
toasts.update(|v| v.push(
    ToastSpec::new(1, "Build failed")
        .severity(ToastSeverity::Danger)
        .body("3 errors in heca-grid-ui")
        .action("rebuild", "Retry")
        .action_with(ToastAction::new("open_log", "View log").variant(ButtonVariant::Ghost)),
));
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
| `kind` | which widget (`WidgetKind`: `VStack`/`HStack`/`Row`/`Label`/`Button`/`Input`/…) |
| `props` | this node's **own** values (`name → PropValue`) — **per node, not inherited** |
| `events` | this node's **own** `event → Intent` bindings (`press` / `change`) — an action **id**, never a closure (keeps it serializable) |
| `children` | a **`Vec<ViewNode>`**, each a full node with its *own* props/events/children |

**Props are per-node.** `.prop("gap", …)` on a `Column` styles *the column*, not its children — the
props sitting next to `.child(…)` calls belong to the node you called `.prop` on (the container). A
child is styled by putting props on *that child*. The builder chains for ergonomics but children are
a plain vector: `.child(n)` appends one, `.children([a,b])` appends many — `Column().child(a).child(b)`
≡ `Column().children([a,b])`.

### Style props — every kind, no list

**Any field of [`Style`](#style--layout-enums) is a prop on any kind**, spelled exactly as the
field is — both halves:

- **Layout**: `padding`, `margin` (+ per-side), `gap`, `gap_spacing`, `align`, `align_self`,
  `justify`, `justify_items`, `justify_self`, `direction`, `width`, `height`, min/max sizes,
  `flex_grow`, `flex_shrink`, `hidden`, `grid_cell`, `size`.
- **Appearance**: `fill`, `border`, `glow`, `radius`, `font_size`, `font_scale`.

`realize` never enumerates them — it merges by name against each half's own fields. Add a field to
either and a description can set it with **no change to the mapper**.

#### Appearance: the theme is the default, not a wall

Appearance is an ordinary property, on every widget. `Visual` used to be deliberately
unserializable, putting it out of a description's reach by construction; it no longer is.

**Set nothing and you follow the theme** — which is what most widgets should do. That is not a new
behaviour bolted on: `fill` / `border` / `glow` are `Option`, and `radius` / `font_size` use a
`0.0 = inherit` sentinel, so "unset" already meant "ask the theme" at paint time. A widget that
overrides nothing is unaffected by any of this.

A **colour** is written two ways, and the difference matters:

```rust
.prop("fill", PropValue::Color("accent".into()))     // a theme token — PREFER THIS
.prop("fill", PropValue::Color("#ff8800".into()))    // a literal (also "#ff8800cc")
```

A **token name** is one of the theme's own colour fields (`accent`, `foreground`, `muted`,
`border`, `danger`, `warning`, `success`, …). The accepted vocabulary *is* that field list — add a
colour to the theme and a description can name it, with no table to update anywhere. It resolves
against the theme the tree is built with, and a theme reload rebuilds every tree, so a token-named
override **follows the new theme**.

A **hex literal** is exactly the colour it says and does not track the theme. That is the trade you
make by writing one.

A colour that is neither — a misspelled token — is dropped like any other bad value: that one
property is skipped and its neighbours on the same node still apply.

This reaches a **widget's own colour builders** too, not only the style halves: `Label::color`,
`Icon::color`, `Tag`'s hue, `Row::highlight` and `Row::attention_color` are all ordinary props now.
The host resolves a token to hex before the value crosses into the library, so `heca-grid-ui` still
knows nothing about themes and `Color::from_str` still only knows hex.

**The first override in heca** is the destructive confirm prompt: its message ("This action cannot
be undone.") is written with `color: "danger"` — the token, not a literal — so it follows the active
theme. See `open_confirm` in `heca/src/handlers.rs`.

Values read the way you would write them:

```rust
ViewNode::new(WidgetKind::VStack)
    .prop("padding", PropValue::Int(12))                     // px
    .prop("width",   PropValue::Text("50%".into()))          // "auto" | 240 | "50%"
    .prop("justify", PropValue::Text("space_between".into())) // enums by name, snake_case
    .prop("gap_spacing", PropValue::Text("md".into()))        // theme token, scales with the font
```

The merge lands **on top of** the constructed widget, so a widget's own constructor settings survive
any property the node doesn't mention — a `Scroll` keeps the zeroed min-sizes and shrink factor that
let a viewport be smaller than its content.

#### Sizes: the three spellings, and the one number written two ways

A size — `width`, `height`, `min_*`, `max_*`, and the per-side margins — accepts exactly three
forms, the same ones CSS does:

| written | means |
|---|---|
| `240` / `"240"` / `"240px"` | logical pixels |
| `"50%"` | **a fraction of the parent** |
| `"auto"` | sized by content and flex rules |
| *nothing at all* | **fills the parent across the cross axis** — CSS `align-items: stretch` |

There is **no second vocabulary for plugins**: the same `Length` deserializer reads a described
tree, an RPC message and a config file, so what an author writes is what the app's own code gets.

⚠️ **`"50"` is fifty pixels, not half** — exactly as in CSS. The `%` is what makes it a fraction.

⚠️ **The same value is spelled two ways on purpose.** On the wire it is `"50%"`, because that is
what an author writes; in Rust it is `Length::Percent(0.5)`, because that is the fraction taffy
takes underneath. One translator does the conversion — `Length`'s `Deserialize`/`Serialize` in
`heca-grid-ui/src/style.rs` — and it round-trips, which is what the layout merge relies on. So a
plugin never sees `0.5` and native code never sees `"50%"`, and neither has to know the other
spelling exists.

Through the typed SDK the same thing is `width_pct(0.5)`, which emits `"50%"` for you
(`a_percentage_width_is_written_the_way_length_reads_it`); that a fraction really lands at half the
parent is held by `a_described_node_stretches_to_its_parent_like_css`.

### Identity props — `key` and `hintable`, on every kind

Two more props are read for **every** kind, for the same reason the style halves are: they live on
`Base` and every widget has them, so they belong to no widget's builder surface.

| Prop | Type | Meaning |
|---|---|---|
| `key` | `Text` | **This node's identity**, when it is one of a collection you are iterating — see [Identity](#builder-traits). Written into `Base::key`, the same slot a native `.key(..)` writes. |
| `hintable` | `Bool` | `false` keeps the node **out of the picker**, however actionable it is. Default `true`. |

```rust
// Typed builder (heca_view::build):
Row::new().key(pane.id).hintable(false).on_press(Intent::new("focus_pane"))

// Raw node:
ViewNode::new(WidgetKind::Row)
    .prop("key", PropValue::Text("pane:7".into()))
    .prop("hintable", PropValue::Bool(false))
    .on_press(Intent::new("focus_pane"));
```

`key` is required **in a collection and nowhere else**, and a description that breaks that is
reported before it is realized — see
[Forcing a `key`](#forcing-a-key--a-warning-not-a-type). `heca_view::unkeyed_collection_items`
is the check, and it lives in `heca-view`, which compiles without anything that draws, so a plugin's
own build can run it against its own tree.

### Widget props — the builders decide, not a list

`realize` names no widget property. Each widget **generates** its property surface from its own
builders, so what a description can set is decided in one place — the widget:

```rust
#[props]
impl Input {
    #[prop] pub fn placeholder(mut self, s: impl Into<String>) -> Self { .. }
    #[host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self { .. }
}
```

Every builder must be one or the other. A builder that is neither **fails the build** — being left
out silently is exactly how `Input::placeholder` and `ScrollRegion`'s second axis stayed unreachable
for months. A drift guard additionally fails when a whole widget has no surface.

**Order never matters.** Properties are applied after children are attached, so a builder that
clamps against its children (`Select`/`Tabs` `selected`) sees the real ones. `#[prop]` rejects any
argument, so a sequencing hint cannot be reintroduced widget by widget.

Deliberately not properties, each with its reason recorded in the code: closures (behaviour crosses
as an `Intent`), composed content (use `children`), and builders bound to live host signals.
Appearance is in that group today and is moving out.

### Props & events by kind

The table below is a **reader's summary** — the widget's builders are the authority. These are the
props a kind reads in addition to the layout set above.

Missing/mistyped props are ignored (the widget keeps its default) — the model is untrusted input,
and a bad value costs only itself: the good props on the same node still apply.

> **A `Button`'s children are its content, and they win.** A Button node *with* children is realized
> as an empty button holding them (an arbitrary tree, any depth); a **childless** node falls back to
> its scalar sugar — `text` → a bold `Label`, `icon` → a leading `Icon` — which builds the very same
> children. See [Button → the two ways to build one](#the-two-ways-to-build-a-button--same-widget-same-retained-tree).
> A widget with **several places** for children (an `Item`'s leading/trailing, a `DockFrame`'s
> header) takes a `slot` prop on the child — see
> [Named child slots](#named-child-slots-the-slot-prop).

| Kind | Props it reads | Events |
|------|----------------|--------|
| `VStack` / `HStack` | (layout only — see above) — the plain boxes | — |
| `Row` | `active`, `nav_selected`, `marker` (`bar` \| `check` \| `none`) + children | `press` |
| `Card` | `text` (title) + children | — |
| `Surface` | (container — children only) | — |
| `Panel` | `text` (the heading; omit it and no header row is drawn) + children | — |
| `Scroll` | `axes` (`vertical` / `horizontal` / `both`, default vertical) + children | — |
| **`Overlay`** | `blocking` (Bool, default `true`), `frosted` (Bool, default `false` — blur what is behind it, at the theme's radius), `open` (Bool), `animation` (`none` / `fade` / `zoom` / `zoom_fade`), **+ children** = the panel (one child *is* the panel; several are stacked into one) | — (dismissal is the host's) |
| `Label` | `text`, `bold`, `italic`, `underline`, `strikethrough` (Bool) | — |
| `Badge` / `Tag` / `Alert` | `text` | — |
| **`Button`** | `variant`, `size`, **+ children** (the content); `text`, `icon` = the **childless sugar** | `press` |
| `BadgeButton` | `text`, `variant`, `size` | `press` |
| `Icon` / `IconButton` / `RailCell` | `icon` (Glyph **name**), `size` | `press` (button/rail) |
| `Input` | `text` (the **value**), `placeholder`, `name` | `change` |
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
ViewNode::new(WidgetKind::VStack)
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
  (`VStack::new().gap(8).child(…)`) is `plugin-task-ui-2`; until then use `ViewNode::new(kind)`.
- **Extending the vocabulary is host-side** (never a plugin): add a `WidgetKind` variant + a
  `realize` arm + the widget's showcase demo + its entry here. Plugins compose from existing kinds.

#### Arguments — what an `Intent` may carry

An argument is added with `.arg(name, PropValue)`, and the value is an **explicit `PropValue`**
rather than anything convertible. That is deliberate: an argument crosses to RPC and to a WASM
plugin as data, so what it *is* should be readable at the call site instead of inferred from
whatever numeric type happened to be in scope.

```rust
Intent::new("focus_pane").arg("pane_id", PropValue::Int(7));
Intent::new("rename").arg("name", PropValue::Text("scratch".into()));
Intent::new("expand").arg("open", PropValue::Bool(true));
```

⚠️ **Integers are `i64`, and most ids here are `u64`.** `PropValue::Int` is an `i64`, because that
is what JSON and the RPC wire carry. A pane id, a notification id and `ToastSpec::id` are all
`u64`, and there is deliberately **no `From<u64>`** — the conversion is lossy above `i64::MAX`, and
a silently wrapped id would arrive as a *negative* number that nothing could diagnose from the UI.
So cast at the call site, where the choice is visible:

```rust
Intent::new("focus_pane").arg("pane_id", PropValue::Int(pane_id as i64));
```

An intent may carry anything `PropValue` holds — `Bool`, `Int`, `Float`, `Text`, a colour or glyph
**name**, a `List`, or a `Map` for a named group of values. It may **never** carry a closure or a
widget: an intent is the one form behaviour takes when it has to survive being sent by RPC, named
in a keybinding, or raised by a plugin.

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

**A wrong argument is never silent.** Every action declares what it takes, so a call is compared
against that declaration before it is built — from a widget, from config, from a plugin, from RPC:

```
[heca] action 'chrome.container.move_to_region': unknown argument 'contaner_id' — did you mean 'container_id'?
[heca] action 'chrome.container.move_to_region': missing required argument 'container_id'
```

A missing required argument stops the action; there is nothing to build. An unknown or malformed one
costs only itself and the rest of the call still stands — the same rule a widget property follows.
`describe-action <name>` (below) is how you find out what an action takes without reading its source.

### Registering a custom (name-keyed) action

`WmAction` is a **closed enum** — a provider or plugin cannot add a variant to it. An action of your
own is therefore keyed by a **stable string id** and registered at runtime, after which every surface
treats it like a built-in: it has a label and an icon, it shows up in menus and introspection, it can
be bound in `config.toml`, and it is judged by the same interaction policy.

Metadata and handler live in two places for a borrow reason, not a design one: an action handler is
`fn(&mut AppState, …)` and gets no registry, so **metadata** must be reachable from `AppState` (the `ActionCatalog`) while the **handler** table must be borrowable alongside `&mut AppState` (the `ActionRegistry`). One call registers both.

```rust
use crate::actions::{
    register_dynamic, unregister_dynamic, ActionCategory, ActionMeta, ArgKind, ArgSpec,
};
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
        // REQUIRED too, and for the same reason: an omitted list would read as "takes nothing",
        // and every argument the handler reads below would be one nobody declared. Declaring it
        // is what lets heca tell a caller that `containr` is not `container`.
        args: vec![ArgSpec {
            name: "container".into(),
            kind: ArgKind::Text,
            required: true,
            description: "Id of the container to restart.".into(),
            values: Vec::new(),                // only an `ArgKind::Enum` fills this
        }],
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
key**, a separate field so one spec can govern several `WmAction` variants — `ClosePane` and
`ClosePaneById` share heca's `close` spec. No built-in needs a name of its own, so `[confirm] close =
false` disables the pane-close prompt.

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
list-actions              → [ {name,label,description,category,default_binding,policy,args,confirm}, … ]
describe-action <name>    → one such object, or an error if the name is unknown
```

`confirm` is the toggle key when the action prompts, `null` otherwise; `policy` is the interaction
policy as a stable string (`global`, `tiled_only`, …). A native `Callback` outcome is never
serialized — introspection reports only *that* a prompt exists. In Rust: `ActionCatalog::describe_all()`
/ `describe(name)` → `ActionInfo`.

`args` is what the action takes, so a caller can learn how to *call* what it just discovered — not
only that the name exists:

```json
{ "name": "resize",
  "args": [
    { "name": "target", "kind": "enum", "required": true,
      "description": "What to resize.", "values": ["column", "col", "pane"] },
    { "name": "axis",   "kind": "enum", "required": true,
      "description": "Which axis to resize along.",
      "values": ["x", "horizontal", "width", "y", "vertical", "height"] },
    { "name": "amount", "kind": "float", "required": true,
      "description": "How much to grow by; negative shrinks." }
  ] }
```

`kind` is one of `int`, `float`, `bool`, `text`, `enum`; `values` appears only for `enum` and lists
every spelling accepted, aliases included. An empty `args` means the action takes none — never "not
stated": heca compares every call against this list and reports an argument that is unknown, missing
or malformed, so a plugin or a script gets told what it got wrong.

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

**Note what the recipe does and does not do:** the second row is added to the column
**unconditionally** and hides itself. Do not turn that into `if has_branch { … }` — see
[the rule on `Visibility`](#visibility). A metadata line that is only attached once it already has
something to say can never be revealed when the answer arrives later, and any inset you wrap
*around* it keeps its box and its gap when the line hides. Put the inset inside the row, as
padding.

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

#### `at_rect` — a rect **inside** a parent, in fractions of it

The margin form above places a **root**. Inside a tree it is the wrong tool twice over: a
margin still takes part in the flow, so the box pushes its siblings along; and a *percentage*
margin resolves against the parent's **width on both axes** (CSS's rule, which taffy
implements faithfully), so a fractional `top` silently produces a number — just the wrong one
— on any parent that is not square.

`LayoutExt::at_rect(left, top, width, height)` is the answer, on every widget:

```rust
// A floating pane drawn over the workspace strip behind it, at its own fraction of it.
strip = strip.child(card.at_rect(
    Length::Percent(f.x / strip_w),      // ← left and width resolve against the parent's WIDTH
    Length::Percent(f.y / screen_h),     // ← top and height against its HEIGHT
    Length::Percent(f.w / strip_w),
    Length::Percent(f.h / screen_h),
));
```

- **It is out of the flow.** The box takes no space from its siblings and is not moved by
  them, so it draws *over* what it is placed on. Later children paint above earlier ones, so
  declare it after what it covers.
- **Each percentage resolves against its own axis** — the difference from a margin, and the
  whole reason this exists.
- **The rect overrides `width`/`height`**: it names both, and a leftover size beside it would
  draw a different rect than the one asked for.
- Units mix freely: a fixed `Px` chip at a proportional `Percent` position is as valid as a fully
  fractional rect.

Declarative form — a plugin authors the same thing as a grouped property, because a rect is
four values and a scalar channel could not carry it:

```rust
ViewNode::new(WidgetKind::Card).prop(
    "placement",
    PropValue::Map([
        ("left".into(),   PropValue::Text("25%".into())),
        ("top".into(),    PropValue::Text("10%".into())),
        ("width".into(),  PropValue::Text("50%".into())),
        ("height".into(), PropValue::Int(120)),        // a bare number is pixels
    ].into_iter().collect()),
)
```

Held by `heca-grid-ui/src/layout.rs` (`a_fractional_rect_resolves_each_percentage_against_its_own_axis`,
`a_placed_box_takes_no_space_from_its_siblings`, and the margin-axis guard that explains why)
and by `heca-view-realize` (`a_description_can_place_a_node_at_a_fractional_rect`).

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

Embed `Base`, implement `Component` (override `paint` and, for input,
[`on_event_capture`/`on_event`](#the-event-model--the-framework-resolves-the-pointer-and-walks-the-tree) — **never a
child walk**, `dispatch` does that — plus `tick` for animation and `remeasure` if the widget's size
depends on the font, read from `self.base.font`), and opt into builder traits. Reuse `PaintCx` helpers (`rect`, `focus_ring`, `bracket_frame`, `text`, `flash`, `dim`)
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
