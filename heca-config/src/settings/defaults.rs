//! Settings defaults read from the embedded configuration.

use super::{
    CenterFocusedColumn, ColumnFocus, ModifierKey, NotificationSystemConfig, PaletteSize, SearchCase,
};
use crate::color::Color;
use serde::de::DeserializeOwned;
use std::sync::OnceLock;

fn value(path: &[&str]) -> Option<&'static toml::Value> {
    static SETTINGS: OnceLock<toml::Value> = OnceLock::new();
    let settings = SETTINGS.get_or_init(|| {
        let config: toml::Value = toml::from_str(crate::loader::CONFIG_DEFAULT)
            .expect("embedded config.default.toml must be valid TOML");
        config.get("settings").cloned()
            .expect("embedded config.default.toml must contain [settings]")
    });
    path.iter().try_fold(settings, |value, key| value.get(*key))
}

fn decode<T: DeserializeOwned>(value: &toml::Value, path: &[&str]) -> T {
    value.clone().try_into().unwrap_or_else(|error| {
        panic!("invalid embedded settings.{}: {error}", path.join("."))
    })
}

/// Read a required default without constructing SettingsConfig recursively.
pub(super) fn required<T: DeserializeOwned>(path: &[&str]) -> T {
    let value = value(path).unwrap_or_else(|| {
        panic!("embedded config.default.toml is missing settings.{}", path.join("."))
    });
    decode(value, path)
}

fn optional<T: DeserializeOwned>(path: &[&str]) -> Option<T> {
    value(path).map(|value| decode(value, path))
}

macro_rules! required_defaults {
    ($($field:ident: $ty:ty),* $(,)?) => {
        $(pub(super) fn $field() -> $ty {
            required(&[stringify!($field)])
        })*
    };
}

macro_rules! optional_defaults {
    ($($field:ident: $ty:ty),* $(,)?) => {
        $(pub(super) fn $field() -> Option<$ty> {
            optional(&[stringify!($field)])
        })*
    };
}

required_defaults! {
    theme: String,
    mouse: bool,
    command_palette_size: PaletteSize,
    search_case: SearchCase,
    search_history: bool,
    search_history_size: usize,
    search_usage_size: usize,
    window_width: u32,
    window_height: u32,
    auto_scroll_edge: bool,
    edge_scroll_distance: f32,
    column_focus: ColumnFocus,
    interactive_move_modifier: ModifierKey,
    swap_modifier: ModifierKey,
    always_center_single_column: bool,
    center_focused_column: CenterFocusedColumn,
    overview_gap: f64,
    overview_zoom_from: f64,
    float_size: f64,
    move_slide_reach: f64,
    drop_edge_reach: f64,
    shell_integration: bool,
    pane_renamed_add_process_name: bool,
    pane_show_cwd: bool,
    terminal_scrollback_lines: usize,
    terminal_mouse: bool,
    terminal_wheel_scroll_lines: usize,
    terminal_font_zoom_step: f32,
    mouse_wheel_change_font_size: bool,
    terminal_scroll_animations: bool,
    show_left_sidebar: bool,
    show_right_sidebar: bool,
    show_top_bar: bool,
    show_bottom_bar: bool,
    notification_system: NotificationSystemConfig,
}

optional_defaults! {
    terminal_foreground: Color,
    terminal_background: Color,
    terminal_cursor_foreground: Color,
    terminal_cursor_background: Color,
    terminal_cursor_border: Color,
    terminal_selection_foreground: Color,
    terminal_selection_background: Color,
    drag_edge_color: Color,
    drag_edge_target_color: Color,
    drag_edge_width: f32,
    terminal_ansi: [Color; 8],
    terminal_brights: [Color; 8],
}
