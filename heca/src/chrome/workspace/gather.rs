//! **The one place the workspace's model is read from the app.**
//!
//! Everything below it takes plain data; this takes `AppState`, once a frame, and says what the
//! columns, the floats and the panes in them are. Nothing here places anything.

use std::collections::{HashMap, HashSet};

use heca_core::layout::PaneId;

use super::{PaneEntry, PickMark, WorkspaceModel};
use crate::app_state::AppState;
use crate::chrome::column::{ColumnShellModel, focus_pane_of};
use crate::chrome::pane::{PaneShellModel, pane_models};
use crate::chrome::pane_header::PaneHeaderInput;

/// What the workspace is this frame. `headers` is what each pane's header shows, when the info bar
/// is on; each is handed to the pane it is about.
pub(crate) fn gather(
    state: &mut AppState,
    mut headers: Option<HashMap<PaneId, PaneHeaderInput>>,
) -> WorkspaceModel {
    let area = crate::chrome::ChromeConfig::of(state).content_rect();
    let models = pane_models(state);
    let tiled: HashSet<PaneId> = state
        .session
        .active_workspace()
        .map(|ws| {
            ws.scrolling
                .panes_with_positions()
                .into_iter()
                .map(|(id, _)| id)
                .collect()
        })
        .unwrap_or_default();

    let columns: Vec<ColumnShellModel> = crate::app::terminal_host::column_frames(state)
        .iter()
        .map(|col| ColumnShellModel {
            col_id: col.id,
            x: col.rect.loc.x as f32,
            y: col.rect.loc.y as f32,
            w: col.rect.size.w as f32,
            h: col.rect.size.h as f32,
            focus_pane: focus_pane_of(state, col),
            // The panes this column holds, in the order the layout engine placed them.
            panes: col
                .panes
                .iter()
                .filter_map(|p| models.iter().find(|m| m.pane_id == p.id).cloned())
                .collect(),
        })
        .collect();
    let floats: Vec<PaneShellModel> = models
        .iter()
        .filter(|m| !tiled.contains(&m.pane_id))
        .cloned()
        .collect();

    // What each pane runs: the terminal the app keeps for it. Read last, so the models above are
    // taken before anything is made.
    let panes = models
        .iter()
        .map(|m| {
            (
                m.pane_id,
                PaneEntry {
                    content: crate::chrome::terminal::view_of(state, m.pane_id),
                    header: headers.as_mut().and_then(|h| h.remove(&m.pane_id)),
                },
            )
        })
        .collect();
    let working_width = state
        .session
        .active_workspace()
        .map_or(0.0, |ws| ws.scrolling.working_area.size.w as f32);
    let pick = pick_marks(state);
    WorkspaceModel {
        pick,
        area,
        working_width,
        slot_share: state.appearance.effective_new_column_slot_share(),
        columns,
        floats,
        panes,
    }
}

/// What the open column pick is offering, as the workspace shows it. Nothing when no pick is open.
fn pick_marks(state: &AppState) -> Vec<PickMark> {
    use crate::app_state::ColumnPickTarget;
    let Some(candidates) = state.input_mode.col_candidates() else {
        return Vec::new();
    };
    candidates
        .iter()
        .filter_map(|(letter, target)| match *target {
            ColumnPickTarget::NewColumn { gap } => Some(PickMark::Gap {
                at: gap,
                letter: *letter,
            }),
            ColumnPickTarget::Existing { ws_idx, col_id, .. }
                if ws_idx == state.session.active_workspace_idx =>
            {
                Some(PickMark::Column(col_id))
            }
            ColumnPickTarget::Existing { .. } => None,
        })
        .collect()
}
