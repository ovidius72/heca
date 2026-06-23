//! Terminal font metric helpers.
//!
//! The terminal host and backend creation should size the PTY grid from the
//! actual loaded terminal font rather than config heuristics alone.

use crate::app_state::AppState;
use heca_config::font::FontConfig;

/// Measure the real monospace cell size for the configured terminal family/size,
/// falling back to the [`FontConfig::terminal_cell_size`] heuristic when the
/// font can't be measured.
pub(crate) fn resolve_terminal_cell_size(
    text_renderer: &mut heca_renderer::text::TextRenderer,
    font_config: &FontConfig,
) -> (f32, f32) {
    text_renderer
        .measure_monospace_cell(
            font_config.size.terminal,
            font_config.family.terminal_normal(),
        )
        .unwrap_or_else(|| font_config.terminal_cell_size())
}

/// Re-measure the terminal cell size and push it into every backend so the PTY
/// grid tracks the real loaded font after a config reload or font change.
pub(crate) fn refresh_terminal_cell_size(state: &mut AppState) {
    state.terminal_cell_size =
        resolve_terminal_cell_size(&mut state.text_renderer, &state.font_config);
    let (cell_w, cell_h) = state.terminal_cell_size;
    for backend in state.backends.values_mut() {
        backend.set_cell_size(cell_w, cell_h);
    }
}
