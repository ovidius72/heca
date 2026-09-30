//! The value sets that say how things are arranged.

use serde::{Deserialize, Serialize};

use crate::PropValue;

/// Semantic cross-axis alignment — mirrors grid-ui `Align` (the plan's `align` prop enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewAlign {
    Start,
    Center,
    End,
    Stretch,
}

value_set! {
    /// Which way a rule runs — mirrors grid-ui `Orientation` (`Separator`).
    pub enum ViewOrientation {
        Horizontal => "horizontal",
        Vertical => "vertical",
    }
}

value_set! {
    /// Which way a scroll region scrolls — mirrors grid-ui `ScrollAxes`.
    pub enum ViewScrollAxes {
        Vertical => "vertical",
        Horizontal => "horizontal",
        Both => "both",
    }
}

value_set! {
    /// Where a scroll region puts the descendant it follows — mirrors grid-ui `RevealAlign`.
    pub enum ViewRevealAlign {
        /// Scroll the least that makes it visible.
        Minimal => "minimal",
        /// Keep it at the centre of the viewport.
        Center => "center",
    }
}

value_set! {
    /// A spacing step from the theme — mirrors grid-ui `Spacing`.
    ///
    /// **Resolved from the inherited font at layout, never a pixel count**, so a described tree spaces
    /// itself the way the rest of the app does and follows a font or theme change with nothing
    /// rewritten. It is what an author reaches for instead of `padding(16.0)`: raw pixels are still
    /// there for the rare case that genuinely needs one, and are the wrong default.
    pub enum ViewSpacing {
        None => "none",
        Xs => "xs",
        Sm => "sm",
        Md => "md",
        Lg => "lg",
    }
}

/// **A space: a number of pixels, or a step of the theme's rhythm** — mirrors grid-ui `Space`.
///
/// One authoring type, because spacing is one property. It was two builders on each axis — a px
/// `gap` beside a `gap_spacing` step, `padding` beside `pad_all` — and the docs told everyone to
/// prefer the step while the px name stayed the shorter, more obvious one.
///
/// ```ignore
/// VStack::new().gap(8)                 // eight pixels
/// VStack::new().gap(ViewSpacing::Sm)   // a step of the rhythm
/// VStack::new().gap("sm")              // the same step, said as JSON would
/// ```
///
/// **Prefer the step.** It is resolved from the inherited font at layout, so it follows a font,
/// size-variant or zoom change with nothing rewritten; a pixel count is tuned for one font size
/// and wrong at every other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewSpace {
    /// Logical pixels, fixed whatever the font does.
    Px(f32),
    /// A step of the theme's rhythm.
    Step(ViewSpacing),
}

impl From<ViewSpacing> for ViewSpace {
    fn from(s: ViewSpacing) -> Self {
        ViewSpace::Step(s)
    }
}

impl From<f32> for ViewSpace {
    fn from(v: f32) -> Self {
        ViewSpace::Px(v)
    }
}

impl From<f64> for ViewSpace {
    /// Rust reads a bare decimal as `f64`, so without this `.gap(8.0)` does not compile.
    fn from(v: f64) -> Self {
        ViewSpace::Px(v as f32)
    }
}

impl From<i32> for ViewSpace {
    /// …and a bare integer as `i32`.
    fn from(v: i32) -> Self {
        ViewSpace::Px(v as f32)
    }
}

impl std::str::FromStr for ViewSpace {
    type Err = ViewSpaceParseError;

    /// **The one reader.** `"sm"` for a step, `"8"` / `"8px"` for pixels — the spellings the
    /// described side reads. A step is found by [`name`](ViewSpacing::name) in
    /// [`ViewSpacing::ALL`], so the words are written once, where the set is.
    fn from_str(t: &str) -> Result<Self, Self::Err> {
        let t = t.trim();
        if let Some(step) = ViewSpacing::ALL
            .iter()
            .find(|step| step.name().eq_ignore_ascii_case(t))
        {
            return Ok(ViewSpace::Step(*step));
        }
        t.strip_suffix("px")
            .map_or(t, str::trim_end)
            .parse::<f32>()
            .map(ViewSpace::Px)
            .map_err(|_| ViewSpaceParseError)
    }
}

/// What [`ViewSpace::from_str`](std::str::FromStr::from_str) returns when a string is neither a step
/// name nor a length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewSpaceParseError;

impl std::fmt::Display for ViewSpaceParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("expected a number, a length like \"8px\", or a step name (none/xs/sm/md/lg)")
    }
}

impl std::error::Error for ViewSpaceParseError {}

impl From<&str> for ViewSpace {
    /// `.gap("sm")` / `.padding("8px")`.
    ///
    /// Anything unreadable is **no space at all** rather than a panic — the rule the widget
    /// library's own `Space` follows: these arrive from config and from plugins, so a typo costs its
    /// author a gap and not the host. Use [`from_str`](std::str::FromStr::from_str) to be told.
    fn from(t: &str) -> Self {
        t.parse().unwrap_or(ViewSpace::Px(0.0))
    }
}

impl From<ViewSpace> for PropValue {
    /// A step travels as its **name** and a length as a number — the two spellings the layout
    /// setting reads back, so one property name carries either.
    fn from(s: ViewSpace) -> Self {
        match s {
            ViewSpace::Px(v) => PropValue::Float(v as f64),
            ViewSpace::Step(step) => step.into(),
        }
    }
}

/// **CSS `order`** for a described node — mirrors grid-ui `Order`: where a node is laid out among
/// its siblings. Lower first; ties and nodes that say nothing (`0`) keep the order they were added.
///
/// A number or a list: `3`, or `[0, 5]` to slot between two siblings at `0` and `1` that you do not
/// own. It travels as a number when it is one place and a list of numbers otherwise, which is what
/// the realizing side's one parser reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewOrder(pub Vec<i64>);

impl From<Vec<i64>> for ViewOrder {
    fn from(places: Vec<i64>) -> Self {
        ViewOrder(places)
    }
}

/// On the wire: a number when it is one place (`3`), a list otherwise (`[0, 5]`) — the spellings
/// the realizing side's one parser reads.
impl serde::Serialize for ViewOrder {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0.as_slice() {
            [one] => s.serialize_i64(*one),
            places => places.serialize(s),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ViewOrder {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Repr {
            One(i64),
            List(Vec<i64>),
        }
        Ok(match Repr::deserialize(d)? {
            Repr::One(v) => ViewOrder(vec![v]),
            Repr::List(places) => ViewOrder(places),
        })
    }
}

impl From<i32> for ViewOrder {
    fn from(v: i32) -> Self {
        ViewOrder(vec![v as i64])
    }
}

impl From<i64> for ViewOrder {
    fn from(v: i64) -> Self {
        ViewOrder(vec![v])
    }
}

impl<const N: usize> From<[i32; N]> for ViewOrder {
    fn from(places: [i32; N]) -> Self {
        ViewOrder(places.iter().map(|&p| p as i64).collect())
    }
}

impl<const N: usize> From<[i64; N]> for ViewOrder {
    fn from(places: [i64; N]) -> Self {
        ViewOrder(places.to_vec())
    }
}

impl From<ViewOrder> for PropValue {
    fn from(o: ViewOrder) -> Self {
        match o.0.as_slice() {
            [one] => PropValue::Int(*one),
            places => PropValue::List(places.iter().map(|&p| PropValue::Int(p)).collect()),
        }
    }
}

value_set! {
    /// How a [`ButtonGroup`](WidgetKind::ButtonGroup) shows its actions — mirrors grid-ui `Display`.
    ///
    /// Whatever does not fit collapses into a ⋮ menu whichever of these is chosen; this only decides
    /// how wide each action is before that happens.
    pub enum ViewDisplay {
        /// Words while there is room, icons once there is not.
        ///
        /// ⚠️ **Not settled** — at some widths the group alternates between the two on successive
        /// layouts, because taking the words off is what makes the row fit. Prefer the other two.
        Auto => "auto",
        /// Always icons, however much room there is. The labels still say what the hover bubble and the
        /// collapsed menu read. **The default**, because it is the one that is settled.
        IconOnly => "icon_only",
        /// Always words. The group collapses into the menu sooner, because each action is wider.
        Full => "full",
    }
}

value_set! {
    /// How children are distributed along the main axis — mirrors grid-ui `Justify`.
    ///
    /// Missed by F003/P011/T019, which took its list from the widgets' own properties: this one is a
    /// `Layout` field, so it never appeared there. Found while building the authoring layer on top.
    pub enum ViewJustify {
        Start => "start",
        Center => "center",
        End => "end",
        SpaceBetween => "space_between",
        SpaceAround => "space_around",
        SpaceEvenly => "space_evenly",
    }
}

impl From<ViewAlign> for PropValue {
    fn from(v: ViewAlign) -> Self {
        PropValue::Align(v)
    }
}
