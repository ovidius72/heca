//! Shared fixtures for this dock's component tests (AGENTS.md § 0b-bis step 7).
//!
//! They exist so each test reads as its **assertion** rather than its setup. The important one is
//! [`seams`]: it hands back the app edges a component binds and nothing else, which is what makes
//! every component here buildable **without a window** — the whole point of the split. Before it,
//! the one projection test in this file had to construct a `ChromeCtx` to get at a catalog.

use super::seams::{DockRegistries, DockSeams};
use super::{ColumnEntry, PaneEntry, WorkspaceEntry, WorkspaceTree};
use crate::actions::ActionCatalog;
use crate::app_state::SidebarItemState;
use crate::chrome::{
    ChromeIntentEmitter, ChromeSignals, DragItemRegistry, SharedChromeState,
    WorkspacesContainerState,
};
use heca_config::programs::ProgramsConfig;
use heca_core::layout::PaneId;
use heca_grid_ui::theme::Theme as GuiTheme;

/// Everything a component's seams borrow from, owned by the test for as long as it runs.
pub(crate) struct Fixture {
    pub(crate) theme: GuiTheme,
    pub(crate) programs: std::rc::Rc<ProgramsConfig>,
    pub(crate) emit: ChromeIntentEmitter,
    pub(crate) catalog: ActionCatalog,
    pub(crate) store: SharedChromeState,
    pub(crate) signals: ChromeSignals,
    pub(crate) drag: DragItemRegistry,
}

impl Default for Fixture {
    fn default() -> Self {
        let store = SharedChromeState::new(300.0, true, 300.0, false);
        store.workspaces.set_active_pane(Some(PaneId(1)));
        Self {
            theme: GuiTheme::default(),
            programs: std::rc::Rc::new(ProgramsConfig::default()),
            emit: crate::chrome::ChromeIntentEmitter::of(
                crate::app::interaction::InteractionSource::Keyboard,
                |_, _| {},
            ),
            catalog: ActionCatalog::with_builtins(),
            store,
            signals: ChromeSignals::default(),
            drag: DragItemRegistry::default(),
        }
    }
}

impl Fixture {
    /// Both halves a component takes, in one call — exactly as `build_body` hands them over.
    ///
    /// One method rather than two accessors because the two halves borrow the fixture
    /// **differently**: the seams are shared and the registries are exclusive, so handing them out
    /// separately would overlap. Splitting them here, as disjoint fields, is the same reason
    /// `BuildCx` exposes plain fields instead of accessor methods.
    pub(crate) fn split<'a>(&'a mut self, mount: &'a str) -> (DockSeams<'a>, DockRegistries<'a>) {
        let ws_state: &WorkspacesContainerState = &self.store.workspaces;
        (
            DockSeams {
                mount,
                theme: &self.theme,
                programs: &self.programs,
                emit: &self.emit,
                catalog: &self.catalog,
                ws_state,
                active_pane: ws_state.active_pane(),
            },
            DockRegistries {
                signals: &mut self.signals,
                drag: &mut self.drag,
            },
        )
    }
}

/// One workspace, one column, one (active) pane — the smallest tree that still exercises every
/// level of the projection.
pub(crate) fn tree() -> WorkspaceTree {
    let mut tree = WorkspaceTree::new();
    tree.workspaces.push(WorkspaceEntry {
        ws_idx: 0,
        ws_id: heca_core::layout::WorkspaceId(0),
        name: "ws1".into(),
        custom_name: None,
        collapsed: false,
        state: SidebarItemState::Active,
        columns: vec![column()],
        floating_panes: Vec::new(),
    });
    tree
}

/// The single column of [`tree`].
pub(crate) fn column() -> ColumnEntry {
    ColumnEntry {
        col_idx: 0,
        col_id: heca_core::layout::ColumnId(0),
        collapsed: false,
        panes: vec![pane(PaneId(1), "pane1")],
    }
}

/// One pane entry, as `model.rs` would have reduced it.
pub(crate) fn pane(pane_id: PaneId, name: &str) -> PaneEntry {
    PaneEntry {
        pane_id,
        name: name.into(),
        custom_name: None,
        state: SidebarItemState::None,
    }
}

/// Every `key` declared anywhere in a built tree — how a right-click finds what it landed on.
pub(crate) fn declared_keys(root: &dyn heca_grid_ui::Component) -> Vec<String> {
    fn walk(n: &dyn heca_grid_ui::Component, out: &mut Vec<String>) {
        if let Some(k) = n.base().key.clone() {
            out.push(k);
        }
        for c in &n.base().children {
            walk(c.as_ref(), out);
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}
