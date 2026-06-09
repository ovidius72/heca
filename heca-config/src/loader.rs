use crate::keys::KeysConfig;
use crate::settings::SettingsConfig;
use crate::theme::Theme;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Errors that can occur when loading the configuration file.
#[derive(Debug)]
pub enum ConfigError {
    /// No config file found in any search path.
    NotFound,
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
        let config = Self::load_config_file().unwrap_or_else(|e| {
            #[cfg(debug_assertions)]
            eprintln!("[heca] config load: {e}, using defaults");
            Config::default()
        });
        let theme = Theme::load(&config.settings.theme);
        Self { config, theme }
    }

    fn load_config_file() -> Result<Config, ConfigError> {
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
                        return Err(ConfigError::Parse {
                            path: path.clone(),
                            source: e,
                        });
                    }
                }
            }
        }
        Err(ConfigError::NotFound)
    }
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
