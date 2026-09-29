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
/// ([`hint_tone`](crate::builders::LayoutExt::hint_tone)) and the text of everything inside a
/// container ([`content_tone`](crate::builders::LayoutExt::content_tone)).
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
}

impl Tone {
    /// The colour, from the theme this frame.
    pub fn resolve(self, theme: &crate::theme::Theme) -> Color {
        match self {
            Tone::Accent => theme.colors.accent,
            Tone::Muted => theme.colors.muted,
            Tone::Warning => theme.colors.warning,
            Tone::Success => theme.colors.success,
            Tone::Danger => theme.colors.danger,
        }
    }
}
