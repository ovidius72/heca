//! The **whole workspaces dock** (F006/P032/T429).
//!
//! It owns the workspaces' vertical order, this placement's scroll area and keyboard gate, and the
//! menu a right-click finds when no row claims it. It owns **nothing** about what is inside a
//! workspace — that is [`WorkspaceFrame`](super::workspace_frame::WorkspaceFrame).
//!
//! This is the body of the `workspaces` container: what `build_body`, the provider's render seam,
//! returns. It is reached only through that seam — nothing in `chrome` calls it.

use super::seams::{DockRegistries, DockSeams};
use super::{container_items, workspace_frame::WorkspaceFrame, WorkspaceTree, MENU_CONTAINER};
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate};
use heca_grid_ui::widgets::{Flex, ScrollRegion};

/// The workspace tree mounted inside the sidebar shell — one frame per workspace, in a scroll area
/// of this placement's own.
pub(crate) struct DockView<'a> {
    /// The session reduced to plain data by `model.rs`.
    pub(crate) tree: &'a WorkspaceTree,
    /// This placement's own scroll offset, keyed by mount id, so the same container placed twice
    /// keeps two positions.
    pub(crate) scroll: Signal<f32>,
    /// Whether this placement holds chrome keyboard focus (F003/P011/T020).
    pub(crate) focused: Signal<bool>,
}

impl DockView<'_> {
    /// Build the dock, returning the concrete widget so the region still sizes it.
    pub(crate) fn build(self, seams: &DockSeams<'_>, reg: &mut DockRegistries<'_>) -> ScrollRegion {
        let DockView {
            tree,
            scroll,
            focused,
        } = self;
        let mut col = Flex::column().gap(6.0).grow(1.0);
        for workspace in &tree.workspaces {
            col = col.child(WorkspaceFrame { workspace }.build(seams, reg));
        }
        // The container nests its **own** scroll area, so its workspace list scrolls inside the slot
        // the region gave it (F003/P011/T021). A scroll area is just a container, so this is
        // composition rather than a capability the shell has to hand down: the shell scrolls its dock
        // list, this scrolls its content, and nesting decides which one a wheel or a keyboard page
        // reaches — the innermost that can move on that axis wins, and the outer one only sees what
        // the inner declines.
        //
        // The offset is the CONTAINER's, so it survives the tree being rebuilt (a pane's git status
        // changing is enough to do that) and is untouched by the dock list scrolling around it.
        // Vertical only. A second axis was tried (2026-07-29) and reverted: a `Label` clips rather than
        // overflowing, so nothing ever exceeds the width and the horizontal axis had nothing to scroll —
        // while the extra scrollbar sat in front of the rows and took the press that starts a drag.
        // Revisit when label truncation/wrapping is settled (AGENTS.md lists it as open).
        let mut region = ScrollRegion::new().grow(1.0);
        // Restore through `scroll_to`, which reports with `event: None`, so the listener below can tell
        // a restore from the user actually scrolling and never writes one back as the other.
        region.scroll_to(scroll.get_untracked());
        region
            // Keyboard scroll intents act on the area the keyboard is aimed at, and every other area
            // declines them — which is what makes the semantic `WidgetIntent::Scroll*` a *broadcast* the
            // focused container answers rather than something the host has to route by hand
            // (F003/P011/T012 consumes this; the gate itself belongs here, per placement).
            .keyboard_target(focused)
            .on_scroll(move |s| {
                if s.event.is_some() {
                    scroll.set(s.offset_y);
                }
            })
            // **The container's own menu — what a right-click on empty space finds.**
            //
            // No empty-space hit test, and no "did I miss a row?" branch anywhere: a right-click that
            // lands between rows, below the last one, or on the region's padding simply finds no menu
            // on the way down and bubbles out to this one. A menu on a row still wins, because bubbling
            // stops at the nearest declaration.
            .context_menu({
                let menu = crate::chrome::context_menu::menu_from_items(
                    "Workspaces",
                    "What you can do here",
                    MENU_CONTAINER,
                    container_items(),
                    seams.catalog,
                    seams.emit,
                );
                heca_grid_ui::widgets::ContextMenu::new(MENU_CONTAINER).child(menu)
            })
            .child(col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::testing::{self, Fixture};

    /// One frame per workspace, in the tree's own order.
    #[test]
    fn the_dock_holds_one_frame_per_workspace() {
        let mut fx = Fixture::default();
        let mut tree = testing::tree();
        let mut second = tree.workspaces[0].clone();
        second.ws_idx = 1;
        // Its own identity, not just its own position — a clone that keeps the original's id is two
        // rows answering to one name, which is what the keys are built from.
        second.ws_id = heca_core::layout::WorkspaceId(1);
        second.columns[0].col_id = heca_core::layout::ColumnId(1);
        second.name = "ws2".into();
        tree.workspaces.push(second);

        let root = {
            let (seams, mut reg) = fx.split("left");
            DockView {
                tree: &tree,
                scroll: heca_grid_ui::reactive::signal(0.0),
                focused: heca_grid_ui::reactive::signal(false),
            }
            .build(&seams, &mut reg)
        };

        let declared = testing::declared_keys(&root);
        for ws_idx in [0, 1] {
            assert!(
                declared.contains(&super::super::workspace_key(heca_core::layout::WorkspaceId(ws_idx as u64))),
                "workspace {ws_idx} has no frame: {declared:?}",
            );
        }
    }
}
