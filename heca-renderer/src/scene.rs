//! Bridge from `heca-grid-ui`'s display list to the GPU renderers.
//!
//! [`enqueue_scene`] walks a [`Scene`] and translates each [`DrawCommand`] into
//! `GridRenderer` (rounded rects, glow, brackets, scanlines) and `TextRenderer`
//! calls. The caller then invokes each renderer's `render()` as usual.

use crate::grid::{GlowRect, GridRenderer};
use crate::text::TextRenderer;
use heca_grid_ui::scene::{BracketCmd, DrawCommand, FontRole, ScanlineCmd};
use heca_grid_ui::{Rectangle, Scene};

const NO_BORDER: [f32; 4] = [0.0; 4];
const NO_GLOW: [f32; 4] = [0.0; 4];
const NO_SHADOW: [f32; 4] = [0.0; 4];

/// Translate a logical `Rectangle` to `(x, y, w, h)` f32 tuple.
fn xywh(rect: &Rectangle) -> (f32, f32, f32, f32) {
    (
        rect.loc.x as f32,
        rect.loc.y as f32,
        rect.size.w as f32,
        rect.size.h as f32,
    )
}

/// A plain (no border, no glow, sharp) filled rect helper.
fn solid(x: f32, y: f32, w: f32, h: f32, fill: [f32; 4]) -> GlowRect {
    GlowRect {
        x,
        y,
        w,
        h,
        fill,
        border: NO_BORDER,
        border_width: 0.0,
        radius: 0.0,
        glow: NO_GLOW,
        glow_radius: 0.0,
        glow_intensity: 0.0,
        shadow: NO_SHADOW,
        shadow_radius: 0.0,
        shadow_offset: [0.0, 0.0],
    }
}

/// Walk `scene` and enqueue its commands into the renderers.
pub fn enqueue_scene(grid: &mut GridRenderer, text: &mut TextRenderer, scene: &Scene) {
    for cmd in scene.iter() {
        match cmd {
            DrawCommand::Rect(r) => {
                let (x, y, w, h) = xywh(&r.rect);
                let (border, border_width) = match r.border {
                    Some(b) => (b.color.to_f32x4(), b.width),
                    None => (NO_BORDER, 0.0),
                };
                let (glow, glow_radius, glow_intensity) = match r.glow {
                    Some(g) => (g.color.to_f32x4(), g.radius, g.intensity),
                    None => (NO_GLOW, 0.0, 0.0),
                };
                let (shadow, shadow_radius, shadow_offset) = match r.shadow {
                    Some(s) => (s.color.to_f32x4(), s.radius, [s.dx, s.dy]),
                    None => (NO_SHADOW, 0.0, [0.0, 0.0]),
                };
                grid.draw(GlowRect {
                    x,
                    y,
                    w,
                    h,
                    fill: r.fill.to_f32x4(),
                    border,
                    border_width,
                    radius: r.radius,
                    glow,
                    glow_radius,
                    glow_intensity,
                    shadow,
                    shadow_radius,
                    shadow_offset,
                });
            }
            DrawCommand::Brackets(b) => draw_brackets(grid, b),
            DrawCommand::Scanline(s) => draw_scanlines(grid, s),
            DrawCommand::Text(t) => {
                let (x, y, w, h) = xywh(&t.rect);
                text.queue_text_in_box(
                    &t.text,
                    x,
                    y,
                    w,
                    h,
                    t.size,
                    t.color.to_f32x4(),
                    t.bold,
                    t.align,
                    t.font == FontRole::Icon,
                );
            }
            // Clipping isn't supported by the renderers yet (planned).
            DrawCommand::PushClip(_) | DrawCommand::PopClip => {}
        }
    }
}

/// Eight thin arms framing the rect's corners (Tron reticle).
fn draw_brackets(grid: &mut GridRenderer, b: &BracketCmd) {
    let (x, y, w, h) = xywh(&b.rect);
    let color = b.color.to_f32x4();
    let (gc, gr, gi) = match b.glow {
        Some(g) => (g.color.to_f32x4(), g.radius, g.intensity),
        None => (NO_GLOW, 0.0, 0.0),
    };
    let t = b.thickness;
    let l = b.len;

    let arm = |gx: f32, gy: f32, gw: f32, gh: f32| GlowRect {
        x: gx,
        y: gy,
        w: gw,
        h: gh,
        fill: color,
        border: NO_BORDER,
        border_width: 0.0,
        radius: 0.0,
        glow: gc,
        glow_radius: gr,
        glow_intensity: gi,
        shadow: NO_SHADOW,
        shadow_radius: 0.0,
        shadow_offset: [0.0, 0.0],
    };

    // Top-left
    grid.draw(arm(x, y, l, t));
    grid.draw(arm(x, y, t, l));
    // Top-right
    grid.draw(arm(x + w - l, y, l, t));
    grid.draw(arm(x + w - t, y, t, l));
    // Bottom-left
    grid.draw(arm(x, y + h - t, l, t));
    grid.draw(arm(x, y + h - l, t, l));
    // Bottom-right
    grid.draw(arm(x + w - l, y + h - t, l, t));
    grid.draw(arm(x + w - t, y + h - l, t, l));
}

/// Horizontal scanlines across a region.
fn draw_scanlines(grid: &mut GridRenderer, s: &ScanlineCmd) {
    if s.opacity <= 0.0 || s.spacing <= 0.0 {
        return;
    }
    let (x, y, w, h) = xywh(&s.rect);
    let mut fill = s.color.to_f32x4();
    fill[3] *= s.opacity;

    let mut ly = y;
    let max_lines = 4096; // safety cap
    let mut drawn = 0;
    while ly < y + h && drawn < max_lines {
        grid.draw(solid(x, ly, w, 1.0, fill));
        ly += s.spacing;
        drawn += 1;
    }
}
