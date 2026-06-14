use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  Vibrancy
// ═══════════════════════════════════════════════════════════════════════════════

/// macOS `NSVisualEffectMaterial` variants (and a best-effort mapping for
/// Windows Acrylic / Mica). On Linux this is a no-op.
///
/// Consumed by the window/surface layer in F2 when `blur = true`. Each variant
/// name is serialised as `snake_case` in TOML (e.g. `vibrancy = "hud_window"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vibrancy {
    /// `NSVisualEffectMaterialSidebar` — sidebar/panel tint (default).
    #[default]
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

fn default_transparent() -> bool {
    false
}

fn default_opacity() -> f32 {
    0.95
}

fn default_pane_opacity() -> f32 {
    1.0
}

fn default_chrome_opacity() -> f32 {
    0.92
}

fn default_blur() -> bool {
    false
}

fn default_vibrancy() -> Vibrancy {
    Vibrancy::default()
}

fn default_blur_amount() -> f32 {
    12.0
}

// ═══════════════════════════════════════════════════════════════════════════════
//  AppearanceConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// Read-only appearance contract shared by all rendering layers.
///
/// Every consumer reads this struct — none owns it:
/// - Window/surface layer reads `transparent`, `blur`, and `vibrancy` (F2).
/// - In-app blur pass reads `blur_amount` (F3).
/// - Chrome (sidebar/status) reads `chrome_opacity`.
/// - Terminal pane reads `pane_opacity`.
/// - The global opacity (`opacity`) is applied to the whole app background.
///
/// All fields are `Copy` (primitives + `Vibrancy` enum) so the struct is cheap
/// to pass by value across the render tree. Missing `[appearance]` sections in
/// the TOML config are filled with these defaults.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Master switch: enable transparent surface and alpha-channel clears.
    /// When `false` all other transparency fields are effectively ignored by
    /// the compositor.
    #[serde(default = "default_transparent")]
    pub transparent: bool,

    /// Global app/background opacity in the range `0.0..=1.0`.
    #[serde(default = "default_opacity")]
    pub opacity: f32,

    /// Pane content background opacity in the range `0.0..=1.0`.
    /// The terminal renderer reads this to tint its background quads.
    #[serde(default = "default_pane_opacity")]
    pub pane_opacity: f32,

    /// Sidebar/status-bar (chrome) background opacity in the range `0.0..=1.0`.
    #[serde(default = "default_chrome_opacity")]
    pub chrome_opacity: f32,

    /// Enable OS-level backdrop blur (macOS Vibrancy / Windows Acrylic).
    /// Requires `transparent = true` to have a visible effect.
    #[serde(default = "default_blur")]
    pub blur: bool,

    /// macOS NSVisualEffectMaterial variant (or Windows Acrylic flavour) used
    /// when `blur = true`. Linux ignores this field.
    #[serde(default = "default_vibrancy")]
    pub vibrancy: Vibrancy,

    /// In-app (shader/software) blur radius in logical pixels (F3).
    /// This is separate from OS backdrop blur and works on all platforms.
    /// Ignored when the in-app blur pass is disabled.
    #[serde(default = "default_blur_amount")]
    pub blur_amount: f32,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            transparent: default_transparent(),
            opacity: default_opacity(),
            pane_opacity: default_pane_opacity(),
            chrome_opacity: default_chrome_opacity(),
            blur: default_blur(),
            vibrancy: default_vibrancy(),
            blur_amount: default_blur_amount(),
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
    fn defaults_match_schema() {
        let cfg = AppearanceConfig::default();
        assert!(!cfg.transparent);
        assert!((cfg.opacity - 0.95).abs() < f32::EPSILON);
        assert!((cfg.pane_opacity - 1.0).abs() < f32::EPSILON);
        assert!((cfg.chrome_opacity - 0.92).abs() < f32::EPSILON);
        assert!(!cfg.blur);
        assert_eq!(cfg.vibrancy, Vibrancy::Sidebar);
        assert!((cfg.blur_amount - 12.0).abs() < f32::EPSILON);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let cfg: AppearanceConfig = toml::from_str(
            r#"
transparent = true
opacity = 0.5
"#,
        )
        .expect("partial appearance toml should parse");

        assert!(cfg.transparent);
        assert!((cfg.opacity - 0.5).abs() < f32::EPSILON);
        // All other fields should fall back to defaults.
        assert!((cfg.pane_opacity - 1.0).abs() < f32::EPSILON);
        assert!((cfg.chrome_opacity - 0.92).abs() < f32::EPSILON);
        assert!(!cfg.blur);
        assert_eq!(cfg.vibrancy, Vibrancy::Sidebar);
        assert!((cfg.blur_amount - 12.0).abs() < f32::EPSILON);
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
    }

    #[test]
    fn config_without_appearance_section_uses_defaults() {
        use crate::loader::Config;

        // Minimal valid config — no [appearance] table at all.
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
