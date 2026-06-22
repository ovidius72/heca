use crate::keys::KeysConfig;
use crate::programs::ProgramsConfig;
use crate::settings::SettingsConfig;
use crate::theme::{self, Theme};
use serde::{Deserialize, Serialize};
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
    /// Structured font configuration (families + sizes), decoupled from the
    /// color theme. See [`crate::font::FontConfig`].
    #[serde(default)]
    pub font: crate::font::FontConfig,
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
            let mut theme = theme::load(&config.settings.theme);
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
        let mut theme = theme::load(&config.settings.theme);
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

/// Apply `[settings]` overrides onto the loaded color `theme`. Covers the
/// terminal color palette overrides only — font family/size configuration has
/// moved to the dedicated `[font]` block (see [`crate::font::FontConfig`]).
fn apply_overrides(theme: &mut Theme, settings: &SettingsConfig) {
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
}

fn validate_config(config: &Config) -> Result<(), String> {
    config.font.validate()?;
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

/// Transitional compatibility shim: preserve the `heca-config::loader::load_theme`
/// entry point while delegating to the unified theme crate via `crate::theme::load`.
pub fn load_theme(name: &str) -> Theme {
    theme::load(name)
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    #[test]
    fn test_default_app_config_uses_grid_tron() {
        let config = Config::default();
        let mut theme = theme::load(&config.settings.theme);
        apply_overrides(&mut theme, &config.settings);

        assert_eq!(config.settings.theme, "grid_tron");
        assert_eq!(theme.name, "Grid Tron");
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
    fn terminal_color_overrides_apply_via_apply_overrides() {
        let mut theme = theme::load("mocha");
        let settings: SettingsConfig = toml::from_str(
            r##"
terminal-background = "#112233"
terminal-foreground = "#ddeeff"
"##,
        )
        .expect("settings should parse");

        apply_overrides(&mut theme, &settings);

        assert_eq!(theme.terminal_background, Some(Color::new(17, 34, 51, 255)));
        assert_eq!(theme.terminal_foreground, Some(Color::new(221, 238, 255, 255)));
    }

    #[test]
    fn terminal_color_values_survive_when_settings_do_not_override_them() {
        let mut theme = theme::load("mocha");
        let original_bg = theme.terminal_background;
        apply_overrides(&mut theme, &SettingsConfig::default());
        assert_eq!(theme.terminal_background, original_bg);
    }

    #[test]
    fn font_config_parses_from_full_config() {
        let toml = r##"
[font.family.ui]
normal = "Iosevka"

[font.family.terminal]
normal = "Iosevka Term"
italic = "Iosevka Term Italic"

[font.size]
ui = 18.0
terminal = 16.0
"##;
        let cfg: Config = toml::from_str(toml).expect("config should parse");
        assert_eq!(cfg.font.family.ui_normal(), "Iosevka");
        assert_eq!(cfg.font.family.terminal_normal(), "Iosevka Term");
        assert_eq!(
            cfg.font.family.terminal.italic.as_deref(),
            Some("Iosevka Term Italic")
        );
        assert_eq!(cfg.font.size.ui, 18.0);
        assert_eq!(cfg.font.size.terminal, 16.0);
        cfg.font.validate().unwrap();
    }

    #[test]
    fn font_config_defaults_when_section_absent() {
        let cfg: Config = toml::from_str("").unwrap();
        assert_eq!(cfg.font.family.ui_normal(), "Geist Mono");
        assert_eq!(cfg.font.family.terminal_normal(), "Maple Mono Normal NF");
        assert_eq!(cfg.font.size.ui, 15.0);
        assert_eq!(cfg.font.size.terminal, 14.0);
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
