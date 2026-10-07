//! [`WmAction`] — every action the window manager can run, and nothing else.

use super::{FontZoomStep, RegionVisibility, ResizeEdge, ResizeTarget, SpawnKind};
use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;

/// Window-manager action.
///
/// Unit variants are used for keybindings (no arguments).
/// Parameterized variants are used for RPC commands and direct invocation
/// (e.g. from the command palette or mouse handlers).
///
/// Each variant's **kind** ([`WmActionKind`], one per variant, fields dropped) is derived, not
/// written: the registry keys handlers by it, and tests walk every kind without keeping a list.
// WmAction itself does not derive Hash: f64 fields in parameterized variants do not implement it.
#[derive(Clone, Debug, PartialEq, strum::EnumDiscriminants)]
#[strum_discriminants(name(WmActionKind), derive(Hash, strum::EnumIter))]
pub enum WmAction {
    // ── Navigation (unit) ──
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    NextPane,
    PrevPane,
    WorkspaceNext,
    WorkspacePrev,
    FocusToggleLocal,
    FocusToggleGlobal,

    // ── Navigation (parameterized) ──
    FocusPane {
        pane_id: PaneId,
    },
    FocusWorkspace {
        ws_idx: usize,
    },

    // ── Layout (unit) ──
    SplitHorizontal,
    SplitVertical,
    ZoomColumn,
    /// Zoom **the column you name**, wherever it is — the parameterized twin of [`ZoomColumn`],
    /// which acts on the active one (F003/P085/T356).
    ///
    /// A component resolves its cursor to a column and dispatches this; a menu entry or an RPC call
    /// names one outright. Without it, "zoom the selected column" could only ever be a handler that
    /// reached into one specific component's model.
    ZoomColumnAtIndex {
        ws_idx: usize,
        col_idx: usize,
    },
    /// Open the pane context menu for the focused pane (keyboard/RPC entry; the mouse right-click
    /// opens it directly). Anchored at the last cursor position.
    OpenContextMenu,
    ResizeIncrease,
    ResizeDecrease,
    PaneHeightIncrease,
    PaneHeightDecrease,
    /// Pan the horizontal view left/right (reveal column overflow / off-screen
    /// content). View-only — does not move focus or change the layout.
    ScrollViewLeft,
    ScrollViewRight,
    SwapLeft,
    SwapRight,
    SwapUp,
    SwapDown,
    /// Move a pane one slot left within its column. `pane_id = None` operates on
    /// the active pane (keyboard); `Some(id)` targets a specific pane (pane-header
    /// button / RPC).
    MovePaneLeft {
        pane_id: Option<PaneId>,
    },
    /// Move a pane one slot right within its column (see [`MovePaneLeft`] re: `pane_id`).
    MovePaneRight {
        pane_id: Option<PaneId>,
    },
    MoveColumnUp,
    MoveColumnDown,

    // ── Layout (parameterized) ──
    Swap {
        a_id: PaneId,
        b_id: PaneId,
    },
    Move {
        pane_id: PaneId,
        target_col: usize,
    },
    MovePaneToWorkspace {
        pane_id: PaneId,
        ws_idx: usize,
    },
    MovePaneToColumn {
        pane_id: PaneId,
        ws_idx: usize,
        col_idx: usize,
    },
    /// **Put a pane at an exact place**: row `pane_idx` of column `col_idx` in workspace
    /// `ws_idx`, or — with no row — a new column of its own at gap `col_idx` (gap `i` is left of
    /// column `i`). Indices past the end clamp to it. From wherever the pane is, tiled or floating,
    /// any workspace. **Within one workspace the numbers are the ones you see with the pane still
    /// where it is**; what taking it out does to them is the workspace's own business.
    ///
    /// What a drag-and-drop means once the gesture has said where it landed, and what a key or a
    /// plugin names to do the same.
    PlacePane {
        pane_id: PaneId,
        ws_idx: usize,
        col_idx: usize,
        pane_idx: Option<usize>,
    },
    MoveColumnToWorkspace {
        col_idx: usize,
        ws_idx: usize,
        focus: bool,
    },
    /// Move a column to a position (within its workspace, or into another) — sidebar
    /// column DnD / RPC (F4.5). `src_ws == dst_ws` reorders; otherwise it moves.
    MoveColumn {
        src_ws: usize,
        src_col: usize,
        dst_ws: usize,
        dst_idx: usize,
        focus: bool,
    },
    /// Swap two columns' positions — Shift+drag column swap / RPC (F4.5).
    SwapColumns {
        a_ws: usize,
        a_col: usize,
        b_ws: usize,
        b_col: usize,
    },
    /// Move the boundary the focused target owns, by `amount` **along the axis**: `+x` is right,
    /// `+y` is **down**. It is a *direction*, not a size — `pane_height_increase` /
    /// `pane_height_decrease` are the size verbs.
    ///
    /// - `column`/`x` — the active column's own right-hand edge, so positive widens it.
    /// - `pane`/`y` — the boundary **below** the active pane, or the one **above** it when it is
    ///   last. For the last pane, moving that boundary down therefore **shrinks** it: the divider
    ///   goes the way the key says, whichever pane is active (F004/P084/T414).
    ///
    /// `amount` is logical px for a pane and thousandths of the working width for a column.
    Resize {
        target: ResizeTarget,
        amount: f64,
        /// **Which edge moves.** Defaults to [`ResizeEdge::Auto`] — the edge the target already
        /// owned — so every binding written before this existed behaves exactly as it did.
        edge: ResizeEdge,
    },
    /// Resize a **specific** column's width by `delta` (a proportion delta /
    /// fraction of the working width). Mouse divider-drag + RPC; the keyboard
    /// `Resize`/`ResizeIncrease` act on the active column only.
    ResizeColumnBy {
        col_idx: usize,
        delta: f64,
    },
    /// Resize a **specific** stacked pane's height by `delta` logical px. Mouse
    /// divider-drag + RPC.
    ResizePaneHeightBy {
        col_idx: usize,
        pane_idx: usize,
        delta: f64,
    },
    ResizeTo {
        target: ResizeTarget,
        width: f64,
        height: f64,
    },

    // ── Font zoom ──
    /// App-wide font zoom (the `app-03` base). Steps **both** the chrome/UI font
    /// (sidebar, tabs, status bar) and every terminal pane together, so the whole
    /// app scales. `Reset` returns to the configured `font.size.*`. No layout
    /// impact (policy: `Global`).
    AppFontZoom {
        step: FontZoomStep,
    },
    /// Per-pane terminal font zoom, layered on top of the global size. `pane_id =
    /// None` targets the focused pane (keyboard); `Some(id)` targets a specific
    /// pane (`Ctrl`/`Meta`+wheel over a pane, or RPC). `Reset` makes the pane
    /// follow the global size again.
    PaneTerminalFontZoom {
        pane_id: Option<PaneId>,
        step: FontZoomStep,
    },
    /// **Type a line into a running terminal.** The target is a `terminal` (the id the store
    /// issued), a `pane_id`, or — with neither — the focused pane's terminal. `enter` presses Enter
    /// after the text, which is what makes it run.
    TerminalRun {
        pane_id: Option<PaneId>,
        terminal: Option<u64>,
        text: String,
        enter: bool,
    },
    /// **End a terminal** — what closing its pane does. Same targets as [`TerminalRun`](Self::TerminalRun).
    TerminalKill {
        pane_id: Option<PaneId>,
        terminal: Option<u64>,
    },

    // ── Pane (unit) ──
    Float,
    ClosePane,
    /// Delete the "current" column (and all its panes): the focused pane's column
    /// in normal mode, or the sidebar selection's column in sidebar-nav mode (the
    /// nav cursor never lands on a column, so `delete_selected` can't reach one).
    /// Resolves its target at dispatch and routes through the y/n confirm prompt.
    DeleteCurrentColumn,
    PaneSelect,
    /// Enter follow-link mode: assign a letter to each visible terminal hyperlink
    /// in the focused pane; the next letter opens that link (via `OpenLink`).
    FollowLink,
    /// Enter the universal hint picker: assign a letter to every actionable chrome
    /// target and fire the chosen target's intent on the next keypress
    /// (`InputMode::HintPick`). Entered with `prefix+/`.
    HintPick,
    SwapPane,
    SwapAndFocusPane,
    /// Enter the "move active column → workspace" letter pick (shows `KeyHint`s over
    /// workspaces; the picked letter dispatches [`MoveColumnToWorkspace`](WmAction::MoveColumnToWorkspace)).
    MoveColumnToWorkspacePick,
    /// Enter the "move active pane → workspace" letter pick (dispatches
    /// [`MovePaneToWorkspace`](WmAction::MovePaneToWorkspace)).
    MovePaneToWorkspacePick,
    /// Enter the "move active pane → column" letter pick (shows `KeyHint`s over the
    /// active workspace's columns; the picked letter dispatches
    /// [`MovePaneToColumn`](WmAction::MovePaneToColumn), stacking into that column).
    MovePaneToColumnPick,
    /// Take the focused pane out of its column and give it one of its own, immediately to the
    /// right. The intersection of `SplitHorizontal` ("make a column here") and `MovePaneToColumn`
    /// ("put this pane in that column"), and it reads like both.
    MovePaneToNewColumn,
    RenamePane,
    RenameColumn,

    // ── Pane (parameterized) ──
    FloatAt {
        pane_id: PaneId,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    ClosePaneById {
        pane_id: PaneId,
    },
    RenameTarget {
        pane_id: PaneId,
        name: String,
    },
    /// Enter rename mode for a specific pane by id (context menu / RPC / sidebar target),
    /// as opposed to [`RenamePane`](WmAction::RenamePane) which renames the focused pane.
    RenamePaneById {
        pane_id: PaneId,
    },
    /// Enter rename mode for a specific workspace by index (context menu / RPC / sidebar
    /// target), as opposed to [`RenameWorkspace`](WmAction::RenameWorkspace) which renames
    /// the active workspace.
    RenameWorkspaceByIdx {
        ws_idx: usize,
    },
    /// Enter rename mode for a specific column by index (context menu / RPC / sidebar
    /// target), as opposed to [`RenameColumn`](WmAction::RenameColumn) which renames the
    /// active column.
    RenameColumnByIdx {
        ws_idx: usize,
        col_idx: usize,
    },
    /// Clear the **focused** pane's custom name, reverting it to the program name.
    ResetPaneName,
    /// Clear a specific pane's custom name by id (context menu / RPC / sidebar target).
    ResetPaneNameById {
        pane_id: PaneId,
    },
    /// Clear the **active** workspace's custom name, reverting it to `Workspace N`.
    ResetWorkspaceName,
    /// Clear a specific workspace's custom name by index (context menu / RPC / sidebar target).
    ResetWorkspaceNameByIdx {
        ws_idx: usize,
    },

    // ── Quick take (unit) ──
    PaneTake,
    PaneTakeAndFocus,

    // ── Workspace (unit) ──
    CreateWorkspace,
    RenameWorkspace,

    // ── Sidebar / Chrome (unit) ──
    SidebarLeft,
    SidebarRight,
    CollapseCurrentWorkspace,
    ExpandCurrentWorkspace,
    ToggleCurrentWorkspaceCollapsed,
    CollapseCurrentColumn,
    ExpandCurrentColumn,
    ToggleCurrentColumnCollapsed,

    // ── System ──
    /// Show / hide / toggle an **addressable layer** by name (F003/P082/T327).
    ///
    /// `name` is `<owner>.<short>` — `heca.expose`, `docker.expose` — the stable handle a layer is
    /// registered under. A `LayerId` could not serve: it is a runtime counter, so no keybinding,
    /// config line or RPC call could ever know it.
    ///
    /// `dock` names **which seating** when a component is placed twice, and is optional for the
    /// same reason `focus_dock`'s is: a keybinding cannot name a placement, so bare resolves the
    /// way `owning_mount` does — the focused seating, else the last focused of that kind. Both
    /// arguments optional keeps the action offerable in the palette, which lists only what it can
    /// run with nothing supplied.
    ShowLayer {
        name: Option<String>,
        dock: Option<String>,
    },
    HideLayer {
        name: Option<String>,
        dock: Option<String>,
    },
    /// Show it if hidden, hide it if shown — one key for a surface you flick in and out of.
    ToggleLayer {
        name: Option<String>,
        dock: Option<String>,
    },

    /// Open the command palette (F004/P092/T393).
    ///
    /// Both arguments are **optional**, and bare is exactly what it always was: the actions list,
    /// empty query. A *required* argument would have taken the action out of the palette's own
    /// listing, which offers only what it can run with no arguments.
    ///
    /// `mode` and `query` are not two mechanisms — they **compose into one prefilled query**.
    /// `mode=pane query=nvim` opens the palette with `"@nvim"` typed, because the sigil is the mode
    /// selector; the widget therefore needs no "open in a mode" API at all.
    CommandPalette {
        mode: Option<String>,
        query: Option<String>,
    },

    // ── External commands ──
    SpawnCommand {
        command: String,
        kind: SpawnKind,
        float: bool,
        close_policy: PaneClosePolicy,
        /// The folder to start in; `None` is heca's own.
        cwd: Option<String>,
    },

    // ── Scrollback (host terminal viewport) ──
    ScrollbackPageUp,
    ScrollbackPageDown,
    ScrollbackLineUp {
        amount: usize,
    },
    ScrollbackLineDown {
        amount: usize,
    },
    ScrollbackToTop,
    ScrollbackToBottom,
    ExitScrollback,

    // ── Direct scroll (no selection mode / caret) ──
    /// Scroll the viewport up by `terminal_wheel_scroll_lines` rows (immediate).
    /// Used by direct non-prefix bindings (Shift+Up etc.).
    ScrollLineUp,
    /// Scroll the viewport down by `terminal_wheel_scroll_lines` rows (immediate).
    ScrollLineDown,
    /// Scroll the viewport up by one page (immediate).
    ScrollPageUp,
    /// Scroll the viewport down by one page (immediate).
    ScrollPageDown,
    /// Scroll to the top of scrollback (immediate).
    ScrollToTop,
    /// Scroll to the live bottom (immediate).
    ScrollToBottom,
    /// Scroll the focused chrome container one page **left** (F003/P085/T352).
    ///
    /// The four horizontal variants have no terminal half: a pane's scrollback has one axis, so
    /// there is nothing for them to do when no dock holds chrome focus. They exist because a scroll
    /// area in a container has two, and the vertical actions above reach it already.
    ScrollPageLeft,
    /// Scroll the focused chrome container one page **right**.
    ScrollPageRight,
    /// Jump the focused chrome container to its **left** edge.
    ScrollToLeftEdge,
    /// Jump the focused chrome container to its **right** edge.
    ScrollToRightEdge,
    /// Jump the host viewport to an explicit offset in rows above the live bottom.
    /// Used by the GUI scrollbar / RPC; no default keybinding.
    ScrollToOffset {
        rows: usize,
    },

    // ── Selection (host capability, reusable across pane types) ──
    EnterSelectionMode,
    SelectionLeft,
    SelectionRight,
    SelectionUp,
    SelectionDown,
    ClearSelection,
    CopySelection,
    PasteClipboard,
    BeginSelection,
    ToggleSelectionEndpoint,
    /// Selection-mode `O`: open the hyperlink under the selection caret (resolves
    /// the caret cell → snapshot hyperlink → `OpenLink`). Parameterless, bound in
    /// the selection-mode keymap.
    OpenLinkAtCaret,
    /// Selection-mode `/`: enter scrollback-search query entry for the selection's
    /// pane.
    SearchScrollback,
    /// Jump to the next scrollback-search match (selection-mode `n`).
    SearchNextMatch,
    /// Jump to the previous scrollback-search match (selection-mode `N`).
    SearchPrevMatch,

    // ── Hyperlinks (parameterized) ──
    /// Open an OSC 8 link target in the OS default handler. Constructed
    /// programmatically (HintKey follow-link, Cmd/Ctrl+click, selection-mode `O`,
    /// context menu, RPC) — never bound to a key directly.
    OpenLink {
        url: String,
    },

    // ── Sidebar-specific (parameterized) ──
    AddPaneToColumn {
        ws_idx: usize,
        col_idx: usize,
    },
    /// Add a new column to a specific workspace. Constructed programmatically (the
    /// sidebar right-click context menu) with an explicit target, so it does not
    /// depend on the sidebar-nav cursor or a key binding.
    AddColumnToWorkspace {
        ws_idx: usize,
    },

    // ── Destructive (parameterized) ──
    DeleteColumn {
        ws_idx: usize,
        col_idx: usize,
    },
    DeleteWorkspace {
        ws_idx: usize,
    },

    // ── Take pane (parameterized) ──
    TakePane {
        pane_id: PaneId,
        focus_after: bool,
    },

    // ── Chrome container placement (parameterized) — plugin-02, §2.9 ──
    // Host-level container moves. Parameterized (carry ids/region), so — like the
    // other parameterized variants — they are constructed programmatically (RPC,
    // mouse, command palette), not bound directly from a config key. They apply to
    // `AppState.chrome_host`.
    MoveContainerToRegion {
        container_id: String,
        region: crate::chrome::RegionId,
    },
    ReorderContainerBefore {
        container_id: String,
        before_id: Option<String>,
    },
    /// Reorder a container to sit immediately **after** another in the same region.
    ///
    /// Not redundant with [`ReorderContainerBefore`](WmAction::ReorderContainerBefore): "after Y"
    /// resolves to "before whatever follows Y", which only the host can compute — a plugin, or a
    /// drag landing on an item's trailing edge, cannot.
    ReorderContainerAfter {
        container_id: String,
        after_id: String,
    },
    /// Show, hide or toggle a chrome region — zero width/height when hidden. The same one state
    /// `[settings] show_left_sidebar` and friends start, and `SidebarLeft`/`SidebarRight` toggle.
    SetRegionVisible {
        region: crate::chrome::RegionId,
        visible: RegionVisibility,
    },

    /// Give chrome **keyboard focus** to a mounted dock — one action, two ways in (F003/P011/T020).
    ///
    /// - `dock: None` (a bare `focus_dock` binding) opens the **pick**: every dock on screen lights
    ///   a letter and the next keypress focuses that one.
    /// - `dock: Some(id)` focuses it directly, no pick — the RPC and scripting case, which falls out
    ///   of the optional argument rather than needing a second action.
    ///
    /// The target is a **container id**, never a side: a dock is focused wherever it is seated.
    ///
    /// **It focuses, and only focuses.** Asking to focus a dock that already has the keyboard is a
    /// no-op, not a release — see [`ToggleDock`](Self::ToggleDock) for the gesture that wants both.
    FocusDock {
        dock: Option<crate::chrome::ContainerId>,
    },

    /// Focus a dock, or **give the keyboard back** if it already has it — the `global_focus`
    /// gesture: `prefix+e` in, `prefix+e` out (F003/P082/T444).
    ///
    /// **The toggle belongs to the binding, not to the action.** It lived inside `FocusDock` until
    /// now, which meant everything that asked to *focus* a dock inherited it: a click inside an
    /// already-focused dock released it, and so did an RPC `focus-dock` or the command palette. The
    /// one caller that noticed carried a guard for it (`aim_keyboard_at_click`), which is a rule in
    /// a call site rather than in the model.
    ///
    /// An action should do one thing. If the name says *focus*, it focuses; a toggle says so.
    ToggleDock {
        dock: Option<crate::chrome::ContainerId>,
    },

    /// Forget the past queries a search surface remembers (F004/P092/T392).
    ///
    /// `scope` is **optional**, and that is what makes one action serve both doors: bare it forgets
    /// every search surface's queries, which is what someone binding a key or picking it from the
    /// palette means; a caller that knows which surface it means says so. A *required* argument
    /// could not be offered in the palette at all.
    ClearSearchHistory {
        scope: Option<String>,
    },

    /// Forget the usage counts that rank a search surface's list (F004/P092/T392).
    ///
    /// Separate from [`ClearSearchHistory`](Self::ClearSearchHistory) because they are different
    /// intentions: forgetting what you typed is not forgetting what you use.
    ClearSearchRanking {
        scope: Option<String>,
    },

    /// Give the keyboard back to the focused pane — chrome focus is released (F003/P085/T352).
    ///
    /// The counterpart of [`FocusDock`](Self::FocusDock), and the reason `Esc` is a *binding* in the
    /// focus layer rather than a key this module recognises: releasing focus has to be reachable
    /// from RPC and a menu too, not only from a key nothing else can rebind.
    UnfocusDock,

    /// **Put a mounted container's cursor on the row named by `key`.**
    ///
    /// A core action, not a provider's: every container has a cursor, so a plugin's rows move it
    /// by naming this — nothing is declared by the widget. It is what a click on a row means, the
    /// generic form of `providers::move_provider_cursor`.
    ///
    /// It moves the cursor and does **nothing else**: it does not activate the row, focus a pane,
    /// or hand the keyboard anywhere. A *pick* does those (`workspaces.peek_selected`), and
    /// serving both gestures from one declaration is the bug that made `prefix+/` walk out of the
    /// sidebar.
    ///
    /// A caller that wants the cursor moved in a dock that does not yet hold the keyboard emits
    /// [`FocusDock`](Self::FocusDock) first: the events are queued and processed in order, so the
    /// dock is `Domain::Container` by the time this runs, which is what
    /// [`ActionPolicy::ContainerFocused`] requires.
    CursorTo {
        mount: crate::chrome::ContainerId,
        key: String,
    },

    // ── Overlay control (parameterized) — plugin-ui, §2.7.2 ──
    // "Everything is an action": an overlay (modal/dropdown) is confirmed or dismissed by
    // dispatching an action carrying the overlay's id. The `OverlayHost` injects the id into
    // each action button it builds; a binding or an RPC line names none and acts on the
    // front-most one (an `OverlayId` is a runtime counter nothing outside can name). Handled
    // in `dispatch_intent` (which has the `ActionRegistry` the resolution needs to run the
    // overlay's completion) — NOT via a registered `ActionHandler`, like `FocusPaneThenAction`.
    /// Confirm overlay `overlay` with action id `action` (a button id or a plugin action id),
    /// resolving its result to `ModalResult::Action { id: action }` and popping it. Bare, it is
    /// the front-most visible modal layer — the same rule as [`CloseOverlay`](WmAction::CloseOverlay).
    SubmitOverlay {
        overlay: Option<crate::chrome::OverlayId>,
        action: String,
    },
    /// Dismiss an overlay, resolving its result to `ModalResult::Dismissed` and popping it.
    ///
    /// `overlay` is **optional so the action can be bound to a key**: an `OverlayId` is a runtime
    /// counter no config line could name. Bare, it closes the front-most visible modal layer —
    /// which is what "close the overlay" means to someone pressing Escape — and does nothing when
    /// none is up, so the key is harmless in normal use.
    CloseOverlay {
        overlay: Option<crate::chrome::OverlayId>,
    },

    // ── Config ──
    ReloadConfig,

    // ── Notifications (F009) ──
    /// Dismiss one visible notification by id. Mouse (the toast's own × — a
    /// real, automatically-pickable Button) and RPC only; no default
    /// keybinding, because a keypress cannot supply an id.
    NotificationDismissOne {
        notification_id: u64,
    },
    /// Dismiss every currently visible notification.
    NotificationDismissAll,
    /// Dismiss the first eligible visible notification in stable toast order.
    NotificationDismissLast,
    /// Toggle the scoped `prefix+/`-style picker over the visible toast
    /// actions/× — in addition to their global pick letters, not instead.
    NotificationPick,
    /// Resolve `ToastStack::on_action(id, key)` into the notification's real `Intent` and fire
    /// it. Exists because a plain `ActionHandler` has no `&ActionRegistry` to dispatch an
    /// arbitrary Intent with, and the real intent cannot be known at mount time (a notification
    /// raised after startup is what carries it) — so the widget callback names this relay by an
    /// id and a key, and the relay's own handler looks the real intent up and re-fires it
    /// through `chrome::layer_emitter` for the event loop's next turn, landing on the exact same
    /// `dispatch_intent` path every other Intent takes.
    NotificationActionRelay {
        notification_id: u64,
        key: String,
    },
}

impl WmAction {
    /// Which variant this is, regardless of its field values — what the registry keys a handler
    /// by, so parameterized variants share one handler.
    pub fn kind(&self) -> WmActionKind {
        self.into()
    }

    /// Confirming or dismissing an overlay. These are resolved by the dispatcher itself, never by a
    /// registered handler: resolving one runs the overlay's completion, which needs the
    /// `ActionRegistry` to dispatch a follow-up — and a handler is given no registry.
    pub fn is_overlay_control(&self) -> bool {
        matches!(
            self,
            WmAction::SubmitOverlay { .. } | WmAction::CloseOverlay { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameterized_variants_constructible() {
        // Exercise all parameterized variants so they are not flagged as dead code.
        let _ = WmAction::FocusPane { pane_id: PaneId(1) };
        let _ = WmAction::FocusWorkspace { ws_idx: 0 };
        let _ = WmAction::Swap {
            a_id: PaneId(1),
            b_id: PaneId(2),
        };
        let _ = WmAction::Move {
            pane_id: PaneId(1),
            target_col: 0,
        };
        let _ = WmAction::MovePaneToWorkspace {
            pane_id: PaneId(1),
            ws_idx: 0,
        };
        let _ = WmAction::MovePaneToColumn {
            pane_id: PaneId(1),
            ws_idx: 0,
            col_idx: 0,
        };
        let _ = WmAction::MoveColumnToWorkspace {
            col_idx: 0,
            ws_idx: 1,
            focus: true,
        };
        let _ = WmAction::ZoomColumn;
        let _ = WmAction::Resize {
            target: ResizeTarget::Column,
            amount: 10.0,
            edge: ResizeEdge::Auto,
        };
        let _ = WmAction::ResizeTo {
            target: ResizeTarget::Pane,
            width: 100.0,
            height: 200.0,
        };
        let _ = WmAction::FloatAt {
            pane_id: PaneId(1),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let _ = WmAction::ClosePaneById { pane_id: PaneId(1) };
        let _ = WmAction::RenameTarget {
            pane_id: PaneId(1),
            name: "test".to_string(),
        };
        let _ = WmAction::SpawnCommand {
            command: "lazygit".to_string(),
            kind: SpawnKind::Terminal,
            float: true,
            close_policy: PaneClosePolicy {
                close_pane: true,
                keep_on_error: true,
                keep_on_success: false,
            },
            cwd: None,
        };
        let _ = WmAction::AddPaneToColumn {
            ws_idx: 0,
            col_idx: 0,
        };
        let _ = WmAction::DeleteColumn {
            ws_idx: 0,
            col_idx: 0,
        };
        let _ = WmAction::DeleteWorkspace { ws_idx: 0 };
        let _ = WmAction::TakePane {
            pane_id: PaneId(1),
            focus_after: false,
        };
        let _ = WmAction::RenameColumn;
        let _ = WmAction::PaneTake;
        let _ = WmAction::PaneTakeAndFocus;
        // Scrollback
        let _ = WmAction::ScrollbackPageUp;
        let _ = WmAction::ScrollbackPageDown;
        let _ = WmAction::ScrollbackLineUp { amount: 1 };
        let _ = WmAction::ScrollbackLineDown { amount: 5 };
        let _ = WmAction::ScrollbackToTop;
        let _ = WmAction::ScrollbackToBottom;
        let _ = WmAction::ExitScrollback;
    }

    #[test]
    fn a_kind_groups_a_variant_regardless_of_its_fields() {
        let a = WmAction::FocusPane { pane_id: PaneId(1) };
        let b = WmAction::FocusPane { pane_id: PaneId(2) };
        let c = WmAction::FocusLeft;
        assert_eq!(a.kind(), b.kind());
        assert_ne!(a.kind(), c.kind());
    }
}
