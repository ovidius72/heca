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
/// [`Auto`](Self::Auto) is `"auto"`, [`Px`](Self::Px) is a bare number, and
/// [`Percent`](Self::Percent) is a percentage string (`"50%"`). So a declarative description writes
/// `"width": 240` or `"width": "50%"`, not `{"px": 240}`. Round-trips, which the layout merge
/// relies on.
///
/// **Set neither a width nor a height and the widget fills its parent across the cross axis**,
/// exactly as CSS `align-items: stretch` does — so `.width(Length::Percent(1.0))` on a child that
/// already fills says nothing, and is better left off.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    /// Sized by content / flex rules.
    #[default]
    Auto,
    /// Fixed logical pixels.
    Px(f32),
    /// **A fraction of the parent, `0.0..=1.0` — NOT a 0–100 percentage.** Half the parent is
    /// `Percent(0.5)`; `Percent(50.0)` is fifty times it, and nothing warns you.
    ///
    /// The fraction is taffy's own convention, which this sits on, and the wire spelling is the
    /// human one: [`Percent(0.5)`](Self::Percent) serializes to `"50%"` and parses back from it.
    /// That is the mismatch the name has to survive, which is why it is spelled out rather than
    /// abbreviated — an author who reads `Percent` asks what the number means, and this answers.
    Percent(f32),
}

impl From<f32> for Length {
    /// A bare number is pixels, the way `width(12.0)` already reads — so every existing
    /// `margin_left(8.0)` keeps its meaning now that a margin may also be a percentage.
    fn from(v: f32) -> Self {
        Length::Px(v)
    }
}

impl From<f64> for Length {
    /// `.width(200.0)` — Rust reads a bare decimal as `f64`, so without this the obvious spelling
    /// does not compile and an author has to write `200.0f32` to say two hundred pixels.
    fn from(v: f64) -> Self {
        Length::Px(v as f32)
    }
}

impl From<i32> for Length {
    /// `.width(200)` — a bare integer is `i32`, and pixels are what a whole number means.
    fn from(v: i32) -> Self {
        Length::Px(v as f32)
    }
}

impl From<u32> for Length {
    fn from(v: u32) -> Self {
        Length::Px(v as f32)
    }
}

impl std::str::FromStr for Length {
    type Err = LengthParseError;

    /// **The one parser.** `"auto"`, `"50%"`, `"200px"`, `"200"` — the spellings CSS uses and the
    /// ones a description already travels in. [`Deserialize`] calls this, so the wire and native
    /// code can never come to disagree about what `"50%"` means.
    fn from_str(t: &str) -> Result<Self, Self::Err> {
        let t = t.trim();
        if t.eq_ignore_ascii_case("auto") {
            return Ok(Length::Auto);
        }
        if let Some(pct) = t.strip_suffix('%') {
            return pct
                .trim()
                .parse::<f32>()
                .map(|v| Length::Percent(v / 100.0))
                .map_err(|_| LengthParseError);
        }
        // `px` is optional and means the same as no suffix, which is what CSS authors expect and
        // what the described side already accepts as a bare number.
        let t = t.strip_suffix("px").map_or(t, str::trim_end);
        t.parse::<f32>()
            .map(Length::Px)
            .map_err(|_| LengthParseError)
    }
}

/// What [`Length::from_str`] returns when a string is none of the four spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LengthParseError;

impl std::fmt::Display for LengthParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "expected a number, \"auto\", a percentage like \"50%\", or a length like \"200px\"",
        )
    }
}

impl std::error::Error for LengthParseError {}

impl From<&str> for Length {
    /// `.width("50%")` / `.width("200px")` / `.width("auto")`.
    ///
    /// ⚠️ **An unrecognised string degrades to [`Auto`](Length::Auto)** rather than panicking —
    /// the same rule the grid's track vocabulary already follows, because these spellings arrive
    /// from descriptions and config as well as from Rust, and a typo must cost its author a
    /// differently-sized box rather than take the host down. Use
    /// [`from_str`](std::str::FromStr::from_str) when you want to be told.
    fn from(t: &str) -> Self {
        t.parse().unwrap_or(Length::Auto)
    }
}

impl From<&String> for Length {
    fn from(t: &String) -> Self {
        Length::from(t.as_str())
    }
}

impl Serialize for Length {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            Length::Auto => s.serialize_str("auto"),
            Length::Px(v) => s.serialize_f32(v),
            Length::Percent(v) => s.serialize_str(&format!("{}%", v * 100.0)),
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
            // **One parser, not a second copy.** This used to spell the rules out again here, so
            // the wire and native code could drift about what `"50%"` meant with nothing failing.
            Repr::Text(t) => t
                .parse::<Length>()
                .map_err(|e| D::Error::custom(e.to_string())),
        }
    }
}

impl Length {
    /// The whole parent — `"100%"`, said without a number to mistype.
    pub const FULL: Length = Length::Percent(1.0);
    /// Half the parent.
    pub const HALF: Length = Length::Percent(0.5);
    /// A third of the parent.
    pub const THIRD: Length = Length::Percent(1.0 / 3.0);
    /// A quarter of the parent.
    pub const QUARTER: Length = Length::Percent(0.25);

    fn to_taffy(self) -> taffy::Dimension {
        use taffy::prelude::*;
        match self {
            Length::Auto => auto(),
            Length::Px(v) => length(v),
            Length::Percent(p) => percent(p),
        }
    }

    /// The same value as a taffy **inset** — the type an edge offset takes, which admits `Auto`
    /// (meaning "this edge is not pinned") where a size would not.
    fn to_taffy_inset(self) -> taffy::LengthPercentageAuto {
        use taffy::prelude::*;
        match self {
            Length::Auto => taffy::LengthPercentageAuto::Auto,
            Length::Px(v) => length(v),
            Length::Percent(p) => percent(p),
        }
    }
}

/// **A rect a node is placed at inside its parent**, taking it out of the flow — CSS
/// `position: absolute` plus insets, which is what "put this box *there*" means in a layout
/// engine.
///
/// Set through [`LayoutExt::at_rect`](crate::builders::LayoutExt::at_rect); see that method for
/// what it is for and why a margin cannot do the job.
///
/// All four are [`Length`]s, so a caller may mix units: a chip at a fixed `Px` size over a
/// proportional `Percent` position is as valid as a fully fractional rect. **A percentage resolves
/// against the parent on its own axis** — `left`/`width` against the parent's width, `top`/`height`
/// against its height — which is the difference from a percentage *margin*, where CSS resolves
/// **both** axes against the width.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    /// Distance from the parent's left content edge.
    pub left: Length,
    /// Distance from the parent's top content edge.
    pub top: Length,
    /// The box's own width. [`Auto`](Length::Auto) leaves the question to the widget: whatever
    /// width it set on itself stands, and with nothing set it is sized by its content.
    pub width: Length,
    /// The box's own height. [`Auto`](Length::Auto) leaves the question to the widget — see
    /// [`width`](Placement::width).
    pub height: Length,
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
    /// Horizontal (left+right) margin override; `None` ⇒ use [`margin`](Self::margin).
    ///
    /// The axis shorthands exist for parity with [`padding_x`](Self::padding_x) /
    /// [`padding_y`](Self::padding_y): without them a **described** tree could set padding by axis
    /// but had to name both sides for a margin. A `Separator` wanting to breathe on one axis is the
    /// case that found it.
    pub margin_x: Option<f32>,
    /// Vertical (top+bottom) margin override; `None` ⇒ use [`margin`](Self::margin).
    pub margin_y: Option<f32>,
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
    /// an overlay panel is capped at `Percent(1.0)` so a fixed `Px` size can never
    /// make a dialog larger than the window.
    pub max_width: Option<Length>,
    /// Maximum height — see [`max_width`](Style::max_width).
    pub max_height: Option<Length>,
    pub flex_grow: f32,
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
    /// box, and a widget keeps its natural size when there is room (F003/P082/T438).
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
    /// **Children that do not fit on one line start another** (CSS `flex-wrap: wrap`).
    ///
    /// A row of controls is the case: three buttons in a card narrower than their sum have to go
    /// somewhere, and the alternatives are both wrong — squeezing every button until its label is
    /// an ellipsis, or laying the overflow out past the edge. `false` (one line) stays the default,
    /// because for most rows — a leading slot, a label and a trailing slot — a second line would be
    /// nonsense (F003/P096/T483).
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
            gap: 0.0,
            margin: 0.0,
            margin_x: None,
            margin_y: None,
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
    pub fn pad_left(&self) -> f32 {
        self.padding_left
            .unwrap_or_else(|| self.padding_x.unwrap_or(self.padding))
    }

    /// Effective right padding in px — see [`pad_left`](Self::pad_left).
    pub fn pad_right(&self) -> f32 {
        self.padding_right
            .unwrap_or_else(|| self.padding_x.unwrap_or(self.padding))
    }

    /// Effective top padding in px — see [`pad_left`](Self::pad_left).
    pub fn pad_top(&self) -> f32 {
        self.padding_top
            .unwrap_or_else(|| self.padding_y.unwrap_or(self.padding))
    }

    /// Effective bottom padding in px — see [`pad_left`](Self::pad_left).
    pub fn pad_bottom(&self) -> f32 {
        self.padding_bottom
            .unwrap_or_else(|| self.padding_y.unwrap_or(self.padding))
    }

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
                // Most specific wins: a side, else its axis, else the uniform value — the same
                // cascade padding has.
                let mx = self.margin_x.unwrap_or(self.margin);
                let my = self.margin_y.unwrap_or(self.margin);
                // A side may be a **percentage** of the parent, which is what lets a caller place
                // a box at a proportional position — a floating pane in the exposé sits at
                // `x / strip_width` of its row, with no pixel scale anywhere (F003/P082/T420).
                let side = |v: Option<Length>, axis: f32| match v {
                    Some(Length::Px(px)) => length(px),
                    Some(Length::Percent(f)) => percent(f),
                    // `Auto` is the CSS centring margin; taffy spells it on this type.
                    Some(Length::Auto) => taffy::LengthPercentageAuto::Auto,
                    None => length(axis),
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
                left: length(self.pad_left()),
                right: length(self.pad_right()),
                top: length(self.pad_top()),
                bottom: length(self.pad_bottom()),
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
            // clicked in both became the window (Antonio, driving, 2026-09-01).
            size: {
                let axis = |placed: Length, own: Length| match placed {
                    Length::Auto => own.to_taffy(),
                    other => other.to_taffy(),
                };
                match self.placement {
                    Some(p) => Size {
                        width: axis(p.width, self.width),
                        height: axis(p.height, self.height),
                    },
                    None => Size { width: self.width.to_taffy(), height: self.height.to_taffy() },
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
                    left: p.left.to_taffy_inset(),
                    top: p.top.to_taffy_inset(),
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
                width: self.min_width.map_or_else(auto, Length::to_taffy),
                height: self.min_height.map_or_else(auto, Length::to_taffy),
            },
            max_size: Size {
                width: self.max_width.map_or_else(auto, Length::to_taffy),
                height: self.max_height.map_or_else(auto, Length::to_taffy),
            },
            flex_wrap: if self.wrap {
                taffy::FlexWrap::Wrap
            } else {
                taffy::FlexWrap::NoWrap
            },
            flex_grow: self.flex_grow,
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

#[cfg(test)]
mod length_spellings {
    use super::*;

    /// **A size is written the way it is said**, so nobody reaches for the enum at a call site.
    ///
    /// `Length::Percent` was written by hand 282 times across the workspace, for one reason: the
    /// type already knew every spelling — its `Deserialize` read `"50%"`, `"200px"` and `"auto"`
    /// from descriptions and config — and none of it could reach a builder, because the sizing
    /// builders took `Length` by value.
    #[test]
    fn every_spelling_a_size_is_written_in_means_the_same_size() {
        assert_eq!(Length::from(200), Length::Px(200.0), "a bare integer is pixels");
        assert_eq!(Length::from(200.0), Length::Px(200.0), "a bare decimal is pixels");
        assert_eq!(Length::from("200"), Length::Px(200.0));
        assert_eq!(Length::from("200px"), Length::Px(200.0), "the px suffix is optional");
        assert_eq!(Length::from(" 200 px "), Length::Px(200.0), "and forgiving of spaces");
        assert_eq!(Length::from("auto"), Length::Auto);
        assert_eq!(Length::from("AUTO"), Length::Auto, "case is not a spelling");
        assert_eq!(Length::from("50%"), Length::Percent(0.5), "the wire spelling is a fraction");
        assert_eq!(Length::from("100%"), Length::FULL);
    }

    /// **The named fractions are the same value said without a number to mistype** — which is the
    /// trap `Percent` carries: it takes `0.0..=1.0`, so `Percent(50.0)` is fifty times the parent.
    #[test]
    fn a_named_fraction_is_the_fraction_it_names() {
        assert_eq!(Length::FULL, Length::Percent(1.0));
        assert_eq!(Length::HALF, Length::Percent(0.5));
        assert_eq!(Length::QUARTER, Length::Percent(0.25));
        assert_eq!(Length::HALF, Length::from("50%"));
    }

    /// **An unrecognised string degrades to `Auto`, it does not panic** — the same rule the grid's
    /// track vocabulary follows, and for the same reason: these spellings arrive from a plugin's
    /// description and from `config.toml`, so a typo must cost its author a differently-sized box
    /// rather than take the host down.
    #[test]
    fn a_size_nobody_can_read_becomes_auto_rather_than_a_panic() {
        assert_eq!(Length::from("fifty percent"), Length::Auto);
        assert_eq!(Length::from(""), Length::Auto);
        assert_eq!(Length::from("%"), Length::Auto);
    }

    /// …and the caller who wants to be told still can.
    #[test]
    fn from_str_reports_what_from_swallows() {
        assert!("fifty percent".parse::<Length>().is_err());
        assert_eq!("50%".parse::<Length>().unwrap(), Length::HALF);
    }

    /// **One parser, not two.** The spellings used to be spelled out a second time inside
    /// `Deserialize`, so the wire and native code could drift about what `"50%"` meant with
    /// nothing failing — the tests exercise one path and the user sees the other. Now the
    /// deserializer calls `from_str`, and this is what holds them to it.
    #[test]
    fn the_wire_and_a_call_site_read_a_size_through_the_same_parser() {
        use serde::de::IntoDeserializer;
        for spelling in ["auto", "50%", "200px", "200", "0%"] {
            let d: serde::de::value::StrDeserializer<serde::de::value::Error> =
                spelling.into_deserializer();
            let from_wire = Length::deserialize(d).expect("the wire reads every spelling");
            assert_eq!(
                from_wire,
                Length::from(spelling),
                "`{spelling}` must mean one thing, whoever wrote it",
            );
        }
    }

    /// …and a number on the wire is pixels there too, the same as a bare number in Rust.
    #[test]
    fn a_bare_number_is_pixels_on_both_sides() {
        use serde::de::IntoDeserializer;
        let d: serde::de::value::F32Deserializer<serde::de::value::Error> =
            240.0f32.into_deserializer();
        assert_eq!(Length::deserialize(d).unwrap(), Length::from(240));
    }
}
