use std::borrow::Cow;
use std::collections::{hash_map::Entry, HashMap};

use crate::color::Color;
use serde::{Deserialize, Deserializer, Serialize};

/// Default nerd-font glyph used for terminal-backed panes.
pub const DEFAULT_TERMINAL_ICON: &str = "\u{eae8}";

/// User-configurable metadata for a raw foreground program name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProgramMeta {
    /// Display name shown in pane chrome.
    #[serde(default)]
    pub name: Option<String>,
    /// Free-form glyph string rendered as the program icon.
    #[serde(default)]
    pub icon: Option<String>,
    /// Reserved for future menus and richer chrome; not rendered today.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional tint color for the pane card or border.
    #[serde(default)]
    pub color: Option<Color>,
}

impl ProgramMeta {
    fn with_terminal_icon() -> Self {
        Self {
            icon: Some(DEFAULT_TERMINAL_ICON.to_string()),
            ..Self::default()
        }
    }

    fn merge_with(&mut self, override_meta: ProgramMeta) {
        let ProgramMeta {
            name,
            icon,
            description,
            color,
        } = override_meta;
        if let Some(name) = name {
            self.name = Some(name);
        }
        if let Some(icon) = icon {
            self.icon = Some(icon);
        }
        if let Some(description) = description {
            self.description = Some(description);
        }
        if let Some(color) = color {
            self.color = Some(color);
        }
    }
}

/// Resolved view of a raw program name after defaults and user overrides.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramView<'a> {
    /// Raw foreground program name reported by the runtime.
    pub raw: &'a str,
    /// Display name shown in the UI.
    pub name: Cow<'a, str>,
    /// Resolved icon glyph string.
    pub icon: Cow<'a, str>,
    /// Optional tint color.
    pub color: Option<Color>,
}

/// Catalog of program metadata keyed by raw program name.
///
/// Deserialization merges user overrides over built-in defaults; serialization
/// emits the merged view because the runtime only needs resolved catalog state.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ProgramsConfig {
    entries: HashMap<String, ProgramMeta>,
}

impl ProgramsConfig {
    /// Resolve a raw program name into a display-oriented view.
    pub fn resolve<'a>(&'a self, raw: &'a str) -> ProgramView<'a> {
        let meta = self.entries.get(raw);
        ProgramView {
            raw,
            name: meta
                .and_then(|entry| entry.name.as_deref())
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Borrowed(raw)),
            icon: meta
                .and_then(|entry| entry.icon.as_deref())
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Borrowed(DEFAULT_TERMINAL_ICON)),
            color: meta.and_then(|entry| entry.color),
        }
    }

    fn from_overrides(overrides: HashMap<String, ProgramMeta>) -> Self {
        let mut config = Self::default();
        for (raw, override_meta) in overrides {
            match config.entries.entry(raw) {
                Entry::Occupied(mut entry) => entry.get_mut().merge_with(override_meta),
                Entry::Vacant(entry) => {
                    entry.insert(override_meta);
                }
            }
        }
        config
    }
}

impl Default for ProgramsConfig {
    fn default() -> Self {
        let mut entries = HashMap::new();
        for shell in ["sh", "bash", "zsh", "fish"] {
            entries.insert(shell.to_string(), ProgramMeta::with_terminal_icon());
        }
        Self { entries }
    }
}

impl<'de> Deserialize<'de> for ProgramsConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let overrides = HashMap::<String, ProgramMeta>::deserialize(deserializer)?;
        Ok(Self::from_overrides(overrides))
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::{ProgramView, ProgramsConfig, DEFAULT_TERMINAL_ICON};
    use crate::color::Color;

    #[test]
    fn default_shell_hit_uses_terminal_icon() {
        let programs = ProgramsConfig::default();

        assert_eq!(
            programs.resolve("zsh"),
            ProgramView {
                raw: "zsh",
                name: Cow::Borrowed("zsh"),
                icon: Cow::Borrowed(DEFAULT_TERMINAL_ICON),
                color: None,
            }
        );
    }

    #[test]
    fn user_override_merges_over_built_in_shell_defaults() {
        let programs: ProgramsConfig = toml::from_str(
            r#"
[zsh]
name = "Z Shell"
"#,
        )
        .expect("programs config should parse");

        assert_eq!(
            programs.resolve("zsh"),
            ProgramView {
                raw: "zsh",
                name: Cow::Borrowed("Z Shell"),
                icon: Cow::Borrowed(DEFAULT_TERMINAL_ICON),
                color: None,
            }
        );
    }

    #[test]
    fn miss_falls_back_to_raw_name_and_terminal_icon() {
        let programs = ProgramsConfig::default();

        assert_eq!(
            programs.resolve("unknown-tool"),
            ProgramView {
                raw: "unknown-tool",
                name: Cow::Borrowed("unknown-tool"),
                icon: Cow::Borrowed(DEFAULT_TERMINAL_ICON),
                color: None,
            }
        );
    }

    #[test]
    fn color_parses_and_passes_through() {
        let programs: ProgramsConfig = toml::from_str(
            r##"
[nvim]
name = "Neovim"
color = "#11223344"
"##,
        )
        .expect("programs config should parse");

        assert_eq!(
            programs.resolve("nvim"),
            ProgramView {
                raw: "nvim",
                name: Cow::Borrowed("Neovim"),
                icon: Cow::Borrowed(DEFAULT_TERMINAL_ICON),
                color: Some(Color::new(17, 34, 51, 68)),
            }
        );
    }
}
