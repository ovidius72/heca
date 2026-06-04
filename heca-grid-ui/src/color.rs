//! Design-system color primitive.
//!
//! `heca-grid-ui` owns its own [`Color`] rather than reusing `heca-config::Color`
//! so the design system stays decoupled and can carry richer helpers
//! (`lerp`, `with_alpha`). Conversions to/from the app's config color are added
//! at the integration boundary (Phase D).

use std::str::FromStr;

/// An 8-bit-per-channel RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Opaque or translucent color from explicit channels.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Fully opaque color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Transparent black.
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);

    /// Convert to linear-ish `[f32; 4]` in `0.0..=1.0` for the GPU boundary.
    pub fn to_f32x4(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    /// Return a copy with the alpha channel replaced.
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Linear interpolation between two colors. `t` is clamped to `0.0..=1.0`.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
            mix(self.a, other.a),
        )
    }
}

impl FromStr for Color {
    type Err = String;

    /// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let hex = s
            .strip_prefix('#')
            .ok_or_else(|| format!("color must start with '#': got {s}"))?;
        let parse = |slice: &str| -> Result<u8, String> {
            u8::from_str_radix(slice, 16).map_err(|e| format!("invalid hex in {s}: {e}"))
        };
        match hex.len() {
            3 => {
                let dup = |c: &str| parse(&c.repeat(2));
                Ok(Self::rgb(
                    dup(&hex[0..1])?,
                    dup(&hex[1..2])?,
                    dup(&hex[2..3])?,
                ))
            }
            6 => Ok(Self::rgb(
                parse(&hex[0..2])?,
                parse(&hex[2..4])?,
                parse(&hex[4..6])?,
            )),
            8 => Ok(Self::new(
                parse(&hex[0..2])?,
                parse(&hex[2..4])?,
                parse(&hex[4..6])?,
                parse(&hex[6..8])?,
            )),
            n => Err(format!("expected 3, 6, or 8 hex digits, got {n} in {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_six_digit_hex() {
        assert_eq!("#1e1e2e".parse::<Color>().unwrap(), Color::rgb(30, 30, 46));
    }

    #[test]
    fn parses_short_and_alpha_hex() {
        assert_eq!("#fff".parse::<Color>().unwrap(), Color::rgb(255, 255, 255));
        assert_eq!(
            "#89b4fad9".parse::<Color>().unwrap(),
            Color::new(137, 180, 250, 217)
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!("1e1e2e".parse::<Color>().is_err());
        assert!("#zz".parse::<Color>().is_err());
    }

    #[test]
    fn lerp_midpoint() {
        let mid = Color::rgb(0, 0, 0).lerp(Color::rgb(255, 255, 255), 0.5);
        assert_eq!(mid, Color::rgb(128, 128, 128));
    }
}
