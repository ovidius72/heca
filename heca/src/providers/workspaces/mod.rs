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

mod model;

pub(crate) use model::{ColumnEntry, PaneEntry, WorkspaceRow, WorkspaceTree};
#[cfg(test)]
pub(crate) use model::WorkspaceEntry;

#[cfg(test)]
mod model_tests;

use crate::chrome::{
    alpha_u8, home_relative_path, pane_info_view, runtime_snapshot, truncate_sidebar_git_branch,
    BuildCx, ChromeDragItem, ChromeIntentEmitter, ChromeSignals, ContainerContribution,
    Contribution, DragItemRegistry, PaneInfoSignals, RegionId, RegionSet,
    RepaintWatch, WidgetModel, WorkspacesContainerState, CARD_META_FONT_SCALE,
};
use crate::actions::{ActionCategory, ActionMeta, ArgKind, ArgSpec};
use crate::app::interaction::ActionPolicy;
use crate::chrome::{Intent, PropMap, PropValue};
// The menu-entry vocabulary, shared with the host's own pane menu (F003/P086/T365).
use crate::chrome::context_menu::{item, item_running, usize_arg};
use crate::chrome::DropdownItem;
use crate::providers::{ChromeCtx, Provider, ProviderCx};
use heca_grid_ui::Handled;
use heca_config::programs::ProgramsConfig;
use heca_core::layout::PaneId;
use heca_core::runtime::ProcessStatus;
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::reactive::{signal, Signal, SignalGet, SignalUpdate};
use heca_grid_ui::style::{Align, Length};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    Badge, DockFrame, Flex, Glyph, HintPlacement, Icon, KeyHint, Label, MarkerGroup,
    Row, ScrollRegion, StatusDot, Tooltip, TooltipSide, Visibility,
};

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
    Box::new(build_workspaces_container(
        ctx,
        &tree,
        &programs,
        theme,
        emit,
        state.workspaces(),
        // This placement's id — the rows' cursor signals are keyed by it, so two seatings of one
        // container light different rows (F003/P085/T354).
        bx.container_id().to_string(),
        scroll,
        focused,
        bx.signals,
        bx.drag,
    ))
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

/// A single pane **card**, styled like the showcase PANES rows: a state-tinted
/// background + radius, an active accent bar, a leading program icon, the display
/// name, an optional exceptional-state indicator, and git metadata when present.
#[expect(
    clippy::too_many_arguments,
    reason = "pane-card projection still threads host/runtime context explicitly; phase-local fix before a larger ChromeCx refactor"
)]
fn pane_card(
    ctx: &ChromeCtx<'_>,
    pane: &PaneEntry,
    // Which column this pane sits in, or `None` for a **floating** pane — which is in no column,
    // so its menu simply lacks the entries that need one rather than aiming them at a guess.
    column_of_pane: Option<(usize, usize)>,
    // The placement whose cursor this card's outline follows.
    mount: &str,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    active_pane: Option<PaneId>,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
) -> RepaintWatch {
    let active = active_pane == Some(pane.pane_id);
    let pane_id = pane.pane_id;
    // Read off this row's own projection rather than back through the store: the projection
    // was built from it, and the menu is about the row being drawn.
    let has_custom_name = pane.custom_name.is_some();
    let runtime = runtime_snapshot(ws_state, pane_id);
    let info = pane_info_view(
        programs,
        &pane.name,
        pane.custom_name.as_deref(),
        runtime.as_ref(),
        // A renamed pane shows a dimmed `(process)` suffix here when the setting is on,
        // so the sidebar keeps surfacing what's actually running under a custom name.
        ws_state.pane_renamed_add_process_name(),
    );
    // A constant theme-driven card; the *selected* look (accent pill + border + bar)
    // is drawn by `Row` from its `active` signal, not baked into the background. This
    // keeps styling fully signal-driven (active flips in place via `sync_chrome_signals`,
    // no tree rebuild) and theme-driven (no ad-hoc per-state alphas).
    // The card is both a drag source and a drop target (F4.5); its opaque DragItemId
    // is assigned by the registry (which records that it's this pane) so the kind
    // round-trips through `drag::source_at`/`resolve_at` without trusting raw ids.
    let drag_id = drag.register(ChromeDragItem::Pane(pane_id));
    // This row kind's declared click, by name — so a menu entry, a keybinding or RPC can fire the
    // same one (F003/P086/T365). A **pick** is declared separately, on the wrapper below: it is a
    // different gesture and this row answers it differently.
    let press = crate::chrome::fires(mount, pane_row_press(pane_id), emit_intent);
    let icon_widget = Icon::new(info.icon).size(14.0).color(theme.colors.foreground);
    let icon_signal = icon_widget.glyph_signal();
    let active_title_label = Label::new(info.title.clone())
        .color(theme.colors.accent)
        .bold(true);
    let active_title_signal = active_title_label.text_signal();
    let active_title = Visibility::new(active_title_label, active);
    let active_title_visible = active_title.visible_signal();
    let inactive_title_label = Label::new(info.title.clone())
        .color(theme.colors.foreground)
        .bold(true);
    let inactive_title_signal = inactive_title_label.text_signal();
    let inactive_title = Visibility::new(inactive_title_label, !active);
    let inactive_title_visible = inactive_title.visible_signal();
    let idle_dot = Visibility::new(StatusDot::offline(), info.status == ProcessStatus::Idle);
    let idle_dot_visible = idle_dot.visible_signal();
    let running_dot = Visibility::new(StatusDot::online(), info.status == ProcessStatus::Running);
    let running_dot_visible = running_dot.visible_signal();
    let success_dot = Visibility::new(StatusDot::online(), info.status == ProcessStatus::Success);
    let success_dot_visible = success_dot.visible_signal();
    let error_dot = Visibility::new(StatusDot::error(), info.status == ProcessStatus::Error);
    let error_dot_visible = error_dot.visible_signal();
    let branch_label_widget = Label::new(truncate_sidebar_git_branch(
        info.git_branch.as_deref().unwrap_or_default(),
    ))
    .color(theme.colors.foreground)
    .font_scale(0.8);
    let branch_display_signal = branch_label_widget.text_signal();
    let branch_signal = signal(info.git_branch.clone().unwrap_or_default());
    let add_label_widget = Label::new(info.git_added.clone().unwrap_or_default())
        .color(theme.colors.success)
        .font_scale(0.8);
    let add_label = add_label_widget.text_signal();
    let add_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Plus).size(12.0).color(theme.colors.success))
            .child(add_label_widget),
        info.git_added.is_some(),
    );
    let add_text_visible_signal = add_segment.visible_signal();
    let modified_label_widget = Label::new(info.git_modified.clone().unwrap_or_default())
        .color(theme.colors.warning)
        .font_scale(0.8);
    let modified_label = modified_label_widget.text_signal();
    let modified_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Warning).size(12.0).color(theme.colors.warning))
            .child(modified_label_widget),
        info.git_modified.is_some(),
    );
    let modified_text_visible_signal = modified_segment.visible_signal();
    let deleted_label_widget = Label::new(info.git_deleted.clone().unwrap_or_default())
        .color(theme.colors.danger)
        .font_scale(0.8);
    let deleted_label = deleted_label_widget.text_signal();
    let deleted_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Minus).size(12.0).color(theme.colors.danger))
            .child(deleted_label_widget),
        info.git_deleted.is_some(),
    );
    let deleted_text_visible_signal = deleted_segment.visible_signal();
    let git_row = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(6.0)
            .child(Icon::new(Glyph::GitBranch).size(12.0).color(theme.colors.warning))
            .child(
                Tooltip::new_signal(branch_label_widget, branch_signal)
                    .side(TooltipSide::Bottom)
                    .delay(0.25),
            )
            .child(add_segment)
            .child(modified_segment)
            .child(deleted_segment),
        info.git_branch.is_some(),
    );
    let git_visible_signal = git_row.visible_signal();
    // A renamed pane surfaces its running program as a dimmed `(process)` suffix after
    // the name (config-gated). Appended to the shared title area so it renders the same
    // in both the git and no-git card layouts. Like the title, it is **signal-driven**
    // (text + visibility updated in `sync_chrome_signals`) so renaming toggles it live —
    // a rename updates signals, it does not rebuild the sidebar card tree.
    let process_hint_text = info
        .process_hint
        .as_ref()
        .map(|program| format!("({program})"))
        .unwrap_or_default();
    // `foreground` (not `muted`) so it's readable on every theme; it still reads as
    // secondary next to the accent + bold name (regular weight, smaller scale).
    let process_hint_label = Label::new(process_hint_text)
        .color(theme.colors.foreground)
        .font_scale(CARD_META_FONT_SCALE);
    let process_hint_signal = process_hint_label.text_signal();
    let process_hint = Visibility::new(process_hint_label, info.process_hint.is_some());
    let process_hint_visible = process_hint.visible_signal();
    let title_area = Flex::row()
        .align(Align::Center)
        .gap(4.0)
        .child(Flex::column().child(active_title).child(inactive_title))
        .child(process_hint);
    // Optional cwd row (folder icon + home-relative path), stacked between the name and
    // git rows. Signal-driven like the git branch: the path updates live on `cd`, and the
    // row's visibility follows `[settings] pane_show_cwd` and whether the pane has a cwd.
    let cwd_path = runtime.as_ref().and_then(|rt| rt.cwd.clone());
    let show_cwd = ws_state.pane_show_cwd() && cwd_path.is_some();
    let cwd_text = cwd_path
        .as_deref()
        .map(home_relative_path)
        .unwrap_or_default();
    // Readable, matching the sibling git-branch row (which colors its label
    // `foreground`); `muted` was too dim for a primary info row.
    let cwd_label = Label::new(cwd_text)
        .color(theme.colors.foreground)
        .font_scale(CARD_META_FONT_SCALE);
    let cwd_signal = cwd_label.text_signal();
    let cwd_row = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(6.0)
            .child(Icon::new(Glyph::Folder).size(12.0).color(theme.colors.foreground))
            .child(cwd_label),
        show_cwd,
    );
    let cwd_visible_signal = cwd_row.visible_signal();
    // The pane's identity row (status dots + program icon + name) — shared by every card
    // layout so the cwd and git rows just stack beneath it in one column.
    let name_row = Flex::row()
        .align(Align::Center)
        .gap(8.0)
        .child(
            Flex::row()
                .align(Align::Center)
                .width(Length::Px(12.0))
                .child(idle_dot)
                .child(running_dot)
                .child(success_dot)
                .child(error_dot),
        )
        .child(Flex::row().align(Align::Center).child(icon_widget))
        .child(title_area);
    // One column: the name row, then the optional cwd and git rows (each 2px-indented and
    // added only when shown, mirroring the git row). A card with only the name row lays
    // out exactly like the former single-row layout — a one-child column adds no gap.
    let mut content = Flex::column().gap(4.0).grow(1.0).child(name_row);
    if show_cwd {
        content = content.child(
            Flex::row()
                .child(Flex::row().width(Length::Px(2.0)))
                .child(cwd_row),
        );
    }
    if info.git_branch.is_some() {
        content = content.child(
            Flex::row()
                .child(Flex::row().width(Length::Px(2.0)))
                .child(git_row),
        );
    }
    let card = Row::new()
        .background(
            theme
                .colors
                .foreground
                .with_alpha(alpha_u8(theme.colors.card_background_alpha)),
        )
                .radius(theme.colors.control_radius())
        .padding(6.0)
        // No bar: the column's `MarkerGroup` already draws one down the left of every row here,
        // and the selected panel says which row is current. Two lines said it twice.
        .active(active)
        .nav_selected(false)
        // The row's ONE identity: the cursor and (later) drag read this single declaration
        // (F003/P085/T354). **The right-click no longer does** — the menu is declared below, on
        // this widget, so a row that forgets `nav_key` still opens its menu (F004/P084/T395).
        .nav_key(pane_nav_key(pane_id))
        // **This row's menu, built where this row's data is.** No `context_path`, no registered
        // builder, no menu-id string: the entries capture `pane_id` from the loop that is already
        // drawing it. Built at trigger time, so "Use process name" appears exactly when there is a
        // custom name to clear.
        .context_menu({
            let catalog = ctx.action_catalog().expect("a render pass has the catalog");
            let ctx_menu = crate::chrome::context_menu::menu_from_items(
                "Pane",
                "What you can do with this pane",
                MENU_PANE,
                pane_row_items(pane_id, column_of_pane, has_custom_name),
                catalog,
                emit_intent,
            );
            heca_grid_ui::widgets::ContextMenu::new(MENU_PANE).child(ctx_menu)
        })
        .draggable(drag_id)
        .drop_target(drag_id)
        // Click / Enter / Space run the row's declared click.
        .on_activate(press)
        .child(content);
    // Bind the card's active signal so focus changes update it without a rebuild.
    signals.pane_active.push((pane_id, card.state()));
    signals.pane_previous.push((pane_id, card.previous_state()));
    signals.row_nav
        .push((mount.to_string(), pane_nav_key(pane_id), card.nav_state()));
    // Wrap the card in a universal `KeyHint` so a move/swap/take pick can stamp this
    // pane's letter over it. `KeyHint` is transparent — it hugs the child and routes
    // events/focus/drag straight through — so the card stays a drag source + target
    // and clickable. The hint signal is driven each frame in `sync_chrome_signals`
    // from the active `InputMode` candidates (keyboard logic stays the source of truth).
    let hint = signal(None);
    signals.pane_hint.push((pane_id, hint));
    signals.pane_info.push((
        pane_id,
        PaneInfoSignals {
            icon: icon_signal,
            title_active: active_title_signal,
            title_inactive: inactive_title_signal,
            title_active_visible: active_title_visible,
            title_inactive_visible: inactive_title_visible,
            process_hint: process_hint_signal,
            process_hint_visible,
            cwd: cwd_signal,
            cwd_visible: cwd_visible_signal,
            status_idle_visible: idle_dot_visible,
            status_running_visible: running_dot_visible,
            status_success_visible: success_dot_visible,
            status_error_visible: error_dot_visible,
            git_visible: git_visible_signal,
            git_branch: branch_signal,
            git_branch_display: branch_display_signal,
            git_added_visible: add_text_visible_signal,
            git_added: add_label,
            git_modified_visible: modified_text_visible_signal,
            git_modified: modified_label,
            git_deleted_visible: deleted_text_visible_signal,
            git_deleted: deleted_label,
        },
    ));
    let (watch, _repaint) = RepaintWatch::new(
        KeyHint::new(card)
            .hint(hint)
            // **What `prefix+/` does to this row**, declared right where its letter is drawn: move
            // the cursor here and bring the pane to the front, staying in the sidebar. Nothing is
            // registered and no id leaves this line — which is the only reason a plugin's row could
            // ever have the same picker (RULE ZERO, F004/P084/T399).
            .on_peek(crate::chrome::fires(
                mount,
                row_peek(pane_nav_key(pane_id)),
                emit_intent,
            ))
            .placement(HintPlacement::CenterRight),
    );
    watch
}

/// One **column**: a generic [`MarkerGroup`] (left marker bar + grip gutter) holding
/// the column's stacked pane cards — no per-column header row (columns are spatial
/// groupings whose only user-facing job is to be a move/swap target + drag handle).
/// The `MarkerGroup` bar brightens to the accent when the column holds the active
/// pane, and its grip gutter is the seam for the future move/swap [`KeyHint`] target
/// and DnD drag handle (F4.4/F4.5) — applied by the host via `KeyHint`/`ComponentExt`, not
/// baked into the widget.
#[expect(
    clippy::too_many_arguments,
    reason = "column projection still threads host/runtime context explicitly; phase-local fix before a larger ChromeCx refactor"
)]
fn column_view(
    ctx: &ChromeCtx<'_>,
    c: &ColumnEntry,
    ws_idx: usize,
    mount: &str,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    active_pane: Option<PaneId>,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
) -> RepaintWatch {
    let active = c.panes.iter().any(|p| active_pane == Some(p.pane_id));
    // The MarkerGroup is a column drag source + drop target (F4.5 step 2). Its grip
    // gutter is the only surface not covered by a child pane card, so innermost-first
    // hit-testing routes a grip press → column and a card press → pane, for free.
    let drag_id = drag.register(ChromeDragItem::Column {
        ws: ws_idx,
        col: c.col_idx,
    });
    let mut col = MarkerGroup::new()
        .active(active)
        .gap(3.0)
        .nav_key(column_nav_key(ws_idx, c.col_idx))
        .context_menu({
            let catalog = ctx.action_catalog().expect("a render pass has the catalog");
            let menu = crate::chrome::context_menu::menu_from_items(
                "Column",
                "What you can do with this column",
                MENU_COLUMN,
                column_row_items(ws_idx, c.col_idx),
                catalog,
                emit_intent,
            );
            heca_grid_ui::widgets::ContextMenu::new(MENU_COLUMN).child(menu)
        })
        .draggable(drag_id)
        .drop_target(drag_id);
    for pane in &c.panes {
        col = col.child(pane_card(
            ctx,
            pane,
            Some((ws_idx, c.col_idx)),
            mount,
            programs,
            theme,
            emit_intent,
            active_pane,
            ws_state,
            signals,
            drag,
        ));
    }
    // Bind the column bar's active signal (lit iff it holds the active pane).
    let pane_ids = c.panes.iter().map(|p| p.pane_id).collect::<Vec<_>>();
    signals.col_active.push((pane_ids, col.state()));
    signals.row_nav.push((
        mount.to_string(),
        column_nav_key(ws_idx, c.col_idx),
        col.nav_state(),
    ));
    // Wrap the column in the universal `KeyHint` so a "move pane → column" pick can
    // stamp this column's letter over it (tinted `success`, distinct from pane/workspace
    // picks). Driven each frame in `sync_chrome_signals`.
    let col_hint = signal::<Option<String>>(None);
    signals.col_hint.push((ws_idx, c.col_idx, col_hint));
    let hinted = KeyHint::new(col)
        .hint(col_hint)
        .color(theme.colors.success)
        .placement(HintPlacement::CenterRight);
    let (watch, _repaint) = RepaintWatch::new(hinted);
    watch
}

/// Build the **WorkspacesContainer** content — the workspace tree mounted inside the
/// sidebar shell (see `heca-sidebar-design-spec`). Each workspace is a `.frameless(true)`
/// [`DockFrame`] (header count [`Badge`] = total panes); its columns are compact
/// [`column_view`]s (left marker bar + pane cards, no "Col N" header rows — those ate
/// the sidebar for no user value). Pure projection of the [`WorkspaceTree`].
///
/// This is the **body of the `workspaces` container** — what [`build_body`], the
/// provider's render seam, returns. It is reached only through that seam: the region
/// asks its mounted provider for a contribution and calls the contribution's `build`.
/// Nothing in `chrome` calls it any more.
#[expect(
    clippy::too_many_arguments,
    reason = "workspace-container projection threads host/runtime context + the drag and hint registries explicitly; phase-local before a larger ChromeCx refactor"
)]
fn build_workspaces_container(
    // The build context, for the one thing a row cannot answer itself: what an action **looks
    // like** (`ctx.action_icon`). Threaded to the rows because a row declares its own menu now
    // (F004/P084/T395) instead of the host resolving one from a position.
    ctx: &ChromeCtx<'_>,
    tree: &WorkspaceTree,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    ws_state: &WorkspacesContainerState,
    // This placement's mount id: every row's cursor signal is registered under it.
    mount: String,
    // This placement's own scroll offset (see `build_body`): per mount, so the same container
    // placed twice keeps two positions.
    scroll: Signal<f32>,
    // Whether this placement holds chrome keyboard focus (see `build_body`).
    focused: Signal<bool>,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
) -> ScrollRegion {
    // Selection is sourced from the container's shared state (the Phase-2 boundary),
    // not from `Session`/`SidebarItemState`. A workspace is "active" iff it hosts the
    // active pane.
    let active_pane = ws_state.active_pane();
    let mut col = Flex::column().gap(6.0).grow(1.0);
    for ws in &tree.workspaces {
        let pane_count =
            ws.columns.iter().map(|c| c.panes.len()).sum::<usize>() + ws.floating_panes.len();
        let active_ws = active_pane.is_some_and(|pid| {
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
        let ws_idx = ws.ws_idx;
        let emit = emit_intent.clone();
        let mut dock = DockFrame::new(ws.name.clone())
            .frameless(true)
            .gap(4.0) // tighten the workspace header → body spacing
            .expanded(!ws_state.is_ws_collapsed(ws_idx))
            .on_toggle(move |_| {
                emit(
                    crate::app::interaction::InteractionIntent::ToggleWorkspaceCollapsed { ws_idx },
                );
            })
            .header(
                Flex::row()
                    .align(Align::Center)
                    .child(badge)
                    .child(Flex::row().width(Length::Px(6.0))),
            );
        // Light accent wash over the whole active workspace area (+ the accent
        // count badge) makes the active workspace clearly prominent. Signal-driven
        // (like the pane/column highlights) so it flips in place via
        // `sync_chrome_signals` instead of forcing a tree rebuild; the alpha is the
        // theme's `active_wash_alpha` token, not a baked-in literal.
        dock = dock.active(active_ws);
        dock = dock.nav_selected(false);
        // **The row's one identity, declared like every other row's.** A pane card and a column
        // both call `nav_key`, and `nav_key_at` — which is how a right-click finds out what it
        // landed on — reads exactly that. The workspace header pushed its key into the signal list
        // and never told the widget, so the hit-test found nothing there: right-clicking a pane or
        // a column opened its menu, a workspace opened none (Antonio, 2026-08-05).
        dock = dock.nav_key(workspace_nav_key(ws_idx));
        // The workspace row's own menu, declared like the other two. The bug in the comment above
        // is the reason this phase exists: a menu resolved from a *position* needs a declaration
        // nobody remembers to write, and this one is the declaration itself.
        dock = dock.context_menu({
            let catalog = ctx.action_catalog().expect("a render pass has the catalog");
            let menu = crate::chrome::context_menu::menu_from_items(
                "Workspace",
                "What you can do with this workspace",
                MENU_WORKSPACE,
                workspace_row_items(ws_idx, ws.custom_name.is_some()),
                catalog,
                emit_intent,
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
        signals.ws_active.push((ws_pane_ids, dock.active_state()));
        signals.ws_previous.push((ws_idx, dock.previous_state()));
        signals.row_nav
            .push((mount.clone(), workspace_nav_key(ws_idx), dock.nav_state()));
        // The whole workspace is a column drop target (F4.5 step 2 scope C): dropping a
        // column anywhere on it that isn't a deeper column/pane target moves the column
        // into this workspace. Innermost-first hit-testing lets columns/panes override.
        dock = dock.drop_target(drag.register(ChromeDragItem::Workspace { ws: ws_idx }));
        // Columns stacked with a clear gap between them (the gap + bar mark each
        // column); panes inside a column are tight. Floating panes have no column.
        let mut cols = Flex::column().gap(8.0);
        for c in &ws.columns {
            cols = cols.child(column_view(
                ctx,
                c,
                ws_idx,
                &mount,
                programs,
                theme,
                emit_intent,
                active_pane,
                ws_state,
                signals,
                drag,
            ));
        }
        for float in &ws.floating_panes {
            cols = cols.child(pane_card(
                ctx,
                float,
                None,
                &mount,
                programs,
                theme,
                emit_intent,
                active_pane,
                ws_state,
                signals,
                drag,
            ));
        }
        dock = dock.child(cols);
        // Wrap the whole workspace dock in the universal `KeyHint` so a
        // "move column/pane → workspace" pick can stamp this workspace's letter over
        // it. The keycap is tinted `warning` (not accent) so a workspace target reads
        // distinctly from a pane target. The hint signal is driven each frame in
        // `sync_chrome_signals` from the active pick candidates.
        let ws_hint = signal::<Option<String>>(None);
        signals.ws_hint.push((ws_idx, ws_hint));
        col = col.child(
            KeyHint::new(dock)
                .hint(ws_hint)
                // **What `prefix+/` does to this row** — the cursor lands on the workspace header
                // and the dock keeps the keyboard, exactly as it does on a pane row.
                .on_peek(crate::chrome::fires(
                    &mount,
                    row_peek(workspace_nav_key(ws_idx)),
                    emit_intent,
                ))
                .color(theme.colors.warning)
                // Top-right (like the pane cards' right-aligned keycap), nudged down
                // onto the workspace title row so it lines up with the name.
                .placement(HintPlacement::TopRight)
                .offset_y((theme.font_size * 0.45) as f64),
        );
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
            let catalog = ctx.action_catalog().expect("a render pass has the catalog");
            let menu = crate::chrome::context_menu::menu_from_items(
                "Workspaces",
                "What you can do here",
                MENU_CONTAINER,
                container_items(),
                catalog,
                emit_intent,
            );
            heca_grid_ui::widgets::ContextMenu::new(MENU_CONTAINER).child(menu)
        })
        .child(col)
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
    #[test]
    fn every_row_kind_carries_a_nav_key_the_hit_test_can_find() {
        let tree = tree();
        let state = store();
        let theme = GuiTheme::default();
        let emit: ChromeIntentEmitter = Rc::new(|_| {});
        let mut signals = ChromeSignals::default();
        let mut drag = DragItemRegistry::default();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let ctx = ChromeCtx::for_build(crate::host::App::new(&state), &theme, &emit, &catalog);
        let root = build_workspaces_container(
            &ctx,
            &tree,
            &ProgramsConfig::default(),
            &theme,
            &emit,
            &state.workspaces,
            "left".to_string(),
            heca_grid_ui::reactive::signal(0.0),
            heca_grid_ui::reactive::signal(false),
            &mut signals,
            &mut drag,
        );

        fn keys(n: &dyn heca_grid_ui::Component, out: &mut Vec<String>) {
            if let Some(k) = n.base().nav_key.clone() {
                out.push(k);
            }
            for c in &n.base().children {
                keys(c.as_ref(), out);
            }
        }
        let mut declared = Vec::new();
        keys(&root, &mut declared);

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
