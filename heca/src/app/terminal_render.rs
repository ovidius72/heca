//! Terminal pane shell + content rendering helpers.
//!
//! This module keeps terminal-specific pane composition out of the top-level
//! frame orchestrator. `render.rs` owns frame ordering and geometry collection;
//! this module owns the `Pane` shell contract and terminal content mounting.

use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionState};
use crate::app::terminal_host::TerminalMount;
use crate::app_state::AppState;
use heca_config::theme::Color;
use heca_core::backend::{TerminalDamage, TerminalSnapshot};
use heca_core::layout::{PaneId, Point, Rectangle, Size};
use heca_grid_ui::builders::{LayoutExt, StyleExt};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::Pane as UiPane;
use heca_grid_ui::{
    Color as GuiColor, Component, LayoutEngine, PaintCx, Point as GuiPoint,
    Rectangle as GuiRectangle, Scene as GuiScene, Size as GuiSize,
};
use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::terminal::{
    CaretIndicator, SelectionOverlay, SelectionOverlaySpan, TerminalRenderer, TerminalStyle,
};
use heca_renderer::text::{TextBox, TextRenderer};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

pub(crate) struct PaneRenderState {
    pub(crate) pane_id: PaneId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) is_active: bool,
    pub(crate) content_rect: Option<Rectangle>,
    pub(crate) mount: Option<TerminalMount>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalCopyBand {
    pub(crate) y: u32,
    pub(crate) height: u32,
}

pub(crate) fn sync_retained_terminal_layers(
    state: &mut AppState,
    panes: &[PaneRenderState],
    terminal_style: TerminalStyle<'_>,
    window_logical_size: (f32, f32),
    window_physical_size: winit::dpi::PhysicalSize<u32>,
) {
    retain_live_terminal_layers(state);

    let render_key = terminal_layer_render_key(&terminal_style);
    for pane in panes {
        let Some(mount) = pane.mount.as_ref() else {
            state.terminal_layers.remove(&pane.pane_id);
            continue;
        };

        let physical_size = retained_terminal_texture_size(mount.content_rect, state.scale_factor);
        // The offscreen scratch must match the pane's exact physical size. A
        // larger reused target would render the terminal at the wrong pixel
        // density and then crop the top-left subset during the copy into the
        // retained layer, which showed up as oversized glyphs while resizing and
        // hidden freshly typed content when damaged frames were presented live.
        state.terminal_layer_scratch.ensure_size(
            &state.device,
            state.surface_config.format,
            physical_size.width,
            physical_size.height,
        );

        let layer = state
            .terminal_layers
            .entry(pane.pane_id)
            .or_insert_with(|| {
                crate::app_state::RetainedTerminalLayer::new(
                    &state.device,
                    state.surface_config.format,
                    physical_size.width,
                    physical_size.height,
                    render_key,
                )
            });

        let resized = layer.ensure_size(
            &state.device,
            state.surface_config.format,
            physical_size.width,
            physical_size.height,
        );
        let style_changed = layer.render_key != render_key;
        if style_changed {
            layer.render_key = render_key;
        }

        let damage = if resized || style_changed {
            TerminalDamage::Full
        } else {
            mount.damage.clone()
        };
        if matches!(damage, TerminalDamage::None) {
            continue;
        }

        render_terminal_layer_update(
            state,
            pane.pane_id,
            mount,
            &damage,
            terminal_style,
            window_logical_size,
            window_physical_size,
        );
    }
}

pub(crate) fn blit_retained_terminal_layer(
    state: &AppState,
    pane_id: PaneId,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    viewport_px: (f32, f32),
    mount: &TerminalMount,
    stencil: Option<&wgpu::TextureView>,
) -> bool {
    let Some(layer) = state.terminal_layers.get(&pane_id) else {
        return false;
    };
    let rect = mount.content_rect;
    let scale = state.scale_factor as f32;
    state.backdrop.draw(
        &state.device,
        &state.queue,
        encoder,
        target,
        layer.view(),
        viewport_px,
        (
            rect.loc.x as f32 * scale,
            rect.loc.y as f32 * scale,
            rect.size.w as f32 * scale,
            rect.size.h as f32 * scale,
        ),
        Some([0.0, 0.0, 1.0, 1.0]),
        1.0,
        stencil,
    );
    true
}

pub(crate) fn queue_terminal_dynamic_overlays(
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    mount: &TerminalMount,
    selection_overlay: Option<SelectionOverlay>,
) {
    let content_box = rect_to_text_box(mount.content_rect);
    let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
    if let Some(ref overlay) = selection_overlay {
        terminal_renderer.render_selection_overlay(
            overlay,
            content_box,
            mount.snapshot.cell_w,
            mount.snapshot.cell_h,
        );
    }
    terminal_renderer.render_cursor_overlay(&mount.snapshot, content_box);
}

fn retain_live_terminal_layers(state: &mut AppState) {
    let mut live_panes = HashSet::new();
    for ws in &state.session.workspaces {
        for col in &ws.scrolling.columns {
            for pane in &col.panes {
                live_panes.insert(pane.id);
            }
        }
        for float in &ws.floating_panes {
            live_panes.insert(float.pane.id);
        }
    }
    state
        .terminal_layers
        .retain(|pane_id, _| live_panes.contains(pane_id));
}

fn terminal_layer_render_key(style: &TerminalStyle<'_>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    style.font_size.to_bits().hash(&mut hasher);
    style.surface_alpha.to_bits().hash(&mut hasher);
    style.families.normal.hash(&mut hasher);
    style.families.bold.hash(&mut hasher);
    style.families.italic.hash(&mut hasher);
    style.families.bold_italic.hash(&mut hasher);
    hasher.finish()
}

fn retained_terminal_texture_size(
    content_rect: Rectangle,
    scale_factor: f64,
) -> winit::dpi::PhysicalSize<u32> {
    let scale = scale_factor as f32;
    let width = (content_rect.size.w as f32 * scale).ceil().max(1.0) as u32;
    let height = (content_rect.size.h as f32 * scale).ceil().max(1.0) as u32;
    winit::dpi::PhysicalSize::new(width, height)
}

fn render_terminal_layer_update(
    state: &mut AppState,
    pane_id: PaneId,
    mount: &TerminalMount,
    damage: &TerminalDamage,
    terminal_style: TerminalStyle<'_>,
    window_logical_size: (f32, f32),
    window_physical_size: winit::dpi::PhysicalSize<u32>,
) {
    let logical_size = (
        mount.content_rect.size.w as f32,
        mount.content_rect.size.h as f32,
    );
    let physical_size = retained_terminal_texture_size(mount.content_rect, state.scale_factor);

    state
        .primitive_renderer
        .set_screen_size(&state.queue, logical_size.0, logical_size.1);
    state
        .text_renderer
        .set_screen_size(&state.queue, logical_size.0, logical_size.1);
    state
        .text_renderer
        .set_target_size(physical_size.width, physical_size.height);
    state.text_renderer.set_clip(None);
    state.text_renderer.set_damage(None);

    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("terminal_layer_update"),
        });
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("terminal_layer_clear_scratch"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: state.terminal_layer_scratch.view(),
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });

    let local_rect = Rectangle::new(
        Point::new(0.0, 0.0),
        Size::new(logical_size.0 as f64, logical_size.1 as f64),
    );
    {
        let mut terminal_renderer =
            TerminalRenderer::new(&mut state.text_renderer, &mut state.primitive_renderer);
        terminal_renderer.render_snapshot_damage(
            &mount.snapshot,
            rect_to_text_box(local_rect),
            terminal_style,
            damage,
        );
    }
    state.primitive_renderer.render(
        &state.device,
        state.terminal_layer_scratch.view(),
        &mut encoder,
    );
    state.text_renderer.render(
        &state.queue,
        state.terminal_layer_scratch.view(),
        &mut encoder,
        None,
    );

    let copy_bands = terminal_damage_copy_bands(
        damage,
        &mount.snapshot,
        logical_size.1,
        state.scale_factor,
        physical_size.height,
    );
    let layer_texture = state
        .terminal_layers
        .get(&pane_id)
        .expect("terminal layer must exist before updating")
        .texture();
    for band in copy_bands {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: state.terminal_layer_scratch.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: band.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: layer_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: band.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: physical_size.width,
                height: band.height,
                depth_or_array_layers: 1,
            },
        );
    }

    state.queue.submit(std::iter::once(encoder.finish()));
    state.primitive_renderer.set_screen_size(
        &state.queue,
        window_logical_size.0,
        window_logical_size.1,
    );
    state
        .text_renderer
        .set_screen_size(&state.queue, window_logical_size.0, window_logical_size.1);
    state
        .text_renderer
        .set_target_size(window_physical_size.width, window_physical_size.height);
}

fn terminal_damage_copy_bands(
    damage: &TerminalDamage,
    snapshot: &TerminalSnapshot,
    logical_height: f32,
    scale_factor: f64,
    texture_height: u32,
) -> Vec<TerminalCopyBand> {
    match damage {
        TerminalDamage::None => Vec::new(),
        TerminalDamage::Full => vec![TerminalCopyBand {
            y: 0,
            height: texture_height.max(1),
        }],
        TerminalDamage::Rows(ranges) => {
            let scale = scale_factor as f32;
            let max_height = logical_height.max(0.0);
            let mut bands = Vec::with_capacity(ranges.len());
            for range in ranges {
                let start = range.start.min(snapshot.rows);
                let end = range.end.min(snapshot.rows);
                if start >= end {
                    continue;
                }
                let top = ((start as f32 * snapshot.cell_h) * scale).floor() as u32;
                let bottom =
                    (((end as f32 * snapshot.cell_h).min(max_height)) * scale).ceil() as u32;
                let y = top.min(texture_height);
                let clipped_bottom = bottom.min(texture_height);
                if clipped_bottom > y {
                    bands.push(TerminalCopyBand {
                        y,
                        height: clipped_bottom - y,
                    });
                }
            }
            bands
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TerminalPaneShell {
    pub(crate) pane_id: PaneId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    pub(crate) border_color: [f32; 4],
    pub(crate) border_width: f32,
    pub(crate) border_radius: f32,
    pub(crate) content_inset: f32,
    pub(crate) is_active: bool,
}

/// Approx `Tag` internal vertical padding (each side) — for bar height/centering.
const BAR_TAG_VPAD: f32 = 5.0;
/// Vertical margin above + below the bar within its reserved strip.
const BAR_VMARGIN: f32 = 5.0;
/// Monospace line-height ratio (matches `heca-grid-ui`'s `MONO_LINE_RATIO`).
const BAR_LINE_RATIO: f32 = 1.4;

/// The info bar's content height for the given `font`.
fn title_bar_height(font: f32) -> f32 {
    font * BAR_LINE_RATIO + 2.0 * BAR_TAG_VPAD
}

/// Total vertical strip the info bar reserves at the pane top.
pub(crate) fn title_bar_reserve(font: f32) -> f32 {
    title_bar_height(font) + 2.0 * BAR_VMARGIN
}

/// The info bar font — the chrome theme's base font, the same size the sidebar
/// tree lays out with (`paint_chrome_root`), so the two read identically.
fn bar_font(state: &AppState) -> f32 {
    crate::chrome::chrome_gui_theme(state).font_size
}

/// Whether the pane info bar renders anything right now — segments **or** action
/// buttons. Drives both the reserved top strip and the header band/paint.
fn pane_info_bar_shown(state: &AppState) -> bool {
    !state.appearance.pane_title_segments.is_empty()
        || !state.appearance.pane_title_actions.is_empty()
}

/// Extra **top** content padding (logical px) reserved for the pane info bar, so
/// terminal content starts below it. Zero when the bar is hidden. Shared by the
/// render path and the mouse→cell mapping so the rendered grid and pointer
/// hit-testing use identical geometry.
pub(crate) fn pane_title_top_inset(state: &AppState) -> f32 {
    if pane_info_bar_shown(state) {
        title_bar_reserve(bar_font(state))
    } else {
        0.0
    }
}

pub(crate) fn stable_tiled_content_rect(
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    content_inset: f32,
    extra_top: f32,
) -> Option<Rectangle> {
    pane_content_rect(px, py, pw, ph, content_inset, extra_top).map(|(x, y, w, h)| {
        Rectangle::new(
            Point::new(x as f64, y as f64),
            Size::new(w as f64, h as f64),
        )
    })
}

pub(crate) fn stable_floating_content_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    content_inset: f32,
    extra_top: f32,
) -> Option<Rectangle> {
    pane_content_rect(x, y, w, h, content_inset, extra_top).map(|(cx, cy, cw, ch)| {
        Rectangle::new(
            Point::new(cx as f64, cy as f64),
            Size::new(cw as f64, ch as f64),
        )
    })
}

pub(crate) fn pane_scissor_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale_factor: f64,
    physical_size: winit::dpi::PhysicalSize<u32>,
) -> Option<(u32, u32, u32, u32)> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    let scale = scale_factor as f32;
    let left = (x.max(0.0) * scale).floor() as u32;
    let top = (y.max(0.0) * scale).floor() as u32;
    let right = ((x + w).max(0.0) * scale).ceil() as u32;
    let bottom = ((y + h).max(0.0) * scale).ceil() as u32;

    let clipped_left = left.min(physical_size.width);
    let clipped_top = top.min(physical_size.height);
    let clipped_right = right.min(physical_size.width);
    let clipped_bottom = bottom.min(physical_size.height);
    let clipped_width = clipped_right.saturating_sub(clipped_left);
    let clipped_height = clipped_bottom.saturating_sub(clipped_top);

    if clipped_width == 0 || clipped_height == 0 {
        return None;
    }

    Some((clipped_left, clipped_top, clipped_width, clipped_height))
}

pub(crate) fn paint_terminal_pane_shell(
    state: &AppState,
    scene: &mut GuiScene,
    shell: TerminalPaneShell,
) {
    let TerminalPaneShell {
        pane_id,
        x,
        y,
        w,
        h,
        border_color,
        border_width,
        border_radius,
        content_inset,
        is_active,
    } = shell;
    let theme = terminal_pane_gui_theme(state, border_color, border_width, border_radius);
    // Frame style is configurable (`pane_border_style`); width/color come from the
    // pane shell theme above (border_width already drives the global control).
    let mut pane = crate::chrome::apply_pane_frame(
        UiPane::new(),
        state.appearance.effective_pane_border_style(),
    )
        .width(heca_grid_ui::Length::Px(w))
        .height(heca_grid_ui::Length::Px(h))
        .padding(content_inset)
        .border(to_gui_color(border_color), border_width)
        .radius(border_radius);
    if is_active {
        pane = pane.glow_with(to_gui_color(border_color), 10.0, 0.55);
    }

    LayoutEngine::new().compute(&mut pane, GuiSize::new(w as f64, h as f64));
    pane.base_mut().bounds = GuiRectangle::new(
        GuiPoint::new(x as f64, y as f64),
        GuiSize::new(w as f64, h as f64),
    );

    let show_bar = pane_info_bar_shown(state);
    let bar_theme = crate::chrome::chrome_gui_theme(state);
    let font = bar_theme.font_size;

    {
        let mut cx = PaintCx::new(scene, &theme);
        // Distinguishable header band behind the title, drawn *before* the frame so
        // the rounded border traces over it. Theme-driven from the dedicated
        // `top_bottom_pane_background` token rather than the generic sidebar/card
        // surface.
        if show_bar {
            cx.rect(
                GuiRectangle::new(
                    GuiPoint::new(x as f64, y as f64),
                    GuiSize::new(w as f64, title_bar_reserve(font) as f64),
                ),
                to_gui_color(
                    state
                        .theme
                        .effective_top_bottom_pane_background()
                        .to_f32x4(),
                ),
                None,
                border_radius,
                None,
            );
        }
        pane.paint(&mut cx);
    }

    // Pane info-bar header (segments + action buttons): painted from the retained
    // per-pane tree that `chrome::sync_pane_headers` built + positioned earlier this
    // frame (before the GPU borrow). Clipped to the pane so the bar/buttons can't
    // spill into a neighbor when the pane is narrow (e.g. after a resize). The clip
    // is a *base-layer* scissor; a button's hover Tooltip draws on the **overlay**
    // layer (rendered after the base PopClip in `render_chrome`), so it still escapes
    // the pane and sits above neighbors. Interactivity is routed in `mouse.rs`.
    if show_bar && let Some(header) = state.pane_headers.get(&pane_id) {
        let clip = GuiRectangle::new(
            GuiPoint::new(x as f64, y as f64),
            GuiSize::new(w as f64, h as f64),
        );
        let mut cx = PaintCx::new(scene, &bar_theme);
        cx.with_clip(clip, |cx| header.root.paint(cx));
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

pub(crate) fn selection_overlay_for_pane(
    state: &AppState,
    pane_id: PaneId,
    cols: usize,
) -> Option<SelectionOverlay> {
    build_selection_overlay(&state.selection, pane_id, cols, &state.theme.accent)
}

fn pane_content_rect(
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    inset: f32,
    extra_top: f32,
) -> Option<(f32, f32, f32, f32)> {
    let content_w = (pw - inset * 2.0).max(0.0);
    let content_h = (ph - inset * 2.0 - extra_top).max(0.0);
    if content_w <= 0.0 || content_h <= 0.0 {
        return None;
    }
    Some((px + inset, py + inset + extra_top, content_w, content_h))
}

fn to_gui_color(color: [f32; 4]) -> GuiColor {
    GuiColor::new(
        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
        (color[3].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

fn terminal_pane_gui_theme(
    state: &AppState,
    border_color: [f32; 4],
    border_width: f32,
    border_radius: f32,
) -> GuiTheme {
    let mut theme = crate::chrome::chrome_gui_theme(state);
    theme.accent = to_gui_color(border_color);
    theme.border = to_gui_color(border_color);
    theme.radius = border_radius;
    theme.border_width = border_width;
    // The title's `Cut` style matches its surroundings against `theme.background`;
    // for a pane that means the real app/window background sitting behind it (the
    // reserved title strip shows the window backdrop, not the chrome grey).
    theme.background = to_gui_color(state.theme.background.to_f32x4());
    theme
}

fn rect_to_text_box(rect: Rectangle) -> TextBox {
    TextBox {
        x: rect.loc.x as f32,
        y: rect.loc.y as f32,
        w: rect.size.w as f32,
        h: rect.size.h as f32,
    }
}

// `pub(super)` so the selection-overlay unit tests (in `app::render`) can reach it
// after #118 moved this fn out of `render.rs`.
pub(super) fn build_selection_overlay(
    selection: &SelectionState,
    pane_id: PaneId,
    cols: usize,
    accent: &Color,
) -> Option<SelectionOverlay> {
    if cols == 0 {
        return None;
    }

    if let SelectionState::Caret { owner, row, col } = selection {
        if *owner != SelectionOwner::Pane(pane_id) {
            return None;
        }
        let color = [
            accent.r as f32 / 255.0,
            accent.g as f32 / 255.0,
            accent.b as f32 / 255.0,
            0.25,
        ];
        return Some(
            SelectionOverlay::new(vec![], color).with_caret(CaretIndicator {
                row: *row,
                col: *col,
                is_selection_endpoint: false,
            }),
        );
    }

    let active = selection.active()?;
    if active.owner != SelectionOwner::Pane(pane_id) {
        return None;
    }
    match &active.region {
        SelectionRegion::HostGrid {
            anchor_row,
            anchor_col,
            focus_row,
            focus_col,
        } => {
            let color = [
                accent.r as f32 / 255.0,
                accent.g as f32 / 255.0,
                accent.b as f32 / 255.0,
                0.25,
            ];
            let last_col = cols.saturating_sub(1);
            let mut spans = Vec::new();
            if anchor_row == focus_row {
                spans.push(SelectionOverlaySpan {
                    row: *anchor_row,
                    start_col: *anchor_col.min(focus_col),
                    end_col: (*anchor_col.max(focus_col)).min(last_col),
                });
            } else if anchor_row < focus_row {
                spans.push(SelectionOverlaySpan {
                    row: *anchor_row,
                    start_col: (*anchor_col).min(last_col),
                    end_col: last_col,
                });
                for row in (*anchor_row + 1)..*focus_row {
                    spans.push(SelectionOverlaySpan {
                        row,
                        start_col: 0,
                        end_col: last_col,
                    });
                }
                spans.push(SelectionOverlaySpan {
                    row: *focus_row,
                    start_col: 0,
                    end_col: (*focus_col).min(last_col),
                });
            } else {
                spans.push(SelectionOverlaySpan {
                    row: *focus_row,
                    start_col: (*focus_col).min(last_col),
                    end_col: last_col,
                });
                for row in (*focus_row + 1)..*anchor_row {
                    spans.push(SelectionOverlaySpan {
                        row,
                        start_col: 0,
                        end_col: last_col,
                    });
                }
                spans.push(SelectionOverlaySpan {
                    row: *anchor_row,
                    start_col: 0,
                    end_col: (*anchor_col).min(last_col),
                });
            }
            Some(
                SelectionOverlay::new(spans, color).with_caret(CaretIndicator {
                    row: *focus_row,
                    col: *focus_col,
                    is_selection_endpoint: true,
                }),
            )
        }
        SelectionRegion::BackendNative => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalCopyBand, build_selection_overlay, terminal_damage_copy_bands};
    use crate::app::selection_model::{
        SelectionOwner, SelectionRegion, SelectionSource, SelectionState,
    };
    use heca_config::theme::Color;
    use heca_core::backend::{TerminalDamage, TerminalSnapshot};
    use heca_core::layout::PaneId;

    fn snapshot(rows: usize, cell_h: f32) -> TerminalSnapshot {
        TerminalSnapshot {
            cols: 80,
            rows,
            cell_w: 8.0,
            cell_h,
            default_fg: [1.0; 4],
            default_bg: [0.0, 0.0, 0.0, 1.0],
            cursor_color: [1.0; 4],
            cursor: heca_core::backend::TerminalCursor {
                col: 0,
                row: 0,
                visible: true,
                shape: heca_core::backend::TerminalCursorShape::Block,
            },
            lines: Vec::new(),
            viewport_offset: 0,
            at_bottom: true,
            scrollback_rows: rows,
        }
    }

    #[test]
    fn build_selection_overlay_returns_none_when_inactive() {
        let selection = SelectionState::new();
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        assert!(build_selection_overlay(&selection, PaneId(1), 10, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_returns_none_when_owner_mismatch() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(7)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 2,
                anchor_col: 3,
                focus_row: 2,
                focus_col: 3,
            },
        );
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        // Query with a different pane id -> None
        assert!(build_selection_overlay(&selection, PaneId(1), 10, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_returns_none_for_backend_native() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::Rpc,
            SelectionRegion::BackendNative,
        );
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        assert!(build_selection_overlay(&selection, PaneId(1), 10, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_returns_overlay_for_host_grid_on_owning_pane() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 2,
                anchor_col: 3,
                focus_row: 2,
                focus_col: 3,
            },
        );
        selection.update_focus(5, 9);
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let overlay = build_selection_overlay(&selection, PaneId(1), 20, &accent);
        assert!(overlay.is_some());
        let overlay = overlay.unwrap();
        assert_eq!(overlay.spans.len(), 4);
        assert_eq!(overlay.spans[0].row, 2);
        assert_eq!(overlay.spans[0].start_col, 3);
        assert_eq!(overlay.spans[0].end_col, 19);
        assert_eq!(overlay.spans[3].row, 5);
        assert_eq!(overlay.spans[3].start_col, 0);
        assert_eq!(overlay.spans[3].end_col, 9);
        // Color should be accent / 255 with 0.25 alpha
        assert_eq!(
            overlay.color,
            [100.0 / 255.0, 150.0 / 255.0, 200.0 / 255.0, 0.25]
        );
    }

    #[test]
    fn build_selection_overlay_handles_reverse_multiline_selection() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::MouseDrag,
            SelectionRegion::HostGrid {
                anchor_row: 8,
                anchor_col: 12,
                focus_row: 8,
                focus_col: 12,
            },
        );
        selection.update_focus(4, 3);
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let overlay = build_selection_overlay(&selection, PaneId(1), 20, &accent).unwrap();
        assert_eq!(overlay.spans.len(), 5);
        assert_eq!(overlay.spans[0].row, 4);
        assert_eq!(overlay.spans[0].start_col, 3);
        assert_eq!(overlay.spans[0].end_col, 19);
        assert_eq!(overlay.spans[4].row, 8);
        assert_eq!(overlay.spans[4].start_col, 0);
        assert_eq!(overlay.spans[4].end_col, 12);
    }

    #[test]
    fn build_selection_overlay_caret_returns_caret_indicator() {
        let mut selection = SelectionState::new();
        selection.set_caret(SelectionOwner::Pane(PaneId(1)), 3, 7);
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let overlay = build_selection_overlay(&selection, PaneId(1), 20, &accent).unwrap();
        // Caret-only state: no selection spans.
        assert!(overlay.spans.is_empty());
        // But we get a caret indicator at the caret position.
        let caret = overlay
            .caret
            .expect("caret should be present in caret-only state");
        assert_eq!((caret.row, caret.col), (3, 7));
        assert!(
            !caret.is_selection_endpoint,
            "caret-only should not be a selection endpoint"
        );
    }

    #[test]
    fn build_selection_overlay_caret_owner_mismatch_returns_none() {
        let mut selection = SelectionState::new();
        selection.set_caret(SelectionOwner::Pane(PaneId(2)), 0, 0);
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        assert!(build_selection_overlay(&selection, PaneId(1), 20, &accent).is_none());
    }

    #[test]
    fn build_selection_overlay_active_selection_has_focus_caret() {
        let mut selection = SelectionState::new();
        selection.begin(
            SelectionOwner::Pane(PaneId(1)),
            SelectionSource::KeyboardMode,
            SelectionRegion::HostGrid {
                anchor_row: 2,
                anchor_col: 0,
                focus_row: 4,
                focus_col: 5,
            },
        );
        let accent = Color {
            r: 100,
            g: 150,
            b: 200,
            a: 255,
        };
        let overlay = build_selection_overlay(&selection, PaneId(1), 20, &accent).unwrap();
        // Active selection: should have both selection spans and a focus-end caret.
        assert!(!overlay.spans.is_empty());
        let caret = overlay
            .caret
            .expect("focus caret should be present for active selection");
        assert_eq!((caret.row, caret.col), (4, 5));
        assert!(
            caret.is_selection_endpoint,
            "active selection caret should be a selection endpoint"
        );
    }

    #[test]
    fn terminal_damage_copy_bands_full_covers_entire_texture() {
        let bands =
            terminal_damage_copy_bands(&TerminalDamage::Full, &snapshot(3, 12.0), 36.0, 2.0, 72);
        assert_eq!(bands, vec![TerminalCopyBand { y: 0, height: 72 }]);
    }

    #[test]
    fn terminal_damage_copy_bands_rows_convert_row_ranges_to_pixel_bands() {
        let damage = TerminalDamage::Rows(vec![
            heca_core::backend::TerminalRowRange::new(1, 3),
            heca_core::backend::TerminalRowRange::new(4, 5),
        ]);
        let bands = terminal_damage_copy_bands(&damage, &snapshot(5, 10.0), 50.0, 2.0, 100);
        assert_eq!(
            bands,
            vec![
                TerminalCopyBand { y: 20, height: 40 },
                TerminalCopyBand { y: 80, height: 20 },
            ]
        );
    }

    #[test]
    fn terminal_damage_copy_bands_clamps_rows_to_visible_height() {
        let damage = TerminalDamage::Rows(vec![heca_core::backend::TerminalRowRange::new(2, 8)]);
        let bands = terminal_damage_copy_bands(&damage, &snapshot(4, 12.0), 48.0, 1.0, 48);
        assert_eq!(bands, vec![TerminalCopyBand { y: 24, height: 24 }]);
    }
}
