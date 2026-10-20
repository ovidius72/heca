//! The **whole workspaces dock**.
//!
//! It owns the workspaces' vertical order, this placement's scroll area and keyboard gate, and the
//! menu a right-click finds when no row claims it. It owns **nothing** about what is inside a
//! workspace — that is [`WorkspaceFrame`].
//!
//! This is the body of the `workspaces` container: what `build_body`, the provider's render seam,
//! returns. It is reached only through that seam — nothing in `chrome` calls it.

use super::seams::{DockRegistries, DockSeams};
use crate::chrome::{BuildCx, WidgetModel};
use crate::providers::ChromeCtx;
use super::{MENU_CONTAINER, WorkspaceTree, container_items, workspace_frame::WorkspaceFrame};
use heca_grid_ui::builders::{LayoutExt, MenuExt, Parent};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate};
use heca_grid_ui::style::Spacing;
use heca_grid_ui::widgets::{Flex, ScrollRegion};

/// The workspace tree mounted inside the sidebar shell — one frame per workspace, in a scroll area
/// of this placement's own.
pub(crate) struct DockView<'a> {
    /// The session reduced to plain data by `model.rs`.
    pub(crate) tree: &'a WorkspaceTree,
    /// This placement's own scroll offset, keyed by mount id, so the same container placed twice
    /// keeps two positions.
    pub(crate) scroll: Signal<f32>,
}

impl DockView<'_> {
    /// Build the dock, returning the concrete widget so the region still sizes it.
    pub(crate) fn build(self, seams: &DockSeams<'_>, reg: &mut DockRegistries<'_>) -> ScrollRegion {
        let DockView { tree, scroll } = self;
        let mut col = Flex::column().gap(Spacing::Sm).grow(1.0);
        for workspace in &tree.workspaces {
            col = col.child(WorkspaceFrame { workspace }.build(seams, reg));
        }
        // The container nests its **own** scroll area, so its workspace list scrolls inside the slot
        // the region gave it. A scroll area is just a container, so this is
        // composition rather than a capability the shell has to hand down: the shell scrolls its dock
        // list, this scrolls its content, and nesting decides which one a wheel or a keyboard page
        // reaches — the innermost that can move on that axis wins, and the outer one only sees what
        // the inner declines.
        //
        // The offset is the CONTAINER's, so it survives the tree being rebuilt (a pane's git status
        // changing is enough to do that) and is untouched by the dock list scrolling around it.
        // Vertical only. A second axis was tried and reverted: a `Label` clips rather than
        // overflowing, so nothing ever exceeds the width and the horizontal axis had nothing to scroll —
        // while the extra scrollbar sat in front of the rows and took the press that starts a drag.
        // Revisit when label truncation/wrapping is settled (AGENTS.md lists it as open).
        let mut region = ScrollRegion::new().grow(1.0);
        // Restore through `scroll_to`, which reports with `event: None`, so the listener below can tell
        // a restore from the user actually scrolling and never writes one back as the other.
        region.scroll_to(scroll.get_untracked());
        region
            // Keyboard scroll intents reach only the dock the keyboard is in — they follow focus, so
            // this region needs no flag to say whether it is the one.
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
                let menu = crate::chrome::surfaces::context_menu::menu_from_items(
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

/// The render seam: project the frame's workspace tree into the container body.
///
/// Every input is read off the two contexts — nothing is captured — so one plain `fn`
/// serves as the build hook for every mount of this container.
///
/// Outside a render pass the context carries no tree/theme/emitter
/// ([`ChromeCtx::new`] vs [`ChromeCtx::for_build`]), so there is nothing to project:
/// the body is an empty column. That is a real state — a host may build a
/// contribution just to read its metadata — not an error, so it does not panic.
pub(super) fn build_body(ctx: &ChromeCtx<'_>, bx: &mut BuildCx<'_>) -> WidgetModel {
    let (Some(theme), Some(emit)) = (ctx.theme(), ctx.emit_intent()) else {
        return Box::new(Flex::column());
    };
    let state = ctx.state();
    // **This component's model and catalog come from its own state** — the shared
    // context carries only what is true for any component. The borrow is a `RefCell`: it lasts for
    // the projection, which reads the store but never writes back into it.
    let tree = state.workspaces().tree();
    let programs = state.workspaces().programs();
    // The three registries are borrowed as *disjoint* fields, which is exactly why they
    // are plain fields on `BuildCx` and not accessor methods: `bx.signals()` three times
    // in one call would be three overlapping `&mut *bx` borrows.
    // This placement's own scroll offset, keyed by mount id: place the container twice and each
    // keeps its own position, while the workspaces it shows come from the shared store either way.
    let scroll = state.container_scroll(bx.container_id());
    let ws_state = state.workspaces();
    // Read the id out before the registries are taken: `container_id()` borrows `bx`, and the two
    // registries below borrow it mutably.
    let mount = bx.container_id().to_string();
    // **The only file that touches `ChromeCtx`.** Everything below this line takes plain data plus
    // the seams it binds, so every component of this dock can be built in a test without a window
    // (AGENTS.md § 0b-bis step 4).
    let seams = DockSeams {
        // This placement's id — the rows' cursor signals are keyed by it, so two seatings of one
        // container light different rows.
        mount: &mount,
        theme,
        programs: &programs,
        emit,
        catalog: ctx.action_catalog().expect("a render pass has the catalog"),
        ws_state,
        panes: state.panes(),
        // Read once here rather than by each row: a workspace, a column and a card all ask the
        // same question of it, and asking the store three hundred times is three hundred chances
        // to ask it differently.
        active_pane: ws_state.active_pane(),
    };
    // The three registries are borrowed as *disjoint* fields, which is exactly why they
    // are plain fields on `BuildCx` and not accessor methods: `bx.signals()` twice
    // in one call would be two overlapping `&mut *bx` borrows.
    let mut reg = DockRegistries {
        signals: bx.signals,
        drag: bx.drag,
    };
    Box::new(
        DockView {
            tree: &tree,
            scroll,
        }
        .build(&seams, &mut reg),
    )
}

#[cfg(test)]
mod tests {
    use super::super::testing::{self, Fixture};
    use super::*;

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
            }
            .build(&seams, &mut reg)
        };

        let declared = testing::declared_keys(&root);
        for ws_idx in [0, 1] {
            assert!(
                declared.contains(&super::super::workspace_key(
                    heca_core::layout::WorkspaceId(ws_idx as u64)
                )),
                "workspace {ws_idx} has no frame: {declared:?}",
            );
        }
    }
}
