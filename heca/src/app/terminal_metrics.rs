//! Terminal font metric helpers.
//!
//! The terminal host and backend creation should size the PTY grid from the
//! actual loaded terminal font rather than config heuristics alone.
//!
//! Font zoom layers two offsets on top of the configured `font.size.terminal`: a **global** one
//! (app-wide) and **each terminal's own** ([`TerminalZoom`](crate::app::terminal_zoom::TerminalZoom)),
//! whoever owns the terminal. Both the PTY cell fit and the rendered glyph size derive from that
//! single value so they never disagree.

use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;
use heca_config::font::FontConfig;

/// Smallest allowed effective terminal font size (points). Guards against
/// zooming a pane down to an unusable or zero-sized grid.
pub(crate) const TERMINAL_FONT_SIZE_MIN: f32 = 6.0;
/// Largest allowed effective terminal font size (points).
pub(crate) const TERMINAL_FONT_SIZE_MAX: f32 = 72.0;
/// Smallest allowed effective chrome/UI font size (points) under app zoom.
pub(crate) const APP_UI_FONT_SIZE_MIN: f32 = 8.0;
/// Largest allowed effective chrome/UI font size (points) under app zoom.
pub(crate) const APP_UI_FONT_SIZE_MAX: f32 = 48.0;
/// Points added/removed per font-zoom step (keyboard notch or wheel notch).
pub(crate) const TERMINAL_FONT_ZOOM_STEP: f32 = 1.0;

/// Measure the real monospace cell size for the configured terminal family/size,
/// falling back to the [`FontConfig::terminal_cell_size`] heuristic when the
/// font can't be measured.
pub(crate) fn resolve_terminal_cell_size(
    text_renderer: &mut heca_renderer::text::TextRenderer,
    font_config: &FontConfig,
) -> (f32, f32) {
    resolve_terminal_cell_size_for(text_renderer, font_config, font_config.size.terminal)
}

/// Measure the monospace cell size for an explicit font size (used by per-pane
/// and global font zoom, where the effective size differs from the configured
/// `size.terminal`). Falls back to the ratio heuristic when measurement fails.
pub(crate) fn resolve_terminal_cell_size_for(
    text_renderer: &mut heca_renderer::text::TextRenderer,
    font_config: &FontConfig,
    font_size: f32,
) -> (f32, f32) {
    text_renderer
        .measure_monospace_cell(font_size, font_config.family.terminal_normal())
        .unwrap_or_else(|| font_config.terminal_cell_size_for(font_size))
}

/// Re-measure terminal cell sizes and push them into every backend so each PTY
/// grid tracks the real loaded font. Honors font zoom: the global cell size uses
/// the effective global font size, and each zoomed terminal is re-measured at its
/// own. Called after a config reload/font change and after any zoom change.
pub(crate) fn refresh_terminal_cell_size(state: &mut AppState) {
    let global_size = state.app_font_size();
    state.terminal_cell_size =
        resolve_terminal_cell_size_for(&mut state.text_renderer, &state.font_config, global_size);

    for terminal in state.terminal_zoom.zoomed() {
        measure_zoomed_cell(state, terminal);
    }

    // The grid and cell size each process has are not set here. A terminal asks for them when its
    // box or its font changes (`TerminalInput::Resize`), and the font it reads is the one just
    // measured above — so there is one place a size is decided.
    let scale = state.scale_factor as f32;
    for backend in state.server.backends.values_mut() {
        backend.set_scale_factor(scale);
    }
}

/// Measure the cell `terminal`'s own font size makes. A terminal following the app's size has
/// none of its own and is left alone.
fn measure_zoomed_cell(state: &mut AppState, terminal: TerminalId) {
    if state.terminal_zoom.offset(terminal) == 0.0 {
        return;
    }
    let size = state.terminal_font_size(terminal);
    let cell = resolve_terminal_cell_size_for(&mut state.text_renderer, &state.font_config, size);
    state.terminal_zoom.set_cell(terminal, cell);
}

/// Apply a global terminal font-zoom step (points). `delta == 0.0` resets the
/// global offset to zero. Re-measures all cell sizes (global + every zoomed pane)
/// and requests a redraw.
pub(crate) fn apply_app_font_zoom(state: &mut AppState, delta: f32) {
    let next = if delta == 0.0 {
        0.0
    } else {
        // Clamp the *effective* global size, then store the offset that produced
        // it so repeated steps at the limit don't accumulate silently.
        let target = (state.font_config.size.terminal + state.app_font_zoom + delta)
            .clamp(TERMINAL_FONT_SIZE_MIN, TERMINAL_FONT_SIZE_MAX);
        target - state.font_config.size.terminal
    };
    if next == state.app_font_zoom {
        return;
    }
    state.app_font_zoom = next;
    refresh_terminal_cell_size(state);
    state.mark_full_redraw();
}

/// **Step `terminal`'s own font by `delta` points** (`0.0` resets it to the global size),
/// whoever owns it. Re-measures its cell size, pushes it into its process, and requests a redraw.
pub(crate) fn apply_terminal_font_zoom(state: &mut AppState, terminal: TerminalId, delta: f32) {
    let base = state.terminal_zoom_base();
    if !state.terminal_zoom.step(terminal, base, delta) {
        return;
    }
    measure_zoomed_cell(state, terminal);

    // Push the resolved cell size straight into the terminal's process so it reflows at once
    // rather than waiting for the grid the terminal asks for on its next layout.
    let cell = state.terminal_base_cell_size(terminal);
    if let Some(backend) = state.server.backends.get_mut_by_id(terminal) {
        backend.set_cell_size(cell.0, cell.1);
    }
    state.mark_full_redraw();
}
