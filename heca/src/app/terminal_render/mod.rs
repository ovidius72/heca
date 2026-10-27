//! Terminal pane shell + content rendering helpers.
//!
//! This module keeps terminal-specific pane composition out of the top-level
//! frame orchestrator. `render.rs` owns frame ordering and geometry collection;
//! this module owns the `Pane` shell contract and terminal content mounting.

mod draw;
mod layers;
mod look;

pub(crate) use draw::{
    draw_surface, hyperlink_decor_from, terminal_font_families_from, TerminalTarget,
};
pub(crate) use layers::sync_retained_terminal_layers;
pub(crate) use look::pane_scissor_rect;

use crate::app::terminal_host::TerminalMount;
use crate::app_state::AppState;
use crate::chrome::panes::terminal::TerminalId;
use heca_core::layout::{PaneId, Rectangle};
use heca_renderer::text::TextBox;

pub(crate) struct TerminalRenderState {
    /// The terminal process this draws, by the id the store issued.
    pub(crate) id: TerminalId,
    /// How see-through its cells' background is: a floating pane's terminal covers what is under
    /// it more than a tiled one. Its own, so wherever it is drawn it looks the same.
    pub(crate) opacity: f32,
    /// The room its terminal's **grid** was given (the layout's box held in by its configured
    /// padding), relative to where the surface is drawn: its size is what is known before the
    /// scene draws it; the padding offset and the surface's place come with the surface the
    /// scene asks for.
    pub(crate) content_rect: Option<Rectangle>,
    pub(crate) mount: Option<TerminalMount>,
}

impl TerminalRenderState {
    /// **The terminal as this frame draws it**: how see-through it is, how much room it was given,
    /// and the snapshot to draw. Also tells the chrome store the terminal's scroll position, so
    /// what shows it follows.
    ///
    /// **The terminal's box is the terminal's own.** How much room it has comes from the layout,
    /// for every pane whether or not it is on screen; where it is drawn comes from the surface the
    /// scene asks for, when the scene is flushed.
    pub(crate) fn collect(
        state: &mut AppState,
        id: TerminalId,
        pane: Option<PaneId>,
        opacity: f32,
    ) -> Self {
        let content_rect = crate::chrome::panes::terminal::content_box_of(state, id);
        let mount = content_rect.and_then(|content_rect| {
            // Read the terminal through the server interface, not by reaching into its backends:
            // the server owns the process and hands back the snapshot and the rows that changed.
            let (snapshot, damage) = state.client.terminal_view(id, state.scale_factor as f32)?;
            // The surface's own box, padding included: the background fills it while the
            // grid above stays held in by `content_rect`.
            let surface_rect = crate::chrome::panes::terminal::full_box_of(state, id)?;
            Some(crate::app::terminal_host::TerminalMount {
                content_rect,
                surface_rect,
                snapshot,
                damage,
            })
        });
        if let (Some(m), Some(pane)) = (&mount, pane) {
            state.chrome_state.panes.set_viewport(
                pane,
                m.snapshot.viewport_offset,
                m.snapshot.at_bottom,
                m.snapshot.scrollback_rows,
            );
        }
        Self {
            id,
            opacity,
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
