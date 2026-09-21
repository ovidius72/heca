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

/// **A space: a number of pixels, or a step of the theme's rhythm.**
///
/// The two were separate builders — `gap(8.0)` beside `gap_spacing(Spacing::Sm)`, `padding(6.0)`
/// beside `pad_all(Spacing::Xs)` — which is two paths over one property, in the library itself. The
/// counts said what that costs: `.gap` was used 112 times against `.gap_spacing`'s 8, and the docs
/// told everyone to prefer the token. Advice loses to whichever name is shorter and more obvious.
///
/// One builder takes either:
///
/// ```ignore
/// Flex::row().gap(8)             // eight pixels
/// Flex::row().gap(Spacing::Sm)   // a step of the rhythm
/// Flex::row().gap("sm")          // the same step, said as a description would
/// ```
///
/// **Prefer the step.** It is resolved from the inherited font at layout, so it scales with the
/// font, the size variant and UI zoom; a raw pixel gap is tuned for one font size and wrong at
/// every other. Use a number when you can say why it should not move with the font.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Space {
    /// Logical pixels, fixed whatever the font does.
    Px(f32),
    /// A step of the theme's rhythm, resolved against the inherited font at layout.
    Step(Spacing),
}

impl From<Spacing> for Space {
    fn from(s: Spacing) -> Self {
        Space::Step(s)
    }
}

impl From<f32> for Space {
    fn from(v: f32) -> Self {
        Space::Px(v)
    }
}

impl From<f64> for Space {
    /// Rust reads a bare decimal as `f64`, so without this `.gap(8.0)` does not compile.
    fn from(v: f64) -> Self {
        Space::Px(v as f32)
    }
}

impl From<i32> for Space {
    /// …and a bare integer as `i32`.
    fn from(v: i32) -> Self {
        Space::Px(v as f32)
    }
}

impl std::str::FromStr for Space {
    type Err = SpaceParseError;

    /// **The one parser.** `"sm"` / `"md"` for a step, `"8"` / `"8px"` for pixels — the spellings a
    /// description already travels in. [`Deserialize`] calls this, so the wire and native code can
    /// never come to disagree about what `"sm"` means.
    fn from_str(t: &str) -> Result<Self, Self::Err> {
        let t = t.trim();
        match t.to_ascii_lowercase().as_str() {
            "none" => return Ok(Space::Step(Spacing::None)),
            "xs" => return Ok(Space::Step(Spacing::Xs)),
            "sm" => return Ok(Space::Step(Spacing::Sm)),
            "md" => return Ok(Space::Step(Spacing::Md)),
            "lg" => return Ok(Space::Step(Spacing::Lg)),
            _ => {}
        }
        // `px` is optional and means the same as no suffix — the same rule `Length` follows.
        let t = t.strip_suffix("px").map_or(t, str::trim_end);
        t.parse::<f32>().map(Space::Px).map_err(|_| SpaceParseError)
    }
}

/// What [`Space::from_str`] returns when a string is neither a step name nor a length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceParseError;

impl std::fmt::Display for SpaceParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("expected a number, a length like \"8px\", or a step name (none/xs/sm/md/lg)")
    }
}

impl std::error::Error for SpaceParseError {}

impl From<&str> for Space {
    /// `.gap("sm")` / `.padding("8px")`.
    ///
    /// ⚠️ Anything unreadable is **no space at all** rather than a panic, the same rule
    /// [`Length`] and the grid's track vocabulary follow: these arrive from config and from
    /// plugins, so a typo costs its author a gap and not the host. Use
    /// [`from_str`](std::str::FromStr::from_str) when you want to be told.
    fn from(t: &str) -> Self {
        t.parse().unwrap_or(Space::Px(0.0))
    }
}

impl From<&String> for Space {
    fn from(t: &String) -> Self {
        Space::from(t.as_str())
    }
}

impl Serialize for Space {
    /// A step travels as its **name** and a length as a number — the two spellings
    /// [`Deserialize`] reads back, and the ones [`plugins.md` §6] documents for every value.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            Space::Px(v) => s.serialize_f32(v),
            Space::Step(step) => step.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Space {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Num(f32),
            Text(String),
        }
        match Repr::deserialize(d)? {
            Repr::Num(v) => Ok(Space::Px(v)),
            // **One parser, not a second copy** — the trap `Length` already documents.
            Repr::Text(t) => t
                .parse::<Space>()
                .map_err(|e| D::Error::custom(e.to_string())),
        }
    }
}

impl Default for Space {
    fn default() -> Self {
        Space::Px(0.0)
    }
}

impl Space {
    /// **The one place a space becomes pixels**, and the only one that can: a step is a fraction of
    /// the inherited font, so nothing can resolve it without knowing that font.
    ///
    /// **Rounded to whole pixels, and that is what makes air look even.** A token is a fraction of
    /// the font (`Xs` is a quarter of it), so it lands on halves at most sizes — and the two sides
    /// of a boundary between siblings then round in different directions. Percentage-sized siblings
    /// put the air in their padding rather than a gap (a gap is added *outside* a percentage and
    /// overflows it), so every boundary in such a row is made of two paddings, and half a pixel
    /// each side became a gap of 6, 7 or 8 where all of them should have been 7. Measured across
    /// fourteen equal columns of the exposé; uniform once the token resolves to a whole pixel. A
    /// widget's own padding moves by at most half a pixel, which is under what the screen can draw;
    /// the rhythm between siblings is the thing an eye actually reads.
    ///
    /// A pixel count is already what it says and is passed through untouched.
    pub fn resolve(self, font_px: f32) -> f32 {
        match self {
            Space::Px(v) => v,
            Space::Step(step) => (font_px * step.scale()).round(),
        }
    }
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
/// exactly as CSS `align-items: stretch` does — so `.width(Length::FULL)` on a child that
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

/// **Read a layout keyword the way a stylesheet writes it** — one parser for every unit enum here.
///
/// It goes through the type's **own serde names**, so there is exactly one vocabulary: a spelling
/// that works in a plugin's JSON works in Rust, and neither side can learn a word the other does
/// not know. `-` and `_` are the same character to it, because CSS writes `space-between` and
/// serde's derived names write `space_between`, and nobody should have to remember which side of
/// the wire they are standing on.
///
/// Unreadable input is `None`, and every caller turns that into the type's default rather than a
/// panic — these arrive from config and from plugins.
fn keyword<T: serde::de::DeserializeOwned>(t: &str) -> Option<T> {
    let name = t.trim().to_ascii_lowercase().replace('-', "_");
    T::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(&name)).ok()
}

macro_rules! keyword_from_str {
    ($($t:ty),+ $(,)?) => {$(
        impl std::str::FromStr for $t {
            type Err = KeywordParseError;
            /// **The one parser** — the type's own serde names, hyphen or underscore.
            fn from_str(t: &str) -> Result<Self, Self::Err> {
                keyword(t).ok_or(KeywordParseError)
            }
        }
        impl From<&str> for $t {
            /// ⚠️ An unreadable keyword is the **default**, never a panic. Use
            /// [`from_str`](std::str::FromStr::from_str) when you want to be told.
            fn from(t: &str) -> Self {
                t.parse().unwrap_or_default()
            }
        }
        impl From<&String> for $t {
            fn from(t: &String) -> Self { Self::from(t.as_str()) }
        }
    )+};
}

/// What a layout keyword's `from_str` returns when the word is not in that type's vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeywordParseError;

// `Spacing` is deliberately NOT here: its five words are already read by [`Space::from_str`], and
// a second parser for the same vocabulary is the defect this whole change removes. Write
// `.gap("sm")`, which goes through that one.
keyword_from_str!(Direction, Justify, Align);

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

/// **A track list, in whichever shape the caller holds it** — one stylesheet line (`"auto 1fr"`),
/// or a list of anything a [`Track`] reads (`["auto", "1fr"]`, `[Track::Auto, Track::Fr(1.0)]`,
/// `[200, 100]`).
///
/// One builder per axis takes this, so there is no `rows` / `template_rows` pair to choose between.
pub trait IntoTracks {
    /// The tracks, parsed.
    fn into_tracks(self) -> Vec<Track>;
}

impl IntoTracks for &str {
    /// Whitespace-separated, exactly as CSS writes `grid-template-rows`.
    fn into_tracks(self) -> Vec<Track> {
        Track::list(self)
    }
}

impl IntoTracks for &String {
    fn into_tracks(self) -> Vec<Track> {
        Track::list(self)
    }
}

impl<T: Into<Track>> IntoTracks for Vec<T> {
    fn into_tracks(self) -> Vec<Track> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<T: Into<Track>, const N: usize> IntoTracks for [T; N] {
    fn into_tracks(self) -> Vec<Track> {
        self.into_iter().map(Into::into).collect()
    }
}

/// **One template row or a list of them** — what [`Grid::template_area`](crate::widgets::Grid::template_area)
/// takes, so a single-row template needs no brackets.
pub trait IntoRows {
    /// The rows, as strings.
    fn into_rows(self) -> Vec<String>;
}

impl IntoRows for &str {
    fn into_rows(self) -> Vec<String> {
        vec![self.to_string()]
    }
}

impl IntoRows for &String {
    fn into_rows(self) -> Vec<String> {
        vec![self.to_string()]
    }
}

impl<T: AsRef<str>> IntoRows for Vec<T> {
    fn into_rows(self) -> Vec<String> {
        self.iter().map(|r| r.as_ref().to_string()).collect()
    }
}

impl<T: AsRef<str>, const N: usize> IntoRows for [T; N] {
    fn into_rows(self) -> Vec<String> {
        self.iter().map(|r| r.as_ref().to_string()).collect()
    }
}

impl std::str::FromStr for Track {
    type Err = TrackParseError;

    /// **The one parser**, and the vocabulary CSS already has: `"auto"`, `"1fr"`, `"22px"`, `"22"`,
    /// `"min-content"` / `"min"`, `"max-content"` / `"max"`.
    ///
    /// [`Deserialize`] calls this, so a described grid and a native one can never come to disagree
    /// about what `"1fr"` means. It used to be a private function in `heca-view-realize`, which put
    /// every spelling out of reach of native code — a `Grid` in Rust could only be written
    /// `Track::Fr(1.0)`, one hand-written variant per track, while a plugin's JSON said `"1fr"`.
    fn from_str(t: &str) -> Result<Self, Self::Err> {
        let t = t.trim();
        match t.to_ascii_lowercase().as_str() {
            "auto" => return Ok(Track::Auto),
            // Hyphen and underscore both, because CSS writes one and serde's own names write the
            // other, and a reader should not have to know which side of the wire they are on.
            "min" | "min-content" | "min_content" => return Ok(Track::MinContent),
            "max" | "max-content" | "max_content" => return Ok(Track::MaxContent),
            _ => {}
        }
        let low = t.to_ascii_lowercase();
        if let Some(fr) = low.strip_suffix("fr") {
            return fr.trim().parse().map(Track::Fr).map_err(|_| TrackParseError);
        }
        // `px` is optional and means the same as no suffix — the rule `Length` and `Space` follow.
        let px = low.strip_suffix("px").map_or(low.as_str(), str::trim_end);
        px.parse::<f32>().map(Track::Px).map_err(|_| TrackParseError)
    }
}

/// What [`Track::from_str`] returns when a string is no track size at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackParseError;

impl From<&str> for Track {
    /// `.rows(["auto", "1fr"])`.
    ///
    /// ⚠️ Anything unreadable is [`Auto`](Track::Auto) rather than a panic — the rule [`Length`]
    /// and [`Space`] follow, because these arrive from config and from plugins, so a typo costs
    /// its author a differently-sized track and not the host. Use
    /// [`from_str`](std::str::FromStr::from_str) when you want to be told.
    fn from(t: &str) -> Self {
        t.parse().unwrap_or(Track::Auto)
    }
}

impl From<&String> for Track {
    fn from(t: &String) -> Self {
        Track::from(t.as_str())
    }
}

impl From<f32> for Track {
    /// A bare number is pixels, the same forgiving reading every other size takes.
    fn from(v: f32) -> Self {
        Track::Px(v)
    }
}

impl From<i32> for Track {
    fn from(v: i32) -> Self {
        Track::Px(v as f32)
    }
}

impl Serialize for Track {
    /// Every track travels as the string a stylesheet would write, except a plain pixel count,
    /// which travels as the number it is.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            Track::Px(v) => s.serialize_f32(v),
            Track::Fr(v) => s.serialize_str(&format!("{v}fr")),
            Track::Auto => s.serialize_str("auto"),
            Track::MinContent => s.serialize_str("min-content"),
            Track::MaxContent => s.serialize_str("max-content"),
        }
    }
}

impl<'de> Deserialize<'de> for Track {
    /// A string through [`from_str`](std::str::FromStr::from_str), or a bare number as pixels.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = Track;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a track size: \"auto\", \"1fr\", \"22px\", a number, \"min-content\" or \"max-content\"")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Track, E> {
                v.parse().map_err(|_| E::custom(format!("not a track size: {v}")))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Track, E> {
                Ok(Track::Px(v as f32))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Track, E> {
                Ok(Track::Px(v as f32))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Track, E> {
                Ok(Track::Px(v as f32))
            }
        }
        d.deserialize_any(V)
    }
}

impl Track {
    /// **Read a whole track list the way a stylesheet writes one**: `"auto 1fr"`,
    /// `"200px 1fr 2fr"`, `"repeat(3, 1fr)"`, `"auto repeat(2, 1fr) auto"`.
    ///
    /// Whitespace-separated, each item through the one parser, and `repeat(n, tracks)` expanded as
    /// CSS expands it — the tracks inside repeated `n` times. Nesting a `repeat` inside a `repeat`
    /// is not a thing in CSS and is not one here.
    pub fn list(template: &str) -> Vec<Track> {
        let mut out = Vec::new();
        let mut rest = template;
        while let Some(at) = rest.to_ascii_lowercase().find("repeat(") {
            out.extend(rest[..at].split_whitespace().map(Track::from));
            let after = &rest[at + "repeat(".len()..];
            let Some(close) = after.find(')') else {
                // Unclosed: read what is there as ordinary tracks rather than dropping it.
                out.extend(after.split_whitespace().map(Track::from));
                return out;
            };
            let (inside, tail) = after.split_at(close);
            if let Some((count, tracks)) = inside.split_once(',') {
                let times = count.trim().parse::<usize>().unwrap_or(1).min(512);
                let unit: Vec<Track> = tracks.split_whitespace().map(Track::from).collect();
                for _ in 0..times {
                    out.extend(unit.iter().copied());
                }
            }
            rest = &tail[1..];
        }
        out.extend(rest.split_whitespace().map(Track::from));
        out
    }

    fn to_taffy(self) -> taffy::style::TrackSizingFunction {
        use taffy::prelude::*;
        match self {
            Track::Px(v) => length(v),
            // **A fraction is `minmax(0, <n>fr)`, which is what an author means by it.**
            //
            // A bare `fr` track carries an automatic minimum of its own CONTENT, so a track holding
            // something tall grows past its share and pushes its neighbours out of the box — two
            // docks in one sidebar, and the second one off the bottom. The floor has to go for the
            // fraction to be a fraction, and it goes here so no grid, and no caller, meets it.
            Track::Fr(v) => minmax(length(0.0), fr(v)),
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
    /// Number of columns spanned (≥ 1), or [`GridCell::ALL`] for every column there are.
    pub col_span: u16,
    /// Number of rows spanned (≥ 1), or [`GridCell::ALL`] for every row there are.
    pub row_span: u16,
}

/// **A grid's template**, borrowed from the widget that owns it — what a child's placement is
/// resolved against during layout.
///
/// See [`Component::grid_template`](crate::component::Component::grid_template).
#[derive(Debug, Clone, Copy)]
pub struct GridTemplate<'a> {
    /// The column tracks.
    pub columns: &'a [Track],
    /// The row tracks.
    pub rows: &'a [Track],
    /// The named areas, one string per row, exactly as written.
    pub areas: &'a [String],
}

impl GridTemplate<'_> {
    /// **Where a named area sits** — the bounding box of every cell carrying that name. `.` and `_`
    /// mark an empty cell and name nothing.
    pub fn area(&self, name: &str) -> Option<GridCell> {
        let mut found: Option<(u16, u16, u16, u16)> = None; // min_c, max_c, min_r, max_r
        for (r, line) in self.areas.iter().enumerate() {
            for (c, token) in line.split_whitespace().enumerate() {
                if token != name {
                    continue;
                }
                let (col, row) = (c as u16 + 1, r as u16 + 1);
                found = Some(match found {
                    None => (col, col, row, row),
                    Some((c0, c1, r0, r1)) => (c0.min(col), c1.max(col), r0.min(row), r1.max(row)),
                });
            }
        }
        found.map(|(c0, c1, r0, r1)| GridCell {
            col: c0,
            row: r0,
            col_span: c1 - c0 + 1,
            row_span: r1 - r0 + 1,
        })
    }

    /// How many columns and rows there are — what [`GridCell::ALL`] resolves to.
    pub fn counts(&self) -> (u16, u16) {
        (self.columns.len().max(1) as u16, self.rows.len().max(1) as u16)
    }
}

/// **How many tracks a grid item covers** — a count, or every one there is.
///
/// `All` is CSS's `1 / -1`: "to the end of the grid", whatever the template turns out to say. It
/// stays true when a column is added later, which a hard-coded count does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// Exactly this many tracks.
    Of(u16),
    /// Every track on that axis.
    All,
}

impl From<u16> for Span {
    fn from(n: u16) -> Self {
        Span::Of(n)
    }
}

impl From<i32> for Span {
    fn from(n: i32) -> Self {
        Span::Of(n.max(1) as u16)
    }
}

impl From<&str> for Span {
    /// `"all"` (or CSS's `"-1"`) for every track; a number for that many.
    fn from(t: &str) -> Self {
        match t.trim().to_ascii_lowercase().as_str() {
            "all" | "-1" => Span::All,
            n => n.parse::<u16>().map_or(Span::All, Span::Of),
        }
    }
}

impl Span {
    /// The stored span: a count, or the [`ALL`](GridCell::ALL) sentinel.
    pub(crate) fn stored(self) -> u16 {
        match self {
            Span::Of(n) => n.max(1),
            Span::All => GridCell::ALL,
        }
    }
}

/// **Where an item starts on one axis, and how far it reaches** — CSS `grid-column` /
/// `grid-row`, in the spellings a stylesheet writes them.
///
/// | spelling | means |
/// | --- | --- |
/// | `"3"` / `3` | start at line 3, one track |
/// | `"1 / -1"` | start at line 1, run to the end |
/// | `"1 / span 2"` | start at line 1, cover two tracks |
/// | `"2 / 4"` | start at line 2, end at line 4 |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridLine {
    /// 1-based start line.
    pub start: u16,
    /// How far it reaches from there.
    pub span: Span,
}

impl From<u16> for GridLine {
    fn from(start: u16) -> Self {
        GridLine { start: start.max(1), span: Span::Of(1) }
    }
}

impl From<i32> for GridLine {
    fn from(start: i32) -> Self {
        GridLine { start: start.max(1) as u16, span: Span::Of(1) }
    }
}

impl From<&str> for GridLine {
    /// The CSS spelling. Anything unreadable is line 1, one track — never a panic, because these
    /// arrive from config and from plugins.
    fn from(t: &str) -> Self {
        let t = t.trim();
        let Some((start, end)) = t.split_once('/') else {
            return GridLine {
                start: t.parse::<u16>().unwrap_or(1).max(1),
                span: Span::Of(1),
            };
        };
        let start_line = start.trim().parse::<u16>().unwrap_or(1).max(1);
        let end = end.trim();
        let span = match end.strip_prefix("span") {
            // `1 / span 2` — a count of tracks.
            Some(n) => Span::from(n.trim()),
            // `1 / -1` — to the end. `2 / 4` — up to that line, which is 4 - 2 tracks.
            None => match end.parse::<i32>() {
                Ok(-1) => Span::All,
                Ok(line) if line > start_line as i32 => Span::Of((line - start_line as i32) as u16),
                _ => Span::All,
            },
        };
        GridLine { start: start_line, span }
    }
}

impl GridCell {
    /// **Span every track on that axis** — CSS's `1 / -1`.
    ///
    /// A sentinel rather than a field, so [`Layout`] stays `Copy`. How many tracks that turns out
    /// to be is the *parent's* answer, so it is resolved during layout, when the parent is known —
    /// which is also what lets a child be built before whatever places it.
    pub const ALL: u16 = u16::MAX;

    /// This cell with every `ALL` span replaced by the real track counts.
    pub(crate) fn resolved(self, columns: u16, rows: u16) -> Self {
        Self {
            col_span: if self.col_span == Self::ALL { columns.max(1) } else { self.col_span },
            row_span: if self.row_span == Self::ALL { rows.max(1) } else { self.row_span },
            ..self
        }
    }
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
    /// **The accent this widget and everything inside it paints its chrome with** — CSS's
    /// inherited custom property, for a hue.
    ///
    /// A composition with a colour of its own — a severity-toned notification card, a pane tinted
    /// by its own frame — sets this and everything inside follows: focus rings, hover fills,
    /// selected washes, scrollbar thumbs, carets. `None` (the default, and almost everywhere) means
    /// inherit whatever an ancestor published, else the theme's accent. Read through
    /// [`PaintCx::accent`](crate::component::PaintCx::accent); applied to the whole subtree by
    /// `paint_child`, so no widget opts in and none can forget.
    ///
    /// It does **not** redefine a declared meaning: a `Badge::accent`, an `Alert::info` or a
    /// destructive button keeps the colour its variant names, exactly as it does inside any other
    /// toned container.
    ///
    /// The alternative it replaces is painting the subtree under a **swapped theme**, which needs a
    /// separate paint call per subtree and is what stops a container from painting its own children.
    pub accent: Option<Color>,
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
            accent: None,
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
    /// Minimum height — see [`min_width`](Style::min_width).
    pub min_height: Option<Length>,
    /// Maximum width. `None` ⇒ unbounded. Used to cap a node against its parent —
    /// an overlay panel is capped at `Percent(1.0)` so a fixed `Px` size can never
    /// make a dialog larger than the window.
    pub max_width: Option<Length>,
    /// Maximum height — see [`max_width`](Style::max_width).
    pub max_height: Option<Length>,
    pub flex_grow: f32,

    /// **Take this much of the room the parent has to give** — a share, whatever kind of parent
    /// that turns out to be.
    ///
    /// The mechanism differs and the meaning does not, which is the whole reason this is a property
    /// and not an idiom a caller writes:
    /// - **in a flex container** it becomes `flex: <n> 1 0` — grow, a zero basis and permission to
    ///   shrink — because grow alone distributes only free space and a column of them collapses to
    ///   its content;
    /// - **in a grid** it is nothing at all. The track already sized the cell, and the item stretches
    ///   into it.
    ///
    /// Writing the flex spelling by hand is what broke two docks in one sidebar: a zero base size
    /// is a *definite zero height* in a grid cell, so each container drew its title row and nothing
    /// else. Resolved in [`crate::layout`], against the parent, so no caller has to know which case
    /// they are in.
    pub share: Option<f32>,
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
            flex_grow: 0.0,
            share: None,
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
            // `auto` unless the author said otherwise, exactly as flexbox has it.
            flex_basis: self
                .flex_basis
                .map_or(taffy::style::Dimension::Auto, |b| b.to_taffy()),
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

#[cfg(test)]
mod one_spacing_builder {
    use super::*;

    /// **A space is written either way through one builder.**
    ///
    /// It was two — `gap(8.0)` beside `gap_spacing(Spacing::Sm)`, `padding(6.0)` beside
    /// `pad_all(Spacing::Xs)` — which is two paths over one property inside the library itself.
    /// The usage said what that costs: `.gap` 112 times against `.gap_spacing`'s 8, with the docs
    /// telling everyone to prefer the token. Advice loses to whichever name is shorter.
    #[test]
    fn a_space_is_pixels_or_a_step_and_one_builder_takes_both() {
        assert_eq!(Space::from(8), Space::Px(8.0), "a bare integer is pixels");
        assert_eq!(Space::from(8.0), Space::Px(8.0), "and a bare decimal");
        assert_eq!(Space::from("8px"), Space::Px(8.0), "the suffix is optional");
        assert_eq!(Space::from(Spacing::Sm), Space::Step(Spacing::Sm));
        assert_eq!(
            Space::from("sm"),
            Space::Step(Spacing::Sm),
            "the wire spelling"
        );
        assert_eq!(
            Space::from("MD"),
            Space::Step(Spacing::Md),
            "case is not a spelling"
        );
    }

    /// **A space nobody can read is none, not a panic** — these arrive from `config.toml` and from
    /// a plugin's description as well as from Rust, the same rule the grid's tracks follow.
    #[test]
    fn an_unreadable_space_is_no_space() {
        assert_eq!(Space::from("roomy"), Space::Px(0.0));
        assert_eq!(Space::from(""), Space::Px(0.0));
    }
}
