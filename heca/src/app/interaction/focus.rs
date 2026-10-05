//! Focus-target helpers: which pane has the keyboard, and whether a pane can take it.

use crate::app_state::AppState;
use heca_core::layout::{Layout, PaneId};
use super::types::InteractionSource;
use super::domain::is_floating_domain;


/// Returns the focused pane ID from `AppState`, if any.
///
/// This is the canonical accessor — handlers should read
/// `state.focused_pane` through this helper rather than touching
/// the field directly, so the access pattern is traceable.
pub(crate) fn focused_pane_id(state: &AppState) -> Option<PaneId> {
    state.focused_pane
}

/// Checks whether a specific pane can receive focus from the given source.
///
/// When in floating domain, only the active floating pane can receive focus
/// from mouse/sidebar sources. Keyboard focus changes are blocked entirely
/// (they go through `dispatch_action` which handles policy).
///
/// Checks whether `pane_id` can receive focus from the given source.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by handlers and sidebar routing in Phase E"
    )
)]
pub(crate) fn can_focus_pane(
    layout: Layout<'_>,
    source: InteractionSource,
    pane_id: PaneId,
) -> bool {
    if is_floating_domain(layout) {
        // In floating domain, only the active floating pane can receive focus.
        let active_floating = layout
            .active_workspace()
            .and_then(|ws| ws.content().floating_panes.iter().find(|f| f.is_active))
            .map(|f| f.pane.id);
        active_floating == Some(pane_id)
    } else {
        // In tiled domain, any pane can receive focus from any source.
        // (Individual sources may still block via route_interaction, but
        // this helper only answers the pane-targeting question.)
        match source {
            InteractionSource::Keyboard => true,
            InteractionSource::MouseContent => true,
            InteractionSource::MouseLeftSidebar => true,
            // A component asking to focus a pane is the sidebar's "activate this row" in another
            // shape — allowed in the tiled domain like every other source, as is a script's. A
            // layer's own tree is the same act again: choosing a card in the exposé.
            InteractionSource::Provider
            | InteractionSource::Rpc
            | InteractionSource::Surface(_) => true,
        }
    }
}
