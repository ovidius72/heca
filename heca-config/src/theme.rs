pub use crate::keys::{
    BindingValue, CommandKeybindConfig, KeybindingMap, KeyModeConfig, KeysConfig,
    ModeBindingConfig,
};
pub use crate::loader::{config_dir, AppConfig, Config};
pub use crate::settings::{ModifierKey, SettingsConfig};
pub use heca_theme::{Color, GlowLevel, Intensity, Shadow, Theme};

/// Load a theme by name.
///
/// Delegates to the unified `heca-theme` loader.
pub fn load(name: &str) -> Theme {
    heca_theme::load_theme(name)
}

/// Convenience loader for the bundled Catppuccin Mocha theme.
///
/// Equivalent to `load("mocha")`.
pub fn catppuccin_mocha() -> Theme {
    load("mocha")
}

/// Convenience loader for the bundled Catppuccin Latte theme.
///
/// Equivalent to `load("latte")`.
pub fn catppuccin_latte() -> Theme {
    load("latte")
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
    fn bundled_latte_is_a_light_theme() {
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
}
