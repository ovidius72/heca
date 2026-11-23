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

/// Opacity (0..=255) of a scrollback-search match's highlight, and of the current match's (bolder),
/// when `search_match_alpha` / `search_current_match_alpha` are unset.
fn default_terminal_search_match_alpha() -> u8 {
    64
}
fn default_terminal_search_current_match_alpha() -> u8 {
    150
}

/// Terminal content padding, **resolved per side** (logical px): how much room
/// the terminal keeps between the pane's edge and its grid. [`TerminalAppearance`]
/// carries the raw config words; this is what the app hands the terminal.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerminalPadding {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
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
    /// Extra space between the pane's edge and the terminal grid, **all four
    /// sides**, logical px. `0` (default) leaves the grid filling the pane; a
    /// value holds the grid in from every side.
    #[serde(default)]
    pub padding: Option<f64>,
    /// Per-side overrides of [`Self::padding`]: whichever of
    /// `padding_top`/`padding_bottom`/`padding_left`/`padding_right` is written
    /// decides that side alone. Defaults: `0`.
    #[serde(default)]
    pub padding_top: Option<f64>,
    #[serde(default)]
    pub padding_bottom: Option<f64>,
    #[serde(default)]
    pub padding_left: Option<f64>,
    #[serde(default)]
    pub padding_right: Option<f64>,
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
            search_match_alpha: default_terminal_search_match_alpha(),
            search_current_match_alpha: default_terminal_search_current_match_alpha(),
            images: default_terminal_images(),
            padding: None,
            padding_top: None,
            padding_bottom: None,
            padding_left: None,
            padding_right: None,
        }
    }
}

impl TerminalAppearance {
    /// The terminal's content padding: [`Self::padding`] for every side first,
    /// then each `padding_{top,bottom,left,right}` deciding its own side. Every
    /// value clamps at `0` — a terminal cannot crop into its frame.
    pub fn effective_terminal_padding(&self) -> TerminalPadding {
        let all = self.padding.unwrap_or(0.0).max(0.0);
        TerminalPadding {
            top: self.padding_top.unwrap_or(all).max(0.0),
            bottom: self.padding_bottom.unwrap_or(all).max(0.0),
            left: self.padding_left.unwrap_or(all).max(0.0),
            right: self.padding_right.unwrap_or(all).max(0.0),
        }
    }
}
