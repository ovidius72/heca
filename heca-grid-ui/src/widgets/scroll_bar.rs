//! [`ScrollBar`] — a standalone vertical scrollbar widget.
//!
//! A generic viewport control: the host supplies a total content extent,
//! visible viewport extent, and current offset **from the top**; the widget
//! renders a thumb and reports new offsets as the user clicks or drags.
//! Visual language intentionally matches [`ScrollRegion`](crate::widgets::ScrollRegion)'s
//! accent thumb so hosts don't get a second scrollbar style.

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, Handled, PaintCx};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::Glow;
use crate::style::{Direction, Length};
use heca_core::layout::{Point, Rectangle, Size};

/// Visible thumb width (logical px).
const THUMB_W: f64 = 3.0;
/// Offset from the right edge so the thumb sits just inside the pane border.
const THUMB_RIGHT_INSET: f64 = 1.5;
/// Minimum thumb height so long histories stay grabbable.
const MIN_THUMB: f64 = 24.0;
/// Glow intensity at rest.
const GLOW_REST_INTENSITY: f32 = 0.015;
/// Glow intensity while hovered/dragging.
const GLOW_ACTIVE_INTENSITY: f32 = 0.035;

/// Standalone vertical scrollbar: content extent, viewport extent, and offset.
pub struct ScrollBar {
    base: Base,
    content_extent: Signal<f32>,
    viewport_extent: Signal<f32>,
    offset: Signal<f32>,
    drag_grab: Option<f64>,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Default for ScrollBar {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollBar {
    /// A new scrollbar. Width is fixed; hosts set the height via layout/bounds.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        base.style.layout.width = Length::Px(8.0);
        Self {
            base,
            content_extent: signal(0.0),
            viewport_extent: signal(0.0),
            offset: signal(0.0),
            drag_grab: None,
            on_change: None,
        }
    }

    /// Total content extent in abstract units (rows, px, items).
    pub fn content_extent_signal(&self) -> Signal<f32> {
        self.content_extent
    }

    /// Visible viewport extent in the same units.
    pub fn viewport_extent_signal(&self) -> Signal<f32> {
        self.viewport_extent
    }

    /// Current offset from the **top** of the content.
    pub fn offset_signal(&self) -> Signal<f32> {
        self.offset
    }

    /// Report value changes. Emits `Action::value("scrollbar-change", Float(offset))`.
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    fn track_rect(&self) -> Rectangle {
        self.base.bounds
    }

    fn max_offset(&self) -> f64 {
        let content = self.content_extent.get_untracked() as f64;
        let viewport = self.viewport_extent.get_untracked() as f64;
        (content - viewport).max(0.0)
    }

    fn clamped_offset(&self) -> f64 {
        (self.offset.get_untracked() as f64).clamp(0.0, self.max_offset())
    }

    fn scrollable(&self) -> bool {
        self.max_offset() > 0.0 && self.base.bounds.size.h > 0.0
    }

    fn thumb_rect(&self) -> Option<Rectangle> {
        if !self.scrollable() {
            return None;
        }
        let track = self.track_rect();
        let content = self.content_extent.get_untracked() as f64;
        let viewport = self.viewport_extent.get_untracked() as f64;
        let thumb_h = ((viewport / content) * track.size.h)
            .max(MIN_THUMB)
            .min(track.size.h.max(0.0));
        let max_off = self.max_offset();
        let frac = if max_off > 0.0 {
            (self.clamped_offset() / max_off).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_y = track.loc.y + (track.size.h - thumb_h) * frac;
        let thumb_x = track.loc.x + track.size.w - THUMB_W - THUMB_RIGHT_INSET;
        Some(Rectangle::new(
            Point::new(thumb_x, thumb_y),
            Size::new(THUMB_W, thumb_h),
        ))
    }

    fn emit_offset(&self, offset: f64) {
        if let Some(f) = &self.on_change {
            f(Action::value(
                "scrollbar-change",
                SignalData::Float(offset.clamp(0.0, self.max_offset())),
            ));
        }
    }

    fn set_offset(&mut self, offset: f64) {
        let clamped = offset.clamp(0.0, self.max_offset()) as f32;
        if (self.offset.get_untracked() - clamped).abs() > f32::EPSILON {
            self.offset.set(clamped);
            self.emit_offset(clamped as f64);
        }
    }

    fn set_from_thumb_top(&mut self, thumb_top: f64) {
        let Some(thumb) = self.thumb_rect() else {
            return;
        };
        let track = self.track_rect();
        let thumb_range = (track.size.h - thumb.size.h).max(0.0);
        let frac = if thumb_range > 0.0 {
            ((thumb_top - track.loc.y) / thumb_range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.set_offset(frac * self.max_offset());
    }

}

impl Component for ScrollBar {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let Some(thumb) = self.thumb_rect() else {
            return;
        };
        let active = self.base.hovered() || self.drag_grab.is_some();
        let accent = cx.accent();
        let glow = Some(Glow {
            color: accent,
            radius: 10.0,
            intensity: if active {
                GLOW_ACTIVE_INTENSITY
            } else {
                GLOW_REST_INTENSITY
            },
        });
        cx.rect(
            thumb,
            accent.with_alpha(if active {
                cx.theme().colors.interaction.thumb_hover
            } else {
                cx.theme().colors.interaction.thumb_rest
            }),
            None,
            cx.theme().colors.control_radius(),
            glow,
        );
    }

    /// Capture, not bubble: a thumb grab is a gesture, and a gesture beats whatever happens to sit
    /// under the cursor.
    ///
    /// **The grab does not have to arrange to hear the rest of itself.** Consuming the press
    /// captures the pointer, so every move and the release come here wherever the cursor goes —
    /// which is the whole of what this widget used to hand-roll by ignoring positions while
    /// `drag_grab` was set, and the exact rule a host got wrong by gating a release on position.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.base.visible.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerMove(p) => {
                if let Some(grab) = self.drag_grab {
                    self.set_from_thumb_top(p.pos.y - grab);
                    return Handled::Yes;
                }
                Handled::No
            }
            Event::PointerDown(p) if self.scrollable() => {
                let thumb = self.thumb_rect().expect("scrollable -> thumb");
                if thumb.contains(p.pos) {
                    self.drag_grab = Some(p.pos.y - thumb.loc.y);
                } else {
                    self.drag_grab = Some(thumb.size.h / 2.0);
                    self.set_from_thumb_top(p.pos.y - thumb.size.h / 2.0);
                }
                Handled::Yes
            }
            Event::PointerUp(_) if self.drag_grab.is_some() => {
                self.drag_grab = None;
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}

impl LayoutExt for ScrollBar {}

#[cfg(test)]
mod tests {
    use crate::event::PointerButton;
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn scrollbar(content: f32, viewport: f32, offset: f32) -> ScrollBar {
        let s = ScrollBar::new();
        s.content_extent_signal().set(content);
        s.viewport_extent_signal().set(viewport);
        s.offset_signal().set(offset);
        let mut s = s;
        s.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(16.0, 100.0));
        s
    }

    #[test]
    fn thumb_scales_with_visible_fraction() {
        let s = scrollbar(200.0, 50.0, 0.0);
        let t = s.thumb_rect().expect("scrollable -> thumb");
        assert!((t.size.h - 25.0).abs() < 1e-6);
        assert!((t.loc.y - 0.0).abs() < 1e-6);
    }

    #[test]
    fn thumb_reaches_bottom_at_max_offset() {
        let s = scrollbar(200.0, 50.0, 150.0);
        let t = s.thumb_rect().expect("scrollable -> thumb");
        assert!((t.loc.y - 75.0).abs() < 1e-6);
    }

    #[test]
    fn press_and_drag_emit_changed_offset() {
        let seen: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
        let out = seen.clone();
        let mut s = ScrollBar::new().on_change(move |a| {
            if let SignalData::Float(v) = a.data {
                out.borrow_mut().push(v);
            }
        });
        s.content_extent_signal().set(200.0);
        s.viewport_extent_signal().set(50.0);
        s.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(16.0, 100.0));
        assert_eq!(
            crate::component::dispatch(&mut s, &Event::pointer_pressed(Point::new(8.0, 60.0), PointerButton::Left)),
            Handled::Yes
        );
        assert!(!seen.borrow().is_empty(), "track click emits a new offset");
        let _ = crate::component::dispatch(&mut s, &Event::pointer_moved(Point::new(8.0, 90.0)));
        assert!(seen.borrow().last().copied().unwrap_or_default() > 0.0);
    }
}
