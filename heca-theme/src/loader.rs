//! Theme loading: user config dir → bundled → grid_tron fallback.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::theme::Theme;

/// The system's configuration folder for heca (`~/Library/Application Support/heca` on macOS,
/// `~/.config/heca` on Linux) — the second of the [`user_dirs`].
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("heca")
}

/// **The folders a user's own files are read from, in priority order** — `~/.config/heca`, then the
/// system's configuration folder ([`config_dir`]). The one search order for `config.toml`,
/// `keybindings.toml`, the icon and the `themes/` folder, so a file is found wherever the others are.
pub fn user_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = dirs::home_dir()
        .map(|home| home.join(".config").join("heca"))
        .into_iter()
        .collect();
    let system = config_dir();
    if !dirs.contains(&system) {
        dirs.push(system);
    }
    dirs
}

/// The `themes/` folders a user's themes are read from, in the [`user_dirs`] order.
fn theme_dirs() -> Vec<PathBuf> {
    user_dirs()
        .into_iter()
        .map(|dir| dir.join("themes"))
        .collect()
}

/// **Load a theme by name** — what [`load_theme_checked`] loads, without its problems.
pub fn load_theme(name: &str) -> Theme {
    load_theme_checked(name).0
}

/// **Load a theme by name, and say what was wrong with it.**
///
/// A theme is found in `~/.config/heca/themes/{name}.toml` (yours), else among the bundled ones —
/// so a file of yours named after a bundled theme takes its place. **A theme may inherit another**:
/// `inherit = "mocha"` starts from that theme (yours or bundled, itself inheriting as it says) and
/// lays this file's keys over it with the same key-by-key merge configuration uses
/// ([`deep_merge`](crate::deep_merge)), so a file says only what it changes:
///
/// ```toml
/// # ~/.config/heca/themes/my-theme.toml — chosen with `theme = "my-theme"`
/// name = "My theme"
/// inherit = "mocha"
/// border = "#ff7edb"
/// ```
///
/// When something is wrong — the file does not parse, it inherits a theme that does not exist, or
/// a chain of themes comes back on itself — the problem is returned rather than dropped, and the
/// theme falls back to the bundled one of that name, else `grid_tron` (never fails).
pub fn load_theme_checked(name: &str) -> (Theme, Vec<String>) {
    load_theme_checked_in(&theme_dirs(), name)
}

/// [`load_theme_checked`], reading the user's themes from the folders `themes` (first found wins) —
/// the one place a test points somewhere other than the user's own folders.
pub fn load_theme_checked_in(themes: &[PathBuf], name: &str) -> (Theme, Vec<String>) {
    let resolved = resolve(themes, name, &mut Vec::new()).and_then(|value| {
        value
            .try_into::<Theme>()
            .map_err(|error| format!("theme `{name}` is not a theme heca can read: {error}"))
    });
    match resolved {
        Ok(theme) => (theme, Vec::new()),
        Err(problem) => {
            let fallback = load_bundled_theme(name).unwrap_or_else(|| {
                load_bundled_theme("grid_tron").expect("bundled grid_tron.toml should always parse")
            });
            (fallback, vec![problem])
        }
    }
}

/// The theme `name` as TOML, with what it inherits laid under it, looked up in the user's folders
/// `themes` first and then among the bundled ones. `chain` is the themes already on the way here, so
/// a chain that comes back on itself is said instead of followed forever.
///
/// A file of yours that inherits **its own name** — `mocha.toml` saying `inherit = "mocha"` — means
/// the bundled theme of that name plus your changes, so that one parent is looked up among the
/// bundled themes only.
fn resolve(themes: &[PathBuf], name: &str, chain: &mut Vec<String>) -> Result<toml::Value, String> {
    if chain.iter().any(|seen| seen == name) {
        chain.push(name.to_string());
        return Err(format!("themes inherit in a circle: {}", chain.join(" → ")));
    }
    chain.push(name.to_string());
    let (mut value, from) = read_theme(themes, name)?;
    let inherit = value
        .as_table_mut()
        .and_then(|table| table.remove("inherit"));
    match inherit {
        None => Ok(value),
        Some(toml::Value::String(base)) if base == name => {
            let (base, _) = read_theme(&[], &base)?;
            Ok(crate::deep_merge(base, value))
        }
        Some(toml::Value::String(base)) => {
            let base = resolve(themes, &base, chain)?;
            Ok(crate::deep_merge(base, value))
        }
        Some(other) => Err(format!(
            "{from}: `inherit` names a theme, so it is text — found {other}"
        )),
    }
}

/// The text of the theme `name` parsed as TOML — the first of the folders `themes` holding
/// `{name}.toml`, else the bundled theme of that name — with where it came from, for a message.
fn read_theme(themes: &[PathBuf], name: &str) -> Result<(toml::Value, String), String> {
    let found = themes
        .iter()
        .map(|dir| dir.join(format!("{name}.toml")))
        .find_map(|path| std::fs::read_to_string(&path).ok().map(|text| (text, path)));
    let (text, from) = match found {
        Some((text, path)) => (text, path.display().to_string()),
        None => match bundled_themes().get(name) {
            Some(text) => (text.to_string(), format!("the bundled theme `{name}`")),
            None => return Err(format!("there is no theme named `{name}`")),
        },
    };
    let value = toml::from_str(&text).map_err(|error| format!("{from}: {error}"))?;
    Ok((value, from))
}

pub(crate) fn bundled_themes() -> HashMap<&'static str, &'static str> {
    [
        ("grid_tron", include_str!("themes/grid_tron.toml")),
        ("mocha", include_str!("themes/mocha.toml")),
        ("latte", include_str!("themes/latte.toml")),
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
        "latte".to_string(),
    ];

    // Add the user's themes, from every folder they are read from.
    for themes_dir in theme_dirs() {
        let Ok(entries) = std::fs::read_dir(themes_dir) else {
            continue;
        };
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
    fn load_latte() {
        let theme = load_theme("latte");
        assert_eq!(theme.name, "Catppuccin Latte");
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
        assert!(themes.contains(&"latte".to_string()));
    }
}

#[cfg(test)]
mod inherit_tests {
    use super::*;

    fn folder(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary folder");
        for (name, text) in files {
            std::fs::write(dir.path().join(format!("{name}.toml")), text).expect("a theme file");
        }
        dir
    }

    /// **A theme that inherits says only what it changes**: everything else is the theme it
    /// inherits — here the bundled `mocha` — and its own keys win.
    #[test]
    fn a_theme_inherits_a_bundled_one_and_changes_only_what_it_says() {
        let dir = folder(&[(
            "my-theme",
            "name = \"My theme\"\ninherit = \"mocha\"\nborder = \"#ff7edb\"\n",
        )]);
        let (theme, problems) = load_theme_checked_in(&[dir.path().to_path_buf()], "my-theme");
        let mocha = load_theme("mocha");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(theme.border, "#ff7edb".parse().unwrap(), "its own key wins");
        assert_eq!(theme.name, "My theme");
        assert_eq!(theme.background, mocha.background, "the rest is mocha's");
        assert_eq!(theme.accent, mocha.accent);
    }

    /// **A file of yours named after a bundled theme can inherit it**: `mocha.toml` saying
    /// `inherit = "mocha"` is the bundled mocha plus its changes — not a circle.
    #[test]
    fn a_theme_inherits_the_bundled_one_of_its_own_name() {
        let dir = folder(&[("mocha", "inherit = \"mocha\"\nborder = \"#ff7edb\"\n")]);
        let (theme, problems) = load_theme_checked_in(&[dir.path().to_path_buf()], "mocha");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(theme.border, "#ff7edb".parse().unwrap());
        assert_eq!(
            theme.name,
            load_theme("mocha").name,
            "the rest is the bundled mocha"
        );
    }

    /// **A chain of themes is followed**: yours inheriting yours inheriting a bundled one.
    #[test]
    fn a_chain_of_inheriting_themes_is_followed() {
        let dir = folder(&[
            (
                "base",
                "name = \"base\"\ninherit = \"mocha\"\naccent = \"#36f9f6\"\n",
            ),
            (
                "top",
                "name = \"top\"\ninherit = \"base\"\nborder = \"#ff7edb\"\n",
            ),
        ]);
        let (theme, problems) = load_theme_checked_in(&[dir.path().to_path_buf()], "top");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            theme.accent,
            "#36f9f6".parse().unwrap(),
            "from the middle theme"
        );
        assert_eq!(
            theme.border,
            "#ff7edb".parse().unwrap(),
            "from the top theme"
        );
        assert_eq!(
            theme.background,
            load_theme("mocha").background,
            "from mocha"
        );
    }

    /// **What is wrong is said, never dropped**: a missing parent, a circle, a file that does not
    /// parse — each comes back as a problem, and the theme falls back to a bundled one.
    #[test]
    fn a_broken_theme_is_reported_and_falls_back() {
        let dir = folder(&[
            ("orphan", "name = \"orphan\"\ninherit = \"nowhere\"\n"),
            ("a", "name = \"a\"\ninherit = \"b\"\n"),
            ("b", "name = \"b\"\ninherit = \"a\"\n"),
            ("broken", "name = = \"broken\"\n"),
        ]);
        let said = |name: &str| {
            load_theme_checked_in(&[dir.path().to_path_buf()], name)
                .1
                .join(" ")
        };
        assert!(
            said("orphan").contains("no theme named `nowhere`"),
            "{}",
            said("orphan")
        );
        assert!(said("a").contains("a → b → a"), "{}", said("a"));
        assert!(said("broken").contains("broken.toml"), "{}", said("broken"));
        let (fallback, _) = load_theme_checked_in(&[dir.path().to_path_buf()], "orphan");
        assert_eq!(
            fallback,
            load_theme("grid_tron"),
            "an unknown name falls back to grid_tron"
        );
    }
}
