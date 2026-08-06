//! Builder-method traits that encode the **layout vs. surface** separation.
//!
//! - [`LayoutExt`] — arrangement; for any container, including layout-only `Flex`.
//! - [`StyleExt`] — visual decoration; for **surface** components only (`Surface`,
//!   `Card`, `Button`). Deliberately *not* implemented for `Flex`, so a pure
//!   layout container cannot be turned into a styled surface.
//! - [`Parent`] — holds children.
//!
//! Default methods mutate `base_mut().style`, so a widget opts into a set of
//! builders just by writing `impl LayoutExt for MyWidget {}` (zero boilerplate).

use crate::color::Color;
use crate::component::Component;
use crate::drag::DragItemId;
use crate::reactive::SignalUpdate;
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Justify, Length, WidgetSize};

/// Arrangement builders: how a container lays out itself and its children.
pub trait LayoutExt: Component + Sized {
    /// Size variant — scales the widget's font and intrinsic padding together
    /// (`Small`/`Normal`/`Big`). Available on every widget; controls honor it in
    /// their `remeasure`, and text simply inherits the scaled font.
    ///
    /// The variant **cascades to composed content**: children that don't set their own inherit
    /// it during layout, so `Button::new("Save").icon(Glyph::Check).size(WidgetSize::Small)`
    /// shrinks the button *and* its `Icon`/`Label`. Setting it here marks it explicit
    /// ([`Style::size_explicit`](crate::style::Style::size_explicit)), which both pins this
    /// widget's variant and makes **it** the one its own children inherit.
    fn size(mut self, size: WidgetSize) -> Self {
        self.base_mut().style.layout.set_size(size);
        self
    }
    /// Main-axis direction.
    fn direction(mut self, d: Direction) -> Self {
        self.base_mut().style.layout.direction = d;
        self
    }
    /// Gap between children.
    fn gap(mut self, v: f32) -> Self {
        self.base_mut().style.layout.gap = v;
        self
    }
    /// Gap between children from a theme [`Spacing`](crate::style::Spacing) token —
    /// resolved to px from the inherited font at layout, so it scales with the
    /// font, size variant and UI zoom (unlike a raw [`gap`](LayoutExt::gap) px).
    /// Use it to group form fields: a tight `Spacing::Xs` inside a label+control
    /// couple, a roomier `Spacing::Md` between couples — no new widget needed.
    fn gap_spacing(mut self, s: crate::style::Spacing) -> Self {
        self.base_mut().style.layout.gap_spacing = Some(s);
        self
    }
    /// Outer margin on all sides.
    fn margin(mut self, m: f32) -> Self {
        self.base_mut().style.layout.margin = m;
        self
    }
    /// Outer margin split per axis: `x` left+right, `y` top+bottom.
    fn margin_xy(mut self, x: f32, y: f32) -> Self {
        let s = &mut self.base_mut().style.layout;
        s.margin_left = Some(x);
        s.margin_right = Some(x);
        s.margin_top = Some(y);
        s.margin_bottom = Some(y);
        self
    }
    /// Horizontal outer margin (left+right) only.
    fn margin_x(mut self, v: f32) -> Self {
        self.base_mut().style.layout.margin_x = Some(v);
        self
    }
    /// Vertical outer margin (top+bottom) only — a rule breathing away from what it separates.
    fn margin_y(mut self, v: f32) -> Self {
        self.base_mut().style.layout.margin_y = Some(v);
        self
    }
    /// Left outer margin only.
    fn margin_left(mut self, v: f32) -> Self {
        self.base_mut().style.layout.margin_left = Some(v);
        self
    }
    /// Right outer margin only.
    fn margin_right(mut self, v: f32) -> Self {
        self.base_mut().style.layout.margin_right = Some(v);
        self
    }
    /// Top outer margin only.
    fn margin_top(mut self, v: f32) -> Self {
        self.base_mut().style.layout.margin_top = Some(v);
        self
    }
    /// Bottom outer margin only.
    fn margin_bottom(mut self, v: f32) -> Self {
        self.base_mut().style.layout.margin_bottom = Some(v);
        self
    }
    /// Main-axis distribution.
    fn justify(mut self, j: Justify) -> Self {
        self.base_mut().style.layout.justify = j;
        self
    }
    /// Cross-axis alignment of this component's **children**.
    fn align(mut self, a: Align) -> Self {
        self.base_mut().style.layout.align = a;
        self
    }
    /// Cross-axis alignment of **this** component inside its parent (CSS `align-self`),
    /// overriding the parent's [`align`](Self::align) for it alone. Use
    /// [`Align::Start`] to keep an `Auto`-sized widget hugging its content instead of
    /// stretching to fill the parent.
    fn align_self(mut self, a: Align) -> Self {
        self.base_mut().style.layout.align_self = Some(a);
        self
    }
    /// **Grid only** — how this grid's items sit **horizontally inside their cells**
    /// (CSS `justify-items`). Default: `Stretch` (an item fills its cell).
    ///
    /// Not to be confused with [`justify`](Self::justify): on a grid that is `justify-content`,
    /// which distributes the whole *track set* inside the container and leaves the items where
    /// they are. The vertical counterpart is [`align`](Self::align).
    fn justify_items(mut self, a: Align) -> Self {
        self.base_mut().style.layout.justify_items = Some(a);
        self
    }
    /// **Grid only** — horizontal placement of **this** item inside its own cell (CSS
    /// `justify-self`), overriding the grid's [`justify_items`](Self::justify_items) for it alone.
    fn justify_self(mut self, a: Align) -> Self {
        self.base_mut().style.layout.justify_self = Some(a);
        self
    }
    /// Inner padding on all sides.
    fn padding(mut self, p: f32) -> Self {
        self.base_mut().style.layout.padding = p;
        self
    }
    /// Inner padding split per axis: `x` left+right, `y` top+bottom.
    fn padding_xy(mut self, x: f32, y: f32) -> Self {
        let s = &mut self.base_mut().style.layout;
        s.padding_x = Some(x);
        s.padding_y = Some(y);
        self
    }
    /// Inner padding on one side, overriding the axis and the uniform value.
    ///
    /// Reserving space along a single edge is not the same as padding the axis: the opposite side
    /// should not move because this one needed room. A `ScrollRegion` keeping its content clear of
    /// its scrollbar is the case that asked for it.
    fn padding_left(mut self, p: f32) -> Self {
        self.base_mut().style.layout.padding_left = Some(p);
        self
    }
    /// Inner padding on the right only — see [`padding_left`](Self::padding_left).
    fn padding_right(mut self, p: f32) -> Self {
        self.base_mut().style.layout.padding_right = Some(p);
        self
    }
    /// Inner padding on the top only — see [`padding_left`](Self::padding_left).
    fn padding_top(mut self, p: f32) -> Self {
        self.base_mut().style.layout.padding_top = Some(p);
        self
    }
    /// Inner padding on the bottom only — see [`padding_left`](Self::padding_left).
    fn padding_bottom(mut self, p: f32) -> Self {
        self.base_mut().style.layout.padding_bottom = Some(p);
        self
    }
    /// Inner padding (both axes) from a theme [`Spacing`](crate::style::Spacing) token —
    /// resolved to px from the font at layout. Prefer this over hand-computed px.
    fn pad_all(mut self, s: crate::style::Spacing) -> Self {
        let st = &mut self.base_mut().style.layout;
        st.pad_spacing_x = Some(s);
        st.pad_spacing_y = Some(s);
        self
    }
    /// Horizontal (left+right) padding from a theme [`Spacing`](crate::style::Spacing) token.
    fn pad_x(mut self, s: crate::style::Spacing) -> Self {
        self.base_mut().style.layout.pad_spacing_x = Some(s);
        self
    }
    /// Vertical (top+bottom) padding from a theme [`Spacing`](crate::style::Spacing) token.
    fn pad_y(mut self, s: crate::style::Spacing) -> Self {
        self.base_mut().style.layout.pad_spacing_y = Some(s);
        self
    }
    /// Width along the main/cross axis.
    fn width(mut self, w: Length) -> Self {
        self.base_mut().style.layout.width = w;
        self
    }
    /// Height along the main/cross axis.
    fn height(mut self, h: Length) -> Self {
        self.base_mut().style.layout.height = h;
        self
    }
    /// Flex grow factor (share of remaining space).
    /// Permission to **shrink** below the natural size, the other half of a share.
    ///
    /// `flex_grow` distributes only *positive* free space, so growing alone never divides a region:
    /// a child keeps its content size and the row overflows. A true share is grow + a zero base
    /// size + this (CSS `flex: 1 1 0`).
    fn shrink(mut self, s: f32) -> Self {
        self.base_mut().style.layout.flex_shrink = Some(s);
        self
    }

    /// Floor for the height — a row that must stay legible however many share the space.
    fn min_height(mut self, h: Length) -> Self {
        self.base_mut().style.layout.min_height = Some(h);
        self
    }

    /// Floor for the width.
    fn min_width(mut self, w: Length) -> Self {
        self.base_mut().style.layout.min_width = Some(w);
        self
    }

    fn grow(mut self, g: f32) -> Self {
        self.base_mut().style.layout.flex_grow = g;
        self
    }
    /// Disable the widget: dimmed, non-interactive, skipped by focus traversal.
    fn disabled(mut self, disabled: bool) -> Self {
        self.base_mut().disabled.set(disabled);
        self
    }
    /// Explicit Tab-order index (like HTML `tabindex`): indexed widgets are
    /// visited first in ascending order, then unindexed ones in tree position.
    fn tab_index(mut self, index: i32) -> Self {
        self.base_mut().tab_index = Some(index);
        self
    }
}

/// Visual decoration builders — **surfaces only**.
pub trait StyleExt: Component + Sized {
    /// Background fill.
    fn background(mut self, c: Color) -> Self {
        self.base_mut().style.visual.fill = Some(c);
        self
    }
    /// Border outline.
    fn border(mut self, c: Color, width: f32) -> Self {
        self.base_mut().style.visual.border = Some(Border { color: c, width });
        self
    }
    /// Neon outer glow (default radius/intensity).
    fn glow(mut self, c: Color) -> Self {
        self.base_mut().style.visual.glow = Some(Glow {
            color: c,
            radius: 8.0,
            intensity: 1.0,
        });
        self
    }
    /// Neon outer glow with explicit falloff radius and intensity.
    fn glow_with(mut self, c: Color, radius: f32, intensity: f32) -> Self {
        self.base_mut().style.visual.glow = Some(Glow {
            color: c,
            radius,
            intensity,
        });
        self
    }
    /// Corner radius.
    fn radius(mut self, r: f32) -> Self {
        self.base_mut().style.visual.radius = r;
        self
    }

    /// Semantic font multiplier relative to the inherited base font (e.g. `2.0`
    /// for a header, `0.8` for a caption). Scales with a global font change.
    fn font_scale(mut self, scale: f32) -> Self {
        self.base_mut().style.visual.font_scale = scale;
        self
    }
}

/// Drag-and-drop opt-in — **universal**, available on every widget via a blanket
/// impl (like the base-level `visible`/`disabled` properties). Marks a widget as a
/// drag source and/or a drop target by storing an opaque [`DragItemId`] the app
/// interprets. The framework resolves these generically from the retained tree's
/// laid-out bounds ([`drag::source_at`](crate::drag::source_at) /
/// [`drag::resolve_at`](crate::drag::resolve_at)) — no per-surface geometry.
///
/// Domain-neutral by construction: the id is opaque and the payload (what the drag
/// *carries*) lives in the app's `DragContext<P>`, never in the widget.
pub trait DragExt: Component + Sized {
    /// Make this widget a **drag source** carrying `id`. A press inside its bounds
    /// can begin a drag; the app maps `id` back to the dragged thing.
    fn draggable(mut self, id: DragItemId) -> Self {
        self.base_mut().drag_source = Some(id);
        self
    }
    /// Make this widget a **drop target** identified by `id`. A drag released over
    /// its bounds drops onto `id`.
    fn drop_target(mut self, id: DragItemId) -> Self {
        self.base_mut().drop_target = Some(id);
        self
    }
}

/// Every component gets the drag/drop builders for free.
impl<T: Component + Sized> DragExt for T {}

/// Opt a widget into the universal leader/vimium **hint picker**: it gets assigned
/// a letter and, on the keypress, the host fires the intent it mapped `id` to.
///
/// Domain-neutral like [`DragExt`]: the id is opaque and the app owns the id→intent
/// map. Enumerated by [`hint::collect_hint_targets`](crate::hint::collect_hint_targets).
pub trait HintExt: Component + Sized {
    /// Make this widget a **hint target** carrying opaque `id`.
    fn hint_target(mut self, id: crate::hint::HintTargetId) -> Self {
        self.base_mut().hint_target = Some(id);
        self
    }
}

/// Every component gets the hint builder for free.
impl<T: Component + Sized> HintExt for T {}

/// Declaring a widget to be a **navigable row** with an identity of its own.
pub trait NavExt: Component + Sized {
    /// Label this row with the identity its component knows it by (`"pane:7"`, `"ws:0"`).
    ///
    /// One declaration, three readers: the keyboard cursor, the right-click target, and later the
    /// drag identity. The string is **opaque to the library** — only the component that wrote it
    /// and the host routing back to that component ever interpret it — and it must be stable across
    /// tree rebuilds, which is what lets a cursor survive one. See [`crate::nav`].
    fn nav_key(mut self, key: impl Into<String>) -> Self {
        self.base_mut().nav_key = Some(key.into());
        self
    }

    /// Name this subtree as an enclosing **scope** — a panel, a dock, a tab group.
    ///
    /// Stamp it on the wrapper around a region and a press anywhere inside it resolves back to that
    /// region, including a press a widget consumes (a scrollbar thumb is still *inside* the panel
    /// holding it). That is what lets "click a panel to focus it" work for every panel with nothing
    /// declared per panel. See [`crate::nav::scope_at`].
    fn scope_key(mut self, id: impl Into<String>) -> Self {
        self.base_mut().scope_key = Some(id.into());
        self
    }
}

/// Every component gets the nav-key builder for free: any widget can be a row.
impl<T: Component + Sized> NavExt for T {}

/// Components that contain children.
pub trait Parent: Component + Sized {
    /// Append a child component.
    fn child(mut self, c: impl Component + 'static) -> Self {
        self.base_mut().children.push(Box::new(c));
        self
    }

    /// Append an **already-boxed** subtree.
    ///
    /// [`child`](Parent::child) takes `impl Component`, and a `Box<dyn Component>` is not
    /// itself `Component` — so a subtree built *dynamically*, where the concrete widget
    /// type is not known at the call site, cannot go through it. That is what the host's
    /// `realize()` (a `ViewNode` tree) and a chrome provider's render seam both return.
    ///
    /// A widget with **several** places to put children names them instead
    /// ([`DockFrame::header_boxed`](crate::widgets::DockFrame::header_boxed),
    /// [`Dialog::body_boxed`](crate::widgets::Dialog::body_boxed)); those inherent methods
    /// take precedence over this one. This is the plain "append it to my children" case.
    ///
    /// ```ignore
    /// let body: Box<dyn Component> = realize(&node, &emit, &mut hints, &mut forms);
    /// let panel = Pane::new().padding(10.0).child_boxed(body);
    /// ```
    fn child_boxed(mut self, c: Box<dyn Component>) -> Self {
        self.base_mut().children.push(c);
        self
    }
}

/// **Event handlers, on any widget.** The one-line opt-in every widget already has for layout
/// ([`LayoutExt`]), style ([`StyleExt`]), drag ([`DragExt`]) and navigation ([`NavExt`]) —
/// the same shape, for what happens to it.
///
/// ```
/// use heca_grid_ui::prelude::*;
///
/// let row = Row::new()
///     .child(Label::new("pane-1"))
///     .on_click(|_| println!("selected"))
///     .on_right_click(|e| println!("menu at {:?}", e.pos))
///     .on_pointer_enter(|_| println!("hovered"));
/// ```
///
/// **Handlers run in the bubble phase**, after the widget's descendants have had the event and
/// before the widget's own [`on_event`](Component::on_event) — so a handler sees what its children
/// declined, and can stop the widget's built-in behaviour by taking the event itself:
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::EventKind;
///
/// // The general form: full control over the walk.
/// let button = Button::primary("Delete").on(EventKind::Click, |cx| {
///     cx.stop_propagation();   // the button will not fire
/// });
/// ```
///
/// The named builders below consume the event (an `on_click` that let the click carry on to the
/// row behind it would be a surprise). Use [`on`](EventExt::on) with
/// [`EventCx`](crate::event::EventCx) when you want to observe without consuming.
pub trait EventExt: Component + Sized {
    /// Register `f` for `kind`, with full control: `f` receives an
    /// [`EventCx`](crate::event::EventCx) and consumes the event only if it calls
    /// [`stop_propagation`](crate::event::EventCx::stop_propagation).
    fn on(mut self, kind: crate::event::EventKind, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.base_mut()
            .handlers
            .get_or_insert_with(Default::default)
            .add(kind, f);
        self
    }

    /// A left click landed on this widget (or a descendant that did not take it). Consumes it.
    fn on_click(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::Click, f)
    }

    /// A second click in the same run. The first still arrived as a
    /// [`click`](EventExt::on_click). Consumes it.
    fn on_double_click(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::DoubleClick, f)
    }

    /// A third click, and any beyond it (`click_count` says which). Consumes it.
    fn on_triple_click(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::TripleClick, f)
    }

    /// A right click landed on this widget. Consumes it.
    ///
    /// This is the whole of "a widget can have its own menu": the widget hears the click, on
    /// itself, with the position — no registry of row identities, no hit-test at the host, and
    /// nothing to forget to declare.
    fn on_right_click(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::RightClick, f)
    }

    /// A middle click landed on this widget. Consumes it.
    fn on_middle_click(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::MiddleClick, f)
    }

    /// A button went down on this widget. Consuming it **captures the pointer**: the moves and the
    /// release that end the gesture come here wherever the cursor goes.
    fn on_pointer_down(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::PointerDown, f)
    }

    /// The button came up. Consumes it.
    fn on_pointer_up(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::PointerUp, f)
    }

    /// The pointer moved over this widget, or anywhere while this widget holds capture.
    /// Consumes it.
    fn on_pointer_move(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::PointerMove, f)
    }

    /// The pointer came over this widget. **Does not consume** — entering is an announcement, and
    /// several widgets on one path enter together.
    fn on_pointer_enter(self, mut f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerEnter, move |cx| {
            if let Some(p) = cx.pointer() {
                f(p);
            }
        })
    }

    /// The pointer left this widget. **Does not consume**, for the same reason.
    fn on_pointer_leave(self, mut f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerLeave, move |cx| {
            if let Some(p) = cx.pointer() {
                f(p);
            }
        })
    }

    /// The wheel turned over this widget. Consumes it — so it does **not** also scroll whatever
    /// contains this widget. Use [`on`](EventExt::on) to watch one without claiming it.
    fn on_scroll(self, f: impl FnMut(&crate::event::PointerEvent) + 'static) -> Self {
        self.on_pointer(crate::event::EventKind::Scroll, f)
    }

    /// A press landed somewhere that is **not** this widget or a descendant — how a popup closes
    /// itself. Does not consume: the press belongs to whatever it landed on.
    fn on_pointer_down_outside(
        self,
        mut f: impl FnMut(&crate::event::PointerEvent) + 'static,
    ) -> Self {
        self.on(crate::event::EventKind::PointerDownOutside, move |cx| {
            if let Some(p) = cx.pointer() {
                f(p);
            }
        })
    }

    /// This widget gained keyboard focus. Does not consume.
    ///
    /// Named `on_focus_gained`, not `on_focus`, because [`Component::on_focus`] is the widget's
    /// own hook for the same moment and two same-named methods on one type is a puzzle nobody
    /// should have to solve at a call site.
    fn on_focus_gained(self, mut f: impl FnMut() + 'static) -> Self {
        self.on(crate::event::EventKind::Focus, move |_| f())
    }

    /// This widget lost keyboard focus — the moment to commit an edit or close a popup. Does not
    /// consume. Named `on_focus_lost` for the same reason as
    /// [`on_focus_gained`](EventExt::on_focus_gained).
    fn on_focus_lost(self, mut f: impl FnMut() + 'static) -> Self {
        self.on(crate::event::EventKind::Blur, move |_| f())
    }

    /// This widget entered a live tree (fired on its first layout pass). Does not consume.
    fn on_mount(self, mut f: impl FnMut() + 'static) -> Self {
        self.on(crate::event::EventKind::Mount, move |_| f())
    }

    /// This widget is being dropped — the tree that held it was rebuilt or thrown away. Does not
    /// consume. Use it to release what the widget registered with the host.
    fn on_unmount(self, mut f: impl FnMut() + 'static) -> Self {
        self.on(crate::event::EventKind::Unmount, move |_| f())
    }

    /// A drag began on this widget (it declared a
    /// [`draggable`](DragExt::draggable) id and the pointer travelled past the threshold).
    fn on_drag_start(self, f: impl FnMut(&crate::event::DragEvent) + 'static) -> Self {
        self.on_drag_kind(crate::event::EventKind::DragStart, f)
    }

    /// The drag this widget started moved.
    fn on_drag(self, f: impl FnMut(&crate::event::DragEvent) + 'static) -> Self {
        self.on_drag_kind(crate::event::EventKind::Drag, f)
    }

    /// The drag this widget started ended, dropped or not.
    fn on_drag_end(self, f: impl FnMut(&crate::event::DragEvent) + 'static) -> Self {
        self.on_drag_kind(crate::event::EventKind::DragEnd, f)
    }

    /// A drag came over this drop target.
    fn on_drag_enter(self, f: impl FnMut(&crate::event::DragEvent) + 'static) -> Self {
        self.on_drag_kind(crate::event::EventKind::DragEnter, f)
    }

    /// A drag moved within this drop target — `side` follows the pointer.
    fn on_drag_over(self, f: impl FnMut(&crate::event::DragEvent) + 'static) -> Self {
        self.on_drag_kind(crate::event::EventKind::DragOver, f)
    }

    /// A drag left this drop target.
    fn on_drag_leave(self, f: impl FnMut(&crate::event::DragEvent) + 'static) -> Self {
        self.on_drag_kind(crate::event::EventKind::DragLeave, f)
    }

    /// A drag was released over this drop target.
    fn on_drop(self, f: impl FnMut(&crate::event::DragEvent) + 'static) -> Self {
        self.on_drag_kind(crate::event::EventKind::Drop, f)
    }

    /// **Give this widget a context menu.** Right-click it — or anything inside it — and the menu
    /// opens at the cursor; trigger the host's `open_context_menu` while focus is here and it
    /// opens under the widget.
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::{Menu, MenuItem};
    ///
    /// let id = 7u64;
    /// let row = Row::new().child(Label::new("nvim")).context_menu(
    ///     Menu::new("Pane", "What you can do with this pane")
    ///         .child(MenuItem::new("Rename").on_click(move || { let _ = id; }))
    ///         .child(MenuItem::new("Close").danger(true).on_click(move || { let _ = id; })),
    /// );
    /// ```
    ///
    /// Nothing else is needed: no row identity, no path string, no registered builder, no
    /// host-side hit test, no `Shift+F10` handling, and no anchor — the framework picks that from
    /// what triggered the menu. Universal, like `nav_key`, so an `Icon` and a plugin's own widget
    /// carry one on the same terms as a `Row`. See [`crate::menu`] for bubbling and the host sink.
    ///
    /// Items whose labels or `enabled` depend on state the tree is **not** rebuilt on want
    /// [`context_menu_built`](EventExt::context_menu_built) instead.
    fn context_menu(mut self, menu: crate::widgets::Menu) -> Self {
        self.base_mut().context_menu = Some(Box::new(move || menu.clone()));
        self
    }

    /// The same, with the menu **built when it is triggered** — for items that must read state
    /// this widget's tree is not rebuilt on.
    fn context_menu_built(mut self, f: impl Fn() -> crate::widgets::Menu + 'static) -> Self {
        self.base_mut().context_menu = Some(Box::new(f));
        self
    }

    /// Shared body of the consuming pointer builders.
    #[doc(hidden)]
    fn on_pointer(
        self,
        kind: crate::event::EventKind,
        mut f: impl FnMut(&crate::event::PointerEvent) + 'static,
    ) -> Self {
        self.on(kind, move |cx| {
            if let Some(p) = cx.pointer() {
                f(p);
                cx.stop_propagation();
            }
        })
    }

    /// Shared body of the drag builders.
    #[doc(hidden)]
    fn on_drag_kind(
        self,
        kind: crate::event::EventKind,
        mut f: impl FnMut(&crate::event::DragEvent) + 'static,
    ) -> Self {
        self.on(kind, move |cx| {
            if let Some(d) = cx.event().drag() {
                f(d);
                cx.stop_propagation();
            }
        })
    }
}

/// Every component gets the event builders for free — the point of the whole design: a widget
/// author positions a widget and the events work, with nothing to opt into and nothing to forward.
impl<T: Component + Sized> EventExt for T {}
