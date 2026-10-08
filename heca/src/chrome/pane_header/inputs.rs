//! What each pane's header needs to know, gathered from the session once a frame.

use super::*;

/// What one pane's header needs to know, as plain data gathered from the session.
pub(crate) struct PaneHeaderInput {
    pub(super) pane_id: PaneId,
    pub(super) ws_idx: usize,
    pub(super) col_idx: usize,
    pub(super) name: String,
    pub(super) custom_name: Option<String>,
    pub(super) runtime: Option<PaneRuntime>,
    pub(super) zoomed: bool,
    pub(super) floating: bool,
    pub(super) avail_w: f32,
}

#[cfg(test)]
impl PaneHeaderInput {
    /// Facts for pane `pane_id` and nothing else — what a test hands a pane.
    pub(crate) fn for_test(pane_id: PaneId) -> Self {
        Self {
            pane_id,
            ws_idx: 0,
            col_idx: 0,
            name: String::new(),
            custom_name: None,
            runtime: None,
            zoomed: false,
            floating: false,
            avail_w: 100.0,
        }
    }

    /// Which pane these facts are about.
    pub(crate) fn pane(&self) -> PaneId {
        self.pane_id
    }
}

/// **Read what every pane's header depends on**, once a frame. `None` when the info bar is
/// disabled: no pane gets one.
///
/// This builds no widget. Each pane's header (a [`Keyed`](heca_grid_ui::widgets::Keyed) the pane
/// holds) is told the result by [`show_pane_header`], and builds only if what it shows changed.
pub(crate) fn pane_header_inputs(
    state: &mut crate::app_state::AppState,
) -> Option<std::collections::HashMap<PaneId, PaneHeaderInput>> {
    let segments = state
        .pane_chips
        .shown(&state.appearance.pane.title_segments);
    let actions = state
        .pane_buttons
        .shown(&state.appearance.pane.title_actions);
    if segments.is_empty() && actions.is_empty() {
        return None;
    }
    let frames = crate::app::terminal_host::pane_outer_frames(state);
    let active_ws = state.layout().active_workspace_idx();
    let mut inputs = std::collections::HashMap::with_capacity(frames.len());
    for (pane_id, _x, _y, w, _h) in frames {
        let (ws_idx, col_idx) = crate::find_pane_location(&state.session, pane_id)
            .map(|(ws, col, _)| (ws, col))
            .unwrap_or((active_ws, 0));
        let (name, custom_name, runtime) = state.layout()
            .active_workspace()
            .and_then(|ws| ws.content().find_pane(pane_id))
            .map(|p| {
                (
                    p.title.clone(),
                    p.custom_name.clone(),
                    Some(p.runtime.clone()),
                )
            })
            .unwrap_or_else(|| (String::new(), None, None));
        // Floating panes aren't in any column (`find_pane_location` returns None); detect them
        // directly so the bar hides tiled-only buttons + flags float active.
        let floating = state.layout()
            .active_workspace()
            .map(|ws| ws.floating_panes.iter().any(|f| f.pane.id == pane_id))
            .unwrap_or(false);
        let zoomed = !floating
            && state.layout()
                .active_workspace()
                .and_then(|ws| ws.content().scrolling.columns.get(col_idx))
                .map(|c| c.is_zoomed() || c.is_full_width)
                .unwrap_or(false);
        inputs.insert(
            pane_id,
            PaneHeaderInput {
                pane_id,
                ws_idx,
                col_idx,
                name,
                custom_name,
                runtime,
                zoomed,
                floating,
                // The pane's own width. The strip insets its content with its own padding, so
                // nothing out here subtracts a margin from it.
                avail_w: w.max(0.0),
            },
        );
    }
    // An added chip's kept answers are for panes that are still there.
    state
        .pane_chips
        .retain_panes(&inputs.keys().copied().collect());
    Some(inputs)
}
