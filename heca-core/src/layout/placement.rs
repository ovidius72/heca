//! **Putting a pane at an exact place** — and taking it out of wherever it was.
//!
//! What a drop means once the gesture has said where it landed, and what the `place_pane` action
//! asks for (F003/P082/T509). The workspace owns it so no caller removes and re-inserts panes by
//! hand, each deciding clamping and focus for itself.

use super::column::Pane;
use super::types::*;
use super::workspace::{FocusDomain, Workspace};

impl Workspace {
    /// Take pane `pane_id` out of this workspace, wherever it is: its column, or the floating
    /// layer. `None` when it is not here.
    pub fn take_pane(&mut self, pane_id: PaneId) -> Option<Pane> {
        if let Some((col, row)) = self.scrolling.pane_indices(pane_id) {
            return self.scrolling.remove_pane(col, row);
        }
        let idx = self
            .floating_panes
            .iter()
            .position(|f| f.pane.id == pane_id)?;
        let float = self.floating_panes.remove(idx);
        if self.floating_panes.is_empty() {
            self.focus_domain = FocusDomain::Tiled;
        }
        Some(float.pane)
    }

    /// Put `pane` at row `row` of column `col` — or, with no row (or no such column), in a new
    /// column `new_column_id` at `col`. Indices past the end clamp to it. The pane is focused, in
    /// the tiling.
    pub fn place_pane(
        &mut self,
        pane: Pane,
        col: usize,
        row: Option<usize>,
        new_column_id: ColumnId,
    ) {
        let columns = self.scrolling.columns.len();
        match row {
            Some(row) if col < columns => {
                let row = row.min(self.scrolling.columns[col].panes.len());
                self.scrolling
                    .add_pane_to_column(col, Some(row), pane, true);
            }
            _ => {
                let column = self.scrolling.new_column(new_column_id, pane);
                self.scrolling
                    .add_column(Some(col.min(columns)), column, true);
            }
        }
        self.deactivate_floating_panes();
        self.focus_domain = FocusDomain::Tiled;
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::types::{LayoutOptions, SessionId, Size};
    use crate::layout::{ColumnId, FocusDomain, Pane, PaneId, Session};

    /// One workspace, three panes, each in its own column: ids 1, 2, 3.
    fn session() -> Session {
        let mut s = Session::new(
            SessionId(1),
            Size::new(1000.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        for i in 1..=3 {
            s.add_pane(Pane::new(PaneId(i), ""), None, true);
        }
        s
    }

    #[test]
    fn a_pane_is_taken_from_its_column_or_from_the_floating_layer() {
        let mut s = session();
        let ws = s.active_workspace_mut().unwrap();
        assert_eq!(ws.take_pane(PaneId(2)).map(|p| p.id), Some(PaneId(2)));
        assert!(ws.scrolling.pane_indices(PaneId(2)).is_none());

        let rect = ws.default_float_rect();
        ws.add_floating_pane(Pane::new(PaneId(9), ""), rect, None);
        assert_eq!(ws.take_pane(PaneId(9)).map(|p| p.id), Some(PaneId(9)));
        assert!(ws.floating_panes.is_empty());
        assert_eq!(ws.focus_domain, FocusDomain::Tiled);
        assert!(ws.take_pane(PaneId(42)).is_none());
    }

    #[test]
    fn a_pane_is_placed_at_a_row_of_a_column() {
        let mut s = session();
        let ws = s.active_workspace_mut().unwrap();
        let pane = ws.take_pane(PaneId(3)).unwrap();
        ws.place_pane(pane, 0, Some(0), ColumnId(99));
        assert_eq!(ws.scrolling.pane_indices(PaneId(3)), Some((0, 0)));
        assert_eq!(ws.active_pane().map(|p| p.id), Some(PaneId(3)));
    }

    #[test]
    fn with_no_row_the_pane_gets_a_column_of_its_own() {
        let mut s = session();
        let ws = s.active_workspace_mut().unwrap();
        let pane = ws.take_pane(PaneId(3)).unwrap();
        ws.place_pane(pane, 0, None, ColumnId(99));
        assert_eq!(ws.scrolling.columns[0].id, ColumnId(99));
        assert_eq!(ws.scrolling.pane_indices(PaneId(3)), Some((0, 0)));
    }

    #[test]
    fn a_place_past_the_end_clamps_to_it() {
        let mut s = session();
        let ws = s.active_workspace_mut().unwrap();
        let pane = ws.take_pane(PaneId(1)).unwrap();
        ws.place_pane(pane, 50, Some(50), ColumnId(99));
        let last = ws.scrolling.columns.len() - 1;
        assert_eq!(ws.scrolling.pane_indices(PaneId(1)), Some((last, 0)));
    }
}
