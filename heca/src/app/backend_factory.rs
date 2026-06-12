//! Backend creation helpers.
//!
//! The app prefers real PTY-backed terminals and falls back to the fake backend
//! only when terminal startup fails on the current machine.

use heca_config::theme::Theme;
use heca_core::backend::{FakeBackend, PaneBackend, TerminalBackend, TerminalPaletteDefaults};
use std::sync::Arc;
use winit::event_loop::EventLoopProxy;

use crate::app::events::AppEvent;

pub(crate) fn create_terminal_backend(
    cols: usize,
    rows: usize,
    theme: &Theme,
    event_proxy: Option<&EventLoopProxy<AppEvent>>,
) -> Box<dyn PaneBackend> {
    let (cell_w, cell_h) = theme.terminal_cell_size();
    let wake_on_output = event_proxy.map(|proxy| {
        let proxy = proxy.clone();
        Arc::new(move || {
            let _ = proxy.send_event(AppEvent::BackendWake);
        }) as Arc<dyn Fn() + Send + Sync>
    });
    let palette_defaults = TerminalPaletteDefaults {
        foreground: theme
            .terminal_foreground
            .map(|color| [color.r, color.g, color.b, color.a]),
        background: theme
            .terminal_background
            .map(|color| [color.r, color.g, color.b, color.a]),
        cursor_fg: theme
            .terminal_cursor_foreground
            .map(|color| [color.r, color.g, color.b, color.a]),
        cursor_bg: theme
            .terminal_cursor_background
            .map(|color| [color.r, color.g, color.b, color.a]),
        cursor_border: theme
            .terminal_cursor_border
            .map(|color| [color.r, color.g, color.b, color.a]),
        selection_fg: theme
            .terminal_selection_foreground
            .map(|color| [color.r, color.g, color.b, color.a]),
        selection_bg: theme
            .terminal_selection_background
            .map(|color| [color.r, color.g, color.b, color.a]),
        ansi: theme.terminal_ansi.map(|colors| colors.map(|color| [color.r, color.g, color.b, color.a])),
        brights: theme
            .terminal_brights
            .map(|colors| colors.map(|color| [color.r, color.g, color.b, color.a])),
    };
    match TerminalBackend::with_cell_size_and_defaults_and_waker(
        cols,
        rows,
        cell_w,
        cell_h,
        if palette_defaults.foreground.is_some()
            || palette_defaults.background.is_some()
            || palette_defaults.cursor_fg.is_some()
            || palette_defaults.cursor_bg.is_some()
            || palette_defaults.cursor_border.is_some()
            || palette_defaults.selection_fg.is_some()
            || palette_defaults.selection_bg.is_some()
            || palette_defaults.ansi.is_some()
            || palette_defaults.brights.is_some()
        {
            Some(palette_defaults)
        } else {
            None
        },
        wake_on_output,
    ) {
        Ok(backend) => Box::new(backend),
        Err(err) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] warning: failed to create TerminalBackend ({err}); falling back to FakeBackend"
            );
            Box::new(FakeBackend::with_cell_size(cols, rows, cell_w, cell_h))
        }
    }
}
