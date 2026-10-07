//! **The places a carried pane can start a new column**, worked out from the rects the layout
//! engine gave — one at each end and one in every gap between two columns.

use heca_core::layout::{Point, Rectangle, Size};

use super::WorkspaceModel;

/// Every slot, by the number of columns to its left, with the window rect it fills.
///
/// A slot is as wide as `slot_share` of the column beside it: wide enough to see and to aim at.
/// It is centred on its gap, and kept inside the content area at the two ends, where half of it
/// would otherwise be clipped away.
pub(super) fn slots(model: &WorkspaceModel) -> Vec<(usize, Rectangle)> {
    let area = model.area;
    let (left, right) = (area.loc.x, area.loc.x + area.size.w);
    let columns = &model.columns;
    let rect = |at: usize, centre: f64, beside: usize| {
        let column = &columns[beside];
        let w = f64::from(column.w * model.slot_share).min(area.size.w);
        let x = (centre - w / 2.0).clamp(left, (right - w).max(left));
        (
            at,
            Rectangle::new(
                Point::new(x, f64::from(column.y)),
                Size::new(w, f64::from(column.h)),
            ),
        )
    };
    if columns.is_empty() {
        return Vec::new();
    }
    let mut out = vec![rect(0, f64::from(columns[0].x), 0)];
    for (at, pair) in columns.windows(2).enumerate() {
        let gap = f64::from((pair[0].x + pair[0].w + pair[1].x) / 2.0);
        out.push(rect(at + 1, gap, at));
    }
    let last = columns.len() - 1;
    out.push(rect(
        columns.len(),
        f64::from(columns[last].x + columns[last].w),
        last,
    ));
    out
}
