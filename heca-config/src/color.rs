use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// ═══════════════════════════════════════════════════════════════════════════════
//  Color
// ═══════════════════════════════════════════════════════════════════════════════

/// An RGBA colour stored as four `u8` channels.
///
/// Parsed from hex strings (`#rrggbb` or `#rrggbbaa`) and serialised back as
/// `#rrggbbaa`.  The `serde` representation is a plain `String`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_f32x4(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
}

impl FromStr for Color {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if !s.starts_with('#') {
            return Err(format!("Color must start with #: got {}", s));
        }
        let hex = &s[1..];
        if hex.len() != 6 && hex.len() != 8 {
            return Err(format!(
                "Color hex must be 6 or 8 chars: got {}",
                hex.len()
            ));
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| format!("Invalid red: {}", e))?;
        let g =
            u8::from_str_radix(&hex[2..4], 16).map_err(|e| format!("Invalid green: {}", e))?;
        let b =
            u8::from_str_radix(&hex[4..6], 16).map_err(|e| format!("Invalid blue: {}", e))?;
        let a = if hex.len() >= 8 {
            u8::from_str_radix(&hex[6..8], 16).map_err(|e| format!("Invalid alpha: {}", e))?
        } else {
            255
        };
        Ok(Color { r, g, b, a })
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:02x}{:02x}{:02x}{:02x}",
            self.r, self.g, self.b, self.a
        )
    }
}

impl From<Color> for String {
    fn from(c: Color) -> Self {
        c.to_string()
    }
}

impl TryFrom<String> for Color {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Color::from_str(&s)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex() {
        let c: Color = "#1e1e2e".parse().unwrap();
        assert_eq!(c.r, 30);
        assert_eq!(c.g, 30);
        assert_eq!(c.b, 46);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn test_color_with_alpha() {
        let c: Color = "#ff000080".parse().unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 0);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 128);
    }

    #[test]
    fn test_color_roundtrip() {
        let c1 = Color::new(30, 30, 46, 255);
        let s: String = c1.into();
        let c2 = Color::from_str(&s).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_color_error_no_hash() {
        assert!(Color::from_str("1e1e2e").is_err());
    }

    #[test]
    fn test_color_error_bad_length() {
        assert!(Color::from_str("#1e1e").is_err());
        assert!(Color::from_str("#1e1e2e3a4b").is_err());
    }

    #[test]
    fn test_color_error_invalid_hex() {
        assert!(Color::from_str("#zzzzzz").is_err());
    }
}
