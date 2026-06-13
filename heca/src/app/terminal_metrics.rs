//! Terminal font metric helpers.
//!
//! The terminal host and backend creation should size the PTY grid from the
//! actual loaded terminal font rather than theme heuristics alone.

use crate::app_state::AppState;
use heca_config::theme::Theme;

pub(crate) fn resolve_terminal_cell_size(
    text_renderer: &mut heca_renderer::text::TextRenderer,
    theme: &Theme,
) -> (f32, f32) {
    text_renderer
        .measure_monospace_cell(theme.terminal_font_size, &theme.terminal_font_family)
        .unwrap_or_else(|| theme.terminal_cell_size())
}

pub(crate) fn refresh_terminal_cell_size(state: &mut AppState) {
    state.terminal_cell_size = resolve_terminal_cell_size(&mut state.text_renderer, &state.theme);
    let (cell_w, cell_h) = state.terminal_cell_size;
    for backend in state.backends.values_mut() {
        backend.set_cell_size(cell_w, cell_h);
    }
}
