//! Component style: layout (mapped to taffy) + Tron visual tokens.
//!
//! The layout enums here ([`Direction`], [`Justify`], [`Align`], [`Length`])
//! are our own, mapped to `taffy` internally — so `taffy` never leaks into the
//! public API and could be swapped without breaking widget code.

use crate::color::Color;
use crate::scene::{Border, Glow};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Main-axis direction of a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Row,
    Column,
}

/// Main-axis distribution of children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Cross-axis alignment of children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

/// Overall **size variant** of a widget. Scales the widget's font **and** its
/// intrinsic padding / fixed dimensions together, so the whole control grows or
/// shrinks proportionally. The font part is applied centrally during layout (see
/// [`LayoutEngine`](crate::layout::LayoutEngine)); each widget scales its own
/// padding by [`pad_scale`](WidgetSize::pad_scale) in `remeasure`.
///
/// `Large` matches the historical (un-sized) look; the default is `Normal`, a more
/// compact baseline. `Header` is the one step *above* `Large` — an emphasized control
/// (`1.25×` the base font) with a tight cluster padding, for icon buttons that sit in a
/// pane/info-bar header and must read a touch larger than the body text.
/// Set per widget via [`LayoutExt::size`](crate::builders::LayoutExt::size).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetSize {
    /// Compact controls (`0.8×`).
    Small,
    /// The default — tighter than the raw base font (`0.9×`).
    #[default]
    Normal,
    /// Roomy controls at the full base font + padding (`1.0×`).
    Large,
    /// Emphasized header controls (`1.25×` font) in a tight cluster — for header /
    /// info-bar action buttons that should out-size the body text.
    Header,
}

impl WidgetSize {
    /// Multiplier for the inherited **font** size.
    pub fn font_scale(self) -> f32 {
        match self {
            WidgetSize::Small => 0.8,
            WidgetSize::Normal => 0.9,
            WidgetSize::Large => 1.0,
            WidgetSize::Header => 1.25,
        }
    }

    /// Multiplier for a widget's intrinsic **padding / fixed dimensions** (track,
    /// box, chevron…). Tighter than the font at `Small` so compact controls aren't
    /// dominated by their padding — the height of a `Small` button is mostly
    /// padding, so this is what actually makes it sidebar-compact. `Normal`/`Large`
    /// match the font scale (no change to them). `Header` deliberately keeps a *snug*
    /// padding (below `Small`) so an emphasized header icon stays large while the
    /// button cluster reads as one tight group, not a row of chunky boxes.
    pub fn pad_scale(self) -> f32 {
        match self {
            WidgetSize::Small => 0.5,
            WidgetSize::Normal => 0.9,
            WidgetSize::Large => 1.0,
            WidgetSize::Header => 0.4,
        }
    }
}

/// A theme-derived **spacing** token for container padding. Resolved to px from the
/// inherited font at layout time (so it scales with the theme / font zoom) — callers
/// pick a token instead of hand-computing px. Used via `LayoutExt::pad`/`pad_x`/`pad_y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Spacing {
    None,
    Xs,
    Sm,
    Md,
    Lg,
}

impl Spacing {
    /// Multiplier applied to the inherited font size to get the padding in px.
    pub fn scale(self) -> f32 {
        match self {
            Spacing::None => 0.0,
            Spacing::Xs => 0.25,
            Spacing::Sm => 0.5,
            Spacing::Md => 0.85,
            Spacing::Lg => 1.25,
        }
    }
}

/// A size along one axis.
///
/// Serializes to the spelling an author would reach for rather than to its enum shape:
/// [`Auto`](Self::Auto) is `"auto"`, [`Px`](Self::Px) is a bare number, and [`Pct`](Self::Pct) is a
/// percentage string (`"50%"`). So a declarative description writes `"width": 240` or
/// `"width": "50%"`, not `{"px": 240}`. Round-trips, which the layout merge relies on.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    /// Sized by content / flex rules.
    #[default]
    Auto,
    /// Fixed logical pixels.
    Px(f32),
    /// Fraction of the parent (`0.0..=1.0`).
    Pct(f32),
}

impl Serialize for Length {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            Length::Auto => s.serialize_str("auto"),
            Length::Px(v) => s.serialize_f32(v),
            Length::Pct(v) => s.serialize_str(&format!("{}%", v * 100.0)),
        }
    }
}

impl<'de> Deserialize<'de> for Length {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Num(f32),
            Text(String),
        }
        match Repr::deserialize(d)? {
            Repr::Num(v) => Ok(Length::Px(v)),
            Repr::Text(t) => {
                let t = t.trim();
                if t.eq_ignore_ascii_case("auto") {
                    Ok(Length::Auto)
                } else if let Some(pct) = t.strip_suffix('%') {
                    pct.trim()
                        .parse::<f32>()
                        .map(|v| Length::Pct(v / 100.0))
                        .map_err(|_| D::Error::custom("percentage is not a number"))
                } else {
                    t.parse::<f32>()
                        .map(Length::Px)
                        .map_err(|_| D::Error::custom("expected a number, \"auto\", or a percentage"))
                }
            }
        }
    }
}

impl Length {
    fn to_taffy(self) -> taffy::Dimension {
        use taffy::prelude::*;
        match self {
            Length::Auto => auto(),
            Length::Px(v) => length(v),
            Length::Pct(p) => percent(p),
        }
    }
}

/// One column/row track size for a [`Grid`](crate::widgets::Grid).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    /// Fixed logical pixels.
    Px(f32),
    /// A fraction of the leftover free space (`1fr`, `2fr`, …).
    Fr(f32),
    /// Sized to fit content / grid rules.
    Auto,
    /// Shrink to the minimum the content allows.
    MinContent,
    /// Grow to the maximum the content wants.
    MaxContent,
}

impl Track {
    fn to_taffy(self) -> taffy::style::TrackSizingFunction {
        use taffy::prelude::*;
        match self {
            Track::Px(v) => length(v),
            Track::Fr(v) => fr(v),
            Track::Auto => auto(),
            Track::MinContent => min_content(),
            Track::MaxContent => max_content(),
        }
    }
}

/// Placement of a child within a [`Grid`](crate::widgets::Grid): a 1-based start
/// column/row plus a span. `Copy`, so it lives on [`Style`] without breaking it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridCell {
    /// 1-based start column.
    pub col: u16,
    /// 1-based start row.
    pub row: u16,
    /// Number of columns spanned (≥ 1).
    pub col_span: u16,
    /// Number of rows spanned (≥ 1).
    pub row_span: u16,
}

impl Justify {
    fn to_taffy(self) -> taffy::JustifyContent {
        use taffy::JustifyContent as J;
        match self {
            Justify::Start => J::Start,
            Justify::Center => J::Center,
            Justify::End => J::End,
            Justify::SpaceBetween => J::SpaceBetween,
            Justify::SpaceAround => J::SpaceAround,
            Justify::SpaceEvenly => J::SpaceEvenly,
        }
    }
}

impl Align {
    fn to_taffy(self) -> taffy::AlignItems {
        use taffy::AlignItems as A;
        match self {
            Align::Start => A::Start,
            Align::Center => A::Center,
            Align::End => A::End,
            Align::Stretch => A::Stretch,
        }
    }
}

/// The **appearance** half of [`Style`] — the pixels. The [`Theme`](crate::theme::Theme) supplies
/// every default; a description may override any of it.
///
/// **Changed 2026-07-27 (F003/P017/T7).** This type used to be deliberately *not* serializable, so
/// that appearance was unreachable from a description by construction — "a description carries
/// semantic intent and the host decides what that looks like". That rule is dead: the theme is the
/// default, not a wall. Unset still means "ask the theme", which is already how the fields behave —
/// [`fill`](Self::fill), [`border`](Self::border) and [`glow`](Self::glow) are `Option`, and
/// [`radius`](Self::radius) / [`font_size`](Self::font_size) use a `0.0 = inherit` sentinel — so a
/// widget that overrides nothing follows a theme reload exactly as before.
///
/// **The split with [`Layout`] keeps its value and is not undone.** It stopped being a barrier; it
/// remains the honest grouping of *what the caller asked for* versus *what the theme decided*, and
/// it is how a reader tells arrangement from appearance at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Visual {
    pub fill: Option<Color>,
    pub border: Option<Border>,
    pub glow: Option<Glow>,
    pub radius: f32,
    /// Explicit font size in logical px. `0.0` = inherit the theme base font.
    pub font_size: f32,
    /// Multiplier applied to the inherited base font (header ≈ 2.0, caption ≈ 0.8,
    /// body = 1.0). Ignored when [`font_size`](Self::font_size) is set explicitly.
    ///
    /// A raw multiplier, so it is host-only: the semantic route a description *can* take is
    /// [`Style::size`] (`Small`/`Normal`/`Big`), which scales font and padding together and
    /// cascades to children.
    pub font_scale: f32,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            fill: None,
            border: None,
            glow: None,
            radius: 0.0,
            // 0.0 = inherit the theme's `font_size`; a widget's `.font_size(x)`
            // (x > 0) overrides it. Resolved centrally during layout.
            font_size: 0.0,
            font_scale: 1.0,
        }
    }
}

/// A component's style: two peer halves, [`layout`](Self::layout) and [`visual`](Self::visual).
///
/// The split says what a value **means**, not what may set it — a distinction the library lived by
/// before plugins existed, since `AGENTS.md` requires every widget to read colours, fonts and radii
/// from the [`Theme`](crate::theme::Theme) and hardcode nothing.
///
/// - [`Layout`] — arrangement plus the semantic [`size`](Layout::size) variant.
/// - [`Visual`] — appearance: what the theme decides unless someone says otherwise.
///
/// **Both halves are serializable, and a description may set either (changed 2026-07-27,
/// F003/P017/T7).** The split used to *be* the plugin boundary: `Visual` was deliberately not
/// serializable, so appearance was unreachable from a description by construction. The theme is
/// the default now, not a wall — unset still means "ask the theme", which is what the `Option`
/// fields and the `0.0 = inherit` sentinels already meant. The grouping was kept because it is
/// worth having on its own terms, not because it was a barrier.
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
    pub gap: f32,
    /// Uniform outer margin (all sides), unless overridden per side by
    /// [`margin_left`](Self::margin_left) / [`margin_right`](Self::margin_right)
    /// / [`margin_top`](Self::margin_top) / [`margin_bottom`](Self::margin_bottom).
    pub margin: f32,
    /// Left margin override; `None` ⇒ use [`margin`](Self::margin).
    pub margin_left: Option<f32>,
    /// Right margin override; `None` ⇒ use [`margin`](Self::margin).
    pub margin_right: Option<f32>,
    /// Top margin override; `None` ⇒ use [`margin`](Self::margin).
    pub margin_top: Option<f32>,
    /// Bottom margin override; `None` ⇒ use [`margin`](Self::margin).
    pub margin_bottom: Option<f32>,
    /// Uniform inner padding (all sides), unless overridden per axis by
    /// [`padding_x`](Self::padding_x) / [`padding_y`](Self::padding_y).
    pub padding: f32,
    /// Horizontal (left+right) padding override; `None` ⇒ use [`padding`](Self::padding).
    pub padding_x: Option<f32>,
    /// Vertical (top+bottom) padding override; `None` ⇒ use [`padding`](Self::padding).
    pub padding_y: Option<f32>,
    /// Left padding override; `None` ⇒ use [`padding_x`](Self::padding_x), then
    /// [`padding`](Self::padding). Mirrors the per-side margins.
    pub padding_left: Option<f32>,
    /// Right padding override; `None` ⇒ [`padding_x`](Self::padding_x), then [`padding`](Self::padding).
    ///
    /// This is what lets a widget reserve space along one edge without moving the opposite one — a
    /// [`ScrollRegion`](crate::widgets::ScrollRegion) keeping its content clear of the scrollbar,
    /// for instance, where padding the whole axis would inset the far side for no reason.
    pub padding_right: Option<f32>,
    /// Top padding override; `None` ⇒ [`padding_y`](Self::padding_y), then [`padding`](Self::padding).
    pub padding_top: Option<f32>,
    /// Bottom padding override; `None` ⇒ [`padding_y`](Self::padding_y), then [`padding`](Self::padding).
    pub padding_bottom: Option<f32>,
    /// Horizontal padding as a theme [`Spacing`] token — resolved to px from the font at
    /// layout (sets `padding_x`). `None` ⇒ use the px padding fields.
    pub pad_spacing_x: Option<Spacing>,
    /// Vertical padding as a theme [`Spacing`] token — resolved to px from the font at layout.
    pub pad_spacing_y: Option<Spacing>,
    /// Gap between children as a theme [`Spacing`] token — resolved to px from the
    /// inherited font at layout (sets [`gap`](Self::gap)). `None` ⇒ use the raw
    /// `gap` px. Prefer this over a literal: a token scales with the font, the size
    /// variant and UI zoom, so rows stay comfortably spaced at every scale instead
    /// of being tuned once for one font size.
    pub gap_spacing: Option<Spacing>,
    pub width: Length,
    pub height: Length,
    /// Minimum size. `None` ⇒ taffy's default, which for a flex item is
    /// **`auto` = its content size** — i.e. it will *not* shrink below its
    /// content. Set `Px(0.0)` to allow shrinking, which a scrolling viewport
    /// ([`ScrollRegion`](crate::widgets::ScrollRegion)) needs: without it a
    /// region in a bounded panel overflows its parent instead of scrolling
    /// (the classic flexbox `min-height: auto` trap).
    pub min_width: Option<Length>,
    /// Minimum height — see [`min_width`](Style::min_width).
    pub min_height: Option<Length>,
    /// Maximum width. `None` ⇒ unbounded. Used to cap a node against its parent —
    /// an overlay panel is capped at `Pct(1.0)` so a fixed `Px` size can never
    /// make a dialog larger than the window.
    pub max_width: Option<Length>,
    /// Maximum height — see [`max_width`](Style::max_width).
    pub max_height: Option<Length>,
    pub flex_grow: f32,
    /// Flex shrink factor. `None` ⇒ `0.0`: widgets use explicit sizes and a flex
    /// container must never squish them. A widget that *should* absorb the
    /// squeeze (again, a scroll viewport) opts in with `Some(1.0)`.
    pub flex_shrink: Option<f32>,

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
    /// Placement when this component is a child of a [`Grid`](crate::widgets::Grid).
    /// `None` ⇒ grid auto-placement. Set by `Grid::cell`/`Grid::area`.
    pub grid_cell: Option<GridCell>,
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
            gap: 0.0,
            margin: 0.0,
            margin_left: None,
            margin_right: None,
            margin_top: None,
            margin_bottom: None,
            padding: 0.0,
            padding_x: None,
            padding_y: None,
            padding_left: None,
            padding_right: None,
            padding_top: None,
            padding_bottom: None,
            pad_spacing_x: None,
            pad_spacing_y: None,
            gap_spacing: None,
            width: Length::Auto,
            height: Length::Auto,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            flex_grow: 0.0,
            flex_shrink: None,
            size: WidgetSize::Normal,
            // Not explicitly chosen ⇒ the layout pass may replace it with the parent's variant.
            size_explicit: false,
            hidden: false,
            grid_cell: None,
        }
    }
}

impl Layout {
    /// Map the layout fields onto a `taffy::Style` for the layout engine.
    pub fn to_taffy(&self) -> taffy::Style {
        use taffy::prelude::*;
        if self.hidden {
            return taffy::Style {
                display: Display::None,
                ..Default::default()
            };
        }
        taffy::Style {
            display: Display::Flex,
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
            gap: Size {
                width: length(self.gap),
                height: length(self.gap),
            },
            margin: {
                let m = self.margin;
                Rect {
                    left: length(self.margin_left.unwrap_or(m)),
                    right: length(self.margin_right.unwrap_or(m)),
                    top: length(self.margin_top.unwrap_or(m)),
                    bottom: length(self.margin_bottom.unwrap_or(m)),
                }
            },
            padding: {
                // Most specific wins: a side, else its axis, else the uniform value.
                let px = self.padding_x.unwrap_or(self.padding);
                let py = self.padding_y.unwrap_or(self.padding);
                Rect {
                    left: length(self.padding_left.unwrap_or(px)),
                    right: length(self.padding_right.unwrap_or(px)),
                    top: length(self.padding_top.unwrap_or(py)),
                    bottom: length(self.padding_bottom.unwrap_or(py)),
                }
            },
            size: Size {
                width: self.width.to_taffy(),
                height: self.height.to_taffy(),
            },
            // `None` leaves taffy's default (`auto`), which for a flex item is its
            // content size — the reason an unset region refuses to shrink.
            min_size: Size {
                width: self.min_width.map_or_else(auto, Length::to_taffy),
                height: self.min_height.map_or_else(auto, Length::to_taffy),
            },
            max_size: Size {
                width: self.max_width.map_or_else(auto, Length::to_taffy),
                height: self.max_height.map_or_else(auto, Length::to_taffy),
            },
            flex_grow: self.flex_grow,
            // Widgets use explicit Px sizes; never let a flex container squish them
            // — unless the widget opts in (a scroll viewport must absorb the squeeze).
            flex_shrink: self.flex_shrink.unwrap_or(0.0),
            // Child placement when this component sits in a Grid (else Auto).
            grid_column: grid_line(self.grid_cell.map(|c| (c.col, c.col_span))),
            grid_row: grid_line(self.grid_cell.map(|c| (c.row, c.row_span))),
            ..Default::default()
        }
    }

    /// Build a **grid container** taffy style: the flex/box fields from
    /// `to_taffy()` plus `display: grid` and the given column/row tracks.
    /// Used by [`Grid`](crate::widgets::Grid) via `Component::taffy_style`.
    pub fn to_taffy_grid(&self, columns: &[Track], rows: &[Track]) -> taffy::Style {
        let mut s = self.to_taffy();
        s.display = taffy::Display::Grid;
        s.grid_template_columns = columns.iter().map(|t| t.to_taffy()).collect();
        s.grid_template_rows = rows.iter().map(|t| t.to_taffy()).collect();
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
