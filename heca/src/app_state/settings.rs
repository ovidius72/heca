//! **The settings a window works by, read from the configuration in one place** — at start and
//! again on every reload, by the same mapping, so no setting can be read at start and forgotten on
//! reload: a new field here is set by both, or nothing compiles.

use heca_config::confirm::ConfirmConfig;
use heca_config::settings::{PaletteSize, SearchCase};
use heca_config::theme::{AppConfig, ModifierKey};

/// The `[settings]` (and `[confirm]`) values a window reads while it runs.
#[derive(Clone, Debug)]
pub struct WindowSettings {
    /// Whether mouse interactions are enabled.
    pub mouse_enabled: bool,
    /// Whether a dragged pane scrolls the view when it nears the content edge.
    pub auto_scroll_edge: bool,
    /// How close (px) a dragged pane must come to the content edge for the view to scroll.
    pub edge_scroll_distance: f32,
    /// How fast (px per second) the view scrolls while a dragged pane is at the edge.
    pub edge_scroll_speed: f32,
    /// How roomy the command palette is.
    pub command_palette_size: PaletteSize,
    /// How a search query's case is treated.
    pub search_case: SearchCase,
    /// Whether the search memory is read from and written to disk at all. Turned off, the store is
    /// neither read nor written — a shared or recorded machine is a real reason, and a user who
    /// turns it off expects the existing file to stop being consulted too.
    pub search_history: bool,
    /// Append the program name (small, dimmed) next to a renamed pane's custom name.
    pub pane_renamed_add_process_name: bool,
    /// Show each pane's working directory as its own row in the sidebar pane card.
    pub expose_show_cwd: bool,
    /// Whether the wheel scrolls a terminal's history (unless the program in it took the mouse).
    /// Does not affect `mouse_enabled`, which is the chrome's.
    pub terminal_mouse_enabled: bool,
    /// History rows per wheel notch.
    pub terminal_wheel_scroll_lines: usize,
    /// Points added or removed per terminal font-size step.
    pub terminal_font_zoom_step: f32,
    /// Whether `Ctrl`/`Meta`+wheel changes the font size. When off, the modified wheel scrolls.
    pub mouse_wheel_change_font_size: bool,
    /// The modifier held to drag a pane.
    pub interactive_move_modifier: ModifierKey,
    /// Which actions ask before they run, by confirm name; an action not listed falls back to its
    /// own default ([`ConfirmConfig::enabled`]).
    pub confirm: ConfirmConfig,
}

impl WindowSettings {
    /// What the configuration says.
    pub fn from_config(app_config: &AppConfig) -> Self {
        let settings = &app_config.config.settings;
        Self {
            mouse_enabled: settings.mouse,
            auto_scroll_edge: settings.auto_scroll_edge,
            edge_scroll_distance: settings.edge_scroll_distance,
            edge_scroll_speed: settings.edge_scroll_speed,
            command_palette_size: settings.command_palette_size,
            search_case: settings.search_case,
            search_history: settings.search_history,
            pane_renamed_add_process_name: settings.pane_renamed_add_process_name,
            expose_show_cwd: crate::chrome::expose_show_cwd(&app_config.config),
            terminal_mouse_enabled: settings.terminal_mouse,
            terminal_wheel_scroll_lines: settings.terminal_wheel_scroll_lines,
            terminal_font_zoom_step: settings.terminal_font_zoom_step,
            mouse_wheel_change_font_size: settings.mouse_wheel_change_font_size,
            interactive_move_modifier: settings.interactive_move_modifier,
            confirm: app_config.config.confirm.clone(),
        }
    }
}
