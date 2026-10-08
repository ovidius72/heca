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

/// One open place: the line it draws, and the area a drop on it counts in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub kind: PlaceKind,
    /// The line, of no thickness: where the place is drawn.
    pub rect: Rectangle,
    /// Where a drop counts as landing on this place. It holds the line; at the edges of the strip
    /// it also takes the room beside it — the empty space before the first column and after the
    /// last, and above and below each column out to the edge of the area and a share into the
    /// pane next to it.
    pub zone: Rectangle,
}

impl Place {
    /// The same place moved by `by` — its line and its drop area together, so the two can never
    /// end up in different places.
    pub fn moved_by(self, by: Point) -> Self {
        let shift = |r: Rectangle| Rectangle::new(Point::new(r.loc.x + by.x, r.loc.y + by.y), r.size);
        Self { kind: self.kind, rect: shift(self.rect), zone: shift(self.zone) }
    }

    /// Whether the line stands up (between columns) rather than lies down (between panes).
    pub fn upright(&self) -> bool {
        matches!(self.kind, PlaceKind::Gap(_))
    }

    /// Where the line sits across [`zone`](Self::zone), as a share of it: `0` at its start, `1` at
    /// its end. The middle when the zone is the line itself.
    pub fn line_share(&self) -> f64 {
        let (line, start, span) = match self.upright() {
            true => (self.rect.loc.x, self.zone.loc.x, self.zone.size.w),
            false => (self.rect.loc.y, self.zone.loc.y, self.zone.size.h),
        };
        match span > 0.0 {
            true => ((line - start) / span).clamp(0.0, 1.0),
            false => 0.5,
        }
    }
}

/// A place whose drop area is the line itself.
fn on_line(kind: PlaceKind, rect: Rectangle) -> Place {
    Place { kind, rect, zone: rect }
}

/// The smallest box holding `a` and `b`.
fn spanning(a: Rectangle, b: Rectangle) -> Rectangle {
    let (x, y) = (a.loc.x.min(b.loc.x), a.loc.y.min(b.loc.y));
    let right = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
    let bottom = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
    Rectangle::new(Point::new(x, y), Size::new(right - x, bottom - y))
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
        let (left, right) = (area.loc.x, area.loc.x + area.size.w);
        for (i, col) in columns.iter().enumerate() {
            let line = upright(col.rect.loc.x - gaps / 2.0);
            // The gap before the first column also takes the empty space to its left.
            let zone = match i {
                0 => spanning(line, upright(left.min(line.loc.x))),
                _ => line,
            };
            out.push(Place { kind: PlaceKind::Gap(i), rect: line, zone });
        }
        if let Some(last) = columns.last() {
            let line = upright(last.rect.loc.x + last.rect.size.w + gaps / 2.0);
            // …and the gap after the last, the empty space to its right.
            let zone = spanning(line, upright(right.max(line.loc.x)));
            out.push(Place { kind: PlaceKind::Gap(columns.len()), rect: line, zone });
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
            // The top border reaches up to the edge of the area and a share into the first pane;
            // the bottom one down to the edge and a share into the last.
            let top_line = line(first.slot.loc.y);
            let into_first = first.slot.size.h * self.space.options.drop_edge_reach;
            out.push(Place {
                kind: PlaceKind::Row { col: col.idx, row: 0 },
                rect: top_line,
                zone: spanning(line(area.loc.y.min(top_line.loc.y)), line(top_line.loc.y + into_first)),
            });
            for (row, pair) in col.panes.windows(2).enumerate() {
                let (above, below) = (pair[0].slot, pair[1].slot);
                out.push(on_line(
                    PlaceKind::Row { col: col.idx, row: row + 1 },
                    line((above.loc.y + above.size.h + below.loc.y) / 2.0),
                ));
            }
            let bottom_line = line(last.slot.loc.y + last.slot.size.h);
            let into_last = last.slot.size.h * self.space.options.drop_edge_reach;
            let area_bottom = area.loc.y + area.size.h;
            out.push(Place {
                kind: PlaceKind::Row { col: col.idx, row: col.panes.len() },
                rect: bottom_line,
                zone: spanning(line(bottom_line.loc.y - into_last), line(area_bottom.max(bottom_line.loc.y))),
            });
        }
        out
    }
}
