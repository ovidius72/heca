//! **The one place the workspace's model is read from the app.**
//!
//! Everything below it takes plain data; this takes `AppState`, once a frame, and says what the
//! columns, the floats and the panes in them are. Nothing here places anything.

use std::collections::{HashMap, HashSet};

use heca_core::layout::PaneId;

use super::{PaneEntry, WorkspaceModel};
use crate::app_state::AppState;
use crate::chrome::column::{ColumnShellModel, focus_pane_of, picking_from_column};
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
            // Only the column the picked pane is in offers a new one beside it.
            new_column_slot: picking_from_column(state, col)
                .then(|| state.appearance.effective_new_column_slot_share()),
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
    WorkspaceModel {
        area,
        working_width,
        columns,
        floats,
        panes,
    }
}
