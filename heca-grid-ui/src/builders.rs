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
    /// **Space between children** — a number of pixels, or a step of the theme's rhythm.
    ///
    /// ```ignore
    /// Flex::row().gap(8)             // eight pixels
    /// Flex::row().gap(Spacing::Sm)   // a step, scaling with the font
    /// Flex::row().gap("sm")          // the same step, said as a description would
    /// ```
    ///
    /// **Prefer the step.** It is resolved from the inherited font at layout, so it moves with the
    /// font, the size variant and UI zoom; a pixel gap is tuned for one font size and wrong at
    /// every other. Use a number when you can say why it should not move.
    ///
    /// It was two builders — this and `gap_spacing` — which is two paths over one property. The
    /// docs said prefer the token and the token was used 8 times against this one's 112, because
    /// advice loses to whichever name is shorter.
    fn gap(mut self, v: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.gap = v.into();
        self
    }
    /// Outer margin on all sides — a number of pixels or a step, like [`gap`](Self::gap).
    fn margin(mut self, m: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.margin = m.into();
        self
    }
    /// Outer margin split per axis: `x` left+right, `y` top+bottom.
    fn margin_xy(mut self, x: f32, y: f32) -> Self {
        let s = &mut self.base_mut().style.layout;
        s.margin_left = Some(x.into());
        s.margin_right = Some(x.into());
        s.margin_top = Some(y.into());
        s.margin_bottom = Some(y.into());
        self
    }
    /// Horizontal outer margin (left+right) only.
    fn margin_x(mut self, v: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.margin_x = Some(v.into());
        self
    }
    /// Vertical outer margin (top+bottom) only — a rule breathing away from what it separates.
    fn margin_y(mut self, v: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.margin_y = Some(v.into());
        self
    }
    /// **Place this widget at a rect of its parent**, instead of letting it flow with its siblings.
    ///
    /// The host often knows *where* something goes while the widget still owns *what it looks
    /// like*: a floating pane drawn over the strip it belongs to, a chip pinned to a pane's corner,
    /// a badge over a cell. The temptation is to measure it yourself and call `cx.rect` + `cx.text`
    /// — don't. Name the rect and the engine places it.
    ///
    /// ```no_run
    /// # use heca_grid_ui::prelude::*;
    /// # use heca_grid_ui::style::Length::Percent;
    /// # let (x, y, w, h, strip_w, screen_h) = (200.0, 100.0, 400.0, 300.0, 1600.0, 900.0);
    /// # let card = Label::new("float");
    /// // A floating pane at its own fraction of the workspace behind it.
    /// let placed = card.at_rect(Percent(x / strip_w), Percent(y / screen_h), Percent(w / strip_w), Percent(h / screen_h));
    /// ```
    ///
    /// Two things follow, and both are the point:
    ///
    /// - **It is out of the flow.** The box takes no space from its siblings and is not moved by
    ///   them, so it draws *over* what it is placed on rather than pushing it aside. Later children
    ///   paint above earlier ones, so declare it after what it covers.
    /// - **A percentage resolves per axis** — `left`/`width` against the parent's width, `top`/
    ///   `height` against its height. This is the difference from
    ///   [`margin_left`](Self::margin_left) / [`margin_top`](Self::margin_top), where CSS resolves
    ///   a percentage on **both** axes against the width; a fractional `top` written as a margin
    ///   silently produces a number, just the wrong one, on any parent that is not square
    ///   (`layout::tests::a_percentage_margin_resolves_against_the_parents_width_on_both_axes`).
    ///
    /// The rect **overrides** [`width`](Self::width) / [`height`](Self::height): it names both, and
    /// a leftover size beside it would draw a different rect than the one asked for. **Except
    /// where it says `Auto`** — an axis left auto has said *where*, not *how big*, so the widget's
    /// own size stands there. That is what lets a caller place something without also resizing it.
    fn at_rect(
        mut self,
        left: impl Into<crate::style::Length>,
        top: impl Into<crate::style::Length>,
        width: impl Into<crate::style::Length>,
        height: impl Into<crate::style::Length>,
    ) -> Self {
        self.base_mut().style.layout.placement = Some(crate::style::Placement {
            left: left.into(),
            top: top.into(),
            width: width.into(),
            height: height.into(),
        });
        self
    }
    /// Left outer margin only.
    fn margin_left(mut self, v: impl Into<crate::style::Length>) -> Self {
        self.base_mut().style.layout.margin_left = Some(v.into());
        self
    }
    /// Right outer margin only.
    fn margin_right(mut self, v: impl Into<crate::style::Length>) -> Self {
        self.base_mut().style.layout.margin_right = Some(v.into());
        self
    }
    /// Top outer margin only.
    fn margin_top(mut self, v: impl Into<crate::style::Length>) -> Self {
        self.base_mut().style.layout.margin_top = Some(v.into());
        self
    }
    /// Bottom outer margin only.
    fn margin_bottom(mut self, v: impl Into<crate::style::Length>) -> Self {
        self.base_mut().style.layout.margin_bottom = Some(v.into());
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
    /// **Inner padding on all sides** — pixels or a step, exactly as [`gap`](LayoutExt::gap).
    fn padding(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.padding = p.into();
        self
    }
    /// **Inner padding per axis**: `x` left+right, `y` top+bottom. Pixels or a step, each.
    fn padding_xy(
        mut self,
        x: impl Into<crate::style::Space>,
        y: impl Into<crate::style::Space>,
    ) -> Self {
        {
            let l = &mut self.base_mut().style.layout;
            l.padding_x = Some(x.into());
            l.padding_y = Some(y.into());
        }
        self
    }

    /// **Inner padding left+right** — pixels or a step.
    ///
    /// There was no pixel form of this: `pad_x` took only a token, so a caller wanting a measured
    /// horizontal inset had to reach for `padding_xy` and restate the vertical one.
    fn padding_x(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.padding_x = Some(p.into());
        self
    }

    /// **Inner padding top+bottom** — see [`padding_x`](LayoutExt::padding_x).
    fn padding_y(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.padding_y = Some(p.into());
        self
    }

    /// Inner padding on one side, overriding the axis and the uniform value.
    ///
    /// Reserving space along a single edge is not the same as padding the axis: the opposite side
    /// should not move because this one needed room. A `ScrollRegion` keeping its content clear of
    /// its scrollbar is the case that asked for it.
    fn padding_left(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.padding_left = Some(p.into());
        self
    }
    /// Inner padding on the right only — see [`padding_left`](Self::padding_left).
    fn padding_right(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.padding_right = Some(p.into());
        self
    }
    /// Inner padding on the top only — see [`padding_left`](Self::padding_left).
    fn padding_top(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.padding_top = Some(p.into());
        self
    }
    /// Inner padding on the bottom only — see [`padding_left`](Self::padding_left).
    fn padding_bottom(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.base_mut().style.layout.padding_bottom = Some(p.into());
        self
    }
    /// Width along the main/cross axis, in any of the spellings a size is written in:
    /// `.width(200)` / `.width("200px")` px, `.width("50%")` a fraction of the parent,
    /// `.width("auto")` content-sized, `.width(Length::HALF)` the same fraction without a number
    /// to mistype. See [`Length`] for the whole vocabulary.
    ///
    /// **Setting neither width nor height already fills the parent** across the cross axis, exactly
    /// as CSS `align-items: stretch` does — so `.width(Length::FULL)` on a child that fills says
    /// nothing, and is better left off.
    fn width(mut self, w: impl Into<Length>) -> Self {
        self.base_mut().style.layout.width = w.into();
        self
    }
    /// Height along the main/cross axis — the same spellings as [`width`](LayoutExt::width).
    fn height(mut self, h: impl Into<Length>) -> Self {
        self.base_mut().style.layout.height = h.into();
        self
    }
    /// Flex grow factor (share of remaining space).
    /// Permission to **shrink** below the natural size, the other half of a share.
    ///
    /// `flex_grow` distributes only *positive* free space, so growing alone never divides a region:
    /// a child keeps its content size and the row overflows. A true share is grow + a zero base
    /// size + this (CSS `flex: 1 1 0`).
    /// **Let children that do not fit start a new line** (CSS `flex-wrap: wrap`).
    ///
    /// Off by default: most rows are a slot, a label and a slot, where a second line would be
    /// nonsense. Turn it on for a row of *peers* — a set of action buttons, a tag list — where
    /// the alternative when the box gets narrow is squeezing every one of them to an ellipsis.
    fn wrap(mut self, wrap: bool) -> Self {
        self.base_mut().style.layout.wrap = wrap;
        self
    }

    fn shrink(mut self, s: f32) -> Self {
        self.base_mut().style.layout.flex_shrink = Some(s);
        self
    }

    /// Floor for the height — a row that must stay legible however many share the space.
    /// Same spellings as [`width`](LayoutExt::width).
    fn min_height(mut self, h: impl Into<Length>) -> Self {
        self.base_mut().style.layout.min_height = Some(h.into());
        self
    }

    /// Floor for the width. Same spellings as [`width`](LayoutExt::width).
    fn min_width(mut self, w: impl Into<Length>) -> Self {
        self.base_mut().style.layout.min_width = Some(w.into());
        self
    }

    /// Ceiling for the height — a box that may not grow past it however tall its content is.
    /// Same spellings as [`width`](LayoutExt::width).
    fn max_height(mut self, h: impl Into<Length>) -> Self {
        self.base_mut().style.layout.max_height = Some(h.into());
        self
    }

    /// Ceiling for the width, the counterpart to [`min_width`](LayoutExt::min_width).
    ///
    /// A panel sized to its content wants both: a floor so a one-word menu is not a sliver, and a
    /// ceiling so one long row does not stretch it across the screen. Past the ceiling the content
    /// is the child's problem — a [`Label`](crate::widgets::Label) with
    /// [`truncate`](crate::widgets::Label::truncate) cuts, anything else overflows.
    fn max_width(mut self, w: impl Into<Length>) -> Self {
        self.base_mut().style.layout.max_width = Some(w.into());
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

/// Components that contain children.
/// **Anything that can be a child** — a widget, or a subtree someone else already built.
///
/// A `Box<dyn Component>` is not itself a [`Component`], which is the whole reason every
/// child-taking builder used to come in twos: `child` / `child_boxed`, `body` / `body_boxed`,
/// `leading` / `leading_boxed`, sixteen of them. A caller with a dynamically built subtree — what
/// the host's `realize()` returns from a `ViewNode`, or a chrome provider's render seam — had to
/// know which spelling to reach for, and a widget author had to remember to write both.
///
/// One bound covers both, so there is one builder per slot and nothing to remember. Implemented
/// for every `Component` and for `Box<dyn Component>`; nothing else needs an impl, and a caller
/// never names this trait.
pub trait IntoComponent {
    /// The subtree, boxed exactly once.
    fn into_component(self) -> Box<dyn Component>;
}

impl<C: Component + 'static> IntoComponent for C {
    fn into_component(self) -> Box<dyn Component> {
        Box::new(self)
    }
}

impl IntoComponent for Box<dyn Component> {
    /// Already boxed — handed straight through, so a realized subtree costs no second allocation.
    fn into_component(self) -> Box<dyn Component> {
        self
    }
}

pub trait Parent: Component + Sized {
    /// Append a child — a widget, or an already-boxed subtree; see [`IntoComponent`].
    fn child(mut self, c: impl IntoComponent) -> Self {
        self.base_mut().children.push(c.into_component());
        self
    }

    /// **The plural of [`child`](Parent::child)** — a whole collection, in one call.
    ///
    /// Without it a caller holding a list has to break the chain and go imperative:
    ///
    /// ```ignore
    /// let mut dock = DockFrame::new(name).active(true).key(k);
    /// for column in &ws.columns {
    ///     dock = dock.child(ColumnGroup { column }.build());   // reassigned, mid-expression
    /// }
    /// ```
    ///
    /// …which is a `mut`, a rebinding and a loop sitting inside what reads everywhere else as one
    /// declarative expression. With the plural it stays one:
    ///
    /// ```ignore
    /// DockFrame::new(name)
    ///     .active(true)
    ///     .key(k)
    ///     .children(ws.columns.iter().map(|c| ColumnGroup { column: c }.build()))
    /// ```
    ///
    /// Takes anything iterable of anything that can be a child, so a `Vec`, a `map` over a slice,
    /// or a mix of widgets and already-realized subtrees all go through the same call.
    fn children(mut self, cs: impl IntoIterator<Item = impl IntoComponent>) -> Self {
        let slot = &mut self.base_mut().children;
        slot.extend(cs.into_iter().map(IntoComponent::into_component));
        self
    }
}

/// **Event handlers, on any widget.** The one-line opt-in every widget already has for layout
/// ([`LayoutExt`]), style ([`StyleExt`]), drag ([`ComponentExt`]) and navigation ([`ComponentExt`]) —
/// the same shape, for what happens to it.
///
/// ```
/// use heca_grid_ui::prelude::*;
///
/// let row = Row::new()
///     .child(Label::new("pane-1"))
///     .on_click(|_| println!("selected"))
///     .on_right_click(|e| println!("menu at {:?}", e.pos()))
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
/// row behind it would be a surprise). Use [`on`](ComponentExt::on) with
/// [`EventCx`](crate::event::EventCx) when you want to observe without consuming.
/// **What [`ComponentExt::context_menu`] accepts: a menu, or a way to make one.**
///
/// One method, two spellings, because both are honest ways to say the same thing:
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
///
/// # fn build_menu(_: u64) -> ContextMenu { ContextMenu::new("m") }
/// let ctx = ContextMenu::new("pane").child(Menu::new("Pane", "…"));
/// Row::new().context_menu(ctx.clone());               // a value
/// Row::new().context_menu(move |_at| build_menu(7));  // a closure
/// ```
///
/// A [`ContextMenu`](crate::widgets::ContextMenu) is `Clone` — its content is plain data and `Rc`
/// closures — so the value form is a clone per opening, not a shared panel. Reach for the closure
/// when the menu's rows depend on state this widget's tree is not rebuilt on, or when building it
/// eagerly would be wasted work.
pub trait IntoContextMenu {
    /// Produce the menu to show, for a right-click at `at`. Called **each time** the menu is
    /// triggered.
    ///
    /// **The point is passed because a menu is always opened at one**, and some entries are answers
    /// about what is under it rather than about the widget as a whole — a terminal's *Open link*
    /// is the case that forced it: whether that entry exists depends on the exact cell clicked, so
    /// a builder that cannot see the point cannot decide. Without it the host had to work the menu
    /// out on the widget's behalf, which is how a pane's menu ended up hand-written in the mouse
    /// handler. Ignore it with `|_at|` when the menu does not vary.
    fn build(&self, at: heca_core::layout::Point) -> crate::widgets::ContextMenu;
}

impl IntoContextMenu for crate::widgets::ContextMenu {
    fn build(&self, _at: heca_core::layout::Point) -> crate::widgets::ContextMenu {
        self.clone()
    }
}

impl<F: Fn(heca_core::layout::Point) -> crate::widgets::ContextMenu> IntoContextMenu for F {
    fn build(&self, at: heca_core::layout::Point) -> crate::widgets::ContextMenu {
        self(at)
    }
}

/// **Everything every component gets.**
///
/// One trait, blanket-implemented for every [`Component`], holding the builders that are true of
/// all of them: what happens to a widget ([`on_click`](ComponentExt::on_click),
/// [`on_key`](ComponentExt::on_key), [`context_menu`](ComponentExt::context_menu)), who it is
/// ([`key`](ComponentExt::key)), and what it does in a drag
/// ([`draggable`](ComponentExt::draggable), [`drop_target`](ComponentExt::drop_target)).
///
/// It was four traits — `ComponentExt`, `ComponentExt`, `ComponentExt`, `ComponentExt` — split by nothing but the order
/// they were added in. All four were unconditional (`impl<T: Component> … for T {}`), so the split
/// carried no rule: four names to learn and four imports for one fact. The two splits that *do*
/// carry a rule are still separate, because the type system enforces them: [`StyleExt`] is
/// surfaces only (a layout-only `Flex` cannot be given a background), and [`Parent`] is containers
/// only (a `Label` has no `.child()`).
pub trait ComponentExt: Component + Sized {

    /// **This widget can be dragged**, and what gets dragged is the identity it already declares
    /// with [`key`](ComponentExt::key) — the same one the keyboard cursor and the right-click
    /// target read.
    ///
    /// ```ignore
    /// Row::new().key("pane:7").draggable().drop_target()
    /// ```
    ///
    /// One declaration, and nothing to hand out: it took an opaque id from a host registry before,
    /// beside a closed list of which surfaces were even allowed to drag — so a row named itself
    /// twice and a plugin's row could not be named at all. A widget with no `key` is not a drag
    /// source; there would be nothing to say about what was picked up.
    fn draggable(mut self) -> Self {
        self.base_mut().draggable = true;
        self
    }
    /// **This widget can be dragged, and it says what it is** — an opaque word its component
    /// chose (`"pane"`, `"column"`, `"docker.container"`), so a target can take some things and
    /// refuse others.
    ///
    /// ```ignore
    /// Row::new().key("col:3").draggable_as("column").drop_target().accepts(["column"])
    /// ```
    ///
    /// Say nothing and every target takes it, which is what [`draggable`](Self::draggable) alone
    /// means. The word is never interpreted by the library, and it is not from a list the library
    /// knows — a closed set is one a plugin cannot join.
    fn draggable_as(mut self, kind: impl Into<String>) -> Self {
        let b = self.base_mut();
        b.draggable = true;
        b.drag_kind = Some(kind.into());
        self
    }
    /// **This widget accepts drops**, identified the same way — by its
    /// [`key`](ComponentExt::key). A drag released over its bounds drops onto it, with the side
    /// (before / onto / after) computed from where in its bounds the pointer sits.
    fn drop_target(mut self) -> Self {
        self.base_mut().drop_target = true;
        self
    }
    /// **What this target takes**, by the words a drag source names itself with
    /// ([`draggable_as`](Self::draggable_as)). Anything else is refused *before* it is drawn on, so
    /// a line never appears over something that would then do nothing.
    ///
    /// Declaring nothing takes anything. Implies [`drop_target`](Self::drop_target), because a
    /// widget saying what it takes has said it takes something.
    fn accepts(mut self, kinds: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let b = self.base_mut();
        b.drop_target = true;
        b.accepts = kinds.into_iter().map(Into::into).collect();
        b.accepts_onto = true;
        self
    }
    /// **What this target takes *beside* itself** — for a sibling in an ordered list, where being
    /// dropped *onto* it means nothing.
    ///
    /// A column dropped on a column is a reorder: it can only land before or after, so the target
    /// reads as two halves and the line flips at the midpoint — never a third band that silently
    /// picks one for you. Use [`accepts`](Self::accepts) where the middle is real, as a pane is for
    /// another pane, which it swaps or moves onto.
    fn accepts_beside(mut self, kinds: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let b = self.base_mut();
        b.drop_target = true;
        b.accepts = kinds.into_iter().map(Into::into).collect();
        b.accepts_onto = false;
        self
    }

    /// Label this row with the identity its component knows it by (`"pane:7"`, `"ws:0"`).
    ///
    /// One declaration, three readers: the keyboard cursor, the right-click target, and later the
    /// drag identity. The string is **opaque to the library** — only the component that wrote it
    /// and the host routing back to that component ever interpret it — and it must be stable across
    /// tree rebuilds, which is what lets a cursor survive one. See [`crate::nav`].
    /// **Let the pointer pass through this widget** — it draws, but it is not a target, and nor is
    /// anything inside it (CSS `pointer-events: none`).
    ///
    /// For decoration that stands in for something else: an echo of a value shown inside the
    /// control that owns it. Without it the echo hovers and clicks in its own right, and the
    /// control ends up wearing two highlights.
    fn pointer_transparent(mut self, transparent: bool) -> Self {
        self.base_mut().pointer_transparent = transparent;
        self
    }

    fn key(mut self, key: impl Into<String>) -> Self {
        self.base_mut().key = Some(key.into());
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

    /// Register `f` for `kind`. It receives an [`EventCx`](crate::event::EventCx) and consumes the
    /// event only if it calls [`stop_propagation`](crate::event::EventCx::stop_propagation).
    fn on(mut self, kind: crate::event::EventKind, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        // **Wiring an action is what makes a widget pickable** (F003/P082/T441). Set here, in the one
        // place every generic listener goes through, rather than repeated in `on_click`,
        // `on_double_click`, `on_key_down` and `on_key_up` — a rule in four call sites is a rule in
        // the wrong place.
        //
        // Right-click and middle-click are deliberately absent: a right-click opens a context menu
        // rather than doing the thing, so it should not spend one of the 52 letters (Antonio,
        // 2026-08-17).
        use crate::event::EventKind as K;
        if matches!(kind, K::Click | K::DoubleClick | K::Key) {
            self.base_mut().activatable = true;
        }
        self.base_mut()
            .handlers
            .get_or_insert_with(Default::default)
            .add(kind, f);
        self
    }

    /// A left click landed on this widget (or a descendant that did not take it).
    fn on_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Click, f)
    }

    /// A second click in the same run. The first still arrived as a
    /// [`click`](ComponentExt::on_click).
    fn on_double_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DoubleClick, f)
    }

    /// A third click, and any beyond it (`click_count` says which).
    fn on_triple_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::TripleClick, f)
    }

    /// A right click landed on this widget.
    ///
    /// This is the whole of "a widget can have its own menu": the widget hears the click, on
    /// itself, with the position — no registry of row identities, no hit-test at the host, and
    /// nothing to forget to declare.
    fn on_right_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::RightClick, f)
    }

    /// A middle click landed on this widget.
    fn on_middle_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::MiddleClick, f)
    }

    /// A button went down on this widget. Stopping propagation here **captures the pointer**: the
    /// moves and the release that end the gesture come here wherever the cursor goes.
    fn on_pointer_down(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerDown, f)
    }

    /// The button came up.
    fn on_pointer_up(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerUp, f)
    }

    /// The pointer moved over this widget, or anywhere while this widget holds capture.
    fn on_pointer_move(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerMove, f)
    }

    /// The pointer came over this widget. Several widgets on one path enter together, so stopping
    /// propagation here is almost always wrong.
    fn on_pointer_enter(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerEnter, f)
    }

    /// The pointer left this widget.
    fn on_pointer_leave(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerLeave, f)
    }

    /// The wheel turned over this widget. Stop propagation to keep it from also scrolling whatever
    /// contains this widget.
    fn on_scroll(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Scroll, f)
    }

    /// A press landed somewhere that is **not** this widget or a descendant — how a popup closes
    /// itself. The press belongs to whatever it landed on, so do not claim it.
    fn on_pointer_down_outside(
        self,
        f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static,
    ) -> Self {
        self.on(crate::event::EventKind::PointerDownOutside, f)
    }

    /// A key went **down** on this widget — it holds the keyboard, or it contains what does.
    ///
    /// The other half of "an event listener in every widget": intercepting a key, a quick-pick
    /// letter or a shortcut is the same one line as intercepting a click, on the same argument,
    /// with the same way to say the event is yours.
    ///
    /// Down and up are separate builders because they are separate events everywhere else — the
    /// DOM's `keydown` / `keyup`, and every toolkit that copies it. One handler with a `pressed`
    /// flag inside makes every caller write the same `if`, which is a rule in N call sites rather
    /// than in the API.
    ///
    /// ```ignore
    /// card.on_key_up(move |cx| {
    ///     if let Event::Key { key: GridKey::Char('x'), .. } = cx.event() {
    ///         delete(id);
    ///         cx.stop_propagation();   // …or let it bubble to the container
    ///     }
    /// })
    /// ```
    fn on_key_down(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on_key_when(true, f)
    }

    /// A key came **up** on this widget. See [`on_key_down`](ComponentExt::on_key_down).
    fn on_key_up(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on_key_when(false, f)
    }

    /// Shared by the two above: one registration on `EventKind::Key`, gated on the half it wants.
    /// Private so there is no third spelling of "a key happened".
    #[doc(hidden)]
    fn on_key_when(
        self,
        want_pressed: bool,
        mut f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static,
    ) -> Self {
        self.on(crate::event::EventKind::Key, move |cx| {
            if matches!(cx.event(), crate::event::Event::Key { pressed, .. } if *pressed == want_pressed)
            {
                f(cx);
            }
        })
    }

    /// Text the user committed — typed, pasted, or composed by an IME.
    fn on_text_input(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::TextInput, f)
    }

    /// This widget gained keyboard focus.
    ///
    /// Named `on_focus_gained`, not `on_focus`, because [`Component::on_focus`] is the widget's
    /// own hook for the same moment and two same-named methods on one type is a puzzle nobody
    /// should have to solve at a call site.
    fn on_focus_gained(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Focus, f)
    }

    /// This widget lost keyboard focus — the moment to commit an edit or close a popup. Named
    /// `on_focus_lost` for the same reason as [`on_focus_gained`](ComponentExt::on_focus_gained).
    fn on_focus_lost(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Blur, f)
    }

    /// This widget entered a live tree (fired on its first layout pass).
    fn on_mount(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Mount, f)
    }

    /// This widget is being dropped — the tree that held it was rebuilt or thrown away. Use it to
    /// release what the widget registered with the host.
    fn on_unmount(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Unmount, f)
    }

    /// A drag began on this widget (it declared a [`draggable`](ComponentExt::draggable) id and the
    /// pointer travelled past the threshold).
    fn on_drag_start(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragStart, f)
    }

    /// The drag this widget started moved.
    fn on_drag(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Drag, f)
    }

    /// The drag this widget started ended, dropped or not.
    fn on_drag_end(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragEnd, f)
    }

    /// A drag came over this drop target.
    fn on_drag_enter(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragEnter, f)
    }

    /// A drag moved within this drop target — `side` follows the pointer.
    fn on_drag_over(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragOver, f)
    }

    /// A drag left this drop target.
    fn on_drag_leave(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragLeave, f)
    }

    /// A drag was released over this drop target.
    fn on_drop(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Drop, f)
    }

    /// **Give this widget a context menu.** Right-click it — or anything inside it — and the menu
    /// opens at the cursor; trigger the host's `open_context_menu` while focus is here and it
    /// opens under the widget.
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::{ContextMenu, Menu, MenuItem};
    ///
    /// let id = 7u64;
    /// let ctx = ContextMenu::new("pane-menu").child(
    ///     Menu::new("Pane", "What you can do with this pane")
    ///         .child(MenuItem::new().label("Rename").on_click(move || { let _ = id; }))
    ///         .child(MenuItem::new().label("Close").danger(true).on_click(move || { let _ = id; })),
    /// );
    ///
    /// // A value — and the same value again on the next row, because a menu is `Clone`.
    /// let row = Row::new().child(Label::new("nvim")).context_menu(ctx.clone());
    /// // …or a closure, when the rows must read state at the moment it opens.
    /// let other = Row::new().context_menu(move |_at| ctx.clone());
    /// ```
    ///
    /// Nothing else is needed: no row identity, no path string, no registered builder, no
    /// host-side hit test, no `Shift+F10` handling, and no anchor — the framework picks that from
    /// what triggered the menu. Universal, like `key`, so an `Icon` and a plugin's own widget
    /// carry one on the same terms as a `Row`. See [`crate::menu`] for bubbling and the host sink.
    ///
    /// **A value or a closure** — see [`IntoContextMenu`]. Either way the menu is realized when it
    /// is triggered, so a composed row's subtree is built fresh for each opening.
    fn context_menu(mut self, menu: impl IntoContextMenu + 'static) -> Self {
        self.base_mut().context_menu = Some(Box::new(move |at| menu.build(at)));
        self
    }

    /// **Declare an action this widget answers to, by name.**
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::{KeyHintGroup, Flex};
    /// use heca_grid_ui::reactive::{signal, SignalUpdate};
    ///
    /// let open = signal(false);
    /// let picker = KeyHintGroup::new(Flex::column())
    ///     .open_when(open)
    ///     .on_action("mypanel.pick", move || open.set(true));
    /// ```
    ///
    /// The name is namespaced by whoever declares it, exactly as a provider's actions are, and a
    /// binding names it the same way:
    ///
    /// ```toml
    /// [[keys.surface]]
    /// name = "mypanel"
    /// pick = "s"          # → mypanel.pick
    /// ```
    ///
    /// **The widget names the action; config names the key.** A key written into a widget would be
    /// unrebindable, absent from the palette and unreachable over RPC — the thing AGENTS § 2
    /// forbids.
    ///
    /// This is what lets a **layer** own a verb. A dock declares its actions through `Provider`;
    /// an overlay had no such seam, so it could only bind verbs the app had already compiled in.
    /// See [`Base::actions`](crate::component::Base::actions).
    /// **Keep this widget out of the picker**, however actionable it is.
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::Button;
    ///
    /// // A close button on every row would eat a letter each, for the one gesture nobody picks.
    /// let b = Button::new("×").hintable(false);
    /// ```
    ///
    /// **The opt-out exists because being pickable is not opt-in.** Anything you can act on — a
    /// click, a double click, a key — wears a letter with nothing declared, so the only thing left
    /// to say is "not me". `hintable(true)` is the default and changes nothing; a widget nobody can
    /// act on has nothing for a letter to run.
    ///
    /// Letters are scarce (52 in one picker, one keystroke each), so this is how a dense surface
    /// keeps them for the targets that matter.
    #[heca_grid_ui_macros::prop]
    fn hintable(mut self, yes: bool) -> Self {
        self.base_mut().hintable = yes;
        self
    }

    /// **Lock what is behind this surface** — refuse actions on it while this is up.
    ///
    /// ```ignore
    /// Overlay::new().lock(true).child(my_panel)
    /// ```
    ///
    /// A dialog locks; a map of the working area does not, because picking one of the things behind
    /// it is the point. **Not about pixels** — the map covers every pixel it draws over and still
    /// passes `false` — and not [`overlay_occludes`](crate::Component::overlay_occludes), which is
    /// the geometric question the pointer asks.
    ///
    /// Widgets that know what they are set it themselves; this is for a raw
    /// [`Overlay`](crate::widgets::Overlay). See [`Base::lock`](crate::Base::lock).
    fn lock(mut self, yes: bool) -> Self {
        self.base_mut().lock = yes;
        self
    }

    /// **Override whether this surface takes the keyboard while it is up.**
    ///
    /// ```ignore
    /// Overlay::new().child(my_panel)                      // derived: open == holds focus == takes keys
    /// my_surface.captures_keyboard(true)                  // say so explicitly
    /// ```
    ///
    /// **Optional, and you almost never want it.** Left alone, the answer is read from the tree with
    /// the same rule the event walk uses — a surface that wants keys holds focus, and every layer
    /// widget binds its open signal to it. Reach for this only when a surface takes the keyboard in
    /// a way the framework cannot see. See [`Base::captures_keyboard`](crate::Base::captures_keyboard).
    fn captures_keyboard(mut self, yes: bool) -> Self {
        self.base_mut().captures_keyboard = Some(yes);
        self
    }

    /// **What this widget says on hover.**
    ///
    /// ```ignore
    /// Button::new("Close").icon(Glyph::Minus).tooltip("Close the pane")
    /// ```
    ///
    /// One line, on the widget, on **every** widget — and the framework owns the rest: the reveal
    /// delay is timed off the hover clock the pointer router already keeps, and the bubble is drawn
    /// in the one place every widget passes through, beside the hint letter and the drag feedback.
    ///
    /// It used to be a wrapper you put *around* the widget
    /// ([`Tooltip`](crate::widgets::Tooltip)), which put the rule in every caller's discipline —
    /// the showcase wraps four buttons in four tooltips in a row — and made a tooltip impossible on
    /// a widget held by a typed container, because wrapping it changes what it is. The wrapper
    /// survives only for a region that is not a widget you can put a builder on, exactly as
    /// [`KeyHint`](crate::widgets::KeyHint) did when the pick declaration moved onto every widget.
    #[heca_grid_ui_macros::prop]
    fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.base_mut().tooltip = Some(crate::widgets::tooltip::Tip::new(text));
        self
    }

    /// **A tooltip whose words are live** — an action's current keybinding, a changing status — so
    /// the bubble follows the signal without the widget being rebuilt.
    fn tooltip_signal(mut self, text: crate::reactive::Signal<String>) -> Self {
        self.base_mut().tooltip = Some(crate::widgets::tooltip::Tip::from_signal(text));
        self
    }

    /// Which side of this widget its tooltip anchors to (default `Top`). Flipped automatically
    /// when there is no room on that side, so this is a preference, not a placement.
    ///
    /// No-op when the widget has declared no tooltip — the side is part of the tip, not a style of
    /// its own.
    #[heca_grid_ui_macros::prop]
    fn tooltip_side(mut self, side: crate::widgets::tooltip::TooltipSide) -> Self {
        if let Some(tip) = self.base_mut().tooltip.as_mut() {
            tip.side = side;
        }
        self
    }

    /// Seconds the pointer must rest before this widget's tooltip appears (default `0.5`).
    /// No-op when the widget has declared no tooltip.
    #[heca_grid_ui_macros::prop]
    fn tooltip_delay(mut self, seconds: f32) -> Self {
        if let Some(tip) = self.base_mut().tooltip.as_mut() {
            tip.delay = seconds.max(0.0);
        }
        self
    }

    /// **What a pick does to this widget, when that differs from acting on it.**
    ///
    /// Being pickable is **not** what this turns on — anything actionable already wears a letter,
    /// and picking it does what clicking it does (F003/P082/T441). This is the **override**:
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::Row;
    ///
    /// // A click activates the pane and leaves the sidebar; a pick looks at it and stays.
    /// let row = Row::new().on_activate(|| { /* focus_pane */ }).on_hint(|| { /* peek_selected */ });
    /// ```
    ///
    /// **It fires in the target phase only** — when *this* widget is the one picked, never when a
    /// pick from a child passes through on the way up. A pane that is itself pickable and holds
    /// pickable rows would otherwise fire both and land you on the pane, and every such container
    /// would hand-write the DOM's `e.target !== e.currentTarget` guard — N copies of a framework
    /// rule, which by our own rule means the API is missing. This deliberately diverges from
    /// [`on_click`](ComponentExt::on_click): clicking a child of a clickable box **is** clicking the
    /// box, because the pointer is over both, while picking a row is **not** picking the pane — a
    /// pick is nominal, not spatial.
    ///
    /// **To watch picks from your children instead**, listen for the bubbled event:
    /// `.on(EventKind::Hint, |e| …)`. It sees which widget was picked and can
    /// [`stop_propagation`](crate::event::EventCx::stop_propagation); it declares nothing and gets
    /// no letter of its own.
    ///
    /// **Say what it is, where you can.** `f` may be a plain closure, or a
    /// [`Hint`](crate::hint::Hint) carrying the [`Intent`](heca_view::Intent) the act *is* — which
    /// is what lets a host ask its own policy about a candidate before spending a letter on it. The
    /// app's `fires` and `realize` both hand over the pair; nothing else has to.
    fn on_hint(mut self, f: impl Into<crate::hint::Hint>) -> Self {
        self.base_mut().hint = Some(f.into());
        self
    }

    /// **Which pickers letter this target.** Unset — the default — means the ordinary one, which
    /// is `prefix+/` in heca and letters everything actionable.
    ///
    /// One surface can carry more than one verb over the same tree. The exposé's cards mean *go
    /// there* and their ⊠ icons mean *remove that*: a picker that letters both hands out sixteen
    /// letters where you wanted eight, and half of them delete what you meant to jump to. So a
    /// target says which sets it belongs to and a picker says which one it hands letters for.
    ///
    /// ```ignore
    /// Row::new().on_hint(go_to(id))                          // the ordinary picker
    /// IconButton::new(Glyph::X).on_hint(remove(id)).hint_scope(["close"])   // only "close"
    /// ```
    ///
    /// **Naming a scope takes the target OUT of the ordinary picker.** That is what makes this
    /// worth more than the surface it was built for: a column in the workspaces dock is a
    /// *destination* for "move a pane to a column", never somewhere `prefix+/` should send you, and
    /// saying so is one line on the widget instead of a rule the picker carries about columns.
    ///
    /// Name several and the target belongs to each of those pickers. Addressing a widget **by
    /// key** — what `prefix+q` and "move to column" do — ignores scopes entirely: those name one
    /// target outright rather than collecting a set, so there is nothing to filter.
    #[heca_grid_ui_macros::prop]
    fn hint_scope(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.base_mut().hint_scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// **Take this widget out of the layout entirely** — CSS `display: none`. Its neighbours close
    /// up, and it is neither painted nor Tab-focused.
    ///
    /// ```ignore
    /// Row::new().hidden(collapsed)
    /// ```
    ///
    /// The field was always here and every widget that folds a subtree writes it
    /// (`ItemGroup`, `DockFrame`, `ChromeRegion`) — but only from *inside itself*, because there
    /// was no builder, so a caller wanting the same thing had to reach for the
    /// [`Visibility`](crate::widgets::Visibility) wrapper. Its described twin `"hidden"` has been
    /// carried by the generic style merge all along, which left the two authoring paths unequal.
    ///
    /// **Its twin is [`visible`](ComponentExt::visible)** — that one keeps the box.
    #[heca_grid_ui_macros::prop]
    fn hidden(mut self, hidden: bool) -> Self {
        self.base_mut().set_hidden(hidden);
        self
    }

    /// **Whether this widget's ink is drawn**, keeping its box either way — CSS `visibility`.
    ///
    /// ```ignore
    /// StatusDot::error().visible(false)   // the row's layout does not move when it appears
    /// ```
    ///
    /// **The other one is [`hidden`](crate::style::Layout::hidden)** — CSS `display: none`, which
    /// takes the widget out of the layout so its neighbours close up. Reach for that when the space
    /// should collapse, and for this when it must not: a row of four status slots showing one at a
    /// time stays still only if the three quiet ones keep their boxes.
    ///
    /// `Base::visible` has always been on every widget; this is the builder that was missing, so
    /// setting it meant wrapping in a [`Visibility`](crate::widgets::Visibility) — which cannot be
    /// done to a widget a typed container holds, and which a description cannot express at all.
    /// The wrapper stays for a region that is not a widget you can put a builder on.
    #[heca_grid_ui_macros::prop]
    fn visible(self, visible: bool) -> Self {
        crate::reactive::SignalUpdate::set(&self.base().visible, visible);
        self
    }

    /// **Where this widget's hint letter sits over it** (default
    /// [`TopCenter`](crate::widgets::HintPlacement::TopCenter)).
    ///
    /// ```ignore
    /// Row::new().hint_placement(HintPlacement::CenterRight)
    /// ```
    ///
    /// [`Base::hint_style`](crate::Base::hint_style) has always been universal — every widget that
    /// wears a letter has one — but the only way to *set* it was to wrap the widget in a
    /// [`KeyHint`](crate::widgets::KeyHint). That is the wrapper rule again: it puts the knowledge
    /// in every caller's discipline, and it cannot be done at all to a widget a typed container
    /// holds, because wrapping it changes what it is — the same defect that made the tooltip a
    /// property. One builder, on the widget.
    ///
    /// The wrapper writes these same four fields, so there is one slot and one rule rather than two
    /// that can drift.
    #[heca_grid_ui_macros::prop]
    fn hint_placement(mut self, placement: crate::widgets::HintPlacement) -> Self {
        self.base_mut().hint_style.placement = placement;
        self
    }

    /// The keycap's font size in logical px. Unset = derived from the widget's resolved font, which
    /// is what keeps a letter proportional to the thing it captions.
    #[heca_grid_ui_macros::prop]
    fn hint_size(mut self, px: f32) -> Self {
        self.base_mut().hint_style.size = Some(px);
        self
    }

    /// The keycap's colour, glow included. Unset = the theme's `accent`, so a letter follows a
    /// theme change with nothing rewritten.
    ///
    /// Prefer [`hint_tone`](ComponentExt::hint_tone) where a *meaning* fits: a literal does not
    /// follow a theme reload, and a widget has no theme at build time to take one from.
    #[heca_grid_ui_macros::prop]
    fn hint_color(mut self, color: Color) -> Self {
        self.base_mut().hint_style.color = Some(color);
        self
    }

    /// **What this widget's keycap MEANS** — the theme picks the colour.
    ///
    /// `Accent` is a place to go, `Muted` a structural control (fold this, close that), and
    /// `Warning` / `Success` / `Danger` are further classes so two kinds never read alike. A
    /// literal via [`hint_color`](ComponentExt::hint_color) still wins where one is set, but a
    /// widget composing itself has no theme to take a literal from — which is why a meaning is the
    /// thing it can say.
    #[heca_grid_ui_macros::prop]
    fn hint_tone(mut self, tone: crate::widgets::HintTone) -> Self {
        self.base_mut().hint_style.tone = Some(tone);
        self
    }

    /// A vertical nudge applied **after** placement — positive moves the cap down. What drops a
    /// [`TopRight`](crate::widgets::HintPlacement::TopRight) cap onto a dock's header line.
    #[heca_grid_ui_macros::prop]
    fn hint_offset_y(mut self, px: f64) -> Self {
        self.base_mut().hint_style.offset_y = px;
        self
    }

    fn on_action(mut self, name: impl Into<String>, f: impl Fn() + 'static) -> Self {
        self.base_mut().actions.push(crate::hint::DeclaredAction {
            name: name.into(),
            run: Box::new(f),
        });
        self
    }
}

/// Every component gets them — the point of the design: put a widget in a tree and it works, with
/// nothing to opt into.
impl<T: Component + Sized> ComponentExt for T {}

#[cfg(test)]
mod one_builder_per_slot {
    use super::*;
    use crate::widgets::{Flex, Label};

    fn subtree() -> Box<dyn Component> {
        Box::new(Label::new("realized"))
    }

    /// **A widget and an already-built subtree go through the SAME builder.**
    ///
    /// `Box<dyn Component>` is not itself a `Component`, and for that one reason every
    /// child-taking builder came in twos — `child`/`child_boxed`, `body`/`body_boxed`,
    /// `leading`/`leading_boxed`, sixteen in all. A caller holding a dynamically built subtree,
    /// which is what `realize()` returns from a description, had to know which spelling to reach
    /// for; a widget author had to remember to write both, and a new slot that forgot its twin
    /// was simply unreachable from the declarative side with nothing to report it.
    #[test]
    fn a_boxed_subtree_and_a_widget_take_the_same_builder() {
        let from_widget = Flex::column().child(Label::new("realized"));
        let from_boxed = Flex::column().child(subtree());
        assert_eq!(from_widget.base().children.len(), 1);
        assert_eq!(from_boxed.base().children.len(), 1);
        assert_eq!(
            from_boxed.base().children[0].text_summary(),
            from_widget.base().children[0].text_summary(),
            "the same subtree, whichever way it arrived",
        );
    }

    /// **A subtree that is already boxed is not boxed again.** The whole point of the second
    /// builder was to avoid that, and the merged one must not give it back: `Box<Box<dyn …>>`
    /// would still work and still walk, so nothing would fail — it would only cost an allocation
    /// and a pointer hop per realized child, invisibly.
    #[test]
    fn an_already_boxed_subtree_is_handed_through_rather_than_wrapped() {
        let boxed = subtree();
        let addr = (&*boxed) as *const dyn Component as *const () as usize;
        let parent = Flex::column().child(boxed);
        let child = &parent.base().children[0];
        let after = (&**child) as *const dyn Component as *const () as usize;
        assert_eq!(addr, after, "the same allocation, not a box around a box");
    }

    /// The named slots take it too — a header, a body, a leading icon. One builder each.
    #[test]
    fn a_named_slot_takes_a_boxed_subtree_through_its_own_builder() {
        use crate::widgets::Item;
        let it = Item::new("row").leading(subtree()).trailing(Label::new("x"));
        assert_eq!(it.base().children.len(), 3, "leading, label, trailing");
    }
}

#[cfg(test)]
mod spacing_builders {
    use super::*;
    use crate::style::Spacing;
    use crate::widgets::Flex;

    /// **A step and a number go through the same builder and land in different places** — which is
    /// the whole point: a step is resolved against the inherited font at layout, a number is not.
    #[test]
    fn one_builder_writes_whichever_kind_it_was_given() {
        let px = Flex::row().gap(8);
        assert_eq!(px.base().style.layout.gap, 8.0);
        assert_eq!(px.base().style.layout.gap_spacing, None);

        let step = Flex::row().gap(Spacing::Sm);
        assert_eq!(step.base().style.layout.gap_spacing, Some(Spacing::Sm));
    }

    /// **Saying it again the other way replaces it**, rather than leaving both set with the token
    /// silently winning at layout — which is what two separate builders allowed.
    #[test]
    fn a_number_after_a_step_really_is_a_number() {
        let w = Flex::row().gap(Spacing::Lg).gap(4);
        assert_eq!(w.base().style.layout.gap, 4.0);
        assert_eq!(
            w.base().style.layout.gap_spacing,
            None,
            "the step has to be cleared, or layout resolves it over the number",
        );
    }

    /// Padding behaves the same, on every axis.
    #[test]
    fn padding_takes_either_kind_per_axis() {
        let w = Flex::row().padding_x(12).padding_y(Spacing::Xs);
        assert_eq!(w.base().style.layout.padding_x, Some(12.0));
        assert_eq!(w.base().style.layout.pad_spacing_x, None);
        assert_eq!(w.base().style.layout.pad_spacing_y, Some(Spacing::Xs));
    }
}
