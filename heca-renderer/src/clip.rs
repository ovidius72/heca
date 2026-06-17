//! Shared logical-rect clip helpers for the render passes.
//!
//! [`intersect`] and [`combine_clip`] operate on logical `[x, y, w, h]` rects
//! (empty — zero area — when disjoint). [`combine_clip`] folds the frame damage
//! region together with a per-draw clip; `None` means "unbounded". Both are used
//! by [`crate::grid`] and [`crate::text`] to cull/sissor draw commands, so they
//! live here once instead of being duplicated per renderer.

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
        assert_eq!(combine_clip(None, None), None, "unbounded ∩ unbounded = unbounded");
        assert_eq!(combine_clip(Some(a), None), Some(a), "one side unbounded passes through");
        assert_eq!(combine_clip(None, Some(b)), Some(b), "one side unbounded passes through");
        assert_eq!(combine_clip(Some(a), Some(b)), Some(intersect(a, b)), "both bounded ⇒ intersect");
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
        assert!(i_touch[2] > 0.0 && i_touch[3] > 0.0, "overlapping draw is kept");
    }
}