use crate::color::Color;
use crate::theme::Theme;
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

/// How a pane's top-border title (icon + program name) is drawn. Serialised
/// `snake_case` in TOML (e.g. `pane_title_style = "filled"`). Mirrors the
/// `heca-grid-ui` `PaneTitleStyle` widget variant; mapped to it in the app.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneTitleStyle {
    /// No pane title at all.
    None,
    /// Float the title in a gap cut into the frame line — no visible box (default).
    #[default]
    Cut,
    /// A solid chip filled with the frame color; title text flips to the interior.
    Filled,
    /// A small bordered box (interior fill + frame-colored border) on the line.
    Boxed,
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

fn default_terminal_transparency() -> u8 {
    0
}

fn default_terminal_blur() -> u8 {
    0
}

fn default_terminal_floating_transparency() -> u8 {
    0
}

fn default_terminal_floating_blur() -> u8 {
    0
}

fn default_vibrancy() -> Vibrancy {
    Vibrancy::None
}

fn default_pane_title_style() -> PaneTitleStyle {
    PaneTitleStyle::Cut
}

/// Maximum in-app blur radius in logical px, at `blur = 100`.
const MAX_BLUR_PX: f32 = 48.0;

// ═══════════════════════════════════════════════════════════════════════════════
//  AppearanceConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// Read-only appearance contract shared by all rendering layers — none owns it.
///
/// Four intuitive, cross-platform controls:
/// - `transparency` — how see-through the app is (`0` opaque, `100` fully
///   transparent). Portable (window/surface alpha).
/// - `blur` — the **in-app** frosted-glass blur amount behind translucent panels
///   (palette/sidebar). Portable (our own GPU pass, F3). `0` = off.
/// - `vibrancy` — the **OS backdrop** material (blurs the desktop *behind* the
///   window). Not numeric, not portable: macOS materials, Windows acrylic, Linux
///   no-op. [`Vibrancy::None`] = off.
/// - `terminal_transparency` / `terminal_blur` — terminal-pane surface controls,
///   independent from the global chrome/window knobs above.
/// - pane chrome — border width, colors, radius, gap. These override the theme
///   when set; leaving them unset inherits from the theme automatically.
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

    /// Terminal pane transparency amount, `0..=100` (`0` opaque, `100`
    /// see-through). Independent from the global window/chrome transparency.
    #[serde(default = "default_terminal_transparency")]
    pub terminal_transparency: u8,

    /// Terminal pane in-app blur amount, `0..=100` (`0` = off). Independent from
    /// the global chrome/window blur amount.
    #[serde(default = "default_terminal_blur")]
    pub terminal_blur: u8,

    /// Floating terminal pane transparency amount, `0..=100` (`0` opaque, `100`
    /// see-through). Independent from the tiled `terminal_transparency` so
    /// floating panes can stay readable (opaque) while tiled panes are frosted.
    /// Default `0` (opaque) — floating panes are solid windows.
    #[serde(default = "default_terminal_floating_transparency")]
    pub terminal_floating_transparency: u8,

    /// Floating terminal pane frosted-tint strength, `0..=100` (`0` = off).
    /// Independent from the tiled `terminal_blur`. Default `0` (no frost —
    /// floating panes are solid); set > 0 to frost floating panes too.
    #[serde(default = "default_terminal_floating_blur")]
    pub terminal_floating_blur: u8,

    /// OS backdrop material ([`Vibrancy::None`] = off). Platform-dependent.
    #[serde(default = "default_vibrancy")]
    pub vibrancy: Vibrancy,

    // ── Pane chrome ──
    // All default to `None` (= inherit from theme). Set explicitly in
    // config.toml to override the theme-derived value.
    /// Pane border stroke width (logical px). `None` → inherits `theme.border_width`.
    #[serde(default)]
    pub pane_border_width: Option<f32>,
    /// Pane border color for inactive/unfocused panes. `None` → inherits `theme.border` at 50% alpha.
    #[serde(default)]
    pub pane_border_color: Option<Color>,
    /// Pane corner radius. `None` → inherits `theme.border_radius`.
    #[serde(default)]
    pub pane_border_radius: Option<f32>,
    /// Pane border color when focused/active. `None` → inherits `theme.accent`.
    #[serde(default)]
    pub pane_active_border_color: Option<Color>,
    /// Pane border color for floating panes (applies to all floating panes,
    /// active or inactive, so they read as a distinct layer). `None` → inherits
    /// `theme.float_accent`. Set to distinguish floating panes from tiled ones.
    #[serde(default)]
    pub pane_floating_border_color: Option<Color>,
    /// Gap between panes (logical px). `None` → 8.0 (built-in layout default).
    #[serde(default)]
    pub pane_gap: Option<f32>,
    /// Internal padding inside panes (logical px). `None` → 4.0; clamped to `[0, 20]`.
    #[serde(default)]
    pub pane_padding: Option<f32>,
    /// Gap between sidebar and content area (logical px). `None` → 12.0.
    #[serde(default)]
    pub sidebar_gap: Option<f32>,
    /// Frosted tint color stamped behind translucent terminals (the
    /// `terminal_blur` frost). `None` → inherits `theme.terminal_frost_color`,
    /// then `theme.background`. Set in config.toml to customize the frosted-glass
    /// tint (e.g. a lifted surface color for a lighter frost).
    #[serde(default)]
    pub terminal_frost_color: Option<Color>,

    // ── Pane title (program icon + name on the top border) ──
    /// How the pane title is drawn: `"none"`, `"cut"` (default), `"filled"`, or
    /// `"boxed"`. `"none"` hides the title entirely.
    #[serde(default = "default_pane_title_style")]
    pub pane_title_style: PaneTitleStyle,
    /// Title **frame** color (the `cut`/`boxed` icon+text, the `filled` chip).
    /// `None` → inherits the pane border color.
    #[serde(default)]
    pub pane_title_color: Option<Color>,
    /// Title **interior** color (the `cut` below-edge half, the `boxed` fill, the
    /// `filled` icon+text). `None` → inherits the terminal's resolved background.
    #[serde(default)]
    pub pane_title_background: Option<Color>,
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

    /// Terminal pane surface opacity in `0.0..=1.0` (`terminal_transparency = 0`
    /// → `1.0` opaque).
    pub fn terminal_opacity(&self) -> f32 {
        1.0 - (self.terminal_transparency.min(100) as f32) / 100.0
    }

    /// Terminal-pane in-app blur radius in logical px (`0.0` = off).
    pub fn terminal_blur_radius(&self) -> f32 {
        (self.terminal_blur.min(100) as f32) / 100.0 * MAX_BLUR_PX
    }

    /// Terminal-pane frosted-tint opacity (`0.0` = invisible, `1.0` = full
    /// frost), driven by `terminal_blur` via a **perceptual sqrt curve**
    /// (0..100 → 0..1) so low blur values show visible frost sooner (blur 10
    /// → ~0.316, 50 → ~0.707, 100 → 1.0). This is the *strength* of the frosted
    /// tint stamped behind a translucent terminal surface; it is **independent
    /// of `terminal_transparency`** (which controls how see-through the terminal
    /// surface itself is). The frost only draws when the surface is translucent
    /// (`terminal_opacity() < 1.0`) AND `terminal_blur > 0` (see `render_frame`).
    /// Decoupled so `terminal_blur` 0→100 produces a visibly monotonic frost
    /// instead of being swamped by a fixed-low surface alpha (the previous bug:
    /// the stamp used `surface_alpha`, so `terminal_transparency=85` pinned the
    /// frost to 0.15 regardless of blur).
    pub fn terminal_frost_opacity(&self) -> f32 {
        ((self.terminal_blur.min(100) as f32) / 100.0).sqrt()
    }

    /// Floating terminal-pane surface opacity in `0.0..=1.0`
    /// (`terminal_floating_transparency = 0` → `1.0` opaque). Independent from
    /// the tiled `terminal_opacity()` so floating panes can stay readable.
    pub fn terminal_floating_opacity(&self) -> f32 {
        1.0 - (self.terminal_floating_transparency.min(100) as f32) / 100.0
    }

    /// Floating terminal-pane frosted-tint blur radius in logical px (`0.0` =
    /// off). Independent from the tiled `terminal_blur_radius()`.
    pub fn terminal_floating_blur_radius(&self) -> f32 {
        (self.terminal_floating_blur.min(100) as f32) / 100.0 * MAX_BLUR_PX
    }

    /// Floating terminal-pane frosted-tint opacity (`0.0` = invisible, `1.0` =
    /// full frost), driven by `terminal_floating_blur` via the same perceptual
    /// sqrt curve as `terminal_frost_opacity()`. Independent of the tiled knob.
    pub fn terminal_floating_frost_opacity(&self) -> f32 {
        ((self.terminal_floating_blur.min(100) as f32) / 100.0).sqrt()
    }

    /// The OS backdrop material to apply, or `None` when disabled.
    pub fn os_vibrancy(&self) -> Option<Vibrancy> {
        (self.vibrancy != Vibrancy::None).then_some(self.vibrancy)
    }

    // ── Pane chrome resolvers ──
    // Config.toml `[appearance]` overrides take precedence; `None` inherits
    // from the theme automatically.

    /// Effective pane border width. Config override → theme `border_width`,
    /// clamped to `[0, 10]` so the border stays a reasonable frame regardless of
    /// config/theme values.
    pub fn effective_pane_border_width(&self, theme: &Theme) -> f32 {
        self.pane_border_width
            .unwrap_or(theme.border_width)
            .clamp(0.0, 10.0)
    }

    /// Effective inactive pane border color. Config override → theme `border` at 50% alpha.
    pub fn effective_pane_border_color(&self, theme: &Theme) -> Color {
        self.pane_border_color
            .unwrap_or_else(|| theme.border.with_alpha(128))
    }

    /// Effective pane corner radius. Config override → theme `border_radius`,
    /// clamped to `[0, 20]`. Higher radii make the rounded content-clip (stencil)
    /// eat into terminal content at the corners; capping keeps the clip gentle
    /// so cells/text aren't cut off.
    pub fn effective_pane_border_radius(&self, theme: &Theme) -> f32 {
        self.pane_border_radius
            .unwrap_or(theme.border_radius)
            .clamp(0.0, 20.0)
    }

    /// Effective active pane border color. Config override → theme `accent`.
    pub fn effective_pane_active_border_color(&self, theme: &Theme) -> Color {
        self.pane_active_border_color.unwrap_or(theme.accent)
    }

    /// Effective floating pane border color (applies to all floating panes,
    /// active or inactive, so they read as a distinct layer). Config override →
    /// `theme.float_accent`.
    pub fn effective_pane_floating_border_color(&self, theme: &Theme) -> Color {
        self.pane_floating_border_color.unwrap_or(theme.float_accent)
    }

    /// Effective pane gap. Config override → 8.0 (built-in layout default).
    pub fn effective_pane_gap(&self, _theme: &Theme) -> f32 {
        self.pane_gap.unwrap_or(8.0)
    }

    /// Effective pane internal padding (content inset from the pane border).
    /// Config override → `theme.pane_padding` (4.0 for mocha), clamped to
    /// `[0, 20]`. Snug by default now that the rounded content-clip (stencil)
    /// handles corners — a small straight-edge gap no longer overflows.
    pub fn effective_pane_padding(&self, theme: &Theme) -> f32 {
        self.pane_padding.unwrap_or(theme.pane_padding).clamp(0.0, 20.0)
    }

    /// Effective sidebar gap. Config override → 12.0.
    pub fn effective_sidebar_gap(&self, _theme: &Theme) -> f32 {
        self.sidebar_gap.unwrap_or(12.0)
    }

    /// Effective frosted tint color stamped behind translucent terminals (the
    /// `terminal_blur` frost). Config override → `theme.terminal_frost_color`
    /// → `theme.background` (the app theme bg, so the default frost reads as a
    /// frosted theme-bg glass). The stamp alpha is `terminal_frost_opacity()`,
    /// separate from this color.
    pub fn effective_terminal_frost_color(&self, theme: &Theme) -> Color {
        self.terminal_frost_color
            .or(theme.terminal_frost_color)
            .unwrap_or(theme.background)
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            transparency: default_transparency(),
            blur: default_blur(),
            terminal_transparency: default_terminal_transparency(),
            terminal_blur: default_terminal_blur(),
            terminal_floating_transparency: default_terminal_floating_transparency(),
            terminal_floating_blur: default_terminal_floating_blur(),
            vibrancy: default_vibrancy(),
            pane_border_width: None,
            pane_border_color: None,
            pane_border_radius: None,
            pane_active_border_color: None,
            pane_floating_border_color: None,
            pane_gap: None,
            pane_padding: None,
            sidebar_gap: None,
            terminal_frost_color: None,
            pane_title_style: default_pane_title_style(),
            pane_title_color: None,
            pane_title_background: None,
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
        assert_eq!(cfg.terminal_transparency, 0);
        assert_eq!(cfg.terminal_blur, 0);
        assert_eq!(cfg.terminal_floating_transparency, 0);
        assert_eq!(cfg.terminal_floating_blur, 0);
        assert_eq!(cfg.vibrancy, Vibrancy::None);
        assert!(!cfg.is_transparent());
        assert!((cfg.opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.blur_radius()).abs() < f32::EPSILON);
        assert!((cfg.terminal_opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.terminal_blur_radius()).abs() < f32::EPSILON);
        assert!((cfg.terminal_floating_opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.terminal_floating_blur_radius()).abs() < f32::EPSILON);
        assert!((cfg.terminal_floating_frost_opacity()).abs() < f32::EPSILON);
        assert_eq!(cfg.os_vibrancy(), None);
    }

    #[test]
    fn pane_title_defaults_to_cut_with_inherited_colors() {
        let cfg = AppearanceConfig::default();
        assert_eq!(cfg.pane_title_style, PaneTitleStyle::Cut);
        assert!(cfg.pane_title_color.is_none());
        assert!(cfg.pane_title_background.is_none());
    }

    #[test]
    fn pane_title_style_parses_snake_case() {
        let none: AppearanceConfig = toml::from_str("pane_title_style = \"none\"").unwrap();
        assert_eq!(none.pane_title_style, PaneTitleStyle::None);

        let filled: AppearanceConfig = toml::from_str(
            "pane_title_style = \"filled\"\npane_title_color = \"#89b4fa\"\npane_title_background = \"#1e1e2e\"",
        )
        .unwrap();
        assert_eq!(filled.pane_title_style, PaneTitleStyle::Filled);
        assert!(filled.pane_title_color.is_some());
        assert!(filled.pane_title_background.is_some());
    }

    #[test]
    fn amounts_map_to_derived_values() {
        let cfg = AppearanceConfig {
            transparency: 25,
            blur: 50,
            terminal_transparency: 40,
            terminal_blur: 75,
            vibrancy: Vibrancy::Sidebar,
            ..Default::default()
        };
        assert!((cfg.opacity() - 0.75).abs() < 1e-6);
        assert!(cfg.is_transparent());
        assert!((cfg.blur_radius() - 24.0).abs() < 1e-6); // 50% of 48px
        assert!((cfg.terminal_opacity() - 0.6).abs() < 1e-6);
        assert!((cfg.terminal_blur_radius() - 36.0).abs() < 1e-6); // 75% of 48px
        assert_eq!(cfg.os_vibrancy(), Some(Vibrancy::Sidebar));
    }

    #[test]
    fn terminal_frost_opacity_maps_blur_to_strength() {
        // Frost tint opacity is driven by terminal_blur via a perceptual sqrt
        // curve (0..100 → 0..1), decoupled from terminal_transparency so blur
        // modulates the frost visibly (the bug it fixes: the stamp used
        // surface_alpha, so a high terminal_transparency pinned the frost to a
        // faint fixed alpha).
        let off = AppearanceConfig { terminal_blur: 0, ..Default::default() };
        assert!((off.terminal_frost_opacity() - 0.0).abs() < f32::EPSILON);

        // Perceptual sqrt curve: blur 10 → ~0.316 (not 0.1).
        let light = AppearanceConfig { terminal_blur: 10, ..Default::default() };
        assert!((light.terminal_frost_opacity() - (0.10f32).sqrt()).abs() < 1e-6);

        let half = AppearanceConfig { terminal_blur: 50, ..Default::default() };
        assert!((half.terminal_frost_opacity() - (0.50f32).sqrt()).abs() < 1e-6);

        let full = AppearanceConfig { terminal_blur: 100, ..Default::default() };
        assert!((full.terminal_frost_opacity() - 1.0).abs() < 1e-6);

        // Over-cap clamps to full frost.
        let over = AppearanceConfig { terminal_blur: 200, ..Default::default() };
        assert!((over.terminal_frost_opacity() - 1.0).abs() < 1e-6);

        // Independence from terminal_transparency: high transparency must not
        // change the frost strength.
        let transparent = AppearanceConfig {
            terminal_blur: 60,
            terminal_transparency: 90,
            ..Default::default()
        };
        assert!((transparent.terminal_frost_opacity() - (0.60f32).sqrt()).abs() < 1e-6);
    }

    #[test]
    fn terminal_frost_color_resolves_config_then_theme_then_background() {
        let mocha = crate::theme::Theme::catppuccin_mocha();

        // Default: no config override, bundled theme field is None → falls back
        // to theme.background (the app theme bg).
        let cfg = AppearanceConfig::default();
        assert_eq!(cfg.effective_terminal_frost_color(&mocha), mocha.background);

        // Theme field wins over background when config is unset.
        let mut themed = mocha.clone();
        themed.terminal_frost_color = Some(Color::new(10, 20, 30, 255));
        assert_eq!(
            cfg.effective_terminal_frost_color(&themed),
            Color::new(10, 20, 30, 255)
        );

        // Config override wins over both.
        let cfg = AppearanceConfig {
            terminal_frost_color: Some(Color::new(1, 2, 3, 255)),
            ..Default::default()
        };
        assert_eq!(
            cfg.effective_terminal_frost_color(&themed),
            Color::new(1, 2, 3, 255)
        );
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let cfg: AppearanceConfig = toml::from_str("transparency = 30\nterminal_transparency = 15\n")
            .expect("partial appearance toml should parse");
        assert_eq!(cfg.transparency, 30);
        assert_eq!(cfg.blur, 0);
        assert_eq!(cfg.terminal_transparency, 15);
        assert_eq!(cfg.terminal_blur, 0);
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

    #[test]
    fn floating_terminal_knobs_are_independent_of_tiled() {
        // Tiled frosted/translucent while floating defaults to opaque, no frost.
        let cfg = AppearanceConfig {
            terminal_transparency: 95,
            terminal_blur: 100,
            terminal_floating_transparency: 0,
            terminal_floating_blur: 0,
            ..Default::default()
        };
        assert!((cfg.terminal_opacity() - 0.05).abs() < 1e-6);
        assert!((cfg.terminal_floating_opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.terminal_frost_opacity() - 1.0).abs() < 1e-6);
        assert!((cfg.terminal_floating_frost_opacity()).abs() < f32::EPSILON);
        assert!((cfg.terminal_floating_blur_radius()).abs() < f32::EPSILON);

        // Floating knobs can be set independently of the tiled ones.
        let cfg = AppearanceConfig {
            terminal_floating_transparency: 50,
            terminal_floating_blur: 25,
            ..Default::default()
        };
        assert!((cfg.terminal_floating_opacity() - 0.5).abs() < 1e-6);
        assert!((cfg.terminal_floating_frost_opacity() - (0.25f32).sqrt()).abs() < 1e-6);
    }

    #[test]
    fn floating_border_color_resolves_config_then_float_accent() {
        let mocha = crate::theme::Theme::catppuccin_mocha();

        // Default: no config override -> theme.float_accent.
        let cfg = AppearanceConfig::default();
        assert_eq!(cfg.effective_pane_floating_border_color(&mocha), mocha.float_accent);

        // Config override wins.
        let cfg = AppearanceConfig {
            pane_floating_border_color: Some(Color::new(1, 2, 3, 255)),
            ..Default::default()
        };
        assert_eq!(
            cfg.effective_pane_floating_border_color(&mocha),
            Color::new(1, 2, 3, 255)
        );
    }

    #[test]
    fn pane_chrome_values_are_clamped_to_safe_ranges() {
        let theme = crate::theme::Theme::default();

        // Explicit config values above the cap clamp down into range.
        let over = AppearanceConfig {
            pane_border_width: Some(999.0),
            pane_border_radius: Some(88.0),
            pane_padding: Some(999.0),
            ..Default::default()
        };
        assert_eq!(over.effective_pane_border_width(&theme), 10.0);
        assert_eq!(over.effective_pane_border_radius(&theme), 20.0);
        assert_eq!(over.effective_pane_padding(&theme), 20.0);

        // Negative values clamp to the lower bound (0).
        let under = AppearanceConfig {
            pane_border_width: Some(-5.0),
            pane_border_radius: Some(-2.0),
            pane_padding: Some(-1.0),
            ..Default::default()
        };
        assert_eq!(under.effective_pane_border_width(&theme), 0.0);
        assert_eq!(under.effective_pane_border_radius(&theme), 0.0);
        assert_eq!(under.effective_pane_padding(&theme), 0.0);

        // Defaults (None) fall back to the theme then clamp — theme defaults
        // (border_width 1.0, border_radius 6.0) are already in range, and pane
        // padding defaults to 4.0.
        let dflt = AppearanceConfig::default();
        assert_eq!(dflt.effective_pane_border_width(&theme), theme.border_width);
        assert_eq!(dflt.effective_pane_border_radius(&theme), theme.border_radius);
        assert_eq!(dflt.effective_pane_padding(&theme), 4.0);

        // In-range values pass through unchanged.
        let mid = AppearanceConfig {
            pane_border_width: Some(4.0),
            pane_border_radius: Some(15.0),
            pane_padding: Some(8.0),
            ..Default::default()
        };
        assert_eq!(mid.effective_pane_border_width(&theme), 4.0);
        assert_eq!(mid.effective_pane_border_radius(&theme), 15.0);
        assert_eq!(mid.effective_pane_padding(&theme), 8.0);
    }
}
