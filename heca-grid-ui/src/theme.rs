//! Theme tokens for the Grid look.
//!
//! A [`Theme`] is the palette + effect configuration a component tree reads from.
//! It **composes** [`heca_theme::Theme`] (the single source of truth for colors
//! and effect tokens) and adds the grid-ui-local font + focus-border-width tokens
//! that are system-local and don't belong in the portable theme crate.

use crate::color::Color;
use crate::font::DEFAULT_MONO_FAMILY;

pub use heca_theme::{FrameStyle, GlowLevel, Intensity};

/// **The border styles, nameable from a description.**
///
/// Written by hand rather than derived because [`FrameStyle`] lives in `heca-theme`, which owns the
/// vocabulary precisely so it can be read at paint time without depending on this crate — and a
/// derive here would invert that. The orphan rule allows it: [`PropName`](crate::prop::PropName) is
/// ours.
///
/// It was three enums for one thing until this: the theme's `FrameStyle`, the config's
/// `BorderStyle`, and a `PaneFrame` that existed only so `Pane` could name the same three choices.
/// The `Pane` copy is gone; the config one stays because it is the user-facing spelling and already
/// converts into this.
///
/// ⚠️ A hand-written variant list can fall behind the enum it describes, so
/// `every_border_style_can_be_named` walks it and fails when one is missing.
impl crate::prop::PropName for FrameStyle {
    const VARIANT_NAMES: &'static [&'static str] = &["none", "bordered", "bracketed"];

    fn from_prop_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(FrameStyle::None),
            "bordered" => Some(FrameStyle::Bordered),
            "bracketed" => Some(FrameStyle::Bracketed),
            _ => None,
        }
    }
}

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
    /// **The picker's letter size, in logical px — one size for every letter in the app.**
    ///
    /// Not derived from the widget the letter sits on. A size variant scales what a widget draws as
    /// its *own content*; a letter is chrome the framework stamps over it and belongs to the picker,
    /// which is app-wide. Read from the target instead, an emphasized header button wore a letter a
    /// quarter larger than the pane's own, in the same picker (Antonio, driving, 2026-09-03).
    ///
    /// Configurable as `[appearance] hint_font_size`.
    pub hint_font_size: f32,
    /// **The picker's letter colour — one colour for every letter in the app.**
    ///
    /// Deliberately its own token rather than the theme accent: a pane is drawn with its own theme,
    /// whose accent differs between the active pane and the rest, so letters taken from the ambient
    /// accent came out in two colours at once in a single picker. This one is not replaced when a
    /// pane's theme is derived, so every letter matches.
    pub hint_color: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            colors: heca_theme::Theme::default(),
            font_family: DEFAULT_MONO_FAMILY.to_string(),
            font_size: 15.0,
            focus_border_width: 1.5,
            hint_font_size: 12.0,
            hint_color: heca_theme::Theme::default().accent,
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
#[cfg(test)]
mod border_style_vocabulary {
    use super::*;
    use crate::prop::PropName;

    /// **Every border style can be named, and the names round-trip.**
    ///
    /// [`FrameStyle`]'s `PropName` impl is hand-written — the enum lives in `heca-theme`, which
    /// must not depend on this crate — so nothing makes the variant list follow the enum. Add a
    /// fourth style and this fails, which is the only thing standing between a new style and a
    /// description that silently cannot name it.
    #[test]
    fn every_border_style_can_be_named() {
        // Exhaustive on purpose: a new variant makes this match fail to compile, which is the
        // earliest possible warning, and the assertions below catch the rest.
        let all = [
            FrameStyle::None,
            FrameStyle::Bordered,
            FrameStyle::Bracketed,
        ];
        for style in all {
            match style {
                FrameStyle::None | FrameStyle::Bordered | FrameStyle::Bracketed => {}
            }
        }
        assert_eq!(
            FrameStyle::VARIANT_NAMES.len(),
            all.len(),
            "a style with no name cannot be set from a description",
        );
        for name in FrameStyle::VARIANT_NAMES {
            let parsed = FrameStyle::from_prop_name(name)
                .unwrap_or_else(|| panic!("`{name}` is offered but does not resolve"));
            assert!(
                all.contains(&parsed),
                "`{name}` resolved to something not in the enum"
            );
        }
    }

    /// An unknown name is refused rather than guessed at.
    #[test]
    fn a_border_style_nobody_recognises_is_not_invented() {
        assert!(FrameStyle::from_prop_name("dashed").is_none());
        assert!(
            FrameStyle::from_prop_name("Bordered").is_none(),
            "names are snake_case"
        );
    }
}
