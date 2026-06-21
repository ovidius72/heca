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

/// How strongly Tron effects (glow, scanlines) are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intensity {
    /// Plain — no glow or scanlines (closest to a standard UI).
    Off,
    Low,
    #[default]
    Medium,
    /// Full Tron: strong glow + visible scanlines.
    Heavy,
}

impl Intensity {
    /// Glow strength multiplier for this level.
    pub fn glow_scale(self) -> f32 {
        match self {
            Intensity::Off => 0.0,
            Intensity::Low => 0.5,
            Intensity::Medium => 1.0,
            Intensity::Heavy => 1.6,
        }
    }

    /// CRT scanline-overlay opacity for this level.
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
    pub fn radius_scale(self) -> f32 {
        match self {
            GlowLevel::None => 0.0,
            GlowLevel::Thin => 0.5,
            GlowLevel::Medium => 1.0,
            GlowLevel::Large => 2.0,
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
    pub color: String,
    pub alpha: f32,
    pub blur: f32,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: "#000000".to_string(),
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
const SIDEBAR_BG_DARKEN_FACTOR: f32 = 0.05;
const TOP_BOTTOM_PANE_BG_DARKEN_FACTOR: f32 = 0.10;

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

    // ── Typography ──
    pub font_family: String,
    pub font_size: f32,

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

    // ── Effect tokens ──
    #[serde(default)]
    pub glow_size: GlowLevel,
    #[serde(default)]
    pub intensity: Intensity,
    #[serde(default = "default_true")]
    pub show_focus_border: bool,
    #[serde(default = "default_icon_secondary_alpha")]
    pub icon_secondary_alpha: f32,

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
    #[serde(default = "default_terminal_font_family")]
    pub terminal_font_family: String,
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
    #[serde(default)]
    pub terminal_frost_color: Option<Color>,
    #[serde(default = "default_terminal_italic_font_family")]
    pub terminal_italic_font_family: String,
    #[serde(default = "default_terminal_font_size")]
    pub terminal_font_size: f32,
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
    0.45
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
fn default_terminal_font_family() -> String {
    "Maple Mono Normal NF".to_string()
}
fn default_terminal_italic_font_family() -> String {
    default_terminal_font_family()
}
fn default_terminal_font_size() -> f32 {
    14.0
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

    /// Approximate terminal cell metrics for the current terminal font size.
    pub fn terminal_cell_size(&self) -> (f32, f32) {
        const TERMINAL_CELL_WIDTH_RATIO: f32 = 0.58;
        const TERMINAL_CELL_HEIGHT_RATIO: f32 = 1.28;
        (
            self.terminal_font_size * TERMINAL_CELL_WIDTH_RATIO,
            self.terminal_font_size * TERMINAL_CELL_HEIGHT_RATIO,
        )
    }

    /// The default dark, cyan-accented Tron theme.
    pub fn grid_tron() -> Self {
        Self {
            name: "Grid Tron".to_string(),
            background: Color::rgb(6, 10, 14),
            surface: Color::rgb(12, 18, 24),
            foreground: Color::rgb(198, 240, 255),
            muted: Color::rgb(96, 130, 146),
            border: Color::rgb(20, 60, 76),
            accent: Color::rgb(64, 224, 255),
            glow: Color::rgb(64, 224, 255),
            shadow: Shadow::default(),
            danger: Color::rgb(255, 70, 84),
            success: Color::rgb(80, 255, 170),
            warning: Color::rgb(255, 190, 70),
            font_family: "Geist Mono".to_string(),
            font_size: 15.0,
            border_radius: 8.0,
            border_width: 1.0,
            pane_padding: 4.0,
            left_sidebar_background: Some(Color::rgb(5, 9, 13)),
            right_sidebar_background: Some(Color::rgb(5, 9, 13)),
            top_bottom_pane_background: Some(Color::rgb(4, 8, 12)),
            glow_size: GlowLevel::Medium,
            intensity: Intensity::Medium,
            show_focus_border: true,
            icon_secondary_alpha: 0.45,
            float_background: Color::new(49, 50, 68, 255),
            float_accent: Color::new(137, 180, 250, 255),
            float_focus: Color::new(250, 179, 135, 255),
            drag_ghost_bg: Color::new(137, 180, 250, 217),
            drag_ghost_fg: Color::new(255, 255, 255, 255),
            drag_source_bg: Color::new(137, 180, 250, 38),
            drag_source_border: Color::new(137, 180, 250, 255),
            drop_target_bg: Color::new(137, 180, 250, 45),
            drop_target_border: Color::new(137, 180, 250, 255),
            drop_insertion: Color::new(137, 180, 250, 255),
            sidebar_label_font_size: 14.0,
            sidebar_button_font_size: 11.0,
            terminal_font_family: "Maple Mono Normal NF".to_string(),
            terminal_foreground: None,
            terminal_background: None,
            terminal_cursor_foreground: None,
            terminal_cursor_background: None,
            terminal_cursor_border: None,
            terminal_selection_foreground: None,
            terminal_selection_background: None,
            terminal_ansi: None,
            terminal_brights: None,
            terminal_frost_color: None,
            terminal_italic_font_family: "Maple Mono Normal NF".to_string(),
            terminal_font_size: 14.0,
        }
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
        assert!((theme.control_radius() - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn chrome_background_tokens_derive_from_background_when_unset() {
        let mut theme = Theme::grid_tron();
        theme.left_sidebar_background = None;
        theme.right_sidebar_background = None;
        theme.top_bottom_pane_background = None;

        assert_ne!(theme.effective_left_sidebar_background(), theme.background);
        assert_ne!(theme.effective_right_sidebar_background(), theme.background);
        assert_ne!(theme.effective_top_bottom_pane_background(), theme.background);
    }

    #[test]
    fn intensity_glow_scales() {
        assert!((Intensity::Off.glow_scale()).abs() < f32::EPSILON);
        assert!((Intensity::Medium.glow_scale() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn glow_level_radius_scales() {
        assert!((GlowLevel::None.radius_scale()).abs() < f32::EPSILON);
        assert!((GlowLevel::Medium.radius_scale() - 1.0).abs() < f32::EPSILON);
        assert!((GlowLevel::Large.radius_scale() - 2.0).abs() < f32::EPSILON);
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
}
