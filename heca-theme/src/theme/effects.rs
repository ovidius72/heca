//! Visual-effect value types shared by the theme: scanline [`Intensity`], neon
//! [`GlowLevel`], drop [`Shadow`], and container [`FrameStyle`].

use crate::color::Color;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  Intensity
// ═══════════════════════════════════════════════════════════════════════════════

/// Scanline / CRT overlay intensity. This controls **scanline-overlay
/// opacity only** — it does **not** affect glow. Glow is owned by
/// [`GlowLevel`] (presence + radius + strength). Higher `Intensity` = a
/// stronger visible CRT grille; `Off` = no scanlines (closest to a standard
/// UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    /// No scanlines / CRT overlay.
    Off,
    Low,
    #[default]
    Medium,
    /// Strong CRT grille (heaviest scanlines). Does not add glow.
    Heavy,
}

impl Intensity {
    /// CRT scanline-overlay opacity for this level — the visible thing
    /// `intensity` controls. `Off` = no scanlines; higher = a stronger CRT
    /// grille. Glow is separate (see [`GlowLevel`]).
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

// ═══════════════════════════════════════════════════════════════════════════════
//  GlowLevel
// ═══════════════════════════════════════════════════════════════════════════════

/// Size of the neon glow halo — scales every glow's falloff radius;
/// `None` disables glow entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// This is the *radius* (halo size) dimension; glow *strength* (alpha) is
    /// [`strength_scale`](GlowLevel::strength_scale). The two are independent
    /// so e.g. `Large` = 2.0× radius but only 1.6× strength.
    pub fn radius_scale(self) -> f32 {
        match self {
            GlowLevel::None => 0.0,
            GlowLevel::Thin => 0.5,
            GlowLevel::Medium => 1.0,
            GlowLevel::Large => 2.0,
        }
    }

    /// Multiplier applied to a glow's base strength (alpha). `0.0` means "off".
    /// This is the *strength* (alpha) dimension; glow *radius* (halo size) is
    /// [`radius_scale`](GlowLevel::radius_scale). `GlowLevel` is the sole owner
    /// of glow — both radius and strength — so `intensity` no longer feeds glow.
    /// Curve: `none=0.0, thin=0.5, medium=1.0, large=1.6` (preserves the former
    /// `Intensity::glow_scale()` values exactly).
    pub fn strength_scale(self) -> f32 {
        match self {
            GlowLevel::None => 0.0,
            // 0.75 (was 0.5): with the faint rest glows (`control_rest_glow` ≈ 0.12
            // base) a 0.5× strength on a 0.5× radius was nearly invisible — Thin
            // read the same as None (user-reported). Thin = tight halo (the 0.5×
            // radius carries the "thin"), still clearly present.
            GlowLevel::Thin => 0.75,
            GlowLevel::Medium => 1.0,
            GlowLevel::Large => 1.6,
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

// ═══════════════════════════════════════════════════════════════════════════════
//  Shadow
// ═══════════════════════════════════════════════════════════════════════════════

/// Drop-shadow configuration for elevated/floating surfaces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub color: Color,
    pub alpha: f32,
    pub blur: f32,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: Color::rgb(0, 0, 0),
            alpha: 0.3,
            blur: 8.0,
        }
    }
}

/// Frame decoration style for a container surface. The theme-level vocabulary
/// behind the app's `[appearance] *_border_style` settings and the grid-ui
/// `PaneFrame`; serialised `snake_case` (`"none" | "bordered" | "bracketed"`).
///
/// It lives here, in the theme crate, because `heca-grid-ui` reads it at paint
/// time (via `Theme`) and cannot depend on the app's config crate — the same path
/// `show_focus_border` takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameStyle {
    /// No frame — fill (and any per-widget border/glow) only.
    None,
    /// A clean continuous border in the surface's border color.
    Bordered,
    /// The accent corner-bracket reticle (bright rounded corners + dimmed line).
    #[default]
    Bracketed,
}
