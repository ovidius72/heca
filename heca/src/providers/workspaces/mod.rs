//! The **workspaces** component — its model, its rows, and the chrome contribution it builds.
//!
//! It is a component in its own right, not a sidebar feature. It is *mounted* in a sidebar today
//! because that is where it happens to be useful; move it to another region — or another shell
//! entirely — and it goes with everything it needs. Nothing here names a side, and nothing outside
//! here should name a workspace, which is why nothing in it is called `Sidebar*`.

mod column_group;
mod dock_view;
mod model;
mod pane_row;
mod row_lines;
mod seams;
mod workspace_frame;
mod actions;
mod keys;
mod menus;
mod verbs;
pub(crate) use keys::*;
pub(crate) use menus::*;
use actions::*;
use dock_view::build_body;
use verbs::{Activate, Fold, RowVerb, Step};

pub(crate) use model::{ColumnEntry, PaneEntry, WorkspaceEntry, WorkspaceRow, WorkspaceTree};
pub(crate) use row_lines::{builtin_row_lines, text_line};

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod testing;

use crate::actions::{ActionCategory, ActionMeta};
use crate::app::interaction::ActionPolicy;
use crate::args::{ArgKind, ArgSpec};
use crate::chrome::{
    ChromeDragItem, ContainerContribution, Contribution, RegionId, RegionSet,
};
use crate::chrome::{Intent, PropMap, PropValue};
// The menu-entry vocabulary, shared with the host's own pane menu.
use crate::chrome::DropdownItem;
use crate::chrome::surfaces::context_menu::item_running;
use crate::providers::{ChromeCtx, Provider, ProviderCx};
use heca_core::layout::PaneId;
use heca_grid_ui::Handled;
use heca_grid_ui::widgets::Glyph;

/// The built-in workspace-tree sidebar container.
///
/// Mounts in the left sidebar by default and is movable to the right sidebar
/// (`supported_regions = sidebars`). It is the first real [`Provider`] registered
/// in [`ChromeHost`](crate::chrome::ChromeHost), proving the
/// pluggable-chrome runtime hosts a domain container — not just the
/// `TestProvider` stand-in in `host.rs` tests.
pub struct WorkspacesContainerProvider {
    /// This placement's **mount id** — what distinguishes one placement from another.
    id: String,
    /// Where this placement is seated on first run.
    region: RegionId,
}

impl WorkspacesContainerProvider {
    /// This container, under its own name.
    /// **Name this placement.** Everything that belongs to one seating keys off it — the cursor,
    /// the scroll offset, whether it holds chrome focus — so two of these are two placements of one
    /// container, not two containers.
    ///
    /// It is required, because a name is how everything else refers to this seating: a keybinding,
    /// the action registry, `focus_dock`, RPC. A placement the host had to name for you is one
    /// nothing you write can reach.
    ///
    /// ```ignore
    /// regions("sidebar.left").append(Workspaces::new("workspaces"));
    /// regions("sidebar.right").append(Workspaces::new("workspaces.right"));
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: name.into(),
            region: RegionId::LeftSidebar,
        }
    }
}

impl Default for WorkspacesContainerProvider {
    fn default() -> Self {
        Self::new("workspaces")
    }
}

impl Provider for WorkspacesContainerProvider {
    fn id(&self) -> &str {
        &self.id
    }

    /// One **type**, however many placements. `workspaces` and `workspaces.right` are two seatings
    /// of the same component, so they share a config namespace (`[[keys.component]] name = "workspaces"`)
    /// and one set of
    /// declared actions, while their cursor, scroll offset and focus stay their own.
    fn kind(&self) -> &str {
        "workspaces"
    }

    fn supported_regions(&self) -> RegionSet {
        RegionSet::sidebars()
    }

    fn default_region(&self) -> RegionId {
        self.region
    }

    fn title(&self) -> &str {
        "Workspaces"
    }

    fn movable(&self) -> bool {
        true
    }

    fn collapsible(&self) -> bool {
        true
    }

    /// The workspace tree has its own keyboard navigation once it holds focus — the `j`/`k` cursor
    /// over workspaces, columns and panes — so it is more than a scrollable list.
    fn keyboard_navigable(&self) -> bool {
        true
    }

    fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
        Contribution::Container(ContainerContribution {
            id: self.id().to_string(),
            title: self.title().to_string(),
            supported_regions: self.supported_regions(),
            default_region: self.default_region(),
            movable: self.movable(),
            collapsible: self.collapsible(),
            build: Box::new(build_body),
        })
    }

    /// What only this component can do: act on **its own** cursor.
    ///
    /// Everything here is selection-dependent — "the row the cursor is on" is a fact no other
    /// component and no built-in can know. Anything that is *not* selection-dependent is missing on
    /// purpose: `zoom_column`, `close`, `next_pane` already exist, and this component **binds**
    /// them in `[[keys.component]]` rather than redeclaring them.
    ///
    /// The cursor moves are its own actions, not host facilities: only this component knows that
    /// its rows are workspaces, columns and panes, and therefore what "next" means among them. A
    /// component showing a single number has no cursor and declares none of this.
    fn actions(&self) -> Vec<ActionMeta> {
        actions::declared()
    }

    /// Every action runs against **this placement's** cursor: the list is pointed at the row the
    /// placement selected before the verb runs, and the row it ends on is written back after one
    /// that acted. The verbs themselves never load or publish a cursor, so none can forget to.
    fn perform(&self, action: &str, args: &Intent, cx: &mut ProviderCx<'_>) -> Handled {
        self.load_cursor(cx);
        let handled = self.run(action, args, cx);
        if handled == Handled::Yes {
            self.publish_cursor(cx);
        }
        handled
    }
}

impl WorkspacesContainerProvider {
    fn run(&self, action: &str, args: &Intent, cx: &mut ProviderCx<'_>) -> Handled {
        match action {
            CURSOR_UP => self.step_cursor(cx, Step::Up),
            CURSOR_DOWN => self.step_cursor(cx, Step::Down),
            COLLAPSE_ROW => self.fold(cx, Fold::Collapse),
            TOGGLE_SELECTED => self.fold(cx, Fold::Toggle),
            ACTIVATE_SELECTED => self.activate(cx, Activate::AndLeave),
            PEEK_SELECTED => {
                // Aimed, when the caller named a row (`prefix+/`): move the cursor onto it first,
                // through this component's own key→row resolution, and the hint then acts on it
                // like any other. A bad or stale key leaves the cursor alone rather than failing —
                // the row it named is simply not there any more.
                if let Some(PropValue::Text(key)) = args.args.get("key") {
                    cx.state().workspaces().tree_mut().point_at(Some(key));
                }
                // **A hint leaves you ON that row, in the dock** — so it has to *take* the keyboard,
                // not merely refrain from releasing it. Written as one rule for every caller: from
                // `Space` the dock already has it and this is a no-op; from `prefix+/`, the palette
                // or RPC the keyboard was elsewhere, and without this the cursor moved somewhere the
                // user could not then drive with `j`/`k`.
                //
                // **Unguarded on purpose.** `focus_dock` only focuses — aimed at the dock that already
                // holds the keyboard it changes nothing (the release is `toggle_dock`) — so there is
                // no state to read first, and the keyboard is asked of the tree by the host, not
                // remembered here.
                let mount = cx.mount().to_string();
                cx.dispatch(
                    "focus_dock",
                    PropMap::from([("dock".to_string(), PropValue::Text(mount))]),
                );
                self.activate(cx, Activate::AndStay)
            }
            CREATE_COLUMN => self.at_workspace(cx, "add_column_to_workspace"),
            CREATE_PANE => self.at_column(cx, "add_pane_to_column"),
            ZOOM_SELECTED => self.at_column(cx, "zoom_column_at_index"),
            DELETE_COLUMN => self.at_column(cx, "delete_column"),
            DELETE_SELECTED => self.on_selected_row(cx, RowVerb::Delete),
            RENAME_SELECTED => self.on_selected_row(cx, RowVerb::Rename),
            _ => Handled::No,
        }
    }
}

#[cfg(test)]
mod tests;
