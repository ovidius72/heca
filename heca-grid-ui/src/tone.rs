//! **A colour by what it means**, left for the theme to decide.

use crate::color::Color;
use serde::{Deserialize, Serialize};

/// **What a colour means**, so the theme can say which pixels that is.
///
/// A widget composing itself has no theme at build time — the theme arrives at paint and changes on
/// reload — so a literal colour is not something it can write. It names a meaning instead, and the
/// theme resolves it each frame.
///
/// Used wherever the library lets a caller colour something by meaning: a keycap
/// ([`hint_tone`](crate::builders::HintExt::hint_tone)) and the text of everything inside a
/// container ([`content_tone`](crate::builders::ToneExt::content_tone)).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, heca_grid_ui_macros::PropName,
)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// The default weight — a keycap for a place you would go.
    Accent,
    /// Secondary: a structural control rather than a destination, quieter text.
    Muted,
    /// A distinct class that should not read like the others.
    Warning,
    /// As `Warning`, a third class; also success.
    Success,
    /// Something destructive, or its consequence.
    Danger,
    /// **Primary text a notch below full foreground**: the muted token blended
    /// `SOFT_MIX` of the way toward the foreground. For the titles of a list whose selected row
    /// reads in full foreground — the rest are there to be scanned, not read.
    Soft,
    /// **Secondary text on a highlighted row**: the foreground at the overlay hint alpha
    /// (`interaction.menu_shortcut`). `Muted` is nearly the colour of a selection fill, so a
    /// selected row's second line says "same ink as the title, quieter" instead.
    Secondary,
}

/// How far toward the foreground [`Tone::Soft`] sits from the muted token (`0` = muted, `1` =
/// foreground).
const SOFT_MIX: f32 = 0.7;

impl Tone {
    /// The colour, from the theme this frame.
    pub fn resolve(self, theme: &crate::theme::Theme) -> Color {
        match self {
            Tone::Accent => theme.colors.accent,
            Tone::Muted => theme.colors.muted,
            Tone::Warning => theme.colors.warning,
            Tone::Success => theme.colors.success,
            Tone::Danger => theme.colors.danger,
            Tone::Soft => theme.colors.muted.lerp(theme.colors.foreground, SOFT_MIX),
            Tone::Secondary => theme
                .colors
                .foreground
                .with_alpha(theme.colors.interaction.menu_shortcut),
        }
    }
}
