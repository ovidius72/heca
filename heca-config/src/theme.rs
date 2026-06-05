pub use crate::color::Color;
pub use crate::keys::{
    BindingValue, CommandKeybindConfig, KeybindingMap, KeyModeConfig, KeysConfig,
    ModeBindingConfig,
};
pub use crate::loader::{config_dir, AppConfig, Config};
pub use crate::settings::{ModifierKey, SettingsConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
//  Shadow & Theme
// ═══════════════════════════════════════════════════════════════════════════════

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
    #[serde(default = "crate::defaults::default_float_bg")]
    pub float_background: Color,
    #[serde(default = "crate::defaults::default_float_accent")]
    pub float_accent: Color,
    #[serde(default = "crate::defaults::default_float_focus")]
    pub float_focus: Color,
    // ── Sidebar drag-and-drop colors ──
    #[serde(default = "crate::defaults::default_drag_ghost_bg")]
    pub sidebar_drag_ghost_bg: Color,
    #[serde(default = "crate::defaults::default_drag_ghost_fg")]
    pub sidebar_drag_ghost_fg: Color,
    #[serde(default = "crate::defaults::default_drag_source_bg")]
    pub sidebar_drag_source_bg: Color,
    #[serde(default = "crate::defaults::default_drag_source_border")]
    pub sidebar_drag_source_border: Color,
    // ── Sidebar font sizes ──
    #[serde(default = "crate::defaults::default_sidebar_label_font_size")]
    pub sidebar_label_font_size: f32,
    #[serde(default = "crate::defaults::default_sidebar_button_font_size")]
    pub sidebar_button_font_size: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
}

impl Theme {
    pub fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".to_string(),
            background: Color::new(30, 30, 46, 255),
            foreground: Color::new(205, 214, 244, 255),
            border: Color::new(49, 50, 68, 255),
            accent: Color::new(137, 180, 250, 255),
            font_family: "JetBrainsMono Nerd Font".to_string(),
            font_size: 32.0,
            border_radius: 6.0,
            border_width: 1.0,
            shadow: Shadow::default(),
            float_background: Color::new(49, 50, 68, 255),
            float_accent: Color::new(137, 180, 250, 255),
            float_focus: Color::new(250, 179, 135, 255),
            sidebar_drag_ghost_bg: Color::new(137, 180, 250, 217),
            sidebar_drag_ghost_fg: Color::new(255, 255, 255, 255),
            sidebar_drag_source_bg: Color::new(137, 180, 250, 38),
            sidebar_drag_source_border: Color::new(137, 180, 250, 255),
            sidebar_label_font_size: 14.0,
            sidebar_button_font_size: 11.0,
        }
    }

    pub fn catppuccin_latte() -> Self {
        Self {
            name: "Catppuccin Latte".to_string(),
            background: Color::new(239, 241, 245, 255),
            foreground: Color::new(76, 79, 105, 255),
            border: Color::new(204, 208, 218, 255),
            accent: Color::new(30, 102, 245, 255),
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
            sidebar_drag_ghost_bg: Color::new(30, 102, 245, 217),
            sidebar_drag_ghost_fg: Color::new(255, 255, 255, 255),
            sidebar_drag_source_bg: Color::new(30, 102, 245, 38),
            sidebar_drag_source_border: Color::new(30, 102, 245, 255),
            sidebar_label_font_size: 14.0,
            sidebar_button_font_size: 11.0,
        }
    }

    pub fn load(name: &str) -> Self {
        Self::load_from_disk(name).unwrap_or_else(|| Self::load_bundled(name).unwrap_or_default())
    }

    fn load_from_disk(name: &str) -> Option<Self> {
        let path = config_dir().join("themes").join(format!("{}.toml", name));
        std::fs::read_to_string(path)
            .ok()
            .and_then(|c| toml::from_str(&c).ok())
    }

    fn load_bundled(name: &str) -> Option<Self> {
        let bundled: HashMap<&str, &str> = [
            ("mocha", include_str!("themes/mocha.toml")),
            ("latte", include_str!("themes/latte.toml")),
        ]
        .into_iter()
        .collect();
        toml::from_str(bundled.get(name)?).ok()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme_is_mocha() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Catppuccin Mocha");
    }

    #[test]
    fn test_bundled_mocha_theme() {
        assert_eq!(Theme::load("mocha").name, "Catppuccin Mocha");
    }

    #[test]
    fn test_bundled_latte_theme() {
        assert_eq!(Theme::load("latte").name, "Catppuccin Latte");
    }

}
