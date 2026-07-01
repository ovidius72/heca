//! Terminal font metric helpers.
//!
//! The terminal host and backend creation should size the PTY grid from the
//! actual loaded terminal font rather than config heuristics alone.
//!
//! Font zoom (`terminal-10`) layers two offsets on top of the configured
//! `font.size.terminal`: a **global** offset (app-wide, the `app-03` base) and a
//! **per-pane** offset. The effective size for a pane is
//! `(config + global + pane).clamp(MIN, MAX)`, and both the PTY cell fit and the
//! rendered glyph size derive from that single value so they never disagree.

use crate::app_state::AppState;
use heca_config::font::FontConfig;
use heca_core::layout::PaneId;

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
/// the effective global font size, and each zoomed pane gets its own re-measured
/// override. Called after a config reload/font change and after any zoom change.
pub(crate) fn refresh_terminal_cell_size(state: &mut AppState) {
    let global_size = state.app_font_size();
    state.terminal_cell_size =
        resolve_terminal_cell_size_for(&mut state.text_renderer, &state.font_config, global_size);

    // Recompute per-pane overrides for zoomed panes; drop entries that have
    // decayed back to zero offset so they follow the global size again.
    let zoomed: Vec<PaneId> = state.pane_font_zoom.keys().copied().collect();
    for pane_id in zoomed {
        refresh_pane_cell_override(state, pane_id);
    }

    let global_cell = state.terminal_cell_size;
    let scale = state.scale_factor as f32;
    let overrides = &state.pane_cell_override;
    for (pane_id, backend) in state.backends.iter_mut() {
        backend.set_scale_factor(scale);
        let (cell_w, cell_h) = overrides.get(&pane_id).copied().unwrap_or(global_cell);
        backend.set_cell_size(cell_w, cell_h);
    }
}

/// Recompute (or clear) a single pane's cell-size override from its current
/// effective font size. Removes the override — and the zoom entry — when the
/// pane's offset is zero so it falls back to the shared global cell size.
fn refresh_pane_cell_override(state: &mut AppState, pane_id: PaneId) {
    let offset = state.pane_font_zoom.get(&pane_id).copied().unwrap_or(0.0);
    if offset == 0.0 {
        state.pane_font_zoom.remove(&pane_id);
        state.pane_cell_override.remove(&pane_id);
        return;
    }
    let size = state.effective_terminal_font_size(pane_id);
    let cell = resolve_terminal_cell_size_for(&mut state.text_renderer, &state.font_config, size);
    state.pane_cell_override.insert(pane_id, cell);
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

/// Apply a per-pane terminal font-zoom step (points) to `pane_id`. `delta == 0.0`
/// resets that pane back to the global size. Re-measures the pane's cell size,
/// pushes it into the backend, and requests a redraw.
pub(crate) fn apply_pane_terminal_font_zoom(state: &mut AppState, pane_id: PaneId, delta: f32) {
    if delta == 0.0 {
        if state.pane_font_zoom.remove(&pane_id).is_none() {
            return;
        }
        state.pane_cell_override.remove(&pane_id);
    } else {
        let current = state.pane_font_zoom.get(&pane_id).copied().unwrap_or(0.0);
        // Clamp on the effective size so a pane can't be stepped past the limits.
        let base = state.font_config.size.terminal + state.app_font_zoom;
        let target = (base + current + delta).clamp(TERMINAL_FONT_SIZE_MIN, TERMINAL_FONT_SIZE_MAX);
        let next = target - base;
        if next == current {
            return;
        }
        state.pane_font_zoom.insert(pane_id, next);
        refresh_pane_cell_override(state, pane_id);
    }

    // Push the resolved cell size straight into the pane's backend so the PTY
    // reflows immediately rather than waiting for the next per-frame fit.
    let cell = state.pane_base_cell_size(pane_id);
    if let Some(backend) = state.backends.get_mut(pane_id) {
        backend.set_cell_size(cell.0, cell.1);
    }
    state.mark_full_redraw();
}
