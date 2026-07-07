use crate::keys::KeysConfig;
use crate::programs::{ProgramMeta, ProgramsConfig};
use crate::settings::SettingsConfig;
use crate::theme::{self, Theme};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ═══════════════════════════════════════════════════════════════════════════════
//  Embedded default config (single source of truth)
// ═══════════════════════════════════════════════════════════════════════════════

/// Non-keybinding defaults (`[settings]`, `[appearance]`, `[font]`, `[program]`).
/// Embedded at build time so the defaults are always parseable and complete by
/// construction — a malformed file panics on startup (guarded by tests).
pub(crate) const CONFIG_DEFAULT: &str = include_str!("../../config.default.toml");

/// Keybinding defaults (`[keys]` only). Embedded at build time alongside
/// [`CONFIG_DEFAULT`]; the two cover disjoint TOML sections.
pub(crate) const KEYS_DEFAULT: &str = include_str!("../../keybindings.default.toml");

/// Parse the embedded defaults into a single merged `toml::Value` (the base every
/// user config is layered over). The two files cover disjoint top-level sections
/// (`config.default.toml` → settings/appearance/font/program; `keybindings.default
/// .toml` → keys), so the merge is a clean union.
fn embedded_base() -> toml::Value {
    let config: toml::Value =
        toml::from_str(CONFIG_DEFAULT).expect("embedded config.default.toml must be valid TOML");
    let keys: toml::Value = toml::from_str(KEYS_DEFAULT)
        .expect("embedded keybindings.default.toml must be valid TOML");
    deep_merge(config, keys)
}

/// Parse the embedded `[keys]` table into a [`KeysConfig`]. Used as the single
/// source for [`KeysConfig::default`] (no recursion: it deserializes the table
/// directly, never calling back into the `Default` impl).
pub(crate) fn parse_default_keys() -> KeysConfig {
    let value: toml::Value =
        toml::from_str(KEYS_DEFAULT).expect("embedded keybindings.default.toml must be valid TOML");
    value
        .get("keys")
        .cloned()
        .expect("embedded keybindings.default.toml must contain a [keys] table")
        .try_into()
        .expect("embedded keybindings.default.toml [keys] must match the KeysConfig schema")
}

/// Parse the embedded `[program]` catalog into the raw entry map. Used as the
/// single source for [`ProgramsConfig::default`]. Deserializes into the plain map
/// type (not [`ProgramsConfig`]) so it never recurses through the catalog's
/// `Deserialize`/`Default` path.
pub(crate) fn parse_default_program_entries() -> HashMap<String, ProgramMeta> {
    let value: toml::Value =
        toml::from_str(CONFIG_DEFAULT).expect("embedded config.default.toml must be valid TOML");
    match value.get("program") {
        Some(program) => program
            .clone()
            .try_into()
            .expect("embedded config.default.toml [program] must match the catalog schema"),
        None => HashMap::new(),
    }
}

/// Deep-merge `over` onto `base`: tables merge per-key (recursively); scalars and
/// arrays are replaced by `over`. Arrays are intentionally NOT concatenated, so a
/// user `[[keys.command]]`/`[[keys.mode]]` list replaces the default list wholesale.
fn deep_merge(base: toml::Value, over: toml::Value) -> toml::Value {
    match (base, over) {
        (toml::Value::Table(mut b), toml::Value::Table(o)) => {
            for (k, ov) in o {
                let nv = match b.remove(&k) {
                    Some(bv) => deep_merge(bv, ov),
                    None => ov,
                };
                b.insert(k, nv);
            }
            toml::Value::Table(b)
        }
        // Scalars and arrays: `over` wins (arrays are replaced, not merged).
        (_, over) => over,
    }
}

/// Errors that can occur when loading the configuration file.
#[derive(Debug)]
#[non_exhaustive]
pub enum ConfigError {
    /// No config file found in any search path.
    NotFound,
    /// Config file parsed, but semantic validation failed.
    Invalid { path: PathBuf, message: String },
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// `[confirm]` — per-action confirmation toggles (generic, keyed by action name).
    #[serde(default)]
    pub confirm: crate::confirm::ConfirmConfig,
}

impl Default for Config {
    /// The default config is the parsed embedded defaults — the files are the
    /// single source of truth, so `Config::default()` and a no-user-config startup
    /// produce identical values.
    fn default() -> Self {
        embedded_base()
            .try_into()
            .expect("embedded default config must deserialize into Config")
    }
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
        // Start from the embedded defaults, then layer the optional user files on
        // top: config.toml (settings/appearance/font/program) and keybindings.toml
        // (keys). Both are deep-merged at the `toml::Value` level so a partial user
        // file overrides only the values it sets and keeps every other default.
        let mut value = embedded_base();
        let mut found_user = false;
        if let Some(user) = read_first_toml(&config_file_paths())? {
            value = deep_merge(value, user);
            found_user = true;
        }
        if let Some(user) = read_first_toml(&keybindings_file_paths())? {
            value = deep_merge(value, user);
            found_user = true;
        }
        if !found_user {
            return Err(ConfigError::NotFound);
        }

        let config: Config = value.try_into().map_err(|e| ConfigError::Invalid {
            path: config_dir().join("config.toml"),
            message: e.to_string(),
        })?;
        if let Err(message) = validate_config(&config) {
            return Err(ConfigError::Invalid {
                path: config_dir().join("config.toml"),
                message,
            });
        }
        Ok(config)
    }
}

/// Candidate paths for the user's `config.toml`, in priority order.
fn config_file_paths() -> [Option<PathBuf>; 2] {
    [
        dirs::home_dir().map(|h| h.join(".config").join("heca").join("config.toml")),
        Some(config_dir().join("config.toml")),
    ]
}

/// Candidate paths for the user's `keybindings.toml`, in priority order.
fn keybindings_file_paths() -> [Option<PathBuf>; 2] {
    [
        dirs::home_dir().map(|h| h.join(".config").join("heca").join("keybindings.toml")),
        Some(config_dir().join("keybindings.toml")),
    ]
}

/// Read and TOML-parse the first existing file among `paths`. Missing files are
/// skipped; a parse error or non-`NotFound` IO error is surfaced. Returns the
/// parsed value, or `None` if no file exists.
fn read_first_toml(paths: &[Option<PathBuf>]) -> Result<Option<toml::Value>, ConfigError> {
    for path in paths.iter().flatten() {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let value = toml::from_str::<toml::Value>(&content).map_err(|e| {
                    ConfigError::Parse {
                        path: path.clone(),
                        source: e,
                    }
                })?;
                return Ok(Some(value));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(ConfigError::Io {
                    path: path.clone(),
                    source: e,
                })
            }
        }
    }
    Ok(None)
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
        assert!(cfg.keys.bindings.contains_key("move_pane_to_column_pick"));
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
        assert_eq!(
            cfg.programs.resolve("nv").icon,
            crate::programs::ProgramIcon::FileCode
        );
        assert_eq!(
            cfg.programs.resolve("nv").color,
            Some(Color::new(17, 34, 51, 255))
        );
        assert_eq!(cfg.keys.mode.len(), 1);
        assert_eq!(cfg.keys.mode[0].name, "resize");
        assert_eq!(cfg.keys.mode[0].bindings.len(), 1);
    }

    /// Guard: the embedded `config.default.toml` must always parse against the
    /// live schema — it is the single source of the non-key defaults, parsed at
    /// every startup. A schema/file drift fails here (and would panic the app).
    #[test]
    fn config_default_toml_parses() {
        let cfg: Config = embedded_base()
            .try_into()
            .expect("config.default.toml + keybindings.default.toml must deserialize into Config");
        assert_eq!(cfg.settings.theme, "grid_tron");
        // The active appearance numeric knobs are all the documented defaults.
        assert_eq!(cfg.appearance, crate::appearance::AppearanceConfig::default());
        // The program catalog is sourced from the file.
        assert_eq!(cfg.programs.resolve("nvim").name, "Neovim");
        assert_eq!(
            cfg.programs.resolve("nvim").icon,
            crate::programs::ProgramIcon::FileCode
        );
        assert_eq!(
            cfg.programs.resolve("yazi").color,
            Some(Color::new(116, 199, 236, 255))
        );
    }

    /// Guard: the embedded `keybindings.default.toml` must parse and carry the
    /// full default keymap (prefix + ~50 bindings + the three built-in modes).
    #[test]
    fn keybindings_default_toml_parses() {
        let keys = crate::loader::parse_default_keys();
        assert_eq!(keys.prefix, "ctrl+b");
        // A representative spread of the flat bindings.
        for action in [
            "focus_left",
            "split_horizontal",
            "zoom_column",
            "move_pane_to_column_pick",
            "close",
            "reload_config",
            "paste_clipboard",
        ] {
            assert!(
                keys.bindings.contains_key(action),
                "missing default binding: {action}"
            );
        }
        assert!(keys.bindings.len() >= 45, "expected the full default keymap");
        // The three built-in modes are present with their bindings.
        for mode_name in ["resize", "sidebar", "selection"] {
            let mode = keys
                .mode
                .iter()
                .find(|m| m.name == mode_name)
                .unwrap_or_else(|| panic!("missing default mode: {mode_name}"));
            assert!(!mode.bindings.is_empty());
        }
    }

    #[test]
    fn deep_merge_merges_tables_per_key_and_replaces_arrays() {
        let base: toml::Value = toml::from_str(
            r#"
[settings]
theme = "grid_tron"
mouse = true

[keys]
focus_left = "prefix+h"
focus_right = "prefix+l"
list = [1, 2, 3]
"#,
        )
        .unwrap();
        let over: toml::Value = toml::from_str(
            r#"
[settings]
mouse = false

[keys]
focus_left = "prefix+a"
list = [9]
"#,
        )
        .unwrap();

        let merged = deep_merge(base, over);
        let settings = merged.get("settings").unwrap();
        // Table values merge per-key: `theme` kept, `mouse` overridden.
        assert_eq!(settings.get("theme").unwrap().as_str(), Some("grid_tron"));
        assert_eq!(settings.get("mouse").unwrap().as_bool(), Some(false));
        let keys = merged.get("keys").unwrap();
        assert_eq!(keys.get("focus_left").unwrap().as_str(), Some("prefix+a"));
        assert_eq!(keys.get("focus_right").unwrap().as_str(), Some("prefix+l"));
        // Arrays are replaced wholesale, not concatenated.
        assert_eq!(keys.get("list").unwrap().as_array().unwrap().len(), 1);
    }

    #[test]
    fn partial_user_config_overlays_on_file_defaults() {
        // User sets a single appearance knob; everything else stays default.
        let user: toml::Value = toml::from_str("[appearance]\ntransparency = 30\n").unwrap();
        let merged = deep_merge(embedded_base(), user);
        let cfg: Config = merged.try_into().unwrap();
        assert_eq!(cfg.appearance.transparency, 30);
        // Untouched values keep their file defaults.
        assert_eq!(cfg.appearance.blur, 0);
        assert_eq!(cfg.settings.theme, "grid_tron");
        assert_eq!(cfg.keys.prefix, "ctrl+b");
        assert!(cfg.keys.bindings.contains_key("focus_left"));
    }

    #[test]
    fn partial_user_keys_overlay_keeps_other_bindings() {
        // User rebinds a single action; every other default binding survives.
        let user: toml::Value = toml::from_str("[keys]\nclose = \"prefix+Shift+x\"\n").unwrap();
        let merged = deep_merge(embedded_base(), user);
        let cfg: Config = merged.try_into().unwrap();
        assert_eq!(
            cfg.keys.bindings.get("close").unwrap().keys(),
            vec!["prefix+Shift+x"]
        );
        assert!(cfg.keys.bindings.contains_key("focus_left"));
        assert!(cfg.keys.bindings.contains_key("split_horizontal"));
    }

    #[test]
    fn config_default_matches_embedded_files() {
        // `Config::default()` is exactly the parsed embedded files.
        let cfg = Config::default();
        assert_eq!(cfg.settings.theme, "grid_tron");
        assert_eq!(cfg.keys.prefix, "ctrl+b");
        assert!(cfg.keys.bindings.len() >= 45);
        assert_eq!(cfg.programs.resolve("zsh").name, "zsh");
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
        assert_eq!(
            theme.terminal_foreground,
            Some(Color::new(221, 238, 255, 255))
        );
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
