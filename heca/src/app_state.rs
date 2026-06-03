use crate::input::WmAction;
use crate::sidebar::SidebarTree;
use heca_config::theme::Theme;
use heca_core::backend::PaneBackend;
use heca_core::layout::Session;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use std::collections::HashMap;
use std::sync::Arc;
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
    Pane(u64),
}



#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Prefix,
    /// Quick-select: each visible pane is assigned a letter; next keypress selects it.
    PaneSelect { candidates: Vec<(char, u64)> },
    /// Quick-swap: each visible pane is assigned a letter; next keypress swaps with it.
    /// `focus_after` determines whether focus follows the swapped pane.
    PaneSwap { candidates: Vec<(char, u64)>, focus_after: bool },
    /// Sidebar navigation: keyboard navigation within the sidebar tree.
    SidebarNav,
    /// Text input mode for renaming workspaces / panes.
    Rename {
        target: RenameTarget,
        buffer: String,
    },
    /// Chord sequence: multi-key binding (e.g. prefix → w → 1).
    /// `sequence` holds the keys pressed so far (after prefix).
    Chord { sequence: Vec<String> },
    /// Custom mode (e.g. resize mode). Stay in mode until Esc.
    /// `name` is the mode identifier from config.
    Mode { name: String },
    /// Confirmation prompt for destructive operations.
    /// `y` executes the stored action, `n` or `Esc` cancels.
    ConfirmDelete {
        message: String,
        action: Box<WmAction>,
    },
}

impl InputMode {
    pub fn candidates(&self) -> Option<&[(char, u64)]> {
        match self {
            InputMode::PaneSelect { candidates } | InputMode::PaneSwap { candidates, .. } => Some(candidates),
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

/// State for the interactive drag-and-drop system.
#[derive(Clone, Debug)]
pub enum DragState {
    None,
    /// Phase 1: rubberband — pane still in layout.
    InteractiveMoveStarting {
        pane_id: u64,
        /// Workspace where the drag originated.
        original_ws: usize,
        start_mouse: (f32, f32),
        threshold_sq: f32,
    },
    /// Phase 2: detached — pane follows pointer.
    InteractiveMove {
        pane_id: u64,
        /// Workspace where the drag originated.
        original_ws: usize,
        /// Mouse offset from pane top-left at grab time.
        offset: (f32, f32),
    },
    /// Phase 0: potential sidebar drag — mouse pressed, waiting for threshold.
    /// On threshold exceeded → transitions to SidebarDrag (move) or SwapSidebarDrag.
    /// On release without threshold → executes click_action instead.
    SidebarDragStarting {
        pane_id: u64,
        original_ws: usize,
        start_mouse: (f32, f32),
        threshold_sq: f32,
        /// If true, drop performs a swap instead of a move.
        swap: bool,
        /// The click action (e.g. FocusPane) to execute if released without dragging.
        click_action: Box<WmAction>,
    },
    /// Sidebar drag — move: pane stays in layout, ghost follows cursor.
    SidebarDrag {
        pane_id: u64,
        /// Workspace where the pane lives.
        original_ws: usize,
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
    pub original_col: usize,
    pub original_col_id: heca_core::layout::ColumnId,
    pub original_pane: usize,
}

/// All mouse-related runtime state.
#[derive(Clone, Debug)]
pub struct MouseState {
    pub pos: (f32, f32),
    /// Current drag state machine.
    pub drag_state: DragState,
    /// Pane being dragged (detached from layout).
    pub detached_pane: Option<DetachedPane>,
    /// Computed drop target during drag.
    pub insert_hint: Option<heca_core::layout::types::InsertPosition>,
    /// Last time edge scroll was processed (for frame-rate independence).
    pub last_edge_scroll_time: Option<std::time::Instant>,
    /// Flat index of sidebar item being hovered during drag (for visual highlight).
    pub drag_hover_sidebar_fi: Option<usize>,
    /// Flat index of sidebar item being dragged (for drag source visual effect).
    pub sidebar_drag_source_fi: Option<usize>,
    /// Label text and position of the item being dragged (for ghost label rendering).
    pub sidebar_drag_label: Option<SidebarDragLabel>,
    /// Index of the button currently hovered in the sidebar (for hover visual effect).
    pub sidebar_hovered_btn_idx: Option<usize>,
}

/// Visual info for a sidebar drag ghost label.
#[derive(Clone, Debug)]
pub struct SidebarDragLabel {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl MouseState {
    pub fn new() -> Self {
        Self {
            pos: (0.0, 0.0),
            drag_state: DragState::None,
            detached_pane: None,
            insert_hint: None,
            last_edge_scroll_time: None,
            drag_hover_sidebar_fi: None,
            sidebar_drag_source_fi: None,
            sidebar_drag_label: None,
            sidebar_hovered_btn_idx: None,
        }
    }
}

pub struct AppState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub primitive_renderer: PrimitiveRenderer,
    pub text_renderer: TextRenderer,
    pub session: Session,
    /// Content backends for panes that have one.
    pub backends: HashMap<u64, Box<dyn PaneBackend>>,
    pub theme: Theme,
    pub scale_factor: f64,
    pub needs_redraw: bool,
    pub focused_pane: Option<u64>,
    pub input_mode: InputMode,
    pub sidebar: SidebarState,
    /// The sidebar tree model for workspace/pane tree navigation.
    pub sidebar_tree: SidebarTree,
    pub active_tab: usize,
    pub tab_names: Vec<String>,
    pub mouse: MouseState,
    pub modifiers: ModifiersState,
    /// Most recently focused pane (for "go back" behavior).
    pub last_focused: Option<u64>,
    /// The last visited workspace index (for dim highlight in sidebar).
    pub last_visited_ws_idx: Option<usize>,
    /// Per-workspace last-visited pane IDs (for dim highlight and Prefix+i toggle).
    pub last_visited_pane_per_ws: Vec<Option<u64>>,
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
        let cands = vec![('a', 1), ('b', 2)];
        assert_eq!(
            InputMode::PaneSelect { candidates: cands.clone() }.candidates(),
            Some(cands.as_slice())
        );
        assert_eq!(
            InputMode::PaneSwap { candidates: cands.clone(), focus_after: false }.candidates(),
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
