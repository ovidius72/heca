pub use crate::color::Color;
pub use crate::keys::{
    BindingValue, CommandKeybindConfig, KeybindingMap, KeyModeConfig, KeysConfig,
    ModeBindingConfig,
};
pub use crate::loader::{config_dir, AppConfig, Config};
pub use crate::settings::{ModifierKey, SettingsConfig};
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  Shadow & Theme
// ═══════════════════════════════════════════════════════════════════════════════

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub accent: Color,
    pub font_family: String,
    pub font_size: f32,
    #[serde(default = "crate::defaults::default_terminal_font_family")]
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
    /// Frosted tint color stamped behind translucent terminals (the
    /// `terminal_blur` frost). `None` → falls back to `background` (app theme
    /// bg). Per-theme override for the frost; config `[appearance]`
    /// `terminal_frost_color` wins over this.
    #[serde(default)]
    pub terminal_frost_color: Option<Color>,
    #[serde(default = "crate::defaults::default_terminal_italic_font_family")]
    pub terminal_italic_font_family: String,
    #[serde(default = "crate::defaults::default_terminal_font_size")]
    pub terminal_font_size: f32,
    pub border_radius: f32,
    pub border_width: f32,
    /// Internal padding inside panes (logical px). Fallback for
    /// `appearance.pane_padding` when unset; clamped to `[0, 20]` downstream.
    pub pane_padding: f32,
    pub shadow: Shadow,
    #[serde(default = "crate::defaults::default_float_bg")]
    pub float_background: Color,
    #[serde(default = "crate::defaults::default_float_accent")]
    pub float_accent: Color,
    #[serde(default = "crate::defaults::default_float_focus")]
    pub float_focus: Color,
    // ── Drag-and-drop colors (generic; any surface/widget, not just the sidebar).
    //    Old `sidebar_drag_*` keys still parse via serde aliases. ──
    #[serde(default = "crate::defaults::default_drag_ghost_bg", alias = "sidebar_drag_ghost_bg")]
    pub drag_ghost_bg: Color,
    #[serde(default = "crate::defaults::default_drag_ghost_fg", alias = "sidebar_drag_ghost_fg")]
    pub drag_ghost_fg: Color,
    #[serde(default = "crate::defaults::default_drag_source_bg", alias = "sidebar_drag_source_bg")]
    pub drag_source_bg: Color,
    #[serde(default = "crate::defaults::default_drag_source_border", alias = "sidebar_drag_source_border")]
    pub drag_source_border: Color,
    /// Fill highlight for a drop zone the cursor is hovering over a valid drop.
    #[serde(default = "crate::defaults::default_drop_target_bg")]
    pub drop_target_bg: Color,
    /// Border/outline of a hovered drop zone.
    #[serde(default = "crate::defaults::default_drop_target_border")]
    pub drop_target_border: Color,
    /// The insertion-line color drawn between items to show where a drop lands.
    #[serde(default = "crate::defaults::default_drop_insertion")]
    pub drop_insertion: Color,
    // ── Sidebar font sizes ──
    #[serde(default = "crate::defaults::default_sidebar_label_font_size")]
    pub sidebar_label_font_size: f32,
    #[serde(default = "crate::defaults::default_sidebar_button_font_size")]
    pub sidebar_button_font_size: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::catppuccin_mocha()
    }
}

impl Theme {
    pub fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".to_string(),
            background: Color::new(30, 30, 46, 255),
            foreground: Color::new(205, 214, 244, 255),
            border: Color::new(49, 50, 68, 255),
            accent: Color::new(137, 180, 250, 255),
            font_family: "JetBrainsMono Nerd Font".to_string(),
            font_size: 32.0,
            terminal_font_family: crate::defaults::default_terminal_font_family(),
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
            terminal_italic_font_family: crate::defaults::default_terminal_italic_font_family(),
            terminal_font_size: crate::defaults::default_terminal_font_size(),
            border_radius: 6.0,
            border_width: 1.0,
            pane_padding: 4.0,
            shadow: Shadow::default(),
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
        }
    }

    pub fn catppuccin_latte() -> Self {
        Self {
            name: "Catppuccin Latte".to_string(),
            background: Color::new(239, 241, 245, 255),
            foreground: Color::new(76, 79, 105, 255),
            border: Color::new(204, 208, 218, 255),
            accent: Color::new(30, 102, 245, 255),
            font_family: "JetBrainsMono Nerd Font".to_string(),
            font_size: 32.0,
            terminal_font_family: crate::defaults::default_terminal_font_family(),
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
            terminal_italic_font_family: crate::defaults::default_terminal_italic_font_family(),
            terminal_font_size: crate::defaults::default_terminal_font_size(),
            border_radius: 6.0,
            border_width: 1.0,
            pane_padding: 4.0,
            shadow: Shadow {
                color: "#000000".to_string(),
                alpha: 0.15,
                blur: 8.0,
            },
            float_background: Color::new(204, 208, 218, 255),
            float_accent: Color::new(30, 102, 245, 255),
            float_focus: Color::new(230, 126, 34, 255),
            drag_ghost_bg: Color::new(30, 102, 245, 217),
            drag_ghost_fg: Color::new(255, 255, 255, 255),
            drag_source_bg: Color::new(30, 102, 245, 38),
            drag_source_border: Color::new(30, 102, 245, 255),
            drop_target_bg: Color::new(30, 102, 245, 45),
            drop_target_border: Color::new(30, 102, 245, 255),
            drop_insertion: Color::new(30, 102, 245, 255),
            sidebar_label_font_size: 14.0,
            sidebar_button_font_size: 11.0,
        }
    }

    pub fn load(name: &str) -> Self {
        crate::loader::load_theme(name)
    }

    /// Approximate terminal cell metrics for the current terminal font size.
    ///
    /// The terminal renderer still shares generic text layout internals, so
    /// these ratios intentionally bias toward the real visual advance/line box
    /// of Maple Mono NF in live terminal workloads rather than theoretical font
    /// metrics. They are tuned to reduce:
    /// - accumulated cursor drift at the end of typed lines
    /// - right-edge slack/gaps in full-screen TUIs such as Telescope
    /// - bottom slack where the PTY grid does not visually fill the pane
    ///
    /// These remain transitional constants until the dedicated terminal glyph
    /// path measures and owns terminal font metrics directly.
    pub fn terminal_cell_size(&self) -> (f32, f32) {
        const TERMINAL_CELL_WIDTH_RATIO: f32 = 0.58;
        const TERMINAL_CELL_HEIGHT_RATIO: f32 = 1.28;
        (
            self.terminal_font_size * TERMINAL_CELL_WIDTH_RATIO,
            self.terminal_font_size * TERMINAL_CELL_HEIGHT_RATIO,
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme_is_mocha() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Catppuccin Mocha");
    }

    #[test]
    fn test_bundled_mocha_theme() {
        assert_eq!(Theme::load("mocha").name, "Catppuccin Mocha");
    }

    #[test]
    fn test_bundled_latte_theme() {
        assert_eq!(Theme::load("latte").name, "Catppuccin Latte");
    }

}
