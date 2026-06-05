use crate::keys::KeysConfig;
use crate::settings::SettingsConfig;
use crate::theme::Theme;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
