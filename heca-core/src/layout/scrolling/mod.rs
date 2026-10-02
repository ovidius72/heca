use super::animation::{Animation, AnimationConfig};
use super::column::{Column, Pane};
use super::types::*;
use super::view_offset::{ViewOffset, compute_new_view_offset};
use super::window_view::ScrollView;

// Re-export PaneInsertTarget for convenience.
pub use super::types::PaneInsertTarget;

/// Minimum width (logical px) a column may be shrunk to by a manual resize, so a
/// column never becomes a thin line.
pub const MIN_COLUMN_WIDTH: f64 = 150.0;

/// Direction for creating a new column when moving a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Left,
    Right,
}

/// The columns of panes in one workspace — **the content every window shares**.
///
/// This is heca's equivalent of NIRI's `ScrollingSpace<W>`: columns are arranged left-to-right
/// with gaps. Where a window is looking at them — the scroll offset, the active column, the box
/// they are laid out in — is not here: it is a [`ScrollView`], held by the window. Anything that
/// needs both goes through [`ScrollingRef`] / [`ScrollingMut`], made with
/// [`through`](Self::through) / [`through_mut`](Self::through_mut).
#[derive(Debug, Clone)]
pub struct ScrollingSpace {
    /// Columns of panes.
    pub columns: Vec<Column>,
    /// Cached per-column data (computed widths).
    pub column_widths: Vec<f64>,
    /// Layout options.
    pub options: LayoutOptions,
}

/// A [`ScrollingSpace`] as one window sees it: the reads that need a view.
#[derive(Clone, Copy)]
pub struct ScrollingRef<'a> {
    space: &'a ScrollingSpace,
    view: &'a ScrollView,
}

/// A [`ScrollingSpace`] as one window sees it: the moves, which change the content and move the
/// view with it.
pub struct ScrollingMut<'a> {
    space: &'a mut ScrollingSpace,
    view: &'a mut ScrollView,
}

impl ScrollingSpace {
    pub fn new(options: LayoutOptions) -> Self {
        Self {
            columns: Vec::new(),
            column_widths: Vec::new(),
            options,
        }
    }

    /// This space as the window holding `view` sees it.
    pub fn through<'a>(&'a self, view: &'a ScrollView) -> ScrollingRef<'a> {
        ScrollingRef { space: self, view }
    }

    /// This space, moved by the window holding `view`.
    pub fn through_mut<'a>(&'a mut self, view: &'a mut ScrollView) -> ScrollingMut<'a> {
        ScrollingMut { space: self, view }
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// X position of each column (cumulative, starting at 0).
    fn column_xs(&self) -> impl Iterator<Item = f64> + '_ {
        let gaps = self.options.gaps;
        let mut x = 0.0;
        let widths = self
            .column_widths
            .iter()
            .copied()
            .chain(std::iter::once(0.0));
        widths.map(move |width| {
            let rv = x;
            x += width + gaps;
            rv
        })
    }

    /// X position of a specific column.
    pub fn column_x(&self, idx: usize) -> f64 {
        self.column_xs().nth(idx).unwrap_or(0.0)
    }

    /// Compute the Y offset of a pane within a column.
    pub fn pane_y_in_column(&self, col_idx: usize, pane_idx: usize) -> f64 {
        let col = &self.columns[col_idx];
        let gaps = self.options.gaps;
        let mut y = gaps;
        for i in 0..pane_idx.min(col.pane_sizes.len()) {
            y += col.pane_sizes[i].h + gaps;
        }
        y
    }

    /// A new column holding `pane`, at the layout's
    /// [`default_column_width`](LayoutOptions::default_column_width).
    pub fn new_column(&self, id: ColumnId, pane: Pane) -> Column {
        Column::new(id, pane, self.options.default_column_width)
    }

    /// Where pane `pane_id` sits: its column index and its row within that column.
    pub fn pane_indices(&self, pane_id: PaneId) -> Option<(usize, usize)> {
        self.columns.iter().enumerate().find_map(|(ci, col)| {
            col.panes
                .iter()
                .position(|p| p.id == pane_id)
                .map(|pi| (ci, pi))
        })
    }

    fn is_centering_focused_column(&self) -> bool {
        self.options.center_focused_column == CenterFocusedColumn::Always
            || (self.options.always_center_single_column && self.columns.len() <= 1)
    }

    fn capture_column_positions(&self) -> Vec<(ColumnId, f64)> {
        self.column_xs()
            .zip(self.columns.iter())
            .map(|(x, col)| (col.id, x))
            .collect()
    }

    /// Animate every column from its previous x (keyed by [`ColumnId`]) to its new
    /// laid-out x — shared by [`ScrollingMut::reorder_column`] and
    /// [`ScrollingMut::swap_columns`].
    fn animate_columns_from(&mut self, old_xs: &[(ColumnId, f64)]) {
        let new_xs: Vec<f64> = self.column_xs().collect();
        for (i, col) in self.columns.iter_mut().enumerate() {
            let old_x = old_xs
                .iter()
                .find(|(id, _)| *id == col.id)
                .map(|(_, x)| *x)
                .unwrap_or(new_xs[i]);
            let diff = old_x - new_xs[i];
            if diff.abs() > 0.5 {
                col.animate_move_from(diff, AnimationConfig::default());
            }
        }
    }

    /// Compute the insert position for a point in space coordinates.
    /// Used during interactive move to determine where to drop a pane.
    ///
    /// Algorithm (from NIRI's `scrolling.insert_position()`):
    /// 1. Transform to space coords and aim for center of gaps.
    /// 2. Find closest column gap vs closest tile gap.
    /// 3. Return whichever is closer.
    pub fn insert_position(&self, pos: Point) -> PaneInsertTarget {
        let gaps = self.options.gaps;
        // pos is already in space coordinates (caller adds view_pos).
        let x = pos.x + gaps / 2.0;
        let y = pos.y + gaps / 2.0;

        // Before first column → NewColumn(0)
        if x < 0.0 {
            return PaneInsertTarget::NewColumn(0);
        }

        // Find the column containing x.
        let mut col_idx = 0usize;
        let mut found_col = false;
        for (i, col_x) in self.column_xs().enumerate() {
            let col_w = self.column_widths.get(i).copied().unwrap_or(0.0);
            if x >= col_x && x < col_x + col_w {
                col_idx = i;
                found_col = true;
                break;
            }
        }

        // Past last column → NewColumn at end.
        if !found_col {
            return PaneInsertTarget::NewColumn(self.columns.len());
        }

        // Find closest column gap.
        let mut closest_col_gap_idx = 0usize;
        let mut closest_col_gap_dist = f64::MAX;
        for (i, col_x) in self.column_xs().enumerate() {
            let dist = (col_x - x).abs();
            if dist < closest_col_gap_dist {
                closest_col_gap_dist = dist;
                closest_col_gap_idx = i;
            }
            // Also check right edge of column (gap center after this column).
            let col_w = self.column_widths.get(i).copied().unwrap_or(0.0);
            let right_x = col_x + col_w + gaps;
            let right_dist = (right_x - x).abs();
            if right_dist < closest_col_gap_dist {
                closest_col_gap_dist = right_dist;
                closest_col_gap_idx = i + 1;
            }
        }

        // Find closest tile gap within the containing column.
        let col = &self.columns[col_idx];
        let _col_x = self.column_x(col_idx);
        let mut tile_y = gaps;
        let mut closest_tile_idx = 0usize;
        let mut closest_tile_gap_dist = f64::MAX;

        for (i, size) in col.pane_sizes.iter().enumerate() {
            let dist = (tile_y - y).abs();
            if dist < closest_tile_gap_dist {
                closest_tile_gap_dist = dist;
                closest_tile_idx = i;
            }
            tile_y += size.h + gaps;
        }
        // Check bottom edge.
        let bottom_dist = (tile_y - y).abs();
        if bottom_dist < closest_tile_gap_dist {
            closest_tile_gap_dist = bottom_dist;
            closest_tile_idx = col.pane_sizes.len();
        }

        // Compare distances: column gap vs tile gap.
        if closest_col_gap_dist <= closest_tile_gap_dist {
            PaneInsertTarget::NewColumn(closest_col_gap_idx.min(self.columns.len()))
        } else {
            PaneInsertTarget::InColumn {
                col_idx,
                pane_idx: closest_tile_idx.min(col.panes.len()),
            }
        }
    }
}

impl<'a> ScrollingRef<'a> {
    /// The box the columns are laid out in.
    pub fn area(&self) -> Rectangle {
        self.view.area
    }

    /// Index of the column the window has focused.
    pub fn active_column_idx(&self) -> usize {
        self.view.active_column
    }

    pub fn active_column(&self) -> Option<&'a Column> {
        self.space.columns.get(self.view.active_column)
    }

    pub fn active_pane(&self) -> Option<&'a Pane> {
        self.active_column().and_then(|c| c.active_pane())
    }

    /// Get all panes with their render positions.
    pub fn panes_with_positions(&self) -> Vec<(PaneId, Rectangle)> {
        self.laid_out_columns()
            .into_iter()
            .flat_map(|col| col.panes)
            .map(|p| (p.id, p.rect))
            .collect()
    }

    /// **Where each column is on screen**, with the panes inside it — one walk, so a caller asking
    /// about a column and a caller asking about a pane cannot disagree about where either sits.
    ///
    /// The column geometry was always computed here and then thrown away: only the pane rects came
    /// out, and anything wanting a column's box had to rebuild the same arithmetic from
    /// [`column_x`](Self::column_x), the render offset, the view offset and the gaps.
    pub fn columns_with_positions(&self) -> Vec<LaidOutColumn> {
        self.laid_out_columns()
    }

    /// The one walk both public views are built on.
    fn laid_out_columns(&self) -> Vec<LaidOutColumn> {
        let view_off = Point::new(-self.view_pos(), 0.0);
        let gaps = self.space.options.gaps;

        self.space.columns
            .iter()
            .enumerate()
            .map(|(col_idx, col)| {
                let col_pos = Point::new(self.space.column_x(col_idx) + col.render_offset(), 0.0);
                let mut pane_y = self.view.area.loc.y + gaps;
                let mut panes = Vec::with_capacity(col.panes.len());

                for (pane_idx, pane) in col.panes.iter().enumerate() {
                    let size = col.pane_sizes.get(pane_idx).copied().unwrap_or(Size::new(
                        col.computed_width,
                        self.view.area.size.h / col.panes.len().max(1) as f64,
                    ));

                    // **Flow, then transform.** The slot is where the column stacks this pane; the
                    // displacement is the pane's own, from a move animation or a drag in flight.
                    // They are returned apart because a container places its children by the first
                    // and the child carries the second — the same split CSS makes between layout
                    // and `transform`.
                    let pane_offset = pane.move_offset.current();
                    let rubber = pane.interactive_move_offset;
                    let slot = view_off + col_pos + Point::new(0.0, pane_y);
                    let displacement =
                        Point::new(pane_offset.x + rubber.x, pane_offset.y + rubber.y);
                    panes.push(LaidOutPane {
                        id: pane.id,
                        rect: Rectangle::new(slot + displacement, size),
                        slot: Rectangle::new(slot, size),
                        displacement,
                    });

                    pane_y += size.h + gaps;
                }

                LaidOutColumn {
                    id: col.id,
                    idx: col_idx,
                    rect: enclosing(&panes).unwrap_or_else(|| {
                        // An empty column has no panes to span, so it is its own width at the top
                        // of the working area — the box it would occupy the moment one arrives.
                        Rectangle::new(
                            view_off + col_pos + Point::new(0.0, self.view.area.loc.y + gaps),
                            Size::new(col.computed_width, 0.0),
                        )
                    }),
                    panes,
                }
            })
            .collect()
    }
}

impl std::ops::Deref for ScrollingRef<'_> {
    type Target = ScrollingSpace;

    fn deref(&self) -> &ScrollingSpace {
        self.space
    }
}

impl std::ops::Deref for ScrollingMut<'_> {
    type Target = ScrollingSpace;

    fn deref(&self) -> &ScrollingSpace {
        self.space
    }
}

impl ScrollingMut<'_> {
    /// The same space, for the reads that only look.
    pub fn reader(&self) -> ScrollingRef<'_> {
        ScrollingRef {
            space: self.space,
            view: self.view,
        }
    }

    /// Index of the column the window has focused.
    pub fn active_column_idx(&self) -> usize {
        self.view.active_column
    }

    /// The box the columns are laid out in.
    pub fn area(&self) -> Rectangle {
        self.view.area
    }

    /// Current view position (column_x + view_offset).
    pub fn view_pos(&self) -> f64 {
        self.reader().view_pos()
    }

    pub fn active_pane(&self) -> Option<&Pane> {
        self.space.columns.get(self.view.active_column)?.active_pane()
    }

    pub fn active_column_mut(&mut self) -> Option<&mut Column> {
        self.space.columns.get_mut(self.view.active_column)
    }
}

/// **One column as it is laid out on screen**, and the panes stacked inside it.
///
/// `rect` spans the **slots** it holds — a column is the box its contents occupy in the layout, so
/// a pane being dragged away does not stretch the column it is leaving.
#[derive(Clone, Debug, PartialEq)]
pub struct LaidOutColumn {
    /// The column's own identity — stable across a split, unlike its index.
    pub id: ColumnId,
    /// Its position in the strip, for the actions that still address a column positionally.
    pub idx: usize,
    /// The box it occupies, in the same space [`ScrollingSpace::panes_with_positions`] reports.
    pub rect: Rectangle,
    /// Its panes, top to bottom.
    pub panes: Vec<LaidOutPane>,
}

/// **One pane as it is laid out**, with its flow position and its own displacement kept apart.
///
/// `slot` is where its column stacks it; `displacement` is what a move animation or a drag in
/// flight has shifted it by; `rect` is the two together, which is where it is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaidOutPane {
    pub id: PaneId,
    /// Where it is drawn — `slot` plus `displacement`.
    pub rect: Rectangle,
    /// Where its column stacks it, before any displacement of its own.
    pub slot: Rectangle,
    /// Its own offset, from a move animation or a drag in flight.
    pub displacement: Point,
}

/// The smallest box containing every pane's **slot** — `None` when there are none.
fn enclosing(panes: &[LaidOutPane]) -> Option<Rectangle> {
    let mut it = panes.iter().map(|p| p.slot);
    let first = it.next()?;
    Some(it.fold(first, |acc, r| {
        let x = acc.loc.x.min(r.loc.x);
        let y = acc.loc.y.min(r.loc.y);
        let right = (acc.loc.x + acc.size.w).max(r.loc.x + r.size.w);
        let bottom = (acc.loc.y + acc.size.h).max(r.loc.y + r.size.h);
        Rectangle::new(Point::new(x, y), Size::new(right - x, bottom - y))
    }))
}

#[cfg(test)]
mod tests;

mod columns;
mod resize;
mod view;
