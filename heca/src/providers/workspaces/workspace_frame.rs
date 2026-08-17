//! One **workspace** of the dock (F006/P032/T429).
//!
//! It owns the frame: the header and its count badge, the collapse toggle, the workspace's own
//! states, its menu, its drop target and its pick letter — and the order its children go in,
//! columns then floats. It owns **nothing** about what a column or a pane row looks like.
//!
//! A floating pane is a [`PaneRow`] with no column rather than a second row shape: it is in no
//! column, so its menu simply lacks the entries that need one. That is the whole difference, and
//! writing it as one line here is what stops a near-duplicate growing beside `ColumnGroup` — which
//! is exactly what `column_view()` had become.

use super::seams::{DockRegistries, DockSeams};
use super::{
    column_group::ColumnGroup, pane_row::PaneRow, row_hint, workspace_key, workspace_row_items,
    ChromeDragItem, WorkspaceEntry, MENU_WORKSPACE,
};
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent};
use heca_grid_ui::style::{Align, Length};
use heca_grid_ui::widgets::{Badge, DockFrame, Flex, HintPlacement, KeyHint};

/// A `.frameless(true)` [`DockFrame`] whose header carries the workspace's total pane count, and
/// whose body is the workspace's columns followed by its floating panes.
pub(crate) struct WorkspaceFrame<'a> {
    /// The workspace this frame draws, already reduced to plain data by `model.rs`.
    pub(crate) workspace: &'a WorkspaceEntry,
}

impl WorkspaceFrame<'_> {
    /// Build the frame, returning the concrete widget so the parent still sizes it.
    pub(crate) fn build(
        self,
        seams: &DockSeams<'_>,
        reg: &mut DockRegistries<'_>,
    ) -> impl heca_grid_ui::Component + use<> {
        let ws = self.workspace;
        let theme = seams.theme;
        let ws_idx = ws.ws_idx;
        let pane_count =
            ws.columns.iter().map(|c| c.panes.len()).sum::<usize>() + ws.floating_panes.len();
        // A workspace is "active" iff it hosts the active pane.
        let active_ws = seams.active_pane.is_some_and(|pid| {
            ws.columns
                .iter()
                .flat_map(|c| &c.panes)
                .chain(&ws.floating_panes)
                .any(|p| p.pane_id == pid)
        });
        let badge = if active_ws {
            Badge::accent(pane_count.to_string())
        } else {
            Badge::neutral(pane_count.to_string())
        };
        // The header toggle records the workspace in the toggle sink; the app flips
        // its collapsed state (canonical, in `chrome_state.workspaces`) and the tree
        // rebuilds (F4.3). Collapse is read back from that same shared state.
        let emit = seams.emit.clone();
        let mut dock = DockFrame::new(ws.name.clone())
            .frameless(true)
            .gap(4.0) // tighten the workspace header → body spacing
            .expanded(!seams.ws_state.is_ws_collapsed(ws_idx))
            .on_toggle(move |_| {
                emit(crate::app::interaction::InteractionIntent::ToggleWorkspaceCollapsed { ws_idx });
            })
            .header(
                Flex::row()
                    .align(Align::Center)
                    .child(badge)
                    .child(Flex::row().width(Length::Px(6.0))),
            );
        // Light accent wash over the whole active workspace area (+ the accent count badge).
        // Signal-driven like every other state here, so it flips in place via
        // `sync_chrome_signals` instead of forcing a tree rebuild; the alpha is the theme's
        // `active_wash_alpha` token, not a baked-in literal.
        dock = dock.active(active_ws);
        dock = dock.nav_selected(false);
        // **The row's one identity, declared like every other row's.** A pane card and a column
        // both call `key`, and `key_at` — which is how a right-click finds out what it
        // landed on — reads exactly that. The workspace header pushed its key into the signal list
        // and never told the widget, so the hit-test found nothing there: right-clicking a pane or
        // a column opened its menu, a workspace opened none (Antonio, 2026-08-05).
        dock = dock.key(workspace_key(ws_idx));
        // The workspace row's own menu, declared like the other two. The bug in the comment above
        // is the reason this phase exists: a menu resolved from a *position* needs a declaration
        // nobody remembers to write, and this one is the declaration itself.
        dock = dock.context_menu({
            let menu = crate::chrome::context_menu::menu_from_items(
                "Workspace",
                "What you can do with this workspace",
                MENU_WORKSPACE,
                workspace_row_items(ws_idx, ws.custom_name.is_some()),
                seams.catalog,
                seams.emit,
            );
            heca_grid_ui::widgets::ContextMenu::new(MENU_WORKSPACE).child(menu)
        });
        let ws_pane_ids = ws
            .columns
            .iter()
            .flat_map(|c| &c.panes)
            .chain(&ws.floating_panes)
            .map(|p| p.pane_id)
            .collect::<Vec<_>>();
        reg.signals.ws_active.push((ws_pane_ids, dock.active_state()));
        reg.signals.ws_previous.push((ws_idx, dock.previous_state()));
        reg.signals.row_nav.push((
            seams.mount.to_string(),
            workspace_key(ws_idx),
            dock.nav_state(),
        ));
        // The whole workspace is a column drop target (F4.5 step 2 scope C): dropping a
        // column anywhere on it that isn't a deeper column/pane target moves the column
        // into this workspace. Innermost-first hit-testing lets columns/panes override.
        dock = dock.drop_target(reg.drag.register(ChromeDragItem::Workspace { ws: ws_idx }));
        // Columns stacked with a clear gap between them (the gap + bar mark each column);
        // panes inside a column are tight. Floating panes have no column.
        let mut cols = Flex::column().gap(8.0);
        for column in &ws.columns {
            cols = cols.child(ColumnGroup { column, ws_idx }.build(seams, reg));
        }
        for float in &ws.floating_panes {
            cols = cols.child(
                PaneRow {
                    pane: float,
                    column_of_pane: None,
                }
                .build(seams, reg),
            );
        }
        dock = dock.child(cols);
        // Wrap the whole workspace dock in the universal `KeyHint` so a
        // "move column/pane → workspace" pick can stamp this workspace's letter over
        // it. The keycap is tinted `warning` (not accent) so a workspace target reads
        // distinctly from a pane target. The hint signal is driven each frame in
        // `sync_chrome_signals` from the active pick candidates.
        KeyHint::new(dock)
            // **What `prefix+/` does to this row** — the cursor lands on the workspace header
            // and the dock keeps the keyboard, exactly as it does on a pane row.
            .on_hint(crate::chrome::fires(
                seams.mount,
                row_hint(workspace_key(ws_idx)),
                seams.emit,
            ))
            .color(theme.colors.warning)
            // Top-right (like the pane cards' right-aligned keycap), nudged down
            // onto the workspace title row so it lines up with the name.
            .placement(HintPlacement::TopRight)
            .offset_y((theme.font_size * 0.45) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::testing::{self, Fixture};
    use heca_core::layout::PaneId;

    /// A floating pane is a row in the frame's body beside the columns — **not** a second row
    /// shape. If this ever needs its own component, the difference has grown into something real
    /// and should be named; today it is one `None`.
    #[test]
    fn a_frame_holds_its_columns_then_its_floats() {
        let mut fx = Fixture::default();
        let mut tree = testing::tree();
        tree.workspaces[0]
            .floating_panes
            .push(testing::pane(PaneId(9), "float"));
        let frame = {
            let (seams, mut reg) = fx.split("left");
            WorkspaceFrame {
                workspace: &tree.workspaces[0],
            }
            .build(&seams, &mut reg)
        };

        let declared = testing::declared_keys(&frame);
        for expected in [
            workspace_key(0),
            super::super::column_key(0, 0),
            super::super::pane_key(PaneId(1)),
            super::super::pane_key(PaneId(9)),
        ] {
            assert!(
                declared.contains(&expected),
                "{expected:?} missing from the frame: {declared:?}",
            );
        }
    }

    /// The frame's own two states are signals, for the same reason a row's are.
    #[test]
    fn a_frame_publishes_both_of_its_states() {
        let mut fx = Fixture::default();
        let tree = testing::tree();
        {
            let (seams, mut reg) = fx.split("left");
            let _ = WorkspaceFrame {
                workspace: &tree.workspaces[0],
            }
            .build(&seams, &mut reg);
        }

        assert!(
            !fx.signals.ws_active.is_empty(),
            "the workspace you are in",
        );
        assert!(
            fx.signals.ws_previous.iter().any(|(idx, _)| *idx == 0),
            "and the one prefix+Shift+i would return to",
        );
    }
}
