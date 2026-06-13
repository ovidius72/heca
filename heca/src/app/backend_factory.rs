//! Backend creation helpers.
//!
//! The app prefers real PTY-backed terminals and falls back to the fake backend
//! only when terminal startup fails on the current machine.

use heca_config::theme::Theme;
use heca_core::backend::{FakeBackend, PaneBackend, TerminalBackend, TerminalPaletteDefaults};
use std::sync::Arc;
use winit::event_loop::EventLoopProxy;

use crate::app::events::AppEvent;
use crate::app_state::AppState;

pub(crate) const FALLBACK_TERMINAL_GRID: (usize, usize) = (80, 24);
const MAX_TERMINAL_UNITS: f64 = 16_384.0;

pub(crate) fn estimate_terminal_grid(
    width: f64,
    height: f64,
    cell_size: (f32, f32),
) -> (usize, usize) {
    let cols = estimate_terminal_units(width, cell_size.0 as f64, FALLBACK_TERMINAL_GRID.0);
    let rows = estimate_terminal_units(height, cell_size.1 as f64, FALLBACK_TERMINAL_GRID.1);
    (cols, rows)
}

fn estimate_terminal_units(extent: f64, approx_cell: f64, fallback: usize) -> usize {
    if !extent.is_finite() || !approx_cell.is_finite() || extent <= 0.0 || approx_cell <= 0.0 {
        return fallback;
    }

    (extent / approx_cell).ceil().clamp(1.0, MAX_TERMINAL_UNITS) as usize
}

pub(crate) fn terminal_grid_for_workspace(state: &AppState, ws_idx: usize) -> (usize, usize) {
    state
        .session
        .workspaces
        .get(ws_idx)
        .map(|ws| {
            estimate_terminal_grid(
                ws.scrolling.working_area.size.w,
                ws.scrolling.working_area.size.h,
                state.terminal_cell_size,
            )
        })
        .unwrap_or(FALLBACK_TERMINAL_GRID)
}

pub(crate) fn create_terminal_backend(
    cols: usize,
    rows: usize,
    theme: &Theme,
    cell_size: (f32, f32),
    event_proxy: Option<&EventLoopProxy<AppEvent>>,
) -> Box<dyn PaneBackend> {
    let (cell_w, cell_h) = cell_size;
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
