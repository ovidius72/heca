//! Defaults stay in agreement across every settings loading path.

use super::{NotificationSystemConfig, SettingsConfig};
use crate::loader::{Config, config_from_layers};

fn embedded_settings() -> toml::Value {
    let config: toml::Value = toml::from_str(crate::loader::CONFIG_DEFAULT).unwrap();
    config.get("settings").unwrap().clone()
}

pub(super) fn embedded_window_defaults() -> (u32, u32) {
    let settings = embedded_settings();
    (settings.get("window_width").unwrap().clone().try_into().unwrap(),
     settings.get("window_height").unwrap().clone().try_into().unwrap())
}

#[test]
fn constructors_and_empty_files_use_the_embedded_settings() {
    let expected: SettingsConfig = embedded_settings().try_into().unwrap();
    assert_eq!(SettingsConfig::default(), expected);
    assert_eq!(toml::from_str::<SettingsConfig>("").unwrap(), expected);
    assert_eq!(Config::default().settings, expected);
    assert_eq!(config_from_layers("").unwrap().settings, expected);
    assert_eq!(config_from_layers("[settings]").unwrap().settings, expected);

    let notifications: NotificationSystemConfig = embedded_settings()
        .get("notification_system").unwrap().clone().try_into().unwrap();
    assert_eq!(NotificationSystemConfig::default(), notifications);
    assert_eq!(toml::from_str::<NotificationSystemConfig>("").unwrap(), notifications);
}

#[test]
fn each_missing_setting_keeps_its_embedded_default() {
    let original = embedded_settings();
    let expected: SettingsConfig = original.clone().try_into().unwrap();
    for name in original.as_table().unwrap().keys() {
        let mut partial = original.clone();
        partial.as_table_mut().unwrap().remove(name);
        let actual: SettingsConfig = partial.try_into().unwrap();
        assert_eq!(actual, expected, "missing settings.{name}");
    }
    let notifications = original.get("notification_system").unwrap();
    for name in notifications.as_table().unwrap().keys() {
        let mut partial = original.clone();
        partial.get_mut("notification_system").unwrap()
            .as_table_mut().unwrap().remove(name);
        let actual: SettingsConfig = partial.try_into().unwrap();
        assert_eq!(actual, expected, "missing settings.notification_system.{name}");
    }
}

#[test]
fn window_overrides_keep_other_file_defaults() {
    let mut expected = SettingsConfig {
        window_width: 1600,
        ..SettingsConfig::default()
    };
    let direct: SettingsConfig = toml::from_str("window_width = 1600").unwrap();
    let layered = config_from_layers("[settings]\nwindow_width = 1600").unwrap();
    assert_eq!(direct, expected);
    assert_eq!(layered.settings, expected);

    // Presence matters: zero and false are explicit values, not requests for defaults.
    expected.window_width = 0;
    expected.mouse = false;
    let direct: SettingsConfig = toml::from_str("window_width = 0\nmouse = false").unwrap();
    let layered = config_from_layers("[settings]\nwindow_width = 0\nmouse = false").unwrap();
    assert_eq!(direct, expected);
    assert_eq!(layered.settings, expected);
}

#[test]
fn aliases_and_nested_overrides_keep_the_other_defaults() {
    let expected = SettingsConfig {
        terminal_wheel_scroll_lines: 7,
        notification_system: NotificationSystemConfig {
            max_visible: 2,
            ..NotificationSystemConfig::default()
        },
        ..SettingsConfig::default()
    };
    let direct: SettingsConfig = toml::from_str(
        "terminal-wheel-scroll-lines = 7\n[notification-system]\nmax_visible = 2",
    ).unwrap();
    assert_eq!(direct, expected);
}

#[test]
fn invalid_explicit_settings_remain_errors() {
    for value in ["window_width = 'wide'", "window_height = -1", "mouse = 'yes'",
                  "[notification_system]\nmax_visible = 'many'"] {
        assert!(toml::from_str::<SettingsConfig>(value).is_err(), "{value}");
        let layered = if value.starts_with('[') {
            value.replacen("[notification_system]", "[settings.notification_system]", 1)
        } else {
            format!("[settings]\n{value}")
        };
        assert!(config_from_layers(&layered).is_err(), "{value}");
    }
}
