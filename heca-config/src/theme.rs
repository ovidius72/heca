pub use crate::keys::{
    BindingValue, CommandKeybindConfig, KeyModeConfig, KeybindingMap, KeysConfig, ModeBindingConfig,
};
pub use crate::loader::{AppConfig, Config, config_dir};
pub use crate::settings::{ModifierKey, SettingsConfig};
pub use heca_theme::{Color, FrameStyle, GlowLevel, Intensity, Shadow, Theme};

/// Load a theme by name.
///
/// Delegates to the unified `heca-theme` loader.
pub fn load(name: &str) -> Theme {
    heca_theme::load_theme(name)
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
        // Focus outline is on (the thin `focus_ring` reads fine on light themes; only glow is off).
        assert!(theme.show_focus_border);
    }
}
