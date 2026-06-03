//! Theme tokens for the Grid look.
//!
//! A [`Theme`] is the palette + effect configuration a component tree reads from.
//! We pick our own colors (no `oklch`/web baggage). The default is a dark,
//! cyan-accented Tron skin ([`Theme::grid_tron`]).

use crate::color::Color;
use crate::font::DEFAULT_MONO_FAMILY;

/// How strongly Tron effects (glow, scanlines) are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Intensity {
    /// Plain — no glow or scanlines (closest to a standard UI).
    Off,
    Low,
    #[default]
    Medium,
    /// Full Tron: strong glow + visible scanlines.
    Heavy,
}

impl Intensity {
    /// Glow strength multiplier for this level.
    pub fn glow_scale(self) -> f32 {
        match self {
            Intensity::Off => 0.0,
            Intensity::Low => 0.5,
            Intensity::Medium => 1.0,
            Intensity::Heavy => 1.6,
        }
    }

    /// Scanline opacity for this level.
    pub fn scanline_opacity(self) -> f32 {
        match self {
            Intensity::Off | Intensity::Low => 0.0,
            Intensity::Medium => 0.04,
            Intensity::Heavy => 0.10,
        }
    }

    /// Cycle to the next level (Off → Low → Medium → Heavy → Off).
    pub fn next(self) -> Self {
        match self {
            Intensity::Off => Intensity::Low,
            Intensity::Low => Intensity::Medium,
            Intensity::Medium => Intensity::Heavy,
            Intensity::Heavy => Intensity::Off,
        }
    }
}

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
    pub danger: Color,
    pub success: Color,
    pub warning: Color,
    pub font_family: String,
    pub font_size: f32,
    pub radius: f32,
    pub intensity: Intensity,
}

impl Default for Theme {
    fn default() -> Self {
        Self::grid_tron()
    }
}

impl Theme {
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
            danger: Color::rgb(255, 70, 84),
            success: Color::rgb(80, 255, 170),
            warning: Color::rgb(255, 190, 70),
            font_family: DEFAULT_MONO_FAMILY.to_string(),
            font_size: 15.0,
            radius: 2.0,
            intensity: Intensity::Medium,
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
            danger: Color::rgb(255, 70, 84),
            success: Color::rgb(80, 255, 170),
            warning: Color::rgb(255, 190, 70),
            font_family: DEFAULT_MONO_FAMILY.to_string(),
            font_size: 15.0,
            radius: 2.0,
            intensity: Intensity::Medium,
        }
    }
}
