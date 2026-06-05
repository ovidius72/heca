pub use crate::color::Color;
pub use crate::keys::{
    BindingValue, CommandKeybindConfig, KeybindingMap, KeyModeConfig, KeysConfig,
    ModeBindingConfig,
};
pub use crate::settings::{ModifierKey, SettingsConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
    #[serde(default = "default_float_bg")]
    pub float_background: Color,
    #[serde(default = "default_float_accent")]
    pub float_accent: Color,
    #[serde(default = "default_float_focus")]
    pub float_focus: Color,
    // ── Sidebar drag-and-drop colors ──
    #[serde(default = "default_drag_ghost_bg")]
    pub sidebar_drag_ghost_bg: Color,
    #[serde(default = "default_drag_ghost_fg")]
    pub sidebar_drag_ghost_fg: Color,
    #[serde(default = "default_drag_source_bg")]
    pub sidebar_drag_source_bg: Color,
    #[serde(default = "default_drag_source_border")]
    pub sidebar_drag_source_border: Color,
    // ── Sidebar font sizes ──
    #[serde(default = "default_sidebar_label_font_size")]
    pub sidebar_label_font_size: f32,
    #[serde(default = "default_sidebar_button_font_size")]
    pub sidebar_button_font_size: f32,
}

fn default_float_bg() -> Color {
    Color::new(49, 50, 68, 255)
}
fn default_float_accent() -> Color {
    Color::new(137, 180, 250, 255)
}
fn default_float_focus() -> Color {
    Color::new(250, 179, 135, 255)
}
fn default_drag_ghost_bg() -> Color {
    Color::new(137, 180, 250, 217)
} // accent @ 85%
fn default_drag_ghost_fg() -> Color {
    Color::new(255, 255, 255, 255)
} // white
fn default_drag_source_bg() -> Color {
    Color::new(137, 180, 250, 38)
} // accent @ 15%
fn default_drag_source_border() -> Color {
    Color::new(137, 180, 250, 255)
} // accent
fn default_sidebar_label_font_size() -> f32 {
    14.0
}
fn default_sidebar_button_font_size() -> f32 {
    11.0
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
//  Config
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub settings: SettingsConfig,
    #[serde(default)]
    pub keys: KeysConfig,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  AppConfig
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub config: Config,
    pub theme: Theme,
}

impl AppConfig {
    pub fn load() -> Self {
        let config = Self::load_config_file().unwrap_or_else(|_| Config::default());
        let theme = Theme::load(&config.settings.theme);
        Self { config, theme }
    }

    fn load_config_file() -> Result<Config, String> {
        let paths = [
            dirs::home_dir().map(|h| h.join(".config").join("heca").join("config.toml")),
            Some(config_dir().join("config.toml")),
        ];
        for path in paths.into_iter().flatten() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                match toml::from_str::<Config>(&content) {
                    Ok(config) => {
                        return Ok(config);
                    }
                    Err(e) => {
                        return Err(format!("parse error in {}: {}", path.display(), e));
                    }
                }
            }
        }
        Err("no config file found".to_string())
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("heca")
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

    #[test]
    fn test_fallback_when_config_missing() {
        let app = AppConfig::load();
        assert_eq!(app.config.settings.theme, "mocha");
        assert_eq!(app.theme.name, "Catppuccin Mocha");
    }

    #[test]
    fn test_config_default_prefix() {
        let cfg = Config::default();
        assert_eq!(cfg.keys.prefix, "ctrl+b");
    }

    #[test]
    fn test_config_has_default_bindings() {
        let cfg = Config::default();
        assert!(cfg.keys.bindings.contains_key("focus_left"));
        assert!(cfg.keys.bindings.contains_key("split_horizontal"));
        assert!(cfg.keys.bindings.contains_key("zoom_column"));
        assert!(cfg.keys.bindings.contains_key("rename_column"));
        assert!(cfg.keys.bindings.contains_key("close"));
    }

    #[test]
    fn test_parse_toml_config() {
        let toml = r#"
[settings]
theme = "mocha"
mouse = true

[keys]
prefix = "ctrl+a"
focus_left = ["h", "Left"]
focus_right = "l"

[[keys.command]]
key = "prefix+Shift+g"
command = "lazygit"

[[keys.mode]]
name = "resize"
trigger = "prefix+r"

[[keys.mode.bindings]]
action = "resize_increase"
keys = "="
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.keys.prefix, "ctrl+a");
        assert_eq!(
            cfg.keys.bindings.get("focus_left").unwrap().keys(),
            vec!["h", "Left"]
        );
        assert_eq!(
            cfg.keys.bindings.get("focus_right").unwrap().keys(),
            vec!["l"]
        );
        assert_eq!(cfg.keys.command.len(), 1);
        assert_eq!(cfg.keys.mode.len(), 1);
        assert_eq!(cfg.keys.mode[0].name, "resize");
        assert_eq!(cfg.keys.mode[0].bindings.len(), 1);
    }
}
