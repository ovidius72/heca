//! The window's state, and the small types it is built from.

use crate::app::events::AppEvent;
pub use crate::app::selection_model::SelectionState;
use heca_config::appearance::AppearanceConfig;
use heca_config::font::FontConfig;
use heca_config::theme::Theme;
use heca_core::layout::{Layout, LayoutMut, PaneId, Session, WindowView};
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


mod input_mode;
mod layout_door;
mod metrics;
mod mouse_state;
mod reaction;
mod search;
mod state;
mod terminal_layers;

pub use input_mode::*;
pub use mouse_state::*;
pub use search::*;
pub use state::*;
pub use terminal_layers::*;
pub(crate) use reaction::{Tracking, show_workspace};

#[cfg(test)]
mod tests;
