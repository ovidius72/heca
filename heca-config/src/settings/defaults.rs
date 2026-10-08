//! The default each setting has when the file does not say — the functions serde calls.

use super::enums::ModifierKey;

pub(super) fn default_search_history() -> bool {
    true
}

pub(super) fn default_search_history_size() -> usize {
    50
}

pub(super) fn default_search_usage_size() -> usize {
    500
}

pub(super) fn default_theme() -> String {
    "grid_tron".to_string()
}

pub(super) fn default_mouse() -> bool {
    true
}

pub(super) fn default_window_width() -> u32 {
    1280
}

pub(super) fn default_window_height() -> u32 {
    800
}

pub(super) fn default_auto_scroll_edge() -> bool {
    true
}

/// Default distance from the content area's edge (logical px) at which a pane drag starts
/// scrolling the view.
pub(super) fn default_edge_scroll_distance() -> f32 {
    80.0
}

/// Shift is the near-universal "and swap them" modifier; it is a default, not a rule.
pub(super) fn default_swap_modifier() -> ModifierKey {
    ModifierKey::Shift
}

/// Default gap between overview rows: a tenth of a screen height, niri's.
pub(super) fn default_overview_gap() -> f64 {
    0.1
}

/// Default scale the exposé **opens from**, relative to its own map size: `0.8`.
///
/// Below 1.0 the map **grows in** from smaller and shrinks away when it closes — a zoom in on the
/// way in, out on the way out. Above 1.0 it does the opposite: it starts larger and pulls back, the
/// way niri's overview does (the panes you were looking at shrinking into cards). `1.0` is no
/// animation at all.
///
/// Both readings are defensible and this is the one chosen: pulling back read as falling in from somewhere, even softened to 1.3, while growing in reads as
/// the map opening. Starting the cards at exactly life size — niri's literal reading — overshoots
/// badly: the outer rows begin off-screen.
pub(super) fn default_overview_zoom_from() -> f64 {
    0.8
}

/// Default share of the working area a pane takes when it floats with nowhere given: `0.95`,
/// which leaves a margin that shows it is floating, not tiled.
pub(super) fn default_float_size() -> f64 {
    0.95
}

/// Default reach of a move's slide animation: nine tenths of the window.
pub(super) fn default_move_slide_reach() -> f64 {
    0.9
}

pub(super) fn default_always_center_single_column() -> bool {
    false
}

pub(super) fn default_shell_integration() -> bool {
    true
}

/// Default for appending the process/program name next to a renamed pane's custom name
/// (e.g. `MyPane (nvim)`). On by default so the running program stays visible after a rename.
pub(super) fn default_pane_renamed_add_process_name() -> bool {
    true
}

/// Default for the deprecated `pane_show_cwd`: off. See `[appearance.expose] show_cwd`.
pub(super) fn default_pane_show_cwd() -> bool {
    false
}

/// Default host terminal scrollback capacity in rows.
///
/// Mirrors `wezterm-term`'s `TerminalConfiguration::scrollback_size()` default
/// so behaviour is unchanged when the user does not set
/// `terminal_scrollback_lines` in `config.toml`.
pub(super) fn default_terminal_scrollback_lines() -> usize {
    3500
}

pub(super) fn default_terminal_mouse() -> bool {
    true
}

pub(super) fn default_terminal_wheel_scroll_lines() -> usize {
    3
}
/// Default points added/removed per terminal font-zoom step (keyboard notch or
/// `Ctrl`/`Meta`+wheel notch).
pub(super) fn default_terminal_font_zoom_step() -> f32 {
    1.0
}
/// Default for whether `Ctrl`/`Meta`+wheel changes the font size. On by default;
/// set false to reserve the modified wheel for the terminal/app instead.
pub(super) fn default_mouse_wheel_change_font_size() -> bool {
    true
}
pub(super) fn default_terminal_scroll_animations() -> bool {
    true
}

/// Chrome regions are shown by default; a `show_*` toggle set to `false` fully
/// starts that region hidden (a sidebar takes no width; the tab / status bar collapses to zero
/// height and the pane area reclaims the space). It is only the starting state: the show/hide
/// toggles flip the same state, and a config reload sets it back.
pub(super) fn default_show_chrome_region() -> bool {
    true
}
