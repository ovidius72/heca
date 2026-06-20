use crate::color::Color;

// ═══════════════════════════════════════════════════════════════════════════════
//  Theme serde-default helpers
// ═══════════════════════════════════════════════════════════════════════════════

pub(crate) fn default_float_bg() -> Color {
    Color::new(49, 50, 68, 255)
}

pub(crate) fn default_float_accent() -> Color {
    Color::new(137, 180, 250, 255)
}

pub(crate) fn default_float_focus() -> Color {
    Color::new(250, 179, 135, 255)
}

pub(crate) fn default_drag_ghost_bg() -> Color {
    Color::new(137, 180, 250, 217)
} // accent @ 85%

pub(crate) fn default_drag_ghost_fg() -> Color {
    Color::new(255, 255, 255, 255)
} // white

pub(crate) fn default_drag_source_bg() -> Color {
    Color::new(137, 180, 250, 38)
} // accent @ 15%

pub(crate) fn default_drag_source_border() -> Color {
    Color::new(137, 180, 250, 255)
} // accent

pub(crate) fn default_drop_target_bg() -> Color {
    Color::new(137, 180, 250, 45)
} // accent @ ~18%

pub(crate) fn default_drop_target_border() -> Color {
    Color::new(137, 180, 250, 255)
} // accent

pub(crate) fn default_drop_insertion() -> Color {
    Color::new(137, 180, 250, 255)
} // accent

pub(crate) fn default_sidebar_label_font_size() -> f32 {
    14.0
}

pub(crate) fn default_sidebar_button_font_size() -> f32 {
    11.0
}

/// Default **UI/chrome** font family (sidebar, pane info bar, status bar). Kept
/// independent of the color theme — color presets carry colors only; the family
/// comes from here (or a `[settings] font_family` override).
pub(crate) fn default_font_family() -> String {
    "Geist Mono".to_string()
}

/// Default **UI/chrome** font size — the real size the chrome renders at (the
/// grid-ui `GuiTheme` base font). Decoupled from the color theme; overridable via
/// `[settings] font_size`.
pub(crate) fn default_font_size() -> f32 {
    15.0
}

pub(crate) fn default_terminal_font_family() -> String {
    "Maple Mono Normal NF".to_string()
}

pub(crate) fn default_terminal_italic_font_family() -> String {
    default_terminal_font_family()
}

pub(crate) fn default_terminal_font_size() -> f32 {
    14.0
}
