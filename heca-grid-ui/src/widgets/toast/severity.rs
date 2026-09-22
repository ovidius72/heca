//! [`ToastSeverity`] — what a notification *is*, and the theme colour that says so.
//!
//! It is a vocabulary, not a palette: the four members name a meaning, and each resolves to a
//! theme token at paint time. Nothing here holds a colour, so a theme reload re-tones every card
//! that is already on screen.

use crate::color::Color;
use crate::theme::Theme;
use crate::widgets::Glyph;

/// Severity of a [`Toast`](super::Toast), mapped to theme tokens at paint time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum ToastSeverity {
    /// Informational (accent).
    #[default]
    Info,
    /// Success (positive).
    Success,
    /// Warning.
    Warning,
    /// Danger (error).
    Danger,
}

#[heca_grid_ui_macros::props]
impl ToastSeverity {
    /// The default leading glyph for this severity (overridable via [`Toast::icon`](super::Toast::icon)).
    #[heca_grid_ui_macros::host_only(
        "carries no value — a property needs one; the equivalent is an explicit setting"
    )]
    pub(crate) fn default_glyph(self) -> Glyph {
        match self {
            ToastSeverity::Info => Glyph::Info,
            ToastSeverity::Success => Glyph::Check,
            ToastSeverity::Warning => Glyph::Warning,
            ToastSeverity::Danger => Glyph::WarningCircle,
        }
    }

    /// This severity's hue, read from the theme it is being drawn against.
    ///
    /// Asked at paint, never stored: a card built under one theme and still on screen after a
    /// reload must say what the *current* theme says, and a colour captured at build time cannot.
    #[heca_grid_ui_macros::host_only(
        "carries no value — a property needs one; the equivalent is an explicit setting"
    )]
    pub(crate) fn tone(self, theme: &Theme) -> Color {
        match self {
            ToastSeverity::Info => theme.colors.accent,
            ToastSeverity::Success => theme.colors.success,
            ToastSeverity::Warning => theme.colors.warning,
            ToastSeverity::Danger => theme.colors.danger,
        }
    }
}
