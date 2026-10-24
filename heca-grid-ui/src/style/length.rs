//! **Lengths** — a size along one axis, and an out-of-flow placement.

use super::*;

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

impl crate::prop::PropName for Length {
    // CSS lengths are an open string value rather than a finite list of enum names.
    const VARIANT_NAMES: &'static [&'static str] = &[];
    fn from_prop_name(name: &str) -> Option<Self> { name.parse().ok() }
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

/// What [`Length`]'s `from_str` returns when a string is none of the four spellings.
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
            // **One parser, not a second copy**, so the wire and native code cannot drift about
            // what `"50%"` means.
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

    pub(super) fn to_taffy(self) -> taffy::Dimension {
        use taffy::prelude::*;
        match self {
            Length::Auto => auto(),
            Length::Px(v) => length(v),
            Length::Percent(p) => percent(p),
        }
    }

    /// The same value as a taffy **inset** — the type an edge offset takes, which admits `Auto`
    /// (meaning "this edge is not pinned") where a size would not.
    pub(super) fn to_taffy_inset(self) -> taffy::LengthPercentageAuto {
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
        assert_eq!(
            Length::from(200),
            Length::Px(200.0),
            "a bare integer is pixels"
        );
        assert_eq!(
            Length::from(200.0),
            Length::Px(200.0),
            "a bare decimal is pixels"
        );
        assert_eq!(Length::from("200"), Length::Px(200.0));
        assert_eq!(
            Length::from("200px"),
            Length::Px(200.0),
            "the px suffix is optional"
        );
        assert_eq!(
            Length::from(" 200 px "),
            Length::Px(200.0),
            "and forgiving of spaces"
        );
        assert_eq!(Length::from("auto"), Length::Auto);
        assert_eq!(Length::from("AUTO"), Length::Auto, "case is not a spelling");
        assert_eq!(
            Length::from("50%"),
            Length::Percent(0.5),
            "the wire spelling is a fraction"
        );
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

    /// **One parser, not two.** The deserializer calls `from_str`, so the wire and native code
    /// cannot drift about what `"50%"` means; this holds them to it.
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
