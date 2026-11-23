//! One **workspace** of the dock.
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
    ChromeDragItem, MENU_WORKSPACE, WorkspaceEntry, column_group::ColumnGroup, pane_row::PaneRow,
    row_hint, workspace_key, workspace_row_items,
};
use heca_grid_ui::Tone;
use heca_grid_ui::builders::{DragExt, HintExt, IdentityExt, LayoutExt, MenuExt, Parent};
use heca_grid_ui::style::Spacing;
use heca_grid_ui::widgets::{Badge, DockFrame, Flex, HintPlacement};

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
    ) -> heca_grid_ui::widgets::DockFrame {
        let ws = self.workspace;
        let theme = seams.theme;
        let ws_idx = ws.ws_idx;
        // Its identity, for keys; `ws_idx` stays for the actions, which act on a position.
        let ws_id = ws.ws_id;
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
        let mut dock = DockFrame::new(ws.name.clone())
            // **What the fold control's letter means** — this is the file that assigns the
            // vocabulary: a workspace's own letter is `warning` a few lines down, a pane's is the
            // accent, a column's is `success`. Folding is a structural control rather than
            // somewhere to navigate, so it reads `muted`. The widget places the letter (only it
            // knows where its chevron is) and says nothing about what it means.
            .fold_hint_tone(Tone::Muted)
            // A row of the dock's list, smaller than the dock's own header: the title row only, so
            // the panes inside keep their size.
            .title_size(heca_grid_ui::style::WidgetSize::Caption)
            .frameless(true)
            .gap(Spacing::Xs) // tighten the workspace header → body spacing
            .expanded(!seams.ws_state.is_collapsed(ws_id))
            // Folding names its action; the dock's listener sends it seated on this dock.
            .on_toggle(move |_| {
                heca_grid_ui::intent::send(
                    crate::chrome::Intent::new("toggle_workspace_collapsed")
                        .arg("ws_idx", crate::chrome::PropValue::Int(ws_idx as i64)),
                );
            })
            .header_actions(badge);
        // Keep the active identity live. The theme may enable a workspace-wide wash through
        // `active_wash_alpha` (off by default); the count badge also identifies this workspace.
        dock = dock.active(active_ws);
        dock = dock.nav_selected(false);
        // **The row's one identity, declared like every other row's.** A pane card and a column
        // both call `key`, and `key_at` — which is how a right-click finds out what it
        // landed on — reads exactly that. The workspace header pushed its key into the signal list
        // and never told the widget, so the hit-test found nothing there: right-clicking a pane or
        // a column opened its menu, a workspace opened none.
        dock = dock.key(workspace_key(ws_id));
        // The workspace row's own menu, declared like the other two. The bug in the comment above
        // is the reason this phase exists: a menu resolved from a *position* needs a declaration
        // nobody remembers to write, and this one is the declaration itself.
        dock = dock.context_menu({
            let menu = crate::chrome::surfaces::context_menu::menu_from_items(
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
        // **The frame follows focus itself**: active while it holds the active pane, "previous" while
        // it is the workspace back-and-forth would return to — both read from the store's
        // signals, so nothing is handed to the host to write every frame.
        {
            let store = seams.ws_state.clone();
            let (active_sig, previous_sig) = (dock.active_state(), dock.previous_state());
            heca_grid_ui::reactive::create_effect(move |_| {
                let active = store.active_pane();
                super::pane_row::set_if_changed(
                    active_sig,
                    active.is_some_and(|a| ws_pane_ids.contains(&a)),
                );
                super::pane_row::set_if_changed(previous_sig, store.previous_ws() == Some(ws_id));
            });
        }
        // The whole workspace is a column drop target (F4.5 step 2 scope C): dropping a
        // column anywhere on it that isn't a deeper column/pane target moves the column
        // into this workspace. Innermost-first hit-testing lets columns/panes override.
        reg.drag.register(
            workspace_key(ws_id),
            ChromeDragItem::Workspace { ws: ws_idx },
        );
        // A workspace takes either: a pane dropped on its header or empty area moves in — the only
        // way into an empty one — and a column moves to its end. **And it is carried by its grip
        // and lands beside another workspace** — a reorder, so before or after, never onto
        // (`providers/workspaces/drop.rs` turns it into `move_workspace`).
        dock = dock
            .accepts(["pane", "column"])
            .draggable_as(crate::chrome::WORKSPACE_DRAG_KIND)
            .accepts_beside([crate::chrome::WORKSPACE_DRAG_KIND]);
        // Columns stacked with a clear gap between them (the gap + bar mark each column);
        // panes inside a column are tight. Floating panes have no column.
        let mut cols = Flex::column().gap(Spacing::Sm);
        for column in &ws.columns {
            cols = cols.child(ColumnGroup { column, ws_idx }.build(seams, reg));
        }
        for float in &ws.floating_panes {
            cols = cols.child(PaneRow::new(float).build(seams, reg));
        }
        // **The dock says all of this about itself** — no wrapper.
        //
        // `KeyHint` is for a region that is not a widget you can put a builder on; a `DockFrame`
        // is one. Its `paint` only paints its child, because the letter is drawn by `paint_child`
        // for any widget carrying one — so wrapping bought a node in the tree and nothing else.
        //
        // The tone says what the target IS and the theme colours it: a workspace reads distinctly
        // from a pane, without this file naming a colour. Top-right like the pane cards, nudged
        // down onto the title row so it lines up with the name.
        dock.child(cols)
            .on_hint(seams.picks(row_hint(workspace_key(ws_id))))
            .hint_tone(Tone::Warning)
            .hint_placement(HintPlacement::TopRight)
            .hint_offset_y((theme.font_size * 0.45) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::{self, Fixture};
    use super::*;
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
            workspace_key(heca_core::layout::WorkspaceId(0)),
            super::super::column_key(heca_core::layout::ColumnId(0)),
            super::super::pane_key(PaneId(1)),
            super::super::pane_key(PaneId(9)),
        ] {
            assert!(
                declared.contains(&expected),
                "{expected:?} missing from the frame: {declared:?}",
            );
        }
    }

    /// The frame's own two states follow focus from the store, for the same reason a row's do:
    /// change them after the frame is built and it follows, with nothing handed to the host.
    #[test]
    fn a_frame_follows_both_of_its_states() {
        use heca_grid_ui::reactive::SignalGet;
        let mut fx = Fixture::default();
        let tree = testing::tree();
        let frame = {
            let (seams, mut reg) = fx.split("left");
            WorkspaceFrame {
                workspace: &tree.workspaces[0],
            }
            .build(&seams, &mut reg)
        };
        let store = fx.store.workspaces.clone();
        store.set_active_pane(Some(PaneId(1)));
        assert!(
            frame.active_state().get_untracked(),
            "the workspace you are in"
        );
        store.set_active_pane(Some(PaneId(99)));
        assert!(
            !frame.active_state().get_untracked(),
            "and not once focus leaves it"
        );
        store.set_previous_ws(Some(tree.workspaces[0].ws_id));
        assert!(
            frame.previous_state().get_untracked(),
            "and the one prefix+Shift+i would return to",
        );
    }
}
