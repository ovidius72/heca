//! One **column** of the workspaces dock (F006/P032/T429).
//!
//! It owns the panes' vertical stacking and the column's own identity — its marker bar, its menu,
//! its drag id, its pick letter. It owns **nothing** about its own width, and nothing about what a
//! pane row looks like: that is [`PaneRow`](super::pane_row::PaneRow), which it composes.
//!
//! The grip gutter is the only surface a child card does not cover, so innermost-first hit-testing
//! routes a grip press to the column and a card press to the pane for free — no geometry here.

use super::seams::{DockRegistries, DockSeams};
use super::{column_key, column_row_items, pane_row::PaneRow, ColumnEntry, MENU_COLUMN};
use crate::chrome::{ChromeDragItem, RepaintWatch};
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent};
use heca_grid_ui::widgets::{HintPlacement, KeyHint, MarkerGroup};

/// A generic [`MarkerGroup`] (left marker bar + grip gutter) holding the column's stacked pane
/// cards — no per-column header row, because columns are spatial groupings whose only user-facing
/// job is to be a move/swap target and a drag handle.
///
/// The bar brightens to the accent when the column holds the active pane.
pub(crate) struct ColumnGroup<'a> {
    /// The column this group draws, already reduced to plain data by `model.rs`.
    pub(crate) column: &'a ColumnEntry,
    /// Which workspace it belongs to — half of a column's positional identity.
    pub(crate) ws_idx: usize,
}

impl ColumnGroup<'_> {
    /// Build the column, returning the concrete widget so the parent still sizes it.
    pub(crate) fn build(self, seams: &DockSeams<'_>, reg: &mut DockRegistries<'_>) -> RepaintWatch {
        let ColumnGroup { column, ws_idx } = self;
        let active = column
            .panes
            .iter()
            .any(|p| seams.active_pane == Some(p.pane_id));
        // The MarkerGroup is a column drag source + drop target, and what it drags is the name it
        // declares below — the same one its cursor and its right-click read.
        reg.drag.register(
            column_key(column.col_id),
            ChromeDragItem::Column {
                ws: ws_idx,
                col: column.col_idx,
            },
        );
        let mut col = MarkerGroup::new()
            .active(active)
            .gap(3.0)
            .key(column_key(column.col_id))
            .context_menu({
                let menu = crate::chrome::context_menu::menu_from_items(
                    "Column",
                    "What you can do with this column",
                    MENU_COLUMN,
                    column_row_items(ws_idx, column.col_idx),
                    seams.catalog,
                    seams.emit,
                );
                heca_grid_ui::widgets::ContextMenu::new(MENU_COLUMN).child(menu)
            })
            .draggable()
            .drop_target();
        for pane in &column.panes {
            col = col.child(
                PaneRow {
                    pane,
                    column_of_pane: Some((ws_idx, column.col_idx)),
                }
                .build(seams, reg),
            );
        }
        // Bind the column bar's active signal (lit iff it holds the active pane).
        let pane_ids = column.panes.iter().map(|p| p.pane_id).collect::<Vec<_>>();
        reg.signals.col_active.push((pane_ids, col.state()));
        reg.signals.row_nav.push((
            seams.mount.to_string(),
            column_key(column.col_id),
            col.nav_state(),
        ));
        // Wrap the column in the universal `KeyHint` so a "move pane → column" pick can stamp this
        // column's letter over it (tinted `success`, distinct from pane/workspace picks). **The
        // letter is offered by key** (`chrome::hint`) and drawn by the widget — nothing is
        // registered here and nothing is projected onto it every frame.
        let hinted = KeyHint::new(col)
            .color(seams.theme.colors.success)
            .placement(HintPlacement::CenterRight);
        let (watch, _repaint) = RepaintWatch::new(hinted);
        watch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::testing::{self, Fixture};
    use heca_core::layout::PaneId;

    /// A column holds one row per pane and declares its own identity beside them, so a grip press
    /// finds the column and a card press finds the pane.
    #[test]
    fn a_column_holds_its_panes_and_declares_its_own_identity() {
        let mut fx = Fixture::default();
        let column = testing::column();
        let group = {
            let (seams, mut reg) = fx.split("left");
            ColumnGroup {
                column: &column,
                ws_idx: 0,
            }
            .build(&seams, &mut reg)
        };

        let declared = testing::declared_keys(&group);
        assert!(
            declared.contains(&column_key(heca_core::layout::ColumnId(0))),
            "the column names itself: {declared:?}",
        );
        assert!(
            declared.contains(&super::super::pane_key(PaneId(1))),
            "and its pane still names itself inside it: {declared:?}",
        );
    }

    /// The bar lights from a signal, not from a bool baked in at build time — the column's half of
    /// the rule in [`PaneRow`](super::super::pane_row)'s module note.
    #[test]
    fn the_columns_bar_follows_focus_through_a_signal() {
        let mut fx = Fixture::default();
        let column = testing::column();
        {
            let (seams, mut reg) = fx.split("left");
            let _ = ColumnGroup {
                column: &column,
                ws_idx: 0,
            }
            .build(&seams, &mut reg);
        }

        assert!(
            fx.signals
                .col_active
                .iter()
                .any(|(panes, _)| panes.contains(&PaneId(1))),
            "the bar is lit by whichever of its panes is active",
        );
    }
}
