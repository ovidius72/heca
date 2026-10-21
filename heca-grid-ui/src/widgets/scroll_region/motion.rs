//! Motion for ScrollRegion.

use super::*;

impl ScrollRegion {
    /// Bake the current `scroll_offset` into the children's bounds. Shifts each
    /// direct child's subtree by `applied_offset − scroll_offset` so the bounds
    /// end at `natural − scroll_offset` (the visual position). Idempotent when
    /// already in sync. Called from `paint` and `event` so bounds are always
    /// current for drawing, hit-testing, and DnD.
    /// Where the content **wants** to sit: the scroll offset, less the centring pad.
    ///
    /// The pad rides the same shift as the offset, and subtracts: a positive pad moves the content
    /// **down and right**, into the middle of the room it does not fill. The two never fight —
    /// where there is slack the offset is pinned at 0, and where there is none the pad is.
    pub(super) fn desired_shift(&self) -> (f64, f64) {
        let (pad_x, pad_y) = self.center_pad();
        (
            self.scroll_offset_x.get_untracked() as f64 - pad_x,
            self.scroll_offset.get_untracked() as f64 - pad_y,
        )
    }
    /// Clamp a shift so the content can **never leave the viewport**.
    ///
    /// The position of an overscrolling surface is two numbers added together — the centring pad
    /// and the scroll offset — and bounding each of them separately does not bound their sum. Twice
    /// now that sum has carried the whole map off screen, leaving an empty window and no way back
    /// but the keyboard. This is the invariant that cannot be violated by any combination of them:
    /// a quarter of the viewport's worth of content stays on screen, whatever the two parts say.
    pub(super) fn clamp_visible(&self, shift: (f64, f64)) -> (f64, f64) {
        let vp = self.base.bounds.size;
        let (content_w, content_h) = self.raw_extent();
        let clamp = |s: f64, viewport: f64, content: f64| {
            if viewport <= 0.0 || content <= 0.0 {
                return s;
            }
            let keep = (viewport * 0.25).min(content);
            s.clamp(keep - viewport, content - keep)
        };
        (
            clamp(shift.0, vp.w, content_w),
            clamp(shift.1, vp.h, content_h),
        )
    }
    pub(super) fn sync_shift(&mut self) {
        let desired = self.clamp_visible(self.desired_shift());
        // Smoothing puts the *eased* value on screen and lets `tick` walk it to `desired`; without
        // it the desired value goes straight on. The first sync snaps either way.
        let (target_x, target_y) = match self.eased.as_mut() {
            None => desired,
            Some((ex, ey)) => {
                if !self.eased_seeded {
                    self.eased_seeded = true;
                    ex.snap(desired.0);
                    ey.snap(desired.1);
                }
                ex.set(desired.0);
                ey.set(desired.1);
                (ex.value(), ey.value())
            }
        };
        let dy = self.applied_offset - target_y;
        let dx = self.applied_offset_x - target_x;
        if dx != 0.0 || dy != 0.0 {
            for child in self.base.children.iter_mut() {
                shift_subtree(child.as_mut(), dx, dy);
            }
            self.applied_offset = target_y;
            self.applied_offset_x = target_x;
            self.base.mark_needs_paint();
        }
    }
    /// Walk the on-screen shift toward [`desired_shift`](Self::desired_shift), returning `true`
    /// while it still has ground to cover (which is what keeps frames coming).
    ///
    /// The motion itself is [`Eased`]'s, in `effects.rs` beside `Flash` and `Attention` — the same
    /// value-plus-`tick` shape every animated widget here embeds.
    pub(super) fn advance_eased(&mut self, dt: f32) -> bool {
        let (dx, dy) = self.clamp_visible(self.desired_shift());
        let Some((ex, ey)) = self.eased.as_mut() else {
            return false;
        };
        ex.set(dx);
        ey.set(dy);
        let moving = ex.tick(dt) | ey.tick(dt);
        self.sync_shift();
        moving
    }
}
