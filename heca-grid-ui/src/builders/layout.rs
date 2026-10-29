//! **Arrangement**: how a container lays out itself and its children.

use crate::component::Component;
use crate::reactive::SignalUpdate;
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
    /// ([`Layout::size_explicit`](crate::style::Layout::size_explicit)), which both pins this
    /// widget's variant and makes **it** the one its own children inherit.
    fn size(mut self, size: WidgetSize) -> Self {
        self.authored_style_mut().layout.set_size(size);
        self
    }
    /// Preferred width / height. Automatic dimensions follow it; two fixed dimensions win.
    /// Non-finite or non-positive values remove the preference.
    fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.authored_style_mut().layout.aspect_ratio = (ratio.is_finite() && ratio > 0.0).then_some(ratio);
        self
    }
    /// Main-axis direction.
    fn direction(mut self, d: impl Into<Direction>) -> Self {
        self.authored_style_mut().layout.direction = d.into();
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
        self.authored_style_mut().layout.gap = v.into();
        self
    }
    /// Outer margin on all sides — a number of pixels or a step, like [`gap`](Self::gap).
    fn margin(mut self, m: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.margin = m.into();
        self
    }
    /// Outer margin split per axis: `x` left+right, `y` top+bottom.
    fn margin_xy(mut self, x: f32, y: f32) -> Self {
        let s = &mut self.authored_style_mut().layout;
        s.margin_left = Some(x.into());
        s.margin_right = Some(x.into());
        s.margin_top = Some(y.into());
        s.margin_bottom = Some(y.into());
        self
    }
    /// Horizontal outer margin (left+right) only.
    fn margin_x(mut self, v: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.margin_x = Some(v.into());
        self
    }
    /// Vertical outer margin (top+bottom) only — a rule breathing away from what it separates.
    fn margin_y(mut self, v: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.margin_y = Some(v.into());
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
        self.authored_style_mut().layout.placement = Some(crate::style::Placement {
            left: left.into(),
            top: top.into(),
            width: width.into(),
            height: height.into(),
        });
        self
    }
    /// Left outer margin only.
    fn margin_left(mut self, v: impl Into<crate::style::Length>) -> Self {
        self.authored_style_mut().layout.margin_left = Some(v.into());
        self
    }
    /// Right outer margin only.
    fn margin_right(mut self, v: impl Into<crate::style::Length>) -> Self {
        self.authored_style_mut().layout.margin_right = Some(v.into());
        self
    }
    /// Top outer margin only.
    fn margin_top(mut self, v: impl Into<crate::style::Length>) -> Self {
        self.authored_style_mut().layout.margin_top = Some(v.into());
        self
    }
    /// Bottom outer margin only.
    fn margin_bottom(mut self, v: impl Into<crate::style::Length>) -> Self {
        self.authored_style_mut().layout.margin_bottom = Some(v.into());
        self
    }
    /// Main-axis distribution.
    fn justify(mut self, j: impl Into<Justify>) -> Self {
        self.authored_style_mut().layout.justify = j.into();
        self
    }
    /// Cross-axis alignment of this component's **children**.
    fn align(mut self, a: impl Into<Align>) -> Self {
        self.authored_style_mut().layout.align = a.into();
        self
    }
    /// Cross-axis alignment of **this** component inside its parent (CSS `align-self`),
    /// overriding the parent's [`align`](Self::align) for it alone. Use
    /// [`Align::Start`] to keep an `Auto`-sized widget hugging its content instead of
    /// stretching to fill the parent.
    fn align_self(mut self, a: impl Into<Align>) -> Self {
        self.authored_style_mut().layout.align_self = Some(a.into());
        self
    }
    /// **Grid only** — how this grid's items sit **horizontally inside their cells**
    /// (CSS `justify-items`). Default: `Stretch` (an item fills its cell).
    ///
    /// Not to be confused with [`justify`](Self::justify): on a grid that is `justify-content`,
    /// which distributes the whole *track set* inside the container and leaves the items where
    /// they are. The vertical counterpart is [`align`](Self::align).
    fn justify_items(mut self, a: impl Into<Align>) -> Self {
        self.authored_style_mut().layout.justify_items = Some(a.into());
        self
    }
    /// **Grid only** — horizontal placement of **this** item inside its own cell (CSS
    /// `justify-self`), overriding the grid's [`justify_items`](Self::justify_items) for it alone.
    fn justify_self(mut self, a: impl Into<Align>) -> Self {
        self.authored_style_mut().layout.justify_self = Some(a.into());
        self
    }
    /// **Inner padding on all sides** — pixels or a step, exactly as [`gap`](LayoutExt::gap).
    fn padding(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.padding = p.into();
        self
    }
    /// **Inner padding per axis**: `x` left+right, `y` top+bottom. Pixels or a step, each.
    fn padding_xy(
        mut self,
        x: impl Into<crate::style::Space>,
        y: impl Into<crate::style::Space>,
    ) -> Self {
        {
            let l = &mut self.authored_style_mut().layout;
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
        self.authored_style_mut().layout.padding_x = Some(p.into());
        self
    }

    /// **Inner padding top+bottom** — see [`padding_x`](LayoutExt::padding_x).
    fn padding_y(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.padding_y = Some(p.into());
        self
    }

    /// Inner padding on one side, overriding the axis and the uniform value.
    ///
    /// Reserving space along a single edge is not the same as padding the axis: the opposite side
    /// should not move because this one needed room. A `ScrollRegion` keeping its content clear of
    /// its scrollbar is the case that asked for it.
    fn padding_left(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.padding_left = Some(p.into());
        self
    }
    /// Inner padding on the right only — see [`padding_left`](Self::padding_left).
    fn padding_right(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.padding_right = Some(p.into());
        self
    }
    /// Inner padding on the top only — see [`padding_left`](Self::padding_left).
    fn padding_top(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.padding_top = Some(p.into());
        self
    }
    /// Inner padding on the bottom only — see [`padding_left`](Self::padding_left).
    fn padding_bottom(mut self, p: impl Into<crate::style::Space>) -> Self {
        self.authored_style_mut().layout.padding_bottom = Some(p.into());
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
        self.authored_style_mut().layout.width = w.into();
        self
    }
    /// Height along the main/cross axis — the same spellings as [`width`](LayoutExt::width).
    fn height(mut self, h: impl Into<Length>) -> Self {
        self.authored_style_mut().layout.height = h.into();
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
        self.authored_style_mut().layout.wrap = wrap;
        self
    }

    fn shrink(mut self, s: f32) -> Self {
        self.authored_style_mut().layout.flex_shrink = Some(s);
        self
    }

    /// Floor for the height — a row that must stay legible however many share the space.
    /// Same spellings as [`width`](LayoutExt::width).
    fn min_height(mut self, h: impl Into<Length>) -> Self {
        self.authored_style_mut().layout.min_height = Some(h.into());
        self
    }

    /// Floor for the width. Same spellings as [`width`](LayoutExt::width).
    fn min_width(mut self, w: impl Into<Length>) -> Self {
        self.authored_style_mut().layout.min_width = Some(w.into());
        self
    }

    /// Ceiling for the height — a box that may not grow past it however tall its content is.
    /// Same spellings as [`width`](LayoutExt::width).
    fn max_height(mut self, h: impl Into<Length>) -> Self {
        self.authored_style_mut().layout.max_height = Some(h.into());
        self
    }

    /// Ceiling for the width, the counterpart to [`min_width`](LayoutExt::min_width).
    ///
    /// A panel sized to its content wants both: a floor so a one-word menu is not a sliver, and a
    /// ceiling so one long row does not stretch it across the screen. Past the ceiling the content
    /// is the child's problem — a [`Label`](crate::widgets::Label) with
    /// [`truncate`](crate::widgets::Label::truncate) cuts, anything else overflows.
    fn max_width(mut self, w: impl Into<Length>) -> Self {
        self.authored_style_mut().layout.max_width = Some(w.into());
        self
    }

    fn grow(mut self, g: f32) -> Self {
        self.authored_style_mut().layout.flex_grow = g;
        self
    }

    /// **CSS `flex: <n>` — take `n` parts of the room the parent has to give.**
    ///
    /// Said by the **child**, about itself, exactly where CSS says it. The parent does not count
    /// its children or write a size per child, so a child added later — a plugin's dock appended to
    /// a sidebar — brings its own size and the rest keep theirs.
    ///
    /// ```ignore
    /// Flex::column()
    ///     .child(docker.flex(1.0))       // ┐
    ///     .child(workspaces.flex(3.0))   // ├ 1 : 3 : 1 of the column, whatever each one holds
    ///     .child(notes.flex(1.0))        // ┘
    ///     .child(git.flex(0.0));         //   as tall as its content, and no more
    /// ```
    ///
    /// - **`n > 0`** — `n` parts of what the parent has, along its direction (height in a column,
    ///   width in a row). `1` beside `3` is a quarter and three quarters. A child holding more
    ///   than its part keeps its part and scrolls or clips inside it; it never pushes its
    ///   neighbours out.
    /// - **`0`** — as big as its content (CSS `flex: none`).
    /// - **in a [`Grid`](crate::widgets::Grid)** — nothing. The grid's own tracks size its cells,
    ///   as in CSS. Place a grid child with `.row` / `.column` / `.area` instead.
    ///
    /// **Why not [`grow`](LayoutExt::grow)?** `grow` is CSS `flex-grow` alone: it divides only the
    /// space left after every child took its content, so two children asked for 2:1 but holding
    /// different amounts land at 606/294 in 900px. `flex` is grow **plus** a zero start and
    /// permission to shrink — CSS `flex: <n> 1 0` — which is what "n parts" means.
    ///
    /// Declarative form: the `flex` property, `{"flex": 3}`. Stored in
    /// [`Layout::flex`](crate::style::Layout::flex), applied during layout against the real parent.
    fn flex(mut self, parts: f32) -> Self {
        self.authored_style_mut().layout.flex = Some(parts);
        self
    }

    /// **CSS `order` — where this child is laid out among its siblings.** Lower first; siblings
    /// that tie, or say nothing (`0`), keep the order they were added in.
    ///
    /// ```ignore
    /// Flex::column()
    ///     .child(notes)                  // 0, as added
    ///     .child(docker.order(-1))       // before everything that said nothing
    ///     .child(git.order([0, 5]))      // after every plain 0, before any 1
    /// ```
    ///
    /// A number or a list, through one parser (`3`, `[0, 5]`, `"0 5"`). A list compares a place at
    /// a time, so `[0, 5]` slots between two siblings at `0` and `1` that you do not own — which a
    /// whole number cannot. See [`Order`](crate::order::Order).
    ///
    /// **Visual only**, as in CSS: it moves where the child is laid out, not where it is in the
    /// tree, so paint order, Tab order and the order the picker hands out letters are unchanged. In
    /// a
    /// [`Grid`](crate::widgets::Grid) it orders what the grid places itself; a child placed with
    /// `.row` / `.column` / `.area` stays where it said.
    ///
    /// Declarative form: the `order` property — `{"order": 3}` or `{"order": [0, 5]}`.
    fn order(mut self, order: impl Into<crate::order::Order>) -> Self {
        self.authored_style_mut().layout.order = Some(order.into());
        self
    }

    /// **What this item starts from before it grows or shrinks** (CSS `flex-basis`) — pixels, a
    /// percentage, or `"auto"`.
    ///
    /// **A share of its container is `.grow(w).basis(0.0).shrink(1.0)`** — CSS `flex: 1 1 0`.
    /// Reach for this rather than `.height(0.0)`: a zero *height* says the box is zero, which is a
    /// different answer to anything measuring the container's content.
    fn basis(mut self, b: impl Into<crate::style::Length>) -> Self {
        self.authored_style_mut().layout.flex_basis = Some(b.into());
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
