use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer};
use crate::color::Color;

// ═══════════════════════════════════════════════════════════════════════════════
//  ModifierKey
// ═══════════════════════════════════════════════════════════════════════════════

/// Modifier keys that can be used for mouse-driven interactive actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ModifierKey {
    /// Super / Command / Windows key.
    #[default]
    Super,
    /// Alt / Option key.
    Alt,
    /// Control key (also accepts "Control" in config).
    #[serde(alias = "Control")]
    Ctrl,
    /// Shift key.
    Shift,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Default-value helpers (used by serde attributes on SettingsConfig)
// ═══════════════════════════════════════════════════════════════════════════════

fn default_theme() -> String {
    "grid_tron".to_string()
}

fn default_mouse() -> bool {
    true
}

fn default_window_width() -> u32 {
    1280
}

fn default_window_height() -> u32 {
    800
}

fn default_auto_scroll_edge() -> bool {
    true
}

fn default_always_center_single_column() -> bool {
    false
}

fn default_shell_integration() -> bool {
    true
}

fn deserialize_terminal_font_size<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let size = Option::<f32>::deserialize(deserializer)?;
    match size {
        Some(value) if !value.is_finite() || value <= 0.0 => {
            Err(de::Error::custom("terminal_font_size must be a finite positive number"))
        }
        other => Ok(other),
    }
}

fn deserialize_font_size<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let size = Option::<f32>::deserialize(deserializer)?;
    match size {
        Some(value) if !value.is_finite() || value <= 0.0 => {
            Err(de::Error::custom("font_size must be a finite positive number"))
        }
        other => Ok(other),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  SettingsConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// User-configurable settings that control behaviour and appearance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_mouse")]
    pub mouse: bool,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    /// Optional UI/chrome font family override from `config.toml` — decouples the
    /// UI font from the color theme. Maps onto `Theme.font_family`.
    #[serde(default, alias = "font-family")]
    pub font_family: Option<String>,
    /// Optional UI/chrome font size override from `config.toml` — decouples the UI
    /// font size from the color theme. Maps onto `Theme.font_size`.
    #[serde(default, alias = "font-size", deserialize_with = "deserialize_font_size")]
    pub font_size: Option<f32>,
    /// Optional terminal font family override from `config.toml`.
    #[serde(default, alias = "terminal-font-family")]
    pub terminal_font_family: Option<String>,
    /// Optional terminal default foreground override from `config.toml`.
    #[serde(default, alias = "terminal-foreground")]
    pub terminal_foreground: Option<Color>,
    /// Optional terminal default background override from `config.toml`.
    #[serde(default, alias = "terminal-background")]
    pub terminal_background: Option<Color>,
    /// Optional terminal cursor foreground override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-foreground")]
    pub terminal_cursor_foreground: Option<Color>,
    /// Optional terminal cursor background override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-background")]
    pub terminal_cursor_background: Option<Color>,
    /// Optional terminal cursor border override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-border")]
    pub terminal_cursor_border: Option<Color>,
    /// Optional terminal selection foreground override from `config.toml`.
    #[serde(default, alias = "terminal-selection-foreground")]
    pub terminal_selection_foreground: Option<Color>,
    /// Optional terminal selection background override from `config.toml`.
    #[serde(default, alias = "terminal-selection-background")]
    pub terminal_selection_background: Option<Color>,
    /// Optional terminal ANSI `0..7` palette override from `config.toml`.
    #[serde(default, alias = "terminal-ansi")]
    pub terminal_ansi: Option<[Color; 8]>,
    /// Optional terminal bright ANSI `8..15` palette override from `config.toml`.
    #[serde(default, alias = "terminal-brights")]
    pub terminal_brights: Option<[Color; 8]>,
    /// Optional terminal italic font family override from `config.toml`.
    #[serde(default, alias = "terminal-italic-font-family")]
    pub terminal_italic_font_family: Option<String>,
    /// Optional terminal font size override from `config.toml`.
    #[serde(
        default,
        alias = "terminal-font-size",
        deserialize_with = "deserialize_terminal_font_size"
    )]
    pub terminal_font_size: Option<f32>,
    /// Automatically scroll the workspace view when the pointer hovers near the left/right edge.
    #[serde(default = "default_auto_scroll_edge")]
    pub auto_scroll_edge: bool,
    /// Modifier key that must be held to initiate an interactive pane drag with the mouse.
    #[serde(default)]
    pub interactive_move_modifier: ModifierKey,
    /// Center a single column even when it fits within the viewport.
    #[serde(default = "default_always_center_single_column")]
    pub always_center_single_column: bool,
    /// Auto-inject shell integration snippets for OSC 133/OSC 7 pane runtime signals.
    #[serde(default = "default_shell_integration")]
    pub shell_integration: bool,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            mouse: default_mouse(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            font_family: None,
            font_size: None,
            terminal_font_family: None,
            terminal_foreground: None,
            terminal_background: None,
            terminal_cursor_foreground: None,
            terminal_cursor_background: None,
            terminal_cursor_border: None,
            terminal_selection_foreground: None,
            terminal_selection_background: None,
            terminal_ansi: None,
            terminal_brights: None,
            terminal_italic_font_family: None,
            terminal_font_size: None,
            auto_scroll_edge: default_auto_scroll_edge(),
            interactive_move_modifier: ModifierKey::default(),
            always_center_single_column: default_always_center_single_column(),
            shell_integration: default_shell_integration(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_config_default_values() {
        let s = SettingsConfig::default();
        assert_eq!(s.theme, "grid_tron");
        assert!(s.mouse);
        assert_eq!(s.window_width, 1280);
        assert_eq!(s.window_height, 800);
        assert_eq!(s.font_family, None);
        assert_eq!(s.font_size, None);
        assert_eq!(s.terminal_font_family, None);
        assert_eq!(s.terminal_foreground, None);
        assert_eq!(s.terminal_background, None);
        assert_eq!(s.terminal_cursor_foreground, None);
        assert_eq!(s.terminal_cursor_background, None);
        assert_eq!(s.terminal_cursor_border, None);
        assert_eq!(s.terminal_selection_foreground, None);
        assert_eq!(s.terminal_selection_background, None);
        assert_eq!(s.terminal_ansi, None);
        assert_eq!(s.terminal_brights, None);
        assert_eq!(s.terminal_italic_font_family, None);
        assert_eq!(s.terminal_font_size, None);
        assert!(s.auto_scroll_edge);
        assert_eq!(s.interactive_move_modifier, ModifierKey::Super);
        assert!(!s.always_center_single_column);
        assert!(s.shell_integration);
    }

    #[test]
    fn test_modifier_key_control_alias() {
        #[derive(Deserialize)]
        struct Wrap {
            #[serde(default)]
            m: ModifierKey,
        }

        let ctrl: Wrap = toml::from_str(r#"m = "Control""#).unwrap();
        assert_eq!(ctrl.m, ModifierKey::Ctrl);

        let pascal: Wrap = toml::from_str(r#"m = "Ctrl""#).unwrap();
        assert_eq!(pascal.m, ModifierKey::Ctrl);
    }

    #[test]
    fn test_terminal_font_size_accepts_positive_finite_values() {
        #[derive(Deserialize)]
        struct Wrap {
            #[serde(
                default,
                alias = "terminal-font-size",
                deserialize_with = "deserialize_terminal_font_size"
            )]
            terminal_font_size: Option<f32>,
        }

        let wrap: Wrap = toml::from_str("terminal-font-size = 14.0").unwrap();
        assert_eq!(wrap.terminal_font_size, Some(14.0));
    }

    #[test]
    fn test_terminal_font_size_rejects_invalid_values() {
        assert!(toml::from_str::<SettingsConfig>("terminal-font-size = 0.0").is_err());
        assert!(toml::from_str::<SettingsConfig>("terminal-font-size = -1.0").is_err());
    }

    #[test]
    fn test_ui_font_overrides_parse() {
        let s: SettingsConfig =
            toml::from_str("font_family = \"Iosevka\"\nfont_size = 16.0").unwrap();
        assert_eq!(s.font_family.as_deref(), Some("Iosevka"));
        assert_eq!(s.font_size, Some(16.0));

        // kebab-case aliases parse too.
        let s: SettingsConfig =
            toml::from_str("font-family = \"Iosevka\"\nfont-size = 16.0").unwrap();
        assert_eq!(s.font_family.as_deref(), Some("Iosevka"));
        assert_eq!(s.font_size, Some(16.0));
    }

    #[test]
    fn test_ui_font_size_rejects_invalid_values() {
        assert!(toml::from_str::<SettingsConfig>("font-size = 0.0").is_err());
        assert!(toml::from_str::<SettingsConfig>("font-size = -1.0").is_err());
    }
}
