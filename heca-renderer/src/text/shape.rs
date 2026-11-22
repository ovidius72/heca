//! **Shaping text into glyphs**, said once: the line height every shaped text gets, the face it is
//! shaped in, and the slant a faux italic is drawn with.

use cosmic_text::{
    Attrs, Buffer, Family, FeatureTag, FontFeatures, FontSystem, Metrics, Shaping, Style, Weight,
};

/// A shaped line is this many times its font size tall — the line box every label and terminal row
/// is centred in.
const LINE_HEIGHT: f32 = 1.2;

/// The box text is shaped in: large enough that nothing wraps. A label or a terminal row is one
/// line; where it goes is decided after shaping.
const SHAPING_BOX: f32 = 10_000.0;

/// How far a faux italic leans, as a share of the line height — the top of a glyph moves right by
/// half of it and the bottom left by half.
const FAUX_ITALIC_SLANT: f32 = 0.14;

/// The CSS weight of an ordinary face, and of a bold one — what an on/off `bold` (a terminal cell)
/// asks for.
pub(super) const REGULAR_WEIGHT: u16 = 400;
pub(super) const BOLD_WEIGHT: u16 = 700;

/// The weight an on/off bold flag asks for.
pub(super) const fn weight_of(bold: bool) -> u16 {
    if bold { BOLD_WEIGHT } else { REGULAR_WEIGHT }
}

/// The face a text is shaped in.
pub(super) struct Face<'a> {
    pub(super) family: &'a str,
    /// CSS weight (400 regular … 700 bold). The family's nearest face to it is used.
    pub(super) weight: u16,
    pub(super) italic: bool,
    /// The air after every letter, in `em` of the font size (`0.0` = the font's own spacing).
    pub(super) letter_spacing: f32,
    /// Whether `calt` / `liga` / `clig` apply. Off shapes every character standalone.
    pub(super) ligatures: bool,
}

/// The metrics a text of `size` (physical px) is shaped with.
pub(super) fn metrics(size: f32) -> Metrics {
    Metrics::new(size, size * LINE_HEIGHT)
}

/// **Shape `text` at `size` (physical px) in `face`** — the one place a label, a terminal row and a
/// cell measurement are shaped, so they cannot disagree about what a face or a line is.
pub(super) fn shape(fonts: &mut FontSystem, text: &str, size: f32, face: &Face<'_>) -> Buffer {
    let mut buffer = Buffer::new(fonts, metrics(size));
    buffer.set_size(Some(SHAPING_BOX), Some(SHAPING_BOX));
    let mut attrs = Attrs::new()
        .family(Family::Name(face.family))
        .weight(Weight(face.weight))
        .style(if face.italic {
            Style::Italic
        } else {
            Style::Normal
        });
    if face.letter_spacing != 0.0 {
        attrs = attrs.letter_spacing(face.letter_spacing);
    }
    // `Shaping::Advanced` runs the full HarfBuzz-style shaper; with the whole text visible,
    // `calt`/`liga`/`clig` (on by default in coding fonts) collapse e.g. `->` into a ligature. When
    // ligatures are off, those features are turned off so each character shapes standalone.
    if !face.ligatures {
        let mut features = FontFeatures::new();
        features
            .disable(FeatureTag::CONTEXTUAL_ALTERNATES)
            .disable(FeatureTag::STANDARD_LIGATURES)
            .disable(FeatureTag::CONTEXTUAL_LIGATURES);
        attrs = attrs.font_features(features);
    }
    // A text-by-default emoji (⏺) the face lacks is shaped in a text face that has it, not handed to
    // the colour emoji face (`presentation.rs`).
    match super::presentation::pieces(fonts, text, face.family) {
        None => buffer.set_text(text, &attrs, Shaping::Advanced, None),
        Some(pieces) => buffer.set_rich_text(
            pieces.iter().map(|(piece, family)| match family {
                Some(family) => (*piece, attrs.clone().family(Family::Name(family))),
                None => (*piece, attrs.clone()),
            }),
            &attrs,
            Shaping::Advanced,
            None,
        ),
    }
    buffer.shape_until_scroll(fonts, false);
    buffer
}

/// **How far a faux italic leans**, in logical px, for a line `line_height` physical px tall —
/// `0.0` when the text is upright. At least a pixel, so a small label still visibly leans.
pub(super) fn faux_italic_slant(line_height: f32, scale: f32, faux_italic: bool) -> f32 {
    if faux_italic {
        (line_height / scale * FAUX_ITALIC_SLANT).max(1.0)
    } else {
        0.0
    }
}
