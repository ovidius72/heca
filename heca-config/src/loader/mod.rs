use crate::keys::KeysConfig;
use crate::programs::{ProgramMeta, ProgramsConfig};
use crate::settings::SettingsConfig;
use crate::theme::{self, Theme};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

mod project;
mod sources;

pub use project::{ProjectFile, project_config_path, read_project, read_project_in, read_project_with};
pub use sources::{ConfigSource, ConfigSources, config_sources};

// ═══════════════════════════════════════════════════════════════════════════════
//  Embedded default config (single source of truth)
// ═══════════════════════════════════════════════════════════════════════════════

/// Non-keybinding defaults (`[settings]`, `[appearance]`, `[font]`, `[program]`).
/// Embedded at build time so the defaults are always parseable and complete by
/// construction — a malformed file panics on startup (guarded by tests).
pub(crate) const CONFIG_DEFAULT: &str = include_str!("../../../config.default.toml");

/// Keybinding defaults (`[keys]` only). Embedded at build time alongside
/// [`CONFIG_DEFAULT`]; the two cover disjoint TOML sections.
pub(crate) const KEYS_DEFAULT: &str = include_str!("../../../keybindings.default.toml");

/// Parse the embedded defaults into a single merged `toml::Value` (the base every
/// user config is layered over). The two files cover disjoint top-level sections
/// (`config.default.toml` → settings/appearance/font/program; `keybindings.default
/// .toml` → keys), so the merge is a clean union.
fn embedded_base() -> toml::Value {
    let config: toml::Value =
        toml::from_str(CONFIG_DEFAULT).expect("embedded config.default.toml must be valid TOML");
    let keys: toml::Value =
        toml::from_str(KEYS_DEFAULT).expect("embedded keybindings.default.toml must be valid TOML");
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

/// **The one key-by-key merge**: `over` onto `base` — tables merge per-key (recursively); scalars and
/// arrays are replaced by `over`. Arrays are intentionally NOT concatenated, so a
/// user `[[keys.command]]`/`[[keys.mode]]` list replaces the default list wholesale.
///
/// Every layer of configuration goes through it — the embedded defaults, the user's files and the
/// project's `.heca/config.toml` — so two layers can never disagree about what "merge" means.
pub fn deep_merge(base: toml::Value, over: toml::Value) -> toml::Value {
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

/// **The configuration the embedded defaults and one user file's text make** — what the loader
/// builds from disk, from a string: a test of "what does this config text do" needs no file.
pub fn config_from_layers(user: &str) -> Result<Config, toml::de::Error> {
    let user: toml::Value = toml::from_str(user)?;
    deep_merge(embedded_base(), user).try_into()
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
        if let Some((_, user)) = read_first_toml(&config_file_paths())? {
            value = deep_merge(value, user);
            found_user = true;
        }
        if let Some((_, user)) = read_first_toml(&keybindings_file_paths())? {
            value = deep_merge(value, user);
            found_user = true;
        }
        // The project's own file goes on last, so any key it sets wins. A broken one is reported
        // and left out: a project's typo must not take the user's whole configuration with it.
        let project = match read_project() {
            Ok(project) => project,
            Err(e) => {
                eprintln!("[heca] project settings ignored: {e}");
                None
            }
        };
        let applied = trusted_layer(project);
        if !found_user && applied.is_none() {
            return Err(ConfigError::NotFound);
        }
        layered(value, applied)
    }
}

/// **A project file applies only once the user has trusted it** (see [`crate::trust`]). An untrusted
/// one is said, with its folder and how to trust it, and left out.
fn trusted_layer(project: Option<ProjectFile>) -> Option<(PathBuf, toml::Value)> {
    let project = project?;
    if project.trusted {
        return Some((project.path, project.value));
    }
    eprintln!(
        "[heca] {} is not trusted, so it is ignored — run `heca --trust` in {} to apply it",
        project.path.display(),
        project.dir.display()
    );
    None
}

/// Finish the configuration from the merged user layers and, if there is one, the project's file.
///
/// A project file that parses but does not fit the schema is reported and ignored — the user's
/// configuration stands — so the two failure modes of a project file (bad TOML, wrong shape) end the
/// same way.
fn layered(
    user: toml::Value,
    project: Option<(PathBuf, toml::Value)>,
) -> Result<Config, ConfigError> {
    if let Some((path, over)) = project {
        match finish(deep_merge(user.clone(), over), &path) {
            Ok(config) => return Ok(config),
            Err(e) => eprintln!("[heca] project settings ignored: {e}"),
        }
    }
    finish(user, &config_dir().join("config.toml"))
}

/// Turn a merged value into a validated [`Config`]; `blame` names the file an error points at.
fn finish(value: toml::Value, blame: &Path) -> Result<Config, ConfigError> {
    let config: Config = value.try_into().map_err(|e| ConfigError::Invalid {
        path: blame.to_path_buf(),
        message: e.to_string(),
    })?;
    if let Err(message) = validate_config(&config) {
        return Err(ConfigError::Invalid {
            path: blame.to_path_buf(),
            message,
        });
    }
    Ok(config)
}

/// Candidate paths for the user's `config.toml`, in priority order.
pub(super) fn config_file_paths() -> [Option<PathBuf>; 2] {
    [
        dirs::home_dir().map(|h| h.join(".config").join("heca").join("config.toml")),
        Some(config_dir().join("config.toml")),
    ]
}

/// Candidate paths for the user's `keybindings.toml`, in priority order.
pub(super) fn keybindings_file_paths() -> [Option<PathBuf>; 2] {
    [
        dirs::home_dir().map(|h| h.join(".config").join("heca").join("keybindings.toml")),
        Some(config_dir().join("keybindings.toml")),
    ]
}

/// Read and TOML-parse the first existing file among `paths`. Missing files are
/// skipped; a parse error or non-`NotFound` IO error is surfaced. Returns the
/// parsed value and the path it came from, or `None` if no file exists.
pub(super) fn read_first_toml(
    paths: &[Option<PathBuf>],
) -> Result<Option<(PathBuf, toml::Value)>, ConfigError> {
    for path in paths.iter().flatten() {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let value =
                    toml::from_str::<toml::Value>(&content).map_err(|e| ConfigError::Parse {
                        path: path.clone(),
                        source: e,
                    })?;
                return Ok(Some((path.clone(), value)));
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
    if let Some(color) = settings.drag_edge_target_color {
        theme.drag_edge_target_color = Some(color);
    }
    if let Some(color) = settings.drag_edge_color {
        theme.drag_edge_color = Some(color);
    }
    if let Some(width) = settings.drag_edge_width {
        theme.drag_edge_width = width.clamp(1.0, 12.0);
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
mod tests;
