//! Terminal pane shell + content rendering helpers.
//!
//! This module keeps terminal-specific pane composition out of the top-level
//! frame orchestrator. `render.rs` owns frame ordering and geometry collection;
//! this module owns the `Pane` shell contract and terminal content mounting.

use crate::app::selection_model::{SelectionOwner, SelectionRegion, SelectionState};
use crate::app::terminal_host::TerminalMount;
use crate::app_state::AppState;
use heca_config::theme::Color;
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

pub(crate) fn stable_tiled_content_rect(
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    content_inset: f32,
) -> Option<Rectangle> {
    pane_content_rect(px, py, pw, ph, content_inset).map(|(x, y, w, h)| {
        Rectangle::new(Point::new(x as f64, y as f64), Size::new(w as f64, h as f64))
    })
}

pub(crate) fn stable_floating_content_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    content_inset: f32,
) -> Option<Rectangle> {
    pane_content_rect(x, y, w, h, content_inset).map(|(cx, cy, cw, ch)| {
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
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    border_color: [f32; 4],
    border_width: f32,
    border_radius: f32,
    content_inset: f32,
    is_active: bool,
) {
    let theme = terminal_pane_gui_theme(state, border_color, border_width, border_radius);
    let mut pane = UiPane::new()
        .bordered()
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
    let mut cx = PaintCx::new(scene, &theme);
    pane.paint(&mut cx);
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
    } = render_ctx;
    let content_box = rect_to_text_box(mount.content_rect);
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
        terminal_renderer.render_snapshot(&mount.snapshot, content_box, terminal_style);
        if let Some(ref overlay) = selection_overlay {
            terminal_renderer.render_selection_overlay(
                overlay,
                content_box,
                mount.snapshot.cell_w,
                mount.snapshot.cell_h,
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
    primitive_renderer.render_clipped(device, view, encoder, clip_rect);
    text_renderer.render(queue, view, encoder);
    {
        let mut terminal_renderer = TerminalRenderer::new(text_renderer, primitive_renderer);
        terminal_renderer.render_cursor_overlay(&mount.snapshot, content_box);
    }
    primitive_renderer.render_clipped(device, view, encoder, clip_rect);
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
) -> Option<(f32, f32, f32, f32)> {
    let content_w = (pw - inset * 2.0).max(0.0);
    let content_h = (ph - inset * 2.0).max(0.0);
    if content_w <= 0.0 || content_h <= 0.0 {
        return None;
    }
    Some((px + inset, py + inset, content_w, content_h))
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
