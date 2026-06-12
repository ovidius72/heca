use crate::app::backend_store::BackendStore;
use crate::app::events::AppEvent;
use crate::input::WmAction;
use crate::sidebar::SidebarTree;
use heca_config::theme::Theme;
use heca_core::layout::{PaneId, Session};
use heca_grid_ui::drag::DragContext;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
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
    #[allow(dead_code)] // Reserved for multi-key chord UX; will be constructed when chord entry is implemented.
    Chord {
        sequence: Vec<String>,
    },
    /// Custom mode (e.g. resize mode). Stay in mode until Esc.
    /// `name` is the mode identifier from config.
    Mode {
        name: String,
    },
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
}

#[derive(Clone, Debug)]
pub struct SidebarState {
    pub left_visible: bool,
    pub left_width: f32,
    pub right_visible: bool,
    pub right_width: f32,
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

/// All mouse-related runtime state.
#[derive(Clone, Debug)]
pub struct MouseState {
    pub pos: (f32, f32),
    /// Surface drag coordinator (sidebar, inspector, etc.).
    pub drag_ctx: DragContext,
    /// Content-area interactive move state (separate from surface drags).
    pub interactive_move: Option<InteractiveMovePhase>,
    /// Pane being dragged (detached from layout).
    pub detached_pane: Option<DetachedPane>,
    /// Computed drop target during interactive move.
    pub insert_hint: Option<heca_core::layout::types::PaneInsertTarget>,
    /// Last time edge scroll was processed (for frame-rate independence).
    pub last_edge_scroll_time: Option<std::time::Instant>,
    /// Pending click action when a sidebar drag doesn't exceed threshold.
    /// Stored here instead of in `SurfaceDragPhase` to keep the framework
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
            interactive_move: None,
            detached_pane: None,
            insert_hint: None,
            last_edge_scroll_time: None,
            pending_click_action: None,
            sidebar_hovered_btn_idx: None,
        }
    }
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
    pub session: Session,
    /// Content backends for panes that have one.
    pub backends: BackendStore,
    pub theme: Theme,
    pub scale_factor: f64,
    pub needs_redraw: bool,
    pub focused_pane: Option<PaneId>,
    pub input_mode: InputMode,
    pub sidebar: SidebarState,
    /// The sidebar tree model for workspace/pane tree navigation.
    pub sidebar_tree: SidebarTree,
    pub mouse: MouseState,
    pub modifiers: ModifiersState,
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
    /// Modifier key for interactive pane drag.
    pub interactive_move_modifier: heca_config::theme::ModifierKey,
    /// When the user entered Prefix mode (for auto-timeout).
    pub prefix_entered_at: Option<std::time::Instant>,
    /// The configured prefix key combo (e.g. Ctrl+b).
    pub prefix_combo: crate::keymap::KeyCombo,
    /// Set to true when the user requests a config reload (e.g. via keybinding).
    /// The app checks this in about_to_wait and rebuilds keymaps/settings.
    pub pending_reload: bool,
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
