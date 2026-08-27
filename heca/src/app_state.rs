use crate::app::backend_store::BackendStore;
use crate::app::events::AppEvent;
pub use crate::app::selection_model::SelectionState;
use crate::input::WmAction;
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
use heca_renderer::image::ImageRenderer;
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

#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Status-bar marker while a destructive-confirm modal is up. The prompt itself is a
    /// host-owned overlay [`Dialog`](heca_grid_ui::Dialog) layer (see
    /// [`handlers::begin_confirm_delete`](crate::handlers::begin_confirm_delete)) which owns
    /// input and carries the action/resume in its completion — this variant just drives the
    /// "CONFIRM" status word.
    ConfirmDelete,
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
    /// Dock (chrome container) letter pick, entered with a bare `focus_dock`: every dock on
    /// screen gets a letter (a `KeyHint` keycap over its body) and the next keypress gives it
    /// chrome **keyboard focus**. Candidates carry a **container id**, so the pick is
    /// position-agnostic — a dock is picked wherever it is seated (F003/P011/T020).
    DockPick {
        candidates: Vec<(char, crate::chrome::ContainerId)>,
    },
    /// Universal leader/vimium **picker** (entered with `prefix+/`): every region that said what a
    /// pick does to it gets a letter (a keycap stamped over its bounds), and the next keypress runs
    /// that region's own declaration. Each candidate carries a
    /// [`HintTarget`](crate::chrome::HintTarget) — the tree it lives in and its path in it, valid
    /// for exactly as long as the letters are up. Any other key / Esc exits.
    HintPick {
        candidates: Vec<(char, crate::chrome::HintTarget)>,
    },
}

/// Active scrollback search: the query, its matches across the searched pane's
/// scrollback, and the currently-focused match. Lives on [`AppState`] so `n`/`N`
/// navigation works after the query overlay closes back into selection mode.
pub struct SearchState {
    /// The query field — a **real [`Input`]**, so the whole editing model comes for
    /// free and behaves exactly as every other text field in the app: selection,
    /// caret motion, word/line delete (`Ctrl+u`, `Ctrl+w`, `Alt+Backspace`),
    /// select-all, click-to-place-caret.
    ///
    /// It used to be a bare `String` that a hand-written key handler pushed
    /// characters onto — it understood Backspace and nothing else, so every editing
    /// shortcut silently did nothing here while working everywhere else.
    ///
    /// Driven manually (bounds + font set at paint) rather than living in the focus
    /// tree, because the bar is drawn as an overlay on the chrome scene. This is the
    /// same arrangement [`CommandPalette`]'s query line uses.
    pub input: std::cell::RefCell<heca_grid_ui::widgets::Input>,
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

    /// **Is a letter picker up, waiting for one keystroke?**
    ///
    /// The family, named once. Every one of these modes ends on the next key — picked, wrong key,
    /// or Esc — which means a modifier being held would end it too: reaching for Shift to type a
    /// capital is *part of* pressing that letter, and the picker would vanish before the letter
    /// arrived.
    ///
    /// **Exhaustive on purpose — no wildcard.** Adding an `InputMode` variant does not compile until
    /// it is classified here, which is the same technique `action_policy()` uses for `WmAction` and
    /// for the same reason. Asking the question per handler first put the guard in three of eight
    /// modes; writing it as a `matches!` list then left `DockPick` out on the day it was written,
    /// and nothing failed — a wildcard answers `false` for whatever nobody listed, silently. A
    /// compile error is the only form of this rule that cannot be forgotten.
    pub fn awaits_pick_letter(&self) -> bool {
        match self {
            // A letter is on screen and the next keystroke chooses one.
            InputMode::PaneSelect { .. }
            | InputMode::PaneSwap { .. }
            | InputMode::PaneTake { .. }
            | InputMode::WorkspacePick { .. }
            | InputMode::ColumnPick { .. }
            | InputMode::DockPick { .. }
            | InputMode::FollowLink { .. }
            | InputMode::HintPick { .. } => true,
            // Everything else reads keys for something other than picking a letter, or reads none.
            InputMode::Normal
            | InputMode::Prefix
            | InputMode::Chord { .. }
            | InputMode::Mode { .. }
            | InputMode::Selection
            | InputMode::ConfirmDelete
            | InputMode::Search => false,
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

    /// Dock pick candidates (letter → container id) while a `DockPick` is active.
    pub fn dock_candidates(&self) -> Option<&[(char, crate::chrome::ContainerId)]> {
        match self {
            InputMode::DockPick { candidates } => Some(candidates),
            _ => None,
        }
    }

    /// The keyboard pick currently in progress (move / select / swap / take), if any —
    /// a structured description of the pending action. Mirrored into the reactive chrome
    /// store (and emitted as `PendingPickChanged`) so any component or plugin can react
    /// to it (e.g. render its own prompt overlay). `None` when no pick is active.
    pub fn pending_pick(&self, catalog: &crate::actions::ActionCatalog) -> Option<PendingPick> {
        // Map the active pick mode to its enter-mode action; the human prompt + label
        // come from that action's `ActionDescriptor` (the catalog is the single source
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
            InputMode::DockPick { .. } => (PickKind::FocusDock, "focus_dock"),
            _ => return None,
        };
        let meta = catalog.find(action_name)?;
        Some(PendingPick {
            kind,
            action_name,
            label: meta.label.clone(),
            prompt: meta.description.clone(),
        })
    }
}

/// A keyboard pick (target-selection) currently in progress. Exposed reactively so
/// components/plugins can render their own UI for the pending action.
///
/// The text is **owned**, not `&'static str`: it is sourced from the action's
/// [`ActionMeta`](crate::actions::ActionMeta) in the runtime catalog, which holds owned metadata so
/// that a plugin-registered action can live there too. `Clone`, not `Copy`, for the same reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPick {
    /// Stable machine-readable kind (match on this in plugins).
    pub kind: PickKind,
    /// The enter-mode action's **config name** string (e.g. `"move_pane_to_column_pick"`) —
    /// not a `WmAction` value.
    pub action_name: &'static str,
    /// The action's command-palette label (from its `ActionMeta`).
    pub label: String,
    /// The action's description, used as the pick prompt (from its `ActionMeta`).
    pub prompt: String,
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
    /// Pick a chrome container to give keyboard focus to.
    FocusDock,
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
        }
    }
}

pub struct RetainedTerminalLayer {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    physical_size: (u32, u32),
    pub render_key: u64,
    /// Signature of the inline-image placements last rendered into this layer.
    /// When it changes, only the affected image rows (old ∪ new) are repainted —
    /// images appear, move, clear, and animate without a full-pane repaint. See
    /// `terminal-09` / `terminal-task-23`.
    pub graphics_sig: u64,
    /// Visible row ranges the last-rendered inline images covered. Retained so a
    /// placement change or animation frame advance can damage the *old* rows too
    /// (otherwise a removed/moved image would leave stale pixels behind).
    pub image_rows: Vec<heca_core::backend::TerminalRowRange>,
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
            graphics_sig: 0,
            image_rows: Vec::new(),
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
/// - the workspaces model is rebuilt via `sync_from_session()` after any layout
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
    /// Inline terminal-image renderer (Sixel / iTerm2 / Kitty graphics).
    pub image_renderer: ImageRenderer,
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
    /// The program catalog (`[program]`), behind an `Rc` so mirroring it into the chrome store is a
    /// pointer clone and the store can tell "unchanged" from "reloaded" by identity
    /// (F003/P086/T367).
    pub programs: std::rc::Rc<ProgramsConfig>,
    /// Appearance contract (transparency/blur/vibrancy) — read-only, copied from config.
    pub appearance: AppearanceConfig,
    /// How roomy the command palette is (`[settings] command_palette_size`). Re-read on reload, so
    /// changing it and pressing reload resizes the next palette.
    pub command_palette_size: heca_config::settings::PaletteSize,
    /// What the user has searched for and chosen — the command palette's memory, ranked into its
    /// order and recalled by its history keys.
    ///
    /// **Host-owned**, because the palette is rebuilt from scratch every time it opens (see
    /// `chrome::palette::open_command_palette`) and a memory that died with the widget would
    /// remember nothing. In-memory for now; F004/P092/T386 gives it a file.
    pub search_store: std::rc::Rc<std::cell::RefCell<heca_grid_ui::search::SearchStore>>,
    /// How a search query's case is treated (`[settings] search_case`). Re-read on reload.
    pub search_case: heca_config::settings::SearchCase,
    /// Whether the search memory is written to disk at all (`[settings] search_history`). Turned
    /// off, the store is neither read nor written — a shared or recorded machine is a real reason,
    /// and a user who turns it off expects the existing file to stop being consulted too.
    pub search_history: bool,
    /// The store revision last written to disk, so a save happens exactly when something new was
    /// recorded — see [`crate::search_state::persist_if_changed`].
    pub search_saved_revision: u64,
    /// What this instance last agreed the history file said. A save applies **our delta** on top of
    /// whatever is on disk now, so another heca window's runs are merged rather than discarded —
    /// and that delta is only computable against this.
    pub search_baseline: crate::search_state::Baseline,
    /// Structured font configuration (families + sizes), decoupled from the color
    /// theme. Read at the same choke points that previously read `theme.font_*`.
    /// Refreshed on `prefix+Shift+r` reload.
    pub font_config: FontConfig,
    pub terminal_cell_size: (f32, f32),
    /// Whether any visible pane is currently showing an animated inline image
    /// (GIF/APNG). Set each frame by `sync_retained_terminal_layers`; the redraw
    /// loop keeps requesting frames while true so the animation plays.
    pub has_animated_images: bool,
    /// App-wide font-zoom **offset in points**, applied on top of BOTH the
    /// configured chrome/UI font (`font_config.size.ui`) and the terminal font
    /// (`font_config.size.terminal`), so the whole app scales together. Driven by
    /// the app-wide zoom action (the `app-03` base) and `Ctrl`/`Meta`+wheel over
    /// chrome. `0.0` = no zoom. Reset returns it to `0.0`; a config reload leaves
    /// it intact.
    pub app_font_zoom: f32,
    /// Per-pane terminal font-zoom **offset in points**, applied on top of the
    /// global base (`font_config.size.terminal + app_font_zoom`) for
    /// that pane only. Absent = follows the global size. The effective per-pane
    /// size is clamped to `[TERMINAL_FONT_SIZE_MIN, TERMINAL_FONT_SIZE_MAX]`.
    pub pane_font_zoom: HashMap<PaneId, f32>,
    /// Resolved base cell size for panes with a non-zero `pane_font_zoom` offset,
    /// recomputed whenever that pane's effective font size changes. Panes absent
    /// here use the global `terminal_cell_size`. Feeds the base cell that
    /// `prepare_terminal_mount` fits the PTY grid to.
    pub pane_cell_override: HashMap<PaneId, (f32, f32)>,
    pub scale_factor: f64,
    pub needs_redraw: bool,
    pub focused_pane: Option<PaneId>,
    pub input_mode: InputMode,
    /// Retained grid-ui chrome tree (sidebar shell + status bar), rebuilt only when
    /// its content/size signature changes. See `chrome::RetainedChrome` (F4.1).
    pub chrome_tree: Option<crate::chrome::RetainedChrome>,
    /// **Retained per-pane shells**, keyed by pane — the frame around whatever app runs inside,
    /// and the widget that carries the pane's identity and its pick letter. Built/positioned each
    /// frame by `chrome::sync_panes`, painted through `heca_grid_ui::paint_child` (which is what
    /// draws the letter).
    ///
    /// Retained rather than rebuilt in paint: the picker writes a letter into the tree when it
    /// opens and reads it back a keystroke later, so a tree that does not outlive the frame cannot
    /// carry one — which is why the pane letters used to be stamped by a host paint pass
    /// (F011/P094/T451).
    pub panes: HashMap<PaneId, crate::chrome::RetainedPane>,
    /// Retained per-pane info-bar headers (segment `Tag` + action `IconButton`s),
    /// keyed by pane. Built/positioned each frame by `chrome::sync_pane_headers`,
    /// painted read-only in `terminal_render`, dispatched pointer events in `mouse`.
    pub pane_headers: HashMap<PaneId, crate::chrome::RetainedPaneHeader>,
    /// Dynamically registered overlay/panel layers (an on-demand exposé, a plugin panel).
    /// The built-in surfaces (panes, sidebar, current overlays) are derived from their own
    /// trees; this holds runtime-added layers that join the same surface stack. See
    /// [`crate::chrome::LayerRegistry`] and `docs/surface-compositor.md` §9.
    pub layers: crate::chrome::LayerRegistry,
    /// Host-owned overlay stack: pending modal completions + action metadata keyed by
    /// [`OverlayId`](crate::chrome::OverlayId). The overlays' *visual* trees live in
    /// [`layers`](AppState::layers) (Modal band); this holds only the result callbacks the
    /// overlay-control actions (`SubmitOverlay`/`CloseOverlay`) resolve. See `chrome::overlay`
    /// and `pluggable-chrome-plugin-plan.md` §2.7.1/§2.7.2.
    pub overlays: crate::chrome::OverlayHost,
    /// Retained per-pane terminal viewport widgets (scrollbar + scrolled-up badge),
    /// keyed by pane. Built once per visible pane, updated/repositioned each frame,
    /// painted read-only in `terminal_render`, dispatched pointer events in `events`.
    pub pane_viewport_widgets: HashMap<PaneId, crate::chrome::RetainedPaneViewportWidgets>,
    /// Display shortcuts for every bound action, keyed by config name, resolved from
    /// config at load/reload (so tooltips/hints show the user's real, rebindable
    /// keys — never the defaults when overridden). Any chrome button looks its own
    /// shortcut up by the action it triggers; see [`crate::chrome::ActionShortcuts`].
    pub action_shortcuts: crate::chrome::ActionShortcuts,
    /// Runtime catalog of action metadata (labels/icons/…; later confirmation specs), seeded from
    /// the built-in descriptors and — with the plugin action API — extended by plugins. The single
    /// runtime home every UI surface resolves action metadata through (see
    /// [`crate::actions::ActionCatalog`]).
    pub action_catalog: crate::actions::ActionCatalog,
    /// Context-menu registry: the content-pane built-in seeded
    /// at startup; plugins attach via `Contribution::ContextMenu` (context-menu-5). Both the
    /// mouse right-click and the keyboard `OpenContextMenu` resolve through it via
    /// `chrome::context_menu::open_context_menu_for`.
    pub context_menu_registry: crate::chrome::ContextMenuRegistry,
    /// Shared, signal-backed chrome/UI state (read-via-signals / write-via-actions).
    /// Owns region visibility/width (migrated from the old `SidebarState`); collapse,
    /// selection, targeting candidates, and scroll migrate onto it next.
    pub chrome_state: crate::chrome::SharedChromeState,
    /// Host runtime for pluggable chrome regions (contract §3.1.1): owns container
    /// placement/ordering per region and host-level moves. plugin-02 wires it
    /// empty; first-party providers register in plugin-03.
    pub chrome_host: crate::chrome::ChromeHost,
    pub mouse: MouseState,
    pub modifiers: ModifiersState,
    /// Host-owned shared selection state, reusable across pane/backend types.
    pub selection: SelectionState,
    /// Deadline of an active **visual bell** flash (`None` = not flashing). Set on a
    /// terminal bell when `[appearance.terminal] bell_visual` is on; the render pass
    /// draws a fading content-area overlay until `Instant::now()` reaches it.
    pub bell_flash_until: Option<std::time::Instant>,
    /// Active scrollback searches, **one per pane**. Drives each pane's query bar,
    /// its match highlights, and `n`/`N` navigation. terminal-task-19.
    ///
    /// Per-pane rather than a single global search: with one shared slot, starting a
    /// search in a second pane silently destroyed the first pane's — its bar and
    /// highlights vanished and there was no way to get them back. Every pane now
    /// keeps its own, and they all render at once.
    ///
    /// A `BTreeMap` so iteration order is stable — panes draw in a deterministic
    /// order frame to frame rather than wandering with hash seeding.
    ///
    /// Reach it through [`search_for`](Self::search_for) /
    /// [`focused_search`](Self::focused_search) and friends rather than indexing, so
    /// "the search the keyboard is driving" has exactly one definition.
    pub searches: std::collections::BTreeMap<PaneId, SearchState>,
    // (The right-click context menu + the destructive-confirm prompt are now host-owned overlay
    // layers in `chrome::overlay` — `open_dropdown` / `open_modal` — not bespoke fields here.)
    /// Most recently focused pane (for "go back" behavior).
    pub last_focused: Option<PaneId>,
    /// The last visited workspace index (for dim highlight in sidebar).
    pub last_visited_ws_idx: Option<usize>,
    /// Per-workspace last-visited pane IDs (for dim highlight and Prefix+i toggle).
    pub last_visited_pane_per_ws: Vec<Option<PaneId>>,
    /// **Where the exposé's highlight is, per workspace** — the map's own cursor, kept here rather
    /// than inside the widget because the map is rebuilt from scratch every time it opens.
    ///
    /// Two things depend on it, and both are things a freshly built tree cannot know:
    /// - the map opens on the pane you came from, not the first card in the session;
    /// - moving to another workspace and back returns the highlight to where you left it, instead
    ///   of restarting at that row's first pane.
    ///
    /// Deliberately **not** [`last_visited_pane_per_ws`](Self::last_visited_pane_per_ws), which is
    /// where the *app's* focus has been. Moving a highlight around a map is looking, not going: it
    /// must not rewrite the history that `prefix+i` and the sidebar's dim highlight read.
    ///
    /// Indexed by workspace, grown with the session like its neighbour above.
    pub expose_cursor_per_ws: Vec<Option<PaneId>>,
    /// **Which row the map's cursor is currently on** — the workspace, not the pane.
    ///
    /// The map is rebuilt whenever the session changes under it, and a rebuild has to put the
    /// cursor back where it was. Its per-workspace memory above cannot answer that on its own: it
    /// is read at the *active* workspace's index, so a rebuild while the cursor sat in another
    /// row moved the highlight to the active row — which reads as the map jumping to a different
    /// workspace the moment you delete a pane (Antonio, driving, 2026-08-11).
    ///
    /// Only consulted while the map is **already up**. Opening it fresh still starts at the
    /// workspace you are standing in, which is what the memory above is for.
    pub expose_cursor_ws: Option<usize>,
    /// **Which targets the host currently has a keycap on**, by the identity they declare
    /// (F003/P082/T427).
    ///
    /// The whole of the letter-ownership rule: `chrome::hint` withdraws exactly what it offered and
    /// never clears a label somebody else set, so the universal picker, a surface's own
    /// `KeyHintGroup` and the move/swap/take modes cannot erase one another. It replaced four
    /// per-widget signal lists projected every frame, which wrote `None` over every offered letter
    /// and needed a host-mode check to stop — a check a plugin could never add itself to.
    pub offered_letters: std::cell::RefCell<crate::chrome::hint::OfferedLetters>,
    /// **Which letter each pick target wore last time** — so it wears the same one again
    /// (F003/P082/T445).
    ///
    /// Keyed by the target's identity rather than its path, because a path lives one frame. Rebuilt
    /// on every pick from what is actually on screen, so a target that has gone releases its letter
    /// instead of holding one nobody can reach.
    ///
    /// Antonio, driving, 2026-08-17: *"I want to expand a pane, prefix+/ and `k` appears on that
    /// icon… then I want to collapse. prefix+/ and `j` appears on that button, while I was expecting
    /// `k`."* The letter was the target's **index**, so anything appearing earlier in the tree
    /// shifted every letter after it.
    pub remembered_letters: std::collections::HashMap<String, char>,
    /// Whether mouse interactions are enabled.
    pub mouse_enabled: bool,
    /// Whether auto edge scroll is enabled.
    pub auto_scroll_edge: bool,
    /// Whether newly spawned terminal panes should auto-inject shell integration.
    pub shell_integration_enabled: bool,
    /// Append the process/program name (small, dimmed) next to a renamed pane's custom name
    /// (`[settings] pane_renamed_add_process_name`). Threaded here so `sync_pane_headers` reads
    /// it each frame and a reload rebuilds the headers.
    pub pane_renamed_add_process_name: bool,
    /// Show each pane's working directory as its own row in the sidebar pane card
    /// (`[settings] pane_show_cwd`). Projected into the chrome store each sync so the card
    /// reads it via `ws_state`; a reload updates it live.
    pub pane_show_cwd: bool,
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
    /// Points added/removed per terminal font-zoom step, from
    /// `[settings] terminal_font_zoom_step`. Threaded here so the zoom handlers
    /// don't reach back into config each dispatch.
    pub terminal_font_zoom_step: f32,
    /// Whether `Ctrl`/`Meta`+wheel changes the font size, from
    /// `[settings] mouse_wheel_change_font_size`. When false the wheel gesture is
    /// skipped and the modified wheel is forwarded normally.
    pub mouse_wheel_change_font_size: bool,
    /// Whether backend-side discrete terminal viewport animations are enabled.
    pub terminal_scroll_animations_enabled: bool,
    /// Mount the left sidebar region at all, from `[settings] show_left_sidebar`.
    /// When false the region is fully hidden (zero width) regardless of the
    /// runtime expand/rail mode — a hard config gate, distinct from the runtime
    /// `prefix`-toggle. See [`AppState::left_sidebar_width`].
    pub show_left_sidebar: bool,
    /// Mount the right sidebar region at all, from `[settings] show_right_sidebar`.
    /// See [`AppState::right_sidebar_width`].
    pub show_right_sidebar: bool,
    /// Show the top bar (tab bar) from `[settings] show_top_bar`. When false the
    /// tab bar collapses to zero height (see [`AppState::tab_bar_height`]).
    pub show_top_bar: bool,
    /// Show the bottom bar (status bar) from `[settings] show_bottom_bar`. When
    /// false the status bar collapses to zero height (see
    /// [`AppState::status_bar_height`]).
    pub show_bottom_bar: bool,
    /// The `[confirm]` table — per-action confirmation toggles (keyed by confirm name:
    /// `delete_pane` / `delete_column` / `delete_workspace`, or any plugin action). Read by the central
    /// confirm gate via [`ConfirmConfig::enabled`](heca_config::confirm::ConfirmConfig::enabled),
    /// which falls back to the action's `ConfirmSpec::default_enabled` when unset.
    pub confirm: heca_config::confirm::ConfirmConfig,
    /// Modifier key for interactive pane drag.
    pub interactive_move_modifier: heca_config::theme::ModifierKey,
    /// When the user entered Prefix mode (for auto-timeout).
    pub prefix_entered_at: Option<std::time::Instant>,
    /// The configured prefix key combo (e.g. Ctrl+b).
    pub prefix_combo: crate::keymap::KeyCombo,
    /// Widget keymap (`widget-keys-config`): the single host-owned `[keys.widgets]`-derived map
    /// from a key chord to the semantic [`WidgetIntent`](heca_grid_ui::WidgetIntent)s it triggers.
    /// Every interactive widget/overlay (context menu, palette, `Select`, `Tabs`, `Dialog`,
    /// `Input`) consults it via [`Keymap::dispatch`](heca_grid_ui::Keymap::dispatch); consumed
    /// only while such a widget/overlay is focused (never hijacks normal input). Rebuilt on config
    /// reload alongside the keymap.
    pub widget_keymap: heca_grid_ui::Keymap,
    /// **Menus a widget declared and asked for**, waiting to become layers (F004/P084/T395).
    ///
    /// A widget builds its own menu and the framework picks the anchor, but only the host can put
    /// one *above everything* — so `install_menu_sink` drops it here and the event loop drains it.
    /// A queue rather than a direct call because the sink is a plain `Fn` installed once at
    /// startup, and inserting a layer needs `&mut AppState`; and rather than an `AppEvent` because
    /// a menu carries closures and a winit user event must be `Send`.
    pub pending_menus: std::rc::Rc<
        std::cell::RefCell<Vec<(heca_grid_ui::widgets::ContextMenu, heca_grid_ui::widgets::MenuAnchor)>>,
    >,
    /// Set to true when the user requests a config reload (e.g. via keybinding).
    /// The app checks this in about_to_wait and rebuilds keymaps/settings.
    pub pending_reload: bool,
    /// Whether the application window is currently focused.
    pub window_focused: bool,
    /// The OS cursor currently set on the window. Tracked so the cursor policy only
    /// calls `Window::set_cursor` when the icon actually changes (cursor-moved fires
    /// very often). See `mouse::update_cursor`.
    pub current_cursor: winit::window::CursorIcon,
    /// The notification store + its retained `Signal<Vec<ToastSpec>>` — F009/T208. Owns queueing,
    /// lifecycle, dedup and the projection the mounted `ToastStack` persistent layer reads.
    pub notifications: crate::notification::NotificationRuntime,
    /// Whether the scoped `notification.pick` picker is open. `KeyHintGroup::open_when` on the
    /// toast layer reads this directly — F009/T492. In addition to global `prefix+/`, not instead.
    pub notification_pick_open: heca_grid_ui::reactive::Signal<bool>,
    /// The mounted `ToastStack` layer's id — F009/T203. `None` until `mount_notification_stack`
    /// runs at startup. Kept so `handle_notification_action_relay` can rebuild the exact same
    /// `chrome::layer_emitter` the mount used, to re-fire a notification's real `Intent` on the
    /// event loop's next turn (the widget callback only carries an id + a key, never the intent
    /// itself — that only exists in the store, looked up at relay time).
    pub notification_layer_id: Option<crate::chrome::LayerId>,
}


impl AppState {

    /// The scrollback search for `pane`, if it has one.
    pub fn search_for(&self, pane: PaneId) -> Option<&SearchState> {
        self.searches.get(&pane)
    }

    /// The pane the keyboard's search acts on: the **selection's** pane if copy-mode
    /// owns one, else the focused pane.
    ///
    /// One definition, used by every search entry point — starting a search, editing
    /// the query, stepping matches, cancelling. They must agree: a search started for
    /// one pane while edits were applied to another would leave the query frozen,
    /// because the keystrokes would land on an entry that does not exist. The old
    /// single-search state avoided this by carrying its own `pane_id`; with per-pane
    /// storage the resolution itself has to be shared.
    pub fn search_target_pane(&self) -> Option<PaneId> {
        match self.selection.owner() {
            Some(crate::app::selection_model::SelectionOwner::Pane(id)) => Some(id),
            _ => self.focused_pane,
        }
    }

    /// The search the keyboard is driving — see [`search_target_pane`](Self::search_target_pane).
    pub fn active_search_mut(&mut self) -> Option<&mut SearchState> {
        let pane = self.search_target_pane()?;
        self.searches.get_mut(&pane)
    }

    /// Drop `pane`'s search, if any. Called when a pane closes so a dead pane cannot
    /// leave a search behind that nothing can reach or clear.
    pub fn clear_search(&mut self, pane: PaneId) {
        self.searches.remove(&pane);
    }
    pub fn mark_full_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// Whether a container's cursor should be shown — **its keyboard focus**, which an overlay
    /// does not take away (context-menu-3, F003/P086/T365).
    ///
    /// The row stays highlighted for the duration of a menu opened on it, instead of losing the
    /// highlight the moment the overlay appears. It used to ask whether the app was in a mode; a
    /// container holding the keyboard is the thing that was always meant.
    pub fn container_cursor_visible(&self) -> bool {
        self.chrome_state.focused_container().is_some()
    }

    /// Effective tab-bar (top bar) height: the default when shown, `0.0` when
    /// hidden via `[settings] show_top_bar`. Chrome layout/hit-testing read this
    /// so a hidden bar reclaims its space everywhere consistently.
    pub fn tab_bar_height(&self) -> f32 {
        if self.show_top_bar {
            crate::chrome::DEFAULT_TAB_BAR_HEIGHT
        } else {
            0.0
        }
    }

    /// Effective status-bar (bottom bar) height: the default when shown, `0.0`
    /// when hidden via `[settings] show_bottom_bar`.
    pub fn status_bar_height(&self) -> f32 {
        if self.show_bottom_bar {
            crate::chrome::DEFAULT_STATUS_BAR_HEIGHT
        } else {
            0.0
        }
    }

    /// Effective left-sidebar width for layout/hit-testing. `0.0` when the region
    /// is unmounted via `[settings] show_left_sidebar` **or** when it is
    /// `Hidden` (the toggle collapses a sidebar to nothing — there is no icon rail;
    /// see `docs/sidebar-provider-modes.md`); otherwise the (resizable) expanded width.
    pub fn left_sidebar_width(&self) -> f32 {
        if !self.show_left_sidebar || !self.chrome_state.left_visible() {
            0.0
        } else {
            self.chrome_state.left_size()
        }
    }

    /// Effective right-sidebar width for layout/hit-testing. `0.0` when unmounted via
    /// `[settings] show_right_sidebar` or when the region is `Hidden`.
    pub fn right_sidebar_width(&self) -> f32 {
        if !self.show_right_sidebar || !self.chrome_state.right_visible() {
            0.0
        } else {
            self.chrome_state.right_size()
        }
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
    /// The effective **terminal** global font size: the configured terminal size
    /// plus the app zoom offset, clamped. This is the base every pane starts from
    /// before its own per-pane offset is applied.
    pub fn app_font_size(&self) -> f32 {
        (self.font_config.size.terminal + self.app_font_zoom).clamp(
            crate::app::terminal_metrics::TERMINAL_FONT_SIZE_MIN,
            crate::app::terminal_metrics::TERMINAL_FONT_SIZE_MAX,
        )
    }

    /// The effective **chrome/UI** font size: the configured UI size plus the same
    /// app zoom offset, clamped. Scales the sidebar, tabs, and status bar together
    /// with the terminals so the app zoom is truly app-wide.
    pub fn app_ui_font_size(&self) -> f32 {
        (self.font_config.size.ui + self.app_font_zoom).clamp(
            crate::app::terminal_metrics::APP_UI_FONT_SIZE_MIN,
            crate::app::terminal_metrics::APP_UI_FONT_SIZE_MAX,
        )
    }

    /// The effective terminal font size for a specific pane: the configured size
    /// plus the global zoom offset plus that pane's per-pane offset, clamped to the
    /// supported range. Both the PTY cell fit and the rendered glyph size derive
    /// from this single value so they never disagree.
    pub fn effective_terminal_font_size(&self, pane_id: PaneId) -> f32 {
        let pane_offset = self.pane_font_zoom.get(&pane_id).copied().unwrap_or(0.0);
        (self.font_config.size.terminal + self.app_font_zoom + pane_offset).clamp(
            crate::app::terminal_metrics::TERMINAL_FONT_SIZE_MIN,
            crate::app::terminal_metrics::TERMINAL_FONT_SIZE_MAX,
        )
    }

    /// The base cell size a pane's PTY grid is fitted to, honoring per-pane zoom.
    /// Falls back to the global `terminal_cell_size` for panes with no override.
    pub fn pane_base_cell_size(&self, pane_id: PaneId) -> (f32, f32) {
        self.pane_cell_override
            .get(&pane_id)
            .copied()
            .unwrap_or(self.terminal_cell_size)
    }

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
    fn test_rename_target_copy() {
        let t = RenameTarget::Workspace(3);
        let copied = t; // RenameTarget is Copy — `t` stays usable below.
        assert_eq!(t, copied);
    }
}
