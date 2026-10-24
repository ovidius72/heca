//! **A component's style** and its arrangement half, mapped to taffy.

use super::*;

/// A component's style: two peer halves, [`layout`](Self::layout) and [`visual`](Self::visual).
///
/// The split says what a value **means**, not what may set it — a distinction the library lived by
/// before plugins existed, since `AGENTS.md` requires every widget to read colours, fonts and radii
/// from the [`Theme`](crate::theme::Theme) and hardcode nothing.
///
/// - [`Layout`] — arrangement plus the semantic [`size`](Layout::size) variant.
/// - [`Visual`] — appearance: what the theme decides unless someone says otherwise.
///
/// **Both halves are serializable, and a description may set either.** The theme is the default,
/// not a wall: unset means "ask the theme", which is what the `Option` fields and the
/// `0.0 = inherit` sentinels mean.
///
/// Both are peers on purpose: neither half is privileged, and a field added to either one is
/// reachable with no further action. Builder methods
/// ([`LayoutExt`](crate::builders::LayoutExt) / [`StyleExt`](crate::builders::StyleExt)) write
/// through to the correct half, so callers never name it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Style {
    /// Arrangement + the semantic size variant — the caller-owned half.
    pub layout: Layout,
    /// Appearance — the theme-owned half.
    pub visual: Visual,
}

/// The **caller-owned arrangement** half of [`Style`] — how a component sits and how big it is.
///
/// Serializable, so a declarative description may set any of it; see [`Visual`] for the half that
/// is not. [`size`](Self::size) lives here rather than in `Visual` because it is *semantic*
/// (`Small`/`Normal`/`Big`) rather than a pixel value, and because the layout pass both reads it
/// and cascades it to children.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    // ── Arrangement ──
    pub direction: Direction,
    pub justify: Justify,
    pub align: Align,
    /// Cross-axis alignment of **this** node inside its parent, overriding the parent's
    /// [`align`](Self::align) for it alone (CSS `align-self`). `None` ⇒ follow the parent.
    ///
    /// The reason it exists: the default [`Align::Stretch`] makes an `Auto`-sized node fill its
    /// parent across the cross axis, so a content-hugging control (a [`Select`](crate::widgets::Select),
    /// which sizes itself to its widest option) would silently go full-width inside a column. A
    /// widget that must hug sets `Some(Align::Start)` and keeps its intrinsic size in either
    /// direction of parent.
    pub align_self: Option<Align>,
    /// **Grid only** — how this container's items are placed **horizontally inside their cell**
    /// (CSS `justify-items`). `None` ⇒ taffy's default (`Stretch`: an item fills its cell).
    ///
    /// Do **not** reach for [`justify`](Self::justify) here: on a grid that is `justify-content`,
    /// which distributes the whole *track set* inside the container — it does not move the items
    /// within their cells. Same word, different axis of meaning; that is exactly why this exists.
    pub justify_items: Option<Align>,
    /// **Grid only** — horizontal placement of **this** item inside its own cell, overriding the
    /// parent's [`justify_items`](Self::justify_items) for it alone (CSS `justify-self`).
    pub justify_self: Option<Align>,
    /// Space between children — a number of pixels or a step of the theme's rhythm. **Prefer the
    /// step**: it is resolved from the inherited font at layout, so it scales with the font, the
    /// size variant and UI zoom.
    pub gap: Space,
    /// Uniform outer margin (all sides), unless overridden per side by
    /// [`margin_left`](Self::margin_left) / [`margin_right`](Self::margin_right)
    /// / [`margin_top`](Self::margin_top) / [`margin_bottom`](Self::margin_bottom).
    pub margin: Space,
    /// Horizontal (left+right) margin override; `None` ⇒ use [`margin`](Self::margin).
    ///
    /// The axis shorthands exist for parity with [`padding_x`](Self::padding_x) /
    /// [`padding_y`](Self::padding_y): without them a **described** tree could set padding by axis
    /// but had to name both sides for a margin. A `Separator` wanting to breathe on one axis is the
    /// case that found it.
    pub margin_x: Option<Space>,
    /// Vertical (top+bottom) margin override; `None` ⇒ use [`margin`](Self::margin).
    pub margin_y: Option<Space>,
    /// Left margin override; `None` ⇒ [`margin_x`](Self::margin_x), then [`margin`](Self::margin).
    pub margin_left: Option<Length>,
    /// Right margin override; `None` ⇒ [`margin_x`](Self::margin_x), then [`margin`](Self::margin).
    pub margin_right: Option<Length>,
    /// Top margin override; `None` ⇒ [`margin_y`](Self::margin_y), then [`margin`](Self::margin).
    pub margin_top: Option<Length>,
    /// Bottom margin override; `None` ⇒ [`margin_y`](Self::margin_y), then [`margin`](Self::margin).
    pub margin_bottom: Option<Length>,
    /// Uniform inner padding (all sides), unless overridden per axis by
    /// [`padding_x`](Self::padding_x) / [`padding_y`](Self::padding_y).
    pub padding: Space,
    /// Horizontal (left+right) padding override; `None` ⇒ use [`padding`](Self::padding).
    pub padding_x: Option<Space>,
    /// Vertical (top+bottom) padding override; `None` ⇒ use [`padding`](Self::padding).
    pub padding_y: Option<Space>,
    /// Left padding override; `None` ⇒ use [`padding_x`](Self::padding_x), then
    /// [`padding`](Self::padding). Mirrors the per-side margins.
    pub padding_left: Option<Space>,
    /// Right padding override; `None` ⇒ [`padding_x`](Self::padding_x), then [`padding`](Self::padding).
    ///
    /// This is what lets a widget reserve space along one edge without moving the opposite one — a
    /// [`ScrollRegion`](crate::widgets::ScrollRegion) keeping its content clear of the scrollbar,
    /// for instance, where padding the whole axis would inset the far side for no reason.
    pub padding_right: Option<Space>,
    /// Top padding override; `None` ⇒ [`padding_y`](Self::padding_y), then [`padding`](Self::padding).
    pub padding_top: Option<Space>,
    /// Bottom padding override; `None` ⇒ [`padding_y`](Self::padding_y), then [`padding`](Self::padding).
    pub padding_bottom: Option<Space>,
    pub width: Length,
    pub height: Length,
    /// Minimum size. `None` ⇒ taffy's default, which for a flex item is
    /// **`auto` = its content size** — i.e. it will *not* shrink below its
    /// content. Set `Px(0.0)` to allow shrinking, which a scrolling viewport
    /// ([`ScrollRegion`](crate::widgets::ScrollRegion)) needs: without it a
    /// region in a bounded panel overflows its parent instead of scrolling
    /// (the classic flexbox `min-height: auto` trap).
    pub min_width: Option<Length>,
    /// Minimum height — see [`min_width`](Layout::min_width).
    pub min_height: Option<Length>,
    /// Maximum width. `None` ⇒ unbounded. Used to cap a node against its parent —
    /// an overlay panel is capped at `Percent(1.0)` so a fixed `Px` size can never
    /// make a dialog larger than the window.
    pub max_width: Option<Length>,
    /// Maximum height — see [`max_width`](Layout::max_width).
    pub max_height: Option<Length>,
    /// Preferred width / height. Used when a dimension is automatic; fixed dimensions win.
    pub aspect_ratio: Option<f32>,
    pub flex_grow: f32,

    /// **CSS `flex: <n>`** — take `n` parts of the room the parent has to give. Set it with
    /// [`LayoutExt::flex`](crate::builders::LayoutExt::flex); `None` means the widget never asked.
    ///
    /// | value | means |
    /// |---|---|
    /// | `1.0` beside `3.0` beside `1.0` | a fifth, three fifths, a fifth — **whatever each holds** |
    /// | `0.0` | as big as its content, no part of the rest (CSS `flex: none`) |
    ///
    /// What it does depends on the parent, which is the whole reason it is a property and not an
    /// idiom a caller writes:
    /// - **in any parent that is not a grid** — `Flex`, `Surface`, `Pane`, `Card`, all flex boxes
    ///   underneath — it becomes `flex: <n> 1 0`: grow by `n`, start from zero, allowed to shrink.
    ///   All three are needed. Grow alone divides only the space left after every child took its
    ///   content, so a 2:1 pair holding different amounts lands at 606/294 in a 900px box instead of
    ///   600/300, and a column of them collapses to its content instead of splitting the box. The
    ///   minimum size **along the parent's direction** also drops to zero, so a child with more
    ///   content than its part keeps its part instead of pushing its neighbours out;
    /// - **in a grid** it is nothing at all. The grid's tracks already sized the cell, and the item
    ///   stretches into it — as in CSS, where `flex` on a grid item does nothing.
    ///
    /// Writing the flex spelling by hand is what broke two docks in one sidebar: a zero base size
    /// is a *definite zero height* in a grid cell, so each container drew its title row and nothing
    /// else. Resolved in [`crate::layout`], against the parent, so no caller has to know which case
    /// they are in.
    pub flex: Option<f32>,
    /// **CSS `order`** — where this child is laid out among its siblings: lower first, ties in the
    /// order they were added. `None` is `0`. Set it with
    /// [`LayoutExt::order`](crate::builders::LayoutExt::order); see [`Order`](crate::order::Order).
    ///
    /// Visual only, as in CSS: paint, Tab and the letter picker keep the order children were added.
    pub order: Option<crate::order::Order>,
    /// Flex shrink factor. `None` ⇒ **`1.0`**, as flexbox has it: an item gives way when its line
    /// is too small, and a widget that must **not** be squeezed opts out with `Some(0.0)`.
    ///
    /// It was `0.0` — "widgets use explicit sizes and a flex container must never squish them" —
    /// which inverted the rarer case onto every author. Nothing gave way unless someone remembered
    /// to ask, so a composition simply kept its content width and overflowed whatever held it: one
    /// defect wearing many faces (a card wider than the strip it is a share of, a folder path
    /// pushing a name out of its card, three cards' text drawn over each other in a narrow window),
    /// and the reason `Label::truncate` had to switch shrinking on before its own cut could ever be
    /// reached. Held honest by the two sweeps in `heca-view-realize`: nothing paints outside its
    /// box, and a widget keeps its natural size when there is room.
    pub flex_shrink: Option<f32>,

    /// **What this item starts from, before it grows or shrinks** (CSS `flex-basis`). `None` ⇒
    /// `auto`, flexbox's default: start from the item's own size, else its content.
    ///
    /// **A share of its container is `.grow(w).basis(0.0).shrink(1.0)`** — CSS `flex: 1 1 0`.
    ///
    /// ⚠️ It is **not** the same as setting `height`/`width` to zero, which is how that idiom had
    /// to be spelled before this existed. A zero basis says "start from nothing, then take your
    /// weight of the room"; a definite zero *height* says the box **is** zero, which is a different
    /// answer to anything that measures the container's content.
    pub flex_basis: Option<Length>,

    /// Overall size variant — scales font + intrinsic padding together. Composes
    /// with [`Visual::font_scale`] (both multiply the base font).
    ///
    /// **Inherited down the tree** (like the base font): a node that never called
    /// [`LayoutExt::size`](crate::builders::LayoutExt::size) adopts its parent's variant during
    /// layout, so a `Small` button's composed content (`Icon`/`Label`, at any depth) shrinks with
    /// it. A node that *did* set one keeps it — see [`size_explicit`](Self::size_explicit).
    pub size: WidgetSize,
    /// Whether [`size`](Self::size) was set **explicitly** by the caller (via
    /// [`LayoutExt::size`](crate::builders::LayoutExt::size)) rather than left at its default.
    ///
    /// This exists because `size` is not an `Option`: its default (`Normal`) is
    /// indistinguishable from an explicit `.size(WidgetSize::Normal)`, so the layout pass could
    /// not otherwise know whether it may overwrite the field with the inherited variant. `false`
    /// ⇒ inherit from the parent; `true` ⇒ keep this node's own (and pass **it** to the node's
    /// children).
    pub size_explicit: bool,
    /// When true the node is removed from layout entirely (`display: none`) — it
    /// takes no space and paints nothing. Used by collapsible containers
    /// (e.g. [`ItemGroup`](crate::widgets::ItemGroup)) to fold rows away.
    pub hidden: bool,
    /// **Children that do not fit on one line start another** (CSS `flex-wrap: wrap`).
    ///
    /// A row of controls is the case: three buttons in a card narrower than their sum have to go
    /// somewhere, and the alternatives are both wrong — squeezing every button until its label is
    /// an ellipsis, or laying the overflow out past the edge. `false` (one line) stays the default,
    /// because for most rows — a leading slot, a label and a trailing slot — a second line would be
    /// nonsense.
    pub wrap: bool,
    /// Placement when this component is a child of a [`Grid`](crate::widgets::Grid).
    /// `None` ⇒ grid auto-placement. Set by `Grid::cell`/`Grid::area`.
    pub grid_cell: Option<GridCell>,
    /// **Placed at a rect of the parent instead of flowing** — `None` ⇒ an ordinary in-flow child.
    ///
    /// Set by [`LayoutExt::at_rect`](crate::builders::LayoutExt::at_rect). It overrides
    /// [`width`](Self::width) / [`height`](Self::height), because a rect names both, and it takes
    /// the node out of its parent's flow, so it neither takes space from its siblings nor is moved
    /// by them.
    pub placement: Option<Placement>,
}

impl Layout {
    /// Choose the [size variant](Self::size) **explicitly**, marking it as the caller's choice.
    ///
    /// This is the single place explicitness is recorded: layout then leaves this node's variant
    /// alone (instead of replacing it with the parent's) and passes **this** variant down to the
    /// node's children. [`LayoutExt::size`](crate::builders::LayoutExt::size) is the public
    /// builder over it; widgets that expose their own `size(..)` for something else (e.g.
    /// [`Icon::size`](crate::widgets::Icon::size), which takes glyph pixels) reach the variant
    /// through here. Assigning [`size`](Self::size) directly does **not** mark it explicit, so the
    /// layout pass will overwrite it.
    pub fn set_size(&mut self, size: WidgetSize) {
        self.size = size;
        self.size_explicit = true;
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            direction: Direction::Row,
            justify: Justify::Start,
            align: Align::Stretch,
            align_self: None,
            justify_items: None,
            justify_self: None,
            gap: Space::Px(0.0),
            margin: Space::Px(0.0),
            margin_x: None,
            margin_y: None,
            margin_left: None,
            margin_right: None,
            margin_top: None,
            margin_bottom: None,
            padding: Space::Px(0.0),
            padding_x: None,
            padding_y: None,
            padding_left: None,
            padding_right: None,
            padding_top: None,
            padding_bottom: None,
            width: Length::Auto,
            height: Length::Auto,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            aspect_ratio: None,
            flex_grow: 0.0,
            flex: None,
            order: None,
            flex_shrink: None,
            flex_basis: None,
            size: WidgetSize::Normal,
            // Not explicitly chosen ⇒ the layout pass may replace it with the parent's variant.
            size_explicit: false,
            hidden: false,
            wrap: false,
            grid_cell: None,
            placement: None,
        }
    }
}

impl Layout {
    /// Effective left padding in px. **Most specific wins:** the side, else its axis, else the
    /// uniform value — the same cascade the layout pass applies.
    ///
    /// These four exist so paint code can ask the question layout already answered, instead of
    /// re-deriving the cascade and drifting from it. A widget that insets a highlight or a marker
    /// needs to know where its content box starts, and by paint time the `pad_spacing_*` tokens
    /// have already been resolved into the px fields these read.
    pub fn pad_left(&self, font_px: f32) -> f32 {
        self.padding_left
            .unwrap_or_else(|| self.padding_x.unwrap_or(self.padding))
            .resolve(font_px)
    }

    /// Effective right padding in px — see [`pad_left`](Self::pad_left).
    pub fn pad_right(&self, font_px: f32) -> f32 {
        self.padding_right
            .unwrap_or_else(|| self.padding_x.unwrap_or(self.padding))
            .resolve(font_px)
    }

    /// Effective top padding in px — see [`pad_left`](Self::pad_left).
    pub fn pad_top(&self, font_px: f32) -> f32 {
        self.padding_top
            .unwrap_or_else(|| self.padding_y.unwrap_or(self.padding))
            .resolve(font_px)
    }

    /// Effective bottom padding in px — see [`pad_left`](Self::pad_left).
    pub fn pad_bottom(&self, font_px: f32) -> f32 {
        self.padding_bottom
            .unwrap_or_else(|| self.padding_y.unwrap_or(self.padding))
            .resolve(font_px)
    }

    /// Map the layout fields onto a `taffy::Style` for the layout engine.
    pub fn to_taffy(&self, font_px: f32) -> taffy::Style {
        use taffy::prelude::*;
        if self.hidden {
            return taffy::Style {
                display: Display::None,
                ..Default::default()
            };
        }
        taffy::Style {
            display: Display::Flex,
            aspect_ratio: self.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0)
                .filter(|_| matches!(self.width, Length::Auto) || matches!(self.height, Length::Auto)),
            flex_direction: match self.direction {
                Direction::Row => FlexDirection::Row,
                Direction::Column => FlexDirection::Column,
            },
            justify_content: Some(self.justify.to_taffy()),
            align_items: Some(self.align.to_taffy()),
            align_self: self.align_self.map(|a| a.to_taffy()),
            // Grid-only (taffy ignores them on a flex container).
            justify_items: self.justify_items.map(|a| a.to_taffy()),
            justify_self: self.justify_self.map(|a| a.to_taffy()),
            gap: {
                let g = self.gap.resolve(font_px);
                Size {
                    width: length(g),
                    height: length(g),
                }
            },
            margin: {
                // Most specific wins: a side, else its axis, else the uniform value — the same
                // cascade padding has.
                let mx = self.margin_x.unwrap_or(self.margin).resolve(font_px);
                let my = self.margin_y.unwrap_or(self.margin).resolve(font_px);
                // A side may be a **percentage** of the parent, which is what lets a caller place
                // a box at a proportional position — a floating pane in the exposé sits at
                // `x / strip_width` of its row, with no pixel scale anywhere.
                let side = |v: Option<Length>, axis: f32| {
                    v.map_or_else(|| length(axis), |v| v.to_taffy_inset(font_px))
                };
                Rect {
                    left: side(self.margin_left, mx),
                    right: side(self.margin_right, mx),
                    top: side(self.margin_top, my),
                    bottom: side(self.margin_bottom, my),
                }
            },
            // Most specific wins: a side, else its axis, else the uniform value — the cascade
            // lives in `pad_left`/`pad_right`/`pad_top`/`pad_bottom` so paint can read the same
            // numbers layout does.
            padding: Rect {
                left: length(self.pad_left(font_px)),
                right: length(self.pad_right(font_px)),
                top: length(self.pad_top(font_px)),
                bottom: length(self.pad_bottom(font_px)),
            },
            // **A placement names the box's size as well as where it goes**, so it wins over the
            // `width`/`height` fields — a caller who said "this rect" has already answered both,
            // and honouring a stale `width` beside it would silently draw a different rect than
            // the one asked for.
            //
            // **Per axis, and `Auto` is not an answer.** A placement that leaves an axis `Auto` has
            // said *where*, not *how big*, so the widget's own size stands on that axis. Reading
            // `Auto` as "shrink to content" instead let a placement quietly overrule a size the
            // widget had set on itself, which is how a context menu seated as a surface came to be
            // stretched down the whole window: the seat gives every surface the viewport, and a
            // menu is not a layer — it *is* its panel, so the box it drew and the box it could be
            // clicked in both became the window.
            size: {
                let axis = |placed: Length, own: Length| match placed {
                    Length::Auto => own.to_taffy(font_px),
                    other => other.to_taffy(font_px),
                };
                match self.placement {
                    Some(p) => Size {
                        width: axis(p.width, self.width),
                        height: axis(p.height, self.height),
                    },
                    None => {
                        let constrained = |value: Length, min: Option<Length>, max: Option<Length>| {
                            let value = value.to_taffy(font_px);
                            if self.aspect_ratio.is_none() { return value }
                            match value {
                                Dimension::Length(mut value) => {
                                    if let Some(Dimension::Length(max)) = max.map(|v| v.to_taffy(font_px)) {
                                        value = value.min(max);
                                    }
                                    if let Some(Dimension::Length(min)) = min.map(|v| v.to_taffy(font_px)) {
                                        value = value.max(min);
                                    }
                                    Dimension::Length(value)
                                }
                                other => other,
                            }
                        };
                        Size { width: constrained(self.width, self.min_width, self.max_width),
                               height: constrained(self.height, self.min_height, self.max_height) }
                    },
                }
            },
            // Out of the flow when placed: an absolutely positioned child takes no space from its
            // siblings and is not moved by them, which is what "drawn *over* the row, where it
            // actually sits" means. `inset` is per-axis — unlike a margin, a percentage `top` here
            // resolves against the parent's **height**.
            position: match self.placement {
                Some(_) => taffy::Position::Absolute,
                None => taffy::Position::Relative,
            },
            inset: match self.placement {
                Some(p) => Rect {
                    left: p.left.to_taffy_inset(font_px),
                    top: p.top.to_taffy_inset(font_px),
                    // The size is given, so the far edges must stay free: pinning all four would
                    // make taffy stretch the box between them and ignore the width and height.
                    right: taffy::LengthPercentageAuto::Auto,
                    bottom: taffy::LengthPercentageAuto::Auto,
                },
                None => Rect {
                    left: taffy::LengthPercentageAuto::Auto,
                    right: taffy::LengthPercentageAuto::Auto,
                    top: taffy::LengthPercentageAuto::Auto,
                    bottom: taffy::LengthPercentageAuto::Auto,
                },
            },
            // `None` leaves taffy's default (`auto`), which for a flex item is its
            // content size — the reason an unset region refuses to shrink.
            min_size: Size {
                width: self.min_width.map_or_else(auto, |v| v.to_taffy(font_px)),
                height: self.min_height.map_or_else(auto, |v| v.to_taffy(font_px)),
            },
            max_size: Size {
                width: self.max_width.map_or_else(auto, |v| v.to_taffy(font_px)),
                height: self.max_height.map_or_else(auto, |v| v.to_taffy(font_px)),
            },
            flex_wrap: if self.wrap {
                taffy::FlexWrap::Wrap
            } else {
                taffy::FlexWrap::NoWrap
            },
            flex_grow: self.flex_grow,
            // `auto` unless the author said otherwise, exactly as flexbox has it.
            flex_basis: self
                .flex_basis
                .map_or(taffy::style::Dimension::Auto, |b| b.to_taffy(font_px)),
            // Widgets use explicit Px sizes; never let a flex container squish them
            // — unless the widget opts in (a scroll viewport must absorb the squeeze).
            flex_shrink: self.flex_shrink.unwrap_or(1.0),
            // Child placement when this component sits in a Grid (else Auto).
            grid_column: grid_line(self.grid_cell.map(|c| (c.col, c.col_span))),
            grid_row: grid_line(self.grid_cell.map(|c| (c.row, c.row_span))),
            ..Default::default()
        }
    }

    /// Build a **grid container** taffy style: the flex/box fields from
    /// `to_taffy()` plus `display: grid` and the given column/row tracks.
    /// Used by [`Grid`](crate::widgets::Grid) via `Component::taffy_style`.
    pub fn to_taffy_grid(&self, font_px: f32, columns: &[Track], rows: &[Track]) -> taffy::Style {
        let mut s = self.to_taffy(font_px);
        s.display = taffy::Display::Grid;
        s.grid_template_columns = columns.iter().map(|t| t.to_taffy()).collect();
        s.grid_template_rows = rows.iter().map(|t| t.to_taffy()).collect();
        // **A track nobody wrote still fills the box.**
        //
        // Say only `template_row("auto 1fr")` and the COLUMN is implicit — and an implicit track is
        // `auto`, which sizes to its content. So a grid templated down one axis drew its contents
        // at their natural width and left the rest of the box empty, which is not what anyone means
        // by templating the rows. The implicit track takes the room instead, on whichever axis was
        // left unsaid.
        use taffy::style_helpers::{fr, length, minmax};
        let fill = minmax(length(0.0), fr(1.0));
        if columns.is_empty() {
            s.grid_auto_columns = vec![fill];
        }
        if rows.is_empty() {
            s.grid_auto_rows = vec![fill];
        }
        s
    }
}

/// Map a 1-based `(start, span)` to a taffy grid line, or `Auto` when `None`.
fn grid_line(cell: Option<(u16, u16)>) -> taffy::geometry::Line<taffy::style::GridPlacement> {
    use taffy::prelude::{line, span};
    match cell {
        // `line(n)` → start at grid line n; `span(k)` → end as a k-track span.
        Some((start, sp)) => taffy::geometry::Line {
            start: line::<taffy::style::GridPlacement>(start as i16),
            end: span::<taffy::style::GridPlacement>(sp.max(1)),
        },
        None => taffy::style::Style::DEFAULT.grid_column,
    }
}
