//! `[settings]` — the user-configurable behaviour and window geometry settings.

use crate::color::Color;
use serde::{Deserialize, Serialize};

mod defaults;
mod enums;
mod notifications;
#[cfg(test)]
mod tests;

pub use enums::{CenterFocusedColumn, ModifierKey, PaletteSize, SearchCase};
pub use notifications::{NotificationSystem, NotificationSystemConfig};

use defaults::*;

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
    /// How close (logical px) the pointer must come to the content area's edge, while dragging a
    /// pane, for the view to start scrolling. Only used with `auto_scroll_edge`. Default 80.
    #[serde(default = "default_edge_scroll_distance")]
    pub edge_scroll_distance: f32,
    /// Modifier key that must be held to initiate an interactive pane drag with the mouse.
    #[serde(default)]
    pub interactive_move_modifier: ModifierKey,
    /// Modifier key that turns a drag from a **move** into a **swap** — dropping exchanges the two
    /// things instead of placing one at the other's position. Applies to every drag alike: a row in
    /// a sidebar, a pane carried across the content area, a plugin's own row.
    ///
    /// Must differ from [`interactive_move_modifier`](Self::interactive_move_modifier): one key
    /// cannot both start a drag and change what it means, or every drag is a swap and a plain move
    /// becomes unreachable. Heca reports the collision at startup and keeps the gesture.
    #[serde(default = "default_swap_modifier")]
    pub swap_modifier: ModifierKey,
    /// Center a single column even when it fits within the viewport.
    #[serde(default = "default_always_center_single_column")]
    pub always_center_single_column: bool,
    /// When focusing a column re-centres the view: `never` | `on_overflow` | `always`.
    #[serde(default)]
    pub center_focused_column: CenterFocusedColumn,
    /// Gap between workspace rows in the exposé, as a fraction of a screen height.
    #[serde(default = "default_overview_gap")]
    pub overview_gap: f64,
    /// The scale the exposé **opens from**, relative to its map size — the zoom it animates out of
    /// and back into. `1.0` disables the animation. Clamped to `0.2..=4.0`.
    #[serde(default = "default_overview_zoom_from")]
    pub overview_zoom_from: f64,
    /// How much of the working area a pane takes when it floats with nowhere given — a fraction of
    /// each side, centred. Clamped to `0.1..=1.0`.
    #[serde(default = "default_float_size")]
    pub float_size: f64,
    /// How far a pane that moved or swapped animates in from, as a fraction of the window.
    /// `0` makes moves jump. Clamped to `0.0..=1.0`.
    #[serde(default = "default_move_slide_reach")]
    pub move_slide_reach: f64,
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
    /// **Deprecated: `[appearance.expose] show_cwd`.** Still read and mapped onto it, with a one-time
    /// notice to move it. It once switched the sidebar's folder line too; that line is now decided by
    /// `[appearance.sidebar] pane_lines` alone.
    #[serde(default = "default_pane_show_cwd", alias = "pane-show-cwd")]
    pub pane_show_cwd: bool,
    /// Host terminal scrollback capacity in rows.
    ///
    /// This is the number of history rows the terminal engine retains above the
    /// visible viewport. The host scrollback viewport (see `terminal-01a`) scrolls
    /// within `[0, terminal_scrollback_lines]`. Mirrors wezterm-term's default of
    /// 3500 when unset.
    #[serde(
        default = "default_terminal_scrollback_lines",
        alias = "terminal-scrollback-lines"
    )]
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
    #[serde(
        default = "default_terminal_wheel_scroll_lines",
        alias = "terminal-wheel-scroll-lines"
    )]
    pub terminal_wheel_scroll_lines: usize,

    /// Points added/removed per terminal font-zoom step (keyboard `prefix+Ctrl/Alt`
    /// bindings and `Ctrl`/`Meta`+wheel). Applies to both the app-wide and the
    /// per-pane zoom. Non-positive values fall back to the default step.
    #[serde(
        default = "default_terminal_font_zoom_step",
        alias = "terminal-font-zoom-step"
    )]
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
    #[serde(
        default = "default_terminal_scroll_animations",
        alias = "terminal-scroll-animations"
    )]
    pub terminal_scroll_animations: bool,

    /// Show the left sidebar region on startup. `false` starts it hidden; the runtime toggle
    /// (`prefix+b`) shows it, and a config reload puts it back to this value.
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

    /// `[settings.notification_system]` — in-app toast notification behaviour.
    #[serde(default, alias = "notification-system")]
    pub notification_system: NotificationSystemConfig,
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
            edge_scroll_distance: default_edge_scroll_distance(),
            interactive_move_modifier: ModifierKey::default(),
            swap_modifier: default_swap_modifier(),
            always_center_single_column: default_always_center_single_column(),
            center_focused_column: CenterFocusedColumn::default(),
            overview_zoom_from: default_overview_zoom_from(),
            overview_gap: default_overview_gap(),
            float_size: default_float_size(),
            move_slide_reach: default_move_slide_reach(),
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
            notification_system: NotificationSystemConfig::default(),
        }
    }
}
