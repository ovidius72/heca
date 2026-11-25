//! Shared clip helpers for the render passes, and **the frame's damage** — the one place every
//! renderer reads what part of the scene a frame repaints.
//!
//! [`intersect`] and [`combine_clip`] operate on logical `[x, y, w, h]` rects
//! (empty — zero area — when disjoint). [`combine_clip`] folds the frame damage
//! region together with a per-draw clip; `None` means "unbounded".
//!
//! # The frame's damage
//!
//! The scene texture is kept between frames, so a frame that changed one terminal redraws only
//! that terminal's box: the host sets the damage with [`set_frame_damage`] around the passes that
//! draw into the scene, and **every renderer limits its own passes to it** — the grid and the text
//! intersect it with each draw's clip, the primitive, image and backdrop passes are scissored to it
//! ([`limit_pass`]). No renderer has to be told; a renderer that begins a pass into the scene calls
//! [`limit_pass`] (or [`combine_clip`]) and is done.
//!
//! It is **unset** (`None`, the whole target) whenever anything else is drawn — a terminal's
//! cached layer, a blur's scratch texture — because those are other textures with other
//! coordinates.

/// Intersection of two logical `[x, y, w, h]` rects. Disjoint rects collapse to
/// a zero-area rect (width/height clamped at 0, never negative) so callers can
/// cull with a simple `w <= 0.0 || h <= 0.0` check.
pub(crate) fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

/// The frame's damage, in logical and in physical pixels.
#[derive(Clone, Copy)]
struct FrameDamage {
    logical: [f32; 4],
    physical: (u32, u32, u32, u32),
}

thread_local! {
    static FRAME_DAMAGE: std::cell::Cell<Option<FrameDamage>> = const { std::cell::Cell::new(None) };
}

/// **Repaint only `rect` (logical px) of the scene** until it is set again; `None` repaints all of
/// it. `scale` turns logical into physical pixels and `target` is the scene's size in physical
/// pixels, which the scissor is clamped to.
pub fn set_frame_damage(rect: Option<[f32; 4]>, scale: f32, target: [u32; 2]) {
    let damage = rect.map(|logical| FrameDamage {
        logical,
        physical: scissor_px(logical, scale, target),
    });
    FRAME_DAMAGE.with(|d| d.set(damage));
}

/// The frame's damage in logical pixels, `None` when the whole scene is repainted.
pub(crate) fn frame_damage() -> Option<[f32; 4]> {
    FRAME_DAMAGE.with(|d| d.get()).map(|d| d.logical)
}

/// **Limit `pass` to the frame's damage**, and to `clip` (physical px) when given. Returns `false`
/// when nothing of the pass can show — the caller draws nothing.
pub(crate) fn limit_pass(
    pass: &mut wgpu::RenderPass<'_>,
    clip: Option<(u32, u32, u32, u32)>,
) -> bool {
    let damage = FRAME_DAMAGE.with(|d| d.get()).map(|d| d.physical);
    let rect = match (damage, clip) {
        (None, None) => return true,
        (Some(r), None) | (None, Some(r)) => r,
        (Some(a), Some(b)) => {
            let (x0, y0) = (a.0.max(b.0), a.1.max(b.1));
            let (x1, y1) = ((a.0 + a.2).min(b.0 + b.2), (a.1 + a.3).min(b.1 + b.3));
            (x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
        }
    };
    if rect.2 == 0 || rect.3 == 0 {
        return false;
    }
    pass.set_scissor_rect(rect.0, rect.1, rect.2, rect.3);
    true
}

/// Convert a logical clip rect to a physical scissor rect clamped to the
/// framebuffer `target`: `(x, y, w, h)`. A zero `w`/`h` means "fully clipped".
pub(crate) fn scissor_px(c: [f32; 4], scale: f32, target: [u32; 2]) -> (u32, u32, u32, u32) {
    let (fw, fh) = (target[0] as f32, target[1] as f32);
    let x0 = (c[0] * scale).floor().clamp(0.0, fw);
    let y0 = (c[1] * scale).floor().clamp(0.0, fh);
    let x1 = ((c[0] + c[2]) * scale).ceil().clamp(0.0, fw);
    let y1 = ((c[1] + c[3]) * scale).ceil().clamp(0.0, fh);
    (
        x0 as u32,
        y0 as u32,
        (x1 - x0).max(0.0) as u32,
        (y1 - y0).max(0.0) as u32,
    )
}

/// Combine the frame damage region with a per-draw clip. `None` means
/// "unbounded": an unbounded side passes the other side through unchanged, and
/// two bounded sides intersect.
pub(crate) fn combine_clip(damage: Option<[f32; 4]>, clip: Option<[f32; 4]>) -> Option<[f32; 4]> {
    match (damage, clip) {
        (None, None) => None,
        (Some(r), None) | (None, Some(r)) => Some(r),
        (Some(a), Some(b)) => Some(intersect(a, b)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersect_overlapping_and_disjoint() {
        // Overlap → the shared box.
        assert_eq!(
            intersect([0.0, 0.0, 100.0, 100.0], [40.0, 30.0, 100.0, 100.0]),
            [40.0, 30.0, 60.0, 70.0],
        );
        // Disjoint → zero area (clamped, never negative).
        let r = intersect([0.0, 0.0, 10.0, 10.0], [50.0, 50.0, 10.0, 10.0]);
        assert_eq!((r[2], r[3]), (0.0, 0.0), "disjoint rects cull to nothing");
    }

    #[test]
    fn combine_clip_pairs() {
        let a = [0.0, 0.0, 100.0, 100.0];
        let b = [40.0, 30.0, 100.0, 100.0];
        assert_eq!(
            combine_clip(None, None),
            None,
            "unbounded ∩ unbounded = unbounded"
        );
        assert_eq!(
            combine_clip(Some(a), None),
            Some(a),
            "one side unbounded passes through"
        );
        assert_eq!(
            combine_clip(None, Some(b)),
            Some(b),
            "one side unbounded passes through"
        );
        assert_eq!(
            combine_clip(Some(a), Some(b)),
            Some(intersect(a, b)),
            "both bounded ⇒ intersect"
        );
    }

    #[test]
    fn cull_skips_only_provably_outside() {
        // A draw whose bbox misses the redraw region is culled; one that touches
        // it (even at the edge) is kept. Mirrors the cull check in the renderers.
        let region = [200.0, 200.0, 100.0, 100.0];
        let outside = [0.0, 0.0, 50.0, 50.0];
        let touching = [250.0, 250.0, 100.0, 100.0];
        let i_out = intersect(region, outside);
        let i_touch = intersect(region, touching);
        assert!(i_out[2] <= 0.0 || i_out[3] <= 0.0, "outside draw is culled");
        assert!(
            i_touch[2] > 0.0 && i_touch[3] > 0.0,
            "overlapping draw is kept"
        );
    }
}
