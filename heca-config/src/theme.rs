pub use crate::keys::{
    BindingValue, CommandKeybindConfig, KeybindingMap, KeyModeConfig, KeysConfig,
    ModeBindingConfig,
};
pub use crate::loader::{config_dir, AppConfig, Config};
pub use crate::settings::{ModifierKey, SettingsConfig};
pub use heca_theme::{Color, GlowLevel, Intensity, Shadow, Theme};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyConfigTheme {
    name: String,
    background: Color,
    foreground: Color,
    border: Color,
    accent: Color,
    terminal_font_family: String,
    terminal_font_size: f32,
    border_radius: f32,
    border_width: f32,
    pane_padding: f32,
    shadow: Shadow,
    float_background: Color,
    float_accent: Color,
    float_focus: Color,
}

/// Load a theme by name.
///
/// Transitional compatibility: `heca-theme` still ships `frappe` while the app
/// side still has a bundled `latte`. Until the global `frappe -> latte` swap
/// lands, keep accepting `latte` here by adapting the legacy bundled config
/// theme into the unified `heca_theme::Theme` shape.
pub fn load(name: &str) -> Theme {
    if name.eq_ignore_ascii_case("latte") {
        load_legacy_latte()
    } else {
        heca_theme::load_theme(name)
    }
}

/// Convenience loader for the bundled Catppuccin Mocha theme.
///
/// Equivalent to `load("mocha")`.
pub fn catppuccin_mocha() -> Theme {
    load("mocha")
}

/// Convenience loader for the temporary Catppuccin Latte compatibility theme.
///
/// Equivalent to `load("latte")`.
pub fn catppuccin_latte() -> Theme {
    load("latte")
}

fn load_legacy_latte() -> Theme {
    let legacy: LegacyConfigTheme = toml::from_str(include_str!("themes/latte.toml"))
        .expect("legacy latte theme should parse into compatibility loader");
    // FIXME(phase-3-frappe-swap): remove this adapter once `heca-theme`
    // ships `latte` directly and active `frappe` references are replaced.
    // Start from the current shipped light preset so `latte` does not become a
    // broken dark/light hybrid during the transition.
    let mut theme = heca_theme::load_theme("frappe");
    theme.name = legacy.name;
    theme.background = legacy.background;
    theme.foreground = legacy.foreground;
    theme.border = legacy.border;
    theme.accent = legacy.accent;
    theme.glow = legacy.accent;
    theme.terminal_font_family = legacy.terminal_font_family;
    theme.terminal_font_size = legacy.terminal_font_size;
    theme.border_radius = legacy.border_radius;
    theme.border_width = legacy.border_width;
    theme.pane_padding = legacy.pane_padding;
    theme.left_sidebar_background = None;
    theme.right_sidebar_background = None;
    theme.top_bottom_pane_background = None;
    theme.shadow = legacy.shadow;
    theme.float_background = legacy.float_background;
    theme.float_accent = legacy.float_accent;
    theme.float_focus = legacy.float_focus;
    theme
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme_is_grid_tron() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Grid Tron");
    }

    #[test]
    fn test_bundled_mocha_theme() {
        assert_eq!(load("mocha").name, "Catppuccin Mocha");
    }

    #[test]
    fn test_bundled_latte_theme() {
        assert_eq!(load("latte").name, "Catppuccin Latte");
    }

    #[test]
    fn legacy_latte_compat_is_a_light_theme_not_a_grid_tron_hybrid() {
        let theme = load("latte");
        assert_eq!(theme.name, "Catppuccin Latte");
        assert_eq!(theme.background, Color::new(239, 241, 245, 255));
        assert_eq!(theme.surface, Color::new(230, 233, 239, 255));
        assert_eq!(theme.foreground, Color::new(76, 79, 105, 255));
        assert_eq!(theme.muted, Color::new(156, 160, 176, 255));
        assert_eq!(theme.danger, Color::new(210, 15, 57, 255));
        assert_eq!(theme.success, Color::new(64, 160, 43, 255));
        assert_eq!(theme.warning, Color::new(223, 142, 29, 255));
        assert_eq!(theme.glow_size, heca_theme::GlowLevel::None);
        assert_eq!(theme.intensity, heca_theme::Intensity::Off);
        assert!(!theme.show_focus_border);
    }

    #[test]
    fn legacy_latte_parser_rejects_unknown_fields() {
        let err = toml::from_str::<LegacyConfigTheme>(
            r##"
name = "Catppuccin Latte"
background = "#eff1f5"
foreground = "#4c4f69"
border = "#ccd0da"
accent = "#1e66f5"
terminal_font_family = "Maple Mono Normal NF"
terminal_font_size = 14.0
border_radius = 6.0
border_width = 1.0
pane_padding = 4.0
shadow = { color = "#000000", alpha = 0.15, blur = 8.0 }
float_background = "#ccd0da"
float_accent = "#1e66f5"
float_focus = "#e67e22"
extra_field = true
"##,
        )
        .err()
        .expect("legacy latte parser should reject unknown fields");
        assert!(err.to_string().contains("unknown field"));
    }
}
