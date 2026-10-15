//! **Spacing** — the theme's spacing tokens and a space written as a token or in pixels.

use super::*;

/// A theme-derived **spacing** token for container padding. Resolved to px from the
/// inherited font at layout time (so it scales with the theme / font zoom) — callers
/// pick a token instead of hand-computing px. Used via `LayoutExt::pad`/`pad_x`/`pad_y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Spacing {
    None,
    /// **A hairline of air** — about a pixel at the usual font, and it still scales with it. The gap
    /// between lines that belong to one block, such as the lines of text inside a card, where `Xs`
    /// reads as separate things. Too thin to separate two bordered controls: a
    /// [`ButtonGroup`](crate::widgets::ButtonGroup) uses `Xs`.
    Hairline,
    Xs,
    Sm,
    Md,
    Lg,
    /// The widest step: room around a thin target the pointer has to find, such as the line a
    /// carried pane is dropped on.
    Xl,
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
            "hairline" => return Ok(Space::Step(Spacing::Hairline)),
            "xs" => return Ok(Space::Step(Spacing::Xs)),
            "sm" => return Ok(Space::Step(Spacing::Sm)),
            "md" => return Ok(Space::Step(Spacing::Md)),
            "lg" => return Ok(Space::Step(Spacing::Lg)),
            "xl" => return Ok(Space::Step(Spacing::Xl)),
            _ => {}
        }
        // `px` is optional and means the same as no suffix — the same rule `Length` follows.
        let t = t.strip_suffix("px").map_or(t, str::trim_end);
        t.parse::<f32>().map(Space::Px).map_err(|_| SpaceParseError)
    }
}

/// What [`Space`]'s `from_str` returns when a string is neither a step name nor a length.
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
            Spacing::Hairline => 0.08,
            Spacing::Xs => 0.25,
            Spacing::Sm => 0.5,
            Spacing::Md => 0.85,
            Spacing::Lg => 1.25,
            Spacing::Xl => 2.0,
        }
    }
}


#[cfg(test)]
mod one_spacing_builder {
    use super::*;

    /// **A space is written either way through one builder.**
    ///
    /// `.gap(8.0)` and `.gap("sm")` are one builder over one property, so there is no second name
    /// for a caller to choose between.
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
