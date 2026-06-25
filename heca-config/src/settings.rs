use crate::color::Color;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  ModifierKey
// ═══════════════════════════════════════════════════════════════════════════════

/// Modifier keys that can be used for mouse-driven interactive actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ModifierKey {
    /// Super / Command / Windows key.
    #[default]
    Super,
    /// Alt / Option key.
    Alt,
    /// Control key (also accepts "Control" in config).
    #[serde(alias = "Control")]
    Ctrl,
    /// Shift key.
    Shift,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Default-value helpers (used by serde attributes on SettingsConfig)
// ═══════════════════════════════════════════════════════════════════════════════

fn default_theme() -> String {
    "grid_tron".to_string()
}

fn default_mouse() -> bool {
    true
}

fn default_window_width() -> u32 {
    1280
}

fn default_window_height() -> u32 {
    800
}

fn default_auto_scroll_edge() -> bool {
    true
}

fn default_always_center_single_column() -> bool {
    false
}

fn default_shell_integration() -> bool {
    true
}

/// Default host terminal scrollback capacity in rows.
///
/// Mirrors `wezterm-term`'s `TerminalConfiguration::scrollback_size()` default
/// so behaviour is unchanged when the user does not set
/// `terminal_scrollback_lines` in `config.toml`.
fn default_terminal_scrollback_lines() -> usize {
    3500
}

fn default_terminal_mouse() -> bool {
    true
}

fn default_terminal_wheel_scroll_lines() -> usize {
    3
}

// ═══════════════════════════════════════════════════════════════════════════════
//  SettingsConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// User-configurable settings that control behaviour and appearance.
///
/// Font family/size configuration has moved out of `[settings]` into the
/// dedicated `[font]` block (see [`crate::font::FontConfig`]). The fields below
/// cover behaviour, window geometry, and terminal color palette overrides only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_mouse")]
    pub mouse: bool,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    /// Optional terminal default foreground override from `config.toml`.
    #[serde(default, alias = "terminal-foreground")]
    pub terminal_foreground: Option<Color>,
    /// Optional terminal default background override from `config.toml`.
    #[serde(default, alias = "terminal-background")]
    pub terminal_background: Option<Color>,
    /// Optional terminal cursor foreground override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-foreground")]
    pub terminal_cursor_foreground: Option<Color>,
    /// Optional terminal cursor background override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-background")]
    pub terminal_cursor_background: Option<Color>,
    /// Optional terminal cursor border override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-border")]
    pub terminal_cursor_border: Option<Color>,
    /// Optional terminal selection foreground override from `config.toml`.
    #[serde(default, alias = "terminal-selection-foreground")]
    pub terminal_selection_foreground: Option<Color>,
    /// Optional terminal selection background override from `config.toml`.
    #[serde(default, alias = "terminal-selection-background")]
    pub terminal_selection_background: Option<Color>,
    /// Optional terminal ANSI `0..7` palette override from `config.toml`.
    #[serde(default, alias = "terminal-ansi")]
    pub terminal_ansi: Option<[Color; 8]>,
    /// Optional terminal bright ANSI `8..15` palette override from `config.toml`.
    #[serde(default, alias = "terminal-brights")]
    pub terminal_brights: Option<[Color; 8]>,
    /// Automatically scroll the workspace view when the pointer hovers near the left/right edge.
    #[serde(default = "default_auto_scroll_edge")]
    pub auto_scroll_edge: bool,
    /// Modifier key that must be held to initiate an interactive pane drag with the mouse.
    #[serde(default)]
    pub interactive_move_modifier: ModifierKey,
    /// Center a single column even when it fits within the viewport.
    #[serde(default = "default_always_center_single_column")]
    pub always_center_single_column: bool,
    /// Auto-inject shell integration snippets for OSC 133/OSC 7 pane runtime signals.
    #[serde(default = "default_shell_integration")]
    pub shell_integration: bool,
    /// Host terminal scrollback capacity in rows.
    ///
    /// This is the number of history rows the terminal engine retains above the
    /// visible viewport. The host scrollback viewport (see `terminal-01a`) scrolls
    /// within `[0, terminal_scrollback_lines]`. Mirrors wezterm-term's default of
    /// 3500 when unset.
    #[serde(default = "default_terminal_scrollback_lines", alias = "terminal-scrollback-lines")]
    pub terminal_scrollback_lines: usize,

    /// Enable terminal mouse support for host scrollback: when `true`, wheel events
    /// scroll the host scrollback viewport instead of forwarding to the terminal
    /// (unless the terminal application has grabbed the mouse). Shift+wheel always
    /// scrolls the host viewport regardless.
    ///
    /// This is terminal-ONLY — it does not affect `settings.mouse` (which controls
    /// chrome interaction like click-to-focus and drag-to-move).
    #[serde(default = "default_terminal_mouse", alias = "terminal-mouse")]
    pub terminal_mouse: bool,

    /// Number of scrollback rows per wheel notch when the host scrollback viewport
    /// is active (i.e. the wheel scrolls the host viewport, not the terminal).
    #[serde(default = "default_terminal_wheel_scroll_lines", alias = "terminal-wheel-scroll-lines")]
    pub terminal_wheel_scroll_lines: usize,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            mouse: default_mouse(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            terminal_foreground: None,
            terminal_background: None,
            terminal_cursor_foreground: None,
            terminal_cursor_background: None,
            terminal_cursor_border: None,
            terminal_selection_foreground: None,
            terminal_selection_background: None,
            terminal_ansi: None,
            terminal_brights: None,
            auto_scroll_edge: default_auto_scroll_edge(),
            interactive_move_modifier: ModifierKey::default(),
            always_center_single_column: default_always_center_single_column(),
            shell_integration: default_shell_integration(),
            terminal_scrollback_lines: default_terminal_scrollback_lines(),
            terminal_mouse: default_terminal_mouse(),
            terminal_wheel_scroll_lines: default_terminal_wheel_scroll_lines(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_config_default_values() {
        let s = SettingsConfig::default();
        assert_eq!(s.theme, "grid_tron");
        assert!(s.mouse);
        assert_eq!(s.window_width, 1280);
        assert_eq!(s.window_height, 800);
        assert_eq!(s.terminal_foreground, None);
        assert_eq!(s.terminal_background, None);
        assert_eq!(s.terminal_cursor_foreground, None);
        assert_eq!(s.terminal_cursor_background, None);
        assert_eq!(s.terminal_cursor_border, None);
        assert_eq!(s.terminal_selection_foreground, None);
        assert_eq!(s.terminal_selection_background, None);
        assert_eq!(s.terminal_ansi, None);
        assert_eq!(s.terminal_brights, None);
        assert!(s.auto_scroll_edge);
        assert_eq!(s.interactive_move_modifier, ModifierKey::Super);
        assert!(!s.always_center_single_column);
        assert!(s.shell_integration);
        assert_eq!(s.terminal_scrollback_lines, 3500);
        assert!(s.terminal_mouse);
        assert_eq!(s.terminal_wheel_scroll_lines, 3);
    }

    #[test]
    fn test_modifier_key_control_alias() {
        #[derive(Deserialize)]
        struct Wrap {
            #[serde(default)]
            m: ModifierKey,
        }

        let ctrl: Wrap = toml::from_str(r#"m = "Control""#).unwrap();
        assert_eq!(ctrl.m, ModifierKey::Ctrl);

        let pascal: Wrap = toml::from_str(r#"m = "Ctrl""#).unwrap();
        assert_eq!(pascal.m, ModifierKey::Ctrl);
    }

    #[test]
    fn test_terminal_foreground_override_parses() {
        let s: SettingsConfig = toml::from_str("terminal-foreground = \"#4c4f69\"").unwrap();
        assert!(s.terminal_foreground.is_some());
    }
}
