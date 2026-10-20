//! **The terminal's look** — `[appearance.terminal]`.

use super::*;

fn default_terminal_transparency() -> u8 {
    0
}

fn default_terminal_floating_transparency() -> u8 {
    0
}

fn default_terminal_floating_blur() -> u8 {
    0
}
fn default_terminal_show_scrollbar() -> ScrollbarVisibility {
    ScrollbarVisibility::WhenNeeded
}

fn default_terminal_show_scrolled_up_badge() -> bool {
    true
}

/// Default terminal ligatures: `true` (coding-font ligatures like `->`/`=>`/`!=`
/// render). Set `false` to disable `calt`/`liga`/`clig` for the terminal font.
fn default_terminal_ligatures() -> bool {
    true
}

/// Default hyperlink decoration: a straight underline (plus the link color).
fn default_terminal_hyperlink_style() -> HyperlinkStyle {
    HyperlinkStyle::Underline
}

/// Default URL auto-detection: `true` (plain-text URLs become clickable links).
fn default_terminal_link_detection() -> bool {
    true
}

/// Default inline-image rendering: `true`.
fn default_terminal_images() -> bool {
    true
}

/// Default bell window-attention policy: `true` (request the OS attention cue —
/// Dock bounce / taskbar flash — on a bell while the window is unfocused).
fn default_terminal_bell_attention() -> bool {
    true
}

/// Default visual-bell policy: `false` (no on-screen flash).
fn default_terminal_bell_visual() -> bool {
    false
}

/// Default audible-bell policy: `false` (no system beep).
fn default_terminal_bell_audible() -> bool {
    false
}

/// Peak opacity (0..=255) of the visual-bell flash when `bell_flash_alpha` is unset.
fn default_terminal_bell_flash_alpha() -> u8 {
    56
}

/// Opacity (0..=255) of a scrollback-search match's highlight, and of the current match's (bolder),
/// when `search_match_alpha` / `search_current_match_alpha` are unset.
fn default_terminal_search_match_alpha() -> u8 {
    64
}
fn default_terminal_search_current_match_alpha() -> u8 {
    150
}

/// Terminal **content** surface translucency/blur (`[appearance.terminal]`). The
/// pane *frame* (border/radius/gap) lives in [`PaneAppearance`] — this owns only
/// the see-through-ness of the terminal surface itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalAppearance {
    /// Tiled terminal-pane transparency, `0..=100` (`0` opaque, `100`
    /// see-through). Surface alpha composited over the z=0 background.
    #[serde(default = "default_terminal_transparency")]
    pub transparency: u8,
    /// Floating terminal-pane transparency, `0..=100` (`0` opaque). Independent of
    /// the tiled `transparency`, so floating panes can stay readable while tiled
    /// panes are frosted. Default `0` (opaque).
    #[serde(default = "default_terminal_floating_transparency")]
    pub floating_transparency: u8,
    /// Floating terminal-pane real-blur strength, `0..=100` (`0` = off).
    /// Independent of the z=0 `background_blur`. Default `0` (no blur).
    #[serde(default = "default_terminal_floating_blur")]
    pub floating_blur: u8,
    /// Scrollbar visibility for panes with host-managed scrollback.
    /// `always | when_needed | never`. Default `when_needed`.
    #[serde(default = "default_terminal_show_scrollbar")]
    pub show_scrollbar: ScrollbarVisibility,
    /// Whether to show the "N lines above" badge while scrolled up.
    /// Default `true`.
    #[serde(default = "default_terminal_show_scrolled_up_badge")]
    pub show_scrolled_up_badge: bool,
    /// Coding-font ligatures (`->`, `=>`, `!=`, …) in terminal panes. When
    /// `false`, `calt`/`liga`/`clig` are disabled in the terminal shaping path so
    /// each character renders standalone. Default `true`. Terminal font only —
    /// UI/chrome text is unaffected.
    #[serde(default = "default_terminal_ligatures")]
    pub ligatures: bool,
    /// Decoration for OSC 8 hyperlinks (`none | color | underline | undercurl`).
    /// Default `underline`. Pairs with `hyperlink_color`.
    #[serde(default = "default_terminal_hyperlink_style")]
    pub hyperlink_style: HyperlinkStyle,
    /// Hyperlink color. `None` → `theme.accent`. Applied for every style except
    /// `none`.
    #[serde(default)]
    pub hyperlink_color: Option<Color>,
    /// Auto-detect plain-text URLs (`echo`, logs, …) and make them links too,
    /// not just explicit OSC 8 links. Default `true`.
    #[serde(default = "default_terminal_link_detection")]
    pub link_detection: bool,
    /// On a terminal **bell** (`\a` / `Alert::Bell`), request the OS window
    /// attention cue (Dock bounce / taskbar flash) while the window is unfocused.
    /// Default `true`.
    #[serde(default = "default_terminal_bell_attention")]
    pub bell_attention: bool,
    /// On a terminal bell, flash the content area briefly (visual bell). Fires
    /// regardless of focus. Default `false`.
    #[serde(default = "default_terminal_bell_visual")]
    pub bell_visual: bool,
    /// On a terminal bell, emit a system beep (audible bell). Fires regardless of
    /// focus. macOS only for now (`NSBeep`); a no-op elsewhere. Default `false`.
    #[serde(default = "default_terminal_bell_audible")]
    pub bell_audible: bool,
    /// How bright the visual bell's flash is at its peak, 0 (invisible) … 255 (solid). It fades out
    /// over the flash window. Only used with `bell_visual`.
    #[serde(default = "default_terminal_bell_flash_alpha")]
    pub bell_flash_alpha: u8,
    /// Opacity of the highlight over every scrollback-search match, 0 … 255.
    #[serde(default = "default_terminal_search_match_alpha")]
    pub search_match_alpha: u8,
    /// Opacity of the highlight over the match the search is on, 0 … 255 — bolder than the rest so
    /// it can be told apart.
    #[serde(default = "default_terminal_search_current_match_alpha")]
    pub search_current_match_alpha: u8,
    /// Render inline images emitted by terminal programs (iTerm2 `OSC 1337`,
    /// Sixel, Kitty graphics — e.g. Yazi previews). Disable to skip image capture
    /// and rendering entirely. Default `true`.
    #[serde(default = "default_terminal_images")]
    pub images: bool,
}

impl Default for TerminalAppearance {
    fn default() -> Self {
        Self {
            transparency: default_terminal_transparency(),
            floating_transparency: default_terminal_floating_transparency(),
            floating_blur: default_terminal_floating_blur(),
            show_scrollbar: default_terminal_show_scrollbar(),
            show_scrolled_up_badge: default_terminal_show_scrolled_up_badge(),
            ligatures: default_terminal_ligatures(),
            hyperlink_style: default_terminal_hyperlink_style(),
            hyperlink_color: None,
            link_detection: default_terminal_link_detection(),
            bell_attention: default_terminal_bell_attention(),
            bell_visual: default_terminal_bell_visual(),
            bell_audible: default_terminal_bell_audible(),
            bell_flash_alpha: default_terminal_bell_flash_alpha(),
            search_match_alpha: default_terminal_search_match_alpha(),
            search_current_match_alpha: default_terminal_search_current_match_alpha(),
            images: default_terminal_images(),
        }
    }
}
