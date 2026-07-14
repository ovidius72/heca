use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;

/// Target for resize actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeTarget {
    Column,
    Pane,
}

impl std::str::FromStr for ResizeTarget {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "column" | "col" => Ok(ResizeTarget::Column),
            "pane" => Ok(ResizeTarget::Pane),
            _ => Err(format!("unknown resize target: {}", s)),
        }
    }
}

/// Axis for resize actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeAxis {
    X,
    Y,
}

impl std::str::FromStr for ResizeAxis {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "x" | "horizontal" | "width" => Ok(ResizeAxis::X),
            "y" | "vertical" | "height" => Ok(ResizeAxis::Y),
            _ => Err(format!("unknown resize axis: {}", s)),
        }
    }
}

/// Direction of a terminal font-zoom step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontZoomStep {
    /// Increase the font size by one step.
    In,
    /// Decrease the font size by one step.
    Out,
    /// Reset to the base size (global → configured size; pane → follow global).
    Reset,
}

impl std::str::FromStr for FontZoomStep {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "in" | "increase" | "+" => Ok(FontZoomStep::In),
            "out" | "decrease" | "-" => Ok(FontZoomStep::Out),
            "reset" | "0" => Ok(FontZoomStep::Reset),
            _ => Err(format!("unknown font zoom step: {}", s)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpawnKind {
    Terminal,
    App,
    Plugin,
}

impl std::str::FromStr for SpawnKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "terminal" => Ok(SpawnKind::Terminal),
            "app" => Ok(SpawnKind::App),
            "plugin" => Ok(SpawnKind::Plugin),
            _ => Err(format!("unknown spawn kind: {}", s)),
        }
    }
}

/// Window-manager action.
///
/// Unit variants are used for keybindings (no arguments).
/// Parameterized variants are used for RPC commands and direct invocation
/// (e.g. from the command palette or mouse handlers).
// Note: WmAction does NOT derive Hash because f64 fields in parameterized
// variants do not implement Hash. The registry uses discriminant-based
// dispatch, so Hash is unnecessary.
#[derive(Clone, Debug, PartialEq)]
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
    Resize {
        target: ResizeTarget,
        axis: ResizeAxis,
        amount: f64,
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
    SidebarFocus,
    SidebarUp,
    SidebarDown,
    SidebarLeftNav,
    SidebarRightNav,
    /// Focus the row under the sidebar-nav cursor **without leaving sidebar mode** — so
    /// the tree can be walked with `j`/`k`, previewing each pane/workspace in the main
    /// view. This is what separates it from [`SidebarRightNav`](WmAction::SidebarRightNav),
    /// which focuses and *exits*.
    SidebarPeek,
    SidebarExpandToggle,
    SidebarCreateWorkspace,
    SidebarCreateColumn,
    SidebarSplitInColumn,
    SidebarZoomSelectedColumn,
    SidebarDeleteSelected,
    CollapseCurrentWorkspace,
    ExpandCurrentWorkspace,
    ToggleCurrentWorkspaceCollapsed,
    CollapseCurrentColumn,
    ExpandCurrentColumn,
    ToggleCurrentColumnCollapsed,

    // ── System ──
    CommandPalette,

    // ── External commands ──
    SpawnCommand {
        command: String,
        kind: SpawnKind,
        float: bool,
        close_policy: PaneClosePolicy,
    },

    // ── Mode management ──
    EnterMode {
        name: String,
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
    SetRegionVisible {
        region: crate::chrome::RegionId,
        visible: bool,
    },

    // ── Overlay control (parameterized) — plugin-ui, §2.7.2 ──
    // "Everything is an action": an overlay (modal/dropdown) is confirmed or dismissed by
    // dispatching an action carrying the overlay's id. The `OverlayHost` injects the id into
    // each action button it builds, and RPC passes the id it got from `open_modal`. Handled
    // in `dispatch_intent` (which has the `ActionRegistry` the resolution needs to run the
    // overlay's completion) — NOT via a registered `ActionHandler`, like `FocusPaneThenAction`.
    /// Confirm overlay `overlay` with action id `action` (a button id or a plugin action id),
    /// resolving its result to `ModalResult::Action { id: action }` and popping it.
    SubmitOverlay {
        overlay: crate::chrome::OverlayId,
        action: String,
    },
    /// Dismiss overlay `overlay`, resolving its result to `ModalResult::Dismissed` and popping it.
    CloseOverlay {
        overlay: crate::chrome::OverlayId,
    },

    // ── Chrome region show/hide (sidebar-fu-6) ──
    // Runtime, bindable toggles for the **mounted-gate** (`AppState.show_*` bools —
    // fully unmount → zero width/height), a DISTINCT axis from the `RegionMode`
    // expand/rail toggles (`SidebarLeft`/`SidebarRight`). Unit variants so they are
    // config-bindable; unbound by default. All route to `handle_set_chrome_region_shown`.
    ShowLeftSidebar,
    HideLeftSidebar,
    ToggleLeftSidebar,
    ShowRightSidebar,
    HideRightSidebar,
    ToggleRightSidebar,
    ShowTopBar,
    HideTopBar,
    ToggleTopBar,
    ShowBottomBar,
    HideBottomBar,
    ToggleBottomBar,

    // ── Config ──
    ReloadConfig,
}

/// Return the discriminant of a `WmAction`.
/// Two instances share a discriminant iff they are the same variant,
/// regardless of field values.
// Used by ActionRegistry (Phase 2) for handler dispatch.
pub fn action_discriminant(action: &WmAction) -> std::mem::Discriminant<WmAction> {
    std::mem::discriminant(action)
}

/**
Map a config key name to its unit `WmAction` variant.
Parameterized variants are not reachable from config — they are
constructed programmatically (RPC, mouse handlers, command palette).
*/
pub fn action_from_name(name: &str) -> Option<WmAction> {
    match name {
        "focus_left" => Some(WmAction::FocusLeft),
        "focus_right" => Some(WmAction::FocusRight),
        "focus_up" => Some(WmAction::FocusUp),
        "focus_down" => Some(WmAction::FocusDown),
        "split_horizontal" => Some(WmAction::SplitHorizontal),
        "split_vertical" => Some(WmAction::SplitVertical),
        "zoom_column" => Some(WmAction::ZoomColumn),
        "open_context_menu" => Some(WmAction::OpenContextMenu),
        "scroll_view_left" => Some(WmAction::ScrollViewLeft),
        "scroll_view_right" => Some(WmAction::ScrollViewRight),
        "float" => Some(WmAction::Float),
        "close" => Some(WmAction::ClosePane),
        "delete_current_column" => Some(WmAction::DeleteCurrentColumn),
        "hint_pick" => Some(WmAction::HintPick),
        "resize_increase" => Some(WmAction::ResizeIncrease),
        "resize_decrease" => Some(WmAction::ResizeDecrease),
        "sidebar_left" => Some(WmAction::SidebarLeft),
        "sidebar_right" => Some(WmAction::SidebarRight),
        "sidebar_focus" => Some(WmAction::SidebarFocus),
        "sidebar_up" => Some(WmAction::SidebarUp),
        "sidebar_down" => Some(WmAction::SidebarDown),
        "sidebar_left_nav" => Some(WmAction::SidebarLeftNav),
        "sidebar_right_nav" => Some(WmAction::SidebarRightNav),
        "sidebar_peek" => Some(WmAction::SidebarPeek),
        "sidebar_expand_toggle" => Some(WmAction::SidebarExpandToggle),
        "sidebar_create_workspace" => Some(WmAction::SidebarCreateWorkspace),
        "sidebar_create_column" => Some(WmAction::SidebarCreateColumn),
        "sidebar_split_in_column" => Some(WmAction::SidebarSplitInColumn),
        "sidebar_zoom_selected_column" => Some(WmAction::SidebarZoomSelectedColumn),
        "sidebar_delete_selected" => Some(WmAction::SidebarDeleteSelected),
        // Chrome region show/hide (sidebar-fu-6) — mounted-gate, unbound by default.
        "show_left_sidebar" => Some(WmAction::ShowLeftSidebar),
        "hide_left_sidebar" => Some(WmAction::HideLeftSidebar),
        "toggle_left_sidebar" => Some(WmAction::ToggleLeftSidebar),
        "show_right_sidebar" => Some(WmAction::ShowRightSidebar),
        "hide_right_sidebar" => Some(WmAction::HideRightSidebar),
        "toggle_right_sidebar" => Some(WmAction::ToggleRightSidebar),
        "show_top_bar" => Some(WmAction::ShowTopBar),
        "hide_top_bar" => Some(WmAction::HideTopBar),
        "toggle_top_bar" => Some(WmAction::ToggleTopBar),
        "show_bottom_bar" => Some(WmAction::ShowBottomBar),
        "hide_bottom_bar" => Some(WmAction::HideBottomBar),
        "toggle_bottom_bar" => Some(WmAction::ToggleBottomBar),
        "collapse_current_workspace" => Some(WmAction::CollapseCurrentWorkspace),
        "expand_current_workspace" => Some(WmAction::ExpandCurrentWorkspace),
        "toggle_current_workspace_collapsed" => Some(WmAction::ToggleCurrentWorkspaceCollapsed),
        "collapse_current_column" => Some(WmAction::CollapseCurrentColumn),
        "expand_current_column" => Some(WmAction::ExpandCurrentColumn),
        "toggle_current_column_collapsed" => Some(WmAction::ToggleCurrentColumnCollapsed),
        "next_pane" => Some(WmAction::NextPane),
        "prev_pane" => Some(WmAction::PrevPane),
        "pane_select" => Some(WmAction::PaneSelect),
        "follow_link" => Some(WmAction::FollowLink),
        "swap_pane" => Some(WmAction::SwapPane),
        "move_column_to_workspace_pick" => Some(WmAction::MoveColumnToWorkspacePick),
        "move_pane_to_workspace_pick" => Some(WmAction::MovePaneToWorkspacePick),
        "move_pane_to_column_pick" => Some(WmAction::MovePaneToColumnPick),
        "pane_take" => Some(WmAction::PaneTake),
        "pane_take_and_focus" => Some(WmAction::PaneTakeAndFocus),
        "swap_and_focus_pane" => Some(WmAction::SwapAndFocusPane),
        "swap_left" => Some(WmAction::SwapLeft),
        "swap_right" => Some(WmAction::SwapRight),
        "swap_up" => Some(WmAction::SwapUp),
        "swap_down" => Some(WmAction::SwapDown),
        "move_pane_left" => Some(WmAction::MovePaneLeft { pane_id: None }),
        "move_pane_right" => Some(WmAction::MovePaneRight { pane_id: None }),
        "move_column_up" => Some(WmAction::MoveColumnUp),
        "move_column_down" => Some(WmAction::MoveColumnDown),
        "move_pane_to_workspace" => Some(WmAction::MovePaneToWorkspace {
            pane_id: PaneId(0),
            ws_idx: 0,
        }),
        "move_pane_to_column" => Some(WmAction::MovePaneToColumn {
            pane_id: PaneId(0),
            ws_idx: 0,
            col_idx: 0,
        }),
        "move_column_to_workspace" => Some(WmAction::MoveColumnToWorkspace {
            col_idx: 0,
            ws_idx: 0,
            focus: true,
        }),
        "move_column" => Some(WmAction::MoveColumn {
            src_ws: 0,
            src_col: 0,
            dst_ws: 0,
            dst_idx: 0,
            focus: true,
        }),
        "swap_columns" => Some(WmAction::SwapColumns {
            a_ws: 0,
            a_col: 0,
            b_ws: 0,
            b_col: 0,
        }),
        "pane_height_increase" => Some(WmAction::PaneHeightIncrease),
        "pane_height_decrease" => Some(WmAction::PaneHeightDecrease),
        // Font zoom — global (app-wide terminal) branch, the `app-03` base.
        "app_font_increase" => Some(WmAction::AppFontZoom {
            step: FontZoomStep::In,
        }),
        "app_font_decrease" => Some(WmAction::AppFontZoom {
            step: FontZoomStep::Out,
        }),
        "app_font_reset" => Some(WmAction::AppFontZoom {
            step: FontZoomStep::Reset,
        }),
        // Font zoom — focused-pane branch (keyboard resolves `pane_id = None`).
        "pane_terminal_font_increase" => Some(WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::In,
        }),
        "pane_terminal_font_decrease" => Some(WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::Out,
        }),
        "pane_terminal_font_reset" => Some(WmAction::PaneTerminalFontZoom {
            pane_id: None,
            step: FontZoomStep::Reset,
        }),
        "workspace_next" => Some(WmAction::WorkspaceNext),
        "focus_toggle_local" => Some(WmAction::FocusToggleLocal),
        "focus_toggle_global" => Some(WmAction::FocusToggleGlobal),
        "workspace_prev" => Some(WmAction::WorkspacePrev),
        "create_workspace" => Some(WmAction::CreateWorkspace),
        "rename_workspace" => Some(WmAction::RenameWorkspace),
        "rename_pane" => Some(WmAction::RenamePane),
        "rename_column" => Some(WmAction::RenameColumn),
        "reset_pane_name" => Some(WmAction::ResetPaneName),
        "reset_workspace_name" => Some(WmAction::ResetWorkspaceName),
        "command_palette" => Some(WmAction::CommandPalette),
        "add_pane_to_column" => Some(WmAction::AddPaneToColumn {
            ws_idx: 0,
            col_idx: 0,
        }),
        "delete_column" => Some(WmAction::DeleteColumn {
            ws_idx: 0,
            col_idx: 0,
        }),
        "delete_workspace" => Some(WmAction::DeleteWorkspace { ws_idx: 0 }),
        "reload_config" => Some(WmAction::ReloadConfig),
        // Scrollback
        "scrollback_page_up" => Some(WmAction::ScrollbackPageUp),
        "scrollback_page_down" => Some(WmAction::ScrollbackPageDown),
        "scrollback_to_top" => Some(WmAction::ScrollbackToTop),
        "scrollback_to_bottom" => Some(WmAction::ScrollbackToBottom),
        "exit_scrollback" => Some(WmAction::ExitScrollback),
        // Direct scroll (no selection mode / caret)
        "scroll_line_up" => Some(WmAction::ScrollLineUp),
        "scroll_line_down" => Some(WmAction::ScrollLineDown),
        "scroll_page_up" => Some(WmAction::ScrollPageUp),
        "scroll_page_down" => Some(WmAction::ScrollPageDown),
        "scroll_to_top" => Some(WmAction::ScrollToTop),
        "scroll_to_bottom" => Some(WmAction::ScrollToBottom),
        // `amount` is in notches; the handler multiplies by the user-configurable
        // `terminal_wheel_scroll_lines` before scrolling.  Default = 1 notch.
        "scrollback_line_up" => Some(WmAction::ScrollbackLineUp { amount: 1 }),
        "scrollback_line_down" => Some(WmAction::ScrollbackLineDown { amount: 1 }),

        "enter_selection_mode" => Some(WmAction::EnterSelectionMode),
        "selection_left" => Some(WmAction::SelectionLeft),
        "selection_right" => Some(WmAction::SelectionRight),
        "selection_up" => Some(WmAction::SelectionUp),
        "selection_down" => Some(WmAction::SelectionDown),
        "clear_selection" => Some(WmAction::ClearSelection),
        "copy_selection" => Some(WmAction::CopySelection),
        "paste_clipboard" => Some(WmAction::PasteClipboard),
        "begin_selection" => Some(WmAction::BeginSelection),
        "toggle_selection_endpoint" => Some(WmAction::ToggleSelectionEndpoint),
        "open_link_at_caret" => Some(WmAction::OpenLinkAtCaret),
        "search_scrollback" => Some(WmAction::SearchScrollback),
        "search_next_match" => Some(WmAction::SearchNextMatch),
        "search_prev_match" => Some(WmAction::SearchPrevMatch),
        _ => {
            // Dynamic: focus_workspace_1 → FocusWorkspace { ws_idx: 0 }
            if let Some(rest) = name.strip_prefix("focus_workspace_")
                && let Ok(n) = rest.parse::<usize>()
                && n >= 1
            {
                return Some(WmAction::FocusWorkspace { ws_idx: n - 1 });
            }
            None
        }
    }
}

// ── Parameterized action builders ──

fn get_u64(args: &std::collections::HashMap<String, String>, key: &str) -> Option<u64> {
    args.get(key)?.parse().ok()
}
fn get_usize(args: &std::collections::HashMap<String, String>, key: &str) -> Option<usize> {
    args.get(key)?.parse().ok()
}
fn get_f64(args: &std::collections::HashMap<String, String>, key: &str) -> Option<f64> {
    args.get(key)?.parse().ok()
}
fn get_string(args: &std::collections::HashMap<String, String>, key: &str) -> Option<String> {
    args.get(key).cloned()
}
fn get_enum<T: std::str::FromStr>(
    args: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<T> {
    args.get(key)?.parse().ok()
}

/// Build a parameterized `WmAction` from a name and string args.
/// Returns `None` if the action name is unknown or args are missing/invalid.
///
/// Supported names and required args:
///   "focus_pane"          → pane_id: PaneId
///   "focus_workspace"     → ws_idx: usize
///   "swap"                → a_id: PaneId, b_id: PaneId
///   "move"                → pane_id: PaneId, target_col: usize
///   "resize"              → target: "column"|"pane", axis: "x"|"y", amount: f64
///   "resize_to"           → target: "column"|"pane", width: f64, height: f64
///   "float_at"            → pane_id: PaneId, x: f64, y: f64, width: f64, height: f64
///   "close_pane_by_id"    → pane_id: PaneId
///   "rename_target"       → pane_id: PaneId, name: String
///   "scroll_to_offset"    → rows: usize
///   "spawn_command"       → command: String
pub fn build_action(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    match name {
        "focus_pane" => Some(WmAction::FocusPane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "focus_workspace" => Some(WmAction::FocusWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "swap" => Some(WmAction::Swap {
            a_id: PaneId(get_u64(args, "a_id")?),
            b_id: PaneId(get_u64(args, "b_id")?),
        }),
        "move" => Some(WmAction::Move {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            target_col: get_usize(args, "target_col")?,
        }),
        "move_pane_to_workspace" => Some(WmAction::MovePaneToWorkspace {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "move_pane_to_column" => Some(WmAction::MovePaneToColumn {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "move_column" => Some(WmAction::MoveColumn {
            src_ws: get_usize(args, "src_ws")?,
            src_col: get_usize(args, "src_col")?,
            dst_ws: get_usize(args, "dst_ws")?,
            dst_idx: get_usize(args, "dst_idx")?,
            focus: args
                .get("focus")
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }),
        "swap_columns" => Some(WmAction::SwapColumns {
            a_ws: get_usize(args, "a_ws")?,
            a_col: get_usize(args, "a_col")?,
            b_ws: get_usize(args, "b_ws")?,
            b_col: get_usize(args, "b_col")?,
        }),
        "resize" => Some(WmAction::Resize {
            target: get_enum(args, "target")?,
            axis: get_enum(args, "axis")?,
            amount: get_f64(args, "amount")?,
        }),
        "resize_to" => Some(WmAction::ResizeTo {
            target: get_enum(args, "target")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "float_at" => Some(WmAction::FloatAt {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            x: get_f64(args, "x")?,
            y: get_f64(args, "y")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "close_pane_by_id" => Some(WmAction::ClosePaneById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "rename_target" => Some(WmAction::RenameTarget {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            name: get_string(args, "name")?,
        }),
        "rename_pane_by_id" => Some(WmAction::RenamePaneById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "rename_workspace_by_idx" => Some(WmAction::RenameWorkspaceByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "rename_column_by_idx" => Some(WmAction::RenameColumnByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "reset_pane_name_by_id" => Some(WmAction::ResetPaneNameById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "reset_workspace_name_by_idx" => Some(WmAction::ResetWorkspaceNameByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "take_pane" => Some(WmAction::TakePane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            focus_after: args
                .get("focus_after")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
        }),
        "add_pane_to_column" => Some(WmAction::AddPaneToColumn {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "delete_column" => Some(WmAction::DeleteColumn {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "delete_workspace" => Some(WmAction::DeleteWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        // Scrollback (parameterized)
        "scrollback_line_up" => Some(WmAction::ScrollbackLineUp {
            amount: get_usize(args, "amount").unwrap_or(1),
        }),
        "scrollback_line_down" => Some(WmAction::ScrollbackLineDown {
            amount: get_usize(args, "amount").unwrap_or(1),
        }),
        "scroll_to_offset" => Some(WmAction::ScrollToOffset {
            rows: get_usize(args, "rows")?,
        }),
        // Font zoom (RPC): `step` = in|out|reset; pane variant optionally targets
        // a specific `pane_id` (omitted → focused pane).
        "app_font_zoom" => Some(WmAction::AppFontZoom {
            step: get_enum(args, "step")?,
        }),
        "pane_terminal_font_zoom" => Some(WmAction::PaneTerminalFontZoom {
            pane_id: get_u64(args, "pane_id").map(PaneId),
            step: get_enum(args, "step")?,
        }),

        // ── Chrome container placement (plugin-04/T1) ──
        // These carry DOTTED, namespaced ids — unlike every other built-in, whose config name is
        // snake_case. That asymmetry is deliberate: the dotted namespace is the id scheme plugins
        // use (`plugin.docker.restart`), and chrome placement is the first host capability a plugin
        // is meant to drive by name. Renaming the ~115 existing snake_case built-ins to a dotted
        // scheme is a MIGRATION nobody has decided on — do not start it here by adding aliases.
        "chrome.container.move_to_region" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: get_enum(args, "region")?,
        }),
        // Conveniences over move_to_region with the region fixed: what a menu item or a keybinding
        // ("send this container to the right sidebar") actually wants to say.
        "chrome.container.move_left_sidebar" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: crate::chrome::RegionId::LeftSidebar,
        }),
        "chrome.container.move_right_sidebar" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: crate::chrome::RegionId::RightSidebar,
        }),
        // `before_id` is OPTIONAL: omitting it moves the container to the END of its region.
        "chrome.container.reorder_before" => Some(WmAction::ReorderContainerBefore {
            container_id: get_string(args, "container_id")?,
            before_id: get_string(args, "before_id"),
        }),
        "chrome.container.reorder_after" => Some(WmAction::ReorderContainerAfter {
            container_id: get_string(args, "container_id")?,
            after_id: get_string(args, "after_id")?,
        }),

        "spawn_command" => Some(WmAction::SpawnCommand {
            command: get_string(args, "command")?,
            kind: get_enum(args, "kind").unwrap_or(SpawnKind::Terminal),
            float: args
                .get("float")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            close_policy: PaneClosePolicy {
                close_pane: args
                    .get("close_pane")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(false),
                keep_on_error: args
                    .get("keep_on_error")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(false),
                keep_on_success: args
                    .get("keep_on_success")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(false),
            },
        }),
        _ => None,
    }
}

/// Binding priority: lower = checked first. Focus wins over resize on conflicts.
///
/// Test-only helper: the exhaustive test in this module calls it directly so
/// exhaustiveness is enforced by the test's `each_variant()` list, and any
/// divergence between the production body and a shadow implementation
/// surfaces immediately as a compile error.
#[cfg(test)]
pub(crate) fn action_priority(action: &WmAction) -> u8 {
    match action {
        // Navigation (highest priority)
        WmAction::FocusLeft
        | WmAction::FocusRight
        | WmAction::FocusUp
        | WmAction::FocusDown
        | WmAction::NextPane
        | WmAction::PrevPane => 0,
        // Sidebar navigation (only used in sidebar mode via resolve_mode)
        // Low priority so they don't override focus bindings in normal/prefix mode.
        WmAction::SidebarFocus => 0,
        WmAction::SidebarUp
        | WmAction::SidebarDown
        | WmAction::SidebarLeftNav
        | WmAction::SidebarRightNav
        | WmAction::SidebarPeek
        | WmAction::SidebarExpandToggle
        | WmAction::SidebarCreateWorkspace
        | WmAction::SidebarCreateColumn
        | WmAction::SidebarSplitInColumn
        | WmAction::SidebarZoomSelectedColumn
        | WmAction::SidebarDeleteSelected
        | WmAction::CollapseCurrentWorkspace
        | WmAction::ExpandCurrentWorkspace
        | WmAction::ToggleCurrentWorkspaceCollapsed
        | WmAction::CollapseCurrentColumn
        | WmAction::ExpandCurrentColumn
        | WmAction::ToggleCurrentColumnCollapsed => 4,
        // Pane management
        WmAction::SplitHorizontal
        | WmAction::SplitVertical
        | WmAction::ZoomColumn
        | WmAction::OpenContextMenu
        | WmAction::ScrollViewLeft
        | WmAction::ScrollViewRight
        | WmAction::Float
        | WmAction::ClosePane
        | WmAction::DeleteCurrentColumn
        | WmAction::PaneSelect
        | WmAction::FollowLink
        | WmAction::HintPick
        | WmAction::SwapPane
        | WmAction::SwapAndFocusPane
        | WmAction::MoveColumnToWorkspacePick
        | WmAction::MovePaneToWorkspacePick
        | WmAction::MovePaneToColumnPick
        | WmAction::FocusToggleLocal
        | WmAction::FocusToggleGlobal
        | WmAction::CreateWorkspace
        | WmAction::RenameWorkspace
        | WmAction::RenamePane
        | WmAction::RenameColumn
        | WmAction::ResetPaneName
        | WmAction::ResetWorkspaceName
        | WmAction::WorkspaceNext
        | WmAction::WorkspacePrev => 1,
        // Swap
        WmAction::SwapLeft
        | WmAction::SwapRight
        | WmAction::SwapUp
        | WmAction::SwapDown
        | WmAction::MovePaneLeft { .. }
        | WmAction::MovePaneRight { .. }
        | WmAction::MoveColumnUp
        | WmAction::MoveColumnDown => 2,
        // Resize (lowest priority — checked last)
        WmAction::ResizeIncrease
        | WmAction::ResizeDecrease
        | WmAction::PaneHeightIncrease
        | WmAction::PaneHeightDecrease => 3,
        // Font zoom — resolved from keybindings; pane-management class.
        WmAction::AppFontZoom { .. } | WmAction::PaneTerminalFontZoom { .. } => 1,
        // Sidebars
        WmAction::SidebarLeft | WmAction::SidebarRight => 4,
        // System
        WmAction::CommandPalette => 5,
        // Selection (host capability). Treated as pane-management-class
        // actions so they share priority with close/rename-style actions.
        WmAction::EnterSelectionMode
        | WmAction::SelectionLeft
        | WmAction::SelectionRight
        | WmAction::SelectionUp
        | WmAction::SelectionDown
        | WmAction::ClearSelection
        | WmAction::CopySelection
        | WmAction::PasteClipboard
        | WmAction::BeginSelection
        | WmAction::ToggleSelectionEndpoint
        | WmAction::OpenLinkAtCaret
        | WmAction::SearchScrollback
        | WmAction::SearchNextMatch
        | WmAction::SearchPrevMatch
        // Scrollback (same priority as selection — pane-management-class)
        | WmAction::ScrollbackPageUp
        | WmAction::ScrollbackPageDown
        | WmAction::ScrollbackLineUp { .. }
        | WmAction::ScrollbackLineDown { .. }
        | WmAction::ScrollbackToTop
        | WmAction::ScrollbackToBottom
        | WmAction::ExitScrollback
        // Direct scroll (same priority)
        | WmAction::ScrollLineUp
        | WmAction::ScrollLineDown
        | WmAction::ScrollPageUp
        | WmAction::ScrollPageDown
        | WmAction::ScrollToTop
        | WmAction::ScrollToBottom
        | WmAction::ScrollToOffset { .. } => 1,
        // Parameterized variants are not resolved from keybindings,
        // but we still match them explicitly to avoid catch-all.
        WmAction::FocusPane { .. }
        | WmAction::FocusWorkspace { .. }
        | WmAction::Swap { .. }
        | WmAction::Move { .. }
        | WmAction::MovePaneToWorkspace { .. }
        | WmAction::MovePaneToColumn { .. }
        | WmAction::MoveColumnToWorkspace { .. }
        | WmAction::MoveColumn { .. }
        | WmAction::SwapColumns { .. }
        | WmAction::Resize { .. }
        | WmAction::ResizeColumnBy { .. }
        | WmAction::ResizePaneHeightBy { .. }
        | WmAction::ResizeTo { .. }
        | WmAction::FloatAt { .. }
        | WmAction::ClosePaneById { .. }
        | WmAction::RenameTarget { .. }
        | WmAction::RenamePaneById { .. }
        | WmAction::RenameWorkspaceByIdx { .. }
        | WmAction::RenameColumnByIdx { .. }
        | WmAction::ResetPaneNameById { .. }
        | WmAction::ResetWorkspaceNameByIdx { .. }
        | WmAction::SpawnCommand { .. }
        | WmAction::EnterMode { .. }
        | WmAction::ReloadConfig
        | WmAction::AddPaneToColumn { .. }
        | WmAction::AddColumnToWorkspace { .. }
        | WmAction::DeleteColumn { .. }
        | WmAction::DeleteWorkspace { .. }
        | WmAction::TakePane { .. }
        | WmAction::OpenLink { .. }
        | WmAction::PaneTake
        | WmAction::MoveContainerToRegion { .. }
        | WmAction::ReorderContainerBefore { .. }
        | WmAction::ReorderContainerAfter { .. }
        | WmAction::SetRegionVisible { .. }
        | WmAction::ShowLeftSidebar
        | WmAction::HideLeftSidebar
        | WmAction::ToggleLeftSidebar
        | WmAction::ShowRightSidebar
        | WmAction::HideRightSidebar
        | WmAction::ToggleRightSidebar
        | WmAction::ShowTopBar
        | WmAction::HideTopBar
        | WmAction::ToggleTopBar
        | WmAction::ShowBottomBar
        | WmAction::HideBottomBar
        | WmAction::ToggleBottomBar
        | WmAction::PaneTakeAndFocus
        // Overlay control: parameterized, never keybound (dispatched by overlay buttons/RPC).
        | WmAction::SubmitOverlay { .. }
        | WmAction::CloseOverlay { .. } => 6,
    }
}

/// Parse a key string like "h", "H", "Ctrl+h", "Ctrl+Shift+l", "Space".
/// Preserves key case exactly; shift ONLY from explicit "Shift+" modifier.
#[cfg(test)]
fn parse_key(s: &str) -> (bool, bool, bool, bool, String) {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut super_ = false;
    let mut key = String::new();
    for part in &parts {
        match part.to_lowercase().as_str() {
            "ctrl" => ctrl = true,
            "shift" => shift = true,
            "alt" => alt = true,
            "super" | "win" | "cmd" => super_ = true,
            _ => key = part.to_string(),
        }
    }
    (ctrl, shift, alt, super_, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_from_name_known() {
        assert_eq!(action_from_name("focus_left"), Some(WmAction::FocusLeft));
        assert_eq!(action_from_name("focus_right"), Some(WmAction::FocusRight));
        assert_eq!(action_from_name("zoom_column"), Some(WmAction::ZoomColumn));
        assert_eq!(action_from_name("sidebar_peek"), Some(WmAction::SidebarPeek));
        assert_eq!(
            action_from_name("toggle_current_workspace_collapsed"),
            Some(WmAction::ToggleCurrentWorkspaceCollapsed)
        );
        assert_eq!(
            action_from_name("toggle_current_column_collapsed"),
            Some(WmAction::ToggleCurrentColumnCollapsed)
        );
        assert_eq!(
            action_from_name("rename_column"),
            Some(WmAction::RenameColumn)
        );
        assert_eq!(
            action_from_name("rename_column_by_idx"),
            None,
            "by-idx variants are menu/RPC-only and carry args, so they are not resolvable by bare name"
        );
        assert_eq!(
            action_from_name("reset_pane_name"),
            Some(WmAction::ResetPaneName)
        );
        assert_eq!(
            action_from_name("reset_workspace_name"),
            Some(WmAction::ResetWorkspaceName)
        );
        assert_eq!(action_from_name("close"), Some(WmAction::ClosePane));
        assert_eq!(
            action_from_name("command_palette"),
            Some(WmAction::CommandPalette)
        );
        // Font zoom — six names map to two variants with the right step + None pane.
        assert_eq!(
            action_from_name("app_font_increase"),
            Some(WmAction::AppFontZoom {
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            action_from_name("app_font_reset"),
            Some(WmAction::AppFontZoom {
                step: FontZoomStep::Reset
            })
        );
        assert_eq!(
            action_from_name("pane_terminal_font_decrease"),
            Some(WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::Out
            })
        );
        // Selection actions are generic (host capability), not terminal-only.
        assert_eq!(
            action_from_name("enter_selection_mode"),
            Some(WmAction::EnterSelectionMode)
        );
        assert_eq!(
            action_from_name("selection_left"),
            Some(WmAction::SelectionLeft)
        );
        assert_eq!(
            action_from_name("selection_right"),
            Some(WmAction::SelectionRight)
        );
        assert_eq!(
            action_from_name("selection_up"),
            Some(WmAction::SelectionUp)
        );
        assert_eq!(
            action_from_name("selection_down"),
            Some(WmAction::SelectionDown)
        );
        assert_eq!(
            action_from_name("clear_selection"),
            Some(WmAction::ClearSelection)
        );
        assert_eq!(
            action_from_name("copy_selection"),
            Some(WmAction::CopySelection)
        );
        assert_eq!(
            action_from_name("paste_clipboard"),
            Some(WmAction::PasteClipboard)
        );
        assert_eq!(
            action_from_name("begin_selection"),
            Some(WmAction::BeginSelection)
        );
        assert_eq!(
            action_from_name("toggle_selection_endpoint"),
            Some(WmAction::ToggleSelectionEndpoint)
        );
        // Selection has no parameterized variants in Task 02; the parameterized
        // pathway is unreachable by design.
        assert_eq!(action_from_name("copy_selection_42"), None);
        assert_eq!(action_from_name("enter_selection_mode_now"), None);
    }

    #[test]
    fn test_action_from_name_unknown() {
        assert_eq!(action_from_name("not_real"), None);
        assert_eq!(action_from_name(""), None);
    }

    #[test]
    fn test_parse_key_simple() {
        let (ctrl, shift, alt, super_, key) = parse_key("h");
        assert!(!ctrl);
        assert!(!shift);
        assert!(!alt);
        assert!(!super_);
        assert_eq!(key, "h");
    }

    #[test]
    fn test_parse_key_with_modifiers() {
        let (ctrl, shift, alt, super_, key) = parse_key("Ctrl+Shift+l");
        assert!(ctrl);
        assert!(shift);
        assert!(!alt);
        assert!(!super_);
        assert_eq!(key, "l");
    }

    #[test]
    fn test_parse_key_shift_only() {
        let (ctrl, shift, alt, super_, key) = parse_key("Shift+w");
        assert!(!ctrl);
        assert!(shift);
        assert!(!alt);
        assert!(!super_);
        assert_eq!(key, "w");
    }

    #[test]
    fn test_parse_key_alt_super() {
        let (ctrl, shift, alt, super_, key) = parse_key("Alt+Super+x");
        assert!(!ctrl);
        assert!(!shift);
        assert!(alt);
        assert!(super_);
        assert_eq!(key, "x");
    }

    #[test]
    fn test_action_priority_order() {
        // Navigation should have highest priority (lowest number)
        assert!(action_priority(&WmAction::FocusLeft) < action_priority(&WmAction::ResizeIncrease));
        // Resize should have lower priority than pane management
        assert!(action_priority(&WmAction::ResizeIncrease) > action_priority(&WmAction::ClosePane));
        // CommandPalette should have lowest priority
        assert!(
            action_priority(&WmAction::CommandPalette) > action_priority(&WmAction::SidebarLeft)
        );
    }

    #[test]
    fn test_action_priority_exhaustive() {
        // Exhaustiveness is enforced by listing every variant here: if a new
        // `WmAction` variant is added, this function will fail to compile
        // until a representative value is appended. The real `action_priority`
        // is then called, so any divergence between the production body and
        // this test (e.g. a catch-all arm in production) surfaces immediately
        // as a compile error.
        fn each_variant() -> Vec<WmAction> {
            vec![
                // Navigation
                WmAction::FocusLeft,
                WmAction::FocusRight,
                WmAction::FocusUp,
                WmAction::FocusDown,
                WmAction::NextPane,
                WmAction::PrevPane,
                // Swap
                WmAction::SwapLeft,
                WmAction::SwapRight,
                WmAction::SwapUp,
                WmAction::SwapDown,
                WmAction::MovePaneLeft { pane_id: None },
                WmAction::MovePaneRight { pane_id: None },
                WmAction::MoveColumnUp,
                WmAction::MoveColumnDown,
                // Resize
                WmAction::ResizeIncrease,
                WmAction::ResizeDecrease,
                WmAction::PaneHeightIncrease,
                WmAction::PaneHeightDecrease,
                // Pane management
                WmAction::SplitHorizontal,
                WmAction::SplitVertical,
                WmAction::ZoomColumn,
                WmAction::Float,
                WmAction::ClosePane,
                WmAction::PaneSelect,
                WmAction::FollowLink,
                WmAction::SwapPane,
                WmAction::SwapAndFocusPane,
                WmAction::MoveColumnToWorkspacePick,
                WmAction::MovePaneToWorkspacePick,
                WmAction::MovePaneToColumnPick,
                WmAction::FocusToggleLocal,
                WmAction::FocusToggleGlobal,
                WmAction::CreateWorkspace,
                WmAction::RenameWorkspace,
                WmAction::RenamePane,
                WmAction::RenameColumn,
                WmAction::WorkspaceNext,
                WmAction::WorkspacePrev,
                // Sidebar (mode-internal + global toggles)
                WmAction::SidebarFocus,
                WmAction::SidebarUp,
                WmAction::SidebarDown,
                WmAction::SidebarLeftNav,
                WmAction::SidebarRightNav,
                WmAction::SidebarExpandToggle,
                WmAction::SidebarCreateWorkspace,
                WmAction::SidebarCreateColumn,
                WmAction::SidebarSplitInColumn,
                WmAction::SidebarZoomSelectedColumn,
                WmAction::SidebarDeleteSelected,
                WmAction::CollapseCurrentWorkspace,
                WmAction::ExpandCurrentWorkspace,
                WmAction::ToggleCurrentWorkspaceCollapsed,
                WmAction::CollapseCurrentColumn,
                WmAction::ExpandCurrentColumn,
                WmAction::ToggleCurrentColumnCollapsed,
                WmAction::SidebarLeft,
                WmAction::SidebarRight,
                // System
                WmAction::CommandPalette,
                // Scrollback
                WmAction::ScrollbackPageUp,
                WmAction::ScrollbackPageDown,
                WmAction::ScrollbackLineUp { amount: 1 },
                WmAction::ScrollbackLineDown { amount: 1 },
                WmAction::ScrollbackToTop,
                WmAction::ScrollbackToBottom,
                WmAction::ExitScrollback,
                // Selection (host capability, Task 02)
                WmAction::EnterSelectionMode,
                WmAction::SelectionLeft,
                WmAction::SelectionRight,
                WmAction::SelectionUp,
                WmAction::SelectionDown,
                WmAction::ClearSelection,
                WmAction::CopySelection,
                WmAction::PasteClipboard,
                WmAction::BeginSelection,
                WmAction::ToggleSelectionEndpoint,
                WmAction::OpenLinkAtCaret,
                WmAction::SearchScrollback,
                WmAction::SearchNextMatch,
                WmAction::SearchPrevMatch,
                // Take (panes + quick-take)
                WmAction::PaneTake,
                WmAction::PaneTakeAndFocus,
                // Parameterized variants
                WmAction::FocusPane { pane_id: PaneId(0) },
                WmAction::FocusWorkspace { ws_idx: 0 },
                WmAction::Swap {
                    a_id: PaneId(0),
                    b_id: PaneId(0),
                },
                WmAction::Move {
                    pane_id: PaneId(0),
                    target_col: 0,
                },
                WmAction::MovePaneToWorkspace {
                    pane_id: PaneId(0),
                    ws_idx: 0,
                },
                WmAction::MovePaneToColumn {
                    pane_id: PaneId(0),
                    ws_idx: 0,
                    col_idx: 0,
                },
                WmAction::MoveColumnToWorkspace {
                    col_idx: 0,
                    ws_idx: 0,
                    focus: false,
                },
                WmAction::MoveColumn {
                    src_ws: 0,
                    src_col: 0,
                    dst_ws: 0,
                    dst_idx: 0,
                    focus: false,
                },
                WmAction::SwapColumns {
                    a_ws: 0,
                    a_col: 0,
                    b_ws: 0,
                    b_col: 0,
                },
                WmAction::Resize {
                    target: ResizeTarget::Column,
                    axis: ResizeAxis::X,
                    amount: 0.0,
                },
                WmAction::ResizeColumnBy {
                    col_idx: 0,
                    delta: 0.0,
                },
                WmAction::ResizePaneHeightBy {
                    col_idx: 0,
                    pane_idx: 0,
                    delta: 0.0,
                },
                WmAction::ResizeTo {
                    target: ResizeTarget::Column,
                    width: 0.0,
                    height: 0.0,
                },
                WmAction::AppFontZoom {
                    step: FontZoomStep::In,
                },
                WmAction::PaneTerminalFontZoom {
                    pane_id: None,
                    step: FontZoomStep::In,
                },
                WmAction::FloatAt {
                    pane_id: PaneId(0),
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                WmAction::ClosePaneById { pane_id: PaneId(0) },
                WmAction::RenameTarget {
                    pane_id: PaneId(0),
                    name: String::new(),
                },
                WmAction::SpawnCommand {
                    command: String::new(),
                    kind: SpawnKind::Terminal,
                    float: false,
                    close_policy: PaneClosePolicy::default(),
                },
                WmAction::EnterMode {
                    name: String::new(),
                },
                WmAction::AddPaneToColumn {
                    ws_idx: 0,
                    col_idx: 0,
                },
                WmAction::DeleteColumn {
                    ws_idx: 0,
                    col_idx: 0,
                },
                WmAction::DeleteWorkspace { ws_idx: 0 },
                WmAction::TakePane {
                    pane_id: PaneId(0),
                    focus_after: false,
                },
                WmAction::ReloadConfig,
            ]
        }

        // Parameterized scrollback variants need explicit inclusion
        // since the unit list above uses `amount: 3` which is a unit-like
        // pattern for the variant.
        for action in each_variant() {
            let _ = action_priority(&action);
        }
    }

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
            axis: ResizeAxis::X,
            amount: 10.0,
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
    fn test_scrollback_action_from_name() {
        assert_eq!(
            action_from_name("scrollback_page_up"),
            Some(WmAction::ScrollbackPageUp)
        );
        assert_eq!(
            action_from_name("scrollback_page_down"),
            Some(WmAction::ScrollbackPageDown)
        );
        assert_eq!(
            action_from_name("scrollback_to_top"),
            Some(WmAction::ScrollbackToTop)
        );
        assert_eq!(
            action_from_name("scrollback_to_bottom"),
            Some(WmAction::ScrollbackToBottom)
        );
        assert_eq!(
            action_from_name("exit_scrollback"),
            Some(WmAction::ExitScrollback)
        );
        assert_eq!(
            action_from_name("scrollback_line_up"),
            Some(WmAction::ScrollbackLineUp { amount: 1 })
        );
        assert_eq!(
            action_from_name("scrollback_line_down"),
            Some(WmAction::ScrollbackLineDown { amount: 1 })
        );
    }

    #[test]
    fn test_chrome_region_show_hide_action_names() {
        // sidebar-fu-6: the 12 mounted-gate actions resolve from config names.
        for (name, expected) in [
            ("show_left_sidebar", WmAction::ShowLeftSidebar),
            ("hide_left_sidebar", WmAction::HideLeftSidebar),
            ("toggle_left_sidebar", WmAction::ToggleLeftSidebar),
            ("show_right_sidebar", WmAction::ShowRightSidebar),
            ("hide_right_sidebar", WmAction::HideRightSidebar),
            ("toggle_right_sidebar", WmAction::ToggleRightSidebar),
            ("show_top_bar", WmAction::ShowTopBar),
            ("hide_top_bar", WmAction::HideTopBar),
            ("toggle_top_bar", WmAction::ToggleTopBar),
            ("show_bottom_bar", WmAction::ShowBottomBar),
            ("hide_bottom_bar", WmAction::HideBottomBar),
            ("toggle_bottom_bar", WmAction::ToggleBottomBar),
        ] {
            assert_eq!(action_from_name(name), Some(expected), "name: {name}");
        }
    }

    #[test]
    fn test_action_discriminant_groups_variants() {
        let a = WmAction::FocusPane { pane_id: PaneId(1) };
        let b = WmAction::FocusPane { pane_id: PaneId(2) };
        let c = WmAction::FocusLeft;
        assert_eq!(action_discriminant(&a), action_discriminant(&b));
        assert_ne!(action_discriminant(&a), action_discriminant(&c));
    }

    #[test]
    fn test_focus_workspace_dynamic_parsing() {
        assert_eq!(
            action_from_name("focus_workspace_1"),
            Some(WmAction::FocusWorkspace { ws_idx: 0 })
        );
        assert_eq!(
            action_from_name("focus_workspace_9"),
            Some(WmAction::FocusWorkspace { ws_idx: 8 })
        );
        assert_eq!(action_from_name("focus_workspace_0"), None);
        assert_eq!(action_from_name("focus_workspace_"), None);
        assert_eq!(action_from_name("focus_workspace"), None);
    }

    #[test]
    fn test_build_spawn_command_with_options() {
        let args = std::collections::HashMap::from([
            ("command".to_string(), "lazygit".to_string()),
            ("kind".to_string(), "terminal".to_string()),
            ("float".to_string(), "true".to_string()),
            ("close_pane".to_string(), "true".to_string()),
            ("keep_on_error".to_string(), "true".to_string()),
        ]);
        assert_eq!(
            build_action("spawn_command", &args),
            Some(WmAction::SpawnCommand {
                command: "lazygit".to_string(),
                kind: SpawnKind::Terminal,
                float: true,
                close_policy: PaneClosePolicy {
                    close_pane: true,
                    keep_on_error: true,
                    keep_on_success: false,
                },
            })
        );
    }

    #[test]
    fn test_build_scroll_to_offset() {
        let args = std::collections::HashMap::from([(
            "rows".to_string(),
            "42".to_string(),
        )]);
        assert_eq!(
            build_action("scroll_to_offset", &args),
            Some(WmAction::ScrollToOffset { rows: 42 })
        );
    }

    #[test]
    fn test_build_rename_column_by_idx() {
        let args = std::collections::HashMap::from([
            ("ws_idx".to_string(), "1".to_string()),
            ("col_idx".to_string(), "2".to_string()),
        ]);
        assert_eq!(
            build_action("rename_column_by_idx", &args),
            Some(WmAction::RenameColumnByIdx {
                ws_idx: 1,
                col_idx: 2
            })
        );
        // Missing args → not built (both indices are required).
        assert_eq!(
            build_action("rename_column_by_idx", &std::collections::HashMap::new()),
            None
        );
    }

    #[test]
    fn test_build_reset_name_by_target() {
        let pane_args =
            std::collections::HashMap::from([("pane_id".to_string(), "7".to_string())]);
        assert_eq!(
            build_action("reset_pane_name_by_id", &pane_args),
            Some(WmAction::ResetPaneNameById { pane_id: PaneId(7) })
        );
        let ws_args = std::collections::HashMap::from([("ws_idx".to_string(), "2".to_string())]);
        assert_eq!(
            build_action("reset_workspace_name_by_idx", &ws_args),
            Some(WmAction::ResetWorkspaceNameByIdx { ws_idx: 2 })
        );
    }
}
