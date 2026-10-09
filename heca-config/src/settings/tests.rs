//! Tests for [`super`].

use super::*;

#[test]
fn test_settings_config_default_values() {
    let s = SettingsConfig::default();
    assert_eq!(s.theme, "grid_tron");
    assert!(s.mouse);
    let file = super::default_tests::embedded_window_defaults();
    assert_eq!((s.window_width, s.window_height), file);
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
    assert_eq!(s.edge_scroll_distance, 80.0);
    assert_eq!(s.interactive_move_modifier, ModifierKey::Super);
    assert_eq!(s.column_focus, ColumnFocus::Last);
    assert_eq!(s.swap_modifier, ModifierKey::Shift);
    assert!(!s.always_center_single_column);
    assert!(s.shell_integration);
    assert_eq!(s.terminal_scrollback_lines, 3500);
    assert!(s.terminal_mouse);
    assert_eq!(s.terminal_wheel_scroll_lines, 3);
    assert!(s.terminal_scroll_animations);
    // Chrome regions are all shown by default.
    assert!(s.show_left_sidebar);
    assert!(s.show_right_sidebar);
    assert!(s.show_top_bar);
    assert!(s.show_bottom_bar);
    // Destructive-action confirmation now lives in the `[confirm]` table (see confirm.rs).
}

#[test]
fn test_show_chrome_region_toggles_parse() {
    let s: SettingsConfig = toml::from_str(
        "show_left_sidebar = false\n\
         show_right_sidebar = false\n\
         show_top_bar = false\n\
         show_bottom_bar = false\n",
    )
    .expect("chrome region toggles should parse");
    assert!(!s.show_left_sidebar);
    assert!(!s.show_right_sidebar);
    assert!(!s.show_top_bar);
    assert!(!s.show_bottom_bar);

    // Kebab-case aliases parse too.
    let k: SettingsConfig =
        toml::from_str("show-top-bar = false\n").expect("kebab alias should parse");
    assert!(!k.show_top_bar);
    assert!(k.show_bottom_bar);
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

/// **The stack's size is config, not a compiled-in number**, and it clamps.
///
/// A zero would queue every notification for ever and show none — that is what `mode = "none"`
/// is for, so a zero here is a typo rather than a way to silence the app. The clamp itself
/// lives in the store; this pins that the value travels.
#[test]
fn notification_max_visible_defaults_to_five_and_is_overridable() {
    let s = SettingsConfig::default();
    assert_eq!(s.notification_system.max_visible, 5);

    let s: SettingsConfig =
        toml::from_str("[notification_system]\nmax_visible = 3\n").expect("parses");
    assert_eq!(s.notification_system.max_visible, 3);

    // Absent from a present subtable → still the default.
    let s: SettingsConfig =
        toml::from_str("[notification_system]\nmode = \"app\"\n").expect("parses");
    assert_eq!(s.notification_system.max_visible, 5);
}

#[test]
fn notification_max_lines_defaults_to_eight_and_is_overridable() {
    let s = SettingsConfig::default();
    assert_eq!(s.notification_system.max_lines, 8);

    let s: SettingsConfig =
        toml::from_str("[notification_system]\nmax_lines = 3\n").expect("parses");
    assert_eq!(s.notification_system.max_lines, 3);

    let s: SettingsConfig =
        toml::from_str("[notification_system]\nmax_visible = 2\n").expect("parses");
    assert_eq!(s.notification_system.max_lines, 8);
}

#[test]
fn test_notification_system_defaults_and_override() {
    // Absent section → default mode app, 4 s.
    let s = SettingsConfig::default();
    assert_eq!(s.notification_system.mode, NotificationSystem::App);
    assert_eq!(s.notification_system.auto_dismiss_ms, 4000);

    // `[settings.notification_system]` as a subtable overrides both.
    let s: SettingsConfig =
        toml::from_str("[notification_system]\nmode = \"none\"\nauto_dismiss_ms = 8000\n")
            .expect("notification_system subtable should parse");
    assert_eq!(s.notification_system.mode, NotificationSystem::None);
    assert_eq!(s.notification_system.auto_dismiss_ms, 8000);

    // `system` parses (reserved — the host falls it back to app).
    let s: SettingsConfig =
        toml::from_str("[notification_system]\nmode = \"system\"\n").unwrap();
    assert_eq!(s.notification_system.mode, NotificationSystem::System);

    // The subtable is optional; unrelated settings still parse without it.
    let s: SettingsConfig = toml::from_str("mouse = false\n").unwrap();
    assert_eq!(s.notification_system.mode, NotificationSystem::App);
    assert_eq!(s.notification_system.auto_dismiss_ms, 4000);
}

/// `column_focus` reads as the two words a user writes, and a missing key keeps today's
/// behaviour.
#[test]
fn column_focus_reads_last_and_row_and_defaults_to_last() {
    let read = |text: &str| toml::from_str::<SettingsConfig>(text).map(|s| s.column_focus);
    assert_eq!(read("").ok(), Some(ColumnFocus::Last));
    assert_eq!(read("column_focus = \"last\"").ok(), Some(ColumnFocus::Last));
    assert_eq!(read("column_focus = \"row\"").ok(), Some(ColumnFocus::Row));
    assert!(read("column_focus = \"diagonal\"").is_err());
}
