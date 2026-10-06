//! [`Splitter`] — **the edge between two neighbours that a drag moves.**
//!
//! It paints nothing: it is a grab zone placed over the gap between two things, with the resize
//! cursor, that tells whoever placed it how far the pointer has moved while it is held. The owner
//! knows what the two neighbours are and what moving the edge means ([`on_resize`](Splitter::on_resize)
//! hands it the distance and nothing else); the splitter knows nothing of columns, panes or
//! indices, so the line a plugin writes is the same one.
//!
//! Where it sits in the tree is where it is on screen: a widget laid over it covers it, so a
//! floating panel above an edge is not grabbed through, and the cursor over that panel is the
//! panel's.

use crate::builders::LayoutExt;
use crate::component::{Base, Component};
use crate::cursor::Cursor;
use crate::event::{Event, Handled, PointerButton};
use crate::style::Space;
use crate::style::Spacing;
use crate::widgets::Orientation;
use heca_core::layout::{Point, Rectangle, Size};

/// How far past its own box the grab zone reaches on each side unless [`grab`](Splitter::grab) says
/// otherwise — a font-relative step, so a thin gap stays a reachable target at any zoom.
const GRAB: Spacing = Spacing::Xs;

/// What is told how far the edge moved, in logical px along the drag axis.
type Moved = dyn FnMut(f32);

/// A grab zone that resizes. See the [module docs](self).
pub struct Splitter {
    base: Base,
    orientation: Orientation,
    /// Where the pointer was at the last move of the drag in flight, along the drag axis.
    last: Option<f64>,
    on_resize: Option<Box<Moved>>,
    /// How far the grab zone reaches past the box on each side.
    grab: Space,
}

impl Splitter {
    fn with(orientation: Orientation) -> Self {
        let mut base = Base::new();
        base.cursor = Some(match orientation {
            Orientation::Vertical => Cursor::ResizeHorizontal,
            Orientation::Horizontal => Cursor::ResizeVertical,
        });
        Self {
            base,
            orientation,
            last: None,
            on_resize: None,
            grab: GRAB.into(),
        }
    }

    /// An edge that runs **up and down** — between two things side by side. It moves left and
    /// right.
    pub fn vertical() -> Self {
        Self::with(Orientation::Vertical)
    }

    /// An edge that runs **left and right** — between two things stacked. It moves up and down.
    pub fn horizontal() -> Self {
        Self::with(Orientation::Horizontal)
    }

    /// **Told how far it was dragged**, in logical px along the axis it moves, each time the
    /// pointer moves while it is held. What moving the edge means is the owner's.
    pub fn on_resize(mut self, f: impl FnMut(f32) + 'static) -> Self {
        self.on_resize = Some(Box::new(f));
        self
    }

    /// **How far the grab zone reaches past the box** on each side — a step of the theme's spacing
    /// (or pixels), so it scales with the font like every other space.
    pub fn grab(mut self, reach: impl Into<Space>) -> Self {
        self.grab = reach.into();
        self
    }

    fn along(&self, p: Point) -> f64 {
        match self.orientation {
            Orientation::Vertical => p.x,
            Orientation::Horizontal => p.y,
        }
    }
}

impl Component for Splitter {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The grab zone: the box, widened by its [`grab`](Splitter::grab) across the edge.
    fn hit_bounds(&self) -> Option<Rectangle> {
        let b = self.base.bounds;
        let outset = self.grab.resolve(self.base.font) as f64;
        Some(match self.orientation {
            Orientation::Vertical => Rectangle::new(
                Point::new(b.loc.x - outset, b.loc.y),
                Size::new(b.size.w + 2.0 * outset, b.size.h),
            ),
            Orientation::Horizontal => Rectangle::new(
                Point::new(b.loc.x, b.loc.y - outset),
                Size::new(b.size.w, b.size.h + 2.0 * outset),
            ),
        })
    }

    /// A press takes the edge and consumes it, which captures the pointer: every move and the
    /// release come here wherever the pointer goes, so the drag cannot outlive the button.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        match ev {
            Event::PointerDown(p) if p.button == PointerButton::Left => {
                self.last = Some(self.along(p.pos));
                Handled::Yes
            }
            Event::PointerMove(p) => {
                let Some(last) = self.last else {
                    return Handled::No;
                };
                let now = self.along(p.pos);
                self.last = Some(now);
                let moved = (now - last) as f32;
                if moved != 0.0
                    && let Some(f) = self.on_resize.as_mut()
                {
                    f(moved);
                }
                Handled::Yes
            }
            Event::PointerUp(_) if self.last.take().is_some() => Handled::Yes,
            _ => Handled::No,
        }
    }
}

impl LayoutExt for Splitter {}

#[cfg(test)]
mod tests;
