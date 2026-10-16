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
    /// Show each pane's folder on its card in the exposé (`[appearance.expose] show_cwd`).
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
            expose_show_cwd: expose_show_cwd(&app_config.config),
            terminal_mouse_enabled: settings.terminal_mouse,
            terminal_wheel_scroll_lines: settings.terminal_wheel_scroll_lines,
            terminal_font_zoom_step: settings.terminal_font_zoom_step,
            mouse_wheel_change_font_size: settings.mouse_wheel_change_font_size,
            interactive_move_modifier: settings.interactive_move_modifier,
            confirm: app_config.config.confirm.clone(),
        }
    }
}

/// **Whether the exposé shows each pane's folder** — `[appearance.expose] show_cwd`.
///
/// The old `[settings] pane_show_cwd` is still read and mapped onto it, so no config breaks, and a
/// user who still has it is told **once** to move it (one input, one output). It once switched the
/// sidebar's folder line too; that line is now decided by `[appearance.sidebar] pane_lines` alone.
fn expose_show_cwd(config: &heca_config::loader::Config) -> bool {
    if config.settings.pane_show_cwd.is_some()
        && crate::announced::first_time("setting:pane_show_cwd")
    {
        crate::chrome::warn_author(
            "[heca] `[settings] pane_show_cwd` is now `[appearance.expose] show_cwd` (it only \
             shows folders in the exposé; the sidebar's folder line is `[appearance.sidebar] \
             pane_lines`) — it still works, please move it"
                .to_string(),
        );
    }
    show_cwd_setting(config)
}

/// The setting's value: the new key when it is written, else the old one, else on. Pure — the
/// notice is separate.
fn show_cwd_setting(config: &heca_config::loader::Config) -> bool {
    config
        .appearance
        .expose
        .show_cwd
        .or(config.settings.pane_show_cwd)
        .unwrap_or(true)
}

#[cfg(test)]
mod show_cwd_tests {
    use super::show_cwd_setting;

    /// **One input, one output**: `[appearance.expose] show_cwd` decides whenever it is written;
    /// the old `[settings] pane_show_cwd` is honoured only while the new key is absent, so no
    /// config breaks; neither written means on.
    #[test]
    fn the_new_key_decides_and_the_old_one_still_works() {
        let mut c = heca_config::loader::Config::default();
        assert!(show_cwd_setting(&c), "on by default");
        c.settings.pane_show_cwd = Some(false);
        assert!(
            !show_cwd_setting(&c),
            "the old key is read while the new one is absent"
        );
        c.appearance.expose.show_cwd = Some(true);
        assert!(
            show_cwd_setting(&c),
            "a written new key wins over the old one"
        );
        c.appearance.expose.show_cwd = Some(false);
        c.settings.pane_show_cwd = Some(true);
        assert!(!show_cwd_setting(&c));
    }
}
