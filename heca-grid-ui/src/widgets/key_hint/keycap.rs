//! Drawing a keycap chip — the shared visual stamped over a target or into a
//! scene. Sizing, the [`KeycapVariant`] styling, and the glyph-contrast rule, all
//! theme-driven. Reused by other overlays (the context-menu quick-pick) and by
//! hosts stamping caps over non-widget targets (terminal hyperlink spans).

use crate::color::Color;
use crate::component::PaintCx;
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::widgets::NfGlyph;
use heca_core::layout::{Rectangle, Size};

/// Horizontal / vertical padding inside the keycap, in fractions of the hint font.
const PAD_X_FRAC: f32 = 0.42;
const PAD_Y_FRAC: f32 = 0.22;
/// Per-glyph advance estimate (fraction of font) for sizing the keycap to its text.
const GLYPH_ADVANCE_FRAC: f32 = 0.62;
/// Keycap glow intensity (scaled by the theme `glow_size`) — soft, not blazing.
const KEYCAP_GLOW: f32 = 0.45;
/// How far a keycap's chip must sit from the theme `background` in luminance for a
/// background-coloured letter to still read on it. Below this the letter takes the contrasting tone
/// instead — see [`keycap_glyph_color`].
const KEYCAP_MIN_GLYPH_CONTRAST: f32 = 0.15;

/// Size of the keycap chip for `text` at `font` (logical px) — the exact sizing
/// [`KeyHint`](super::KeyHint) uses. Exposed so hosts can stamp a standalone keycap over targets
/// that are not part of a component tree (e.g. terminal hyperlink spans) without
/// duplicating the formula.
pub fn keycap_size(font: f32, text: &str) -> Size {
    keycap_size_cells(font, text.chars().count().max(1))
}

/// [`keycap_size`] for a **Nerd Font** keycap — one square cell, whatever the glyph.
pub fn keycap_size_nf(font: f32) -> Size {
    keycap_size_cells(font, 1)
}

/// The chip size for `cells` character cells of content — the one formula both callers share.
fn keycap_size_cells(font: f32, cells: usize) -> Size {
    let font64 = font as f64;
    let pad_x = (font * PAD_X_FRAC) as f64;
    let pad_y = (font * PAD_Y_FRAC) as f64;
    let w =
        (cells as f64 * (font * GLYPH_ADVANCE_FRAC) as f64 + 2.0 * pad_x).max(font64 + 2.0 * pad_y);
    let h = font64 + 2.0 * pad_y;
    Size::new(w, h)
}

/// Visual style of a keycap chip painted by [`paint_keycap`]. The caller picks a
/// semantic variant; [`paint_keycap`] owns the styling (all values from the theme).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeycapVariant {
    /// Solid glowing chip — opaque base + accent tint + dark glyph. The default
    /// hint/pick look, stamped over arbitrary content (terminal hyperlink spans,
    /// pick targets), where the chip must stay legible against whatever is beneath
    /// it.
    #[default]
    Filled,
    /// A quiet [`Tag`](crate::widgets::Tag)-style chip: subtle fill + accent border, **no glow**,
    /// accent glyph. For keycaps shown on an already dark, host-owned surface (e.g. the
    /// context-menu quick-pick inside the menu panel) where a glowing halo would be too
    /// heavy and a crisp outline reads better.
    Bordered,
}

/// Paint a standalone keycap — a glowing chip with a centered dark glyph — at `cap`
/// with `text`. The shared hint visual, factored out of [`KeyHint`](super::KeyHint) so it can be
/// stamped directly into a scene over targets that are not widgets (e.g. terminal
/// hyperlink spans, via the host's overlay pass) or reused by other overlays (the
/// context-menu quick-pick). `color` overrides the default theme `accent` for both
/// fill and glow; `variant` picks [`KeycapVariant::Filled`] (content overlays) or
/// [`KeycapVariant::Bordered`] (keycaps on an owned menu/panel surface).
pub fn paint_keycap(
    cx: &mut PaintCx,
    cap: Rectangle,
    text: &str,
    font: f32,
    color: Option<Color>,
    variant: KeycapVariant,
) {
    if text.is_empty() {
        return;
    }
    paint_keycap_content(cx, cap, KeycapContent::Text(text), font, color, variant);
}

/// [`paint_keycap`] for a **Nerd Font** glyph — a shift/command/escape key drawn as its own picture
/// rather than spelled out (`NfGlyph`).
///
/// A separate entry point rather than a `&str` a caller could pass a glyph char in: the codepoint
/// has to be shaped with the Nerd Font family, and that is a property of the *run*, not of the
/// string. Passing the char to [`paint_keycap`] would shape it with the UI face, which does not have
/// it — a silent empty box.
pub fn paint_keycap_nf(
    cx: &mut PaintCx,
    cap: Rectangle,
    glyph: NfGlyph,
    font: f32,
    color: Option<Color>,
    variant: KeycapVariant,
) {
    paint_keycap_content(cx, cap, KeycapContent::Nf(glyph), font, color, variant);
}

/// What is drawn inside a keycap chip: a string in the UI face, or a Nerd Font glyph.
#[derive(Clone, Copy)]
enum KeycapContent<'a> {
    Text(&'a str),
    Nf(NfGlyph),
}

impl KeycapContent<'_> {
    /// Draw the content centered in `cap`, in the face it belongs to.
    fn paint(self, cx: &mut PaintCx, cap: Rectangle, color: Color, font: f32) {
        match self {
            KeycapContent::Text(t) => {
                cx.text(cap, t, color, font, TextAlign::Center, TextStyle::BOLD)
            }
            KeycapContent::Nf(g) => {
                if let Some(c) = g.char() {
                    cx.nf_icon(cap, &c.to_string(), color, font);
                }
            }
        }
    }
}

/// One cap of a keyboard chord: a **Nerd Font key glyph** where the key has a picture (shift,
/// command, escape, the arrows), the literal text where it does not (`h`, `]`, `F5`).
///
/// Two cases and not one, because the two are drawn in different faces — see [`paint_keycap_nf`].
/// A caller builds a chord as a `Vec<KeyCap>` (`[λ] [⇧] [e]`) and the widget draws the chips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyCap {
    /// A key with a glyph in the embedded Nerd Font.
    Nf(NfGlyph),
    /// A key spelled out — a letter, a digit, a punctuation key, or a named key with no glyph.
    Text(String),
}

impl KeyCap {
    /// The chip size for this cap at `font`.
    pub fn size(&self, font: f32) -> Size {
        match self {
            KeyCap::Nf(_) => keycap_size_nf(font),
            KeyCap::Text(t) => keycap_size(font, t),
        }
    }

    /// Draw this cap at `cap`, in the face it belongs to.
    pub fn paint(
        &self,
        cx: &mut PaintCx,
        cap: Rectangle,
        font: f32,
        color: Option<Color>,
        variant: KeycapVariant,
    ) {
        match self {
            KeyCap::Nf(g) => paint_keycap_nf(cx, cap, *g, font, color, variant),
            KeyCap::Text(t) => paint_keycap(cx, cap, t, font, color, variant),
        }
    }
}

/// The shared chip: the surface (per [`KeycapVariant`]) plus whatever sits in it.
/// **The letter's colour on a filled keycap** — the theme `background`, unless the chip is too close
/// to it to read, in which case the contrasting tone.
///
/// The chip is not `tint`: it is an opaque `background` base with `tint` laid over it at `alpha`, so
/// the fill the eye sees is the blend. Asking about `tint` alone would answer for a colour that is
/// never painted.
///
/// **Why not simply [`Theme::on`](heca_theme::Theme::on) every time.** That is the more contrasty answer and it would
/// change how heca looks: measured on `grid_tron`, the chip lands at luminance 0.24 against a 0.003
/// background and a 0.82 foreground, so `on` returns the *foreground* and the letters would flip
/// from dark-on-accent to light-on-accent. The dark glyph is the design; this only steps in when
/// keeping it would make the letter vanish.
///
/// **Why it steps in at all.** The glyph used to be `background` unconditionally, which is right for
/// an accent far from the background and wrong the moment it is not — a light-background theme had a
/// light glyph on a light chip. `hint_color` then made that reachable on any theme: set the letters
/// near your own background and you get an empty keycap. A colour a user is free to choose cannot
/// carry a legibility rule that only holds for one of its values.
fn keycap_glyph_color(theme: &heca_theme::Theme, tint: Color, alpha: u8) -> Color {
    let chip = theme.background.lerp(tint, alpha as f32 / 255.0);
    let reads_on_background =
        (chip.luminance() - theme.background.luminance()).abs() >= KEYCAP_MIN_GLYPH_CONTRAST;
    if reads_on_background {
        theme.background
    } else {
        theme.on(chip)
    }
}

fn paint_keycap_content(
    cx: &mut PaintCx,
    cap: Rectangle,
    content: KeycapContent<'_>,
    font: f32,
    color: Option<Color>,
    variant: KeycapVariant,
) {
    let (accent, glow_c, background, ctrl_radius, keycap_alpha) = {
        let t = cx.theme();
        (
            cx.accent(),
            t.colors.glow,
            t.colors.background,
            t.colors.control_radius(),
            t.colors.interaction.keycap,
        )
    };
    let keycap_c = color.unwrap_or(accent);
    let radius = ctrl_radius.min((cap.size.h / 2.0) as f32);
    match variant {
        KeycapVariant::Filled => {
            let keycap_glow = color.unwrap_or(glow_c);
            // Opaque base (carrying the glow) so the chip never lets underlying content bleed
            // through — a keycap stamped over an icon/glyph (e.g. a drag handle or toolbar icon)
            // must stay legible, not show a ghost of what's beneath it. Over a dark surface this
            // matches the old translucent look; over content it hides it.
            cx.rect(
                cap,
                background,
                None,
                radius,
                Some(Glow {
                    color: keycap_glow,
                    radius: 6.0,
                    intensity: KEYCAP_GLOW,
                }),
            );
            // Tint on top of the opaque base, then the glyph in whichever of the theme's
            // background/foreground contrasts with the chip — see [`keycap_glyph_color`].
            cx.rect(cap, keycap_c.with_alpha(keycap_alpha), None, radius, None);
            let glyph = {
                let t = cx.theme();
                keycap_glyph_color(&t.colors, keycap_c, keycap_alpha)
            };
            content.paint(cx, cap, glyph, font);
        }
        KeycapVariant::Bordered => {
            // Outline-only chip: **no fill** (empty interior) + a full-strength **accent** border
            // (the `keycap_c` colour at full alpha, so it reads as the theme accent — not a washed
            // tint), no glow, accent glyph. Matches the showcase KeyHint (no background). For
            // keycaps on an already-dark owned surface (a menu panel).
            let border = cx.border(keycap_c);
            cx.rect(cap, Color::TRANSPARENT, border, radius, None);
            content.paint(cx, cap, keycap_c, font);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{keycap_glyph_color, keycap_size};

    /// **A letter stays readable whatever colour it is given**.
    ///
    /// `hint_color` is a colour the user picks, so the glyph cannot assume the chip is bright. The
    /// glyph was a constant — always the theme background — which is legible under a bright accent
    /// and invisible the moment the chip is near the background instead.
    #[test]
    fn a_keycap_letter_contrasts_with_the_chip_whatever_colour_it_is_given() {
        let theme = heca_theme::Theme::grid_tron();
        let alpha = theme.interaction.keycap;

        // The dangerous setting: the letters painted the same colour as the background behind them.
        let glyph = keycap_glyph_color(&theme, theme.background, alpha);
        assert_ne!(
            glyph, theme.background,
            "a chip the colour of the background must not carry a background-coloured letter",
        );
        assert_eq!(
            glyph, theme.foreground,
            "it takes the contrasting tone instead"
        );

        // …and the default is unchanged: a bright accent chip still carries the dark glyph.
        assert_eq!(
            keycap_glyph_color(&theme, theme.accent, alpha),
            theme.background,
            "the accent letters must look exactly as they always did",
        );
    }

    /// **The rule is about the chip, not about the theme being dark.** A light theme whose accent is
    /// also light gives a chip that a background-coloured letter disappears into — the same failure
    /// a badly chosen `hint_color` produces, reached without touching the setting.
    #[test]
    fn a_pale_chip_on_a_pale_theme_switches_the_letter_too() {
        let mut light = heca_theme::Theme::grid_tron();
        // A light palette is the dark one's two tones swapped — enough to state the rule without
        // depending on which bundled theme happens to be light.
        std::mem::swap(&mut light.background, &mut light.foreground);
        let alpha = light.interaction.keycap;

        // A mid-toned accent is far enough from the pale background: the letter is unchanged.
        assert_eq!(
            keycap_glyph_color(&light, light.accent, alpha),
            light.background,
            "a chip that already stands off the background keeps the design's own letter",
        );

        // A pale accent is not, so the letter takes the contrasting tone rather than vanishing.
        let pale = light.background.lerp(light.accent, 0.1);
        assert_eq!(
            keycap_glyph_color(&light, pale, alpha),
            light.foreground,
            "a chip the eye cannot separate from the background gets the other tone",
        );
    }

    #[test]
    fn keycap_size_is_positive_and_grows_with_text() {
        let one = keycap_size(13.0, "a");
        assert!(one.w > 0.0 && one.h > 0.0);
        // A wider label needs a wider chip; height is text-length independent.
        let many = keycap_size(13.0, "abc");
        assert!(many.w > one.w);
        assert_eq!(many.h, one.h);
    }

    #[test]
    fn keycap_size_scales_with_font() {
        let small = keycap_size(10.0, "a");
        let large = keycap_size(20.0, "a");
        assert!(large.w > small.w);
        assert!(large.h > small.h);
    }
}
