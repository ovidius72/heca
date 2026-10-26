//! Scroll for ScrollRegion.

use super::*;

impl ScrollRegion {
    /// The reactive vertical scroll offset (content px). Read or drive it from the
    /// host: `region.scroll_offset().get_untracked()` / `.set(v)`. Use
    /// [`scroll_to`](Self::scroll_to) to set with clamping.
    pub fn scroll_offset(&self) -> Signal<f32> {
        self.scroll_offset
    }
    /// The reactive **horizontal** scroll offset (content px). Only meaningful when
    /// [`axes`](Self::axes) includes horizontal. Use [`scroll_to_x`](Self::scroll_to_x)
    /// to set with clamping.
    pub fn scroll_offset_x(&self) -> Signal<f32> {
        self.scroll_offset_x
    }
    /// Set the horizontal scroll offset, clamped to `[0, max_offset_x]`, bake it
    /// into the children's bounds, and request a repaint. Returns the clamped value.
    /// Pauses layout-only following until navigation or typing, including across resize.
    pub fn scroll_to_x(&mut self, offset: f32) -> f32 {
        let before = self.scroll_offset_x.get_untracked();
        let applied = self.set_offset_x(offset, None);
        self.reveal_paused |= applied != before;
        applied
    }
    /// Set the scroll offset, clamped to `[0, max_offset]`, bake it into the
    /// children's bounds immediately, and request a repaint. Returns the clamped
    /// value actually applied. Prefer this over raw `scroll_offset().set()` —
    /// it keeps the shifted bounds (used for paint, hit-testing, and DnD) in
    /// sync with the offset in the same call.
    /// Pauses layout-only following until navigation or typing, including across resize.
    pub fn scroll_to(&mut self, offset: f32) -> f32 {
        let before = self.scroll_offset.get_untracked();
        let applied = self.set_offset_y(offset, None);
        self.reveal_paused |= applied != before;
        applied
    }
    /// The **only** way the vertical offset moves: clamp, bake the shift, report.
    ///
    /// Every path funnels through here — wheel, thumb drag, held track press, scroll-into-view and
    /// the public API — so a listener cannot miss a movement, and a new way to scroll cannot forget
    /// to announce itself. `cause` is the input that did it, or `None` when the host did.
    pub(super) fn set_offset_y(&mut self, offset: f32, cause: Option<&Event>) -> f32 {
        let (min, max) = self.offset_bounds_y();
        let v = offset.clamp(min as f32, max as f32);
        let changed = v != self.scroll_offset.get_untracked();
        self.scroll_offset.set(v);
        self.snap_if_driven(cause);
        self.sync_shift();
        if changed {
            self.reveal_paused |= cause.is_some();
            self.report_scroll(cause);
        }
        v
    }
    /// The horizontal counterpart of [`set_offset_y`](Self::set_offset_y).
    pub(super) fn set_offset_x(&mut self, offset: f32, cause: Option<&Event>) -> f32 {
        let (min, max) = self.offset_bounds_x();
        let v = offset.clamp(min as f32, max as f32);
        let changed = v != self.scroll_offset_x.get_untracked();
        self.scroll_offset_x.set(v);
        self.snap_if_driven(cause);
        self.sync_shift();
        if changed {
            self.reveal_paused |= cause.is_some();
            self.report_scroll(cause);
        }
        v
    }
    /// A movement the user is driving **with the pointer** lands immediately: a thumb drag has to
    /// track the finger one to one, and a bar that lags it by even a few frames feels broken.
    ///
    /// A **wheel notch is not that**. It is a discrete step — a tenth of a viewport at a time — so
    /// applying it instantly is a jump, and a run of them is a run of jumps. Eased, the same notches
    /// read as one continuous movement. So the wheel is deliberately *not* snapped here even though
    /// it is user input: what has to track the input is the thing under the finger, and a wheel has
    /// nothing under it.
    pub(super) fn snap_if_driven(&mut self, cause: Option<&Event>) {
        if !matches!(cause, Some(Event::PointerMove(_) | Event::PointerDown(_))) {
            return;
        }
        let (x, y) = self.desired_shift();
        if let Some((ex, ey)) = self.eased.as_mut() {
            ex.snap(x);
            ey.snap(y);
        }
    }
    /// Report a movement: `scroll_start` if this began one, then `scroll` — and arm the idle timer
    /// that ends a wheel gesture, which has no release to end it.
    pub(super) fn report_scroll(&mut self, cause: Option<&Event>) {
        if !self.scrolling {
            self.scrolling = true;
            self.fire(self.on_scroll_start.as_deref(), cause);
        }
        // A gesture with a release ends on that release; a wheel ends on silence.
        self.wheel_idle = matches!(cause, Some(Event::Scroll(_)) | None).then_some(WHEEL_IDLE_END);
        self.fire(self.on_scroll.as_deref(), cause);
    }
    /// Act on a keyboard scroll intent, or decline it.
    ///
    /// Declines (`Handled::No`) when the intent is not a scroll one, or when this region cannot
    /// scroll that axis at all — either the axis is disabled or the content fits. Declining is what
    /// lets a nested region, or the host, get a turn instead of the key dying here.
    pub(super) fn scroll_intent(&mut self, intent: WidgetIntent, cause: &Event) -> Handled {
        let vp = self.base.bounds;
        let (vertical, target) = match intent {
            WidgetIntent::ScrollPageUp => (
                true,
                self.scroll_offset.get_untracked() as f64 - PAGE_STEP_FRAC * vp.size.h,
            ),
            WidgetIntent::ScrollPageDown => (
                true,
                self.scroll_offset.get_untracked() as f64 + PAGE_STEP_FRAC * vp.size.h,
            ),
            WidgetIntent::ScrollToTop => (true, 0.0),
            // Past the end on purpose: `set_offset_y` clamps, so this lands exactly on the last
            // scrollable pixel without this code having to know where that is.
            WidgetIntent::ScrollToBottom => (true, f64::MAX),
            WidgetIntent::ScrollPageLeft => (
                false,
                self.scroll_offset_x.get_untracked() as f64 - PAGE_STEP_FRAC * vp.size.w,
            ),
            WidgetIntent::ScrollPageRight => (
                false,
                self.scroll_offset_x.get_untracked() as f64 + PAGE_STEP_FRAC * vp.size.w,
            ),
            WidgetIntent::ScrollToLeftEdge => (false, 0.0),
            WidgetIntent::ScrollToRightEdge => (false, f64::MAX),
            _ => return Handled::No,
        };
        let room = if vertical {
            self.max_offset()
        } else {
            self.max_offset_x()
        };
        if room <= 0.0 {
            return Handled::No;
        }
        let target = target.clamp(0.0, room) as f32;
        if vertical {
            self.set_offset_y(target, Some(cause));
        } else {
            self.set_offset_x(target, Some(cause));
        }
        self.end_scroll(Some(cause));
        Handled::Yes
    }
    /// Declare the gesture over and report it once. No-op when nothing was in flight.
    pub(super) fn end_scroll(&mut self, cause: Option<&Event>) {
        if !self.scrolling {
            return;
        }
        self.scrolling = false;
        self.wheel_idle = None;
        self.fire(self.on_scroll_end.as_deref(), cause);
    }
    /// Hand one listener the current position. Silent when nobody is listening — the region drives
    /// itself either way, which is the whole point of these being optional.
    pub(super) fn fire(&self, listener: Option<&dyn for<'a> Fn(ScrollInfo<'a>)>, cause: Option<&Event>) {
        let Some(listener) = listener else { return };
        listener(ScrollInfo {
            offset_x: self.scroll_offset_x.get_untracked(),
            offset_y: self.scroll_offset.get_untracked(),
            max_x: self.max_offset_x() as f32,
            max_y: self.max_offset() as f32,
            content: Size::new(self.content_extent_x(), self.content_extent()),
            viewport: self.base.bounds.size,
            event: cause,
        });
    }
    /// Scroll by `delta` content px (signed: positive = down), clamped to
    /// `[0, max_offset]`. The wheel and click-track paging go through here; host
    /// scroll actions may call it too.
    pub(super) fn scroll_by(&mut self, delta: f64, cause: Option<&Event>) {
        let next = self.scroll_offset.get_untracked() as f64 + delta;
        self.set_offset_y(next as f32, cause);
    }
    /// End any in-flight thumb drag or held track press. `true` if one was in flight.
    ///
    /// The single definition of "the gesture is over", so the release path and the stale-grab
    /// recovery on the next press cannot drift apart.
    pub(super) fn release_grabs(&mut self) -> bool {
        let ended = self.thumb_grab.take().is_some()
            | self.h_thumb_grab.take().is_some()
            | self.track_repeat.take().is_some();
        if ended {
            // The thumb may go from bright to rest if the cursor is no longer over the lane.
            self.base.mark_needs_paint();
        }
        ended
    }
    /// One track-paging step **toward `pos`** on the given axis: a screenful in
    /// the cursor's direction relative to the thumb, and a no-op once the thumb
    /// has reached the cursor (the classic pause-under-the-pointer behavior).
    /// Shared by the initial track press and each held-press repeat.
    pub(super) fn page_toward(&mut self, horizontal: bool, pos: Point, cause: Option<&Event>) {
        if horizontal {
            if let Some(thumb) = self.h_thumb_rect() {
                let dir = if pos.x < thumb.loc.x {
                    -1.0
                } else if pos.x > thumb.loc.x + thumb.size.w {
                    1.0
                } else {
                    return; // thumb reached the cursor — pause
                };
                let next =
                    self.scroll_offset_x.get_untracked() as f64 + dir * self.base.bounds.size.w;
                self.set_offset_x(next as f32, cause);
            }
        } else if let Some(thumb) = self.thumb_rect() {
            let dir = if pos.y < thumb.loc.y {
                -1.0
            } else if pos.y > thumb.loc.y + thumb.size.h {
                1.0
            } else {
                return; // thumb reached the cursor — pause
            };
            self.scroll_by(dir * self.base.bounds.size.h, cause);
        }
    }
}
