use std::borrow::Cow;
use std::collections::{HashMap, hash_map::Entry};

use crate::color::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError};

/// Semantic Phosphor icon ids supported by the program catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProgramIcon {
    Terminal,
    FileCode,
    Folder,
    FolderOpen,
    GitBranch,
    Gear,
    Search,
}

impl ProgramIcon {
    /// Stable config spelling for this icon.
    pub const fn as_str(self) -> &'static str {
        match self {
            ProgramIcon::Terminal => "terminal",
            ProgramIcon::FileCode => "file_code",
            ProgramIcon::Folder => "folder",
            ProgramIcon::FolderOpen => "folder_open",
            ProgramIcon::GitBranch => "git_branch",
            ProgramIcon::Gear => "gear",
            ProgramIcon::Search => "search",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        let normalized = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        match normalized.as_str() {
            "terminal" => Some(ProgramIcon::Terminal),
            "file_code" | "filecode" | "code" | "editor" => Some(ProgramIcon::FileCode),
            "folder" => Some(ProgramIcon::Folder),
            "folder_open" | "folderopen" => Some(ProgramIcon::FolderOpen),
            "git_branch" | "gitbranch" | "branch" => Some(ProgramIcon::GitBranch),
            "gear" | "settings" => Some(ProgramIcon::Gear),
            "search" => Some(ProgramIcon::Search),
            _ => None,
        }
    }
}

impl Serialize for ProgramIcon {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProgramIcon {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).ok_or_else(|| {
            D::Error::custom(format!(
                "unknown program icon '{raw}'; expected one of: terminal, file_code, folder, folder_open, git_branch, gear, search"
            ))
        })
    }
}

/// Default Phosphor icon used for terminal-backed panes.
pub const DEFAULT_TERMINAL_ICON: ProgramIcon = ProgramIcon::Terminal;

/// User-configurable metadata for a foreground program.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProgramMeta {
    /// Disable this catalog entry entirely so built-in defaults fall back to the
    /// raw process name and default terminal icon.
    #[serde(default)]
    pub disabled: Option<bool>,
    /// Display name shown in pane chrome.
    #[serde(default)]
    pub name: Option<String>,
    /// Raw process names that should resolve to this canonical entry.
    #[serde(default)]
    pub processes: Option<Vec<String>>,
    /// Semantic Phosphor icon id rendered for this program.
    #[serde(default, alias = "icons")]
    pub icon: Option<ProgramIcon>,
    /// Reserved for future menus and richer chrome; not rendered today.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional tint color for the pane card or border.
    #[serde(default)]
    pub color: Option<Color>,
}

impl ProgramMeta {
    fn disabled(&self) -> bool {
        self.disabled.unwrap_or(false)
    }

    fn with_terminal_icon() -> Self {
        Self {
            icon: Some(DEFAULT_TERMINAL_ICON),
            ..Self::default()
        }
    }

    fn named(name: &str, processes: &[&str], icon: ProgramIcon, color: Option<Color>) -> Self {
        Self {
            name: Some(name.to_string()),
            processes: Some(
                processes
                    .iter()
                    .map(|process| (*process).to_string())
                    .collect(),
            ),
            icon: Some(icon),
            color,
            ..Self::default()
        }
    }

    fn merge_with(&mut self, override_meta: ProgramMeta) {
        let ProgramMeta {
            disabled,
            name,
            processes,
            icon,
            description,
            color,
        } = override_meta;
        if let Some(disabled) = disabled {
            self.disabled = Some(disabled);
        }
        if let Some(name) = name {
            self.name = Some(name);
        }
        if let Some(processes) = processes {
            self.processes = Some(processes);
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
    /// Resolved semantic icon.
    pub icon: ProgramIcon,
    /// Optional tint color.
    pub color: Option<Color>,
}

/// Catalog of program metadata keyed by canonical app id.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramsConfig {
    entries: HashMap<String, ProgramMeta>,
    aliases: HashMap<String, String>,
}

impl ProgramsConfig {
    /// Resolve a raw program name into a display-oriented view.
    pub fn resolve<'a>(&'a self, raw: &'a str) -> ProgramView<'a> {
        let meta = self
            .aliases
            .get(raw)
            .and_then(|canonical| self.entries.get(canonical))
            .or_else(|| self.entries.get(raw))
            .filter(|entry| !entry.disabled());
        ProgramView {
            raw,
            name: meta
                .and_then(|entry| entry.name.as_deref())
                .map(Cow::Borrowed)
                .unwrap_or_else(|| Cow::Borrowed(raw)),
            icon: meta
                .and_then(|entry| entry.icon)
                .unwrap_or(DEFAULT_TERMINAL_ICON),
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
        config.rebuild_aliases();
        config
    }

    fn rebuild_aliases(&mut self) {
        self.aliases.clear();
        let mut canonical_ids: Vec<&String> = self.entries.keys().collect();
        canonical_ids.sort();
        for canonical in canonical_ids {
            let Some(meta) = self.entries.get(canonical) else {
                continue;
            };
            if meta.disabled() {
                continue;
            }
            self.aliases
                .entry(canonical.clone())
                .or_insert_with(|| canonical.clone());
            if let Some(processes) = &meta.processes {
                for process in processes {
                    self.aliases
                        .entry(process.clone())
                        .or_insert_with(|| canonical.clone());
                }
            }
        }
    }
}

impl Default for ProgramsConfig {
    fn default() -> Self {
        let mut entries = HashMap::new();
        for shell in ["sh", "bash", "zsh", "fish"] {
            entries.insert(shell.to_string(), ProgramMeta::with_terminal_icon());
        }
        entries.insert(
            "claude".to_string(),
            ProgramMeta::named("Claude", &["claude"], ProgramIcon::Terminal, None),
        );
        entries.insert(
            "codex".to_string(),
            ProgramMeta::named("Codex", &["codex"], ProgramIcon::Terminal, None),
        );
        entries.insert(
            "helix".to_string(),
            ProgramMeta::named(
                "Helix",
                &["helix"],
                ProgramIcon::FileCode,
                Some(Color::new(163, 190, 140, 255)),
            ),
        );
        entries.insert(
            "nvim".to_string(),
            ProgramMeta::named(
                "Neovim",
                &["v", "nvim"],
                ProgramIcon::FileCode,
                Some(Color::new(137, 180, 250, 255)),
            ),
        );
        entries.insert(
            "opencode".to_string(),
            ProgramMeta::named("OpenCode", &["opencode"], ProgramIcon::Terminal, None),
        );
        entries.insert(
            "pi".to_string(),
            ProgramMeta::named("Pi", &["pi"], ProgramIcon::Terminal, None),
        );
        entries.insert(
            "ranger".to_string(),
            ProgramMeta::named(
                "Ranger",
                &["ranger"],
                ProgramIcon::FolderOpen,
                Some(Color::new(148, 226, 213, 255)),
            ),
        );
        entries.insert(
            "vim".to_string(),
            ProgramMeta::named(
                "Vim",
                &["v", "vim"],
                ProgramIcon::FileCode,
                Some(Color::new(166, 227, 161, 255)),
            ),
        );
        entries.insert(
            "yazi".to_string(),
            ProgramMeta::named(
                "Yazi",
                &["yazi"],
                ProgramIcon::FolderOpen,
                Some(Color::new(116, 199, 236, 255)),
            ),
        );
        let mut config = Self {
            entries,
            aliases: HashMap::new(),
        };
        config.rebuild_aliases();
        config
    }
}

impl Serialize for ProgramsConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.entries.serialize(serializer)
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

    use super::{DEFAULT_TERMINAL_ICON, ProgramIcon, ProgramView, ProgramsConfig};
    use crate::color::Color;

    #[test]
    fn default_shell_hit_uses_terminal_icon() {
        let programs = ProgramsConfig::default();

        assert_eq!(
            programs.resolve("zsh"),
            ProgramView {
                raw: "zsh",
                name: Cow::Borrowed("zsh"),
                icon: DEFAULT_TERMINAL_ICON,
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
                icon: DEFAULT_TERMINAL_ICON,
                color: None,
            }
        );
    }

    #[test]
    fn alias_processes_resolve_to_canonical_entry() {
        let programs: ProgramsConfig = toml::from_str(
            r#"
[nvim]
name = "Neovim"
processes = ["v", "nvim", "nv"]
icon = "file-code"
"#,
        )
        .expect("programs config should parse");

        assert_eq!(
            programs.resolve("nv"),
            ProgramView {
                raw: "nv",
                name: Cow::Borrowed("Neovim"),
                icon: ProgramIcon::FileCode,
                color: Some(Color::new(137, 180, 250, 255)),
            }
        );
    }

    #[test]
    fn built_in_defaults_resolve_known_programs() {
        let programs = ProgramsConfig::default();

        assert_eq!(programs.resolve("nvim").name, "Neovim");
        assert_eq!(programs.resolve("nvim").icon, ProgramIcon::FileCode);
        assert_eq!(
            programs.resolve("yazi").color,
            Some(Color::new(116, 199, 236, 255))
        );
    }

    #[test]
    fn disabled_entry_falls_back_to_raw_and_terminal_icon() {
        let programs: ProgramsConfig = toml::from_str(
            r#"
[nvim]
disabled = true
"#,
        )
        .expect("programs config should parse");

        assert_eq!(
            programs.resolve("nvim"),
            ProgramView {
                raw: "nvim",
                name: Cow::Borrowed("nvim"),
                icon: DEFAULT_TERMINAL_ICON,
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
                icon: DEFAULT_TERMINAL_ICON,
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
                icon: ProgramIcon::FileCode,
                color: Some(Color::new(17, 34, 51, 68)),
            }
        );
    }

    #[test]
    fn icon_alias_field_deserializes() {
        let programs: ProgramsConfig = toml::from_str(
            r#"
[yazi]
name = "Yazi"
icons = "folder_open"
"#,
        )
        .expect("programs config should parse");

        assert_eq!(
            programs.resolve("yazi"),
            ProgramView {
                raw: "yazi",
                name: Cow::Borrowed("Yazi"),
                icon: ProgramIcon::FolderOpen,
                color: Some(Color::new(116, 199, 236, 255)),
            }
        );
    }
}
