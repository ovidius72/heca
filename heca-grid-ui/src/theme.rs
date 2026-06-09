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

    /// CRT scanline-overlay opacity for this level — the visible thing `intensity`
    /// controls. `Off` = no scanlines; higher = a stronger CRT grille.
    pub fn scanline_opacity(self) -> f32 {
        match self {
            Intensity::Off => 0.0,
            Intensity::Low => 0.05,
            Intensity::Medium => 0.11,
            Intensity::Heavy => 0.20,
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

/// Size of the neon glow halo — a configurable token (e.g. from `config.toml`).
/// Scales every glow's falloff radius; `None` disables glow entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlowLevel {
    /// No glow at all.
    None,
    /// A tight halo.
    Thin,
    #[default]
    Medium,
    /// A wide, soft halo.
    Large,
}

impl GlowLevel {
    /// Multiplier applied to a glow's base falloff radius. `0.0` means "off".
    pub fn radius_scale(self) -> f32 {
        match self {
            GlowLevel::None => 0.0,
            GlowLevel::Thin => 0.5,
            GlowLevel::Medium => 1.0,
            GlowLevel::Large => 2.0,
        }
    }

    /// Parse a `config.toml` value (case-insensitive). Unknown → `Medium`.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => GlowLevel::None,
            "thin" => GlowLevel::Thin,
            "large" => GlowLevel::Large,
            _ => GlowLevel::Medium,
        }
    }

    /// Stable ordering used by selectors: None, Thin, Medium, Large.
    pub const ALL: [GlowLevel; 4] = [
        GlowLevel::None,
        GlowLevel::Thin,
        GlowLevel::Medium,
        GlowLevel::Large,
    ];

    /// Uppercase label for UI (matches `ALL` order).
    pub fn label(self) -> &'static str {
        match self {
            GlowLevel::None => "NONE",
            GlowLevel::Thin => "THIN",
            GlowLevel::Medium => "MEDIUM",
            GlowLevel::Large => "LARGE",
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
    /// Base corner radius in logical px. Widgets read this (possibly scaled) rather
    /// than hardcoding their own.
    pub radius: f32,
    /// Base border width in logical px. Used by container/pane borders.
    pub border_width: f32,
    /// Glow halo size — scales every glow's falloff radius (`None` = no glow).
    pub glow_size: GlowLevel,
    pub intensity: Intensity,
    /// Show the keyboard focus ring (focus-visible indicator).
    pub show_focus_border: bool,
    /// Opacity (`0.0..=1.0`) of a duotone [`Icon`](crate::widgets::Icon)'s
    /// secondary layer when its color isn't set explicitly — theme/config-driven,
    /// not baked into the widget. Phosphor's reference value is `0.2`.
    pub icon_secondary_alpha: f32,
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
            danger: Color::rgb(255, 70, 84),
            success: Color::rgb(80, 255, 170),
            warning: Color::rgb(255, 190, 70),
            font_family: DEFAULT_MONO_FAMILY.to_string(),
            font_size: 15.0,
            radius: 8.0,
            border_width: 1.0,
            glow_size: GlowLevel::Medium,
            intensity: Intensity::Medium,
            show_focus_border: true,
            icon_secondary_alpha: 0.2,
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
            radius: 6.0,
            border_width: 1.0,
            glow_size: GlowLevel::Medium,
            intensity: Intensity::Medium,
            show_focus_border: true,
            icon_secondary_alpha: 0.2,
        }
    }
}
