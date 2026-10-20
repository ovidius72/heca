use super::super::*;

#[test]
fn partial_toml_fills_defaults() {
    let cfg: AppearanceConfig =
        toml::from_str("transparency = 30\n[terminal]\ntransparency = 15\n")
            .expect("partial appearance toml should parse");
    assert_eq!(cfg.transparency, 30);
    assert_eq!(cfg.blur, 0);
    assert_eq!(cfg.terminal.transparency, 15);
    assert_eq!(cfg.terminal.show_scrollbar, ScrollbarVisibility::WhenNeeded);
    assert!(cfg.terminal.show_scrolled_up_badge);
    assert_eq!(cfg.vibrancy, Vibrancy::None);
}

#[test]
fn terminal_scrollbar_visibility_parses_snake_case() {
    let cfg: AppearanceConfig = toml::from_str("[terminal]\nshow_scrollbar = \"always\"\n")
        .expect("terminal show_scrollbar should parse");
    assert_eq!(cfg.terminal.show_scrollbar, ScrollbarVisibility::Always);

    let cfg: AppearanceConfig = toml::from_str("[terminal]\nshow_scrollbar = \"never\"\n")
        .expect("terminal show_scrollbar should parse never");
    assert_eq!(cfg.terminal.show_scrollbar, ScrollbarVisibility::Never);
}

#[test]
fn terminal_scrolled_up_badge_setting_parses() {
    let cfg: AppearanceConfig = toml::from_str("[terminal]\nshow_scrolled_up_badge = false\n")
        .expect("terminal show_scrolled_up_badge should parse");
    assert!(!cfg.terminal.show_scrolled_up_badge);

    let cfg: AppearanceConfig = toml::from_str("[terminal]\nshow_scrolled_up_badge = true\n")
        .expect("terminal show_scrolled_up_badge should parse true");
    assert!(cfg.terminal.show_scrolled_up_badge);
}

#[test]
fn vibrancy_parses_snake_case() {
    #[derive(Deserialize)]
    struct Wrapper {
        vibrancy: Vibrancy,
    }

    let w: Wrapper = toml::from_str(r#"vibrancy = "hud_window""#)
        .expect("hud_window should parse to Vibrancy::HudWindow");
    assert_eq!(w.vibrancy, Vibrancy::HudWindow);
    let n: Wrapper =
        toml::from_str(r#"vibrancy = "none""#).expect("none should parse to Vibrancy::None");
    assert_eq!(n.vibrancy, Vibrancy::None);
}

#[test]
fn config_without_appearance_section_uses_defaults() {
    use crate::loader::Config;

    let cfg: Config = toml::from_str(
        r#"
[settings]
theme = "mocha"
"#,
    )
    .expect("config without [appearance] should parse");

    assert_eq!(cfg.appearance, AppearanceConfig::default());
}
