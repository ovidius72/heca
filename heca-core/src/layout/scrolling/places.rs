//! **The places a pane can be put** — read from where the columns are laid out in one window.

use super::ScrollingRef;
use crate::layout::types::{Point, Rectangle, Size};

/// What a [`Place`] is a place for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceKind {
    /// A new column at gap `i` (the gap left of column `i`; the last is the end).
    Gap(usize),
    /// **The border** above pane `row` of column `col`, or below the last when `row` is the pane
    /// count — a line of no thickness, so the place takes no room.
    Row { col: usize, row: usize },
}

/// One open place and the box it fills.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub kind: PlaceKind,
    pub rect: Rectangle,
}

impl ScrollingRef<'_> {
    /// **The places a pane can be put** — one per gap between columns and at each end, and the
    /// border above, between and below the panes of every column. Worked out from the same layout
    /// as the columns, so none of them overlaps a pane or a column.
    pub fn places(&self) -> Vec<Place> {
        let columns = self.laid_out_columns();
        let gaps = self.space.options.gaps;
        let area = self.view.area;
        let top = area.loc.y + gaps;
        let height = (area.size.h - 2.0 * gaps).max(0.0);
        let mut out = Vec::new();
        // The borders between columns, drawn like the ones between panes: a standing line in the
        // middle of the gap before each column, and one after the last. It takes no room.
        let upright = |x: f64| Rectangle::new(Point::new(x, top), Size::new(0.0, height));
        for (i, col) in columns.iter().enumerate() {
            out.push(Place { kind: PlaceKind::Gap(i), rect: upright(col.rect.loc.x - gaps / 2.0) });
        }
        if let Some(last) = columns.last() {
            out.push(Place {
                kind: PlaceKind::Gap(columns.len()),
                rect: upright(last.rect.loc.x + last.rect.size.w + gaps / 2.0),
            });
        }
        // The borders: a line along the top of each column, between each two stacked panes (in the
        // middle of whatever gap there is, or exactly on the shared edge when there is none) and
        // along the bottom. A line has no thickness here — it takes no room, so what is drawn on
        // it and how far from it a pointer still counts is the caller's.
        for col in &columns {
            let (Some(first), Some(last)) = (col.panes.first(), col.panes.last()) else {
                continue;
            };
            let (x, width) = (col.rect.loc.x, col.rect.size.w);
            let line = |y: f64| Rectangle::new(Point::new(x, y), Size::new(width, 0.0));
            out.push(Place { kind: PlaceKind::Row { col: col.idx, row: 0 }, rect: line(first.slot.loc.y) });
            for (row, pair) in col.panes.windows(2).enumerate() {
                let (above, below) = (pair[0].slot, pair[1].slot);
                out.push(Place {
                    kind: PlaceKind::Row { col: col.idx, row: row + 1 },
                    rect: line((above.loc.y + above.size.h + below.loc.y) / 2.0),
                });
            }
            out.push(Place {
                kind: PlaceKind::Row { col: col.idx, row: col.panes.len() },
                rect: line(last.slot.loc.y + last.slot.size.h),
            });
        }
        out
    }
}
