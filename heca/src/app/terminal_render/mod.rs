//! Terminal pane shell + content rendering helpers.
//!
//! This module keeps terminal-specific pane composition out of the top-level
//! frame orchestrator. `render.rs` owns frame ordering and geometry collection;
//! this module owns the `Pane` shell contract and terminal content mounting.

mod draw;
mod layers;
mod look;

pub(crate) use draw::{
    TerminalTarget, draw_surface, hyperlink_decor_from, terminal_font_families_from,
};
pub(crate) use layers::sync_retained_terminal_layers;
pub(crate) use look::pane_scissor_rect;

use crate::app::terminal_host::TerminalMount;
use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;
use heca_core::layout::{PaneId, Rectangle};
use heca_renderer::text::TextBox;

pub(crate) struct TerminalRenderState {
    /// The terminal process this draws, by the id the store issued.
    pub(crate) id: TerminalId,
    /// The pane it belongs to, if one does: what only a pane has (zoom, selection) is asked of it.
    pub(crate) pane: Option<PaneId>,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    /// The room its terminal was given, at the origin: its size is what is known before the scene
    /// draws it; where it is comes with the surface the scene asks for.
    pub(crate) content_rect: Option<Rectangle>,
    pub(crate) mount: Option<TerminalMount>,
}

impl TerminalRenderState {
    /// **The pane as this frame draws it**, whether tiled or floating: its box `(x, y, w, h)`, how
    /// much room its terminal was given, and the snapshot to draw. Also tells the chrome store the terminal's scroll position, so what shows it follows.
    ///
    /// **The terminal's box is the terminal's own.** How much room it has comes from the layout,
    /// for every pane whether or not it is on screen; where it is drawn comes from the surface the
    /// scene asks for, when the scene is flushed.
    pub(crate) fn collect(
        state: &mut AppState,
        id: TerminalId,
        pane: Option<PaneId>,
        (x, y, w, h): (f32, f32, f32, f32),
    ) -> Self {
        let content_rect = crate::chrome::terminal::room_of(state, id)
            .map(|room| Rectangle::new(heca_core::layout::Point::new(0.0, 0.0), room));
        let mount = content_rect.and_then(|content_rect| {
            crate::app::terminal_host::prepare_terminal_mount(
                &mut state.backends,
                id,
                content_rect,
                state.scale_factor as f32,
            )
        });
        if let (Some(m), Some(pane)) = (&mount, pane) {
            state.chrome_state.workspaces.set_pane_viewport(
                pane,
                m.snapshot.viewport_offset,
                m.snapshot.at_bottom,
                m.snapshot.scrollback_rows,
            );
        }
        Self {
            id,
            pane,
            x,
            y,
            w,
            h,
            content_rect,
            mount,
        }
    }
}

/// A rectangle as the text renderer's box.
fn rect_to_text_box(rect: Rectangle) -> TextBox {
    TextBox {
        x: rect.loc.x as f32,
        y: rect.loc.y as f32,
        w: rect.size.w as f32,
        h: rect.size.h as f32,
    }
}
