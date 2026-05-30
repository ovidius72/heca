use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// A color value parsed from hex strings like `#RRGGBB` or `#RRGGBBAA`.
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

    /// Convert to [f32; 4] for GPU usage (RGBA, 0.0–1.0).
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
        let len = hex.len();
        if len != 6 && len != 8 {
            return Err(format!(
                "Color hex must be 6 or 8 chars: got {} ({})",
                len, s
            ));
        }
        let r = u8::from_str_radix(&hex[0..2], 16)
            .map_err(|e| format!("Invalid red component: {}", e))?;
        let g = u8::from_str_radix(&hex[2..4], 16)
            .map_err(|e| format!("Invalid green component: {}", e))?;
        let b = u8::from_str_radix(&hex[4..6], 16)
            .map_err(|e| format!("Invalid blue component: {}", e))?;
        let a = if len >= 8 {
            u8::from_str_radix(&hex[6..8], 16)
                .map_err(|e| format!("Invalid alpha component: {}", e))?
        } else {
            255
        };
        Ok(Color { r, g, b, a })
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
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

/// Shadow styling for chrome elements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub color: String,
    pub alpha: f32,
    pub blur: f32,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: "#000000".to_string(),
            alpha: 0.3,
            blur: 8.0,
        }
    }
}

/// A visual theme for the compositor chrome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub accent: Color,
    pub font_family: String,
    pub font_size: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub shadow: Shadow,
    /// Background color for floating panes.
    #[serde(default = "default_float_bg")]
    pub float_background: Color,
    /// Accent color for floating panes (border / title bar).
    #[serde(default = "default_float_accent")]
    pub float_accent: Color,
    /// Focused float border color.
    #[serde(default = "default_float_focus")]
    pub float_focus: Color,
}

fn default_float_bg() -> Color {
    Color::new(49, 50, 68, 255) // same as border for subtle distinction
}

fn default_float_accent() -> Color {
    Color::new(137, 180, 250, 255) // same as accent
}

fn default_float_focus() -> Color {
    Color::new(250, 179, 135, 255) // peach — warm highlight
}

impl Default for Theme {
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
}

impl Theme {
    /// Catppuccin Mocha — the default dark theme.
    pub fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".to_string(),
            background: Color::new(30, 30, 46, 255),    // #1e1e2e
            foreground: Color::new(205, 214, 244, 255), // #cdd6f4
            border: Color::new(49, 50, 68, 255),        // #313244
            accent: Color::new(137, 180, 250, 255),     // #89b4fa
            font_family: "JetBrainsMono Nerd Font".to_string(),
            font_size: 32.0,
            border_radius: 6.0,
            border_width: 1.0,
            shadow: Shadow::default(),
            float_background: Color::new(49, 50, 68, 255),
            float_accent: Color::new(137, 180, 250, 255),
            float_focus: Color::new(250, 179, 135, 255),
        }
    }

    /// Catppuccin Latte — the bundled light theme.
    pub fn catppuccin_latte() -> Self {
        Self {
            name: "Catppuccin Latte".to_string(),
            background: Color::new(239, 241, 245, 255), // #eff1f5
            foreground: Color::new(76, 79, 105, 255),   // #4c4f69
            border: Color::new(204, 208, 218, 255),     // #ccd0da
            accent: Color::new(30, 102, 245, 255),      // #1e66f5
            font_family: "JetBrainsMono Nerd Font".to_string(),
            font_size: 32.0,
            border_radius: 6.0,
            border_width: 1.0,
            shadow: Shadow {
                color: "#000000".to_string(),
                alpha: 0.15,
                blur: 8.0,
            },
            float_background: Color::new(204, 208, 218, 255),
            float_accent: Color::new(30, 102, 245, 255),
            float_focus: Color::new(230, 126, 34, 255),
        }
    }

    /// Load a theme by name from the filesystem or bundled defaults.
    pub fn load(name: &str) -> Self {
        // Try filesystem first
        if let Some(theme) = Self::load_from_disk(name) {
            return theme;
        }
        // Fall back to bundled themes
        Self::load_bundled(name).unwrap_or_default()
    }

    fn load_from_disk(name: &str) -> Option<Self> {
        let path = config_dir().join("themes").join(format!("{}.toml", name));
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }

    fn load_bundled(name: &str) -> Option<Self> {
        let bundled: HashMap<&str, &str> = [
            ("mocha", include_str!("themes/mocha.toml")),
            ("latte", include_str!("themes/latte.toml")),
        ]
        .into_iter()
        .collect();
        let content = bundled.get(name)?;
        toml::from_str(content).ok()
    }
}

/// General application configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub window_width: u32,
    pub window_height: u32,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            window_width: 1280,
            window_height: 800,
        }
    }
}

/// Keybinding map: action name -> key string(s), comma-separated (e.g. "focus_left" -> "h,ArrowLeft").
pub type KeybindingMap = HashMap<String, String>;

/// Root configuration struct loaded from `config.toml`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub theme: String,
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub keybindings: KeybindingMap,
}

impl Default for Config {
    fn default() -> Self {
        let mut keybindings = HashMap::new();
        keybindings.insert("focus_left".to_string(), "h,ArrowLeft".to_string());
        keybindings.insert("focus_right".to_string(), "l,ArrowRight".to_string());
        keybindings.insert("focus_up".to_string(), "k,ArrowUp".to_string());
        keybindings.insert("focus_down".to_string(), "j,ArrowDown".to_string());
        keybindings.insert("split_horizontal".to_string(), "-".to_string());
        keybindings.insert("split_vertical".to_string(), "v".to_string());
        keybindings.insert("float".to_string(), "f".to_string());
        keybindings.insert("scratchpad".to_string(), "s".to_string());
        keybindings.insert("hide".to_string(), "z".to_string());
        keybindings.insert("close".to_string(), "x".to_string());
        keybindings.insert("tab_next".to_string(), "]".to_string());
        keybindings.insert("tab_prev".to_string(), "[".to_string());
        keybindings.insert("next_pane".to_string(), "n".to_string());
        keybindings.insert("prev_pane".to_string(), "p".to_string());
        keybindings.insert("pane_select".to_string(), "q".to_string());
        keybindings.insert("resize_left".to_string(), "H".to_string());
        keybindings.insert("resize_right".to_string(), "L".to_string());
        keybindings.insert("resize_up".to_string(), "K".to_string());
        keybindings.insert("resize_down".to_string(), "J".to_string());
        keybindings.insert("sidebar_left".to_string(), "Space".to_string());
        keybindings.insert("swap_left".to_string(), "Ctrl+h".to_string());
        keybindings.insert("swap_right".to_string(), "Ctrl+l".to_string());
        keybindings.insert("swap_up".to_string(), "Ctrl+k".to_string());
        keybindings.insert("swap_down".to_string(), "Ctrl+j".to_string());
        Self {
            theme: "mocha".to_string(),
            general: GeneralConfig::default(),
            keybindings,
        }
    }
}

/// Combined config + theme, ready for use by the app.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub config: Config,
    pub theme: Theme,
}

impl AppConfig {
    /// Load config from the user's config directory.
    /// Falls back to defaults if files are missing.
    pub fn load() -> Self {
        let config = Self::load_config_file().unwrap_or_default();
        let theme = Theme::load(&config.theme);
        Self { config, theme }
    }

    fn load_config_file() -> Option<Config> {
        let path = config_dir().join("config.toml");
        let content = std::fs::read_to_string(path).ok()?;
        toml::from_str(&content).ok()
    }
}

/// Return the heca config directory (`~/.config/heca/` on Unix).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("heca")
}

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
    fn test_default_theme_is_mocha() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Catppuccin Mocha");
        assert_eq!(theme.background, Color::new(30, 30, 46, 255));
        assert_eq!(theme.foreground, Color::new(205, 214, 244, 255));
    }

    #[test]
    fn test_bundled_mocha_theme() {
        let theme = Theme::load("mocha");
        assert_eq!(theme.name, "Catppuccin Mocha");
    }

    #[test]
    fn test_bundled_latte_theme() {
        let theme = Theme::load("latte");
        assert_eq!(theme.name, "Catppuccin Latte");
    }

    #[test]
    fn test_fallback_when_config_missing() {
        // AppConfig::load() should never panic, even with no files on disk
        let app_config = AppConfig::load();
        assert_eq!(app_config.config.theme, "mocha");
        assert_eq!(app_config.theme.name, "Catppuccin Mocha");
    }
}
