//! The named choices `[appearance]` takes: window vibrancy, border style, link style, scrollbar
//! visibility.

use super::*;

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

/// Frame decoration style for a container surface (panes, sidebar). Maps onto the
/// grid-ui `PaneFrame` in the app layer. Serialised `snake_case` in TOML (e.g.
/// `pane_border_style = "bracketed"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    /// No border — background fill only.
    None,
    /// A clean continuous border.
    Bordered,
    /// The accent corner-bracket reticle (bright rounded corners + a dimmed line).
    Bracketed,
}

/// The config enum is the user-facing spelling of the theme's [`FrameStyle`];
/// they are the same three choices, so one converts into the other rather than
/// each consumer re-matching. (The theme owns the vocabulary because
/// `heca-grid-ui` reads it at paint time and cannot depend on this crate.)
impl From<BorderStyle> for FrameStyle {
    fn from(s: BorderStyle) -> Self {
        match s {
            BorderStyle::None => FrameStyle::None,
            BorderStyle::Bordered => FrameStyle::Bordered,
            BorderStyle::Bracketed => FrameStyle::Bracketed,
        }
    }
}

/// How OSC 8 hyperlinks are decorated in terminal panes. The link **color** is a
/// separate knob (`hyperlink_color`, default `theme.accent`); this picks the
/// decoration on top of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperlinkStyle {
    /// No special rendering — links look like normal text.
    None,
    /// Recolor only (no line).
    Color,
    /// Recolor + straight underline.
    #[default]
    Underline,
    /// Recolor + wavy undercurl.
    Undercurl,
}

/// When the terminal scrollback scrollbar should be shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollbarVisibility {
    /// Always show the scrollbar when the pane has scrollable history.
    Always,
    /// Show only while the viewport is scrolled away from the live bottom.
    #[default]
    WhenNeeded,
    /// Never show the scrollbar.
    Never,
}
