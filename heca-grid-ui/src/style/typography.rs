//! **How text is set**: its weight, the air between its letters, the height of its lines and the
//! case it is shown in — CSS `font-weight`, `letter-spacing`, `line-height` and `text-transform`.
//!
//! Spacing and line height are **steps**, the way a space is a [`Spacing`](super::Spacing) step:
//! each is a multiple of the font size, so they follow a font-size change and no caller ever writes a
//! number. A [`Label`](crate::widgets::Label) takes all four:
//!
//! ```ignore
//! Label::new("Members · 4")
//!     .font_weight(FontWeight::SemiBold)
//!     .letter_spacing(LetterSpacing::Wide)
//!     .line_height(LineHeight::Relaxed)
//!     .transform(TextTransform::Uppercase)
//! ```

use crate::font::MONO_LINE_RATIO;

/// **How heavy the glyphs are** — CSS `font-weight`.
///
/// The embedded UI family (Geist Mono) ships a real face for each of the four, so none is faked. A
/// family the user configures is asked for the same weight and gives its nearest installed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, heca_grid_ui_macros::PropName)]
pub enum FontWeight {
    /// The ordinary weight (CSS 400). The default.
    #[default]
    Regular,
    /// A little heavier than regular (CSS 500): text that should stand out without shouting.
    Medium,
    /// Between medium and bold (CSS 600): a section title, a small uppercase heading.
    SemiBold,
    /// Bold (CSS 700).
    Bold,
}

impl FontWeight {
    /// The CSS / OpenType weight number the shaper asks the font for.
    pub const fn value(self) -> u16 {
        match self {
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
            Self::Bold => 700,
        }
    }
}

/// **The air added after every letter** — CSS `letter-spacing`, as a step of the font size.
///
/// Wide spacing is what a small uppercase heading wants; tight spacing pulls a large title together.
/// The step is the same everywhere it is read — what the label measures, where it cuts, how it wraps
/// and what the renderer draws — so text and its box never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, heca_grid_ui_macros::PropName)]
pub enum LetterSpacing {
    /// A little less than the font's own spacing.
    Tight,
    /// The font's own spacing — nothing added. The default.
    #[default]
    Normal,
    /// A little more: a small uppercase heading.
    Wide,
    /// More again: a label meant to read as a spaced-out caption.
    Wider,
}

impl LetterSpacing {
    /// The space added after each letter, as a fraction of the font size (CSS `em`). Negative pulls
    /// letters together.
    pub const fn em(self) -> f32 {
        match self {
            Self::Tight => -0.025,
            Self::Normal => 0.0,
            Self::Wide => 0.05,
            Self::Wider => 0.1,
        }
    }
}

/// **How tall one line of text is** — CSS `line-height`, as a step of the font size.
///
/// The text sits in the middle of its line, so a taller line adds the same air above and below it.
/// A wrapping label stacks its lines at this height, and a one-line label is this tall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, heca_grid_ui_macros::PropName)]
pub enum LineHeight {
    /// Lines close together: a large title on two lines.
    Tight,
    /// The library's own line — the height every label had before this step existed. The default.
    #[default]
    Normal,
    /// More air between lines: a paragraph meant to be read.
    Relaxed,
    /// Double spacing.
    Loose,
}

impl LineHeight {
    /// The line height as a multiple of the font size.
    pub const fn ratio(self) -> f32 {
        match self {
            Self::Tight => 1.15,
            Self::Normal => MONO_LINE_RATIO,
            Self::Relaxed => 1.7,
            Self::Loose => 2.0,
        }
    }
}

/// **The case the text is shown in** — CSS `text-transform`.
///
/// Only what is drawn changes: the text a label holds, and what it reports as its accessible name,
/// stay as written. That is what lets a language file keep ordinary casing while a heading shows it
/// in capitals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, heca_grid_ui_macros::PropName)]
pub enum TextTransform {
    /// Shown as written. The default.
    #[default]
    None,
    /// Every letter in capitals.
    Uppercase,
    /// Every letter in small letters.
    Lowercase,
    /// The first letter of every word in capitals, the rest as written.
    Capitalize,
}

impl TextTransform {
    /// `text` as it is shown.
    ///
    /// A letter whose capital is two letters (`ß` → `SS`) becomes two characters, as in CSS.
    pub fn apply(self, text: &str) -> String {
        match self {
            Self::None => text.to_owned(),
            Self::Uppercase => text.to_uppercase(),
            Self::Lowercase => text.to_lowercase(),
            Self::Capitalize => {
                let mut out = String::with_capacity(text.len());
                let mut at_word_start = true;
                for ch in text.chars() {
                    if at_word_start && ch.is_alphanumeric() {
                        out.extend(ch.to_uppercase());
                    } else {
                        out.push(ch);
                    }
                    at_word_start = ch.is_whitespace();
                }
                out
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_transform_changes_only_what_it_names() {
        let text = "members of the team";
        assert_eq!(TextTransform::None.apply(text), text);
        assert_eq!(TextTransform::Uppercase.apply(text), "MEMBERS OF THE TEAM");
        assert_eq!(TextTransform::Lowercase.apply("Members"), "members");
        assert_eq!(TextTransform::Capitalize.apply(text), "Members Of The Team");
        assert_eq!(
            TextTransform::Capitalize.apply("mIXED case"),
            "MIXED Case",
            "the rest of each word stays as written, as in CSS"
        );
    }

    #[test]
    fn the_default_steps_change_nothing() {
        assert_eq!(FontWeight::default().value(), 400);
        assert_eq!(LetterSpacing::default().em(), 0.0);
        assert_eq!(LineHeight::default().ratio(), MONO_LINE_RATIO);
    }
}
