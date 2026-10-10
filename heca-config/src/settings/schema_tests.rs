//! New fields need no helper registration; serde still owns input validation.

use super::{defaults, schema::declare_settings, SettingsConfig};
use serde::{Deserialize, Serialize};

declare_settings! {
    defaults: [];

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct ExtendedSettings {
        window_width: u32,
        #[serde(alias = "terminal-mouse")]
        terminal_mouse: bool,
        new_items: Option<Vec<String>>,
    }
}

#[test]
fn a_new_optional_type_needs_only_its_field_declaration() {
    let file_defaults = SettingsConfig::default();
    let empty: ExtendedSettings = toml::from_str("").unwrap();
    assert_eq!(empty.window_width, file_defaults.window_width);
    assert_eq!(empty.terminal_mouse, file_defaults.terminal_mouse);
    assert_eq!(empty.new_items, None);

    let explicit: ExtendedSettings = toml::from_str(
        "window_width = 0\nterminal-mouse = false\nnew_items = ['first', 'second']",
    ).unwrap();
    assert_eq!(explicit.window_width, 0);
    assert!(!explicit.terminal_mouse);
    assert_eq!(explicit.new_items, Some(vec!["first".into(), "second".into()]));
    let round_trip: ExtendedSettings = toml::from_str(&toml::to_string(&explicit).unwrap()).unwrap();
    assert_eq!(round_trip, explicit);
}

#[test]
fn explicit_null_clears_an_optional_file_default() {
    declare_settings! {
        defaults: [];

        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct OptionalDefault {
            window_width: Option<u32>,
        }
    }

    let empty: OptionalDefault = serde_json::from_str("{}").unwrap();
    assert_eq!(empty.window_width, Some(SettingsConfig::default().window_width));
    let cleared: OptionalDefault = serde_json::from_str(r#"{"window_width":null}"#).unwrap();
    assert_eq!(cleared.window_width, None);
}

#[test]
fn json_round_trip_and_validation_keep_the_existing_contract() {
    let expected = SettingsConfig::default();
    let actual: SettingsConfig = serde_json::from_value(serde_json::to_value(&expected).unwrap()).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(serde_json::from_str::<SettingsConfig>("{}").unwrap(), expected);
    for input in [
        r#"{"window_width":null}"#,
        r#"{"notification_system":null}"#,
        r#"{"notification_system":{"max_visible":null}}"#,
        r#"{"terminal_mouse":true,"terminal-mouse":false}"#,
    ] {
        assert!(serde_json::from_str::<SettingsConfig>(input).is_err(), "{input}");
    }
    assert!(toml::from_str::<SettingsConfig>(
        "terminal_mouse = true\nterminal-mouse = false",
    ).is_err());
}

#[test]
#[should_panic(expected = "embedded config.default.toml is missing settings.new_required_setting")]
fn a_missing_required_file_default_is_reported() {
    defaults::field::<u32>(&["new_required_setting"]);
}

#[test]
#[should_panic(expected = "invalid embedded settings.theme")]
fn an_incompatible_file_default_is_reported() {
    defaults::field::<u32>(&["theme"]);
}
