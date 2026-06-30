//! Theme tokens for the Grid look.
//!
//! A [`Theme`] is the palette + effect configuration a component tree reads from.
//! We pick our own colors (no `oklch`/web baggage). The default is a dark,
//! cyan-accented Tron skin ([`Theme::grid_tron`]).

use crate::color::Color;
use crate::font::DEFAULT_MONO_FAMILY;

pub use heca_theme::{GlowLevel, Intensity};

/// Palette + effect tokens for a component tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub surface: Color,
    pub foreground: Color,
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    pub glow: Color,
    /// Drop-shadow umbra color (including its alpha) for elevated/floating
    /// surfaces (e.g. [`Modal`](crate::widgets::Modal)). Independent of `glow` and
    /// `border_width` — a shadow shows even when both are off. Set alpha 0 to
    /// disable shadows globally.
    pub shadow: Color,
    pub danger: Color,
    pub success: Color,
    pub warning: Color,
    pub font_family: String,
    pub font_size: f32,
    /// Base corner radius in logical px. Widgets read this (possibly scaled) rather
    /// than hardcoding their own.
    pub radius: f32,
    /// Base border width in logical px. Used by container/pane borders.
    pub border_width: f32,
    /// Width in logical px of the **affordance** outlines — the keyboard focus
    /// ring ([`PaintCx::corner_brackets`](crate::PaintCx::corner_brackets)) and the
    /// selected-item highlight. Deliberately independent of [`border_width`](Self::border_width)
    /// so focus and selection stay visible even with decorative borders turned off
    /// (`border_width == 0`). Config maps `focus_border_width` here; the default
    /// keeps it visible.
    pub focus_border_width: f32,
    /// Glow halo size — scales every glow's falloff radius (`None` = no glow).
    pub glow_size: GlowLevel,
    pub intensity: Intensity,
    /// Show the keyboard focus ring (focus-visible indicator).
    pub show_focus_border: bool,
    /// Opacity (`0.0..=1.0`) of a duotone [`Icon`](crate::widgets::Icon)'s
    /// secondary layer when its color isn't set explicitly — theme/config-driven,
    /// not baked into the widget. Phosphor's web reference is `0.2` (tuned for
    /// light backgrounds); on this dark theme the default is higher so the
    /// two-tone reads.
    pub icon_secondary_alpha: f32,
    /// Opacity (`0.0..=1.0`) of the **active-region wash** — the faint accent
    /// overlay a [`DockFrame`](crate::widgets::DockFrame) paints over itself when
    /// marked active (e.g. the active workspace in the sidebar). Theme-driven, not
    /// baked into the widget.
    pub active_wash_alpha: f32,
    /// Opacity (`0.0..=1.0`) of a sidebar/list card's resting background tint
    /// (e.g. each pane card). Kept very low so the card reads as a subtle raised
    /// surface rather than a filled block.
    pub card_background_alpha: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::grid_tron()
    }
}

/// Small controls (inputs, selects, checkboxes, chips) round at this fraction of
/// the base [`Theme::radius`], so one global radius scales every widget together.
const CONTROL_RADIUS_FRAC: f32 = 0.5;

impl Theme {
    /// Corner radius for small controls — a fraction of the base container
    /// [`radius`](Theme::radius). Driving everything off `radius` means the global
    /// radius setting scales all widgets proportionally (like `glow_size` does for
    /// glow), instead of each widget hardcoding its own corners.
    pub fn control_radius(&self) -> f32 {
        self.radius * CONTROL_RADIUS_FRAC
    }

    /// The default dark, cyan-accented Tron theme.
    pub fn grid_tron() -> Self {
        Self {
            name: "Grid Tron".to_string(),
            background: Color::rgb(6, 10, 14),
            surface: Color::rgb(12, 18, 24),
            foreground: Color::rgb(198, 240, 255),
            muted: Color::rgb(96, 130, 146),
            border: Color::rgb(20, 60, 76),
            accent: Color::rgb(64, 224, 255),
            glow: Color::rgb(64, 224, 255),
            shadow: Color::new(0, 0, 0, 130),
            danger: Color::rgb(255, 70, 84),
            success: Color::rgb(80, 255, 170),
            warning: Color::rgb(255, 190, 70),
            font_family: DEFAULT_MONO_FAMILY.to_string(),
            font_size: 15.0,
            radius: 8.0,
            border_width: 1.0,
            focus_border_width: 1.5,
            glow_size: GlowLevel::Medium,
            intensity: Intensity::Medium,
            show_focus_border: true,
            icon_secondary_alpha: 0.45,
            active_wash_alpha: 0.11,
            card_background_alpha: 0.02,
        }
    }

    /// The "Ares" red Tron variant.
    pub fn grid_ares() -> Self {
        Self {
            name: "Grid Ares".to_string(),
            background: Color::rgb(14, 6, 7),
            surface: Color::rgb(24, 10, 12),
            foreground: Color::rgb(255, 214, 214),
            muted: Color::rgb(150, 96, 100),
            border: Color::rgb(82, 22, 28),
            accent: Color::rgb(255, 56, 72),
            glow: Color::rgb(255, 56, 72),
            shadow: Color::new(0, 0, 0, 130),
            danger: Color::rgb(255, 70, 84),
            success: Color::rgb(80, 255, 170),
            warning: Color::rgb(255, 190, 70),
            font_family: DEFAULT_MONO_FAMILY.to_string(),
            font_size: 15.0,
            radius: 6.0,
            border_width: 1.0,
            focus_border_width: 1.5,
            glow_size: GlowLevel::Medium,
            intensity: Intensity::Medium,
            show_focus_border: true,
            icon_secondary_alpha: 0.45,
            active_wash_alpha: 0.11,
            card_background_alpha: 0.02,
        }
    }
}
