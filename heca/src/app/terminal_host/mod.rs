//! Terminal host adapter between pane shells and terminal backends.
//!
//! This keeps terminal sizing and snapshot acquisition out of a specific pane
//! implementation so future `heca-grid-ui` pane shells can host terminals
//! through the same contract. What a terminal's input *means* — the pointer, the wheel, selection,
//! links, search — lives in the files beside this one, one concern each.

mod frames;
mod hyperlinks;
mod input;
mod search;
pub(crate) mod selection;
mod typing;
mod wheel;

pub(crate) use frames::{column_frames, laid_out_pane_ids, layout_origin, pane_outer_frames};
pub(crate) use hyperlinks::{cell_screen_pos, collect_link_hints};
pub(crate) use input::{HeardPresses, on_left_release, on_terminal_input};
pub(crate) use search::{enter_scrollback_search, search_step};
pub(crate) use selection::{
    ensure_caret_visible, enter_selection_mode_for_focused_terminal,
    move_focused_terminal_selection,
};

use crate::app_state::AppState;
use heca_core::backend::{TerminalDamage, TerminalSnapshot};
use heca_core::layout::{PaneId, Rectangle};

#[derive(Clone)]
pub(crate) struct TerminalMount {
    /// The **grid's** box, relative to where the surface is drawn: the surface's
    /// box held in by the terminal's configured padding (`Rectangle::loc` is the
    /// inset, i.e. (0, 0) with no padding). Cells, cursor and selection live here.
    pub content_rect: Rectangle,
    /// The surface's **whole** box, relative to itself: (0, 0) with the surface's
    /// own size. The terminal's background fills it — the padding is *inside* the
    /// terminal — while the grid above stays held in by `content_rect`.
    pub surface_rect: Rectangle,
    pub snapshot: TerminalSnapshot,
    /// Pending visible damage for this pane's terminal content. Carried through
    /// the mount/render boundary even when the renderer still falls back to
    /// full redraw, so later retained-content work can consume real row damage
    /// without changing the host contract again.
    pub damage: TerminalDamage,
}

/// **Did a terminal that holds the keyboard in the window tree take this intent?** Only asked while
/// the keyboard really is in the tree (a dock or a layer in front); the panes' keys are not. The
/// one door a terminal action uses to reach a terminal placed anywhere before falling back to the
/// focused pane's.
pub(crate) fn tree_answers(state: &mut AppState, intent: heca_grid_ui::WidgetIntent) -> bool {
    crate::app::tree_keys::window_holds_keyboard(state)
        && crate::chrome::deliver(state, &heca_grid_ui::Event::Widget(intent))
}

pub(crate) fn notify_focus_changed(
    state: &mut AppState,
    prev: Option<PaneId>,
    next: Option<PaneId>,
) {
    if prev == next {
        return;
    }

    if let Some(prev) = prev
        && Some(prev) != next
        && let Some(backend) = state.server.backends.get_mut(prev)
    {
        backend.focus_changed(false);
    }

    if let Some(next) = next
        && Some(next) != prev
        && let Some(backend) = state.server.backends.get_mut(next)
    {
        backend.focus_changed(true);
    }
}

pub(crate) fn notify_window_focus_changed(state: &mut AppState, focused: bool) {
    let Some(pane_id) = state.focused_pane else {
        return;
    };
    if let Some(backend) = state.server.backends.get_mut(pane_id) {
        backend.focus_changed(focused);
    }
}
