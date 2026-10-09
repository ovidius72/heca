//! `[settings]` — the user-configurable behaviour and window geometry settings.

use crate::color::Color;
use serde::{Deserialize, Serialize};

mod defaults;
mod enums;
mod notifications;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod default_tests;

pub use enums::{CenterFocusedColumn, ColumnFocus, ModifierKey, PaletteSize, SearchCase};
pub use notifications::{NotificationSystem, NotificationSystemConfig};

/// User-configurable settings that control behaviour and appearance.
///
/// Font family/size configuration has moved out of `[settings]` into the
/// dedicated `[font]` block (see [`crate::font::FontConfig`]). The fields below
/// cover behaviour, window geometry, and terminal color palette overrides only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsConfig {
    #[serde(default = "defaults::theme")]
    pub theme: String,
    #[serde(default = "defaults::mouse")]
    pub mouse: bool,
    /// How roomy the command palette is (`small` / `normal` / `large`).
    #[serde(default = "defaults::command_palette_size", alias = "command-palette-size")]
    pub command_palette_size: PaletteSize,
    /// How a search query's case is treated (`smart` / `sensitive` / `insensitive`).
    #[serde(default = "defaults::search_case", alias = "search-case")]
    pub search_case: SearchCase,
    /// Remember what has been searched for and chosen, across restarts.
    #[serde(default = "defaults::search_history", alias = "search-history")]
    pub search_history: bool,
    /// How many past queries each search surface keeps.
    #[serde(default = "defaults::search_history_size", alias = "search-history-size")]
    pub search_history_size: usize,
    /// How many commands' usage counts each search surface keeps for ranking.
    #[serde(default = "defaults::search_usage_size", alias = "search-usage-size")]
    pub search_usage_size: usize,
    #[serde(default = "defaults::window_width")]
    pub window_width: u32,
    #[serde(default = "defaults::window_height")]
    pub window_height: u32,
    /// Optional terminal default foreground override from `config.toml`.
    #[serde(default = "defaults::terminal_foreground", alias = "terminal-foreground")]
    pub terminal_foreground: Option<Color>,
    /// Optional terminal default background override from `config.toml`.
    #[serde(default = "defaults::terminal_background", alias = "terminal-background")]
    pub terminal_background: Option<Color>,
    /// Optional terminal cursor foreground override from `config.toml`.
    #[serde(default = "defaults::terminal_cursor_foreground", alias = "terminal-cursor-foreground")]
    pub terminal_cursor_foreground: Option<Color>,
    /// Optional terminal cursor background override from `config.toml`.
    #[serde(default = "defaults::terminal_cursor_background", alias = "terminal-cursor-background")]
    pub terminal_cursor_background: Option<Color>,
    /// Optional terminal cursor border override from `config.toml`.
    #[serde(default = "defaults::terminal_cursor_border", alias = "terminal-cursor-border")]
    pub terminal_cursor_border: Option<Color>,
    /// Optional terminal selection foreground override from `config.toml`.
    #[serde(
        default = "defaults::terminal_selection_foreground",
        alias = "terminal-selection-foreground",
    )]
    pub terminal_selection_foreground: Option<Color>,
    /// Optional terminal selection background override from `config.toml`.
    #[serde(
        default = "defaults::terminal_selection_background",
        alias = "terminal-selection-background",
    )]
    pub terminal_selection_background: Option<Color>,
    /// Optional override of the colour of the line a carried pane can be dropped on (default: the
    /// theme's accent).
    #[serde(default = "defaults::drag_edge_color")]
    pub drag_edge_color: Option<Color>,
    /// Optional override of the colour of the line a drop would land on now (default: the theme's
    /// warning colour).
    #[serde(default = "defaults::drag_edge_target_color")]
    pub drag_edge_target_color: Option<Color>,
    /// Optional override of that line's thickness in logical px (default 3, half at rest).
    #[serde(default = "defaults::drag_edge_width")]
    pub drag_edge_width: Option<f32>,
    /// Optional terminal ANSI `0..7` palette override from `config.toml`.
    #[serde(default = "defaults::terminal_ansi", alias = "terminal-ansi")]
    pub terminal_ansi: Option<[Color; 8]>,
    /// Optional terminal bright ANSI `8..15` palette override from `config.toml`.
    #[serde(default = "defaults::terminal_brights", alias = "terminal-brights")]
    pub terminal_brights: Option<[Color; 8]>,
    /// Automatically scroll the workspace view when the pointer hovers near the left/right edge.
    #[serde(default = "defaults::auto_scroll_edge")]
    pub auto_scroll_edge: bool,
    /// How close (logical px) the pointer must come to the content area's edge, while dragging a
    /// pane, for the view to start scrolling. Only used with `auto_scroll_edge`. Default 80.
    #[serde(default = "defaults::edge_scroll_distance")]
    pub edge_scroll_distance: f32,
    /// Which pane focus lands on when it moves to the column on the left or right: the one
    /// last used in that column (`"last"`, the default), or the one level with the pane you leave
    /// (`"row"`).
    #[serde(default = "defaults::column_focus")]
    pub column_focus: ColumnFocus,
    /// Modifier key that must be held to initiate an interactive pane drag with the mouse.
    #[serde(default = "defaults::interactive_move_modifier")]
    pub interactive_move_modifier: ModifierKey,
    /// Modifier key that turns a drag from a **move** into a **swap** — dropping exchanges the two
    /// things instead of placing one at the other's position. Applies to every drag alike: a row in
    /// a sidebar, a pane carried across the content area, a plugin's own row.
    ///
    /// Must differ from [`interactive_move_modifier`](Self::interactive_move_modifier): one key
    /// cannot both start a drag and change what it means, or every drag is a swap and a plain move
    /// becomes unreachable. Heca reports the collision at startup and keeps the gesture.
    #[serde(default = "defaults::swap_modifier")]
    pub swap_modifier: ModifierKey,
    /// Center a single column even when it fits within the viewport.
    #[serde(default = "defaults::always_center_single_column")]
    pub always_center_single_column: bool,
    /// When focusing a column re-centres the view: `never` | `on_overflow` | `always`.
    #[serde(default = "defaults::center_focused_column")]
    pub center_focused_column: CenterFocusedColumn,
    /// Gap between workspace rows in the exposé, as a fraction of a screen height.
    #[serde(default = "defaults::overview_gap")]
    pub overview_gap: f64,
    /// The scale the exposé **opens from**, relative to its map size — the zoom it animates out of
    /// and back into. `1.0` disables the animation. Clamped to `0.2..=4.0`.
    #[serde(default = "defaults::overview_zoom_from")]
    pub overview_zoom_from: f64,
    /// How much of the working area a pane takes when it floats with nowhere given — a fraction of
    /// each side, centred. Clamped to `0.1..=1.0`.
    #[serde(default = "defaults::float_size")]
    pub float_size: f64,
    /// How far a pane that moved or swapped animates in from, as a fraction of the window.
    /// `0` makes moves jump. Clamped to `0.0..=1.0`.
    #[serde(default = "defaults::move_slide_reach")]
    pub move_slide_reach: f64,
    /// How far into the pane next to it the top and bottom border of a column take a dropped
    /// pane, as a fraction of that pane's height. Clamped to `0.0..=0.5`.
    #[serde(default = "defaults::drop_edge_reach")]
    pub drop_edge_reach: f64,
    /// Auto-inject shell integration snippets for OSC 133/OSC 7 pane runtime signals.
    #[serde(default = "defaults::shell_integration")]
    pub shell_integration: bool,
    /// When a pane has a **custom name** (set via rename), also show its process/program name as
    /// a small dimmed label next to it (e.g. `MyPane (nvim)`). The process label is *not* part of
    /// the name — it is appended for reference only and never edited by rename. Set `false` to
    /// show just the custom name.
    #[serde(
        default = "defaults::pane_renamed_add_process_name",
        alias = "pane-renamed-add-process-name"
    )]
    pub pane_renamed_add_process_name: bool,
    /// **Deprecated: `[appearance.expose] show_cwd`.** Still read and mapped onto it, with a one-time
    /// notice to move it. It once switched the sidebar's folder line too; that line is now decided by
    /// `[appearance.sidebar] pane_lines` alone.
    #[serde(default = "defaults::pane_show_cwd", alias = "pane-show-cwd")]
    pub pane_show_cwd: bool,
    /// Host terminal scrollback capacity in rows.
    ///
    /// This is the number of history rows the terminal engine retains above the
    /// visible viewport. The host scrollback viewport (see `terminal-01a`) scrolls
    /// within `[0, terminal_scrollback_lines]`. Mirrors wezterm-term's default of
    /// 3500 when unset.
    #[serde(
        default = "defaults::terminal_scrollback_lines",
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
    #[serde(default = "defaults::terminal_mouse", alias = "terminal-mouse")]
    pub terminal_mouse: bool,

    /// Number of scrollback rows per wheel notch when the host scrollback viewport
    /// is active (i.e. the wheel scrolls the host viewport, not the terminal).
    #[serde(
        default = "defaults::terminal_wheel_scroll_lines",
        alias = "terminal-wheel-scroll-lines"
    )]
    pub terminal_wheel_scroll_lines: usize,

    /// Points added/removed per terminal font-zoom step (keyboard `prefix+Ctrl/Alt`
    /// bindings and `Ctrl`/`Meta`+wheel). Applies to both the app-wide and the
    /// per-pane zoom. Non-positive values fall back to the default step.
    #[serde(
        default = "defaults::terminal_font_zoom_step",
        alias = "terminal-font-zoom-step"
    )]
    pub terminal_font_zoom_step: f32,

    /// Whether `Ctrl`/`Meta`+mouse-wheel changes the font size (app-wide over
    /// chrome, focused pane over a pane). When false the modified wheel is left
    /// alone (forwarded like a normal wheel), and font zoom stays keyboard-only.
    #[serde(
        default = "defaults::mouse_wheel_change_font_size",
        alias = "mouse-wheel-change-font-size"
    )]
    pub mouse_wheel_change_font_size: bool,

    /// Enable smooth animation for backend-side discrete terminal viewport jumps.
    /// When false, animated scroll APIs degrade to immediate scroll.
    #[serde(
        default = "defaults::terminal_scroll_animations",
        alias = "terminal-scroll-animations"
    )]
    pub terminal_scroll_animations: bool,

    /// Show the left sidebar region on startup. `false` starts it hidden; the runtime toggle
    /// (`prefix+b`) shows it, and a config reload puts it back to this value.
    #[serde(default = "defaults::show_left_sidebar", alias = "show-left-sidebar")]
    pub show_left_sidebar: bool,
    /// Show the right sidebar region on startup. `false` starts it hidden.
    #[serde(default = "defaults::show_right_sidebar", alias = "show-right-sidebar")]
    pub show_right_sidebar: bool,
    /// Show the top bar (tab bar). `false` fully hides it (zero height) and the
    /// pane area reclaims the space.
    #[serde(default = "defaults::show_top_bar", alias = "show-top-bar")]
    pub show_top_bar: bool,
    /// Show the bottom bar (status bar). `false` fully hides it (zero height).
    #[serde(default = "defaults::show_bottom_bar", alias = "show-bottom-bar")]
    pub show_bottom_bar: bool,

    /// `[settings.notification_system]` — in-app toast notification behaviour.
    #[serde(default = "defaults::notification_system", alias = "notification-system")]
    pub notification_system: NotificationSystemConfig,
    // Destructive-action confirmation moved to the generic `[confirm]` table
    // (`ConfirmConfig`, keyed by action name: `close` / `delete_column` / `delete_workspace`).
}

impl Default for SettingsConfig {
    fn default() -> Self {
        defaults::required(&[])
    }
}
