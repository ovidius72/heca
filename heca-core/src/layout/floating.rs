//! **Floating a pane** — where it goes when it floats, and where it goes back.
//!
//! The workspace owns this rather than each caller: floating, floating at a given place, spawning a
//! pane that floats and unfloating were four hand-written copies in the app, each pushing into
//! `floating_panes` and choosing the default size for itself (F003/P082/T509).

use super::column::Pane;
use super::types::*;
use super::workspace::{FloatingPane, FocusDomain, Workspace};

impl Workspace {
    /// Where a pane floats when nothing says where: centred, [`float_size`] of the working area.
    ///
    /// [`float_size`]: LayoutOptions::float_size
    pub fn default_float_rect(&self) -> Rectangle {
        self.scrolling
            .working_area
            .centred_fraction(self.scrolling.options.float_size)
    }

    /// Put `pane` in front as a floating pane at `rect`, and give floating the focus.
    ///
    /// `origin` is the column and row it came from, so [`unfloat_pane`](Self::unfloat_pane) can put
    /// it back; `None` for a pane that never tiled (one spawned floating).
    pub fn add_floating_pane(
        &mut self,
        pane: Pane,
        rect: Rectangle,
        origin: Option<(usize, usize)>,
    ) {
        self.deactivate_floating_panes();
        self.floating_panes.push(FloatingPane {
            pane,
            position: rect.loc,
            size: rect.size,
            is_active: true,
            original_column_idx: origin.map(|(col, _)| col),
            original_pane_idx: origin.map(|(_, row)| row),
        });
        self.focus_domain = FocusDomain::Floating;
    }

    /// Take the tiled pane `pane_id` out of its column and float it at `rect`, remembering where it
    /// was. `false` when it is not tiled here.
    pub fn float_tiled_pane(&mut self, pane_id: PaneId, rect: Rectangle) -> bool {
        let Some((col, row)) = self.scrolling.pane_indices(pane_id) else {
            return false;
        };
        let Some(pane) = self.scrolling.remove_pane(col, row) else {
            return false;
        };
        self.add_floating_pane(pane, rect, Some((col, row)));
        true
    }

    /// Put the floating pane `pane_id` back into the tiling and focus it there — at the column and
    /// row it came from while that column still exists, else in a new column `new_column_id`.
    /// `false` when it is not floating here.
    pub fn unfloat_pane(&mut self, pane_id: PaneId, new_column_id: ColumnId) -> bool {
        let Some(idx) = self
            .floating_panes
            .iter()
            .position(|f| f.pane.id == pane_id)
        else {
            return false;
        };
        let float = self.floating_panes.remove(idx);
        self.deactivate_floating_panes();
        match float.original_column_idx {
            Some(col) if col < self.scrolling.columns.len() => {
                let row = float
                    .original_pane_idx
                    .unwrap_or(0)
                    .min(self.scrolling.columns[col].panes.len());
                self.scrolling
                    .add_pane_to_column(col, Some(row), float.pane, true);
            }
            _ => {
                let column = self.scrolling.new_column(new_column_id, float.pane);
                self.scrolling.add_column(None, column, true);
            }
        }
        self.focus_domain = FocusDomain::Tiled;
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::types::{LayoutOptions, Point, Rectangle, SessionId, Size};
    use crate::layout::{ColumnId, FocusDomain, Pane, PaneId, Session};

    /// One workspace holding three panes, each in its own column: ids 1, 2, 3.
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
    fn the_default_float_is_centred_at_the_configured_share() {
        let s = session();
        let ws = s.active_workspace().unwrap();
        let area = ws.scrolling.working_area;
        let rect = ws.default_float_rect();
        let share = LayoutOptions::default().float_size;
        assert_eq!(rect.size.w, area.size.w * share);
        assert_eq!(rect.size.h, area.size.h * share);
        let centre = |r: Rectangle| (r.loc.x + r.size.w / 2.0, r.loc.y + r.size.h / 2.0);
        assert_eq!(centre(rect), centre(area));
    }

    #[test]
    fn floating_a_tiled_pane_takes_it_out_of_its_column_and_focuses_it() {
        let mut s = session();
        let ws = s.active_workspace_mut().unwrap();
        let rect = Rectangle::new(Point::new(10.0, 20.0), Size::new(300.0, 200.0));
        assert!(ws.float_tiled_pane(PaneId(2), rect));
        assert!(ws.scrolling.pane_indices(PaneId(2)).is_none());
        let float = ws
            .floating_panes
            .iter()
            .find(|f| f.pane.id == PaneId(2))
            .unwrap();
        assert_eq!((float.position, float.size), (rect.loc, rect.size));
        assert_eq!(ws.focus_domain, FocusDomain::Floating);
        assert_eq!(ws.active_pane().map(|p| p.id), Some(PaneId(2)));
    }

    #[test]
    fn unfloating_puts_a_pane_back_where_it_came_from() {
        let mut s = session();
        let ws = s.active_workspace_mut().unwrap();
        let before = ws.scrolling.pane_indices(PaneId(2));
        let rect = ws.default_float_rect();
        ws.float_tiled_pane(PaneId(2), rect);
        assert!(ws.unfloat_pane(PaneId(2), ColumnId(99)));
        assert_eq!(ws.scrolling.pane_indices(PaneId(2)), before);
        assert!(ws.floating_panes.is_empty());
        assert_eq!(ws.focus_domain, FocusDomain::Tiled);
    }

    #[test]
    fn a_pane_that_never_tiled_unfloats_into_a_new_column() {
        let mut s = session();
        let ws = s.active_workspace_mut().unwrap();
        let columns = ws.scrolling.columns.len();
        let rect = ws.default_float_rect();
        ws.add_floating_pane(Pane::new(PaneId(7), ""), rect, None);
        assert!(ws.unfloat_pane(PaneId(7), ColumnId(99)));
        assert_eq!(ws.scrolling.columns.len(), columns + 1);
        assert!(ws.scrolling.columns.iter().any(|c| c.id == ColumnId(99)));
    }

    #[test]
    fn only_a_pane_that_is_there_can_float_or_unfloat() {
        let mut s = session();
        let ws = s.active_workspace_mut().unwrap();
        let rect = ws.default_float_rect();
        assert!(!ws.float_tiled_pane(PaneId(42), rect));
        assert!(!ws.unfloat_pane(PaneId(1), ColumnId(99)));
    }
}
