//! **Keys a configuration file sets that heca does not read** — said once at load, never ignored
//! in silence.
//!
//! A key heca does not know used to be dropped without a word, so a retired key simply stopped
//! working: write `[settings] confirm_close_pane = false` and the prompt kept appearing. Now every
//! key a user's file sets is checked against what heca actually read from it, and each one that did
//! not land is reported with the file it is in — and, when it is a name heca used to read, the name
//! that replaced it.
//!
//! **How a key is known to have been read.** The finished [`Config`] is written back out as TOML: a
//! key that is in a user's file but not in that written-out form found no field to land in. There is
//! no list of the fields to keep in step — the schema is the one that reads the file.
//!
//! **Retired names are one table** ([`RETIRED`]): old dotted path → what replaced it. A renamed field
//! stops reading its old name and adds a line here, so its user is told instead of silently losing
//! the setting.

use super::Config;
use std::path::Path;

/// **Names heca used to read, and what replaced each** — dotted paths as written in the file.
///
/// A line here does not make the old name work: it says, at load, that it no longer does and what to
/// write instead.
pub const RETIRED: &[(&str, &str)] = &[
    // Confirmation prompts moved to a table of their own, keyed by action name.
    ("settings.confirm_close_pane", "confirm.close"),
    ("settings.confirm_delete_column", "confirm.delete_column"),
    (
        "settings.confirm_delete_workspace",
        "confirm.delete_workspace",
    ),
    ("confirm.delete_pane", "confirm.close"),
    ("settings.pane_show_cwd", "appearance.expose.show_cwd"),
    // One list of pane lines became one per view, and a list for the current pane.
    (
        "appearance.sidebar.pane_lines",
        "appearance.sidebar.pane_extended_view",
    ),
    ("settings.pane-show-cwd", "appearance.expose.show_cwd"),
    // Kebab-case spellings of settings: the snake_case one is the name.
    (
        "settings.command-palette-size",
        "settings.command_palette_size",
    ),
    ("settings.search-case", "settings.search_case"),
    ("settings.search-history", "settings.search_history"),
    (
        "settings.search-history-size",
        "settings.search_history_size",
    ),
    ("settings.search-usage-size", "settings.search_usage_size"),
    (
        "settings.terminal-foreground",
        "settings.terminal_foreground",
    ),
    (
        "settings.terminal-background",
        "settings.terminal_background",
    ),
    (
        "settings.terminal-cursor-foreground",
        "settings.terminal_cursor_foreground",
    ),
    (
        "settings.terminal-cursor-background",
        "settings.terminal_cursor_background",
    ),
    (
        "settings.terminal-cursor-border",
        "settings.terminal_cursor_border",
    ),
    (
        "settings.terminal-selection-foreground",
        "settings.terminal_selection_foreground",
    ),
    (
        "settings.terminal-selection-background",
        "settings.terminal_selection_background",
    ),
    ("settings.terminal-ansi", "settings.terminal_ansi"),
    ("settings.terminal-brights", "settings.terminal_brights"),
    ("settings.pane-size", "settings.pane_size"),
    (
        "settings.pane-renamed-add-process-name",
        "settings.pane_renamed_add_process_name",
    ),
    (
        "settings.terminal-scrollback-lines",
        "settings.terminal_scrollback_lines",
    ),
    ("settings.terminal-mouse", "settings.terminal_mouse"),
    (
        "settings.terminal-wheel-scroll-lines",
        "settings.terminal_wheel_scroll_lines",
    ),
    (
        "settings.terminal-font-zoom-step",
        "settings.terminal_font_zoom_step",
    ),
    (
        "settings.mouse-wheel-change-font-size",
        "settings.mouse_wheel_change_font_size",
    ),
    (
        "settings.terminal-scroll-animations",
        "settings.terminal_scroll_animations",
    ),
    ("settings.show-left-sidebar", "settings.show_left_sidebar"),
    ("settings.show-right-sidebar", "settings.show_right_sidebar"),
    ("settings.show-top-bar", "settings.show_top_bar"),
    ("settings.show-bottom-bar", "settings.show_bottom_bar"),
    (
        "settings.notification-system",
        "settings.notification_system",
    ),
    // Other old spellings.
    ("programs", "program"),
    ("program.*.icons", "program.<id>.icon"),
    ("keys.prefix_key", "keys.prefix"),
    ("keys.command.key", "keys.command.keys"),
    ("keys.command.command_type", "keys.command.kind"),
    ("keys.mode.bindings.key", "keys.mode.bindings.keys"),
    // A terminal bell is attention feedback, like an agent waiting for you.
    ("appearance.terminal.bell_attention", BELL_IS_ATTENTION),
    ("appearance.terminal.bell_visual", BELL_IS_ATTENTION),
    ("appearance.terminal.bell_audible", BELL_IS_ATTENTION),
    ("appearance.terminal.bell_flash_alpha", BELL_IS_ATTENTION),
    // Colours come from the theme: a theme of your own can inherit one and change only these.
    (
        "appearance.border_color",
        "the theme's `border` (a theme of yours: `inherit = \"mocha\"` and `border = \"#…\"`)",
    ),
    (
        "appearance.pane.border_color",
        "the theme's `pane_border` (a theme of yours: `inherit = \"mocha\"` and `pane_border = \"#…\"`)",
    ),
    (
        "appearance.pane.active_border_color",
        "the theme's `pane_active_border` (a theme of yours: `inherit = \"mocha\"` and `pane_active_border = \"#…\"`)",
    ),
    (
        "appearance.pane.floating_border_color",
        "the theme's `float_pane_border` (a theme of yours: `inherit = \"mocha\"` and `float_pane_border = \"#…\"`)",
    ),
];

/// What a retired bell key is told: the bell is now the attention feedback every pane has.
const BELL_IS_ATTENTION: &str = "the attention feedback: a bell in a pane you are not looking at flashes its row and plays the `bell` status sound (`[settings.agents.statuses.bell]`)";

/// Whether `pattern` (a [`RETIRED`] path, `*` standing for any one name) matches `key`.
fn matches(pattern: &str, key: &str) -> bool {
    let (mut p, mut k) = (pattern.split('.'), key.split('.'));
    loop {
        match (p.next(), k.next()) {
            (None, None) => return true,
            (Some(a), Some(b)) if a == "*" || a == b => {}
            _ => return false,
        }
    }
}

/// What replaced `key`, if it is a name heca used to read.
fn retired(key: &str) -> Option<&'static str> {
    RETIRED
        .iter()
        .find(|(old, _)| matches(old, key))
        .map(|(_, now)| *now)
}

/// One key a file set that heca did not read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unread {
    /// Its dotted path, as written (`settings.confirm_close_pane`).
    pub key: String,
    /// What replaced it, when it is a name heca used to read.
    pub now: Option<&'static str>,
}

impl std::fmt::Display for Unread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.now {
            // A renamed key is a name to write; a setting that moved elsewhere is said in words.
            Some(now) if now.contains(' ') => {
                write!(f, "`{}` is no longer read — use {now} instead", self.key)
            }
            Some(now) => write!(
                f,
                "`{}` is no longer read — write `{now}` instead",
                self.key
            ),
            None => write!(
                f,
                "`{}` is not a setting heca knows — it is ignored",
                self.key
            ),
        }
    }
}

/// **The keys `file` sets that did not land in `config`**, retired ones with their replacement. Keys
/// under `[extensions.*]` belong to an extension, which reads its own table, and a binding's `args`
/// are checked against its action when the keys are loaded, so neither is reported here.
pub fn unread_keys(file: &toml::Value, config: &Config) -> Vec<Unread> {
    let Ok(read) = toml::Value::try_from(config) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    walk(file, &read, "", &mut found);
    found
        .into_iter()
        .map(|key| {
            let now = retired(&key);
            Unread { key, now }
        })
        .collect()
}

/// Every path in `file` with no counterpart in `read`, into `out`.
fn walk(file: &toml::Value, read: &toml::Value, at: &str, out: &mut Vec<String>) {
    match (file, read) {
        (toml::Value::Table(file), toml::Value::Table(read)) => {
            for (key, value) in file {
                let path = if at.is_empty() {
                    key.clone()
                } else {
                    format!("{at}.{key}")
                };
                // An extension reads its own table; an action's `args` are checked against what the
                // action declares when the binding is loaded, with the reason.
                if path == "extensions" || key == "args" {
                    continue;
                }
                // A retired name is said even where a free-form table (`[confirm]`) took it in.
                match read.get(key) {
                    Some(_) if retired(&path).is_some() => out.push(path),
                    Some(read) => walk(value, read, &path, out),
                    None => out.push(path),
                }
            }
        }
        // An array of tables (`[[keys.mode]]`): a key is read when some element read has it. The
        // file's elements are not matched one to one, because the final list may be merged.
        (toml::Value::Array(file), toml::Value::Array(read)) => {
            let read_tables: Vec<&toml::Table> =
                read.iter().filter_map(toml::Value::as_table).collect();
            if read_tables.is_empty() {
                return;
            }
            let mut union = toml::Table::new();
            for table in read_tables {
                for (key, value) in table {
                    union.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
            for element in file.iter().filter(|e| e.is_table()) {
                walk(element, &toml::Value::Table(union.clone()), at, out);
            }
        }
        _ => {}
    }
}

/// **Say every unread key of `file`, once, with the file's path** — what the loader calls for each
/// of the user's files and a trusted project file.
pub(super) fn report(path: &Path, file: &toml::Value, config: &Config) {
    for unread in unread_keys(file, config) {
        eprintln!("[heca] {}: {unread}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(file: &str) -> (toml::Value, Config) {
        let value: toml::Value = toml::from_str(file).unwrap();
        let config = super::super::deep_merge(super::super::embedded_base(), value.clone())
            .try_into()
            .unwrap();
        (value, config)
    }

    #[test]
    fn heca_s_own_defaults_are_all_read() {
        let base = super::super::embedded_base();
        let config: Config = base.clone().try_into().unwrap();
        assert_eq!(
            unread_keys(&base, &config),
            Vec::new(),
            "every shipped key lands in a field"
        );
    }

    /// **The old bell keys are said to be the attention feedback now** — each one reported, naming
    /// the `bell` status that replaced them.
    #[test]
    fn a_retired_bell_key_points_at_the_attention_feedback() {
        let (file, config) = config(
            "[appearance.terminal]\nbell_attention = true\nbell_visual = true\nbell_audible = true\nbell_flash_alpha = 56\n",
        );
        let said: Vec<String> = unread_keys(&file, &config)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(said.len(), 4, "{said:?}");
        assert!(
            said.iter()
                .all(|line| line.contains("settings.agents.statuses.bell")),
            "{said:?}"
        );
    }

    /// **A border colour set in configuration is said to belong to the theme now** — each of the
    /// four retired keys is reported, in words naming the theme's colour that replaced it.
    #[test]
    fn a_retired_border_colour_points_at_the_theme() {
        let (file, config) = config(
            "[appearance]\nborder_color = \"#ff0000\"\n\n[appearance.pane]\nborder_color = \"#ff0000\"\nactive_border_color = \"#00ff00\"\nfloating_border_color = \"#0000ff\"\n",
        );
        let said: Vec<String> = unread_keys(&file, &config)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(said.len(), 4, "{said:?}");
        for (key, theme_colour) in [
            ("appearance.border_color", "`border`"),
            ("appearance.pane.border_color", "`pane_border`"),
            (
                "appearance.pane.active_border_color",
                "`pane_active_border`",
            ),
            (
                "appearance.pane.floating_border_color",
                "`float_pane_border`",
            ),
        ] {
            assert!(
                said.iter().any(|line| line.contains(&format!("`{key}`"))
                    && line.contains(theme_colour)
                    && line.contains("inherit")),
                "{key} → {theme_colour}: {said:?}"
            );
        }
    }

    #[test]
    fn a_retired_key_is_said_with_its_replacement_and_a_typo_as_unknown() {
        let (file, config) = config(
            "[settings]\nconfirm_close_pane = false\nterminal_scrolback_lines = 5\n\n[extensions.rooms]\nanything = 1\n",
        );
        let unread = unread_keys(&file, &config);
        assert_eq!(
            unread,
            vec![
                Unread {
                    key: "settings.confirm_close_pane".into(),
                    now: Some("confirm.close")
                },
                Unread {
                    key: "settings.terminal_scrolback_lines".into(),
                    now: None
                },
            ],
            "an extension's table is its own business",
        );
        assert!(
            unread[0]
                .to_string()
                .contains("write `confirm.close` instead")
        );
    }

    #[test]
    fn a_key_inside_a_list_of_tables_is_checked_too() {
        let (file, config) =
            config("[[keys.mode]]\nname = \"resize\"\ntrigger = \"prefix+r\"\nstickyy = true\n");
        let unread = unread_keys(&file, &config);
        assert_eq!(unread.len(), 1, "{unread:?}");
        assert_eq!(unread[0].key, "keys.mode.stickyy");
    }
}
