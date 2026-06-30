//! Theme tokens for the Grid look.
//!
//! A [`Theme`] is the palette + effect configuration a component tree reads from.
//! It **composes** [`heca_theme::Theme`] (the single source of truth for colors
//! and effect tokens) and adds the grid-ui-local font + focus-border-width tokens
//! that are system-local and don't belong in the portable theme crate.

use crate::color::Color;
use crate::font::DEFAULT_MONO_FAMILY;

pub use heca_theme::{GlowLevel, Intensity};

/// Palette + effect tokens for a component tree.
///
/// Composes `heca_theme::Theme` (the shared color/effect payload, accessed via
/// the explicit [`colors`](Self::colors) field) with grid-ui-local tokens
/// (`font_family`, `font_size`, `focus_border_width`).
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Shared color + effect payload from `heca-theme`. Access colors as
    /// `theme.colors.background`, radius as `theme.colors.border_radius`, etc.
    pub colors: heca_theme::Theme,
    /// UI font family (system-local — not portable across themes).
    pub font_family: String,
    /// UI base font size in logical px.
    pub font_size: f32,
    /// Width in logical px of the **affordance** outlines — the keyboard focus
    /// ring ([`PaintCx::corner_brackets`](crate::PaintCx::corner_brackets)) and the
    /// selected-item highlight. Deliberately independent of
    /// [`border_width`](heca_theme::Theme::border_width) so focus and selection
    /// stay visible even with decorative borders turned off
    /// (`border_width == 0`).
    pub focus_border_width: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            colors: heca_theme::Theme::default(),
            font_family: DEFAULT_MONO_FAMILY.to_string(),
            font_size: 15.0,
            focus_border_width: 1.5,
        }
    }
}

impl Theme {
    /// Convenience: the drop-shadow umbra color for elevated/floating surfaces.
    /// Derived from the config-driven `colors.shadow` token (color + alpha),
    /// exposed as a [`Color`] for the renderer boundary.
    pub fn shadow_color(&self) -> Color {
        self.colors.shadow.color.with_alpha_f32(self.colors.shadow.alpha)
    }
}