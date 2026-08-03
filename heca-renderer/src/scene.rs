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
const LIGHT_BG_GLOW_ALPHA_SCALE: f32 = 0.62;
const LIGHT_BG_GLOW_RADIUS_SCALE: f32 = 1.25;
const LIGHT_BG_LUMA_THRESHOLD: f32 = 0.60;

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
        glow_alpha_scale: 0.0,
        shadow: NO_SHADOW,
        shadow_radius: 0.0,
        shadow_offset: [0.0, 0.0],
    }
}

fn srgb_channel_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(color: [f32; 4]) -> f32 {
    let r = srgb_channel_to_linear(color[0]);
    let g = srgb_channel_to_linear(color[1]);
    let b = srgb_channel_to_linear(color[2]);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Choose the glow compositing mode for a scene background.
///
/// Dark backgrounds keep the original additive neon. Light backgrounds switch
/// to a translucent tinted halo so the glow remains visible instead of washing
/// out against near-white surfaces.
pub fn glow_alpha_scale_for_background(background: [f32; 4]) -> f32 {
    if relative_luminance(background) >= LIGHT_BG_LUMA_THRESHOLD {
        LIGHT_BG_GLOW_ALPHA_SCALE
    } else {
        0.0
    }
}

/// Intersection of two logical `[x, y, w, h]` clip rects (empty if disjoint).
fn intersect_clip(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

/// Walk `scene` and enqueue its commands into the renderers.
///
/// `glow_alpha_scale` selects the glow compositing strategy for this scene:
/// `0.0` keeps additive-only glow (best on dark themes); non-zero emits glows
/// as translucent tinted halos so they remain visible on light backgrounds.
pub fn enqueue_scene(
    grid: &mut GridRenderer,
    text: &mut TextRenderer,
    scene: &Scene,
    glow_alpha_scale: f32,
) {
    // Active clip rects (each already intersected with its parent), so nested
    // `PushClip`s clip to their intersection. Both renderers scissor to the top.
    // Glyph halos switch composite mode with the background exactly as rect glows
    // do — otherwise a halo that adds light is invisible on a light theme.
    text.set_glow_alpha_scale(glow_alpha_scale);
    let mut clip_stack: Vec<[f32; 4]> = Vec::new();
    for cmd in scene.iter() {
        match cmd {
            DrawCommand::Rect(r) => {
                let (x, y, w, h) = xywh(&r.rect);
                let (border, border_width) = match r.border {
                    Some(b) => (b.color.to_f32x4(), b.width),
                    None => (NO_BORDER, 0.0),
                };
                let (glow, mut glow_radius, glow_intensity) = match r.glow {
                    Some(g) => (g.color.to_f32x4(), g.radius, g.intensity),
                    None => (NO_GLOW, 0.0, 0.0),
                };
                if glow_alpha_scale > 0.0 {
                    glow_radius *= LIGHT_BG_GLOW_RADIUS_SCALE;
                }
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
                    glow_alpha_scale,
                    shadow,
                    shadow_radius,
                    shadow_offset,
                });
            }
            DrawCommand::Brackets(b) => {
                draw_brackets(grid, b, glow_alpha_scale);
            }
            DrawCommand::Scanline(s) => draw_scanlines(grid, s),
            DrawCommand::Text(t) => {
                let (x, y, w, h) = xywh(&t.rect);
                // The scene names a **role**; which family that is lives here, so no widget knows a
                // font's name. `Icon` has its own flag (the icon family is resolved inside the text
                // renderer); `NerdFont` rides the per-run family override the terminal path already
                // uses, so it needed no new plumbing.
                let (icon, family) = match t.font {
                    FontRole::Text => (false, None),
                    FontRole::Icon => (true, None),
                    FontRole::NerdFont => (false, Some(crate::font::NERD_FONT_FAMILY)),
                };
                text.queue_text_in_box(
                    &t.text,
                    x,
                    y,
                    w,
                    h,
                    t.size,
                    t.color.to_f32x4(),
                    t.style.bold,
                    t.style.italic,
                    t.align,
                    icon,
                    family,
                    // The scene asks for a halo declaratively; the text renderer
                    // decides how to realize it (see `TextGlow`).
                    t.glow.map(|g| crate::text::TextGlow {
                        color: g.color.to_f32x4(),
                        radius: g.radius,
                        intensity: g.intensity,
                    }),
                );
            }
            DrawCommand::PushClip(r) => {
                let (x, y, w, h) = xywh(r);
                let rect = [x, y, w, h];
                // Intersect with the enclosing clip so nested clips never exceed it.
                let eff = clip_stack
                    .last()
                    .map_or(rect, |&prev| intersect_clip(prev, rect));
                clip_stack.push(eff);
                grid.set_clip(Some(eff));
                text.set_clip(Some(eff));
            }
            DrawCommand::PopClip => {
                clip_stack.pop();
                let eff = clip_stack.last().copied();
                grid.set_clip(eff);
                text.set_clip(eff);
            }
        }
    }
    // Defensive: clear any unbalanced clip so it can't leak.
    if !clip_stack.is_empty() {
        grid.set_clip(None);
        text.set_clip(None);
    }
}

/// Eight thin arms framing the rect's corners (Tron reticle).
fn draw_brackets(
    grid: &mut GridRenderer,
    b: &BracketCmd,
    glow_alpha_scale: f32,
) {
    let (x, y, w, h) = xywh(&b.rect);
    let color = b.color.to_f32x4();
    let (gc, mut gr, gi) = match b.glow {
        Some(g) => (g.color.to_f32x4(), g.radius, g.intensity),
        None => (NO_GLOW, 0.0, 0.0),
    };
    if glow_alpha_scale > 0.0 {
        gr *= LIGHT_BG_GLOW_RADIUS_SCALE;
    }
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
        glow_alpha_scale,
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

#[cfg(test)]
mod tests {
    use super::{LIGHT_BG_GLOW_ALPHA_SCALE, glow_alpha_scale_for_background, intersect_clip};

    #[test]
    fn intersect_clip_returns_the_overlapping_region() {
        // Two overlapping rects → their intersection.
        let a = [0.0, 0.0, 100.0, 100.0];
        let b = [40.0, 30.0, 100.0, 100.0];
        assert_eq!(
            intersect_clip(a, b),
            [40.0, 30.0, 60.0, 70.0],
            "intersection should be the overlapping box"
        );
    }

    #[test]
    fn intersect_clip_is_empty_when_disjoint() {
        // Disjoint rects → zero-area (clamped, never negative).
        let a = [0.0, 0.0, 10.0, 10.0];
        let b = [50.0, 50.0, 10.0, 10.0];
        let r = intersect_clip(a, b);
        assert_eq!(
            (r[2], r[3]),
            (0.0, 0.0),
            "disjoint clips intersect to nothing"
        );
    }

    #[test]
    fn dark_backgrounds_keep_additive_glow() {
        assert!(
            glow_alpha_scale_for_background([0.02, 0.03, 0.05, 1.0]).abs() < f32::EPSILON,
            "dark scenes should keep additive-only glow"
        );
    }

    #[test]
    fn light_backgrounds_switch_to_tinted_glow() {
        assert_eq!(
            glow_alpha_scale_for_background([0.94, 0.95, 0.97, 1.0]),
            LIGHT_BG_GLOW_ALPHA_SCALE,
            "light scenes should use the tinted halo glow path"
        );
    }
}
