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
