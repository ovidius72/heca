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
        s.margin_left = Some(x.into());
        s.margin_right = Some(x.into());
        s.margin_top = Some(y.into());
        s.margin_bottom = Some(y.into());
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
    /// **Place this widget at a rect of its parent**, instead of letting it flow with its siblings.
    ///
    /// The host often knows *where* something goes while the widget still owns *what it looks
    /// like*: a floating pane drawn over the strip it belongs to, a chip pinned to a pane's corner,
    /// a badge over a cell. The temptation is to measure it yourself and call `cx.rect` + `cx.text`
    /// — don't. Name the rect and the engine places it.
    ///
    /// ```no_run
    /// # use heca_grid_ui::prelude::*;
    /// # use heca_grid_ui::style::Length::Pct;
    /// # let (x, y, w, h, strip_w, screen_h) = (200.0, 100.0, 400.0, 300.0, 1600.0, 900.0);
    /// # let card = Label::new("float");
    /// // A floating pane at its own fraction of the workspace behind it.
    /// let placed = card.at_rect(Pct(x / strip_w), Pct(y / screen_h), Pct(w / strip_w), Pct(h / screen_h));
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
    fn min_height(mut self, h: Length) -> Self {
        self.base_mut().style.layout.min_height = Some(h);
        self
    }

    /// Floor for the width.
    fn min_width(mut self, w: Length) -> Self {
        self.base_mut().style.layout.min_width = Some(w);
        self
    }

    /// Ceiling for the height — a box that may not grow past it however tall its content is.
    fn max_height(mut self, h: Length) -> Self {
        self.base_mut().style.layout.max_height = Some(h);
        self
    }

    /// Ceiling for the width, the counterpart to [`min_width`](LayoutExt::min_width).
    ///
    /// A panel sized to its content wants both: a floor so a one-word menu is not a sliver, and a
    /// ceiling so one long row does not stretch it across the screen. Past the ceiling the content
    /// is the child's problem — a [`Label`](crate::widgets::Label) with
    /// [`truncate`](crate::widgets::Label::truncate) cuts, anything else overflows.
    fn max_width(mut self, w: Length) -> Self {
        self.base_mut().style.layout.max_width = Some(w);
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
/// Row::new().context_menu(move || build_menu(7));     // a closure
/// ```
///
/// A [`ContextMenu`](crate::widgets::ContextMenu) is `Clone` — its content is plain data and `Rc`
/// closures — so the value form is a clone per opening, not a shared panel. Reach for the closure
/// when the menu's rows depend on state this widget's tree is not rebuilt on, or when building it
/// eagerly would be wasted work.
pub trait IntoContextMenu {
    /// Produce the menu to show. Called **each time** the menu is triggered.
    fn build(&self) -> crate::widgets::ContextMenu;
}

impl IntoContextMenu for crate::widgets::ContextMenu {
    fn build(&self) -> crate::widgets::ContextMenu {
        self.clone()
    }
}

impl<F: Fn() -> crate::widgets::ContextMenu> IntoContextMenu for F {
    fn build(&self) -> crate::widgets::ContextMenu {
        self()
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
    /// let other = Row::new().context_menu(move || ctx.clone());
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
        self.base_mut().context_menu = Some(Box::new(move || menu.build()));
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
