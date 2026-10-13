//! **Floating a pane** — where it goes when it floats, and where it goes back.
//!
//! The workspace owns this rather than each caller: floating, floating at a given place, spawning a
//! pane that floats and unfloating were four hand-written copies in the app, each pushing into
//! `floating_panes` and choosing the default size for itself.

use super::column::Pane;
use super::types::*;
use super::scrolling::SpaceEffect;
use super::workspace::{FloatOrigin, FloatingPane, FocusDomain, Workspace};

impl Workspace {
    /// Put `pane` in front as a floating pane at `rect`, and give floating the focus.
    ///
    /// `origin` is where it came from, so [`unfloat_pane`](Self::unfloat_pane) can put it back;
    /// `None` for a pane that never tiled (one spawned floating).
    pub fn add_floating_pane(&mut self, pane: Pane, rect: Rectangle, origin: Option<FloatOrigin>) {
        self.deactivate_floating_panes();
        self.floating_panes.push(FloatingPane {
            pane,
            position: rect.loc,
            size: rect.size,
            is_active: true,
            origin,
        });
        self.focus_domain = FocusDomain::Floating;
    }
}

impl Workspace {
    /// Where a pane floats when nothing says where: centred in `area`, [`float_size`] of it.
    ///
    /// [`float_size`]: crate::layout::types::LayoutOptions::float_size
    pub fn float_rect_in(&self, area: Rectangle) -> Rectangle {
        area.centred_fraction(self.scrolling.options.float_size)
    }

    /// Take the tiled pane `pane_id` out of its column and float it at `rect`, remembering where it
    /// was. Says what leaving did to the columns; `None` when it is not tiled here.
    pub fn float_tiled_pane(&mut self, pane_id: PaneId, rect: Rectangle) -> Option<SpaceEffect> {
        let (col, row) = self.scrolling.pane_indices(pane_id)?;
        // Read before the pane leaves: a column that loses its last pane goes with it.
        let origin = FloatOrigin {
            column: self.scrolling.columns[col].id,
            position: col,
            row,
        };
        let (pane, effect) = self.scrolling.take_pane_from(col, row)?;
        self.add_floating_pane(pane, rect, Some(origin));
        Some(effect)
    }

    /// Put the floating pane `pane_id` back into the tiling — in the column it came from, at its
    /// row, while that column still exists; else in a new column `new_column_id` where that column
    /// used to be (at the end for a pane that never tiled) — making room for it in a window
    /// `working_height` tall. Says what arriving did to the columns; `None` when it is not floating
    /// here.
    pub fn unfloat_pane(
        &mut self,
        pane_id: PaneId,
        new_column_id: ColumnId,
        working_height: f64,
    ) -> Option<SpaceEffect> {
        let idx = self
            .floating_panes
            .iter()
            .position(|f| f.pane.id == pane_id)?;
        let float = self.floating_panes.remove(idx);
        let home = float.origin.and_then(|origin| {
            let col = self
                .scrolling
                .columns
                .iter()
                .position(|column| column.id == origin.column)?;
            Some((col, origin.row))
        });
        let (col, row) = match home {
            Some((col, row)) => (col, Some(row)),
            None => {
                let end = self.scrolling.columns.len();
                (float.origin.map_or(end, |origin| origin.position.min(end)), None)
            }
        };
        self.place_pane(float.pane, col, row, new_column_id, working_height)
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::testing::Windowed;
    use crate::layout::types::{LayoutOptions, Point, Rectangle, Size};
    use crate::layout::{ColumnId, FocusDomain, Pane, PaneId};

    /// The window a pane is given room in.
    const HEIGHT: f64 = 800.0;

    /// One workspace holding three panes, each in its own column: ids 1, 2, 3.
    fn session() -> Windowed {
        let mut s = Windowed::new(Size::new(1000.0, 800.0), 1.0);
        for i in 1..=3 {
            s.m().add_pane(Pane::new(PaneId(i), ""), None, true);
        }
        s
    }

    #[test]
    fn the_default_float_is_centred_at_the_configured_share() {
        let s = session();
        let ws = s.l().active_workspace().unwrap();
        let area = ws.scroll().area();
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
        let mut ws = s.ws();
        let rect = Rectangle::new(Point::new(10.0, 20.0), Size::new(300.0, 200.0));
        assert!(ws.float_tiled_pane(PaneId(2), rect).is_some());
        assert!(ws.scrolling.pane_indices(PaneId(2)).is_none());
        let float = ws
            .floating_panes
            .iter()
            .find(|f| f.pane.id == PaneId(2))
            .unwrap();
        assert_eq!((float.position, float.size), (rect.loc, rect.size));
        assert_eq!(ws.focus_domain, FocusDomain::Floating);
        assert_eq!(ws.reader().active_pane().map(|p| p.id), Some(PaneId(2)));
    }

    /// The ids of the panes in column order, one entry per column of one pane: the neighbours of a
    /// pane are what "back where it came from" means.
    fn order(ws: &super::Workspace) -> Vec<u64> {
        ws.scrolling
            .columns
            .iter()
            .flat_map(|c| c.panes.iter().map(|p| p.id.0))
            .collect()
    }

    #[test]
    fn unfloating_a_pane_alone_in_its_column_puts_it_back_between_the_same_neighbours() {
        let mut s = session();
        let mut ws = s.ws();
        assert_eq!(order(&ws), vec![1, 2, 3]);
        let rect = ws.reader().default_float_rect();
        ws.float_tiled_pane(PaneId(2), rect);
        // Its column went with it: pane 3 now sits where pane 2's column was.
        assert_eq!(order(&ws), vec![1, 3]);
        assert!(ws.unfloat_pane(PaneId(2), ColumnId(99), HEIGHT).is_some());
        assert_eq!(
            order(&ws),
            vec![1, 2, 3],
            "between 1 and 3, not split into 3"
        );
        let (col, row) = ws.scrolling.pane_indices(PaneId(2)).unwrap();
        assert_eq!(
            ws.scrolling.columns[col].panes.len(),
            1,
            "in a column of its own"
        );
        assert_eq!(row, 0);
        assert!(ws.floating_panes.is_empty());
        assert_eq!(ws.focus_domain, FocusDomain::Tiled);
    }

    #[test]
    fn unfloating_a_pane_from_a_shared_column_goes_back_to_its_row() {
        let mut s = session();
        let mut ws = s.ws();
        // Stack panes 4 and 5 under pane 1 in column 0 (rows 1 and 2).
        ws.scroll_mut().add_pane_to_column(0, None, Pane::new(PaneId(4), ""), false);
        ws.scroll_mut().add_pane_to_column(0, None, Pane::new(PaneId(5), ""), false);
        let before = ws.scrolling.pane_indices(PaneId(4)).unwrap();
        let rect = ws.reader().default_float_rect();
        ws.float_tiled_pane(PaneId(4), rect);
        assert!(ws.unfloat_pane(PaneId(4), ColumnId(99), HEIGHT).is_some());
        assert_eq!(ws.scrolling.pane_indices(PaneId(4)), Some(before));
    }

    #[test]
    fn a_pane_that_never_tiled_unfloats_into_a_new_column() {
        let mut s = session();
        let mut ws = s.ws();
        let columns = ws.scrolling.columns.len();
        let rect = ws.reader().default_float_rect();
        ws.add_floating_pane(Pane::new(PaneId(7), ""), rect, None);
        assert!(ws.unfloat_pane(PaneId(7), ColumnId(99), HEIGHT).is_some());
        assert_eq!(ws.scrolling.columns.len(), columns + 1);
        assert!(ws.scrolling.columns.iter().any(|c| c.id == ColumnId(99)));
    }

    #[test]
    fn only_a_pane_that_is_there_can_float_or_unfloat() {
        let mut s = session();
        let mut ws = s.ws();
        let rect = ws.reader().default_float_rect();
        assert!(ws.float_tiled_pane(PaneId(42), rect).is_none());
        assert!(ws.unfloat_pane(PaneId(1), ColumnId(99), HEIGHT).is_none());
    }
}
