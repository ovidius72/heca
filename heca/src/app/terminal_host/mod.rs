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
mod selection;
mod wheel;

pub(crate) use frames::{column_frames, pane_outer_frames};
pub(crate) use hyperlinks::{
    cell_screen_pos, collect_link_hints, hyperlink_uri_at_position, hyperlink_uri_at_stable_cell,
};
pub(crate) use input::{HeardPresses, on_left_release, on_terminal_input};
pub(crate) use search::{enter_scrollback_search, run_scrollback_search, search_step};
pub(crate) use selection::{
    ensure_caret_visible, enter_selection_mode_for_focused_terminal,
    move_focused_terminal_selection,
};

use crate::app::backend_store::BackendStore;
use crate::app_state::AppState;
use heca_core::backend::{TerminalDamage, TerminalSnapshot};
use heca_core::layout::{PaneId, Rectangle};

#[derive(Clone)]
pub(crate) struct TerminalMount {
    pub content_rect: Rectangle,
    pub snapshot: TerminalSnapshot,
    /// Pending visible damage for this pane's terminal content. Carried through
    /// the mount/render boundary even when the renderer still falls back to
    /// full redraw, so later retained-content work can consume real row damage
    /// without changing the host contract again.
    pub damage: TerminalDamage,
}

pub(crate) fn prepare_terminal_mount(
    backends: &mut BackendStore,
    terminal: crate::chrome::terminal::TerminalId,
    content_rect: Rectangle,
    scale: f32,
) -> Option<TerminalMount> {
    let backend = backends.get_mut_by_id(terminal)?;
    // Keep the device scale current so inline images report physical pixels (crisp on HiDPI).
    // Cheap: the backend ignores an unchanged scale. The *grid* is not set here — the terminal asks
    // for it when it changes (`TerminalInput::Resize`).
    backend.set_scale_factor(scale);
    let snapshot = backend.terminal_snapshot()?;
    let damage = backend.take_terminal_damage();

    Some(TerminalMount {
        content_rect,
        snapshot,
        damage,
    })
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
        && let Some(backend) = state.backends.get_mut(prev)
    {
        backend.focus_changed(false);
    }

    if let Some(next) = next
        && Some(next) != prev
        && let Some(backend) = state.backends.get_mut(next)
    {
        backend.focus_changed(true);
    }
}

pub(crate) fn notify_window_focus_changed(state: &mut AppState, focused: bool) {
    let Some(pane_id) = state.focused_pane else {
        return;
    };
    if let Some(backend) = state.backends.get_mut(pane_id) {
        backend.focus_changed(focused);
    }
}
