use crate::app::backend_store::BackendStore;
use crate::app::events::AppEvent;
pub use crate::app::selection_model::SelectionState;
use crate::input::WmAction;
use crate::sidebar::SidebarTree;
use heca_config::appearance::AppearanceConfig;
use heca_config::font::FontConfig;
use heca_config::programs::ProgramsConfig;
use heca_config::theme::Theme;
use heca_core::layout::{PaneId, Session};
use heca_grid_ui::drag::DragContext;
use heca_renderer::backdrop::Backdrop;
use heca_renderer::background::BackgroundLayer;
use heca_renderer::blur::Blur;
use heca_renderer::composite::Compositor;
use heca_renderer::grid::GridRenderer;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use std::collections::HashMap;
use std::sync::Arc;
use winit::event_loop::EventLoopProxy;
use winit::keyboard::ModifiersState;
use winit::window::Window;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarItemState {
    /// Currently active (focused workspace / pane).
    Active,
    /// Was visited previously this session (last focused before current).
    Visited,
    /// Not visited this session.
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RenameTarget {
    Workspace(usize),
    Column { ws_idx: usize, col_idx: usize },
    Pane(PaneId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Prefix,
    /// Quick-select: each visible pane is assigned a letter; next keypress selects it.
    PaneSelect {
        candidates: Vec<(char, PaneId)>,
    },
    /// Quick-swap: each visible pane is assigned a letter; next keypress swaps with it.
    /// `focus_after` determines whether focus follows the swapped pane.
    PaneSwap {
        candidates: Vec<(char, PaneId)>,
        focus_after: bool,
    },
    /// Sidebar navigation: keyboard navigation within the sidebar tree.
    SidebarNav,
    /// Text input mode for renaming workspaces / panes.
    Rename {
        target: RenameTarget,
        buffer: String,
    },
    /// Chord sequence: multi-key binding (e.g. prefix → w → 1).
    /// `sequence` holds the keys pressed so far (after prefix).
    ///
    /// Partially wired: render and input handling exist, but no command path
    /// constructs this variant yet. See handle_chord_mode() in app/input.rs.
    #[expect(
        dead_code,
        reason = "Reserved for multi-key chord UX; will be constructed when chord entry is implemented."
    )]
    Chord {
        sequence: Vec<String>,
    },
    /// Custom mode (e.g. resize mode). Stay in mode until Esc.
    /// `name` is the mode identifier from config.
    Mode {
        name: String,
    },
    /// Host-owned selection mode. Entered via `WmAction::EnterSelectionMode`.
    /// The actual selection data lifecycle (begin/update_focus/end) is driven
    /// by mouse/UI/RPC adapters and `WmAction::ClearSelection`; this mode
    /// represents the input state where selection semantics are active. Esc
    /// clears the selection and returns to `Normal`; Enter confirms the
    /// selection and returns to `Normal`. Other keys are ignored.
    Selection,
    /// Confirmation prompt for destructive operations.
    /// `y` executes the stored action, `n` or `Esc` cancels.
    /// When `resume_sidebar` is true, the prompt returns to `SidebarNav`
    /// instead of `Normal` after confirm/cancel.
    ConfirmDelete {
        message: String,
        action: Box<WmAction>,
        resume_sidebar: bool,
    },
    /// Take-pane letter selection mode.
    /// User picks a pane which gets moved to the active column bottom.
    PaneTake {
        candidates: Vec<(char, PaneId)>,
        focus_after: bool,
    },
    /// Move-to-workspace letter pick: each workspace is assigned a letter (shown as a
    /// universal `KeyHint` over its sidebar dock); the next keypress moves `target`
    /// (the active column or pane, captured on entry) into that workspace.
    WorkspacePick {
        candidates: Vec<(char, usize)>,
        target: WorkspacePickTarget,
    },
    /// Move-to-column letter pick: each column (across all workspaces) is assigned a
    /// letter (shown as a `KeyHint` over its sidebar column); the next keypress moves
    /// the active pane into that `(ws_idx, col_idx)` column (stacking with its panes).
    ColumnPick {
        candidates: Vec<(char, usize, usize)>,
        pane_id: PaneId,
    },
    /// Follow-link letter pick: each visible terminal hyperlink across **all
    /// visible panes** is assigned a letter (a keycap drawn over the link's first
    /// cell); the next keypress opens that link via [`WmAction::OpenLink`]. Entered
    /// with `prefix+Shift+o`. Each candidate carries its own `pane_id`.
    FollowLink {
        candidates: Vec<LinkHint>,
    },
    /// Scrollback-search query entry (entered with `/` in selection mode). Typing
    /// edits `AppState.search`'s query and re-runs the search live; Enter keeps the
    /// matches (so `n`/`N` navigate in selection mode), Esc cancels. The query +
    /// matches live in [`SearchState`], not here.
    Search,
}

/// Active scrollback search: the query, its matches across the searched pane's
/// scrollback, and the currently-focused match. Lives on [`AppState`] so `n`/`N`
/// navigation works after the query overlay closes back into selection mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchState {
    /// Pane whose scrollback is being searched.
    pub pane_id: PaneId,
    /// Current query text (edited live in [`InputMode::Search`]).
    pub query: String,
    /// All matches, ascending by stable row / column.
    pub matches: Vec<heca_core::backend::SearchMatch>,
    /// Index into `matches` of the focused match, if any.
    pub current: Option<usize>,
}

/// A single follow-link candidate: the letter to press, the pane it lives in, where
/// to stamp its keycap (the link's first visible cell — `row` from the viewport top,
/// `start_col` inclusive), and the URL to open. Built from `snapshot.hyperlinks`, so
/// OSC 8 and auto-detected (linkify) links are followed identically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkHint {
    pub label: char,
    pub pane_id: PaneId,
    pub row: usize,
    pub start_col: usize,
    pub url: String,
}

/// What a [`InputMode::WorkspacePick`] moves into the picked workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspacePickTarget {
    /// A column, captured by its **full address** (`ws_idx` + `col_idx`) at pick entry,
    /// so resolution is correct even if the active workspace drifts mid-pick. For
    /// `MoveColumnToWorkspace`.
    Column { ws_idx: usize, col_idx: usize },
    /// A specific pane (by stable id), for `MovePaneToWorkspace`.
    Pane(PaneId),
}

impl InputMode {
    pub fn candidates(&self) -> Option<&[(char, PaneId)]> {
        match self {
            InputMode::PaneSelect { candidates }
            | InputMode::PaneSwap { candidates, .. }
            | InputMode::PaneTake { candidates, .. } => Some(candidates),
            _ => None,
        }
    }

    /// Workspace pick candidates (letter → `ws_idx`) while a `WorkspacePick` is active.
    pub fn ws_candidates(&self) -> Option<&[(char, usize)]> {
        match self {
            InputMode::WorkspacePick { candidates, .. } => Some(candidates),
            _ => None,
        }
    }

    /// Column pick candidates (letter → `(ws_idx, col_idx)`) while a `ColumnPick` is active.
    pub fn col_candidates(&self) -> Option<&[(char, usize, usize)]> {
        match self {
            InputMode::ColumnPick { candidates, .. } => Some(candidates),
            _ => None,
        }
    }

    /// The keyboard pick currently in progress (move / select / swap / take), if any —
    /// a structured description of the pending action. Mirrored into the reactive chrome
    /// store (and emitted as `PendingPickChanged`) so any component or plugin can react
    /// to it (e.g. render its own prompt overlay). `None` when no pick is active.
    pub fn pending_pick(&self) -> Option<PendingPick> {
        // Map the active pick mode to its enter-mode action; the human prompt + label
        // come from that action's `ActionDescriptor` (the registry is the single source
        // of truth for action text — no duplicated strings here).
        let (kind, action_name) = match self {
            InputMode::PaneSelect { .. } => (PickKind::SelectPane, "pane_select"),
            InputMode::PaneSwap {
                focus_after: true, ..
            } => (PickKind::SwapPane, "swap_and_focus_pane"),
            InputMode::PaneSwap {
                focus_after: false, ..
            } => (PickKind::SwapPane, "swap_pane"),
            InputMode::PaneTake {
                focus_after: true, ..
            } => (PickKind::TakePane, "pane_take_and_focus"),
            InputMode::PaneTake {
                focus_after: false, ..
            } => (PickKind::TakePane, "pane_take"),
            InputMode::WorkspacePick {
                target: WorkspacePickTarget::Pane(_),
                ..
            } => (PickKind::MovePaneToWorkspace, "move_pane_to_workspace_pick"),
            InputMode::WorkspacePick {
                target: WorkspacePickTarget::Column { .. },
                ..
            } => (
                PickKind::MoveColumnToWorkspace,
                "move_column_to_workspace_pick",
            ),
            InputMode::ColumnPick { .. } => {
                (PickKind::MovePaneToColumn, "move_pane_to_column_pick")
            }
            _ => return None,
        };
        let desc = crate::actions::ActionRegistry::find(action_name)?;
        Some(PendingPick {
            kind,
            action_name,
            label: desc.label,
            prompt: desc.description,
        })
    }
}

/// A keyboard pick (target-selection) currently in progress. Exposed reactively so
/// components/plugins can render their own UI for the pending action. `Copy` — all
/// fields are static strings sourced from the action's [`ActionDescriptor`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingPick {
    /// Stable machine-readable kind (match on this in plugins).
    pub kind: PickKind,
    /// The enter-mode action's **config name** string (e.g. `"move_pane_to_column_pick"`) —
    /// not a `WmAction` value.
    pub action_name: &'static str,
    /// The action's command-palette label (from its `ActionDescriptor`).
    pub label: &'static str,
    /// The action's description, used as the pick prompt (from its `ActionDescriptor`).
    pub prompt: &'static str,
}

/// The kind of in-progress keyboard pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    SelectPane,
    SwapPane,
    TakePane,
    MovePaneToWorkspace,
    MoveColumnToWorkspace,
    MovePaneToColumn,
}

/// What a surface drag carries — the app payload `P` for
/// [`DragContext<AppDragPayload>`]. The `heca-grid-ui` drag framework is
/// payload-agnostic (generic over `P`); this struct is the *one* place the app's
/// drag semantics live, keeping pane/workspace concepts out of the UI crate.
///
/// One variant per draggable sidebar source. Panes were first (1a/1b); columns
/// arrive in F4.5 step 2. The drop *target* is resolved separately at release
/// (see `ChromeDragItem`) — this is only what the in-flight drag carries.
#[derive(Clone, Debug)]
pub enum AppDragPayload {
    /// A pane card dragged from the sidebar.
    Pane {
        /// The pane being dragged.
        pane_id: PaneId,
        /// Workspace the drag originated in.
        origin_ws: usize,
        /// If true, drop performs a swap instead of a move.
        swap: bool,
    },
    /// A column (its `MarkerGroup` grip) dragged from the sidebar.
    Column {
        /// Workspace the column lives in (its origin).
        ws: usize,
        /// The column's index within that workspace.
        col: usize,
        /// If true, drop performs a swap instead of a move.
        swap: bool,
    },
}

/// State for the interactive content-area drag (pane moved by mouse).
///
/// This is separate from the surface drag system (`DragContext`) because
/// interactive move detaches a pane from the layout, shows a ghost pane
/// following the cursor, and computes an insert hint — all content-area
/// concepts that don't apply to sidebar/inspector surfaces.
#[derive(Clone, Debug)]
pub enum InteractiveMovePhase {
    /// Phase 1: rubberband — pane still in layout, waiting for threshold.
    Starting {
        pane_id: PaneId,
        /// Workspace where the drag originated.
        original_ws: usize,
        start_mouse: (f32, f32),
        threshold_sq: f32,
        /// If true, drop performs a swap instead of a move.
        swap: bool,
    },
    /// Phase 2: detached — pane follows pointer (move mode).
    /// In swap mode, the pane stays in layout and only the insert hint is shown.
    Moving {
        pane_id: PaneId,
        /// Workspace where the drag originated.
        _original_ws: usize,
        /// Mouse offset from pane top-left at grab time.
        offset: (f32, f32),
        /// If true, drop performs a swap instead of a move.
        swap: bool,
    },
}

/// A pane that has been removed from the layout for interactive move.
#[derive(Clone, Debug)]
pub struct DetachedPane {
    pub pane: heca_core::layout::Pane,
    pub render_pos: heca_core::layout::types::Point,
    pub size: heca_core::layout::types::Size,
    pub original_ws: usize,
    pub _original_col: usize,
    pub original_col_id: heca_core::layout::ColumnId,
    pub original_pane: usize,
}

/// Which layout divider a mouse resize-drag is acting on.
///
/// Indices are into the **active workspace's** `scrolling.columns` (the same
/// indices [`crate::find_pane_location`] returns), so they map straight onto
/// [`heca_core::layout::scrolling::ScrollingSpace::resize_column`] /
/// `resize_pane_height`. A resize-drag never leaves the active workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeDivider {
    /// The vertical gap to the right of column `col` → resize that column's width.
    Column { col: usize },
    /// The horizontal gap below pane `pane` in column `col` → resize that pane's height.
    Pane { col: usize, pane: usize },
}

/// An in-flight mouse resize-drag (drag a column/pane divider). Distinct from the
/// DnD item-move surfaces: this mutates layout sizes, not pane positions.
#[derive(Clone, Copy, Debug)]
pub struct ResizeDrag {
    pub divider: ResizeDivider,
    /// Cursor position at the last applied delta; the next move resizes by the
    /// incremental difference so the divider tracks the pointer.
    pub last_pos: (f32, f32),
}

/// All mouse-related runtime state.
#[derive(Clone, Debug)]
pub struct MouseState {
    pub pos: (f32, f32),
    /// Surface drag coordinator (sidebar, inspector, etc.).
    pub drag_ctx: DragContext<AppDragPayload>,
    /// In-flight column/pane divider resize-drag (`None` when not resizing).
    pub resize: Option<ResizeDrag>,
    /// Content-area interactive move state (separate from surface drags).
    pub interactive_move: Option<InteractiveMovePhase>,
    /// Pane being dragged (detached from layout).
    pub detached_pane: Option<DetachedPane>,
    /// Computed drop target during interactive move.
    pub insert_hint: Option<heca_core::layout::types::PaneInsertTarget>,
    /// Last time edge scroll was processed (for frame-rate independence).
    pub last_edge_scroll_time: Option<std::time::Instant>,
    /// Pending click action when a sidebar drag doesn't exceed threshold.
    /// Stored here instead of in `DragPhase` to keep the framework
    /// dependency-free (no `WmAction` in `heca-grid-ui`).
    pub pending_click_action: Option<WmAction>,
    /// Index of the button currently hovered in the sidebar (for hover visual effect).
    pub sidebar_hovered_btn_idx: Option<usize>,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            pos: (0.0, 0.0),
            drag_ctx: DragContext::new(),
            resize: None,
            interactive_move: None,
            detached_pane: None,
            insert_hint: None,
            last_edge_scroll_time: None,
            pending_click_action: None,
            sidebar_hovered_btn_idx: None,
        }
    }
}

pub struct RetainedTerminalLayer {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    physical_size: (u32, u32),
    pub render_key: u64,
}

impl RetainedTerminalLayer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        render_key: u64,
    ) -> Self {
        let (texture, view) = make_terminal_texture(
            device,
            format,
            width,
            height,
            "terminal_layer",
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        Self {
            texture,
            view,
            physical_size: (width.max(1), height.max(1)),
            render_key,
        }
    }

    pub fn ensure_size(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> bool {
        let next_size = (width.max(1), height.max(1));
        if self.physical_size == next_size {
            return false;
        }
        let (texture, view) = make_terminal_texture(
            device,
            format,
            next_size.0,
            next_size.1,
            "terminal_layer",
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        self.texture = texture;
        self.view = view;
        self.physical_size = next_size;
        true
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

pub struct RetainedTerminalScratch {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    physical_size: (u32, u32),
}

impl RetainedTerminalScratch {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let (texture, view) = make_terminal_texture(
            device,
            format,
            width,
            height,
            "terminal_layer_scratch",
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        Self {
            texture,
            view,
            physical_size: (width.max(1), height.max(1)),
        }
    }

    pub fn ensure_size(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let next_size = (width.max(1), height.max(1));
        if self.physical_size == next_size {
            return;
        }
        let (texture, view) = make_terminal_texture(
            device,
            format,
            next_size.0,
            next_size.1,
            "terminal_layer_scratch",
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        self.texture = texture;
        self.view = view;
        self.physical_size = next_size;
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

fn make_terminal_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    label: &str,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Central application runtime state.
///
/// Holds the winit window, GPU resources, session layout, backends,
/// input mode, sidebar model, theme, and all per-frame bookkeeping.
///
/// # Invariants
///
/// - `focused_pane` always matches the session's active pane ID (kept in sync
///   by `sync_focus` after every mutation).
/// - `backends` contains an entry for every pane in the session that has a
///   backend. Removing a pane from the session must also remove its backend
///   via `BackendStore::remove_for_pane`.
/// - `input_mode` is `Normal` unless an explicit mode transition happened
///   (prefix key, sidebar entry, rename, etc.). Mode transitions always go
///   through the input dispatch, never by direct field mutation.
/// - `sidebar_tree` is rebuilt via `sync_from_session()` after any layout
///   or focus change that affects the sidebar projection.
pub struct AppState {
    pub window: Arc<Window>,
    pub event_proxy: EventLoopProxy<AppEvent>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub primitive_renderer: PrimitiveRenderer,
    pub text_renderer: TextRenderer,
    pub grid_renderer: GridRenderer,
    pub compositor: Compositor,
    pub terminal_layers: HashMap<PaneId, RetainedTerminalLayer>,
    pub terminal_layer_scratch: RetainedTerminalScratch,
    /// In-app frosted-blur primitive (shared, compositor-owned).
    /// Produces a blurred copy of the scene texture once per frame, then many
    /// `Backdrop::draw` calls stamp it into pane surface rects.
    pub blur: Blur,
    /// Backdrop sampler (shared, stateless pipeline). Draws blurred scene regions
    /// into arbitrary on-screen rects with alpha blending.
    pub backdrop: Backdrop,
    /// z=0 heca-owned frosted gradient background layer (the bottom-most layer
    /// panes composite translucently over). Cached; recomputes only on resize
    /// or gradient/blur param change. See `heca-renderer/src/background.rs`.
    pub background: BackgroundLayer,
    /// Host-owned git metadata cache keyed by repo root / pane cwd.
    pub git_runtime_cache: crate::app::git_monitor::GitRuntimeCache,
    pub session: Session,
    /// Content backends for panes that have one.
    pub backends: BackendStore,
    pub theme: Theme,
    /// Resolved program catalog copied from config and refreshed on reload.
    pub programs: ProgramsConfig,
    /// Appearance contract (transparency/blur/vibrancy) — read-only, copied from config.
    pub appearance: AppearanceConfig,
    /// Structured font configuration (families + sizes), decoupled from the color
    /// theme. Read at the same choke points that previously read `theme.font_*`.
    /// Refreshed on `prefix+Shift+r` reload.
    pub font_config: FontConfig,
    pub terminal_cell_size: (f32, f32),
    pub scale_factor: f64,
    pub needs_redraw: bool,
    pub focused_pane: Option<PaneId>,
    pub input_mode: InputMode,
    /// The sidebar tree model for workspace/pane tree navigation.
    pub sidebar_tree: SidebarTree,
    /// Retained grid-ui chrome tree (sidebar shell + status bar), rebuilt only when
    /// its content/size signature changes. See `chrome::RetainedChrome` (F4.1).
    pub chrome_tree: Option<crate::chrome::RetainedChrome>,
    /// Retained per-pane info-bar headers (segment `Tag` + action `IconButton`s),
    /// keyed by pane. Built/positioned each frame by `chrome::sync_pane_headers`,
    /// painted read-only in `terminal_render`, dispatched pointer events in `mouse`.
    pub pane_headers: HashMap<PaneId, crate::chrome::RetainedPaneHeader>,
    /// Retained per-pane terminal viewport widgets (scrollbar + scrolled-up badge),
    /// keyed by pane. Built once per visible pane, updated/repositioned each frame,
    /// painted read-only in `terminal_render`, dispatched pointer events in `events`.
    pub pane_viewport_widgets: HashMap<PaneId, crate::chrome::RetainedPaneViewportWidgets>,
    /// Tooltip keybind hints for the pane-action buttons, resolved from config at
    /// load/reload (so the tooltips show the user's real, rebindable keys).
    pub pane_action_hints: crate::chrome::PaneActionHints,
    /// Shared, signal-backed chrome/UI state (read-via-signals / write-via-actions).
    /// Owns region visibility/width (migrated from the old `SidebarState`); collapse,
    /// selection, targeting candidates, and scroll migrate onto it next.
    pub chrome_state: crate::chrome::SharedChromeState,
    pub mouse: MouseState,
    pub modifiers: ModifiersState,
    /// Host-owned shared selection state, reusable across pane/backend types.
    pub selection: SelectionState,
    /// Deadline of an active **visual bell** flash (`None` = not flashing). Set on a
    /// terminal bell when `[appearance.terminal] bell_visual` is on; the render pass
    /// draws a fading content-area overlay until `Instant::now()` reaches it.
    pub bell_flash_until: Option<std::time::Instant>,
    /// Active scrollback search (`None` = none). Drives the query overlay, match
    /// highlights, and `n`/`N` navigation. terminal-task-19.
    pub search: Option<SearchState>,
    /// Open right-click context menu (`None` when closed). The app's first
    /// stateful overlay: the host owns the widget so it is laid out/painted each
    /// frame and fed pointer/key events while open. terminal-task-18 / app-task-33.
    pub context_menu: Option<heca_grid_ui::widgets::ContextMenu>,
    /// Action chosen from `context_menu`. Each entry's `on_select` writes here;
    /// the event loop drains and dispatches it through the registry after feeding
    /// an event into the menu (grid-ui widgets cannot dispatch `WmAction`s
    /// directly — the closure → action sink bridges that).
    pub context_menu_action: std::rc::Rc<std::cell::RefCell<Option<WmAction>>>,
    /// Most recently focused pane (for "go back" behavior).
    pub last_focused: Option<PaneId>,
    /// The last visited workspace index (for dim highlight in sidebar).
    pub last_visited_ws_idx: Option<usize>,
    /// Per-workspace last-visited pane IDs (for dim highlight and Prefix+i toggle).
    pub last_visited_pane_per_ws: Vec<Option<PaneId>>,
    /// Whether mouse interactions are enabled.
    pub mouse_enabled: bool,
    /// Whether auto edge scroll is enabled.
    pub auto_scroll_edge: bool,
    /// Whether newly spawned terminal panes should auto-inject shell integration.
    pub shell_integration_enabled: bool,
    /// Host terminal scrollback capacity (rows) threaded from
    /// `SettingsConfig::terminal_scrollback_lines`; used when spawning terminal
    /// backends so the engine retains the configured amount of history.
    pub terminal_scrollback_lines: usize,
    /// Terminal-only mouse support for host scrollback wheel routing.
    /// When true, wheel scrolls the host scrollback viewport unless the terminal
    /// app has grabbed the mouse. Does not affect `mouse_enabled` (chrome).
    pub terminal_mouse_enabled: bool,
    /// Number of scrollback rows per wheel notch.
    pub terminal_wheel_scroll_lines: usize,
    /// Whether backend-side discrete terminal viewport animations are enabled.
    pub terminal_scroll_animations_enabled: bool,
    /// Modifier key for interactive pane drag.
    pub interactive_move_modifier: heca_config::theme::ModifierKey,
    /// When the user entered Prefix mode (for auto-timeout).
    pub prefix_entered_at: Option<std::time::Instant>,
    /// The configured prefix key combo (e.g. Ctrl+b).
    pub prefix_combo: crate::keymap::KeyCombo,
    /// Set to true when the user requests a config reload (e.g. via keybinding).
    /// The app checks this in about_to_wait and rebuilds keymaps/settings.
    pub pending_reload: bool,
    /// Whether the application window is currently focused.
    pub window_focused: bool,
    /// The OS cursor currently set on the window. Tracked so the cursor policy only
    /// calls `Window::set_cursor` when the icon actually changes (cursor-moved fires
    /// very often). See `mouse::update_cursor`.
    pub current_cursor: winit::window::CursorIcon,
}

impl AppState {
    pub fn mark_full_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// A first-party [`host`](crate::host) API handle (`app.on` / `app.state`) over
    /// the shared chrome store. The seam first-party providers (and the future WASM
    /// bridge) use to observe events + read state without touching internal signals.
    #[allow(
        dead_code,
        reason = "host API seam — first-party providers land in a later phase"
    )]
    pub fn host(&self) -> crate::host::App {
        crate::host::App::new(&self.chrome_state)
    }
}

impl AppState {
    /// Terminal pane surface opacity, derived from the shared appearance contract.
    ///
    /// Returns `1.0` (opaque) when `terminal_transparency = 0`, and the
    /// terminal-specific `terminal_opacity()` otherwise. This is the alpha used
    /// for the translucent pane surface fill drawn over the frosted backdrop.
    ///
    /// In the z=0 model, the tiled frost is the background layer showing through
    /// the translucent terminal surface, so this is simply the surface alpha
    /// used when compositing the terminal over z=0. See `render_frame`.
    pub fn terminal_surface_opacity(&self) -> f32 {
        if self.appearance.terminal.transparency > 0 {
            self.appearance.terminal_opacity()
        } else {
            1.0
        }
    }

    /// Floating terminal-pane surface opacity, mirroring
    /// [`terminal_surface_opacity`](Self::terminal_surface_opacity) but driven by
    /// `terminal_floating_transparency`. Default `0` -> `1.0` (opaque, readable)
    /// so floating panes stay solid while tiled panes are frosted.
    pub fn terminal_floating_surface_opacity(&self) -> f32 {
        if self.appearance.terminal.floating_transparency > 0 {
            self.appearance.terminal_floating_opacity()
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_mode_candidates_none() {
        assert_eq!(InputMode::Normal.candidates(), None);
        assert_eq!(InputMode::Prefix.candidates(), None);
        assert_eq!(InputMode::SidebarNav.candidates(), None);
    }

    #[test]
    fn test_input_mode_candidates_some() {
        let cands = vec![('a', PaneId(1)), ('b', PaneId(2))];
        assert_eq!(
            InputMode::PaneSelect {
                candidates: cands.clone()
            }
            .candidates(),
            Some(cands.as_slice())
        );
        assert_eq!(
            InputMode::PaneSwap {
                candidates: cands.clone(),
                focus_after: false
            }
            .candidates(),
            Some(cands.as_slice())
        );
    }

    #[test]
    fn test_sidebar_item_state_eq() {
        assert_eq!(SidebarItemState::Active, SidebarItemState::Active);
        assert_ne!(SidebarItemState::Active, SidebarItemState::Visited);
    }

    #[test]
    fn test_rename_target_clone() {
        let t = RenameTarget::Workspace(3);
        let cloned = t.clone();
        assert_eq!(t, cloned);
    }
}
