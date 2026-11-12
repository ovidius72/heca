//! **The in-pane header** — its info bar, its action buttons, and the retained trees behind them.
//!
//! It owns what a pane says about itself: the composed name and program icon, the chips (cwd, git,
//! and any a program built on heca adds), the action buttons and their shortcuts, and the per-pane retained widgets the host
//! keeps in step with the session. It owns **nothing** about the chrome around it — the regions,
//! the layer stack and the letters all live beside it, not here.
//!
//! Split out of a 5236-line `chrome/mod.rs` that held every kind of logic at once.

use crate::chrome::*;
use heca_grid_ui::builders::StyleExt as _;
use heca_grid_ui::widgets::{Button, ButtonGroup, ButtonVariant};

mod env;
pub(crate) use env::{HeaderEnv, PaneHeader, give_header_facts};

mod build;
mod info;
mod inputs;
mod segments;
mod shortcuts;
#[cfg(test)]
mod tests;

pub(crate) use build::*;
pub(crate) use info::*;
pub(crate) use inputs::*;
pub(crate) use segments::*;
pub(crate) use shortcuts::*;

impl HeaderEnv {
    /// Take what a header is built from, as it is now.
    pub(crate) fn of(state: &crate::app_state::AppState) -> std::rc::Rc<Self> {
        std::rc::Rc::new(Self {
            theme: crate::chrome::chrome_gui_theme(state),
            band: crate::chrome::window::theme::top_bottom_pane_background_color(&state.theme),
            segment_names: state.appearance.pane.title_segments.clone(),
            action_names: state.appearance.pane.title_actions.clone(),
            chips: state.pane_chips.clone(),
            buttons: state.pane_buttons.clone(),
            programs: state.client.programs.clone(),
            shortcuts: state.action_shortcuts.clone(),
            catalog: state.action_catalog.clone(),
            event_proxy: state.event_proxy.clone(),
        })
    }
}


/// **Read what every pane's header depends on**, once a frame. `None` when the info bar is
/// disabled: no pane gets one.
///
/// This builds no widget. Each pane's header (a [`Keyed`](heca_grid_ui::widgets::Keyed) the pane
/// holds) is told the result by `show_pane_header`, and builds only if what it shows changed.
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
    let active_ws = state.layout().active_workspace_idx();
    let pane_ids: Vec<_> = state.layout().active_workspace()
        .map(|workspace| workspace.content().panes().map(|pane| pane.id).collect())
        .unwrap_or_default();
    let mut inputs = std::collections::HashMap::with_capacity(pane_ids.len());
    for pane_id in pane_ids {
        let (ws_idx, col_idx) = crate::find_pane_location(&state.session, pane_id)
            .map(|(ws, col, _)| (ws, col))
            .unwrap_or((active_ws, 0));
        let (name, custom_name, runtime) = state
            .layout()
            .active_workspace()
            .and_then(|ws| ws.content().find_pane(pane_id))
            .map(|p| {
                (
                    p.title.clone(),
                    p.custom_name.clone(),
                    state.chrome_state.panes.presentation(pane_id),
                )
            })
            .unwrap_or_else(|| (String::new(), None, None));
        // Floating panes aren't in any column (`find_pane_location` returns None); detect them
        // directly so the bar hides tiled-only buttons + flags float active.
        let floating = state
            .layout()
            .active_workspace()
            .map(|ws| ws.floating_panes.iter().any(|f| f.pane.id == pane_id))
            .unwrap_or(false);
        let zoomed = !floating
            && state
                .layout()
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
            },
        );
    }
    // An added chip's kept answers are for panes that are still there.
    state
        .pane_chips
        .retain_panes(&inputs.keys().copied().collect());
    Some(inputs)
}
