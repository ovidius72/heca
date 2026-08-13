//! The **workspaces** component — its model, its rows, and the chrome contribution it builds.
//!
//! It is a component in its own right, not a sidebar feature. It is *mounted* in a sidebar today
//! because that is where it happens to be useful; move it to another region — or another shell
//! entirely — and it goes with everything it needs. Nothing here names a side, and nothing outside
//! here should name a workspace.
//!
//! That is also why nothing in this component is called `Sidebar*` any more: the old names asserted
//! that a workspace tree was a sidebar thing, and every reader of those names inherited the
//! confusion (F003/P085/T356).

mod column_group;
mod dock_view;
mod model;
mod pane_row;
mod seams;
mod workspace_frame;

pub(crate) use model::{ColumnEntry, PaneEntry, WorkspaceEntry, WorkspaceRow, WorkspaceTree};

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod testing;

use crate::chrome::{
    BuildCx, ChromeDragItem, ContainerContribution, Contribution, RegionId, RegionSet, WidgetModel,
};
use crate::actions::{ActionCategory, ActionMeta, ArgKind, ArgSpec};
use crate::app::interaction::ActionPolicy;
use crate::chrome::{Intent, PropMap, PropValue};
// The menu-entry vocabulary, shared with the host's own pane menu (F003/P086/T365).
use crate::chrome::context_menu::{item, item_running, usize_arg};
use crate::chrome::DropdownItem;
use crate::providers::{ChromeCtx, Provider, ProviderCx};
use dock_view::DockView;
use seams::{DockRegistries, DockSeams};
use heca_grid_ui::Handled;
use heca_core::layout::PaneId;
use heca_grid_ui::widgets::{Flex, Glyph};

/// The built-in workspace-tree sidebar container.
///
/// Mounts in the left sidebar by default and is movable to the right sidebar
/// (`supported_regions = sidebars`). It is the first real [`Provider`] registered
/// in [`ChromeHost`](crate::chrome::ChromeHost) (plugin-03), proving the
/// pluggable-chrome runtime hosts a domain container — not just the
/// `TestProvider` stand-in in `host.rs` tests.
pub struct WorkspacesContainerProvider {
    /// This placement's **mount id** — what distinguishes one placement from another.
    id: String,
    /// Where this placement is seated on first run.
    region: RegionId,
}

impl WorkspacesContainerProvider {
    /// The default placement: id `workspaces`, seated in the left sidebar.
    pub fn new() -> Self {
        Self::placed("workspaces", RegionId::LeftSidebar)
    }

    /// **Place this container**, with its own mount id, in `region`.
    ///
    /// A widget is a component and placing one is placing it — so a second placement is this call,
    /// not a second implementation of the provider. The build hook is one plain `fn` that reads
    /// everything off the contexts, so it already serves every placement; what a placement needs of
    /// its own is an id (state that is per mount, like its scroll position, is keyed by it) and
    /// somewhere to sit.
    ///
    /// The **content is the same either way**: the workspaces come from the shared store, exactly as
    /// two renders of one component show the same data.
    pub fn placed(id: impl Into<String>, region: RegionId) -> Self {
        Self {
            id: id.into(),
            region,
        }
    }
}

impl Default for WorkspacesContainerProvider {
    fn default() -> Self {
        Self::new()
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

    fn default_order(&self) -> i32 {
        0
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
            default_order: self.default_order(),
            movable: self.movable(),
            collapsible: self.collapsible(),
            grow: self.grow(),
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
        // **The policy is per action, not per component.** Moving the cursor and deleting a
        // workspace are not the same kind of act and must not share a gate: the cursor is chrome
        // state and stays drivable while a pane is floating, while every mutation below touches the
        // tiled layout and is blocked there — which is exactly what the `sidebar_*` built-ins they
        // replaced declared (`TiledOnly`). Passing it in is what keeps the two apart, and a single
        // shared default is how they silently became one (F003/P085/T356).
        let act = |name: &str, label: &str, description: &str, icon, policy| ActionMeta {
            name: name.to_string(),
            label: label.to_string(),
            description: description.to_string(),
            category: ActionCategory::Navigation,
            // Stamped by the host at registration (`register_provider_actions`) — a component
            // declares what it does, never who it is.
            owner: None,
            icon,
            policy,
            args: Vec::new(),
            confirm: None,
        };
        // Chrome state: the cursor is not the pane layout, so these stay reachable while a floating
        // pane is active — the dock is still there to be driven.
        let cursor = ActionPolicy::Global;
        // Everything that acts on **the row the cursor is on**, which is only a question worth
        // asking while this dock has the keyboard (F003/P086/T371). They declared `TiledOnly`, which
        // was true but not the point: it blocked them while floating and left them reachable from
        // the command palette and RPC with the dock unfocused, acting on a cursor the user cannot
        // see. `ContainerFocused` says what they mean; `Container` still permits the tiled actions
        // beside them, so `prefix+Enter` keeps splitting the pane you last worked in.
        let mutates = ActionPolicy::ContainerFocused;
        vec![
            act(
                CURSOR_UP,
                "Cursor Up",
                "Move the workspaces cursor to the previous row.",
                Some(Glyph::CaretUp),
                cursor,
            ),
            act(
                CURSOR_DOWN,
                "Cursor Down",
                "Move the workspaces cursor to the next row.",
                Some(Glyph::CaretDown),
                cursor,
            ),
            act(
                COLLAPSE_ROW,
                "Collapse Row",
                "Collapse the row under the cursor, or move out to its parent.",
                Some(Glyph::CaretLeft),
                cursor,
            ),
            act(
                ACTIVATE_SELECTED,
                "Activate Row",
                "Expand a structural row, or focus the pane under the cursor and leave the dock.",
                Some(Glyph::CaretRight),
                cursor,
            ),
            // **A peek can be aimed.** It acts on the cursor by default; the universal picker
            // passes the nav key of the row whose letter was chosen, so `prefix+/` lands on *that*
            // row rather than wherever the cursor happened to be. Optional, because every other
            // caller — the keybinding, the palette, RPC — means "the row I am on".
            ActionMeta {
                args: vec![ArgSpec {
                    name: "key".to_string(),
                    kind: ArgKind::Text,
                    required: false,
                    description: "The row to peek, by its nav key (default: the cursor).".to_string(),
                    values: Vec::new(),
                }],
                ..act(
                    PEEK_SELECTED,
                    "Peek Row",
                    "Focus what the cursor points at without leaving the dock.",
                    Some(Glyph::Search),
                    cursor,
                )
            },
            act(
                TOGGLE_SELECTED,
                "Expand / Collapse Row",
                "Fold or unfold the structural row under the cursor.",
                Some(Glyph::List),
                cursor,
            ),
            act(
                CREATE_COLUMN,
                "Add Column",
                "Add a column to the workspace the cursor is in.",
                Some(Glyph::ColumnsPlusRight),
                mutates,
            ),
            act(
                CREATE_PANE,
                "Add Pane",
                "Add a pane to the column the cursor is in.",
                Some(Glyph::FolderSimplePlus),
                mutates,
            ),
            act(
                RENAME_SELECTED,
                "Rename Row",
                "Rename what the cursor is on — a pane or a workspace.",
                Some(Glyph::NotePencil),
                mutates,
            ),
            act(
                DELETE_SELECTED,
                "Delete Row",
                "Delete what the cursor is on — a pane or a workspace.",
                Some(Glyph::Trash),
                mutates,
            ),
            act(
                DELETE_COLUMN,
                "Delete Column",
                "Delete the column the cursor is in, and every pane in it.",
                Some(Glyph::Trash),
                mutates,
            ),
            act(
                ZOOM_SELECTED,
                "Zoom Column",
                "Toggle zoom on the column the cursor is in.",
                Some(Glyph::FrameCorners),
                mutates,
            ),
        ]
    }


    /// The host moved this placement's cursor — reconcile the tree's positional index with it.
    ///
    /// Resolved by **matching this component's own keys against its own rows**, rather than parsing
    /// the key back into a selection. Parsing would have to guess: a tiled pane and a floating one
    /// are both `pane:<id>`, and only the row knows which it is.
    fn cursor_moved(&self, key: &str, cx: &mut ProviderCx<'_>) {
        let state = cx.state();
        let ws = state.workspaces();
        // Read, then drop the borrow: `tree_mut` below takes the same `RefCell`.
        let selection = {
            let tree = ws.tree();
            tree.flat_items
                .iter()
                .map(|row| row.selection())
                .find(|sel| selection_nav_key(*sel) == key)
        };
        let Some(selection) = selection else {
            return;
        };
        ws.tree_mut().apply_nav_selection(Some(selection));
        ws.set_nav_selection(Some(selection));
    }

    fn perform(&self, action: &str, args: &Intent, cx: &mut ProviderCx<'_>) -> Handled {
        match action {
            CURSOR_UP => self.step_cursor(cx, Step::Up),
            CURSOR_DOWN => self.step_cursor(cx, Step::Down),
            COLLAPSE_ROW => self.fold(cx, Fold::Collapse),
            TOGGLE_SELECTED => self.fold(cx, Fold::Toggle),
            ACTIVATE_SELECTED => self.activate(cx, Activate::AndLeave),
            PEEK_SELECTED => {
                // Aimed, when the caller named a row (`prefix+/`): move the cursor onto it first,
                // through this component's own key→row resolution, and the peek then acts on it
                // like any other. A bad or stale key leaves the cursor alone rather than failing —
                // the row it named is simply not there any more.
                if let Some(PropValue::Text(key)) = args.args.get("key") {
                    self.cursor_moved(key, cx);
                }
                // **A peek leaves you ON that row, in the dock** — so it has to *take* the keyboard,
                // not merely refrain from releasing it. Written as one rule for every caller: from
                // `Space` the dock already has it and this is a no-op; from `prefix+/`, the palette
                // or RPC the keyboard was elsewhere, and without this the cursor moved somewhere the
                // user could not then drive with `j`/`k`. Antonio, 2026-08-10, driving the picker:
                // *"it activates the pane but the keyboard goes to the terminal"*.
                //
                // **Guarded, because `focus_dock` aimed at the dock that already holds the keyboard
                // is the way back out** (`handle_focus_dock`) — dispatching it unconditionally would
                // make every peek from inside the dock release the keyboard instead.
                if cx.state().focused_container().as_deref() != Some(cx.mount()) {
                    let mount = cx.mount().to_string();
                    cx.dispatch(
                        "focus_dock",
                        PropMap::from([("dock".to_string(), PropValue::Text(mount))]),
                    );
                }
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

// The ids this component declares. Written once so the declaration and `perform` cannot drift, and
// namespaced by `kind()` so two components can both have a `cursor_up`.
const CURSOR_UP: &str = "workspaces.cursor_up";
const CURSOR_DOWN: &str = "workspaces.cursor_down";
const COLLAPSE_ROW: &str = "workspaces.collapse_row";
const ACTIVATE_SELECTED: &str = "workspaces.activate_selected";
const PEEK_SELECTED: &str = "workspaces.peek_selected";
const TOGGLE_SELECTED: &str = "workspaces.toggle_selected";
// The mutations, moved off the app's built-ins (F003/P085/T356). Each resolves the cursor to a
// target and dispatches an EXISTING parameterized action — none of them reimplements anything.
const CREATE_COLUMN: &str = "workspaces.create_column";
const CREATE_PANE: &str = "workspaces.create_pane";
const DELETE_SELECTED: &str = "workspaces.delete_selected";
// Renaming the row the cursor is on. The app's own `rename_pane` / `rename_workspace` mean the
// *focused* pane and the *active* workspace, which is a different question — the host used to bend
// them at the cursor by reading the row off a stashed context target, i.e. by resolving facts about
// a row it cannot see (F003/P086/T365, user decision 2026-07-30). Cursor-dependent ⇒ this
// component's, like `delete_selected` beside it.
const RENAME_SELECTED: &str = "workspaces.rename_selected";
const DELETE_COLUMN: &str = "workspaces.delete_selected_column";
const ZOOM_SELECTED: &str = "workspaces.zoom_selected";

enum Step {
    Up,
    Down,
}

enum Fold {
    Collapse,
    Toggle,
}

enum Activate {
    /// `Enter`/`l` — focus the row and hand the keyboard back to the pane.
    AndLeave,
    /// `Space` — focus the row in the main view but keep driving the dock.
    AndStay,
}

/// What to do to the row under the cursor. Both verbs resolve the same way — the row decides
/// whether the by-id action is the pane's or the workspace's — so they share one arm.
#[derive(Clone, Copy)]
enum RowVerb {
    Rename,
    Delete,
}

impl WorkspacesContainerProvider {
    /// Move the cursor over this component's own rows.
    fn step_cursor(&self, cx: &mut ProviderCx<'_>, step: Step) -> Handled {
        {
            let state = cx.state();
            let mut tree = state.workspaces().tree_mut();
            match step {
                Step::Up => tree.cursor_up(),
                Step::Down => tree.cursor_down(),
            }
        }
        self.publish_cursor(cx);
        Handled::Yes
    }

    /// Fold or unfold the structural row under the cursor. A leaf has nothing to fold, so `Toggle`
    /// on a pane does nothing rather than guessing at a meaning for it.
    fn fold(&self, cx: &mut ProviderCx<'_>, fold: Fold) -> Handled {
        {
            let state = cx.state();
            let ws = state.workspaces();
            let leaf = matches!(
                ws.tree().current_item(),
                Some(WorkspaceRow::Pane { .. }) | Some(WorkspaceRow::FloatingPane { .. })
            );
            match fold {
                // `h` on a leaf still means "out to my parent", which `collapse` implements.
                Fold::Collapse => ws.tree_mut().collapse(ws),
                Fold::Toggle if !leaf => ws.tree_mut().toggle_expand(ws),
                Fold::Toggle => {}
            }
        }
        self.publish_cursor(cx);
        Handled::Yes
    }

    /// Focus what the cursor points at. A structural row expands instead — `l` on a collapsed
    /// workspace opens it, which is what makes one key walk the tree outward.
    fn activate(&self, cx: &mut ProviderCx<'_>, mode: Activate) -> Handled {
        // Read, then drop the borrow: `dispatch` reaches back into the store.
        let row = {
            let state = cx.state();
            state.workspaces().tree().current_item().cloned()
        };
        let Some(row) = row else {
            return Handled::No;
        };
        match row {
            WorkspaceRow::Pane { pane_id } | WorkspaceRow::FloatingPane { pane_id, .. } => {
                cx.dispatch(
                    "focus_pane",
                    PropMap::from([("pane_id".to_string(), PropValue::Int(pane_id.0 as i64))]),
                );
                // Activating a leaf hands the keyboard back; peeking keeps it here.
                if matches!(mode, Activate::AndLeave) {
                    cx.dispatch("unfocus_dock", PropMap::new());
                }
            }
            WorkspaceRow::Workspace { ws_idx } => {
                let collapsed = {
                    let state = cx.state();
                    let ws = state.workspaces();
                    let collapsed = ws.is_ws_collapsed(ws_idx);
                    // Expanding is this component's own business — no app state involved.
                    if collapsed && matches!(mode, Activate::AndLeave) {
                        ws.tree_mut().expand(ws);
                    }
                    collapsed
                };
                if !(collapsed && matches!(mode, Activate::AndLeave)) {
                    cx.dispatch(
                        "focus_workspace",
                        PropMap::from([("ws_idx".to_string(), PropValue::Int(ws_idx as i64))]),
                    );
                }
            }
            // The cursor never lands on a column (`WorkspaceTree::is_navigable`).
            WorkspaceRow::Column { .. } => return Handled::No,
        }
        self.publish_cursor(cx);
        Handled::Yes
    }

    // ── The mutations (F003/P085/T356) ──
    //
    // **"New workspace" is not here on purpose.** It is about no row, so it is not this component's
    // to declare — `create_workspace` already exists and the component simply BINDS it (`w`), which
    // is the same rule that keeps `zoom_column`, `close` and `next_pane` out of the list above.
    // Declaring `workspaces.create_workspace` would also have been unreachable: a short name that
    // matches a built-in resolves to the built-in, by design.
    //
    // Every one of these does the same two things: resolve **this component's cursor** to a target,
    // then dispatch an action that already exists with that target as arguments. The component
    // contributes the only part nothing else can know — which row the cursor is on — and reuses the
    // rest. That is why `zoom_column_at_index` had to be added: without a parameterized form there
    // was nothing to dispatch, and "zoom the selected column" could only be a handler reaching into
    // this component's model from outside.

    /// The workspace the cursor is in, whatever kind of row it is on.
    fn cursor_workspace(&self, cx: &ProviderCx<'_>) -> Option<usize> {
        let state = cx.state();
        let tree = state.workspaces().tree();
        match tree.current_item()? {
            WorkspaceRow::Workspace { ws_idx }
            | WorkspaceRow::Column { ws_idx, .. }
            | WorkspaceRow::FloatingPane { ws_idx, .. } => Some(*ws_idx),
            WorkspaceRow::Pane { pane_id } => tree.locate_pane(*pane_id).map(|(ws, _)| ws),
        }
    }

    /// The column the cursor is in — `None` on a workspace row or a floating pane, neither of which
    /// is inside a column.
    fn cursor_column(&self, cx: &ProviderCx<'_>) -> Option<(usize, usize)> {
        let state = cx.state();
        let tree = state.workspaces().tree();
        match tree.current_item()? {
            WorkspaceRow::Column { ws_idx, col_idx } => Some((*ws_idx, *col_idx)),
            WorkspaceRow::Pane { pane_id } => tree.locate_pane(*pane_id),
            WorkspaceRow::Workspace { .. } | WorkspaceRow::FloatingPane { .. } => None,
        }
    }

    /// Dispatch `action` with the cursor's workspace.
    fn at_workspace(&self, cx: &mut ProviderCx<'_>, action: &str) -> Handled {
        // Read, then drop the borrow: `dispatch` reaches back into the store.
        let Some(ws_idx) = self.cursor_workspace(cx) else {
            return Handled::No;
        };
        cx.dispatch(
            action,
            PropMap::from([("ws_idx".to_string(), PropValue::Int(ws_idx as i64))]),
        );
        Handled::Yes
    }

    /// Dispatch `action` with the cursor's column. A cursor that is not in one declines — a silent
    /// decline is a real answer, not an error.
    fn at_column(&self, cx: &mut ProviderCx<'_>, action: &str) -> Handled {
        let Some((ws_idx, col_idx)) = self.cursor_column(cx) else {
            return Handled::No;
        };
        cx.dispatch(
            action,
            PropMap::from([
                ("ws_idx".to_string(), PropValue::Int(ws_idx as i64)),
                ("col_idx".to_string(), PropValue::Int(col_idx as i64)),
            ]),
        );
        Handled::Yes
    }

    /// Act on what the cursor is **on** — a pane or a workspace — with the by-id action for
    /// whichever it is.
    ///
    /// Never a column: the cursor does not land on one (`WorkspaceTree::is_navigable`), which is why
    /// `workspaces.delete_selected_column` is a separate verb rather than a case here.
    ///
    /// Every one goes out as an ordinary dispatch, so the central destructive gate raises the same
    /// confirm it does for a keybinding, a header button or an RPC call — the component neither asks
    /// nor can skip it.
    fn on_selected_row(&self, cx: &mut ProviderCx<'_>, verb: RowVerb) -> Handled {
        let row = {
            let state = cx.state();
            state.workspaces().tree().current_item().cloned()
        };
        // `(action, argument name, argument value)` — the row decides which of the two by-id
        // actions the verb means, and what to aim it at.
        let (action, arg, value) = match (verb, row) {
            (RowVerb::Delete, Some(WorkspaceRow::Pane { pane_id }))
            | (RowVerb::Delete, Some(WorkspaceRow::FloatingPane { pane_id, .. })) => {
                ("close_pane_by_id", "pane_id", pane_id.0 as i64)
            }
            (RowVerb::Rename, Some(WorkspaceRow::Pane { pane_id }))
            | (RowVerb::Rename, Some(WorkspaceRow::FloatingPane { pane_id, .. })) => {
                ("rename_pane_by_id", "pane_id", pane_id.0 as i64)
            }
            (RowVerb::Delete, Some(WorkspaceRow::Workspace { ws_idx })) => {
                ("delete_workspace", "ws_idx", ws_idx as i64)
            }
            (RowVerb::Rename, Some(WorkspaceRow::Workspace { ws_idx })) => {
                ("rename_workspace_by_idx", "ws_idx", ws_idx as i64)
            }
            (_, Some(WorkspaceRow::Column { .. })) | (_, None) => return Handled::No,
        };
        cx.dispatch(
            action,
            PropMap::from([(arg.to_string(), PropValue::Int(value))]),
        );
        Handled::Yes
    }

    /// Publish the cursor into the store: the generic per-mount cursor the row outlines read, and —
    /// until F003/P085/T356 finishes — the domain-typed `nav_selection` beside it.
    fn publish_cursor(&self, cx: &mut ProviderCx<'_>) {
        let selection = {
            let state = cx.state();
            let selection = state.workspaces().tree().selection();
            state.workspaces().set_nav_selection(selection);
            selection
        };
        cx.set_selected(selection.map(selection_nav_key));
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
fn build_body(ctx: &ChromeCtx<'_>, bx: &mut BuildCx<'_>) -> WidgetModel {
    let (Some(theme), Some(emit)) = (ctx.theme(), ctx.emit_intent()) else {
        return Box::new(Flex::column());
    };
    let state = ctx.state();
    // **This component's model and catalog come from its own state** (F003/P086/T367) — the shared
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
    // …and whether this placement is the one the keyboard is aimed at (F003/P011/T020). Per mount for
    // the same reason: only one of two placements can hold focus.
    let focused = state.container_keyboard_target(bx.container_id());
    let ws_state = state.workspaces();
    // Read the id out before the registries are taken: `container_id()` borrows `bx`, and the two
    // registries below borrow it mutably.
    let mount = bx.container_id().to_string();
    // **The only file that touches `ChromeCtx`.** Everything below this line takes plain data plus
    // the seams it binds, so every component of this dock can be built in a test without a window
    // (AGENTS.md § 0b-bis step 4).
    let seams = DockSeams {
        // This placement's id — the rows' cursor signals are keyed by it, so two seatings of one
        // container light different rows (F003/P085/T354).
        mount: &mount,
        theme,
        programs: &programs,
        emit,
        catalog: ctx.action_catalog().expect("a render pass has the catalog"),
        ws_state,
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
            focused,
        }
        .build(&seams, &mut reg),
    )
}

// ── The workspaces container's row identities (F003/P085/T354) ──
//
// A row declares one of these and the host derives everything that has to *name* a row from it: the
// keyboard cursor, the right-click target, later the drag identity. They are the same identities
// `SidebarSelection` carries today, written as strings so nothing outside this component has to
// know a workspace from a pane — which is what lets a plugin row be pointed at at all.
//
// They must stay **stable across a tree rebuild**: a cursor that resets whenever a pane's git status
// changes is not a cursor. Positions therefore never appear in a key that has an id available.

/// `pane:<id>` — a pane card, tiled or floating.
pub(crate) fn pane_nav_key(pane: PaneId) -> String {
    format!("pane:{}", pane.0)
}

/// `ws:<idx>` — a workspace header.
pub(crate) fn workspace_nav_key(ws_idx: usize) -> String {
    format!("ws:{ws_idx}")
}

/// `col:<ws>:<idx>` — a column group. Positional because a column has no id of its own; it is
/// re-derived on rebuild like every other column reference in the app.
pub(crate) fn column_nav_key(ws_idx: usize, col_idx: usize) -> String {
    format!("col:{ws_idx}:{col_idx}")
}

// ── What each row kind does when you press it (F003/P086/T365) ──
//
// **Each item kind declares its own click, by name.** A workspace row and a pane row genuinely do
// different things, so neither inherits from the other and nothing is forced on a kind that has no
// gesture — a column row declares none, and that is a real answer.
//
// A name, not a closure: the same declaration answers the click, the `prefix+/` pick, a menu entry,
// a keybinding and RPC, and is routed by that action's own policy. Both of these **bind actions
// that already exist** — focusing a pane means the same thing with or without this component, so by
// the ownership test they are the app's and this component only points at them.

/// Pressing a **pane row** focuses that pane.
fn pane_row_press(pane_id: PaneId) -> Intent {
    Intent::new("focus_pane").arg("pane_id", PropValue::Int(pane_id.0 as i64))
}

// A workspace row declares **no click**: a `DockFrame`'s own header owns its press (the disclosure
// caret folds the section), and nothing forces a kind to declare a gesture it does not have. It
// used to carry `focus_workspace` here purely so the `prefix+/` picker had an intent to register —
// which is what pointing one declaration at two gestures looks like from the other side. Its peek
// says the same thing and more (`peek_selected` also lands the cursor on the row), so the
// stand-in is gone.

/// **What a `prefix+/` pick does to a row of this component — and it is not what a click does.**
///
/// A click on a row means *go there and leave*: `focus_pane` + the dock releasing the keyboard. A
/// pick means *look at that one*, so it moves the cursor onto the picked row and brings its pane to
/// the front **without leaving the dock** — this component's own `peek_selected`, aimed at a row by
/// its nav key instead of the cursor. Antonio, 2026-08-07: *"i want it to focus the cursor in the
/// peeked letter and is good if the pane gets active"*.
///
/// Pointing one intent at both gestures is precisely the bug this replaces: commit `e712d70`
/// (2026-07-30) made the row's hint target fire the row's *click*, and `prefix+/` on a sidebar row
/// started activating the pane and leaving the sidebar.
fn row_peek(nav_key: String) -> Intent {
    Intent::new(PEEK_SELECTED).arg("key", PropValue::Text(nav_key))
}

/// The nav key naming the row a [`SidebarSelection`] points at.
///
/// The bridge between the domain-typed selection this container still keeps and the generic cursor
/// every row's outline now reads. It goes away with the selection itself (F003/P085/T356) — until
/// then it is the single conversion point, so the two cannot drift.
pub(crate) fn selection_nav_key(selection: crate::chrome::SidebarSelection) -> String {
    use crate::chrome::SidebarSelection as S;
    match selection {
        S::Pane { pane_id } | S::FloatingPane { pane_id, .. } => pane_nav_key(pane_id),
        S::Column { ws_idx, col_idx } => column_nav_key(ws_idx, col_idx),
        S::Workspace { ws_idx } => workspace_nav_key(ws_idx),
    }
}

// ── This component's context menus (F003/P086/T365) ──
//
// **A container's menu is the component's, not the host's.** These three used to be registered in
// `ContextMenuRegistry::with_builtins` under `sidebar.pane` / `sidebar.column` / `sidebar.workspace`
// — paths named for *where the rows are drawn*, which is the same defect `SidebarTree` and the
// `sidebar_*` actions had. They are the component's own paths now, namespaced like its action ids,
// and a Docker dock names its rows `docker.container` with no host constant involved.

/// A pane row of this component.
pub(crate) const MENU_PANE: &str = "workspaces.pane";
/// A column row of this component.
pub(crate) const MENU_COLUMN: &str = "workspaces.column";
/// A workspace row of this component.
pub(crate) const MENU_WORKSPACE: &str = "workspaces.workspace";
/// The container itself — the menu for empty space, found by bubbling when no row claims the click.
pub(crate) const MENU_CONTAINER: &str = "workspaces.container";

/// Menu items for the container's **empty space**: the verbs that are about the list rather than
/// about any row in it. Facts-only, so it is unit-testable without a ctx, like its row siblings.
pub(crate) fn container_items() -> Vec<DropdownItem> {
    vec![item("create_workspace", "New workspace")]
}

/// The row this component's `key` names, or `None` for a key it did not write.
///
/// **Matched, never parsed** — the one resolution every reader of a key goes through here (the
/// cursor, the menu path, the menu itself), so they cannot disagree about what a key points at.
fn row_at_key(tree: &WorkspaceTree, key: &str) -> Option<WorkspaceRow> {
    tree.flat_items
        .iter()
        .find(|row| selection_nav_key(row.selection()) == key)
        .cloned()
}

/// Menu items for a **pane** row (facts-only, so unit-testable without a ctx). Every entry acts on
/// *that* row: the pane by id, "New pane" in the pane's own column when it has one.
pub(crate) fn pane_row_items(
    pane_id: PaneId,
    column: Option<(usize, usize)>,
    has_custom_name: bool,
) -> Vec<DropdownItem> {
    let pane = PropValue::Int(pane_id.0 as i64);
    let mut items = Vec::new();
    if let Some((ws_idx, col_idx)) = column {
        items.push(item_running(
            "add_pane_to_column",
            "New pane",
            "add_pane_to_column",
            &[("ws_idx", usize_arg(ws_idx)), ("col_idx", usize_arg(col_idx))],
        ));
    }
    items.push(item_running(
        "rename_pane",
        "Rename pane",
        "rename_pane_by_id",
        &[("pane_id", pane.clone())],
    ));
    if has_custom_name {
        items.push(item_running(
            "reset_pane_name",
            "Use process name",
            "reset_pane_name_by_id",
            &[("pane_id", pane.clone())],
        ));
    }
    items.push(
        item_running("close", "Close pane", "close_pane_by_id", &[("pane_id", pane)]).danger(true),
    );
    items
}

/// Menu items for a **column** row (facts-only, so unit-testable without a ctx).
pub(crate) fn column_row_items(ws_idx: usize, col_idx: usize) -> Vec<DropdownItem> {
    vec![
        item_running(
            "add_pane_to_column",
            "New pane",
            "add_pane_to_column",
            &[("ws_idx", usize_arg(ws_idx)), ("col_idx", usize_arg(col_idx))],
        ),
        item_running(
            "split_horizontal",
            "New column",
            "add_column_to_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
        // NB: no "Rename column" entry — a column's name is not displayed anywhere yet (columns
        // render as a MarkerGroup with no header/label), so renaming would have no visible effect.
        // The rename action stays wired (RPC + handler) for when columns surface a name.
        item_running(
            "delete_column",
            "Delete column",
            "delete_column",
            &[("ws_idx", usize_arg(ws_idx)), ("col_idx", usize_arg(col_idx))],
        )
        .danger(true),
    ]
}

/// Menu items for a **workspace** row. `has_custom_name` gates the "Use default name"
/// reset entry — it only appears when there is a custom name to clear.
pub(crate) fn workspace_row_items(ws_idx: usize, has_custom_name: bool) -> Vec<DropdownItem> {
    let mut items = vec![
        item_running(
            "split_horizontal",
            "New column",
            "add_column_to_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
        item("create_workspace", "New workspace"),
        item_running(
            "rename_workspace",
            "Rename workspace",
            "rename_workspace_by_idx",
            &[("ws_idx", usize_arg(ws_idx))],
        ),
    ];
    if has_custom_name {
        items.push(item_running(
            "reset_workspace_name",
            "Use default name",
            "reset_workspace_name_by_idx",
            &[("ws_idx", usize_arg(ws_idx))],
        ));
    }
    items.push(
        item_running(
            "delete_workspace",
            "Delete workspace",
            "delete_workspace",
            &[("ws_idx", usize_arg(ws_idx))],
        )
        .danger(true),
    );
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::SidebarItemState;
    use crate::chrome::{
        ChromeEventBus, ChromeHost, ChromeIntentEmitter, ChromeSignals, DragItemRegistry,
        SharedChromeState,
    };
    use crate::providers::workspaces::{ColumnEntry, PaneEntry, WorkspaceTree, WorkspaceEntry};
    use heca_core::layout::PaneId;
    use heca_grid_ui::theme::Theme as GuiTheme;
    use std::rc::Rc;

    /// **Every row kind declares its own `nav_key` on the widget.**
    ///
    /// `nav_key_at` — how a right-click finds out what it landed on — reads `Base::nav_key` and
    /// nothing else. The workspace header pushed its key into the cursor-signal list and never told
    /// the widget, so the hit-test found nothing at that row: right-clicking a pane or a column
    /// opened its menu and a workspace opened none (Antonio, 2026-08-05). Pushing the key to the
    /// signals and declaring it on the widget are two different acts, and the projection tests
    /// only ever checked the first.
    ///
    /// Note what this test no longer needs: a `ChromeCtx`, and therefore a window. The dock is
    /// components now, so it is built from its seams (F006/P032/T429).
    #[test]
    fn every_row_kind_carries_a_nav_key_the_hit_test_can_find() {
        let tree = testing::tree();
        let mut fx = testing::Fixture::default();
        let root = {
            let (seams, mut reg) = fx.split("left");
            DockView {
                tree: &tree,
                scroll: heca_grid_ui::reactive::signal(0.0),
                focused: heca_grid_ui::reactive::signal(false),
            }
            .build(&seams, &mut reg)
        };

        let declared = testing::declared_nav_keys(&root);
        for expected in [
            workspace_nav_key(0),
            column_nav_key(0, 0),
            pane_nav_key(PaneId(1)),
        ] {
            assert!(
                declared.contains(&expected),
                "no widget declares {expected:?}, so a right-click there finds nothing: {declared:?}",
            );
        }
    }

    /// One workspace, one column, one (active) pane — the smallest tree that still
    /// exercises every level of the projection.
    fn tree() -> WorkspaceTree {
        let mut tree = WorkspaceTree::new();
        tree.workspaces.push(WorkspaceEntry {
            ws_idx: 0,
            name: "ws1".into(),
            custom_name: None,
            collapsed: false,
            state: SidebarItemState::Active,
            columns: vec![ColumnEntry {
                col_idx: 0,
                collapsed: false,
                panes: vec![PaneEntry {
                    pane_id: PaneId(1),
                    name: "pane1".into(),
                    custom_name: None,
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });
        tree
    }

    fn store() -> SharedChromeState {
        let store = SharedChromeState::new(300.0, true, 300.0, false);
        store.workspaces.set_active_pane(Some(PaneId(1)));
        store
    }

    fn container(p: &WorkspacesContainerProvider, ctx: &ChromeCtx<'_>) -> ContainerContribution {
        match p.build_contribution(ctx) {
            Contribution::Container(c) => c,
            _ => panic!("the workspaces provider contributes a Container"),
        }
    }

    /// The same tree with a **floating** pane beside the tiled one, and a renamed workspace — the
    /// two facts the row menus have to read off this component's own model.
    fn tree_with_a_floating_pane() -> WorkspaceTree {
        let mut tree = tree();
        let ws = &mut tree.workspaces[0];
        ws.name = "My WS".into();
        ws.custom_name = Some("My WS".into());
        ws.floating_panes.push(PaneEntry {
            pane_id: PaneId(9),
            name: "float".into(),
            custom_name: None,
            state: SidebarItemState::None,
        });
        tree.sync_flat_items();
        tree
    }

    /// A store whose model is the little tree above, with `mount` holding the keyboard.
    fn store_with_tree(mount: &str) -> SharedChromeState {
        let store = store();
        *store.workspaces.tree_mut() = tree();
        store.workspaces.tree_mut().sync_flat_items();
        store.set_focused_container(Some(mount.to_string()));
        store
    }

    // ── What the component declares (F003/P085/T356) ──

    #[test]
    fn it_declares_only_what_acts_on_its_own_cursor() {
        let declared: Vec<String> = WorkspacesContainerProvider::new()
            .actions()
            .into_iter()
            .map(|m| m.name)
            .collect();

        for id in [
            CURSOR_UP,
            CURSOR_DOWN,
            COLLAPSE_ROW,
            ACTIVATE_SELECTED,
            PEEK_SELECTED,
            TOGGLE_SELECTED,
        ] {
            assert!(declared.contains(&id.to_string()), "{id} is declared");
        }
        assert!(
            declared.iter().all(|n| n.starts_with("workspaces.")),
            "every id is namespaced by kind(), so two components can both have a cursor: {declared:?}",
        );
        // The point of the contract: it redeclares nothing that already exists.
        for existing in ["zoom_column", "close", "next_pane", "create_workspace"] {
            assert!(
                !declared.iter().any(|n| n == existing),
                "{existing} already exists — bind it, do not redeclare it",
            );
        }
    }

    // ── What `perform` does with it ──

    #[test]
    fn the_cursor_moves_over_the_components_own_rows() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();
        let mut cx = ProviderCx::new("workspaces", store.clone());

        let first = store.workspaces.tree().cursor;
        assert_eq!(
            p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut cx),
            Handled::Yes,
        );
        assert_ne!(store.workspaces.tree().cursor, first, "the cursor moved");

        p.perform(CURSOR_UP, &Intent::new(CURSOR_UP), &mut cx);
        assert_eq!(store.workspaces.tree().cursor, first, "and moved back");
    }

    /// Moving the cursor publishes it, so the row outlines follow with no rebuild — and it lands on
    /// **this mount's** cursor, not a global one.
    #[test]
    fn moving_the_cursor_publishes_it_for_this_mount() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();
        let mut cx = ProviderCx::new("workspaces", store.clone());

        p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut cx);

        assert!(cx.selected().is_some(), "the mount's cursor names a row");
        assert!(
            store.workspaces.nav_selection().is_some(),
            "and the domain-typed selection beside it still agrees",
        );
        assert_eq!(
            ProviderCx::new("workspaces.right", store).selected(),
            None,
            "the other placement of the same component has its own cursor",
        );
    }

    /// Activating a leaf asks the host to focus the pane **and** to hand the keyboard back; peeking
    /// asks only for the focus. Both go out as queued intents — a component never mutates app state.
    #[test]
    fn activating_a_row_asks_the_host_rather_than_acting() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();

        let mut cx = ProviderCx::new("workspaces", store.clone());
        p.perform(ACTIVATE_SELECTED, &Intent::new(ACTIVATE_SELECTED), &mut cx);
        let asked: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
        assert!(asked.contains(&"focus_pane".to_string()) || asked.contains(&"focus_workspace".to_string()));

        let mut cx = ProviderCx::new("workspaces", store);
        p.perform(PEEK_SELECTED, &Intent::new(PEEK_SELECTED), &mut cx);
        let asked: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
        assert!(
            !asked.contains(&"unfocus_dock".to_string()),
            "peek keeps the keyboard on the dock: {asked:?}",
        );
    }

    /// **The cursor and the click are the same thing** (F003/P086/T365): a click moves the tree's
    /// own cursor, so the next `j` continues from the clicked row rather than from wherever the
    /// cursor was before.
    ///
    /// The resolution matches this component's own keys against its own rows — never parses one —
    /// so an unknown key is a no-op rather than a guess.
    #[test]
    fn a_click_moves_the_cursor_so_stepping_continues_from_it() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();
        let mut cx = ProviderCx::new("workspaces", store.clone());

        // The fixture's rows are the workspace and its one pane; aim at the pane.
        let pane_key = pane_nav_key(PaneId(1));
        p.cursor_moved(&pane_key, &mut cx);

        assert!(
            matches!(
                store.workspaces.tree().current_item(),
                Some(WorkspaceRow::Pane { pane_id: PaneId(1) }),
            ),
            "the tree's own cursor moved, not just the highlight",
        );
        assert_eq!(
            store.workspaces.nav_selection(),
            Some(crate::chrome::SidebarSelection::Pane { pane_id: PaneId(1) }),
            "and the selection that survives a rebuild agrees",
        );

        // Stepping now continues from the clicked row.
        p.perform(CURSOR_UP, &Intent::new(CURSOR_UP), &mut cx);
        assert!(
            matches!(
                store.workspaces.tree().current_item(),
                Some(WorkspaceRow::Workspace { .. }),
            ),
            "`k` went to the row above the one clicked",
        );
    }

    #[test]
    fn a_key_this_component_did_not_write_leaves_the_cursor_alone() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();
        let mut cx = ProviderCx::new("workspaces", store.clone());
        let before = store.workspaces.tree().cursor;

        p.cursor_moved("docker:container:abc", &mut cx);

        assert_eq!(store.workspaces.tree().cursor, before, "no guess, no move");
    }

    /// The `Space` regression, pinned at the only level a unit test can reach (F003/P086/T364).
    ///
    /// The two verbs differ by **one queued intent** and nothing else: activate asks for
    /// `unfocus_dock`, peek does not. F003/P085/T352 instead made the *host* release container focus
    /// inside `handle_focus_pane`, so peek's `focus_pane` released it too and `j`/`k` stopped. That
    /// rule is gone; whether the keyboard leaves is decided here, by what the user asked for.
    ///
    /// The end of the story — the keyboard actually staying on the dock — needs an `AppState`, which
    /// cannot be built without a window. The user drives that half in the app.
    #[test]
    fn peek_and_activate_differ_only_by_the_release_they_ask_for() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();

        // Park the cursor on the pane: the workspace row takes the `focus_workspace` arm, which
        // never releases either way, so it cannot tell the two verbs apart.
        let mut cx = ProviderCx::new("workspaces", store.clone());
        p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut cx);
        cx.drain();
        assert!(
            matches!(
                store.workspaces.tree().current_item(),
                Some(WorkspaceRow::Pane { .. }),
            ),
            "the fixture's second navigable row is the pane",
        );

        p.perform(PEEK_SELECTED, &Intent::new(PEEK_SELECTED), &mut cx);
        let peeked: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
        assert!(
            peeked.contains(&"focus_pane".to_string()),
            "peek still brings the pane to the front: {peeked:?}",
        );
        assert!(
            !peeked.contains(&"unfocus_dock".to_string()),
            "…and asks for no release, so j/k keep working: {peeked:?}",
        );
        assert_eq!(
            store.focused_container(),
            Some("workspaces".to_string()),
            "peek leaves the container holding the keyboard",
        );

        p.perform(ACTIVATE_SELECTED, &Intent::new(ACTIVATE_SELECTED), &mut cx);
        let activated: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
        assert!(
            activated.contains(&"focus_pane".to_string())
                && activated.contains(&"unfocus_dock".to_string()),
            "activate-and-leave asks for the release itself: {activated:?}",
        );
    }

    /// **An aimed peek lands on the row that was picked, not on the cursor** (F004/P084/T399).
    ///
    /// This is the half `prefix+/` needs: the picker knows which letter was chosen, so it names
    /// that row by its nav key and the peek acts there. Without the argument the verb could only
    /// ever mean "the row I am already on", which is why the picker used to be pointed at the
    /// row's *click* instead — and that click leaves the sidebar.
    #[test]
    fn a_peek_can_be_aimed_at_a_row_by_key_and_moves_the_cursor_there() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();
        let mut cx = ProviderCx::new("workspaces", store.clone());

        // The cursor starts on the workspace row; aim at the pane row instead.
        let pane_id = match store.workspaces.tree().flat_items.iter().find_map(|row| match row {
            WorkspaceRow::Pane { pane_id } => Some(*pane_id),
            _ => None,
        }) {
            Some(id) => id,
            None => panic!("the fixture has a pane row"),
        };
        let aimed = Intent::new(PEEK_SELECTED)
            .arg("key", PropValue::Text(pane_nav_key(pane_id)));
        p.perform(PEEK_SELECTED, &aimed, &mut cx);

        assert!(
            matches!(
                store.workspaces.tree().current_item(),
                Some(WorkspaceRow::Pane { pane_id: on }) if *on == pane_id,
            ),
            "the cursor moved onto the row that was named",
        );
        let fired: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
        assert!(
            fired.contains(&"focus_pane".to_string())
                && !fired.contains(&"unfocus_dock".to_string()),
            "…and it peeks it: the pane comes forward, the dock keeps the keyboard: {fired:?}",
        );
    }

    /// **A peek from outside the dock takes the keyboard.** Landing the cursor on a row is only
    /// worth anything if `j`/`k` then move it, so the verb asks for the dock — once, and only when
    /// the dock does not already have it, because `focus_dock` aimed at the focused dock is the way
    /// back out and would release instead. Antonio, driving `prefix+/` on 2026-08-10: *"it
    /// activates the pane but the keyboard goes to the terminal"*.
    #[test]
    fn a_peek_from_outside_the_dock_asks_for_the_keyboard_and_from_inside_does_not() {
        let p = WorkspacesContainerProvider::new();

        // Nothing holds chrome focus (the keyboard is in a pane).
        let away = store();
        *away.workspaces.tree_mut() = tree();
        away.workspaces.tree_mut().sync_flat_items();
        let mut cx = ProviderCx::new("workspaces", away.clone());
        p.perform(PEEK_SELECTED, &Intent::new(PEEK_SELECTED), &mut cx);
        let asked: Vec<crate::chrome::Intent> = cx.drain();
        let focus_dock = asked.iter().find(|i| i.action == "focus_dock");
        assert_eq!(
            focus_dock.map(|i| i.args.get("dock").cloned()),
            Some(Some(PropValue::Text("workspaces".to_string()))),
            "it asks for its own mount: {asked:?}",
        );

        // The dock already holds it: asking again would toggle it off.
        let held = store_with_tree("workspaces");
        let mut cx = ProviderCx::new("workspaces", held.clone());
        p.perform(PEEK_SELECTED, &Intent::new(PEEK_SELECTED), &mut cx);
        let asked: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
        assert!(
            !asked.contains(&"focus_dock".to_string()),
            "focus_dock aimed at the focused dock releases it: {asked:?}",
        );
    }

    /// A key naming a row that is not there leaves the cursor alone rather than failing — a tree
    /// rebuilt under the letters is not an error.
    #[test]
    fn an_aimed_peek_with_a_stale_key_leaves_the_cursor_where_it_was() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();
        let mut cx = ProviderCx::new("workspaces", store.clone());
        let before = store.workspaces.tree().cursor;

        let stale = Intent::new(PEEK_SELECTED)
            .arg("key", PropValue::Text("pane:9999".to_string()));
        p.perform(PEEK_SELECTED, &stale, &mut cx);

        assert_eq!(store.workspaces.tree().cursor, before, "no guess, no move");
    }

    // ── This component's row menus (F003/P086/T365) ──

    /// A facade over a store — what a menu builder and `context_path` receive.
    fn ctx_over(store: &SharedChromeState) -> ChromeCtx<'_> {
        ChromeCtx::new(crate::host::App::new(store))
    }

    /// **A row's menu is built from this component's own model**, at the row: the pane's column
    /// (absent for a floating pane, which is in none) and whether the workspace has a name to
    /// clear. These are the facts the host used to resolve and hand over; now the row reads them
    /// where it is drawn, which is why there is no key to map and no path to declare.
    #[test]
    fn a_row_menu_reads_the_components_own_model() {
        let store = store();
        *store.workspaces.tree_mut() = tree_with_a_floating_pane();
        let tree = store.workspaces.tree();

        let tiled = pane_row_items(PaneId(1), tree.locate_pane(PaneId(1)), false);
        let new_pane = tiled
            .iter()
            .find(|i| i.id == "add_pane_to_column")
            .expect("a tiled pane is in a column");
        assert_eq!(new_pane.intent.args.get("ws_idx"), Some(&PropValue::Int(0)));
        assert_eq!(new_pane.intent.args.get("col_idx"), Some(&PropValue::Int(0)));

        // A floating pane is in no column, so `pane_card` passes `None`.
        let floating = pane_row_items(PaneId(9), None, false);
        assert!(
            !floating.iter().any(|i| i.id == "add_pane_to_column"),
            "a floating pane is in no column, so the entry that needs one is absent rather than \
             aimed at a guess: {:?}",
            floating.iter().map(|i| &i.id).collect::<Vec<_>>(),
        );
        assert!(floating.iter().any(|i| i.id == "close"), "it is still closable");

        let renamed = tree.workspaces.iter().find(|w| w.ws_idx == 0).expect("workspace 0");
        let ws = workspace_row_items(0, renamed.custom_name.is_some());
        assert!(
            ws.iter().any(|i| i.id == "reset_workspace_name"),
            "the workspace was renamed, so there is a name to clear",
        );
    }

    /// Renaming the row the cursor is on is **this component's** verb (user decision, 2026-07-30):
    /// it resolves the cursor and dispatches the by-id action for whichever kind of row it is. The
    /// app's `rename_pane` / `rename_workspace` keep meaning the focused pane / active workspace.
    #[test]
    fn rename_selected_targets_the_cursor_row_by_id() {
        let store = store_with_tree("workspaces");
        let p = WorkspacesContainerProvider::new();
        let mut cx = ProviderCx::new("workspaces", store.clone());
        let intent = Intent::new(RENAME_SELECTED);

        // The fixture's first row is the workspace header.
        assert_eq!(p.perform(RENAME_SELECTED, &intent, &mut cx), Handled::Yes);
        let asked = cx.drain();
        assert_eq!(asked[0].action, "rename_workspace_by_idx");
        assert_eq!(asked[0].args.get("ws_idx"), Some(&PropValue::Int(0)));

        // Step onto the pane row and the same key means that pane.
        p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut cx);
        cx.drain();
        assert_eq!(p.perform(RENAME_SELECTED, &intent, &mut cx), Handled::Yes);
        let asked = cx.drain();
        assert_eq!(asked[0].action, "rename_pane_by_id");
        assert_eq!(asked[0].args.get("pane_id"), Some(&PropValue::Int(1)));
    }

    #[test]
    fn an_action_this_component_does_not_own_declines() {
        let store = store_with_tree("workspaces");
        let mut cx = ProviderCx::new("workspaces", store);
        assert_eq!(
            WorkspacesContainerProvider::new().perform(
                "docker.restart_selected",
                &Intent::new("docker.restart_selected"),
                &mut cx,
            ),
            Handled::No,
        );
    }

    #[test]
    fn provider_metadata_is_self_consistent() {
        let p = WorkspacesContainerProvider::new();
        assert_eq!(p.id(), "workspaces");
        assert_eq!(p.title(), "Workspaces");
        assert_eq!(p.default_region(), RegionId::LeftSidebar);
        assert!(p.supported_regions().contains(RegionId::LeftSidebar));
        assert!(p.supported_regions().contains(RegionId::RightSidebar));
        assert_eq!(p.default_order(), 0);
        assert!(p.movable());
        assert!(p.collapsible());
    }

    #[test]
    fn register_seats_in_left_sidebar_at_order_zero() {
        let mut host = ChromeHost::new(ChromeEventBus::default());
        host.register(Box::new(WorkspacesContainerProvider::new()));
        // Seated in the left sidebar (its default_region), not the right.
        let left = host.contributions(RegionId::LeftSidebar);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id(), "workspaces");
        assert_eq!(left[0].title(), "Workspaces");
        assert!(host.contributions(RegionId::RightSidebar).is_empty());
        assert_eq!(host.placement("workspaces"), Some(RegionId::LeftSidebar));
    }

    #[test]
    fn contribution_carries_the_provider_metadata() {
        let p = WorkspacesContainerProvider::new();
        let ctx = ChromeCtx::new(crate::host::App::new(&store()));
        let c = container(&p, &ctx);
        assert_eq!(c.id, "workspaces");
        assert_eq!(c.title, "Workspaces");
        assert_eq!(c.default_region, RegionId::LeftSidebar);
        assert_eq!(c.default_order, 0);
        assert!(c.supported_regions.contains(RegionId::LeftSidebar));
        assert!(c.supported_regions.contains(RegionId::RightSidebar));
        assert!(c.movable);
        assert!(c.collapsible);
    }

    #[test]
    fn build_produces_the_real_workspace_tree_and_registers_host_ids() {
        // The seam builds the *same* body the bespoke path builds: a column with one
        // child per workspace, and — the part a placeholder could never fake — the
        // drag and hint ids the interactive rows need, allocated in the host's
        // registries.
        let p = WorkspacesContainerProvider::new();
        let theme = GuiTheme::default();
        let emit: ChromeIntentEmitter = Rc::new(|_| {});
        let store = store();
        // The model is the component's own, read off its state — not handed in by the host.
        *store.workspaces.tree_mut() = tree();

        let catalog = crate::actions::ActionCatalog::with_builtins();
        let ctx = ChromeCtx::for_build(crate::host::App::new(&store), &theme, &emit, &catalog);
        let c = container(&p, &ctx);

        let mut signals = ChromeSignals::default();
        let mut drag = DragItemRegistry::default();
        let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
        let body = (c.build)(&ctx, &mut bx);

        assert_eq!(
            body.base().children.len(),
            1,
            "one workspace in the tree ⇒ one workspace dock in the body",
        );
        // Drag ids: the workspace (drop target), its column, its pane.
        assert_eq!(drag.items().len(), 3);
        // Pick targets: the pane card and the workspace dock each declared what `prefix+/` does to
        // it. A column declares none — it only carries a pick-letter signal, stamped when it is a
        // *destination* for a move/swap.
        assert_eq!(heca_grid_ui::collect_peeks(body.as_ref()).len(), 2);
        // The active pane's card bound its `active` signal for per-frame updates.
        assert_eq!(signals.pane_active.len(), 1);
    }

    /// **A row's gesture is a NAME** (F003/P086/T365) — an action id plus arguments, so a menu
    /// entry, a keybinding and RPC all reach the same thing a click does. It used to be a closure
    /// emitting a host-side variant, which only the click could ever run.
    ///
    /// **And a pick is not a click** (F004/P084/T399): both rows aim `prefix+/` at this component's
    /// own `peek_selected`, carrying the picked row's nav key — *look at that one*, staying in the
    /// dock — where a click on the pane row means *go there and leave*.
    ///
    /// **Every gesture also names the seating it was declared in**, so nothing has to resolve the
    /// call back to an instance: see
    /// `a_gesture_names_the_seating_it_was_built_in_so_two_placements_answer_for_themselves`.
    #[test]
    fn a_rows_gesture_is_a_named_intent_and_a_pick_is_its_own_gesture() {
        use crate::app::interaction::InteractionIntent;
        let p = WorkspacesContainerProvider::new();
        let theme = GuiTheme::default();
        let fired: Rc<std::cell::RefCell<Vec<InteractionIntent>>> = Default::default();
        let emit: ChromeIntentEmitter = {
            let fired = fired.clone();
            Rc::new(move |intent| fired.borrow_mut().push(intent))
        };
        let store = store();
        *store.workspaces.tree_mut() = tree();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let ctx = ChromeCtx::for_build(crate::host::App::new(&store), &theme, &emit, &catalog);
        let c = container(&p, &ctx);

        let mut signals = ChromeSignals::default();
        let mut drag = DragItemRegistry::default();
        let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
        let body = (c.build)(&ctx, &mut bx);

        // Every pick declaration in the built body, run in document order.
        for (path, _) in heca_grid_ui::collect_peeks(body.as_ref()) {
            assert!(heca_grid_ui::fire_peek(body.as_ref(), &path));
        }
        let declared: Vec<(String, Option<PropValue>)> = fired
            .borrow()
            .iter()
            .map(|intent| match intent {
                InteractionIntent::View(vi) => {
                    (vi.action.clone(), vi.args.get("key").cloned())
                }
                other => panic!("a row's gesture must be a named intent, got {other:?}"),
            })
            .collect();
        assert_eq!(
            declared,
            vec![
                // Document order: the workspace row, then the pane rows inside it.
                (
                    "workspaces.peek_selected".to_string(),
                    Some(PropValue::Text("ws:0".to_string())),
                ),
                (
                    "workspaces.peek_selected".to_string(),
                    Some(PropValue::Text("pane:1".to_string())),
                ),
            ],
            "each row aims the picker at its own row, by nav key",
        );
    }

    /// **A gesture names the seating it was built in** (F004/P084/T399), so the same container
    /// mounted twice has two rows that each answer for themselves — the way two `<div>`s on a page
    /// do, rather than one being resolved back to "whichever instance has focus".
    ///
    /// Without it the host guessed the owner (`owning_mount`: the focused seating, else the last
    /// focused, else the first that declares the name), so a right-sidebar row's letter was
    /// performed by the left-sidebar copy — with the wrong dock taking the keyboard.
    #[test]
    fn a_gesture_names_the_seating_it_was_built_in_so_two_placements_answer_for_themselves() {
        use crate::app::interaction::InteractionIntent;
        use crate::providers::SEAT_ARG;

        let seats = |mount: &str| {
            let p = WorkspacesContainerProvider::new();
            let theme = GuiTheme::default();
            let fired: Rc<std::cell::RefCell<Vec<InteractionIntent>>> = Default::default();
            let emit: ChromeIntentEmitter = {
                let fired = fired.clone();
                Rc::new(move |intent| fired.borrow_mut().push(intent))
            };
            let store = store();
            *store.workspaces.tree_mut() = tree();
            let catalog = crate::actions::ActionCatalog::with_builtins();
            let ctx = ChromeCtx::for_build(crate::host::App::new(&store), &theme, &emit, &catalog);
            let c = container(&p, &ctx);
            let mut signals = ChromeSignals::default();
            let mut drag = DragItemRegistry::default();
            let mut bx = BuildCx::new(mount, &mut signals, &mut drag);
            let body = (c.build)(&ctx, &mut bx);
            for (path, _) in heca_grid_ui::collect_peeks(body.as_ref()) {
                assert!(heca_grid_ui::fire_peek(body.as_ref(), &path));
            }
            let seats: Vec<PropValue> = fired
                .borrow()
                .iter()
                .map(|intent| match intent {
                    InteractionIntent::View(vi) => vi.args.get(SEAT_ARG).cloned().unwrap_or_else(
                        || panic!("a gesture must name its seating, got {:?}", vi.args),
                    ),
                    other => panic!("a row's gesture must be a named intent, got {other:?}"),
                })
                .collect();
            assert!(!seats.is_empty(), "the body declared at least one gesture");
            seats
        };

        for (mount, expected) in [("dock.left", "dock.left"), ("dock.right", "dock.right")] {
            for seat in seats(mount) {
                assert_eq!(seat, PropValue::Text(expected.to_string()));
            }
        }
    }

    #[test]
    fn build_outside_a_render_pass_yields_an_empty_body() {
        // An observe-only context has no tree/theme/emitter to project. Building is
        // still legal (a host may want the metadata) and must not panic.
        let p = WorkspacesContainerProvider::new();
        let store = store();
        let ctx = ChromeCtx::new(crate::host::App::new(&store));
        let c = container(&p, &ctx);

        let mut signals = ChromeSignals::default();
        let mut drag = DragItemRegistry::default();
        let mut bx = BuildCx::new("workspaces", &mut signals, &mut drag);
        let body = (c.build)(&ctx, &mut bx);

        assert!(body.base().children.is_empty());
        assert!(drag.items().is_empty());
        assert!(heca_grid_ui::collect_peeks(body.as_ref()).is_empty());
    }
}
