use heca_config::theme::Theme;
use heca_core::pane::PaneTree;
use heca_core::types::Rect;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;
use std::sync::Arc;
use winit::keyboard::ModifiersState;
use winit::window::Window;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Prefix,
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
        start_mouse: (f32, f32),
        start_rect: Rect,
    },
}

pub struct AppState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub primitive_renderer: PrimitiveRenderer,
    pub text_renderer: TextRenderer,
    pub panetree: PaneTree,
    pub theme: Theme,
    pub scale_factor: f64,
    pub needs_redraw: bool,
    pub focused_pane: Option<u64>,
    pub input_mode: InputMode,
    pub drag_state: DragState,
    pub sidebar: SidebarState,
    pub active_tab: usize,
    pub tab_names: Vec<String>,
    pub mouse_pos: (f32, f32),
    pub modifiers: ModifiersState,
}
