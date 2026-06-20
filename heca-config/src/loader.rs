use crate::keys::KeysConfig;
use crate::programs::ProgramsConfig;
use crate::settings::SettingsConfig;
use crate::theme::Theme;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Errors that can occur when loading the configuration file.
#[derive(Debug)]
#[non_exhaustive]
pub enum ConfigError {
    /// No config file found in any search path.
    NotFound,
    /// Config file parsed, but semantic validation failed.
    Invalid {
        path: PathBuf,
        message: String,
    },
    /// Config file found but could not be parsed as valid TOML.
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    /// Config file could not be read from disk.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NotFound => write!(f, "no config file found"),
            ConfigError::Invalid { path, message } => {
                write!(f, "invalid config in {}: {message}", path.display())
            }
            ConfigError::Parse { path, source } => {
                write!(f, "parse error in {}: {source}", path.display())
            }
            ConfigError::Io { path, source } => {
                write!(f, "io error reading {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::NotFound => None,
            ConfigError::Invalid { .. } => None,
            ConfigError::Parse { source, .. } => Some(source),
            ConfigError::Io { source, .. } => Some(source),
        }
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
    pub appearance: crate::appearance::AppearanceConfig,
    #[serde(default, alias = "program")]
    pub programs: ProgramsConfig,
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
    /// Load config with fallback to built-in defaults on error.
    ///
    /// Intended for initial startup where a user config is optional.
    pub fn load() -> Self {
        Self::try_load().unwrap_or_else(|e| {
            eprintln!("[heca] config load: {e}, using defaults");
            let config = Config::default();
            let mut theme = Theme::load(&config.settings.theme);
            apply_overrides(&mut theme, &config.settings);
            Self { config, theme }
        })
    }

    /// Load config from disk, failing on parse/io errors rather than
    /// falling back to defaults.
    ///
    /// Intended for runtime reload so a bad config file doesn't silently
    /// overwrite the current working config.
    pub fn try_load() -> Result<Self, ConfigError> {
        let config = Self::load_config_file()?;
        let mut theme = Theme::load(&config.settings.theme);
        apply_overrides(&mut theme, &config.settings);
        Ok(Self { config, theme })
    }

    fn load_config_file() -> Result<Config, ConfigError> {
        let paths = [
            dirs::home_dir().map(|h| h.join(".config").join("heca").join("config.toml")),
            Some(config_dir().join("config.toml")),
        ];
        for path in paths.into_iter().flatten() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match toml::from_str::<Config>(&content) {
                        Ok(config) => {
                            validate_config(&config).map_err(|message| ConfigError::Invalid {
                                path: path.clone(),
                                message,
                            })?;
                            return Ok(config);
                        }
                        Err(e) => {
                            return Err(ConfigError::Parse {
                                path: path.clone(),
                                source: e,
                            });
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(ConfigError::Io {
                        path: path.clone(),
                        source: e,
                    });
                }
            }
        }
        Err(ConfigError::NotFound)
    }
}

/// Apply `[settings]` overrides onto the loaded color `theme`. Covers the UI
/// font (decoupled from the color preset) and all terminal overrides.
fn apply_overrides(theme: &mut Theme, settings: &SettingsConfig) {
    // UI/chrome font — decoupled from the color theme (§ Phase 7 B).
    if let Some(family) = &settings.font_family {
        theme.font_family = family.clone();
    }
    if let Some(size) = settings.font_size {
        theme.font_size = size;
    }
    if let Some(family) = &settings.terminal_font_family {
        theme.terminal_font_family = family.clone();
    }
    if let Some(color) = settings.terminal_foreground {
        theme.terminal_foreground = Some(color);
    }
    if let Some(color) = settings.terminal_background {
        theme.terminal_background = Some(color);
    }
    if let Some(color) = settings.terminal_cursor_foreground {
        theme.terminal_cursor_foreground = Some(color);
    }
    if let Some(color) = settings.terminal_cursor_background {
        theme.terminal_cursor_background = Some(color);
    }
    if let Some(color) = settings.terminal_cursor_border {
        theme.terminal_cursor_border = Some(color);
    }
    if let Some(color) = settings.terminal_selection_foreground {
        theme.terminal_selection_foreground = Some(color);
    }
    if let Some(color) = settings.terminal_selection_background {
        theme.terminal_selection_background = Some(color);
    }
    if let Some(colors) = settings.terminal_ansi {
        theme.terminal_ansi = Some(colors);
    }
    if let Some(colors) = settings.terminal_brights {
        theme.terminal_brights = Some(colors);
    }
    if let Some(family) = &settings.terminal_italic_font_family {
        theme.terminal_italic_font_family = family.clone();
    }
    if let Some(size) = settings.terminal_font_size {
        theme.terminal_font_size = size;
    }
}

fn validate_config(config: &Config) -> Result<(), String> {
    for command in &config.keys.command {
        command.validate()?;
    }
    Ok(())
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("heca")
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Theme loading
// ═══════════════════════════════════════════════════════════════════════════════

/// Try loading a theme from the user's theme directory, then fall back to a
/// bundled theme, then fall back to the default theme.
pub fn load_theme(name: &str) -> Theme {
    load_theme_from_disk(name)
        .or_else(|| load_bundled_theme(name))
        .unwrap_or_default()
}

fn load_theme_from_disk(name: &str) -> Option<Theme> {
    let path = config_dir().join("themes").join(format!("{}.toml", name));
    std::fs::read_to_string(path)
        .ok()
        .and_then(|c| toml::from_str(&c).ok())
}

fn load_bundled_theme(name: &str) -> Option<Theme> {
    let bundled: HashMap<&str, &str> = [
        ("mocha", include_str!("themes/mocha.toml")),
        ("latte", include_str!("themes/latte.toml")),
    ]
    .into_iter()
    .collect();
    toml::from_str(bundled.get(name)?).ok()
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

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
        let toml = r##"
[settings]
theme = "mocha"
mouse = true

[keys]
prefix = "ctrl+a"
focus_left = ["h", "Left"]
focus_right = "l"

[[keys.command]]
keys = "prefix+Shift+g"
command = "lazygit"
float = true
close_pane = true
keep_on_error = true

[[keys.mode]]
name = "resize"
trigger = "prefix+r"

[[keys.mode.bindings]]
action = "resize_increase"
keys = "="

[program.nvim]
name = "Neovim"
processes = ["v", "nvim", "nv"]
icon = "file_code"
color = "#112233"
"##;
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
        assert_eq!(cfg.keys.command[0].kind, "terminal");
        assert!(cfg.keys.command[0].float);
        assert!(cfg.keys.command[0].close_pane);
        assert!(cfg.keys.command[0].keep_on_error);
        assert_eq!(cfg.programs.resolve("nv").name, "Neovim");
        assert_eq!(cfg.programs.resolve("nv").icon, crate::programs::ProgramIcon::FileCode);
        assert_eq!(
            cfg.programs.resolve("nv").color,
            Some(Color::new(17, 34, 51, 255))
        );
        assert_eq!(cfg.keys.mode.len(), 1);
        assert_eq!(cfg.keys.mode[0].name, "resize");
        assert_eq!(cfg.keys.mode[0].bindings.len(), 1);
    }

    #[test]
    fn terminal_theme_values_survive_when_settings_do_not_override_them() {
        let mut theme = Theme::load("mocha");
        let original_family = theme.terminal_font_family.clone();
        let original_italic_family = theme.terminal_italic_font_family.clone();
        let original_size = theme.terminal_font_size;

        apply_overrides(&mut theme, &SettingsConfig::default());

        assert_eq!(theme.terminal_font_family, original_family);
        assert_eq!(theme.terminal_italic_font_family, original_italic_family);
        assert_eq!(theme.terminal_font_size, original_size);
    }

    #[test]
    fn settings_override_bundled_terminal_theme_values() {
        let mut theme = Theme::load("mocha");
        let settings: SettingsConfig = toml::from_str(
            r##"
terminal-font-family = "Iosevka Term"
terminal-italic-font-family = "Iosevka Term Italic"
terminal-font-size = 16.0
terminal-background = "#112233"
terminal-foreground = "#ddeeff"
"##,
        )
        .expect("settings should parse");

        apply_overrides(&mut theme, &settings);

        assert_eq!(theme.terminal_font_family, "Iosevka Term");
        assert_eq!(theme.terminal_italic_font_family, "Iosevka Term Italic");
        assert_eq!(theme.terminal_font_size, 16.0);
        assert_eq!(theme.terminal_background, Some(Color::new(17, 34, 51, 255)));
        assert_eq!(theme.terminal_foreground, Some(Color::new(221, 238, 255, 255)));
    }

    #[test]
    fn ui_font_decoupled_from_color_preset_uses_defaults() {
        // Color presets no longer carry the UI font — it falls back to the
        // struct default (Geist Mono / 15.0), not the dead 32.0 of old.
        let theme = Theme::load("mocha");
        assert_eq!(theme.font_family, crate::defaults::default_font_family());
        assert_eq!(theme.font_size, crate::defaults::default_font_size());
    }

    #[test]
    fn settings_override_ui_font() {
        let mut theme = Theme::load("mocha");
        let settings: SettingsConfig = toml::from_str(
            r##"
font-family = "Iosevka"
font-size = 18.0
"##,
        )
        .expect("settings should parse");

        apply_overrides(&mut theme, &settings);

        assert_eq!(theme.font_family, "Iosevka");
        assert_eq!(theme.font_size, 18.0);
    }

    #[test]
    fn test_parse_toml_rejects_invalid_command_kind() {
        let toml = r#"
[[keys.command]]
keys = "prefix+g"
command = "lazygit"
kind = "terminl"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert!(validate_config(&cfg).is_err());
    }
}
