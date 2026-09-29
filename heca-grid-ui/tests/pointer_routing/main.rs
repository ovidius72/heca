//! **What a raw pointer stream means**, held to the answers the design settled on.
//!
//! The framework resolves the stream once: hit-test, hover transitions, press/release pairing,
//! click runs, drag thresholds. Every widget in the library reads those answers instead of working
//! them out, so a mistake here is a mistake in all of them at once — which is exactly why the
//! rules are written down as tests rather than as comments.
//!
//! The edge cases below are not hypothetical. Each one is a way the hand-rolled versions went
//! wrong: a leave suppressed by a neighbour's click, a first click swallowed while something
//! waited to see whether a second was coming, a right-click that also fired a left one, a press
//! whose widget was gone before the release, a container that ate an event on its way past.

#[path = "../common/mod.rs"]
mod common;

use common::{click_at, press_at, release_at};
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::{Dialog, DockFrame, FocusScope, Overlay, ScrollRegion, Select};
use heca_grid_ui::{
    Base, Component, Event, EventKind, Handled, LayoutEngine, Point, PointerButton, Rectangle, Size,
};
use std::cell::RefCell;
use std::rc::Rc;

// ── a probe that records the vocabulary ───────────────────────────────────────────────────────

/// Records every resolved event that reaches it, by name. Consumes nothing, so it never changes
/// what would have happened without it.
struct Probe {
    base: Base,
    seen: Seen,
    consume: Option<EventKind>,
}

impl Probe {
    fn new(w: f32, h: f32) -> (Self, Seen) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut base = Base::new();
        base.style.layout.width = Length::Px(w);
        base.style.layout.height = Length::Px(h);
        (
            Self {
                base,
                seen: seen.clone(),
                consume: None,
            },
            seen,
        )
    }

    /// The same probe, but it takes one kind of event — for the tests about what happens after
    /// something claims one.
    fn consuming(mut self, kind: EventKind) -> Self {
        self.consume = Some(kind);
        self
    }
}

impl Component for Probe {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn on_event(&mut self, ev: &Event) -> Handled {
        let name = format!("{:?}", ev.kind());
        self.seen.borrow_mut().push(name);
        if self.consume == Some(ev.kind()) {
            Handled::Yes
        } else {
            Handled::No
        }
    }
}

fn kinds(seen: &Seen) -> Vec<String> {
    seen.borrow().clone()
}

/// The same, without the two events every widget gets whatever happens — mounting, and the moves
/// that carry the pointer around — so a test can state the shape of a gesture rather than a
/// transcript of it.
fn meaningful(seen: &Seen) -> Vec<String> {
    seen.borrow()
        .iter()
        .filter(|k| *k != "Mount" && *k != "PointerMove")
        .cloned()
        .collect()
}

fn mv(root: &mut dyn Component, pos: Point) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_moved(pos));
}

/// What a probe records, shared with the test that mounted it.
type Seen = Rc<RefCell<Vec<String>>>;

/// Two probes side by side, laid out: `[0]` at x 0..100, `[1]` at x 100..200, both 0..50 tall.
fn two_probes() -> (Box<dyn Component>, Seen, Seen) {
    let (a, seen_a) = Probe::new(100.0, 50.0);
    let (b, seen_b) = Probe::new(100.0, 50.0);
    let mut root: Box<dyn Component> = Box::new(Flex::row().child(a).child(b));
    LayoutEngine::new().compute(root.as_mut(), Size::new(200.0, 50.0));
    (root, seen_a, seen_b)
}

const LEFT: Point = Point { x: 10.0, y: 10.0 };
const RIGHT: Point = Point { x: 150.0, y: 10.0 };

mod button_ownership;
mod capture;
mod clicks;
mod containers;
mod handlers;
mod hover;
mod wheel;
