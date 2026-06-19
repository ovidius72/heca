//! Theme loading: user config dir → bundled → grid_tron fallback.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::theme::Theme;

/// Configuration directory for heca (`~/.config/heca`).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("heca")
}

/// Load a theme by name.
///
/// Resolution order:
/// 1. `~/.config/heca/themes/{name}.toml` (user override)
/// 2. Bundled `{name}.toml` (shipped with the app)
/// 3. Fallback to bundled `grid_tron.toml` (never fails)
pub fn load_theme(name: &str) -> Theme {
    load_theme_from_disk(name)
        .or_else(|| load_bundled_theme(name))
        .unwrap_or_else(|| {
            // Last resort: parse the bundled grid_tron (which we know is valid).
            load_bundled_theme("grid_tron").expect("bundled grid_tron.toml should always parse")
        })
}

fn load_theme_from_disk(name: &str) -> Option<Theme> {
    let path = config_dir().join("themes").join(format!("{name}.toml"));
    let content = std::fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

fn bundled_themes() -> HashMap<&'static str, &'static str> {
    [
        ("grid_tron", include_str!("themes/grid_tron.toml")),
        ("mocha", include_str!("themes/mocha.toml")),
        ("frappe", include_str!("themes/frappe.toml")),
    ]
    .into_iter()
    .collect()
}

fn load_bundled_theme(name: &str) -> Option<Theme> {
    let bundled = bundled_themes();
    let content = bundled.get(name)?;
    toml::from_str(content).ok()
}

/// List all available theme names (bundled + user overrides).
pub fn available_themes() -> Vec<String> {
    let mut names: Vec<String> = vec![
        "grid_tron".to_string(),
        "mocha".to_string(),
        "frappe".to_string(),
    ];

    // Add user themes from config dir.
    let themes_dir = config_dir().join("themes");
    if let Ok(entries) = std::fs::read_dir(themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                && !names.contains(&stem.to_string())
            {
                names.push(stem.to_string());
            }
        }
    }

    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_grid_tron_by_default() {
        let theme = load_theme("grid_tron");
        assert_eq!(theme.name, "Grid Tron");
    }

    #[test]
    fn load_mocha() {
        let theme = load_theme("mocha");
        assert_eq!(theme.name, "Catppuccin Mocha");
    }

    #[test]
    fn load_frappe() {
        let theme = load_theme("frappe");
        assert_eq!(theme.name, "Catppuccin Frappe");
    }

    #[test]
    fn unknown_theme_falls_back_to_grid_tron() {
        let theme = load_theme("nonexistent_theme_xyz");
        assert_eq!(theme.name, "Grid Tron");
    }

    #[test]
    fn available_themes_includes_bundled() {
        let themes = available_themes();
        assert!(themes.contains(&"grid_tron".to_string()));
        assert!(themes.contains(&"mocha".to_string()));
        assert!(themes.contains(&"frappe".to_string()));
    }
}
