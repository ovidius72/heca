use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  Vibrancy
// ═══════════════════════════════════════════════════════════════════════════════

/// OS backdrop-blur material (the *desktop-behind-the-window* frost).
///
/// This is **not portable and not numeric**: macOS maps each variant to an
/// `NSVisualEffectMaterial`; Windows uses Acrylic/Mica (best-effort); Linux is a
/// no-op. The window/surface layer applies it once after window creation when it
/// is not [`Vibrancy::None`]. For a *portable, numeric* blur, use the in-app blur
/// amount instead (`AppearanceConfig::blur`). Serialised `snake_case` in TOML
/// (e.g. `vibrancy = "hud_window"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vibrancy {
    /// No OS backdrop material (default) — disables vibrancy entirely.
    #[default]
    None,
    /// `NSVisualEffectMaterialSidebar` — sidebar/panel tint.
    Sidebar,
    /// `NSVisualEffectMaterialHUDWindow` — dark floating HUD panel.
    HudWindow,
    /// `NSVisualEffectMaterialUnderWindowBackground` — blurred desktop behind the window.
    UnderWindowBackground,
    /// `NSVisualEffectMaterialPopover` — popover / tooltip material.
    Popover,
    /// `NSVisualEffectMaterialMenu` — menu bar / dropdown material.
    Menu,
    /// `NSVisualEffectMaterialFullScreenUI` — full-screen overlay material.
    FullScreenUi,
    /// `NSVisualEffectMaterialWindowBackground` — generic window background.
    WindowBackground,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Default-value helpers (used by serde attributes on AppearanceConfig)
// ═══════════════════════════════════════════════════════════════════════════════

fn default_transparency() -> u8 {
    0
}

fn default_blur() -> u8 {
    0
}

fn default_vibrancy() -> Vibrancy {
    Vibrancy::None
}

/// Maximum in-app blur radius in logical px, at `blur = 100`.
const MAX_BLUR_PX: f32 = 24.0;

// ═══════════════════════════════════════════════════════════════════════════════
//  AppearanceConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// Read-only appearance contract shared by all rendering layers — none owns it.
///
/// Three intuitive, cross-platform controls (all amounts are `0..=100`):
/// - `transparency` — how see-through the app is (`0` opaque, `100` fully
///   transparent). Portable (window/surface alpha).
/// - `blur` — the **in-app** frosted-glass blur amount behind translucent panels
///   (palette/sidebar). Portable (our own GPU pass, F3). `0` = off.
/// - `vibrancy` — the **OS backdrop** material (blurs the desktop *behind* the
///   window). Not numeric, not portable: macOS materials, Windows acrylic, Linux
///   no-op. [`Vibrancy::None`] = off.
///
/// All fields are `Copy`. Missing `[appearance]` sections fall back to these
/// defaults (everything off → identical to an opaque app).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Window/app transparency amount, `0..=100` (`0` opaque, `100` see-through).
    #[serde(default = "default_transparency")]
    pub transparency: u8,

    /// In-app frosted blur amount, `0..=100` (`0` = off). Portable GPU pass (F3);
    /// distinct from the OS `vibrancy` backdrop.
    #[serde(default = "default_blur")]
    pub blur: u8,

    /// OS backdrop material ([`Vibrancy::None`] = off). Platform-dependent.
    #[serde(default = "default_vibrancy")]
    pub vibrancy: Vibrancy,
}

impl AppearanceConfig {
    /// Background opacity in `0.0..=1.0` (`transparency = 0` → `1.0` opaque).
    pub fn opacity(&self) -> f32 {
        1.0 - (self.transparency.min(100) as f32) / 100.0
    }

    /// Whether the window/surface should be created transparent.
    pub fn is_transparent(&self) -> bool {
        self.transparency > 0
    }

    /// Chrome panel (sidebar/status/tab) background opacity. A bit more
    /// see-through than the global `opacity()` so panels read as *frosted*
    /// (vibrancy showing through) rather than a flat dark tint. `1.0` (opaque)
    /// when the window isn't transparent.
    pub fn chrome_opacity(&self) -> f32 {
        if self.is_transparent() {
            (self.opacity() * 0.7).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// In-app blur radius in logical px (`0.0` = off). Scales `blur` 0..100 to
    /// `0..=MAX_BLUR_PX`. Read by the in-app blur pass (F3).
    pub fn blur_radius(&self) -> f32 {
        (self.blur.min(100) as f32) / 100.0 * MAX_BLUR_PX
    }

    /// The OS backdrop material to apply, or `None` when disabled.
    pub fn os_vibrancy(&self) -> Option<Vibrancy> {
        (self.vibrancy != Vibrancy::None).then_some(self.vibrancy)
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            transparency: default_transparency(),
            blur: default_blur(),
            vibrancy: default_vibrancy(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_all_off() {
        let cfg = AppearanceConfig::default();
        assert_eq!(cfg.transparency, 0);
        assert_eq!(cfg.blur, 0);
        assert_eq!(cfg.vibrancy, Vibrancy::None);
        assert!(!cfg.is_transparent());
        assert!((cfg.opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.blur_radius()).abs() < f32::EPSILON);
        assert_eq!(cfg.os_vibrancy(), None);
    }

    #[test]
    fn amounts_map_to_derived_values() {
        let cfg = AppearanceConfig {
            transparency: 25,
            blur: 50,
            vibrancy: Vibrancy::Sidebar,
        };
        assert!((cfg.opacity() - 0.75).abs() < 1e-6);
        assert!(cfg.is_transparent());
        assert!((cfg.blur_radius() - 12.0).abs() < 1e-6); // 50% of 24px
        assert_eq!(cfg.os_vibrancy(), Some(Vibrancy::Sidebar));
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let cfg: AppearanceConfig = toml::from_str("transparency = 30\n")
            .expect("partial appearance toml should parse");
        assert_eq!(cfg.transparency, 30);
        assert_eq!(cfg.blur, 0);
        assert_eq!(cfg.vibrancy, Vibrancy::None);
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
}
