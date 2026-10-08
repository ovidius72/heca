//! **Drawing one terminal where the scene put it**: its retained texture, or a full render for a
//! terminal with none yet.

mod selection;

use crate::app::terminal_host::TerminalMount;
use crate::app::terminal_render::layers::{
    blit_retained_terminal_layer, queue_terminal_dynamic_overlays,
};
use crate::app::terminal_render::{TerminalRenderState, pane_scissor_rect, rect_to_text_box};
use crate::app_state::{AppState, InputMode};
use heca_core::layout::Rectangle;
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::terminal::{
    HyperlinkDecor, SelectionOverlay, TerminalFontFamilies, TerminalRenderer, TerminalStyle,
};
use heca_renderer::text::TextRenderer;
use selection::selection_overlay_for_terminal;

/// Project the terminal font-family group from config into the renderer's
/// per-style family slots. The renderer stays config-free; this is the app-side
/// bridge. Takes the whole `FontFamilies` so the `normal` slot can be resolved
/// with the **terminal** embedded fallback (`terminal_normal()`), not the UI
/// fallback — omitting `[font.family.terminal].normal` keeps Maple Mono, not
/// Geist Mono. Borrows from `families` so the returned slots live as long as it.
pub(crate) fn terminal_font_families_from(
    families: &heca_config::font::FontFamilies,
) -> TerminalFontFamilies<'_> {
    let tf = &families.terminal;
    TerminalFontFamilies {
        normal: families.terminal_normal(),
        bold: tf.bold.as_deref(),
        italic: tf.italic.as_deref(),
        bold_italic: tf.bold_italic.as_deref(),
    }
}

/// Map the config hyperlink decoration onto the renderer's enum.
pub(crate) fn hyperlink_decor_from(
    style: heca_config::appearance::HyperlinkStyle,
) -> HyperlinkDecor {
    use heca_config::appearance::HyperlinkStyle as S;
    match style {
        S::None => HyperlinkDecor::None,
        S::Color => HyperlinkDecor::Color,
        S::Underline => HyperlinkDecor::Underline,
        S::Undercurl => HyperlinkDecor::Undercurl,
    }
}

/// **Where and how one terminal is drawn**: the texture it goes into, the mask and scissor that
/// round it to its pane, and how see-through it is. A tiled pane and a floating one differ only in
/// these, so they are one value, not two code paths.
pub(crate) struct TerminalTarget<'a> {
    pub(crate) view: &'a wgpu::TextureView,
    /// The rounded content-clip mask the terminal tests against. A pane has one; a terminal in a
    /// dock or an overlay has none and is clipped by the scene alone.
    pub(crate) stencil: Option<&'a wgpu::TextureView>,
    /// The scissor its cursor and selection are flushed under.
    pub(crate) scissor: Option<(u32, u32, u32, u32)>,
    pub(crate) surface_alpha: f32,
    /// Everything a terminal draws is kept inside this (the content area).
    pub(crate) content_clip: Rectangle,
    /// Whether to keep it inside what clips it **in its scene** instead — a terminal in a dock or
    /// an overlay is clipped by the scroll region or the panel around it, not by the content area.
    pub(crate) follow_scene_clip: bool,
}

/// **Draw the terminal surface `surface` a scene asked for**, if it is one of `panes`. A surface
/// nobody can draw is left out rather than guessed at.
pub(crate) fn draw_surface(
    state: &mut AppState,
    terminals: &[TerminalRenderState],
    surface: &heca_grid_ui::SurfaceAt,
    target: &TerminalTarget<'_>,
    encoder: &mut wgpu::CommandEncoder,
) {
    if let Some(terminal) = terminals.iter().find(|t| t.id.0 == surface.id) {
        let clip = surface.clip.filter(|_| target.follow_scene_clip);
        draw_terminal(state, terminal, surface.rect, target, clip, encoder);
    }
}

/// **Draw one terminal where the scene put it**: its retained texture, then its cursor and
/// selection over it — or, for a terminal with no retained layer yet, the full render.
///
/// `at` is where the scene put it. A terminal with nothing to draw (no room or no snapshot yet) is
/// skipped.
pub(crate) fn draw_terminal(
    state: &mut AppState,
    pane: &TerminalRenderState,
    at: Rectangle,
    target: &TerminalTarget<'_>,
    clip: Option<Rectangle>,
    encoder: &mut wgpu::CommandEncoder,
) {
    let (Some(room), Some(mount)) = (pane.content_rect, pane.mount.as_ref()) else {
        return;
    };
    // Its size is what the layout gave it; its place is where the scene drew it.
    let rect = Rectangle::new(at.loc, room.size);
    let surface_physical_size = state.window.inner_size();
    // Any terminal's, whoever owns it: the selection is a terminal's, asked of its id.
    let selection_overlay = selection_overlay_for_terminal(state, pane.id, &mount.snapshot);
    let font_size = state.terminal_font_size(pane.pane);
    // Clipped by its scene (a dock, an overlay) or by the target's own box (a pane).
    let content_clip = clip.unwrap_or(target.content_clip);
    let scissor = match clip {
        Some(c) => pane_scissor_rect(
            c.loc.x as f32,
            c.loc.y as f32,
            c.size.w as f32,
            c.size.h as f32,
            state.scale_factor,
            surface_physical_size,
        ),
        None => target.scissor,
    };
    if blit_retained_terminal_layer(
        state,
        pane.id,
        encoder,
        target,
        (
            surface_physical_size.width as f32,
            surface_physical_size.height as f32,
        ),
        rect,
        clip,
    ) {
        queue_terminal_dynamic_overlays(
            &mut state.text_renderer,
            &mut state.primitive_renderer,
            mount,
            rect,
            selection_overlay,
            matches!(state.input_mode, InputMode::Selection),
        );
        state.primitive_renderer.render_clipped(
            &state.device,
            target.view,
            encoder,
            scissor,
            target.stencil,
        );
    } else {
        render_terminal_mount(
            TerminalRenderPassContext {
                text_renderer: &mut state.text_renderer,
                primitive_renderer: &mut state.primitive_renderer,
                device: &state.device,
                queue: &state.queue,
                view: target.view,
                encoder,
                scale_factor: state.scale_factor,
                surface_physical_size,
                content_clip,
                stencil: target.stencil,
            },
            TerminalStyle {
                font_size,
                families: terminal_font_families_from(&state.font_config.family),
                surface_alpha: target.surface_alpha,
                ligatures: state.appearance.terminal.ligatures,
                hyperlink_style: hyperlink_decor_from(state.appearance.terminal.hyperlink_style),
                hyperlink_color: state
                    .appearance
                    .terminal
                    .hyperlink_color
                    .unwrap_or(state.theme.accent)
                    .to_f32x4(),
            },
            TerminalMount {
                content_rect: rect,
                ..mount.clone()
            },
            selection_overlay,
        );
    }
}

pub(crate) struct TerminalRenderPassContext<'a> {
    pub(crate) text_renderer: &'a mut TextRenderer,
    pub(crate) primitive_renderer: &'a mut PrimitiveRenderer,
    pub(crate) device: &'a wgpu::Device,
    pub(crate) queue: &'a wgpu::Queue,
    pub(crate) view: &'a wgpu::TextureView,
    pub(crate) encoder: &'a mut wgpu::CommandEncoder,
    pub(crate) scale_factor: f64,
    pub(crate) surface_physical_size: winit::dpi::PhysicalSize<u32>,
    pub(crate) content_clip: Rectangle,
    /// Rounded content-clip mask. When `Some`, terminal surface/cells/selection/cursor
    /// + glyphs test against it so content follows the pane's rounded border.
    pub(crate) stencil: Option<&'a wgpu::TextureView>,
}

pub(crate) fn render_terminal_mount(
    render_ctx: TerminalRenderPassContext<'_>,
    terminal_style: TerminalStyle<'_>,
    mount: TerminalMount,
    selection_overlay: Option<SelectionOverlay>,
) {
    let TerminalRenderPassContext {
        text_renderer,
        primitive_renderer,
        device,
        queue,
        view,
        encoder,
        scale_factor,
        surface_physical_size,
        content_clip,
        stencil,
    } = render_ctx;
    let TerminalMount {
        content_rect,
        snapshot,
        damage: _damage,
    } = mount;
    let content_box = rect_to_text_box(content_rect);
    let clip_x = content_box.x.max(content_clip.loc.x as f32);
    let clip_y = content_box.y.max(content_clip.loc.y as f32);
    let clip_right =
        (content_box.x + content_box.w).min((content_clip.loc.x + content_clip.size.w) as f32);
    let clip_bottom =
        (content_box.y + content_box.h).min((content_clip.loc.y + content_clip.size.h) as f32);
    let clip_w = (clip_right - clip_x).max(0.0);
    let clip_h = (clip_bottom - clip_y).max(0.0);
    {
        text_renderer.set_clip(Some([clip_x, clip_y, clip_w, clip_h]));
        let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
        terminal_renderer.render_snapshot(&snapshot, content_box, terminal_style);
        if let Some(ref overlay) = selection_overlay {
            terminal_renderer.render_selection_overlay(
                overlay,
                content_box,
                snapshot.cell_w,
                snapshot.cell_h,
            );
        }
        text_renderer.set_clip(None);
    }

    let clip_rect = pane_scissor_rect(
        clip_x,
        clip_y,
        clip_w,
        clip_h,
        scale_factor,
        surface_physical_size,
    );
    primitive_renderer.render_clipped(device, view, encoder, clip_rect, stencil);
    text_renderer.render(queue, view, encoder, stencil);
    {
        let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
        terminal_renderer.render_cursor_overlay(&snapshot, content_box);
    }
    primitive_renderer.render_clipped(device, view, encoder, clip_rect, stencil);
}
