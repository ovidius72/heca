use crate::sidebar::SidebarTree;
use heca_config::theme::Theme;
use heca_core::backend::PaneBackend;
use heca_core::layout::Session;
use heca_core::types::Rect;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
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
pub enum InputMode {
    Normal,
    Prefix,
    /// Quick-select: each visible pane is assigned a letter; next keypress selects it.
    PaneSelect { candidates: Vec<(char, u64)> },
    /// Quick-swap: each visible pane is assigned a letter; next keypress swaps with it.
    PaneSwap { candidates: Vec<(char, u64)> },
    /// Sidebar navigation: keyboard navigation within the sidebar tree.
    SidebarNav,
}

impl InputMode {
    pub fn candidates(&self) -> Option<&[(char, u64)]> {
        match self {
            InputMode::PaneSelect { candidates } | InputMode::PaneSwap { candidates } => Some(candidates),
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

#[derive(Clone, Debug)]
pub enum DragState {
    None,
    Resizing {
        pane_id: u64,
        dir: heca_core::pane::SplitDirection,
        start_pos: (f32, f32),
    },
    MovingFloat {
        pane_id: u64,
        /// Mouse position minus float top-left at drag start (screen coords)
        offset: (f32, f32),
        start_rect: Rect,
    },
    ResizingFloat {
        pane_id: u64,
        edge: FloatEdge,
        start_mouse: (f32, f32),
        start_rect: Rect,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FloatEdge {
    Left, Right, Top, Bottom,
    TopLeft, TopRight, BottomLeft, BottomRight,
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
    pub drag_state: DragState,
    pub sidebar: SidebarState,
    /// The sidebar tree model for workspace/pane tree navigation.
    pub sidebar_tree: SidebarTree,
    pub active_tab: usize,
    pub tab_names: Vec<String>,
    pub mouse_pos: (f32, f32),
    pub modifiers: ModifiersState,
    /// Most recently focused pane (for "go back" behavior).
    pub last_focused: Option<u64>,
    /// The last visited workspace index (for dim highlight in sidebar).
    pub last_visited_ws_idx: Option<usize>,
    /// Per-workspace last-visited pane IDs (for dim highlight).
    pub last_visited_pane_per_ws: Vec<Option<u64>>,
    /// Whether mouse interactions are enabled.
    pub mouse_enabled: bool,
    /// Last frame render time for rate-limiting.
    pub last_render_time: Option<Instant>,
}
