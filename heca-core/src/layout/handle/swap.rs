//! Swapping two panes, wherever they are: one column, two columns of a workspace, or two
//! workspaces. Every swap animates each pane from where it was to where it lands.

use crate::layout::animation::AnimationConfig;
use crate::layout::types::{ColumnId, Point, Rectangle};
use crate::layout::{LayoutMut, Pane, PaneId, WorkspaceMut};

/// Where a pane is: workspace, column, and row within the column.
#[derive(Clone, Copy)]
struct Spot {
    ws: usize,
    col: usize,
    pane: usize,
}

impl LayoutMut<'_> {
    /// Swap two panes. Answers `false` — and changes nothing — when they are the same pane or
    /// either one is not in a column.
    pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> bool {
        if a == b {
            return false;
        }
        let (Some(at), Some(bt)) = (self.spot_of(a), self.spot_of(b)) else {
            return false;
        };
        match (at.ws == bt.ws, at.col == bt.col) {
            (true, true) => self
                .workspace_mut(at.ws)
                .is_some_and(|mut ws| ws.swap_in_column(at.col, at.pane, bt.pane)),
            (true, false) => self.swap_between_columns(a, b, at, bt),
            (false, _) => self.swap_between_workspaces(a, b, at, bt),
        }
    }

    fn spot_of(&self, pane: PaneId) -> Option<Spot> {
        self.session.pane_location(pane).map(|(ws, col, pane)| Spot { ws, col, pane })
    }

    /// Two columns of one workspace. Placeholders hold both landing places while the panes are
    /// lifted out, so no column disappears under the swap (which would slide the others).
    fn swap_between_columns(&mut self, a: PaneId, b: PaneId, at: Spot, bt: Spot) -> bool {
        let placeholder_a = PaneId(self.next_id());
        let placeholder_b = PaneId(self.next_id());
        let column_for_a = ColumnId(self.next_id());
        let column_for_b = ColumnId(self.next_id());
        let slide = self.slide();
        let Some(mut ws) = self.workspace_mut(at.ws) else {
            return false;
        };
        let old_a = rect_of(&ws, a);
        let old_b = rect_of(&ws, b);

        // Descending column order, so creating one column does not shift the other's index.
        let mut holds = [
            (bt, placeholder_a, column_for_a),
            (at, placeholder_b, column_for_b),
        ];
        holds.sort_by_key(|(spot, ..)| std::cmp::Reverse(spot.col));
        for (spot, placeholder, new_column) in holds {
            let pane = Pane::new(placeholder, String::new());
            match spot.col < ws.scrolling.columns.len() {
                true => {
                    let row = spot.pane.min(ws.scrolling.columns[spot.col].panes.len());
                    ws.scroll_mut().add_pane_to_column(spot.col, Some(row), pane, true);
                }
                false => {
                    let at = spot.col.min(ws.scrolling.columns.len());
                    ws.scroll_mut().add_new_column(Some(at), new_column, pane, true);
                }
            }
        }

        // Lift the real panes out, last column first so the indices hold.
        let mut lifted: Vec<(usize, usize, PaneId)> = [a, b]
            .into_iter()
            .filter_map(|id| ws.scrolling.pane_indices(id).map(|(c, r)| (c, r, id)))
            .collect();
        lifted.sort_by_key(|(col, ..)| std::cmp::Reverse(*col));
        let (mut lifted_a, mut lifted_b) = (None, None);
        for (col, row, id) in lifted {
            if let Some(pane) = ws.scroll_mut().remove_pane(col, row) {
                match id == a {
                    true => lifted_a = Some(pane),
                    false => lifted_b = Some(pane),
                }
            }
        }

        if let Some(pane) = lifted_a {
            put_over_placeholder(&mut ws, placeholder_a, pane, old_a, slide);
        }
        if let Some(pane) = lifted_b {
            put_over_placeholder(&mut ws, placeholder_b, pane, old_b, slide);
        }
        true
    }

    /// Two workspaces. Each pane goes where the other was; a column that only held the pane is
    /// made again for the one arriving.
    fn swap_between_workspaces(&mut self, a: PaneId, b: PaneId, at: Spot, bt: Spot) -> bool {
        let slide = self.slide();
        let old_a = self.workspace_mut(at.ws).and_then(|ws| rect_of(&ws, a));
        let old_b = self.workspace_mut(bt.ws).and_then(|ws| rect_of(&ws, b));

        let Some(lifted_a) = self.workspace_mut(at.ws).and_then(|mut ws| lift(&mut ws, a)) else {
            return false;
        };
        let Some(lifted_b) = self.workspace_mut(bt.ws).and_then(|mut ws| lift(&mut ws, b)) else {
            return false;
        };

        if let Some(mut ws) = self.workspace_mut(bt.ws) {
            land(&mut ws, lifted_a, bt);
            slide_from(&mut ws, a, old_a, slide);
        }
        if let Some(mut ws) = self.workspace_mut(at.ws) {
            land(&mut ws, lifted_b, at);
            slide_from(&mut ws, b, old_b, slide);
        }
        true
    }

    pub(super) fn slide(&self) -> Point {
        let viewport = self.reader().viewport();
        let reach = self.session.options.move_slide_reach;
        Point::new(viewport.w * reach, viewport.h * reach)
    }
}

impl WorkspaceMut<'_> {
    /// Swap the pane in row `first` of column `col` with the one in row `second`, animating both
    /// vertically. Answers `false` when there is nothing to swap.
    pub fn swap_in_column(&mut self, col: usize, first: usize, second: usize) -> bool {
        if first == second {
            return false;
        }
        let gap = self.scrolling.options.gaps;
        let height = self.scroll().area().size.h;
        let Some(column) = self.scrolling.columns.get_mut(col) else {
            return false;
        };
        if first >= column.panes.len() || second >= column.panes.len() {
            return false;
        }
        // The active pane travels with the swap when it is one of the two.
        let new_active = match column.active_pane_idx {
            i if i == first => second,
            i if i == second => first,
            i => i,
        };
        let (upper, lower) = (first.min(second), first.max(second));
        let heights = column.pane_heights(height, gap);
        let up_offset = heights.get(upper).copied().unwrap_or(0.0) + gap;
        let down_offset = -(heights.get(lower).copied().unwrap_or(0.0) + gap);

        column.panes[upper].animate_move_y_from(down_offset, AnimationConfig::default());
        column.panes[lower].animate_move_y_from(up_offset, AnimationConfig::default());
        column.panes.swap(upper, lower);
        column.active_pane_idx = new_active;
        true
    }

    /// Swap the active pane with the one above it in its column.
    pub fn swap_active_pane_up(&mut self) -> bool {
        let col = self.scroll().active_column_idx();
        let Some(row) = self.scroll().active_column().map(|c| c.active_pane_idx) else {
            return false;
        };
        self.swap_in_column(col, row, row.saturating_sub(1))
    }

    /// Swap the active pane with the one below it in its column.
    pub fn swap_active_pane_down(&mut self) -> bool {
        let col = self.scroll().active_column_idx();
        let Some((row, len)) = self.scroll().active_column().map(|c| (c.active_pane_idx, c.panes.len()))
        else {
            return false;
        };
        self.swap_in_column(col, row, (row + 1).min(len.saturating_sub(1)))
    }
}

pub(super) fn rect_of(ws: &WorkspaceMut<'_>, pane: PaneId) -> Option<Rectangle> {
    ws.scroll()
        .panes_with_positions()
        .into_iter()
        .find_map(|(id, rect)| (id == pane).then_some(rect))
}

/// How a pane came out of its workspace: the pane, whether its column went with it, and that
/// column's identity.
struct Lifted {
    pane: Pane,
    column_gone: bool,
    column_id: ColumnId,
}

fn lift(ws: &mut WorkspaceMut<'_>, pane: PaneId) -> Option<Lifted> {
    let (col, row) = ws.scrolling.pane_indices(pane)?;
    let column = &ws.scrolling.columns[col];
    let (column_gone, column_id) = (column.panes.len() == 1, column.id);
    let pane = ws.scroll_mut().remove_pane(col, row)?;
    Some(Lifted { pane, column_gone, column_id })
}

/// Put a lifted pane into `spot` of `ws`: back into its column when that still exists, or into a
/// fresh column with the same identity when the swap had emptied it.
fn land(ws: &mut WorkspaceMut<'_>, lifted: Lifted, spot: Spot) {
    let Lifted { pane, column_gone, column_id } = lifted;
    if column_gone {
        let at = spot.col.min(ws.scrolling.columns.len());
        let column = ws.scrolling.new_column(column_id, pane);
        ws.scroll_mut().add_column(Some(at), column, true);
        return;
    }
    let last = ws.scrolling.columns.len().saturating_sub(1);
    let col = ws
        .scrolling
        .columns
        .iter()
        .position(|c| c.id == column_id)
        .unwrap_or(spot.col.min(last));
    let row = spot.pane.min(ws.scrolling.columns[col].panes.len());
    ws.scroll_mut().add_pane_to_column(col, Some(row), pane, true);
}

/// Replace the placeholder with the real pane, then slide the pane in from where it was.
fn put_over_placeholder(
    ws: &mut WorkspaceMut<'_>,
    placeholder: PaneId,
    pane: Pane,
    from: Option<Rectangle>,
    slide: Point,
) {
    let Some((col, row)) = ws.scrolling.pane_indices(placeholder) else {
        return;
    };
    let id = pane.id;
    ws.scrolling.columns[col].panes[row] = pane;
    ws.scrolling.columns[col].active_pane_idx = row;
    slide_from(ws, id, from, slide);
}

/// Animate a pane from `from` to where it is now, at most `slide` away.
pub(super) fn slide_from(ws: &mut WorkspaceMut<'_>, pane: PaneId, from: Option<Rectangle>, slide: Point) {
    let (Some(from), Some((col, row))) = (from, ws.scrolling.pane_indices(pane)) else {
        return;
    };
    let Some(to) = rect_of(ws, pane) else {
        return;
    };
    let dx = (from.loc.x - to.loc.x).clamp(-slide.x, slide.x);
    let dy = (from.loc.y - to.loc.y).clamp(-slide.y, slide.y);
    ws.scrolling.columns[col].panes[row]
        .animate_move_from(Point::new(dx, dy), AnimationConfig::default());
}
