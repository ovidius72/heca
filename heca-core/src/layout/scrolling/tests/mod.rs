//! Tests for [`super`], split by what each group tests.

use super::*;
use crate::layout::testing::Seen;


pub(super) fn test_scrolling_space() -> Seen {
    Seen::new(
        Rectangle::from_size(Size::new(1000.0, 800.0)),
        1.0,
        LayoutOptions::default(),
    )
}


pub(super) fn test_column(id: u64, width: ColumnWidth) -> Column {
    Column::new(
        ColumnId(id),
        Pane::new(PaneId(id), format!("Pane {id}")),
        width,
    )
}


/// A space with columns whose ids are `1..=n`, in order.
pub(super) fn space_with_columns(n: u64) -> Seen {
    let mut space = test_scrolling_space();
    // activate=true appends in order (activate=false inserts at active+1).
    for id in 1..=n {
        space.m().add_column(None, test_column(id, ColumnWidth::Proportion(0.5)), true);
    }
    space
}


pub(super) fn column_ids(space: &ScrollingSpace) -> Vec<u64> {
    space.columns.iter().map(|c| c.id.0).collect()
}

mod columns;
mod laid_out;
mod pane_effect;
mod pane_react;
mod panes;
mod view;
