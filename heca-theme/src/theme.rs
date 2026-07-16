//! Unified theme types for heca.
//!
//! A [`Theme`] is the palette + effect configuration shared by all rendering
//! layers — grid-ui widgets, chrome, sidebar, and terminal panes.
//! The default theme is the dark, cyan-accented Tron skin ([`Theme::grid_tron`]).

use crate::color::Color;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  Intensity
// ═══════════════════════════════════════════════════════════════════════════════

/// Scanline / CRT overlay intensity. This controls **scanline-overlay
/// opacity only** — it does **not** affect glow. Glow is owned by
/// [`GlowLevel`] (presence + radius + strength). Higher `Intensity` = a
/// stronger visible CRT grille; `Off` = no scanlines (closest to a standard
/// UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    /// No scanlines / CRT overlay.
    Off,
    Low,
    #[default]
    Medium,
    /// Strong CRT grille (heaviest scanlines). Does not add glow.
    Heavy,
}

impl Intensity {
    /// CRT scanline-overlay opacity for this level — the visible thing
    /// `intensity` controls. `Off` = no scanlines; higher = a stronger CRT
    /// grille. Glow is separate (see [`GlowLevel`]).
    pub fn scanline_opacity(self) -> f32 {
        match self {
            Intensity::Off => 0.0,
            Intensity::Low => 0.05,
            Intensity::Medium => 0.11,
            Intensity::Heavy => 0.20,
        }
    }

    /// Cycle to the next level (Off → Low → Medium → Heavy → Off).
    pub fn next(self) -> Self {
        match self {
            Intensity::Off => Intensity::Low,
            Intensity::Low => Intensity::Medium,
            Intensity::Medium => Intensity::Heavy,
            Intensity::Heavy => Intensity::Off,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  GlowLevel
// ═══════════════════════════════════════════════════════════════════════════════

/// Size of the neon glow halo — scales every glow's falloff radius;
/// `None` disables glow entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlowLevel {
    /// No glow at all.
    None,
    /// A tight halo.
    Thin,
    #[default]
    Medium,
    /// A wide, soft halo.
    Large,
}

impl GlowLevel {
    /// Multiplier applied to a glow's base falloff radius. `0.0` means "off".
    /// This is the *radius* (halo size) dimension; glow *strength* (alpha) is
    /// [`strength_scale`](GlowLevel::strength_scale). The two are independent
    /// so e.g. `Large` = 2.0× radius but only 1.6× strength.
    pub fn radius_scale(self) -> f32 {
        match self {
            GlowLevel::None => 0.0,
            GlowLevel::Thin => 0.5,
            GlowLevel::Medium => 1.0,
            GlowLevel::Large => 2.0,
        }
    }

    /// Multiplier applied to a glow's base strength (alpha). `0.0` means "off".
    /// This is the *strength* (alpha) dimension; glow *radius* (halo size) is
    /// [`radius_scale`](GlowLevel::radius_scale). `GlowLevel` is the sole owner
    /// of glow — both radius and strength — so `intensity` no longer feeds glow.
    /// Curve: `none=0.0, thin=0.5, medium=1.0, large=1.6` (preserves the former
    /// `Intensity::glow_scale()` values exactly).
    pub fn strength_scale(self) -> f32 {
        match self {
            GlowLevel::None => 0.0,
            // 0.75 (was 0.5): with the faint rest glows (`control_rest_glow` ≈ 0.12
            // base) a 0.5× strength on a 0.5× radius was nearly invisible — Thin
            // read the same as None (user-reported). Thin = tight halo (the 0.5×
            // radius carries the "thin"), still clearly present.
            GlowLevel::Thin => 0.75,
            GlowLevel::Medium => 1.0,
            GlowLevel::Large => 1.6,
        }
    }

    /// Parse a `config.toml` value (case-insensitive). Unknown → `Medium`.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => GlowLevel::None,
            "thin" => GlowLevel::Thin,
            "large" => GlowLevel::Large,
            _ => GlowLevel::Medium,
        }
    }

    /// Stable ordering used by selectors: None, Thin, Medium, Large.
    pub const ALL: [GlowLevel; 4] = [
        GlowLevel::None,
        GlowLevel::Thin,
        GlowLevel::Medium,
        GlowLevel::Large,
    ];

    /// Uppercase label for UI (matches `ALL` order).
    pub fn label(self) -> &'static str {
        match self {
            GlowLevel::None => "NONE",
            GlowLevel::Thin => "THIN",
            GlowLevel::Medium => "MEDIUM",
            GlowLevel::Large => "LARGE",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Shadow
// ═══════════════════════════════════════════════════════════════════════════════

/// Drop-shadow configuration for elevated/floating surfaces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub color: Color,
    pub alpha: f32,
    pub blur: f32,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: Color::rgb(0, 0, 0),
            alpha: 0.3,
            blur: 8.0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Theme
// ═══════════════════════════════════════════════════════════════════════════════

/// Small controls (inputs, selects, checkboxes, chips) round at this fraction of
/// the base [`Theme::radius`], so one global radius scales every widget together.
const CONTROL_RADIUS_FRAC: f32 = 0.5;
/// How far the derived focus-outline color is shifted from a tone toward the theme's
/// `foreground` when `focus_ring` is unset. Because `foreground` is light on dark themes and dark
/// on light themes, this brightens the ring on dark themes and darkens it on light ones — so it
/// separates from the widget's own accent border either way (no hardcoded light/dark, no fixed
/// white/black).
const FOCUS_RING_CONTRAST_FACTOR: f32 = 0.35;
const SIDEBAR_BG_DARKEN_FACTOR: f32 = 0.05;
const TOP_BOTTOM_PANE_BG_DARKEN_FACTOR: f32 = 0.10;
/// How much the derived z=0 gradient *bottom* color darkens
/// [`Theme::background`] toward black when `background_gradient_bottom` is unset
/// (subtle vertical depth). Bundled themes set explicit gradient colors, so this
/// is only a safety net for custom themes missing the field.
///
/// `0.08` is a visual sweet-spot: deep enough to give the blurred gradient
/// visible vertical depth, shallow enough not to read as a separate color band
/// (which the blur would smear awkwardly). Tuned to match the subtle depth of
/// the existing chrome-token darkening factors.
const GRADIENT_BOTTOM_DARKEN_FACTOR: f32 = 0.08;

/// Palette + effect tokens for the entire application.
///
/// This is the single source of truth for all theming. Grid-ui widgets read it
/// via `cx.theme()`, the config system loads it from `.toml` files, and the
/// renderer uses it for chrome/terminal colors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,

    // ── Core palette ──
    pub background: Color,
    #[serde(default = "default_surface")]
    pub surface: Color,
    pub foreground: Color,
    #[serde(default = "default_muted")]
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    #[serde(default = "default_glow_color")]
    pub glow: Color,
    #[serde(default)]
    pub shadow: Shadow,
    #[serde(default = "default_danger")]
    pub danger: Color,
    #[serde(default = "default_success")]
    pub success: Color,
    #[serde(default = "default_warning")]
    pub warning: Color,

    // ── Chrome geometry ──
    pub border_radius: f32,
    pub border_width: f32,
    #[serde(default = "default_pane_padding")]
    pub pane_padding: f32,

    // ── Chrome background tokens ──
    #[serde(default)]
    pub left_sidebar_background: Option<Color>,
    #[serde(default)]
    pub right_sidebar_background: Option<Color>,
    #[serde(default)]
    pub top_bottom_pane_background: Option<Color>,

    // ── z=0 background gradient (compositor-blur refactor) ──
    /// Optional top color of the z=0 vertical gradient. `None` → falls back to
    /// [`Theme::background`] (see [`Theme::effective_background_gradient_top`]).
    #[serde(default)]
    pub background_gradient_top: Option<Color>,
    /// Optional bottom color of the z=0 vertical gradient. `None` → falls back to
    /// a slightly darkened [`Theme::background`] (see
    /// [`Theme::effective_background_gradient_bottom`]).
    #[serde(default)]
    pub background_gradient_bottom: Option<Color>,

    // ── Effect tokens ──
    #[serde(default)]
    pub glow_size: GlowLevel,
    #[serde(default)]
    pub intensity: Intensity,
    #[serde(default = "default_true")]
    pub show_focus_border: bool,
    /// Optional color of the keyboard **focus outline** — the thin ring drawn just *outside* a
    /// focused widget (see [`effective_focus_ring`](Self::effective_focus_ring) /
    /// [`PaintCx::focus_ring`](../heca_grid_ui/struct.PaintCx.html)). `None` → the accent shifted
    /// toward `foreground` for contrast, which stays legible on both dark and light themes. Set it
    /// in a theme (`focus_ring = "#rrggbb"`) to override the default/accent focus color; tonal
    /// rings that aren't the accent (e.g. a destructive button's `danger` ring) always derive via
    /// [`focus_ring_tone`](Self::focus_ring_tone) and are not overridden by this token.
    #[serde(default)]
    pub focus_ring: Option<Color>,
    #[serde(default = "default_icon_secondary_alpha")]
    pub icon_secondary_alpha: f32,
    /// Opacity (`0.0..=1.0`) of the **active-region wash** — the faint accent
    /// overlay a `DockFrame` paints over itself when marked active (e.g. the
    /// active workspace in the sidebar). Theme/config-driven, not baked into the
    /// widget, so the active highlight flips in place via the bound signal.
    #[serde(default = "default_active_wash_alpha")]
    pub active_wash_alpha: f32,
    /// Opacity (`0.0..=1.0`) of a sidebar/list **card's resting background** tint
    /// (e.g. each pane card). Kept very low so a card reads as a subtle raised
    /// surface rather than a filled block. Theme/config-driven.
    #[serde(default = "default_card_background_alpha")]
    pub card_background_alpha: f32,

    /// Interaction-state alpha tokens (hover / active / border / tonal-fill /
    /// scrim …). Theme-owned so the whole UI's interaction feel is tuned in one
    /// place, not per-widget constants. `#[serde(default)]` → existing theme TOMLs
    /// (which don't list them) inherit [`InteractionAlphas::default`].
    #[serde(default)]
    pub interaction: InteractionAlphas,

    // ── Float pane colors ──
    #[serde(default = "default_float_bg")]
    pub float_background: Color,
    #[serde(default = "default_float_accent")]
    pub float_accent: Color,
    #[serde(default = "default_float_focus")]
    pub float_focus: Color,

    // ── Drag-and-drop colors ──
    #[serde(default = "default_drag_ghost_bg", alias = "sidebar_drag_ghost_bg")]
    pub drag_ghost_bg: Color,
    #[serde(default = "default_drag_ghost_fg", alias = "sidebar_drag_ghost_fg")]
    pub drag_ghost_fg: Color,
    #[serde(default = "default_drag_source_bg", alias = "sidebar_drag_source_bg")]
    pub drag_source_bg: Color,
    #[serde(
        default = "default_drag_source_border",
        alias = "sidebar_drag_source_border"
    )]
    pub drag_source_border: Color,
    #[serde(default = "default_drop_target_bg")]
    pub drop_target_bg: Color,
    #[serde(default = "default_drop_target_border")]
    pub drop_target_border: Color,
    #[serde(default = "default_drop_insertion")]
    pub drop_insertion: Color,

    // ── Sidebar font sizes ──
    #[serde(default = "default_sidebar_label_font_size")]
    pub sidebar_label_font_size: f32,
    #[serde(default = "default_sidebar_button_font_size")]
    pub sidebar_button_font_size: f32,

    // ── Terminal ──
    #[serde(default)]
    pub terminal_foreground: Option<Color>,
    #[serde(default)]
    pub terminal_background: Option<Color>,
    #[serde(default)]
    pub terminal_cursor_foreground: Option<Color>,
    #[serde(default)]
    pub terminal_cursor_background: Option<Color>,
    #[serde(default)]
    pub terminal_cursor_border: Option<Color>,
    #[serde(default)]
    pub terminal_selection_foreground: Option<Color>,
    #[serde(default)]
    pub terminal_selection_background: Option<Color>,
    #[serde(default)]
    pub terminal_ansi: Option<[Color; 8]>,
    #[serde(default)]
    pub terminal_brights: Option<[Color; 8]>,
}

/// Interaction-state alpha tokens — raw `0..=255` alpha bytes a widget lays over a
/// base hue (accent / foreground / danger …) for its hover, active/selected, border,
/// tonal-fill, scrim and overlay states. Grouped on [`Theme::interaction`] so the
/// interaction feel is a single theme-tuned surface instead of scattered per-widget
/// `const … _ALPHA` values. Values default to the historical per-widget constants;
/// where several widgets shared a role the value is unified (see field docs).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InteractionAlphas {
    // ── Controls (buttons / toggles / inputs / selects) ──
    /// Toggle track fill at full-on (~50% accent wash).
    pub toggle_on_fill: u8,
    /// Hover fill over a control's tone (icon button).
    pub control_hover_fill: u8,
    /// Hover border over a control's tone (icon button).
    pub control_hover_border: u8,
    /// Held-on (toggled) fill (icon button).
    pub control_active_fill: u8,
    /// Held-on (toggled) border (icon button).
    pub control_active_border: u8,
    /// Resting border of a control — unifies button/input/toggle/checkbox/select (was `150` in each).
    pub control_rest_border: u8,
    /// Resting **glow intensity** of a control's surface (×255 — a `Glow.intensity`
    /// of `30` ≈ 0.12), so buttons/inputs/toggles/selects carry a faint neon halo at
    /// rest and the `glow_size` setting visibly scales them without hover/focus.
    /// `0` = flat rest look (glow only on hover/active), matching the pre-token
    /// behaviour. Scaled — like every glow — by `glow_size` at the
    /// `PaintCx::scaled_glow` chokepoint.
    pub control_rest_glow: u8,

    // ── List rows / sidebar cells ──
    /// Hover fill of a list row / sidebar cell — unifies row/item/rail (16/16/18 → 16).
    pub row_hover_fill: u8,
    /// Selected fill of a list row / sidebar cell — unifies row/item/rail (30/30/34 → 30).
    pub row_active_fill: u8,
    /// Selected border of a list row / sidebar cell — unifies row/rail (180/190 → 185).
    pub row_active_border: u8,
    /// Selected-state highlight tint (row).
    pub row_active_tint: u8,
    /// Hover-state highlight tint (row).
    pub row_hover_tint: u8,

    // ── Nav cursor (keyboard-nav highlight on rows / docks) ──
    /// Nav-cursor outline — unifies row/dock (220/235 → 225).
    pub nav_outline: u8,
    /// Nav-cursor wash fill (dock).
    pub nav_wash: u8,

    // ── Tonal fills (badges / tags / alerts / toasts) ──
    /// Badge fill — unifies badge/badge_button (both `38`).
    pub badge_fill: u8,
    /// Tag / alert tonal fill — unifies tag/alert (both `22`).
    pub tag_fill: u8,
    /// Tag border / divider.
    pub tag_border: u8,
    /// Toast background tint.
    pub toast_tint: u8,
    /// Badge-button outline, resting.
    pub outline_rest: u8,
    /// Badge-button outline, hovered.
    pub outline_hover: u8,

    // ── Text selection / list hilite ──
    /// Text-selection fill (input).
    pub selection: u8,
    /// Dropdown row hilite (select).
    pub hilite: u8,

    // ── Overlays (modal / palette / context menu) ──
    /// Backdrop scrim behind a modal/palette — unifies modal/palette (150/140 → 150).
    pub scrim: u8,
    /// Keycap background — unifies context_menu/key_hint (both `200`).
    pub keycap: u8,
    /// Overlay panel border (palette / context menu).
    pub panel_border: u8,
    /// Overlay panel selected-row border.
    pub panel_row_border: u8,
    /// Overlay panel selected-row fill.
    pub panel_row_fill: u8,
    /// Tooltip border, in the accent hue.
    pub tooltip_border: u8,
    /// Context-menu shortcut/hint text.
    pub menu_shortcut: u8,
    /// Context-menu shortcut/hint text, dimmed (disabled row).
    pub menu_shortcut_dim: u8,

    // ── Scrollbar thumb ──
    /// Scrollbar thumb, resting — unifies scroll_bar/scroll_region (both `90`).
    pub thumb_rest: u8,
    /// Scrollbar thumb, hovered — unifies scroll_bar/scroll_region (both `200`).
    pub thumb_hover: u8,

    // ── Dim / unlit ──
    /// Unlit / dimmed element (gauge).
    pub unlit: u8,
}

impl Default for InteractionAlphas {
    fn default() -> Self {
        Self {
            toggle_on_fill: 128,
            control_hover_fill: 28,
            control_hover_border: 190,
            control_active_fill: 64,
            control_active_border: 215,
            control_rest_border: 150,
            control_rest_glow: 30,
            row_hover_fill: 16,
            row_active_fill: 30,
            row_active_border: 185,
            row_active_tint: 90,
            row_hover_tint: 40,
            nav_outline: 225,
            nav_wash: 30,
            badge_fill: 38,
            tag_fill: 22,
            tag_border: 130,
            toast_tint: 16,
            outline_rest: 150,
            outline_hover: 235,
            selection: 70,
            hilite: 48,
            scrim: 150,
            keycap: 200,
            panel_border: 200,
            panel_row_border: 150,
            panel_row_fill: 30,
            tooltip_border: 180,
            menu_shortcut: 180,
            menu_shortcut_dim: 120,
            thumb_rest: 90,
            thumb_hover: 200,
            unlit: 40,
        }
    }
}

// ── Serde default helpers ──

fn default_surface() -> Color {
    Color::rgb(12, 18, 24)
}
fn default_muted() -> Color {
    Color::rgb(96, 130, 146)
}
fn default_glow_color() -> Color {
    Color::rgb(64, 224, 255)
}
fn default_danger() -> Color {
    Color::rgb(255, 70, 84)
}
fn default_success() -> Color {
    Color::rgb(80, 255, 170)
}
fn default_warning() -> Color {
    Color::rgb(255, 190, 70)
}
fn default_pane_padding() -> f32 {
    4.0
}
fn default_true() -> bool {
    true
}
fn default_icon_secondary_alpha() -> f32 {
    // Kept low so the duotone secondary layer stays a subtle wash — a higher value makes
    // solid/filled glyphs (Stop, Circle, filled Play) collapse into a flat, muddy blob.
    0.30
}
fn default_active_wash_alpha() -> f32 {
    0.11
}
fn default_card_background_alpha() -> f32 {
    0.02
}
fn default_float_bg() -> Color {
    Color::new(49, 50, 68, 255)
}
fn default_float_accent() -> Color {
    Color::new(137, 180, 250, 255)
}
fn default_float_focus() -> Color {
    Color::new(250, 179, 135, 255)
}
fn default_drag_ghost_bg() -> Color {
    Color::new(137, 180, 250, 217)
}
fn default_drag_ghost_fg() -> Color {
    Color::new(255, 255, 255, 255)
}
fn default_drag_source_bg() -> Color {
    Color::new(137, 180, 250, 38)
}
fn default_drag_source_border() -> Color {
    Color::new(137, 180, 250, 255)
}
fn default_drop_target_bg() -> Color {
    Color::new(137, 180, 250, 45)
}
fn default_drop_target_border() -> Color {
    Color::new(137, 180, 250, 255)
}
fn default_drop_insertion() -> Color {
    Color::new(137, 180, 250, 255)
}
fn default_sidebar_label_font_size() -> f32 {
    14.0
}
fn default_sidebar_button_font_size() -> f32 {
    11.0
}
impl Default for Theme {
    fn default() -> Self {
        Self::grid_tron()
    }
}

impl Theme {
    /// Corner radius for small controls — a fraction of the base [`radius`](Theme::radius).
    pub fn control_radius(&self) -> f32 {
        self.border_radius * CONTROL_RADIUS_FRAC
    }

    fn derived_darker_background(&self, t: f32) -> Color {
        self.background.lerp(Color::rgb(0, 0, 0), t.clamp(0.0, 1.0))
    }

    pub fn effective_left_sidebar_background(&self) -> Color {
        self.left_sidebar_background
            .unwrap_or_else(|| self.derived_darker_background(SIDEBAR_BG_DARKEN_FACTOR))
    }

    pub fn effective_right_sidebar_background(&self) -> Color {
        self.right_sidebar_background
            .unwrap_or_else(|| self.derived_darker_background(SIDEBAR_BG_DARKEN_FACTOR))
    }

    pub fn effective_top_bottom_pane_background(&self) -> Color {
        self.top_bottom_pane_background
            .unwrap_or_else(|| self.derived_darker_background(TOP_BOTTOM_PANE_BG_DARKEN_FACTOR))
    }

    /// A readable text/glyph color to place **on** a tonal fill (`accent`, `danger`,
    /// `success`, …): whichever of the theme's `background` / `foreground` contrasts
    /// more with the fill's luminance. Theme-driven (no hardcoded light/dark) — so a
    /// dark label lands on a bright accent and a light label on a saturated danger.
    pub fn on(&self, fill: Color) -> Color {
        let l = fill.luminance();
        if (l - self.background.luminance()).abs() >= (l - self.foreground.luminance()).abs() {
            self.background
        } else {
            self.foreground
        }
    }

    /// Effective z=0 gradient *top* color: the theme field when set, else
    /// [`Theme::background`] (an unset gradient is a flat bg-color fill — still a
    /// valid frost source once blurred). The app's `[appearance]`
    /// `background_gradient_top` override (if any) takes precedence over this —
    /// see `heca_config::appearance::AppearanceConfig::effective_background_gradient_top`.
    pub fn effective_background_gradient_top(&self) -> Color {
        self.background_gradient_top.unwrap_or(self.background)
    }

    /// Effective z=0 gradient *bottom* color: the theme field when set, else a
    /// slightly darkened [`Theme::background`] (subtle vertical depth). The app's
    /// `[appearance]` `background_gradient_bottom` override (if any) takes
    /// precedence — see
    /// `heca_config::appearance::AppearanceConfig::effective_background_gradient_bottom`.
    pub fn effective_background_gradient_bottom(&self) -> Color {
        self.background_gradient_bottom
            .unwrap_or_else(|| self.derived_darker_background(GRADIENT_BOTTOM_DARKEN_FACTOR))
    }

    /// The keyboard **focus-outline color** for the default (accent) tone — what
    /// [`PaintCx::focus_ring`](../heca_grid_ui/struct.PaintCx.html) draws on every focused widget.
    /// Returns the theme's `focus_ring` token when set, else the accent shifted toward `foreground`
    /// (see [`focus_ring_tone`](Self::focus_ring_tone)) so the ring reads distinct from an accent
    /// border on both dark and light themes.
    pub fn effective_focus_ring(&self) -> Color {
        self.focus_ring.unwrap_or_else(|| self.focus_ring_tone(self.accent))
    }

    /// Derive a focus-outline color from any semantic tone (`accent`, `danger`, …) by shifting it
    /// toward the theme's `foreground`. `foreground` is the theme's high-contrast-against-background
    /// color — light on dark themes, dark on light themes — so this brightens the ring on dark
    /// themes and darkens it on light ones, keeping it separate from the tone's own border in both.
    /// Theme-driven, no hardcoded light/dark (same idiom as [`on`](Self::on)). Use this for tonal
    /// focus rings that aren't the default accent (e.g. a destructive button's `danger` ring); the
    /// `focus_ring` token overrides only the default/accent case via [`effective_focus_ring`](Self::effective_focus_ring).
    pub fn focus_ring_tone(&self, base: Color) -> Color {
        base.lerp(self.foreground, FOCUS_RING_CONTRAST_FACTOR)
    }

    /// The default dark, cyan-accented Tron theme.
    ///
    /// Sourced from the bundled `themes/grid_tron.toml` (embedded at compile
    /// time via `include_str!`) so there is a **single source of truth** — no
    /// parallel hand-maintained Rust color literals to drift from the TOML.
    /// `Theme::default()` returns this. If the TOML ever fails to parse the
    /// `Theme` shape, that is a compile-time-shippable bug and we panic eagerly.
    pub fn grid_tron() -> Self {
        toml::from_str(include_str!("themes/grid_tron.toml"))
            .expect("bundled grid_tron.toml must parse into Theme")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_grid_tron() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Grid Tron");
    }

    #[test]
    fn control_radius_is_half_of_radius() {
        let theme = Theme::grid_tron();
        assert!((theme.control_radius() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn focus_ring_is_theme_aware_and_respects_override() {
        // Dark theme (grid_tron): the derived ring is *lighter* than the accent (shifted toward the
        // light foreground) so it separates from the accent border.
        let mut dark = Theme::grid_tron();
        dark.focus_ring = None;
        let d = dark.effective_focus_ring();
        assert_ne!(d, dark.accent);
        assert!(d.luminance() > dark.accent.luminance(), "dark theme: ring brighter than accent");

        // Light theme (latte): the SAME logic derives a *darker* ring (shifted toward the dark
        // foreground) — the direction flips automatically, no hardcoded light/dark.
        let light: Theme = toml::from_str(include_str!("themes/latte.toml"))
            .expect("bundled latte.toml must parse into Theme");
        let l = light.effective_focus_ring();
        assert!(l.luminance() < light.accent.luminance(), "light theme: ring darker than accent");

        // Any tone derives via the same helper (this is how the destructive/danger ring is built).
        assert_eq!(
            dark.focus_ring_tone(dark.danger),
            dark.danger.lerp(dark.foreground, FOCUS_RING_CONTRAST_FACTOR)
        );

        // The `focus_ring` token overrides only the default/accent case, verbatim.
        dark.focus_ring = Some(Color::rgb(10, 20, 30));
        assert_eq!(dark.effective_focus_ring(), Color::rgb(10, 20, 30));
    }

    #[test]
    fn chrome_background_tokens_derive_from_background_when_unset() {
        let mut theme = Theme::grid_tron();
        theme.left_sidebar_background = None;
        theme.right_sidebar_background = None;
        theme.top_bottom_pane_background = None;

        assert_ne!(theme.effective_left_sidebar_background(), theme.background);
        assert_ne!(theme.effective_right_sidebar_background(), theme.background);
        assert_ne!(
            theme.effective_top_bottom_pane_background(),
            theme.background
        );
    }

    #[test]
    fn glow_level_radius_scales() {
        assert!((GlowLevel::None.radius_scale()).abs() < f32::EPSILON);
        assert!((GlowLevel::Thin.radius_scale() - 0.5).abs() < f32::EPSILON);
        assert!((GlowLevel::Medium.radius_scale() - 1.0).abs() < f32::EPSILON);
        assert!((GlowLevel::Large.radius_scale() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn glow_level_strength_scales() {
        // GlowLevel is the sole owner of glow strength; curve preserves the
        // former Intensity::glow_scale() values exactly (none/thin/medium/large).
        assert!((GlowLevel::None.strength_scale()).abs() < f32::EPSILON);
        assert!((GlowLevel::Thin.strength_scale() - 0.5).abs() < f32::EPSILON);
        assert!((GlowLevel::Medium.strength_scale() - 1.0).abs() < f32::EPSILON);
        assert!((GlowLevel::Large.strength_scale() - 1.6).abs() < f32::EPSILON);
    }

    #[test]
    fn intensity_scanline_opacity_is_separate_from_glow() {
        // Intensity owns scanlines only; it no longer exposes glow_scale().
        // Pin exact values (not just ordering) so an accidental curve change is caught.
        assert!((Intensity::Off.scanline_opacity()).abs() < f32::EPSILON);
        assert!((Intensity::Low.scanline_opacity() - 0.05).abs() < 1e-6);
        assert!((Intensity::Medium.scanline_opacity() - 0.11).abs() < 1e-6);
        assert!((Intensity::Heavy.scanline_opacity() - 0.20).abs() < 1e-6);
        // Ordering still holds as a sanity check.
        assert!(Intensity::Medium.scanline_opacity() > Intensity::Low.scanline_opacity());
        assert!(Intensity::Heavy.scanline_opacity() > Intensity::Medium.scanline_opacity());
    }

    #[test]
    fn glow_level_parse_case_insensitive() {
        assert_eq!(GlowLevel::parse("NONE"), GlowLevel::None);
        assert_eq!(GlowLevel::parse("thin"), GlowLevel::Thin);
        assert_eq!(GlowLevel::parse("LARGE"), GlowLevel::Large);
        assert_eq!(GlowLevel::parse("unknown"), GlowLevel::Medium);
    }

    #[test]
    fn intensity_next_cycles() {
        assert_eq!(Intensity::Off.next(), Intensity::Low);
        assert_eq!(Intensity::Low.next(), Intensity::Medium);
        assert_eq!(Intensity::Medium.next(), Intensity::Heavy);
        assert_eq!(Intensity::Heavy.next(), Intensity::Off);
    }

    #[test]
    fn gradient_colors_derive_from_background_when_unset() {
        let mut theme = Theme::grid_tron();
        theme.background_gradient_top = None;
        theme.background_gradient_bottom = None;
        // Top falls back to the theme background.
        assert_eq!(theme.effective_background_gradient_top(), theme.background);
        // Bottom falls back to a slightly darkened background (not the raw
        // background — subtle vertical depth).
        assert_ne!(
            theme.effective_background_gradient_bottom(),
            theme.background
        );
    }

    /// Bundled themes ship explicit z=0 gradient colors in their TOMLs. This pins
    /// that contract so a dropped key is caught (the resolver would silently fall
    /// back to the derived single-color gradient otherwise). Iterates the loader's
    /// bundled-theme map so a newly added bundled theme is covered automatically.
    #[test]
    fn bundled_themes_ship_explicit_gradient_colors() {
        for name in crate::loader::bundled_themes().keys() {
            let theme = crate::load_theme(name);
            assert!(
                theme.background_gradient_top.is_some(),
                "{name} should set background_gradient_top in its TOML"
            );
            assert!(
                theme.background_gradient_bottom.is_some(),
                "{name} should set background_gradient_bottom in its TOML"
            );
            // Top and bottom differ so the gradient is actually a gradient
            // (not a flat fill that hides the blur).
            assert_ne!(
                theme.effective_background_gradient_top(),
                theme.effective_background_gradient_bottom(),
                "{name} gradient top and bottom should differ"
            );
        }
    }

    /// `Shadow.color` is a `Color` (not a `String`) - pinned so a regression to
    /// `String` is caught. The bundled grid_tron TOML writes
    /// `shadow = { color = "#000000", alpha = 0.3, blur = 8.0 }`; serde must parse
    /// the hex string into a `Color` via `Color`'s `try_from<String>`.
    #[test]
    fn shadow_color_is_color_from_toml_hex() {
        let theme = crate::load_theme("grid_tron");
        assert_eq!(theme.shadow.color, Color::rgb(0, 0, 0));
        assert!((theme.shadow.alpha - 0.3).abs() < f32::EPSILON);
        assert!((theme.shadow.blur - 8.0).abs() < f32::EPSILON);
    }

    /// Serde round-trip: a `Shadow` with a hex `color` deserializes to a `Color`
    /// and serializes back, so the TOML config representation stays stable.
    #[test]
    fn shadow_serde_roundtrips_color() {
        let toml = "color = \"#89b4fa\"\nalpha = 0.25\nblur = 12.0\n";
        let s: Shadow = toml::from_str(toml).unwrap();
        assert_eq!(s.color, Color::rgb(0x89, 0xb4, 0xfa));
        assert!((s.alpha - 0.25).abs() < f32::EPSILON);
        assert!((s.blur - 12.0).abs() < f32::EPSILON);
        let reserialized: String = toml::to_string(&s).unwrap();
        let s2: Shadow = toml::from_str(&reserialized).unwrap();
        assert_eq!(s, s2);
    }
}
