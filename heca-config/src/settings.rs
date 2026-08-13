use crate::color::Color;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  ModifierKey
// ═══════════════════════════════════════════════════════════════════════════════

/// Modifier keys that can be used for mouse-driven interactive actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ModifierKey {
    /// Super / Command / Windows key.
    #[default]
    Super,
    /// Alt / Option key.
    Alt,
    /// Control key (also accepts "Control" in config).
    #[serde(alias = "Control")]
    Ctrl,
    /// Shift key.
    Shift,
}

/// When the workspace view centres the focused column, mirroring
/// `heca_core::layout::CenterFocusedColumn`.
///
/// Mirrored rather than shared because `heca-config` depends on nothing but the theme — the same
/// arrangement every other layout value here uses, with `startup.rs` converting at the boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CenterFocusedColumn {
    /// Never centre: focusing an off-screen column scrolls it to the nearest edge. **The default**,
    /// matching niri's, and the one that keeps the view where you put it.
    #[default]
    Never,
    /// Centre a column only when it cannot fit on screen beside the previously focused one.
    OnOverflow,
    /// The focused column is always centred.
    Always,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Default-value helpers (used by serde attributes on SettingsConfig)
// ═══════════════════════════════════════════════════════════════════════════════

fn default_search_history() -> bool {
    true
}

fn default_search_history_size() -> usize {
    50
}

fn default_search_usage_size() -> usize {
    500
}

fn default_theme() -> String {
    "grid_tron".to_string()
}

fn default_mouse() -> bool {
    true
}

fn default_window_width() -> u32 {
    1280
}

fn default_window_height() -> u32 {
    800
}

fn default_auto_scroll_edge() -> bool {
    true
}

/// Default overview zoom: `0.5`, niri's — half life size, where a workspace row is half the
/// screen and about one and a half rows are on screen at once.
fn default_overview_zoom() -> f64 {
    0.5
}

/// Default gap between overview rows: a tenth of a screen height, niri's.
fn default_overview_gap() -> f64 {
    0.1
}

/// Default scale the exposé **opens from**, relative to its own map size: `0.8`.
///
/// Below 1.0 the map **grows in** from smaller and shrinks away when it closes — a zoom in on the
/// way in, out on the way out. Above 1.0 it does the opposite: it starts larger and pulls back, the
/// way niri's overview does (the panes you were looking at shrinking into cards). `1.0` is no
/// animation at all.
///
/// Both readings are defensible and this is the one Antonio chose after driving them (2026-08-11):
/// pulling back read as falling in from somewhere, even softened to 1.3, while growing in reads as
/// the map opening. `1 / overview_zoom` (2.0 at the default) is the literal niri reading — the cards
/// start at exactly life size — and overshoots badly: the outer rows begin off-screen.
fn default_overview_zoom_from() -> f64 {
    0.8
}

fn default_always_center_single_column() -> bool {
    false
}

fn default_shell_integration() -> bool {
    true
}

/// Default for appending the process/program name next to a renamed pane's custom name
/// (e.g. `MyPane (nvim)`). On by default so the running program stays visible after a rename.
fn default_pane_renamed_add_process_name() -> bool {
    true
}

/// Default for showing a pane's working directory as its own row in the sidebar card.
/// Off by default — it is opt-in extra detail (the cwd is also available as the info-bar
/// `location` segment).
fn default_pane_show_cwd() -> bool {
    false
}

/// Default host terminal scrollback capacity in rows.
///
/// Mirrors `wezterm-term`'s `TerminalConfiguration::scrollback_size()` default
/// so behaviour is unchanged when the user does not set
/// `terminal_scrollback_lines` in `config.toml`.
fn default_terminal_scrollback_lines() -> usize {
    3500
}

fn default_terminal_mouse() -> bool {
    true
}

fn default_terminal_wheel_scroll_lines() -> usize {
    3
}
/// Default points added/removed per terminal font-zoom step (keyboard notch or
/// `Ctrl`/`Meta`+wheel notch).
fn default_terminal_font_zoom_step() -> f32 {
    1.0
}
/// Default for whether `Ctrl`/`Meta`+wheel changes the font size. On by default;
/// set false to reserve the modified wheel for the terminal/app instead.
fn default_mouse_wheel_change_font_size() -> bool {
    true
}
fn default_terminal_scroll_animations() -> bool {
    true
}

/// Chrome regions are shown by default; a `show_*` toggle set to `false` fully
/// hides that region (the sidebar collapses to `RegionMode::Hidden`; the tab /
/// status bar collapses to zero height and the pane area reclaims the space).
fn default_show_chrome_region() -> bool {
    true
}

// ═══════════════════════════════════════════════════════════════════════════════
//  SettingsConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// How roomy the command palette is — `small` / `normal` / `large`, as in Zed.
///
/// It decides the panel's **maximum width and how many rows it shows**, not the text size: the
/// palette is read at the same size whatever its width. On a screen too small for the chosen
/// variant the window wins — the panel is capped at a fraction of the viewport, so `large` on a
/// laptop is simply as large as fits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteSize {
    /// 560px, 6 rows.
    Small,
    /// 700px, 8 rows.
    #[default]
    Normal,
    /// 1000px, 12 rows.
    Large,
}

/// How a search query's case is treated — the command palette today, and any search surface that
/// follows it.
///
/// Three, because the right answer is a habit rather than a fact: `smart` is what fzf and ripgrep
/// do and suits most people, `sensitive` suits anyone whose command names differ only by case, and
/// `insensitive` suits anyone who never wants Shift to change what they find.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchCase {
    /// Insensitive until the query contains an uppercase character, then sensitive.
    #[default]
    Smart,
    /// Always case-sensitive.
    Sensitive,
    /// Never case-sensitive.
    Insensitive,
}

/// User-configurable settings that control behaviour and appearance.
///
/// Font family/size configuration has moved out of `[settings]` into the
/// dedicated `[font]` block (see [`crate::font::FontConfig`]). The fields below
/// cover behaviour, window geometry, and terminal color palette overrides only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_mouse")]
    pub mouse: bool,
    /// How roomy the command palette is (`small` / `normal` / `large`).
    #[serde(default, alias = "command-palette-size")]
    pub command_palette_size: PaletteSize,
    /// How a search query's case is treated (`smart` / `sensitive` / `insensitive`).
    #[serde(default, alias = "search-case")]
    pub search_case: SearchCase,
    /// Remember what has been searched for and chosen, across restarts.
    #[serde(default = "default_search_history", alias = "search-history")]
    pub search_history: bool,
    /// How many past queries each search surface keeps.
    #[serde(default = "default_search_history_size", alias = "search-history-size")]
    pub search_history_size: usize,
    /// How many commands' usage counts each search surface keeps for ranking.
    #[serde(default = "default_search_usage_size", alias = "search-usage-size")]
    pub search_usage_size: usize,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    /// Optional terminal default foreground override from `config.toml`.
    #[serde(default, alias = "terminal-foreground")]
    pub terminal_foreground: Option<Color>,
    /// Optional terminal default background override from `config.toml`.
    #[serde(default, alias = "terminal-background")]
    pub terminal_background: Option<Color>,
    /// Optional terminal cursor foreground override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-foreground")]
    pub terminal_cursor_foreground: Option<Color>,
    /// Optional terminal cursor background override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-background")]
    pub terminal_cursor_background: Option<Color>,
    /// Optional terminal cursor border override from `config.toml`.
    #[serde(default, alias = "terminal-cursor-border")]
    pub terminal_cursor_border: Option<Color>,
    /// Optional terminal selection foreground override from `config.toml`.
    #[serde(default, alias = "terminal-selection-foreground")]
    pub terminal_selection_foreground: Option<Color>,
    /// Optional terminal selection background override from `config.toml`.
    #[serde(default, alias = "terminal-selection-background")]
    pub terminal_selection_background: Option<Color>,
    /// Optional terminal ANSI `0..7` palette override from `config.toml`.
    #[serde(default, alias = "terminal-ansi")]
    pub terminal_ansi: Option<[Color; 8]>,
    /// Optional terminal bright ANSI `8..15` palette override from `config.toml`.
    #[serde(default, alias = "terminal-brights")]
    pub terminal_brights: Option<[Color; 8]>,
    /// Automatically scroll the workspace view when the pointer hovers near the left/right edge.
    #[serde(default = "default_auto_scroll_edge")]
    pub auto_scroll_edge: bool,
    /// Modifier key that must be held to initiate an interactive pane drag with the mouse.
    #[serde(default)]
    pub interactive_move_modifier: ModifierKey,
    /// Center a single column even when it fits within the viewport.
    #[serde(default = "default_always_center_single_column")]
    pub always_center_single_column: bool,
    /// When focusing a column re-centres the view: `never` | `on_overflow` | `always`.
    #[serde(default)]
    pub center_focused_column: CenterFocusedColumn,
    /// Zoom of the workspace thumbnails in overview mode, as a fraction of life size (niri's
    /// `overview.zoom`). Clamped to `0.05..=0.75`.
    ///
    /// ⚠️ **The exposé does not read it** (F003/P082/T420): the map is built out of shares of the
    /// window, so it fits by construction and has no scale to set.
    #[serde(default = "default_overview_zoom")]
    pub overview_zoom: f64,
    /// Gap between workspace rows in the exposé, as a fraction of a screen height.
    #[serde(default = "default_overview_gap")]
    pub overview_gap: f64,
    /// The scale the exposé **opens from**, relative to its map size — the zoom it animates out of
    /// and back into. `1.0` disables the animation. Clamped to `0.2..=4.0`.
    #[serde(default = "default_overview_zoom_from")]
    pub overview_zoom_from: f64,
    /// Auto-inject shell integration snippets for OSC 133/OSC 7 pane runtime signals.
    #[serde(default = "default_shell_integration")]
    pub shell_integration: bool,
    /// When a pane has a **custom name** (set via rename), also show its process/program name as
    /// a small dimmed label next to it (e.g. `MyPane (nvim)`). The process label is *not* part of
    /// the name — it is appended for reference only and never edited by rename. Set `false` to
    /// show just the custom name.
    #[serde(
        default = "default_pane_renamed_add_process_name",
        alias = "pane-renamed-add-process-name"
    )]
    pub pane_renamed_add_process_name: bool,
    /// Show each pane's working directory as its own row (folder icon + home-relative path)
    /// in the sidebar pane card, between the name row and the git-status row. Off by default;
    /// the cwd is also available as the info-bar `location` segment.
    #[serde(default = "default_pane_show_cwd", alias = "pane-show-cwd")]
    pub pane_show_cwd: bool,
    /// Host terminal scrollback capacity in rows.
    ///
    /// This is the number of history rows the terminal engine retains above the
    /// visible viewport. The host scrollback viewport (see `terminal-01a`) scrolls
    /// within `[0, terminal_scrollback_lines]`. Mirrors wezterm-term's default of
    /// 3500 when unset.
    #[serde(default = "default_terminal_scrollback_lines", alias = "terminal-scrollback-lines")]
    pub terminal_scrollback_lines: usize,

    /// Enable terminal mouse support for host scrollback: when `true`, wheel events
    /// scroll the host scrollback viewport instead of forwarding to the terminal
    /// (unless the terminal application has grabbed the mouse). Shift+wheel always
    /// scrolls the host viewport regardless.
    ///
    /// This is terminal-ONLY — it does not affect `settings.mouse` (which controls
    /// chrome interaction like click-to-focus and drag-to-move).
    #[serde(default = "default_terminal_mouse", alias = "terminal-mouse")]
    pub terminal_mouse: bool,

    /// Number of scrollback rows per wheel notch when the host scrollback viewport
    /// is active (i.e. the wheel scrolls the host viewport, not the terminal).
    #[serde(default = "default_terminal_wheel_scroll_lines", alias = "terminal-wheel-scroll-lines")]
    pub terminal_wheel_scroll_lines: usize,

    /// Points added/removed per terminal font-zoom step (keyboard `prefix+Ctrl/Alt`
    /// bindings and `Ctrl`/`Meta`+wheel). Applies to both the app-wide and the
    /// per-pane zoom. Non-positive values fall back to the default step.
    #[serde(default = "default_terminal_font_zoom_step", alias = "terminal-font-zoom-step")]
    pub terminal_font_zoom_step: f32,

    /// Whether `Ctrl`/`Meta`+mouse-wheel changes the font size (app-wide over
    /// chrome, focused pane over a pane). When false the modified wheel is left
    /// alone (forwarded like a normal wheel), and font zoom stays keyboard-only.
    #[serde(
        default = "default_mouse_wheel_change_font_size",
        alias = "mouse-wheel-change-font-size"
    )]
    pub mouse_wheel_change_font_size: bool,

    /// Enable smooth animation for backend-side discrete terminal viewport jumps.
    /// When false, animated scroll APIs degrade to immediate scroll.
    #[serde(default = "default_terminal_scroll_animations", alias = "terminal-scroll-animations")]
    pub terminal_scroll_animations: bool,

    /// Show the left sidebar region on startup. `false` starts it hidden
    /// (`RegionMode::Hidden`); the runtime toggle can still reveal it.
    #[serde(default = "default_show_chrome_region", alias = "show-left-sidebar")]
    pub show_left_sidebar: bool,
    /// Show the right sidebar region on startup. `false` starts it hidden.
    #[serde(default = "default_show_chrome_region", alias = "show-right-sidebar")]
    pub show_right_sidebar: bool,
    /// Show the top bar (tab bar). `false` fully hides it (zero height) and the
    /// pane area reclaims the space.
    #[serde(default = "default_show_chrome_region", alias = "show-top-bar")]
    pub show_top_bar: bool,
    /// Show the bottom bar (status bar). `false` fully hides it (zero height).
    #[serde(default = "default_show_chrome_region", alias = "show-bottom-bar")]
    pub show_bottom_bar: bool,
    // Destructive-action confirmation moved to the generic `[confirm]` table
    // (`ConfirmConfig`, keyed by action name: `close` / `delete_column` / `delete_workspace`).
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            mouse: default_mouse(),
            command_palette_size: PaletteSize::default(),
            search_case: SearchCase::default(),
            search_history: default_search_history(),
            search_history_size: default_search_history_size(),
            search_usage_size: default_search_usage_size(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            terminal_foreground: None,
            terminal_background: None,
            terminal_cursor_foreground: None,
            terminal_cursor_background: None,
            terminal_cursor_border: None,
            terminal_selection_foreground: None,
            terminal_selection_background: None,
            terminal_ansi: None,
            terminal_brights: None,
            auto_scroll_edge: default_auto_scroll_edge(),
            interactive_move_modifier: ModifierKey::default(),
            always_center_single_column: default_always_center_single_column(),
            center_focused_column: CenterFocusedColumn::default(),
            overview_zoom: default_overview_zoom(),
            overview_zoom_from: default_overview_zoom_from(),
            overview_gap: default_overview_gap(),
            shell_integration: default_shell_integration(),
            pane_renamed_add_process_name: default_pane_renamed_add_process_name(),
            pane_show_cwd: default_pane_show_cwd(),
            terminal_scrollback_lines: default_terminal_scrollback_lines(),
            terminal_mouse: default_terminal_mouse(),
            terminal_wheel_scroll_lines: default_terminal_wheel_scroll_lines(),
            terminal_font_zoom_step: default_terminal_font_zoom_step(),
            mouse_wheel_change_font_size: default_mouse_wheel_change_font_size(),
            terminal_scroll_animations: default_terminal_scroll_animations(),
            show_left_sidebar: default_show_chrome_region(),
            show_right_sidebar: default_show_chrome_region(),
            show_top_bar: default_show_chrome_region(),
            show_bottom_bar: default_show_chrome_region(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_config_default_values() {
        let s = SettingsConfig::default();
        assert_eq!(s.theme, "grid_tron");
        assert!(s.mouse);
        assert_eq!(s.window_width, 1280);
        assert_eq!(s.window_height, 800);
        assert_eq!(s.terminal_foreground, None);
        assert_eq!(s.terminal_background, None);
        assert_eq!(s.terminal_cursor_foreground, None);
        assert_eq!(s.terminal_cursor_background, None);
        assert_eq!(s.terminal_cursor_border, None);
        assert_eq!(s.terminal_selection_foreground, None);
        assert_eq!(s.terminal_selection_background, None);
        assert_eq!(s.terminal_ansi, None);
        assert_eq!(s.terminal_brights, None);
        assert!(s.auto_scroll_edge);
        assert_eq!(s.interactive_move_modifier, ModifierKey::Super);
        assert!(!s.always_center_single_column);
        assert!(s.shell_integration);
        assert_eq!(s.terminal_scrollback_lines, 3500);
        assert!(s.terminal_mouse);
        assert_eq!(s.terminal_wheel_scroll_lines, 3);
        assert!(s.terminal_scroll_animations);
        // Chrome regions are all shown by default.
        assert!(s.show_left_sidebar);
        assert!(s.show_right_sidebar);
        assert!(s.show_top_bar);
        assert!(s.show_bottom_bar);
        // Destructive-action confirmation now lives in the `[confirm]` table (see confirm.rs).
    }

    #[test]
    fn test_show_chrome_region_toggles_parse() {
        let s: SettingsConfig = toml::from_str(
            "show_left_sidebar = false\n\
             show_right_sidebar = false\n\
             show_top_bar = false\n\
             show_bottom_bar = false\n",
        )
        .expect("chrome region toggles should parse");
        assert!(!s.show_left_sidebar);
        assert!(!s.show_right_sidebar);
        assert!(!s.show_top_bar);
        assert!(!s.show_bottom_bar);

        // Kebab-case aliases parse too.
        let k: SettingsConfig =
            toml::from_str("show-top-bar = false\n").expect("kebab alias should parse");
        assert!(!k.show_top_bar);
        assert!(k.show_bottom_bar);
    }

    #[test]
    fn test_modifier_key_control_alias() {
        #[derive(Deserialize)]
        struct Wrap {
            #[serde(default)]
            m: ModifierKey,
        }

        let ctrl: Wrap = toml::from_str(r#"m = "Control""#).unwrap();
        assert_eq!(ctrl.m, ModifierKey::Ctrl);

        let pascal: Wrap = toml::from_str(r#"m = "Ctrl""#).unwrap();
        assert_eq!(pascal.m, ModifierKey::Ctrl);
    }

    #[test]
    fn test_terminal_foreground_override_parses() {
        let s: SettingsConfig = toml::from_str("terminal-foreground = \"#4c4f69\"").unwrap();
        assert!(s.terminal_foreground.is_some());
    }
}
